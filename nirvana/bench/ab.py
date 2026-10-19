#!/usr/bin/env python3
"""Plan 12 O7: the A/B of Nirvana Claude (arm N) against plain Claude Code (arm C) on the Moto project.

The owner, L1 (2026-10-17): "The goal is to utilize both my claude subscription and my agy gemini subsritpion to buid
better apps and longer running agentic workflows." This driver is the measurement: both arms run claude-opus-5-5 at
medium effort, so only the harness differs. It is ported from old Nirvana's bench/drive.py (the 2026-10-12 Moto A/B,
docs/plans/2026-10-12-moto-ab.md) with the parts that were that daemon's left out, and its lessons kept:

  * print mode's `--output-format json` writes only at exit, so a run killed at its wall leaves nothing: arm C uses
    stream-json and a reader that writes each line as it arrives.
  * a clone of D:\\MotoProject lacks the files an editor start needs (they are untracked there): the scenario's repo
    block attaches them in one bench commit, checked by sha256.
  * a truth check that scores "is the server up" must tell this run's server from the owner's: the probe accepts only a
    listener whose /api/project root is the run's clone or one of its worktrees.
  * a verdict that does not arrive reads as a pass (CLAUDE.md rule 8): a missing reading, truth or transcript is a
    named gap in run.json, never a zero.

    python nirvana/bench/ab.py run <out> M1 N          one run
    python nirvana/bench/ab.py sequence <out>          M1, M2, M5 x 3 per arm, alternating N and C
    python nirvana/bench/ab.py table <out>             the markdown table, recomputed from the run directories
    python nirvana/bench/ab.py usage                   the guard's reading of the Claude pool

Hard limits, each checked in code: nothing is written under the source repository (`refuse_inside`); every run is on a
fresh `git clone --local`; arm N runs in an isolated herdr server (XDG_CONFIG_HOME, XDG_STATE_HOME, HERDR_SESSION and
NIRVANA_HERDR_HOME under the run directory); `BROWSER` is a program that opens nothing (CLAUDE.md rule 15); the trust
prompt is answered only for a clone inside the out directory; only processes this run started are stopped.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import threading
import time
import urllib.request
from pathlib import Path
from typing import Callable, Iterable

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SCENARIOS = HERE / "scenarios" / "moto.json"
TRUTH_DIR = HERE / "truth"
LAUNCH_RS = ROOT / "src" / "nirvana_launch.rs"

MODEL = "claude-opus-5-5"
EFFORT = "medium"
ARMS = ("N", "C")
NOT_MEASURED = "n.a."

# The guard (brief): no paid run while Claude's 5 h window is above 80% or its week above 85%.
GUARD_FIVE_H = 0.80
GUARD_WEEK = 0.85
POLL_S = 3.0
SETTLE_S = 20.0            # the orchestrator must stay idle with nothing pending this long before a run is over
MAX_REPLIES = 3            # a scenario's `reply` is typed at most this often (the person policy)
START_GRACE_S = 60.0       # an ask that produced no work in this long is recorded as never started
BLOCKED_S = 120.0          # an unrecognised block (not the trust prompt) this long ends the run as stuck
STALL_S = 300.0            # working with an unchanged screen this long is an incident
SERVED_PORTS = range(8770, 8800)
TITLE = re.compile(r"<title>(.*?)</title>", re.I | re.S)
SPINNER = re.compile(r"^\s*\S\s+[A-Za-z][A-Za-z-]*…\s*\((?:\d+m\s*)?\d+s", re.M)


class Refused(Exception):
    """A rule of the bench that the caller broke. The message is one sentence."""


# ---- paths and the hard limits ---------------------------------------------------------------------------------------


def norm(path: str | Path) -> str:
    """One spelling of a path for comparisons: forward slashes, no trailing slash, lower case."""
    return str(path).replace("\\", "/").rstrip("/").lower()


def is_inside(path: str | Path, parent: str | Path) -> bool:
    p, q = norm(path), norm(parent)
    return p == q or p.startswith(q + "/")


def refuse_inside(path: str | Path, protected: str | Path, what: str = "path") -> None:
    """Nothing is ever written under the protected folder (the owner's D:\\MotoProject)."""
    if is_inside(path, protected):
        raise Refused(f"refused: {what} {path} is inside {protected}, which the bench only reads.")


def mangle(path: str | Path) -> str:
    """Claude Code's project-folder name for a working directory: every non-alphanumeric character becomes `-`."""
    return re.sub(r"[^A-Za-z0-9]", "-", str(path))


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="milliseconds")


