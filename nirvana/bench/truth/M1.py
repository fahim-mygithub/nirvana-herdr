# Copied from old Nirvana (GraftworkGames Nirvana repo, commit on int/moto-ab) bench/truth/M1.py, 2026-10-18; the held-out truth of the 2026-10-12 Moto A/B, unchanged except this line.
"""Held-out truth for moto scenario M1: the owner's "what is this project and how do I start it" is answered, nothing built.

THE OWNER, event 40567 (ask_dispatch, 2026-09-30T15:04:46Z, run-20260930T150442, title "Explain project and how to
start"): "What is this project and can you start it?". M1 keeps the explain half and turns "can you start it" into
"how I start" so the scenario stays a question (M2 is the starting half); the scenario's `source` says so.

WHAT A GOOD ANSWER HOLDS, read from the clone on 2026-10-12 (D:/MotoProject at 60476eb plus the scenario's
attachments): CLAUDE.md opens "Turn 360 motorcycle footage into vertical Instagram reels by programmatically
reframing one spherical capture into multiple virtual camera angles"; editor/server/main.py's docstring is
`python editor/server/main.py --port 8770` (DEFAULT_PORT 8770, the owner's own servers.json command); editor/CONTRACT.md
puts presets in `angles/*.json` and the per-clip `forward_yaw` and marks in `marks/<clip>.json` (the hike roster is
`angles_hike/`).

THE RULES are floors, not a grader: the answer names the pipeline (360 or equirectangular footage AND a vertical /
9:16 / Instagram reel), names how to start the editor (`main.py`, or the 8770 port), and names where presets or marks
live (`angles`, `angles_hike` or `marks` as a folder). A question is answered, not built (decision 0029 C1): HEAD is
still the base, the clone's one branch and one worktree, no tracked change (bytecode under `__pycache__/` aside: the
project tracks scripts/__pycache__/*.pyc and any Python run rewrites them, measured in moto_facts.json).

Usage: python bench/truth/M1.py --tree <final tree> --base <base sha> --answer <answer.txt>
Prints `PASS <rule>` / `FAIL <rule> -- <detail>` per rule, then `RESULT PASS (<n> checks)` or
`RESULT FAIL (<k> of <n> failed)`. Exit 0 all pass, 1 a rule failed, 2 usage error.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import M2  # noqa: E402  git, answer and result helpers

SPHERE = re.compile(r"\b360\b|360[°º-]|equirect|spherical", re.IGNORECASE)
VERTICAL = re.compile(r"\bvertical\b|\b9\s*:\s*16\b|\binstagram\b|\breels?\b.{0,40}\b(?:portrait|1080\s*x\s*1920)",
                      re.IGNORECASE)
START = re.compile(r"\bmain\.py\b|\b8770\b", re.IGNORECASE)
WHERE = re.compile(r"\bangles(?:_hike|_high)?/|\bmarks/|`(?:angles|angles_hike|marks)`|"
                   r"\b(?:angles|angles_hike|marks)\s+(?:folder|directory|dir)\b|"
                   r"\b(?:folder|directory|dir)\s+`?(?:angles|angles_hike|marks)\b", re.IGNORECASE)
PATHS = ("CLAUDE.md", "editor/server/main.py", "editor/CONTRACT.md")


def main(argv: list[str] | None = None) -> int:
    ap = M2.parser(__doc__)
    try:
        args = ap.parse_args(argv)
    except SystemExit as e:
        return 0 if e.code == 0 else 2
    tree = Path(args.tree)
    if not tree.is_dir():
        print(f"usage error: --tree {tree} is not a directory")
        return 2
    rows, check = M2.runner()
    answer = M2.read_answer(args.answer)
    check("answer_not_empty", bool(answer.strip()), f"{len(answer)} characters in {args.answer}")
    present = [p for p in PATHS if M2.git(tree, "cat-file", "-e", f"{args.base}:{p}").returncode == 0]
    check("the base holds the project and its editor", len(present) == len(PATHS), f"only {present} at {args.base[:12]}")
    check("names_the_pipeline (360/equirect footage to a vertical reel)",
          bool(SPHERE.search(answer)) and bool(VERTICAL.search(answer)),
          f"360/equirect named: {bool(SPHERE.search(answer))}, vertical/9:16/Instagram named: {bool(VERTICAL.search(answer))}")
    check("names_how_to_start_the_editor (main.py or port 8770)", bool(START.search(answer)), "neither")
    check("names_where_presets_or_marks_live (angles/ or marks/)", bool(WHERE.search(answer)), "no angles/ or marks/ folder")
    M2.git_rules(check, tree, args.base)
    branches = [b for b in M2.git(tree, "branch", "--list", "--format=%(refname:short)").stdout.split() if b]
    check("no_new_branch", len(branches) == 1, f"branches {branches}")
    worktrees = [ln for ln in M2.git(tree, "worktree", "list", "--porcelain").stdout.splitlines()
                 if ln.startswith("worktree ")]
    check("no_new_worktree", len(worktrees) == 1, f"{len(worktrees)} worktrees")
    return M2.finish(rows)


if __name__ == "__main__":
    sys.exit(main())
