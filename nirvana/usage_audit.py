"""Plan 40 AUDIT (2026-11-09): how Nirvana has been used over its lifetime, per model, from its own logs.

CLAUDE.md rule 4: a count in prose rots, so every number in `plans/2026-11-09-40-records/usage-audit.md` comes from this
script, which recomputes it from the sources each time. It reads, never writes:

1. herdr's seat record, `<home>/nirvana/record.jsonl` (one line per finished order), `usage.json` beside it.
2. the run journals and receipts, `<project>/.nirvana/runs/<run>/journal.jsonl|receipt.json`, the order files under
   `.nirvana/orders/`, and the names and sizes (never the bytes) of `.nirvana/uploads/`.
3. Claude Code's transcripts, `~/.claude/projects/<folder>/*.jsonl` and their `subagents/` files, streamed line by line.
4. old Nirvana's `~/.nirvana/events.sqlite`, opened read-only (`mode=ro`).

It prints aggregates only: no conversation text, no prompt, no tool output. A tool result is measured by its length.
An image is measured by its base64 length and its pixel size, which the script reads from the first bytes of the image
header (PNG IHDR, JPEG SOF, GIF, WebP); the image itself is never shown to anyone.

    PYTHONIOENCODING=utf-8 python nirvana/usage_audit.py [--json] [--since YYYY-MM-DD] [--now <unix seconds>]
        [--home <herdr home>] [--claude-projects <dir>] [--old-nirvana <events.sqlite>] [--project <root> ...]

Known traps the parsers handle, each from a measurement:
- One API response is written as several transcript lines (one per content block), each repeating the same `usage`.
  Usage counts once per `message.id` + `requestId`; content blocks count per line (each line carries a different one).
- A resumed or forked session copies earlier lines into a new file: lines are deduplicated by `uuid`.
- Cache writes come in two lifetimes, 5 minutes (1.25x input) and 1 hour (2x input); Claude Code writes `usage.
  cache_creation.ephemeral_1h_input_tokens`. The 1 h rate is applied where the split is given.
- The seat table's Sonnet 5.5 cache read is $0.20 per MTok; the list price is $0.10 (claude-api skill, cached
  2026-11-03). Fable 5.2 is $8 / $40, cache read $0.20; the table names only Fable 5.1. Both are corrected below and
  said in the output.
"""

import argparse
import base64
import bisect
import json
import os
import re
import sqlite3
import statistics
import struct
import sys
import time
from collections import Counter, defaultdict
from pathlib import Path

try:
    import tomllib
except ImportError:  # Python < 3.11
    tomllib = None

HERE = Path(__file__).resolve().parent
SEATS_TOML = HERE.parent / "src" / "nirvana_seats.toml"

DEFAULT_PROJECTS = [
    r"C:\Users\fahim\Desktop\Pojects\3D-Printer Flow",
    r"C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow-xtool",
    r"C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow-xtool-xcs",
    r"C:\Users\fahim\Desktop\Pojects\groove-coach",
    r"C:\Users\fahim\Desktop\Pojects\WorkoutApp\WorkOutAppRefactor",
    r"D:\MotoProject",
]

# ---- prices ------------------------------------------------------------------------------------------------------

# USD per million tokens: input, output, cache read, cache write (5 min). The 1 h write is 2x input.
# Corrections to the seat table, each with its source.
PRICE_FIXES = {
    "claude-sonnet-5-5": {"cache_read": 0.10, "basis": "list price $0.10 cache read (claude-api skill 2026-11-03); "
                                                       "the seat table says 0.20"},
}
# Models the seat table does not name. "list" ones are from the claude-api skill (cached 2026-11-03); "assumed" ones
# take cache read at 0.1x and the 5 min write at 1.25x input, which the skill does not state for them.
EXTRA_PRICES = {
    "claude-fable-5-2": (8.0, 40.0, 0.20, 10.0, "list: $8/$40, cache read $0.20; write 1.25x input"),
    "claude-mythos-5-2": (8.0, 40.0, 0.20, 10.0, "list, as Fable 5.2"),
    "claude-fable-5-1": (10.0, 50.0, 0.25, 12.5, "list $10/$50, the seat table's row before 2026-11-09"),
    "claude-fable-5": (10.0, 50.0, 0.25, 12.5, "list $10/$50; cache as Fable 5.1"),
    "claude-opus-5": (5.0, 25.0, 0.50, 6.25, "list $5/$25; cache assumed"),
    "claude-opus-4-8": (5.0, 25.0, 0.50, 6.25, "list $5/$25; cache assumed"),
    "claude-opus-4-7": (5.0, 25.0, 0.50, 6.25, "list $5/$25; cache assumed"),
    "claude-opus-4-6": (5.0, 25.0, 0.50, 6.25, "list $5/$25; cache assumed"),
    "claude-sonnet-5": (2.0, 10.0, 0.20, 2.5, "list $2/$10; cache assumed"),
    "claude-sonnet-4-6": (3.0, 15.0, 0.30, 3.75, "list $3/$15; cache assumed"),
    "claude-haiku-4-5": (1.0, 5.0, 0.10, 1.25, "list $1/$5; cache assumed"),
}
# Haiku 5.5's list price is for prompts up to 100K tokens; above, $0.50 / $2.50 (5x). Cache rates scaled the same,
# an assumption.
HAIKU_LONG = ("claude-haiku-5-5", 100_000, 5.0)


def load_seat_table(path=SEATS_TOML):
    """The seat table's seats: id -> {model, cli, pool, prices (latest), use priors}. tomllib when present."""
    if tomllib is None or not Path(path).is_file():
        return {}
    with open(path, "rb") as f:
        data = tomllib.load(f)
    seats = {}
    for s in data.get("seat", []):
        prices = s.get("prices") or []
        seats[s["id"]] = {
            "model": s.get("model"), "cli": s.get("cli"), "pool": s.get("pool"),
            "prices": prices, "use": s.get("use", []),
        }
    return seats


class Prices:
    """Prices by model id (transcripts) and by seat id (the record). A turn on an unknown model costs 0 and is listed."""

    def __init__(self, seats):
        self.by_model, self.basis, self.unpriced = {}, {}, Counter()
        self.seat_prices = {}
        for sid, s in seats.items():
            if not s["prices"]:
                continue
            # The newest row whose date has come. Plan 40 merge: the table gained dated rows (Sonnet's cache read,
            # Fable 5.2), and taking row 0 priced `claude-fable-5-2` at Fable 5.1's $10/$50. A row dated in the
            # future (Argon's 2026-11-30 doubling) is not yet a price.
            today = time.strftime("%Y-%m-%d", time.gmtime())
            due = [row for row in s["prices"] if str(row.get("from", "")) <= today]
            p = dict(due[-1] if due else s["prices"][0])
            model = s["model"].replace("-{effort}", "")
            fix = PRICE_FIXES.get(model, {})
            row = (p["input"], p["output"], fix.get("cache_read", p["cache_read"]), p["cache_write"])
            self.seat_prices[sid] = row
            if s["cli"] == "claude" and "{" not in s["model"]:
                self.by_model[model] = row
                self.basis[model] = "seat table " + sid + ("; " + fix["basis"] if fix else "")
            elif s["cli"] == "agy" and s.get("pool") in ("gemini", "argon"):
                self.by_model[model] = row
                self.basis[model] = "seat table " + sid
        for model, (i, o, cr, cw, basis) in EXTRA_PRICES.items():
            if model not in self.by_model:
                self.by_model[model] = (i, o, cr, cw)
                self.basis[model] = basis

    @staticmethod
    def norm(model):
        """`claude-opus-5-5[1m]`, `claude-haiku-4-5-20251001`, `gemini-3.8-flash-low` -> the priced id."""
        if not model:
            return ""
        m = model.split("[")[0].strip()
        m = re.sub(r"-20\d{6}$", "", m)
        m = re.sub(r"-(low|medium|high|xhigh|max)$", "", m)
        return m

    def row(self, model):
        return self.by_model.get(self.norm(model))

    def cost(self, model, inp, out, cr, cw, cw_1h=0, ctx=None, count_unpriced=True):
        """List-price USD of one turn or one total. `cw` is every cache write; `cw_1h` the part written for 1 h."""
        row = self.row(model)
        if row is None:
            if count_unpriced:
                self.unpriced[self.norm(model) or "?"] += inp + out + cr + cw
            return 0.0
        i, o, r, w = row
        if ctx is not None and self.norm(model) == HAIKU_LONG[0] and ctx > HAIKU_LONG[1]:
            i, o, r, w = (x * HAIKU_LONG[2] for x in (i, o, r, w))
        cw_1h = min(cw_1h, cw)
        return (inp * i + out * o + cr * r + (cw - cw_1h) * w + cw_1h * 2 * i) / 1e6

    def seat_cost(self, seat, model, t):
        row = self.seat_prices.get(seat)
        if row is None:
            return self.cost(model, t["input"], t["output"], t["cache_read"], t["cache_write"])
        i, o, r, w = row
        return (t["input"] * i + t["output"] * o + t["cache_read"] * r + t["cache_write"] * w) / 1e6


# ---- small helpers -----------------------------------------------------------------------------------------------

