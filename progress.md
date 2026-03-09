# progress.md — État d'avancement clings

Date: 2026-03-09

## État général

| Indicateur | Valeur |
|-----------|--------|
| Exercices total | 274 |
| Sujets | 21 |
| Chapitres NSY103 | 15 |
| Alignement NSY103 | ~90% |
| Alignement UTC502 | ~75% |
| Tests unitaires | Oui (mastery.rs, chapters.rs) |
| Build | OK (`cargo build`) |
| Lint | OK (`cargo clippy -- -D warnings`) |

## Sessions de travail

### Session 2026-03-09 (current)

- [x] Audit d'alignement NSY103/UTC502 lancé (3 subagents parallèles)
- [x] Cartographie des 274 exercices par sujet et difficulté
- [x] Identification des lacunes critiques (page replacement, scheduling, FIFO nommés)
- [x] findings.md créé
- [x] task_plan.md créé
- [x] progress.md créé (ce fichier)
- [x] T3 — pipe_fifo_named_01.json + pipe_fifo_ipc_01.json (tubes nommés mkfifo)
- [x] T5 — fs_blocks_01.json (st_blocks/st_blksize, portabilité tmpfs avec memset non-nul)
- [x] T6 — sync_readers_writers_starvation_01.json (writer-prefer, entry_mutex)
- [x] T1 — vm_page_replacement_fifo/lru/opt_01.json (FIFO=9, LRU=10, OPT=7 fautes)
- [x] T2 — sched_fifo/sjf/rr/priority_01.json (4 algos scheduling)
- [x] T7 — Ch.6 "Ordonnancement" ajouté à chapters.rs (16 chapitres total)
- [ ] T4, T8 — vérif fork/thread + documentation

### Session précédente (2026-03-08)

- Audit qualité documentation + tests coverage (voir mémoire #S1084)
- Exploration src/watcher.rs, tmux.rs, Cargo.toml, main.rs, piscine.rs

## Tâches en cours

Voir `task_plan.md` pour le détail complet.

### T1 — Page replacement (UTC502) [ ]
- 3 exercices à créer: FIFO, LRU, Optimal
- Référence: `docs/utc502/ex.md` + `docs/utc502/Chapitres/chapitre4_UTC502.pdf`
- Correction disponible: `docs/utc502/Corriges/chapitre4_UTC502_correction_exercices_1_2_gestion_des_pages.pdf`

### T2 — Ordonnancement [ ]
- 4 exercices: FIFO, Round-Robin, SJF, priority
- Référence: `docs/utc502/Chapitres/chapitre1a_UTC502.pdf`, `chapitre6_UTC502.pdf`

### T3 — Tubes nommés (mkfifo) [ ]
- 2 exercices: mkfifo basique + IPC multi-processus
- Référence: `docs/nsy103/lestubesanonymes.pdf`

### T4-T8 — Vérifications et documentation [ ]
- Détails dans task_plan.md

## Fichiers clés

| Fichier | Rôle |
|---------|------|
| `src/chapters.rs` | Progression 15 chapitres NSY103 |
| `src/models.rs` | Types Exercise, Subject, Difficulty |
| `src/exercises.rs` | Chargement JSON depuis exercises/ |
| `src/runner.rs` | Compilation gcc + validation output |
| `src/mastery.rs` | Algorithme SRS (spaced repetition) |
| `exercises/*/` | 274 exercices JSON |
| `docs/nsy103/` | Cours NSY103 + 3 annales |
| `docs/utc502/` | Cours UTC502 + 2 annales + TP |
| `findings.md` | Résultats de l'audit |
| `task_plan.md` | Plan d'action priorisé |

## Décisions architecturales

- **Exercices de type "simulation"** (page replacement, scheduling): utiliser `exercise_type: "complete"` + `validation.mode: "Output"` avec expected_output calculé pour la séquence de référence
- **Nouveau sujet scheduling**: créer `exercises/scheduling/` + ajouter Ch.6 dans chapters.rs (renumérote)
- **UTC502 vs NSY103**: exercices page replacement vont dans `exercises/virtual_memory/` (Ch.14), pas un nouveau chapitre
