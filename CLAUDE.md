# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is this?

clings (`clings`) is a rustlings-style TUI trainer for C systems programming, aligned with the NSY103 "Linux: noyau et programmation système" curriculum. Users solve C exercises in their editor while `clings` watches for file saves, compiles with `gcc`, validates output, and tracks mastery via an SRS (spaced repetition) system.

## Build & Development

```bash
cargo build                      # Build (binary: target/debug/clings)
cargo clippy -- -D warnings      # Lint (must pass clean)
cargo test                       # Run all tests (currently in mastery.rs)
cargo test test_score_increment  # Run a single test by name
```

The binary name is `clings` (defined in Cargo.toml `[[bin]]`). Runtime requires `gcc` installed on the system.

## Architecture

### Data flow

```
exercises/*.json → exercises.rs (load/parse) → chapters.rs (order by NSY103 curriculum)
                                              → runner.rs (gcc compile → run → validate output)
                                              → progress.rs (SQLite DB) ↔ mastery.rs (SRS algorithm)
                                              → display.rs (TUI rendering with colored + box-drawing)
```

### Module responsibilities

- **`main.rs`** — CLI entry point (clap). Subcommands: `watch` (default), `list`, `run`, `progress`, `hint`, `solution`, `reset`, `piscine`, `review`, `stats`, `annales`, `export`, `import`. Manages terminal raw mode via `libc::termios` directly. Raw mode uses RAII guard (`RawModeGuard`) restored on drop.
- **`exercises.rs`** — Loads exercise JSON files recursively from `exercises/` directory. Resolution order: `CLINGS_EXERCISES` env var → ancestors of binary path → CWD-relative. `annales_map.json` lives in the same exercises directory.
- **`chapters.rs`** — Hardcoded NSY103 chapter progression (12 chapters mapping subjects to curriculum order). Orders exercises: chapter → difficulty → SRS priority (lowest mastery first).
- **`runner.rs`** — Compiles C code with `gcc -Wall -Wextra -std=c11`, writes starter code to `~/.clings/current.c`, validates program output against expected. Subject-specific linker flags (`-lpthread`, `-lrt`). Custom `wait_timeout` trait on `Child` for 10s execution limit.
- **`progress.rs`** — SQLite database at `~/.clings/progress.db` with WAL mode. Two tables: `subjects` (mastery tracking) and `practice_log` (attempt history). Also stores piscine checkpoint (`load_piscine_checkpoint` / `save_piscine_checkpoint` / `clear_piscine_checkpoint`). Handles streak, due subjects for review, and SRS decay.
- **`mastery.rs`** — SRS algorithm: mastery score 0.0–5.0, success +1.0 / failure -0.5, difficulty unlock at thresholds (D2 at 2.0, D3 at 4.0, D4 at 4.5, D5 at 5.0), 14-day decay, review interval with 2.5x multiplier.
- **`piscine.rs`** — Mode piscine: linear progression through all exercises (no difficulty gating), with checkpoint persistence. Ordered by chapter then difficulty. Keys: `[h]` hint, `[n]` skip, `[q]` quit, `[r]` compile+check.
- **`display.rs`** — All TUI output using `colored` crate and Unicode box-drawing. Progress bars (`█▓░`), mini-map (`●◉○`), chapter indicators. Includes `show_visualizer` for the interactive memory visualizer and `show_annales` for past exams mapping.
- **`watcher.rs`** — File watcher (`notify` crate) with 200ms debounce + keyboard input via separate stdin thread. Returns `WatchAction` enum.
- **`tmux.rs`** — Optional tmux integration: auto-opens neovim in a split pane when running inside tmux.
- **`models.rs`** — Core types: `Exercise`, `Subject`, `Difficulty`, `ValidationMode`, `ExerciseType`, `Lang`. Exercises support staged starter code (S0–S4) based on mastery. Contains `Visualizer` / `VisStep` / `VisVar` for the interactive memory viewer.

### Exercise format

Exercises are JSON files in `exercises/<subject>/` directories. Required fields: `id`, `subject`, `lang`, `difficulty`, `title`, `description`, `starter_code`, `solution_code`, `hints`, `validation` (`mode`, `expected_output`). Optional fields: `starter_code_stages` (S0–S4 for adaptive scaffolding), `files` (aux files copied to `~/.clings/`), `exercise_type` (`complete`|`fix_bug`|`fill_blank`|`refactor`), `key_concept`, `common_mistake`, `kc_ids`, `visualizer` (memory diagram steps with `stack`/`heap` snapshots).

`exercises/annales_map.json` maps past NSY103 exam questions to exercise IDs (used by `clings annales`).

### Key conventions

- Error handling uses `thiserror` with `KfError` enum in `src/error.rs` and crate-local `Result<T>` alias
- UI text is bilingual: French for user-facing messages, English for code/technical terms
- `ValidationMode::Test` and `Both` are stubbed — exercises with these modes are skipped silently in `watch` and `piscine`. Only `Output` validation works.
- Working directory for user code: `~/.clings/`
- Exercise ordering is curriculum-driven, not alphabetical
- Watch-mode keybinds: `[h]` hint, `[j]` next, `[k]` prev, `[n]` skip, `[q]` quit, `[r]` compile+check (no auto-compile on save), `[l]` list exercises, `[v]` open memory visualizer (arrow keys to step through, any key to close). Piscine lacks `[j]`, `[k]`, `[l]` and `[v]`.
- Tests exist in both `mastery.rs` and `models.rs`

## Curricula

- **NSY103-core** : structs, pointers, string_formatting, bitwise_ops, memory_allocation, errno,
  file_io, fd_basics, filesystem, processes, signals, pipes, message_queues, shared_memory,
  semaphores, sync_concepts, pthreads, sockets, proc_memory
- **UTC502-extended** : scheduling (Ch.1/Ch.6), virtual_memory (Ch.4 — page replacement)
- **Annales de référence** :
  - NSY103 : 3 annales dans `docs/nsy103/`
  - UTC502 : 2 annales + TPs dans `docs/utc502/`
