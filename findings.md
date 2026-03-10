# findings.md — Audit d'alignement NSY103 / UTC502 / clings

Date: 2026-03-09

## 1. Structure documentaire

### Cours référencés

| Dossier | Cours | Description |
|---------|-------|-------------|
| `docs/nsy103/` | NSY103 | "Linux : noyau et programmation système" — cours principal ciblé |
| `docs/utc502/` | UTC502 | "Gestion des ressources informatiques" — cours connexe, niveau plus élevé |

**Important**: `docs/utc502/ex.md` et les exercices FIFO/LRU/MFU appartiennent à **UTC502**, pas à NSY103.

### Annales disponibles

**NSY103** (`docs/nsy103/annales/`) :
- `premieresession20223024NSY103FOD.pdf` — Session 1 2023-24
- `secondesessionNSY103FODS220222023.pdf` — Session 2 2022-23
- `deuxièmesessionNsy103FODS120222023.pdf` — Session 2 2022-23

**UTC502** (`docs/utc502/example examen/`) :
- `premieresession20212022UTC502FOD.pdf`
- `secondedsession20212022UTC502FOD.pdf`
- `NFA003_23-24.pdf` — cours NFA003 (différent)

### Code source exemples (UTC502 examen)

`docs/utc502/example examen/SourcesC_Processus_Thread_Mutex/` :
- `fork0.c`, `fork1.c` — fork basique
- `fork_boucle.c`, `fork_boucle_wait.c` — fork en boucle
- `thread_sans_mutex.c`, `thread_with_mutex.c` — threads mutex
- `stock1.c` — problème producteur-consommateur

## 2. Inventaire des exercices (274 total)

| Sujet | Exercices | Ch. | Statut |
|-------|-----------|-----|--------|
| bitwise_ops | 12 | 2 | Complet |
| capstones | 12 | 15 | Complet |
| errno | 12 | 3 | Complet |
| fd_basics | 12 | 4 | Complet |
| file_io | 13 | 4 | Complet |
| filesystem | 12 | 5 | Complet |
| memory_allocation | 13 | 3 | Complet |
| message_queues | 12 | 9 | Complet |
| pipes | 12 | 8 | Complet |
| pointers | 13 | 1 | Complet |
| proc_memory | 12 | 14 | Complet |
| processes | 12 | 6 | Complet |
| pthreads | 13 | 12 | Complet |
| semaphores | 12 | 11 | Complet |
| shared_memory | 12 | 10 | Complet |
| signals | 12 | 7 | Complet |
| sockets | 13 | 13 | Complet |
| string_formatting | 12 | 2 | Complet |
| structs | 13 | 1 | Complet |
| sync_concepts | 12 | 12 | Complet |
| virtual_memory | 14 | 14 | Complet |

### Distribution des difficultés

| Niveau | Count | % |
|--------|-------|---|
| D1 Easy | ~89 | 32% |
| D2 Medium | ~80 | 29% |
| D3 Advanced | ~63 | 23% |
| D4 Hard | ~34 | 12% |
| D5 Expert | ~8 | 3% |

## 3. Alignement NSY103

### Sujets d'examen récurrents (d'après annales 2022-24)

1. **fork() et processus** — toujours présent (8 pts en 2023-24)
2. **Gestion du FS** — inodes, blocs (3 pts)
3. **Files de messages** ou **Sockets** — communication (4 pts)
4. **Threads et mutexes** — sync pthread (variable)
5. **Tubes anonymes** — occasionnel

### Couverture par thème NSY103

| Thème NSY103 | Couverture KF | Exercices |
|---|---|---|
| Processus (fork/exec/wait) | Excellente | 12 exercices Ch.6 |
| Signaux POSIX (1-63) | Excellente | 12 exercices Ch.7 |
| Tubes anonymes | Excellente | 12 exercices Ch.8 |
| Files de messages (SysV) | Excellente | 12 exercices Ch.9 |
| Mémoire partagée | Excellente | 12 exercices Ch.10 |
| Sémaphores POSIX | Excellente | 12 exercices Ch.11 |
| Threads POSIX (pthreads) | Excellente | 13 exercices Ch.12 |
| Sockets TCP/UDP | Bonne | 13 exercices Ch.13 |
| Système de fichiers (inode) | Bonne | 12 exercices Ch.5 |
| Producteur-consommateur | Couverte | sem_prodcons_01, pthread_pool_01 |
| Lecteurs-rédacteurs | Couverte | sem_readers_01, pthread_rwlock_01 |
| **Tubes nommés (FIFO)** | **Absente** | 0 exercice dédié |

