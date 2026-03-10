# task_plan.md — Plan d'amélioration clings

Date: 2026-03-09
Basé sur: audit d'alignement NSY103/UTC502 (voir findings.md)

## Contexte

clings (`clings`) est un entraîneur TUI de programmation système C, aligné sur NSY103 et UTC502.
**274 exercices** existent dans 21 sujets sur 15 chapitres. L'alignement global est ~85-90%.

## Objectif

Combler les lacunes identifiées dans findings.md pour atteindre un alignement ~98% avec NSY103 et UTC502.

---

## Tâches

### Priorité 1 — Critiques (examen)

#### T1: Exercices algorithmes de remplacement de pages (UTC502 Ch.4)
- **Fichier source**: `docs/utc502/ex.md` + `docs/utc502/Chapitres/chapitre4_UTC502.pdf`
- **Corrrigés**: `docs/utc502/Corriges/chapitre4_UTC502_correction_exercices_1_2_gestion_des_pages.pdf`
- Créer dans `exercises/virtual_memory/`:
  - [x] `vm_page_replacement_fifo_01.json` — D2: simuler FIFO avec 3 frames, compter défauts
  - [x] `vm_page_replacement_lru_01.json` — D3: LRU avec historique accès
  - [x] `vm_page_replacement_opt_01.json` — D4: algorithme optimal (Bélády)
- **Validation**: programme C qui simule l'algo et affiche nombre de défauts de page
- **Status**: [x] DONE

#### T2: Exercices ordonnancement (UTC502 Ch.1/Ch.6, NSY103)
- **Fichier source**: `docs/utc502/Chapitres/chapitre1a_UTC502.pdf`, `chapitre6_UTC502.pdf`
- Créer dans `exercises/processes/` ou nouveau sujet `scheduling/`:
  - [x] `sched_fifo_01.json` — D2: simuler FIFO scheduling, calculer temps moyen
  - [x] `sched_rr_01.json` — D3: Round-Robin avec quantum, Gantt chart en sortie
  - [x] `sched_sjf_01.json` — D2: SJF (Shortest Job First), temps d'attente
  - [x] `sched_priority_01.json` — D3: priorités et préemption avec nice()
- **Status**: [x] DONE

#### T3: Tubes nommés / mkfifo (NSY103 — lestubesanonymes.pdf)
- **Fichier source**: `docs/nsy103/lestubesanonymes.pdf`
- Créer dans `exercises/pipes/`:
  - [x] `pipe_fifo_named_01.json` — D2: mkfifo, open en mode RDONLY/WRONLY, écriture/lecture
  - [x] `pipe_fifo_ipc_01.json` — D3: FIFO nommé entre deux processus indépendants
- **Status**: [x] DONE

---

### Priorité 2 — Secondaires (complétude pédagogique)

#### T4: Vérification des exercices UTC502 source code (fork/threads)
- **Fichier source**: `docs/utc502/example examen/SourcesC_Processus_Thread_Mutex/`
  - `fork_boucle.c` — fork dans une boucle, compter processus
  - `thread_without_mutex.c` vs `thread_with_mutex.c` — race condition visible
  - `stock1.c` — producteur-consommateur avec mutex
- Vérifier si ces patterns sont couverts par des exercices existants:
  - [x] Mapper `fork_boucle.c` → exercice existant dans `processes/`
  - [x] Mapper `thread_with_mutex.c` → exercice existant dans `pthreads/`
  - [x] Mapper `stock1.c` → exercice existant (sem_prodcons_01 ?)
- Créer uniquement si pas couvert
- **Status**: [x] DONE

#### T5: Exercice inode/bloc de FS (NSY103 — SGFlinux.pdf)
- **Fichier source**: `docs/nsy103/SGFlinux.pdf`, `SGFnotionsgénérales.pdf`
- Les annales NSY103 ont souvent une question sur les inodes et blocs (3 pts)
- Vérifier si `exercises/filesystem/fs_inode_01.json` couvre: st_ino, st_nlink, hard links
  - [x] Créer `fs_blocks_01.json` — D3: st_blocks/st_blksize avec memset non-nul (portabilité tmpfs)
  - [x] Audit: fs_inode_01 couvre déjà hard links / st_nlink → pas d'exercice supplémentaire nécessaire
- **Status**: [x] DONE

#### T6: Exercice lecteurs-rédacteurs dédicacé (NSY103 — lecteurs-rédacteurs.pdf)
- `sem_readers_01.json` et `pthread_rwlock_01.json` existent — vérifier s'ils couvrent:
  - [x] sem_readers_01 / pthread_rwlock_01 / sync_rwlock_01 couvrent le problème classique
  - [x] Starvation et solution writer-prefer : `sync_readers_writers_starvation_01.json` créé (D4)
- **Status**: [x] DONE

---

### Priorité 3 — Mise à jour chapters.rs (si T2 aboutit)

#### T7: Ajouter chapitre "Ordonnancement" dans src/chapters.rs
- Si les exercices scheduling sont créés, ajouter:
  ```rust
  Chapter {
      number: 6,  // avant Processus
      title: "Ordonnancement",
      subjects: &["scheduling"],
  },
  ```
