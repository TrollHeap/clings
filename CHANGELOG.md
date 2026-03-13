# Changelog

## [3.0.0] — 2026-03-13

### Remédiation audit — HIGH findings

- **DRY `handle_search_key()`** : 65 lignes dupliquées entre `update_watch()` et `update_piscine()` extraites dans un helper statique retournant `bool` ; les callers gèrent les effets de bord (load exercice, save checkpoint)
- **DRY `handle_vis_key()`** : bloc visualiseur dupliqué remplacé par un helper dédié dans les deux modes
- **Erreurs silencieuses checkpoints** : 10+ `let _ = save_piscine_checkpoint(…)` / `let _ = save_exam_checkpoint(…)` remplacés par `self.save_checkpoint(conn, session_id, idx)` — logue les erreurs via `eprintln!`
- **Triple scan HashMap O(n)** : 3 itérations `review_map.values().filter(…).count()` par frame dans `ui_watch.rs` remplacées par `state.due_count()` (méthode sur `AppState`)
- **Allocation zéro par frame** : `mini_map()` et `render_visualizer_overlay()` dans `common.rs` — `Vec<&str>` intermédiaire remplacé par `String::with_capacity` + `push_str`
- **Panic sur embedded file** : `.unwrap()` dans `exercises.rs` remplacé par `ok_or_else()?` — erreur propagée proprement

---

## [2.9.9] — 2026-03-13

### TUI — Finalisation série v2.9.x

- **v2.9.7** — Overlay d'aide `[?]` en mode watch : popup 60%×70% listant tous les raccourcis, fermé par n'importe quelle touche. `help_active: bool` dans `AppState`. `render_help_overlay()` dans `common.rs`. Status bar : `[?] aide` dans les hints normaux, `[Esc/?] fermer` quand actif.
- **v2.9.8** — Navigation vim `[g]`/`[G]` dans l'overlay search (watch + piscine) : `G` → dernier résultat, `gg` → premier résultat. `search_g_pending: bool` dans `AppState`. Hint bar mise à jour : `[g/G] début/fin`.
- **v2.9.2–v2.9.6** (consolidés) — Fix sélection invisible (`ListState` + all items), `status_msg_at` expiration 3s, filtre sujet `[Tab]` en recherche, perf sidebar O(n) → O(1) via `subject_order` cache, search piscine.

### Perf — Zéro allocation par frame dans render_search_overlay

- `render_search_overlay` (`common.rs`) : suppression du `Vec<(&Exercise, usize)>` intermédiaire — itération directe sur `search_results`
- Troncature UTF-8 safe via `char_indices().nth(N)` — élimine les allocations `String` temporaires par item de liste
- Résultat : 0 allocation par frame dans le chemin de rendu search (60Hz)

---

## [2.9.1] — 2026-03-13

### Cohérence — documentation et tests search

- CLAUDE.md : `[/]` ajouté dans la liste des keybinds watch-mode ; note tests mise à jour (`search.rs` inclus)
- `src/search.rs` : 4 tests smoke — query vide (match universel), query connue, filtre sujet, tri par score décroissant
- Découverte : nucleo matche tout avec query vide (bypass dans `rebuild_search` reste pour la perf)

---

## [2.9.0] — 2026-03-13

### TUI — Recherche fuzzy `[/]`

- Touche `[/]` depuis le mode watch ouvre un overlay de recherche en temps réel
- Filtrage fuzzy via `nucleo-matcher` (réutilise `search::search_exercises`)
- Navigation `[j]`/`[k]`/flèches, `[Entrée]` pour sauter à l'exercice, `[Esc]` pour fermer
- `AppState` enrichi : `search_active`, `search_query`, `search_results`, `search_selected`
- Overlay centré 80%×70% avec curseur clignotant animé et liste `ListState` stateful
- Status bar contextuelle : affiche les raccourcis search quand l'overlay est actif
- `[/] search` ajouté dans les keybinds normaux du status bar

---

## [2.8.0] — 2026-03-13

### Refactor DRY — extraction common.rs

- `src/tui/common.rs` créé : ~280 lignes extraites de `ui_watch.rs` et `ui_piscine.rs`
- Fonctions mutualisées : rendu sidebar, mastery bar, header, layout helpers
- Suppression de la duplication entre les 9 fonctions dupliquées identifiées dans l'audit v2.7.0

