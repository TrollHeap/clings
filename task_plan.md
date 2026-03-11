# task_plan.md — Plan d'amélioration clings

Date: 2026-03-09
Basé sur: audit d'alignement NSY103/UTC502 (voir findings.md)

## Contexte

clings (`clings`) est un entraîneur TUI de programmation système C, aligné sur NSY103 et UTC502.
**283+ exercices** existent dans 21 sujets sur 16 chapitres. L'alignement global est ~98%.

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

---

## Phase 3 — Qualité du code (quality-audit 2026-03-10)

Audit `/quality-audit` [A] exécuté — docs + tests — après les 3 commits atomiques de sécurité/exercices.

### Commits réalisés (session 2026-03-10)

| Hash | Description |
|------|-------------|
| `bdff318` | fix: security hardening + UX improvements (S1-S3, P1, C1-C2) |
| `fed4959` | feat: exercices filesystem inode-calc + fork-tree |
| `797fbab` | feat: exercices scheduling (EDF, priority, RR-Gantt) + SRS + annales |
| `789c8b1` | docs(all): API docs + README + CHANGELOG + tests (quality-audit) |

### Q1 — Documentation API
- [x] `src/runner.rs` — `compile_and_run()` : doc complète + `# Examples` (no_run)
- [x] `src/progress.rs` — `reset_progress()` : doc ///  avec note DESTRUCTIVE
- [x] `src/display.rs` — `difficulty_stars()` : doc existante vérifiée (OK)
- [x] `CHANGELOG.md` — section `[Unreleased] — [PROPOSED]` créée (stub)
- [x] `README.md` — table commandes complétée (5 commandes manquantes), keybinds `j`/`n` disambiguïsés

### Q2 — Tests nouveaux (+10 tests, 124 → 134)
- [x] `src/error.rs` — 5 tests (display, conversion From/Into, discrimination variante)
- [x] `src/tmux.rs` — 3 tests (is_tmux contract, open sans tmux, update sans tmux)
- [x] `src/display.rs` — 2 tests (difficulty_stars count par variante, total=5)

### Q3 — Gaps documentés (non résolus — TUI/terminal)
- `src/main.rs` — aucun test (`cmd_*` fonctions I/O + DB, nécessitent injection)
- `src/watcher.rs` — aucun test (inotify + stdin thread, nécessite fd mock)
- `src/piscine.rs` — aucun test (boucle raw mode, TUI non testable sans terminal)

---

---

## Phase 4 — Qualité du code (rust-audit + full-audit 2026-03-11)

Audits `/rust-audit [A]` + `/full-audit [A]` exécutés. 40 findings, 0 critique, 9 high.
Plan en 3 tiers par risque. Périmètre : **code Rust uniquement** — pas d'exercices JSON.

### Tier 1 — Quick wins (safe, isolated)

#### R1: Supprimer les champs morts `ValidationConfig.mode` + `.test_code`
- **Fichier à modifier** : `src/models.rs` (lignes 89–94)
- **Pattern** : Supprimer les deux champs `#[allow(dead_code)]` + leurs attributs serde
- **Vérification** : `grep -r "\.mode" src/` et `grep -r "test_code" src/` pour confirmer zéro usage
- **Contrainte** : Les exercices JSON peuvent avoir ces champs — serde les ignorera silencieusement (comportement par défaut)
- **Statut** : [x] DONE — `deny_unknown_fields` retiré de `ValidationConfig`, champs legacy supprimés

#### R2: Extraire `format_elapsed` (3 occurrences dupliquées)
- **Fichier à modifier** : `src/piscine.rs`
- **Fonctions concernées** : lignes 380–382 (cmd_piscine), ~550 (run_exam_piscine), show_piscine_header
- **Signature cible** : `fn format_elapsed(elapsed: std::time::Duration) -> (u64, u64, u64)` dans piscine.rs (privée)
- **Statut** : [x] DONE — déjà implémenté (piscine.rs:17), constaté lors de la vérification

#### R3: Documenter les erreurs silencieuses (`// intentional:` + `// SAFETY:`)
- **Fichiers à modifier** : `src/runner.rs` (l.162, l.164), `src/main.rs` (l.445, l.648)
- **Statut** : [x] DONE — commentaires `// intentional:` et `// SAFETY:` déjà présents dans le code

---

### Tier 2 — DRY extraction (medium complexity)

