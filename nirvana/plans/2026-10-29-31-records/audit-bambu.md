# Audit, 2026-10-29: bambu (Sonnet high, read only)

# Bambu Studio GUI audit, runs from 2026-10-26 on

## Top 3 improvements

1. **Make P29a work on the real beta Studio before it merges.** The beta's slice button is "Slice all", but P29a still hard-codes "Slice plate".
   - 4 workers hit the button-name mismatch: `dune-slice-v4`, `dune-print-v4`, `dune-slice-v5`, `dune-hook-fix2`.
   - P29a's `CLICK_ALLOWED` (`guards.py:81`) lists only "Slice plate". A batch click on "Slice all" would be refused.
   - The 136x30 filament-slot pane size (`gui.py:886`) and the fixed row-count tables (`gui.py:208`) are also unchanged.
   - Measured failures P29a leaves alone, across 13 workers: 5 "row count matches no known layout", 5 "only 0 filament slots", 7 "file dialog did not open".
   - Expected gain: the first `beta_run` step stops failing, and about 17 failed calls disappear.
   - Size: medium. It is edits to `gui.py` plus tests built from the recorded layouts.

2. **Make the orchestrator actually route GUI orders through the MCP path.** All 22 claude orders in scope have `needs: []`, and all 22 worker init lines show `mcp_servers: []`. So 0.43.0's per-order config never fired.
   - Nirvana Claude's seat picks use `--override sonnet` with the reason "GUI worker must see desktop window". The guidance I read (`NIRVANA.md`) never mentions `--needs bambu-studio-beta`.
   - The 0.43.0 brief line "never start a print or a job" is unconditional (`nirvana_gui_tools.rs:40`). Three orders in this window carried an owner-authorized Send, so the two will conflict once `needs` is set.
   - Expected gain: it is the precondition for any P29 gain. Size: small. It is one orchestrator rule plus a brief-line exception for orders that carry an authorized Send.

3. **Stop prints on an unresolved slicer warning.**
   - `dune-slice-v5` reported a new warning ("1 overhang region could not get support"). `dune-print-v5` sent the print anyway and wrote "I did not locate it". The gate accepted it.
   - The owner then wrote: "The ear hook keeps failing something is causing that support to fail and stinging near the ear hook."
   - That cost three more rounds: `dune-hook-fix`, `dune-hook-fix2`, `dune-hook-send`, 1,549 s of worker time, plus the failed print.
   - No tool reads the slice warnings; `beta_state` lists dialogs only. Adding a `warnings` field to the slice result, and a rule that a send order halts on any warning it cannot explain, closes this. Size: small to medium.

Outside the GUI, the biggest owner-visible cost is delivery latency. It is item 1 under "Other" below.

## Per-run table

Worker wall is journal `started` to `ended`. GUI share is time inside shell calls that drive Studio, taken from session transcripts. Those calls can overlap, so the share is approximate.

| Run | Worker (seat) | Wall s | GUI calls / GUI s (share) | Images | Outcome |
|---|---|---|---|---|---|
| dune-nose | nose-build (sonnet) | 1095 | 33 / 529 (48%) | 21 | delivered |
| dune-v2 | crysknife-slice (sonnet) | 2460 | 50 / 2069 (84%) | 37 | delivered |
| dune-v2 | dune-slice-v4 (sonnet) | 1222 | 36 / 920 (75%) | 17 | delivered |
| dune-print | print-v4 (sonnet) | 370 | 29 / 242 (66%) | 16 | blocked correctly: PLA grouped to Left/Ext |
| dune-print | print-v4b (sonnet) | 369 | 23 / 262 (71%) | 18 | sent, authorized |
| dune-v5 | slice-v5 (sonnet) | 468 | 26 / 348 (75%) | 21 | needs_answer, sent_back |
| dune-v5 | print-v5 (sonnet) | 351 | 23 / 254 (73%) | 15 | sent, authorized, with the warning |
| dune-v5b | hook-fix (sonnet) | 484 | 17 / 383 (79%) | 15 | blocked: the Type dropdown did not apply |
| dune-v5b | hook-fix2 (sonnet) | 706 | 26 / 555 (79%) | 15 | delivered, warning cleared |
| dune-v5b | hook-send (sonnet) | 359 | 19 / 270 (75%) | 15 | sent, authorized |
| dune-v6 | v6b-print (sonnet) | in flight | 6 / 220 | 2 | two `Studio().connect()` calls hung, 121 s and 91 s |
| jev-box | pilot3 (sonnet) | 790 | 27 / 534 (68%) | 12 | delivered, fails the bar |
| gui-bench | claude (sonnet) | 124 | 12 / 64 (52%) | 6 | delivered, 1 mis-click |

- The other dune workers (`dune-fit-*`, `dune-fix*`, `dune-hook-v2`, the reviewers) drive no GUI. `dune-face-research` and the other research workers sat on agy seats, which ran no Studio work.
- **Totals for the 13 GUI workers:**
  - 9,091 s of worker time and 648 tool calls.
  - 327 GUI shell calls, 50% of all calls, taking 6,651 s.
  - The median GUI call took 8.1 s and the p90 37 s. 43 calls took over 30 s and 21 over 60 s.
