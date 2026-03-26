#!/usr/bin/env python3
"""
audit_compile.py — Vérifie que starter_code et chaque stage S0-S4
compilent pour les sujets du groupe spécifié.
Usage: python3 audit_compile.py --subjects a|b

Produit:
  --subjects a → /tmp/clings-audit-compile-a.md
  --subjects b → /tmp/clings-audit-compile-b.md
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

EXERCISES_DIR = Path("/home/trollheap/Developer/TOOLS/clings/exercises")

SUBJECTS_A = [
    "structs", "pointers", "string_formatting", "bitwise_ops",
    "memory_allocation", "errno", "file_io", "fd_basics",
    "filesystem", "scheduling", "processes",
]

SUBJECTS_B = [
    "signals", "pipes", "message_queues", "shared_memory",
    "semaphores", "sync_concepts", "pthreads", "sockets",
    "proc_memory", "virtual_memory", "capstones",
]

LINKER_FLAGS = {
    "pthreads": ["-lpthread"],
    "semaphores": ["-lpthread"],
    "sync_concepts": ["-lpthread"],
    "sockets": ["-lpthread"],
    "capstones": ["-lpthread"],
    "message_queues": ["-lrt", "-lpthread"],
    "shared_memory": ["-lrt", "-lpthread"],
    "file_io": ["-lrt"],
}

GCC_BASE = [
    "gcc", "-Wall", "-Wextra", "-std=c11", "-D_GNU_SOURCE",
    "-Wno-unused-variable", "-Wno-unused-parameter",
    "-Wno-unused-function", "-Wno-implicit-function-declaration",
]

def compile_code(code, subject, extra_files=None, label=""):
    """Compile C code in a tempdir. Returns (ok: bool, error_msg: str)."""
    with tempfile.TemporaryDirectory() as tmpdir:
        src = os.path.join(tmpdir, "code.c")
        out = os.path.join(tmpdir, "a.out")
        with open(src, "w") as f:
            f.write(code)

        # Write auxiliary files if present
        if extra_files:
            for ef in extra_files:
                name = ef.get("name", "")
                content = ef.get("content", "")
                if name and ".." not in name and not name.startswith("/"):
                    fpath = os.path.join(tmpdir, name)
                    os.makedirs(os.path.dirname(fpath), exist_ok=True)
                    with open(fpath, "w") as f:
                        f.write(content)

        cmd = GCC_BASE + ["-o", out, src, f"-I{tmpdir}"] + LINKER_FLAGS.get(subject, [])
        try:
            result = subprocess.run(
                cmd, capture_output=True, text=True, timeout=30
            )
            if result.returncode == 0:
                return True, ""
            # Extract first meaningful error line
            stderr = result.stderr.strip()
            first_error = next(
                (l for l in stderr.splitlines() if "error:" in l),
                stderr.splitlines()[0] if stderr else "erreur inconnue"
            )
            return False, first_error
        except subprocess.TimeoutExpired:
            return False, "Timeout gcc (>30s)"
        except Exception as e:
            return False, str(e)

def audit_subjects(subjects, report_path):
    findings = []
    ok_count = 0
    skip_count = 0

    for subject in subjects:
        subj_dir = EXERCISES_DIR / subject
        if not subj_dir.is_dir():
            print(f"  [SKIP] Sujet non trouvé : {subject}")
            continue

        json_files = sorted(subj_dir.rglob("*.json"))
        print(f"  {subject}: {len(json_files)} exercice(s)")

        for jf in json_files:
            if jf.name in ("annales_map.json",):
                continue
            try:
                ex = json.loads(jf.read_text())
            except json.JSONDecodeError:
                findings.append(("ERROR", jf.stem, "json", "JSON invalide"))
                continue

            ex_id = ex.get("id", jf.stem)
            mode = ex.get("validation", {}).get("mode", "output")

            # On ne teste que les exercices output/both (solution_code a un main())
            if mode == "test":
                skip_count += 1
                continue

            extra_files = ex.get("files", [])
            starter = ex.get("starter_code", "")
            stages = ex.get("starter_code_stages", [])

            # Test starter_code
            if starter.strip():
                ok, err = compile_code(starter, subject, extra_files, f"{ex_id}:starter")
                if ok:
                    ok_count += 1
                else:
                    findings.append(("ERROR", ex_id, "starter_code", err))
            else:
                findings.append(("WARNING", ex_id, "starter_code", "starter_code vide"))

            # Test chaque stage S0-S4
            for i, stage_code in enumerate(stages):
                if not stage_code or not stage_code.strip():
                    findings.append(("WARNING", ex_id, f"stage_S{i}", f"Stage S{i} vide"))
                    continue
                ok, err = compile_code(stage_code, subject, extra_files, f"{ex_id}:S{i}")
                if ok:
                    ok_count += 1
                else:
                    findings.append(("ERROR", ex_id, f"stage_S{i}", err))

    errors   = [(s,i,a,m) for s,i,a,m in findings if s=="ERROR"]
    warnings = [(s,i,a,m) for s,i,a,m in findings if s=="WARNING"]

    lines = []
    lines.append(f"# Rapport Audit — Compilation ({report_path.stem})\n")
    lines.append(f"**Sujets analysés** : {', '.join(subjects)}\n")
    lines.append(f"**Compilations OK** : {ok_count}")
    lines.append(f"**Ignorés (mode test)** : {skip_count}\n")
    lines.append("| Sévérité | Nombre |")
    lines.append("|----------|--------|")
    lines.append(f"| ERROR    | {len(errors)} |")
    lines.append(f"| WARNING  | {len(warnings)} |\n")

    lines.append("## ERRORs de compilation\n")
    if errors:
        for _,ex_id,where,msg in sorted(errors, key=lambda x: x[1]):
            lines.append(f"- **[{ex_id}]** `{where}` — {msg}")
    else:
        lines.append("Aucune erreur de compilation.")

    lines.append("\n## WARNINGs\n")
    for _,ex_id,where,msg in sorted(warnings, key=lambda x: x[1]):
        lines.append(f"- **[{ex_id}]** `{where}` — {msg}")

    report_path.write_text("\n".join(lines))
    print(f"Rapport écrit : {report_path}")
    return len(errors) == 0

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--subjects", choices=["a", "b"], required=True)
    args = parser.parse_args()

    if args.subjects == "a":
        subjects = SUBJECTS_A
        report = Path("/tmp/clings-audit-compile-a.md")
    else:
        subjects = SUBJECTS_B
        report = Path("/tmp/clings-audit-compile-b.md")

    print(f"Compilation audit groupe {args.subjects.upper()} ({len(subjects)} sujets)...")
    ok = audit_subjects(subjects, report)
    return 0 if ok else 1

if __name__ == "__main__":
    sys.exit(main())
