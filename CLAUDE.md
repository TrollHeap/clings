# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is this?

KernelForge CLI (`kf`) is a rustlings-style TUI trainer for C systems programming, aligned with the NSY103 "Linux: noyau et programmation système" curriculum. Users solve C exercises in their editor while `kf` watches for file saves, compiles with `gcc`, validates output, and tracks mastery via an SRS (spaced repetition) system.

## Build & Development

```bash
cargo build                      # Build (binary: target/debug/kf)
cargo clippy -- -D warnings      # Lint (must pass clean)
cargo test                       # Run all tests (currently in mastery.rs)
cargo test test_score_increment  # Run a single test by name
```

The binary name is `kf` (defined in Cargo.toml `[[bin]]`). Runtime requires `gcc` installed on the system.

## Architecture

### Data flow

```
exercises/*.json → exercises.rs (load/parse) → chapters.rs (order by NSY103 curriculum)
                                              → runner.rs (gcc compile → run → validate output)
                                              → progress.rs (SQLite DB) ↔ mastery.rs (SRS algorithm)
                                              → display.rs (TUI rendering with colored + box-drawing)
```

### Module responsibilities

- **`main.rs`** — CLI entry point (clap). Subcommands: `watch` (default), `list`, `run`, `progress`, `hint`, `solution`, `reset`. Manages terminal raw mode via `libc::termios` directly.
- **`exercises.rs`** — Loads exercise JSON files recursively from `exercises/` directory. Resolution order: `KERNELFORGE_EXERCISES` env var → ancestors of binary path → CWD-relative.
- **`chapters.rs`** — Hardcoded NSY103 chapter progression (12 chapters mapping subjects to curriculum order). Orders exercises: chapter → difficulty → SRS priority (lowest mastery first).
- **`runner.rs`** — Compiles C code with `gcc -Wall -Wextra -std=c11`, writes starter code to `~/.kernelforge/current.c`, validates program output against expected. Subject-specific linker flags (`-lpthread`, `-lrt`). Custom `wait_timeout` trait on `Child` for 10s execution limit.
- **`progress.rs`** — SQLite database at `~/.kernelforge/progress.db` with WAL mode. Two tables: `subjects` (mastery tracking) and `practice_log` (attempt history). Handles streak calculation.
- **`mastery.rs`** — SRS algorithm: mastery score 0.0–5.0, success +1.0 / failure -0.5, difficulty unlock at thresholds (D2 at 2.0, D3 at 4.0), 14-day decay, review interval with 2.5x multiplier.
- **`display.rs`** — All TUI output using `colored` crate and Unicode box-drawing. Progress bars (`█▓░`), mini-map (`●◉○`), chapter indicators.
- **`watcher.rs`** — File watcher (`notify` crate) with 200ms debounce + keyboard input via separate stdin thread. Returns `WatchAction` enum.
- **`tmux.rs`** — Optional tmux integration: auto-opens neovim in a split pane when running inside tmux.
- **`models.rs`** — Core types: `Exercise`, `Subject`, `Difficulty`, `ValidationMode`, `ExerciseType`, `Lang`. Exercises support staged starter code (S0–S4) based on mastery.

### Exercise format

Exercises are JSON files in `exercises/<subject>/` directories. Each defines: id, subject, lang, difficulty, title, description, starter_code, solution_code, hints, validation (expected_output), and optional `starter_code_stages` for adaptive scaffolding.

### Key conventions

- Error handling uses `thiserror` with `KfError` enum in `src/error.rs` and crate-local `Result<T>` alias
- UI text is bilingual: French for user-facing messages, English for code/technical terms
- The `ValidationMode::Test` path is stubbed (returns `false`) — only `Output` validation works
- Working directory for user code: `~/.kernelforge/`
- Exercise ordering is curriculum-driven, not alphabetical
