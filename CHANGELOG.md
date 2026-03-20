# Changelog

## [1.0.0] — 2026-03-20

Release initiale publique. Consolidation de tout le développement interne.

### Fonctionnalités

- 283+ exercices C couvrant 21 sujets du curriculum NSY103/UTC502
- Mode watch avec compilation gcc, validation de sortie, progression SRS
- Mode piscine (parcours linéaire complet, checkpoint persistant)
- Mode exam avec annales NSY103 et sélecteur TUI interactif
- Recherche fuzzy (`nucleo-matcher`) : CLI (`clings search`) et overlay TUI (`[/]`)
- Validation combinée output + tests unitaires (mode `"both"`)
- TUI Ratatui avec architecture TEA/Elm, palette Catppuccin Mocha
- Système de maîtrise SRS (répétition espacée, décroissance 14 jours)
- Progression par chapitres NSY103 (16 chapitres)
- Intégration tmux optionnelle (split neovim automatique)
- Shell completions (bash/zsh/fish) via `clap_complete`
- Générateur d'exercices (`clings new`) et validateur
- Export/import de progression
- Configuration utilisateur (`~/.clings/clings.toml`)
- Starter code adaptatif par niveau de maîtrise (stages S0–S4)
- Visualiseur mémoire interactif (stack/heap)

### Architecture

- Modules : `commands/` (5 handlers), `tui/` (12 modules), core (15 modules)
- Sécurité : injection guard, path traversal protection, ReDoS guard, test_code validation
- Performance : zéro allocation hot-path dans le rendu TUI (60Hz)
- Binaire autonome : exercices embarqués via `rust-embed`
