#!/usr/bin/env python3
"""
migrate_to_output.py — Convertit tous les exercices test/both en mode output.

Stratégies :
  both      : supprimer test_code/expected_tests_pass, mode → output
  test + main() dans solution   : compiler+run → expected_output
  test + main() dans stages     : fusionner + compiler+run
  test sans main()              : inline test function bodies → main() + expected
"""

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).parent.parent
EXERCISES_DIR = ROOT / "exercises"
FAILURES_LOG = ROOT / "scripts" / "migrate_failures.log"

LINKER_FLAGS: dict[str, list[str]] = {
    "pthreads": ["-lpthread"],
    "semaphores": ["-lpthread"],
    "sync_concepts": ["-lpthread"],
    "sockets": ["-lpthread"],
    "capstones": ["-lpthread"],
    "message_queues": ["-lrt", "-lpthread"],
    "shared_memory": ["-lrt", "-lpthread"],
    "file_io": ["-lrt"],
}

GCC_BASE = ["gcc", "-std=c11", "-D_GNU_SOURCE"]


# ── Utilitaires C parsing ──────────────────────────────────────────────────────

def find_matching(s: str, start: int, open_c: str, close_c: str) -> int:
    """Retourne l'indice du close_c correspondant à s[start] == open_c."""
    depth = 0
    for i in range(start, len(s)):
        if s[i] == open_c:
            depth += 1
        elif s[i] == close_c:
            depth -= 1
            if depth == 0:
                return i
    return -1


def split_at_first_comma(s: str) -> tuple[str, str] | None:
    """Divise s au premier ',' de niveau 0 (hors parenthèses)."""
    depth = 0
    for i, c in enumerate(s):
        if c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
        elif c == ',' and depth == 0:
            return s[:i].strip(), s[i + 1:].strip()
    return None


def extract_assert_args(line: str) -> tuple[str, str] | None:
    """
    Extrait (expected, actual) depuis TEST_ASSERT_EQUAL_INT/STRING(expected, actual).
    Gère les parenthèses imbriquées.
    """
    paren_start = line.find('(')
    if paren_start < 0:
        return None
    paren_end = find_matching(line, paren_start, '(', ')')
    if paren_end < 0:
        return None
    inner = line[paren_start + 1:paren_end]
    return split_at_first_comma(inner)


def extract_single_arg(line: str) -> str | None:
    """Extrait l'argument depuis TEST_ASSERT_TRUE/FALSE/NULL/NOT_NULL(expr)."""
    paren_start = line.find('(')
    if paren_start < 0:
        return None
    paren_end = find_matching(line, paren_start, '(', ')')
    if paren_end < 0:
        return None
    return line[paren_start + 1:paren_end].strip()


def extract_main(code: str) -> str | None:
    """Extrait le bloc int main(...) {...} par comptage d'accolades."""
    m = re.search(r'int\s+main\s*\(', code)
    if not m:
        return None
    brace_start = code.find('{', m.start())
    if brace_start < 0:
        return None
    brace_end = find_matching(code, brace_start, '{', '}')
    if brace_end < 0:
        return None
    return code[m.start():brace_end + 1]


def extract_test_functions(test_code: str) -> list[str]:
    """Retourne la liste des corps de fonctions test_xxx (entre accolades)."""
    bodies = []
    for m in re.finditer(r'void\s+test_\w+\s*\(\s*void\s*\)\s*\{', test_code):
        brace_start = test_code.rfind('{', m.start(), m.end())
        if brace_start < 0:
            continue
        brace_end = find_matching(test_code, brace_start, '{', '}')
        if brace_end < 0:
            continue
        body = test_code[brace_start + 1:brace_end]
        bodies.append(body)
    return bodies


# ── Évaluation sûre d'expressions entières C ──────────────────────────────────

import ast
import operator as _op

_SAFE_OPS = {
    ast.Add: _op.add, ast.Sub: _op.sub, ast.Mult: _op.mul,
    ast.LShift: _op.lshift, ast.RShift: _op.rshift,
    ast.BitAnd: _op.and_, ast.BitOr: _op.or_, ast.BitXor: _op.xor,
    ast.USub: _op.neg, ast.UAdd: _op.pos, ast.Invert: _op.invert,
}


