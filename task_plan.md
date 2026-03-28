# task_plan.md — Mode NSY103 Learning + Mode Examen séparé

## Contexte

Le mode NSY103 actuel (`LaunchMode::Nsy103`) lance directement la sélection d'annales
avec timer — ce n'est pas un mode d'apprentissage. L'utilisateur veut :

1. **Mode NSY103** = mode watch filtré sur les sujets NSY103-core (C basics → système)
2. **Mode Examen NSY103** = l'actuel Nsy103 (annales + timer), séparé et explicite

Sujets **exclus** du mode NSY103 Learning (spécifiques UTC502) : `scheduling`, `virtual_memory`

Résultat : menu launcher à 4 options — Watch | Piscine | NSY103 | Examen NSY103

---

## Fichiers à modifier

| Fichier                  | Rôle                                    |
| ------------------------ | --------------------------------------- |
| `src/tui/ui_launcher.rs` | Enum LaunchMode, menu, sélection, rendu |
| `src/commands/watch.rs`  | Ajouter param `nsy103_only: bool`       |
| `src/main.rs`            | Dispatcher LaunchChoice → commandes     |

---

## T1 : `src/commands/watch.rs` — paramètre `nsy103_only`

**Signature actuelle** : `pub fn cmd_watch(filter_chapter: Option<u8>) -> Result<()>`

**Nouvelle signature** : `pub fn cmd_watch(filter_chapter: Option<u8>, nsy103_only: bool) -> Result<()>`

Ajouter après le chargement des exercices, avant `order_by_chapters` :

```rust
const NSY103_EXCLUDED: &[&str] = &["scheduling", "virtual_memory"];

let filtered: Vec<Exercise> = if nsy103_only {
    gated_exercises
        .into_iter()
        .filter(|ex| !NSY103_EXCLUDED.contains(&ex.subject.as_str()))
        .collect()
} else {
    gated_exercises
};
// Utiliser `filtered` au lieu de `gated_exercises` pour order_by_chapters
```

**Tous les appels existants** à `cmd_watch` à mettre à jour :

- `src/main.rs` (dispatch Watch subcommand) → `cmd_watch(chapter, false)`
- `src/main.rs` (dispatch NSY103 depuis launcher) → `cmd_watch(None, true)`

---

## T2 : `src/tui/ui_launcher.rs` — 4 modes

### T2a : Enum LaunchMode

```rust
// AVANT
pub enum LaunchMode { Watch, Piscine, Nsy103 }

// APRÈS
pub enum LaunchMode { Watch, Piscine, Nsy103, ExamNsy103 }
```

### T2b : Screen::Chapter — NSY103 ne passe PAS par Chapter screen

Comportement :

- Watch → `Screen::Chapter(LaunchMode::Watch)` (inchangé)
- Piscine → `Screen::Chapter(LaunchMode::Piscine)` (inchangé)
- NSY103 → `LaunchChoice::Start { mode: LaunchMode::Nsy103, chapter: None }` (direct, comme l'actuel Nsy103)
- ExamNsy103 → `LaunchChoice::Start { mode: LaunchMode::ExamNsy103, chapter: None }` (direct)

### T2c : Logique de sélection (cursor → mode)

Chercher la fonction de sélection dans `ui_launcher.rs` (autour ligne 79-100).
Modifier le `match cursor_offset` pour 4 items (Watch=0, Piscine=1, NSY103=2, ExamNsy103=3).

### T2d : Draw functions

- `draw_mode_screen()` : 4 items dans la liste :
  - `"Watch"` + desc `"SRS adaptatif — tout le curriculum"`
  - `"Piscine"` + desc `"Linéaire — tout débloqué"`
  - `"NSY103"` + desc `"Apprentissage C système (ch. 1–14 + proc_memory)"`
  - `"Examen NSY103"` + desc `"Simule une annale avec timer"`

- `draw_chapter_screen()` : modifier le titre selon le mode
  - `LaunchMode::Nsy103 => "NSY103 — Sélectionner un chapitre"` (si on ajoute le chapitre au NSY103 mode, sinon pas besoin)

- Match `LaunchMode::ExamNsy103` dans `draw_mode_screen` (derive Display si nécessaire)

---

## T3 : `src/main.rs` — Dispatcher

Chercher le dispatch de `LaunchChoice::Start { mode, chapter }` dans `main.rs`.

```rust
// AVANT
LaunchMode::Watch   => cmd_watch(chapter),
LaunchMode::Piscine => cmd_piscine(chapter, None),
LaunchMode::Nsy103  => exam::cmd_exam(None, false),

// APRÈS
LaunchMode::Watch      => cmd_watch(chapter, false),
LaunchMode::Piscine    => cmd_piscine(chapter, None),
LaunchMode::Nsy103     => cmd_watch(None, true),
LaunchMode::ExamNsy103 => exam::cmd_exam(None, false),
```

---

## Vérification

```bash
# Build
cc-run build cargo build

# Tests (non-régression)
cc-run tests cargo test

# Lint
cc-run lint "cargo clippy -- -D warnings"

# Manuel 1 : Mode Watch
cargo run -- watch
# → Launcher affiche 4 modes, choisir Watch → chapter screen → exercices SRS normal

# Manuel 2 : Mode NSY103
cargo run -- watch
# → Choisir NSY103 → démarre watch immédiatement → aucun exercice scheduling/ virtual_memory

# Manuel 3 : Mode Examen NSY103
cargo run -- watch
# → Choisir "Examen NSY103" → sélecteur d'annales avec timer (comportement actuel)

# Manuel 4 : Mode Piscine inchangé
cargo run -- watch
# → Choisir Piscine → chapter screen → exercices linéaires comme avant

# CLI direct inchangé
cargo run -- exam           # Sélecteur annales
cargo run -- piscine        # Piscine normale
```

### Edge cases

- NSY103 mode : vérifier que `scheduling/` et `virtual_memory/` n'apparaissent pas dans la liste
- ExamNsy103 mode : vérifier que l'annale NSY103-s1-2022-2023 se lance avec timer 150min
- Watch mode : vérifier que scheduling + virtual_memory RESTENT présents (nsy103_only=false)
