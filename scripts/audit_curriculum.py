#!/usr/bin/env python3
"""
audit_curriculum.py — Vérifie la cohérence curriculum et SRS des 291 exercices.
Produit /tmp/clings-audit-curriculum.md
"""

import json
import sys
from pathlib import Path
from collections import defaultdict

EXERCISES_DIR = Path("/home/trollheap/Developer/TOOLS/clings/exercises")
REPORT_PATH = Path("/tmp/clings-audit-curriculum.md")

# Ordre des chapitres NSY103 (depuis chapters.rs)
CHAPTER_ORDER = {
    "structs": 1, "pointers": 1,
    "string_formatting": 2, "bitwise_ops": 2,
    "memory_allocation": 3, "errno": 3,
    "file_io": 4, "fd_basics": 4,
    "filesystem": 5,
    "scheduling": 6,
    "processes": 7,
    "signals": 8,
    "pipes": 9,
    "message_queues": 10,
    "shared_memory": 11,
    "semaphores": 12,
    "pthreads": 13, "sync_concepts": 13,
    "sockets": 14,
    "proc_memory": 15, "virtual_memory": 15,
    "capstones": 16,
}

findings = []  # (severity, subject, exercise_id_or_subject, message)

def find(severity, subject, ex_id, msg):
    findings.append((severity, subject, ex_id, msg))

def load_all():
    by_subject = defaultdict(list)
    all_ids = {}
    for f in sorted(EXERCISES_DIR.rglob("*.json")):
        if f.name in ("annales_map.json", "kc_error_map.json"):
            continue
        try:
            ex = json.loads(f.read_text())
            ex_id = ex.get("id", f.stem)
            subject = ex.get("subject", f.parent.name)
            all_ids[ex_id] = ex
            by_subject[subject].append(ex)
        except:
            pass
    return by_subject, all_ids

def check_difficulty_distribution(by_subject):
    """Chaque sujet doit couvrir ≥3 niveaux de difficulté (1–5)."""
    for subject, exercises in sorted(by_subject.items()):
        diffs = set(ex.get("difficulty", 0) for ex in exercises)
        diffs.discard(0)
        if len(diffs) < 3:
            find("WARNING", subject, subject,
                 f"Distribution difficulté limitée : {sorted(diffs)} ({len(diffs)} niveaux, min 3)")
        # Vérifier que D1 et D2 existent (exercices d'entrée)
        if 1 not in diffs:
            find("WARNING", subject, subject, "Pas d'exercice D1 (débutant)")
        if max(diffs, default=0) < 3:
            find("INFO", subject, subject,
                 f"Niveau max = D{max(diffs, default=0)}, pas d'exercice avancé")

def check_prerequisites(by_subject, all_ids):
    """Tous les prérequis référencés doivent exister."""
    for subject, exercises in sorted(by_subject.items()):
        for ex in exercises:
            ex_id = ex.get("id", "?")
            for prereq in ex.get("prerequisites", []):
                if prereq not in all_ids:
                    find("ERROR", subject, ex_id,
                         f"Prérequis inexistant : '{prereq}'")
                else:
                    # Vérifier cohérence : le prérequis doit être de difficulté ≤
                    prereq_ex = all_ids[prereq]
                    if prereq_ex.get("difficulty", 0) > ex.get("difficulty", 0):
                        find("WARNING", subject, ex_id,
                             f"Prérequis '{prereq}' (D{prereq_ex.get('difficulty')}) "
                             f"plus difficile que l'exercice lui-même (D{ex.get('difficulty')})")

def check_id_coherence(by_subject):
    """Dans un sujet, les IDs doivent être cohérents avec les difficultés."""
    for subject, exercises in sorted(by_subject.items()):
        # Trier par ID alphabétique
        sorted_ex = sorted(exercises, key=lambda e: e.get("id", ""))
        difficulties = [ex.get("difficulty", 0) for ex in sorted_ex]
        ids = [ex.get("id", "?") for ex in sorted_ex]

        # Détecter des sauts de difficulté bizarres (> 2 crans en arrière)
        for i in range(1, len(difficulties)):
            if difficulties[i-1] > 0 and difficulties[i] > 0:
                drop = difficulties[i-1] - difficulties[i]
                if drop > 2:
                    find("INFO", subject, ids[i],
                         f"Chute de difficulté : {ids[i-1]} (D{difficulties[i-1]}) → {ids[i]} (D{difficulties[i]})")

