"""Plan 41 P41c: the agy orchestrator's copy of the Nirvana plugin, built from `nirvana/claude/` at every install.

    python nirvana/agy_plugin.py --home <dir>               # build <dir>/nirvana/agy/nirvana, validate, print
    python nirvana/agy_plugin.py --home <dir> --no-bladebro # the same with no mcp_config.json

Owner ruling R1, changed to (b) on 2026-11-14 (plan 41 §7): P41a found that interactive `agy -i` loads no hooks from
an `--add-dir` folder. So the built folder is a native agy plugin (`plugin.json` at its root) and `install.py` passes
it to `agy plugin install <folder>`, which copies it into agy's config under `~/.gemini`. The owner approved that
write for this installer step only, and this module never writes there itself. The CLI's `main` builds and validates
the folder; it does not install it into agy.

Built, never hand-kept: every part comes from the Claude plugin's own files, so the two cannot drift. A Claude file
this module cannot read the way it expects (a hook command that is not the installed herdr, a tool name with no agy
row, a skill over agy's size limit) fails the build instead of shipping a half translation.

What goes where (agy 1.5.9's built-in docs, read out of agy.exe on 2026-11-14):

- `plugin.json`: `{"name": "nirvana", "version": <nirvana/VERSION>}`, the marker that makes the folder a native
  plugin. agy does no extension staging for a native plugin, so every part below sits at the plugin root.
- `hooks.json`: agy's named-hook format. Each top-level key is a hook name holding events; SessionStart,
  PreInvocation and Stop are flat lists of handlers, PreToolUse and PostToolUse are groups of `matcher` + `hooks`.
  A handler is `{"type": "command", "command": <string>, "timeout": <s>}`, run by `cmd /c` on Windows with the folder
  holding `hooks.json` as its cwd. Each command is the installed herdr with the Claude hook's args and `--host agy`
  (P41b's adapter; until it lands herdr refuses the flag and the hook fails, which agy logs and moves past).
- `skills/<name>/SKILL.md`: the same format as Claude's; agy drops one whose frontmatter `name` is not its folder.
- No `rules/`. `ROLE.md` is the body of `agents/nirvana.md`, so only a session started with `--agent nirvana` gets it.
  The plugin is installed into agy's config, where a rule would reach every agy session on the machine, the seated
  agy workers and the owner's own agy included (P41c scope ruling, 2026-11-14).
- `agents/<name>.md`: the orchestrator agent only, as agy's main agent; see `AGENTS` for why the workers stay out.
- `mcp_config.json`: bladebro, decision 0043's recipe as `src/nirvana_bladebro.rs::definition` writes it for agy
  workers, with the orchestrator's own `BLADE_HOME`. Written only when bladebro is on PATH (0043 item 6).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import windows_shell  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "nirvana" / "claude"

# The built plugin is `<home>/nirvana/agy/<NAME>`. The manifest `name` and the folder's own name are the same word
# because agy keys an installed plugin on its install directory name (its plugins guide: "`dir` is the identifier
# that matters ... It can differ from `name` in `plugin.json`"). With both fixed, each install replaces the last one
# instead of adding a second plugin. While R1 was (a) the folder was `.agents`, the name agy reads inside an add-dir.
AGY_DIR = Path("nirvana") / "agy"
NAME = "nirvana"
MANIFEST = "plugin.json"
# A limit for one `agy plugin` call. The baseline test run did two validates in under a second, so two minutes
# means agy has hung.
AGY_TIMEOUT_S = 120

# The flag P41b's adapter takes on every hook command. Spelling fixed by the plan 41 brief; feat/p41-hooks owns it.
HOST_FLAG = ["--host", "agy"]

# How the Claude plugin names the installed herdr, in hooks.json and in the skills' text. Anything else is drift.
CLAUDE_EXE = "${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"
CLAUDE_EXE_QUOTED = f'"{CLAUDE_EXE}"'

# Claude event -> agy event. SubagentStop has no agy event (plan 41 §5.1): the agents panel already reads seated
# orders, and agy's own subagents go unseen.
EVENTS = {
    "SessionStart": "SessionStart",
    "UserPromptSubmit": "PreInvocation",
    "PreToolUse": "PreToolUse",
    "PostToolUse": "PostToolUse",
    "Stop": "Stop",
    "SubagentStop": None,
}
GROUPED = {"PreToolUse", "PostToolUse"}   # agy events that take `matcher` groups; the rest are flat lists

# The one table of Claude tool names to agy hook tool names. agy's docs: a hook's tool name is the step type,
# lowercased, without `CORTEX_STEP_TYPE_`; the CLI's own tools are `third_party/oneharness/internal/tools/*.go` in
# agy.exe (read 2026-11-14: read_file, write_file, edit_file, run_command, invoke_subagent, define_subagent,
# manage_subagents, ...). The IDE agent's names (view_file, write_to_file, replace_file_content,
# multi_replace_file_content) are in the binary too. Measured in hook payloads (`toolCall.name`, agy 1.5.9 `-p`,
# P41c live check 2026-11-14): `write_file` for a new file, `edit_file` for a change, `read_file`, `run_command`.
# `invoke_subagent` is in the tool list agy gave the nirvana agent but was never called. The IDE names and
# `edit_notebook` are read off the binary only: P41a confirms them in the interactive mode, and a name agy never
# sends only costs a regex branch that never matches.
TOOLS = {
    "Edit": ["write_file", "edit_file", "replace_file_content", "multi_replace_file_content", "write_to_file"],
    "Write": ["write_file", "edit_file", "replace_file_content", "multi_replace_file_content", "write_to_file"],
    "MultiEdit": ["write_file", "edit_file", "replace_file_content", "multi_replace_file_content", "write_to_file"],
    "NotebookEdit": ["edit_notebook"],     # a step type in the binary; no CLI tool file names it
    "Read": ["read_file", "view_file"],
    "Bash": ["run_command"],
    "Agent": ["invoke_subagent"],          # P41a: `define_subagent` and `manage_subagents` exist too
}

# Claude agent -> what agy gets. Only the orchestrator maps. Each worker file fixes a Claude model and effort and
# its body says so ("Your model and effort are fixed in this file"); agy's own import guide drops `model`, so on agy
# the worker would run on the session's Gemini model under a name that says Opus. Seating stays `herdr seat run`
# (plan 41 §3: seating carries over because it is a command).
AGENTS = {"nirvana": "main"}
# An agy agent with no `tools` gets a small read-only set (agy's import guide), so the orchestrator names its tools:
# the guide's full set, plus the subagent tool so it can still hand a short read to an agy subagent.
MAIN_TOOLS = ["read_file", "edit_file", "write_file", "run_command", "manage_task", "search_web", "invoke_subagent"]
# Claude's `nirvana.md` has no body (in Claude the role comes from `ROLE.md`). Measured 2026-11-14 in the first P41c
# live check: with an empty body, `agy --agent nirvana` listed agy's default tools (swarm, run_workflow,
# define_subagent, ...), so the agent did not take. With a one-line body, the same launch listed exactly `MAIN_TOOLS`
# and answered from the body. So the agy body is that line, then `ROLE.md`. The body may not open with an H1: agy's
# 1.1.6 note calls Markdown agents "H1-delimited", and the one run whose body opened with `# Nirvana` sent no model
# call in 180 s (0 tokens). `demote_h1` also turns ROLE.md's own H1s into H2s.
MAIN_BODY = ("You are Nirvana's orchestrator on the agy seat. Your role follows. It was written for the Claude seat; "
             "on agy it is the same role, with agy's own tool names.\n\n")

ROLE = "ROLE.md"
# agy states no limit for an agent body. This is the 24 KB it gives a rule, so the role is no bigger here than it
# could have been as a rule.
BODY_CAP = 24 * 1024
SKILL_CAP = 24 * 1024     # the plan 41 brief: SKILL.md body limit 24 KB

# The orchestrator's bladebro home: outside `nirvana/agy`, which every install replaces whole, and never `~/.blade`
# or the owner's Chrome profile (0043). One home for the one orchestrator session, reused, so nothing leaks per run.
BLADE_HOME = Path("nirvana") / "blade-orchestrator"
BLADE_IDLE_TIMEOUT_S = 60   # src/nirvana_bladebro.rs IDLE_TIMEOUT_S


class BuildError(ValueError):
    """The Claude plugin holds something this build cannot translate; nothing is installed."""


def herdr(home: Path) -> Path:
    return windows_shell.herdr_exe(home)


def command(home: Path, args: list[str]) -> str:
    """One `cmd /c` command line. `list2cmdline` quotes the exe only when its path has a space: with exactly two
    quotes around an existing program `cmd /c` keeps them, and with none it has nothing to strip."""
    return subprocess.list2cmdline([str(herdr(home)), *args, *HOST_FLAG])


def matcher(claude: str) -> str:
    names: list[str] = []
    for tool in claude.split("|"):
        if tool not in TOOLS:
            raise BuildError(f"hooks.json matcher {claude!r}: no agy row for Claude tool {tool!r} in agy_plugin.TOOLS")
        for name in TOOLS.get(tool, ()):
            if name not in names:
                names.append(name)
    return "|".join(names)


def handler(home: Path, hook: dict) -> dict:
    if hook.get("type", "command") != "command" or hook.get("command") != CLAUDE_EXE:
        raise BuildError(f"hooks.json: a hook that is not the installed herdr: {hook!r}")
    return {"type": "command", "command": command(home, list(hook.get("args", []))),
            "timeout": int(hook.get("timeout", 30))}


def hooks(claude: dict, home: Path) -> tuple[dict, list[dict], list[str]]:
    """agy's hooks.json from Claude's, the rows of the mapping, and what was skipped. One named hook per Claude
    event (`nirvana-<event>`), so `"enabled": false` can switch one event off without the others."""
    out: dict = {}
    rows: list[dict] = []
    skipped: list[str] = []
    for event, groups in claude["hooks"].items():
        if event not in EVENTS:
            raise BuildError(f"hooks.json: Claude event {event!r} has no row in agy_plugin.EVENTS")
        agy = EVENTS[event]
        for group in groups:
            for hook in group["hooks"]:
                args = " ".join(hook.get("args", []))
                if agy is None:
                    skipped.append(f"hook {event} `{args}`: agy has no {event} event")
                    continue
                rows.append({"claude": event, "matcher": group.get("matcher"), "args": args, "agy": agy,
                             "agy_matcher": matcher(group["matcher"]) if agy in GROUPED else None})
        if agy is None:
            continue
        spec = out.setdefault(f"nirvana-{agy.lower()}", {})
        if agy in GROUPED:
            spec[agy] = [{"matcher": matcher(g["matcher"]), "hooks": [handler(home, h) for h in g["hooks"]]}
                         for g in groups]
        else:
            # A Claude group with no matcher is a flat list on agy; its handlers keep their order.
            spec.setdefault(agy, []).extend(handler(home, h) for g in groups for h in g["hooks"])
    return out, rows, skipped


def frontmatter(text: str) -> tuple[dict, str]:
    """The `key: value` lines between the opening `---` pair, unquoted, and the body after. Enough for the plugin's
    own files, which hold one line per key; a file without the pair has none."""
    m = re.match(r"---\r?\n(.*?)\r?\n---\r?\n?", text, re.S)
    if not m:
        return {}, text
    fields = {}
    for line in m.group(1).splitlines():
        key, sep, value = line.partition(":")
        if sep:
            value = value.strip()
            if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
                value = value[1:-1]
            fields[key.strip()] = value
    return fields, text[m.end():]


def demote_h1(text: str) -> str:
    """Every `# ` heading outside a code fence becomes `## `, so no H1 can split or end the agent's prompt."""
    out, fenced = [], False
    for line in text.split("\n"):
        if line.lstrip().startswith("```"):
            fenced = not fenced
        elif not fenced and line.startswith("# "):
            line = "#" + line
        out.append(line)
    return "\n".join(out)


