#!/usr/bin/env python3
"""
fix_starter_code_main.py — Ajoute un main() minimal aux starter_code qui n'en ont pas.

Pour le mode output, le starter_code doit être un programme C compilable.
Stratégie : prendre le stage le plus minimal (S4 → S3 → S2) qui a un main(),
extraire uniquement ce main(), l'ajouter au starter_code.
"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).parent.parent
EXERCISES_DIR = ROOT / "exercises"

MINIMAL_MAIN = "\nint main(void) {\n    /* TODO */\n    return 0;\n}\n"


def find_matching_brace(s: str, start: int) -> int:
    depth = 0
    for i in range(start, len(s)):
        if s[i] == '{':
            depth += 1
        elif s[i] == '}':
            depth -= 1
            if depth == 0:
                return i
    return -1


def extract_main(code: str) -> str | None:
    m = re.search(r'int\s+main\s*\(', code)
    if not m:
        return None
    brace_start = code.find('{', m.start())
    if brace_start < 0:
        return None
    brace_end = find_matching_brace(code, brace_start)
    if brace_end < 0:
        return None
    return code[m.start():brace_end + 1]


def main_is_trivial(main_block: str) -> bool:
    """Retourne True si le main() est juste return 0 (inutile comme modèle)."""
    body = main_block[main_block.find('{') + 1:main_block.rfind('}')].strip()
    return body in ('', 'return 0;', 'return 0 ;')


def fix_stages(ex: dict) -> bool:
    """Ajoute main() depuis solution_code dans chaque stage qui en manque."""
    stages = ex.get('starter_code_stages')
    if not stages:
        return False

    solution_main = extract_main(ex.get('solution_code', ''))
    if not solution_main:
        return False

    changed = False
    for i, code in enumerate(stages):
        if 'int main(' not in code:
            stages[i] = code.rstrip() + '\n\n' + solution_main + '\n'
            changed = True

    return changed


def process(path: Path) -> str:
    with open(path) as f:
        ex = json.load(f)

    if ex.get('validation', {}).get('mode', 'output') != 'output':
        return 'skip-mode'

    starter = ex.get('starter_code', '')
    changed = False

    if 'int main(' not in starter:
        stages = ex.get('starter_code_stages', [])
        chosen_main = None

        # Parcourir des stages du plus minimal (S4) au plus guidé (S0)
        for stage in reversed(stages):
            m = extract_main(stage)
            if m and not main_is_trivial(m):
                chosen_main = m
                break

        if chosen_main is None:
            for stage in reversed(stages):
                m = extract_main(stage)
                if m:
                    chosen_main = m
                    break

        if chosen_main is None:
            chosen_main = MINIMAL_MAIN.strip()

        ex['starter_code'] = starter.rstrip() + '\n\n' + chosen_main + '\n'
        changed = True

    # Fixer aussi les starter_code_stages manquant un main()
    if fix_stages(ex):
        changed = True

    if not changed:
        return 'skip-has-main'

    with open(path, 'w') as f:
        json.dump(ex, f, indent=2, ensure_ascii=False)
        f.write('\n')

    return 'fixed'


def main() -> None:
    paths = sorted(EXERCISES_DIR.rglob('*.json'))
    paths = [p for p in paths if 'annales_map' not in p.name]

    counts = {'fixed': 0, 'skip-has-main': 0, 'skip-mode': 0}
    for path in paths:
        result = process(path)
        counts[result] = counts.get(result, 0) + 1
        if result == 'fixed':
            print(f'  ✓ {path.relative_to(ROOT)}')

    print('\n─── Résultat ───')
    print(f'  Corrigés         : {counts["fixed"]}')
    print(f'  Déjà OK (main)   : {counts["skip-has-main"]}')
    print(f'  Ignorés (mode)   : {counts["skip-mode"]}')


if __name__ == '__main__':
    main()
