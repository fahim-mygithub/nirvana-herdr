"""Plan 25 §3: the lean-start probe. One `claude -p ok` per variant, Nirvana Claude's argv, Haiku.

Run from the worktree root: `python nirvana/plans/2026-10-22-25-records/probe.py <probe folder>`.

The work order's rules, each kept here:
- a new folder under the session scratchpad, never an owner folder: `<probe folder>/<variant>/` is each run's cwd,
  and `<probe folder>/home` stands in for the Nirvana home (a copy of this worktree's `nirvana/claude`), so no hook
  writes into the installed home and no file of the owner's install is read;
- `--model haiku`, `--output-format json`, the prompt `ok`, a list argv from Python (never Git Bash: MSYS rewrites
  slash arguments);
- `BROWSER` set to a program that opens nothing (`where.exe`, as the parent repo's `tools/capture_transcript.py`
  measured on 2026-09-20), so no sign-in can finish;
- one run per variant, at most 14.

The argv is `nirvana_launch::nirvana_claude_args` with the probe's home, with two changes: `--model haiku` for the
seat's model, and no `--rc`, which would register the run with the owner's remote-control session (an outward
action the probe has no need of; it adds no context). `--resume` is never passed.

The reading is the first turn's `input_tokens + cache_creation_input_tokens + cache_read_input_tokens`, from the
result's `usage` (one API call for the prompt `ok`; `num_turns` is printed to show it).
"""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
NO_BROWSER = r"C:\Windows\System32\where.exe" if os.name == "nt" else "/bin/true"
TOOLS = ["Artifact", "SendFeedback", "Workflow", "ScheduleWakeup", "ReportFindings", "ReadNotifications"]
# The duplicate plan 25 §1 found; plan 28 names both ids. The one kept is `nirvana_superpowers::DEFAULT_ID`.
DUPLICATE = "superpowers@claude-plugins-official"
MAX_RUNS = 14
# What `nirvana_lean::DISALLOWED` holds after the first ten readings.
TAKEN = ["Workflow", "ScheduleWakeup", "ReportFindings"]
# The tools Nirvana Claude needs (plan 25 §3.3), as a -p session's init names them: the Agent tool is `Task` there,
# and `AskUserQuestion` is an interactive tool a -p session does not list.
NEEDED = ["Bash", "Read", "Edit", "Write", "Grep", "Glob", "Task", "Skill"]


def child_env(home: Path) -> dict:
    # A clean child: no variable of this session's own Claude, herdr pane or Nirvana home leaks in. The owner's
    # settings re-apply their own `env` inside the child.
    env = {
        k: v
        for k, v in os.environ.items()
        if not k.upper().startswith(("CLAUDE", "HERDR_", "NIRVANA_"))
    }
    env["BROWSER"] = NO_BROWSER
    env["NIRVANA_HERDR_HOME"] = str(home)
    return env


def user_skills() -> list:
    # Names only: the folders under the owner's user skills, which `skillOverrides` can switch one by one (plan 13's
    # proven switch). A plugin's skills have no switch of their own.
    base = Path.home() / ".claude" / "skills"
    return sorted(p.name for p in base.iterdir() if (p / "SKILL.md").is_file()) if base.is_dir() else []


def argv(home: Path, settings: Path, extra: list) -> list:
    plugin = home / "nirvana" / "claude"
    claude = shutil.which("claude")
    if not claude:
        sys.exit("claude is not on PATH")
    return [
        claude,
        "-p",
        "ok",
        "--output-format",
        "json",
        "--plugin-dir",
        str(plugin),
        "--agent",
        "nirvana",
        "--model",
        "haiku",
        "--effort",
        "medium",
        "--append-system-prompt-file",
        str(plugin / "ROLE.md"),
        "--name",
        "Nirvana",
        "--settings",
        str(settings),
        "--chrome",
        *extra,
    ]


