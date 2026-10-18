# Plan 12 O4b record: the Nirvana Claude plugin's hooks, worker agents, skills and statusline; the launch keeps Claude Code's own prompt

Builder "Opus 5.5 high - O4b Nirvana Claude plugin build", 2026-10-18, branch `o4b-plugin`. The brief said to branch
from master at alpha 0.15.0 (14a71d9a); the branch sits on `o3-runner` (O3 rebased onto that same master, with its
merge commit 94701741), because the hooks call O3's journal (`check`, `replay`, `live`, the runs folder) and O3's
`record_once`, which master did not have. Not merged; the version is the orchestrator's to bump.

## Ledger for this package

| # | Verbatim quote | Surface | Verifier |
|---|---|---|---|
| L5 | "Claude will be the orchistrator model alwys when you right click on a folder or start a Nirvana herder session  you will create a Nirvana version of Claude with all the tool / skills ready to work task in the manner we are describing." | the plugin's skills, worker agents, hooks, statusline; `ROLE.md` | `/context` on the new launch lists `nirvana:nirvana`, the five `nirvana:worker-*` agents and five `nirvana:*` skills; the SessionStart hook ran (its dump file and the Messages row) |
| L6 | "The reason we are building this as a seperate version which tbh is proably just Claude Code with these Orchistrator level skills and memories and hooks is to differentiate between the two." | the launch keeps Claude Code's own system prompt and appends the role | `plain_claude_code_and_agy_carry_no_nirvana_flag_and_no_nirvana_env`; `/context` system prompt: plain 2.5k, O4a's launch 1.5k, this launch 4k |
| L10 | "Cosnider Jev and its capbilities in this process as well." | the `jev` skill; `herdr jev compact --plugin` on Stop (§9 roles 1 and 4) | the skill in `/context`; `every_hook_names_a_herdr_subcommand_that_runs_quiet_and_fast` |
| L11 | "One other thing to consider is a form of run receipt Nirvana had this for checking how the agents ran and how it can improve. This was sort of baked into that memmory feature we were talking about." | the digest names the open retro proposals and `NIRVANA.md`; SubagentStop records each worker subagent's line | `only_open_proposals_are_named`, `the_board_line_is_written_once_beside_the_lessons_lines`, `a_worker_subagents_outcome_is_recorded_under_its_seat_effort_and_class` |

## What was built

| Part | Where |
|---|---|
| The launch: `--agent nirvana --model claude-opus-5-5 --effort medium --append-system-prompt-file ROLE.md` | `src/nirvana_launch.rs` (and its test in `src/app/nirvana_new.rs`) |
| The role, appended to Claude Code's prompt; the agent file's body emptied | `nirvana/claude/ROLE.md`; `nirvana/claude/agents/nirvana.md` (front matter only) |
| Five worker subagents | `nirvana/claude/agents/worker-{opus-high,opus-medium,sonnet-medium,sonnet-low,haiku-low}.md` |
| Skills `seat`, `usage`, `handoff`, `jev` | `nirvana/claude/skills/*/SKILL.md` |
| Hooks | `nirvana/claude/hooks/hooks.json` |
| `herdr nirvana digest|guard-spawn|subagent-stop|stop|statusline` | `src/nirvana_claude.rs`, `src/nirvana_claude/tests.rs`; the arm in `nirvana_launch::cli` |
| `--plugin` on `herdr board notice` and `herdr jev compact` | `src/nirvana_board.rs`, `src/nirvana_jev.rs`, `src/nirvana_jev_compact.rs` (`hook` became `hook_input`) |
| The installed statusline | `nirvana/windows_shell.py` `statusline`, `install_plugin` |
| The hooks file test, on the built binary | `tests/nirvana_plugin_hooks.rs` |
| Controls | `nirvana/controls.d/o4b.py` (56) |

## Measured (2026-10-18, this PC, Claude Code 2.1.302)

All `/context` runs: `claude -p /context --output-format json` from Python with a list argv, in the scratch repo
`target/o4b-live/proj`, `BROWSER` a `.cmd` that opens nothing, 0 turns, $0.

