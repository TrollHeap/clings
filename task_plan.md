# task_plan.md — Full Audit Remediation v3.1.1

## Phase A — Sécurité + Quick Wins (4 tâches)

### T1 : Permissions répertoire ~/.clings/ — mode 0o700 [SEC-1]
- **Fichiers à modifier** : `src/progress.rs` (ligne 66), `src/config.rs` (ligne 182)
- **Pattern à suivre** : `runner.rs:work_dir()` utilise `DirBuilder::new().mode(0o700)`
- **Contraintes** : Unix-only — conditionner avec `#[cfg(unix)]` pour portabilité
- **Fichiers adjacents** : `src/runner.rs` (pattern correct existant)
- **Statut** : [ ] pending

### T2 : Error variant watcher — Config → Watch [CONV-1]
- **Fichiers à modifier** : `src/watcher.rs` (ligne 68)
- **Pattern à suivre** : `KfError::Watch(String)` existe déjà dans `error.rs`
- **Contraintes** : Changement 1 ligne, pas de breaking change (KfError est interne)
- **Fichiers adjacents** : `src/error.rs`
- **Statut** : [ ] pending

### T3 : Index SQLite manquants [PERF-2, CONV-2]
- **Fichiers à modifier** : `src/progress.rs` — ajouter dans `SCHEMA` (après ligne 56)
- **Pattern à suivre** : Les CREATE TABLE existants dans SCHEMA
- **Contraintes** : Utiliser `CREATE INDEX IF NOT EXISTS` pour idempotence
- **Indexes** :
  - `idx_practice_log_practiced_at ON practice_log(practiced_at DESC)`
  - `idx_subjects_next_review ON subjects(next_review_at ASC, mastery_score ASC)`
- **Fichiers adjacents** : `src/progress.rs` (schema + migrate_v1)
- **Statut** : [ ] pending

### T4 : Supprimer dev-deps inutilisées [DRY]
- **Fichiers à modifier** : `Cargo.toml` (lignes 36-37)
- **Contraintes** : Supprimer `insta` et `proptest` de `[dev-dependencies]`
- **Statut** : [ ] pending

## Phase B — Performance (2 tâches)

### T5 : build_list_display_items O(n²) → O(n) [PERF-1, CLEAN-9]
- **Fichiers à modifier** : `src/tui/app.rs` — fn `build_list_display_items()` (lignes 311-360)
- **Algo actuel** : Pour chaque chapter header, boucle imbriquée `exercises[ex_idx..]` pour compter
- **Algo cible** : Un seul pass avec compteurs : pré-calculer les positions de chaque chapter boundary, insérer les headers avec les counts rétroactivement (ou 2-pass: pass 1 calcule counts, pass 2 construit items)
- **Pattern** : Même pattern que `order_by_chapters()` dans `chapters.rs:122-126` qui fait `subject_to_chapter` map en 1 pass
- **Fichiers adjacents** : `src/chapters.rs` (CHAPTERS array), `src/tui/app.rs` (ListDisplayItem enum)
- **Statut** : [ ] pending

### T6 : get_streak() — NaiveDate au lieu de string parsing [PERF-3]
- **Fichiers à modifier** : `src/progress.rs` — fn `get_streak()` (lignes 263-304)
- **Actuel** : `Utc::now().format("%Y-%m-%d").to_string()` + parse dans boucle
- **Cible** : Construire `NaiveDate` pour today/yesterday, comparer directement avec `NaiveDate::parse_from_str` une seule fois pour chaque entrée SQL, sans re-parse dans la boucle `windows(2)`
- **Contraintes** : API chrono — `chrono::NaiveDate` déjà importable (dep `chrono = "0.4"`)
- **Fichiers adjacents** : `src/progress.rs`
- **Statut** : [ ] pending

## Phase C — Clean Code : app.rs refactoring (3 tâches)

### T7 : Extraire handle_compile() depuis update_watch() [CLEAN-1]
- **Fichiers à modifier** : `src/tui/app.rs` — fn `update_watch()` (lignes 801-854)
- **Extraction** : Créer `fn handle_compile(&mut self, conn: &Connection)` qui encapsule:
  - Lecture path, compile_and_run, gestion success/failure, record_attempt, navigate
  - Le bloc `KeyCode::Char('r')` (lignes 801-855) entier
- **Pattern** : Similaire à `handle_hint_reveal()`, `handle_vis_toggle()` — méthodes `&mut self`
- **Fichiers adjacents** : `src/runner.rs` (compile_and_run), `src/progress.rs` (record_attempt)
- **Statut** : [ ] pending

### T8 : Dédupliquer logging dans handle_overlay_dispatch() [CLEAN-2]
- **Fichiers à modifier** : `src/tui/app.rs` — fn `handle_overlay_dispatch()` (lignes 629-669)
- **Actuel** : `load_current_exercise` + logging dupliqué aux lignes 637-643 et 649-655
- **Extraction** : Créer `fn load_exercise_and_save_checkpoint(&mut self, conn, session_id)`
- **Fichiers adjacents** : `src/tui/app.rs`
- **Statut** : [ ] pending

