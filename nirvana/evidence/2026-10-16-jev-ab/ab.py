"""Jev A/B: the same task with the Jev guide off and on, per agent. Real models, real Jev.

python ab.py <agent: claude|agy> <repeats> [tasks comma list]
Writes one JSON line per run to results-<agent>.jsonl beside this script, and every transcript to runs/.
Expected before the run (the red lines): with the guide on, the agent runs `herdr jev` at least once in
most T1 runs; a run that answers wrong is reported, never dropped; tokens are compared, not assumed.
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
SCRATCH = HERE.parent
BASE = SCRATCH / "jev-ab" / os.environ.get("AB_BASE", "base")
RUNS = SCRATCH / "jev-ab" / "runs"
HOME = SCRATCH / "nh-jev-ab"
EXE = HOME / "bin" / "herdr.exe"
LOG = HOME / "jev" / "calls.jsonl"

TASKS = {
    "T1": ("The proration test fails because of rounding. Which file and function compute the amount wrong? "
           "Do not edit any file. Answer with the path and the function name, and one sentence why."),
    "T2": ("Run the tests with `python -m unittest discover -s tests -t .` and tell me in one sentence why they fail. "
           "Do not edit any file."),
}
RIGHT = {
    "T1": lambda a: "subscriptions" in a and "_prorated_amount" in a,
    "T2": lambda a: bool(re.search(r"trunc|int\(|round", a, re.I)),
}


def env():
    e = dict(os.environ)
    e["NIRVANA_HERDR_HOME"] = str(HOME)
    e["BROWSER"] = "true"
    e.pop("CLAUDECODE", None)
    e.pop("CLAUDE_CODE_ENTRYPOINT", None)
    return e


def log_lines():
    try:
        return LOG.read_text(encoding="utf-8").splitlines()
    except OSError:
        return []


def run_claude(folder, prompt, out):
    cmd = ["claude", "-p", prompt, "--model", "sonnet", "--output-format", "stream-json", "--verbose",
           "--dangerously-skip-permissions"]
    started = time.time()
    with open(out, "w", encoding="utf-8") as fh:
        p = subprocess.run(cmd, cwd=folder, env=env(), stdout=fh, stderr=subprocess.STDOUT, timeout=900,
                           stdin=subprocess.DEVNULL)
    secs = time.time() - started
    tools, jev, answer, usage, cost, turns, hook_seen = [], 0, "", {}, None, None, False
    for line in open(out, encoding="utf-8", errors="replace"):
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        if ev.get("type") == "system" and ev.get("subtype") in ("hook_response",) and "jev" in json.dumps(ev):
            hook_seen = True
        if ev.get("type") == "assistant":
            for b in ev["message"].get("content", []):
                if b.get("type") == "tool_use":
                    name = b["name"]
                    detail = b.get("input", {}).get("command", "") if name == "Bash" else b.get("input", {}).get("file_path", "") or b.get("input", {}).get("pattern", "")
                    tools.append(f"{name}:{str(detail)[:90]}")
                    if name == "Bash" and " jev " in f" {b['input'].get('command', '')} ":
                        jev += 1
        if ev.get("type") == "result":
            answer = ev.get("result") or ""
            usage = ev.get("usage") or {}
            cost = ev.get("total_cost_usd")
            turns = ev.get("num_turns")
    tokens = sum(int(usage.get(k) or 0) for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"))
    return {"exit": p.returncode, "secs": round(secs), "tools": tools, "jev_commands": jev, "answer": answer,
            "input_tokens_total": tokens, "output_tokens": usage.get("output_tokens"), "cost_usd": cost,
            "turns": turns, "guide_hook_seen": hook_seen}


def run_agy(folder, prompt, out):
    from agy_parse import parse
    cmd = ["agy", "--model", "gemini-3.8-flash-low", "--dangerously-skip-permissions", "--output-format",
           "stream-json", f"-p={prompt}"]
    started = time.time()
    with open(out, "w", encoding="utf-8") as fh:
        p = subprocess.run(cmd, cwd=folder, env=env(), stdout=fh, stderr=subprocess.STDOUT, timeout=900,
                           stdin=subprocess.DEVNULL)
    secs = time.time() - started
    return {"exit": p.returncode, "secs": round(secs), **parse(out)}
    tools, jev, answer, usage, steps = [], 0, "", {}, 0
    text = Path(out).read_text(encoding="utf-8", errors="replace")
    for line in text.splitlines():
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        if ev.get("type") == "step_update":
            steps += 1
            blob = json.dumps(ev)
            for m in re.finditer(r'"(run_command|view_file|grep_search|list_dir|find_by_name|view_file_outline|view_code_item)"', blob):
                tools.append(m.group(1))
            if re.search(r"jev (file|files|text)", blob):
                jev += 1
        if ev.get("type") == "result":
            usage = ev.get("usage") or {}
            answer = ev.get("result") or ev.get("text") or json.dumps(ev)[:2000]
    return {"exit": p.returncode, "secs": round(secs), "tools": tools, "jev_mentions": jev, "answer": answer,
            "input_tokens_total": int(usage.get("input_tokens") or 0) + int(usage.get("cache_read_tokens") or 0),
            "output_tokens": usage.get("output_tokens"), "steps": steps}


def main():
    agent, repeats = sys.argv[1], int(sys.argv[2])
    tasks = sys.argv[3].split(",") if len(sys.argv) > 3 else list(TASKS)
    results = HERE / f"results-{agent}{os.environ.get('AB_SUFFIX', '')}.jsonl"
    RUNS.mkdir(parents=True, exist_ok=True)
    for i in range(repeats):
        for task in tasks:
            for arm in os.environ.get("AB_ARMS", "off,on").split(","):
                name = f"{agent}{os.environ.get('AB_SUFFIX', '')}-{task}-{arm}-{i}-{int(time.time())}"
                folder = RUNS / name
                shutil.copytree(BASE, folder)
                if arm == "on":
                    sw = subprocess.run([str(EXE), "jev", "on", "guide", "--cwd", str(folder)], env=env(),
                                        capture_output=True, text=True)
                    assert sw.returncode == 0, sw.stderr
                before = len(log_lines())
                runner = run_claude if agent == "claude" else run_agy
                row = runner(folder, TASKS[task], RUNS / f"{name}.jsonl")
                calls = [json.loads(l) for l in log_lines()[before:]]
                calls = [c for c in calls if str(folder) in str(c.get("folder", "")) or str(folder).replace("\\", "/") in str(c.get("folder", "")).replace("\\", "/")]
                row.update({"agent": agent, "task": task, "arm": arm, "repeat": i, "folder": name,
                            "right": RIGHT[task](row["answer"]), "jev_calls_logged": len(calls),
                            "jev_content_tokens": sum(int(c.get("content_tokens") or 0) for c in calls),
                            "jev_cost_usd": round(sum(float(c.get("cost_usd") or 0) for c in calls), 6),
                            "jev_agents": sorted({c.get("agent") for c in calls})})
                with open(results, "a", encoding="utf-8") as fh:
                    fh.write(json.dumps(row) + "\n")
                print(name, row["right"], row["input_tokens_total"], "jev", row["jev_calls_logged"], row["secs"], "s", flush=True)


if __name__ == "__main__":
    main()