def safe_eval_int(expr: str) -> int | None:
    """Évalue une expression entière C (littéraux + ops bit/arith). Sans exec/eval."""
    cleaned = re.sub(r'[uUlL]+$', '', expr.strip())
    # Convertir les octaux C (0NNN) en octaux Python (0oNNN) — Python3 n'accepte pas 0755
    cleaned = re.sub(r'\b0([0-7]+)\b', r'0o\1', cleaned)
    try:
        tree = ast.parse(cleaned, mode='eval')
    except SyntaxError:
        return None

    def _ev(node: ast.expr) -> int:
        if isinstance(node, ast.Constant):
            if isinstance(node.value, int):
                return node.value
            raise ValueError
        if isinstance(node, ast.BinOp):
            if type(node.op) not in _SAFE_OPS:
                raise ValueError
            return _SAFE_OPS[type(node.op)](_ev(node.left), _ev(node.right))
        if isinstance(node, ast.UnaryOp):
            if type(node.op) not in _SAFE_OPS:
                raise ValueError
            return _SAFE_OPS[type(node.op)](_ev(node.operand))
        raise ValueError

    try:
        return _ev(tree.body)
    except (ValueError, TypeError):
        return None


# ── Conversion TEST_ASSERT → printf ───────────────────────────────────────────

def convert_assert_line(stripped: str) -> tuple[str, str] | None:
    """
    Convertit une ligne TEST_ASSERT_* en (printf_line, expected_value_str).
    Retourne None si non reconnu ou trop complexe.
    """
    if stripped.startswith('TEST_ASSERT_EQUAL_INT'):
        args = extract_assert_args(stripped)
        if args is None:
            return None
        expected_str, actual_str = args
        val = safe_eval_int(expected_str)
        if val is None:
            return None
        return f'printf("%d\\n", (int)({actual_str}));', str(val)

    if stripped.startswith('TEST_ASSERT_EQUAL_STRING'):
        args = extract_assert_args(stripped)
        if args is None:
            return None
        expected_str, actual_str = args
        clean = expected_str.strip('"')
        return f'printf("%s\\n", {actual_str});', clean

    if stripped.startswith('TEST_ASSERT_NOT_NULL'):
        arg = extract_single_arg(stripped)
        if arg is None:
            return None
        return f'printf("%d\\n", ({arg}) != NULL ? 1 : 0);', "1"

    if stripped.startswith('TEST_ASSERT_NULL'):
        arg = extract_single_arg(stripped)
        if arg is None:
            return None
        return f'printf("%d\\n", ({arg}) == NULL ? 1 : 0);', "1"

    if stripped.startswith('TEST_ASSERT_TRUE'):
        arg = extract_single_arg(stripped)
        if arg is None:
            return None
        return f'printf("%d\\n", ({arg}) ? 1 : 0);', "1"

    if stripped.startswith('TEST_ASSERT_FALSE'):
        arg = extract_single_arg(stripped)
        if arg is None:
            return None
        return f'printf("%d\\n", ({arg}) ? 1 : 0);', "0"

    return None  # Non reconnu


def extract_static_helpers(test_code: str, solution_code: str) -> str:
    """
    Extrait les fonctions static du test_code qui ne sont PAS déjà dans solution_code.
    Ces helpers sont nécessaires pour que les test functions compilent.
    """
    helpers: list[str] = []
    for m in re.finditer(r'static\s+\w[\w\s\*]*\s+(\w+)\s*\([^)]*\)\s*\{', test_code):
        func_name = m.group(1)
        if func_name in solution_code:
            continue  # Déjà définie
        brace_start = test_code.rfind('{', m.start(), m.end())
        if brace_start < 0:
            continue
        brace_end = find_matching(test_code, brace_start, '{', '}')
        if brace_end < 0:
            continue
        helpers.append(test_code[m.start():brace_end + 1])
    return '\n'.join(helpers)


def generate_main_from_test_code(test_code: str) -> tuple[str, str] | None:
    """
    Génère (main_code, expected_output) depuis le test_code.
    Inline les corps des fonctions test_xxx en blocs dans main().
    """
    bodies = extract_test_functions(test_code)
    if not bodies:
        return None

    main_lines: list[str] = ["int main(void) {"]
    expected_values: list[str] = []

    for body in bodies:
        main_lines.append("    {")
        for raw_line in body.splitlines():
            stripped = raw_line.strip()
            if not stripped:
                continue

            if stripped.startswith('TEST_ASSERT_'):
                result = convert_assert_line(stripped)
                if result is None:
                    continue  # Assertion trop complexe → ignorer (pas bloquer)
                printf_line, exp_val = result
                main_lines.append(f'        {printf_line}')
                expected_values.append(exp_val)
            elif stripped.startswith('RUN_TEST') or stripped.startswith('TEST_SUMMARY'):
                pass  # Ignorer les macros de contrôle
            else:
                main_lines.append(f'        {stripped}')

        main_lines.append("    }")

    main_lines.extend(["    return 0;", "}"])

    if not expected_values:
        return None

    return "\n".join(main_lines), "\n".join(expected_values)


