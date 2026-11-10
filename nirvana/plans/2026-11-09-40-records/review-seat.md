# Review: area "seat" (Nirvana-Herdr master 86b37c7e, read only)

Read in full: nirvana_seat.rs (non-test 1-4215, plus the tests I cite), nirvana_seats.toml, nirvana_usage.rs (1-1640),
nirvana_accounts.rs (1-720), nirvana_idle_review.rs (1-1120), nirvana_seats_square.rs (1-400), nirvana.rs (1-330),
nirvana_launch.rs (150-300, 470-760). Skimmed: nirvana_preset.rs maybe_upgrade, nirvana_superpowers.rs, client/shell/nirvana_seats.rs
(only unwrap/cast scan). No cargo, no git change. One read-only Python script compared every committed version of
nirvana_seats.toml against HISTORICAL_DEFAULTS (result under Q3).

## B (must fix)

### B1. `herdr seat pick` holds `leases.lock` across a network call to Jev
- `src/nirvana_seat.rs:4014` `take_lock`, then `:4049-4059` `advise_seating(... true)`, and only `:4077` `drop(lock)`.
- `advise_seating` (`src/nirvana_jev_roles.rs:413`) calls `client.ask` whenever the setting is not Off, the pick is not an
  override, and `pick.contenders.len() >= 2` (`seating_question`, `:317`). The setting defaults to `shadow`
  (`nirvana_jev_roles.rs:122`, missing file or key), and shadow still asks; `live_client()` is Some whenever the Jev key file
  exists (it does on the owner's machine). `CALL_TIMEOUT` is 20 s (`nirvana_jev.rs:48`) plus a retry inside that deadline.
- `Lock::take` waits 10 s (`nirvana_seat.rs:2622`) and then fails (`take_lock` exits 4; `record_once` returns `Err(TimedOut)`,
  `:2901`, which `record_cli` propagates at `:4200`).
- Scenario: the orchestrator seats three orders in parallel (the case the lock exists for, "four parallel picks in five
  seconds"). Pick A holds the lock while OpenRouter takes 14 s. Picks B and C, and the end-of-order `seat record` of a running
  worker, time out at 10 s: B and C exit 4 "nothing was changed, try again", and a `seat record` fails with an I/O error, so the
  order's outcome line is not written. A hung Jev call turns every seat command into a 10 s stall.
- Fix: take the lock only for the read-pick-book-write. Ask Jev before the lock on the same world; under the lock re-run
  `pick` without Jev, and apply the advice only if the rule's seat and contender list are unchanged (else keep the rule's pick and
  say "Jev advice dropped: bookings moved"). Also make `record_once` not depend on a lock that a network call can hold.
- Test: a stub `Transport` that sleeps 12 s, `seat pick` on a thread, and on another thread `release_cli`/`Lock::take` 1 s later:
  the second must succeed in under 10 s. Today it fails with TimedOut.
- Confidence: confirmed by reading the path end to end (default settings, key present).

### B2. agy "signed in" is read from old log files, so a signed-out agy is `ready` for up to 8 starts
- `src/nirvana_accounts.rs:182-198` `agy_account_in` takes the newest of the last 8 logs that names a sign-in; `read` (`:243`) turns
  that into `Signed::In`; `nirvana_seat.rs:3473-3481` `cli_state` uses it as the gate for every agy seat.
- Scenario: the owner runs `/logout` (or the token is revoked). The next agy start writes no "authenticated successfully" line, but
  the 7 older logs still have one, so `Signed::In(old email)`, `cli_state("agy")` is None, and `pick` seats flash, argon, g-sonnet or
  g-opus. The worker then fails at its first message. With a herdr window open the usage foot starts agy every 240 s
  (`AGY_EVERY`), so it takes about 32 minutes (8 starts) to clear; with no window open it never clears. The code comment at
  `:179-181` accepts this for the panel's display, but the seat gate is a spend decision and must fail closed.
- Fix: a sign-in counts only if the newest log (by name) names it; an empty or missing newest log is `Out`. Keep the 8-log scan only
  for the display email.
- Test: logs `cli-..._11.log` (signed in) and `cli-..._13.log` (no line): `agy_account_in` must be None, and
  `signed_within`/`cli_state` must report not signed in. The existing test at `nirvana_accounts.rs:758` pins the opposite and must
  change.
- Confidence: confirmed for the read path; plausible for the log format (no `/logout` log was read).

### B3. `usage.json` writes lose each other across processes
- `src/nirvana_usage.rs:1044-1066` `write_source`: `static WRITING` serialises threads of one process only; the read of the whole
  cache, the merge and the rename are not under a file lock. Windows and `herdr usage` / `herdr seat pick`
  (`current_sources`, `:1608-1640`, which writes after every stale read) are separate processes.
- Scenario: a worker's `seat pick` reads Claude (2.8 s) while the window's Antigravity thread finishes (20 s reads). Both read the
  file before either renames, each writes only its own provider's entry on top of the old copy, the later rename drops the
  earlier one. The dropped Claude entry is then absent, `needs_read` is true, and the next pick spends another `claude -p /usage`
  (about 3 s) and a window reads again. Not data loss, but readings flicker to "not read" and `orchestrator_seat` answers
  "readings missing" (opus medium) in the gap.
- Fix: hold an exclusive `File::lock` on `usage.json.lock` for the read-merge-write (as `change_json` in nirvana_idle_review.rs:375
  already does).
- Test: two threads each writing a different provider 200 times; afterwards both providers must be present.
- Confidence: plausible (the race window is small, the path is real).

### B4. `run_in` can block forever after the child has exited
- `src/nirvana_usage.rs:788-826`: the deadline covers only `try_wait`. After `Ok(Some(_)) => break` it calls `reader.join()` with no
  limit. The reader thread ends only at EOF of the child's stdout; a grandchild that inherited the pipe (an update helper,
  a daemonised MCP/browser process that `claude` or `agy` starts) keeps it open.
