# Audit, 2026-10-29: xtool (Sonnet high, read only)

**Report: xTool Studio GUI runs.** I read 38 of the 53 worker streams in these folders; the other 15 are Gemini streams that carry no tool-call structure. I read no MCP tool results, and read none of the screenshots themselves.

## Top 3 improvements

1. **Show the real run order of layers before the owner presses Process.**
   - On 2026-10-27 the `hers_TR_v2` dial cut its outline and window first, before any gold went on. The owner called it "a failure" and the orchestrator told them to stop the job.
   - Three workers had written that Studio's "Settings overview" order is not the run order, and that `canvas_list`'s `layers[].order` is. Nobody checked this against Studio. Evidence: `f2u-studio-load-3/REPORT.md:43-55`, `f2u-strip-ladder` and `f2u-dial-final` reports.
   - The orchestrator later admitted "I haven't confirmed that Studio processes in that order" (orchestrator session `f7dce3de` line 13692). The screenshot at line 13765 showed Studio grouping the cut layers first.
   - Neither P29b nor 0.43.0 covers this. `state.py` only passes `layers[].order` through.
   - Gain: it prevents the one failure that cost titanium and an owner stop. Size: small to medium. Hypothesis: read the order from Studio's processing preview, or add a "layers sharing a group" warning to `ui_state`.

2. **Make `file_open` + `material_set` + `params_set` + save + readback one call, and fix the sticky defaults.**
   - Studio resets density to 300 on every `file_open`. That is in 8 reports, and every loader worker then redoes `params_set`.
   - Five `f2u-studio-load` workers ran the same recipe: Opus medium, 1–3 min, $0.46–1.11 each, $3.99 total.
   - There are 12 hand-written `make_steps.py` and 12 `compare.py` copies, plus 20 hand-written `harness.py` copies (15 distinct) across the reports folders.
   - P29b's `xtool_run` replaces `harness.py`. It does not replace the legend-to-steps builder or the compare script.
   - Gain: removes roughly 10–12 tool calls per load and a class of parse errors. Size: medium. Add `xtool_apply_legend(xs, legend)` with the `compare` step built in, and have `file_open` return the density it found.

3. **Give `3D-Printer-Flow-xtool` a `.mcp.json` and a real `needs: xtool` path.**
   - The folder has only `.mcp.json.example`. All 38 Claude worker `init` lines show `mcp_servers: []`. There were 0 `mcp__xtool` calls anywhere, orchestrator included.
   - The 0.43.0 per-order `--mcp-config` reads `<root>/.mcp.json` (`src/nirvana_gui_tools.rs:121`). In this folder it finds nothing, so a `needs: xtool` order cannot get the server there.
   - The only `xtool` entry I found is in `3D-Printer Flow/.mcp.json`. Per plan §1 it points at the old master copy (not checked here).
   - Gain: every P29b feature stays unused until this is fixed. Size: small, a config file in the xtool folder and a repoint to the P29b worktree.

## Per-run table

I took wall time from `seated` to `ended` and cost from the stream's `result` event. GUI share is the harness-run Bash calls (or `xtool-mcp run`) as a share of all tool calls.

| Run | Workers (seat) | Wall | Cost | Outcome |
|---|---|---|---|---|
| `f2u-setup` xTool part | 19 Claude workers: 7 Opus high, 5 Opus medium (all studio-load), 5 Haiku low, 2 Sonnet medium; 5 Gemini research workers | 78 min for the 19 Claude workers | $17.96 | all delivered; 6 `needs_answer` |
| `xtool-mcp` | 2 (Sonnet medium 8 min; Flash 25 min) | 33 min | $0.75 | delivered |
| `errands` | 1 Sonnet low | 0 min | $0.13 | not xTool |
| `ti-build` (xcs) | 6 (3 Flash, 3 Sonnet) | 1–13 min each | $1.49 (Sonnet only) | built the CDP params tools |
| `ti-cal` (xcs) | 3 (2 Flash, 1 Sonnet) | 1–11 min each | $0.36 | research only |
| `gift-box` | 2 Sonnet medium, 2 and 19 min | 21 min | $1.97 | Bambu, out of scope |

