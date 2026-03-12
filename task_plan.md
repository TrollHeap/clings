# task_plan.md — Full Audit Remediation v2.6.1

## T1 — Visualizer nav DRY [x] skipped
- Helpers `step_forward`/`step_back` jugés non nécessaires (inline lisible, 2 sites seulement)

## T2 — apply_all_decay() SQL filter [x] done
- Implémenté via paramètre `decay_days: i64` dans la signature + WHERE en SQL

## T3 — get_streak() LIMIT [x] done
- `LIMIT 365` → `LIMIT 90` dans `src/progress.rs:244`

## T4 — normalize() 2 allocs [x] done
- Réécrit avec `String::with_capacity` + boucle `s.lines()` dans `src/runner.rs`

## T5 — Import redondant [x] done
- Import local `use crossterm::event::{...}` supprimé dans `src/piscine.rs`

## T6 — Swallowed flush [x] done
- Commentaire `// best-effort flush — non-critique` ajouté dans :
  - `src/commands/data.rs` (2 sites)
  - `src/exam.rs` (1 site)

## Phase 4 — Vérification [x] complete
- `cargo build` ✓
- `cargo clippy -- -D warnings` ✓ (0 warnings)
- `cargo test` ✓ (174/174)
