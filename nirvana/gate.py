"""The one way to run the scoped gates (owner ruling 2026-11-05: gates scoped to the changed modules and their controls).

    python nirvana/gate.py nirvana_spaces nirvana_remote            # nextest, one run per module filter
    python nirvana/gate.py nirvana_spaces --clippy --controls c40e_label_family c40e_empty_upload

Each run gets a private short TEMP, `%LOCALAPPDATA%\\Temp\\ng-<pid>`, as TEMP and TMP for every step, and removes it
when the steps end, passed, failed or interrupted. Plan 40, measured 2026-11-09: 35,326 `herdr-*`/`nirvana-*` folders in
%TEMP%, one per test per run, because the tests make folders under `std::env::temp_dir()` and never remove them; a
private TEMP from one package held 2,025 more. A run that was killed before its cleanup leaves its `ng-<pid>`, which a
later run removes once it is a day old and its pid is gone.

One line per step, its exit code and the names of the tests that failed; the full output of each step goes to
`target/gate/<n>-<step>.log`. `windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override` fails on
master (RULES.md): it is reported on its own line and does not make the exit code non-zero. Exit 0 when every step
passed, else 1.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import clean_temp  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
ALLOWED = frozenset({"windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override"})
STALE_S = 24 * 3600
PRIVATE = re.compile(r"^ng-(\d{1,7})$")
# A nextest result line: "        FAIL [   0.012s] (3/9) herdr::bin/herdr nirvana_spaces::tests::x". The test is the
# last word; the binary may or may not be printed and the (n/m) counter is newer nextest only.
NEXTEST_FAIL = re.compile(r"^\s*(?:FAIL|TIMEOUT|SIGSEGV|SIGABRT|ABORT|LEAK-FAIL)\s+\[[^\]]*\]\s.*?(\S+)\s*$")


@dataclass
class Step:
    label: str
    argv: list[str]
    kind: str            # "nextest", "clippy" or "control"


@dataclass
class Result:
    step: Step
    code: int
    failed: list[str] = field(default_factory=list)
    allowed: list[str] = field(default_factory=list)
    log: Path | None = None

    @property
    def ok(self) -> bool:
        return self.code == 0 or (bool(self.allowed) and not self.failed)


def temp_base(env: dict | None = None) -> Path:
    env = os.environ if env is None else env
    base = env.get("LOCALAPPDATA") or str(Path.home() / "AppData" / "Local")
    return Path(base) / "Temp"


def steps(filters: list[str], clippy: bool, controls: list[str]) -> list[Step]:
    out = [Step(f"nextest {f}", ["cargo", "nextest", "run", "--no-fail-fast", "-E", f"test(/{f}/)"], "nextest")
           for f in filters]
    if clippy:
        out.append(Step("clippy", ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"], "clippy"))
    out.extend(Step(f"control {name}", [sys.executable, str(ROOT / "nirvana" / "controls.py"), "--only", name],
                    "control") for name in controls)
    return out


def failed_tests(text: str) -> list[str]:
    """The tests a nextest run printed as failed, each once, in order."""
    seen: list[str] = []
    for line in text.splitlines():
        m = NEXTEST_FAIL.match(line)
        if m and m[1] not in seen:
            seen.append(m[1])
    return seen


def judge(step: Step, code: int, text: str) -> Result:
    result = Result(step, code)
    if step.kind == "nextest":
        names = failed_tests(text)
        # A name is the full path (`module::tests::name`); the allowed one is matched by its last segment.
        result.allowed = [n for n in names if n.rsplit("::", 1)[-1] in ALLOWED]
        result.failed = [n for n in names if n not in result.allowed]
        if code != 0 and not names:
            result.failed = ["(no test named: a build error or a crash; see the log)"]
    elif step.kind == "clippy" and code != 0:
        errors = sum(1 for line in text.splitlines() if line.startswith("error"))
        result.failed = [f"{errors} error lines"]
    elif step.kind == "control" and code != 0:
        result.failed = [line for line in text.splitlines() if line.startswith(("FAIL", "unknown"))][:3] or \
            [f"exit {code}"]
    return result


def run_child(argv: list[str], env: dict, cwd: Path) -> tuple[int, str]:
    done = subprocess.run(argv, cwd=str(cwd), env=env, capture_output=True, text=True, encoding="utf-8",
                          errors="replace")
    return done.returncode, (done.stdout or "") + "\n" + (done.stderr or "")


def sweep_stale(base: Path, now: float, alive=clean_temp.pid_alive) -> list[str]:
    """Removes `ng-<pid>` folders a killed run left: a day old and the pid gone. A line each."""
    lines: list[str] = []
    if not base.is_dir():
        return lines
    for entry in os.scandir(base):
        m = PRIVATE.match(entry.name)
        if not m or clean_temp.is_link(entry.path) or not entry.is_dir(follow_symlinks=False):
            continue
        try:
            age = now - entry.stat(follow_symlinks=False).st_mtime
        except OSError:
            continue
        if age < STALE_S or alive(int(m[1])):
            continue
        failures = clean_temp.remove_tree(entry.path)
        lines.append(f"removed a stale {entry.name}" if not failures
                     else f"stale {entry.name}: {len(failures)} entries left, first {failures[0]}")
    return lines


def line_for(r: Result) -> str:
    verdict = "PASS" if r.ok else "FAIL"
    text = f"{verdict}  {r.step.label}: exit {r.code}"
    if r.failed:
        text += f"; failed: {', '.join(r.failed)}"
    if not r.ok and r.log is not None:
        text += f"; log {r.log}"
    return text


def run(filters: list[str], clippy: bool, controls: list[str], *, runner=run_child, base: Path | None = None,
        root: Path = ROOT, pid: int | None = None, now: float | None = None, alive=clean_temp.pid_alive,
        out=print) -> int:
    base = temp_base() if base is None else Path(base)
    pid = os.getpid() if pid is None else pid
    now = time.time() if now is None else now
    for line in sweep_stale(base, now, alive):
        out(line)
    private = base / f"ng-{pid}"
    private.mkdir(parents=True, exist_ok=True)
    logs = root / "target" / "gate"
    results: list[Result] = []
    try:
        env = dict(os.environ, TEMP=str(private), TMP=str(private), PYTHONIOENCODING="utf-8")
        for n, step in enumerate(steps(filters, clippy, controls), 1):
            code, text = runner(step.argv, env, root)
            result = judge(step, code, text)
            try:
                logs.mkdir(parents=True, exist_ok=True)
                slug = re.sub(r"[^A-Za-z0-9_.-]+", "-", step.label)
                result.log = logs / f"{n}-{slug}.log"
                result.log.write_bytes(text.encode("utf-8"))
            except OSError:
                result.log = None
            results.append(result)
            out(line_for(result))
            for name in result.allowed:
                out(f"ALLOWED  {step.label}: {name} fails on master too (RULES.md); not counted")
    finally:
        failures = clean_temp.remove_tree(private) if private.is_dir() else []
        if failures:
            out(f"private TEMP {private}: {len(failures)} entries left, first {failures[0]}")
    bad = sum(not r.ok for r in results)
    out(f"gate: {len(results) - bad} of {len(results)} steps passed")
    return 1 if bad else 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="gate")
    ap.add_argument("filters", nargs="*", help="nextest module filters, each run as -E 'test(/<filter>/)'")
    ap.add_argument("--clippy", action="store_true")
    ap.add_argument("--controls", nargs="+", default=[], metavar="NAME", help="controls to run alone, by exact name")
    ns = ap.parse_args(argv)
    if not ns.filters and not ns.clippy and not ns.controls:
        ap.error("name a module filter, --clippy or --controls")
    try:
        return run(ns.filters, ns.clippy, [name.strip() for name in ns.controls])
    except KeyboardInterrupt:
        print("gate: interrupted; the private TEMP is removed")
        return 130


if __name__ == "__main__":
    sys.exit(main())
