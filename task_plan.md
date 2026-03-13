# task_plan.md — v2.8.1 (cohérence Test/Both)

## T1 — Corriger CLAUDE.md [ ] pending
- **Fichier** : `CLAUDE.md`
- **Ligne** : supprimer/corriger "ValidationMode::Test and Both are stubbed — exercises with these modes are skipped silently in watch and piscine. Only Output validation works."
- **Nouveau texte** : refléter que Test/Both sont opérationnels dans runner.rs

## T2 — Ajouter test de non-filtrage [ ] pending
- **Fichier** : `src/exercises.rs` (bloc `#[cfg(test)]`, après `test_output_validation_has_expected`)
- **Fonction à réutiliser** : `load_all_exercises()` ligne 117, `ValidationMode` import existant
- **Pattern** : identique aux autres tests du fichier (voir `test_load_all_exercises_finds_files`)
- **Test** : vérifier que la liste chargée contient des exercices avec `ValidationMode::Test` et `ValidationMode::Both`

---

# task_plan.md — Full Audit Remediation v2.6.1 (archivé)

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
