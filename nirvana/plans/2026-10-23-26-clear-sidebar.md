# Feature 26: a clearer sidebar, and finished work leaves the screen

Status: **PLANNED (2026-10-23)**, to build on agy (Gemini 3.8 Flash high) at master alpha 0.37.0.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Can we furthure improve the carity o the spaces and agents side bar." | §4, §5 | orchestrator | §4 and §5 tests |
| L2 | "THis to me also seems odd when I think the job was done or handded of but the window is not cleared" | §2, §3 | orchestrator | §2 and §3 tests |
| L3 | "Close done, keep problems (Recommended)", whose text was: "A pane closes as soon as its worker delivers done. A blocked, stopped or failed worker keeps its pane until it's gated, because you may want to read why. Every closed pane's view stays in `herdr seat read <order>`." | §2 | orchestrator | §2 tests |
| L4 | "Close it (Recommended)", whose text was: "When `herdr run end` finishes, its tab closes. The receipt and journal stay on disk, and the board still lists the run." | §3 | orchestrator | §3 tests |
| L5 | "Who, model, state (Recommended)", whose text was: "One row per agent: a role name you can read, the short model, and a state word. The chat title moves to a hover line. Idle agy panes fold into one row." Its preview: `✻ Nirvana   S5.5 working`, `✻ Claude    S5.5 idle`, `▲ 2 agy panes  idle`. | §4 | orchestrator | §4 tests |
| L6 | "By run, live first (Recommended)", whose text was: "Workers are grouped under their run. Live ones come first with elapsed time. Finished ones show a mark and a word: ✓ done, ✗ blocked, ■ stopped. A finished row leaves once its order is gated." | §5 | orchestrator | §5 tests |

## 1. Measured 2026-10-23, on master at alpha 0.37.0

The owner's two screenshots, and a read of the code. The builder re-measures every claim before relying on it.

- **A worker's pane closes only at its gate or merge.**
  - `cleanup_cli` in `src/nirvana_worker.rs` closes the pane, then removes the worktree and releases the booking.
  - It is called from `src/nirvana_gate.rs` after a `Gated` event and after a merge, and by hand as
    `herdr seat cleanup`.
  - An order delivered but never gated keeps its pane with an idle shell. In the screenshot's `run studio-gui` tab, a
    done, a blocked and a stopped worker each kept a pane, and a fourth was still running in the next tab.
- **A run's tab never closes.** Five `run …` tabs were open, four of them for runs that had ended. `open_pane` finds
  the run's tab from where its last worker pane still is, and opens a new tab when there is none.
- **The sidebar is 26 cells wide** (`sidebar_width: 26`, `src/config/model.rs`).
- **A space's agent row is `<icon> <chat title> - <model>`** (`render_space_agents`,
  `src/client/shell/nirvana_spaces.rs`).
  - At 26 cells the title is cut to four cells: "Nirv… - Sonnet 5.5", "No t… - Sonnet 5.5".
  - The title is Claude Code's terminal title (`task_title`, `src/nirvana_spaces.rs`).
  - The server sends a kind (`claude`, `codex`, `antigravity`, `local`) and a model label as agent tokens
    (`KIND_TOKEN`, `MODEL_TOKEN`). Nothing marks which Claude is the space's Nirvana Claude.
  - Idle agy panes each take a row ("▲ No task yet - gem…" twice in MotoProject), and the space's fourth agent
    becomes "+1 more".
- **The agents panel lists workers as subagent rows** under the Nirvana Claude agent row (plan 17,
  `src/client/shell/agent_sidebar.rs`, `read_seat_orders` in `src/nirvana_subagents.rs`).
  - The rows are flat, not grouped by run.
  - Below 34 cells the panel is "narrow" and drops the version and the elapsed time; names and models are cut
    ("studio-gui-mcp-v2 Sonn").
  - Done rows already show ✓ and failed rows ✗. A finished row stays until its run is gone.

## 2. A worker that delivers done closes its pane

- When `seat exec` ends an attempt with the Work Report status `done`, and the order's phase is `Delivered`, it closes
  its own pane through herdr, as the last thing it does.
- **Kept panes:**
  - A worker that delivers `blocked`, `failed` or anything other than `done` keeps its pane.
  - So does a stopped or interrupted worker.
  - These panes still close at the gate or merge, as now.
