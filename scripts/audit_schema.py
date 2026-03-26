#!/usr/bin/env python3
"""
audit_schema.py — Vérifie le schéma, la conformité et la qualité pédagogique
des 291 exercices clings. Produit /tmp/clings-audit-schema.md
"""

import json
import sys
from pathlib import Path
from collections import defaultdict

EXERCISES_DIR = Path("/home/trollheap/Developer/TOOLS/clings/exercises")
REPORT_PATH = Path("/tmp/clings-audit-schema.md")

PLACEHOLDERS = [
    "__TITLE__", "__DESCRIPTION__", "__STARTER_CODE__",
    "__SOLUTION_CODE__", "__EXPECTED_OUTPUT__", "__HINT__",
    "__KEY_CONCEPT__", "__COMMON_MISTAKE__", "TODO_FILL",
]

VALID_MODES = {"output", "test", "both"}
VALID_EXERCISE_TYPES = {"complete", "fix_bug", "fill_blank", "refactor"}

findings = []  # list of (severity, exercise_id, axis, message)

def find(severity, ex_id, axis, msg):
    findings.append((severity, ex_id, axis, msg))

def load_all():
    exercises = {}
    for f in sorted(EXERCISES_DIR.rglob("*.json")):
        if f.name in ("annales_map.json", "kc_error_map.json"):
            continue
        try:
            ex = json.loads(f.read_text())
            ex_id = ex.get("id", f.stem)
            exercises[ex_id] = ex
        except json.JSONDecodeError as e:
            findings.append(("ERROR", f.stem, "json", f"JSON invalide : {e}"))
    return exercises

def check(ex_id, ex, all_ids):
    mode = ex.get("validation", {}).get("mode", "output")

    # --- ERRORS : champs requis ---
    for field in ["id", "title", "description", "starter_code", "solution_code"]:
        v = ex.get(field, "")
        if not v or not str(v).strip():
            find("ERROR", ex_id, "schema", f"Champ requis absent/vide : {field}")

    hints = ex.get("hints", [])
    if not hints:
        find("ERROR", ex_id, "schema", "hints absent ou vide")

    val = ex.get("validation", {})
    if mode in ("output", "both"):
        eo = val.get("expected_output")
        if not eo:
            find("ERROR", ex_id, "schema", "validation.expected_output absent pour mode output/both")

    if mode not in VALID_MODES:
        find("ERROR", ex_id, "schema", f"validation.mode invalide : '{mode}'")

    # --- ERRORS : placeholders ---
    for field in ["title", "description", "starter_code", "solution_code", "key_concept", "common_mistake"]:
        v = str(ex.get(field, ""))
        for ph in PLACEHOLDERS:
            if ph in v:
                find("ERROR", ex_id, "placeholder", f"Placeholder '{ph}' dans '{field}'")

    eo_str = str(val.get("expected_output", ""))
    for ph in PLACEHOLDERS:
        if ph in eo_str:
            find("ERROR", ex_id, "placeholder", f"Placeholder '{ph}' dans validation.expected_output")

    # --- ERRORS : prérequis inexistants ---
    for prereq in ex.get("prerequisites", []):
        if prereq not in all_ids:
            find("ERROR", ex_id, "schema", f"Prérequis inexistant : '{prereq}'")

    # --- WARNINGS : qualité pédagogique ---
    desc = ex.get("description", "")
    if len(desc) < 100:
        find("WARNING", ex_id, "pedagogique", f"description trop courte ({len(desc)} chars, min 100)")

    if len(hints) < 2:
        find("WARNING", ex_id, "pedagogique", f"Moins de 2 hints ({len(hints)})")

    for i, h in enumerate(hints):
        if isinstance(h, str) and len(h) > 250:
            find("WARNING", ex_id, "pedagogique", f"Hint {i} trop long ({len(h)} chars > 250)")

    kc = str(ex.get("key_concept", "")).strip()
    if not kc:
        find("WARNING", ex_id, "pedagogique", "key_concept absent ou vide")
    elif len(kc) < 10:
        find("WARNING", ex_id, "pedagogique", f"key_concept trop court/générique : '{kc}'")

    cm = str(ex.get("common_mistake", "")).strip()
    if not cm:
        find("WARNING", ex_id, "pedagogique", "common_mistake absent ou vide")

    kc_ids = ex.get("kc_ids", [])
    if not kc_ids:
        find("WARNING", ex_id, "pedagogique", "kc_ids vide (pas de mapping KC)")

    stages = ex.get("starter_code_stages", [])
    if mode in ("output", "both"):
        if not stages:
            find("WARNING", ex_id, "schema", "starter_code_stages absent (mode output)")
        elif len(stages) != 5:
            find("WARNING", ex_id, "schema", f"starter_code_stages : {len(stages)} entrées (attendu 5)")

    starter = str(ex.get("starter_code", "")).strip()
    solution = str(ex.get("solution_code", "")).strip()
    if starter and solution and starter == solution:
        find("WARNING", ex_id, "pedagogique", "starter_code identique à solution_code")

    viz = ex.get("visualizer", {})
    if not viz or not viz.get("type"):
        find("WARNING", ex_id, "contenu", "visualizer absent")

    ex_type = ex.get("exercise_type", "")
    if ex_type not in VALID_EXERCISE_TYPES:
        find("INFO", ex_id, "schema", f"exercise_type non renseigné ou invalide : '{ex_type}'")

