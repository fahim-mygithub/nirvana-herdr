# Package review a: P40a SEAT (c40a-seat 8091c6c5 against 86b37c7e)

Read only. Commands run: `git diff`, `python nirvana/seat_cells.py --check` (ok, 0 differ), `python nirvana/controls.py --dry-run`
(in the worktree: "3236 of 3240 find their text exactly once"; in the master checkout: "3211 of 3211"). No cargo run.

## S findings (fix before merge)

**S1. A live booking of Fable is launched without the fail-closed check.** `src/nirvana_worker.rs:3410` (`book_with`):
`if let Some(lease) = leases.live(&request.order, world.now)` takes the stored pick and never calls `pick`/`scoped_skip`
again. `src/nirvana_seat.rs:4574-4582` (`pick_cli --book` on an existing order) does the same. `scoped_skip` runs only in
`candidate`, so only on a fresh pick. Scenario: 10:00 `herdr seat pick --class plan --mode sota --book o1` with a fresh
`week.fable` 40% used, lease 60 min (small) to 480 min (large, `nirvana_seats.toml:101`). 11:30 the owner has 95% used
(or the window closed at 10:06 and the reading is 84 min old). `herdr seat run --order o1 --seat fable` reuses the lease
and starts `claude -p --model claude-fable-5-2`, which bills usage credits without asking. The same holds for an idle
review lease and for a lease a gate redo books and starts later. The record's claim "every path goes through
`scoped_skip`" is false for this path. Fix: in `book_with`'s lease branch and in `existing_booking`'s caller, when the
leased seat has `extra_windows`, re-run the check on the live world with the lease's own booking taken out of `booked`
(`request.order` is already excluded in `populate_windows`), and on failure release the lease and exit 3 with the
skip's reason. Control: stale reading with a live fable lease must give exit 3.

**S2. Four older controls are red in `--dry-run`; the order's gate is not clean.** `p14_toml_max_week_per_usd`
(`per_usd` line now has `"week.fable"`), `p20_orchestrator_efficient_mode`, `p20_orchestrator_sota_mode`,
`p20_orchestrator_balanced_t_threshold` (find text `let seat = "sonnet".to_string();` etc. went with the
hard-coded branches). The record says nothing about them. Retarget them (`p20_*` to `OrchestratorTable::default()` lines
and to `take_choice`/`orchestrator_from`); as it stands the `t < 0.5` threshold and the efficient default have no
sabotage control at all. Rule 8: a control that finds nothing reads as a pass.

**S3. B2's grace is keyed on file mtime, so a signed-out agy that is running defeats the fix.** `nirvana_accounts.rs`
`agy_gate_account_in`: `settling` uses `metadata().modified()`. A running agy appends to its log, so the mtime stays
under 30 s for as long as the window is open. If that start was signed out (no sign-in line), the gate passes over it
forever and reads the previous log, which names the old account: agy stays `ready`, which is the review's B2 for the
case that matters most (the owner just logged out inside a live agy). Fix: use the log's creation time, or the time
in its file name (`cli-YYYYMMDD_HHMMSS.log`), not mtime. The test ages by passing `later` for the clock and never
holds a file that is both young and still written.

## N findings (nits, to record)

**N1. `CLAUDE_CODE_SUBAGENT_MODEL` reaches workers.** It is written only into the per-folder launch settings file
(`nirvana_launch.rs` `with_subagent_env`, guarded by `args.iter().any(|a| Path::new(a) == launch)`), so plain Claude Code
panes and the plugin's own settings.json do not get it. But settings `env` becomes the Claude process env, and the
Bash tool's `herdr seat run` passes it on: `nirvana_worker.rs:210` `ENV_REMOVED` lists six keys and not this one, so a
Fable or Opus worker's own general-purpose subagent defaults to Sonnet. Same leak as `NIRVANA_ORCHESTRATOR=1`. Add
the key to `ENV_REMOVED` (a hunk in a file this package does not own). The docs order (fetched now): call `model`,
frontmatter `model`, the variable, the main model, since v2.1.251; so the `worker-*` files keep their own model. Make
sure `CLAUDE_CODE_SUBAGENT_MODEL_FORCE` is never set (it would override them).

**N2. The `worker-fable-*.md` agent files can start Fable with no booking.** An interactive Nirvana Claude may spawn
`worker-fable-high` directly; the "booking comes first" line is prose. Interactive Claude Code asks before billing
credits, so this is not the headless case, but the file's description should not suggest the rule guards it.

**N3. `--effort` on `seat run` raises a booked Fable order after the check.** `nirvana_worker.rs` ~3500: effort is
changed after the pick with only `seat.efforts.contains`. A Fable high booking run at xhigh spends 1.56x the
reserved share, outside `fable_reserve`.

**N4. `[orchestrator]` is not "exactly as before" for a table that turns a seat off.** Old code seated `sonnet`/`opus`
whatever `enabled` said; `take_choice` now refuses an off seat and falls to any other claude seat or `LAST_RESORT`.
A table with no `[orchestrator]` and all seats on is identical (defaults checked against 86b37c7e lines 3015-3150:
efficient sonnet medium, balanced opus medium, hot sonnet medium, sota opus high, unread opus medium). An unknown seat
or a non-claude seat is refused at parse (`problems`, line 592-596); an effort the seat lacks is taken nearest below.
Balanced: the climb is computed on the pool of `o.balanced.seat` over `pool.windows` (5h, week), so `week.fable`
never enters the climb, and a Fable choice is gated by `scoped_orchestrator_ok`. Fine.

**N5. `week.fable` denominator is unverified.** `per_usd."week.fable" = 0.00156` and the 10% reserve assume the
`/usage` Fable line is a percent of Fable's 50% allowance. If it is a percent of the whole week, Fable bills credits
at 50% used while the rule sees half left. One live reading against the all-models week settles it; write that down
in the record.

**N6. Upgrade and notice are clean.** v12 is in `HISTORICAL_DEFAULTS`; `differs_from_default` is string work on a text
the square already read (`read_table`), no process, no extra file read, on the square's background thread. An edited
table keeps its text and gets `DIFFERS_LINE`.

**N7. B1, B3, B4.** B1: readings stay from before the lock, `world.now` is renewed, so a reading that went stale during
Jev's call reads "not read" and Fable fails closed; advice is kept only when `same_contest` (winner, effort and
contenders equal), so a booking that the rule would not make is not made. B3: the OS lock is held for read, merge and
rename only, not the CLI read. B4: 2 s after exit, then what was read.

**N8. `seat_cells.py`.** Hand check, fable medium: 43,807 x 0.20 + 1,286 x 40 = 60,201.4 per 1e6 = $0.0602; x 0.04 =
0.002408 (5h), x 0.00078 = 4.696e-05 (week), x 0.00156 = 9.391e-05 (week.fable): all equal the toml. Rust tests hold no
re-typed cell constants (grep of 0.002408, 9.391e-05, 0.0602, 0.0345 outside the toml: none). `EFFORT` has no `max` key,
so a seat gaining a `max` effort would KeyError (fails loudly, acceptable).

**N9. Code shapes kept for old controls.** I could not find the "14 restored" in the record or the commit message.
The one marker in the diff region is the pre-existing `#[allow(clippy::collapsible_if)] // preserves the target line of
the seat_override_cap control` (also on master), which is a misleading comment on live code, not new dead code. The
builder should list the 14 in the record.
