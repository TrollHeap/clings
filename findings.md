# Findings — Full Audit Remediation (2026-03-12)

## T1 — Visualizer nav DRY (4× duplication + inconsistance)

**Fichiers :** `src/main.rs`, `src/piscine.rs`
**Pattern canonique** (main.rs:331–366, main.rs:600–628, piscine.rs:206–237) :
```rust
KeyCode::Right => {
    let n = exercise.visualizer.steps.len();
    vis_step = (vis_step + 1).min(n.saturating_sub(1));
    print!("\x1b[{vis_lines}A\x1b[J");
    vis_lines = display::show_visualizer(exercise, vis_step);
}
```
**Pattern incohérent** (piscine.rs:490–524, `run_exam_piscine`) :
```rust
KeyCode::Right => {
    let total_steps = exercise.visualizer.steps.len();
    if vis_step + 1 < total_steps {   // conditionnel — pas de redraw si dernière étape
        print!("\x1b[{vis_lines}A\x1b[J");
        vis_step += 1;
        vis_lines = display::show_visualizer(exercise, vis_step);
    }
    return None;
}
```
**Fix :** Extraire `fn step_forward(step: usize, total: usize) -> usize` et `fn step_back(step: usize) -> usize`
dans `src/display/visualizer.rs`, réutiliser dans les 4 sites.

## T2 — apply_all_decay() full table scan

**Fichier :** `src/progress.rs:487`
**Problème :** appelle `get_all_subjects(conn)?` — retourne TOUTES les lignes même avec mastery=0.
**Fix :** Nouvelle requête SQL directe avec WHERE :
```sql
SELECT subject, mastery_score, last_practiced_at, next_review_at, difficulty_unlocked
FROM subjects
WHERE mastery_score > 0.0
  AND last_practiced_at IS NOT NULL
  AND last_practiced_at < unixepoch('now') - (?1 * 86400)
```
Paramètre : `decay_days: i64` depuis `crate::config::get().srs.decay_days`.
**Call sites :** `src/main.rs` — adapter l'appel à `progress::apply_all_decay(conn, decay_days)`.

## T3 — get_streak() LIMIT 365

**Fichier :** `src/progress.rs:244`
**Fix :** `LIMIT 365` → `LIMIT 90`

## T4 — normalize() 4 allocations

**Fichier :** `src/runner.rs:458`
**Fix (2 allocs) :**
```rust
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        if !out.is_empty() { out.push('\n'); }
        out.push_str(line.trim_end());
    }
    out.trim().to_string()
}
```

## T5 — Import redondant piscine.rs

**Fichier :** `src/piscine.rs:486`
```rust
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};  // doublon ligne 8
```
**Fix :** Supprimer la ligne locale (~486).

## T6 — Swallowed flush errors

**Fichier :** `src/main.rs` (lignes ~337, ~344, ~606, ~613)
**Fix :** Ajouter commentaire `// best-effort flush` ou remplacer par `.ok()`.

## Patterns réutilisables

- `crate::config::get().srs.decay_days` → valeur dynamique decay
- `display::show_visualizer(exercise, step) -> usize` → retourne nombre de lignes affichées
- `let _ = std::io::Write::flush(...)` pattern existant dans annales.rs:135
