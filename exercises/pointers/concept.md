# Pointers

## Overview

A pointer is a variable that stores the memory address of another variable. Pointers
are central to C — they enable dynamic memory allocation, efficient array traversal,
passing large structures to functions by address, and building data structures such
as linked lists and trees. Understanding pointer arithmetic, dereferencing, and the
special meaning of `NULL` is essential before writing non-trivial C programs.

## Key Concepts

- **Declaration and address-of**: `int *p;` declares a pointer to `int`. The
  address-of operator `&x` produces a pointer to `x`. Assign with `int *p = &x;`.
- **Dereferencing**: `*p` accesses the value at the address stored in `p`. Reading
  or writing through a null or uninitialized pointer is undefined behavior.
- **Pointer arithmetic**: Adding an integer `n` to a pointer advances it by
  `n * sizeof(pointed-to type)` bytes. This is how arrays are traversed.
- **`NULL`**: A sentinel value (typically `(void *)0`) indicating a pointer that
  does not point to valid memory. Always initialize pointers and check for `NULL`
  before dereferencing.
- **Double pointers (`T **`)**: A pointer to a pointer. Used to modify a pointer
  inside a function (simulating pass-by-reference for pointers) or to represent
  two-dimensional arrays.

## Common Patterns

### Basic pointer operations and pointer arithmetic

```c
#include <stdio.h>

int main(void) {
    int arr[] = {10, 20, 30, 40, 50};
    int *p = arr; // points to arr[0]

    for (int i = 0; i < 5; i++) {
        printf("arr[%d] = %d  (address %p)\n", i, *(p + i), (void *)(p + i));
    }

    // Pointer difference tells you how many elements apart two pointers are.
    int *first = &arr[0];
    int *last  = &arr[4];
    printf("distance: %td elements\n", last - first); // 4

    return 0;
}
```

### Passing a pointer to modify a variable

```c
#include <stdio.h>

void swap(int *a, int *b) {
    int tmp = *a;
    *a = *b;
    *b = tmp;
}

int main(void) {
    int x = 3, y = 7;
    swap(&x, &y);
    printf("x=%d, y=%d\n", x, y); // x=7, y=3
    return 0;
}
```

### Double pointers — modifying a pointer inside a function

```c
#include <stdio.h>
#include <stdlib.h>

void allocate(int **out, int value) {
    *out = malloc(sizeof(int));
    if (*out == NULL) { return; }
    **out = value;
}

int main(void) {
    int *p = NULL;
    allocate(&p, 42);
    if (p != NULL) {
        printf("value: %d\n", *p); // 42
        free(p);
    }
    return 0;
}
```

## Common Mistakes

- **Dereferencing a `NULL` or uninitialized pointer**: Accessing memory through a
  `NULL` pointer causes a segmentation fault. Always initialize pointers to `NULL`
  and check before dereferencing: `if (p != NULL) { *p = ...; }`.

- **Pointer arithmetic past array bounds**: Advancing a pointer beyond one element
  past the end of an array and then dereferencing it is undefined behavior. Keep
  arithmetic within `[arr, arr + N]`; only the one-past-the-end address may be
  computed (not dereferenced).

- **Returning a pointer to a local variable**: Local variables live on the stack
  and are destroyed when the function returns. Returning a pointer to one yields a
  dangling pointer. Return a heap-allocated value (via `malloc`) and document that
  the caller must `free` it.

## Further Reading

- [cppreference — Pointer declaration](https://en.cppreference.com/w/c/language/pointer)
- [cppreference — Pointer arithmetic](https://en.cppreference.com/w/c/language/operator_arithmetic)
- [cppreference — NULL](https://en.cppreference.com/w/c/types/NULL)