- **Usage:** the per-order 5h window shares for these Claude GUI orders sum to about 56 points. Their week shares sum to about 1.1 points.
- **Tool mix:**
  - Every Studio action in the 13 workers was a Bash/PowerShell process start. There were zero MCP calls.
  - 210 screenshot Reads. 189 of them (90%) came immediately after a GUI shell call, so the pattern is act, then look.
  - Image size: median 218 KB, 55 MB across 259 images (13 GUI workers plus 4 non-GUI workers). Token counts I could not measure.
  - `crysknife-slice` read `raw_now.png` 27 times. It is one reused filename, so the content differs each time.

## Failure modes (13 GUI workers)

| Mode | Count | Evidence | Already fixed? |
|---|---|---|---|
| Shell timeout, mostly slice or dialog waits | 11 calls, 1,837 s (28% of GUI time), 6 workers | `dune-slice-v4` 501 s: "Slice call timed out the shell at 8 min, but the slice had already finished". `crysknife-slice` 401 s. | No. `slice()` is unchanged. |
| File dialog "did not open (Ctrl+I)" | 7, 216 s, 6 workers | `dune-slice-v4` 106 s. The dialog is not listed by UIA. | Partly. P29a closes a stuck dialog after 20 s and fails with its title. It still requires UIA to see the dialog, which the workers say it cannot. That makes it a hypothesis that the load still refuses. |
| Row count "matches no known layout" | 5, 200 s | `dune-hook-fix2` could not read the Quality/Others tabs. | No. |
| Slice button is "Slice all", not "Slice plate" | 4 workers | `dune-slice-v4`, `dune-print-v4`, `dune-hook-fix2` gotchas. | No, new (see top item 1). |
| "only 0 filament slots" | 5 | The pane is 346 px wide, not 136 px. | No. |
| `set_edit_text` COMError | 3, 212 s | `dune-nose-build` 69 s, `crysknife-slice` 73 s. | No. |
| Wrong Python: no pywinauto | 3 workers | `dune-nose-build`, `dune-print-v5`, `jev-box-pilot3`. | The P29a batch CLI avoids it. |
| Dropdown pick not applied | 1 | `dune-hook-fix` reported "Type change never applied" and then `sent_back`. Keyboard Down+Enter worked later. | No. |
| Slice click a no-op | 1 | `dune-hook-fix2`: Ctrl+R after clicking the canvas sliced. | No. |
| Stale Preview screenshot | 1 | `dune-slice-v5` gotcha: "ghosted until you switch tabs". | Partly. P29a reports the tab as `unsure` and the screenshot hash. |
| Preview camera keys do nothing | 2 | `dune-hook-fix2`, `dune-hook-send` skipped the hook close-up. | No. |
| Ctrl+Alt+S silently saves a stray 3MF | 2 | `dune-slice-v4` wrote the gotcha, then `dune-print-v5` repeated it. | No. |
| Opened or replaced the owner's project | 3 | `crysknife-slice` (Ctrl+N), `dune-hook-fix` (replaced `jig_all_h2c`), `dune-slice-v5` (opened the AMS sync dialog by accident and Esc'd it). | No. |
| Nozzle grouping flips to Left/Ext for PLA | 4 worker rounds (`print-v4`, `print-v4b`, `slice-v5`, `print-v5`) | Recipe found: Print dialog, "Regroup and slice", Custom, drag the chip. | No tool covers it. |

**Send attempts:**
- Three sends happened (`print-v4b`, `print-v5`, `hook-send`). Each order carried the owner's verbatim authorization.
- `print-v4` stopped before Send because the grouping was wrong, which was correct.
- No unauthorized send occurred in this window.
- All three sends were raw pywinauto `mouse.click` in worker-written scripts (`hook-send` wrote `/tmp/w/send.py`). P29a's guards do not cover raw scripts.

**Wrong window and wrong state:**
- None found in this window.
- Plan 29's examples (Ctrl+S to Chrome, `active_tab` reading Device) did not recur in this window.
- The 18 UIA-read failures above are the only failures that look like stale reads.

**Success claimed and later disproved:**
- Only the `print-v5` case above. Workers were otherwise candid in their gotchas.

## What the owner's benchmarks found

**gui-bench**
- The task was a read-only Studio walk-through, run on four seats.
- Journal-wall time per seat:

| Seat | Wall | Result |
|---|---|---|
| claude sonnet | 124 s | 1 mis-click |
| agy flash (first order) | 211 s | blocked |
| agy flash2 | 172 s | 0 mis-clicks, GUI portion 31 s over 2 `herdr job run` calls |
| agy argon | 528 s | 3 mis-click bugs |

