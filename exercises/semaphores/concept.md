# Sémaphores POSIX — Référence rapide

## API POSIX (sémaphores sans nom)

| Appel | Signature | Rôle |
|-------|-----------|------|
| `sem_init()` | `sem_init(&sem, pshared, value)` | Init sémaphore en mémoire ; `pshared=0`: thread, `1`: processus |
| `sem_wait()` | `sem_wait(&sem)` | Décrémente (P) ; bloque si valeur = 0 |
| `sem_post()` | `sem_post(&sem)` | Incrémente (V) ; débloque un waiter |
| `sem_trywait()` | `sem_trywait(&sem)` | P non-bloquant ; `errno = EAGAIN` si valeur = 0 |
| `sem_getvalue()` | `sem_getvalue(&sem, &val)` | Lit la valeur courante |
| `sem_destroy()` | `sem_destroy(&sem)` | Libère le sémaphore sans nom |

## API POSIX (sémaphores nommés — IPC interprocessus)

| Appel | Signature | Rôle |
|-------|-----------|------|
| `sem_open()` | `sem_open(name, O_CREAT, mode, val)` | Crée ou ouvre un sémaphore nommé |
| `sem_close()` | `sem_close(sem)` | Ferme la référence locale |
| `sem_unlink()` | `sem_unlink(name)` | Supprime le sémaphore du FS |

## Producteur-consommateur

```c
#include <stdio.h>
#include <pthread.h>
#include <semaphore.h>

#define N 5
int buffer[N];
int in = 0, out = 0;

sem_t empty;   /* cases libres */
sem_t full;    /* cases remplies */
sem_t mutex;   /* exclusion mutuelle */

void *producer(void *arg) {
    for (int i = 0; i < 10; i++) {
        sem_wait(&empty);
        sem_wait(&mutex);
        buffer[in] = i;
        in = (in + 1) % N;
        sem_post(&mutex);
        sem_post(&full);
    }
    return NULL;
}

void *consumer(void *arg) {
    for (int i = 0; i < 10; i++) {
        sem_wait(&full);
        sem_wait(&mutex);
        int val = buffer[out];
        out = (out + 1) % N;
        sem_post(&mutex);
        sem_post(&empty);
        printf("Consommé : %d\n", val);
    }
    return NULL;
}

int main(void) {
    sem_init(&empty, 0, N);
    sem_init(&full,  0, 0);
    sem_init(&mutex, 0, 1);

    pthread_t p, c;
    pthread_create(&p, NULL, producer, NULL);
    pthread_create(&c, NULL, consumer, NULL);
    pthread_join(p, NULL);
    pthread_join(c, NULL);

    sem_destroy(&empty);
    sem_destroy(&full);
    sem_destroy(&mutex);
    return 0;
}
```

## POSIX vs SysV

| Critère | POSIX | SysV |
|---------|-------|------|
| Header | `<semaphore.h>` | `<sys/sem.h>` |
| Création | `sem_init()` / `sem_open()` | `semget(key, nsems, flags)` |
| P (attendre) | `sem_wait()` | `semop(id, &sop, 1)` avec `sem_op=-1` |
| V (signaler) | `sem_post()` | `semop(id, &sop, 1)` avec `sem_op=+1` |
| Suppression | `sem_destroy()` / `sem_unlink()` | `semctl(id, 0, IPC_RMID)` |
| Lisibilité | Élevée | Faible (API complexe) |
| Portabilité | POSIX.1 | XSI (moins portable) |

## SysV — exemple minimal

```c
#include <sys/sem.h>

key_t key = ftok("/tmp", 'S');
int semid = semget(key, 1, IPC_CREAT | 0600);

/* P (wait) */
struct sembuf sop = { .sem_num = 0, .sem_op = -1, .sem_flg = 0 };
semop(semid, &sop, 1);

/* V (post) */
sop.sem_op = 1;
semop(semid, &sop, 1);

/* Suppression */
semctl(semid, 0, IPC_RMID);
```

## Points clés

- Valeur initiale 1 → **mutex** (exclusion mutuelle)
- Valeur initiale 0 → **synchronisation** (attendre un événement)
- `sem_wait()` peut être interrompu par un signal → vérifier `errno == EINTR`
- Lier avec `-lpthread` (ou `-lrt` sur certains systèmes)