def jsonl(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                try:
                    yield json.loads(line)
                except ValueError:
                    continue
    except OSError:
        return


def iso_to_unix(ts):
    if not ts:
        return None
    try:
        from datetime import datetime
        return datetime.fromisoformat(ts.replace("Z", "+00:00")).timestamp()
    except ValueError:
        return None


def pct(xs, q):
    if not xs:
        return None
    xs = sorted(xs)
    k = (len(xs) - 1) * q
    lo, hi = int(k), min(int(k) + 1, len(xs) - 1)
    return xs[lo] + (xs[hi] - xs[lo]) * (k - lo)


def med(xs):
    return statistics.median(xs) if xs else None


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * ((p * (1 - p) / n + z * z / (4 * n * n)) ** 0.5) / d
    return (max(0.0, c - h), min(1.0, c + h))


def tok_total(t):
    return t.get("input", 0) + t.get("output", 0) + t.get("cache_read", 0) + t.get("cache_write", 0)


def day(ts):
    return time.strftime("%Y-%m-%d", time.gmtime(ts)) if ts else "?"


# ---- image header probe (numbers only) ---------------------------------------------------------------------------

def image_dims(b64):
    """(width, height) from the first bytes of a base64 image, or None. Reads only the header."""
    try:
        head = base64.b64decode(b64[:65536][: (min(len(b64), 65536) // 4) * 4], validate=False)
    except (ValueError, TypeError):
        return None
    if head[:8] == b"\x89PNG\r\n\x1a\n" and len(head) >= 24:
        return struct.unpack(">II", head[16:24])
    if head[:6] in (b"GIF87a", b"GIF89a") and len(head) >= 10:
        return struct.unpack("<HH", head[6:10])
    if head[:4] == b"RIFF" and head[8:12] == b"WEBP" and len(head) >= 30:
        kind = head[12:16]
        if kind == b"VP8X":
            w = 1 + int.from_bytes(head[24:27], "little")
            h = 1 + int.from_bytes(head[27:30], "little")
            return (w, h)
        if kind == b"VP8 " and len(head) >= 30:
            w, h = struct.unpack("<HH", head[26:30])
            return (w & 0x3FFF, h & 0x3FFF)
        if kind == b"VP8L" and len(head) >= 25:
            b = head[21:25]
            w = 1 + (((b[1] & 0x3F) << 8) | b[0])
            h = 1 + (((b[3] & 0xF) << 10) | (b[2] << 2) | ((b[1] & 0xC0) >> 6))
            return (w, h)
    if head[:2] == b"\xff\xd8":
        i = 2
        while i + 9 < len(head):
            if head[i] != 0xFF:
                i += 1
                continue
            marker = head[i + 1]
            if marker in (0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF):
                h, w = struct.unpack(">HH", head[i + 5:i + 9])
                return (w, h)
            if marker in (0xD8, 0x01) or 0xD0 <= marker <= 0xD7:
                i += 2
                continue
            seg = struct.unpack(">H", head[i + 2:i + 4])[0]
            i += 2 + seg
    return None


def image_tokens(dims, b64_len):
    """Estimated input tokens of one image: Anthropic's published rule, tokens = w*h/750, after the API scales the
    long edge to at most 1568 px and the area to about 1.15 MP. Without a readable header: base64 bytes / 750 x 0.75,
    a crude stand-in, said in the output."""
    if dims:
        w, h = dims
        if w <= 0 or h <= 0:
            return 0
        s = min(1.0, 1568 / max(w, h), (1_150_000 / (w * h)) ** 0.5)
        return int(w * s * h * s / 750)
    return int(b64_len * 0.75 / 750)


# ---- session classification --------------------------------------------------------------------------------------

def folder_kind(folder, project_folders):
    """The group a transcript folder implies, before the session's own marks (worker id, Nirvana title) are known."""
    f = folder
    if f.startswith("C--Users-fahim--nirvana-"):
        return "old-nirvana-transcripts"
    if "-Temp-bench-" in f or "jev-ab" in f or "-o7-bench-" in f:
        return "bench-probe"
    if "pytest-of-" in f:
        return "tests"
    if "AppData-Local-Temp" in f or "Nirvana-Herdr-nirvana-preload-probe" in f or "scratch-workspaces" in f:
        return "bench-probe"
    if f.startswith("C--Users-fahim-Desktop-Pojects-Nirvana"):
        return "harness-build"
    for pf in project_folders:
        if f == pf or f.startswith(pf + "--nirvana-worktrees-"):
            return "same-project"
    return "other-projects"


def project_folder_name(root):
    """Claude Code's folder name for a project root: every non-alphanumeric character becomes '-'."""
    return re.sub(r"[^A-Za-z0-9]", "-", str(root))


# ---- source 1 and 2: the record and the journals -----------------------------------------------------------------

def load_record(home):
    return list(jsonl(Path(home) / "nirvana" / "record.jsonl"))


def load_journals(roots):
    """Every run's journal events, receipts, order files and uploads, per project root."""
    runs, uploads = [], {}
    for root in roots:
        nv = Path(root) / ".nirvana"
        up = nv / "uploads"
        if up.is_dir():
            files = [p for p in up.rglob("*") if p.is_file()]
            uploads[str(root)] = {"files": len(files), "bytes": sum(p.stat().st_size for p in files),
                                  "names": sorted(p.name for p in files)}
        rdir = nv / "runs"
        if not rdir.is_dir():
            continue
        for run in sorted(p for p in rdir.iterdir() if p.is_dir()):
            events = list(jsonl(run / "journal.jsonl"))
            receipt = None
            try:
                receipt = json.loads((run / "receipt.json").read_text(encoding="utf-8"))
            except (OSError, ValueError):
                pass
            runs.append({"project": str(root), "run": run.name, "events": events, "receipt": receipt})
    return runs, uploads


def worker_sessions(runs):
    """Claude Code session id -> (project, run, order) from the journals' `usage` events, which name the worker's
    session. The orchestrator's usage events carry no order and are not workers."""
    out = {}
    for r in runs:
        for e in r["events"]:
            if e.get("event") == "usage" and e.get("order") and e.get("session"):
                out.setdefault(e["session"], (r["project"], r["run"], e["order"]))
    return out


# ---- source 3: Claude Code transcripts ---------------------------------------------------------------------------

READ_EXT_IMAGE = (".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp")


class SessionAgg:
    __slots__ = ("sid", "folder", "title_nirvana", "entry", "turns", "first_user_chars", "tools", "reads",
                 "images", "agent_calls", "agent_models", "seat_runs", "seat_picks", "workflow_calls", "files")

    def __init__(self, sid, folder):
        self.sid, self.folder = sid, folder
        self.title_nirvana = False
        self.entry = Counter()
        # (ts, model, input, output, cache_read, cache_write, cw_1h, effort, chain) where chain is "main" or an agent id
        self.turns = []
        self.first_user_chars = None
        self.tools = Counter()          # category -> calls
        self.reads = Counter()          # (path, offset, limit) -> times
        self.images = []                # (ts, chain, est_tokens, b64_len, via)
        self.agent_calls = Counter()    # "explicit model" / "inherit"
        self.agent_models = Counter()
        self.seat_runs = 0
        self.seat_picks = 0
        self.workflow_calls = 0
        self.files = set()


class TranscriptScan:
    def __init__(self, prices, since=None, until=None):
        self.prices, self.since, self.until = prices, since, until
        self.sessions = {}
        self.seen_msg, self.seen_uuid = set(), set()
        self.tool_use = {}  # tool_use_id -> (category, session, path-key, model)
        self.tool_results = defaultdict(list)  # category -> [est tokens]
        self.repeat_read_tokens = Counter()     # session -> tokens of re-reads
        self.effort_by_req = {}
        self.stats = Counter()
        self.mm = Counter()  # (session, chain, kind) -> n ; filled per session later
        self.mm_rows = []    # (session, chain, kind, model, est_tokens, bytes)

    def session(self, sid, folder):
        s = self.sessions.get(sid)
        if s is None:
            s = self.sessions[sid] = SessionAgg(sid, folder)
        return s

    @staticmethod
    def tool_category(name, inp):
        if name == "Read":
            p = str(inp.get("file_path", "")).lower()
            if p.endswith(READ_EXT_IMAGE):
                return "Read:image"
            if p.endswith(".pdf"):
                return "Read:pdf"
            return "Read:text"
        if name.startswith("mcp__"):
            parts = name.split("__")
            return "mcp:" + (parts[1] if len(parts) > 1 else "?")
        return name

    def scan_file(self, path, folder, chain_hint=None):
        """One transcript file. `chain_hint` is the subagent's id for a `subagents/` file."""
        self.stats["files"] += 1
        sid_file = path.stem
        try:
            f = open(path, encoding="utf-8", errors="replace")
        except OSError:
            return
        with f:
            for line in f:
                self.stats["lines_read"] += 1
                if '"type":"assistant"' in line:
                    kind = "assistant"
                elif '"type":"user"' in line:
                    kind = "user"
                elif '"custom-title"' in line or '"agent-name"' in line:
                    kind = "title"
                elif '"type":"api-request"' in line and '"effort"' in line:
                    kind = "req"
                else:
                    continue
                try:
                    d = json.loads(line)
                except ValueError:
                    self.stats["bad_lines"] += 1
                    continue
                t = d.get("type")
                sid = d.get("sessionId") or sid_file
                if t == "custom-title" or t == "agent-name":
                    if (d.get("customTitle") or d.get("agentName")) == "Nirvana":
                        self.session(sid, folder).title_nirvana = True
                    continue
                if t == "api-request":
                    eff = ((d.get("params") or {}).get("output_config") or {}).get("effort")
                    if d.get("id") and eff:
                        self.effort_by_req[d["id"]] = eff
                    continue
                if t not in ("assistant", "user"):
                    continue
                uuid = d.get("uuid")
                if uuid:
                    if uuid in self.seen_uuid:
                        self.stats["dup_lines"] += 1
                        continue
                    self.seen_uuid.add(uuid)
                ts = iso_to_unix(d.get("timestamp"))
                if (self.since and ts and ts < self.since) or (self.until and ts and ts > self.until):
                    continue
                s = self.session(sid, folder)
                self.stats["lines"] += 1  # user and assistant lines inside the window: stable under a past --now
                if d.get("entrypoint"):
                    s.entry[d["entrypoint"]] += 1
                chain = chain_hint or (("side:" + str(d.get("agentId") or "?")) if d.get("isSidechain") else "main")
                msg = d.get("message") or {}
                if t == "assistant":
                    self.assistant(s, d, msg, ts, chain)
                else:
                    self.user(s, d, msg, ts, chain)

    def assistant(self, s, d, msg, ts, chain):
        model = msg.get("model") or ""
        key = (msg.get("id"), d.get("requestId"))
        u = msg.get("usage")
        if u and model != "<synthetic>" and key[0] and key not in self.seen_msg:
            self.seen_msg.add(key)
            cc = u.get("cache_creation") or {}
            cw = u.get("cache_creation_input_tokens") or 0
            eff = self.effort_by_req.get(d.get("requestRef")) or "?"
            s.turns.append((ts or 0, model, u.get("input_tokens") or 0, u.get("output_tokens") or 0,
                            u.get("cache_read_input_tokens") or 0, cw, cc.get("ephemeral_1h_input_tokens") or 0,
                            eff, chain))
            self.stats["turns"] += 1
            if eff != "?":
                self.stats["turns_with_effort"] += 1
        elif key in self.seen_msg:
            self.stats["repeated_usage_lines"] += 1
        for b in msg.get("content") or []:
            if not isinstance(b, dict) or b.get("type") != "tool_use":
                continue
            name, inp = b.get("name") or "?", b.get("input") or {}
            if not isinstance(inp, dict):
                inp = {}
            cat = self.tool_category(name, inp)
            s.tools[cat] += 1
            pkey = None
            if name == "Read":
                pkey = (str(inp.get("file_path")), inp.get("offset"), inp.get("limit"), inp.get("pages"))
                s.reads[pkey] += 1
                if cat in ("Read:image", "Read:pdf"):
                    self.mm_rows.append((s.sid, chain, cat, model, 0, 0))
            elif name in ("Agent", "Task"):
                s.agent_calls["explicit model" if inp.get("model") else "inherit"] += 1
                if inp.get("model"):
                    s.agent_models[str(inp["model"])] += 1
            elif name == "Workflow":
                s.workflow_calls += 1
            elif name in ("Bash", "PowerShell"):
                cmd = str(inp.get("command", ""))
                # Measured in the transcripts: Nirvana Claude calls herdr through a variable (`& $h seat run ...`),
                # so a command counts when it names herdr anywhere and the verb anywhere.
                if "herdr" in cmd and re.search(r"\bseat\s+run\b", cmd):
                    s.seat_runs += 1
                if "herdr" in cmd and re.search(r"\bseat\s+pick\b", cmd):
                    s.seat_picks += 1
                if "herdr" in cmd and re.search(r"(^|[\s;&|])media\s+\w", cmd):
                    self.mm_rows.append((s.sid, chain, "herdr media", model, 0, 0))
                if "screenshot" in cmd.lower():
                    self.mm_rows.append((s.sid, chain, "screenshot (shell)", model, 0, 0))
            if name == "mcp__bladebro__vision":
                self.mm_rows.append((s.sid, chain, "bladebro vision", model, 0, 0))
            elif "screenshot" in name.lower() or (name.endswith("__computer") and inp.get("action") == "screenshot"):
                self.mm_rows.append((s.sid, chain, "screenshot (tool)", model, 0, 0))
            if b.get("id"):
                self.tool_use[b["id"]] = (cat, s.sid, pkey, model)

    def user(self, s, d, msg, ts, chain):
        content = msg.get("content")
        if isinstance(content, str):
            if s.first_user_chars is None and chain == "main" and not d.get("isMeta"):
                s.first_user_chars = len(content)
            return
        if not isinstance(content, list):
            return
        texts = 0
        for b in content:
            if not isinstance(b, dict):
                continue
            bt = b.get("type")
            if bt == "text":
                texts += len(b.get("text") or "")
            elif bt == "image":
                self.image(s, b, ts, chain, "pasted", "")
            elif bt == "document":
                self.mm_rows.append((s.sid, chain, "document block", "", 0, len(str((b.get("source") or {})
                                                                                        .get("data") or ""))))
            elif bt == "tool_result":
                tu = self.tool_use.get(b.get("tool_use_id"))
                cat = tu[0] if tu else "?"
                inner = b.get("content")
                chars, img_tok = 0, 0
                if isinstance(inner, str):
                    chars = len(inner)
                elif isinstance(inner, list):
                    for x in inner:
                        if not isinstance(x, dict):
                            continue
                        if x.get("type") == "text":
                            chars += len(x.get("text") or "")
                        elif x.get("type") == "image":
                            img_tok += self.image(s, x, ts, chain, "tool:" + cat, tu[3] if tu else "")
                        elif x.get("type") == "document":
                            self.mm_rows.append((s.sid, chain, "document block", tu[3] if tu else "", 0,
                                                 len(str((x.get("source") or {}).get("data") or ""))))
                est = chars // 4 + img_tok
                self.tool_results[cat].append(est)
                if tu and tu[2] is not None and s.reads.get(tu[2], 0) > 1:
                    self.repeat_read_tokens[s.sid] += est
        if s.first_user_chars is None and chain == "main" and texts and not d.get("isMeta"):
            s.first_user_chars = texts

    def image(self, s, b, ts, chain, via, model):
        src = b.get("source") or {}
        data = src.get("data") or ""
        if not isinstance(data, str):
            return 0
        dims = image_dims(data) if data else None
        est = image_tokens(dims, len(data))
        if dims is None:
            self.stats["images_no_header"] += 1
        s.images.append((ts or 0, chain, est, len(data), via))
        self.mm_rows.append((s.sid, chain, "image block", model, est, len(data) * 3 // 4))
        return est

    def scan_all(self, root, progress=True):
        root = Path(root)
        folders = sorted(p for p in root.iterdir() if p.is_dir())
        t0 = time.time()
        for n, folder in enumerate(folders, 1):
            for path in sorted(folder.glob("*.jsonl")):
                self.scan_file(path, folder.name)
                sub = folder / path.stem / "subagents"
                if sub.is_dir():
                    # Agent-tool subagents sit in subagents/, Workflow agents in subagents/workflows/wf_*/ beside
                    # the workflow's own journal.jsonl, which is not a transcript.
                    for sp in sorted(sub.rglob("*.jsonl")):
                        if sp.name == "journal.jsonl":
                            continue
                        wf = "workflows" in sp.relative_to(sub).parts
                        self.stats["workflow_agent_files" if wf else "subagent_files"] += 1
                        self.scan_file(sp, folder.name, chain_hint=("workflow:" if wf else "agent:") + sp.stem)
            if progress and (n % 20 == 0 or n == len(folders)):
                print(f"  transcripts: {n}/{len(folders)} folders, {self.stats['lines_read']:,} lines, "
                      f"{time.time() - t0:.0f} s", file=sys.stderr)


def classify(s, workers, project_folders):
    """A session's group. The journals' worker ids win, then the Nirvana title, then the folder."""
    kind = folder_kind(s.folder, project_folders)
    if s.sid in workers:
        return "fork-worker"
    if s.title_nirvana:
        return "fork-orchestrator" if kind in ("same-project", "other-projects") else kind + ":orchestrator"
    if kind == "same-project":
        if s.entry.get("sdk-cli", 0) > s.entry.get("cli", 0):
            return "fork-print-other"
        return "plain-cc-same-projects"
    return kind


# ---- source 4: old Nirvana ---------------------------------------------------------------------------------------

def load_old_nirvana(path, since=None):
    """Old Nirvana's finished turns from `events.sqlite`: claude `result` events carry `modelUsage` per model (with
    Claude Code's own list cost); agy `result` events carry usage, and the order's `spawn` names the model."""
    out = {"turns": [], "verdicts": Counter(), "error": None}
    if not Path(path).is_file():
        out["error"] = "not found"
        return out
    try:
        con = sqlite3.connect(f"file:{Path(path).as_posix()}?mode=ro", uri=True)
        spawn_model = {}
        for wo, b in con.execute("select e.work_order_id, p.bytes from events e join payloads p on "
                                 "p.ref=e.payload_ref where e.event_type='spawn'"):
            try:
                d = json.loads(bytes(b))
                spawn_model[wo] = (d.get("model"), d.get("effort"))
            except ValueError:
                pass
        rows = con.execute("select e.work_order_id, e.provider, e.ts, p.bytes from events e join payloads p on "
                           "p.ref=e.payload_ref where e.event_type='result' and e.actor='tailer'")
        for wo, prov, ts, b in rows:
            at = iso_to_unix(ts)
            if since and at and at < since:
                continue
            try:
                d = json.loads(bytes(b))
            except ValueError:
                continue
            eff = (spawn_model.get(wo) or (None, None))[1] or "?"
            if prov == "claude":
                for model, mu in (d.get("modelUsage") or {}).items():
                    out["turns"].append({"at": at, "model": model, "effort": eff,
                                         "input": mu.get("inputTokens", 0), "output": mu.get("outputTokens", 0),
                                         "cache_read": mu.get("cacheReadInputTokens", 0),
                                         "cache_write": mu.get("cacheCreationInputTokens", 0),
                                         "cc_cost": mu.get("costUSD", 0.0)})
            else:
                res = d.get("result")
                if not isinstance(res, dict):
                    continue  # a result line with no usage object (an error text): no tokens to count
                u = res.get("usage") or {}
                model = (spawn_model.get(wo) or ("?", None))[0] or "?"
                out["turns"].append({"at": at, "model": model, "effort": eff, "input": u.get("input_tokens", 0),
                                     "output": u.get("output_tokens", 0), "cache_read": u.get("cache_read_tokens", 0),
                                     "cache_write": 0, "cc_cost": None})
        for ts, b in con.execute("select e.ts, p.bytes from events e join payloads p on p.ref=e.payload_ref "
                                 "where e.event_type='verdict'"):
            at = iso_to_unix(ts)
            if since and at and at < since:
                continue
            try:
                out["verdicts"][json.loads(bytes(b)).get("verdict")] += 1
            except ValueError:
                pass
        con.close()
    except sqlite3.Error as e:
        out["error"] = str(e)
    return out


# ---- the audit ---------------------------------------------------------------------------------------------------

def audit(args):
    now = args.now or time.time()
    since = None
    if args.since:
        since = time.mktime(time.strptime(args.since, "%Y-%m-%d")) - time.timezone
    recent = now - 14 * 86400
    seats = load_seat_table(args.seats)
    prices = Prices(seats)
    roots = args.project or DEFAULT_PROJECTS
    project_folders = [project_folder_name(r) for r in roots]
    R = {"meta": {"now": int(now), "since": args.since, "recent_from": day(recent), "projects": roots,
                  "prices": {m: {"usd_per_mtok": prices.by_model[m], "basis": prices.basis[m]}
                             for m in sorted(prices.by_model)}}}

    # `--now` is also a cutoff: live sessions keep writing while the script reads (measured: two runs a minute apart
    # differed only in harness-build rows), so a past `--now` makes two runs print the same bytes.
    until = args.now
    def inside(at):
        return (not since or (at or 0) >= since) and (not until or (at or 0) <= until)
    print("reading the record and the journals", file=sys.stderr)
    record = [r for r in load_record(args.home) if inside(r.get("at", 0))]
    runs, uploads = load_journals(roots)
    if since or until:
        for r in runs:
            r["events"] = [e for e in r["events"] if inside(e.get("at", 0))]
        runs = [r for r in runs if r["events"]]
    workers = worker_sessions(runs)
    cli_of = {(r["run"], e.get("order")): e.get("cli") for r in runs for e in r["events"] if e.get("event") == "seated"}
    claude_workers = {sid for sid, (_, run, order) in workers.items() if cli_of.get((run, order)) == "claude"}

    print("reading old Nirvana", file=sys.stderr)
    old = load_old_nirvana(args.old_nirvana, since)
    old["turns"] = [t for t in old["turns"] if inside(t["at"])]

    scan = TranscriptScan(prices, since, until)
    if not args.no_transcripts and Path(args.claude_projects).is_dir():
        print("reading Claude Code transcripts", file=sys.stderr)
        scan.scan_all(args.claude_projects)
    groups = {sid: classify(s, workers, project_folders) for sid, s in scan.sessions.items()}

    R["sources"] = {
        "record_lines": len(record), "runs": len(runs), "journal_events": sum(len(r["events"]) for r in runs),
        "receipts": sum(1 for r in runs if r["receipt"]), "worker_sessions_named_by_journals": len(workers),
        "worker_sessions_found_in_transcripts": sum(1 for g in groups.values() if g == "fork-worker"),
        "claude_worker_sessions_named_by_journals": len(claude_workers),
        "transcript_files_all": scan.stats["files"], "subagent_files": scan.stats["subagent_files"],
        "workflow_agent_files": scan.stats["workflow_agent_files"],
        "transcript_message_lines": scan.stats["lines"], "api_responses": scan.stats["turns"],
        "usage_lines_repeating_a_response": scan.stats["repeated_usage_lines"],
        "duplicate_lines_by_uuid": scan.stats["dup_lines"], "bad_lines": scan.stats["bad_lines"],
        "responses_with_effort": scan.stats["turns_with_effort"],
        "old_nirvana_results": len(old["turns"]), "old_nirvana_error": old["error"],
        "sessions_by_group": dict(sorted(Counter(groups.values()).items())),
    }

    # -- A: spend per era, model and effort
    rows = defaultdict(lambda: Counter())
    def add(era, model, effort, t, cost, ts, n=1, cc_cost=None):
        for scope in ("lifetime", "14d") if ts and ts >= recent else ("lifetime",):
            k = (scope, era, Prices.norm(model) or model, effort)
            c = rows[k]
            c["n"] += n
            for f in ("input", "output", "cache_read", "cache_write"):
                c[f] += t[f]
            c["usd"] += cost
            if cc_cost is not None:
                c["cc_usd"] += cc_cost
    for t in old["turns"]:
        tt = {f: t[f] or 0 for f in ("input", "output", "cache_read", "cache_write")}
        add("old-nirvana(sqlite)", t["model"], t["effort"], tt,
            prices.cost(t["model"], tt["input"], tt["output"], tt["cache_read"], tt["cache_write"]), t["at"],
            cc_cost=t["cc_cost"])
    for r in record:
        if r.get("seat") in ("sonnet", "opus", "haiku", "fable"):
            continue  # a claude worker is counted from its transcript below; its record line is the cross-check
        add("fork-worker(agy, record)", r["model"], r.get("effort", "?"), r["tokens"],
            prices.seat_cost(r["seat"], r["model"], r["tokens"]), r.get("at"))
    turn_cost = {}
    for sid, s in scan.sessions.items():
        g = groups[sid]
        for tr in s.turns:
            ts, model, i, o, cr, cw, cw1h, eff, chain = tr
            c = prices.cost(model, i, o, cr, cw, cw1h, ctx=i + cr + cw)
            turn_cost[id(tr)] = c
            era = g + (" (subagents)" if chain != "main" else "")
            add(era, model, eff, {"input": i, "output": o, "cache_read": cr, "cache_write": cw}, c, ts)
    total = {sc: sum(c["usd"] for (scope, era, _, _), c in rows.items() if scope == sc
                     and not era.startswith("old-nirvana-transcripts")) for sc in ("lifetime", "14d")}
    A = []
    for (scope, era, model, eff), c in sorted(rows.items(), key=lambda kv: (kv[0][0], -kv[1]["usd"])):
        A.append({"scope": scope, "era": era, "model": model, "effort": eff, "n": c["n"], "input": c["input"],
                  "output": c["output"], "cache_read": c["cache_read"], "cache_write": c["cache_write"],
                  "usd": round(c["usd"], 2), "cc_usd": round(c["cc_usd"], 2) if c["cc_usd"] else None,
                  "share": round(c["usd"] / total[scope], 4) if total[scope]
                  and not era.startswith("old-nirvana-transcripts") else None})
    era_tot = defaultdict(Counter)
    for a in A:
        e = era_tot[(a["scope"], a["era"].replace(" (subagents)", ""))]
        e["usd"] += a["usd"]
        e["n"] += a["n"]
        e["sub_usd"] += a["usd"] if a["era"].endswith("(subagents)") else 0
        e["tokens"] += a["input"] + a["output"] + a["cache_read"] + a["cache_write"]
    R["A_spend_rows"] = A
    R["A_by_era"] = [{"scope": sc, "era": era, "n": c["n"], "tokens": c["tokens"], "usd": round(c["usd"], 2),
                      "of_which_subagents_usd": round(c["sub_usd"], 2),
                      "share": round(c["usd"] / total[sc], 4) if total[sc] and
                      not era.startswith("old-nirvana-transcripts") else None}
                     for (sc, era), c in sorted(era_tot.items(), key=lambda kv: (kv[0][0], -kv[1]["usd"]))]
    R["A_totals_usd"] = {k: round(v, 2) for k, v in total.items()}
    R["A_unpriced_tokens"] = dict(prices.unpriced.most_common())
    # where the "other" and harness spend sits: the dearest transcript folders, lifetime
    folder_usd = defaultdict(Counter)
    for sid, s in scan.sessions.items():
        f = folder_usd[(s.folder, groups[sid])]
        for tr in s.turns:
            c = turn_cost.get(id(tr), 0.0)
            f["usd"] += c
            f["sub_usd"] += c if tr[8] != "main" else 0.0
            f["n"] += 1
    R["A_top_folders"] = [{"folder": k[0], "group": k[1], "n": v["n"], "usd": round(v["usd"], 2),
                           "subagent_usd": round(v["sub_usd"], 2)}
                          for k, v in sorted(folder_usd.items(), key=lambda kv: -kv[1]["usd"])[:15]]
    by_model = defaultdict(Counter)
    for a in A:
        if a["era"].startswith("old-nirvana-transcripts"):
            continue
        m = by_model[(a["scope"], a["model"])]
        m["usd"] += a["usd"]
        m["n"] += a["n"]
    R["A_by_model"] = [{"scope": sc, "model": m, "n": c["n"], "usd": round(c["usd"], 2),
                        "share": round(c["usd"] / total[sc], 4) if total[sc] else None}
                       for (sc, m), c in sorted(by_model.items(), key=lambda kv: (kv[0][0], -kv[1]["usd"]))]

    # cross-check: the record's claude worker tokens against the transcripts of the same sessions
    rec_claude = Counter()
    for r in record:
        if r.get("seat") in ("sonnet", "opus", "haiku", "fable"):
            for f in ("input", "output", "cache_read", "cache_write"):
                rec_claude[f] += r["tokens"].get(f, 0)
    tr_claude = Counter()
    for sid, s in scan.sessions.items():
        if groups[sid] == "fork-worker":
            for tr in s.turns:
                for f, v in zip(("input", "output", "cache_read", "cache_write"), (tr[2], tr[3], tr[4], tr[5])):
                    tr_claude[f + (":subagents" if tr[8] != "main" else "")] += v
    R["A_check_claude_workers"] = {"record": dict(rec_claude), "transcripts": dict(tr_claude)}

    # the Max week against list price: readings of `claude /usage` in the journals and usage.json
    R["A_week_calibration"] = week_calibration(runs, args.home, scan, prices, until)

    # -- B: outcome by seat and class
    first_verdict, verdicts_by_order, parents = {}, defaultdict(list), {}
    for r in runs:
        for e in r["events"]:
            if e.get("event") == "gated":
                key = (r["run"], e.get("order"))
                verdicts_by_order[key].append(e.get("verdict"))
                first_verdict.setdefault(key, e.get("verdict"))
            elif e.get("event") == "seated" and e.get("parent"):
                parents[(r["run"], e.get("order"))] = e.get("parent")
    B = []
    cells = defaultdict(list)
    for r in record:
        cells[(r["seat"], r["class"])].append(r)
    for (seat, cls), rs in sorted(cells.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        n = len(rs)
        # Measured 2026-11-09: 55 record lines say `failed` where the journal's last verdict is `accepted` (the
        # orchestrator took the delivery with a reason, no axes); nirvana_lessons.rs PASS_WORDS counts `accepted` as a
        # pass. Both are shown: the record's own pass, and pass with accepted.
        accepted = sum(1 for r in rs if r["outcome"] != "pass"
                       and (verdicts_by_order.get((r["run"], r["order"])) or [None])[-1] == "accepted")
        passes = sum(1 for r in rs if r["outcome"] == "pass") + accepted
        record_passes = passes - accepted
        fg = [first_verdict.get((r["run"], r["order"])) for r in rs]
        fg_known = [v for v in fg if v]
        cost = sum(prices.seat_cost(r["seat"], r["model"], r["tokens"]) for r in rs)
        toks = sum(tok_total(r["tokens"]) for r in rs)
        dur = [r["ended_at"] - r["seated_at"] for r in rs if r.get("ended_at") and r.get("seated_at")]
        lo, hi = wilson(passes, n)
        B.append({"seat": seat, "class": cls, "n": n, "record_pass": record_passes, "accepted": accepted,
                  "pass": passes, "pass_rate": round(passes / n, 3),
                  "pass_ci95": [round(lo, 2), round(hi, 2)],
                  "first_gate_pass": sum(1 for v in fg_known if v == "pass"),
                  "first_gate_known": len(fg_known),
                  "sent_back": sum(1 for r in rs if r["outcome"] == "sent_back"),
                  "failed": sum(1 for r in rs if r["outcome"] == "failed"),
                  "blocked": sum(1 for r in rs if r["outcome"] == "blocked"),
                  "subagent_no_report": sum(1 for r in rs if r["outcome"].startswith("subagent")),
                  "send_backs": sum(r.get("send_backs") or 0 for r in rs),
                  "reseats": sum(r.get("reseats") or 0 for r in rs),
                  "stalls": sum(r.get("stalls") or 0 for r in rs),
                  "rate_limit_stops": sum(r.get("rate_limit_stops") or 0 for r in rs),
                  "median_min": round(med(dur) / 60, 1) if dur else None,
                  "tokens": toks, "usd": round(cost, 2),
                  "tokens_per_pass": int(toks / passes) if passes else None,
                  "usd_per_pass": round(cost / passes, 3) if passes else None})
    R["B_seat_class"] = B
    R["B_outcomes"] = dict(Counter(r["outcome"] for r in record).most_common())
    R["B_cheap_vs_dear"] = cheap_vs_dear(B, prices)
    R["B_priors"] = priors(record)

    # -- C: orchestrator against workers
    R["C_runs"] = orchestrator_share(runs, record, prices, scan, groups)
    R["C_orchestrator_sessions"] = orch_sessions(scan, groups, prices)

    # -- D: waste
    R["D_waste"] = waste(scan, groups, prices)
    R["D_tool_results"] = {cat: {"n": len(v), "est_tokens": sum(v), "p50": int(pct(v, .5)), "p90": int(pct(v, .9)),
                                 "max": max(v)} for cat, v in sorted(scan.tool_results.items(),
                                                                     key=lambda kv: -sum(kv[1])) if v}
    R["D_repeat_reads"] = repeat_reads(scan, groups)
    R["D_briefs"] = briefs(scan, groups, runs, workers)
    R["D_first_turn"] = first_turn(runs)

    # -- E: multimodal
    R["E_multimodal"] = multimodal(scan, groups, prices, record, runs, uploads)

    # -- F: output quality
    R["F_quality"] = quality(record, runs, first_verdict, verdicts_by_order, parents, old, roots)

    # -- G: Fable
    R["G_fable"] = fable(scan, groups, prices, record, runs, args.home, until)
    return R


def week_calibration(runs, home, scan, prices, now):
    """Pairs of `claude /usage` readings of the Max week (same reset) against the list price of every Claude Code turn
    on this machine between them. The ratio is herdr's `per_usd` for max.week. It is an upper bound on the true rate:
    use on other machines or on claude.ai is in the reading and not in the transcripts."""
    readings = {}
    def take(rd):
        if now and rd.get("read_at", 0) > now:
            return
        if rd.get("pool") == "max" and rd.get("window") in ("week", "week.fable") and rd.get("read_at"):
            readings[(rd["window"], rd["read_at"])] = (rd.get("used"), rd.get("resets_at"))
    for r in runs:
        for e in r["events"]:
            for rd in e.get("pools") or []:
                if isinstance(rd, dict):
                    take(rd)
    try:
        u = json.loads((Path(home) / "nirvana" / "usage.json").read_text(encoding="utf-8"))
        for rd in ((u.get("sources") or {}).get("claude") or {}).get("readings") or []:
            take(rd)
    except (OSError, ValueError):
        pass
    times = []
    for sid, s in scan.sessions.items():
        for tr in s.turns:
            if tr[0]:
                times.append((tr[0], prices.cost(tr[1], tr[2], tr[3], tr[4], tr[5], tr[6], ctx=tr[2] + tr[4] + tr[5],
                                                 count_unpriced=False), tr[1]))
    times.sort()
    ts_only = [t[0] for t in times]
    cum, fcum, acc, facc = [], [], 0.0, 0.0
    for t in times:
        acc += t[1]
        facc += t[1] if Prices.norm(t[2]).startswith("claude-fable") else 0.0
        cum.append(acc)
        fcum.append(facc)
    def spend(a, b, fable_only=False):
        arr = fcum if fable_only else cum
        i, j = bisect.bisect_right(ts_only, a), bisect.bisect_right(ts_only, b)
        return (arr[j - 1] if j else 0.0) - (arr[i - 1] if i else 0.0)
    out = []
    by_reset = defaultdict(list)
    for (window, at), (used, resets) in readings.items():
        if used is not None and resets:
            # one week's reset is written hours apart by different readers (measured: two rows for the week of
            # 2026-10-30 at hour grain); a weekly window resets once a day at most, so the UTC day names the week
            by_reset[(window, int(resets // 86400) * 86400)].append((at, used))
    for (window, resets), rs in sorted(by_reset.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        rs.sort()
        a, b = rs[0], rs[-1]
        if b[0] - a[0] < 3600 or b[1] - a[1] <= 0:
            continue
        usd = spend(a[0], b[0], fable_only=(window == "week.fable"))
        out.append({"window": window, "resets": day(resets), "from": day(a[0]), "to": day(b[0]),
                    "readings": len(rs), "used_from": a[1], "used_to": b[1], "list_usd_between": round(usd, 2),
                    "share_per_usd": round((b[1] - a[1]) / usd, 6) if usd > 0 else None,
                    "usd_per_full_window": round(usd / (b[1] - a[1]), 0) if b[1] > a[1] else None})
    return out


def cheap_vs_dear(B, prices):
    """Per class with two or more seats at n >= 5: does the cheaper seat pass as often (95% intervals overlap) and
    what does a pass cost on each? Cheapness is the seat's list price per output MTok."""
    out = []
    by_class = defaultdict(list)
    for b in B:
        if b["n"] >= 5:
            by_class[b["class"]].append(b)
    for cls, bs in sorted(by_class.items()):
        if len(bs) < 2:
            continue
        bs.sort(key=lambda b: (prices.seat_prices.get(b["seat"]) or (0, 0))[1])
        cheap = bs[0]
        for dear in bs[1:]:
            overlap = cheap["pass_ci95"][1] >= dear["pass_ci95"][0] and dear["pass_ci95"][1] >= cheap["pass_ci95"][0]
            out.append({"class": cls, "cheap": cheap["seat"], "cheap_n": cheap["n"],
                        "cheap_pass_rate": cheap["pass_rate"], "cheap_usd_per_pass": cheap["usd_per_pass"],
                        "dear": dear["seat"], "dear_n": dear["n"], "dear_pass_rate": dear["pass_rate"],
                        "dear_usd_per_pass": dear["usd_per_pass"], "within_noise": overlap,
                        "cheap_costs_more_per_pass": (cheap["usd_per_pass"] or 1e9) > (dear["usd_per_pass"] or 1e9)})
    return out


def priors(record):
    """The seat table's `expected` share against the `use` herdr booked from the order's real tokens, per seat and
    effort and per window. `use` is tokens priced by the table and turned into a share by the pool's `per_usd`, so
    this measures the token prior, not the quota reading."""
    out = []
    cells = defaultdict(list)
    for r in record:
        cells[(r["seat"], r.get("effort"), r.get("size"))].append(r)
    for (seat, eff, size), rs in sorted(cells.items(), key=lambda kv: (kv[0][0], str(kv[0][1]), str(kv[0][2]))):
        for w in ("week", "5h"):
            ratios = [r["use"][w] / r["expected"][w] for r in rs
                      if (r.get("expected") or {}).get(w) and (r.get("use") or {}).get(w) is not None]
            if not ratios:
                continue
            out.append({"seat": seat, "effort": eff, "size": size, "window": w, "n": len(ratios),
                        "median_ratio": round(med(ratios), 2), "p10": round(pct(ratios, .1), 2),
                        "p90": round(pct(ratios, .9), 2),
                        "sum_ratio": round(sum(r["use"][w] for r in rs if (r.get("use") or {}).get(w) is not None and
                                               (r.get("expected") or {}).get(w)) /
                                           sum(r["expected"][w] for r in rs if (r.get("expected") or {}).get(w) and
                                               (r.get("use") or {}).get(w) is not None), 2)})
    return out


def worker_totals(events):
    """(order, attempt) -> tokens. A worker's `usage` events are running totals of its stream (measured: an order's
    events summed to 15x its record line), so each attempt counts its last event, as the receipt does
    (nirvana_run.rs: "the stream's last usage in each attempt, summed")."""
    last = {}
    for e in events:
        if e.get("event") == "usage" and e.get("order"):
            last[(e["order"], e.get("attempt", 1))] = e.get("tokens") or {}
    return last


def orchestrator_share(runs, record, prices, scan=None, groups=None):
    """Per run: workers from the journal (last usage event per attempt), the orchestrator two ways. (1) the journal's
    usage events with no order, the receipt's own basis, which Nirvana Claude's hooks write once per turn; priced as
    Opus 5.5 because the journal does not name the orchestrator's model. (2) every API response of a Nirvana Claude
    session in the run's project folder between the run's first and last event, priced at its own model; a response
    inside two runs' windows goes to the run opened last."""
    seat_of = {(r["run"], r["order"]): (r["seat"], r["model"]) for r in record}
    F = ("input", "output", "cache_read", "cache_write")
    # (2) the transcripts' orchestrator responses, by project folder
    orch_turns = defaultdict(list)
    if scan is not None:
        for sid, s in scan.sessions.items():
            if groups.get(sid) != "fork-orchestrator":
                continue
            for tr in s.turns:
                if tr[0]:
                    orch_turns[s.folder].append(tr)
    windows = defaultdict(list)  # folder -> [(start, end, run index)]
    for i, r in enumerate(runs):
        ats = [e.get("at") for e in r["events"] if e.get("at")]
        if ats:
            windows[project_folder_name(r["project"])].append((min(ats), max(ats), i))
    tr_orch = defaultdict(lambda: [0, 0.0, 0.0])  # run index -> [tokens, usd, subagent usd]
    for folder, trs in orch_turns.items():
        ws = sorted(windows.get(folder, []))
        for tr in trs:
            best = None
            for a, b, i in ws:
                if a <= tr[0] <= b:
                    best = i  # sorted by start: the last match is the run opened last
            if best is None:
                continue
            c = prices.cost(tr[1], tr[2], tr[3], tr[4], tr[5], tr[6], ctx=tr[2] + tr[4] + tr[5], count_unpriced=False)
            tr_orch[best][0] += tr[2] + tr[3] + tr[4] + tr[5]
            tr_orch[best][1] += c
            if tr[8] != "main":
                tr_orch[best][2] += c
    per_run = []
    tot = Counter()
    for i, r in enumerate(runs):
        orch = Counter()
        seated = {e.get("order"): (e.get("seat"), e.get("model")) for e in r["events"] if e.get("event") == "seated"}
        for e in r["events"]:
            if e.get("event") == "usage" and not e.get("order"):
                for f in F:
                    orch[f] += (e.get("tokens") or {}).get(f, 0)
        wt, wcost = 0, 0.0
        for (order, _), t in worker_totals(r["events"]).items():
            tt = {f: t.get(f, 0) for f in F}
            seat, model = seated.get(order) or seat_of.get((r["run"], order)) or ("?", "?")
            wt += sum(tt.values())
            wcost += prices.seat_cost(seat, model, tt)
        ocost = prices.cost("claude-opus-5-5", orch["input"], orch["output"], orch["cache_read"], orch["cache_write"],
                            count_unpriced=False)
        ot = sum(orch.values())
        t_tok, t_usd, t_sub = tr_orch.get(i, [0, 0.0, 0.0])
        if ot + wt + t_tok == 0:
            continue
        per_run.append({"project": Path(r["project"]).name, "run": r["run"], "orders": len(seated),
                        "worker_tokens": wt, "worker_usd": round(wcost, 2),
                        "orch_journal_tokens": ot, "orch_journal_usd_as_opus": round(ocost, 2),
                        "orch_transcript_tokens": t_tok, "orch_transcript_usd": round(t_usd, 2),
                        "orch_transcript_subagent_usd": round(t_sub, 2),
                        "orch_share_usd_journal": round(ocost / (ocost + wcost), 3) if ocost + wcost else None,
                        "orch_share_usd_transcript": round(t_usd / (t_usd + wcost), 3) if t_usd + wcost else None})
        tot["ot"] += ot
        tot["wt"] += wt
        tot["ocost"] += ocost
        tot["wcost"] += wcost
        tot["t_tok"] += t_tok
        tot["t_usd"] += t_usd
        tot["t_sub"] += t_sub
    sj = [p["orch_share_usd_journal"] for p in per_run if p["orch_share_usd_journal"] is not None]
    st = [p["orch_share_usd_transcript"] for p in per_run if p["orch_share_usd_transcript"] is not None]
    return {"runs": len(per_run), "worker_tokens": tot["wt"], "worker_usd": round(tot["wcost"], 2),
            "orch_journal_tokens": tot["ot"], "orch_journal_usd_as_opus": round(tot["ocost"], 2),
            "orch_transcript_tokens": tot["t_tok"], "orch_transcript_usd": round(tot["t_usd"], 2),
            "orch_transcript_subagent_usd": round(tot["t_sub"], 2),
            "orch_share_usd_journal": round(tot["ocost"] / (tot["ocost"] + tot["wcost"]), 3)
            if tot["ocost"] + tot["wcost"] else None,
            "orch_share_usd_transcript": round(tot["t_usd"] / (tot["t_usd"] + tot["wcost"]), 3)
            if tot["t_usd"] + tot["wcost"] else None,
            "median_run_share_journal": round(med(sj), 3) if sj else None,
            "median_run_share_transcript": round(med(st), 3) if st else None,
            "runs_orch_over_half_transcript": sum(1 for x in st if x > 0.5),
            "per_run": sorted(per_run, key=lambda p: -(p["orch_transcript_usd"] + p["worker_usd"]))}


def orch_sessions(scan, groups, prices):
    """Nirvana Claude's own sessions: main chain against its Agent-tool subagents, the subagents' models, and how
    many Agent spawns it made beside its `herdr seat run` calls."""
    out = {"sessions": 0, "main_usd": 0.0, "subagent_usd": 0.0, "main_models": Counter(),
           "subagent_models_usd": Counter(), "agent_calls": Counter(), "agent_explicit_models": Counter(),
           "seat_runs": 0, "seat_picks": 0, "workflow_calls": 0, "subagent_same_model_as_main_usd": 0.0}
    for sid, s in scan.sessions.items():
        if groups[sid] != "fork-orchestrator":
            continue
        out["sessions"] += 1
        main_model = Counter(tr[1] for tr in s.turns if tr[8] == "main").most_common(1)
        main_model = main_model[0][0] if main_model else ""
        for tr in s.turns:
            c = prices.cost(tr[1], tr[2], tr[3], tr[4], tr[5], tr[6], ctx=tr[2] + tr[4] + tr[5], count_unpriced=False)
            if tr[8] == "main":
                out["main_usd"] += c
                out["main_models"][Prices.norm(tr[1])] += 1
            else:
                out["subagent_usd"] += c
                out["subagent_models_usd"][Prices.norm(tr[1])] += c
                if tr[1] == main_model:
                    out["subagent_same_model_as_main_usd"] += c
        out["agent_calls"].update(s.agent_calls)
        out["agent_explicit_models"].update(s.agent_models)
        out["seat_runs"] += s.seat_runs
        out["seat_picks"] += s.seat_picks
        out["workflow_calls"] += s.workflow_calls
    for k in ("main_usd", "subagent_usd", "subagent_same_model_as_main_usd"):
        out[k] = round(out[k], 2)
    out["subagent_models_usd"] = {k: round(v, 2) for k, v in out["subagent_models_usd"].most_common()}
    for k in ("main_models", "agent_calls", "agent_explicit_models"):
        out[k] = dict(out[k].most_common())
    # the same for every group: who spawns Agent subagents, and on what
    by_group = defaultdict(Counter)
    for sid, s in scan.sessions.items():
        g = by_group[groups[sid]]
        g["agent_inherit"] += s.agent_calls.get("inherit", 0)
        g["agent_explicit"] += s.agent_calls.get("explicit model", 0)
        g["seat_runs"] += s.seat_runs
        g["seat_picks"] += s.seat_picks
        for tr in s.turns:
            if tr[8] != "main":
                g["subagent_usd_x100"] += (100 * prices.cost(tr[1], tr[2], tr[3], tr[4], tr[5], tr[6],
                                                                ctx=tr[2] + tr[4] + tr[5], count_unpriced=False))
    out["agent_spawns_by_group"] = {g: {"inherit": c["agent_inherit"], "explicit_model": c["agent_explicit"],
                                        "herdr_seat_run": c["seat_runs"], "herdr_seat_pick": c["seat_picks"],
                                        "subagent_usd": round(c["subagent_usd_x100"] / 100, 2)}
                                    for g, c in sorted(by_group.items())}
    return out


def waste(scan, groups, prices):
    """Context size, cache ratios, cache misses after idle gaps and long sessions, per group."""
    g = defaultdict(Counter)
    for sid, s in scan.sessions.items():
        grp = groups[sid]
        c = g[grp]
        chains = defaultdict(list)
        for tr in s.turns:
            chains[tr[8]].append(tr)
        ts_all = [tr[0] for tr in s.turns if tr[0]]
        if ts_all:
            span = max(ts_all) - min(ts_all)
            c["sessions"] += 1
            if span > 8 * 3600:
                c["sessions_over_8h"] += 1
                c["tokens_in_sessions_over_8h"] += sum(tr[2] + tr[3] + tr[4] + tr[5] for tr in s.turns)
                c["usd_x100_in_sessions_over_8h"] += (100 * sum(prices.cost(tr[1], tr[2], tr[3], tr[4], tr[5],
                                                                               tr[6], ctx=tr[2] + tr[4] + tr[5],
                                                                               count_unpriced=False)
                                                                   for tr in s.turns))
        for chain, trs in chains.items():
            trs.sort(key=lambda t: t[0])
            prev = None
            for tr in trs:
                ts, model, i, o, cr, cw, cw1h, eff, _ = tr
                ctx = i + cr + cw
                c["input_all"] += ctx
                c["fresh"] += i + cw
                c["cache_read"] += cr
                if ctx > 150_000:
                    c["input_over_150k"] += ctx
                if ctx > 300_000:
                    c["input_over_300k"] += ctx
                c["turns"] += 1
                if prev is not None and ts and prev:
                    gap = ts - prev
                    if gap > 300 and ctx >= 20_000 and cw >= 0.5 * ctx:
                        band = "60m" if gap > 3600 else "5m"
                        c["miss_" + band] += 1
                        c["miss_tokens_" + band] += cw
                        row = prices.row(model)
                        if row:
                            # what the rewrite cost above a cache read of the same tokens
                            extra = (cw - cw1h) * row[3] + cw1h * 2 * row[0] - cw * row[2]
                            c["miss_usd_x100_" + band] += (100 * extra / 1e6)
                    if gap > 300:
                        c["gaps_" + ("60m" if gap > 3600 else "5m")] += 1
                prev = ts or prev
    out = {}
    for grp, c in sorted(g.items()):
        out[grp] = {"sessions": c["sessions"], "turns": c["turns"],
                    "share_input_over_150k": round(c["input_over_150k"] / c["input_all"], 3) if c["input_all"] else None,
                    "share_input_over_300k": round(c["input_over_300k"] / c["input_all"], 3) if c["input_all"] else None,
                    "cache_read_per_fresh": round(c["cache_read"] / c["fresh"], 1) if c["fresh"] else None,
                    "gaps_5_60m": c["gaps_5m"], "gaps_over_60m": c["gaps_60m"],
                    "misses_after_5_60m": c["miss_5m"], "miss_tokens_5_60m": c["miss_tokens_5m"],
                    "miss_extra_usd_5_60m": round(c["miss_usd_x100_5m"] / 100, 2),
                    "misses_after_60m": c["miss_60m"], "miss_tokens_60m": c["miss_tokens_60m"],
                    "miss_extra_usd_60m": round(c["miss_usd_x100_60m"] / 100, 2),
                    "sessions_over_8h": c["sessions_over_8h"],
                    "tokens_in_sessions_over_8h": c["tokens_in_sessions_over_8h"],
                    "usd_in_sessions_over_8h": round(c["usd_x100_in_sessions_over_8h"] / 100, 2)}
    return out


def repeat_reads(scan, groups):
    g = defaultdict(Counter)
    for sid, s in scan.sessions.items():
        extra = sum(n - 1 for n in s.reads.values() if n > 1)
        c = g[groups[sid]]
        c["reads"] += sum(s.reads.values())
        c["repeat_reads"] += extra
        c["sessions_with_repeats"] += 1 if extra else 0
        c["est_tokens_of_reads_in_repeated_keys"] += scan.repeat_read_tokens.get(sid, 0)
    return {k: dict(v) for k, v in sorted(g.items())}


def briefs(scan, groups, runs, workers):
    """The first user message of each worker session (the launch line), the order files the line names (the real
    brief), and how many of an order file's lines already appeared in an earlier order of the same run."""
    first = [s.first_user_chars // 4 for sid, s in scan.sessions.items()
             if groups[sid] == "fork-worker" and s.first_user_chars]
    order_tokens, repeat_share = [], []
    by_seat = defaultdict(list)
    for r in runs:
        seen_lines = set()
        for e in r["events"]:
            if e.get("event") != "seated" or not e.get("order_file"):
                continue
            try:
                text = Path(e["order_file"]).read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            order_tokens.append(len(text) // 4)
            by_seat[e.get("seat")].append(len(text) // 4)
            lines = [ln.strip() for ln in text.splitlines() if len(ln.strip()) > 20]
            if lines and seen_lines:
                repeat_share.append(sum(1 for ln in lines if ln in seen_lines) / len(lines))
            seen_lines.update(lines)
    return {"launch_line_tokens": {"n": len(first), "median": med(first), "p90": pct(first, .9)},
            "order_file_tokens": {"n": len(order_tokens), "median": med(order_tokens),
                                  "p90": pct(order_tokens, .9), "max": max(order_tokens) if order_tokens else None},
            "order_file_tokens_by_seat": {k: {"n": len(v), "median": med(v)} for k, v in sorted(by_seat.items(),
                                                                                          key=lambda kv: str(kv[0]))},
            "later_orders_line_repeat_share": {"n": len(repeat_share), "median": round(med(repeat_share), 3)
                                               if repeat_share else None,
                                               "p90": round(pct(repeat_share, .9), 3) if repeat_share else None}}


def first_turn(runs):
    """A worker's fixed overhead: its first `usage` event (attempt 1), input + cache write + cache read, per seat."""
    per = defaultdict(list)
    for r in runs:
        seat_of = {e.get("order"): e.get("seat") for e in r["events"] if e.get("event") == "seated"}
        done = set()
        for e in r["events"]:
            if e.get("event") == "usage" and e.get("order") and e.get("attempt", 1) == 1 and e["order"] not in done:
                done.add(e["order"])
                t = e.get("tokens") or {}
                per[seat_of.get(e["order"], "?")].append(t.get("input", 0) + t.get("cache_write", 0) +
                                                         t.get("cache_read", 0))
    return {k: {"n": len(v), "median": med(v), "p90": pct(v, .9)} for k, v in sorted(per.items(),
                                                                                    key=lambda kv: str(kv[0]))}


def dominant_models(scan):
    """(session, chain) -> the model of most of its responses: a pasted image's user line names no model."""
    out = {}
    for sid, s in scan.sessions.items():
        per = defaultdict(Counter)
        for tr in s.turns:
            per[tr[8]][tr[1]] += 1
        for chain, c in per.items():
            out[(sid, chain)] = c.most_common(1)[0][0]
    return out


def multimodal(scan, groups, prices, record, runs, uploads):
    dom = dominant_models(scan)
    rows = defaultdict(Counter)
    for sid, chain, kind, model, est, nbytes in scan.mm_rows:
        g = groups.get(sid, "?")
        role = "main" if chain == "main" else "subagent"
        model = model or dom.get((sid, chain), "")
        c = rows[(g, role, kind, Prices.norm(model) or "?")]
        c["n"] += 1
        c["est_tokens"] += est
        c["bytes"] += nbytes
    table = [{"group": g, "chain": role, "kind": kind, "model": m, "n": c["n"], "est_tokens": c["est_tokens"],
              "bytes": c["bytes"]} for (g, role, kind, m), c in sorted(rows.items())]
    # an image in a main chain is re-read with the context on every later turn of that chain (until a /clear or
    # compaction, which the transcript does not mark here): an upper bound on its cache-read tokens
    carry = defaultdict(Counter)
    for sid, s in scan.sessions.items():
        if not s.images:
            continue
        g = groups[sid]
        by_chain = defaultdict(list)
        for tr in s.turns:
            by_chain[tr[8]].append(tr[0])
        for ts, chain, est, blen, via in s.images:
            later = sum(1 for t in by_chain.get(chain, []) if t and t > ts)
            c = carry[g]
            c["images"] += 1
            c["est_tokens"] += est
            c["carry_tokens_upper"] += est * later
            row = prices.row(dom.get((sid, chain), ""))
            if row:
                c["carry_usd_x100_upper"] += (100 * est * later * row[2] / 1e6)
            c["b64_bytes"] += blen
            c["in_" + ("main" if chain == "main" else "subagent")] += 1
    record_mm = defaultdict(Counter)
    for r in record:
        if r["class"] in ("judge.visual", "build.frontend") or r["class"].startswith("media."):
            record_mm[(r["class"], r["seat"])][r["outcome"]] += 1
    named = {}
    for root, u in uploads.items():
        hits = 0
        odir = Path(root) / ".nirvana" / "orders"
        if odir.is_dir():
            for p in odir.glob("*.md"):
                try:
                    text = p.read_text(encoding="utf-8", errors="replace")
                except OSError:
                    continue
                if "uploads" in text or any(n in text for n in u["names"]):
                    hits += 1
        named[Path(root).name] = {"files": u["files"], "bytes": u["bytes"], "orders_naming_uploads": hits}
    return {"rows": table,
            "image_carry": {g: {**{k: v for k, v in c.items() if k != "carry_usd_x100_upper"},
                                "carry_usd_upper": round(c["carry_usd_x100_upper"] / 100, 2)}
                            for g, c in sorted(carry.items())},
            "record_visual_media_orders": [{"class": k[0], "seat": k[1], **dict(v)}
                                           for k, v in sorted(record_mm.items())],
            "uploads": named,
            "images_without_readable_header": scan.stats["images_no_header"]}


def quality(record, runs, first_verdict, verdicts_by_order, parents, old, roots):
    by_version = defaultdict(Counter)
    for r in record:
        v = ".".join(str(r.get("herdr", "?")).split(".")[:2])
        word = r["outcome"]
        if word != "pass" and (verdicts_by_order.get((r["run"], r["order"])) or [None])[-1] == "accepted":
            word = "accepted (record: " + word + ")"
        by_version[v][word] += 1
    jverd = Counter(v for vs in verdicts_by_order.values() for v in vs)
    passed = {k for k, vs in verdicts_by_order.items() if any(v in ("pass", "accepted") for v in vs)}
    redone = [(run, order) for (run, order), parent in parents.items() if (run, parent) in passed]
    asked_per_run = Counter()
    no_receipt, ended_runs = 0, 0
    for r in runs:
        asked_per_run[r["run"]] = sum(1 for e in r["events"] if e.get("event") == "asked")
        if not r["receipt"]:
            no_receipt += 1
    asked = list(asked_per_run.values())
    lessons = None
    try:
        sys.path.insert(0, str(HERE))
        import lessons_census
        classes = lessons_census.router_classes()
        lc = [lessons_census.census(r, classes) for r in roots]
        lessons = {Path(p["project"]).name: {k: p[k] for k in ("lessons", "live", "never_offered", "offered",
                                                              "opened", "passed_after_opened", "lesson_review_orders")}
                   for p in lc}
    except Exception as e:  # the census is another module; its failure is reported, never fatal
        lessons = {"error": str(e)}
    return {"record_outcome_by_herdr_minor": {k: dict(v) for k, v in sorted(by_version.items(),
                                                                           key=lambda kv: [int(x) if x.isdigit() else 0
                                                                                           for x in kv[0].split(".")])},
            "journal_gate_verdicts": dict(jverd.most_common()),
            "orders_gated": len(verdicts_by_order),
            "first_gate_verdicts": dict(Counter(first_verdict.values()).most_common()),
            "children_of_a_passed_order": len(redone),
            "asked_events": sum(asked), "runs_with_a_question": sum(1 for a in asked if a),
            "max_questions_in_a_run": max(asked) if asked else 0, "runs": len(runs),
            "runs_without_receipt": no_receipt,
            "old_nirvana_verdicts": dict(old["verdicts"].most_common()),
            "lessons": lessons}


def fable(scan, groups, prices, record, runs, home, until=None):
    rows = defaultdict(Counter)
    for sid, s in scan.sessions.items():
        for tr in s.turns:
            if Prices.norm(tr[1]).startswith(("claude-fable", "claude-mythos")):
                c = rows[(groups[sid], Prices.norm(tr[1]), "main" if tr[8] == "main" else "subagent")]
                c["turns"] += 1
                c["tokens"] += tr[2] + tr[3] + tr[4] + tr[5]
                c["output"] += tr[3]
                c["usd_x100"] += (100 * prices.cost(tr[1], tr[2], tr[3], tr[4], tr[5], tr[6],
                                                       count_unpriced=False))
                c["sessions"] = c["sessions"]
                c["first"] = min(c["first"] or 1e12, tr[0] or 1e12)
                c["last"] = max(c["last"], tr[0] or 0)
    fable_sessions = defaultdict(set)
    for sid, s in scan.sessions.items():
        for tr in s.turns:
            if Prices.norm(tr[1]).startswith(("claude-fable", "claude-mythos")):
                fable_sessions[(groups[sid], Prices.norm(tr[1]), "main" if tr[8] == "main" else "subagent")].add(sid)
    week = {}
    for r in runs:
        for e in r["events"]:
            for rd in e.get("pools") or []:
                if isinstance(rd, dict) and rd.get("window") == "week.fable" and rd.get("read_at"):
                    week[rd["read_at"]] = rd.get("used")
    try:
        u = json.loads((Path(home) / "nirvana" / "usage.json").read_text(encoding="utf-8"))
        for rd in ((u.get("sources") or {}).get("claude") or {}).get("readings") or []:
            if rd.get("window") == "week.fable" and (not until or rd.get("read_at", 0) <= until):
                week[rd["read_at"]] = rd.get("used")
    except (OSError, ValueError):
        pass
    by_day = {}
    for at in sorted(week):
        by_day[day(at)] = week[at]  # the day's last reading
    return {"turns": [{"group": k[0], "model": k[1], "chain": k[2], "sessions": len(fable_sessions[k]),
                       "turns": c["turns"], "tokens": c["tokens"], "output": c["output"],
                       "usd": round(c["usd_x100"] / 100, 2), "first": day(c["first"]), "last": day(c["last"])}
                      for k, c in sorted(rows.items(), key=lambda kv: -kv[1]["usd_x100"])],
            "record_orders_on_fable": sum(1 for r in record if "fable" in str(r.get("model"))),
            "week_fable_last_reading_by_day": by_day}


# ---- the text report ---------------------------------------------------------------------------------------------

def fmt_n(x):
    if x is None:
        return "-"
    if isinstance(x, str):
        return x
    if isinstance(x, bool):
        return "yes" if x else "no"
    if isinstance(x, float):
        return f"{x:,.3f}" if abs(x) < 10 else f"{x:,.2f}"
    if isinstance(x, int):
        return f"{x:,}"
    if isinstance(x, (list, tuple)):
        return "[" + ", ".join(fmt_n(v) for v in x) + "]"
    return str(x)


def table(rows, cols, out, limit=None):
    rows = rows[:limit] if limit else rows
    cells = [[fmt_n(r.get(c)) for c in cols] for r in rows]
    widths = [max([len(c)] + [len(row[i]) for row in cells]) for i, c in enumerate(cols)]
    out.append("  " + "  ".join(c.ljust(w) for c, w in zip(cols, widths)))
    for row in cells:
        out.append("  " + "  ".join(v.ljust(w) for v, w in zip(row, widths)))


def text_report(R):
    out = []
    m = R["meta"]
    out.append(f"usage audit · now {day(m['now'])} · since {m['since'] or 'the start'} · last 14 days from "
               f"{m['recent_from']}")
    out.append("")
    out.append("== sources")
    for k, v in R["sources"].items():
        out.append(f"  {k}: {v}")
    out.append("")
    out.append("== prices used (USD per MTok: input, output, cache read, cache write 5 min; 1 h write = 2x input)")
    for k, v in m["prices"].items():
        out.append(f"  {k}: {v['usd_per_mtok']} ({v['basis']})")
    out.append("")
    out.append("== A. spend by era (list price; share excludes old-nirvana-transcripts, which sqlite already counts)")
    for scope in ("lifetime", "14d"):
        out.append(f"-- {scope}: total ${R['A_totals_usd'][scope]:,.2f}")
        table([e for e in R["A_by_era"] if e["scope"] == scope], ["era", "n", "tokens", "usd",
                                                                  "of_which_subagents_usd", "share"], out)
    out.append("-- by model")
    for scope in ("lifetime", "14d"):
        out.append(f"   {scope}")
        table([e for e in R["A_by_model"] if e["scope"] == scope], ["model", "n", "usd", "share"], out)
    out.append("-- era x model x effort, lifetime, rows over 0.1% of spend (n = API responses, orders for agy and "
               "sqlite results)")
    table([a for a in R["A_spend_rows"] if a["scope"] == "lifetime" and (a["share"] or 0) >= 0.001],
          ["era", "model", "effort", "n", "input", "output", "cache_read", "cache_write", "usd", "share"], out)
    out.append("-- the same, last 14 days, over 0.1%")
    table([a for a in R["A_spend_rows"] if a["scope"] == "14d" and (a["share"] or 0) >= 0.001],
          ["era", "model", "effort", "n", "input", "output", "cache_read", "cache_write", "usd", "share"], out)
    out.append(f"-- unpriced tokens by model: {R['A_unpriced_tokens']}")
    out.append("-- the 15 dearest transcript folders, lifetime")
    table(R["A_top_folders"], ["folder", "group", "n", "usd", "subagent_usd"], out)
    out.append(f"-- check, claude workers: record {R['A_check_claude_workers']['record']}")
    out.append(f"                          transcripts {R['A_check_claude_workers']['transcripts']}")
    out.append("-- Max week against list price, between two /usage readings of one week (upper bound on per_usd)")
    table(R["A_week_calibration"], ["window", "resets", "from", "to", "readings", "used_from", "used_to",
                                    "list_usd_between", "share_per_usd", "usd_per_full_window"], out)
    out.append("")
    out.append("== B. outcome by seat and class (record)")
    out.append(f"  outcomes: {R['B_outcomes']}")
    out.append("  pass = the record's pass plus orders whose last journal verdict is accepted (the record calls them "
               "failed)")
    table(R["B_seat_class"], ["class", "seat", "n", "record_pass", "accepted", "pass", "pass_rate", "pass_ci95",
                              "first_gate_pass",
                              "first_gate_known", "sent_back", "failed", "blocked", "subagent_no_report",
                              "send_backs", "reseats", "stalls", "rate_limit_stops", "median_min", "usd",
                              "usd_per_pass", "tokens_per_pass"], out)
    out.append("-- cheap against dear, same class, n >= 5 each")
    table(R["B_cheap_vs_dear"], ["class", "cheap", "cheap_n", "cheap_pass_rate", "cheap_usd_per_pass", "dear",
                                 "dear_n", "dear_pass_rate", "dear_usd_per_pass", "within_noise",
                                 "cheap_costs_more_per_pass"], out)
    out.append("-- the table's expected use against booked use (ratio = booked / expected)")
    table(R["B_priors"], ["seat", "effort", "size", "window", "n", "median_ratio", "p10", "p90", "sum_ratio"], out)
    out.append("")
    c = R["C_runs"]
    out.append("== C. orchestrator against workers, per run")
    out.append(f"  runs {c['runs']} · workers {c['worker_tokens']:,} tokens ${c['worker_usd']:,.2f} (last usage per "
               f"attempt, priced at the seat)")
    out.append(f"  orchestrator, journal (hook usage, priced as Opus 5.5): {c['orch_journal_tokens']:,} tokens "
               f"${c['orch_journal_usd_as_opus']:,.2f} · share {c['orch_share_usd_journal']} · median run "
               f"{c['median_run_share_journal']}")
    out.append(f"  orchestrator, transcripts in the run's window: {c['orch_transcript_tokens']:,} tokens "
               f"${c['orch_transcript_usd']:,.2f} (of which its subagents ${c['orch_transcript_subagent_usd']:,.2f})"
               f" · share {c['orch_share_usd_transcript']} · median run {c['median_run_share_transcript']} · runs "
               f"over half {c['runs_orch_over_half_transcript']}")
    out.append("  the 12 dearest runs")
    table(c["per_run"][:12], ["project", "run", "orders", "worker_tokens", "worker_usd", "orch_journal_tokens",
                              "orch_transcript_tokens", "orch_transcript_usd", "orch_transcript_subagent_usd",
                              "orch_share_usd_transcript"], out)
    o = R["C_orchestrator_sessions"]
    out.append(f"  Nirvana Claude sessions (transcripts) {o['sessions']} · main ${o['main_usd']:,.2f} · subagents "
               f"${o['subagent_usd']:,.2f} (on the main chain's model ${o['subagent_same_model_as_main_usd']:,.2f})")
    out.append(f"  main chain models (responses) {o['main_models']}")
    out.append(f"  subagent spend by model {o['subagent_models_usd']}")
    out.append(f"  Agent calls {o['agent_calls']} · explicit models {o['agent_explicit_models']} · shell calls "
               f"naming herdr seat run {o['seat_runs']}, seat pick {o['seat_picks']} · Workflow calls "
               f"{o['workflow_calls']}")
    out.append("  Agent spawns by group (inherit = no model given: the subagent runs on its caller's model)")
    table([{"group": g, **v} for g, v in o["agent_spawns_by_group"].items()],
          ["group", "inherit", "explicit_model", "herdr_seat_run", "herdr_seat_pick", "subagent_usd"], out)
    out.append("")
    out.append("== D. where tokens go")
    table([{"group": g, **v} for g, v in R["D_waste"].items()],
          ["group", "sessions", "turns", "share_input_over_150k", "share_input_over_300k", "cache_read_per_fresh",
           "gaps_5_60m", "misses_after_5_60m", "miss_tokens_5_60m", "miss_extra_usd_5_60m", "gaps_over_60m",
           "misses_after_60m", "miss_tokens_60m", "miss_extra_usd_60m", "sessions_over_8h",
           "usd_in_sessions_over_8h"], out)
    out.append("-- tool results by type (estimated tokens = characters / 4; images by their pixel estimate)")
    table([{"type": k, **v} for k, v in R["D_tool_results"].items()][:25],
          ["type", "n", "est_tokens", "p50", "p90", "max"], out)
    out.append("-- repeated reads (same path, offset, limit in one session)")
    table([{"group": g, **v} for g, v in R["D_repeat_reads"].items()],
          ["group", "reads", "repeat_reads", "sessions_with_repeats", "est_tokens_of_reads_in_repeated_keys"], out)
    b = R["D_briefs"]
    out.append(f"-- worker briefs: launch line {b['launch_line_tokens']} · order file {b['order_file_tokens']}")
    out.append(f"   order file by seat {b['order_file_tokens_by_seat']}")
    out.append(f"   lines of a later order already in an earlier order of the run {b['later_orders_line_repeat_share']}")
    out.append("-- a worker's first turn (input + cache write + cache read), by seat")
    for k, v in R["D_first_turn"].items():
        out.append(f"   {k}: {v}")
    out.append("")
    e = R["E_multimodal"]
    out.append("== E. multimodal (image block = an image that entered a context, tokens by w*h/750 after the 1568 px "
               "and 1.15 MP scaling; the other kinds are calls)")
    table(e["rows"], ["group", "chain", "kind", "model", "n", "est_tokens", "bytes"], out)
    out.append("-- images re-read with their context (upper bound: every later turn of the same chain)")
    table([{"group": g, **v} for g, v in e["image_carry"].items()],
          ["group", "images", "in_main", "in_subagent", "est_tokens", "carry_tokens_upper", "carry_usd_upper",
           "b64_bytes"], out)
    out.append(f"-- images whose header the probe could not read: {e['images_without_readable_header']}")
    out.append("-- record orders of the visual and media classes, outcomes")
    for r in e["record_visual_media_orders"]:
        out.append(f"   {r}")
    out.append(f"-- uploads {e['uploads']}")
    out.append("")
    q = R["F_quality"]
    out.append("== F. output quality signals")
    out.append("-- record outcome by herdr minor version")
    for k, v in q["record_outcome_by_herdr_minor"].items():
        n = sum(v.values())
        acc = sum(c for w, c in v.items() if w.startswith("accepted"))
        out.append(f"   {k}: n {n} · pass {v.get('pass', 0)} ({v.get('pass', 0) / n:.0%}) · with accepted "
                   f"{(v.get('pass', 0) + acc) / n:.0%} · {v}")
    for k in ("journal_gate_verdicts", "orders_gated", "first_gate_verdicts", "children_of_a_passed_order",
              "asked_events", "runs_with_a_question", "max_questions_in_a_run", "runs", "runs_without_receipt",
              "old_nirvana_verdicts"):
        out.append(f"   {k}: {q[k]}")
    out.append(f"   lessons: {q['lessons']}")
    out.append("")
    g = R["G_fable"]
    out.append("== G. Fable")
    table(g["turns"], ["group", "model", "chain", "sessions", "turns", "tokens", "output", "usd", "first", "last"], out)
    out.append(f"   record orders on a Fable seat: {g['record_orders_on_fable']}")
    out.append(f"   week.fable, the day's last /usage reading: {g['week_fable_last_reading_by_day']}")
    return "\n".join(out)


def main(argv=None):
    home = Path(os.environ.get("LOCALAPPDATA", "")) / "Programs" / "Nirvana-Herdr"
    user = Path(os.path.expanduser("~"))
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("--json", action="store_true")
    p.add_argument("--since", help="YYYY-MM-DD, UTC")
    p.add_argument("--now", type=float, help="unix seconds; the 14-day rows count back from it")
    p.add_argument("--home", default=str(home))
    p.add_argument("--claude-projects", default=str(user / ".claude" / "projects"))
    p.add_argument("--old-nirvana", default=str(user / ".nirvana" / "events.sqlite"))
    p.add_argument("--seats", default=str(SEATS_TOML))
    p.add_argument("--project", action="append", help="a project root; repeat; default the six order projects")
    p.add_argument("--no-transcripts", action="store_true", help="skip the transcripts (fast; A, C-D-E-G thin)")
    args = p.parse_args(argv)
    t0 = time.time()
    R = audit(args)
    print(f"done in {time.time() - t0:.0f} s", file=sys.stderr)
    if args.json:
        print(json.dumps(R, indent=1, default=lambda o: dict(o) if isinstance(o, Counter) else str(o)))
    else:
        print(text_report(R))
    return 0


if __name__ == "__main__":
    sys.exit(main())
