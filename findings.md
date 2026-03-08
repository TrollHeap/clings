# CLI Findings — Phase 2 (Design + NSY103 Chapters)

## Discovery Results

### 14 Populated Subjects (3 exercises each = 42 total)
bitwise_ops, file_io, memory_allocation, message_queues, pipes, pointers,
processes, pthreads, semaphores, shared_memory, signals, sockets,
string_formatting, structs

### NSY103 Chapter Mapping
NSY103 "Linux: noyau et programmation système" follows this progression:

| Ch# | Topic | Subject(s) |
|-----|-------|-----------|
| 1 | Fondamentaux C : structs & pointeurs | `structs`, `pointers` |
| 2 | Manipulation de chaînes et bits | `string_formatting`, `bitwise_ops` |
| 3 | Allocation mémoire | `memory_allocation` |
| 4 | Entrées/sorties fichiers | `file_io` |
| 5 | Processus | `processes` |
| 6 | Signaux | `signals` |
| 7 | Tubes (pipes) | `pipes` |
| 8 | Files de messages | `message_queues` |
| 9 | Mémoire partagée | `shared_memory` |
| 10 | Sémaphores | `semaphores` |
| 11 | Threads POSIX | `pthreads` |
| 12 | Sockets | `sockets` |

### Current Ordering (main.rs:317-349)
`build_srs_order()` uses `mastery::priority_sorted()` (lowest mastery first),
then sorts by difficulty within each subject. No chapter/sequence concept.

### Display Code (display.rs)
- 420 lines, 15 public functions
- Uses `colored` crate for ANSI styling
- Progress bar: `[####------] 5/42 (11%)` — functional but plain
- Mini-map: dots (`.` pending, `*` done, `>` current)
- No box-drawing characters, no themed header, no chapter grouping
- Keybinds shown as flat text line

### Reusable Patterns
- `colored::Colorize` for all styling — `.bold()`, `.green()`, `.dimmed()`, etc.
- `clear_screen()` uses raw ANSI `\x1b[2J\x1b[H`
- `mastery_bar()` creates `##########` visual at `display.rs:408`
- Progress stored in SQLite via `progress.rs` — chapter order is code-only

### Files to Modify
- `cli/src/chapters.rs` — NEW: chapter definitions + ordering logic
- `cli/src/main.rs` — replace `build_srs_order()` with chapter-based ordering
- `cli/src/display.rs` — redesign all display functions
- `cli/src/exercises.rs` — add chapter-aware grouping