def strip_ansi(text: str) -> str:
    return re.sub(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07", "", text)


# ---- the scenario file ------------------------------------------------------------------------------------------------


def load_scenarios(path: Path = SCENARIOS) -> dict:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def scenario(doc: dict, sid: str) -> dict:
    for sc in doc["scenarios"]:
        if sc["id"] == sid:
            return sc
    raise Refused(f"refused: no scenario {sid!r}; the file has {[s['id'] for s in doc['scenarios']]}.")


# ---- arm C: the plain Claude Code argv and its stream-json ------------------------------------------------------------


def cc_argv(prompt: str, extra: Iterable[str] = ()) -> list[str]:
    """Plan 12 O7: `claude -p <ask> --model claude-opus-5-5 --effort medium --output-format stream-json --verbose`.
    `claude` is resolved to its path: a list argv on Windows does not search for a shim."""
    return [shutil.which("claude") or "claude", "-p", prompt, "--model", MODEL, "--effort", EFFORT,
            "--output-format", "stream-json", "--verbose", *extra]


def zero_tokens() -> dict:
    return {"input": 0, "cache_write": 0, "cache_read": 0, "output": 0}


def add_tokens(a: dict, b: dict) -> dict:
    return {k: int(a.get(k) or 0) + int(b.get(k) or 0) for k in zero_tokens()}


def gross(tokens: dict | None) -> int | None:
    """The meter's definition (old Nirvana's parity meter): input + cache write + cache read, output excluded."""
    if not tokens:
        return None
    return int(tokens.get("input") or 0) + int(tokens.get("cache_write") or 0) + int(tokens.get("cache_read") or 0)


def _usage_tokens(usage: dict) -> dict:
    return {"input": int(usage.get("input_tokens") or 0),
            "cache_write": int(usage.get("cache_creation_input_tokens") or 0),
            "cache_read": int(usage.get("cache_read_input_tokens") or 0),
            "output": int(usage.get("output_tokens") or 0)}


def split_tokens(rows: Iterable[dict], *, all_workers: bool = False) -> dict:
    """Tokens in four kinds, for the orchestrator and for workers, from assistant rows of a stream-json run or a Claude
    Code transcript. One API message appears in several rows (one per content block), each repeating its usage, so a
    message id counts once. A row is a worker's when it is a sidechain row (transcript), has a parent tool use
    (stream-json), or `all_workers` says the whole file is a subagent's."""
    seen: dict[str, tuple[bool, dict]] = {}
    anon = 0
    for row in rows:
        if not isinstance(row, dict) or row.get("type") != "assistant":
            continue
        msg = row.get("message") or {}
        usage = msg.get("usage")
        if not isinstance(usage, dict):
            continue
        worker = all_workers or bool(row.get("isSidechain")) or bool(row.get("parent_tool_use_id"))
        key = msg.get("id")
        if not key:
            anon += 1
            key = f"anon-{anon}"
        seen[key] = (worker, _usage_tokens(usage))
    out = {"orchestrator": zero_tokens(), "workers": zero_tokens(), "messages": len(seen)}
    for worker, tokens in seen.values():
        slot = "workers" if worker else "orchestrator"
        out[slot] = add_tokens(out[slot], tokens)
    return out


def parse_stream(rows: list[dict]) -> dict:
    """What an arm C stream-json run says: tokens split orchestrator and workers, the `result` row's own figures, the
    answer, the models seen, rate-limit events. A run with no assistant row has `messages` 0 and zero tokens; a
    missing `result` row is `result: None`, never a made-up success."""
    tokens = split_tokens(rows)
    result = next((r for r in reversed(rows) if isinstance(r, dict) and r.get("type") == "result"), None)
    models: dict[str, int] = {}
    limits = []
    for r in rows:
        if not isinstance(r, dict):
            continue
        if r.get("type") == "assistant":
            m = (r.get("message") or {}).get("model")
            if m:
                models[m] = models.get(m, 0) + 1
        if r.get("type") == "rate_limit_event":
            limits.append(r.get("rate_limit_info") or r)
    answer = ""
    if result and isinstance(result.get("result"), str):
        answer = result["result"]
    else:
        for r in reversed(rows):
            if isinstance(r, dict) and r.get("type") == "assistant" and not r.get("parent_tool_use_id"):
                text = "".join(b.get("text", "") for b in (r.get("message") or {}).get("content") or []
                               if isinstance(b, dict) and b.get("type") == "text")
                if text.strip():
                    answer = text
                    break
    return {"tokens": tokens, "answer": answer, "models": models, "rate_limits": limits,
            "result": None if result is None else {
                k: result.get(k) for k in ("subtype", "is_error", "duration_ms", "num_turns", "total_cost_usd",
                                           "session_id") if k in result},
            "permission_denials": [d.get("tool_name") for d in (result or {}).get("permission_denials") or []
                                   if isinstance(d, dict)],
            "result_usage": None if not result else _usage_tokens(result.get("usage") or {})}


def parse_lines(text: str) -> list[dict]:
    """One JSON object per line, anything else skipped (a half-written last line of a killed run included)."""
    out = []
    for ln in text.splitlines():
        ln = ln.strip()
        if ln.startswith("{"):
            try:
                row = json.loads(ln)
            except ValueError:
                continue
            if isinstance(row, dict):
                out.append(row)
    return out


# ---- arm N: the launch, the screen, the end of a run ------------------------------------------------------------------


# 0.25.2 gave Nirvana Claude the owner's --rc and --chrome. The bench leaves them out: --rc would register a
# remote-control session for every run, and --chrome would drive the owner's browser.
OWNER_ONLY_FLAGS = ("--rc", "--chrome")


def nirvana_claude_args(home: Path) -> list[str]:
    """src/nirvana_launch.rs `nirvana_claude_args`, in its order, less `OWNER_ONLY_FLAGS` (a test reads the Rust source
    and holds this to it)."""
    plugin = Path(home) / "nirvana" / "claude"
    return ["--plugin-dir", str(plugin), "--agent", "nirvana", "--model", MODEL, "--effort", EFFORT,
            "--append-system-prompt-file", str(plugin / "ROLE.md"), "--name", "Nirvana",
            "--settings", str(plugin / "settings.json")]


def powershell_line(program: str, args: list[str]) -> str:
    """The line typed into a pwsh pane: each word single-quoted when it needs it."""
    def q(word: str) -> str:
        return word if re.fullmatch(r"[A-Za-z0-9_./:\\=+-]+", word) else "'" + word.replace("'", "''") + "'"
    return " ".join([q(program), *map(q, args)])


def screen_kind(raw: str) -> str:
    """What a Claude Code pane shows, from its screen text: `trust` (the folder-trust prompt), `permission` (a tool
    approval), `working` (the spinner's interrupt hint), `typed` (the prompt line holds text), `idle` (an empty prompt
    line or its placeholder) or `unknown`. Recorded screens are the fixtures (nirvana/bench/fixtures)."""
    text = strip_ansi(raw)
    low = text.lower()
    # The trust prompt is the box that says so AND asks for Enter: an answer that talks about trusting a folder is not it.
    if ("yes, i trust this folder" in low and "enter to confirm" in low) or "do you trust the files" in low:
        return "trust"
    if "do you want to" in low and re.search(r"(^|\s)1\.\s*yes", low):
        return "permission"
    # MEASURED 2026-10-18 (claude 2.1.302): the working screen has no "esc to interrupt"; its spinner line reads
    # "✢ Accomplishing… (7s · ↓ 52 tokens)" and the finished one "✻ Crunched for 31s · done 3:11 AM".
    if SPINNER.search(text) or "esc to interrupt" in low or "ctrl+c to interrupt" in low:
        return "working"
    # The input box is the LAST prompt line: the lines above it echo earlier asks and also start with the prompt mark.
    for line in reversed(text.splitlines()):
        s = line.strip()
        if s.startswith("\u276f") or s.startswith(">"):
            body = s[1:].strip()
            if re.match(r"^\d+\.", body):
                continue
            if body and not body.startswith("Try \""):
                return "typed"
            return "idle"
    return "unknown"


def check_trust_scope(clone: str | Path, out: str | Path) -> None:
    """The folder-trust prompt is answered for a clone inside the bench out directory and for no other folder: the
    answer writes a trust entry into the owner's Claude config."""
    if not is_inside(clone, out):
        raise Refused(f"refused: not answering a trust prompt for {clone}, which is outside {out}.")


def trust_action(raw: str) -> str | None:
    """What to press on Claude Code's folder-trust prompt: `enter` only when the highlighted line (the one starting `❯`)
    is the "Yes, I trust" one, `down` when it is anything else ("No, exit" is the default in 2.1.302, MEASURED
    2026-10-18: the first smoke pressed Enter on it, again and again), None when no option is highlighted."""
    for line in strip_ansi(raw).splitlines():
        s = line.strip()
        if s.startswith("❯"):
            return "enter" if "yes, i trust" in s.lower() else "down"
    return None


def classify_check(text: str, code: int | None = None) -> str:
    """One word for a line of `herdr run check <run>` (src/nirvana_run.rs `check`):
    ended | finished_unended | wake_cap | live | deferred | queued | gate | stuck | none | unknown."""
    t = text.strip().lower()
    if not t:
        return "unknown"
    if "the run ended" in t:
        return "ended"
    if "every order is finished and the run is not ended" in t:
        return "finished_unended"
    if "wake cap asked the owner" in t:
        return "wake_cap"
    if "worker live:" in t:
        return "live"
    if "is deferred to" in t:
        return "deferred"
    if "wait for the orchestrator" in t:
        return "queued"
    if "waits for the gate" in t or "waits for an answer" in t or "was interrupted" in t:
        return "gate"
    if "no live worker and no pending step" in t:
        return "stuck"
    if "the run has no order seated" in t:
        return "none"
    return "unknown"


# What each class means for "is the run over". `finished_unended` and `wake_cap` are over, but the Stop hook should have
# raised them: they are incidents. `stuck` and `none` will not move without the orchestrator: over once it is idle.
OVER = {"ended": None, "finished_unended": "dead_end", "wake_cap": "wake_cap", "stuck": "dead_end", "none": "dead_end"}
BUSY = {"live", "deferred", "queued", "gate", "unknown"}


def journal_state(events: list[dict]) -> tuple[str, str]:
    """A run's state from its journal alone (`.nirvana/runs/<run>/journal.jsonl`), as `herdr run check` would say, minus
    liveness (a journal cannot tell a live worker from a dead one): (class, why) with the class one of
    `empty | ended | working | pending | finished_unended`. Conservative: a state it cannot place is `pending`, never
    `ended`. Recorded journals are the fixtures."""
    if not events:
        return "empty", "no event"
    if any(e.get("event") == "ended" and not e.get("order") for e in events):
        return "ended", "the run's ended event"
    last: dict[str, dict] = {}
    for e in events:
        order = e.get("order")
        if order and e.get("event") not in ("woken",):
            last[order] = e
    if not last:
        return "pending", "the run has no order seated"
    replaced = {o for o in last if any(p.startswith(o + "-s") or p.startswith(o + "-r") for p in last)}
    working, pending = [], []
    for order, e in last.items():
        kind = e.get("event")
        if kind in ("seated", "started", "usage", "resumed"):
            working.append(order)
        elif kind == "merged" or order in replaced:
            continue
        elif kind == "ended" and e.get("outcome") in ("failed", "stopped", "interrupted"):
            pending.append(order)
        elif kind in ("delivered", "ended", "gated", "asked", "interrupted", "checkpoint", "stalled", "rate_limited",
                      "deferred", "reseated"):
            pending.append(order)
        else:
            pending.append(order)
    if working:
        return "working", "worker running: " + ", ".join(sorted(working))
    if pending:
        return "pending", "a step is waiting: " + ", ".join(sorted(pending))
    return "finished_unended", "every order is finished and the run is not ended"


def wants_reply(answer: str) -> bool:
    """Did the orchestrator end its turn on a question: its last paragraph, markdown stripped, ends in `?`. (Old
    Nirvana's A/B lost runs to a card whose question had no question mark, and to a person policy that never answered
    one: this is the policy, and it answers only a question.)"""
    paragraphs = [p.strip() for p in re.split(r"\n\s*\n", answer.strip()) if p.strip()]
    if not paragraphs:
        return False
    return paragraphs[-1].rstrip("*_` )\"'").endswith("?")


def latest_activity(clone: Path, since: float) -> float | None:
    """The newest write among the orchestrator's transcripts, its subagents' and the runs' journals: what a pane that
    says it is working must keep producing. The spinner's own clock changes the screen every second, so the screen
    cannot show a stall."""
    mains, subs = session_files(clone, since)
    paths = [*mains, *subs, *(clone / ".nirvana" / "runs").glob("*/journal.jsonl")]
    times = []
    for p in paths:
        try:
            times.append(p.stat().st_mtime)
        except OSError:
            pass
    return max(times) if times else None


class EndDetector:
    """The end of an arm N run (brief): the orchestrator is idle, every run under `.nirvana/runs` shows no live worker
    and no pending step, no wake is queued, and that has held for `settle_s`. If the ask opened no run (a question), the
    first idle after the answer ends it. `feed` takes one sample per poll and returns None, or `("over", quiet_at, why,
    incidents)` where `quiet_at` is when the quiet began, so the wall does not include the settle time.

    An ask that never produced any work is not a pass: after `grace_s` without a single busy sample the verdict is
    `no_response`. The orchestrator being blocked on a prompt is never quiet."""

    def __init__(self, settle_s: float = SETTLE_S, grace_s: float = START_GRACE_S, blocked_s: float = BLOCKED_S):
        self.settle_s, self.grace_s, self.blocked_s = settle_s, grace_s, blocked_s
        self.t0: float | None = None
        self.seen_busy = False
        self.quiet_at: float | None = None
        self.blocked_at: float | None = None
        self.n_quiet = 0

    def start(self, t: float) -> None:
        self.t0 = t

    def feed(self, t: float, *, status: str, kind: str, runs: dict[str, str]) -> tuple | None:
        if self.t0 is None:
            raise RuntimeError("EndDetector.feed before start")
        classes = set(runs.values())
        busy = status == "working" or kind == "working" or bool(classes & BUSY) or kind == "typed"
        blocked = status == "blocked" or kind in ("permission", "trust")
        if busy or blocked:
            # A pane blocked on a prompt after the ask is work that started: the ask registered.
            self.seen_busy = self.seen_busy or busy or blocked
            self.quiet_at, self.n_quiet = None, 0
            if blocked:
                self.blocked_at = self.blocked_at if self.blocked_at is not None else t
                if t - self.blocked_at >= self.blocked_s:
                    return ("stuck_blocked", self.blocked_at, f"blocked on a {kind} prompt for {self.blocked_s:g} s", [])
            else:
                self.blocked_at = None
            if not self.seen_busy and t - self.t0 >= self.grace_s:
                return ("no_response", self.t0, f"no work seen {self.grace_s:g} s after the ask", ["never_started"])
            return None
        self.blocked_at = None
        if self.quiet_at is None:
            self.quiet_at = t
        self.n_quiet += 1
        if not self.seen_busy and t - self.t0 < self.grace_s:
            return None        # the first idle can be the prompt before the ask registered
        if t - self.quiet_at >= self.settle_s and self.n_quiet >= 2:
            incidents = sorted({OVER[c] for c in classes if OVER.get(c)})
            if not self.seen_busy:
                return ("no_response", self.t0, "the orchestrator never left idle", ["never_started"])
            return ("over", self.quiet_at, "idle with no live worker, no pending step, no wake queued", incidents)
        return None


# ---- the served probe (M2) --------------------------------------------------------------------------------------------


def http_get(url: str, timeout: float = 2.0) -> tuple[int | None, str]:
    """(status, body) of one GET, or (None, "") when nothing answered. No proxy, no redirect followed to elsewhere."""
    try:
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(url, timeout=timeout) as resp:
            return resp.status, resp.read(65536).decode("utf-8", "replace")
    except Exception as e:  # noqa: BLE001 - the probe's job is to turn every failure into "nothing answered"
        code = getattr(e, "code", None)
        return (code if isinstance(code, int) else None), ""


def probe_served(clone: Path, sc: dict, *, get: Callable[[str], tuple[int | None, str]] = http_get,
                 ports: Iterable[int] = SERVED_PORTS, now: Callable[[], str] = utc_now) -> dict | None:
    """The first listener in `ports` that is THIS run's editor: `GET /` is 200, `GET <api>` is 200 and its project root
    is the clone or a folder under it (the owner's own server on 8770 is told apart by that root). Returns the
    served.json the truth reads, or None."""
    spec = sc.get("served") or {}
    api = spec.get("api", "/api/project")
    for port in ports:
        base = f"http://127.0.0.1:{port}"
        status, body = get(base + spec.get("path", "/"))
        if status != 200:
            continue
        a_status, a_body = get(base + api)
        root = re.search(r'"root"\s*:\s*"([^"]+)"', a_body or "")
        # The JSON body escapes backslashes: one level decoded.
        root_text = root.group(1).replace("\\\\", "\\") if root else ""
        if a_status != 200 or not root_text or not is_inside(root_text, clone):
            continue
        title = TITLE.search(body)
        return {"port": port, "status": status, "title": title.group(1).strip() if title else None,
                "api_status": a_status, "api_body": a_body[:4096], "first_served_at": now(),
                "pid": None, "pid_command_line": None}
    return None


# ---- the meter --------------------------------------------------------------------------------------------------------


def usage_snapshot(doc: dict) -> dict:
    """`herdr usage --json` as {pool: {window: {used, fresh}}}; a pool or window without a reading is simply absent."""
    out: dict = {}
    for p in doc.get("pools") or []:
        wins = {w["window"]: {"used": w.get("used"), "fresh": w.get("fresh"), "read_at": w.get("read_at"),
                              "resets_at": w.get("resets_at")}
                for w in p.get("windows") or [] if w.get("window") and w.get("used") is not None}
        if wins:
            out[p["pool"]] = wins
    return out


def usage_delta(before: dict, after: dict) -> dict:
    """{pool: {window: after - before}} for every window both snapshots read. A reset in between makes a window go
    DOWN: it is reported as `reset` and not as a negative use."""
    out: dict = {}
    for pool, wins in after.items():
        for window, now in wins.items():
            was = (before.get(pool) or {}).get(window)
            if was is None or was.get("used") is None or now.get("used") is None:
                continue
            d = round(float(now["used"]) - float(was["used"]), 6)
            out.setdefault(pool, {})[window] = "reset" if d < 0 else d
    return out


def guard(snapshot: dict, *, five_h: float = GUARD_FIVE_H, week: float = GUARD_WEEK,
          allow_unknown: bool = False) -> tuple[bool, str]:
    """May a paid run start? Claude's 5 h above `five_h` or its week above `week` refuses it, and so does a missing
    reading: a verdict that does not arrive reads as a pass otherwise."""
    wins = snapshot.get("max") or {}
    h, w = (wins.get("5h") or {}).get("used"), (wins.get("week") or {}).get("used")
    if h is None or w is None:
        return (allow_unknown, "Claude's 5h or week reading is missing" + (" (allowed)" if allow_unknown else ""))
    if h > five_h:
        return False, f"Claude 5h is at {h:.0%}, above {five_h:.0%}"
    if w > week:
        return False, f"Claude week is at {w:.0%}, above {week:.0%}"
    return True, f"Claude 5h {h:.0%}, week {w:.0%}"


# ---- the truth adapters -----------------------------------------------------------------------------------------------


def truth_argv(sc: dict, *, tree: Path, base: str, branch: str, answer: Path, served: Path | None,
               run_dir: Path) -> list[str]:
    """The held-out check of a scenario, as a list argv. Every moto truth takes the same flags (M2.parser)."""
    script = TRUTH_DIR / Path(sc["truth"]["check"]).name
    argv = [sys.executable, "-I", str(script), "--tree", str(tree), "--base", base, "--answer", str(answer),
            "--run-dir", str(run_dir)]
    if sc["truth"].get("served") and served is not None:
        argv += ["--served", str(served)]
    if script.name == "M5.py":
        argv += ["--branch", branch]
    return argv


def parse_truth(stdout: str, code: int) -> dict:
    """`PASS <rule>` / `FAIL <rule> -- <detail>` lines and `RESULT ...` into {result, passed, failed, total}. Exit 2 or
    no RESULT line is `ERROR`, which the table prints as such and never as a fail or a pass."""
    passed, failed = 0, []
    result = None
    for ln in stdout.splitlines():
        parts = ln.split()
        if not parts:
            continue
        if parts[0] == "PASS" and len(parts) > 1:
            passed += 1
        elif parts[0] == "FAIL" and len(parts) > 1:
            failed.append(parts[1])
        elif parts[0] == "RESULT" and len(parts) > 1:
            result = parts[1]
    if code == 2 or result not in ("PASS", "FAIL"):
        return {"result": "ERROR", "passed": passed, "failed": failed, "exit": code}
    return {"result": result, "passed": passed, "failed": failed, "exit": code}


def run_truth(sc: dict, rundir: Path, clone: Path, base: str, branch: str, answer: Path,
              served: Path | None) -> dict:
    argv = truth_argv(sc, tree=clone, base=base, branch=branch, answer=answer, served=served, run_dir=rundir)
    try:
        done = subprocess.run(argv, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=300,
                              cwd=str(TRUTH_DIR))
    except (OSError, subprocess.SubprocessError) as e:
        return {"result": "ERROR", "passed": 0, "failed": [], "exit": None, "error": f"{type(e).__name__}: {e}"}
    (rundir / "truth.txt").write_text(done.stdout + done.stderr, encoding="utf-8", newline="\n")
    truth = parse_truth(done.stdout, done.returncode)
    (rundir / "truth.json").write_text(json.dumps(truth, indent=1) + "\n", encoding="utf-8", newline="\n")
    return truth


# ---- the clone --------------------------------------------------------------------------------------------------------


def _git(repo: Path, *args: str, check: bool = True) -> str:
    done = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, encoding="utf-8",
                          errors="replace", timeout=900)
    if check and done.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {done.stderr.strip()[-400:]}")
    return done.stdout.strip()


