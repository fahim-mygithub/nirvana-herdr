# P40a SEAT: Fable 5.2 takes a seat, Nirvana Claude's seat from the table, the seat review's bugs

Branch `c40a-seat` off master 86b37c7e (alpha 0.62.0). Order: plan 40 work order P40a. Review fixed from:
`review-seat.md` (area "seat"). Not merged, no version bump, no CHANGELOG line (the merge does those).

## The owner, 2026-11-09

"Fable 5.2 has now released check the claude documentation and update the apps seating now that we have this model
avaiable." and "Do a project lceanup / code review and ifx any bugs."

## Facts from the Claude docs (fetched 2026-11-09 by the lead, quoted, not re-derived)

- Model id `claude-fable-5-2`; `--model fable` resolves to it in Claude Code 2.1.321. Efforts
  `--effort low|medium|high|xhigh|max`; Claude Code's default for Fable 5.2, Opus 5.5, Sonnet 5.5 and Haiku 5.5 is
  `medium`. Prices per MTok: Fable 5.2 input 8, output 40, cache read 0.20, 5-minute cache write 10. Opus 5.5
  4 / 20 / 0.20. Sonnet 5.5 2 / 10, cache read 0.10. 1M context on all.
- support.claude.com, "Claude Fable models on your plan", on Max: "You can use up to 50% of your weekly usage limits
  on Fable models at no extra cost. They draw from your plan's regular weekly usage limits and use them faster than
  other Claude models." Past that: "you can keep using Fable models with usage credits, or switch to another model".
- code.claude.com model-config: "In non-interactive mode with the `-p` flag... Claude Code never asks for consent.
  When a Fable request there would bill to usage credits, Claude Code bills it without asking." So every Fable rule
  here fails closed.
- Prompting guide: "`low` suits ... fast, intelligent subagents", "`medium` suits interactive coding", "`high` suits
  hard coding, research, agentic work, and long unattended runs", "Most of the gain in capability is already present
  at `high`", "Early testers had good results with Claude Fable 5.2 as the planner and orchestrator for other models".
- Subagent model order: the call's `model`, the agent file's `model`, `CLAUDE_CODE_SUBAGENT_MODEL`, then the main
  conversation's model.
- Live `/usage` on 2026-11-09: `Current week (Fable): 2% used · resets Nov 13, 11am`, so the window is `week.fable`.
- `claude --help`: `--autocompact <auto|tokens>` ("Auto-compact window size (auto, or 100k–1M tokens)"); 1M-context
  models compact "at about 967K tokens by default". Read from the installed binary's own text (not run): the parser
  takes "500k, 200000, or 200 as shorthand".

## What was built

1. **Upgrade path first.** `src/nirvana_seats_v12.toml` is the 86b37c7e default byte for byte, appended to
   `HISTORICAL_DEFAULTS`; the 35c upgrade test covers v12; a new test checks the shipped default is none of the
   historical ones and that none of them reads as "differs".
2. **The Fable seat.** `claude-fable-5-2`, efforts low to xhigh, on, opus's capabilities (browser too), a dated price
   row 2026-11-09 beside the old one, one use cell per effort with `5h`, `week`, `week.fable`. Pool `max` gets
   `per_usd."week.fable" = 0.00156`, twice the week's, labelled derived from the doc's 50% rule, not measured (the
   review round replaced it with 0.005, measured once: Part 2 below).
3. **Fail closed** (`scoped_skip` in `candidate`): a seat with `extra_windows` is skipped unless every window it spends
   has a fresh reading, and unless the order leaves `fable_reserve` (0.10, a hypothesis) of each of its own windows.
   It runs in `candidate`, so every mode (SOTA too) and every override, inherited ones included, meets it. The reason
   reads "Fable week not read; a headless Fable run past its week bills usage credits without asking". The general
   floor still applies.
4. **Window name.** `week_scope` strips a trailing version or "model(s)": "Fable 5.2", "Fable models", "fable v5"
   are `week.fable`. `expected_windows(MAX)` is unchanged: the fail-closed rule already reads an absent line as not
   read (no reading at all), so marking it unknown was not needed for it, and would have added a `used: null` row to
   every Max reading the foot and the guard read.