- Scenario: `claude -p /usage` exits at 2.8 s but leaves a background child holding stdout. `join` never returns, so the usage
  thread for Claude stops forever (the foot freezes, `Source` never updates), and in `current_sources` the `thread::scope` never
  ends, so `herdr seat pick` hangs with no timeout at all. The timeout branch (kill and return) is only reached while the child
  is alive.
- Fix: after exit, `join` through a channel with `recv_timeout` of a few seconds; on a miss return what was read (or None).
- Test: spawn a child that starts a detached `ping -n 60` holding the pipe and exits; `run` must return in under `timeout`.
- Confidence: plausible (no observed hang; `--strict-mcp-config` removes the main source, the helper case remains).

### B5. A damaged `idle-review.json` switches idle review back on and resets its caps
- `src/nirvana_idle_review.rs:351-356` `read_json` turns any read or parse error into `Default`; `change_json` (`:375`) then writes
  that default back. `Global.off` defaults to false and `facts.on = !global.off` (`:1058`).
- Scenario: the owner ran `herdr lessons idle off`. A partial or locked read of `idle-review.json` (an editor or backup tool holding
  it, a torn file after a crash on a path where the rename did not land) reads as `Global::default()`, `on` becomes true, and the
  next `change_global` (every tick calls it, `:1025`) overwrites the file, so "off" and the per-window review counts are gone and
  a paid review can queue. The module header promises "any fact it cannot read is a reason not to run".
- Fix: `read_json` returns `Result`; `change_json` refuses to write when the file exists and fails to parse (keep it as
  `.bad`), and `ready()` treats a read error as `doubt`.
- Test: write `{` into `idle-review.json` after `set_on(false)`; `tick_with` must not queue and the file must not be replaced.
- Confidence: plausible (rename-based writes make a torn file rare; a sharing violation on read is real on Windows).

(5 B kept; nothing dropped.)

## M (model and seat drift)

### M1. The table's Fable seat is stale for Fable 5.2 (see the change list below)
`src/nirvana_seats.toml:171-188`: `claude-fable-5-1`, efforts `["medium"]` only, one use cell, prices dated 2026-10-17, off.

### M2. Edited seats.toml never gets a changed default, and nothing says so
`maybe_upgrade` (`nirvana_seat.rs:712`) rewrites only a file that equals a historical default apart from the `mode` line. A file with
any other edit is left alone silently; `table()` parses it and `seat list` shows its old prices and models. After the Fable 5.2 change
an owner who edited one score never sees the new seat, and no line tells them. Fix: when the file is not the current default,
`seat list` and the Seats square print one line "seats.toml differs from herdr's default (herdr seat diff)", and add a `seat diff`
that prints the unified difference to the new default. Full answer under Q3.

