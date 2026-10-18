# Copied from old Nirvana (GraftworkGames Nirvana repo, commit on int/moto-ab) bench/truth/M5.py, 2026-10-18; the held-out truth of the 2026-10-12 Moto A/B, unchanged except this line.
"""Held-out truth for moto scenario M5: a /api/health route, committed on the base branch, that tells the truth.

THE ASK (user-shaped, not the owner's words; the scenario's `source` says why): "Add a /api/health endpoint to the
editor server that returns the loaded manifest name and how many hike clips are indexed." The owner's nearest MotoProject
build ask ("Hike reel editor design", 2026-10-01) was a plan, and a plan has no objective truth.

WHAT IS CHECKED, AND ON WHAT. The COMMITTED tip of the base branch (`git archive <branch>` into a scratch folder), never
the working tree, so a change left uncommitted cannot pass. In that copy the editor is started on a free port
(8781-8799, never the scenario's 8770 and never a TEST daemon's 7960-7969) with `--no-prewarm --manifest
plan/post_alum_cave_v3.json` -- a NON-default manifest, so a route that returns a hard-coded "manifest_v4" fails, and the
name it must return is the one actually loaded. Measured in moto_facts.json: the clone serves `/` about a second after
start without media. The server is stopped in a `finally`, its whole process tree.

THE RULES are floors, never key names a worker picks: the base branch is ahead of the base (`committed_on_base`; a change
only in the tree FAILs here and the detail says it is uncommitted); the server starts; `GET /api/health` is 200 JSON;
some string value in it names the loaded manifest (`post_alum_cave_v3`); some integer value in it (not a bool) equals the
number of clips in index/hike/hike_*.json at the base (the same count /api/project's post view lists); `GET /` is
still 200 with the editor's title and `GET /api/project` still 200 (the existing routes answer).

Usage: python bench/truth/M5.py --tree <final tree> --base <base sha> [--branch master] [--answer <answer.txt>]
Prints `PASS <rule>` / `FAIL <rule> -- <detail>` per rule, then `RESULT PASS (<n> checks)` or
`RESULT FAIL (<k> of <n> failed)`. Exit 0 all pass, 1 a rule failed, 2 usage error.
"""
from __future__ import annotations

import argparse
import io
import json
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import M2  # noqa: E402

MANIFEST = "plan/post_alum_cave_v3.json"
MANIFEST_STEM = "post_alum_cave_v3"
PORTS = range(8781, 8800)
START_WAIT_S = 60


def free_port() -> int:
    for port in PORTS:
        with socket.socket() as s:
            try:
                s.bind(("127.0.0.1", port))
                return port
            except OSError:
                continue
    raise SystemExit("usage error: no free port in 8781-8799")


def get(port: int, path: str) -> tuple[int | None, str]:
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}{path}", timeout=30) as r:
            return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")
    except Exception as e:  # noqa: BLE001 -- refused, reset, timeout: all "did not answer"
        return None, f"{type(e).__name__}: {e}"


def stop(proc: subprocess.Popen) -> None:
    if proc.poll() is not None:
        return
    if sys.platform == "win32":
        subprocess.run(["taskkill", "/PID", str(proc.pid), "/T", "/F"], capture_output=True)
    else:
        proc.kill()
    try:
        proc.wait(15)
    except subprocess.TimeoutExpired:
        proc.kill()


def export(tree: Path, ref: str, into: Path) -> str:
    done = subprocess.run(["git", "-C", str(tree), "archive", "--format=tar", ref], capture_output=True, timeout=300)
    if done.returncode != 0:
        return done.stderr.decode("utf-8", "replace").strip()[:200]
    with tarfile.open(fileobj=io.BytesIO(done.stdout)) as tar:
        tar.extractall(into)
    return ""


