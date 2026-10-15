# Feature 1: the Nirvana header, its version, and reload onto the newest build

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Lets fork and modify Herder to function similarly to the design objective of Nirvana." | GitHub fork fahim-mygithub/nirvana-herdr, local clone Desktop/Pojects/Nirvana-Herdr, upstream remote herdrdev/herdr | orchestrator | `git remote -v` shows origin = the fork, upstream = herdrdev/herdr |
| L2 | "We will go one feature at a time." | this plan holds one feature; nothing else lands with it | orchestrator | the diff touches only the files in the table below |
| L3 | "For our first implementation similar to Nirvana hot realoding the newest changes and keeping track of the project have this element for the Herdr instance [Image #11]" | the sidebar's top row: mark, "Nirvana", a version badge "alpha X.Y.Z", the sidebar toggle at the right; the window title "<workspace> · Nirvana"; a Reload when a newer build is installed than the one running | orchestrator | unit tests on the drawn buffer; a live run in Windows Terminal read by a leaf agent from a screenshot; a reload run from 0.1.0 to 0.1.1 |

Image #11, as the owner pasted it: the window title "Downloads · Nirvana"; under it one row with Nirvana's lotus mark in
teal, "Nirvana" in bold, a rounded badge "alpha 0.51.0", and a sidebar-collapse icon at the right edge.

"Hot reloading the newest changes" is read from Nirvana's own behaviour (Nirvana CHANGELOG "Version 0.1.0 (alpha)",
`ui/components/shell.js`): one version number bumped per merge, drawn beside the name, and a Reload drawn when the
running server moved past what the window shows. "Keeping track of the project" is read as both the version that
tracks the project's progress and the window title that names the project in view. Both readings ship; the owner
corrects either.

## Shape

- **Version.** `nirvana/VERSION` holds the fork's own number, alpha 0.1.0, separate from herdr's Cargo version
  (herdr's version gates its update manifest and protocol checks; changing it would break both). `build.rs` exports it
  as `NIRVANA_VERSION`. `nirvana/version.py bump feature|fix` moves it, once per merge (Nirvana CLAUDE.md rule 16).
- **Header.** The workspace section's first row becomes the brand row: `◆ Nirvana  alpha 0.1.0` and the toggle `«`
  at its right. The " spaces" label moves to the second row. The collapsed sidebar shows `◆` on its first row.
  Drawn in the client (TUI presentation, AGENTS.md runtime/client boundary).
- **Window title.** The default `ui.window_title` becomes `{workspace} · Nirvana`.
- **Reload.** `nirvana/install.py` builds the release binary and installs it to
  `%LOCALAPPDATA%\Programs\Nirvana-Herdr\bin\herdr.exe` with a stamp `nirvana-version.txt` beside it; a running
  exe is renamed aside first, which Windows allows. The server's background version check, which upstream points
  at herdr's release manifest, reads that stamp instead; a newer stamp sets the snapshot's existing
  `update_available`. The header then draws `↻ alpha 0.1.1` as a button. A click stops the server gracefully
  (it saves the session), starts the installed build, and reattaches. Panes come back from the saved session.
- **Never pull upstream builds.** The fork's `herdr update` and the background check never fetch herdr's releases:
  they would replace the fork with upstream. Upstream changes come in by `git merge upstream/master`.
- **Its own session.** The launcher runs the fork with `HERDR_SESSION=nirvana`, so it never meets the upstream
  herdr server already on this machine (default session).

## Files

| File | Change |
|---|---|
| nirvana/VERSION, nirvana/version.py, nirvana/install.py, nirvana/README.md, nirvana/CHANGELOG.md | new |
| build.rs, src/build_info.rs | export and read NIRVANA_VERSION |
| src/client/shell/sidebar.rs, src/client/shell/endpoint_sidebar.rs, src/client/shell/state.rs | brand row, toggle, reload hit |
| src/config/window_title.rs | default title |
| src/update.rs | the fork's check reads the local stamp; self-update refuses |
| Nirvana-C nirvana_c/herdr.py | run the fork's exe with HERDR_SESSION=nirvana |

## Record (2026-10-15)

- **Built** on Windows 11 with Rust 1.96.1 (rust-toolchain.toml) and Zig 0.16.0 (scoop). Release build 1 min.
- **Suite:** `cargo nextest run --locked`: 3493 run, 3492 passed, 1 failed. The failure,
  `app::api::plugins::tests::windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override`, fails the same
  way on unmodified upstream b431a063 on this machine. `cargo clippy --all-targets --locked -- -D warnings` clean,
  `cargo fmt --check` clean, `scripts/config_reference_check.py` exit 0, the UI hot-path, config-reference and
  changelog script tests OK (53).
- **Fork tests:** 8 Rust (`cargo nextest run nirvana`), 4 Python (`python -m unittest nirvana.test_scripts`).
  **Controls:** 10 of 10 redden their test (`python nirvana/controls.py`).
- **Live, scratch home, session `nirvana-test`:** a leaf agent read the window: tab and title
  "nirvana-herdr-test · Nirvana"; first row "◆ Nirvana  alpha 0.1.0 «" with a teal ◆, bold "Nirvana", the version
  on a shaded badge; second row "spaces"; the workspace below.
- **Live reload:** a 0.1.1 build installed beside the running 0.1.0. The server logged "nirvana: a newer build is
  installed version=0.1.1" 3 s later; the badge read "↻ 0.1.1" (teal). First press: "restart?" (red). Second press:
  the server logged "server shutdown initiated reason=server.stop request", "session saved", then a new server
  "restored session already has workspaces"; the same supervisor (pid 21024) started a new client and server from
  the new binary; the row read "alpha 0.1.1". The tab stayed open.
- **Found in the live run and fixed:** the badge lost its closing pad column and touched the toggle. The badge now
  runs up to the toggle; `brand_row_names_nirvana_and_its_running_version` asserts the pad, control
  `header_badge_clipped` reddens it.
- **Not measured:** whether a Claude Code conversation in a pane survives a reload. It does not survive as a live
  process (the server stops; Windows has no live handoff). herdr can resume Claude in the restored pane only with its
  Claude integration installed (`herdr integration install claude`, hooks in ~/.claude), which this feature does not
  install. herdr's own "update ready" toast did not appear in the live run; the badge carried the news alone.
- **Through the verb (Nirvana-C 0.5.0):** the registry command on `nirvana-verb-test` exited 0 in 5.2 s; the fork's
  server runs as `herdr.exe --session nirvana server`; the window is `herdr.exe --session nirvana nirvana-shell` in
  Windows Terminal; `agent list` shows Claude idle in w1:p1 with `--settings {"theme":"custom:nirvana-dark"}`.
