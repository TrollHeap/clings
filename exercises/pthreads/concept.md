# C Concurrency (pthreads)

POSIX threads (`<pthread.h>`) are the standard threading API on Linux/macOS. The exercises here simulate patterns using sequential code — WASI doesn't support real threads, but the structures and idioms are identical.

## Core API

| Function | Purpose |
|----------|---------|
| `pthread_create(&tid, NULL, func, arg)` | Spawn thread running `func(arg)` |
| `pthread_join(tid, NULL)` | Wait for thread to exit |
| `pthread_mutex_lock(&m)` | Acquire mutex (blocks if held) |
| `pthread_mutex_unlock(&m)` | Release mutex |
| `pthread_cond_wait(&c, &m)` | Atomically release mutex and block |
| `pthread_cond_signal(&c)` | Wake one thread waiting on condition |

## Thread Function Signature

```c
void *thread_func(void *arg) {
    MyStruct *s = (MyStruct *)arg;  // cast void* to your type
    // ... work ...
    return NULL;
}
```

## Common Patterns

**Mutex protection**:
```c
pthread_mutex_lock(&mutex);
shared_resource++;
pthread_mutex_unlock(&mutex);
```

**Condition variable (producer/consumer)**:
```c
// Consumer:
pthread_mutex_lock(&m);
while (!data_ready) pthread_cond_wait(&cond, &m);
// use data
pthread_mutex_unlock(&m);

// Producer:
pthread_mutex_lock(&m);
data_ready = 1;
pthread_cond_signal(&cond);
pthread_mutex_unlock(&m);
```

## Deadlock Prevention

Always acquire multiple mutexes in the **same global order** across all threads. Release in reverse order (LIFO).
