# Files de messages — Référence rapide

## API POSIX (`mqueue.h`)

| Appel | Signature | Rôle |
|-------|-----------|------|
| `mq_open()` | `mq_open(name, flags, mode, &attr)` | Crée ou ouvre une file nommée |
| `mq_send()` | `mq_send(mqd, msg, size, prio)` | Envoie un message (priorité 0–31) |
| `mq_receive()` | `mq_receive(mqd, buf, size, &prio)` | Reçoit le message de plus haute priorité |
| `mq_close()` | `mq_close(mqd)` | Ferme la référence locale |
| `mq_unlink()` | `mq_unlink(name)` | Supprime la file du FS |
| `mq_getattr()` | `mq_getattr(mqd, &attr)` | Lit les attributs (taille max, nb messages) |

## API SysV (`sys/msg.h`)

| Appel | Signature | Rôle |
|-------|-----------|------|
| `msgget()` | `msgget(key, flags)` | Crée ou accède à une file par clé IPC |
| `msgsnd()` | `msgsnd(id, &msg, size, flags)` | Envoie ; `IPC_NOWAIT` non-bloquant |
| `msgrcv()` | `msgrcv(id, &msg, size, mtype, flags)` | Reçoit par type de message |
| `msgctl()` | `msgctl(id, cmd, &buf)` | Contrôle (`IPC_RMID` pour supprimer) |

## Structure msgbuf (SysV)

```c
struct msgbuf {
    long mtype;    /* type > 0, sert de filtre à msgrcv() */
    char mtext[1]; /* corps du message (taille variable) */
};
```

## Génération de clé IPC — ftok()

```c
#include <sys/ipc.h>

key_t key = ftok("/tmp/myapp", 'Q');
/* Combine inode + numéro de device + proj_id en une clé unique */
```

## Exemple POSIX — envoi/réception

```c
#include <mqueue.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    struct mq_attr attr = {
        .mq_flags   = 0,
        .mq_maxmsg  = 10,
        .mq_msgsize = 64,
    };

    /* Émetteur */
    mqd_t mq = mq_open("/myqueue", O_CREAT | O_WRONLY, 0600, &attr);
    mq_send(mq, "hello", 5, 1);
    mq_close(mq);

    /* Récepteur */
    mq = mq_open("/myqueue", O_RDONLY);
    char buf[64];
    unsigned int prio;
    mq_receive(mq, buf, sizeof(buf), &prio);
    printf("Reçu (prio=%u) : %.*s\n", prio, 5, buf);
    mq_close(mq);
    mq_unlink("/myqueue");
    return 0;
}
```

## Filtrage des messages SysV (mtype)

| `mtype` passé à `msgrcv()` | Comportement |
|----------------------------|-------------|
| `0` | Premier message disponible (FIFO) |
| `> 0` | Premier message de ce type exact |
| `< 0` | Premier message dont le type ≤ `|mtype|` |

```c
/* Reçoit uniquement les messages de type 2 */
struct { long mtype; char text[64]; } msg;
msgrcv(msgid, &msg, sizeof(msg.text), 2, 0);
```

## POSIX vs SysV

| Critère | POSIX | SysV |
|---------|-------|------|
| Header | `<mqueue.h>` | `<sys/msg.h>` |
| Identification | Nom (`/myqueue`) | Clé numérique (`ftok`) |
| Priorité | 0–31, triée automatiquement | Type long, filtrage manuel |
| Notification | `mq_notify()` (async) | Aucune intégrée |
| Portabilité | POSIX.1-2001 | XSI |
| Lien | `-lrt` | aucun |

## Points clés

- Les noms POSIX commencent par `/` et sont visibles dans `/dev/mqueue/` (Linux)
- Taille du buffer de `mq_receive()` ≥ `mq_msgsize` de la file
- Messages persistants : survivent jusqu'à `mq_unlink()` ou redémarrage
