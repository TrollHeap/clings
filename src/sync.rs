//! Synchronisation multi-machine via un dépôt Git privé.
//!
//! Seul `progress.json` est versionné — le SQLite reste local.
//! Résolution de conflits : MAX score par sujet (aucune perte possible).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use rusqlite::Connection;

use crate::config::SyncConfig;
use crate::constants::{
    SYNC_DEFAULT_BRANCH, SYNC_GITIGNORE_CONTENT, SYNC_GIT_TIMEOUT_SECS, SYNC_SNAPSHOT_FILENAME,
};
use crate::error::{KfError, Result};
use crate::progress;

// ── Types publics ─────────────────────────────────────────────────────────────

/// État du sync retourné par `status()`.
pub struct SyncStatus {
    pub enabled: bool,
    pub remote: String,
    pub branch: String,
    pub last_commit: Option<String>,
    pub subject_count: usize,
}

// ── API publique ──────────────────────────────────────────────────────────────

/// Initialise `clings_dir` comme dépôt Git, push initial, active sync dans config.
pub fn init(remote: &str, clings_dir: &Path) -> Result<()> {
    ensure_git_installed()?;

    // git init (idempotent)
    git(clings_dir, &["init"]).map_err(|e| KfError::Sync(format!("git init failed: {e}")))?;

    // .gitignore
    let gitignore = clings_dir.join(".gitignore");
    std::fs::write(&gitignore, SYNC_GITIGNORE_CONTENT)?;

    // Snapshot initial
    write_snapshot(clings_dir, &progress::open_db()?)?;

    // Commit initial
    git(clings_dir, &["add", "progress.json", ".gitignore"])
        .map_err(|e| KfError::Sync(format!("git add failed: {e}")))?;

    // Vérifier s'il y a quelque chose à committer
    let nothing_to_commit = git_output(clings_dir, &["status", "--porcelain"])
        .map(|out| out.trim().is_empty())
        .unwrap_or(false);

    if !nothing_to_commit {
        git(clings_dir, &["commit", "-m", "init: clings sync"])
            .map_err(|e| KfError::Sync(format!("git commit failed: {e}")))?;
    }

    // Remote
    let has_remote = git_output(clings_dir, &["remote"])
        .map(|out| out.lines().any(|l| l == "origin"))
        .unwrap_or(false);

    if has_remote {
        git(clings_dir, &["remote", "set-url", "origin", remote])
            .map_err(|e| KfError::Sync(format!("git remote set-url failed: {e}")))?;
    } else {
        git(clings_dir, &["remote", "add", "origin", remote])
            .map_err(|e| KfError::Sync(format!("git remote add failed: {e}")))?;
    }

    // Rename branch à main si nécessaire (git init crée parfois "master")
    let current_branch = git_output(clings_dir, &["branch", "--show-current"]).unwrap_or_default();
    let current_branch = current_branch.trim();
    if current_branch != SYNC_DEFAULT_BRANCH && !current_branch.is_empty() {
        git(clings_dir, &["branch", "-M", SYNC_DEFAULT_BRANCH])
            .map_err(|e| KfError::Sync(format!("git branch -M failed: {e}")))?;
    }

    // Push
    git_timeout(
        clings_dir,
        &["push", "-u", "origin", SYNC_DEFAULT_BRANCH],
        Duration::from_secs(SYNC_GIT_TIMEOUT_SECS),
    )
    .map_err(|e| {
        KfError::Sync(format!(
            "git push failed: {e} — vérifiez l'accès SSH/HTTPS au remote"
        ))
    })?;

    // Activer le sync dans la config
    crate::config::set_value("sync", "enabled", "true")?;
    crate::config::set_value("sync", "remote", remote)?;
    crate::config::set_value("sync", "branch", SYNC_DEFAULT_BRANCH)?;

    Ok(())
}

/// Pull depuis le remote, importe `progress.json` si modifié (merge MAX).
/// Retourne le nombre de sujets mis à jour, ou `None` si rien de nouveau.
pub fn pull_and_merge(clings_dir: &Path, conn: &mut Connection) -> Result<Option<usize>> {
    if !is_git_repo(clings_dir) {
        return Err(KfError::Sync(
            "~/.clings/ n'est pas un dépôt Git — lancez `clings sync init <remote>`".to_string(),
        ));
    }

    // Hash du snapshot avant pull
    let snapshot_path = clings_dir.join(SYNC_SNAPSHOT_FILENAME);
    let hash_before = file_hash(&snapshot_path);

    // Pull (avec timeout)
    git_timeout(
        clings_dir,
        &["pull", "--rebase", "origin"],
        Duration::from_secs(SYNC_GIT_TIMEOUT_SECS),
    )
    .map_err(|e| KfError::Sync(format!("git pull failed: {e}")))?;

    // Vérifier si le snapshot a changé
    let hash_after = file_hash(&snapshot_path);
    if hash_before == hash_after {
        return Ok(None);
    }

    // Importer avec merge MAX
    let json = std::fs::read_to_string(&snapshot_path)?;
    let (count, warnings) = progress::import_progress(conn, &json, false)?;
    for w in &warnings {
        eprintln!("  ⚠ sync: {w}");
    }

    Ok(Some(count))
}