---

## [2.7.0] — 2026-03-12

### TUI — Migration Ratatui v3

- Refonte complète de la couche d'affichage : module `src/display/` supprimé (~1 700 lignes), remplacé par `src/tui/` avec architecture TEA (The Elm Architecture)
- `ui_watch.rs` : vue watch en Ratatui — layout 2 colonnes (≥90 cols), sidebar mastery/sujet, mastery bar colorée (vert/jaune/rouge), fond opaque, header L4
- `ui_piscine.rs` : vue piscine en Ratatui — timer Gauge, progression en temps réel, fond opaque
- `ui_run.rs` : vue résultat de compilation/exécution
- `ui_exam_selector.rs` : sélecteur d'examens avec flèches/jk + Entrée
- `ui_annales.rs`, `ui_list.rs`, `ui_stats.rs` : vues info (annales, liste exercices, statistiques)
- `app.rs` + `events.rs` : AppState centralisé, event loop TEA, gestion crossterm 0.29

### Mise à jour dépendances

- `ratatui` 0.29 → **0.30**
- `crossterm` 0.28 → **0.29**

### Audit remediation v2.6.1

- `progress.rs` : `apply_all_decay()` accepte `decay_days: i64` en paramètre — WHERE SQL direct plutôt que filtrage Rust
- `progress.rs` : `get_streak()` `LIMIT 365` → `LIMIT 90` — évite de charger plus de 3 mois d'historique
- `runner.rs` : `normalize()` réécrit avec `String::with_capacity` + boucle `lines()` — réduit les allocations
- `piscine.rs` : import `crossterm::event` dédoublonné supprimé
- `commands/data.rs`, `exam.rs` : commentaires `// best-effort flush — non-critique` ajoutés sur tous les sites de flush silencieux

---

## [2.6.0] — 2026-03-12

### Fonctionnalités

- **`clings search`** : recherche hybride BM25 + sémantique via `nucleo-matcher` — trouve les exercices par mot-clé
- **Shell completions** : `clings completions <shell>` génère les complétions bash/zsh/fish via `clap_complete`
- **Binaire autonome** : exercices embarqués via `rust-embed` — le binaire fonctionne sans répertoire `exercises/` adjacent

### Architecture

- `main.rs` (1 009 lignes) modularisé → `src/commands/` : `watch.rs`, `run.rs`, `info.rs`, `progress_cmds.rs`, `data.rs`
- `runner.rs` : `Child::wait_timeout()` remplacé par polling `try_wait()` — supprime la dépendance `wait-timeout`

### Audit remediation

- `mastery.rs` / `models.rs` : `.chars().next().unwrap()` remplacé par `.chars().next().unwrap_or_default()` — supprime le panic sur slice vide
- `constants.rs` : seuils `PCT_GREEN_THRESHOLD` / `PCT_YELLOW_THRESHOLD` centralisés
- `exam.rs` : utilise `display::header_box()` — supprime l'affichage dupliqué
- `search.rs` : buffer char réutilisé entre appels — 288 → 1 allocation par recherche
- `progress.rs` : `trim_practice_log()` — rétention limitée à 10 000 lignes dans `practice_log`
- `progress.rs` : `get_streak()` plafonnée à `LIMIT 365`

---

## [2.5.0] — 2026-03-12

### Sécurité
- `tmux.rs` : `is_valid_executable()` utilise désormais des arguments positionnels (`sh -c "command -v \"$1\"" -- bin`) au lieu d'une interpolation directe — élimine le vecteur d'injection shell
- `tmux.rs` : validation des arguments éditeur (EDITOR/VISUAL) — filtre les caractères dangereux (`'`, `"`, `;`, `|`, `&`, etc.) avant passage à tmux

### Performance
- `display/mod.rs` : `wrap_text()` utilise `std::mem::take()` au lieu de `.clone()` — zéro allocation supplémentaire par ligne
- `progress.rs` : `get_streak()` plafonnée à `LIMIT 365` — évite de charger tout l'historique
- `runner.rs` : `test_code` passé comme `&str` (`.as_str()`) au lieu de cloné
- `main.rs` : variable `exercise_clone` supprimée dans la boucle watch — `&Exercise` utilisé directement
- `main.rs` : lookup O(1) via HashMap dans `cmd_review` — `.cloned()` inutile supprimé

