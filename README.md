# clings

**Interactive C systems programming trainer, aligned with the NSY103 — Linux: kernel and system programming curriculum.**

Solve 283 C exercises directly in your editor. `clings` watches your saves, compiles with `gcc`, validates output, and measures your mastery via a spaced repetition algorithm (SRS).

---

## Installation

**Prerequisites:** Rust (stable toolchain), `gcc` installed on the system.

```bash
# From source
git clone https://github.com/trollheap/clings
cd clings
cargo build --release
# Binary: target/release/clings

# Or directly into ~/.cargo/bin/
cargo install --path .
```

---

## Commands

| Command                     | Description                                                                   |
| --------------------------- | ----------------------------------------------------------------------------- |
| `clings` or `clings watch`  | SRS watch mode: exercises prioritized by mastery level, automatic progression |
| `clings list`               | List all exercises (optional filter: `--subject <subject>`)                   |
| `clings run <id>`           | Run a specific exercise in watch mode (e.g. `clings run ptr-deref-01`)        |
| `clings progress`           | Dashboard: mastery by subject, consecutive-day streak                         |
| `clings hint <id>`          | Display exercise hints                                                        |
| `clings solution <id>`      | Display the solution (requires at least one attempt)                          |
| `clings reset`              | Reset all progress (confirmation required)                                    |
| `clings piscine`            | Piscine mode: full linear walkthrough, all exercises unlocked from the start  |
| `clings review`             | SRS review: exercises due according to spaced repetition intervals            |
| `clings stats`              | Global statistics: success rate, mastery distribution                         |
| `clings annales`            | Exercise ↔ NSY103/UTC502 past exam topic mapping                              |
| `clings exam`               | Simulated exam mode: reproduces an NSY103/UTC502 past paper with a timer      |
| `clings export`             | Export progress to a JSON file                                                |
| `clings import <file>`      | Import progress from a JSON file                                              |
| `clings config <key> <val>` | Modify user configuration (`~/.clings/clings.toml`)                           |
| `clings new`                | Generate an exercise skeleton or validate an existing JSON file               |
| `clings search <query>`     | Fuzzy search over exercises (title, ID, subject, key concepts)                |
| `clings sync`               | Sync progress between machines via Git (`init`, `status`, `now`)              |
| `clings completions`        | Generate shell completions (bash/zsh/fish)                                    |
| `clings schema`             | Export the exercise JSON Schema format                                        |

### Keyboard shortcuts (watch mode)

| Key | Action                                              |
| --- | --------------------------------------------------- |
| `h` | Show a hint                                         |
| `s` | Solution overlay (all hints revealed or 3 failures) |
| `j` | Next exercise (curriculum order)                    |
| `n` | Skip the current exercise                           |
| `k` | Go back to the previous exercise                    |
| `r` | Compile and check now                               |
| `l` | Show exercise list                                  |
| `v` | Open memory visualizer                              |
| `?` | Show help                                           |
| `/` | Fuzzy search overlay                                |
| `q` | Quit                                                |

---

## How it works

1. `clings watch` selects the next exercise based on the SRS algorithm (lowest mastery first).
2. The starter code is written to `~/.clings/current.c`.
3. Open this file in your editor and modify it.
4. On each save, `clings` compiles with `gcc -Wall -Wextra -std=c11` and compares output to the expected value.
5. On success, the subject's mastery increases (+1.0) and the next exercise is loaded.
6. On a runtime failure (no compilation error), mastery decreases (−0.5).

### SRS Algorithm

- Mastery score: 0.0 to 5.0 per subject
- Difficulty D2 unlocked at 2.0 — D3 at 4.0
- 14-day decay for inactivity
- Review intervals multiplied by 2.5

### Validation modes

Exercises support several validation modes:

- **`output`** — validates the program's stdout (default mode)
- **`test`** — validates via C unit tests (Unity framework: `UNITY_BEGIN`/`UNITY_END`/`RUN_TEST` macros)

### Content

- **283+ exercises** across **21 subjects**
- **16 NSY103 chapters**: C Fundamentals → Strings & Bitwise → Memory Allocation → I/O → Filesystem → Scheduling → Processes → Signals → Pipes → Message Queues → Shared Memory → Semaphores → POSIX Threads → Sockets → Virtual Memory → Integrative Projects
- Difficulty levels D1 to D5
- Adaptive starter code by stage (S0–S4) based on mastery

---

## Curricula covered

### NSY103 — Linux: kernel and system programming (Cnam)

Primary curriculum. Covers the 16 chapters of the NSY103 course: processes, signals, pipes, POSIX IPC (message queues, shared memory, semaphores), POSIX threads, sockets, filesystem, virtual memory.

### UTC502 — Computer resource management

Additional exercises aligned with UTC502: page replacement algorithms, CPU scheduling policies, advanced virtual memory.

| Subject                                                     | Curriculum      | Chapter |
| ----------------------------------------------------------- | --------------- | ------- |
| `processes`, `signals`, `pipes`, `file_io`                  | NSY103          | 7–10    |
| `pthreads`, `semaphores`, `shared_memory`, `message_queues` | NSY103          | 11–13   |
| `sockets`                                                   | NSY103          | 14      |
| `virtual_memory` (mmap, cow, tlb, brk…)                     | NSY103 + UTC502 | 15      |
| `virtual_memory` (page_replacement_fifo/lru/opt)            | UTC502          | 15      |
| `scheduling` (fifo, rr, sjf, priority)                      | UTC502          | 6       |

---

## Configuration

| Parameter          | Default value | Description                                                |
| ------------------ | ------------- | ---------------------------------------------------------- |
| Working directory  | `~/.clings/`  | Current file, SQLite database                              |
| `CLINGS_EXERCISES` | _(unset)_     | Alternative path to the `exercises/` directory             |
| tmux integration   | automatic     | If `clings` runs inside tmux, opens neovim in a split pane |

The progress database is located at `~/.clings/progress.db`.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

---

## License

MIT