/// Exporte la DB → `progress.json`, commit, push.
/// Séquence complète : export + git add + commit + push.
pub fn export_and_push(clings_dir: &Path, conn: &Connection, cfg: &SyncConfig) -> Result<()> {
    if !is_git_repo(clings_dir) {
        return Err(KfError::Sync(
            "~/.clings/ n'est pas un dépôt Git".to_string(),
        ));
    }

    // Export (nécessite la Connection)
    write_snapshot(clings_dir, conn)?;
    commit_and_push(clings_dir, cfg)
}

/// Commit + push uniquement — suppose que `progress.json` est déjà écrit.
/// Utile pour appel depuis un thread background (`Connection` n'est pas `Send`).
pub fn commit_and_push(clings_dir: &Path, cfg: &SyncConfig) -> Result<()> {
    if !is_git_repo(clings_dir) {
        return Err(KfError::Sync(
            "~/.clings/ n'est pas un dépôt Git".to_string(),
        ));
    }

    // git add
    git(clings_dir, &["add", SYNC_SNAPSHOT_FILENAME])
        .map_err(|e| KfError::Sync(format!("git add failed: {e}")))?;

    // Rien à committer ? (exit 0 = pas de changements stagés, exit 1 = changements présents)
    let nothing_staged = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(clings_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if nothing_staged {
        return Ok(());
    }

    // Commit
    let mut hostname = resolve_hostname(cfg);
    // Sanitize hostname to prevent injection via commit message
    hostname = hostname.replace(['\n', '\0', '"', '\''], "_");
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let msg = format!("sync: {hostname} {timestamp}");
    git(clings_dir, &["commit", "-m", &msg])
        .map_err(|e| KfError::Sync(format!("git commit failed: {e}")))?;

    // Push (avec retry pull --rebase si rejeté)
    let branch = if cfg.branch.is_empty() {
        SYNC_DEFAULT_BRANCH
    } else {
        validate_branch_name(&cfg.branch)?;
        &cfg.branch
    };
    let push_result = git_timeout(
        clings_dir,
        &["push", "origin", branch],
        Duration::from_secs(SYNC_GIT_TIMEOUT_SECS),
    );

    if push_result.is_err() {
        // Retry : pull --rebase puis re-push
        git_timeout(
            clings_dir,
            &["pull", "--rebase", "origin", branch],
            Duration::from_secs(SYNC_GIT_TIMEOUT_SECS),
        )
        .map_err(|e| KfError::Sync(format!("git pull --rebase (retry) failed: {e}")))?;

        git_timeout(
            clings_dir,
            &["push", "origin", branch],
            Duration::from_secs(SYNC_GIT_TIMEOUT_SECS),
        )
        .map_err(|e| KfError::Sync(format!("git push (retry) failed: {e}")))?;
    }

    Ok(())
}

/// Retourne l'état courant du sync.
pub fn status(clings_dir: &Path, cfg: &SyncConfig) -> Result<SyncStatus> {
    let last_commit = if is_git_repo(clings_dir) {
        git_output(clings_dir, &["log", "-1", "--format=%ci %s"])
            .ok() // git log may fail on empty repo — None is the safe fallback
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    let snapshot_path = clings_dir.join(SYNC_SNAPSHOT_FILENAME);
    let subject_count = if snapshot_path.exists() {
        count_subjects_in_snapshot(&snapshot_path).unwrap_or(0)
    } else {
        0
    };

    Ok(SyncStatus {
        enabled: cfg.enabled,
        remote: cfg.remote.clone(),
        branch: if cfg.branch.is_empty() {
            SYNC_DEFAULT_BRANCH.to_string()
        } else {
            cfg.branch.clone()
        },
        last_commit,
        subject_count,
    })
}

// ── Helpers privés ────────────────────────────────────────────────────────────

fn ensure_git_installed() -> Result<()> {
    Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| {
            KfError::Sync(
                "`git` n'est pas installé — installez git pour utiliser le sync".to_string(),
            )
        })?;
    Ok(())
}

fn is_git_repo(dir: &Path) -> bool {
    dir.join(".git").is_dir()
}

/// Exécute une commande git dans `dir`, retourne Ok(()) si exit code 0.
fn git(dir: &Path, args: &[&str]) -> std::io::Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "git {} exited with {:?}",
            args.join(" "),
            status.code()
        )))
    }
}

