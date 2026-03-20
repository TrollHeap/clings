# Findings — Full Audit Remediation v3.1.1 (2026-03-20)

## Patterns existants

### Error handling
- Pattern: `KfError` enum via `thiserror` dans `src/error.rs`, alias `Result<T>`
- Propagation via `?`, jamais de `.unwrap()` en prod
- Logging erreurs: `eprintln!("[clings] erreur {contexte}: {e}");`
- Variant `Watch(String)` existe dans KfError — watcher.rs utilise Config à tort (ligne 68)

### SQLite
- Schema dans `progress.rs:21-56`, WAL mode activé `progress.rs:71`
- Parameterized queries via `params![]` partout
- `prepare_cached()` pour queries répétées
- Migrations additives: `migrate_v1()` pattern expand-only
- Pas d'index sur `practice_log(practiced_at)` ni `subjects(next_review_at)`

### TUI Architecture (TEA/Elm)
- `AppState` = centralized model dans `app.rs:84+`
- `Msg` enum (Key, FileChanged, Tick) dans `app.rs:28-33`
- `update_watch()` dispatche `Msg` → mutations état (`app.rs:765-876`)
- Overlays: `OverlayState` struct dans `app.rs:36-50`
- `ListDisplayItem` enum (ChapterHeader | Exercise) dans `app.rs:14-25`
- Rendu: `view()` → `render_*()` dans `ui_watch.rs`, `ui_piscine.rs`
- Palette: constantes `C_*` dans `common.rs:29+`

### Directory permissions
- Pattern correct dans `runner.rs:work_dir()`: `DirBuilder::new().mode(0o700)`
- Pattern incorrect dans `progress.rs:66`, `config.rs:182`: `std::fs::create_dir_all()` sans mode

## Fonctions réutilisables

| Fonction | Fichier:ligne | Usage |
|----------|---------------|-------|
| `invalidate_header_cache()` | `app.rs:171` | Invalide cache header après mutation mastery |
| `navigate_next/prev()` | `app.rs` | Navigation entre exercices |
| `load_current_exercise()` | `app.rs` | Charge fichier exercice courant |
| `save_checkpoint()` | `app.rs:617-624` | Sauvegarde checkpoint piscine/exam |
| `handle_hint_reveal()` | `app.rs` | Révèle un indice |
| `handle_vis_toggle()` | `app.rs` | Toggle visualizer |
| `open_list_overlay()` | `app.rs` | Ouvre overlay liste |
| `record_attempt()` | `progress.rs:159` | Enregistre tentative + SRS update |
| `get_subject()` | `progress.rs:249` | Récupère un Subject |
| `get_streak()` | `progress.rs:263` | Calcul streak via string dates |
| `trim_practice_log()` | `progress.rs:237` | Trim log > 10000 rows |
| `get_due_subjects()` | `progress.rs:420` | Subjects dus pour review |
| `apply_all_decay()` | `progress.rs:522` | Décroissance SRS batch |
| `compile_and_run()` | `runner.rs` | Point d'entrée compilation+validation |
| `run_output()` | `runner.rs:274` | Mode output: compile+run+validate |
| `spawn_gcc_and_collect()` | `runner.rs:140` | Compilation gcc + collect stdout |
| `write_exercise_files()` | `runner.rs:99` | Écrit fichiers auxiliaires |
| `order_by_chapters()` | `chapters.rs:112` | Tri curriculum |
| `flatten_chapters()` | `chapters.rs:180` | Aplatit blocs chapitres |
| `filter_by_chapter()` | `chapters.rs:188` | Filtre par n° chapitre |

## Fichiers adjacents

- `src/error.rs` — KfError enum (Watch, Config, Io, Database, ExerciseNotFound, Json)
- `src/constants.rs` — toutes les constantes
- `src/models.rs` — Exercise, Subject, MasteryScore, SrsIntervalDays, Difficulty
- `src/mastery.rs` — update_mastery(), compute_next_review(), apply_decay()
- `src/tui/common.rs` (942 lignes) — rendering helpers partagés, palette
- `src/tui/ui_piscine.rs` (354 lignes) — rendu mode piscine (similar status bar)

## Conventions
- Français pour UI/user-facing, anglais pour code/technique
- Tests inline `#[cfg(test)] mod tests { ... }` dans chaque module
- Pas de TODOs existants
- Clippy clean `-D warnings`, 167 tests passent
