# Copied from old Nirvana (GraftworkGames Nirvana repo, commit on int/moto-ab) bench/truth/M2.py, 2026-10-18; the held-out truth of the 2026-10-12 Moto A/B, unchanged except this line.
"""Held-out truth for moto scenario M2: the reel editor is served from this run's tree with a hike post loaded.

THE OWNER, event 43721 (ask_dispatch, 2026-10-08T04:58:18Z, run-20261008T045818-b469, D:/MotoProject, seat agy
Gemini 3.8 Flash):
    "Can you start the reel project server with the hike workload loaded?"
Decision 0041 §1: Nirvana's worker spent 766 s and served nothing; Claude Code Sonnet served it in 49 s with
`python editor/server/main.py --port 8770 --manifest plan/post_alum_cave_v3.json`.

WHAT THE EDITOR SERVES, MEASURED (bench/truth/moto_facts.json, rebuilt by P-A2's facts_build.py on 2026-10-12): in the
bench clone (60476eb plus the attachments the scenario names) `GET /` is 200 with `<title>reel editor</title>` about a
second after start, with or without --no-prewarm. `GET /api/project` describes the LOADED edit: with the default
manifest (plan/manifest_v4.json, a motorcycle reel) it says `"kind":"reel"`; with any plan/post_*.json (the three
Alum Cave hike carousels) it says `"kind":"post"`. BOTH list the hike clip ids, because the reel's clip list merges the
hike index -- so "names a hike clip" alone would pass a server with no hike workload loaded, and the rule that carries
"with the hike workload loaded" is `"kind":"post"`. Its `project.root` is the folder the server runs from, which is
how a listener left over from somewhere else (the owner's own folder) is told apart from this run's.

THE RULES are floors the owner's own good outcome passes (the 49 s run would): served.json present (the driver writes it
on the first `GET /` 200 within the wall); `/` answered 200 with the measured title; the api answered 200, says a post
is loaded, names at least one clip id of the tree's hike index, and its root is this tree or a worktree of it; the
answer names the URL (127.0.0.1 or localhost with the served port); HEAD is still the base and no tracked file changed.
"No tracked file changed" skips `**/__pycache__/*.pyc`: the project TRACKS scripts/__pycache__/*.pyc (60476eb), and
merely starting the server rewrites three of them (measured: plan, render, review_marks), so a server start would
otherwise read as an edit.

Usage: python bench/truth/M2.py --tree <final tree> --base <base sha> --answer <answer.txt> --served <served.json>
Prints `PASS <rule>` / `FAIL <rule> -- <detail>` per rule, then `RESULT PASS (<n> checks)` or
`RESULT FAIL (<k> of <n> failed)`. Exit 0 all pass, 1 a rule failed, 2 usage error.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Callable

FACTS = Path(__file__).resolve().parent / "moto_facts.json"


def facts() -> dict:
    return json.loads(FACTS.read_text(encoding="utf-8"))


def page_title() -> str:
    """The `<title>` the editor serves, as measured (never typed here: rule 4)."""
    return facts()["variants"]["full_post_v3_noprewarm"]["html_title"]


def git(tree: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(tree), *args], capture_output=True, text=True, encoding="utf-8",
                          errors="replace", timeout=60)


def hike_ids_at(tree: Path, base: str) -> list[str]:
    """Every clip id in index/hike/hike_*.json AT THE BASE (the scenario attaches the index in the bench commit)."""
    out: list[str] = []
    names = git(tree, "ls-tree", "--name-only", base, "index/hike/").stdout.split()
    for path in names:
        if not re.fullmatch(r"index/hike/hike_\d+\.json", path):
            continue
        try:
            doc = json.loads(git(tree, "show", f"{base}:{path}").stdout)
        except ValueError:
            continue
        out += [str(f["name"]) for f in doc.get("files", []) if isinstance(f, dict) and f.get("name")]
    return out


def bytecode(path: str) -> bool:
    return "__pycache__/" in path or path.endswith(".pyc")


def task_record(path: str) -> bool:
    """Nirvana's own task record, `<repo>/.nirvana/tasks/<run id>.json`, which the daemon writes and STAGES in the
    project by design (decision 0026, the owner 2026-09-21: "be stored and checked on Git"; `nirvanad/records.py`
    `track`). MEASURED on the first paid M1 arm A run (2026-10-12 09:49Z): the one tracked change was
    `A  .nirvana/tasks/run-20261012T094844-06cb.json`, and the owner's own D:/MotoProject shows three such staged
    records. It is the harness's record, not the Agent's edit of the project, so it is not a tracked change here."""
    return path.startswith(".nirvana/tasks/") or "/.nirvana/tasks/" in path


def tracked_changes(tree: Path) -> list[str]:
    lines = git(tree, "status", "--porcelain", "--untracked-files=no").stdout.splitlines()
    return [ln for ln in lines if ln.strip()
            and not bytecode(ln[3:].strip().strip('"')) and not task_record(ln[3:].strip().strip('"'))]


def read_answer(path: str | None) -> str:
    p = Path(path) if path else None
    return p.read_text(encoding="utf-8", errors="replace") if p and p.is_file() else ""


def read_served(path: str | None) -> tuple[dict | None, str]:
    if not path:
        return None, "no --served given"
    p = Path(path)
    if not p.is_file():
        return None, f"no served.json at {p}: nothing answered GET / within the wall"
    try:
        doc = json.loads(p.read_text(encoding="utf-8"))
    except ValueError as e:
        return None, f"served.json is not JSON: {e}"
    return (doc, "") if isinstance(doc, dict) else (None, "served.json is not an object")


def _norm(p: str) -> str:
    return str(p).replace("\\", "/").rstrip("/").lower()


def root_is_this_tree(root: str | None, tree: Path) -> tuple[bool, str]:
    """The api's project.root is the tree, a folder under it, or one of its git worktrees."""
    if not root:
        return False, "the api body names no project.root"
    r = _norm(root)
    places = [_norm(str(tree.resolve()))]
    for ln in git(tree, "worktree", "list", "--porcelain").stdout.splitlines():
        if ln.startswith("worktree "):
            places.append(_norm(ln[len("worktree "):]))
    ok = any(r == p or r.startswith(p + "/") for p in places)
    return ok, f"root {root!r} is not {places}"


