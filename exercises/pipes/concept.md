# Tubes (pipes) POSIX — Référence rapide

## API principale

| Appel | Signature | Rôle |
|-------|-----------|------|
| `pipe()` | `pipe(fd[2])` | Crée un tube ; `fd[0]`=lecture, `fd[1]`=écriture |
| `dup2()` | `dup2(oldfd, newfd)` | Duplique `oldfd` sur `newfd` (ferme `newfd` si ouvert) |
| `mkfifo()` | `mkfifo(path, mode)` | Crée un FIFO nommé (persistant dans le FS) |
| `read()` | `read(fd, buf, n)` | Lecture ; bloque si le tube est vide |
| `write()` | `write(fd, buf, n)` | Écriture ; bloque si le tube est plein |
| `close()` | `close(fd)` | Ferme l'extrémité ; déclenche EOF côté lecture si dernière réf. |

## Tube anonyme — parent vers enfant

```c
#include <stdio.h>
#include <unistd.h>
#include <string.h>

int main(void) {
    int fd[2];
    pipe(fd);
    pid_t pid = fork();

    if (pid == 0) {
        /* Enfant : lit depuis le tube */
        close(fd[1]);               /* ferme l'extrémité écriture */
        char buf[64];
        ssize_t n = read(fd[0], buf, sizeof(buf) - 1);
        buf[n] = '\0';
        printf("Enfant reçu : %s\n", buf);
        close(fd[0]);
    } else {
        /* Parent : écrit dans le tube */
        close(fd[0]);               /* ferme l'extrémité lecture */
        write(fd[1], "hello", 5);
        close(fd[1]);               /* EOF pour l'enfant */
        wait(NULL);
    }
    return 0;
}
```

> Toujours fermer les extrémités inutilisées — sinon EOF n'est jamais généré.

## Redirection avec dup2()

```c
/* Redirige stdout vers fd[1] (écriture du tube) */
close(fd[0]);
dup2(fd[1], STDOUT_FILENO);
close(fd[1]);
execlp("ls", "ls", "-l", NULL);
```

- `dup2(old, new)` duplique `old` sur `new` ; si `new` est ouvert, il est d'abord fermé.
- Pattern classique : `cmd1 | cmd2` — stdout de cmd1 devient stdin de cmd2.

## FIFO nommé (mkfifo)

```c
/* Créateur */
mkfifo("/tmp/myfifo", 0600);
int fd = open("/tmp/myfifo", O_WRONLY);
write(fd, "data", 4);
close(fd);

/* Lecteur (autre processus) */
int fd = open("/tmp/myfifo", O_RDONLY);
char buf[32];
read(fd, buf, sizeof(buf));
close(fd);
unlink("/tmp/myfifo");  /* suppression explicite */
```

- L'`open()` bloque jusqu'à ce que les deux côtés soient ouverts.
- Visible dans le FS (`ls -l` → type `p`).

## SIGPIPE

| Situation | Comportement |
|-----------|-------------|
| Écriture sur tube sans lecteur | `SIGPIPE` envoyé à l'écrivain |
| `SIGPIPE` ignoré (`SIG_IGN`) | `write()` retourne `-1`, `errno = EPIPE` |
| Tubes vidés avant fermeture | Les données en attente sont lisibles |

```c
signal(SIGPIPE, SIG_IGN);  /* pour gérer EPIPE manuellement */
```

## Caractéristiques du buffer

| Propriété | Valeur typique |
|-----------|----------------|
| Taille buffer | 65 536 octets (Linux) |
| `write()` atomique | ≤ `PIPE_BUF` (4 096 octets) |
| Comportement si plein | `write()` bloque |
| Comportement si vide | `read()` bloque |

## Flux : pipeline shell `cmd1 | cmd2`

```
pipe(fd)
fork() ──► enfant1 : dup2(fd[1], stdout) → exec(cmd1)
fork() ──► enfant2 : dup2(fd[0], stdin)  → exec(cmd2)
parent : close(fd[0]), close(fd[1]), waitpid x2
```