/// Exécute git avec timeout via un channel mpsc.
fn git_timeout(dir: &Path, args: &[&str], timeout: Duration) -> std::io::Result<()> {
    let dir = dir.to_path_buf();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = Command::new("git")
            .args(&args)
            .current_dir(&dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        tx.send(result).ok(); // receiver may be dropped if main thread timed out — discard silently
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(status)) => Err(std::io::Error::other(format!(
            "git exited with {:?}",
            status.code()
        ))),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "git command timed out",
        )),
    }
}

/// Exécute git et capture stdout.
fn git_output(dir: &Path, args: &[&str]) -> std::io::Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stderr(Stdio::null())
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Écrit le snapshot JSON de la progression dans `clings_dir/progress.json`.
fn write_snapshot(clings_dir: &Path, conn: &Connection) -> Result<()> {
    let json = progress::export_progress(conn)?;
    let path = clings_dir.join(SYNC_SNAPSHOT_FILENAME);
    std::fs::write(&path, json)?;
    Ok(())
}

/// Hash léger d'un fichier (taille + quelques octets) pour détecter les changements.
/// Pas de dépendance cryptographique — détection de changement uniquement.
fn file_hash(path: &Path) -> Option<u64> {
    let meta = std::fs::metadata(path).ok()?;
    let size = meta.len();
    // Lire les 64 premiers octets pour un hash rapide
    let bytes = std::fs::read(path).ok()?;
    let sample: u64 = bytes
        .iter()
        .take(64)
        .enumerate()
        .fold(0u64, |acc, (i, &b)| {
            acc.wrapping_add((b as u64).wrapping_mul(i as u64 + 1))
        });
    Some(size.wrapping_add(sample))
}

/// Valide le nom de branche Git.
/// Rejette les branches vides, commençant par `-`, ou contenant espaces/caractères nuls.
fn validate_branch_name(branch: &str) -> Result<()> {
    if branch.is_empty() || branch.starts_with('-') || branch.contains([' ', '\n', '\0']) {
        return Err(KfError::Config(format!(
            "Nom de branche invalide : '{branch}'"
        )));
    }
    Ok(())
}

/// Résout le hostname pour le message de commit.
fn resolve_hostname(cfg: &SyncConfig) -> String {
    if !cfg.hostname.is_empty() {
        return cfg.hostname.clone();
    }
    Command::new("hostname")
        .output()
        .ok() // hostname command may be unavailable — fallback to "unknown"
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Compte les sujets dans le snapshot JSON (sans ouvrir de DB).
fn count_subjects_in_snapshot(path: &Path) -> Option<usize> {
    #[derive(serde::Deserialize)]
    struct Snap {
        subjects: Vec<serde_json::Value>,
    }
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Snap>(&json)
        .ok()
        .map(|s| s.subjects.len())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gitignore_excludes_db() {
        assert!(SYNC_GITIGNORE_CONTENT.contains("*.db"));
        assert!(SYNC_GITIGNORE_CONTENT.contains("*.db-wal"));
        assert!(SYNC_GITIGNORE_CONTENT.contains("*.toml"));
        assert!(SYNC_GITIGNORE_CONTENT.contains("*.c"));
    }

    #[test]
    fn test_status_no_git_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = SyncConfig::default();
        let status = status(tmp.path(), &cfg).unwrap();
        assert!(!status.enabled);
        assert!(status.last_commit.is_none());
        assert_eq!(status.subject_count, 0);
    }

    #[test]
    fn test_pull_and_merge_no_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        let result = pull_and_merge(tmp.path(), &mut conn);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("erreur de synchronisation"));
    }

    #[test]
    fn test_file_hash_differs_on_change() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.json");
        std::fs::write(&path, r#"{"subjects":[]}"#).unwrap();
        let h1 = file_hash(&path);
        std::fs::write(&path, r#"{"subjects":[{"name":"pointers"}]}"#).unwrap();
        let h2 = file_hash(&path);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_file_hash_none_on_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(file_hash(&tmp.path().join("missing.json")).is_none());
    }

    #[test]
    fn test_resolve_hostname_uses_config() {
        let mut cfg = SyncConfig::default();
        cfg.hostname = "machine-a".to_string();
        assert_eq!(resolve_hostname(&cfg), "machine-a");
    }
}
