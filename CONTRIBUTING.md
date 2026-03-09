# Contribuer à KernelForge CLI

## Prérequis

- Rust (toolchain stable, `rustup` recommandé)
- `gcc` installé (`gcc --version` doit fonctionner)
- `sqlite3` (optionnel, pour inspecter la base de données)

## Build & test

```bash
cargo build                    # Compilation debug (binaire : target/debug/kf)
cargo build --release          # Compilation optimisée
cargo clippy -- -D warnings    # Lint — doit passer sans erreur
cargo test                     # Tests unitaires
cargo test <nom_du_test>       # Un test précis
```

Toute contribution doit passer `cargo clippy -- -D warnings` et `cargo test` sans régression.

## Format des exercices

Les exercices sont des fichiers JSON dans `exercises/<sujet>/`. Chaque fichier définit :

```jsonc
{
  "id": "ptr-deref-01",
  "subject": "pointers",
  "lang": "c",
  "difficulty": 1,
  "title": "...",
  "description": "...",
  "starter_code": "...",
  "solution_code": "...",
  "hints": ["...", "..."],
  "validation": { "mode": "output", "expected_output": "..." }
}
```

Le champ `starter_code_stages` (optionnel) permet un échafaudage adaptatif S0–S4.
Nommage des fichiers : `<sujet>_<num>.json` (ex : `ptr_deref_01.json`).

## Convention de commit

```
<type>(<périmètre>): <description courte en impératif>

# Types : feat | fix | refactor | test | docs | chore
# Exemples :
feat(exercises): add vm_mmap_01 virtual memory exercise
fix(runner): correct linker flags for semaphore exercises
docs(readme): update installation instructions
```

Un commit par changement logique. Pas de `git push --force` sur `main`.
