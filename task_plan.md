# CLI Improvements Plan — Phase 2

## Task 1: Create chapter ordering system (`chapters.rs`)
- **Files**: new `cli/src/chapters.rs`, modify `cli/src/main.rs` (add `mod chapters`)
- **Status**: [x] complete

### Changes:
- Define `Chapter` struct: `number: u8`, `title: &str`, `subjects: &[&str]`
- Hardcode NSY103 chapter list (12 chapters, 14 subjects)
- Function `chapter_order() -> &[Chapter]` returning static slice
- Function `order_exercises_by_chapter(exercises, subjects) -> Vec<(chapter, exercises)>`
  - Primary: chapter order (1→12)
  - Secondary within chapter: difficulty (Easy→Hard)
  - Tertiary: SRS priority (lowest mastery first) for same difficulty
- Function `flatten_chapter_exercises(...)` → flat `Vec<&Exercise>` for watch mode

### Chapter definitions:
```
1. Fondamentaux C          → structs, pointers
2. Chaînes & bits          → string_formatting, bitwise_ops
3. Allocation mémoire      → memory_allocation
4. Entrées/sorties         → file_io
5. Processus               → processes
6. Signaux                 → signals
7. Tubes                   → pipes
8. Files de messages       → message_queues
9. Mémoire partagée        → shared_memory
10. Sémaphores             → semaphores
11. Threads POSIX          → pthreads
12. Sockets                → sockets
```

## Task 2: Redesign CLI display (`display.rs`)
- **Files**: `cli/src/display.rs`
- **Status**: [x] complete

### Changes:
- Add themed header with box-drawing: `╔═══ KERNELFORGE ═══╗`
- Redesign progress bar with block characters: `█▓░`
- Add chapter indicator: `Chapter 3/12 — Allocation mémoire`
- Redesign mini-map: use `●` (done), `◉` (current), `○` (pending)
- Add separator lines between sections with `─` characters
- Redesign result display: framed success/error boxes
- Redesign keybinds: `[h] hint  [n] skip  [q] quit` style
- Update `show_exercise_list()` to group by chapter, not just subject
- Update `show_progress()` to show chapter-based progression

## Task 3: Wire chapter ordering into watch mode
- **Files**: `cli/src/main.rs`
- **Status**: [x] complete

### Changes:
- Replace `build_srs_order()` with `chapters::order_exercises_by_chapter()`
- Pass chapter info to display functions for chapter header rendering
- Update `show_exercise_watch()` call to include chapter context
- Update `cmd_list()` to show chapter-grouped output

## Task 4: Verify
- **Status**: [x] complete
- `cargo clippy -- -D warnings`
- `cargo test`
- Manual test: `kf list`, `kf progress`, `kf watch`