def main():
    all_ex = load_all()
    all_ids = set(all_ex.keys())
    print(f"Chargé : {len(all_ex)} exercices")

    for ex_id, ex in sorted(all_ex.items()):
        check(ex_id, ex, all_ids)

    errors   = [(s,i,a,m) for s,i,a,m in findings if s == "ERROR"]
    warnings = [(s,i,a,m) for s,i,a,m in findings if s == "WARNING"]
    infos    = [(s,i,a,m) for s,i,a,m in findings if s == "INFO"]

    # Group by subject (first part of ID before '_' or '-')
    by_subject = defaultdict(list)
    for s,i,a,m in findings:
        subject = i.rsplit("_", 1)[0] if "_" in i else i.split("-")[0]
        by_subject[subject].append((s,i,a,m))

    lines = []
    lines.append("# Rapport Audit — Schéma & Qualité Pédagogique\n")
    lines.append(f"**Exercices analysés** : {len(all_ex)}\n")
    lines.append(f"**Total findings** : {len(findings)}\n")
    lines.append("| Sévérité | Nombre |")
    lines.append("|----------|--------|")
    lines.append(f"| ERROR    | {len(errors)} |")
    lines.append(f"| WARNING  | {len(warnings)} |")
    lines.append(f"| INFO     | {len(infos)} |\n")

    lines.append("## ERRORs\n")
    if errors:
        for _,i,a,m in sorted(errors, key=lambda x: x[1]):
            lines.append(f"- **[{i}]** `{a}` — {m}")
    else:
        lines.append("Aucune erreur.")

    lines.append("\n## WARNINGs\n")
    if warnings:
        for _,i,a,m in sorted(warnings, key=lambda x: (x[1],x[3])):
            lines.append(f"- **[{i}]** `{a}` — {m}")
    else:
        lines.append("Aucun warning.")

    lines.append("\n## INFOs\n")
    for _,i,a,m in sorted(infos, key=lambda x: x[1]):
        lines.append(f"- **[{i}]** `{a}` — {m}")

    lines.append("\n## Répartition par sujet\n")
    lines.append("| Sujet | ERR | WARN | INFO |")
    lines.append("|-------|-----|------|------|")
    for subj in sorted(by_subject.keys()):
        fs = by_subject[subj]
        e = sum(1 for s,_,_,_ in fs if s=="ERROR")
        w = sum(1 for s,_,_,_ in fs if s=="WARNING")
        inf = sum(1 for s,_,_,_ in fs if s=="INFO")
        if e+w+inf > 0:
            lines.append(f"| {subj} | {e} | {w} | {inf} |")

    REPORT_PATH.write_text("\n".join(lines))
    print(f"Rapport écrit : {REPORT_PATH}")
    return 0 if not errors else 1

if __name__ == "__main__":
    sys.exit(main())
