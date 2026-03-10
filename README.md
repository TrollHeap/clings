# clings

**Entraîneur interactif de programmation systèmes C, aligné sur le cursus NSY103 — Linux : noyau et programmation système.**

Résolvez 260 exercices C directement dans votre éditeur. `clings` surveille vos sauvegardes, compile avec `gcc`, valide la sortie, et mesure votre maîtrise via un algorithme de répétition espacée (SRS).

---

## Installation

**Prérequis :** Rust (toolchain stable), `gcc` installé sur le système.

```bash
# Depuis les sources
git clone <repo>
cd clings
cargo build --release
# Binaire : target/release/clings

# Ou directement dans ~/.cargo/bin/
cargo install --path .
```

---

## Commandes

| Commande | Description |
|---|---|
| `clings` ou `clings watch` | Mode surveillance SRS : exercices priorisés par niveau de maîtrise, avancement automatique |
| `clings list` | Liste tous les exercices (filtre possible : `--subject <sujet>`) |
| `clings run <id>` | Lance un exercice précis en mode surveillance (ex : `clings run ptr-deref-01`) |
| `clings progress` | Tableau de bord : maîtrise par sujet, série de jours consécutifs |
| `clings hint <id>` | Affiche les indices de l'exercice |
| `clings solution <id>` | Affiche la solution (nécessite au moins une tentative) |
| `clings reset` | Réinitialise toute la progression (confirmation requise) |
| `clings piscine` | Mode piscine : parcours linéaire intégral, tous les exercices déverrouillés d'emblée |

### Raccourcis clavier (mode watch)

| Touche | Action |
|---|---|
| `h` | Afficher un indice |
| `n` | Passer à l'exercice suivant |
| `c` | Compiler et vérifier maintenant |
| `l` | Afficher la liste des exercices |
| `q` | Quitter |

---

## Fonctionnement

1. `clings watch` sélectionne l'exercice suivant selon l'algorithme SRS (maîtrise la plus faible en priorité).
2. Le code de départ est écrit dans `~/.clings/current.c`.
3. Ouvrez ce fichier dans votre éditeur et modifiez-le.
4. À chaque sauvegarde, `clings` compile avec `gcc -Wall -Wextra -std=c11` et compare la sortie à la valeur attendue.
5. En cas de succès, la maîtrise du sujet augmente (+1,0) et l'exercice suivant est chargé.
6. En cas d'échec d'exécution (pas d'erreur de compilation), la maîtrise diminue (−0,5).

### Algorithme SRS

- Score de maîtrise : 0,0 à 5,0 par sujet
- Difficulté D2 déverrouillée à 2,0 — D3 à 4,0
- Décroissance de 14 jours en cas d'inactivité
- Intervalles de révision multipliés par 2,5

### Contenu

- **260+ exercices** répartis sur **21 sujets**
- **15 chapitres NSY103** : Fondamentaux C → Chaînes & bits → Allocation mémoire → E/S → Système de fichiers → Processus → Signaux → Tubes → Sockets → Mémoire partagée → Sémaphores → Threads POSIX → Mémoire virtuelle → Projets intégrateurs
- Niveaux de difficulté D1 à D5
- Code de départ adaptatif par stades (S0–S4) selon la maîtrise

---

## Curricula couverts

### NSY103 — Linux : noyau et programmation système (Cnam)

Curriculum principal. Couvre les 16 chapitres du cours NSY103 : processus, signaux, tubes, IPC POSIX (files de messages, mémoire partagée, sémaphores), threads POSIX, sockets, système de fichiers, mémoire virtuelle.

### UTC502 — Gestion des ressources informatiques

Exercices supplémentaires alignés sur UTC502 : algorithmes de remplacement de pages, politiques d'ordonnancement CPU, mémoire virtuelle avancée.

| Sujet | Curriculum | Chapitre |
|-------|-----------|---------|
| `processes`, `signals`, `pipes`, `file_io` | NSY103 | 7–10 |
| `pthreads`, `semaphores`, `shared_memory`, `message_queues` | NSY103 | 11–13 |
| `sockets` | NSY103 | 14 |
| `virtual_memory` (mmap, cow, tlb, brk…) | NSY103 + UTC502 | 15 |
| `virtual_memory` (page_replacement_fifo/lru/opt) | UTC502 | 15 |
| `scheduling` (fifo, rr, sjf, priority) | UTC502 | 6 |

---

## Configuration

| Paramètre | Valeur par défaut | Description |
|---|---|---|
| Répertoire de travail | `~/.clings/` | Fichier courant, base de données SQLite |
| `CLINGS_EXERCISES` | _(non défini)_ | Chemin alternatif vers le dossier `exercises/` |
| Intégration tmux | automatique | Si `clings` tourne dans tmux, ouvre neovim dans un split |

La base de données de progression se trouve dans `~/.clings/progress.db`.

---

## Contribuer

Voir [CONTRIBUTING.md](CONTRIBUTING.md).

---

## Licence

[TODO: choisir licence]
# clings