5. **Classes.** fable in plan 97, build.load_bearing 97, build.frontend 96, judge.visual 96, fix.debug 95,
   review.adversary 96, each listed after opus, each `why` saying "a prior (decision 0012)".
6. **step_up** (plan 37, only after invented sources): a sent-back opus order today steps up in research.web to flash
   (84 over opus's 82, the cheaper of flash and argon); with no Fable in the research classes nothing else changes. In
   a class that lists Fable it is the last rung, and the redo's override meets the fail-closed rule. No code change.
7. **effort_for** keeps its cap at high; the header comment now says so (M6). `xhigh` by override only (tested).
8. **`[orchestrator]`** (M5): six choices with serde defaults equal to the old code, one constructor
   (`orchestrator_from`), `enabled` and `efforts` respected (nearest effort below). The shipped table sets
   `sota = fable high`, `sota_fallback = opus high`. SOTA takes Fable only when it is on, its windows are fresh, and
   `week.fable` has `fable_orchestrator_left` (0.25, a hypothesis) left after bookings; otherwise the fallback with why
   ("SOTA, Fable week 82% used", "SOTA, Fable week not read"), which the square shows and `restart_to_switch` compares.
9. **Launch.** `CLAUDE_CODE_SUBAGENT_MODEL` from `subagent_default` ("sonnet") goes into the per-folder launch settings
   file's `env`, beside `NIRVANA_ORCHESTRATOR=1` (the menu, the verb's fresh space, Restart Claude and a restored pane
   all pass through `launch_args_in`; a pane env would miss the verb's fresh space). The `worker-*` agent files name
   their model, which wins over the variable. `--autocompact <tokens>` from `autocompact_tokens` (0, the default since the
   review round, passes nothing;
   outside 100k-1M the table is refused). Interaction: `fresh_above` (120k) starts a fresh session between runs, the
   idle compact types `/compact` into an idle pane past its own token line, and this flag only compacts inside one run
   that grows past 400k. `app/nirvana_restart.rs` and `app/nirvana_new.rs` build no argv of their own: no hunk.
10. **Workers.** `worker-fable-high.md`, `worker-fable-medium.md`.
11. **Cells by script** (M3, M4). `nirvana/seat_cells.py` recomputes every Max cell from P4's tokens, the price row and
    `per_usd`; `--check` fails on a difference. Sonnet and g-sonnet have a 2026-11-09 row with cache read 0.10;
    g-sonnet's cells stay (google3p shares measured in plan 19, not priced).
12. **Bugs.** B1 Jev asked before `leases.lock`, the pick redone under it, advice kept only from the same contest.
    B2 the gate reads agy's newest log only (a log under 30 s old with no line is passed over once). B3 an OS lock on
    `usage.json.lock`. B4 after the child exits, stdout's EOF is waited 2 s and what was read is the answer. B5 a
    damaged `idle-review.json` or space file is a doubt, never written over (a `.bad` copy is kept). M2 one line in
    `seat list` and the Seats square, and `herdr seat diff`. D3, D4, D6 (`use_cell` is public; the guard's copy is a
    hunk). T2 a minute's cache in `signin.json`. T1 not done: the Jev shadow default is the owner's setting.

## The pick table (test `c40a_the_pick_table_on_three_worlds`, `--no-capture`), re-run after the review round

Expected before the run: SOTA seats Fable in the first world and Opus in the other two; Balanced and Efficient seat
Fable in none. Got: as expected. One line per world; build.load_bearing differs since the round (flash's score there
fell under the Efficient bar), so it has its own column:

| world | Efficient | Balanced | SOTA | build.load_bearing E / B |
|---|---|---|---|---|
| Fable week 2% | flash medium | flash high (fix.debug: flash medium) | fable high | argon medium / argon high |
| Fable week 95% | flash medium | flash high (fix.debug: flash medium) | opus high | argon medium / argon high |
| Fable week unread | flash medium | flash high (fix.debug: flash medium) | opus high | argon medium / argon high |

The worlds are the test module's 2026-10-17 readings (max 5h 41%, week 37%), where Balanced already dropped its bar
to flash before this change; that is the old rule's, not Fable's.

## Review round (pkg-review-a.md and the fix order SEAT-FIX-ORDER.md)

- **S1.** `may_spend_now` (public, `nirvana_seat.rs`) asks the fail-closed question on the world as it is now, with
  the order's own booking out of `booked`: every window fresh, `fable_reserve` kept, the seat still on. A seat with no
  scoped window is never refused and its usage is not read. `pick_and_book` calls it before it answers a live lease;
  on a refusal the lease is released under the lock and the answer leads with "Book <order> again with `herdr seat
  pick --book <order>`". `check_before_spend` is the same at a worker's launch, with the release; it is called by the
  hunk for `nirvana_worker::exec`, which also covers N3 (it checks the effort the attempt runs at). `book_with` gets
  its own hunk. The spawn guard (`nirvana_claude::guard_with_leases`) also reuses a lease; it is the interactive path,
  where Claude Code asks before billing credits, so it is recorded here and not changed.
- **S2.** p14 and p20 retargeted (`OrchestratorTable::default()` lines, `let choice = if t < 0.5`); the efficient
  default now has a test (the v12 table in Efficient) and a control. Dry-run: 3256 of 3257, the one being o5's
  `seat_tally_ignores_the_words`, retargeted by a hunk in the report (with it: 3257 of 3257, run and red).
- **S3.** B2's grace reads the log's creation time. A file system without one reads as old: signed out.
- **N1.** Hunk: `CLAUDE_CODE_SUBAGENT_MODEL` joins the worker's `ENV_REMOVED`. `CLAUDE_CODE_SUBAGENT_MODEL_FORCE` is set
  nowhere (grep).
- **N2.** The Fable agent files now say the check runs at the booking, not at the spawn.
- **N4.** An owner's table that turns `sonnet` or `opus` off: each choice passed over is named in the reason
  ("sonnet off in seats.toml"), then the first claude seat still on with no window of its own takes it at medium (or
  its nearest effort below), and the reason says "no named seat can be taken, so the first claude seat on". With
  every claude seat off: Opus medium, "as the last resort", said in the reason. Test `c40a_n4_...`.
- **N5.** Answered by the owner's fixture: the Fable line is a share of Fable's own allowance (comment on
  `claude_readings`).
- **N9.** The 14 restored shapes are not listed in any file I could read (record, commit message, review); the first
  builder's report was not given to me. The one I found: the nested `if` under `#[allow(clippy::collapsible_if)]` "to
  keep the seat_override_cap control's line". It is one condition now; `seat_override_cap` and
  `p24_send_back_cap_exempt` (which found the same nesting) are retargeted and run red.
- **N3** is closed by the launch hunk. **N6-N8** need nothing.

## Part 2: the table refit from the logs (each number's command beside it)

1. `autocompact_tokens` is 0 by default: the 55 automatic compactions since 2026-10-23 started at 201k-429k (the
   lead's measurement, quoted in the toml), so a 400k cap changed nothing. The knob stays.
2. `per_usd."5h"` 0.0073, measured once (the pool's basis). What the 5 h reserve rule now allows: an order's 5 h
   share fell 5.5 times at the factor alone, so a max order that needed more than the 15% reserve plus its share is
   seated with less left. After the refit of item 4 the two cancel for the measured seats (sonnet 6.81, opus 10.58
   times P4), so a medium sonnet order's 5 h share is about what it was, and now matches what such orders spend.
3. `per_usd."week.fable"` 0.005, measured once. In plain terms: the Fable week holds about $200 of list price, so a
   Fable orchestrator can spend it in two or three working days; then SOTA falls back to Opus by the
   `fable_orchestrator_left` rule, and seated Fable orders by `fable_reserve`.
4. `python nirvana/seat_fit.py` (reads the installed `record.jsonl`, writes nothing) prints the cells, the sizes and
   the durations with n. Refit (outside 0.5-2.0 at n of 5 or more): flash low, flash medium, opus medium, opus high,
   sonnet medium. `seat_cells.py` reads each fit from its cell's basis (`fit x<ratio> (<source>)`); an effort with no
   fit of its own takes its seat's medium fit. Fable takes opus's fits and haiku sonnet's, as priors: see "Found
   wrong". Sizes: small 3.0, large 4.1 (large under medium is the record's word, n 27). `minutes` and
   `lease_minutes` kept: the record's p90 is inside every lease. Re-running `seat_fit.py` prints every refit cell at
   1.00.
5. Scores: `build.load_bearing` flash 74 to 68 (under the Efficient bar 70), `review.adversary` sonnet 86 to 89 (over
   the Balanced bar 88), each `why` with its n.
6. `tally_word`: `accepted` a pass; O5's words keep the gate's answer; every other word neither; the last line for an
   order and run decides. `use_record` stays off.

## The numbers from the script

`python nirvana/seat_cells.py` prints the rows; `--check` passes. Medium cells at P4's tokens: opus $0.0345, sonnet
$0.0172 (was $0.022 at cache read 0.20), haiku $0.0011, fable $0.0602. The Max week cells fell 6.4 times (factor
0.00078, not 0.005). Read the script's output for the cells themselves; they are not copied here. Since the review
round each cell also carries its record fit (Part 2 item 4): the dollar figures are P4's, the cells are not.

## Found wrong

- In the order: nothing that changed the build. Two notes: `expected_windows` needed no change for the rule (item 4);
  `app/nirvana_restart.rs` and `app/nirvana_new.rs` need no hunk.
- In the review: B1's "make `record_once` not depend on a lock a network call can hold" is done by moving Jev out;
  `nirvana_worker.rs`'s `seat run` still asks Jev (act false) under the lock: a hunk. M4's haiku "unverified" cache
  price is still unverified: the docs facts named no Haiku price. T2's "in usage.json" became its own `signin.json`.

## Found wrong in the review round

- **Refitting only the cells with n of 5 or more** made every unmeasured seat look 7 to 10 times cheaper than the
  measured ones. Balanced then moved build.bounded from flash medium to haiku low, and Fable's own reserve would have
  been checked against a share a tenth of what an Opus-like order spends. Fable takes opus's fits and haiku sonnet's,
  labelled priors in their basis. argon, g-opus and g-sonnet read 12 to 49 times their cells at n 1 and stay.
- **The audit's "medium near 1" (sonnet medium medium 1.0, n 72)** was measured against the old week cells, which
  review M3 had already found 6.4 times too high. Against the corrected cells medium orders spend 6.8 times (sonnet,
  n 66) to 10.6 times (opus medium, n 8) P4's prior: the cells, not only the sizes, were off.
- **`blocked` as a failure:** plan 37b review S1 ruled it uncounted (a login wall is the owner's input), and
  `nirvana_gate::Outcome::BLOCKED` says so. The tally keeps that ruling; flipping it is one arm in `tally_word`.
- **Fix order S2** names "the hunks the first builder wrote": none were in any file I was given.

## The owner's live checks, in order

1. Install, then `herdr seat list`: `fable ... claude-fable-5-2 ... ready`, its windows `5h`, `week`, `week.fable`.
2. With the window open (fresh readings): `herdr seat pick --class plan --mode sota` gives fable high; `herdr usage`
   shows the Fable week read.
3. Close every herdr window for 6 minutes, then the same pick: opus high, fable "Fable week reading is stale" or "not
   read". A `--override fable --reason x --run r` is refused the same way.
4. Set SOTA in the Seats square: the row reads `fable high · SOTA` while the Fable week is under 75%, else
   `opus high · SOTA, Fable week N% used`; a session started before shows "restart to switch".
5. Start Nirvana Claude and check the launch settings file under `<home>/nirvana/` names
   `CLAUDE_CODE_SUBAGENT_MODEL: sonnet`, and the pane's argv has no `--autocompact` (the default 0). Spawn a
   general-purpose subagent with no model and see it run on Sonnet (the first proof that the settings `env` reaches
   subagents).
6. `herdr seat diff` on the installed table: "seats.toml is herdr's default." after the upgrade.
