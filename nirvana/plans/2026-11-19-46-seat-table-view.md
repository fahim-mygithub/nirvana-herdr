# Plan 46: the seat table on screen, checked against real runs

Status: LANDED 2026-11-19 as alpha 0.88.0. Three calls in §6 are the orchestrator's, inside the owner's ask, and the owner may change them. The live checks of §7 are owed.

## 1. The ask

The owner, 2026-11-19, on the Seats panel: "In this modal I would like to see a submenu wher we have a table for the 3
different modes efficent balanced and SOTA. I would like to see the seating table with the task and model type. Use the
latest logs to update the table as refrencses. During the idle lesson learning runs have it check / maintain the table
using real runs as calibration / validation."

## 2. Requirements ledger

| # | Quote | Surface | Owner | Verifier |
|---|-------|---------|-------|----------|
| L1 | "In this modal I would like to see a submenu wher we have a table for the 3 different modes efficent balanced and SOTA" | a Table page in the Seats panel, beside the orders page | P46b | a shell test: `t` and a click on the page word switch pages; three mode columns |
| L2 | "the seating table with the task and model type" | one row per task class; each cell is the seat, model and effort the pick gives now | P46a, P46b | each cell equals `pick` on the same table and readings |
| L3 | "Use the latest logs to update the table as refrencses" | the record beside each cell: orders, first gate, cost against booking; one refit in the default table | P46a | the Rust figures equal `seat_fit.py`'s on a fixture record |
| L4 | "During the idle lesson learning runs have it check / maintain the table" | a check with no model in the idle tick; a calibration file beside the table | P46a | a fixture record moves a cell; a thin one does not |
| L5 | "using real runs as calibration / validation" | measured cost replaces booked cost where the rule fires; pass rates are shown against scores | P46a | §4's rules, each with a sabotage control |

## 3. What the code and the record say

Two read-only audits, 2026-11-19: the seat code, and every project's journals (7 projects, 109 runs, 692 original
orders, 2026-10-21 to 2026-11-19).

- **The pick is a pure function.** `pick(table, world, request)` reads no file. 22 classes in 3 modes is 66 calls.
- **Nothing learns from runs today.** The table changes by hand and by release. `use_record`, which blends pass rates
  into scores, is off in the default and in the owner's table.
- **The owner's table is frozen.** It is the 0.78.2 default with one line changed, `overrides_per_run = 5`. The
  upgrade compares whole text, so it took none of the last three defaults. The panel says "seats.toml differs".
- **Cost booking is mostly right.** Of 14 seat and effort pairs with 5 or more medium orders, 11 sit inside the 0.5 to
  2.0 band. Outside it: Hydrogen low at 0.37 (n 7, two projects), Carbon medium at 2.02 (n 5, one order from the
  edge), Argon high at 11.13 (n 7, the seat is off).
- **Argon's pool is miscalibrated, which is the "week 525%" in the panel.** Its week was sized from two probe orders
  of two cents each. Real orders cost up to $10.94 at list.
- **Projects disagree on cost.** Opus high reads 2.30 in Megajam (n 60), 1.75 in xtool, 1.28 in MotoProject and 0.56
  in groove-coach.
- **Pass rates are not yet safe to act on.** The written score rule would move Opus in build.load_bearing from 95 to
  68 on 8 passes of 21. Without refusals for an unowned file or a rebase, the same orders read 8 of 10. No journal
  yet carries the 0.83.0 fault field that separates the two.
- **Where the record contradicts a score, with enough orders to say so.** Sonnet in ops.terminal, score 88: 25 of 59
  at the first gate, 0 of 12 in the last 14 days. Hydrogen in build.frontend, score 72: 17 of 19.
- **Priors with no record.** Neon has no order anywhere and sits in most Balanced sets. Five classes have no order.

## 4. The design

**The Table page (L1, L2, L3).** The Seats panel gets two pages, `Orders` and `Table`. `t`, Tab, or a click on the
word switches. The Table page shows:

- one row for Nirvana Claude, then one row per task class;
- three columns, Efficient, Balanced and SOTA, with the project's mode marked;
- in each cell the seat and effort the pick gives now. A cell that live usage has pushed off its cool choice is
  marked, and the cool choice is named in the detail;
- for the selected row, a detail block: every seat the class lists, its score, and its record in that class: orders,
  first gate passes, cost against booking, median minutes, orders in the last 14 days. A cell under 5 orders reads
  `thin`. A seat with none reads `no record`;
- one line saying when the table was last checked and what the check found.

**The calibration (L4, L5).** A new file beside the table, `<home>/nirvana/seat-calibration.json`, written by herdr
and never by hand. The table stays the prior and keeps upgrading; the file holds what runs measured.

- It is recomputed whole from the record each time, so it holds no state of its own.
- **Cost.** For a seat and effort with 5 or more medium orders whose median cost is outside 0.5 to 2.0 times the
  booking, the pick books the measured cost. A project's own figure is used when that project has 5 or more such
  orders, else the pooled one. One step never moves a cell by more than 4 times; a larger gap is flagged.
- **Scores.** The check computes the score rule and writes the result as a finding. It does not change a score.
- **When.** In the idle tick, with no model and no seat, when the record has grown since the last check. Also on
  `herdr seat calibrate`.
- **Seen.** In the Table page, and in the Learning panel's check line.
- **Undone.** `calibration = false` in the table switches it off. Each change is a row in a ledger.

**The record (L3).** The record line gains the first gate verdict and, from 0.83.0 gates, the fault. Today the first
verdict is only in the journals.

**The upgrade.** A table that differs from a kept default only in plain values at the top level or in `[constants]`
takes the new default with those values carried over, after a backup. The owner's table then upgrades by itself and
keeps `overrides_per_run = 5`.

**The default table.** One refit from §3: Hydrogen low, 0.37 times its cell. Carbon medium is left for the
calibration to decide. Argon is off and is not refit.

## 5. Packages

| Package | What | Files it owns |
|---|---|---|
| P46a | The calibration, the record fields, the upgrade, the matrix the page draws, the idle step, the refit | a new `nirvana_seat_calib.rs`, `nirvana_seat.rs`, `nirvana_seats_square.rs`, `nirvana_seats.toml` and the kept default, `nirvana_run.rs` (the record line only), one hunk in `nirvana_idle_review.rs` |
| P46b | The Table page | `client/shell/nirvana_seats.rs` |

P46a first. Plan 45's P45d shares `nirvana_seat.rs` and `nirvana_run.rs` and is built after it.

## 6. Calls made inside the ask

| # | Call | Why |
|---|---|---|
| D1 | Measured cost is applied by herdr; a score change is only shown | Cost is a measurement. A score decides who gets the work, and §3 shows today's pass rates would wrongly demote Opus. Revisit when journals carry the fault field. |
| D2 | herdr writes a calibration file, not the owner's table | A table herdr edits stops matching any kept default and stops upgrading, which is the fault the owner's table has now. |
| D3 | Opus high is measured and shown but not booked (`calibration_hold = ["opus:high"]`) | On the record Opus high reads x2.23 (n 65), mostly run ia1's SOTA work. Booked, it moves 4 Balanced picks (plan, build.load_bearing, build.frontend, judge.visual) from Opus to Fable. The owner left Opus high unrefit on 2026-11-18. Delete the pair from the line to book it. |

### What the build changed from this plan

- The calibration file is bound to the table by a fingerprint of the cells it measured. An older file reads as none.
- One upgrade attempt runs at a time under an OS lock, a backup is created exclusively, and a complete backup is
  never removed. Three reads at once had lost the owner's bytes in 5 of 60 rounds.
- A failed upgrade leaves a note beside the table and waits 10 minutes before it tries again.
- herdr no longer writes the default over a table it cannot read; it writes it only where no file exists.
- The panel remembers its page for the session.

## 7. Live checks owed after building

1. Open the Table page in two projects in different modes.
2. After an install, see the owner's table upgraded with `overrides_per_run = 5` kept, and "differs" gone or naming
   only that line.
3. Leave the machine idle and see the check line change.