def herdr_text(text: str, home: Path) -> str:
    """The Claude plugin's `"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"` as the installed path: agy has no such
    variable, and a `herdr` on PATH may be upstream herdr."""
    return text.replace(CLAUDE_EXE_QUOTED, f'"{herdr(home).as_posix()}"')


def write(path: Path, text: str) -> None:
    """Bytes, so Windows writes no CRLF (`.gitattributes` pins LF; memory note write-text-crlf-hollows-controls)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def bladebro(home: Path) -> dict:
    """`src/nirvana_bladebro.rs::definition` in agy's `mcp_config.json` shape (`nirvana_agy_tools::translate`
    passes command, args and env). No `cwd`: the orchestrator has no one project folder, and bladebro needs none."""
    blade = (Path(home) / BLADE_HOME).as_posix()
    env = {
        "BLADE_HOME": blade,
        "BLADE_CHROME_FLAGS": "--headless=new",
        "BLADE_LANE": "agent",
        "BLADE_IDLE_TIMEOUT": str(BLADE_IDLE_TIMEOUT_S),
        "BLADE_NO_UPDATE_CHECK": "1",
        "BLADE_NO_WARMING": "1",
        "BLADE_PLAIN": "1",
    }
    command_, args = ("cmd", ["/c", "bladebro", "mcp"]) if os.name == "nt" else ("bladebro", ["mcp"])
    return {"mcpServers": {"bladebro": {"command": command_, "args": args, "env": env}}}


def plugin_version() -> str:
    """The fork's version, so `agy plugin list` shows which build the installed plugin came from."""
    return (ROOT / "nirvana" / "VERSION").read_text(encoding="utf-8").strip()