def attach_files(repo: dict, target: Path) -> list[str]:
    """The files the scenario names, copied into the clone with their bytes checked against the recorded sha256."""
    out = []
    for f in repo.get("files") or []:
        data = Path(f["from"]).read_bytes()
        if f.get("sha256") and hashlib.sha256(data).hexdigest() != f["sha256"]:
            raise Refused(f"refused: {f['from']} is not the attachment the scenario names (sha256 differs).")
        dst = Path(target) / f["to"]
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_bytes(data)
        out.append(Path(f["to"]).as_posix())
    return out


def attach_dirs(repo: dict, target: Path) -> list[str]:
    """`repo.dirs`: each directory copied byte for byte, names in `exclude` skipped at any depth, links never followed."""
    out: list[str] = []
    for d in repo.get("dirs") or []:
        src, to = Path(d["from"]), Path(d["to"])
        skip = {str(n) for n in d.get("exclude") or []} | {".git"}
        for root, names, found in os.walk(src):
            names[:] = sorted(n for n in names if n not in skip)
            for name in sorted(found):
                s = Path(root) / name
                if name in skip or s.is_symlink():
                    continue
                rel = to / s.relative_to(src)
                dst = Path(target) / rel
                dst.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(s, dst)
                out.append(rel.as_posix())
    return out