- **Tokens:** I did not total them. Cost per worker is in `rows.json`.
- **GUI share:** 74 of 488 Bash and tool calls across the 19 Claude workers (15%). The rest is building `.xs` files, validating, reading docs, and parsing logs.
- **No `3D-Printer Flow` run touched xTool.** One call in `jev-images-research2` matched my xTool pattern and I did not check it. The 2026-10-16 session `a6648c9a` (68 xtool-related calls) predates these runs and was the original MCP build.

## Tool-call mix, xTool workers

| Call type | Count | Evidence |
|---|---|---|
| xtool MCP-protocol calls | 0 | no `mcp__xtool` call in any session |
| Hand-rolled harness driving the server's tools over stdio | 74 Bash calls, with 166 `params_set`, 50 `file_open`, 28 `file_save`, 38 `ui_screenshot`, 32 `camera_snapshot` | `reports/*/run*.log` |
| Build and check scripts (Bash) | 146 | `an2.py` |
| Other Bash | 134 | |
| pixel-level driving (`pywinauto`, `pyautogui`, `SendInput`, `SetForegroundWindow`) | 0 | |
| CDP `Runtime.evaluate` direct | 5 | |
| Image reads in the 19 workers | 39, about 10.1M base64 characters | |
| Orchestrator image reads since 2026-10-23 | 36 `Read` calls, plus 2 full-screen PowerShell grabs | session `f7dce3de` |

xTool Studio is driven over its debugging port (CDP), not by clicking pixels. So "wrong window" and "coordinates" never occurred. The failures are about state and what the tools can read.

## Failure modes

| # | Failure | Count | Evidence | Already fixed? |
|---|---|---|---|---|
| F1 | Layer run order assumed, then wrong | 1 job (owner stop), 3 reports asserting it | `f7dce3de` lines 13670–13838; `f2u-studio-load-3/REPORT.md:55` | No. New. |
| F2 | Studio resets density to 300 on `file_open` | 8 reports | `f2u-colour-v5`, `f2u-dial-final`, `f2u-studio-load` and others | No. P29b reads back but does not prevent. |
| F3 | Tab strip `modified: true` after a good save | 10 reports | `f2u-studio-load-2/REPORT.md` and others | Partly. P29b reports it as a warning (`file.py save_readback`). |
| F4 | Stale or blank camera background in Studio | 10 reports | `f2u-studio-load` and `-2`, `f2u-hers-gold24` | No. New. |
| F5 | `params_set` could not set defocus | 3 reports; workers used the `.xs` API | `f2u-studio-load-2` and `-3` gotchas | P29b adds it, but see the conflict below. |
| F6 | Wrong geometry that readback cannot see: `add_display("PATH")` stretches each mark to 30.5 mm, and `params_get` and `canvas_list` passed. Only a screenshot caught it. | 1, caught pre-burn | `f2u-hers-tr` gotchas | No. New. |
| F7 | Plumbing around harness output: 20 tracebacks in 21 streams, about 12 of them JSON-in-log, `/tmp` path, cp1252, missing PIL, `__file__` | about 12 of 20 | `an3.py` output; `f2u-studio-load` line 54 | Mostly: `xtool_run` and the CLI return structured reports. |
| F8 | `file_open` on a new path while another tab is active loads it in the background; a second call is needed | 50 opens vs 28 saves; reported in `f2u-studio-load-2` | gotcha text | Partly: P29b's open readback fails loudly when the active tab is wrong. |
| F9 | `device_status` lid_closed false while the owner says the lid is shut | 3 reports (`f2u-calib-prep`, `f2u-dial-place-2`, `xtool-status-fix-2`) | `xtool-status-fix-2` delivered text | Unresolved. P29b found the recorded payloads read right and made no change. |
| F10 | `camera_snapshot` refused while the laser sleeps | 1 | `f2u-dial-place` | No. Hypothesis: auto-wake is an owner action; ask first. |
| F11 | Orchestrator had no way to read job state, so it relied on owner screenshots and "check now" prompts | 7 owner images and 5 "check now / progress?" prompts | `own1.txt` | No. New. |
| F12 | Studio Auto mode: "Closing the enclosure will start the automated process" with a test grid loaded | 1 near-miss | `f7dce3de` line 7870 | No. Hypothesis: add the Auto flag to `ui_state` (I did not find an `auto` field in `state.py`). |
| F13 | Tool gaps found live: no thickness tool (built as `material_set/get` after the owner asked); no read of the auto-measure result | 2 | `f7dce3de` lines 7632, 7694, 7855 | Thickness fixed. Measure readout still not covered. |