- **System prompt**: plain Claude Code 2.5k; O4a's `--agent nirvana` 1.5k; with no `--agent` and the role appended
  3.9k; `--agent nirvana` with an empty-bodied agent file 2.5k, and with the role appended 4k (the shipped launch;
  ROLE.md grew by two paragraphs between the runs). Custom agents 423 to 957 (`nirvana:nirvana` 79, the five workers
  105 to 110 each). Skills 9.9k to 10k (the four new ones and media, 80 to 100 each).
- `--append-system-prompt-file` is not in `claude --help`'s list but is named under `--bare`; a missing file exits 1
  with "Append system prompt file not found". `--effort bogus` warns "Unknown --effort value 'bogus' — ignoring it";
  `--effort medium` printed nothing on stderr. The effort a session runs at is not shown by `/context`, the init
  message or the SessionStart input (its fields: session_id, transcript_path, cwd, hook_event_name, source,
  session_title), so medium is the flag's, not observed.
- **The digest**: printed 10 times by a scratch SessionStart hook against 0, `/context`'s Messages row 1.6k to 4.5k:
  the 498-character digest is 290 tokens, +-10. A 753-character first draft was 380 and a 567-character one 310, so
  `/context` counts this digest at 1.8 to 2 characters a token (plain prose 2.9); the scratch paths are about 40
  characters longer than an install's. 23 to 43 ms a run, once 466 to 709 ms just after the exe was copied.
  The shipped digest, live:

  ```text
  Nirvana 0.15.0; herdr is "C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-o4b\target\o4b-live\home\bin\herdr.exe".
  Pools: Claude 5h 87% (33m), week 52% (5d10h); Gemini 5h 0% (4h59m), week 0.8% (6d10h).
  Not in the foot, with headroom: Argon week 7%, G·Claude week 4%.
  Board rev 1: plan.
  Runs open: live2: slow was interrupted: the orchestrator cont…
  Retro proposals open: p-0a1b2c3d (stall on flash low).
  Memory: C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-o4b\target\o4b-live\home\nirvana\NIRVANA.md.
  ```
  (usage.json copied from the owner's home; live1 and live2 are O3's live journals, copied; the proposal and board
  are scratch.)
- **Which herdr the Bash tool runs**: a haiku-low probe (`--model claude-haiku-5-5 --effort low`, $0.012 and
  $0.007) ran `command -v herdr`: `/c/Users/fahim/.herdr/packages/standalone/releases/0.9.5-x86_64-pc-windows-msvc/herdr`,
  and `herdr nirvana statusline` answered "unknown command: nirvana". So a plain `herdr` in Nirvana Claude's shell
  is upstream herdr, and O4a's agent text ("`herdr seat pick`") would have run it. A `bin/herdr` shim in the plugin
  (Claude Code puts a plugin's `bin/` on the Bash tool's PATH) did not come first and was removed. herdr is named by
  path instead: the digest's first line, `${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe` in the skills and worker agents.
- **The guard, live in Claude Code**: the probe's `general-purpose` spawn with no model or effort came back
  "PreToolUse:Agent hook error: herdr guard: this spawn of general-purpose names no model and no effort...", and its
  `nirvana:worker-haiku-low` spawn ran (Claude Code 2.1.302 ran it as a background agent). The matcher `Agent`
  fires although the init message's tool list names `Task`.
- **SubagentStop, live**: the first probe's record line had `tokens: null`; the subagent's transcript held its response
  a moment after the hook read it. With the 3 s wait the second line was
  `{"order":"o4b-probe-2","run":"o4b-live","seat":"haiku","model":"claude-haiku-5-5","effort":"low","class":"chore.script","chain":"subagent","outcome":"subagent-done","tokens":{"input":2,"cache_write":1136,"cache_read":5391,"output":5},...}`.
- **Stop, CLI**: a dead-end copy of live1 (its run-level `ended` removed): first stop `{"decision":"block",...}`, the
  same state again nothing, `stop_hook_active` with nothing raised before a `systemMessage`; 16 to 29 ms.
- **Statusline, CLI** on the documented JSON: COLUMNS 160 "Claude week 41% 3d0h · Gemini week 0.8% 6d10h · orders:
  slow haiku interrupted"; 60 "Claude week 41% · Gemini week 0.8% · 1 order"; 30 "Claude week 41% · Gemini …".
- **Guard, CLI**, worker-opus-high on review.adversary: allowed at the statusline's 23.5%; at 84.9% refused, "would
  leave max 5h at 14.8%, under your reserve of 15% (84.9% used, about 0.3% for this spawn)".