### T9 : Extraire navigation chapitres depuis handle_list_key() [CLEAN-3]
- **Fichiers à modifier** : `src/tui/app.rs` — fn `handle_list_key()` (lignes 432-476)
- **Extraction** : Créer 2 fonctions statiques :
  - `fn find_next_chapter_exercise(items: &[ListDisplayItem], from: usize) -> Option<usize>`
  - `fn find_prev_chapter_exercise(items: &[ListDisplayItem], from: usize) -> Option<usize>`
- **Pattern** : Même style que `next_exercise_item()` déjà extrait (ligne 363)
- **Fichiers adjacents** : `src/tui/app.rs`
- **Statut** : [ ] pending

## Phase D — Clean Code : modules backend (4 tâches)

### T10 : Extraire build_gcc_args depuis run_output() [CLEAN-8]
- **Fichiers à modifier** : `src/runner.rs` — fn `run_output()` (lignes 274-320)
- **Extraction** : Créer `fn build_gcc_args(...)` pour la construction de `extra_args` (lignes 281-289)
- **Pattern** : Même pattern que `run_test()` qui a une construction similaire
- **Fichiers adjacents** : `src/runner.rs`
- **Statut** : [ ] pending

### T11 : Doc comments sur fonctions pub [CONV-3]
- **Fichiers à modifier** :
  - `src/chapters.rs` — `order_by_chapters()` (112), `flatten_chapters()` (180), `filter_by_chapter()` (188)
  - `src/piscine.rs` — `run_exam_piscine()` (128)
- **Contraintes** : Doc comments `///` style, concis (1-2 phrases)
- **Statut** : [ ] pending

### T12 : Supprimer paramètre mort _filter_subject [DRY]
- **Fichiers à modifier** : `src/tui/ui_list.rs` — fn `draw_list()` (ligne 95)
- **Contraintes** : Fonction privée, vérifier tous les call sites avec Grep
- **Fichiers adjacents** : `src/tui/ui_list.rs` (appels internes)
- **Statut** : [ ] pending

### T13 : Extraire render_opaque_background helper [DRY]
- **Fichiers à modifier** :
  - `src/tui/common.rs` — ajouter `pub fn render_opaque_background(f: &mut Frame, area: Rect)`
  - `src/tui/ui_watch.rs` (ligne 18-21), `src/tui/ui_piscine.rs` (17-21), `src/tui/ui_list.rs` (100-104) — remplacer par appel helper
- **Pattern** : `Block::default().style(Style::default().bg(C_BG))`
- **Fichiers adjacents** : `src/tui/common.rs`
- **Statut** : [ ] pending

## Phase E — Remaining Clean Code (optionnel, effort > valeur)

### T14 : render_header() décomposition [CLEAN-6]
- **Note** : 105 lignes mais très linéaire (3 sections L1/L2/L3). Effort de refactor > gain lisibilité. **Skip sauf si explicitement demandé.**
- **Statut** : [ ] skipped — trade-off défavorable

### T15 : spawn_gcc_and_collect() décomposition [CLEAN-4]
- **Note** : 99 lignes mais pipeline séquentiel (compile → spawn → collect → timeout). Découpage forcerait passage de nombreux paramètres. **Skip sauf si explicitement demandé.**
- **Statut** : [ ] skipped — trade-off défavorable

### T16 : order_by_chapters() décomposition [CLEAN-7]
- **Note** : Déjà refactoré avec map + buckets + sort. 56 lignes effectives. Bien structuré. **Skip.**
- **Statut** : [ ] skipped — déjà propre

### T17 : record_attempt() décomposition [CLEAN-5]
- **Note** : 74 lignes, transaction atomique. Découper casserait la cohérence transactionnelle. **Skip.**
- **Statut** : [ ] skipped — transaction atomique

## Phase F — Low findings (batch)

### T18 : Batch de low findings actionnables
- Hint counter duplication watch/piscine → helper dans common.rs
- get_streak() déjà traité par T6
- **Statut** : [ ] pending — faible priorité

---

## Résumé exécution

| Phase | Tâches | Fichiers | Priorité |
|-------|--------|----------|----------|
| A — Sécurité + Quick Wins | T1-T4 | 4 | Haute |
| B — Performance | T5-T6 | 2 | Haute |
| C — Clean Code app.rs | T7-T9 | 1 | Moyenne |
| D — Clean Code backend | T10-T13 | 5 | Moyenne |
| E — Optionnel | T14-T17 | — | Skipped |
| F — Low batch | T18 | 2 | Basse |

**Total tâches actives** : 13 (T1-T13 + T18)
**Fichiers modifiés** : ~10 fichiers
**Tests à vérifier** : `cargo test` + `cargo clippy -- -D warnings` après chaque phase
