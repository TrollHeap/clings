# String Formatting in C

## Overview

C provides a family of functions for formatted string input and output. The `printf` family writes formatted output, while `scanf` parses formatted input. Understanding format specifiers, width and precision modifiers, and buffer safety is essential for working with strings in C.

## Key Concepts

- **Format specifiers** control how values are converted to text: `%d` (int), `%f` (double), `%s` (string), `%c` (char), `%p` (pointer), `%x` (hex), `%o` (octal), `%u` (unsigned)
- **Width and precision** modifiers refine output: `%10d` (right-aligned, width 10), `%-10s` (left-aligned), `%.3f` (3 decimal places), `%.*s` (precision from argument)
- **`snprintf`** is the safe alternative to `sprintf` — it takes a buffer size parameter and never writes beyond it
- **`sscanf`** parses formatted data from a string, the inverse of `sprintf`
- **Length modifiers** specify argument size: `%ld` (long), `%lld` (long long), `%zu` (size_t), `%hd` (short)

## Common Patterns

### Safe string formatting with snprintf

```c
char buf[64];
int n = snprintf(buf, sizeof(buf), "Hello, %s! You scored %d%%.", name, score);
if (n >= (int)sizeof(buf)) {
    // Output was truncated
    fprintf(stderr, "Warning: output truncated\n");
}
```

### Parsing structured data with sscanf

```c
const char *line = "2026-02-24 15:30:00 ERROR disk full";
int year, month, day, hour, min, sec;
char level[16], message[64];

int matched = sscanf(line, "%d-%d-%d %d:%d:%d %15s %63[^\n]",
    &year, &month, &day, &hour, &min, &sec, level, message);
if (matched == 8) {
    printf("Parsed: %s at %02d:%02d\n", level, hour, min);
}
```

### Custom width and alignment

```c
// Table-style output
printf("%-20s %8s %8s\n", "Name", "Score", "Grade");
printf("%-20s %8d %8.1f\n", "Alice", 95, 4.0);
printf("%-20s %8d %8.1f\n", "Bob", 87, 3.5);
```

## Common Mistakes

- **Buffer overflow with `sprintf`**: Always prefer `snprintf` with an explicit size limit. `sprintf` has no bounds checking and will write past the end of the buffer.
- **Mismatched format specifiers**: Using `%d` for a `long` or `%f` for an `int` causes undefined behavior. Enable `-Wformat` warnings.
- **Forgetting the return value**: `snprintf` returns the number of characters that *would have been written* (not counting the null terminator). If this is >= buffer size, the output was truncated.
- **Using user input as format string**: Never pass user-supplied strings as the format argument — this creates a format string vulnerability. Always use `printf("%s", user_input)` instead of `printf(user_input)`.

## Further Reading

- [printf - cppreference.com](https://en.cppreference.com/w/c/io/fprintf)
- [scanf - cppreference.com](https://en.cppreference.com/w/c/io/fscanf)
- [Format String Attack - OWASP](https://owasp.org/www-community/attacks/Format_string_attack)