### M3. Max-pool `week` use cells still use the old $200 week factor
`nirvana_seats.toml` opus, sonnet, haiku, fable cells: week values equal the 5h value times 0.125, e.g. sonnet medium
5h 0.00088 and week 0.00011 = $0.022 x 0.005. Plan 14 (`nirvana/plans/2026-10-18-14-seat-spend-down.md:24-25,63`) changed
`per_usd.week` from 0.005 to 0.00078 (pool max, line 83) but not the cells, so every Max `week` expectation is 6.4 times
too high ($0.022 x 0.00078 = 0.0000172). Effect: week `holds`, `spare` and pressure are pessimistic, and an order's
`expected` booked in `leases.json` over-books the week by 6.4 times. Fix: recompute the max-pool week cells in a script
from tokens x price x per_usd (CLAUDE.md rule 4) instead of hand values.

### M4. Cache-read prices in the table are inconsistent (Q2, below)
opus 0.20 against input 4.0 (5%), sonnet 0.20 against 2.0 (10%), haiku 0.01 against 0.10 (10%), fable 0.25 against 10.0 (2.5%),
g-opus and g-sonnet copy 0.20. I could not verify the published numbers from inside the repo; the lead's 0.10 for Sonnet
would make sonnet 5% of input like opus. All four are labelled "claude-api skill price table, cached 2026-10-06".

### M5. `orchestrator_seat` and `launch` hard-code model fallbacks and seat names
`nirvana_seat.rs:3024,3045,3079,3107,3167,3169,3191` carry six literals of `claude-opus-5-5` / `claude-sonnet-5-5` used only when the
table lacks the seat, and `nirvana_launch.rs:168-169` another pair. The seat ids `"opus"`, `"sonnet"` and the efforts
(Medium, High) are code, not table. It also ignores `enabled` and `efforts`: an owner who turns `opus` off or removes `medium`
still gets it as orchestrator. A table-driven rule (`orchestrator = { efficient = "sonnet:medium", sota = "opus:high",
balanced_hot = ..., balanced_cool = ... }` in `[constants]`) removes the literals and makes Fable a table edit.

### M6. Header comment says SOTA lifts to xhigh; the code caps at high
`nirvana_seat.rs:25-26` ("SOTA lifts plan and load-bearing builds to xhigh; max is never chosen") against `:1217-1220`
(`want > High && parent.is_none()` becomes High, note "capped at high") and the table (`sota_effort = "high"` everywhere). An
`xhigh` entry in a seat's `efforts` (opus has it) is reachable only through `--override seat:xhigh` or a parent's effort.
For Fable 5.2 with xhigh this matters: the rule will never seat it at xhigh. Decide: lift the cap per class (for example `max_effort`
in the class, default high) or fix the comment.

(6 M kept.)

## T (token waste)

### T1. Shadow-mode Jev asks on nearly every pick
Default `seating = shadow` asks OpenRouter on every pick with 2+ contenders and acts on nothing (`acted: false`); it costs a
paid call and the B1 lock hold for a log line. Make the default `off` until the owner turns it on, or sample (one in N).

### T2. `claude auth status` and `agy` log scans run at every pick
`live_world` (`nirvana_seat.rs:3597-3601`) starts a thread and a `claude auth status` process (about 0.5-1 s) and reads up to 8 agy
logs per pick. Cache the per-CLI sign-in in `usage.json` for 60 s (a sign-in does not change in a minute). Not a model turn.

(2 T kept.)

## D (dead code and cleanup)

