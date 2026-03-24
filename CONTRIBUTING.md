# Contributing to clings

## Prerequisites

- Rust (stable toolchain, `rustup` recommended)
- `gcc` installed (`gcc --version` must work)
- `sqlite3` (optional, for inspecting the database)

## Build & test

```bash
cargo build                    # Debug build (binary: target/debug/clings)
cargo build --release          # Optimized build
cargo clippy -- -D warnings    # Lint — must pass clean
cargo test                     # Unit tests
cargo test <test_name>         # A specific test
```

All contributions must pass `cargo clippy -- -D warnings` and `cargo test` without regression.

## Exercise format

Exercises are JSON files in `exercises/<subject>/`. Each file defines:

```jsonc
{
  "id": "ptr-deref-01",
  "subject": "pointers",
  "lang": "c",
  "difficulty": 1,
  "title": "...",
  "description": "...",
  "starter_code": "...",
  "solution_code": "...",
  "hints": ["...", "..."],
  "validation": { "mode": "output", "expected_output": "..." },
}
```

The optional `starter_code_stages` field enables adaptive scaffolding S0–S4.
File naming: `<subject>_<num>.json` (e.g. `ptr_deref_01.json`).

## Commit convention

```
<type>(<scope>): <short description in imperative mood>

# Types: feat | fix | refactor | test | docs | chore
# Examples:
feat(exercises): add vm_mmap_01 virtual memory exercise
fix(runner): correct linker flags for semaphore exercises
docs(readme): update installation instructions
```

One commit per logical change. No `git push --force` on `main`.
