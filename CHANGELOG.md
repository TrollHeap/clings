# Changelog

## [1.1.0-dev] — unreleased

### Added

- Git-based progress sync module (`clings sync init/status/now`)

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