def values(v):
    if isinstance(v, dict):
        for x in v.values():
            yield from values(x)
    elif isinstance(v, list):
        for x in v:
            yield from values(x)
    else:
        yield v


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tree", required=True)
    ap.add_argument("--base", required=True)
    ap.add_argument("--branch", default="master", help="the base branch the change must be committed on")
    ap.add_argument("--answer", default=None, help="accepted for the driver's sake; M5 judges the tree, not the answer")
    ap.add_argument("--served", default=None, help="accepted and ignored: M5 starts its own server")
    ap.add_argument("--run-dir", default=None, help="accepted and ignored: every moto truth takes it")
    try:
        args = ap.parse_args(argv)
    except SystemExit as e:
        return 0 if e.code == 0 else 2
    tree = Path(args.tree)
    if not tree.is_dir():
        print(f"usage error: --tree {tree} is not a directory")
        return 2
    rows, check = M2.runner()

    ahead = M2.git(tree, "rev-list", "--count", f"{args.base}..{args.branch}").stdout.strip()
    dirty = M2.tracked_changes(tree)
    untracked = [ln[3:] for ln in M2.git(tree, "status", "--porcelain").stdout.splitlines()
                 if ln.startswith("??") and ln[3:].strip().endswith(".py")]
    where = f"; the tree has uncommitted changes: {'; '.join(dirty + untracked)[:200]}" if dirty or untracked else ""
    check("committed_on_base", ahead.isdigit() and int(ahead) >= 1,
          f"{args.branch} is {ahead or '?'} commits ahead of {args.base[:12]}{where}")

    ids = M2.hike_ids_at(tree, args.base)
    check("the base holds the hike index", bool(ids), f"no index/hike/hike_*.json clips at {args.base[:12]}")

    scratch = Path(tempfile.mkdtemp(prefix="m5-truth-"))
    proc = None
    health: tuple[int | None, str] = (None, "the server did not start")
    root: tuple[int | None, str] = (None, "")
    project: tuple[int | None, str] = (None, "")
    started, why = False, ""
    try:
        err = export(tree, args.branch, scratch)
        if err:
            why = f"git archive {args.branch} failed: {err}"
        else:
            port = free_port()
            log = open(scratch / "m5-server.log", "w", encoding="utf-8")
            try:
                proc = subprocess.Popen([sys.executable, "editor/server/main.py", "--port", str(port), "--no-prewarm",
                                         "--manifest", MANIFEST], cwd=str(scratch), stdout=log,
                                        stderr=subprocess.STDOUT)
                t0 = time.monotonic()
                while time.monotonic() - t0 < START_WAIT_S:
                    if proc.poll() is not None:
                        break
                    root = get(port, "/")
                    if root[0] == 200:
                        started = True
                        break
                    time.sleep(0.5)
                if started:
                    health = get(port, "/api/health")
                    project = get(port, "/api/project")
            finally:
                if proc is not None:
                    stop(proc)
                log.close()
            if not started:
                tail = (scratch / "m5-server.log").read_text(encoding="utf-8", errors="replace").strip().splitlines()
                why = f"no GET / 200 within {START_WAIT_S} s; exit {proc.poll() if proc else None}; " + " | ".join(tail[-3:])
    finally:
        if proc is not None:
            stop(proc)
        shutil.rmtree(scratch, ignore_errors=True)

    check("server_starts_from_the_committed_tree", started, why[:400])
    try:
        doc = json.loads(health[1]) if health[0] == 200 else None
    except ValueError:
        doc = None
    check("health_is_200_json", doc is not None, f"GET /api/health status {health[0]!r}: {health[1][:200]}")
    vals = list(values(doc)) if doc is not None else []
    check("health_names_the_loaded_manifest", any(isinstance(v, str) and MANIFEST_STEM in v for v in vals),
          f"no string value names {MANIFEST_STEM} (loaded with --manifest {MANIFEST}); got {health[1][:200]}")
    n = len(ids)
    check("health_counts_the_hike_clips", any(isinstance(v, int) and not isinstance(v, bool) and v == n for v in vals),
          f"no integer value equals the hike index's {n} clips; got {health[1][:200]}")
    title = M2.page_title()
    check("root_still_serves_the_editor", root[0] == 200 and f"<title>{title}</title>" in root[1],
          f"GET / status {root[0]!r}")
    check("project_route_still_answers", project[0] == 200, f"GET /api/project status {project[0]!r}: {project[1][:160]}")
    return M2.finish(rows)


if __name__ == "__main__":
    sys.exit(main())