def served_rules(check: Callable[[str, bool, str], None], tree: Path, base: str, served_path: str | None,
                 answer: str) -> dict | None:
    """The serve rules M2, M3 and M4 share. Every rule is printed even when served.json is missing."""
    served, why = read_served(served_path)
    s = served or {}
    body = str(s.get("api_body") or "")
    check("served_json_present", served is not None, why)
    check("root_answered_200", s.get("status") == 200, f"GET / status {s.get('status')!r}")
    title = page_title()
    check("root_is_the_reel_editor", str(s.get("title") or "").strip() == title,
          f"title {s.get('title')!r}, the editor's is {title!r}")
    check("api_answered_200", s.get("api_status") == 200, f"api status {s.get('api_status')!r}")
    kind = re.search(r'"kind"\s*:\s*"(\w+)"', body)
    check("hike_post_loaded", bool(kind) and kind.group(1) == "post",
          f"the loaded edit's kind is {kind.group(1) if kind else None!r}; a hike carousel (plan/post_*.json) says 'post'")
    ids = hike_ids_at(tree, base)
    named = [c for c in ids if c in body]
    check("api_names_a_hike_clip", bool(ids) and bool(named),
          f"none of the hike index's {ids or '(no hike index at the base)'} is in the api body")
    rm = re.search(r'"root"\s*:\s*"([^"]+)"', body)
    ok, detail = root_is_this_tree(rm.group(1) if rm else None, tree)
    check("served_from_this_tree", ok, detail)
    port = s.get("port")
    url = re.compile(rf"(?:127\.0\.0\.1|localhost):{int(port)}\b" if isinstance(port, int) else r"(?!x)x", re.I)
    check("answer_names_the_url", bool(url.search(answer)),
          f"the answer names no 127.0.0.1:{port} or localhost:{port}")
    return served


def git_rules(check: Callable[[str, bool, str], None], tree: Path, base: str) -> None:
    head = git(tree, "rev-parse", "HEAD").stdout.strip()
    want = git(tree, "rev-parse", base).stdout.strip()
    check("head_is_the_base", bool(head) and head == want, f"HEAD {head[:12]}, base {want[:12] or base}")
    dirty = tracked_changes(tree)
    check("no_tracked_change", not dirty, "; ".join(dirty)[:300])


def runner() -> tuple[list[bool], Callable[[str, bool, str], None]]:
    rows: list[bool] = []

    def check(rule: str, ok: bool, detail: str = "") -> None:
        rows.append(bool(ok))
        print(f"PASS {rule}" if ok else f"FAIL {rule} -- {detail}")
    return rows, check


def finish(rows: list[bool]) -> int:
    bad = rows.count(False)
    print(f"RESULT FAIL ({bad} of {len(rows)} failed)" if bad else f"RESULT PASS ({len(rows)} checks)")
    return 1 if bad else 0


def parser(doc: str) -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(description=doc, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tree", required=True)
    ap.add_argument("--base", required=True)
    ap.add_argument("--answer", required=True)
    ap.add_argument("--served", default=None, help="the driver's served.json; missing = nothing was served")
    # Every moto truth takes --run-dir (the driver passes it to all of them); only M3 reads it.
    ap.add_argument("--run-dir", default=None, help="the run directory (read by M3's browser_used; ignored here)")
    return ap


def main(argv: list[str] | None = None) -> int:
    try:
        args = parser(__doc__).parse_args(argv)
    except SystemExit as e:
        return 0 if e.code == 0 else 2
    tree = Path(args.tree)
    if not tree.is_dir():
        print(f"usage error: --tree {tree} is not a directory")
        return 2
    rows, check = runner()
    answer = read_answer(args.answer)
    check("answer_not_empty", bool(answer.strip()), f"{len(answer)} characters in {args.answer}")
    served_rules(check, tree, args.base, args.served, answer)
    git_rules(check, tree, args.base)
    return finish(rows)


if __name__ == "__main__":
    sys.exit(main())