### D1. `src/nirvana_seats_v1..v11.toml` are 239 KB of full copies used as a list of fingerprints
`HISTORICAL_DEFAULTS` is compared byte for byte only to detect "the owner never edited this". Checked: `grep -rn nirvana_seats_v`
finds the list in nirvana_seat.rs and `nirvana/controls.d/lrs.py`; tests parse `HISTORICAL_DEFAULTS[0]` and `[2]` (v3, about 20 tests
use it as a frozen "Plan 14 table") and v7-v11 in `lrs_an_installed_34a_or_35c_default_upgrades_to_r2`. Keep v3 as a fixture; the
others could be sha256 of the normalised text (CRLF to LF, mode line fixed) in one `const`, computed by a script. Also add a test
that every committed default is in the list: I ran that check by script and all 12 committed versions are covered today
(80ec1044 v1 ... 40917ee3 v8, dd7a3963 current), but v7 and v9-v11 had been missed before (nirvana_seat.rs:50-55 says so), and nothing
stops the next miss. A control that fails when `git show HEAD~:src/nirvana_seats.toml` is in none of the lists would.

### D2. Tests that parse an old table instead of the shipped one
About 22 tests start `let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();` (4672, 4692, 4735, 4755, 4825 ... 6095) and so pass
whatever the current `nirvana_seats.toml` says; the current table's cells are pinned only by `the_default_table_...` (4308) and
`:4573-4581`. A price or score edit in the shipped table is invisible to them. Not a bug; note it before trusting "all green"
after the Fable change.

### D3. `let _ = override_effort;` in `evaluate` (nirvana_seat.rs:1643)
`override_effort` is parsed again in `evaluate` and discarded; `candidate` (2018-2031) already owns it. Remove the binding.

### D4. `#[allow(dead_code)]` on live code in nirvana_launch.rs:496-657
`window_mark_path`, `read_window_mark`, `write_window_mark`, `remove_window_mark`, `is_live_window_mark` are called from `open()`
(`:746-747`); the comment "wired into window and open in step 2" is done. Checked with grep of the names in src/. Drop the allows.

### D5. `record.jsonl` only grows and is read whole
installed file: 323 lines, 321 KB. `read_record` (`:2936`) loads all of it for every `record_once` under the lock, for
`known_projects`, and for `tally` when `use_record` is on. Rotate at a size (keep a year of lines in `record.<n>.jsonl`) or index
by order id. Low urgency (about 1 KB per order).

### D6. Duplicate model helpers
`Seat::use_cell` (nirvana_seat.rs:603) and `nirvana_claude::expected_share` (`nirvana_claude.rs:829`) are the same lookup written
twice; one should call the other. Also the five identical `OrchestratorSeat { .. }` blocks in `orchestrator_seat` collapse into
one constructor.

(6 D kept.)

## X (multimodal)
None in this area. Capabilities `image`, `pdf`, `video`, `audio` are table words only; no code in these files reads an image.

## Fable 5.2 change list (question 1)

Goal: a live `claude-fable-5-2` seat, efforts low..xhigh, its own `week.fable` window, in the classes, and as an orchestrator choice
by mode.

Table (`src/nirvana_seats.toml`)
1. Seat `fable` (171-188): `model = "claude-fable-5-2"`; `efforts = ["low","medium","high","xhigh"]`; `enabled = true` (replace the
   comment at 181, the "orchestrator or nothing" R1 line); check `capabilities` (opus has `browser`, fable does not: a browser order
   would skip it); add a price row dated the release day (keep the old row, `price_at` takes the latest whose date came); one `use`
   cell per effort, each with `week.fable` as well as `5h` and `week` (`problems()` rejects a cell naming a window the seat does not
   spend, and a missing effort falls to the nearest dearer cell, so xhigh with no cell silently uses the highest one).
2. Pool `max` (83): add `"week.fable" = <factor>` to `per_usd`. `computed_use` (nirvana_seat.rs:2793-2799) takes `pool.per_usd.get(window)?`
   for each of `seat_windows`, so without it a recorded Fable order has no `week.fable` share and the record under-reports it.