def manifest(version: str) -> dict:
    """agy's native-plugin marker. A name and a version is the smallest manifest `--strict` passes: without the
    version it warns that the marketplace cannot list it (measured 2026-11-14, agy 1.5.9). There is no timestamp or
    path in it, so a rebuild of the same version writes the same bytes."""
    return {"name": NAME, "version": version}


def build(out: Path, home: Path, src: Path | None = None, with_bladebro: bool | None = None,
          version: str | None = None) -> dict:
    """Writes the plugin tree into `out` (which must not exist) and returns the report: the hook rows, the files
    written and what was skipped. `with_bladebro` None asks PATH; `version` None reads `nirvana/VERSION`."""
    src = SRC if src is None else Path(src)
    out, home = Path(out), Path(home)
    if with_bladebro is None:
        with_bladebro = shutil.which("bladebro") is not None
    report: dict = {"hooks": [], "skills": [], "agents": [], "mcp": [], "skipped": []}

    # agy's plugins guide: a native plugin is "any directory containing `plugin.json`" and "**must** place
    # `hooks.json` directly at the plugin root"; skills/, agents/ and mcp_config.json sit beside it.
    write(out / MANIFEST, json.dumps(manifest(plugin_version() if version is None else version)) + "\n")

    claude = json.loads((src / "hooks" / "hooks.json").read_bytes())
    agy_hooks, report["hooks"], skipped = hooks(claude, home)
    report["skipped"] += skipped
    write(out / "hooks.json", json.dumps(agy_hooks, indent=2) + "\n")

    for skill in sorted(p for p in (src / "skills").iterdir() if p.is_dir()):
        main = skill / "SKILL.md"
        fields, _ = frontmatter(main.read_text(encoding="utf-8"))
        if fields.get("name") != skill.name or not fields.get("description"):
            raise BuildError(f"{main}: agy drops a skill whose frontmatter name is not its folder or has no "
                             f"description (name {fields.get('name')!r})")
        for path in sorted(p for p in skill.rglob("*") if p.is_file()):
            rel = path.relative_to(src)
            if path.suffix == ".md":
                text = herdr_text(path.read_text(encoding="utf-8"), home)
                if path.name == "SKILL.md" and len(text.encode("utf-8")) > SKILL_CAP:
                    raise BuildError(f"{path}: {len(text.encode('utf-8'))} bytes, over agy's {SKILL_CAP}")
                write(out / rel, text)
            else:
                (out / rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, out / rel)
        report["skills"].append(skill.name)

    role = demote_h1(herdr_text((src / ROLE).read_text(encoding="utf-8"), home))

    for agent in sorted((src / "agents").glob("*.md")):
        fields, body = frontmatter(agent.read_text(encoding="utf-8"))
        name = fields.get("name") or agent.stem
        if AGENTS.get(name) != "main":
            report["skipped"].append(f"agent {name}: a Claude worker with a fixed Claude model; on agy it would run "
                                     "on the session's model, so seat it through `herdr seat run`")
            continue
        head = [f"name: {name}", f"description: {json.dumps(fields.get('description', ''))}",
                "mainAgent: true", "subagent: false", f"tools: [{', '.join(MAIN_TOOLS)}]"]
        # `model`, `effort` and `color` are Claude's: the agy seat's model is the launch's `--model` (R3).
        body = (demote_h1(herdr_text(body, home)).rstrip("\n") + "\n\n" if body.strip() else MAIN_BODY) + role
        if len(body.encode("utf-8")) > BODY_CAP:
            raise BuildError(f"{agent} with {src / ROLE}: {len(body.encode('utf-8'))} bytes of body, over {BODY_CAP}")
        text = "---\n" + "\n".join(head) + "\n---\n" + body
        write(out / "agents" / agent.name, text)
        report["agents"].append(name)
        dropped = [key for key in fields if key in ("model", "effort", "color")]
        if dropped:
            report["skipped"].append(f"agent {name}: fields {', '.join(dropped)} dropped (Claude's)")

    for extra in ("settings.json",):
        if (src / extra).exists():
            report["skipped"].append(f"{extra}: Claude Code settings (statusline, theme); agy takes its own")
    if with_bladebro:
        write(out / "mcp_config.json", json.dumps(bladebro(home), indent=2) + "\n")
        report["mcp"].append("bladebro")
    else:
        report["skipped"].append("mcp bladebro: not on PATH, so no mcp_config.json (0043 item 6)")
    return report


