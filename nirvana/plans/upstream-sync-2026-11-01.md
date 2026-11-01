# Upstream sync, 2026-11-01

Merge commit `ac2414ba` (parents master `5fd98b52`, alpha 0.51.1, and `upstream/master` `ee5bfb50`, herdrdev/herdr).
Merge base `a2008661`. No version bump and no `nirvana/CHANGELOG.md` entry here: the merge to master does that.

## The 13 upstream commits

- `ee5bfb50` chore: point agents at the source instead of agent-guide and llms.txt
- `bc8c2898` ci: close stale issues without outside engagement monthly
- `2713feb5` fix: install opencode and kilo integrations under XDG_CONFIG_HOME (#5585)
- `65da135f` fix: make the remote ready marker work under tcsh login shells (#5519)
- `de8503c1` fix: bound clipboard helpers so a hung wl-copy or wl-paste can't freeze herdr (#5583)
- `5608de06` test: wait for the hook to exit instead of a fixed timer in hook tests
- `b796ca60` chore: remove the claude.md symlink
- `43b36b68` fix: detect hermes started by the newer installer launcher (#5518)
- `3f5032c9` fix: send pane appearance changes only after the matching background (#5517)
- `6b526626` fix: keep the saved layout when panes die during host shutdown (#5510)
- `ff5881ef` fix: keep a slow server's socket instead of treating a connect timeout as stale (#5511)
- `9168dd33` fix: use newer stable releases on the preview channel
- `5bd900d8` test: synchronize shell and server startup checks (#5476)

## Conflicts

Both files conflicted only because the two sides inserted lines at the same anchor. Each was resolved by keeping
both, Nirvana's lines first, then upstream's.

| File, hunk | Nirvana wanted | Upstream wanted (commit) | Resolution |
|---|---|---|---|
| `src/app/mod.rs`, `App` fields after `pending_agent_resume_deadline` | `nirvana_relaunch_at`, `nirvana_relaunch_until` (relaunch of Nirvana Claude in restored panes), `idle_compact` (plan 27) | `pending_host_appearance_deadline`, `pane_host_appearance`: hold a pane appearance change until the matching background arrives (3f5032c9) | both |
| `src/app/mod.rs`, `App::new` initializers, same place | `None`, `None`, `config.ui.idle_compact` | `None`, `None` | both |
| `src/app/runtime.rs`, the server loop's deadline list | `nirvana_relaunch_at`, `idle_compact_deadline()`, `fresh_deadline()` (plans 25, 27) | `pending_host_appearance_deadline`, so the loop wakes to release the hold (3f5032c9) | both |

## Nirvana code that upstream's changes reach

- 3f5032c9 moved every herdr pane spawn from `state.host_terminal_appearance` to `pane_host_terminal_appearance()`.
  The one spawn that still read the field directly was Nirvana's editor tab, `src/app/nirvana_edit.rs`; it now
  takes the same helper. No control covers that line (touched.py did not list one).
- `config::update_file_at` now takes a closure returning `Result`; no Nirvana code calls it.
- `self_update`: Nirvana's disable guard still runs before upstream's new code; `check_installed` path unchanged.
- `server/headless.rs`: upstream renamed `set_host_terminal_appearance_state` to `set_host_terminal_state` and
  added `release_host_appearance_if_ready`; Nirvana's per-pass calls (`nirvana_relaunch_restored`, idle compact,
  fresh, wake retry) auto-merged beside it.
- The root `CHANGELOG.md` is identical to upstream's after the merge.

## Gates (TEMP and TMP = `C:\Users\fahim\AppData\Local\Temp\sync`)

- `cargo fmt --check`: exit 0.
- `cargo clippy --all-targets --locked -- -D warnings`: exit 0.
- `cargo nextest run --locked --no-fail-fast`: 4589 run, 4588 passed, 1 failed, 15 skipped; the failure is
  `windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override`, which fails on master.
- `python -m unittest nirvana.test_scripts nirvana.test_windows_shell`: 86 tests, OK.
- `python nirvana/controls.py --dry-run`: 2440 of 2440 find their text exactly once.
- touched.py against `master`: `p27_server_setting_from_config`, `p27_loop_deadline`, `p25_loop_deadline` (all
  `find`, because their lines sit beside the conflict). Each run alone: PASS, red line seen. No control text needed
  changing.
