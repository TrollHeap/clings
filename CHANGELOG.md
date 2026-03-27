# Changelog

## [1.1.0-dev] — unreleased

### Added

- **NSY103 learning mode** (`--nsy103-only` flag in watch mode): dedicated subset of 103 exercises aligned with NSY103 core curriculum, separate from exam mode
- **NSY103 exam mode** (`clings exam`): timed session based on past NSY103 exam papers with interactive TUI session selector
- **NSY103 exam scenario exercises**: intermediate exercise sequences aligned with annales preparation
- **Unity C test validation mode**: exercises now support unit test harness validation (C with Unity framework) alongside output validation
- **Chapter-based reporting** (`clings report [chapter]`): per-chapter learning analytics (mastery distribution, completion rate, weak spots)
- **libsys library support**: validated C library export for selected exercises
- **Git-based progress sync module** (`clings sync init/status/now`): bidirectional sync with remote Git repository

### Changed

- **Exercise format migration**: TOML replaced JSON for exercise definitions (backwards-compatible loader)
- **TUI performance optimization**: cached header strings, eliminated hot-path allocations (60Hz rendering)
- **Refactored piscine/progress/error modules**: reduced duplication, aligned error handling conventions

## [1.0.1] — 2026-03-20

### Fixed

- Version string snapshots updated

## [1.0.0] — 2026-03-20

Initial public release. Consolidation of all internal development.

### Features

- 283+ C exercises covering 21 NSY103/UTC502 curriculum subjects
- Watch mode with gcc compilation, output validation, SRS progression
- Piscine mode (full linear walkthrough, persistent checkpoint)
- Exam mode with NSY103 past papers and interactive TUI selector
- Fuzzy search (`nucleo-matcher`): CLI (`clings search`) and TUI overlay (`[/]`)
- Combined output + unit test validation (mode `"both"`)
- Ratatui TUI with TEA/Elm architecture, Catppuccin Mocha palette
- SRS mastery system (spaced repetition, 14-day decay)
- Chapter-based progression through 16 NSY103 chapters
- Optional tmux integration (automatic neovim split)
- Shell completions (bash/zsh/fish) via `clap_complete`
- Exercise generator (`clings new`) and validator
- Progress export/import
- User configuration (`~/.clings/clings.toml`)
- Adaptive starter code by mastery stage (S0–S4)
- Interactive memory visualizer (stack/heap)

### Architecture

- Modules: `commands/` (5 handlers), `tui/` (12 modules), core (15 modules)
- Security: injection guard, path traversal protection, ReDoS guard, test_code validation
- Performance: zero allocation on TUI hot-path rendering (60Hz)
- Self-contained binary: exercises embedded via `rust-embed`
