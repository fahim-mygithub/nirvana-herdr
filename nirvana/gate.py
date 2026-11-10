"""The one way to run the scoped gates (owner ruling 2026-11-05: gates scoped to the changed modules and their controls).

    python nirvana/gate.py nirvana_spaces nirvana_remote            # nextest, one run per module filter
    python nirvana/gate.py nirvana_spaces --clippy --controls c40e_label_family c40e_empty_upload

Each run gets a private short TEMP, `%LOCALAPPDATA%\\Temp\\ng-<pid>-<random>` made by `tempfile.mkdtemp`, as TEMP and
TMP for every step, and removes it when the steps end, passed, failed or interrupted. On an interrupt the step's whole
process tree is ended first (a Windows job object; a killed gate takes its tree with it too), so no test binary still
writes into the folder being removed. Plan 40, measured 2026-11-09: 35,326 `herdr-*`/`nirvana-*` folders in
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
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import clean_temp  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
# The one test allowed to fail, by its module path's tail (review N3: by the last segment alone a same-named test in
# any other module was hidden too).
ALLOWED = frozenset({"plugins::tests::windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override"})
STALE_S = 24 * 3600
STEP_TIMEOUT_S = 90 * 60
# `ng-<pid>-<mkdtemp suffix>`, and the `ng-<pid>` the first version made.
PRIVATE = re.compile(r"^ng-(\d{1,7})(?:-[a-z0-9_]{8})?$")
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
        # A name is the full path (`module::tests::name`); the allowed one is matched by its path's tail.
        result.allowed = [n for n in names if any(n == a or n.endswith("::" + a) for a in ALLOWED)]
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


class _Tree:
    """The step's process and everything it starts, ended as one.

    Review N2: `subprocess.run` killed only `cargo` on an interrupt; `cargo-nextest` and the test binaries kept
    running with TEMP set to the folder the `finally` was emptying, and a killed gate (no `finally` at all) left them
    running for good. On Windows the child is started suspended, put in a job object that ends every process in it
    when its last handle closes (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`), then resumed, so no grandchild can start
    before it is in the job; the gate holds the only handle, so a gate that dies ends the tree too. Elsewhere the
    child leads a new process group. Never a kill by image name: only this step's own tree."""

    def __init__(self, argv: list[str], env: dict, cwd: Path):
        self.job = None
        kw: dict = {}
        if os.name == "nt":
            kw["creationflags"] = 0x00000004          # CREATE_SUSPENDED
        else:
            kw["start_new_session"] = True
        self.proc = subprocess.Popen(argv, cwd=str(cwd), env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace", **kw)
        if os.name == "nt":
            try:
                self.job = _job_for(self.proc)
            finally:
                _resume(self.proc)

    def end(self) -> None:
        """Ends the step's tree: the job, or `taskkill /T` on the child's own pid when no job could be made."""
        if os.name == "nt":
            if self.job is not None:
                _kernel().TerminateJobObject(self.job, 1)
            elif self.proc.poll() is None:
                subprocess.run(["taskkill", "/T", "/F", "/PID", str(self.proc.pid)], capture_output=True)
        else:
            import signal
            try:
                os.killpg(self.proc.pid, signal.SIGKILL)
            except OSError:
                pass

    def close(self) -> None:
        if self.job is not None:
            _kernel().CloseHandle(self.job)
            self.job = None


def _kernel():
    import ctypes
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateJobObjectW.restype = wintypes.HANDLE
    kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
    kernel.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
    kernel.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    kernel.TerminateJobObject.argtypes = [wintypes.HANDLE, wintypes.UINT]
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    return kernel


def _job_for(proc: subprocess.Popen):
    """A kill-on-close job object holding `proc`, or None when Windows refuses (then `taskkill /T` is the way)."""
    import ctypes
    from ctypes import wintypes

    class IO(ctypes.Structure):
        _fields_ = [(n, ctypes.c_ulonglong) for n in ("r", "w", "o", "rb", "wb", "ob")]

    class Basic(ctypes.Structure):
        _fields_ = [("per_process_time", ctypes.c_longlong), ("per_job_time", ctypes.c_longlong),
                    ("flags", wintypes.DWORD), ("min_ws", ctypes.c_size_t), ("max_ws", ctypes.c_size_t),
                    ("active", wintypes.DWORD), ("affinity", ctypes.c_size_t), ("priority", wintypes.DWORD),
                    ("sched", wintypes.DWORD)]

    class Extended(ctypes.Structure):
        _fields_ = [("basic", Basic), ("io", IO), ("process_memory", ctypes.c_size_t),
                    ("job_memory", ctypes.c_size_t), ("peak_process", ctypes.c_size_t),
                    ("peak_job", ctypes.c_size_t)]

    kernel = _kernel()
    job = kernel.CreateJobObjectW(None, None)
    if not job:
        return None
    info = Extended()
    info.basic.flags = 0x00002000                    # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
    ok = kernel.SetInformationJobObject(job, 9, ctypes.byref(info), ctypes.sizeof(info))  # 9: extended limits
    if ok:
        ok = kernel.AssignProcessToJobObject(job, int(proc._handle))
    if not ok:
        kernel.CloseHandle(job)
        return None
    return job


def _resume(proc: subprocess.Popen) -> None:
    import ctypes
    from ctypes import wintypes
    ntdll = ctypes.WinDLL("ntdll")
    ntdll.NtResumeProcess.argtypes = [wintypes.HANDLE]
    ntdll.NtResumeProcess(int(proc._handle))


def run_child(argv: list[str], env: dict, cwd: Path, timeout: float = STEP_TIMEOUT_S) -> tuple[int, str]:
    """The step's exit code and output. It waits in short slices so a Ctrl-C is seen at once; on an interrupt, or
    past `timeout`, the whole tree is ended before this returns or raises."""
    tree = _Tree(argv, env, cwd)
    deadline = time.monotonic() + timeout
    try:
        while True:
            try:
                out, err = tree.proc.communicate(timeout=0.5)
                break
            except subprocess.TimeoutExpired:
                if time.monotonic() > deadline:
                    tree.end()
                    out, err = tree.proc.communicate()
                    return 124, (out or "") + "\n" + (err or "") + f"\ngate: timed out after {timeout:g} s"
    except BaseException:
        tree.end()
        try:
            tree.proc.communicate(timeout=30)
        except BaseException:
            pass
        raise
    finally:
        tree.close()
    return tree.proc.returncode, (out or "") + "\n" + (err or "")


def sweep_stale(base: Path, now: float, alive=clean_temp.pid_alive) -> list[str]:
    """Removes `ng-<pid>` folders a killed run left: a day old and the pid gone. A line each."""
    lines: list[str] = []
    if not base.is_dir():
        return lines
    for entry in os.scandir(base):
        m = PRIVATE.match(entry.name)
        # A link or junction at the top is never removed through (S1, the same rule as `clean_temp`; `remove_tree`
        # asks again right before it lists the folder).
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
    # Plan 40 review S1: this was `mkdir(exist_ok=True)` on `ng-<pid>`, which took a name that was already there, a
    # junction included, and the `finally` then emptied the junction's target. `mkdtemp` makes a new folder or
    # fails, so the `finally` can only ever remove a folder this run created.
    base.mkdir(parents=True, exist_ok=True)
    private = Path(tempfile.mkdtemp(prefix=f"ng-{pid}-", dir=base))
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
        failures = clean_temp.remove_tree(private) if os.path.lexists(private) else []
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
