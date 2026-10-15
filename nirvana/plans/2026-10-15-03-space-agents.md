# Feature 3: each space lists its agents' task and model, with the agent's icon

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "See how in our old Nirvana we showcase the task a summerised title and its active model in Nirvana case its called agent but for herder it would be the model provider and tyle ie Claude Sonnet.[Image #16] Basically I would to have spaces do somethign similar and on the left Instead of a green circle its the agents Icon. We need one for claude antigravity codex and local." | under each space in the sidebar's spaces list, two rows per agent running in that space: the task's summarized title, then the agent's icon and its provider and model ("Claude Sonnet 5.5") | orchestrator | unit tests on the model readers, the labels, the icons and the rows; controls that redden each; a live run with a screenshot of a space holding a Claude after a prompt |

Image #16, as the owner pasted it: the old Nirvana window. Left panel: the space "Downloads", a section "Answered" with
the task "Greeting, no task yet", and under it a row "Agent" with a small green circle on its left. The main panel holds
the conversation ("hi", then the Agent's answer).

## Shape

- **Rows.** Under a space's own row(s), for each agent in that space, in the agents panel's order:
  1. the task: the agent's terminal title with herdr's activity glyph stripped. Claude Code sets that title to a short
     summary of the conversation. A title that only names the program ("Claude Code", a shell path) reads "No task yet",
     the old Nirvana's words.
  2. the agent: its icon, then provider and model, e.g. "Claude Sonnet 5.5".
- **The icon replaces the circle.** The circle was herdr's status dot. The icon keeps the dot's colour, so idle is still
  green, working yellow, blocked red, done teal; its shape says which agent. One cell wide each:
  Claude ✻ (Claude Code's own glyph), Codex ❖, Antigravity ▲, Local ⌂. Any other agent keeps herdr's dot.
- **Local** is Claude Code driven against a local model through Ollama's Anthropic endpoint, the way Nirvana's local seat
  runs (memory nirvana-local-seat-for-chores): a Claude whose model is not an Anthropic model, e.g. `qwen3.6:35b-a3b`.
- **Where the model comes from**, read by the server, never by the client:
  - Claude: `<config>/sessions/<pid>.json` gives the session id and folder (feature 2's reader); the transcript
    `<config>/projects/<folder slug>/<id>.jsonl` names the model on each reply; the last one wins. Before the first reply:
    the `--model` argument, else just "Claude".
  - Codex: `--model`/`-m`, else the newest `~/.codex/sessions/**/rollout-*.jsonl` started in the pane's folder, else
    `model` in `~/.codex/config.toml`, else just "Codex".
  - Antigravity: `--model`, else the last "selected model override" line of `~/.gemini/antigravity-cli/cli.log`, else just
    "Antigravity". The log is shared by every agy, so two agy panes on different models both show the newest one.
- **Off the main loop.** The server's snapshot builder asks a cache; a stale entry (over 3 s) is refreshed by one worker
  thread, so no file or process read ever runs on the server's loop.
- **Wire.** Two entries in the agent's existing `tokens` list, `nirvana_model` and `nirvana_kind`. No new field, so an
  older server and a newer client still talk (the 0.1.0 server and 0.2.0 client of 2026-10-15 did not, for a command).
  An older server sends no tokens and the rows fall back to the agent's herdr name.

## Files

| File | Change |
|---|---|
| src/nirvana_spaces.rs (new) | kinds, icons, labels, the model readers, the cache and its worker, tests |
| src/main.rs | `mod nirvana_spaces;` |
| src/server/client_shell.rs | the two tokens on each snapshot agent |
| src/client/shell/nirvana_spaces.rs (new), src/client/shell.rs | the rows: lines from the snapshot, drawing them |
| src/client/shell/sidebar.rs | each space's height counts its agents' rows; the rows drawn under the space |
| src/client/shell/tests/nirvana_space_agents.rs (new), src/client/shell/tests/mod.rs | the rendering tests |
| nirvana/controls.py, nirvana/CHANGELOG.md, nirvana/VERSION, nirvana/README.md | controls, entry, 0.3.0, usage |

## Record

Built 2026-10-15 on alpha 0.2.0 (bc08f598), as alpha 0.3.0. The unfinished 0.2.1 "Reload first" notice, which the
owner stopped mid-step, is parked in `git stash` ("alpha 0.2.1 WIP") and is not in this build.

**Departures from the shape, found while building.**

1. *A space shows three agents, then "+N more".* Upstream's `sidebar_scrollbars_use_proportional_shared_geometry_and_drag`
   puts ten agents in one space; drawn in full, that space was twenty-two rows and pushed every other space out of the
   list. The agents panel below still lists them all. That upstream test now composes at 26 rows, not 20 (CHANGELOG).
2. *Codex is ❖, not ⬡.* The first live screenshot drew the hexagon correctly, but at sidebar size a leaf agent read it
   as the idle ring ○, the status mark it was meant to replace. `each_kind_has_its_own_one_cell_icon` now holds every
   icon apart from herdr's six status marks; control `spaces_icon_is_status_mark`.

**Live runs**, scratch home `scratchpad/nh-test3`, session `nirvana-s3`, folders `scratchpad/nirvana-spaces-test` and
`scratchpad/nirvana-spaces-local`. Four agents: Claude `--model sonnet` (prompted), Codex and Antigravity (left at their
trust prompts, never prompted), and a Claude `--model qwen3.6:35b-a3b` in a second space (left at its trust prompt).
Screenshots by leaf agents "Sonnet - read sidebar screenshot", "Sonnet - compare icon glyphs" and "Sonnet - read new
sidebar shot", files `scratchpad/f3/shot-1.png`, `shot-2.png` and their zooms.

1. First build. Claude's title went "Claude Code" to "PELICAN" after the prompt; its transcript, at the path the server
   builds from `sessions/<pid>.json` and the folder slug, names `claude-sonnet-5-5`. The sidebar read, under the space:
   `PELICAN` / `✻ Claude Sonnet 5.5`, `No task yet` / `▲ Gemini 3.8 Flash…`, `No task yet` / `(icon) Codex`; under the
   second space `No task yet` / `⌂ Local qwen3.6:35b…`. Icons green when idle, red for the blocked Local Claude. No glyph
   drew as a missing-character box. The Codex icon read as a ring (departure 2).
2. Second build, the session stopped and started again (it restored the first run's second space as an empty
   "local-seat"). Prompted "Reply with exactly the word HERON"; the sidebar read `◆ Nirvana alpha 0.3.0`, then under the
   space `HERON` / `✻ Claude Sonnet 5.5`, `No task yet` / `▲ Gemini 4 Argon (M…`, `No task yet` / `❖ Codex`, and under
   the second space `No task yet` / `⌂ Local qwen3.6:35b…`. The leaf agent: none of the four icons looks like the ring
   or the dot of the space rows. The Antigravity label changed between the runs because its shared log named a newer
   model switch, which is the read working, and the documented limit: every agy pane shows the newest switch.

**Not measured.** A Codex or Antigravity model read from a prompted session: both stayed at their trust prompts, so
Codex showed its fallback "Codex" and Antigravity showed the model its shared log last named. A `/model` switch inside a
running Claude was not run; the cache refreshes every 3 s and the transcript's last reply wins. Spaces on a remote
endpoint (`endpoint_sidebar.rs`) and the collapsed sidebar are unchanged.
