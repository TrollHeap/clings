# Findings — Full Audit Remediation (2026-03-13)

## Fonctions réutilisables clés

| Fonction | Fichier:ligne | Usage |
|---|---|---|
| `common::mastery_bar_string(score: f64, width: usize) -> String` | `src/tui/common.rs:117` | Barre mastery avec slices statiques |
| `common::mastery_color(score: f64) -> Color` | `src/tui/common.rs:78` | Couleur gradient selon score |
| `common::render_split_status_bar(f, area, left: String, right: String, style: Style, right_width: u16)` | `src/tui/common.rs:200` | Status bar partagée |
| `common::render_description_panel(f, area, state: &AppState)` | `src/tui/common.rs:130` | Panel description |
| `common::render_run_result(f, area, result, exercise)` | `src/tui/common.rs:236` | Résultat compilation |
| `common::run_result_height(result: &RunResult) -> u16` | `src/tui/common.rs:225` | Hauteur dynamique résultat |
| `common::mini_map(completed: &[bool], idx: usize) -> String` | `src/tui/common.rs` | Mini-map ●◉○ |
| `common::difficulty_stars(d: Difficulty) -> &'static str` | `src/tui/common.rs` | Stars ★ difficulté |
| `common::difficulty_color(d: Difficulty) -> Color` | `src/tui/common.rs` | Couleur difficulté |
| `common::stage_label(stage: u8) -> &'static str` | `src/tui/common.rs` | Label stage S0-S4 |
| `BODY_SIDEBAR_THRESHOLD: u16 = 90` | `src/tui/common.rs:14` | Seuil sidebar |
| `SIDEBAR_WIDTH: u16 = 26` | `src/tui/common.rs:16` | Largeur sidebar |
| `FULL_BAR: &str = "██████████"` | `src/tui/common.rs:21` | Slice statique (max width=10) |

## Dead code confirmé

### src/tui/ui_watch.rs:57–68
Fonction locale `mastery_bar()` — duplique `mastery_bar_string` + `mastery_color` de common.
```rust
fn mastery_bar(score: f64, width: usize) -> (String, Color) {
    const FULL: &str = "██████████";
    const EMPTY: &str = "░░░░░░░░░░";
    // ...
}
```
**Fix T2 :** Remplacer par `common::mastery_bar_string(score, width)` + `common::mastery_color(score)`.

### src/tui/ui_stats.rs:22–31
`mastery_bar_spans()` utilise `String::repeat` au lieu des slices statiques.
```rust
let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
```
**Fix T3 :** `common::mastery_bar_string(score, 10)` — zéro allocation intermédiaire.

### src/mastery.rs — next_interval_days()
`pub(crate) fn next_interval_days(mastery: f32) -> u32` — `#[allow(dead_code)]`, jamais appelé en production.
3 tests orphelins : `next_interval_days_min_clamp`, `next_interval_days_max_clamp`, `next_interval_days_mid`.
**Fix T8 :** Supprimer la fonction ET ses 3 tests.

### src/tui/app.rs — champs morts
- `AppMode::Watch { chapter: Option<u8> }` — `chapter` jamais lu, `#[allow(dead_code)]`
- `AppMode::Piscine { chapter: Option<u8>, timed: Option<u64> }` — les deux champs jamais lus
- `AppState.mode: AppMode` — stocké dans `new()` mais jamais lu
- `Msg::Quit` — `#[allow(dead_code)]`, jamais construit ni envoyé
**Fix T5 :** Vérifier les call sites dans `src/main.rs` avant de supprimer.

### src/watcher.rs — WatchAction variants morts
- `WatchAction::Skip` (~ligne 23-24)
- `WatchAction::Next` (~ligne 28-29)
- `WatchAction::Prev` (~ligne 30-32)
Tous en `#[allow(dead_code)]`, jamais retournés par `watch_file_interactive`. Le mode TUI utilise `tui::events::spawn_event_reader`.
**Fix T6 :** Vérifier qu'aucun match arm dans main.rs ne les référence avant de supprimer.

## Sécurité

### src/tmux.rs:77 — injection via '='
```rust
// AVANT (vulnérable)
let safe_chars = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '=');
// APRÈS (safe — garde '/' pour les chemins absolus)
let safe_chars = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/');
```
`'='` permet `--option=value` injection vers l'éditeur (ex: `nvim --cmd=source /evil/script`).
`'/'` est nécessaire pour les chemins absolus passés en arg.
**Fix T1 :** Retirer `'='` uniquement.

