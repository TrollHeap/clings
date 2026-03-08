# Ordonnancement Linux

## API d'ordonnancement

```c
#include <sched.h>

// Changer la politique et la priorité d'un processus
int sched_setscheduler(pid_t pid, int policy, const struct sched_param *param);

// Lire la politique courante
int sched_getscheduler(pid_t pid);

// Lire/écrire les paramètres RT (priorité)
int sched_getparam(pid_t pid, struct sched_param *param);
int sched_setparam(pid_t pid, const struct sched_param *param);

// Ajuster la valeur nice (SCHED_OTHER uniquement)
int nice(int inc);
int setpriority(int which, id_t who, int prio);
int getpriority(int which, id_t who);
// which = PRIO_PROCESS | PRIO_PGRP | PRIO_USER

// Affinité CPU
int sched_setaffinity(pid_t pid, size_t cpusetsize, const cpu_set_t *mask);
int sched_getaffinity(pid_t pid, size_t cpusetsize, cpu_set_t *mask);
// CPU_ZERO(&set); CPU_SET(cpu, &set); CPU_ISSET(cpu, &set);
```

## Politiques d'ordonnancement

| Politique       | Type       | Priorité       | Description                                    |
|-----------------|------------|----------------|------------------------------------------------|
| `SCHED_OTHER`   | Normal     | nice -20..+19  | CFS (Completely Fair Scheduler), défaut        |
| `SCHED_BATCH`   | Normal     | nice -20..+19  | CFS, optimisé batch (pas d'interactivité)      |
| `SCHED_IDLE`    | Normal     | —              | Très basse priorité, CPU idle seulement        |
| `SCHED_FIFO`    | Temps-réel | RT 1..99       | FIFO, sans quantum, préempte SCHED_OTHER       |
| `SCHED_RR`      | Temps-réel | RT 1..99       | Round-Robin, avec quantum (~100ms)             |
| `SCHED_DEADLINE`| Deadline   | deadline params| EDF, garantit des délais (depuis Linux 3.14)   |

**Règle de priorité :** RT (FIFO/RR) > SCHED_OTHER. Au sein des RT, priorité RT plus haute gagne.

## Priorités

### SCHED_OTHER : valeur nice
```
nice : -20  -10   0  +10  +19
poids: 88761 9548 1024 110   15

part_cpu ≈ poids_i / Σ poids_j
```

- `nice()` ajoute un incrément à la valeur courante
- `setpriority(PRIO_PROCESS, pid, val)` fixe la valeur absolue
- Seul root peut passer nice en dessous de 0 (`CAP_SYS_NICE`)

### SCHED_FIFO / SCHED_RR : priorité RT
```c
struct sched_param p = { .sched_priority = 50 };
sched_setscheduler(0, SCHED_FIFO, &p);
// Plage valide : sched_get_priority_min(SCHED_FIFO) .. sched_get_priority_max(SCHED_FIFO)
// En pratique : 1 .. 99
```

## Quantum et context switch

| Événement            | Coût typique  |
|----------------------|---------------|
| Context switch       | ~1–10 µs      |
| Quantum SCHED_RR     | ~100 ms       |
| Quantum CFS min      | ~0.75 ms      |
| Cache miss (L3)      | ~40 ns        |

Un context switch sauvegarde/restaure les registres, TLB flush partiel, pipeline flush.

## CFS : vruntime et poids

Le CFS sélectionne toujours la tâche avec le **vruntime** (virtual runtime) le plus bas :

```
vruntime += delta_exec × (NICE_0_WEIGHT / weight)
NICE_0_WEIGHT = 1024
```

- Tâche lourde (poids élevé) : vruntime progresse lentement → sélectionnée souvent
- Tâche légère (poids faible) : vruntime progresse vite → sélectionnée moins souvent

Les tâches sont stockées dans un **rbtree** trié par vruntime ; l'arbre est O(log n).

## Temps-réel : SCHED_FIFO vs SCHED_RR

```
SCHED_FIFO :                  SCHED_RR :
  prio=50 ──────────────►       prio=50 ──Q──Q──Q──►
  prio=30 (attend)              prio=30 ──Q──Q──Q──►
  (pas de quantum)              (quantum partagé à même prio)
```

- **FIFO** : une tâche s'exécute jusqu'à `sched_yield()`, blocage I/O, ou préemption par RT+
- **RR** : même règle mais avec rotation entre tâches de même priorité RT

```c
// Céder volontairement le CPU (FIFO)
sched_yield();

// Obtenir la taille du quantum RR
struct timespec ts;
sched_rr_get_interval(0, &ts);
```

## Affinité CPU

```c
cpu_set_t set;
CPU_ZERO(&set);
CPU_SET(0, &set);  // lier au CPU 0
CPU_SET(1, &set);  // et CPU 1
sched_setaffinity(getpid(), sizeof(set), &set);

// Masque binaire équivalent : 0x3 = 0b0011 = CPU0 + CPU1
```

**Usages courants :**
- Thread RT isolé sur un CPU dédié (éviter interruptions des autres cœurs)
- Localité de cache : thread de traitement + données sur le même NUMA node
- Benchmark reproductible : fixer le processus de test sur 1 CPU

```c
// Lire le CPU courant d'exécution (sans appel système)
#include <sched.h>
int cpu = sched_getcpu();
```
