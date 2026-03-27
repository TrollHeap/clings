# task_plan.md — Full Audit Remediation v3.1.1

## Phase A — Sécurité + Quick Wins

### T1 : Permissions 0o700

- **Statut** : [x] done (progress.rs + config.rs — déjà appliqué)

### T2 : Error variant watcher — Config → Watch

- **Statut** : [x] done (watcher.rs ligne 68 — déjà appliqué)

### T3 : Index SQLite

- **Statut** : [x] done (progress.rs SCHEMA — déjà appliqué)

### T4 : Dev-deps inutilisées

- **Statut** : [x] invalid — insta + proptest utilisés dans tests/

## Phase B — Performance

### T5 : build_list_display_items O(n²) → O(n)

- **Statut** : [x] done (2-pass avec HashMap — déjà appliqué)

### T6 : get_streak() NaiveDate

- **Statut** : [x] done (NaiveDate direct — déjà appliqué)

## Phase C — Clean Code app.rs

### T7 : handle_compile() extrait

- **Statut** : [x] done (app.rs ligne 887 — déjà appliqué)

### T8 : Déduplication handle_overlay_dispatch()

- **Statut** : [x] done (déjà appliqué)

### T9 : Navigation chapitres extraite

- **Statut** : [x] done (find_next/prev_chapter_exercise — déjà appliqués)

## Phase D — Clean Code backend

### T10 : build_gcc_args extrait

- **Statut** : [x] done (build_gcc_compilation_args — déjà appliqué)

### T11 : Doc comments pub

- **Statut** : [x] done (chapters.rs + piscine.rs — déjà appliqués)

### T12 : Paramètre mort _filter_subject

- **Statut** : [x] invalid — filter_subject est utilisé dans run_list()

### T13 : render_opaque_background helper

- **Statut** : [x] done (common.rs:997 — déjà appliqué)

## Phase E — Optionnel (skipped)

T14–T17 : trade-off défavorable ou déjà propre.

## Phase F — Low findings

### T18 : Hint counter duplication

- **Statut** : [x] done (append_hint_counter_if_visible dans common.rs — déjà appliqué)

---

## Bugs corrigés en session (non listés dans l'audit)

- **pipe-epoll-01** : output non-déterministe → qsort avant printf (commit 8b93f14)
- **capstone-proxy-01** : race condition printf/write → printf avant write dans client_thread (commit 8b93f14)

---

## Résumé

**Statut global : TERMINÉ**

Toutes les tâches d'audit étaient déjà appliquées. Deux bugs de flakiness corrigés.
Tests : 526 passed, 0 failed. Clippy : clean.
