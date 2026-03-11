# Changelog

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
