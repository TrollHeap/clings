# task_plan.md — v2.9.1 → v2.9.9

## Releases planifiées

### v2.9.1 — Fix: search overlay scrolling invisible [x] done
**Bug :** `render_search_overlay` (common.rs:347) applique `.take(max_visible * 3)` sur les résultats avant de les passer à `List::new()`, mais `list_state.select(Some(state.search_selected))` utilise l'indice global. Si `search_selected = 20` et seulement 15 items sont rendus → sélection invisible.
- **Fichier** : `src/tui/common.rs` (lignes 343–397)
- **Fix** : supprimer le `.take(max_visible.max(1) * 3)`, passer TOUS les résultats à `List::new()`. Ratatui + `ListState` gère nativement le scroll. Garder seulement `filter_map` pour résoudre les indices → exercises.
- **Contraintes** : ne pas changer la signature, ne pas toucher `app.rs`

### v2.9.2 — Fix: rebuild_search O(n²) → O(n) [x] done
**Bug :** `rebuild_search` (app.rs:190) appelle `.iter().position(|e| e.id == ex.id)` pour chaque résultat retourné par `search_exercises` → O(n) par résultat = O(n²) total.
- **Fichier** : `src/tui/app.rs` (lignes 183–194)
- **Fix** : les références retournées par `search_exercises` viennent du même slice `state.exercises`. Calculer l'offset via pointeur : `(ex as *const Exercise).offset_from(state.exercises.as_ptr()) as usize`. Wrap dans `fn ptr_offset` locale avec `unsafe` + SAFETY comment.
- **Contraintes** : résultat identique, aucun changement de comportement observable

### v2.9.3 — Feature: search overlay en mode piscine [x] done
**Context :** `update_piscine` n'a pas de bloc search. `ui_piscine::view()` n'a pas de branche `search_active`.
- **Fichiers** : `src/tui/app.rs` (update_piscine ~ligne 444), `src/tui/ui_piscine.rs` (view ~ligne 40)
- **Plan** :
  1. `update_piscine` : ajouter bloc `if self.state.search_active { ... return; }` avant `if self.state.vis_active` — copier le bloc de `update_watch` (lignes 204–247)
  2. Ajouter `[/]` dans le match normal de piscine → `search_active = true + rebuild_search`
  3. `ui_piscine::view()` : ajouter `else if state.search_active { common::render_search_overlay(...) }` entre vis et body
  4. `render_piscine_status_bar()` : branche search_active comme dans watch
- **Contraintes** : `rebuild_search` est `fn rebuild_search(state: &mut AppState)` — même appel depuis piscine

### v2.9.4 — UX: status_msg auto-clear [x] done
**Context :** `Msg::Tick` handler est vide (app.rs:378-380). Le message "fichier sauvegardé" reste affiché jusqu'à la prochaine action.
- **Fichier** : `src/tui/app.rs` (struct AppState + update_watch Tick handler)
- **Plan** :
  1. Ajouter `status_msg_at: Option<std::time::Instant>` dans `AppState`
  2. Init : `status_msg_at: None`
  3. Chaque fois que `status_msg` est set → set `status_msg_at = Some(Instant::now())`
  4. `Msg::Tick` handler : si `status_msg_at` est Some ET `elapsed > 3s` → clear les deux
- **Contraintes** : 2 sites de set status_msg (watch:376, piscine:616), setter les deux

### v2.9.5 — Feature: filtre sujet courant dans search (`[Tab]`) [x] done
**Context :** La recherche porte sur tous les exercices. Utile de filtrer au sujet courant.
- **Fichiers** : `src/tui/app.rs`, `src/tui/common.rs`
- **Plan** :
  1. `AppState` : ajouter `pub search_subject_filter: bool` (init false)
  2. `rebuild_search` : si `search_subject_filter` → passer `Some(&state.exercises[state.current_index].subject)` à `search_exercises`
  3. `update_watch` search block : `KeyCode::Tab` → toggle `search_subject_filter` + `rebuild_search`
  4. `render_search_overlay` : titre devient `"/ Recherche (sujet: X)"` si filter actif, sinon `"/ Recherche"`; hint bar : ajouter `[Tab] filtre sujet`
- **Contraintes** : `search_subject_filter` reset à `false` à l'ouverture de l'overlay

