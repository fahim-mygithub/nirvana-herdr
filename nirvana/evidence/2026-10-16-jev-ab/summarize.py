"""Recomputes the Jev A/B figures from the result files beside it. Numbers live here, not in prose.

Each results-<agent><suffix>.jsonl line is one run (ab.py). Suffix: '' the 71 KB project, '-big' the 568 KB one
with the first guide, '-big2' and '-big3' the 568 KB one with the second guide ("open only the read first file").
Tokens are every input token the agent's model calls consumed: fresh input plus cache reads plus cache writes for
Claude, `input_tokens` (cache reads included) for agy.
"""
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent
GUIDE = {"": "guide v1", "-big": "guide v1", "-big2": "guide v2", "-big3": "guide v2"}
PROJECT = {"": "71 KB", "-big": "568 KB", "-big2": "568 KB", "-big3": "568 KB"}

cells = defaultdict(list)
for path in sorted(HERE.glob("results-*.jsonl")):
    agent_suffix = path.stem[len("results-"):]
    agent, _, rest = agent_suffix.partition("-")
    suffix = f"-{rest}" if rest else ""
    for line in path.read_text(encoding="utf-8").splitlines():
        r = json.loads(line)
        guide = GUIDE[suffix] if r["arm"] == "on" else "no guide"
        cells[(PROJECT[suffix], r["task"], agent, guide)].append(r)

print(f"{'project':7} {'task':4} {'agent':6} {'arm':9} {'runs':>4} {'right':>5} {'used jev':>8} {'mean tokens':>11} {'median':>8} {'min':>8} {'max':>8} {'jev $':>8} {'secs':>5}")
for key in sorted(cells):
    rows = cells[key]
    tokens = [r["input_tokens_total"] for r in rows]
    used = sum(1 for r in rows if r["jev_calls_logged"] > 0)
    print(f"{key[0]:7} {key[1]:4} {key[2]:6} {key[3]:9} {len(rows):>4} {sum(r['right'] for r in rows):>5} {used:>8} "
          f"{statistics.mean(tokens):>11,.0f} {statistics.median(tokens):>8,.0f} {min(tokens):>8,} {max(tokens):>8,} "
          f"{sum(r['jev_cost_usd'] for r in rows):>8.4f} {statistics.mean(r['secs'] for r in rows):>5.0f}")

print()
for (project, task, agent, guide), rows in sorted(cells.items()):
    if guide == "no guide":
        continue
    base = cells.get((project, task, agent, "no guide"))
    if not base:
        continue
    a = statistics.mean(r["input_tokens_total"] for r in base)
    b = statistics.mean(r["input_tokens_total"] for r in rows)
    print(f"{project} {task} {agent} {guide}: {b - a:+,.0f} tokens per task ({(b - a) / a:+.0%}) against {len(base)} runs with no guide")
