# Mémoire Virtuelle — Référence rapide

## API mémoire virtuelle Linux

| Appel | Signature | Rôle |
|-------|-----------|------|
| `mmap()` | `mmap(addr, len, prot, flags, fd, offset)` | Mappe fichier ou zone anonyme dans l'espace d'adressage |
| `munmap()` | `munmap(addr, len)` | Libère un mapping ; décrémente ref_count des pages |
| `mprotect()` | `mprotect(addr, len, prot)` | Change les permissions (PROT_READ/WRITE/EXEC) d'une zone |
| `madvise()` | `madvise(addr, len, advice)` | Indique au noyau le pattern d'accès (MADV_SEQUENTIAL…) |
| `msync()` | `msync(addr, len, flags)` | Synchronise un mapping fichier avec le disque |
| `brk()/sbrk()` | `sbrk(increment)` | Étend le segment heap (utilisé par malloc en interne) |

## Structure d'un espace d'adressage (x86-64 Linux)

```
0xFFFFFFFFFFFFFFFF  --- noyau (non accessible en user space)
0x7FFFFFFFFFFFFFFF
                   --- stack (croit vers le bas)
                       [variables locales, cadres d'appel]
                   --- mmap / libs partagees (.so)
                       [ld-linux, libc, autres mappings]
                   --- heap (croit vers le haut via brk)
                       [malloc, new, realloc]
0x0000000000602000 --- bss  (variables globales non initialisees, zeroed)
0x0000000000601000 --- data (variables globales initialisees)
0x0000000000400000 --- text (code executable, lecture seule)
0x0000000000000000 --- NULL (non mappe — segfault garanti)
```

- `text` : read+exec, partage entre processus utilisant le meme binaire
- `data` : read+write, valeurs initiales copiees depuis l'ELF
- `bss`  : read+write, occupe sans espace sur disque (zero-fill on demand)
- `heap` : read+write, gere par l'allocateur (ptmalloc2, jemalloc…)
- `stack`: read+write, limite par defaut 8 Mo (`ulimit -s`)

## Pagination

| Terme | Definition |
|-------|-----------|
| Page | Bloc de memoire virtuelle (taille fixe, typiquement 4 Ko) |
| Frame | Bloc de memoire physique de meme taille |
| Table des pages | Structure noyau VA→PA ; une par processus |
| Present bit | 1 = page en RAM, 0 = absente (declenche page fault) |
| Dirty bit | 1 = page modifiee depuis chargement (a ecrire sur disque) |
| Accessed bit | Mis a 1 par la MMU a chaque acces (utilise par LRU) |

**Traduction d'adresse :**
```
VA  = page_number * PAGE_SIZE + offset
PA  = frame_number * PAGE_SIZE + offset
```

Sur x86-64 : hierarchie a 4 niveaux (PGD → PUD → PMD → PTE) pour adresses 48 bits.

## TLB — Translation Lookaside Buffer

Cache materiel des traductions recentes (typiquement 64–1024 entrees) :

```
TLB hit  (~1 cycle)  : page trouvee → PA direct
TLB miss (~100 cycles): parcourir la table des pages → charger dans TLB
```

- **Politique LRU** : evicte l'entree la moins recemment utilisee
- **TLB flush** : necessaire lors d'un changement de CR3 (context switch) ou `munmap()`
- **ASID** (Address Space ID) : evite les flush complets sur ARM et x86-64 modernes
- Taux de hits typique : 95–99 % grace a la localite temporelle/spatiale

## Page Fault

**Causes principales :**
1. Page absente (demand paging) — premiere fois qu'on accede a une page
2. Page swappee sur disque — liberee par le noyau sous pression memoire
3. Violation de protection — ecriture sur page read-only → SIGSEGV

**Algorithmes de remplacement :**

| Algorithme | Principe | Anomalie Belady |
|------------|----------|-----------------|
| FIFO | Evicte la page chargee en premier | Oui (plus de frames → plus de faults) |
| LRU | Evicte la page accedee le plus anciennement | Non |
| Clock | Approximation LRU avec bit reference | Non |
| OPT | Evicte la page utilisee le plus tard (optimal, irrealisable) | Non |

## mmap() — Mappage de fichiers

```c
// Lire un fichier entier sans read()
int fd = open("data.bin", O_RDONLY);
void *ptr = mmap(NULL, size, PROT_READ, MAP_PRIVATE, fd, 0);
close(fd);  // fd peut etre ferme apres mmap

// Acces direct comme un tableau
int *arr = (int*)ptr;
printf("%d\n", arr[42]);  // lit depuis le page cache

munmap(ptr, size);
```

**Flags importants :**

| Flag | Effet |
|------|-------|
| `MAP_PRIVATE` | Modifications locales (CoW), fichier non modifie |
| `MAP_SHARED` | Modifications visibles par tous les mappings du fichier |
| `MAP_ANONYMOUS` | Zone memoire sans fichier (comme malloc mais alignee page) |
| `MAP_FIXED` | Forcer l'adresse exacte (dangereux) |

## Copy-on-Write (CoW)

Mecanisme cle de fork() et MAP_PRIVATE :

```
fork()
  parent et enfant partagent les memes frames (ref_count++)
  pages marquees read-only dans les deux tables
  premier write → protection fault
    noyau alloue un nouveau frame
    copie la page originale
    remet la page en read+write pour le processus qui ecrit
    ref_count-- sur l'original
```

**Avantages :**
- `fork()` en O(1) — pas de copie memoire immediate
- Economie si l'enfant fait `execve()` immediatement (shell, daemon)
- Base des snapshots dans Redis, PostgreSQL, ZFS

**Cout cache :** chaque page modifiee entraine un page fault + allocation + copie (~microseconde).

## Fragmentation heap

- **Interne** : bloc alloue plus grand que demande (alignement, header)
- **Externe** : nombreux petits blocs libres non contigus
- **Coalescence** : fusionner les blocs libres adjacents (glibc le fait)
- **Compactage** : deplacer les objets pour reunir la memoire libre (GC Java)
