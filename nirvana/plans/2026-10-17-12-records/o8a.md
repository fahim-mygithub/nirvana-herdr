# O8a record: the Seats square and the mode

Built 2026-10-18 on master b675a9b3 (alpha 0.18.0), branch `o8a-seats`, by the leaf "Opus 5.5 high - O8a Seats square
build", in parallel with O8b (Jev roles 2 and 3). Not merged; the version is the orchestrator's to bump.

## Ledger for this package

| # | Verbatim quote | Surface | Verifier |
|---|---|---|---|
| §8 | "A Seats square in the mod strip of a Nirvana tab: each open order with its seat, model and effort, pool, state, time and use; its reason line; a click focuses the worker's pane." | the Seats square and its panel (`client/shell/nirvana_seats.rs`); the data (`nirvana_seats_square.rs`) | `client::shell::tests::nirvana_seats::*`, `nirvana_seats_square::tests::*`; the live run below |
| §8 | "The mode (Efficient, Balanced, SOTA) in the Seats square's header, one key to change it." | the panel's title and mode row; `m`; `seats.toml`'s `mode =`; `mode_changed`; `herdr seat mode` | `the_mode_key_*`, `with_mode_*`, `seat_pick_takes_the_mode_*`; live |
| §5.3.5 | "The reason line goes to the journal, `herdr seat list` and, after O8, the Seats square" | the line under each order | `a_running_order_shows_*`, `the_wide_panel_*`, `the_narrow_panel_*` |
| §5.3.4 | "the journal counts each run's classes, and the Seats square shows them" | each run's line | `the_run_line_counts_the_runs_classes_and_overrides` |
| L7 | "sometimes I just want Claude Code or agy traditional to tackle the project." | no Seats square in a tab without Nirvana Claude | `a_nirvana_tab_gets_the_seats_square_and_a_plain_claude_tab_does_not`; live |

## What was built

