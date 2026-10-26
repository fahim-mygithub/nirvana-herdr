# Feature 30: live workers show in the spaces view

Status: **PLANNED (2026-10-26)**. A small client change on top of plan 26's sidebar.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "If there are sub agents running have only the active agents on thebotton be shown in the spacess vieww" | §2 | builder | §2 tests |
| L2 | "Like moto projeect should have v8-build s5.5 xMxxS" | §2 | builder | §2 tests |

## 1. Measured 2026-10-26: the owner's screenshot, MotoProject at alpha 0.40.0

- **The spaces view, at the top of the sidebar.** The MotoProject space shows two rows: `✻ Nirvana O5.5 idle` and
  `✻ Claude S5.5 idle`.
- **The agents panel, at the bottom, built by plan 26 §5.** It groups workers under `Run run-20261023-024041`:
  - `v8-build` is live, shown as `S5.5 1m07s`;
  - below it are finished rows: `✓ v7-build done`, `■ reveal-bui… stopped`, and so on.
- The spaces view shows nothing of the live `v8-build`.

## 2. What to build

- Under each space's agent rows in the spaces view, add one row per **live** worker of that space. The rows read:
  - the order name;
  - the short model;
  - the elapsed time.

  These are the same words and format the agents panel uses for a live row, for example `v8-build S5.5 1m07s`.
- **Live means exactly** what the agents panel counts as live: its "live first" group. Finished, blocked, stopped and
  failed rows never show in the spaces view.
- The rows come from the same data the agents panel already reads. No new reads, no new polling.
- With no live worker, the space looks exactly as it does today.
- Space for the rows:
  - indent them under the space's agents, with a dim marker, so they read as that space's workers;
  - one row per worker;
  - names are cut the way the agents panel cuts them;
  - past 4 live workers, show 3 and a `+N more` row.
- The elapsed time updates as the agents panel's does.
- The agents panel at the bottom is unchanged.

**Tests:**
- a space with one live and several finished workers shows exactly one worker row, in this format;
- a space with no live worker renders the same as before;
- the 5-worker case shows 3 rows and `+2 more`;
- workers of another space never show under this one.

## 3. Controls and gates

As plan 27. The controls go in `nirvana/controls.d/live_in_spaces.py`, named `p30_*`.

## 4. Live check, for the owner

Start a run. While a worker runs, its row shows under the space in the spaces view. It leaves when the worker
finishes.
