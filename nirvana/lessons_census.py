"""Order LEARN (2026-11-09): the lesson counters of every project herdr has run orders in, recomputed from the files.

CLAUDE.md rule 4: a count in prose rots, so the record names this script and the numbers come from it. It reads only:
each project's `.nirvana/lessons/index.json` and `ledger.jsonl`, and its run journals' `seated` and `gated` events. The
projects are the ones `<home>/nirvana/record.jsonl` names (every order that ever ended), plus any given on the command
line. It writes nothing.

For each lesson it says whether its `class:` is one the router names (`[[class]] id` in herdr's `nirvana_seats.toml`),
the class its own order was seated with, and how many orders seated after it was written had each: the first is what
an exact-class offer could reach, the second what it reaches once the lesson carries its order's class.

    python nirvana/lessons_census.py [--home <nirvana home>] [--json] [<project root> ...]
"""

import json
import os
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SEATS = HERE.parent / "src" / "nirvana_seats.toml"
DECAY_AFTER, HELP_EVERY = 6, 3  # nirvana_lessons.rs


def router_classes(path=SEATS):
    text = path.read_text(encoding="utf-8")
    out, in_class = [], False
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("[["):
            in_class = s == "[[class]]"
        m = re.match(r'id\s*=\s*"([^"]+)"', s)
        if in_class and m:
            out.append(m.group(1))
            in_class = False
    return set(out)


def jsonl(path):
    try:
        with open(path, encoding="utf-8") as f:
            for line in f:
                try:
                    yield json.loads(line)
                except ValueError:
                    continue
    except OSError:
        return


def default_home():
    base = os.environ.get("LOCALAPPDATA")
    return Path(base) / "Programs" / "Nirvana-Herdr" if base else None


def projects_from_record(home):
    out = set()
    if not home:
        return out
    for row in jsonl(home / "nirvana" / "record.jsonl"):
        journal = row.get("journal") or ""
        if ".nirvana" in journal:
            out.add(journal.split(".nirvana")[0].rstrip("\\/"))
    return out


def live(c):
    return c["offered"] < DECAY_AFTER or c["passed_after_opened"] * HELP_EVERY >= c["offered"]


def census(root, classes):
    root = Path(root)
    nv = root / ".nirvana"
    index = {}
    try:
        index = json.loads((nv / "lessons" / "index.json").read_text(encoding="utf-8")).get("lessons", {})
    except (OSError, ValueError):
        pass
    seated = []  # (at, run, order, class)
    gated = 0
    runs = nv / "runs"
    if runs.is_dir():
        for run in sorted(p for p in runs.iterdir() if p.is_dir()):
            for ev in jsonl(run / "journal.jsonl"):
                if ev.get("event") == "seated":
                    seated.append((ev.get("at", 0), run.name, ev.get("order"), ev.get("class")))
                elif ev.get("event") == "gated":
                    gated += 1
    own_class = {(r, o): c for _, r, o, c in seated}
    by_class = {}
    for _, _, _, c in seated:
        by_class[c] = by_class.get(c, 0) + 1
    ops = {}
    for row in jsonl(nv / "lessons" / "ledger.jsonl"):
        ops[row.get("op")] = ops.get(row.get("op"), 0) + 1
    lessons = []
    for slug, e in sorted(index.items()):
        c = {k: e.get(k, 0) for k in ("offered", "opened", "passed_after_opened")}
        # A review's or a merge's lesson names the review order, whose class is `lesson-review`; its class is already
        # the reviewed order's (nirvana_review::apply_review refuses any other), so that is its order class.
        order_class = (
            own_class.get((e.get("run"), e.get("order")))
            if e.get("origin", "worker") == "worker"
            else e.get("class")
        )
        at = e.get("written_at", 0)
        later = [s for s in seated if s[0] > at]
        lessons.append({
            "slug": slug,
            "class": e.get("class"),
            "router_class": e.get("class") in classes,
            "order_class": order_class,
            "origin": e.get("origin", "worker"),
            "profile": e.get("profile"),
            **c,
            "live": live(c),
            "later_same_class": sum(1 for s in later if s[3] == e.get("class")),
            "later_order_class": sum(1 for s in later if order_class and s[3] == order_class),
        })
    return {
        "project": str(root),
        "lessons": len(lessons),
        "live": sum(1 for l in lessons if l["live"]),
        "not_router_class": sum(1 for l in lessons if not l["router_class"]),
        "never_offered": sum(1 for l in lessons if l["offered"] == 0),
        "offered": sum(l["offered"] for l in lessons),
        "opened": sum(l["opened"] for l in lessons),
        "passed_after_opened": sum(l["passed_after_opened"] for l in lessons),
        "orders_seated": len(seated),
        "orders_gated": gated,
        "lesson_review_orders": by_class.get("lesson-review", 0),
        "ledger_ops": ops,
        "seated_by_class": dict(sorted(by_class.items(), key=lambda kv: -kv[1])),
        "per_lesson": lessons,
    }


