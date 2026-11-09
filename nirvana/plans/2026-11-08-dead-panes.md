# Dead panes: finished workers stop counting and their panes close (2026-11-08)

Builder: "Opus high - dead worker panes". Branch `dead-panes` off master 88be52d6 (alpha 0.58.2). No version bump, no
CHANGELOG line: the merge does those.

## The owner

> "Notice how all the agents underneath are not clcosed and are falsly running. Plus all the dead tabs at the top can
> wew fix this"

Their screenshot: sidebar rows under the spaces counting for days (`quad-x… S5.5 128h8m`, `fixtu… S5.5 391h59m`,
`auth-… Flash 178h9m`, "+6 more"), the agents panel's Run groups, and a tab bar of `run …` tabs plus `Jobs`.

## Measured (read only, the owner's `nirvana` session and its files)

The counts below come from scripts, not from reading; rerun them to refresh (the scripts sit in the builder's TEMP,
`C:\Users\fahim\AppData\Local\Temp\dead\coverage.py` and `backlog.py`, and read a `pane list` / `tab list` dump):

- `pane list`: 123 panes, 116 with a worker label (`<seat>·<order>`) or a job label (`job·<name>`), every one of them
  with a bare `pwsh.exe` as its terminal title; `tab list`: 38 tabs, 29 of them `run …`, the oldest run 2026-10-22.
- `coverage.py`: every one of the 116 has a record the sweep can find (80 workers in a project journal, 36 jobs in
  `<home>/nirvana/jobs`).
- `backlog.py` (the close rule up to the process read): 115 would close if `process-info` reads a bare shell; 1 stays
  because the owner typed in it after its worker ended. The oldest end is 2026-10-22 15:47Z.
- The journals: `quad-xl-slice2` (the 128 h row) is `asked` at seq 57 and `ended delivered` at seq 58; its live lock
  file is not held. `quad-xl-slice4` (127 h) was started and never ended: its supervisor died with a server restart.

## Where each surface comes from

| Surface | Source | Liveness before this change |
|---|---|---|
| The spaces' worker rows (sidebar, plan 30) | `client/shell/nirvana_spaces.rs` `space_workers` -> `agent_sidebar::group_subagents_by_run(..).live` | `is_live_subagent(state)` = `SubagentState::Running` |
| The agents panel's Run groups (plan 26) | `client/shell/agent_sidebar.rs` `group_subagents_by_run`, finished rows drawn with done/blocked/failed/stopped | same |
| Both rows' data | `nirvana_subagents::read_seat_orders`: replays each run's journal; Seated/Running/Asked/Deferred read `Running`, elapsed `now - seated_at` | **the journal phase alone** |
| The Running square | `nirvana_running.rs`: the orchestrator Claude's own transcript and screen, not the workers | unaffected |
| The statusline's order list ("orders: … asked") | `nirvana_claude::open_orders` / `statusline_text`: the phase word of every non-terminal order of an open run | the phase word, so "running" for a dead supervisor |
| A worker's pane | `nirvana_worker::open_pane`: `tab create --label "run <run>"` or a split, `pane rename <seat>·<order>`, `pane run herdr seat exec` | |
| What ends a worker | `nirvana_worker::exec` holds `live/<order>.lock` (`take_live`) for the supervisor's life; it journals Delivered / Asked / Interrupted / Failed / Ended and exits | `live()` reads the lock; the OS frees it however the process ends |
| Pane closing before this | `exec`: only after a `done` delivery with another run pane in the tab (`worker_should_close_pane`); `close_run_tab`: only at `herdr run end` / `run tidy` | most runs never end; Asked, interrupted, failed and died workers kept their panes |
| A job's pane | `nirvana_job::open_job_pane`: the `Jobs` tab, `pane rename job·<name>`, `pane run herdr job exec <id>`; the record `<home>/nirvana/jobs/<id>/job.json` gets the pane | never closed |

## The cause

1. The rows: `read_seat_orders` called an order live from its phase. An Asked order's supervisor has exited (`exec`
   returns once the phase stops wanting a worker), and a Running one's supervisor dies with a server restart while the
   journal keeps saying `started`. Both rows counted `now - seated_at` for ever.
2. The panes: nothing closes a worker's pane unless its delivery was `done` with a neighbour, or its run ended. Jobs'
   panes are never closed.

## The rules (what was built)