- Renumérote les chapitres 6→15 en 7→16
- **Status**: [x] DONE

---

### Priorité 4 — Documentation

#### T8: Documenter la distinction NSY103 vs UTC502
- Ajouter dans CLAUDE.md ou README.md:
  - Exercices marqués NSY103-core vs UTC502-extended
  - Quelle annale sert de référence pour chaque chapitre
- **Status**: [x]

---

---

## Phase 2 — Features UX/Mastery (audit post-alignement)

Date: 2026-03-09

### F1: `clings review` — Renforcement mastery orienté pratique
- **Fichiers à modifier**: `src/progress.rs`, `src/main.rs`, `src/display.rs`
- **Fonctions à créer**:
  - `progress.rs`: `get_due_subjects(conn) -> Vec<Subject>` — WHERE `next_review_at <= unixepoch()`
  - `main.rs`: `cmd_review(conn, exercises)` — filtre exercices des sujets "due", lance le premier
  - `display.rs`: `show_review_prompt(due_count)` — affiche banner "N sujets à renforcer"
- **Pattern à suivre**: `cmd_progress()` pour l'ouverture DB + apply_decay
- **Contrainte**: Framing "renforcement mastery pratique" pas "révision théorique SRS"
- **Status**: [x] DONE

### F2: `starter_code_stages` dans les JSON d'exercices
- **Fichiers à modifier**: tous les `exercises/**/*.json` (sauf sched_fifo_01.json déjà fait)
- **Format**: 5 stages indexés 0-4 dans `starter_code_stages: Vec<String>`
  - S0 (mastery<1.0): Exemple commenté complet + squelette TODO
  - S1 (mastery<2.0): Guidance forte, blancs à combler avec commentaires
  - S2 (mastery<3.0): Blancs `____` comme starter_code actuel
  - S3 (mastery<4.0): Squelette #include + main() uniquement
  - S4 (mastery>=4.0): `#include <...>\nint main(void) { return 0; }` vide
- **Template de référence**: `exercises/scheduling/sched_fifo_01.json`
- **Batches**:
  - Batch A: `bitwise_ops/`, `pointers/`, `structs/`, `string_formatting/` — déjà remplis
  - Batch B: `processes/`, `pthreads/`, `semaphores/`, `signals/` — pthread_race_01 corrigé (3→5 stages)
  - Batch C: `pipes/`, `sockets/`, `file_io/`, `shared_memory/`, `message_queues/`, `memory_allocation/` — déjà remplis
- **Status**: [x] DONE

### F3: `clings stats` — Statistiques mastery par sujet
- **Fichiers à modifier**: `src/main.rs`, `src/display.rs`
- **Fonctions à créer**:
  - `main.rs`: `cmd_stats(conn, exercises)` — agrège par sujet
  - `display.rs`: `show_stats(subjects, streak)` — tableau mastery + succès + streak
- **Données disponibles**: `Subject` a mastery, `practice_log` a success/failure
- **Pattern à suivre**: `cmd_progress()` → `show_progress()` pour le pattern
- **Status**: [x] DONE

### F4: Diff visuel ligne-à-ligne dans `show_result()`
- **Fichiers à modifier**: `src/display.rs`
- **Fonction à modifier**: `show_result()` (L499–L550 approx)
- **Comportement**: split expected/obtained par `\n`, comparer ligne-par-ligne
  - Ligne identique → vert `✓ ligne`
  - Ligne différente → rouge expected `✗ exp:` + rouge obtained `  got:`
  - Lignes supplémentaires → rouge `+ ligne` / `- ligne`
- **Contrainte**: conserver le format box-drawing existant, adapter INNER_W=52
- **Status**: [x] DONE

---

## Non-prioritaires (hors scope NSY103/UTC502)

Ces sujets enrichiraient la plateforme mais ne sont pas dans les curricula:

- **Namespaces Linux** (unshare, setns) — containers modernes
- **Cgroups v2** — resource management avancé
- **eBPF / perf** — tracing kernel
- **Netlink sockets** — kernel-userspace IPC
- **seccomp** — sandboxing
- **Device drivers / ioctl** — programmation matériel

**Décision**: Reporter à une version future (v2.0+). Pas d'action maintenant.

---

## Résumé de priorités

| ID | Tâche | Impact | Effort | Statut |
|----|-------|--------|--------|--------|
| T1 | Page replacement FIFO/LRU/OPT | Critique examen UTC502 | 3 JSONs | [x] |
| T2 | Scheduling FIFO/RR/SJF | Critique NSY103/UTC502 | 4 JSONs | [x] |
| T3 | Tubes nommés mkfifo | Important NSY103 | 2 JSONs | [x] |
| T4 | Vérif fork/thread examples | Vérification | Audit + pthread_race_01 | [x] |
| T5 | Inode/bloc FS | NSY103 exam | 2 JSONs max | [x] |
| T6 | Lecteurs-rédacteurs | NSY103 | Vérif + 1 JSON | [x] |
| T7 | chapters.rs scheduling | Dépend T2 | 1 fichier Rust | [x] |
| T8 | Documentation NSY103 vs UTC502 | Clarté | README | [x] |
