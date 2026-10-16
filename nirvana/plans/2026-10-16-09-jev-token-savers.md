# Feature 9: Jev token savers, for every agent in a pane, switched in Preload

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "I would like to actually implement these token saving jev efatures for real. I know we only have them shadow tested and never ran Nirvana but lets actually put them to use in herder." | `herdr jev file`, `herdr jev files`, `herdr jev text` (levels 8, 9 and 10's triage) and `herdr jev compact` (level 7), switched on, not shadow | orchestrator | unit tests with a mock Jev; live calls on the owner's key |
| L2 | "Ad a section in the preload so we can toggle this" | Preload's first group, "Jev token savers (herdr)": Jev guide, Compact advice, today | orchestrator | shell test of the group and its switches |
| L3 | "I imagine every agent needs some sort of preload file to know how to save token with Jev." | the guide (`herdr jev guide`): a Claude `SessionStart` hook; an `AGENTS.md` block for Codex and agy | orchestrator | each agent proven to load it |
| L4 | "Lets try to utilize it in Nirvana herdr ensure its compatible in claude code, agy and codex." | the tools are this binary's CLI, run from any agent's shell; the caller is named in the log | orchestrator | Claude and agy ran them live; Codex loaded the guide |
| L5 | "THen perform a real test uou dont need to hold back on using real open router tokens jev is virtually free." | an A/B: the same task with the guide off and on, per agent, real models, real Jev | orchestrator | `nirvana/evidence/2026-10-16-jev-ab/summarize.py` |

Background: the owner asked first which of Nirvana's Jev features "ended up being useful". None had a real-use saving:
12 were built, one ever called Jev in the owner's use (the output screen, 7 shadow calls, 0 flags), and the two that
target tokens (cc-compact, jev-files) were never installed (leaf "Opus - map Nirvana jev usefulness", events.sqlite read
only, 2026-09-16 to 2026-10-16).

## What reaches each agent (measured 2026-10-16, leaf "Sonnet - agent instruction surfaces")

- Claude Code 2.1.301: a `SessionStart` hook's output reaches the model; Claude never loads `AGENTS.md`.
- agy 1.4.0: loads `AGENTS.md` and `GEMINI.md` from the root, walking up from its folder.
- Codex 0.42.0: loads only `AGENTS.md` (root and walk-down); no override file, no import, no hooks. Its sign-in is dead
  on this PC ("401 Unauthorized"), so no Codex model ran; its session record still lists the instructions it loaded.

## Shape

- **The tools** live in this binary, so any agent with a shell runs them and the agent's own permission rules hold:
  this binary never runs a command for an agent (`text` reads a pipe the agent's own shell made).
- **What is never sent** is Nirvana's list, in code before any call: outside the project, `..`, build and dependency
  folders, git-ignored files, binaries, over 256 KB, secret names and secret-looking text, and the live key. Output
  piped to `text` goes with secret shapes redacted.
- **A failure is never a yes**: no key, timeout, HTTP error or a reply that breaks the answer contract prints `failed`.
- **Level 7** is a Claude Code `Stop` hook: under 150k tokens nothing; above, four Jev questions, and a
  `systemMessage` line to the owner at a clean break. The lines are Nirvana's hypotheses from the owner's transcripts.
- **The log** is `<home>/jev/calls.jsonl` and `<home>/jev/compact.jsonl`: what was judged, the answer, the cost, the
  caller, never the text. Preload's "today" row and `herdr jev stats` read it.

## Files

- `src/nirvana_jev.rs` (new): client, answer contract, guards, the tools, the log, the guide, the CLI.
- `src/nirvana_jev_compact.rs` (new): level 7.
- `src/nirvana_jev_install.rs` (new): the hooks and the `AGENTS.md` block.
- `src/nirvana_preload.rs`, `src/client/shell/nirvana_mods.rs`: the Jev group and its switches.
- `src/main.rs`, `Cargo.toml` (ureq with rustls: the one HTTPS POST).
- tests beside each; `nirvana/controls.py`; `nirvana/CHANGELOG.md`; `nirvana/evidence/2026-10-16-jev-ab/`.

## Record

Built 2026-10-16 on alpha 0.8.0 (b0802388) as alpha 0.9.0. Figures: `nirvana/evidence/2026-10-16-jev-ab/summary.txt`,
recomputed by `summarize.py` from the result files beside it; the harness is `ab.py` (real models, the owner's key).

1. **Live Jev** (`probe_size.py`): right on one file from 8 KB to 300 KB; 300 KB in 998 ms for $0.0033.
2. **The A/B**: 46 runs, 46 right. With the guide on, the agents ran Jev by themselves. Tokens moved little on the 71 KB
   project. On the 568 KB project, finding the bug: agy -7 to -9% and about 17 s slower; Claude +5% with the first
   wording, -16% with the second (6 runs each side, wide spread). The Claude runs paid the guide twice (no CLAUDE.md in
   the test project), which the build now prevents.
3. **Level 7 live** (`compact_live.py`, lines lowered): two defects found and fixed (the transcript lagging the reply;
   the three-turn rest); afterwards the clean break got its line, for $0.00003.
4. **Codex**: its session record held the guide; no Codex model ran, its sign-in is dead on this PC.
5. **In a herdr window**: the Preload group drew first; clicks switched both hooks and wrote an `AGENTS.md` git did not
   see; a second click took them away; `r` exposed the restart defect above, fixed; Claude in the pane then used Jev
   unprompted and answered right (17 calls, $0.0062).
6. 22 Jev controls and 1 restart control redden their tests; the suite passes but the one upstream test that failed
   before.

**Not measured.** Codex using Jev (sign in again: `codex login`, and update from 0.42.0). A local model. Compact advice
reaching the owner in the interactive screen (its line was seen in the hook's log, not on screen). Level 7 at the real
150k/300k/500k lines. Whether Jev saves tokens over long sessions, where a file not read is a file not paid for on
every later turn: the A/B's tasks were one question each.
