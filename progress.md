# progress.md — v2.7.0 — Audit Remediation

| Tâche | Statut |
|-------|--------|
| T1 — rust-embed (binaire autonome) | [x] complete |
| T2 — Drop wait-timeout (try_wait polling) | [x] complete |
| T3 — clap_complete (completions shell) | [x] complete |
| T4 — nucleo-matcher (clings search) | [x] complete |
| Phase vérification — clippy + 174 tests | [x] complete — 0 warnings, 174/174 |
| **Audit Remediation (F + medium)** | |
| [H1] Fix `.chars().next().unwrap()` dans stats.rs tests | [x] complete |
| [M1] Constantes PCT_GREEN/YELLOW_THRESHOLD dans constants.rs | [x] complete |
| [M2] exam.rs — utiliser display::header_box() | [x] complete |
| [M3] search.rs — buffer char réutilisé (288→1 alloc/search) | [x] complete |
| [M5] practice_log retention (trim >10k rows) | [x] complete |
| [H2/H3/H4] DRY piscine display / visualizer nav | [ ] skipped (refactor closure trop risqué) |
| [M4] Annales render write! buffer | [ ] skipped (UI path, impact négligeable) |
| [M6] main.rs modularisation (1009 lignes) | [x] complete — src/commands/{watch,run,info,progress_cmds,data}.rs |
| [M7] tmux editor validation at config load | [ ] skipped (déjà fait dans resolve_editor()) |
| Vérification finale — clippy + 174 tests | [x] 0 warnings, 174/174 |