## Choices where the plan was silent

1. **An empty-bodied `--agent nirvana` plus `--append-system-prompt-file`.** The first build dropped `--agent`
   (3.9k, the same parity); O3's merge commit 94701741 then made the wake type only into a pane whose `claude` argv
   has `--agent nirvana` and `--plugin-dir <home>/nirvana/claude`, the one thing that tells Nirvana Claude from plain
   Claude Code. Claude Code leaves the system prompt unchanged for an agent with no prompt, so the agent file keeps
   only its front matter: the wake guard, L5's "lists the nirvana agent" and parity all hold, and O3's file is not
   touched. The agent stays spawnable as a subagent, so `guard-spawn` refuses `nirvana` and `nirvana:nirvana`.
2. **Model and effort twice**, in the agent's front matter and as `--model` and `--effort`: the brief named the flags,
   and the front matter is what a spawn of the file would read.
3. **Exec-form hooks on `${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe`.** The install's layout puts the binary there;
   the form spawns it with no shell. Windows only, as the install is.
4. **The statusline is written by install** into the installed `settings.json`, with the installed exe's path in
   forward slashes and quotes for Git Bash, because neither a plugin's settings nor `${CLAUDE_PLUGIN_ROOT}` reach a
   `statusLine`. `refreshInterval` 30 s, because event-driven runs go quiet while the orchestrator waits.
5. **The digest reads caches only** (`usage.json`, the statusline's file, the journals), never a reader: a usage read
   takes 4 to 20 s and a SessionStart hook delays the session.
6. **The digest's budget is 540 characters**, filled by priority: herdr's path, the pools and the memory line always;
   then the runs (dead ends first), the proposals, the board, the hidden pools, each in a short form when the full
   one does not fit. The board comes in full with the first prompt from `board notice`, so its line is short.
7. **"Headroom" for a hidden pool**: every window every seat spends is read, not past a reset, and above
   `seats.toml`'s floor.
8. **Codex, local and other pools in a word** are left out of the digest; the foot shows them.
9. **NIRVANA.md's board line** is one line per project with its own marker, `<!-- herdr-board <root> -->`, replaced in
   place like O5's lessons line; a new file gets a short head. The digest writes it, only when it changes.
10. **Open proposals** are read in O5's form (`status: open`, the gap after the first line's ": ") by a copy of O5's
    `nirvana_retro::open`, so this builds before O5 merges. Exact hunk in the report.
11. **The guard's expected use** is the seat's use cell at the effort (the engine's `use_cell` rule), times
    `[size.small]`, times the brief's `Class:` weight when it names one; no price scale (the Claude seats have one
    price). An unknown or stale window lets the spawn through, as the seat engine's reserve filter does. The guard
    books no lease: SubagentStop names the agent by id, PreToolUse by tool-use id, and nothing joins the two.
12. **A fork is refused**: it always runs on the orchestrator's seat.
13. **A subagent's record**: class, order and run from its brief's first lines (`Class:`, `Order:`, `Run:`; the seat
    skill and the agents ask for them), order `sub-<agent id>` without one, and no line at all without a class.
    Outcome `subagent-<status>`, chain `subagent`: the orchestrator, not the gate, judged it, and O2's tally would
    count it as a failure on a class chain.
14. **Stop raises a dead end once per (run, last event)**: a block when the hook is not already continuing, a
    `systemMessage` when it is, then nothing until the run moves. Remembered in `<home>/nirvana/stop-raised.json`.
15. **`--plugin` on `board notice` and `jev compact`**: the folder's own settings (local, shared, the repository
    root's, the user's) are checked for the same command; when they run it, the plugin's copy is quiet.
16. **`herdr run tick` is not in a hook.** It ends in `wake`, which may type into the orchestrator's own pane; called
    from that pane's own hook it could type into itself. The digest and the Stop hook name a due deferral through
    O3's `check` text ("due, `herdr run tick` starts it").
17. **Worker tools**: Read, Grep, Glob, Bash, Edit, Write for all five; no Agent (no nested spawns), no web tools.

## Controls

52 in `nirvana/controls.d/o4b.py`, each run alone with `controls.py --only <exact name>`; results in the package
report.