- The first agy order was blocked because agy's shell has no desktop (`exebox-*`). It could neither see Studio nor screenshot.
- Both later agy runs got Studio through `herdr job run --wait`, which runs on the Default desktop. So the plan 29 premise "agy cannot see the Studio window" holds only for the direct shell.
- `flash2` was not independent: it read argon's scripts and report first.
- Cost: claude used 1.42% of the 5h window and flash2 used 0.049%, about 29 times less.
- Argon used 33.3% of its week for this one order. I could not explain that figure.
- Hypothesis: allowing agy GUI orders through the bridge is worth a test. The 0.43.0 rule that skips agy for desktop servers is not.

**vision-bakeoff**
- It was abandoned after 158 s because the orchestrator misread the owner's request.
- The owner meant agy worker against Claude worker, not vision models. The gate reason reads "owner clarified: meant agy worker vs Claude worker speed, not vision models". The gui-bench run followed.

**jev-box (Jev 1.14 Box pilot)**
- Pilots 1 and 2 stalled 225 s hunting for an OpenRouter key (the second hit an auto-mode block), and then the pilot3 order ran.
- Pilot3 ran 39 calls. Results:
  - 3 of 5 targets hit, against a bar of 4.
  - Median 0.41 s capture-to-decision against about 5 s for `jev_click`.
  - Confidence did not separate hits from misses: 0.67 against 0.67.
- Verdict from its report: do not replace `jev_click`; use Box only on large cropped regions.

## Other: improvements outside Bambu GUI

1. **Delivery latency to the owner.**
   - Four deliveries after 04:07Z reached the orchestrator only when the owner typed. `ended` to `gated` took 20,275 s (`hook-fix`), 1,506 s (`hook-fix2`), 25,798 s (`hook-send`) and 1,132 s (`fit-v6`), about 13.5 hours in total.
   - Evidence: owner "What's progress it didn't print" at 12:20Z (transcript line 4977). The notice arrived via the `UserPromptSubmit` hook (line 4978).
   - Seven earlier deliveries came in as injected prompts in seconds.
   - The cause is unknown.
2. **Message to a finished worker.**
   - After `dune-slice-v5` finished, the orchestrator sent the owner's go-ahead to its pane (lines 4283 and 4312).
   - The worker never saw it. 4,111 s passed until the owner asked "whats the progresss can we print?" (line 4317).
3. **Helper scripts are rewritten every time.**
   - About 190 inline Python shell calls across the 13 workers.
   - `hook-send` wrote "No helpers existed in slice_v5b".
   - Workers also rediscovered the Python venv path.
   - Lessons did not propagate: the stray-save gotcha repeated.
4. **Screenshots.** P29a's crops and unchanged-hash skip target the 210 reads. The size of the gain is a hypothesis until measured live.

## Ranked improvements

| # | Improvement | Evidence | Gain | Size |
|---|---|---|---|---|
| 1 | Fix P29a for the real beta: "Slice all" and the allowlist, 346 px slot pane, row layouts, dialog detection without UIA | 4 workers, 17 failed calls | First batch works | Medium |
| 2 | Orchestrator sets `needs`; exempt authorized-send orders from the brief line | 22 of 22 orders with `needs: []` | Enables everything in P29 | Small |
| 3 | Read slicer warnings; halt a send on an unexplained one | 1 failed print, 3 rounds | Avoids a bad print | Small to medium |
| 4 | Slice and long waits as async: return, then poll `beta_state` or use `herdr job run` | 11 timeouts, 1,837 s | 28% of GUI time | Medium |
| 5 | Fix delivery wake and the message-to-finished-worker path | 4 of 4 late deliveries | Cuts owner wait | Small to medium |
| 6 | A `beta_regroup` step for nozzle grouping (it opens the Print dialog, so it needs a guard exception) | 4 worker rounds | Removes a known dead end | Medium |
| 7 | Test agy GUI orders through the bridge; keep argon out | flash2 0 mis-clicks, 29x cheaper (n=1) | Hypothesis | Small to test |
| 8 | Block Ctrl+Alt+S and Ctrl+N; refuse to replace an open owner project | 2 stray saves, 3 project replacements | Protects owner files | Small |
| 9 | Keyboard fallback for dropdown picks; Ctrl+R slice fallback; mouse-orbit camera step | 2 to 3 workers each | Fewer stalls | Small |

## What I could not measure

- **Image token counts.** I have bytes only. I used plan 29's 1.76k median as context, not as my number.
- **The `jig-*` runs.** I skipped them because plan 29 covered them.
- **agy worker streams.** They are not in the claude stream format. agy tool mix and times come from the workers' own reports, so they are self-reported.
- **Whether the daemon was 0.43.0.** The installed `herdr.exe` is dated 2026-10-27 06:08, two minutes after the merge, but the installed `VERSION` file was not at the path I checked.
- **Why the notices stopped.**
- **Argon's 33.3%.**
- **`dune-v6b-print`.** It was still running, so its outcome is open.
- **Parallel tool-call overlap** in the GUI-time totals.
- **Screenshots.** I viewed none.

Scratch files, including the analysis scripts, are in `C:\Users\fahim\AppData\Local\Temp\claude\C--Users-fahim-Desktop-Pojects-Nirvana\audit-bambu\`.