## 4. Alignement UTC502

### Thèmes UTC502 dans docs/utc502/Chapitres/

- Ch.1a/1b — Introduction, ordonnancement processus
- Ch.2 — Gestion de la mémoire
- Ch.3 — Mémoire virtuelle, pagination
- Ch.4 — Algorithmes de remplacement de pages (FIFO, LRU, MFU, Optimal)
- Ch.5 — Système de fichiers
- Ch.6 — Ordonnancement
- Ch.7 — E/S et interruptions
- Ch.8 — Sécurité

### Couverture UTC502 par clings

| Thème UTC502 | Couverture KF | Notes |
|---|---|---|
| Pagination / pages mémoire | Partielle | vm_page_01 (getpagesize seulement) |
| **Algorithmes FIFO/LRU/MFU** | **ABSENTE** | ex.md montre que c'est clé d'examen |
| **Ordonnancement (FIFO/RR/SJF)** | **ABSENTE** | Aucun exercice |
| Mémoire virtuelle avancée | Bonne | 14 exercices (CoW, mmap, TLB, NUMA) |
| Système de fichiers (FS) | Bonne | 12 exercices Ch.5 |
| Processus / threads | Excellente | Ch.6, Ch.12 |

## 5. Lacunes identifiées

### Critiques (examen)

1. **Page replacement (FIFO/LRU/MFU)** — directement dans `docs/utc502/ex.md`
   - Exercices à créer: `vm_fifo_01.json`, `vm_lru_01.json`, `vm_mfu_01.json`
   - Exercice: simuler algorithme sur séquence d'accès, compter défauts de page

2. **Ordonnancement (scheduling)** — Ch.1 et Ch.6 UTC502, Ch.1 NSY103
   - Exercices à créer: simulation FIFO/Round-Robin/SJF avec processus et quantum

3. **Tubes nommés (FIFO nommés / mkfifo)** — dans `lestubesanonymes.pdf` NSY103
   - Exercice à créer: `pipe_fifo_named_01.json`

### Secondaires (enrichissement)

4. **Unix domain sockets** — IPC avancée (sock_unix_01 existe mais à vérifier)
5. **Signalfd / eventfd** — sig_signalfd_01 existe
6. **Namespaces Linux** — non couverts (hors scope NSY103, pertinent UTC502)
7. **Cgroups** — hors scope NSY103

## 6. Sujets sur-représentés

- `virtual_memory`: 14 exercices (2 de plus que la moyenne)
- Le contenu est de qualité mais légèrement redondant sur les bases (vm_page_01 + vm_align_01 traitent des concepts très proches)

## 7. Qualité des exercices échantillonnés

Tous les exercices lus montrent:
- Structure JSON bien définie (id, subject, difficulty, key_concept, starter_code, solution_code, hints, validation)
- Progressions pédagogiques cohérentes au sein de chaque sujet
- Niveaux de difficulté appropriés au contenu
- `exercise_type: "complete"` uniformément (complétion de code)

## 8. Qualité du code (audit 2026-03-10)

### Couverture API docs

| État | Avant | Après |
|------|-------|-------|
| Symboles publics documentés | ~92% | ~98% |
| Fonctions sans doc `///` | `compile_and_run`, `reset_progress` | 0 critique |
| Exemples (`# Examples`) | 0 | 1 (`compile_and_run`) |

### Couverture tests

| État | Avant | Après |
|------|-------|-------|
| Tests totaux | 124 | 134 |
| Fichiers avec tests | 7 | 9 |
| Nouveaux fichiers testés | — | `error.rs`, `tmux.rs` |
| Tests display ajoutés | — | `difficulty_stars` (×2) |

### Sécurité (commits bdff318)

| Fix | Fichier | Nature |
|-----|---------|--------|
| Path traversal | `src/runner.rs` | `canonicalize()` après création fichier |
| Atomic write | `src/runner.rs` | `rename()` depuis `.tmp` |
| HOME hard-fail | `src/progress.rs`, `src/runner.rs` | Erreur explicite si `$HOME` absent |

### Gaps non résolus (TUI/terminal — intentionnel)

| Fichier | Raison |
|---------|--------|
| `src/main.rs` | `cmd_*` : I/O + DB, nécessite injection de dépendances |
| `src/watcher.rs` | boucle inotify + stdin thread, fd mock difficile |
| `src/piscine.rs` | boucle raw mode TUI, pas testable sans terminal |

Ces 3 fichiers représentent la couche d'entrée/TUI — les tests unitaires seraient fragiles.
La logique métier sous-jacente (mastery, progress, runner) est couverte.
