"""The Max pool's use cells in `src/nirvana_seats.toml`, recomputed from the measured order (plan 40a, review M3/M4).

CLAUDE.md rule 4: a count in prose rots, so the numbers live here. Every cell of a `max` seat is

    P4's tokens x the seat's price row in force on the cell's `at` date x the effort's multiplier x per_usd[window]

P4 is probe P4's order (one `slugify` function and its tests): 43,807 cache reads and 1,286 output tokens, measured on
sonnet medium (plan 12 §15). The effort multipliers are the table's priors (low x0.6, high x1.6, xhigh x2.5 of medium).
`per_usd` is the pool's: `5h` a prior, `week` derived from the week of 2026-10-16, `week.fable` derived from the
published 50% rule (twice the week's factor). Review M3 found the week cells 6.4 times too high: they were still on the
old $200 week factor 0.005 after plan 14 moved `per_usd.week` to 0.00078.

    python nirvana/seat_cells.py           # print the cells as toml rows
    python nirvana/seat_cells.py --check   # exit 1 when a cell in the toml differs from the computed one
"""
from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TABLE = ROOT / "src" / "nirvana_seats.toml"

# Probe P4's order, sonnet medium (plan 12 §15): measured.
P4 = {"input": 0, "cache_write": 0, "cache_read": 43_807, "output": 1_286}
# The table's priors for the other efforts, as multiples of medium.
EFFORT = {"low": 0.6, "medium": 1.0, "high": 1.6, "xhigh": 2.5}
POOL = "max"
# Relative slack for --check: the toml holds four significant digits.
SLACK = 1e-3


def sig4(value: float) -> float:
    return float(f"{value:.4g}")


def price_at(seat: dict, day: str) -> dict:
    """The latest price row whose date has come on `day`, else the first (as `Seat::price_at`)."""
    rows = sorted(seat.get("prices", []), key=lambda row: row["from"])
    came = [row for row in rows if row["from"] <= day]
    return (came or rows)[-1]


def cost(price: dict) -> float:
    return sum(P4[key] * price[key] for key in P4) / 1e6


def cells(table: dict, day: str | None = None):
    """(seat, effort, at, usd, {window: share}) for every effort of every `max` seat; `day` picks the price row,
    else the newest row's date."""
    pool = next(p for p in table["pool"] if p["id"] == POOL)
    for seat in table["seat"]:
        if seat["pool"] != POOL or not seat.get("prices"):
            continue
        at = day or max(row["from"] for row in seat["prices"])
        usd = cost(price_at(seat, at))
        windows = list(pool["windows"]) + list(seat.get("extra_windows", []))
        for effort in seat["efforts"]:
            mult = EFFORT[effort]
            yield seat["id"], effort, at, usd, {w: sig4(usd * mult * pool["per_usd"][w]) for w in windows}


def row(seat: str, effort: str, at: str, usd: float, windows: dict) -> str:
    inner = ", ".join(f'"{w}" = {v}' if not w.isalnum() or w[0].isdigit() else f"{w} = {v}"
                      for w, v in windows.items())
    if effort == "medium":
        basis = (f"derived: P4's 43,807 cache reads and 1,286 out at {seat}'s {at} prices, ${usd:.4f}, "
                 "times per_usd (nirvana/seat_cells.py)")
    else:
        basis = f"prior: medium x {EFFORT[effort]} (nirvana/seat_cells.py)"
    return f'  {{ effort = "{effort}", windows = {{ {inner} }}, at = "{at}", basis = "{basis}" }},'


def check(table: dict) -> list[str]:
    wrong = []
    for seat in table["seat"]:
        if seat["pool"] != POOL or not seat.get("prices"):
            continue
        given = {cell["effort"]: cell for cell in seat["use"]}
        for effort in seat["efforts"]:
            if effort not in given:
                wrong.append(f"{seat['id']} {effort}: no use cell")
        for effort, cell in given.items():
            [(_, _, _, _, want)] = [c for c in cells(table, cell["at"]) if c[0] == seat["id"] and c[1] == effort]
            for window, share in want.items():
                have = cell["windows"].get(window)
                if have is None or abs(have - share) > SLACK * share:
                    wrong.append(f"{seat['id']} {effort} {window}: toml {have}, computed {share} (at {cell['at']})")
            for window in cell["windows"]:
                if window not in want:
                    wrong.append(f"{seat['id']} {effort} {window}: a window the seat does not spend")
    return wrong


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--table", type=Path, default=TABLE)
    args = parser.parse_args()
    table = tomllib.loads(args.table.read_text(encoding="utf-8"))
    if args.check:
        wrong = check(table)
        for line in wrong:
            print(f"seat_cells: {line}")
        print(f"seat_cells --check: {'FAIL' if wrong else 'ok'}, {len(wrong)} cell(s) differ")
        return 1 if wrong else 0
    last = None
    for seat, effort, at, usd, windows in cells(table):
        if seat != last:
            print(f"# {seat}")
            last = seat
        print(row(seat, effort, at, usd, windows))
    return 0


if __name__ == "__main__":
    sys.exit(main())
