"""Jev single run: one five-turn Claude session that has a use for every Jev endpoint, Jev off then on.

python single_run.py [arms, default off,on]

One Claude process per arm, the turns fed through stream-json input, so the session starts once like an
interactive one (one SessionStart, a Stop hook per turn). The 568 KB project, Claude Sonnet, real Jev.
Each turn is built for one endpoint:
  1 `jev files`  find the bug among 14 modules
  2 `jev file`   a yes/no about one 54 KB file
  3 `jev text`   why the test run fails
  4 (none)       the fix: an edit needs the real read
  5 compact      a new question after finished work: the clean break level 7 looks for
The compact lines are lowered to 40k/80k/150k for this run, since a five-turn session stays under 150k.

Red lines written before the run:
- the off arm logs no Jev call and no compact line;
- the on arm logs a compact line for every turn, and at least three of files, file, text by the agent;
- every turn answers right in both arms, and the tests pass after turn 4; a wrong turn is reported;
- tokens are compared per turn and in total, not assumed.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
BASE = HERE / "big"
RUNS = HERE / "single"
HOME = HERE.parent / "nh-jev-ab"
EXE = HOME / "bin" / "herdr.exe"
CALLS = HOME / "jev" / "calls.jsonl"
COMPACT = HOME / "jev" / "compact.jsonl"
LINES = "40000,80000,150000"

TURNS = [
    ("files", "The proration test fails because of rounding. Which file and function compute the amount wrong? "
              "Do not edit any file yet. Answer with the path and the function name.",
     lambda a, d: "subscriptions" in a and "_prorated_amount" in a),
    ("file", "Does ledgerly/invoices.py also truncate cents with int() anywhere? Answer yes or no first.",
     lambda a, d: bool(re.match(r"\W*no\b", a.strip(), re.I))),
    ("text", "Run the tests with `python -m unittest discover -s tests -t . -v` and tell me in one sentence "
             "which test fails and why.",
     lambda a, d: "cancel_refund" in a and bool(re.search(r"trunc|int\(|round|1666|1667", a, re.I))),
    ("edit", "Fix _prorated_amount so it rounds with money.round_cents instead of int(), then run the tests and "
             "confirm they pass.",
     lambda a, d: tests_pass(d)),
    ("compact", "Thanks, that is done. Completely different question now: which module defines the dunning steps? "
                "One line.",
     lambda a, d: "notifications" in a),
]


def tests_pass(folder):
    r = subprocess.run([sys.executable, "-m", "unittest", "discover", "-s", "tests", "-t", "."], cwd=folder,
                       capture_output=True, text=True)
    return r.returncode == 0


def env():
    e = dict(os.environ, NIRVANA_HERDR_HOME=str(HOME), BROWSER="true", NIRVANA_JEV_COMPACT_LINES=LINES)
    e.pop("CLAUDECODE", None)
    e.pop("CLAUDE_CODE_ENTRYPOINT", None)
    return e


def lines(path):
    try:
        return path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return []


def run_arm(arm):
    name = f"single-{arm}-{int(time.time())}"
    folder = RUNS / name
    shutil.copytree(BASE, folder)
    if arm == "on":
        for switch in ("guide", "compact"):
            sw = subprocess.run([str(EXE), "jev", "on", switch, "--cwd", str(folder)], env=env(),
                                capture_output=True, text=True)
            assert sw.returncode == 0, sw.stderr
    calls_before, compact_before = len(lines(CALLS)), len(lines(COMPACT))
    out = open(RUNS / f"{name}.jsonl", "w", encoding="utf-8")
    p = subprocess.Popen(["claude", "-p", "--model", "sonnet", "--input-format", "stream-json", "--output-format",
                          "stream-json", "--verbose", "--dangerously-skip-permissions"],
                         cwd=folder, env=env(), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace")
    rows, session = [], None
    for i, (endpoint, prompt, right) in enumerate(TURNS, 1):
        started = time.time()
        p.stdin.write(json.dumps({"type": "user", "message": {"role": "user", "content": prompt}}) + "\n")
        p.stdin.flush()
        tools, jev, context, hooks, result = [], [], 0, [], None
        for line in p.stdout:
            out.write(line)
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            session = ev.get("session_id") or session
            if ev.get("type") == "system" and "hook" in str(ev.get("subtype")) and "jev" in line:
                hooks.append(ev.get("subtype"))
            if ev.get("type") == "assistant":
                u = ev["message"].get("usage") or {}
                context = max(context, sum(int(u.get(k) or 0) for k in
                                           ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")))
                for b in ev["message"].get("content", []):
                    if b.get("type") == "tool_use":
                        inp = b.get("input", {})
                        detail = inp.get("command") or inp.get("file_path") or inp.get("pattern") or ""
                        tools.append(f"{b['name']}:{str(detail)[:100]}")
                        m = re.search(r"\bjev (files|file|text)\b", str(inp.get("command", "")))
                        if m:
                            jev.append(m.group(1))
            if ev.get("type") == "result":
                result = ev
                break
        if result is None:
            rows.append({"turn": i, "endpoint": endpoint, "error": "no result"})
            break
        u = result.get("usage") or {}
        answer = result.get("result") or ""
        rows.append({"turn": i, "endpoint": endpoint, "secs": round(time.time() - started),
                     "right": right(answer, folder), "answer": answer[:300], "tools": tools, "jev_commands": jev,
                     "input_tokens_total": sum(int(u.get(k) or 0) for k in
                                               ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")),
                     "output_tokens": u.get("output_tokens"), "peak_context": context, "turns": result.get("num_turns"),
                     "jev_hooks_seen": hooks})
        print(arm, "turn", i, endpoint, rows[-1]["right"], rows[-1]["input_tokens_total"], "jev", jev, flush=True)
    p.stdin.close()
    try:
        p.wait(timeout=60)
    except subprocess.TimeoutExpired:
        p.kill()
    out.close()
    calls = [json.loads(l) for l in lines(CALLS)[calls_before:]]
    calls = [c for c in calls if Path(str(c.get("folder", ""))).resolve() == folder.resolve()]
    compact = [json.loads(l) for l in lines(COMPACT)[compact_before:]]
    compact = [c for c in compact if c.get("session") == session]
    return {"arm": arm, "folder": name, "session": session, "turns": rows,
            "jev_calls": [{k: c.get(k) for k in ("tool", "path", "answer", "content_tokens", "cost_usd", "agent", "ms")}
                          for c in calls],
            "compact": [{k: c.get(k) for k in ("level", "outcome", "why", "tokens", "reading", "said")} for c in compact]}


def main():
    arms = sys.argv[1].split(",") if len(sys.argv) > 1 else ["off", "on"]
    RUNS.mkdir(exist_ok=True)
    for arm in arms:
        row = run_arm(arm)
        with open(HERE / "single-results.jsonl", "a", encoding="utf-8") as fh:
            fh.write(json.dumps(row) + "\n")


if __name__ == "__main__":
    main()
