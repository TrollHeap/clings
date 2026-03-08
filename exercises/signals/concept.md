# Signaux POSIX — Référence rapide

## Signaux courants

| Signal | Numéro | Cause / Usage |
|--------|--------|---------------|
| `SIGHUP` | 1 | Déconnexion terminal ou rechargement config daemon |
| `SIGINT` | 2 | Ctrl+C — interruption interactive |
| `SIGKILL` | 9 | Terminaison forcée (non capturable, non ignorable) |
| `SIGSEGV` | 11 | Faute de segmentation (accès mémoire invalide) |
| `SIGPIPE` | 13 | Écriture sur tube sans lecteur |
| `SIGALRM` | 14 | Alarme timer (`alarm()`) |
| `SIGTERM` | 15 | Terminaison propre (par défaut de `kill`) |
| `SIGUSR1` | 10 | Signal utilisateur libre |
| `SIGUSR2` | 12 | Signal utilisateur libre |
| `SIGCHLD` | 17 | Enfant terminé ou stoppé |

## API principale

| Appel | Signature | Rôle |
|-------|-----------|------|
| `signal()` | `signal(signum, handler)` | Handler simple (ANSI C, moins portable) |
| `sigaction()` | `sigaction(signum, &act, &old)` | Handler complet avec masque et flags |
| `kill()` | `kill(pid, sig)` | Envoie un signal à un processus (`pid<0` → groupe) |
| `raise()` | `raise(sig)` | Envoie un signal à soi-même |
| `sigprocmask()` | `sigprocmask(how, &set, &old)` | Modifie le masque de signaux |
| `sigemptyset()` | `sigemptyset(&set)` | Initialise un ensemble vide |
| `sigaddset()` | `sigaddset(&set, signum)` | Ajoute un signal à l'ensemble |
| `sigfillset()` | `sigfillset(&set)` | Remplit l'ensemble avec tous les signaux |

## sigaction — Handler complet

```c
#include <signal.h>
#include <stdio.h>

static void handler(int sig) {
    /* Ne pas appeler de fonctions non async-signal-safe ici */
    write(STDOUT_FILENO, "Signal reçu\n", 12);
}

int main(void) {
    struct sigaction sa = {0};
    sa.sa_handler = handler;
    sigemptyset(&sa.sa_mask);
    sa.sa_flags = SA_RESTART;  /* reprend les appels système interrompus */

    sigaction(SIGINT, &sa, NULL);
    pause();  /* attend un signal */
    return 0;
}
```

## Flags sigaction

| Flag | Effet |
|------|-------|
| `SA_RESTART` | Redémarre automatiquement les appels système interrompus |
| `SA_NOCLDWAIT` | Évite les zombies (avec `SIGCHLD`) |
| `SA_SIGINFO` | Handler reçoit `siginfo_t` (infos étendues) |
| `SA_RESETHAND` | Restaure le handler par défaut après le premier appel |

## Handler SIGCHLD — Récolte des zombies

```c
static void reap_children(int sig) {
    (void)sig;
    while (waitpid(-1, NULL, WNOHANG) > 0)
        ;
}

/* Dans main() */
struct sigaction sa = { .sa_handler = reap_children, .sa_flags = SA_RESTART };
sigemptyset(&sa.sa_mask);
sigaction(SIGCHLD, &sa, NULL);
```

## Masque de signaux

```c
sigset_t mask, oldmask;
sigemptyset(&mask);
sigaddset(&mask, SIGINT);
sigaddset(&mask, SIGTERM);

/* Bloque SIGINT et SIGTERM pendant la section critique */
sigprocmask(SIG_BLOCK, &mask, &oldmask);
/* ... section critique ... */
sigprocmask(SIG_SETMASK, &oldmask, NULL);  /* restaure */
```

| `how` | Effet |
|-------|-------|
| `SIG_BLOCK` | Ajoute `set` au masque courant |
| `SIG_UNBLOCK` | Retire `set` du masque courant |
| `SIG_SETMASK` | Remplace le masque par `set` |

## Fonctions async-signal-safe

Dans un handler, utiliser **uniquement** des fonctions async-signal-safe :
`write()`, `read()`, `_exit()`, `kill()`, `waitpid()`, `sigprocmask()`.

**Interdits** dans un handler : `printf()`, `malloc()`, `free()`, `exit()`.
