# Feature 23: each project keeps its own seat mode

Status: **PLANNED (2026-10-22)**, to build on agy (Gemini 3.8 Flash high).

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Another fefature or change we should implement. Make the seats attach to teh project folder. One project might want balanced or SOTA while the other might want efficicent." | §2 to §6 | orchestrator | unit tests in §7; the live check in §8 |

L1 replaces plan 12 O8a's single `mode =` line for every project, and plan 20 §3's reading of that line for the
orchestrator's seat. The mode's meaning (plan 19) does not change; only where it is read from.

This plan reads "seats" in L1 as the mode. The seat table itself (seats, classes, prices, `off`) stays one file for
the machine. A project that wants its own table is a later plan, if the owner asks.

## 1. Measured 2026-10-22, on master at alpha 0.35.0

- **One mode for the machine.** The mode is the top-level `mode =` line of `<herdr home>/nirvana/seats.toml`
  (`nirvana_seat::SEATS_FILE`, read by `nirvana_seat::table()` at `src/nirvana_seat.rs` `pub fn table`).
- **Setting it** (`src/nirvana_seats_square.rs` `change_mode`) writes that global line through `set_mode_in`, then logs
  `mode_changed` into the open runs of the current project only. So the journal already treats the mode as a
  project event while the file treats it as a machine setting.
- **Readers of the mode,** each through `nirvana_seat::table()`:
  - `herdr seat pick`: `default_request` (`src/nirvana_seat.rs`), `--mode` overriding it;
  - the worker supervisor: `src/nirvana_worker.rs` `pick_request.mode = request.mode.unwrap_or(table.mode)`;
  - the orchestrator's seat at launch: `src/nirvana_launch.rs` `orchestrator_seat(&table, &world)`;
  - the restart notice and the board: `src/nirvana_board.rs` `orchestrator_seat`;
  - the Seats square: `src/nirvana_seats_square.rs` (`mode: table.map(|table| table.mode)` and its `orchestrator_seat`);
  - the spawn guard: `src/nirvana_gate.rs` (the `table.mode` it passes on);
  - `src/nirvana_claude.rs`, `src/nirvana_run.rs`, and `herdr seat mode` (`mode_cli`).
- **A project** is `nirvana_run::Project`: `$NIRVANA_PROJECT`, else the git top level of the folder. A worker's pane
  carries `NIRVANA_PROJECT`, so a worker in its worktree resolves to the project, not the worktree.
- **`<project>/.nirvana/` is local:** in WorkOutAppRefactor it is excluded by `.git/info/exclude`, so a file there
  is never committed to the owner's project.

The builder re-measures every line of this section before relying on it and reports any that are wrong.

## 2. The project's mode file

- **Path:** `<project>/.nirvana/seats.toml`.
- **Content:** one key, `mode = "efficient" | "balanced" | "sota"`, and comments. Any other key, or a bad value,
  refuses the file, with a message that names the path and the line.
- **No file means the machine's mode,** the global `mode =` line, as today. Nothing changes for a project until a
  mode is set in it.
- **Written whole** with `write_atomic`, keeping the owner's comments when it already exists (the `with_mode` rule).

## 3. Which mode wins

From strongest to weakest:

1. `--mode` on the command (`herdr seat pick --mode`, an order's own mode);
2. the project's file;
3. the global `mode =` line.

- **One function decides it:** `nirvana_seat::project_mode(root: &Path, table: &Table) -> Result<(Mode, ModeSource), String>`,
  with `ModeSource::{Project, Machine}`. Beside it, `nirvana_seat::table_for(root: &Path) -> Result<Table, String>`
  returns the global table with its `mode` replaced by the project's.
- **Every reader in §1 that knows its project** calls `table_for` in place of `table()`. A reader inside the server
  (the Seats square, the launch, the board) uses the project of the space it is drawing or launching: the git top
  level of the space's folder, by the same rule as `Project::from_env` without the environment variable.
- **A refused project file** is an error where `table()`'s error is an error today (`herdr seat pick`, the worker). At
  launch it falls back the way plan 20 falls back on a refused table: Nirvana Claude still starts, on Opus medium,
  and the launch line says why.

## 4. Setting the mode

- **`herdr seat mode`** prints the mode and where it came from: `efficient (this project)` or `balanced (machine)`.
- **`herdr seat mode <mode>`** writes the current project's file and logs `mode_changed` in its open runs, as today.
- **`herdr seat mode --machine <mode>`** writes the global line: today's behaviour, for projects without a file.
- **`herdr seat mode --clear`** removes the project's file, so the project follows the machine again.
- **The Seats square's mode toggle** writes the file of the space it is drawn for.
- The usage text (`USAGE` in `src/nirvana_seats_square.rs`) says all of this.

## 5. The orchestrator's seat

- **Each space's Nirvana Claude** takes its seat from its own project's mode at launch (plan 20 §3's table, unchanged).
- **The restart notice** compares a space's running seat with its own project's mode. Setting one project's mode never
  raises the notice in another project's space.

## 6. What the owner sees

- **The Seats square's mode row** reads `Efficient · this project` or `Balanced · machine`. SOTA stays red (plan 19 L4).
- **The space list does not change.**

## 7. Tests

Every test uses `crate::nirvana::with_test_home(name)` and temporary project folders. None reads or writes the
owner's home, `~/.claude`, `~/.gemini` or a real project.

- `project_mode`: no file gives the machine's mode and `Machine`; a file gives its mode and `Project`; a file with
  another key, or a bad value, is refused with the path in the message.
- **Two projects, one machine table,** each with its own file, give two different modes from `table_for`. This is
  the owner's case in L1.
- `--mode` beats the project's file in `herdr seat pick`'s request.
- The worker supervisor's pick uses the project's mode when the order names none.
- `orchestrator_seat` through `table_for`: Efficient in one project gives Sonnet medium while SOTA in another gives
  Opus high.
- `herdr seat mode <m>` writes only the project's file and leaves the global line's bytes unchanged; `--machine`
  writes only the global line; `--clear` removes the project's file.
- The restart notice for project A does not fire when project B's mode changes.
- The Seats square's mode row shows the source word.
- Each assertion has a sabotage control in `nirvana/controls.d/project_mode.py`, and every older control that
  targets a line this plan changes is updated and still reddens its test.

## 8. Live check, for the owner

1. Press the reload badge twice.
2. In the WorkOutAppRefactor space, run `herdr seat mode efficient`. In the 3D-Printer-Flow space, run `herdr seat
   mode sota`.
3. Each space's Seats square shows its own mode. Restart Claude in each: WorkOutAppRefactor's Nirvana Claude starts on
   Sonnet medium, 3D-Printer-Flow's on Opus high.

## 9. Not in this plan

- A per-project seat table (turning a seat off for one project, other prices).
- Syncing the mode through git. The file lives in `.nirvana/`, which is local.
