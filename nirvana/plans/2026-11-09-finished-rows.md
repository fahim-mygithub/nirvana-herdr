# Finished rows: workers show once, ended runs leave the panel (2026-11-09)

Builder: "Opus high - finished rows cleanup". Branch `dead-rows` off master a57fa44a (alpha 0.60.1). No version bump,
no CHANGELOG line: the merge does those. Follows 0.59.0's dead panes (`2026-11-08-dead-panes.md`).

## The owner

> "The ui cleanup still isint proactive. For example I see a claude idle underneath 3d-pritner low. Plus the runs on
> the bottom are not cleared. in the agents panal."

Their screenshot: under 3D-Printer Flow, "Nirvana O5.5 idle", "Claude S5.5 idle", "quad-v5… S5.5 4m50s"; the agents
panel's Run groups `quad-v5` (live), `bs-2-09`, `quadxlslice`, `quad-split` (stopped/blocked) and `print-audit`
(done); the statusline's "orders: bs-2-09-bump-s1 opus interrupted, audit-adapter-settings sonnet delivered, ...".

## Where each surface comes from

| Surface | Source | Cause |
|---|---|---|
| A space's agent rows ("Claude S5.5 idle") | `client/shell/nirvana_spaces.rs` `space_agents` (called from `sidebar.rs` twice), over `snapshot.agents`; the server fills that from `App::collect_agent_infos` (`app/agents.rs`, every pane herdr detects an agent in) via `server/client_shell.rs` `snapshot_with_completions`, with the model token from `nirvana_spaces::cached_model` | `space_agents` had no worker filter. The agents panel (`agent_sidebar::agent_rows`) and the spaces' worker rows (`space_workers`) did, by `nirvana_subagents::is_seat_worker_pane`, a set the poller fills only after it has read the run. herdr detects the worker's `claude -p` in its pane while it runs, so the worker showed as a space agent and as its order's row |
| The agents panel's Run groups | `nirvana_subagents::read_seat_orders` (the poller, `all_subagents.extend(read_seat_orders(..))`) -> `agent_sidebar::group_subagents_by_run` -> `render_grouped_subagents` | `read_seat_orders` dropped a finished order only when its run had `herdr run end`; most runs never end, so their groups stayed for good |
| The statusline's order list (the foot under the Nirvana Claude) | `nirvana_claude::open_orders` over `open_runs` -> `statusline_text`; Claude Code runs `herdr nirvana statusline` every 30 s (`refreshInterval`, `nirvana/windows_shell.py`) | every non-terminal order of a run with no `ended` line: Delivered, Asked, Interrupted and stopped orders stayed for good |

## The rules (what was built)

1. **A worker shows once.** `nirvana_spaces::is_worker_pane`: a pane whose label is `<seat>·<order>` or `job·<name>`
   (`nirvana_dead::parse_label`, the names `open_pane` and `open_job_pane` give it before the worker starts), or one the
   poller saw an order seated in. `space_agents`, `space_workers` and `agent_rows` all leave such a pane out, so the
   worker appears only as its order's row under the orchestrator, with the order's state (Running from the supervisor's
   lock, `nirvana_dead::order_live`), never herdr's idle/working guess. The space's sparkle (`space_working`) still
   counts a worker pane herdr sees working (plan 38): unchanged.
2. **Ended runs leave the panel.** `nirvana_dead::ROWS_GRACE_S` = 300 s, `still_shown(live, last_at, now)`. In
   `read_seat_orders` a run's group is shown while any of its orders is live, or until 5 minutes after its newest
   order's last journal event. A run with a live order keeps its ended orders beside it, as context (chosen: they are
   the steps that led to the live one, and the panel's cap already bounds the lines). A hidden run's panes are still
   recorded as worker panes, so a pane left open never becomes an agent row.
3. **Ended orders leave the statusline.** `open_orders` lists an order while it is live, or until 5 minutes after its
   own last event, whatever its run (chosen per order, not per run: the foot is one line).
4. **Why 5 minutes.** The orchestrator is woken with the outcome within the minute and reports it in its own pane; a
   finished worker's pane closes 2 to 3 minutes after its end (`CLOSE_GRACE_S` plus a pass); the statusline refreshes
   every 30 s and the poller every 10 s (2 s while a row runs). So a row outlives its pane by a couple of minutes and
   leaves before the owner would have to clear it.
5. **The dry run from a plain shell.** `nirvana_dead::sweep_target`: inside herdr, its own server; from a plain shell,
   `--dry-run` asks the session `--session` named, else `nirvana` (the only session whose server sweeps), by setting
   `HERDR_SESSION` for the herdr calls the sweep makes as children. A closing pass from a plain shell is refused with
   "not in herdr: only `herdr run sweep --dry-run` runs from a plain shell; the Nirvana server sweeps every minute": it
   would not know which tabs the window's clients have on screen (review S2's `--watched`).

