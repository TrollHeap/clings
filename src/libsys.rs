//! Gestion de la librairie personnelle libsys.
//!
//! Chaque exercice `LibraryExport` validé exporte une fonction C vers
//! un repo Git local (`libsys_path`). L'historique Git matérialise la
//! progression de l'étudiant et le repo est pushable vers GitHub.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{KfError, Result};

/// Données d'une fonction à exporter dans libsys.
pub struct LibsysExport {
    /// Module cible (ex. "my_string") — correspond à src/my_string.c + include/my_string.h
    pub module: String,
    /// Nom de la fonction (ex. "my_strdup")
    pub function: String,
    /// Code C complet de l'implémentation (contenu du fichier source étudiant)
    pub c_code: String,
    /// Déclaration header (ex. "char *my_strdup(const char *s);")
    pub h_decl: String,
}

/// Statut d'un module dans libsys.
#[derive(Debug, Clone)]
pub struct ModuleStatus {
    pub name: String,
    pub functions: Vec<ExportedFn>,
    /// Subject NSY103 requis pour débloquer (None = standalone).
    /// Affiché dans l'overlay portfolio [b] pour les modules verrouillés.
    pub unlock_subject: Option<String>,
}

/// Fonction déjà exportée dans un module.
#[derive(Debug, Clone)]
pub struct ExportedFn {
    pub name: String,
    /// Hash du commit git associé à l'export (affiché dans l'overlay portfolio [b]).
    pub commit_hash: String,
}

/// Initialise le repo libsys si absent : crée la structure de répertoires,
/// le Makefile, le README, et fait un premier commit.
pub fn init_repo(path: &Path) -> Result<()> {
    // Crée les répertoires si nécessaire
    std::fs::create_dir_all(path.join("include")).map_err(KfError::Io)?;
    std::fs::create_dir_all(path.join("src")).map_err(KfError::Io)?;

    // Init git si pas déjà initialisé
    if !path.join(".git").exists() {
        run_git(path, &["init"])?;
        run_git(path, &["config", "user.email", "libsys@clings"])?;
        run_git(path, &["config", "user.name", "clings"])?;

        write_makefile(path)?;
        write_readme(path, &[])?;

        run_git(path, &["add", "-A"])?;
        run_git(path, &["commit", "-m", "chore: init libsys repo"])?;
    }

    Ok(())
}

/// Exporte une fonction validée vers libsys et fait un commit git.
///
/// Idempotent : si la fonction est déjà présente dans le fichier .c (détection
/// par signature), l'export est silencieusement ignoré.
pub fn export(path: &Path, export: &LibsysExport) -> Result<()> {
    init_repo(path)?;

    let c_path = path.join("src").join(format!("{}.c", export.module));
    let h_path = path.join("include").join(format!("{}.h", export.module));

    // Vérifie si la fonction est déjà exportée (idempotence)
    if c_path.exists() {
        let existing = std::fs::read_to_string(&c_path).map_err(KfError::Io)?;
        if function_already_present(&existing, &export.function) {
            return Ok(());
        }
    }

    // Append dans le .c
    let c_entry = format_c_entry(&export.c_code, &export.function);
    append_to_file(&c_path, &c_entry)?;

    // Append dans le .h
    let h_entry = format_h_entry(&export.h_decl, &export.function);
    append_to_file(&h_path, &h_entry)?;

    // Régénère le README
    let modules = portfolio_status(path)?;
    write_readme(path, &modules)?;

    // Commit
    run_git(path, &["add", "-A"])?;
    let msg = format!("feat({}): add {}", export.module, export.function);
    run_git(path, &["commit", "-m", &msg])?;

    Ok(())
}

/// Retourne le statut de tous les modules connus dans libsys.
pub fn portfolio_status(path: &Path) -> Result<Vec<ModuleStatus>> {
    let known_modules = [
        ("my_string", None),
        ("my_memory", None),
        ("my_list", None),
        ("my_algo", None),
        ("my_io", None),
        ("my_process", Some("processes")),
        ("my_signal", Some("signals")),
        ("my_ipc", Some("pipes")),
        ("my_thread", Some("pthreads")),
        ("my_sync", Some("semaphores")),
        ("my_socket", Some("sockets")),
    ];

    let mut modules = Vec::new();
    for (name, unlock) in &known_modules {
        let functions = exported_functions_for(path, name);
        modules.push(ModuleStatus {
            name: name.to_string(),
            functions,
            unlock_subject: unlock.map(|s| s.to_string()),
        });
    }

    Ok(modules)
}