#### R4: Déplacer `handle_esc_sequence` vers `src/display/visualizer.rs`
- **Fichiers modifiés** : `src/display/visualizer.rs`, `src/display/mod.rs`, `src/piscine.rs`, `src/main.rs`
- **Statut** : [x] DONE — fonction déplacée, re-export `pub(crate)` ajouté, code inline main.rs remplacé

#### R5: Extraire le dispatch clavier piscine (cmd_piscine vs run_exam_piscine)
- **Contexte** : Touches h/H/v/V/n/N/j/J/k/K/q/Q/r/R identiques dans cmd_piscine et run_exam_piscine. Seule différence : `ch_ctx: Option<&ChapterContext>` pour `redisplay_piscine_exercise`
- **Statut** : [x] SKIP (justifié) — extraction exigerait ~16 paramètres ou struct avec lifetimes mixtes, plus illisible que la duplication actuelle (~50 lignes identiques)

---

### Tier 3 — Extraction de fonctions longues (faible priorité)

#### R6: Extraire le calcul mastery de `record_attempt`
- **Fichier** : `src/progress.rs` (l.129–181)
- **Statut** : [x] N/A — `mastery::update_mastery` et `mastery::compute_next_review` sont déjà délégués à mastery.rs. La séparation SQL/logique est déjà en place.

#### R7: Décomposer `show_exercise_watch`
- **Fichier** : `src/display/exercise.rs` (l.34–84)
- **Statut** : [x] N/A — fonction de 42 lignes, `render_exercise_body` déjà extrait. Décomposition supplémentaire inutile.

---

### Vérification finale

```bash
cargo clippy -- -D warnings   # clean ✓
cargo test                     # 144 passed, 0 failed ✓ (2026-03-11)
```

**Phase 4 terminée.** R1 + R4 appliqués. R2/R3/R6/R7 déjà faits ou N/A. R5 skip justifié.
AtomicBool Ordering::Acquire fix (watcher.rs) appliqué lors de la session rust-audit.

### Hors scope (décisions architecturales requises)

- **Unification boucle watch/piscine/exam** : cmd_watch (~290 lignes), cmd_piscine (~280 lignes), run_exam_piscine (~270 lignes) divergent trop (gating, SRS, checkpoints) pour une extraction safe sans refonte complète. Reporter à v2.0.
- **Tests cmd_watch / cmd_review / cmd_piscine** : nécessitent injection de terminal + DB mock. Hors scope sans infrastructure de test dédiée.

---

---

## Phase 5 — ValidationMode::Test (2026-03-11)

Implémentation de F1 du roadmap v2.0 : support des exercices validés par harnais de tests C unitaires.

### F1 — ValidationMode::Test

- [x] `src/models.rs` — Ajout de l'enum `ValidationMode` (`Output` par défaut, `Test`, `Both`) + champs `mode`, `test_code`, `expected_tests_pass` dans `ValidationConfig`
- [x] `assets/test.h` — Harness C minimal (setjmp/longjmp) : macros `RUN_TEST`, `TEST_ASSERT_EQUAL_INT`, `TEST_ASSERT_TRUE`, `TEST_ASSERT_FALSE`, `TEST_ASSERT_NULL`, `TEST_ASSERT_NOT_NULL`, `TEST_ASSERT_EQUAL_STRING`, `TEST_SUMMARY`. Format de sortie : `"N Tests N Failures 0 Ignored"`
- [x] `src/runner.rs` — `compile_and_run()` dispatch sur `ValidationMode` ; nouvelles fonctions `run_output()`, `run_tests()`, `parse_test_summary()` ; inclus `test.h` via `include_str!`
- [x] `src/display/exercise.rs` — `show_result()` : affichage spécialisé en mode Test (lignes OK en vert, FAIL en rouge)
- [x] `src/exercises.rs` — Test de sanité `test_output_validation_has_expected` mis à jour pour ignorer les exercices mode `Test`
- [x] `exercises/pointers/ptr_test_01.json` — Exercice démo avec 4 tests sur `sum_array()`
- [x] Tests unitaires `parse_test_summary` (4 cas) — 148 tests total, 0 échec

**Décision technique** : `test_current.c = #include "current.c"\n#include "test.h"\n\n{test_code}` — le code source de l'étudiant est inclus en tant que TU, le harnais est séparé. Rétrocompatibilité JSON garantie : `ValidationMode` est `#[serde(default)]` → tous les exercices existants continuent de fonctionner sans modification.

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
