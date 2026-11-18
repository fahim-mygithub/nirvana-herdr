"""The seats refresh (2026-11-18): the numbers behind its changes to `src/nirvana_seats.toml`, recomputed.

CLAUDE.md rule 4: a count in prose rots, so the table's comments name this script and the numbers live here.

    python nirvana/seat_refresh.py classes <journal.jsonl> [--by seat,effort,class] [--min N]
    python nirvana/seat_refresh.py hydrogen [--check]

`classes` reads one herdr run journal (run ia1's was the source) and prints, per group of seated orders:
  n           seated orders (a send-back's `-sN` child and a reseat's `-rN` are their own orders, as the journal has them)
  ovr         orders whose `seated.override` is set (the seat was chosen, not the rule's)
  gated       orders with at least one `gated` event
  first_pass  of the gated, the share whose FIRST verdict is pass or accepted
  merged      share of n with a `merged` event
  med_min     median minutes from `seated` to the first `ended`
  med_usd     median list-price USD of the order's tokens (each attempt's last cumulative `usage`, summed), at the seat's
              price row on the order's day in the table
The rule the refresh moved scores by, written before it was applied: a seat and class with 10 or more gated orders
whose first-pass share is under 0.40 goes to the class's Efficient bar less 2 (the review round's flash, 9 of 28, to
68); one at 0.90 or more, with 0.70 or more merged, that sits under the Balanced bar goes to that bar plus 1 (the
review round's sonnet review, 17 of 17, to 89). Any other group keeps its score. Under 10 gated is too few.

`hydrogen` prices plan 71's ten probe orders again, each model at its own price row in the table (Google's page gave
Hydrogen twice Flash 3.8's in and out; plan 71 had assumed the same), and prints Hydrogen's cost per order over Flash's
at each effort. `--check` exits 1 when a Hydrogen cell is not Flash's cell times that ratio (rounded to 2 places)
times the `refit x<r>` its basis names, if any.

Standard library only. Reads; writes nothing.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import statistics
import sys
import tomllib
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TABLE = ROOT / "src" / "nirvana_seats.toml"
PASS = {"pass", "accepted"}
KEYS = ("input", "cache_write", "cache_read", "output")

# Plan 71's probe (2026-11-12, agy 1.5.7): one order each, a 42-line copy of nirvana/version.py and one two-part
# question, all ten answers right. (model, input, cache_read, output) from each order's `result.usage`: input is the
# uncached input, cache reads come on top, output counts thinking. The raw streams were in that session's scratchpad.
PLAN71 = (
    ("gemini-3.8-flash-low", 24805, 0, 235),
    ("gemini-4-hydrogen-low", 15650, 10604, 535),
    ("gemini-3.8-flash-low", 24801, 0, 237),
    ("gemini-4-hydrogen-low", 15619, 10605, 445),
    ("gemini-3.8-flash-medium", 24883, 0, 620),
    ("gemini-4-hydrogen-medium", 15665, 10608, 890),
    ("gemini-3.8-flash-medium", 24905, 0, 1291),
    ("gemini-4-hydrogen-medium", 15662, 10603, 890),
    ("gemini-3.8-flash-high", 25240, 0, 4384),
    ("gemini-4-hydrogen-high", 15653, 10601, 2060),
)
# Both models were probed on this day, so it picks each one's price row.
PLAN71_DAY = "2026-11-12"
REFIT = re.compile(r"refit x([0-9.]+)")
SLACK = 1e-3


def price_at(seat: dict, day: str) -> dict | None:
    """The latest price row whose date has come on `day`, else the first (as `Seat::price_at`)."""
    rows = sorted(seat.get("prices", []), key=lambda r: r["from"])
    came = [r for r in rows if r["from"] <= day]
    return (came or rows or [None])[-1]


def usd(price: dict, tokens: dict) -> float:
    return sum((tokens.get(k) or 0) * price[k] for k in KEYS) / 1e6


# ---- classes ----

def load(journal: Path) -> dict[str, dict]:
    orders: dict[str, dict] = {}
    for line in journal.open(encoding="utf-8"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        ev, name = e.get("event"), e.get("order")
        if ev == "seated":
            orders[name] = {"seat": e["seat"], "effort": e["effort"], "class": e["class"], "mode": e.get("mode"),
                            "override": e.get("override"), "seated_at": e["at"], "verdicts": [], "merged": False,
                            "ended_at": None, "usage": {}}
            continue
        o = orders.get(name) if name else None
        if o is None:
            continue
        if ev == "gated":
            o["verdicts"].append(e.get("verdict"))
        elif ev == "merged":
            o["merged"] = True
        elif ev == "ended" and o["ended_at"] is None:
            o["ended_at"] = e["at"]
        elif ev == "usage":
            # Cumulative within an attempt (nirvana_run.rs `Kind::Usage`): the last one per attempt counts.
            o["usage"][e.get("attempt", 0)] = e.get("tokens") or {}
    return orders


def order_usd(o: dict, seats: dict) -> float | None:
    tokens = {k: sum((t.get(k) or 0) for t in o["usage"].values()) for k in KEYS}
    seat = seats.get(o["seat"])
    if not seat or not any(tokens.values()):
        return None
    day = dt.datetime.fromtimestamp(o["seated_at"], dt.timezone.utc).strftime("%Y-%m-%d")
    price = price_at(seat, day)
    return usd(price, tokens) if price else None


def median(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def classes(args) -> int:
    seats = {s["id"]: s for s in tomllib.loads(args.table.read_text(encoding="utf-8"))["seat"]}
    orders = load(args.journal)
    keys = args.by.split(",")
    groups = defaultdict(list)
    for o in orders.values():
        groups[tuple(o[k] for k in keys)].append(o)
    print(f"orders {len(orders)}, overridden {sum(1 for o in orders.values() if o['override'])}")
    print("\t".join(keys + ["n", "ovr", "gated", "first_pass", "merged", "med_min", "med_usd"]))
    for key in sorted(groups, key=lambda k: (-len(groups[k]), k)):
        g = groups[key]
        if len(g) < args.min:
            continue
        gated = [o for o in g if o["verdicts"]]
        first = sum(1 for o in gated if o["verdicts"][0] in PASS)
        mins = median([(o["ended_at"] - o["seated_at"]) / 60 if o["ended_at"] else None for o in g])
        cost = median([order_usd(o, seats) for o in g])
        print("\t".join(list(map(str, key)) + [
            str(len(g)), str(sum(1 for o in g if o["override"])), str(len(gated)),
            f"{first}/{len(gated)}={first / len(gated):.2f}" if gated else "-",
            f"{sum(o['merged'] for o in g) / len(g):.2f}",
            f"{mins:.1f}" if mins is not None else "-",
            f"{cost:.3f}" if cost is not None else "-",
        ]))
    return 0


# ---- hydrogen ----

def ratios(table: dict) -> dict[str, tuple[float, float]]:
    """effort -> (Hydrogen's mean cost per order over Flash's, each at its own price; the same at Flash's price)."""
    seats = {s["id"]: s for s in table["seat"]}
    flash_price = price_at(seats["flash"], PLAN71_DAY)
    hydrogen_price = price_at(seats["hydrogen"], PLAN71_DAY)
    cost = defaultdict(lambda: defaultdict(list))
    for model, inp, cache, out in PLAN71:
        tokens = {"input": inp, "cache_read": cache, "output": out}
        effort = model.rsplit("-", 1)[1]
        if "flash" in model:
            cost[effort]["flash"].append(usd(flash_price, tokens))
        else:
            cost[effort]["own"].append(usd(hydrogen_price, tokens))
            cost[effort]["flash_price"].append(usd(flash_price, tokens))
    return {e: (statistics.mean(c["own"]) / statistics.mean(c["flash"]),
                statistics.mean(c["flash_price"]) / statistics.mean(c["flash"])) for e, c in cost.items()}


def hydrogen(args) -> int:
    table = tomllib.loads(args.table.read_text(encoding="utf-8"))
    seats = {s["id"]: s for s in table["seat"]}
    cells = lambda seat: {c["effort"]: c for c in seats[seat]["use"]}  # noqa: E731
    flash, hyd = cells("flash"), cells("hydrogen")
    wrong = []
    for effort, (own, at_flash) in sorted(ratios(table).items()):
        refit = REFIT.search(hyd[effort]["basis"])
        factor = round(own, 2) * (float(refit.group(1)) if refit else 1.0)
        print(f"{effort}: hydrogen over flash {own:.2f} at its own price ({at_flash:.2f} at flash's)"
              + (f", refit x{refit.group(1)}" if refit else ""))
        for window, share in flash[effort]["windows"].items():
            want, have = share * factor, hyd[effort]["windows"].get(window)
            if have is None or abs(have - want) > SLACK * want + 5e-8:
                wrong.append(f"hydrogen {effort} {window}: toml {have}, computed {want:.4g}")
    for line in wrong:
        print(f"seat_refresh: {line}")
    if args.check:
        print(f"seat_refresh hydrogen --check: {'FAIL' if wrong else 'ok'}, {len(wrong)} cell(s) differ")
        return 1 if wrong else 0
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--table", type=Path, default=TABLE)
    sub = parser.add_subparsers(dest="what", required=True)
    c = sub.add_parser("classes")
    c.add_argument("journal", type=Path)
    c.add_argument("--by", default="seat,effort,class")
    c.add_argument("--min", type=int, default=1)
    h = sub.add_parser("hydrogen")
    h.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    return classes(args) if args.what == "classes" else hydrogen(args)


if __name__ == "__main__":
    sys.exit(main())