def append_extra(repo: dict, target: Path) -> list[str]:
    out: list[str] = []
    for x in repo.get("extra_files") or []:
        dst = Path(target) / x["to"]
        old = dst.read_bytes() if dst.is_file() else b""
        add = x["append"].encode("utf-8")
        if old and add and not old.endswith(b"\n"):
            old += b"\n"
        dst.write_bytes(old + add)
        out.append(Path(x["to"]).as_posix())
    return out


def make_clone(repo: dict, into: Path, out_dir: Path) -> dict:
    """A fresh `git clone --local` of the source at the scenario's sha, then ONE bench commit holding the attachments
    (the clone's base). The source is only read: `into` must be inside the out directory and outside the source."""
    source = Path(repo["source"])
    refuse_inside(into, source, "the clone")
    if not is_inside(into, out_dir):
        raise Refused(f"refused: the clone {into} is not inside the out directory {out_dir}.")
    if into.exists():
        raise Refused(f"refused: {into} exists; every run gets a fresh clone.")
    into.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["git", "clone", "--local", "--no-hardlinks", "--no-checkout", "--quiet", str(source), str(into)], check=True,
                   capture_output=True, timeout=1800)
    branch = repo.get("branch") or "master"
    _git(into, "checkout", "-q", "-B", branch, repo["sha"])
    for b in _git(into, "branch", "--list", "--format=%(refname:short)").split():
        if b != branch:
            _git(into, "branch", "-D", b)
    _git(into, "remote", "remove", "origin", check=False)
    _git(into, "config", "user.name", "Bench Fixture")
    _git(into, "config", "user.email", "bench@example.invalid")
    paths = attach_files(repo, into) + attach_dirs(repo, into)
    paths += [p for p in append_extra(repo, into) if p not in paths]
    if paths:
        done = subprocess.run(["git", "--literal-pathspecs", "-C", str(into), "add", "-f", "--pathspec-from-file=-",
                               "--pathspec-file-nul"], input="\0".join(paths).encode("utf-8"), capture_output=True,
                              timeout=1800)
        if done.returncode != 0:
            raise RuntimeError(f"git add of the attachments: {done.stderr.decode('utf-8', 'replace')[-400:]}")
        _git(into, "commit", "-q", "--no-verify", "-m", "bench: the scenario's attachments (never the owner's folder)")
    return {"clone": str(into), "base": _git(into, "rev-parse", "HEAD"), "branch": branch, "attached": len(paths)}


def _writable(func, path, _exc):
    os.chmod(path, 0o700)
    func(path)


def finalize_clone(clone: Path, rundir: Path, keep: bool) -> dict:
    """Write what the clone ends as (git log, status, worktrees, diffstat against the base) into the run directory, then
    remove it unless `keep`: the source's 690 MiB of loose objects are copied into every clone, and 18 runs would be
    12 GB. Removal is of the run's own clone only (`is_inside(clone, rundir)`)."""
    note: dict = {"kept": keep}
    if not clone.is_dir():
        return note
    parts = []
    for args in (("log", "--oneline", "--all", "-20"), ("status", "--short", "--untracked-files=no"),
                 ("worktree", "list"), ("branch", "--list", "-a")):
        parts.append("$ git " + " ".join(args) + "\n" + _git(clone, *args, check=False))
    (rundir / "git-final.txt").write_text(("\n\n").join(parts) + "\n", encoding="utf-8", newline="\n")
    if not keep and is_inside(clone, rundir):
        # Worktrees the run's workers made are folders under the clone; git itself is not asked to prune them.
        shutil.rmtree(clone, onexc=_writable)
        note["removed"] = not clone.exists()
    return note


# ---- processes this run started ---------------------------------------------------------------------------------------


def kill_tree(pid: int) -> list[int]:
    """Stop a process this run started and everything below it. Returns the pids stopped."""
    import psutil
    stopped: list[int] = []
    try:
        root = psutil.Process(pid)
        procs = root.children(recursive=True) + [root]
    except psutil.Error:
        return stopped
    for p in procs:
        try:
            p.kill()
            stopped.append(p.pid)
        except psutil.Error:
            pass
    return stopped


def names_clone(clone: str | Path, cmd: str, cwd: str) -> bool:
    """Is this process one the run's own agent started: its working folder is under the clone, or its command line
    names the clone. The owner's editor on 8770 runs from D:/MotoProject and matches neither."""
    return is_inside(cwd, clone) or norm(clone) in norm(cmd)


def stop_run_listeners(clone: Path, ports: Iterable[int] = SERVED_PORTS) -> list[dict]:
    """Stop the listeners on `ports` whose command line or working folder names the run's clone: servers the run's own
    agent started. The owner's server on 8770 has another folder and is never matched."""
    import psutil
    wanted = {int(p) for p in ports}
    stopped = []
    try:
        conns = psutil.net_connections(kind="tcp")
    except (psutil.Error, OSError):
        return stopped
    for c in conns:
        if c.status != psutil.CONN_LISTEN or not c.laddr or c.laddr.port not in wanted or not c.pid:
            continue
        try:
            p = psutil.Process(c.pid)
            cmd = " ".join(p.cmdline())
            cwd = p.cwd()
        except psutil.Error:
            continue
        if names_clone(clone, cmd, cwd):
            stopped.append({"port": c.laddr.port, "pid": c.pid, "cmd": cmd[:300], "stopped": kill_tree(c.pid)})
    return stopped


# ---- herdr: the isolated server and its panes ------------------------------------------------------------------------


def nothing_cmd(directory: Path) -> Path:
    """A program that opens nothing, for BROWSER (CLAUDE.md rule 15)."""
    path = directory / "browser-opens-nothing.cmd"
    path.write_text("@echo off\r\nexit /b 0\r\n", encoding="ascii", newline="")
    return path


def bench_env(rundir: Path, home: Path, session: str, *, isolated: bool) -> dict:
    env = dict(os.environ)
    env["BROWSER"] = str(nothing_cmd(rundir))
    if isolated:
        env.update(XDG_CONFIG_HOME=str(rundir / "xdg-config"), XDG_STATE_HOME=str(rundir / "xdg-state"),
                   HERDR_SESSION=session, NIRVANA_HERDR_HOME=str(home))
        env.pop("HERDR_SOCKET_PATH", None)
    return env


def seed_home(home: Path, herdr_exe: Path, plugin_src: Path, owner_home: Path) -> list[str]:
    """A scratch NIRVANA_HERDR_HOME: the debug build at bin/herdr.exe (the plugin's hooks name it at
    `${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe`), the worktree's plugin dir, and read-only copies of the owner's
    usage.json and seats.toml. Returns what was copied."""
    seeded = []
    (home / "bin").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(herdr_exe, home / "bin" / "herdr.exe")
    seeded.append("bin/herdr.exe")
    dst = home / "nirvana" / "claude"
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(plugin_src, dst)
    seeded.append("nirvana/claude")
    (home / "nirvana").mkdir(parents=True, exist_ok=True)
    for name in ("usage.json", "seats.toml"):
        src = owner_home / "nirvana" / name
        if src.is_file():
            shutil.copyfile(src, home / "nirvana" / name)
            seeded.append(f"nirvana/{name}")
    return seeded