// ── Helpers privés ────────────────────────────────────────────────────────────

/// Retourne les fonctions déjà exportées pour un module donné, en lisant git log.
fn exported_functions_for(path: &Path, module: &str) -> Vec<ExportedFn> {
    let c_path = path.join("src").join(format!("{module}.c"));
    if !c_path.exists() {
        return Vec::new();
    }

    // Lit le fichier .c et cherche les marqueurs de fonctions exportées
    let Ok(content) = std::fs::read_to_string(&c_path) else {
        return Vec::new();
    };

    let mut functions = Vec::new();
    for line in content.lines() {
        if let Some(fn_name) = parse_libsys_fn_marker(line) {
            let hash = git_log_hash_for_fn(path, module, fn_name).unwrap_or_default();
            functions.push(ExportedFn {
                name: fn_name.to_string(),
                commit_hash: hash,
            });
        }
    }
    functions
}

/// Extrait le nom de fonction depuis un marqueur `/* libsys: fn_name */`.
fn parse_libsys_fn_marker(line: &str) -> Option<&str> {
    let line = line.trim();
    let inner = line.strip_prefix("/* libsys: ")?.strip_suffix(" */")?;
    Some(inner)
}

/// Cherche le hash du commit qui a ajouté une fonction dans un module.
fn git_log_hash_for_fn(path: &Path, module: &str, function: &str) -> Option<String> {
    let output = Command::new("git")
        .args([
            "-C",
            &path.to_string_lossy(),
            "log",
            "--oneline",
            "--grep",
            &format!("add {function}"),
            "--",
            &format!("src/{module}.c"),
        ])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .next()
        .map(|l| l.split_whitespace().next().unwrap_or("").to_string())
}

/// Vérifie si la fonction est déjà présente dans le code C (via marqueur).
fn function_already_present(c_content: &str, fn_name: &str) -> bool {
    c_content.contains(&format!("/* libsys: {fn_name} */"))
}

/// Formate l'entrée à ajouter dans le fichier .c.
fn format_c_entry(c_code: &str, fn_name: &str) -> String {
    format!("\n/* libsys: {fn_name} */\n{}\n", c_code.trim_end())
}

/// Formate la déclaration à ajouter dans le fichier .h.
fn format_h_entry(h_decl: &str, fn_name: &str) -> String {
    format!("/* libsys: {fn_name} */\n{}\n", h_decl.trim())
}

/// Ajoute du contenu à la fin d'un fichier (crée le fichier si absent).
fn append_to_file(path: &Path, content: &str) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(KfError::Io)?;
    file.write_all(content.as_bytes()).map_err(KfError::Io)
}

/// Exécute une commande git dans le repo libsys.
fn run_git(repo: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args(args)
        .status()
        .map_err(KfError::Io)?;

    if !status.success() {
        return Err(KfError::Config(format!(
            "git {} échoué (code {:?})",
            args.join(" "),
            status.code()
        )));
    }
    Ok(())
}