def gaps(roots, days=14, now=None):
    """The machine's quiet stretches: every journal event of every project, merged and sorted, over the last `days`.
    Counts the gaps of at least 30 and 60 minutes between consecutive events and the hours they hold, which is how often
    an idle review (nirvana_idle_review::IDLE_AFTER_S) could have run. Stream writes are not events, so a long-running
    order shows as a gap here and not to the tick, which stats its stream."""
    import time
    now = now or time.time()
    since = now - days * 86400
    times = []
    for root in roots:
        runs = Path(root) / ".nirvana" / "runs"
        if not runs.is_dir():
            continue
        for run in runs.iterdir():
            for ev in jsonl(run / "journal.jsonl"):
                at = ev.get("at", 0)
                if at >= since:
                    times.append(at)
    times.sort()
    out = {"days": days, "events": len(times)}
    for minutes in (30, 60):
        long = [b - a for a, b in zip(times, times[1:]) if b - a >= minutes * 60]
        out[f"gaps_{minutes}m"] = len(long)
        out[f"hours_in_gaps_{minutes}m"] = round(sum(long) / 3600, 1)
    return out


def main(argv):
    home, as_json, roots = default_home(), False, []
    it = iter(argv)
    for a in it:
        if a == "--home":
            home = Path(next(it))
        elif a == "--json":
            as_json = True
        else:
            roots.append(a)
    classes = router_classes()
    found = sorted(set(roots) | projects_from_record(home))
    out = [census(r, classes) for r in found]
    quiet = gaps(found)
    if as_json:
        print(json.dumps({"projects": out, "gaps": quiet}, indent=2))
        return 0
    print(f"== the machine, last {quiet['days']} days: {quiet['events']} journal events; gaps of 30 min or more "
          f"{quiet['gaps_30m']} ({quiet['hours_in_gaps_30m']} h); of 60 min or more {quiet['gaps_60m']} "
          f"({quiet['hours_in_gaps_60m']} h)")
    for p in out:
        print(f"== {p['project']}")
        print(
            f"   lessons {p['lessons']} (live {p['live']}, class not a router class {p['not_router_class']}, "
            f"never offered {p['never_offered']}) · offered {p['offered']} · opened {p['opened']} · "
            f"passed after opened {p['passed_after_opened']}"
        )
        print(
            f"   orders seated {p['orders_seated']} · gated events {p['orders_gated']} · lesson-review orders "
            f"{p['lesson_review_orders']} · ledger ops {p['ledger_ops']}"
        )
        print(f"   seated by class {p['seated_by_class']}")
        for l in p["per_lesson"]:
            print(
                f"   - {l['slug'][:44]:44} class {l['class']!r:42} router {str(l['router_class'])[0]} "
                f"order {l['order_class']!s:18} {l['origin']:6} off {l['offered']} open {l['opened']} "
                f"pao {l['passed_after_opened']} · later same class {l['later_same_class']} · later order class "
                f"{l['later_order_class']}"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