def agy_run(agy: str, args: list[str]) -> subprocess.CompletedProcess:
    """One `agy` call with its output read back. Stdin is an empty pipe, not NUL: on Windows NUL reads as a terminal
    (memory windows-nul-isatty-true), so a prompt reading it would hang the install. NO_COLOR keeps colour codes out
    of the output we parse; agy's changelog fixed colour in piped output for these commands twice."""
    return subprocess.run([agy, *args], input="", capture_output=True, text=True, encoding="utf-8",
                          errors="replace", timeout=AGY_TIMEOUT_S, env={**os.environ, "NO_COLOR": "1"})


def validate(plugin: Path, agy: str | None = None) -> dict:
    """`agy plugin validate <plugin> --strict --json` on the built folder itself, so the folder that is installed is
    the one that was validated. Measured 2026-11-14 on agy 1.5.9: it reports a rule with no valid trigger, a skill
    named unlike its folder, an unknown hook event and bad agent YAML. Returns `{"ok": None, ...}` when agy is not on
    PATH: the folder was not validated, the report says so, and it never counts as a pass."""
    agy = agy or shutil.which("agy")
    if agy is None:
        return {"ok": None, "why": "agy is not on PATH, so the folder was not validated", "findings": []}
    done = agy_run(agy, ["plugin", "validate", str(plugin), "--strict", "--json"])
    try:
        parsed = json.loads(done.stdout)
    except ValueError:
        return {"ok": False, "why": f"agy plugin validate printed no JSON (exit {done.returncode}): "
                                    f"{(done.stdout + done.stderr).strip()[:400]}", "findings": []}
    # `--strict` exits non-zero on any error or warning; a note (no logo) is not a finding.
    findings = [f for f in parsed.get("findings", []) if f.get("severity") != "note"]
    return {"ok": done.returncode == 0, "why": f"exit {done.returncode}",
            "findings": findings, "components": parsed.get("components", {})}