def main() -> int:
    out = Path(sys.argv[1]).resolve()
    out.mkdir(parents=True, exist_ok=True)
    home = out / "home"
    if not (home / "nirvana" / "claude").is_dir():
        shutil.copytree(ROOT / "nirvana" / "claude", home / "nirvana" / "claude")
    base = home / "nirvana" / "claude" / "settings.json"
    base_value = json.loads(base.read_text(encoding="utf-8"))

    def settings_with(name: str, extra: dict) -> Path:
        path = home / "nirvana" / "launch" / f"{name}.settings.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(json.dumps({**base_value, **extra}, indent=2).encode("utf-8"))
        return path

    variants = [("today", base, [])]
    variants += [(f"no-{tool}", base, ["--disallowedTools", tool]) for tool in TOOLS]
    variants.append(("no-six", base, ["--disallowedTools", ",".join(TOOLS)]))
    variants.append(
        ("superpowers-once", settings_with("superpowers-once", {"enabledPlugins": {DUPLICATE: False}}), [])
    )
    skills = user_skills()
    variants.append(
        ("user-skills-off", settings_with("user-skills-off", {"skillOverrides": {s: "off" for s in skills}}), [])
    )
    # The cuts the first ten readings passed (each at least 500 below today), together, as `launch_args` now passes
    # them: one comma-joined value.
    variants.append(("taken", base, ["--disallowedTools", ",".join(TAKEN)]))
    assert len(variants) <= MAX_RUNS
    rows = []
    for name, settings, extra in variants:
        cwd = out / name
        cmd = argv(home, settings, extra)
        if (cwd / "stdout.json").is_file():
            # A run already made is read again, never repeated: the budget is one run per variant. (The first
            # pass's parser expected one object; Claude Code 2.1.310 prints the list of messages.)
            stdout, code = (cwd / "stdout.json").read_bytes(), 0
        else:
            cwd.mkdir(exist_ok=True)
            proc = subprocess.run(
                cmd,
                cwd=cwd,
                env=child_env(home),
                stdin=subprocess.DEVNULL,
                capture_output=True,
                timeout=300,
            )
            (cwd / "argv.json").write_text(json.dumps(cmd[1:], indent=1), encoding="utf-8")
            (cwd / "stdout.json").write_bytes(proc.stdout)
            (cwd / "stderr.txt").write_bytes(proc.stderr)
            stdout, code = proc.stdout, proc.returncode
        try:
            messages = json.loads(stdout.decode("utf-8"))
        except ValueError:
            rows.append({"variant": name, "exit": code, "error": "stdout is not JSON"})
            print(json.dumps(rows[-1]), flush=True)
            continue
        messages = messages if isinstance(messages, list) else [messages]
        result = next((m for m in messages if m.get("type") == "result"), {})
        init = next((m for m in messages if m.get("subtype") == "init"), {})
        tools = init.get("tools", [])
        usage = result.get("usage", {})
        reading = sum(
            usage.get(key, 0)
            for key in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")
        )
        rows.append(
            {
                "variant": name,
                "exit": code,
                "is_error": result.get("is_error"),
                "num_turns": result.get("num_turns"),
                "models": sorted(result.get("modelUsage", {})),
                "input": usage.get("input_tokens"),
                "cache_creation": usage.get("cache_creation_input_tokens"),
                "cache_read": usage.get("cache_read_input_tokens"),
                "reading": reading,
                # Whether each tool the variants cut is in the session's own tool list at all.
                "tools_present": {tool: tool in tools for tool in TOOLS},
                "tool_count": len(tools),
                "needed_missing": [tool for tool in NEEDED if tool not in tools],
                "result": str(result.get("result", ""))[:60],
            }
        )
        print(json.dumps(rows[-1]), flush=True)
    today = next((r["reading"] for r in rows if r["variant"] == "today" and "reading" in r), None)
    for row in rows:
        if today is not None and "reading" in row:
            row["saving"] = today - row["reading"]
    (out / "readings.json").write_text(json.dumps({"user_skills": skills, "rows": rows}, indent=1), encoding="utf-8")
    print(json.dumps(rows, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