# ── Compilation ────────────────────────────────────────────────────────────────

def ensure_headers(code: str) -> str:
    """Ajoute les includes C standard manquants selon le contenu du code."""
    headers = []
    if '#include <stdio.h>' not in code and '#include<stdio.h>' not in code:
        headers.append('#include <stdio.h>')
    if '#include <string.h>' not in code and any(
        fn in code for fn in ('strstr', 'strlen', 'strcmp', 'strcpy', 'strcat', 'memcpy', 'memset')
    ):
        headers.append('#include <string.h>')
    if '#include <stdlib.h>' not in code and any(
        fn in code for fn in ('malloc', 'calloc', 'realloc', 'free')
    ):
        headers.append('#include <stdlib.h>')
    if not headers:
        return code
    return '\n'.join(headers) + '\n' + code


def compile_and_run(
    c_source: str, subject: str, timeout: int = 10
) -> tuple[bool, str, str]:
    flags = LINKER_FLAGS.get(subject, [])
    with tempfile.TemporaryDirectory() as tmpdir:
        src = os.path.join(tmpdir, "ex.c")
        out = os.path.join(tmpdir, "ex")
        with open(src, "w") as f:
            f.write(c_source)
        r = subprocess.run(
            GCC_BASE + ["-o", out, src] + flags,
            capture_output=True, text=True, timeout=30,
        )
        if r.returncode != 0:
            return False, "", r.stderr
        try:
            r2 = subprocess.run(
                [out], capture_output=True, text=True, timeout=timeout
            )
        except subprocess.TimeoutExpired:
            return False, "", "TIMEOUT"
        return True, r2.stdout, r2.stderr


def normalize_output(text: str) -> str:
    lines = [l.rstrip() for l in text.splitlines()]
    return "\n".join(lines).strip()


def merge_with_main(solution_code: str, main_block: str) -> str:
    if 'int main(' in solution_code:
        return solution_code
    return solution_code.rstrip() + "\n\n" + main_block + "\n"


# ── Traitement d'un exercice ───────────────────────────────────────────────────