# agy 1.5.9's install summary, read out of agy.exe: `Installed plugin %q from %s` (or `... from %q, replacing the %q
# plugin that was already installed`), then `Installed to %s`, plus ` (replaced existing installation)` when it
# overwrote an earlier copy. `%q` is Go's quoting, so the name is in double quotes.
_INSTALLED_NAME = re.compile(r'Installed plugin "((?:[^"\\]|\\.)*)"')
_INSTALLED_TO = re.compile(r"^[ \t]*Installed to (.+?)([ \t]+\(replaced existing installation\))?[ \t]*$", re.M)


def temp_root() -> Path:
    return Path(tempfile.gettempdir())


def _under(path, root) -> bool:
    try:
        Path(path).resolve().relative_to(Path(root).resolve())
        return True
    except ValueError:
        return False


def agy_install(plugin: Path, agy: str | None = None) -> dict:
    """`agy plugin install <plugin>`. agy copies the folder into its config and swaps it in whole; if the install
    fails it keeps its previous copy (its 1.5.x changelog). Returns `ok` True with the name and folder agy printed,
    False with agy's own error, or None when agy is not on PATH. `argv` is the command line, for the report.

    A real agy is never handed a plugin from under the temp folder. P41c found `test_scripts.py` running
    `install.main` into a temp home with agy on PATH. Before R1 (b) that ran only the validator; now it would put a
    `nirvana` plugin into the owner's `~/.gemini`, enabled in every agy session, with hooks naming a herdr.exe that
    is deleted with the folder. The test case is a temp plugin with an agy outside temp; a fake agy in temp passes."""
    agy = agy or shutil.which("agy")
    if agy is None:
        return {"ok": None, "why": "agy is not on PATH, so the plugin was not installed into agy", "argv": None}
    if _under(plugin, temp_root()) and not _under(agy, temp_root()):
        return {"ok": None, "why": f"the plugin is under the temp folder and agy ({agy}) is not; a test or a check, "
                                   "so it was not installed into agy", "argv": None}
    argv = [agy, "plugin", "install", str(plugin)]
    try:
        done = agy_run(agy, argv[1:])
    except (OSError, subprocess.TimeoutExpired) as err:
        return {"ok": False, "why": f"agy plugin install did not finish: {err}", "argv": argv}
    out = "\n".join(s.strip() for s in (done.stdout, done.stderr) if s.strip())
    if done.returncode != 0:
        # agy prints `Error: <reason>` and exits 1, seen 2026-11-14: "Error: install target must be a directory".
        return {"ok": False, "why": f"agy plugin install exit {done.returncode}: {out[-600:] or 'no output'}",
                "argv": argv, "output": out}
    name = _INSTALLED_NAME.search(out)
    folder = _INSTALLED_TO.search(out)
    return {"ok": True, "why": f"exit {done.returncode}", "argv": argv, "output": out,
            "name": name.group(1) if name else None, "folder": folder.group(1) if folder else None,
            "replaced": bool(folder and folder.group(2))}


