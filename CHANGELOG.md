# Changelog

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