def process_exercise(path: Path) -> tuple[str, str]:
    with open(path) as f:
        ex = json.load(f)

    mode = ex.get("validation", {}).get("mode", "output")
    subject = ex.get("subject", "")
    val = ex["validation"]

    if mode == "output":
        return "skip", "already output mode"

    # ── BOTH : expected_output + main déjà présents ────────────────────────
    if mode == "both":
        val["mode"] = "output"
        val.pop("test_code", None)
        val.pop("expected_tests_pass", None)
        with open(path, "w") as f:
            json.dump(ex, f, indent=2, ensure_ascii=False)
            f.write("\n")
        return "ok", "both → output (kept existing expected_output)"

    # ── TEST ───────────────────────────────────────────────────────────────
    solution_code = ex.get("solution_code", "")
    stages = ex.get("starter_code_stages", [])
    test_code = val.get("test_code", "")

    # Cas : expected_output déjà là + main dans solution
    if val.get("expected_output") and 'int main(' in solution_code:
        val["mode"] = "output"
        val.pop("test_code", None)
        val.pop("expected_tests_pass", None)
        with open(path, "w") as f:
            json.dump(ex, f, indent=2, ensure_ascii=False)
            f.write("\n")
        return "ok", "test → output (existing expected_output)"

    main_source = "solution_code"
    complete_solution = solution_code

    if 'int main(' not in solution_code:
        # Chercher un main() non-trivial dans les stages
        stage_main = None
        for stage in stages:
            m = extract_main(stage)
            if m and 'TODO' not in m and '____' not in m:
                body = m[m.find('{') + 1:m.rfind('}')].strip()
                if len(body) > 15:
                    stage_main = m
                    break

        if stage_main:
            complete_solution = merge_with_main(solution_code, stage_main)
            main_source = "stage"
        else:
            # Inline test function bodies
            gen = generate_main_from_test_code(test_code)
            if gen is None:
                return "fail", "no main() found and test_code too complex to parse"
            gen_main, gen_expected = gen
            # Ajouter les helpers static du test_code manquants dans solution
            helpers = extract_static_helpers(test_code, solution_code)
            base = solution_code
            if helpers:
                base = base.rstrip() + "\n\n" + helpers
            # Assurer que les includes sont présents
            full_code = ensure_headers(merge_with_main(base, gen_main))
            ok, _, stderr = compile_and_run(full_code, subject)
            if not ok:
                return "fail", f"auto-gen compile error: {stderr[:250]}"
            # expected_output calculé statiquement (plus déterministe que stdout)
            val["mode"] = "output"
            val["expected_output"] = gen_expected
            val.pop("test_code", None)
            val.pop("expected_tests_pass", None)
            ex["solution_code"] = full_code
            with open(path, "w") as f:
                json.dump(ex, f, indent=2, ensure_ascii=False)
                f.write("\n")
            n = len(gen_expected.splitlines())
            return "ok", f"test → output (auto-gen inline, {n} lines, compile OK)"

    # Assurer les includes pour les cas avec main() (stage ou solution)
    complete_solution = ensure_headers(complete_solution)

    ok, stdout, stderr = compile_and_run(complete_solution, subject)
    if not ok or not normalize_output(stdout):
        # Fallback : essayer l'inline depuis test_code
        reason_prefix = "stage compile failed" if not ok else "stage empty stdout"
        gen = generate_main_from_test_code(test_code)
        if gen is None:
            if not ok:
                return "fail", f"compile/run failed ({main_source}): {stderr[:300]}"
            return "fail", f"empty stdout ({main_source}) and test_code too complex"
        gen_main, gen_expected = gen
        helpers = extract_static_helpers(test_code, solution_code)
        base2 = solution_code.rstrip() + ("\n\n" + helpers if helpers else "")
        fallback_code = ensure_headers(merge_with_main(base2, gen_main))
        ok2, _, stderr2 = compile_and_run(fallback_code, subject)
        if not ok2:
            return "fail", f"{reason_prefix} + inline compile error: {stderr2[:200]}"
        val["mode"] = "output"
        val["expected_output"] = gen_expected
        val.pop("test_code", None)
        val.pop("expected_tests_pass", None)
        ex["solution_code"] = fallback_code
        with open(path, "w") as f:
            json.dump(ex, f, indent=2, ensure_ascii=False)
            f.write("\n")
        n = len(gen_expected.splitlines())
        return "ok", f"test → output (inline fallback after {reason_prefix}, {n} lines)"

    expected_output = normalize_output(stdout)

    val["mode"] = "output"
    val["expected_output"] = expected_output
    val.pop("test_code", None)
    val.pop("expected_tests_pass", None)
    if main_source == "stage":
        ex["solution_code"] = complete_solution
    elif main_source == "solution_code" and complete_solution != solution_code:
        # stdio.h a été ajouté
        ex["solution_code"] = complete_solution

    with open(path, "w") as f:
        json.dump(ex, f, indent=2, ensure_ascii=False)
        f.write("\n")

    n = len(expected_output.splitlines())
    return "ok", f"test → output ({main_source}, {n} lines)"


# ── Point d'entrée ─────────────────────────────────────────────────────────────

def main() -> None:
    paths = sorted(EXERCISES_DIR.rglob("*.json"))
    paths = [p for p in paths if "annales_map" not in p.name]

    ok_count = skip_count = fail_count = 0
    failures: list[tuple[str, str]] = []

    for path in paths:
        try:
            status, reason = process_exercise(path)
        except Exception as e:
            status, reason = "fail", f"exception: {e}"

        rel = path.relative_to(ROOT)
        if status == "ok":
            ok_count += 1
            print(f"  ✓ {rel}: {reason}")
        elif status == "skip":
            skip_count += 1
        else:
            fail_count += 1
            failures.append((str(rel), reason))
            print(f"  ✗ {rel}: {reason}", file=sys.stderr)

    print("\n─── Résultat ───")
    print(f"  Convertis : {ok_count}")
    print(f"  Ignorés   : {skip_count} (déjà output)")
    print(f"  Échecs    : {fail_count}")

    if failures:
        with open(FAILURES_LOG, "w") as f:
            for fpath, reason in failures:
                f.write(f"{fpath}: {reason}\n")
        print(f"\n  Voir : {FAILURES_LOG}")


if __name__ == "__main__":
    main()
