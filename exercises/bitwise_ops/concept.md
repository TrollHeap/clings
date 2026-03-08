# Bitwise Operations

## Overview

Bitwise operators work on the binary representation of integer values one bit at a
time. They are indispensable in systems programming for manipulating hardware
registers, encoding multiple flags into a single integer (bitmasks), implementing
efficient algorithms, and performing fast multiplication or division by powers of
two. C provides six bitwise operators: AND, OR, XOR, NOT, left shift, and right
shift. All operate on integer types; the result type follows the usual arithmetic
promotions.

## Key Concepts

- **AND (`&`)**: Sets a bit to 1 only if both operands have a 1 in that position.
  Used to test and clear individual bits.
- **OR (`|`)**: Sets a bit to 1 if either operand has a 1. Used to set bits.
- **XOR (`^`)**: Sets a bit to 1 if exactly one operand has a 1. Used to toggle
  bits and swap values without a temporary.
- **NOT (`~`)**: Inverts every bit (bitwise complement). Used to create masks from
  their inverse.
- **Left shift (`<<`)**: Shifts bits toward more significant positions; equivalent
  to multiplying by a power of two. Shifting signed values into the sign bit is
  undefined behavior — prefer `unsigned` types for bit manipulation.
- **Right shift (`>>`)**: Shifts toward less significant positions. For `unsigned`
  types, vacated bits are filled with 0 (logical shift). For signed types,
  behavior is implementation-defined (usually arithmetic shift).

## Common Patterns

### Testing, setting, clearing, and toggling bits

```c
#include <stdio.h>
#include <stdint.h>

#define BIT(n)          (1u << (n))
#define SET_BIT(v, n)   ((v) |  BIT(n))
#define CLEAR_BIT(v, n) ((v) & ~BIT(n))
#define TOGGLE_BIT(v,n) ((v) ^  BIT(n))
#define TEST_BIT(v, n)  (((v) >> (n)) & 1u)

int main(void) {
    uint8_t flags = 0b00000000;

    flags = SET_BIT(flags, 2);   // 0b00000100
    flags = SET_BIT(flags, 5);   // 0b00100100
    printf("after set:    0x%02X\n", flags);

    printf("bit 2 set?    %u\n", TEST_BIT(flags, 2)); // 1
    printf("bit 3 set?    %u\n", TEST_BIT(flags, 3)); // 0

    flags = CLEAR_BIT(flags, 2); // 0b00100000
    flags = TOGGLE_BIT(flags, 0);// 0b00100001
    printf("final:        0x%02X\n", flags);

    return 0;
}
```

### Packing and unpacking multiple values into one integer

```c
#include <stdio.h>
#include <stdint.h>

// Pack two 4-bit nibbles and one 8-bit byte into a uint16_t:
//   [15..12] = high_nibble  [11..8] = low_nibble  [7..0] = byte_val
uint16_t pack(uint8_t high_nibble, uint8_t low_nibble, uint8_t byte_val) {
    return ((uint16_t)(high_nibble & 0xF) << 12)
         | ((uint16_t)(low_nibble  & 0xF) <<  8)
         | (byte_val);
}

void unpack(uint16_t packed, uint8_t *high, uint8_t *low, uint8_t *byte_val) {
    *high     = (packed >> 12) & 0xF;
    *low      = (packed >>  8) & 0xF;
    *byte_val =  packed        & 0xFF;
}

int main(void) {
    uint16_t packed = pack(0xA, 0xB, 0xCD);
    printf("packed: 0x%04X\n", packed); // 0xABCD

    uint8_t h, l, b;
    unpack(packed, &h, &l, &b);
    printf("high=%X low=%X byte=%02X\n", h, l, b); // A B CD
    return 0;
}
```

### Fast power-of-two checks and alignment

```c
#include <stdio.h>
#include <stdint.h>

// A number is a power of two iff it has exactly one bit set.
int is_power_of_two(uint32_t n) {
    return n != 0 && (n & (n - 1)) == 0;
}

// Round up to the next multiple of a power-of-two alignment.
uint32_t align_up(uint32_t value, uint32_t align) {
    // align must be a power of two
    return (value + align - 1) & ~(align - 1);
}

int main(void) {
    printf("%d %d %d\n", is_power_of_two(8), is_power_of_two(0), is_power_of_two(6));
    printf("%u\n", align_up(13, 8));  // 16
    printf("%u\n", align_up(16, 8));  // 16
    return 0;
}
```

## Common Mistakes

- **Confusing `&` (bitwise AND) with `&&` (logical AND)**: `a & b` returns an
  integer result; `a && b` returns 0 or 1 and short-circuits. Using the wrong one
  in a condition compiles silently but produces incorrect results.

- **Shifting by an amount >= the type width**: Shifting a 32-bit integer by 32 or
  more positions is undefined behavior in C11. Guard shifts: `n < 32 ? (x << n) : 0`.

- **Applying bitwise NOT to `int` expecting a bitmask**: `~0` has type `int` and
  is all-bits-set for a signed type. Surprises arise with sign extension when
  assigning to wider types. Use `~0u` or the explicit type `~(uint32_t)0` for
  unsigned bitmasks.

## Further Reading

- [cppreference — Bitwise operators](https://en.cppreference.com/w/c/language/operator_arithmetic)
- [cppreference — Integer arithmetic (shifts)](https://en.cppreference.com/w/c/language/operator_arithmetic#Shift_operators)
- [cppreference — stdint.h fixed-width types](https://en.cppreference.com/w/c/types/integer)
