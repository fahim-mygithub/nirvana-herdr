# Feature 25: a leaner Nirvana Claude, borrowed from pi

Status: **PLANNED (2026-10-22)**. Build after plan 24. §3's probe runs first and decides its cuts.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Sure lets fix that anything from pi-agent we should consider?" | all | orchestrator | — |
| L2 | The owner picked all four of: "Fresh session + handoff" ("Nirvana Claude starts fresh at each run from a short note built from the journal. Largest expected saving; I measure it first."), "Lean startup context" ("Measure Nirvana Claude's first-turn tokens and cut what orchestration doesn't use."), "Results while busy" ("A finished worker's result is queued into Nirvana Claude as a prompt, not left waiting for idle."), and "Browser batches" ("Browser workers are told to use bladebro run batches so only the result reaches the model."). | §2 to §5 | orchestrator | each section's tests; §6 |

- After measuring, the orchestrator recommended dropping "Results while busy" and shrinking "Browser batches" to one
  brief line (§1). The owner has not answered that recommendation. Until they do, §4 builds nothing; it is not
  dropped from the ledger.
- L2's fresh session does not replace plan 12 R3, "Resume last session". R3 decides what a window reopen continues.
  §2 decides when a long session is cleared during its life. A reopen still resumes the last session, which is then the
  fresh one.

## 1. Measured 2026-10-22

The measurements come from both Nirvana Claude transcripts, WorkOutAppRefactor `b4c82fab` and 3D-Printer-Flow-xtool
`f7dce3de`, and the run journals of both projects. Turns are de-duplicated by `message.id`. The scripts are in the
session scratchpad, `pi-measure/`.

| | WorkOut | xtool |
|---|---|---|
| Turns | 297 | 482 |
| Context per turn: first / median / max | 60.7k / 170.7k / 428.5k | 71.0k / 176.7k / 422.5k |
| Cache-read total | 58.9M | 91.9M |
| `/compact` events | 1 | 5 |
| Runs inside the session | 5 | 2 |

- **A fresh session per run, estimated.** Each run would start at its first-turn size plus a 2k note, then grow as it
  really did:
  - WorkOut would have read 29.0M fewer cached tokens, 49% of its 58.9M.
  - xtool would have read 2.7M fewer, 3%, because about 400 of its turns came before any run.
  - The saving sits where a run began deep in a long session: `resume-session` began at 260k, `list-mode` at 157k.
  - The estimate ignores two things: the cost of writing the cache again, about 26k to 37k per fresh start, and
    compactions that would have fallen anyway.
- **What the first turn is made of,** WorkOut, at 60.7k measured. These are char/4 estimates:
  - 20 built-in tool schemas, about 23.4k. Of these, the `Artifact` tool alone is about 9.4k, and PowerShell,
    SendFeedback, Workflow, ScheduleWakeup, AskUserQuestion, ReportFindings and ReadNotifications together are
    about 8.5k.
  - The skill listing, 63 skills, about 7.0k.
  - The system prompt, about 4.6k.
  - The deferred tool names, 143 of them, about 3.5k.
  - MCP instructions, about 2.3k.
  - The agent listing, about 1.7k.
  - The SessionStart hooks, about 2k. `superpowers` is enabled twice, and its "using-superpowers" hook text is about 1.1k.
- **Results while busy.** The journals hold 24 `woken` events: 23 typed as a prompt within 2 s, and 1 queued as a
  notice for 521 s. In that one case Nirvana Claude was working, and `wake_decision` (`src/nirvana_run.rs`) queues
  while it works by design.
- **Browser batches.** No worker stream in either project called bladebro or agent-browser. The fork's worker briefs
  never mention a browser.

## 2. A fresh session after a run ends

- **When.** The Stop hook runs at the end of a turn. It checks two things:
  - the run this session last ended (`herdr run end`) ended since the session's last clear;
  - the session's context is above `fresh_above`, a constant in `seats.toml` set to 2 times the measured first turn,
    120k to start.
  When both hold, it records a pending fresh start for the pane.
- **How.**
  - herdr types `/clear` into Nirvana Claude's pane, then the handoff note as the next prompt.
  - It uses the wake's own rules (`wake_decision`): only into Nirvana Claude, only while it is idle, only when the
    prompt line is empty, and only after the owner has been quiet for `OWNER_QUIET_S`.
  - The process, its flags, its seat and its pane stay; only the conversation restarts. No relaunch is needed.
- **The handoff note,** at most 2,000 tokens, comes from files, not from the model:
  - the ended run's receipt line and outcome;
  - every run still open, with its orders and their phases;
  - the board's open items;
  - the owner's last 3 prompts, verbatim, from the transcript;
  - the project's `.nirvana/lessons` index line.
  It ends: "This is a fresh session after run <id>. Ask the owner before assuming anything the note does not say."
- **`herdr nirvana fresh`** does the same on demand, for the owner.
- **The journal** gets a `fresh` event with the context size before the clear and the note's size.
- **Why the end of a run and not its start:** a run is opened in the middle of Nirvana Claude's turn, and the
  conversation that planned it is still needed. Clearing at the end makes the next run start fresh.

Tests:
- The trigger fires only after a run end, and only above `fresh_above`.
- The note keeps within its budget, and holds the open runs and the last prompts.
- Nothing is typed when the pane is busy, when the owner typed in the last `OWNER_QUIET_S`, or when the prompt line is
  not empty.

Live check: the context-per-turn table for the next week's runs, measured by the same script.

## 3. A lean start

1. **The probe first.** One `claude -p "ok" --output-format json` run per variant, with Nirvana Claude's own
   argv from `launch_args`, Haiku, `BROWSER` a no-op, in a scratch folder. The first turn's input plus cache tokens
   is the reading. Variants:
   - as today;
   - `--disallowedTools` for each of `Artifact`, `SendFeedback`, `Workflow`, `ScheduleWakeup`, `ReportFindings` and
     `ReadNotifications`, one at a time, then all together;
   - the folder's generated settings with the second `superpowers` plugin disabled;
   - skill overrides that drop skills orchestration never uses.
   Expected: each cut lowers the reading by about its §1 estimate. A cut that lowers it by under 500 tokens is not
   taken.
2. **The cuts that pass** go into `launch_args`, and into the generated settings file that plan 13's preset writes per
   folder.
3. **Nothing Nirvana Claude uses is cut.** It needs Bash, Read, Edit, Write, Grep, Glob, the Agent tool for the
   guarded spawn, `AskUserQuestion`, and the herdr skills. The probe also checks that one turn can still run
   `herdr seat pick --help`.

Tests: `launch_args` carries the chosen `--disallowedTools` list; a test names each tool Nirvana Claude needs and
fails if any is in the list.

## 4. Results while busy

Not built unless the owner asks again after reading §1. If they do, the design is to type a worker's result while
Nirvana Claude is working, which Claude Code queues as the next prompt. That rests on Claude Code's queueing of
typed input, which is not yet measured.

## 5. Browser batches

- The worker brief for any order whose class or `needs` name a browser gets one line: "Use bladebro `run` to batch
  steps; read the result, not each page."
- Test: the brief for a browser order holds the line; a build order's brief does not.

## 6. Controls and gates

As plan 24 §7. The controls go in `nirvana/controls.d/lean.py`, named `p25_*`.
