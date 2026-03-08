# C File I/O

File I/O in C uses `FILE*` stream handles. All operations go through the standard library (`<stdio.h>`).

## Key Functions

| Function | Purpose |
|----------|---------|
| `fopen(path, mode)` | Open file — returns `FILE*` or `NULL` on error |
| `fclose(f)` | Flush buffer and close; always required |
| `fprintf(f, fmt, ...)` | Formatted write to stream |
| `fgets(buf, n, f)` | Read up to n-1 chars; preserves newline |
| `fread(ptr, size, n, f)` | Binary read: n items of `size` bytes |
| `fwrite(ptr, size, n, f)` | Binary write |
| `fseek(f, offset, whence)` | Reposition: `SEEK_SET`, `SEEK_CUR`, `SEEK_END` |
| `ftell(f)` | Current position in bytes |
| `perror(msg)` | Print `msg: strerror(errno)` to stderr |
| `ferror(f)` | Non-zero if stream has an error flag set |

## Open Modes

- `"r"` — read (file must exist)
- `"w"` — write (creates or truncates)
- `"a"` — append
- `"rb"` / `"wb"` — binary variants (important for structs/images)

## Error Handling

```c
FILE *f = fopen("file.txt", "r");
if (f == NULL) { perror("fopen"); return 1; }
// ... use f ...
fclose(f);
```

## Buffering

stdio is line-buffered (terminal) or fully-buffered (files). Data may sit in the buffer until `fflush(f)` or `fclose(f)`. Call `fflush` if you need intermediate writes visible immediately.