/// Retourne le chemin libsys depuis `LIBSYS_PATH` env var, la config, ou
/// fallback vers `$HOME/Developer/TOOLS/libsys`.
pub fn libsys_path() -> PathBuf {
    if let Ok(p) = std::env::var("LIBSYS_PATH") {
        return PathBuf::from(p);
    }
    if let Some(p) = crate::config::get().libsys_path.clone() {
        return p;
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join("Developer/TOOLS/libsys")
}

/// Génère le README.md du repo libsys avec la liste des fonctions exportées.
pub fn write_readme(path: &Path, modules: &[ModuleStatus]) -> Result<()> {
    let mut content = String::from("# libsys\n\nBibliothèque C personnelle générée par [clings](https://github.com/trollheap/clings).\n\n");

    content.push_str("## Modules\n\n");
    for module in modules {
        if module.functions.is_empty() {
            continue;
        }
        content.push_str(&format!("### `{}`\n\n", module.name));
        for f in &module.functions {
            content.push_str(&format!("- `{}()`\n", f.name));
        }
        content.push('\n');
    }

    if modules.iter().all(|m| m.functions.is_empty()) {
        content.push_str("_Aucune fonction exportée pour l'instant._\n");
    }

    std::fs::write(path.join("README.md"), content).map_err(KfError::Io)
}

fn write_makefile(path: &Path) -> Result<()> {
    let content = r#"# Makefile — libsys
# Généré automatiquement par clings

CC      = gcc
CFLAGS  = -Wall -Wextra -std=c11 -I./include
AR      = ar
ARFLAGS = rcs
NAME    = libsys.a

SRC     = $(wildcard src/*.c)
OBJ     = $(SRC:.c=.o)

all: $(NAME)

$(NAME): $(OBJ)
	$(AR) $(ARFLAGS) $@ $^

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OBJ)

fclean: clean
	rm -f $(NAME)

re: fclean all

.PHONY: all clean fclean re
"#;
    std::fs::write(path.join("Makefile"), content).map_err(KfError::Io)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp_repo() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn test_init_repo_creates_structure() {
        let dir = tmp_repo();
        init_repo(dir.path()).unwrap();

        assert!(dir.path().join(".git").exists(), "repo git manquant");
        assert!(dir.path().join("include").exists(), "include/ manquant");
        assert!(dir.path().join("src").exists(), "src/ manquant");
        assert!(dir.path().join("Makefile").exists(), "Makefile manquant");
        assert!(dir.path().join("README.md").exists(), "README.md manquant");
    }

    #[test]
    fn test_init_repo_idempotent() {
        let dir = tmp_repo();
        init_repo(dir.path()).unwrap();
        // Deuxième appel — ne doit pas échouer ni créer un second commit
        init_repo(dir.path()).unwrap();

        let output = Command::new("git")
            .args(["-C", &dir.path().to_string_lossy(), "log", "--oneline"])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            log.lines().count(),
            1,
            "devrait avoir exactement 1 commit initial"
        );
    }

    #[test]
    fn test_export_creates_files() {
        let dir = tmp_repo();
        let exp = LibsysExport {
            module: "my_string".to_string(),
            function: "my_strdup".to_string(),
            c_code: "char *my_strdup(const char *s) { return NULL; }".to_string(),
            h_decl: "char *my_strdup(const char *s);".to_string(),
        };
        export(dir.path(), &exp).unwrap();

        assert!(dir.path().join("src/my_string.c").exists());
        assert!(dir.path().join("include/my_string.h").exists());

        let c = std::fs::read_to_string(dir.path().join("src/my_string.c")).unwrap();
        assert!(c.contains("/* libsys: my_strdup */"));
        assert!(c.contains("my_strdup"));

        let h = std::fs::read_to_string(dir.path().join("include/my_string.h")).unwrap();
        assert!(h.contains("char *my_strdup(const char *s);"));
    }

    #[test]
    fn test_export_commits() {
        let dir = tmp_repo();
        let exp = LibsysExport {
            module: "my_string".to_string(),
            function: "my_strdup".to_string(),
            c_code: "char *my_strdup(const char *s) { return NULL; }".to_string(),
            h_decl: "char *my_strdup(const char *s);".to_string(),
        };
        export(dir.path(), &exp).unwrap();

        let output = Command::new("git")
            .args(["-C", &dir.path().to_string_lossy(), "log", "--oneline"])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&output.stdout);
        assert!(
            log.contains("feat(my_string): add my_strdup"),
            "commit manquant: {log}"
        );
    }

    #[test]
    fn test_export_idempotent() {
        let dir = tmp_repo();
        let exp = LibsysExport {
            module: "my_string".to_string(),
            function: "my_strdup".to_string(),
            c_code: "char *my_strdup(const char *s) { return NULL; }".to_string(),
            h_decl: "char *my_strdup(const char *s);".to_string(),
        };
        export(dir.path(), &exp).unwrap();
        export(dir.path(), &exp).unwrap(); // second appel — doit être ignoré

        let output = Command::new("git")
            .args(["-C", &dir.path().to_string_lossy(), "log", "--oneline"])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&output.stdout);
        // init commit + 1 export = 2 commits, pas 3
        assert_eq!(log.lines().count(), 2, "export dupliqué détecté: {log}");
    }

    #[test]
    fn test_portfolio_status_empty() {
        let dir = tmp_repo();
        init_repo(dir.path()).unwrap();
        let status = portfolio_status(dir.path()).unwrap();
        assert!(!status.is_empty(), "doit retourner les modules connus");
        assert!(
            status.iter().all(|m| m.functions.is_empty()),
            "repo vide → aucune fonction"
        );
    }

    #[test]
    fn test_portfolio_status_after_export() {
        let dir = tmp_repo();
        let exp = LibsysExport {
            module: "my_string".to_string(),
            function: "my_strdup".to_string(),
            c_code: "char *my_strdup(const char *s) { return NULL; }".to_string(),
            h_decl: "char *my_strdup(const char *s);".to_string(),
        };
        export(dir.path(), &exp).unwrap();

        let status = portfolio_status(dir.path()).unwrap();
        let my_string = status
            .iter()
            .find(|m| m.name == "my_string")
            .expect("my_string absent");
        assert_eq!(my_string.functions.len(), 1);
        assert_eq!(my_string.functions[0].name, "my_strdup");
    }

    #[test]
    fn test_function_already_present() {
        let code = "/* libsys: my_strdup */\nchar *my_strdup(const char *s) { return NULL; }\n";
        assert!(function_already_present(code, "my_strdup"));
        assert!(!function_already_present(code, "my_strlen"));
    }

    #[test]
    fn test_libsys_path_env_var_overrides() {
        // LIBSYS_PATH doit avoir la priorité sur config et HOME
        std::env::set_var("LIBSYS_PATH", "/tmp/my_custom_libsys");
        let path = libsys_path();
        std::env::remove_var("LIBSYS_PATH");
        assert_eq!(path, std::path::PathBuf::from("/tmp/my_custom_libsys"));
    }

    #[test]
    fn test_libsys_path_fallback_uses_home() {
        // Sans LIBSYS_PATH ni config, doit utiliser $HOME/Developer/TOOLS/libsys
        std::env::remove_var("LIBSYS_PATH");
        let path = libsys_path();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let expected = std::path::PathBuf::from(home).join("Developer/TOOLS/libsys");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_write_readme_empty_modules() {
        let dir = tmp_repo();
        init_repo(dir.path()).unwrap();
        let modules: Vec<ModuleStatus> = vec![];
        write_readme(dir.path(), &modules).unwrap();
        let content = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
        assert!(
            content.contains("libsys"),
            "README doit contenir le titre libsys"
        );
    }

    #[test]
    fn test_write_readme_lists_modules() {
        let dir = tmp_repo();
        init_repo(dir.path()).unwrap();
        let modules = vec![
            ModuleStatus {
                name: "my_string".to_string(),
                functions: vec![
                    ExportedFn {
                        name: "my_strdup".to_string(),
                        commit_hash: "abc1234".to_string(),
                    },
                    ExportedFn {
                        name: "my_strlen".to_string(),
                        commit_hash: "def5678".to_string(),
                    },
                ],
                unlock_subject: None,
            },
            ModuleStatus {
                name: "my_memory".to_string(),
                functions: vec![],
                unlock_subject: None,
            },
        ];
        write_readme(dir.path(), &modules).unwrap();
        let content = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
        assert!(
            content.contains("my_string"),
            "README doit lister my_string"
        );
        assert!(
            content.contains("my_strdup"),
            "README doit lister my_strdup"
        );
        assert!(
            content.contains("my_strlen"),
            "README doit lister my_strlen"
        );
        // Modules vides (my_memory) sont skippés par write_readme — comportement attendu
        assert!(
            !content.contains("my_memory"),
            "README ne doit pas lister les modules sans fonctions exportées"
        );
    }
}