class Herdr:
    """The herdr CLI against one isolated server. Every call is a list argv with the bench environment."""

    def __init__(self, exe: Path, env: dict, project: Path | None = None):
        self.exe, self.env, self.project = Path(exe), env, project
        self.proc: subprocess.Popen | None = None

    def call(self, *args: str, timeout: float = 60, cwd: Path | None = None) -> tuple[int, str, str]:
        env = dict(self.env)
        if self.project is not None:
            env["NIRVANA_PROJECT"] = str(self.project)
        done = subprocess.run([str(self.exe), *args], env=env, capture_output=True, text=True, encoding="utf-8",
                              errors="replace", timeout=timeout, cwd=str(cwd or self.project or ROOT))
        return done.returncode, done.stdout, done.stderr

    def json(self, *args: str, timeout: float = 60) -> dict:
        code, out, err = self.call(*args, timeout=timeout)
        try:
            return json.loads(out)["result"]
        except (ValueError, KeyError, TypeError) as e:
            raise RuntimeError(f"herdr {' '.join(args)} (exit {code}): {out[:200]!r} {err[:200]!r}") from e

    def start_server(self, log: Path, wait_s: float = 20) -> int:
        self.proc = subprocess.Popen([str(self.exe), "server"], env=self.env, stdout=open(log, "wb"),
                                     stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        end = time.monotonic() + wait_s
        while time.monotonic() < end:
            if self.call("status", timeout=10)[1].count("status: running"):
                return self.proc.pid
            time.sleep(0.5)
        raise RuntimeError("the isolated herdr server did not come up")

    def stop_server(self) -> dict:
        """Stop the server this run started, by its own command, then by its pid; its panes' processes go with it."""
        record: dict = {"pid": self.proc.pid if self.proc else None}
        if self.proc is None:
            return record
        children = []
        try:
            import psutil
            children = [(c.pid, c.create_time()) for c in psutil.Process(self.proc.pid).children(recursive=True)]
        except Exception:  # noqa: BLE001
            pass
        self.call("server", "stop", timeout=30)
        try:
            self.proc.wait(20)
            record["exit"] = self.proc.returncode
        except subprocess.TimeoutExpired:
            record["killed"] = kill_tree(self.proc.pid)
        left = []
        for pid, born in children:
            try:
                import psutil
                # A pid is the run's own only while it is the same process: the create time guards against reuse.
                if psutil.pid_exists(pid) and psutil.Process(pid).create_time() == born:
                    left += kill_tree(pid)
            except Exception:  # noqa: BLE001
                pass
        record["children_left_and_stopped"] = left
        return record

    def pane(self, pane: str) -> dict:
        return self.json("pane", "get", pane)["pane"]

    def read(self, pane: str, lines: int = 60, source: str = "visible") -> str:
        return self.call("pane", "read", pane, "--source", source, "--lines", str(lines))[1]

    def send(self, pane: str, text: str) -> None:
        self.call("pane", "send-text", pane, text)

    def key(self, pane: str, key: str) -> None:
        self.call("pane", "send-keys", pane, key)


# ---- the transcripts and the receipt of arm N ------------------------------------------------------------------------


def claude_projects_dir() -> Path:
    return Path(os.environ.get("CLAUDE_CONFIG_DIR") or Path.home() / ".claude") / "projects"


def read_jsonl(path: Path) -> list[dict]:
    try:
        return parse_lines(path.read_text(encoding="utf-8", errors="replace"))
    except OSError:
        return []


def session_files(clone: Path, since: float) -> tuple[list[Path], list[Path]]:
    """The orchestrator's transcripts (main sessions in the clone's project folder written after `since`) and its
    subagents' (`<session>/subagents/agent-*.jsonl`)."""
    folder = claude_projects_dir() / mangle(clone)
    mains = sorted(p for p in folder.glob("*.jsonl") if p.stat().st_mtime >= since) if folder.is_dir() else []
    subs: list[Path] = []
    for m in mains:
        subs += sorted((folder / m.stem / "subagents").glob("agent-*.jsonl"))
    return mains, subs


def transcript_tokens(mains: list[Path], subs: list[Path]) -> dict:
    out = {"orchestrator": zero_tokens(), "workers": zero_tokens(), "messages": 0}
    for m in mains:
        part = split_tokens(read_jsonl(m))
        out["orchestrator"] = add_tokens(out["orchestrator"], part["orchestrator"])
        out["workers"] = add_tokens(out["workers"], part["workers"])
        out["messages"] += part["messages"]
    for s in subs:
        part = split_tokens(read_jsonl(s), all_workers=True)
        out["workers"] = add_tokens(out["workers"], part["workers"])
        out["messages"] += part["messages"]
    return out


def models_of(paths: list[Path]) -> dict[str, int]:
    """{model: assistant messages} over transcript files (a message counted once by id)."""
    seen: dict[str, str] = {}
    for path in paths:
        for row in read_jsonl(path):
            msg = row.get("message") or {}
            if row.get("type") == "assistant" and msg.get("id") and msg.get("model"):
                seen[msg["id"]] = msg["model"]
    out: dict[str, int] = {}
    for model in seen.values():
        out[model] = out.get(model, 0) + 1
    return out


def final_answer(mains: list[Path]) -> str:
    """The orchestrator's last text in the newest main transcript: what the person was told."""
    for m in reversed(mains):
        for row in reversed(read_jsonl(m)):
            if row.get("type") == "assistant" and not row.get("isSidechain"):
                text = "".join(b.get("text", "") for b in (row.get("message") or {}).get("content") or []
                               if isinstance(b, dict) and b.get("type") == "text")
                if text.strip():
                    return text
    return ""


def seats_of(receipt: dict | None) -> list[str]:
    """`class -> seat effort (cli model) on pool: verdict`, one line per order of the receipt."""
    out = []
    for o in (receipt or {}).get("orders") or []:
        out.append(f"{o.get('order')}: {o.get('class')} -> {o.get('seat')} {o.get('effort')} "
                   f"({o.get('cli')} {o.get('model')}) on {o.get('pool')}: {o.get('verdict') or o.get('phase')}")
    return out


def receipt_workers(receipt: dict | None) -> dict | None:
    w = (receipt or {}).get("workers")
    return None if not isinstance(w, dict) else {k: int(w.get(k) or 0) for k in zero_tokens()}


# ---- a run ------------------------------------------------------------------------------------------------------------


def claude_settings_note() -> dict:
    """Arm C runs with the owner's normal settings; which, recorded (never their contents)."""
    p = Path.home() / ".claude" / "settings.json"
    note: dict = {"path": str(p)}
    try:
        data = p.read_bytes()
        doc = json.loads(data)
        note.update(sha256=hashlib.sha256(data).hexdigest(), model=doc.get("model"), effortLevel=doc.get("effortLevel"),
                    enabled_plugins=sorted(k for k, v in (doc.get("enabledPlugins") or {}).items() if v),
                    permissions=doc.get("permissions"), tui=doc.get("tui"))
    except (OSError, ValueError) as e:
        note["error"] = f"{type(e).__name__}: {e}"
    return note


def claude_version() -> str:
    try:
        return subprocess.run([shutil.which("claude") or "claude", "--version"], capture_output=True, text=True,
                              timeout=30).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return NOT_MEASURED


def read_usage(exe: Path, home: Path, owner_home: Path, refresh: bool = True) -> dict:
    """The pools, read now through a SCRATCH home's `herdr usage --json` (0 tokens; a refresh takes 4 to 20 s). The
    scratch home holds a copy of the owner's usage.json, so the owner's cache file is never written."""
    (home / "nirvana").mkdir(parents=True, exist_ok=True)
    src = owner_home / "nirvana" / "usage.json"
    if src.is_file() and not (home / "nirvana" / "usage.json").is_file():
        shutil.copyfile(src, home / "nirvana" / "usage.json")
    env = dict(os.environ, NIRVANA_HERDR_HOME=str(home))
    done = subprocess.run([str(exe), "usage", "--json", *(["--refresh"] if refresh else [])], env=env,
                          capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=180)
    try:
        return usage_snapshot(json.loads(done.stdout))
    except ValueError:
        return {}


def next_rundir(out: Path, sid: str, arm: str) -> tuple[Path, int]:
    base = out / "runs" / sid
    k = 1
    while (base / f"{arm}-{k}").exists():
        k += 1
    return base / f"{arm}-{k}", k


def base_record(sc: dict, arm: str, k: int, rundir: Path) -> dict:
    return {"schema": "nirvana-ab-run/1", "scenario": sc["id"], "arm": arm, "k": k, "from": utc_now(),
            "prompt": sc["prompt"], "ends_on": sc["ends_on"], "wall_limit_s": sc["wall_limit_s"],
            "model": MODEL, "effort": EFFORT, "claude": claude_version(), "incidents": [], "gaps": []}


def finish_record(rec: dict, rundir: Path) -> dict:
    rec["to"] = utc_now()
    (rundir / "run.json").write_text(json.dumps(rec, indent=1, default=str) + "\n", encoding="utf-8", newline="\n")
    return rec


def incident(rec: dict, kind: str, detail: str, at_s: float | None = None) -> None:
    rec["incidents"].append({"kind": kind, "detail": detail, **({} if at_s is None else {"at_s": round(at_s, 1)})})


def score(rec: dict, sc: dict, rundir: Path, clone: Path, base: str, branch: str, answer: str,
          served: dict | None) -> None:
    (rundir / "answer.txt").write_text(answer, encoding="utf-8", newline="\n")
    served_path = None
    if served is not None:
        served_path = rundir / "served.json"
        served_path.write_text(json.dumps(served, indent=1) + "\n", encoding="utf-8", newline="\n")
    rec["truth"] = run_truth(sc, rundir, clone, base, branch, rundir / "answer.txt", served_path)


def run_arm_c(doc: dict, sc: dict, out: Path, *, wall_s: float | None = None, herdr_exe: Path | None = None,
              owner_home: Path | None = None, extra_argv: Iterable[str] = (), keep_clones: bool = False) -> dict:
    """Arm C: `claude -p` in a fresh clone, stream-json read line by line into stream.jsonl as it arrives."""
    rundir, k = next_rundir(out, sc["id"], "C")
    rundir.mkdir(parents=True)
    rec = base_record(sc, "C", k, rundir)
    wall = float(wall_s or sc["wall_limit_s"])
    try:
        info = make_clone(sc["repo"], rundir / "clone", out)
        clone, base = Path(info["clone"]), info["base"]
        rec.update(clone=str(clone), base=base, settings=claude_settings_note())
        extra = [*(sc.get("claude_code") or {}).get("extra_argv", []), *extra_argv]
        argv = cc_argv(sc["prompt"], extra)
        rec["argv"] = argv[:1] + ["-p", "<ask>"] + argv[3:]
        env = bench_env(rundir, rundir / "home", "none", isolated=False)
        exe = Path(herdr_exe or default_herdr())
        owner = Path(owner_home or owner_home_default())
        before = read_usage(exe, rundir / "usage-home", owner)
        t0 = time.monotonic()
        rec["spawn"] = utc_now()
        proc = subprocess.Popen(argv, cwd=str(clone), env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=open(rundir / "stderr.log", "wb"))
        stream = open(rundir / "stream.jsonl", "wb")

        def pump():
            for line in proc.stdout:
                stream.write(line)
                stream.flush()
        reader = threading.Thread(target=pump, daemon=True)
        reader.start()
        served: dict | None = None
        ended = "finished"
        while True:
            try:
                proc.wait(timeout=POLL_S)
                break
            except subprocess.TimeoutExpired:
                pass
            if sc.get("ends_on") == "served" and served is None:
                served = probe_served(clone, sc)
                if served:
                    served["first_served_s"] = round(time.monotonic() - t0, 1)
            if time.monotonic() - t0 > wall:
                rec["killed"] = kill_tree(proc.pid)
                ended = "wall"
                break
        reader.join(10)
        stream.close()
        wall_s_ = time.monotonic() - t0
        rec.update(exit=proc.returncode, wall_s=round(wall_s_, 3))
        if ended == "finished" and proc.returncode != 0:
            ended = f"exit {proc.returncode}"
        rec["ended"] = ended
        if sc.get("ends_on") == "served":
            after_exit = probe_served(clone, sc)
            rec["served_after_exit"] = after_exit is not None
            served = served or after_exit
        parsed = parse_stream(parse_lines((rundir / "stream.jsonl").read_text(encoding="utf-8", errors="replace")))
        if parsed["result"] is None:
            rec["gaps"].append("no result row in the stream: the run did not finish its turn")
        tokens = parsed["tokens"]
        ru = parsed["result_usage"]
        if ru and not gross(tokens["workers"]) and not tokens["workers"]["output"]:
            # MEASURED 2026-10-18 (M1 C-1): a stream-json assistant row carries the usage as the message STARTED, so its
            # output_tokens read 48 where the result row read 1,848 (input and cache figures agreed). With no sub-agent
            # in the run the result row is the whole run; with one it is not split, so the messages' figures stay.
            tokens = {**tokens, "orchestrator": ru, "basis": "the result row's usage (no sub-agent ran)"}
        rec.update(tokens=tokens, result=parsed["result"], result_usage=parsed["result_usage"],
                   models=parsed["models"], seats=[f"{m} x{n} messages" for m, n in parsed["models"].items()])
        for lim in parsed["rate_limits"]:
            if isinstance(lim, dict) and str(lim.get("status", "allowed")) not in ("allowed", "allowed_warning"):
                incident(rec, "rate_limit", json.dumps(lim)[:200])
        if parsed["result"] and parsed["result"].get("is_error"):
            incident(rec, "error_result", str(parsed["result"])[:200])
        if parsed["permission_denials"]:
            # MEASURED 2026-10-18: print mode with the owner's settings runs Bash but denies Write and Edit until
            # `--permission-mode auto` (the mode arm N's pane starts in); a denial is an incident, never silence.
            incident(rec, "permission_denied", ", ".join(parsed["permission_denials"]))
        if ended == "wall":
            incident(rec, "wall", f"killed at {wall:g} s")
        rec["served"] = served
        score(rec, sc, rundir, clone, base, info["branch"], parsed["answer"], served)
        rec["stopped_listeners"] = stop_run_listeners(clone)
        after = read_usage(exe, rundir / "usage-home", owner)
        rec["usage"] = {"before": before, "after": after, "delta": usage_delta(before, after)}
    except Exception as e:  # noqa: BLE001 - a run that broke is a record, not a crash of the sequence
        rec["ended"] = rec.get("ended") or "driver_error"
        rec["error"] = f"{type(e).__name__}: {e}"
    if rec.get("clone"):
        rec["clone_final"] = finalize_clone(Path(rec["clone"]), rundir, keep_clones)
    return finish_record(rec, rundir)


def default_herdr() -> Path:
    return ROOT / "target" / "debug" / "herdr.exe"


def owner_home_default() -> Path:
    base = os.environ.get("LOCALAPPDATA") or str(Path.home() / "AppData" / "Local")
    return Path(base) / "Programs" / "Nirvana-Herdr"


def run_arm_n(doc: dict, sc: dict, out: Path, *, wall_s: float | None = None, herdr_exe: Path | None = None,
              plugin_dir: Path | None = None, owner_home: Path | None = None, keep_clones: bool = False) -> dict:
    """Arm N: Nirvana Claude interactive in an isolated herdr server, in a fresh clone."""
    rundir, k = next_rundir(out, sc["id"], "N")
    rundir.mkdir(parents=True)
    rec = base_record(sc, "N", k, rundir)
    wall = float(wall_s or sc["wall_limit_s"])
    exe = Path(herdr_exe or default_herdr())
    session = f"o7{sc['id'].lower()}n{k}"
    home = rundir / "home"
    h: Herdr | None = None
    clone: Path | None = None
    try:
        info = make_clone(sc["repo"], rundir / "clone", out)
        clone, base = Path(info["clone"]), info["base"]
        rec.update(clone=str(clone), base=base, herdr=str(exe), session=session)
        rec["seeded"] = seed_home(home, exe, plugin_dir or ROOT / "nirvana" / "claude", Path(owner_home or owner_home_default()))
        env = bench_env(rundir, home, session, isolated=True)
        h = Herdr(exe, env, clone)
        owner = Path(owner_home or owner_home_default())
        before = read_usage(exe, rundir / "usage-home", owner)
        rec["server_pid"] = h.start_server(rundir / "server.log")
        ws = h.json("workspace", "create", "--cwd", str(clone), "--env", "NIRVANA_ORCHESTRATOR=1", "--label", "nirvana")
        pane = ws["root_pane"]["pane_id"]
        rec["pane"] = pane
        line = powershell_line("claude", nirvana_claude_args(home))
        rec["launch"] = line
        h.call("pane", "run", pane, line)
        since = time.time() - 5
        # The prompt: answer the folder-trust prompt for this clone only, then wait for an idle prompt line.
        ready = False
        deadline = time.monotonic() + 120
        trusted: list[str] = []
        while time.monotonic() < deadline:
            time.sleep(2)
            screen = h.read(pane)
            kind = screen_kind(screen)
            if kind == "trust":
                check_trust_scope(clone, out)
                (rundir / "screen-trust.txt").write_text(strip_ansi(screen), encoding="utf-8", newline="\n")
                action = trust_action(screen)
                if action == "enter":
                    h.key(pane, "Enter")
                    if str(clone) not in trusted:
                        trusted.append(str(clone))
                elif action == "down":
                    h.key(pane, "Down")
                continue
            if kind == "idle":
                ready = True
                break
        rec["trusted_folders"] = trusted
        if not ready:
            rec["ended"] = "never_ready"
            (rundir / "screen-final.txt").write_text(strip_ansi(h.read(pane, 100)), encoding="utf-8", newline="\n")
            incident(rec, "never_ready", "no idle prompt within 120 s of the launch")
            raise StopIteration
        t_ask = time.monotonic()
        rec["ask_at"] = utc_now()
        h.send(pane, sc["prompt"])
        time.sleep(0.5)
        h.key(pane, "Enter")
        det = EndDetector()
        det.start(t_ask)
        served: dict | None = None
        verdict = None
        replies: list[str] = []
        while True:
            time.sleep(POLL_S)
            t = time.monotonic()
            status = h.pane(pane).get("agent_status", "unknown")
            screen = h.read(pane)
            kind = screen_kind(screen)
            if kind in ("working", "permission") and not (rundir / f"screen-{kind}.txt").exists():
                (rundir / f"screen-{kind}.txt").write_text(strip_ansi(screen), encoding="utf-8", newline="\n")
            if kind == "working" and not any(i["kind"] == "stall" for i in rec["incidents"]):
                seen = latest_activity(clone, since)
                if seen is not None and time.time() - seen >= STALL_S:
                    incident(rec, "stall", f"working, and no transcript or journal written for {STALL_S:g} s", t - t_ask)
            runs = {}
            for rid in sorted(p.name for p in (clone / ".nirvana" / "runs").glob("*") if p.is_dir()) \
                    if (clone / ".nirvana" / "runs").is_dir() else []:
                code, text, _ = h.call("run", "check", rid, timeout=30)
                runs[rid] = classify_check(text, code)
            if sc.get("ends_on") == "served" and served is None:
                served = probe_served(clone, sc)
                if served:
                    served["first_served_s"] = round(t - t_ask, 1)
            if re.search(r"rate limit|usage limit|limit reached|hit your limit", strip_ansi(screen), re.I) \
                    and not any(i["kind"] == "rate_limit" for i in rec["incidents"]):
                incident(rec, "rate_limit", "the pane's screen names a limit", t - t_ask)
            if t - t_ask > wall:
                verdict = ("wall", t, f"killed at {wall:g} s", [])
                break
            verdict = det.feed(t, status=status, kind=kind, runs=runs)
            if verdict and verdict[0] == "over" and sc.get("reply") and len(replies) < MAX_REPLIES \
                    and wants_reply(final_answer(session_files(clone, since)[0])):
                # The person policy: the orchestrator ended its turn on a question; answer it with the scenario's reply.
                h.send(pane, sc["reply"])
                time.sleep(0.5)
                h.key(pane, "Enter")
                replies.append(sc["reply"])
                incident(rec, "person_reply", f"answered a question with {sc['reply']!r}", t - t_ask)
                det = EndDetector()
                det.start(t)
                verdict = None
                continue
            if verdict:
                break
        ended, quiet_at, why, kinds = verdict
        rec["person_actions"] = len(replies)
        rec["ended"] = {"over": "answered" if not runs else "run_over"}.get(ended, ended)
        rec["wall_s"] = round(quiet_at - t_ask, 3)
        rec["end_why"] = why
        rec["runs_seen"] = runs
        for kd in kinds:
            incident(rec, kd, why)
        if ended in ("wall", "stuck_blocked", "no_response"):
            incident(rec, ended, why)
        (rundir / "screen-final.txt").write_text(strip_ansi(h.read(pane, 200, "recent")), encoding="utf-8", newline="\n")
        # Receipts, journals and the run folders.
        receipts = {}
        for rid in runs:
            if runs.get(rid) == "finished_unended":
                # The orchestrator left the run open (an incident already): the driver ends it so a receipt exists.
                h.call("run", "end", rid, timeout=60)
                rec.setdefault("driver_ended_runs", []).append(rid)
            code, text, _ = h.call("run", "receipt", rid, "--json", timeout=60)
            try:
                receipts[rid] = json.loads(text)
            except ValueError:
                rec["gaps"].append(f"no receipt for run {rid}: {text[:100]!r}")
        if (clone / ".nirvana" / "runs").is_dir():
            shutil.copytree(clone / ".nirvana" / "runs", rundir / "runs-journals", dirs_exist_ok=True)
        rec["receipts"] = receipts
        mains, subs = session_files(clone, since)
        if not mains:
            rec["gaps"].append("no orchestrator transcript found under " + str(claude_projects_dir() / mangle(clone)))
        tokens = transcript_tokens(mains, subs)
        wk = [receipt_workers(r) for r in receipts.values()]
        for w in wk:
            if w:
                tokens["workers"] = add_tokens(tokens["workers"], w)
        rec["tokens"] = tokens
        rec["transcripts"] = [str(p) for p in mains + subs]
        for i, p in enumerate(mains):
            shutil.copyfile(p, rundir / f"transcript-{i + 1}.jsonl")
        rec["seats"] = [s for r in receipts.values() for s in seats_of(r)]
        rec["seats"] += [f"subagent {m} x{n} messages" for m, n in models_of(subs).items()]
        if not rec["seats"]:
            rec["seats"] = ["orchestrator only"]
        # Each pool's use also from the worker records: the receipt's per-order use at the seat's price (O3 §7.4.1).
        rec["worker_use"] = [{"run": rid, "order": o.get("order"), "seat": o.get("seat"), "pool": o.get("pool"),
                              "use": o.get("use"), "tokens": o.get("tokens")}
                             for rid, r in receipts.items() for o in r.get("orders") or []]
        rec["served"] = served
        answer = final_answer(mains) or strip_ansi(h.read(pane, 120, "recent"))
        if not final_answer(mains):
            rec["gaps"].append("answer taken from the pane's screen: no assistant text in the transcript")
        score(rec, sc, rundir, clone, base, info["branch"], answer, served)
        rec["stopped_listeners"] = stop_run_listeners(clone)
        after = read_usage(exe, rundir / "usage-home", owner)
        rec["usage"] = {"before": before, "after": after, "delta": usage_delta(before, after)}
    except StopIteration:
        pass
    except Exception as e:  # noqa: BLE001
        rec["ended"] = rec.get("ended") or "driver_error"
        rec["error"] = f"{type(e).__name__}: {e}"
    finally:
        if h is not None:
            try:
                rec["server_stop"] = h.stop_server()
            except Exception as e:  # noqa: BLE001
                rec["server_stop"] = {"error": f"{type(e).__name__}: {e}"}
        if clone is not None:
            rec.setdefault("stopped_listeners", stop_run_listeners(clone))
            rec["clone_final"] = finalize_clone(clone, rundir, keep_clones)
    return finish_record(rec, rundir)


# ---- the table --------------------------------------------------------------------------------------------------------


def _med(values: list) -> float | None:
    vals = [v for v in values if v is not None]
    return statistics.median(vals) if vals and len(vals) == len(values) else None


def _fmt(x) -> str:
    return NOT_MEASURED if x is None else (f"{x:,.0f}" if isinstance(x, (int, float)) and abs(x) >= 1000 else f"{x:g}")


def _cell(text) -> str:
    return str(text).replace("|", "/").replace("\n", " ")


def run_gross(rec: dict) -> int | None:
    t = rec.get("tokens")
    if not t:
        return None
    return (gross(t.get("orchestrator")) or 0) + (gross(t.get("workers")) or 0)


def frac(rec: dict) -> float | None:
    t = rec.get("truth")
    if not t or t.get("result") not in ("PASS", "FAIL"):
        return None
    total = t["passed"] + len(t["failed"])
    return t["passed"] / total if total else None


def load_runs(out: Path) -> dict[str, list[dict]]:
    """{scenario: [run.json ...]} for every run directory under <out>/runs; a directory with no run.json is reported by
    `table` as a gap, not skipped silently."""
    found: dict[str, list[dict]] = {}
    base = out / "runs"
    for sdir in sorted(base.glob("*")) if base.is_dir() else []:
        for rdir in sorted(sdir.glob("*-*")):
            rj = rdir / "run.json"
            if rj.is_file():
                rec = json.loads(rj.read_text(encoding="utf-8"))
                rec["dir"] = f"{sdir.name}/{rdir.name}"
                found.setdefault(sdir.name, []).append(rec)
            else:
                found.setdefault(sdir.name, []).append({"dir": f"{sdir.name}/{rdir.name}", "arm": rdir.name[:1],
                                                        "ended": "no run.json", "incidents": [], "gaps": ["no run.json"]})
    return found


def token_line(rec: dict) -> str:
    """The four kinds for the orchestrator and for workers: `M1/N-1: orchestrator in 8 / cache write 56,546 / ...`."""
    t = rec.get("tokens")
    if not t:
        return f"{rec['dir']}: tokens {NOT_MEASURED}"

    def four(k: dict) -> str:
        return (f"in {k.get('input', 0):,} / cache write {k.get('cache_write', 0):,} / "
                f"cache read {k.get('cache_read', 0):,} / out {k.get('output', 0):,}")
    return f"{rec['dir']}: orchestrator {four(t.get('orchestrator') or {})}; workers {four(t.get('workers') or {})}"


def _delta_text(rec: dict, window: str) -> str:
    d = ((rec.get("usage") or {}).get("delta") or {}).get("max", {}).get(window)
    if d is None:
        return NOT_MEASURED
    return d if isinstance(d, str) else f"{d * 100:+.2f} pts"


def render_table(out: Path, doc: dict) -> str:
    runs = load_runs(out)
    lines = ["# Plan 12 O7: Nirvana Claude against plain Claude Code (generated)", "",
             f"- out: `{out}`", f"- scenario file sha256: `{hashlib.sha256(SCENARIOS.read_bytes()).hexdigest()}`",
             f"- both arms: {MODEL} at {EFFORT}; gross tokens = input + cache write + cache read (output excluded)",
             "- Claude 5h and week: whole-percent readings of the whole account before and after the run, so another "
             "session's use in the span is inside them; the token columns are the meter",
             ""]
    for sc in doc["scenarios"]:
        recs = runs.get(sc["id"], [])
        lines += [f"## {sc['id']} ({sc.get('class', '-')})", ""]
        if not recs:
            lines += ["No run directories.", ""]
            continue
        lines += ["| run | arm | ended | wall s | truth | failed rules | orchestrator gross | workers gross | "
                  "person | Claude 5h | Claude week | seats | incidents |", "|" + "---|" * 13]
        for r in recs:
            t = r.get("truth")
            truth = "not read" if not t else f"{t['result']} ({t['passed']} pass, {len(t['failed'])} fail)"
            tk = r.get("tokens") or {}
            seats = "; ".join(r.get("seats") or []) or NOT_MEASURED
            inc = "; ".join(f"{i['kind']}" for i in r.get("incidents") or []) or "-"
            cells = [r["dir"], r.get("arm"), r.get("ended"), _fmt(r.get("wall_s")), truth,
                     ", ".join(t["failed"]) if t and t.get("failed") else "-", _fmt(gross(tk.get("orchestrator"))),
                     _fmt(gross(tk.get("workers"))), _fmt(r.get("person_actions")), _delta_text(r, "5h"),
                     _delta_text(r, "week"), seats, inc]
            lines.append("| " + " | ".join(_cell(c) for c in cells) + " |")
        lines += ["", "Tokens by kind (fresh input, cache write, cache read, output), orchestrator and workers apart:", ""]
        lines += [f"- {token_line(r)}" for r in recs]
        lines.append("")
        by_arm = {a: [r for r in recs if r.get("arm") == a] for a in ARMS}
        for a, label in (("N", "Nirvana Claude (N)"), ("C", "plain Claude Code (C)")):
            rs = by_arm[a]
            if rs:
                lines.append(f"- {label}: runs {len(rs)}, median wall {_fmt(_med([r.get('wall_s') for r in rs]))} s, "
                             f"median pass fraction {_fmt(_med([frac(r) for r in rs]))}, "
                             f"median gross {_fmt(_med([run_gross(r) for r in rs]))}")
        need = int(sc.get("runs_per_arm", 3))
        n, c = by_arm["N"], by_arm["C"]
        if len(n) < need or len(c) < need:
            lines.append(f"- verdict: insufficient runs (n<{need}); N {len(n)}, C {len(c)}")
        else:
            nf, cf = _med([frac(r) for r in n]), _med([frac(r) for r in c])
            if nf is None or cf is None:
                lines.append("- truth parity: not measured (a run has no truth verdict)")
            else:
                lines.append(f"- truth parity: N {_fmt(nf)}, C {_fmt(cf)}: {'at parity or better' if nf >= cf else 'below C'}")
            ng, cg = _med([run_gross(r) for r in n]), _med([run_gross(r) for r in c])
            if ng is None or cg in (None, 0):
                lines.append("- usage ratio: not measured")
            else:
                ratio, tgt, ceil = ng / cg, sc["target"]["ratio"], sc["ceiling"]["ratio"]
                band = "within target" if ratio <= tgt else ("over target" if ratio <= ceil else "over ceiling")
                lines.append(f"- usage ratio: N gross / C gross = {ratio:.2f} (target {tgt:g}, ceiling {ceil:g}): {band}")
        lines.append("")
    return "\n".join(lines) + "\n"


# ---- the command line -------------------------------------------------------------------------------------------------


def guard_reading(herdr_exe: Path, owner_home: Path, scratch: Path) -> dict:
    return read_usage(herdr_exe, scratch / "guard-home", owner_home)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="ab.py", description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("run", "sequence"):
        p = sub.add_parser(name)
        p.add_argument("out", type=Path)
        if name == "run":
            p.add_argument("scenario")
            p.add_argument("arm", choices=ARMS)
        else:
            p.add_argument("--scenarios", default="M1,M2,M5")
            p.add_argument("--runs", type=int, default=3)
            p.add_argument("--arms", default="N,C")
        p.add_argument("--wall", type=float, default=None)
        p.add_argument("--herdr", type=Path, default=default_herdr())
        p.add_argument("--plugin", type=Path, default=ROOT / "nirvana" / "claude")
        p.add_argument("--owner-home", type=Path, default=owner_home_default())
        p.add_argument("--allow-unknown-window", action="store_true")
        p.add_argument("--keep-clones", action="store_true", help="keep each run's clone (700 MB each)")
        p.add_argument("--no-guard", action="store_true", help="skip the usage guard (recorded in run.json)")
        p.add_argument("--c-arg", action="append", default=[], help="an extra argument for arm C's claude")
    t = sub.add_parser("table")
    t.add_argument("out", type=Path)
    u = sub.add_parser("usage")
    u.add_argument("--herdr", type=Path, default=default_herdr())
    u.add_argument("--owner-home", type=Path, default=owner_home_default())
    ns = ap.parse_args(argv)
    doc = load_scenarios()
    if ns.cmd == "table":
        text = render_table(ns.out, doc)
        (ns.out / "README.md").write_text(text, encoding="utf-8", newline="\n")
        print(text)
        return 0
    if ns.cmd == "usage":
        snap = guard_reading(ns.herdr, ns.owner_home, Path(os.environ.get("TEMP", ".")) / "o7-usage")
        print(json.dumps(snap, indent=1))
        ok, why = guard(snap)
        print(("OK: " if ok else "REFUSED: ") + why)
        return 0 if ok else 3
    out = ns.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    refuse_inside(out, doc["scenarios"][0]["repo"]["source"], "the out directory")

    def one(sid: str, arm: str) -> dict:
        sc = scenario(doc, sid)
        if not ns.no_guard:
            snap = guard_reading(ns.herdr, ns.owner_home, out)
            ok, why = guard(snap, allow_unknown=ns.allow_unknown_window)
            (out / f"guard-{sid}-{arm}-{int(time.time())}.json").write_text(
                json.dumps({"ok": ok, "why": why, "snapshot": snap}, indent=1), encoding="utf-8", newline="\n")
            if not ok:
                print(f"guard refused {sid} {arm}: {why}")
                raise SystemExit(3)
            print(f"guard: {why}")
        runner = run_arm_n if arm == "N" else run_arm_c
        kwargs = dict(wall_s=ns.wall, herdr_exe=ns.herdr, owner_home=ns.owner_home, keep_clones=ns.keep_clones)
        if arm == "N":
            kwargs["plugin_dir"] = ns.plugin
        else:
            kwargs["extra_argv"] = ns.c_arg
        rec = runner(doc, sc, out, **kwargs)
        t = rec.get("truth") or {}
        print(f"{rec['scenario']} {rec['arm']}-{rec['k']}: {rec.get('ended')} wall {rec.get('wall_s')} s truth "
              f"{t.get('result')} ({t.get('passed')} pass, {len(t.get('failed') or [])} fail) "
              f"gross {run_gross(rec)} incidents {[i['kind'] for i in rec['incidents']]}")
        return rec

    if ns.cmd == "run":
        one(ns.scenario, ns.arm)
        return 0
    for sid in ns.scenarios.split(","):
        for k in range(ns.runs):
            for arm in ns.arms.split(","):
                if len(list((out / "runs" / sid).glob(f"{arm}-*"))) > k:
                    continue            # resumable: this run exists
                one(sid, arm)
    print(render_table(out, doc))
    return 0


if __name__ == "__main__":
    sys.exit(main())