- **The tab's last pane stays** while the run is open, so the run keeps its tab and the next worker can reuse that
  pane (plan 0.35.1's placement). It closes with the tab at `herdr run end` (§3).
- **Nothing is lost.** `herdr seat read <order>` must still show the full view after the pane is gone. Re-measure
  where it reads from (the stream file, not the pane), and test it.
- The worktree and the booking are untouched here; they still go at the gate or merge.
- Outside herdr (`seat run` detached), there is no pane, and nothing changes.

## 3. `herdr run end` closes the run's tab

- After `run end` writes the run's end, it closes the run's tab.
  - The tab is the one holding the run's worker panes, found the same way `open_pane` finds it.
  - It closes the whole tab only if every pane in it is one of this run's worker panes, by the pane ids in the
    journal. Otherwise it closes only this run's panes and leaves the tab, since the owner put something there.
  - It closes nothing when the run's tab cannot be found, and says so in one line.
- `run end` on a run that already ended closes its tab the same way, so an older run's tab can be cleared by
  ending it again.
- **New `herdr run tidy`:** closes the tabs of every ended run in this project, by the same rule, and prints what it
  closed. It is never run automatically.
- The receipt, the journal and the board are unchanged.

## 4. A space's agents read as who, model, state

- **One row per agent:** `<icon> <who> <model> <state>`, fitted to the sidebar's width.
  - **who:**
    - `Nirvana` for the space's Nirvana Claude;
    - `Claude`, `Codex`, `agy` and `Local` for the others, by kind.
    - The server decides which Claude is Nirvana Claude and sends it as a new agent token, `nirvana_role`. Find
      the most reliable sign, and report which you chose. Candidates:
      - the pane the space launch opened for Nirvana Claude;
      - a command line naming the Nirvana plugin folder;
      - `NIRVANA_ORCHESTRATOR=1` in its environment.
  - **model:** the short form, such as `S5.5`, `O5.5`, `H5.5`, `Flash`, `Pro`. Reuse `short_model` in
    `src/nirvana_subagents.rs` if it fits; otherwise add one function both use.
  - **state:** `working`, `idle`, `blocked`, `done`, from the agent's status, in the status colour.
  - The icon keeps its shape per kind and its status colour.
  - Columns line up within a space: `who` is padded to the longest `who` in that space.
- **Fold idle agy panes.** Two or more agy agents in one space that are idle with no task fold into one row, as in
  `▲ 2 agy panes  idle`. The same fold applies to any kind; the label is `<n> <who> panes`.
- **The chat title moves to a hover line.**
  - When the pointer rests on an agent row, that row shows the chat title in place of model and state, cut to fit.
  - First re-measure whether the client gets mouse-move events over the sidebar (`src/client/shell/mouse.rs`,
    `link_hover.rs`).
  - If it does not, show the title instead on the row of the agent in the focused pane. Report which one you built.
  - `No task yet` is never shown on a row.
- The `+N more` row counts rows after folding.
- The space name and branch lines are unchanged.

## 5. The agents panel groups workers by run, live first

- Under the Nirvana Claude agent row, workers are grouped under a run header, the run's name.
  - The run with a live worker comes first, then the others, newest first.
  - In a run, live workers come first, then finished ones.
- **A live worker** takes two lines: its order name, then the short model and the elapsed time, so nothing is cut at
  26 cells.
- **A finished worker** takes one line: a mark, then its name, cut to fit.
  - `✓` for done, `✗` for blocked or failed, `■` for stopped or interrupted.
  - The mark carries the meaning; the colour repeats it.
- **A finished row leaves once its order is gated, merged or abandoned.** A run with no rows left drops its header.
- The panel's `+N more` counts rows after grouping.

## 6. Controls and gates

- Every assertion in §2 to §5 has a sabotage control in `nirvana/controls.d/clear_sidebar.py`, named `p26_*`, on a
  product line, naming the test that goes red.
- Every older control whose target line changes is updated and still reddens its test. The sidebar and subagent
  panel tests from 0.3.0, 0.33.2 and plan 17 will change; say which, and why each change keeps their intent.
- Tests use `crate::nirvana::with_test_home(name)` and temporary projects only. Rendering tests draw into a buffer
  at 26 cells and assert the rows.

## 7. Live check, for the owner

1. On a run, a worker that delivers done closes its pane. A blocked one keeps its pane.
2. `herdr run end` closes the run's tab. `herdr run tidy` clears the older run tabs.
3. The spaces list reads `✻ Nirvana  S5.5 working`, and idle agy panes fold into one row.
4. The agents panel groups workers by run, live first.

## 8. Not in this plan

- The sidebar's width stays 26 cells.
- The layout of the spaces header, the usage foot and the squares.