### v2.9.6 — Perf: cache subject order dans AppState [x] done
**Context :** `render_mastery_sidebar` (ui_watch.rs:259–276) reconstruit un `HashSet` + `Vec<&String>` de déduplication à chaque frame (50ms). Pour 300+ exercices, c'est ~300 iterations inutiles.
- **Fichiers** : `src/tui/app.rs`, `src/tui/ui_watch.rs`, `src/commands/watch.rs`
- **Plan** :
  1. `AppState` : ajouter `pub subject_order: Vec<String>` (sujets uniques dans l'ordre d'apparition)
  2. `commands/watch.rs` : remplir `subject_order` depuis la liste d'exercices après flatten (même algo que sidebar actuel)
  3. `render_mastery_sidebar` : utiliser `state.subject_order` au lieu de reconstruire
  4. Supprimer `seen: HashSet` et la boucle de déduplication de `render_mastery_sidebar`
- **Contraintes** : comportement identique, même ordre, même priorité sujet courant

### v2.9.7 — Feature: overlay `[?]` help dans watch [x] done
**Context :** Les keybinds sont visibles dans la status bar mais pas documentés en détail.
- **Fichiers** : `src/tui/app.rs`, `src/tui/common.rs`, `src/tui/ui_watch.rs`
- **Plan** :
  1. `AppState` : ajouter `pub help_active: bool` (init false)
  2. `update_watch` : avant vis_active block, ajouter `if help_active { Esc/tout → close }`. Dans normal keys : `[?]` → `help_active = true`
  3. `common.rs` : `pub fn render_help_overlay(f: &mut Frame, area: Rect)` — popup 60%×70%, liste des keybinds avec descriptions, statique (pas besoin de AppState)
  4. `ui_watch::view()` : `else if state.help_active { common::render_help_overlay(f, body_area); }`
  5. Status bar : si `help_active` → `"[Esc/?] fermer"`; sinon ajouter `[?] aide` dans la liste
- **Contraintes** : overlay statique, pas de navigation interne

### v2.9.8 — Feature: `[g]`/`[G]` first/last dans search overlay [x] done
**Context :** Navigation vim : `gg` → premier résultat, `G` → dernier. Améliore l'ergonomie.
- **Fichier** : `src/tui/app.rs` (update_watch search block)
- **Plan** :
  1. `AppState` : ajouter `pub search_g_pending: bool` (init false — attend le second `g` pour `gg`)
  2. `update_watch` search block :
     - `KeyCode::Char('G')` → `search_selected = search_results.len().saturating_sub(1)`, reset `search_g_pending`
     - `KeyCode::Char('g')` si `search_g_pending` → `search_selected = 0`, reset pending
     - `KeyCode::Char('g')` sinon → `search_g_pending = true`
     - Tout autre key : reset `search_g_pending`
  3. `rebuild_search` : reset `search_g_pending = false`
  4. Hint bar dans overlay : ajouter `[g/G] début/fin`
- **Contraintes** : `search_g_pending` reset sur Esc, Backspace, tout Char non-`g`

### v2.9.9 — Chore: CHANGELOG + bump 2.9.8 → 2.9.9 + tests [x] done
- **Fichiers** : `Cargo.toml`, `CHANGELOG.md`
- **Tests à écrire** : `cargo test` doit passer à 153+ tests ; ajouter si coverage manque
- **Tag** : `git tag v2.9.9`

---

## Audit Remediation — full-audit 2026-03-13