def check_chapter_alignment(by_subject):
    """Tous les sujets dans exercises/ doivent être dans CHAPTER_ORDER."""
    existing = set(by_subject.keys())
    for subj in sorted(existing):
        if subj not in CHAPTER_ORDER:
            find("WARNING", subj, subj,
                 f"Sujet '{subj}' non référencé dans CHAPTER_ORDER (chapters.rs)")
    for subj in sorted(CHAPTER_ORDER.keys()):
        if subj not in existing:
            find("WARNING", subj, subj,
                 f"Sujet '{subj}' dans chapters.rs mais absent de exercises/")

def check_exercise_count(by_subject):
    """Chaque sujet devrait avoir ≥ 5 exercices."""
    for subject, exercises in sorted(by_subject.items()):
        if len(exercises) < 5:
            find("INFO", subject, subject,
                 f"Peu d'exercices : {len(exercises)} (recommandé ≥ 5)")

def check_difficulty_per_exercise(by_subject):
    """Vérifier que difficulty est dans [1, 5]."""
    for subject, exercises in by_subject.items():
        for ex in exercises:
            d = ex.get("difficulty", -1)
            ex_id = ex.get("id", "?")
            if not isinstance(d, int) or d < 1 or d > 5:
                find("ERROR", subject, ex_id,
                     f"Difficulté invalide : {d} (attendu 1–5)")

def main():
    by_subject, all_ids = load_all()
    total_ex = sum(len(v) for v in by_subject.values())
    print(f"Chargé : {total_ex} exercices, {len(by_subject)} sujets")

    check_difficulty_distribution(by_subject)
    check_prerequisites(by_subject, all_ids)
    check_id_coherence(by_subject)
    check_chapter_alignment(by_subject)
    check_exercise_count(by_subject)
    check_difficulty_per_exercise(by_subject)

    errors   = [(s,subj,i,m) for s,subj,i,m in findings if s=="ERROR"]
    warnings = [(s,subj,i,m) for s,subj,i,m in findings if s=="WARNING"]
    infos    = [(s,subj,i,m) for s,subj,i,m in findings if s=="INFO"]

    # Stats par sujet
    by_subj_stats = defaultdict(lambda: {"E":0,"W":0,"I":0})
    for sev, subj, _, _ in findings:
        by_subj_stats[subj][{"ERROR":"E","WARNING":"W","INFO":"I"}[sev]] += 1

    lines = []
    lines.append("# Rapport Audit — Curriculum & Cohérence SRS\n")
    lines.append(f"**Exercices analysés** : {total_ex} | **Sujets** : {len(by_subject)}\n")
    lines.append("| Sévérité | Nombre |")
    lines.append("|----------|--------|")
    lines.append(f"| ERROR    | {len(errors)} |")
    lines.append(f"| WARNING  | {len(warnings)} |")
    lines.append(f"| INFO     | {len(infos)} |\n")

    lines.append("## ERRORs\n")
    for _,subj,i,m in sorted(errors, key=lambda x: (x[1],x[2])):
        lines.append(f"- **[{i}]** `{subj}` — {m}")

    lines.append("\n## WARNINGs\n")
    for _,subj,i,m in sorted(warnings, key=lambda x: (x[1],x[2])):
        lines.append(f"- **[{i}]** `{subj}` — {m}")

    lines.append("\n## INFOs\n")
    for _,subj,i,m in sorted(infos, key=lambda x: (x[1],x[2])):
        lines.append(f"- **[{i}]** `{subj}` — {m}")

    lines.append("\n## Répartition par sujet\n")
    lines.append("| Sujet | Ch. | Exercices | ERR | WARN | INFO |")
    lines.append("|-------|-----|-----------|-----|------|------|")
    for subj in sorted(by_subject.keys()):
        ch = CHAPTER_ORDER.get(subj, "?")
        n = len(by_subject[subj])
        stats = by_subj_stats.get(subj, {"E":0,"W":0,"I":0})
        lines.append(f"| {subj} | {ch} | {n} | {stats['E']} | {stats['W']} | {stats['I']} |")

    REPORT_PATH.write_text("\n".join(lines))
    print(f"Rapport écrit : {REPORT_PATH}")
    return 0 if not errors else 1

if __name__ == "__main__":
    sys.exit(main())