**Nothing on disk changes.** The journals, streams, reports and receipts stay. To see an old run: `herdr run status`
(every run's orders in the project, `--json` too), `herdr run status <run>`, `herdr run receipt <run>`, `herdr seat
read <order>`. No history screen was built.

## `wP:p5` stayed: the owner-key rule, working as meant

`wP:p5` (`sonnet·crawl-beta-gui-3-s1`, tab `run studio-gui`). Its order ended at seq 203, `at` 1792746293
(2026-10-23 09:04:53Z, `ended stopped` after `herdr seat stop`); the run ended at seq 236. The owner's key record
`%LOCALAPPDATA%\Programs\Nirvana-Herdr\nirvana\keys\wP_p5` holds 1792840486 (2026-10-24 11:14:46Z), a day after the
end. Rule 7 of the sweep (`decide`: "the owner typed in it after its worker or job ended") keeps it; a kept pane is not
logged, so `dead-panes.log` has no line for it. Every earlier rule passes: its label parses, its tab holds no agent and is
not the space's last, its order's lock (`runs/studio-gui/live/crawl-beta-gui-3-s1.lock`) is free. The test
`the_studio_gui_pane_is_kept_by_the_owners_later_key` runs `decide` on those numbers. Not a sweep bug. Two cautions for
the owner: the key record is keyed by pane id alone, not by session (review S6), so a key into a `wP:p5` of another
session would read the same; and the keep has no end, so the pane stays until the owner closes it. Closing it by hand
(or typing `exit` in it) is the fix; changing the rule is the owner's call.

## Tests and controls

| Item | What was done | Test | Controls |
|---|---|---|---|
| 1 surfaces | cited above | - | - |
| 2 a worker shows once | `is_worker_pane`; `space_agents`, `space_workers`, `agent_rows` use it | `client::shell::nirvana_spaces::sparkle_tests::a_worker_shows_once_as_its_orders_row` | `rows_space_agents_skip_workers`, `rows_label_names_a_worker`, `rows_panel_skips_workers`, `rows_workers_skip_worker_panes` |
| 3 runs leave the panel | `ROWS_GRACE_S`, `still_shown`, `read_seat_orders`'s `run_shown` | `nirvana_subagents::tests::an_ended_run_leaves_the_panel_after_the_grace`, `nirvana_dead::tests::an_ended_row_stays_the_grace_and_a_live_one_always` | `rows_run_leaves`, `rows_live_run_stays`, `rows_run_newest_event`, `rows_hidden_pane_recorded`, `rows_grace_rule` |
| 3 the foot | `open_orders_at` filters by `still_shown` | `nirvana_claude::tests::the_statusline_lists_only_live_and_recent_orders` | `rows_foot_leaves`, `rows_foot_live_stays` |
| 4 `wP:p5` | evidence above; no code | `nirvana_dead::tests::the_studio_gui_pane_is_kept_by_the_owners_later_key` | `rows_p5_owner_key` |
| 5 dry run from a plain shell | `sweep_target`, `HERDR_SESSION` for the children | `nirvana_dead::tests::a_dry_run_from_a_plain_shell_asks_the_nirvana_session` | `rows_dry_run_nirvana_session`, `rows_closing_pass_refused_outside`, `rows_inside_herdr_own_server` |

Older tests moved inside the grace (their `now` was far past their journal's events, which now hides the run):
`a_worker_with_no_supervisor_stops_counting` (5 000 and 900 000 -> 1 100 and 1 300),
`wake_pane_seat_orders_follow_the_run_to_its_new_orchestrator` (2 000 -> 1 100),
`the_statusline_names_an_order_with_no_supervisor_stopped` (`open_orders_at(.., 1100)`). Older controls whose find
moved: `p30_own_space_only`, `p30_seat_worker_filter` (`live_in_spaces.py`, now on `is_worker_pane`); each run alone,
red, with `dead_rows_read_the_lock`, `dead_rows_time_stops`, `dead_status_word` (listed by `touched.py`).

**Untested, by design:** the `HERDR_SESSION` set in `nirvana_dead::cli` and a real dry run against a real server (no
test reaches a server); Claude Code's own statusline refresh.

## Where the work order was wrong

- Item 1 said `agent list` reported `wP:p4N` as `agent: claude, agent_status: idle`. The `rows-agent.json` given with
  it lists five agents, all `nirvana*` Claudes in each space's tab 1; `wP:p4N` is not among them, and `rows-pane.json`
  gives it no `agent`. The lists were read after the worker's `claude -p` had ended (the sweep closed `wP:p4N` and
  `wP:p4P` at 18:44:27Z, `dead-panes.log`); the screenshot's row was herdr's detection while the worker ran. The cause
  stands either way: the spaces list had no worker filter.
- Item 4's "was not closed although run `studio-gui` ended": closed by design (above).

## The owner's live check (in order)

1. Restart Nirvana with the reload badge once the build is installed.
2. Seat a short order. While it runs, its space shows the orchestrator's row and the worker's order row
   (`<order> S5.5 <time>`), and no "Claude S5.5 idle" row; the agents panel lists it once, under its Run group.
3. Let it end. Its row turns done/blocked/stopped; about 5 minutes after the run's last order ended, the whole Run group
   leaves the panel (within 10 s more, the poller's reread). A run with a live order keeps its ended orders.
4. The statusline under the Nirvana Claude lists only live orders and those that ended under 5 minutes ago (it refreshes
   every 30 s). The old `bs-2-09-bump-s1 opus interrupted, ...` entries are gone on the first refresh.
5. From a plain PowerShell (not a Nirvana pane):
   `& "$env:LOCALAPPDATA\Programs\Nirvana-Herdr\bin\herdr.exe" run sweep --dry-run` prints a verdict per labelled
   pane of the `nirvana` session; `wP:p5` reads `keeps wP:p5 (sonnet·crawl-beta-gui-3-s1): the owner typed in it after
   its worker or job ended`. Without `--dry-run` it refuses and says why.
6. An old run is still readable: `herdr run status`, `herdr run receipt <run>`, `herdr seat read <order>`.