class AgyInstallError(RuntimeError):
    """The plugin was built and validated and is in the install folder, but `agy plugin install` failed, so agy
    keeps the copy it had. Carries the report so the caller can still print it."""

    def __init__(self, why: str, dst: Path, report: dict):
        super().__init__(why)
        self.dst, self.report = dst, report


def install(home: Path, src: Path | None = None, with_bladebro: bool | None = None,
            check=validate, register=agy_install) -> tuple[Path, dict]:
    """Builds beside `<home>/nirvana/agy` and validates the build (plan 41 §5.2: `agy plugin validate --strict` gates
    it). Then it swaps the build in with the Claude plugin's own replace and passes `<home>/nirvana/agy/nirvana` to
    `agy plugin install` (R1 (b)). A failed validation raises `BuildError` before anything moves, so both the folder
    and agy's copy stay as they were. A failed agy install raises `AgyInstallError` after the swap. With agy not on
    PATH nothing is validated or installed into agy, and the report says so. If the swap left files a running
    program holds, `windows_shell.StaleFiles` comes up and agy is not asked: the folder it would copy is mixed."""
    home = Path(home)
    dst = home / AGY_DIR
    stage = dst.with_name(f"{dst.name}.build-{os.getpid()}")
    windows_shell._remove_quietly(stage)
    try:
        report = build(stage / NAME, home, src, with_bladebro)
        report["validate"] = check(stage / NAME)
        if report["validate"]["ok"] is False:
            raise BuildError(f"agy plugin validate --strict failed ({report['validate']['why']}): "
                             f"{report['validate']['findings']}")
        windows_shell._replace_tree(stage, dst)
    finally:
        windows_shell._remove_quietly(stage)
    # agy copies from the installed folder, not the stage: the stage is deleted by now, and this way agy is given
    # the same source path at every install.
    if report["validate"]["ok"] is None:
        report["agy_install"] = {"ok": None, "why": "agy is not on PATH, so the plugin was not installed into agy",
                                 "argv": None}
    else:
        report["agy_install"] = register(dst / NAME)
    if report["agy_install"]["ok"] is False:
        raise AgyInstallError(report["agy_install"]["why"], dst, report)
    return dst, report


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="agy_plugin")
    ap.add_argument("--home", required=True, help="the install root; the folder lands at <home>/nirvana/agy")
    ap.add_argument("--no-bladebro", action="store_true")
    ns = ap.parse_args(argv)
    # Build and validate only. Installing into agy's config is install.py's step, and the approval covers that step.
    skipped = {"ok": None, "why": "agy_plugin's own CLI does not install into agy; install.py does", "argv": None}
    dst, report = install(Path(ns.home), with_bladebro=False if ns.no_bladebro else None,
                          register=lambda plugin: skipped)
    print(json.dumps({"dir": str(dst), **report}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