## Duplication de layout (watch/piscine)

`render_body()` (ui_watch.rs:157–194) et `render_piscine_body()` (ui_piscine.rs:180–217) sont quasi-identiques :
- Même logique sidebar threshold + split horizontal
- Même logique description + result layout vertical
- Seule différence : callback sidebar (`render_mastery_sidebar` vs `render_piscine_sidebar`)

**Pattern à introduire dans common.rs :**
```rust
pub fn render_body_with_sidebar(
    f: &mut Frame,
    area: Rect,
    state: &AppState,
    render_sidebar: fn(&mut Frame, Rect, &AppState),
) {
    let (content_area, sidebar_opt) = if area.width >= BODY_SIDEBAR_THRESHOLD {
        let [left, right] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(SIDEBAR_WIDTH),
        ]).areas(area);
        (left, Some(right))
    } else {
        (area, None)
    };

    let (desc_area, result_area_opt) = if let Some(result) = &state.run_result {
        let h = run_result_height(result);
        let [desc, res] = Layout::vertical([Constraint::Fill(1), Constraint::Length(h)]).areas(content_area);
        (desc, Some(res))
    } else {
        (content_area, None)
    };

    render_description_panel(f, desc_area, state);

    if let Some(result_area) = result_area_opt {
        if let Some(result) = &state.run_result {
            let exercise = &state.exercises[state.current_index];
            render_run_result(f, result_area, result, exercise);
        }
    }

    if let Some(sb_area) = sidebar_opt {
        render_sidebar(f, sb_area, state);
    }
}
```
**Fix T4 :** Ajouter cette fonction dans `common.rs`, réduire `render_body` et `render_piscine_body` à un seul appel chacun.

## Constantes manquantes

**src/constants.rs** — Ajouter :
```rust
pub const MASTERY_BAR_WIDTH: usize = 10;       // Remplace magic 10 dans ui_watch, ui_stats
pub const PISCINE_PROGRESS_BAR_WIDTH: usize = 20; // Remplace magic 20usize dans ui_piscine
```

### ui_piscine.rs:234 — magic 20
```rust
let bar_width = 20usize;   // → PISCINE_PROGRESS_BAR_WIDTH
let filled = (ratio * bar_width as f64).round() as usize;
let progress_bar = format!(
    "[{}{}] {}/{}",
    "█".repeat(filled),              // → mastery_bar_string equivalent
    "░".repeat(bar_width - filled),
    ...
```
**Fix T10 :** Remplacer `20usize` par `constants::PISCINE_PROGRESS_BAR_WIDTH`.

## Conventions — Chapter::title

`Chapter::title: &'static str` — `#[allow(dead_code)]` dans `src/chapters.rs:12`.
**Décision :** Laisser en l'état — champ réservé, coût runtime nul, commentaire d'intention déjà présent.

## Ordre d'implémentation

| ID | Priorité | Fichier(s) | Description |
|---|---|---|---|
| T1 | SEC-M1 | src/tmux.rs:77 | Supprimer `'='` de safe_chars |
| T2 | DRY-H1 | src/tui/ui_watch.rs:57–68 | Supprimer mastery_bar() locale |
| T3 | DRY-M3 | src/tui/ui_stats.rs:22–31 | Remplacer String::repeat par mastery_bar_string |
| T4 | DRY-H2 | src/tui/common.rs + watch + piscine | Extraire render_body_with_sidebar |
| T5 | DRY-H3 | src/tui/app.rs + src/main.rs | Supprimer dead AppMode/Msg fields |
| T6 | DRY-H4 | src/watcher.rs + src/main.rs | Supprimer WatchAction variants morts |
| T7 | LOW | src/constants.rs | Ajouter MASTERY_BAR_WIDTH + PISCINE_PROGRESS_BAR_WIDTH |
| T8 | LOW | src/mastery.rs | Supprimer next_interval_days + 3 tests |
| T9 | LOW | src/chapters.rs | Keep as-is (reserved field) |
| T10 | LOW | src/tui/ui_piscine.rs:234 | magic 20 → PISCINE_PROGRESS_BAR_WIDTH |
