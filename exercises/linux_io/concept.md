# Linux I/O — Descripteurs, Pipes, Multiplexage

## Descripteurs de fichiers

Sous POSIX, chaque ressource ouverte est représentée par un **descripteur de fichier** (fd) — un entier indexant la table des fd du processus.

```
fd  Nom     Direction  Bufferisation                        Destination par défaut
0   stdin   entrée     ligne (terminal) / bloc (fichier)    clavier / fichier
1   stdout  sortie     ligne (terminal) / bloc (fichier)    terminal / fichier
2   stderr  erreurs    non bufférisé                        terminal (même si stdout redirigé)
```

Appels clés :

| Appel            | Description                                      |
|------------------|--------------------------------------------------|
| `open(path, flags)` | Ouvre un fichier, retourne un fd              |
| `close(fd)`      | Ferme le fd                                      |
| `dup(fd)`        | Duplique fd vers le plus petit fd libre          |
| `dup2(old, new)` | Duplique old vers new (ferme new si nécessaire)  |
| `read(fd, buf, n)` | Lit jusqu'à n octets                           |
| `write(fd, buf, n)` | Écrit n octets                                |

---

## Pipes : pipe(), dup2(), redirection

`pipe(fds)` crée deux fds : `fds[0]` (lecture) et `fds[1]` (écriture). Les données écrites sur `fds[1]` sont lisibles sur `fds[0]`.

**Séquence fork+pipe :**

```c
int pipefd[2];
pipe(pipefd);

if (fork() == 0) {        // enfant : écrit dans le pipe
    dup2(pipefd[1], 1);   // stdout → pipe write
    close(pipefd[0]);
    close(pipefd[1]);
    execlp("ls", "ls", NULL);
} else {                  // parent : lit depuis le pipe
    dup2(pipefd[0], 0);   // stdin ← pipe read
    close(pipefd[0]);
    close(pipefd[1]);
    // lire depuis stdin...
}
```

**Règle :** toujours `close()` les extrémités inutilisées — sinon EOF ne sera jamais signalé.

---

## I/O Multiplexage : select(), poll(), epoll()

Le multiplexage permet d'attendre **simultanément** plusieurs fd sans bloquer sur un seul.

### select()
```c
fd_set rfds;
FD_ZERO(&rfds);
FD_SET(fd1, &rfds);
FD_SET(fd2, &rfds);
select(maxfd+1, &rfds, NULL, NULL, &timeout);
if (FD_ISSET(fd1, &rfds)) { /* fd1 prêt */ }
```

### poll()
```c
struct pollfd fds[] = {
    { fd1, POLLIN, 0 },
    { fd2, POLLOUT, 0 },
};
int n = poll(fds, 2, 1000);  // timeout 1000ms
for (int i = 0; i < 2; i++)
    if (fds[i].revents & POLLIN) { /* lire */ }
```

### epoll() (Linux uniquement)
```c
int epfd = epoll_create1(0);
struct epoll_event ev = { .events = EPOLLIN, .data.fd = fd1 };
epoll_ctl(epfd, EPOLL_CTL_ADD, fd1, &ev);

struct epoll_event events[10];
int n = epoll_wait(epfd, events, 10, -1);
for (int i = 0; i < n; i++) { /* events[i].data.fd prêt */ }
```

---

## Comparatif select / poll / epoll

| Critère           | select        | poll            | epoll           |
|-------------------|--------------|-----------------|-----------------|
| Limite fd         | FD_SETSIZE (1024) | aucune     | aucune          |
| Complexité        | O(n)         | O(n)            | O(1) wake-up    |
| Modifie le set    | Oui (réinitialiser à chaque appel) | Non | Non  |
| Portabilité       | POSIX        | POSIX           | Linux seulement |
| Mode triggered    | Level only   | Level only      | Level + Edge    |

**Edge-triggered (EPOLLET)** : notification unique à l'arrivée de données → doit tout lire d'un coup.
**Level-triggered** (défaut) : notification répétée tant que des données sont disponibles.

---

## Bufferisation stdio vs syscall direct

```c
// stdio (bufferisé) — flush automatique à \n (terminal) ou à la fermeture
printf("hello\n");            // → stdout buffer → write(1, ...) au flush
fprintf(stderr, "err\n");     // stderr non bufferisé → write(2, ...) immédiat

// syscall direct (non bufferisé)
write(1, "hello\n", 6);       // write immédiat, pas de buffer
```

Pour forcer le flush : `fflush(stdout)` ou `setvbuf(stdout, NULL, _IONBF, 0)`.

---

## Ordonnancement et I/O : nice(), sched_getscheduler()

| Politique       | Valeur | Usage                          |
|-----------------|--------|--------------------------------|
| SCHED_OTHER     | 0      | CFS temps partagé (défaut)     |
| SCHED_BATCH     | 3      | CPU-bound, pas de latence exigée |
| SCHED_IDLE      | 5      | Tâches d'arrière-plan ultra-basse priorité |
| SCHED_FIFO      | 1      | Temps réel FIFO (nécessite root) |
| SCHED_RR        | 2      | Temps réel round-robin          |

**Priorité statique CFS** = `120 + nice` (100 pour nice=-20, 139 pour nice=+19).

```c
int policy = sched_getscheduler(pid);   // obtenir la politique
int prio   = getpriority(PRIO_PROCESS, pid);  // nice actuel
setpriority(PRIO_PROCESS, pid, 10);     // renice → +10
```

Poids CFS : `nice 0 → 1024`, `nice -20 → 88761` (×86 de CPU), `nice +19 → 15` (×68 moins).