**Liveness** (`nirvana_dead::order_live`, one rule for the panel, the spaces and the statusline): live = the phase wants
a worker (Seated, Running, Deferred) and a supervisor holds the order's lock, or the order was seated under 60 s ago
(`SEAT_GRACE_S`: `open_pane` waits up to 10.3 s for the prompt before it types `seat exec`). Otherwise:
Asked -> `Blocked` ("blocked", waits on the owner, as plan 37b's blocked delivery does); a phase that wants a worker
with no supervisor -> `Stopped`; the time stops at the order's last journal event. The statusline names such an order
"stopped". Deferred with its supervisor waiting in its pane stays live and keeps plan 38's `waiting` (no sparkle).

**The sweep** (`herdr run sweep [--dry-run]`, `nirvana_dead::sweep`, started by the server every 60 s, the first
60 s after the server starts; `app/nirvana_dead_panes.rs`). A pane closes only when all hold, checked in this order:

1. Its label is `<seat>·<order>` with both valid ids, or `job·<name>`. **Never closed:** any other pane.
2. No pane of its tab, itself included, has an agent herdr detects. **Never closed:** the space's own agent tab, or a
   pane a Claude runs in.
3. It is not the only pane of its space's only tab (counted live through the pass). **Never closed:** a space's last
   pane, which would close the space.
4. A record names it: the order in the project's journal (the project from `<home>/nirvana/panes/<pane>`, else the
   first folder at or above the pane's cwd with `.nirvana/runs`) on the label's seat; or the pane's latest job record,
   with the label's name. **Never closed:** a pane no record names.
5. Its worker's supervisor lock is free, or its job's process is gone (`job_state`). **Never closed:** a pane whose
   worker or job runs.
6. It ended at least 120 s ago (`CLOSE_GRACE_S`): the owner sees the last screen of a worker that just ended, and a
   pane a new worker of the run is being started in (renamed, `seat exec` not yet typed: the seat grace) is never
   taken for a finished one.
7. The owner's last key in the pane (`<home>/nirvana/keys/<pane>`, written by the server as it forwards a key,
   `nirvana_run::note_owner_key`) is older than the end. **Never closed:** a pane the owner typed in since its worker
   ended; an unreadable key record keeps it too.
8. `pane process-info` reads a non-empty foreground list of shells only (`nirvana_worker::only_shells`). **Never
   closed:** a pane running anything else, or one whose processes cannot be read.
9. Just before the close, `pane get` reads the same label again (a reused pane is renamed first).

herdr closes a tab with its last pane (`Workspace::close_pane`), so a `run …` tab and the `Jobs` tab go when their
last finished pane does. Closed panes and failures are logged to `<home>/nirvana/dead-panes.log`.

**Nothing the owner needs is lost.** A pane close ends only the idle shell. The run's journal
(`<project>/.nirvana/runs/<run>/journal.jsonl`), the worker's stream (`streams/<order>.jsonl`, and `.log` for a
detached one), the report, receipt and record stay; `herdr seat read <order>` prints the report or the stream's last
lines. A job's output stays in `<home>/nirvana/jobs/<id>/job.log` (`herdr job tail <id>`) until `herdr job clean`.

**The backlog**: the first sweep after the owner restarts Nirvana (60 s after the server starts) applies the same
rules to the old panes; `backlog.py` measured 115 of 116 closeable by the record rules.

## Tests and controls

`src/nirvana_dead/tests.rs` (fake herdr, real journals, job records, key files and supervisor locks in temp folders),
`nirvana_subagents::tests::a_worker_with_no_supervisor_stops_counting`, the reworked
`read_seat_orders_phases_and_ended_runs`, `nirvana_claude::tests::the_statusline_names_an_order_with_no_supervisor_stopped`.
Controls: `nirvana/controls.d/dead.py` (prefix `dead_`), each run alone and red; `p38_waiting_phases`'s find moved.

## Where the plan (the work order) was wrong or departed from

- Plan 38 review S4 made Asked rows live-but-waiting; the owner's screenshot shows that row (`quad-xl-slice2`,
  asked) as the false 128 h runner. Asked is now finished (`blocked`); only a live Deferred order waits.
- The order names "the Running square" as a surface: it reads the orchestrator Claude's transcript and screen, not the
  workers, and was not changed.
- The order's "close a run tab when its last pane closes" needs no code: herdr closes a tab with its last pane.

## The owner's live check

1. Install the merged build and restart Nirvana. Before 60 s pass, in any Nirvana pane: `herdr run sweep --dry-run`.
   Expect `would close …` for the old `run …` / `job·…` panes and `keeps …: <reason>` for any you typed in.
2. Wait a minute. The `run …` tabs whose panes all closed are gone; `Jobs` is gone when no job runs.
   `type %LOCALAPPDATA%\Programs\Nirvana-Herdr\nirvana\dead-panes.log` lists each close.
3. The sidebar rows under the spaces show only workers that run now; the agents panel's Run groups show the ended
   ones as stopped/blocked/done with their time frozen.
4. Seat a short order. Its row counts while it runs; when it ends its pane stays about 2 to 3 minutes, then closes.
   Type into a finished worker's pane before then: that pane stays.
5. `herdr seat read <order>` on a closed worker still prints its report or stream.
