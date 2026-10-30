# Feature 31: the printer and laser tools in real use

Status: **P31a BUILT (2026-10-30)** as Nirvana-Herdr alpha 0.44.0. P31b and P31c are built and handed to the owner,
not merged (§6). Three packages in three worktrees, built in parallel. The evidence is the audit of
2026-10-29, `nirvana/plans/2026-10-29-31-records/audit-bambu.md` and `audit-xtool.md`.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Audit the Nirvan runs is there any improvements I can make? I wwant to mainly improve computer use of xtool and bambu slicer" | all | orchestrator | the audit records |
| L2 | "These all look like good improvements." (answer to the ranked list 1 to 7, the two Nirvana bugs, and the Send rule) | P31a, P31b, P31c | builders | each section's tests |
| L3 | The Send rule, proposed by the orchestrator and taken as approved by L2: a Send is allowed only when the order quotes the owner's approval, and no slicer warning is left unexplained. | P31a §2, P31b §3 | P31a, P31b | §2 and §3 tests |
| L4 | "The ear hook keeps failing something is causing that support to fail and stinging near the ear hook." (run `dune-v5b`, after a print sent with an unread warning) | P31b §3 | P31b | §3 tests |
| L5 | "What's progress it didn't print" and "whats the progresss can we print?" (deliveries and a go-ahead that never arrived) | P31a §3, §4 | P31a | §3, §4 tests |

