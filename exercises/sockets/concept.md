# Sockets POSIX — Référence rapide

## API principale

| Appel | Signature | Rôle |
|-------|-----------|------|
| `socket()` | `socket(domain, type, protocol)` | Crée un endpoint réseau ; renvoie un fd |
| `bind()` | `bind(fd, &addr, addrlen)` | Associe le socket à une adresse locale |
| `listen()` | `listen(fd, backlog)` | Mode passif (serveur TCP) |
| `accept()` | `accept(fd, &peer, &len)` | Accepte une connexion entrante ; renvoie un nouveau fd |
| `connect()` | `connect(fd, &addr, addrlen)` | Initie la connexion (client TCP) |
| `send()` | `send(fd, buf, n, flags)` | Envoie des données sur socket connecté |
| `recv()` | `recv(fd, buf, n, flags)` | Reçoit des données sur socket connecté |
| `sendto()` | `sendto(fd, buf, n, 0, &dst, dstlen)` | Envoie un datagramme UDP |
| `recvfrom()` | `recvfrom(fd, buf, n, 0, &src, &srclen)` | Reçoit un datagramme + adresse source |
| `close()` | `close(fd)` | Ferme le socket, libère le fd |

## Domaines (`domain`)

- `AF_INET` (2) — IPv4
- `AF_INET6` (10) — IPv6
- `AF_UNIX` (1) — socket Unix local (chemin de fichier)

## Types (`type`)

| Constante | Valeur | Protocole typique |
|-----------|--------|-------------------|
| `SOCK_STREAM` | 1 | TCP — orienté connexion, fiable, ordonné |
| `SOCK_DGRAM` | 2 | UDP — sans connexion, non fiable |
| `SOCK_RAW` | 3 | Accès direct aux paquets IP |

## Adressage IPv4 — `sockaddr_in`

```c
struct sockaddr_in {
    sa_family_t    sin_family;  /* AF_INET */
    in_port_t      sin_port;    /* port en network byte order : htons(8080) */
    struct in_addr sin_addr;    /* adresse IP : INADDR_ANY ou inet_aton() */
};
```

- `htons(port)` / `ntohs(port)` — conversion host↔network pour les ports
- `htonl(addr)` / `ntohl(addr)` — conversion host↔network pour les adresses
- `INADDR_ANY` (0) — écouter sur toutes les interfaces

## Flux TCP — Côté serveur

```
socket()  →  bind()  →  listen()  →  accept()  →  send()/recv()  →  close()
```

## Flux TCP — Côté client

```
socket()  →  connect()  →  send()/recv()  →  close()
```

## Flux UDP

```
socket()  →  bind() (serveur)  →  sendto()/recvfrom()  →  close()
              ─────────────────────────────────────────────────────
socket()  →  sendto()/recvfrom()  →  close()   (client sans bind)
```

## TCP vs UDP

| Critère | TCP | UDP |
|---------|-----|-----|
| Connexion | Oui (handshake 3-way) | Non |
| Fiabilité | Garantie (retransmission) | Aucune |
| Ordre | Garanti | Non garanti |
| Surcoût | Plus élevé (ACK, retrans.) | Faible |
| Usages | HTTP, SSH, BDD | DNS, jeux, streaming, VoIP |

## I/O Multiplexing — `select()`

```c
fd_set rfds;
FD_ZERO(&rfds);
FD_SET(fd1, &rfds);
FD_SET(fd2, &rfds);

int ready = select(max_fd + 1, &rfds, NULL, NULL, &timeout);
if (FD_ISSET(fd1, &rfds)) { /* fd1 prêt à lire */ }
```

- `FD_ZERO` — vide le jeu de fd
- `FD_SET(fd, &set)` — ajoute fd au jeu
- `FD_ISSET(fd, &set)` — teste si fd est prêt après select()
- `nfds` = fd le plus grand + 1

Alternatives modernes : `poll()`, `epoll()` (Linux), `kqueue()` (macOS/BSD).

## Mode non-bloquant

```c
int flags = fcntl(fd, F_GETFL, 0);
fcntl(fd, F_SETFL, flags | O_NONBLOCK);
```

- `recv()` retourne `-1` avec `errno = EAGAIN` si aucune donnée disponible
- `connect()` retourne immédiatement ; tester la complétion avec `select()` sur le fd en écriture
- Base des architectures event-driven (nginx, Node.js, Redis)

## Réutilisation d'adresse

```c
int opt = 1;
setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
```

À appeler avant `bind()` pour éviter « Address already in use » après redémarrage du serveur.
