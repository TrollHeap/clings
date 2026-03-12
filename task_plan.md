# task_plan.md — Full Audit Remediation v2.6.1

## T1 — Visualizer nav DRY [ ] pending
- **Fichiers :** `src/display/visualizer.rs` (ajout helpers), `src/main.rs` (4 sites), `src/piscine.rs` (4 sites)
- **Ajouter dans visualizer.rs :**
  ```rust
  pub fn step_forward(step: usize, total: usize) -> usize {
      (step + 1).min(total.saturating_sub(1))
  }
  pub fn step_back(step: usize) -> usize {
      step.saturating_sub(1)
  }
  ```
- **Remplacer** les 4 blocs Right/Left inline par appels aux helpers
- **Fix incohérence piscine.rs:490** : aligner sur pattern canonique (toujours redraw)

## T2 — apply_all_decay() SQL filter [ ] pending
- **Fichier :** `src/progress.rs:487`
- **Nouvelle signature :** `pub fn apply_all_decay(conn: &mut Connection, decay_days: i64) -> Result<()>`
- **Requête SQL directe** avec WHERE sur mastery_score > 0.0 et last_practiced_at < now - decay_days
- **Adapter call site** dans `src/main.rs`

## T3 — get_streak() LIMIT [ ] pending
- **Fichier :** `src/progress.rs:244`
- **Changement :** `LIMIT 365` → `LIMIT 90`

## T4 — normalize() 2 allocs [ ] pending
- **Fichier :** `src/runner.rs:458`
- **Réécrire** avec `String::with_capacity` + boucle `s.lines()`

## T5 — Import redondant [ ] pending
- **Fichier :** `src/piscine.rs` ligne ~486
- **Supprimer** l'import local `use crossterm::event::{...}` en doublon

## T6 — Swallowed flush [ ] pending
- **Fichier :** `src/main.rs` (~337, ~344, ~606, ~613)
- **Ajouter** commentaire `// best-effort flush — non-critique`

## Phase 4 — Vérification [ ] pending
- `cargo build`
- `cargo clippy -- -D warnings`
- `cargo test`