- `src/nirvana_seats_square.rs` (new): the view (open orders, each run's line, the mode), composed purely from
  replayed runs, the leases and the table; the read (journals, `leases.json`, `seats.toml`), each journal's replay
  kept with its length and modified time; the window's cache, read on a thread started from the timer tick
  (`take_changed`), never from the draw; the mode change (`change_mode`, `log_mode`) and `herdr seat mode`.
- `src/client/shell/nirvana_seats.rs` (new): which tab is a Nirvana tab, the square's wide and narrow values, the
  panel's wide and narrow forms, its keys and mouse.
- `src/nirvana_seat.rs` (O2's): `Mode::title`, `Mode::next`; `with_mode`, which rewrites only the top-level `mode =`
  line; `set_mode_in`; `default_request`, which `seat pick` now builds its request with; the `mode` verb.
- `src/nirvana_run.rs` (O3's): the `mode_changed` event and `RunState.modes`.
- `src/client/shell/{nirvana_mods,composition,input,mouse,state,surface_patch,nirvana_memory,nirvana_accounts}.rs`,
  `src/client/{mod,shell}.rs`, `src/main.rs`: wiring, as for Running, Preload and Memory.
- Fixtures `src/nirvana_seats_square/fixtures/`: the recorded run below.
- Controls: `nirvana/controls.d/o8a.py`.

## The recording, and the live run (2026-10-18, this PC)

Two isolated herdr servers from this branch's debug build under `target/o8a-live`: `XDG_CONFIG_HOME`,
`XDG_STATE_HOME` and `NIRVANA_HERDR_HOME` there, `BROWSER` a script that opens nothing, sessions `o8ain` and
`o8aout`. The owner's server and sessions were not used. The project: a scratch git repo `target/o8a-live/proj`.

**A Nirvana tab without starting Claude.** The tab's pane is a pwsh shell that reports itself as a Claude agent
(`herdr pane report-agent --agent claude --state idle w1:p1`) and is renamed `nirvana` (`herdr agent rename`), the
name O4a's launch gives. No interactive Claude or agy was started; no folder-trust prompt was shown or answered. The
orchestrator pane's wake guard (O3) reads its argv, sees no `--agent nirvana`, and queues: nothing was typed into it.

**The recording**, one real worker, flash low on agy (no Claude quota):

```text
$ herdr seat run flash --order ../orders/mul.md --order-id mul --run rec1 --class sweep.mechanical --size small --not-family anthropic --worktree
mul new · run rec1 · flash gemini-3.8-flash-low low · pane w1:p2
sweep.mechanical → flash low: gemini 5h 1.0% used, week 1% used
$ herdr seat status            (8 s later; the journal copied: rec1-running.jsonl, 4 events)
rec1 mul running (live) · flash low · pool gemini · attempt 1 · in 15194 out 83 · last 2026-10-18 08:26:12Z
$ herdr seat pick --class build.bounded --size small --book fix1 --run rec1      (leases.json copied)
build.bounded → sonnet medium: max 5h 38% used, week 59% used, reserve 15%
seat sonnet · claude claude-sonnet-5-5 · effort medium · pool max · booked (new)
$ herdr seat status            (the journal copied: rec1-delivered.jsonl, 8 events)
rec1 mul delivered · flash low · pool gemini · attempt 1 · in 96988 out 875 · last 2026-10-18 08:26:30Z
```

**The window, read as text.** The inner session's window ran in a pane of the outer session (`herdr --session
o8ain` with `HERDR_ENV` and the pane's `HERDR_*` removed, so the nested check let it start), and `herdr --session
o8aout pane read w1:p1 --source visible` read what it drew. Clicks and keys went in as console input records written
to the pane's console (`inject.ps1`, from feature 5), since SGR mouse text sent with `pane send-text` did not reach
the window as a click. The outer pane is about 120 columns, so the panel drew its narrow form; with the sidebar
collapsed (a click on `«`) it drew the wide one.

```text
 spaces                  │ ✻ Claude   Running idle ▾   Seats 2 · Bal ▴   Preload 23.5k ▾
 ○ proj                  │P╭─ Seats · proj · Balanced ─────────────────────────────────────────────────── ✕ ─╮
   master                │ │ Mode   Efficient  [Balanced]  SOTA                                 m changes it │
                         │ │ run rec1 · 1 order · sweep.mechanical 1                                         │
                         │ │ mul · delivered 8m 52s                                                          │
                         │ │   flash low · gemini-3.8-flash-low · gemini · 5h 0.30%                          │
                         │ │   ↳ sweep.mechanical → flash low: gemini 5h 1.0% used, week 1% used             │
                         │ │ fix1 · booked 8m 35s                                                            │
                         │ │   sonnet medium · claude-sonnet-5-5 · max · 5h ~0.09% booked                    │
                         │ │   ↳ build.bounded → sonnet medium: max 5h 38% used, week 59% used, reserve 15%  │
                         │ │ ↑/↓ choose · enter focus its pane · m mode · esc close                          │
```

(the panel's blank lines and right part trimmed here). A click on `mul`'s line, `click:30,6`: `herdr pane list` went
from `w1:p1 focused` to `w1:p2 focused`, tab `w1:t2` ("run rec1"), and the window showed the worker's pane, `herdr
seat exec mul --run rec1` and its stream. A click on tab `1` came back. Then `m`:

```text
 spaces                  │ ✻ Claude   Running idle ▾   Seats 2 · SOTA ▴   Preload 23.5k ▾
 ○ proj                  │P╭─ Seats · proj · SOTA ─────────────────────────────────────────────────────── ✕ ─╮
   master                │ │ Mode   Efficient   Balanced  [SOTA]                                m changes it │
                         │ │ run rec1 · 1 order · sweep.mechanical 1 · mode → SOTA 08:35Z                    │
                         │ │   ↳ seated at Balanced · sweep.mechanical → flash low: gemini 5h 1.0% used, …   │
                         │ │ mode SOTA: the next seat pick uses it; logged in run rec1                       │
$ tail -1 proj/.nirvana/runs/rec1/journal.jsonl
{"v":1,"seq":9,"at":1792312526,"event":"mode_changed","from":"balanced","to":"sota","by":"seats square"}
$ grep '^mode' home/nirvana/seats.toml
mode = "sota"
$ herdr seat pick --class plan --size small --json      (no --mode)
mode sota | seat opus xhigh | plan → opus xhigh: max 5h 40% used, 0.1% booked, week 59% used, reserve 15%; ...
```

With the sidebar collapsed, the strip and the wide panel, after two more presses (sota → efficient → balanced, two
more `mode_changed` lines, seq 10 and 11):

```text
   │ ✻ Claude   Running idle ▾   Seats 2 open · Balanced ▴   Preload 23.5k ▾
   │P╭─ Seats · proj · Balanced ──────────────────────────────────────────────────────────────────────── ✕ ─╮
   │ │ Mode   Efficient  [Balanced]  SOTA                                                      m changes it │
   │ │ run rec1 · 1 order · sweep.mechanical 1 · mode → Balanced 08:36Z                                     │
   │ │ order        seat effort      model                    pool     state       time         use         │
   │ │ mul          flash low        gemini-3.8-flash-low     gemini   delivered   10m 2s       5h 0.30%    │
   │ │   ↳ sweep.mechanical → flash low: gemini 5h 1.0% used, week 1% used                                  │
   │ │ fix1         sonnet medium    claude-sonnet-5-5        max      booked      9m 45s       5h ~0.09% bo│
   │ │   ↳ build.bounded → sonnet medium: max 5h 38% used, week 59% used, reserve 15%                       │
   │ │ mode Balanced: the next seat pick uses it; logged in run rec1                                        │
```

(trimmed at the right here; the window drew "5h ~0.09% booked" whole). Renaming the agent `moto` dropped the square
from the strip ("✻ Claude   Running idle ▾   Preload 23.5k ▾"); `nirvana` brought it back. `herdr run receipt rec1`
read the journal with its three new events. Both sessions were stopped with `herdr session stop`; no herdr or agy
process from `target/o8a-live` was left (the three `herdr.exe` still running are the owner's install).

**Found live and fixed.** A booked order's line read "seated at Balanced" after a mode change; it reads "booked at"
now, with a test assertion and a control. The live run was not repeated after it.

## Choices where the plan was silent

1. **The mode lives in `seats.toml`'s own `mode =` line**, not a `mode` file: `seat pick` already reads it, and one
   place cannot disagree with itself. The line is rewritten in place; a result that does not parse to the new mode is
   not written, and a refused table is never touched.
2. **A Nirvana tab** is one with a herdr agent named `nirvana` or `nirvana-<n>` (`nirvana_launch::is_nirvana_agent_name`,
   O4a's own test for "is this Nirvana Claude"). The square reads that agent's pane folder, its git root found by
   walking the folder tree (`nirvana_board::root`), because `Project::from_env` runs `git`.
3. **Open orders** are a run's orders whose phase is not terminal, delivered and asked included (the gate has not
   ruled, or someone must answer), and a live lease of an open run with no `seated` ("booked"). A lease of a run this
   project has no journal for is left out: `leases.json` is the machine's and a lease does not name its project.
4. **Use** is the order's tokens at its seat's price on its seated day (`computed_use`, the receipt's basis), else
   the booking's expectation marked `~… booked`; the window shown is the one the order weighs most on.
5. **Time** is since seated (or booked); a deferral shows "until HH:MMZ".
6. **The square sits after Running**, before Preload: the tab's seating right after what its Claude does. It is
   narrow under a 100-column strip; the panel is up to 110 wide (the others 92), wide at 106 cells inside.
7. **A click on any line of an order focuses its pane and closes the panel**; Enter does the same for the selected
   order. An order whose pane is not in the window's snapshot, a detached one, and a booked one say why instead.
8. **The read runs on the window's timer tick** (`take_changed`, where Running and Accounts report changes), at most
   every 2 s, on its own thread; a project not drawn for 30 s is dropped. While an order runs, the view is reported
   changed every read so its time moves.
9. **`herdr seat mode`** prints or sets the mode from a shell and logs the change the same way, for the orchestrator
   and for a window that is not open.
10. **Re-seats keep their parent's mode** (O3: `request.mode = seated.mode`). A mode change seats the next new order;
    I did not change O3's re-seat (see "Not measured").

## Controls

68 in `nirvana/controls.d/o8a.py`, one per assertion, each run alone with `controls.py --only <exact name>` from a
list argv, names stripped of carriage returns. The expected result, written before the run: every one PASS, `git
status --porcelain` the same after each. Result: 68 of 68 PASS, each its test's FAIL line seen, the status the same
after every one. None needed a second try; the three existing controls whose find text this package's wiring first
broke (`panels_both_open`, `memory_panel_beside_preload`, `panel_painted_over`) were found by `--dry-run` before the
run, and the wiring was moved so their bytes stand (CHANGELOG entry 4).

## Not measured

- The draw path starting no read is a design claim, not an assertion (a source scan would be hollow).
- `pick_cli`'s use of `default_request` has no test: the CLI reads the live world. The live `seat pick` above
  answered `mode sota` with no `--mode`.
- A real Nirvana Claude pane (O4a's launch) as the tab's agent; the tab here was a shell reporting a `claude` agent
  named `nirvana`.
- A running order in the live window: the recorded worker delivered in 24 s, before the window was up; the running
  row is tested from `rec1-running.jsonl`.
- A worker pane gone from the snapshot, a detached worker, a deferral and an asked order, live (tested on events added
  to the recorded journal).
- Whether a re-seat after a mode change should take the new mode: O3 keeps the parent's.
- Off Windows nothing was built.