### DRY
- `constants.rs` : `SECS_PER_DAY`, `ANSI_ESC_BYTE`, `ANSI_CLEAR_SCREEN` centralisés
- `display/mod.rs` : `ArrowKey` enum + `try_parse_arrow()` — déduplication de la détection touches fléchées (visualizer + annales)
- `display/mod.rs` : `color_pct()` helper — coloration verte/jaune/rouge centralisée
- `display/stats.rs` : `avg_mastery()` extraite comme fonction privée réutilisable
- `display/keybinds.rs` : `show_keybinds_list()` privée supprimée, inlinée dans `show_keybinds_with_vis()`

### Conventions
- `progress.rs` : `import_progress()` retourne `(usize, Vec<String>)` — les avertissements de clamping sont désormais surfacés à l'utilisateur lors de `clings import`
- `display/stats.rs` : `avg_mastery()` protégée contre la division par zéro sur slice vide

## [2.4.0] — 2026-03-12

### Fonctionnalités

- **`clings exam` sans argument** : sélecteur TUI interactif (flèches/jk + Entrée) avec mémorisation de la dernière session choisie
- **`clings new`** : générateur d'exercices assisté (`--subject`, `--difficulty`, `--mode`, `--output`) et validateur (`--validate-only`)
- **Capstone `mode: "both"`** : capstone_allocator_01, capstone_prodcons_01, capstone_shell_01 convertis vers validation combinée output + tests unitaires

### Base de données

- Nouvelle table `exercise_scores` : suivi des tentatives et succès par exercice (migration additive v1, non-breaking)
- `clings review` utilise désormais les scores par exercice pour prioriser les exercices les plus faibles

### Performance

- `cmd_review` : une requête SQL batch (window function `ROW_NUMBER OVER PARTITION BY`) remplace N+1 requêtes par sujet
- Constantes nommées pour les seuils de mastery bar et le comptage top-N

### Qualité

- Module `config.rs` : gestion centralisée de la configuration utilisateur (`~/.clings/clings.toml`)
- Documentation `//!` ajoutée sur les modules `runner`, `mastery`, `exercises`, `watcher`, `tmux`

### Corrections

- `config.rs` : clés ALLOWED corrigées (`tmux.enabled`, `ui.tmux_pane_width`)

## [1.0.1] — 2026-03-11

### Refactoring
- `handle_esc_sequence` extrait dans `display/visualizer.rs` (était inline dans `runner.rs`)
- `ValidationConfig` nettoyée : `deny_unknown_fields` retiré, champs legacy supprimés

### Corrections
- `AtomicBool::store` utilise désormais `Ordering::Release` (cohérence avec `Acquire` sur load)

## [1.0.0] — 2026-03-11

### Sécurité
- Path traversal : validation par canonicalize() après création de fichier (`runner.rs`)
- Écriture atomique de `current.c` via rename() sur fichier temporaire (`runner.rs`)
- Erreur explicite si `$HOME` non défini au lieu d'un fallback silencieux vers CWD

### Fonctionnalités pédagogiques
- Affichage de `common_mistake` dans les indices (bloc jaune, `clings hint`)
- Affichage de `key_concept` dans l'en-tête de l'exercice (mode watch)

### Exercices ajoutés
- `filesystem` : fs_inode_calc_01 (D2), fs_inode_calc_02 (D3), fs_inode_calc_03 (D3)
- `processes` : fork_tree_01 (D3) — arbre de fork sans _exit()
- `scheduling` : sched_edf_01 (D4), sched_priority_arrival_01 (D3),
  sched_priority_inversion_01 (D4), sched_rr_gantt_01 (D3)

### Corrections
- README : table des commandes complétée (review, stats, annales, export, import)
- README : distinction `j` (next) vs `n` (skip) dans les raccourcis clavier
- SRS : multiplicateur de décroissance ajusté à 1.8 (cohérent avec intervalles 14 jours)

## [0.1.0] — 2025-11-01
- Version initiale avec 283+ exercices C couvrant 21 sujets
- Mode watch avec compilation gcc, validation de sortie, progression SRS
- Système de maîtrise SRS (répétition espacée, décroissance 14 jours)
- Progression par chapitres NSY103 (15 chapitres)
- Mode piscine (parcours linéaire complet, checkpoint persistant)
- Intégration tmux optionnelle (split neovim automatique)
