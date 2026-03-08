# Mémoire partagée — Référence rapide

## API POSIX (`sys/mman.h`)

| Appel | Signature | Rôle |
|-------|-----------|------|
| `shm_open()` | `shm_open(name, flags, mode)` | Crée ou ouvre un segment nommé ; renvoie un fd |
| `ftruncate()` | `ftruncate(fd, size)` | Définit la taille du segment |
| `mmap()` | `mmap(NULL, size, prot, MAP_SHARED, fd, 0)` | Mappe le segment dans l'espace d'adressage |
| `munmap()` | `munmap(ptr, size)` | Démapper le segment (ne supprime pas) |
| `shm_unlink()` | `shm_unlink(name)` | Supprime le segment du FS |

## API SysV (`sys/shm.h`)

| Appel | Signature | Rôle |
|-------|-----------|------|
| `shmget()` | `shmget(key, size, flags)` | Crée ou accède à un segment par clé IPC |
| `shmat()` | `shmat(id, NULL, 0)` | Attache le segment (NULL → kernel choisit l'adresse) |
| `shmdt()` | `shmdt(ptr)` | Détache le segment (ne supprime pas) |
| `shmctl()` | `shmctl(id, IPC_RMID, NULL)` | Supprime le segment |

## Exemple POSIX — deux processus

```c
/* Processus écrivain */
#include <sys/mman.h>
#include <fcntl.h>
#include <semaphore.h>
#include <string.h>

typedef struct { sem_t mutex; char data[64]; } SharedData;

int main(void) {
    int fd = shm_open("/myshm", O_CREAT | O_RDWR, 0600);
    ftruncate(fd, sizeof(SharedData));

    SharedData *shm = mmap(NULL, sizeof(SharedData),
                           PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);

    sem_init(&shm->mutex, 1, 1);   /* pshared=1 : entre processus */

    sem_wait(&shm->mutex);
    strcpy(shm->data, "hello from writer");
    sem_post(&shm->mutex);

    munmap(shm, sizeof(SharedData));
    /* Le lecteur appellera shm_unlink() */
    return 0;
}
```

## Race condition sans synchronisation

```c
/* Thread 1 */          /* Thread 2 */
read counter (5)        read counter (5)
increment → 6           increment → 6
write counter (6)       write counter (6)
/* Résultat : 6 au lieu de 7 — perte d'une mise à jour */
```

**Solution** : protéger l'accès avec un sémaphore ou un mutex partagé (`sem_init` avec `pshared=1`).

## Pattern combiné shm + sémaphore

```c
/* Structure en mémoire partagée */
typedef struct {
    sem_t   mutex;          /* exclusion mutuelle */
    sem_t   data_ready;     /* synchronisation producteur→consommateur */
    int     counter;
    char    buffer[256];
} SharedRegion;

/* Init (une seule fois, côté producteur) */
sem_init(&shm->mutex,      1, 1);
sem_init(&shm->data_ready, 1, 0);

/* Producteur */
sem_wait(&shm->mutex);
shm->counter++;
strcpy(shm->buffer, "data");
sem_post(&shm->mutex);
sem_post(&shm->data_ready);

/* Consommateur */
sem_wait(&shm->data_ready);
sem_wait(&shm->mutex);
printf("counter=%d, data=%s\n", shm->counter, shm->buffer);
sem_post(&shm->mutex);
```

## POSIX vs SysV

| Critère | POSIX | SysV |
|---------|-------|------|
| Header | `<sys/mman.h>` | `<sys/shm.h>` |
| Identification | Nom (`/myshm`) visible dans `/dev/shm/` | Clé numérique (`ftok`) |
| Dimensionnement | `ftruncate()` après création | Fixé à `shmget()` |
| Accès | `mmap()` — pointeur direct | `shmat()` — pointeur direct |
| Suppression | `shm_unlink()` | `shmctl(IPC_RMID)` |
| Lien | `-lrt` | aucun |

## Points clés

- La mémoire partagée est le mécanisme IPC le **plus rapide** (pas de copie noyau)
- Toujours synchroniser les accès : la mémoire partagée seule n'est **pas thread-safe**
- `shmdt()` / `munmap()` ne détruit pas le segment — appeler `shmctl(IPC_RMID)` / `shm_unlink()`
- Vérifier les fuites avec `ipcs -m` (SysV) ou `ls /dev/shm/` (POSIX)
