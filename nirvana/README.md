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

Explorer's "Open Nirvana - CC" (Nirvana-C) runs the installed fork in the session `nirvana`, inside its
`nirvana-shell` supervisor. By hand: `herdr.exe --session nirvana nirvana-shell`.

## Features

| # | Feature | Plan |
|---|---|---|
| 1 | Sidebar header "◆ Nirvana alpha X.Y.Z", window title "<workspace> · Nirvana", reload onto a newer installed build | plans/2026-10-15-01-header-and-reload.md |
| 2 | Tab right-click "Restart Claude": the tab's Claude Code starts again in the same pane and folder, resuming the same conversation | plans/2026-10-15-02-restart-claude.md |
| 3 | Each space lists its agents: the task's summarized title, then the agent's icon (Claude ✻, Codex ❖, Antigravity ▲, Local ⌂) and its provider and model | plans/2026-10-15-03-space-agents.md |

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