3. Classes: add `{ seat = "fable", score = N }` where wanted: today no class lists it. Each class lists its seats in tie-break
   order (the class's order breaks the last tie), so position matters. Bars: a fable score above 95 raises `best_score` and so
   the dropped bar for `plan`, `build.load_bearing`, `review.adversary`.
4. `version = 1` (line 10) stays; `Table::problems` checks `version != 1`.

Upgrade path (must come first, or installed tables never move; see Q3)
5. Before editing the toml, copy the current default to `src/nirvana_seats_v12.toml`, append `include_str!` to `HISTORICAL_DEFAULTS`
   (`nirvana_seat.rs:43-59`), and extend `lrs_an_installed_34a_or_35c_default_upgrades_to_r2` (6859) to list v12.

Code
6. `orchestrator_seat` (`nirvana_seat.rs:3015-3208`): ids and efforts are literals in four branches (Efficient sonnet medium; SOTA opus
   high; Balanced cold opus medium; Balanced hot opus or sonnet medium). A Fable orchestrator needs a new arm (for example SOTA ->
   `fable`), `OrchestratorSeat.seat` consumers keep working as a string: `nirvana_seats_square.rs:383-385` (`restart_to_switch`
   compares seat and effort), `nirvana_launch.rs:177-198` (model and effort for `--model` and `--effort`),
   `nirvana_claude::SessionSeatRecord`. In Balanced, `window_names` come from pool `max` only (3062), so `week.fable` is not in the
   climb; add the seat's `extra_windows`.
7. `effort_for` cap at High (1217): needed if Fable is to be seated at xhigh by the rule (see M6).
8. `nirvana_launch.rs:168-169` `MODEL`/`EFFORT` fallback consts, and the seven fallback literals in `orchestrator_seat`.
9. `nirvana_usage.rs:495-502`: the window name is `week.` + `slug(label in /usage)`. It is `week.fable` only while the line reads
   "Current week (Fable)". If the 5.2 line reads "(Fable 5.2)" the window becomes `week.fable-5-2` and the seat's `week.fable`
   is never read (tier 1, "not read", forever). Check a live `claude -p /usage` first, then pin the mapping (strip a version).
   `expected_windows(MAX)` (161-167) does not list `week.fable`, so an absent line is not even marked unknown.
10. `nirvana_spaces.rs:140` aliases already include `fable`; `claude_label` and `compact_model` are generic (labels reach
    `compact_model`, not ids: `Claude Fable 5.2` gives `F5.2`). No change; but `compact_model("claude-fable-5-2")` on a raw id gives
    `F5` (hyphen split), so keep feeding it labels.
11. `nirvana_claude.rs:819` `seat_for_model`: matches `seat.model == model` or `model.contains(seat.id)`; "claude-fable-5-2" contains
    "fable". Fine once the table is right.
12. `nirvana/claude/agents/`: no `worker-fable-*.md` and `nirvana.md` fixes `model: claude-opus-5-5`; add worker files only if
    the orchestrator may spawn Fable subagents. `nirvana/claude/skills/usage/SKILL.md:17` and the seat skill name `week.fable`/models in prose.

Tests (src)
13. `nirvana_seat.rs:4324` (`on` list, add fable), `:4332` (`off` list: remove fable), `:5970` (`seat("fable")["why_not"] == "off in seats.toml"`),
    `:4315` comment; tests at 4246 and 6108 already build a `week.fable` reading; `:4671-4678` use v3 (fable off) and stay green.
    `nirvana_spaces.rs:853-854` (label test 5.1: add 5.2), `client/shell/tests/nirvana_subagents.rs:1110` (`Fable 5.1`),
    `nirvana_worker/tests.rs:369`, `nirvana_idle_review/tests.rs:971`, `nirvana_claude/tests.rs:32,964`, `client/shell/tests/nirvana_footer.rs:347,622`
    use `week.fable` only as a window name.
Controls
14. `nirvana/controls.py:830` `seat_toml_fable_on`: the find text is the R1 comment plus `enabled = false`; it stops matching the moment the
    comment changes (the control then reads "find text not on a product line"). Rewrite it to sabotage `enabled = true` to false.
    `controls.py:200` (`if name == "all models" {` -> `"Fable"`) is generic. `controls.d/o4b.py:14` writes the literal `claude-fable-5-1`
    as a sabotage value; rename to 5.2. `controls.d/lrs.py` (HISTORICAL list) and `controls.d/foot2.py:40` need a pass.

## Q2. Sonnet's cache_read 0.20

Where it sits: `nirvana_seats.toml:146` (sonnet), `:204` (g-opus, 0.20 copy of opus), `:223` (g-sonnet, 0.20 copy of sonnet).
Numbers derived from the Sonnet figure (P4's order: 43,807 cache reads and 1,286 out):
- sonnet medium use cell: `43,807 x 0.20 / 1e6 + 1,286 x 10 / 1e6 = $0.0216` ("$0.022"), times 0.04 gives 5h 0.00088; low = x0.6 (0.00053), high = x1.6 (0.0014). With 0.10 the order is $0.0172 and every sonnet cell falls by 20%.
- week cells (0.00011, 0.000066, 0.000176) via the stale 0.005 factor (M3).
- g-sonnet cells (`:226-228`, "scaled by 0.207 and 0.221 from P4 g-sonnet medium", test `nirvana_seat.rs:4573-4581` pins 0.0068517 / 0.0036686 / 0.0058786) and the v3 test at `:5877-5878` (3.31% 5h, 1.66% week from 45,863 reads and 1,287 out at g-sonnet price) are built on the same 0.20.
- opus and fable cells use their own cache prices (`opus` 0.20: $0.0345; `fable` 0.25: $0.0753); haiku uses 0.01. They are not derived from sonnet's, but they carry the same inconsistency (M4).
- `cost()` (`:628`) and `computed_use` (`:2783`) apply the price to every recorded order's tokens; the Seats square's "spent" figure (`nirvana_seats_square.rs:130-133`) and `record.jsonl` `spent` all move with it.
- `price_scale` (`:618`) uses input plus output only, so a cache price change does not touch the Argon doubling.
To change it: edit the price row, recompute the sonnet and g-sonnet cells from tokens by script, update the basis strings, and fix
the test constants at 4573-4581. Do it as a new dated price row if the old price applied to past records.

## Q3. How a changed default reaches an owner

`read_or_create` (`nirvana_seat.rs:656`) calls `maybe_upgrade` on every read. It reads the file, converts CRLF to LF, replaces the
top-level `mode` line with `mode = "balanced"` in both the file and each historical default, and compares.
- File equals the current default (mode aside): nothing happens.
- File equals any `HISTORICAL_DEFAULTS` entry (mode aside): the file is copied to `<home>/nirvana/backup/seats.toml` (overwriting an older
  backup), then `write_atomic` writes the current default with the owner's `mode` line kept. Verified by script that all twelve
  committed versions (80ec1044 ... dd7a3963) are in the list or are the current file, and the installed
  `%LOCALAPPDATA%\Programs\Nirvana-Herdr\nirvana\seats.toml` equals the repo default today.
- File differs in any other way (a score, `use_record = true`, a comment, a disabled seat): untouched forever, no message. It is the
  owner's, and `parse` accepts it as long as it validates. They keep old models and prices; a seat id new in the default is simply
  absent (a class that names a seat they lack is a `problems()` error, so a half-merged file is refused with its reason).
- File missing or refused: missing is rewritten from DEFAULT; refused stays refused ("delete it to get herdr's default back").
Gaps: no `version` bump drives it (`version = 1` is fixed); the upgrade depends on a v-copy being added by hand each time
(missed for v7 and v9-v11 once); there is no notice for the edited case (M2).

## Q4. Is the effort passed to the `claude` CLI

Yes, as the flag `--effort <word>`, never as a setting.
- Workers: `nirvana_worker.rs:235-238` `claude -p ... --model <id> --effort <spec.effort.word()>`; for agy the effort is inside the model
  id (`gemini-3.8-flash-low`, `claude-opus-5-5-high`) and no `--effort` is passed (test `nirvana_worker/tests.rs:81`).
- Orchestrator: `nirvana_launch.rs:216-219` `--model <model> --effort <effort>` from `launch_seat`/`orchestrator_seat` (only Medium or
  High today); Restart Claude and a New press build the same pair (`app/nirvana_restart.rs:253-255`, `app/nirvana_new.rs:377-378`).
  `app/nirvana_relaunch.rs` test only asserts `--model`, not `--effort` (the call goes through `launch_args`, which sets both).
- Accepted names: `Effort::parse` (`nirvana_seat.rs:102`) lowercases and trims and takes `low`, `medium`, `high`, `xhigh`, `max`; anything else
  is None (an override `seat:foo` is refused, `evaluate` 1632). The rule never picks `max` (offered list drops it unless an override or a
  parent at max) and caps at `high` unless overridden (M6). A seat's `efforts` in the table are typed `Effort`, so a misspelt word
  fails the whole table parse. `use_cell` for an effort the seat has no cell for takes the nearest dearer cell, else the dearest.
