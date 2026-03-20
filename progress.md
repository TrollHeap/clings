# progress.md — Full Audit Remediation v3.1.1 — COMPLETE

| Tâche | Description | Statut |
|-------|-------------|--------|
| T1 | Permissions ~/.clings/ mode 0o700 | [x] complete |
| T2 | Error variant watcher Config → Watch | [x] complete |
| T3 | Index SQLite practice_log + subjects | [x] complete |
| T4 | Supprimer dev-deps insta/proptest | [x] skipped — utilisées par tests/ |
| T5 | build_list_display_items O(n²) → O(n) | [x] complete |
| T6 | get_streak() NaiveDate | [x] complete |
| T7 | Extraire handle_compile() | [x] complete |
| T8 | Dédupliquer logging overlay dispatch | [x] complete |
| T9 | Extraire navigation chapitres | [x] complete |
| T10 | Extraire build_gcc_args | [x] complete |
| T11 | Doc comments pub functions | [x] complete |
| T12 | Supprimer _filter_subject mort | [x] complete |
| T13 | Extraire render_opaque_background | [x] complete |
| T18 | Hint counter duplication → helper | [x] complete |

## Vérification finale
- cargo test: 185 tests OK
- cargo clippy -- -D warnings: clean
- Date: 2026-03-20
