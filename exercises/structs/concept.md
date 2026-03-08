# Structs

## Overview

A struct in C groups heterogeneous data fields under a single name, allowing you to
treat related data as a unit. Unlike arrays (which hold homogeneous elements),
struct fields can have different types. Structs are fundamental to organizing
programs beyond simple variables — they model domain objects, form the nodes of
linked lists and trees, and serve as the basis for object-oriented patterns in C.
`typedef` removes the need to prefix every use with the `struct` keyword.

## Key Concepts

- **Struct definition and instantiation**: `struct Point { int x; int y; };` defines
  a type. Variables are declared with `struct Point p = {3, 4};` or using designated
  initializers `{.x = 3, .y = 4}`.
- **`typedef`**: `typedef struct { ... } Name;` creates an alias so you can write
  `Name` instead of `struct Name` everywhere.
- **Member access**: Use `.` for direct access (`p.x`) and `->` for access through
  a pointer (`ptr->x`, equivalent to `(*ptr).x`).
- **Nested structs**: A struct field can itself be of a struct type, enabling
  hierarchical data models.
- **Function pointers in structs**: Storing function pointers as struct members
  simulates methods and vtables, enabling polymorphism in C.

## Common Patterns

### Defining structs with `typedef` and using designated initializers

```c
#include <stdio.h>
#include <math.h>

typedef struct {
    double x;
    double y;
} Point;

double point_distance(Point a, Point b) {
    double dx = a.x - b.x;
    double dy = a.y - b.y;
    return sqrt(dx * dx + dy * dy);
}

int main(void) {
    Point origin = {.x = 0.0, .y = 0.0};
    Point target = {.x = 3.0, .y = 4.0};

    printf("distance: %.2f\n", point_distance(origin, target)); // 5.00
    return 0;
}
```

### Struct pointers and dynamic allocation

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char name[64];
    int  age;
} Person;

Person *person_new(const char *name, int age) {
    Person *p = malloc(sizeof(Person));
    if (p == NULL) return NULL;
    strncpy(p->name, name, sizeof(p->name) - 1);
    p->name[sizeof(p->name) - 1] = '\0';
    p->age = age;
    return p;
}

void person_print(const Person *p) {
    printf("%s (age %d)\n", p->name, p->age);
}

int main(void) {
    Person *alice = person_new("Alice", 30);
    if (alice == NULL) return 1;
    person_print(alice); // Alice (age 30)
    free(alice);
    return 0;
}
```

### Function pointers in structs (vtable-style polymorphism)

```c
#include <stdio.h>
#include <stdlib.h>

typedef struct Shape Shape;

struct Shape {
    double (*area)(const Shape *self);
    void   (*print)(const Shape *self);
};

typedef struct {
    Shape  base;   // must be first member
    double radius;
} Circle;

static double circle_area(const Shape *self) {
    const Circle *c = (const Circle *)self;
    return 3.14159265358979 * c->radius * c->radius;
}

static void circle_print(const Shape *self) {
    const Circle *c = (const Circle *)self;
    printf("Circle(r=%.2f, area=%.2f)\n", c->radius, self->area(self));
}

int main(void) {
    Circle c = {
        .base   = { .area = circle_area, .print = circle_print },
        .radius = 5.0,
    };
    Shape *s = (Shape *)&c;
    s->print(s); // Circle(r=5.00, area=78.54)
    return 0;
}
```

## Common Mistakes

- **Struct padding surprises**: The compiler may insert padding bytes between fields
  to satisfy alignment requirements. Do not assume `sizeof(struct)` equals the sum
  of its field sizes, and never `memcpy` a struct to a fixed-size wire format
  without accounting for padding (use `__attribute__((packed))` or serialize field
  by field).

- **Shallow copies of pointer fields**: Assigning one struct to another copies the
  values of all fields — including pointer fields. Both structs then share the same
  pointed-to memory. Freeing one and then accessing through the other is
  use-after-free.

- **Forgetting `->` when accessing through a pointer**: `ptr.field` is a syntax
  error when `ptr` is a pointer; use `ptr->field`. A common slip after changing a
  variable from a value to a pointer type.

## Further Reading

- [cppreference — Struct declaration](https://en.cppreference.com/w/c/language/struct)
- [cppreference — typedef](https://en.cppreference.com/w/c/language/typedef)
- [cppreference — Operator precedence (member access)](https://en.cppreference.com/w/c/language/operator_precedence)
