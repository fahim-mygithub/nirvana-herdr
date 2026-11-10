"""The seat table's use priors against what the record's orders spent (plan 40a review round, Part 2 item 4).

The usage audit (plans/2026-11-09-40-records/usage-audit.md, finding 3) found small orders booked at 4 to 27 times
their prior and large ones at 0.14 to 0.38, with medium near 1. CLAUDE.md rule 4: the numbers live here, not in prose.

For each record line with tokens, a seat, an effort, a size and a known class:

    actual   = the line's tokens x the seat's price row on the cell's `at` date x the pool's per_usd["week"]
    unit     = actual / (cell["week"] x the class's weight)      # the order in P4 units, before its size
    ratio    = unit / size[line's size].use                      # 1.0 is the table's prior exactly

Pricing at the cell's own date keeps a price change out of the fit. For a `max` seat the cell is P4 x price x per_usd,
so per_usd cancels and the ratio is in tokens. The `week` window, because every pool but `local` has one.

Printed, each with n:
  1. per seat and effort, the median ratio of medium orders against the cell (a cell is refit when it is outside
     0.5 to 2.0 with n of 5 or more);
  2. per size, the median across seat-and-effort groups of (median unit of that size / median unit of medium), and
     what `size.<name>.use` would be with medium kept at its value;
  3. per size, the median and p90 minutes from seated to ended, beside `minutes` and `lease_minutes`.

    python nirvana/seat_fit.py                       # the installed record, the repository's table
    python nirvana/seat_fit.py --record <path> --table <path>

Standard library only. Reads the record; writes nothing.
"""
from __future__ import annotations

import argparse
import json
import os
import statistics
import sys
import tomllib
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TABLE = ROOT / "src" / "nirvana_seats.toml"
RECORD = Path(os.environ.get("LOCALAPPDATA", "")) / "Programs" / "Nirvana-Herdr" / "nirvana" / "record.jsonl"
WINDOW = "week"
BAND = (0.5, 2.0)
MIN_N = 5
# A size group needs this many lines of its own and of medium before its ratio to medium is taken.
MIN_GROUP = 3
# The worker's subagents: judged by the orchestrator, priced by no cell (nirvana_claude::SUBAGENT_CHAIN).
SKIP_CHAINS = {"subagent"}
TOKENS = ("input", "cache_write", "cache_read", "output")


def price_at(seat: dict, day: str) -> dict | None:
    """The latest price row whose date has come on `day`, else the first (as `Seat::price_at`)."""
    rows = sorted(seat.get("prices", []), key=lambda row: row["from"])
    came = [row for row in rows if row["from"] <= day]
    return (came or rows or [None])[-1]


def usd(price: dict, tokens: dict) -> float:
    return sum((tokens.get(key) or 0) * price[key] for key in TOKENS) / 1e6


def pct(values: list[float], q: float) -> float:
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(q * len(ordered)))]


def lines_of(record: Path) -> list[dict]:
    out = []
    for raw in record.read_text(encoding="utf-8").splitlines():
        if raw.strip():
            out.append(json.loads(raw))
    return out


def fit(table: dict, lines: list[dict]):
    seats = {seat["id"]: seat for seat in table["seat"]}
    pools = {pool["id"]: pool for pool in table["pool"]}
    weight = {cls["id"]: cls.get("weight", 1.0) for cls in table["class"]}
    sizes = table["size"]
    units = defaultdict(list)  # (seat, effort, size) -> [unit]
    minutes = defaultdict(list)  # size -> [minutes]
    for line in lines:
        seat = seats.get(line.get("seat"))
        size, effort, tokens = line.get("size"), line.get("effort"), line.get("tokens")
        if line.get("chain") in SKIP_CHAINS or not (seat and size in sizes and effort and tokens):
            continue
        if line.get("seated_at") and line.get("ended_at"):
            minutes[size].append((line["ended_at"] - line["seated_at"]) / 60)
        cell = next((c for c in seat.get("use", []) if c["effort"] == effort), None)
        per = pools.get(seat["pool"], {}).get("per_usd", {}).get(WINDOW)
        price = price_at(seat, cell["at"]) if cell else None
        share = (cell or {}).get("windows", {}).get(WINDOW)
        if not (cell and per and price and share and line.get("class") in weight):
            continue
        units[(seat["id"], effort, size)].append(usd(price, tokens) * per / (share * weight[line["class"]]))
    return units, minutes, sizes


def report(table: dict, lines: list[dict]) -> list[str]:
    units, minutes, sizes = fit(table, lines)
    medium_use = sizes["medium"]["use"]
    out = [f"-- 1. medium orders against the cell ({WINDOW} window; refit outside {BAND[0]}-{BAND[1]} at n >= {MIN_N})"]
    for (seat, effort, size), values in sorted(units.items()):
        if size != "medium":
            continue
        ratio = statistics.median(values) / medium_use
        flag = "REFIT" if len(values) >= MIN_N and not BAND[0] <= ratio <= BAND[1] else "keep"
        out.append(f"cell {seat} {effort}: median ratio {ratio:.2f}, n {len(values)}, {flag}")
    out.append("-- 2. each size against medium, median across seat-and-effort groups "
               f"(a group needs n >= {MIN_GROUP} of both)")
    for size in ("small", "large"):
        ratios, n_lines = [], 0
        for (seat, effort, s), values in sorted(units.items()):
            medium = units.get((seat, effort, "medium"), [])
            if s != size or len(values) < MIN_GROUP or len(medium) < MIN_GROUP:
                continue
            ratios.append(statistics.median(values) / statistics.median(medium))
            n_lines += len(values)
            out.append(f"  {size} {seat} {effort}: {ratios[-1]:.3f} of medium, n {len(values)} against {len(medium)}")
        if ratios:
            r = statistics.median(ratios)
            out.append(f"size {size}: {r:.3f} of medium, groups {len(ratios)}, n {n_lines}; "
                       f"use = {r * medium_use:.2g} (medium {medium_use:g}; table has {sizes[size]['use']:g})")
    out.append("-- 3. minutes from seated to ended, per size")
    for size in ("small", "medium", "large"):
        values = minutes.get(size, [])
        if values:
            out.append(f"minutes {size}: median {statistics.median(values):.0f}, p90 {pct(values, 0.9):.0f}, "
                       f"n {len(values)}; table minutes {sizes[size]['minutes']}, "
                       f"lease_minutes {sizes[size]['lease_minutes']}")
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--record", type=Path, default=RECORD)
    parser.add_argument("--table", type=Path, default=TABLE)
    args = parser.parse_args()
    table = tomllib.loads(args.table.read_text(encoding="utf-8"))
    for line in report(table, lines_of(args.record)):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