**Conflict inside P29b.** It accepts `defocus_distance` only from 1 to 12 mm (README and `params.py`). The titanium recipes use +0.5 mm (`f2u-colour-v5`, `f2u-studio-load-2`). Either the notes are on a different scale or the range is wrong. P29b's own record lists the keys and range as "unverified until the live check", so treat the range as a hypothesis.

## What I checked in the brief

- **A worker attempting start/frame/send:** 0 in 488 worker tool calls; 0 `job_start`/`job_frame`/`job_resume` in the orchestrator's commands after 2026-10-23. Both runs that went wrong were the owner pressing Process and Framing in Studio.
- **"Bambu and xTool MCP-protocol calls = 4" (plan §1):** for xTool the figure is 0 in these folders.
- **0.43.0 `mcp-config`:** works only where a `.mcp.json` exists (improvement 3).
- **P29b fixes:** batch, `ui_state`, crops, readbacks, defocus keys. It does not touch `xsfile.py`, density-on-open, or the camera background.

## Ranked improvement list

| Rank | Change | Evidence count | Expected gain | Size |
|---|---|---|---|---|
| 1 | Verify and report the real layer run order; refuse or warn when cut runs before colour | 1 failed job, 3 wrong claims | stops wasted titanium | S–M |
| 2 | `xtool_apply_legend` (load, thickness, params, save, compare) and a density-on-open fix | 8 reports, 12+12+20 script copies, 5 loader workers | −10 calls per load; one place for the compare logic | M |
| 3 | `.mcp.json` in the xtool folder, repointed to the P29b branch | 38 of 38 workers had no servers | enables everything P29b built | S |
| 4 | Per-display size check against the source path (`ui_state` or `xsfile`), because readback missed the stretch | 1 | catches silent geometry errors before the owner sees the burn | S |
| 5 | Camera-freshness flag in `ui_state` | 10 reports | stops placement checks on a stale background | S |
| 6 | Resolve the defocus scale (1–12 vs +0.5) in the live check before trusting `params_set` | 3 reports | avoids refused or wrong values | S |
| 7 | Job state and Auto-mode flag readable by the orchestrator | 5 prompts, 7 images, 1 near-miss | fewer owner screenshots | M |
| 8 (hypothesis) | Studio-load orders on Sonnet instead of Opus medium | 5 workers, $3.99 | roughly 50% of that spend; untested | S |

## What I could not measure

- **Time per Studio action.** Streams carry no per-call timing, so I have no seconds figure.
- **Token cost of the images.** I have the 39 image count and base64 size, not tokens. The plan's 1.76k median may or may not apply.
- **The 15 Gemini worker streams.** They have no parsable tool calls, so their screenshot and tool mix is unknown.
- **Orchestrator transcripts other than `f7dce3de`** (the large session). Four others in the xtool folder were shorter worker sessions.
- **Whether F9 is a firmware reading or a tool bug.** Only a live check settles it.
- **Whether the cut-first order came from Studio's auto planning or from layer data.** The orchestrator guessed "Auto planning" without a measurement.

Scratch scripts are in `C:\Users\fahim\AppData\Local\Temp\claude\C--Users-fahim-Desktop-Pojects-Nirvana\audit-xtool\` (`an.py`, `an2.py`, `an3.py`, `orch.py`, `win.py`, `rows.json`, `own1.txt`). I edited nothing, called no MCP tool, and looked at no screenshot.
