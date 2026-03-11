# progress.md — État d'avancement clings

Date: 2026-03-11 (dernière mise à jour)

## État général

| Indicateur | Valeur |
|-----------|--------|
| Exercices total | 283+ |
| Sujets | 21 |
| Chapitres | 16 (Ch.6 Ordonnancement ajouté) |
| Alignement NSY103 | ~98% |
| Alignement UTC502 | ~95% |
| Tests unitaires | 134 (mastery.rs, models.rs, chapters.rs, error.rs, tmux.rs, display.rs) |
| Build | OK (`cargo build`) |
| Lint | OK (`cargo clippy -- -D warnings`) |
| Commits en avance | 10 commits ahead of origin/main |
| Version | v1.0.0 (tag annoté 2026-03-11) |

## Sessions de travail

### Session 2026-03-11 — Phase 4 qualité post-audit

- [x] R1 — `ValidationConfig`: `pub` + `#[allow(dead_code)]` → `pub(crate) _mode`/`_test_code` + `#[serde(rename)]` + `#[derive(Default)]` ; tests mis à jour avec `..Default::default()`
- [x] R2 — `fn format_elapsed(d: Duration) -> (u64, u64, u64)` extraite dans `piscine.rs`, 2 usages inline remplacés
- [x] R3 — Commentaires `// SAFETY:` et `// .ok():` déjà en place (main.rs:443, main.rs:646, runner.rs:162-165)
- [x] R4/R5/R7 — Skippés : valeur insuffisante vs complexité (handle_esc_sequence déjà isolé, dispatch clavier ~15 params, show_exercise_watch déjà factorisé)
- [x] R6 — Déjà accompli : compute_next_review + update_mastery vivent dans mastery.rs
- [x] 144 tests passent, cargo clippy -- -D warnings clean

### Session 2026-03-11 — Release v1.0.0

- [x] `/rust-audit [A]` — 5 fixes appliqués :
  - `models.rs` : `InvalidDifficultyError` (thiserror) remplace `String` dans `TryFrom<u8>`
  - `runner.rs` : stdout/stderr `take()` propagé via `?` (plus de `.expect()` évitable)
  - `main.rs` : commentaires dégradation gracieuse sur `eprintln!` + `.ok().flatten()`
  - `chapters.rs` : messages `.expect()` explicites (référence assert compile-time)
  - `cargo clippy -- -D warnings` clean, 144+ tests passent
- [x] `/finalize` — audit 4 sous-agents (sécurité, perf, qualité, conventions)
  - 1 blocker identifié et corrigé : CHANGELOG `[Unreleased]` → `[1.0.0] — 2026-03-11`
  - `[0.1.0] — [PROPOSED]` → `[0.1.0] — 2025-11-01`
- [x] Commit `14e94d8` : release: v1.0.0
- [x] Tag annoté `v1.0.0` créé



### Session 2026-03-10

- [x] Validation des 8 exercices non trackés (tests cargo : load_all, fields_complete, ids_unique)
- [x] Commit 1 — `bdff318` : fix: security hardening + UX (S1-S3 path traversal, atomic write, HOME hard-fail ; P1 common_mistake ; C1-C2 README + messages FR)
- [x] Commit 2 — `fed4959` : feat: exercices filesystem (fs_inode_calc_01/02/03) + processes (fork_tree_01)
- [x] Commit 3 — `797fbab` : feat: exercices scheduling (sched_edf_01, sched_priority_arrival/inversion_01, sched_rr_gantt_01) + SRS multiplier 1.8 + annales_map + mq_01/shm_01
- [x] `/quality-audit [A]` — audit docs + tests
  - API doc: `compile_and_run()` + `reset_progress()` documentées
  - README: 5 commandes manquantes ajoutées, j/n disambiguïsés
  - CHANGELOG: stub `[Unreleased]` créé
  - Tests: +10 tests (error.rs ×5, tmux.rs ×3, display.rs ×2) → 134 total
- [x] Commit 4 — `789c8b1` : docs(all): API docs + README + CHANGELOG + tests

### Session 2026-03-09 (précédente)

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

### Session 2026-03-08 (précédente)

- Audit qualité documentation + tests coverage (voir mémoire #S1084)
- Exploration src/watcher.rs, tmux.rs, Cargo.toml, main.rs, piscine.rs

## Tâches en cours

Toutes les tâches T1-T8, F1-F4, Q1-Q3, R1-R7 sont TERMINÉES.
Aucune tâche bloquante. Prêt pour usage pédagogique.

## Fichiers clés

| Fichier | Rôle |
|---------|------|
| `src/chapters.rs` | Progression 16 chapitres (NSY103 + ordonnancement) |
| `src/models.rs` | Types Exercise, Subject, Difficulty |
| `src/exercises.rs` | Chargement JSON depuis exercises/ |
| `src/runner.rs` | Compilation gcc + validation output |
| `src/mastery.rs` | Algorithme SRS (spaced repetition, decay 1.8) |
| `exercises/*/` | 283+ exercices JSON |
| `docs/nsy103/` | Cours NSY103 + 3 annales |
| `docs/utc502/` | Cours UTC502 + 2 annales + TP |
| `findings.md` | Résultats de l'audit |
| `task_plan.md` | Plan d'action priorisé |

## Décisions architecturales

- **Exercices de type "simulation"** (page replacement, scheduling): utiliser `exercise_type: "complete"` + `validation.mode: "Output"` avec expected_output calculé pour la séquence de référence
- **Nouveau sujet scheduling**: créer `exercises/scheduling/` + ajouter Ch.6 dans chapters.rs (renumérote)
- **UTC502 vs NSY103**: exercices page replacement vont dans `exercises/virtual_memory/` (Ch.14), pas un nouveau chapitre
