# Processus POSIX — Référence rapide

## API principale

| Appel | Signature | Rôle |
|-------|-----------|------|
| `fork()` | `fork()` | Duplique le processus ; renvoie 0 dans l'enfant, PID dans le parent |
| `exec*()` | `execl/execv/execvp/execve(path, ...)` | Remplace l'image du processus par un nouveau programme |
| `wait()` | `wait(&status)` | Attend n'importe quel enfant ; récupère le code de sortie |
| `waitpid()` | `waitpid(pid, &status, opts)` | Attend un enfant spécifique ; `WNOHANG` non-bloquant |
| `exit()` | `exit(code)` | Termine le processus, envoie `SIGCHLD` au parent |
| `getpid()` | `getpid()` | Renvoie le PID du processus courant |
| `getppid()` | `getppid()` | Renvoie le PID du processus parent |

## États d'un processus

| État | Lettre | Description |
|------|--------|-------------|
| Running | `R` | En cours d'exécution ou prêt à s'exécuter |
| Sleeping | `S` | En attente d'un événement (I/O, signal) |
| Zombie | `Z` | Terminé, mais `wait()` non encore appelé par le parent |
| Stopped | `T` | Suspendu par `SIGSTOP` ou `SIGTSTP` |
| I/O wait | `D` | Attente I/O non-interruptible |

## Exemple — fork() parent/enfant

```c
#include <stdio.h>
#include <unistd.h>
#include <sys/wait.h>

int main(void) {
    pid_t pid = fork();

    if (pid < 0) {
        perror("fork");
        return 1;
    } else if (pid == 0) {
        /* Code exécuté par l'enfant */
        printf("Enfant PID=%d, parent PID=%d\n", getpid(), getppid());
        return 42;
    } else {
        /* Code exécuté par le parent */
        int status;
        waitpid(pid, &status, 0);
        if (WIFEXITED(status))
            printf("Enfant terminé, code=%d\n", WEXITSTATUS(status));
    }
    return 0;
}
```

## Zombies et orphelins

**Zombie** : l'enfant est terminé mais le parent n'a pas appelé `wait()`. L'entrée reste dans la table des processus.

```c
/* Prévention : attendre tous les enfants */
while (waitpid(-1, NULL, WNOHANG) > 0)
    ;  /* récolte sans bloquer */
```

**Orphelin** : le parent se termine avant l'enfant. Le noyau réattribue l'enfant à `init` (PID 1) qui appellera `wait()` automatiquement.

## Macros d'état (`sys/wait.h`)

| Macro | Description |
|-------|-------------|
| `WIFEXITED(st)` | Vrai si terminaison normale |
| `WEXITSTATUS(st)` | Code de sortie (0–255) |
| `WIFSIGNALED(st)` | Vrai si tué par un signal |
| `WTERMSIG(st)` | Numéro du signal fatal |
| `WIFSTOPPED(st)` | Vrai si suspendu (`SIGSTOP`) |

## exec() — Variantes

| Fonction | Arguments | Environnement |
|----------|-----------|---------------|
| `execl` | liste variadique | hérité |
| `execv` | tableau `argv[]` | hérité |
| `execvp` | tableau + recherche `PATH` | hérité |
| `execve` | tableau + `envp[]` | explicite |

```c
/* Remplace le processus par /bin/ls -l */
char *args[] = {"ls", "-l", NULL};
execvp("ls", args);
perror("execvp");  /* atteint seulement en cas d'erreur */
```

## Flux typique fork + exec

```
parent: fork() ──► enfant: execvp("prog", args)
                   ──────────────────────────────
parent: waitpid(pid, &st, 0)  ←─ SIGCHLD quand enfant termine
```