### T1 — SEC: tmux.rs safe_chars — supprimer '=' [x] done
- **Fichier** : `src/tmux.rs:77`
- **Fix** : `matches!(c, '_' | '-' | '.' | '/' | '=')` → `matches!(c, '_' | '-' | '.' | '/')`
- **Contrainte** : garder `'/'` (nécessaire pour les chemins absolus d'éditeur)

### T2 — DRY: supprimer mastery_bar() locale dans ui_watch.rs [x] done
- **Fichier** : `src/tui/ui_watch.rs:57–68`
- **Fonctions à réutiliser** : `common::mastery_bar_string(score, width)` + `common::mastery_color(score)`
- **Fix** : supprimer `fn mastery_bar()` entière. Remplacer les 2 call sites (lignes ~82, ~213) :
  ```rust
  // AVANT
  let (bar, bar_color) = mastery_bar(mastery, 10);
  // APRÈS
  let bar = common::mastery_bar_string(mastery, 10);
  let bar_color = common::mastery_color(mastery);
  ```
- **Contraintes** : `FULL_BAR` dans common.rs supporte max width=10, OK pour width=8 et 10

### T3 — DRY: mastery_bar_spans() → common::mastery_bar_string [x] done
- **Fichier** : `src/tui/ui_stats.rs:22–31`
- **Fix** : remplacer `format!("{}{}", "█".repeat(filled), "░".repeat(empty))` par `common::mastery_bar_string(score, 10)`
- **Contraintes** : supprimer les calculs `filled`/`empty` redondants dans cette fonction

### T4 — DRY: extraire render_body_with_sidebar dans common.rs [x] done
- **Fichiers** : `src/tui/common.rs`, `src/tui/ui_watch.rs:157–194`, `src/tui/ui_piscine.rs:180–217`
- **Pattern** : voir findings.md section "Duplication de layout"
- **Ajouter dans common.rs** : `pub fn render_body_with_sidebar(f, area, state, render_sidebar: fn(&mut Frame, Rect, &AppState))`
- **Réduire** :
  - `render_body` → `render_body_with_sidebar(f, area, state, render_mastery_sidebar)`
  - `render_piscine_body` → `render_body_with_sidebar(f, area, state, render_piscine_sidebar)`
- **Lire avant de coder** : `src/tui/common.rs` (imports, exports existants)

### T5 — Dead code: AppMode + AppState.mode + Msg::Quit [x] done
- **Fichier principal** : `src/tui/app.rs:14–40`
- **Lire avant** : `src/commands/watch.rs`, `src/commands/piscine.rs` — call sites de `AppState::new()`
- **Fix** :
  - Supprimer `enum AppMode`
  - Changer `AppState::new(mode: AppMode)` → `AppState::new()` (retirer le paramètre)
  - Supprimer `pub mode: AppMode` + `#[allow(dead_code)]` correspondant
  - Supprimer `Msg::Quit` + `#[allow(dead_code)]`
  - Adapter les match arms `Msg::Quit` dans `update_watch` / `update_piscine` (les supprimer)
- **Contraintes** : vérifier que `AppState::new(AppMode::Watch {...})` ne passe pas de données utiles

### T6 — Dead code: WatchAction variants Skip/Next/Prev [x] done
- **Fichier** : `src/watcher.rs:23–32`
- **Lire avant** : `src/main.rs` — tous les `match` sur `WatchAction` (chercher `WatchAction::Skip`, `Next`, `Prev`)
- **Fix** : supprimer les 3 variants + leurs `#[allow(dead_code)]` si aucun call site trouvé
- **Contraintes** : si des match arms les référencent en main.rs → supprimer aussi ces bras

### T7 — Constantes: ajouter MASTERY_BAR_WIDTH + PISCINE_PROGRESS_BAR_WIDTH [x] done
- **Fichier** : `src/constants.rs`
- **Ajouter** :
  ```rust
  pub const MASTERY_BAR_WIDTH: usize = 10;
  pub const PISCINE_PROGRESS_BAR_WIDTH: usize = 20;
  ```
- **Faire avant T10** (T10 les utilise)

### T8 — Dead code: next_interval_days() + 3 tests [x] done
- **Fichier** : `src/mastery.rs`
- **Fix** : supprimer `pub(crate) fn next_interval_days(mastery: f32) -> u32` ET les 3 tests :
  `next_interval_days_min_clamp`, `next_interval_days_max_clamp`, `next_interval_days_mid`
- **Vérifier** : `grep -r "next_interval_days"` avant de supprimer

### T10 — Magic 20 → PISCINE_PROGRESS_BAR_WIDTH [x] done
- **Fichier** : `src/tui/ui_piscine.rs:234`
- **Après T7** (utilise la constante)
- **Fix** : `let bar_width = 20usize;` → `let bar_width = constants::PISCINE_PROGRESS_BAR_WIDTH;`
- **Ajouter** : `use crate::constants;` si absent

### T_VERIFY — cargo test + clippy [x] done
- `cargo test` — 152 tests passed ✓
- `cargo clippy -- -D warnings` — 0 warnings ✓

---

## Archivé

### v2.9.0 — TUI fuzzy search `[/]` [x] done
- AppState: search_active, search_query, search_results, search_selected
- rebuild_search, update_watch search block, render_search_overlay, ui_watch T4