- Not in scope: agy GUI orders through `herdr job run`. The `gui-bench` run (n=1, flash2 read argon's scripts first)
  is not enough to drop 0.43.0's claude-only rule. It is a live check for the owner (§7).
- The `.mcp.json` for `3D-Printer-Flow-xtool` is the owner's file. The orchestrator hands over the lines (§6); no
  builder writes it.

## 1. What the audit measured

- 0 MCP calls in 22 Bambu GUI orders and 38 xTool workers. Every Studio action was a worker-written script.
  - Bambu: all 22 orders had `needs: []`, so 0.43.0 gave no worker a server.
  - xTool: the folder has no `.mcp.json`; every worker `init` listed `mcp_servers: []`.
- Bambu, 13 GUI workers: 327 GUI shell calls, median 8.1 s, 11 shell timeouts (1,837 s, 28% of GUI time), 210
  screenshot reads. P29a breaks on the beta: its allowlist has "Slice plate"; the beta button is "Slice all"
  (4 workers).
- A print was sent with an unread warning, "1 overhang region could not get support" (L4), then took 3 more rounds.
- xTool: the `hers_TR_v2` dial cut before its gold; three workers assumed Studio's settings list is the run order.
  Studio resets density to 300 on every `file_open` (8 reports).
- Four deliveries reached Nirvana Claude only when the owner typed: about 13.5 h of wait in all. A go-ahead was sent
  to a finished worker's pane and sat 68 minutes.

## 2. P31a, Nirvana-Herdr: route GUI orders to their servers, and the Send rule

**Worktree** `C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-p31`, branch `p31-gui-in-use`, from master 69a4e01a.

1. **`needs` for GUI orders.**
   - Nirvana Claude's guidance (`NIRVANA.md` or `ROLE.md`, whichever holds the seat-run rules) says: an order that
     drives a desktop app names its `.mcp.json` server in `needs`, for example `needs: bambu-studio-beta`.
   - `herdr seat run` also checks: when the project's `.mcp.json` defines a desktop server and the order's text names
     its app (Bambu Studio, xTool Studio) while `needs` names no server, it prints one warning line naming the
     server to add. It does not refuse.
   - Measure the check on the 22 recorded orders in `3D-Printer Flow\.nirvana\runs` from 2026-10-26 on, and on
     the research and review orders of those runs: how many it flags, and how many of those were GUI orders.
2. **The Send rule.**
   - `herdr seat run --send-ok "<the owner's words>"` records the quote on the order.
   - herdr checks the quote appears verbatim in one of the owner's own prompts in Nirvana Claude's transcript
     (plan 25's `owner_prompts` filter, so `<task-notification>` and `!` lines do not count). A quote it cannot
     find refuses the order, with the reason.
   - An order with a verified quote gets, in place of the device line: "You may press Send once, as the last step,
     only if the slice shows no warning you cannot explain. Otherwise stop and report the warning." Every other
     server order keeps the device line.
   - The quote and the check's result go into the journal's `seated` event.
3. **Late deliveries.** Find the root cause first (the superpowers systematic-debugging skill). Evidence: four
   `ended` to `gated` gaps after 04:07Z on 2026-10-29 (`hook-fix` 20,275 s, `hook-fix2` 1,506 s, `hook-send`
   25,798 s, `fit-v6` 1,132 s) in `3D-Printer Flow\.nirvana\runs\dune-v5b` and `dune-v6`, where the notice came only
   through the `UserPromptSubmit` hook. Seven earlier deliveries arrived in seconds. Suspects to rule in or out with
   evidence: plan 25's fresh start and plan 27's idle compact sharing the idle stretch with the wake; the wake's
   own rules; the daemon version running then. Fix the cause, with a test that fails before the fix.
4. **A message to a finished worker.** When Nirvana Claude sends text to a pane whose worker has ended, herdr says
   so and names the order, instead of typing into it. Find the path the orchestrator used (`dune-slice-v5`, its
   transcript lines 4283 and 4312) and guard that path.
5. **Owed from P29c.**
   - A test that a send-back child's brief and a reseat's continuation brief keep the device line, and the Send
     line when the parent had one.
   - `p24_send_back_inherits_override` cannot go red: its test reads the machine's live seat state. Make the test
     use the stand-in world (`set_test_world`) so the sabotage reddens it.

## 3. P31b, Bambu: P29a made to work on the beta Studio

**Worktree** `C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow-gui29`, branch `feat/gui-tools-29`, on top of 1f84fc4.
No builder starts Bambu Studio. Every layout fix is built from what the workers recorded: their reports, gotchas,
UIA dumps and screenshots under `3D-Printer Flow\.nirvana\runs\dune-*` and the workers' `reports` folders.

1. **The beta's controls.** "Slice all" in the click allowlist beside "Slice plate"; the filament-slot pane at its
   recorded width (346 px, not 136); the settings-tab row tables for the recorded layouts (the Quality and Others
   tabs `dune-hook-fix2` could not read); the file dialog found without UIA where the workers say UIA misses it.
2. **Long waits return at once.** `slice` and dialog waits start the action and return; the next call reads progress
   from `beta_state`. No call blocks past 60 s.
3. **Slice warnings.** The slice result and `beta_state` carry a `warnings` list, read from the slice's notice area.
   The brief's Send line (P31a §2) depends on it. A send step in a batch refuses when `warnings` is not empty,
   naming them.
4. **Nozzle grouping.** A `beta_regroup` step for the recipe the workers found (Print dialog, "Regroup and slice",
   Custom, drag the chip). It opens the Print dialog, so it must close that dialog without pressing Send; the guard
   is a test.
5. **Owner files.** Batches refuse Ctrl+Alt+S and Ctrl+N, and refuse to open a file while the owner's project has
   unsaved changes.
6. **Fallbacks the workers found.** A dropdown pick that does not apply retries with Down and Enter; a slice click
   that does nothing retries with Ctrl+R after clicking the canvas.
7. Tests as P29a's: fixture layouts from the recordings. Controls in P29a's `controls_p29.py`.

## 4. P31c, xTool: the layer order, and one call to load a job

**Worktree** `C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow-xtool29`, branch `feat/xtool-tools-29`, on top of 2e6d4c3.
No builder starts xTool Studio or the laser.

1. **The layer run order.** Find from evidence how Studio orders processing: its installed app files (read only),
   the `.xs` format, and the `hers_TR_v2` job's files and screenshots. `ui_state` and a new read report the order
   the job will run in, and warn when a cut layer runs before a fill or engrave layer that shares its area. Any part
   of the order not proven from evidence is reported as `unverified`, never as fact.
2. **`xtool_apply_legend`.** One call: open the file, set thickness and material, set each layer's params from a
   legend, save, read back, and compare. It returns the compare table. It replaces the 12 `make_steps.py`, 12
   `compare.py` and 20 `harness.py` copies the workers wrote.
3. **Density on open.** `file_open` returns the density it found; `xtool_apply_legend` sets it after the open.
4. **Camera freshness.** `ui_state` says whether the canvas background is fresh, stale or blank, from evidence of
   how the workers told (10 reports).
5. **Display size check.** A display whose size on the canvas differs from its source path by more than a
   tolerance is a warning (the `add_display("PATH")` stretch to 30.5 mm that readback missed).
6. **Defocus scale.** P29b accepts 1 to 12 mm; the titanium recipes use +0.5. Resolve from Studio's own files
   which is right, and fix the range or the notes. If evidence cannot settle it, the range stays and the record says
   so.

## 5. Gates

- **P31a:** plan 27's gates, nextest with a short private TEMP. Controls in `nirvana/controls.d/gui_in_use.py`,
  named `p31_*`.
- **P31b and P31c:** each package's own suite.
  - Bambu: `BS_EXE_PATH=C:/nonexistent/none.exe uv run --extra dev pytest -q -p no:cacheprovider
    --ignore=tests/test_e2e -m "not e2e"` only. Its e2e fixture kills every `bambu-studio.exe`.
  - Every new assertion has a sabotage control in the package's `controls_p29.py`.
- One Sonnet review per package.

## 6. Hand-over

- P31a merges to Nirvana-Herdr master with a version bump.
- P31b and P31c are not merged by the orchestrator. They go to the owner with the P29a and P29b branches they sit
  on, and the `.mcp.json` lines for `3D-Printer Flow` and for `3D-Printer-Flow-xtool`.

## 7. Live checks for the owner

- A Bambu order with `needs: bambu-studio-beta`: its `init` lists the server, it calls `mcp__bambu-studio-beta__*`
  tools, and a slice with a warning stops before Send.
- An xTool load through `xtool_apply_legend`, and the run-order read checked against what Studio does.
- A delivery arriving while the owner is away, within a minute.
- Hypothesis, not built: an agy Flash order driving Studio through `herdr job run`, three runs, on a fresh task.
