# Memory Allocation

## Overview

C gives the programmer direct control over heap memory through a small set of
standard library functions: `malloc`, `calloc`, `realloc`, and `free`. Unlike stack
memory (managed automatically by the compiler) or global memory, heap allocations
persist until explicitly freed. This power requires discipline: every allocation
must be paired with exactly one `free`, resources must not be accessed after
freeing, and allocations that can fail must be checked. Memory leaks, use-after-free,
and double-free errors are the most common and dangerous consequences of misuse.

## Key Concepts

- **`malloc(size_t n)`**: Allocates `n` bytes on the heap. Returns a `void *` to
  the block, or `NULL` on failure. Contents are uninitialized.
- **`calloc(size_t count, size_t size)`**: Allocates `count * size` bytes and
  zero-initializes them. Returns `NULL` on failure. Safer default than `malloc`
  when zero-initialization is needed.
- **`realloc(void *ptr, size_t new_size)`**: Resizes a previously allocated block.
  May move it to a new address. Returns `NULL` on failure — the original pointer
  remains valid in that case.
- **`free(void *ptr)`**: Releases the memory. `ptr` must be the exact pointer
  returned by `malloc`/`calloc`/`realloc`. Passing `NULL` to `free` is a safe
  no-op.
- **Memory leaks**: Failing to `free` every allocation. In long-running processes
  this eventually exhausts available memory.

## Common Patterns

### Basic `malloc` / `free`

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void) {
    size_t n = 5;
    int *arr = malloc(n * sizeof(int));
    if (arr == NULL) {
        perror("malloc");
        return 1;
    }

    for (size_t i = 0; i < n; i++) {
        arr[i] = (int)(i * i);
    }

    for (size_t i = 0; i < n; i++) {
        printf("%d ", arr[i]); // 0 1 4 9 16
    }
    printf("\n");

    free(arr);
    arr = NULL; // defensive: prevents accidental reuse
    return 0;
}
```

### Dynamic array growth with `realloc`

```c
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    size_t capacity = 4;
    size_t length   = 0;
    int *buf = malloc(capacity * sizeof(int));
    if (buf == NULL) { return 1; }

    int incoming[] = {10, 20, 30, 40, 50, 60};
    for (size_t i = 0; i < 6; i++) {
        if (length == capacity) {
            capacity *= 2;
            int *tmp = realloc(buf, capacity * sizeof(int));
            if (tmp == NULL) { free(buf); return 1; } // keep original on failure
            buf = tmp;
        }
        buf[length++] = incoming[i];
    }

    for (size_t i = 0; i < length; i++) { printf("%d ", buf[i]); }
    printf("\n");
    free(buf);
    return 0;
}
```

### `calloc` for zero-initialized structures

```c
#include <stdio.h>
#include <stdlib.h>

typedef struct {
    int id;
    double score;
} Record;

int main(void) {
    size_t count = 3;
    Record *records = calloc(count, sizeof(Record));
    if (records == NULL) { return 1; }

    // All fields are zero-initialized by calloc
    for (size_t i = 0; i < count; i++) {
        printf("records[%zu]: id=%d score=%.1f\n", i,
               records[i].id, records[i].score);
    }

    records[0].id = 1;
    records[0].score = 98.5;

    free(records);
    return 0;
}
```

## Common Mistakes

- **Using the return value of `realloc` directly into the original pointer**: If
  `realloc` fails it returns `NULL` but the original block is still valid. Writing
  `ptr = realloc(ptr, new_size)` loses the original pointer on failure, causing a
  leak. Always assign to a temporary and check before updating `ptr`.

- **Double-free**: Calling `free(p)` twice on the same pointer is undefined
  behavior, often leading to heap corruption. Set the pointer to `NULL` immediately
  after freeing so that a second `free(NULL)` is harmless.

- **Reading past allocated bounds**: `malloc(n)` allocates exactly `n` bytes;
  indexing `arr[n]` is undefined behavior even if no crash occurs. Use `n *
  sizeof(element)` and track the length separately.

## Further Reading

- [cppreference — malloc](https://en.cppreference.com/w/c/memory/malloc)
- [cppreference — realloc](https://en.cppreference.com/w/c/memory/realloc)
- [cppreference — free](https://en.cppreference.com/w/c/memory/free)
