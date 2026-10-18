# Nirvana-Herdr

A fork of [herdr](https://github.com/herdrdev/herdr) that grows, one feature at a time, toward the design objective of
Nirvana: a multi-shell harness with Claude Code as its engine. The owner, 2026-10-15: "Lets fork and modify Herder to
function similarly to the design objective of Nirvana. We will go one feature at a time."

Everything the fork adds lives in this folder, `src/nirvana.rs`, `src/app/nirvana_*.rs`, and
`src/client/shell/nirvana_*.rs`. Upstream files
carry only small hooks, each marked `Nirvana fork:`, so `git merge upstream/master` stays easy. That is how the fork
gets herdr's own updates; the owner wants "a popular maintained harnes repo to get free updates from".

## Use

```
python nirvana/install.py          # build release and install to %LOCALAPPDATA%\Programs\Nirvana-Herdr\bin
python nirvana/version.py show     # alpha X.Y.Z
```

Explorer's "Open Nirvana" (installed by `install.py` since 0.15.0) runs `herdr --session nirvana nirvana window
<folder>` in a new Windows Terminal window: Nirvana Claude on the folder, with its own taskbar button, letter badge and
toast. By hand: `herdr.exe --session nirvana nirvana-shell`.

## Features

| # | Feature | Plan |
|---|---|---|
| 1 | Sidebar header "◆ Nirvana alpha X.Y.Z", window title "<workspace> · Nirvana", reload onto a newer installed build | plans/2026-10-15-01-header-and-reload.md |
| 2 | Tab right-click "Restart Claude": the tab's Claude Code starts again in the same pane and folder, resuming the same conversation | plans/2026-10-15-02-restart-claude.md |
| 3 | Each space lists its agents: the task's summarized title, then the agent's icon (Claude ✻, Codex ❖, Antigravity ▲, Local ⌂) and its provider and model | plans/2026-10-15-03-space-agents.md |
| 4 | The sidebar's foot: the servers running in each space (a click opens one), and this machine's usage, since 0.12.0 a row per pool: Claude, Gemini, Argon, G·Claude (Google's Claude and GPT pool), Codex, Local | plans/2026-10-15-04-servers-and-usage.md |
| 5 | A mod strip under the tab bar for a tab running Claude, with one mod, Preload: everything a new Claude in the folder preloads, with its tokens; switch an item off or on, open its file, restart Claude to take it | plans/2026-10-16-05-preload-mod.md |
| 6 | The strip's Running square: what Claude is doing now with a summary, and under it what it waits on you for, answered with a digit or a click | plans/2026-10-16-06-running-and-waiting.md |
| 12 (O1) | Usage readings: every pool and window with its reset, freshness and pace; `herdr usage [--json] [--refresh]` over `<home>/nirvana/usage.json` | plans/2026-10-17-12-nirvana-claude-orchestrator.md §15 |
| 12 (O2) | Seats: `<home>/nirvana/seats.toml` (seats, classes, bars, priors, prices); `herdr seat pick` seats an order by filters, bar and pool pressure with a reason line, books it (`--book`), takes three overrides a run; `herdr seat list`, `release`, `record` (`record.jsonl`, blend off) | plans/2026-10-17-12-nirvana-claude-orchestrator.md §16 |
| 12 (O6) | Media seats: `herdr media probe` (ffprobe, the 100 MB `view_file` limit), `herdr media shrink` (ffmpeg to at most 1280 px, 10 fps, mono audio, under the limit, never over the input), `herdr media prompt` (the full-path `view_file` line for argon or flash); the `media` skill in `nirvana/claude/skills/media/` | plans/2026-10-17-12-records/o6.md |
| 12 (O4a) | Nirvana Claude: the plugin (`nirvana/claude`) and its launch; the "new" menu (Nirvana, Claude Code, Antigravity, Shell); Explorer's "Open Nirvana" (`herdr nirvana window`), replacing Nirvana-C's and old Nirvana's verbs; `herdr.exe` carries the Nirvana mark; a Nirvana window gets its own taskbar button, the folder's letter as badge, and the toast "Nirvana · <folder>" | plans/2026-10-17-12-records/o4a.md |
| 12 (O3) | The runner: `herdr seat run <seat> --order <file>` opens a `<seat>·<order>` pane running the worker headless (claude and agy with plan 12 §4's launch lessons, a job object per worker), idempotent per order; `.nirvana/runs/<run>/journal.jsonl`; rate limits defer, re-seat or ask; stalls back off; `herdr run wake` types only into an idle Nirvana Claude, never while the owner types; `herdr run notice`, `check`, `resume`, `tick`, `log`; `herdr seat status`, `read`, `stop`, `cleanup`; `herdr run receipt` and `end` (receipt.md, receipt.json); the record line per order | plans/2026-10-17-12-records/o3.md |

## Rules for this fork

- **Never open an issue or pull request on herdrdev/herdr from here.** herdr's CONTRIBUTING.md closes unsolicited
  pull requests, and this fork is private work. The `upstream` remote's push URL is disabled on purpose.
- **Never let the fork update itself from herdr's releases.** `herdr update` and the background check are turned
  off in the fork (src/update.rs): they would install upstream herdr over it.
- **One version per merge.** `python nirvana/version.py bump feature|fix`, committed with the merge.
- **Never share a cargo target directory with another checkout of herdr.** On 2026-10-15 an upstream build in the
  fork's target directory left herdr's build-script output fresh without `NIRVANA_VERSION`, and the next fork build
  failed with "environment variable `NIRVANA_VERSION` not defined". `touch build.rs` recovers.
- **Checks:** `cargo nextest run --locked` (herdr's runner), `python -m unittest nirvana.test_scripts`,
  `cargo clippy --all-targets --locked -- -D warnings`, `cargo fmt --check`, and `python nirvana/controls.py`.
  One upstream test fails on this machine before any fork change:
  `app::api::plugins::tests::windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override`.
