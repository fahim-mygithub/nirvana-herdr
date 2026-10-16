# Feature 7: Preload grouped by provider, and "edit" in a terminal editor in a new tab

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "I wawn tto make a change to the Preload Organize the list by provider or built in." | the Preload panel's rows are grouped by where each item comes from: this project, the user's own config, each plugin, claude.ai, Claude Code built in, managed policy; each group a header with its tokens | orchestrator | unit tests on the fixture captured from this machine; a live run, screenshot read by a leaf agent |
| L2 | "Also when you edit have a terminal editor that is mouse compatible" | "edit" opens a terminal editor that takes the mouse; Notepad is never the fallback | orchestrator | unit test on the editor choice; a live run that clicks inside the editor |
| L3 | "and open that file in a new tab not open through notepad." | the editor runs in a new herdr tab of the same space, focused, labelled with the file's name; the agent's tab is left as it was | orchestrator | app test that a new tab is created and focused; the live run |

## Shape

- **Providers.** Each row gets a provider from Claude Code's own `source` fields in `/context`: `projectSettings`,
  `localSettings`, a memory file of type Project or Local, an MCP server of source `project`/`local` are **This
  project**; `userSettings`, type User, AutoMem, source `user` are **You (~/.claude)**; `plugin`, a skill's
  `plugin_name`, an agent named `plugin:...`, an MCP server named `plugin:<name>:<server>` are **Plugin <name>**, whose
  first row is the plugin's own switch; `syncedSkills`, `claudeai` are **claude.ai**; `built-in` skills, the built-in
  tools and the hooks row are **Built in (Claude Code)**; `policySettings`/`managed` are **Managed**. Order: This
  project, You, each plugin by name, claude.ai, Built in, Managed. Inside a group, rows go by kind (CLAUDE.md and
  memory, skills, MCP, agents, tools, hooks) then name, and each row's detail names its kind.
- **A header's tokens** are the sum of its rows (the plugin's own row excluded, it sums its parts); Built in adds the
  "System tools" category, which `/context` does not split per tool.
- **Rows switched off** are gone from `/context`; their provider is remembered with their tokens from the last read.
  With nothing remembered (herdr restarted since), the provider is inferred from the switch: a CLAUDE.md under the
  folder is This project, else You; an MCP server named `plugin:` is its plugin, `claude.ai ` is claude.ai, else You; a
  skill with a folder under the project or the user's skills is that, else Built in.
- **The editor**, first found: `NIRVANA_EDITOR` (a program, run with the file), then Microsoft Edit (`edit`, part of
  Windows 11; mouse: click, select, scroll, menus), then `micro`, then `nano --mouse`. `$VISUAL`/`$EDITOR` are not
  used: on this machine neither is set, and either may name a window editor. None found: the panel says so.
- **The tab.** `nirvana.edit_file:<path>` makes a new tab in the panel's space whose one pane runs the editor in the
  file's folder, focuses it and labels the pane with the file's name. herdr's own behaviour closes the tab when the
  editor exits.
- 0.5.0 opened the editor through herdr's scrollback helper, which on Linux and macOS deletes the file when the editor
  exits (`platform/linux.rs` `rm -f "$scrollback_file"`): a CLAUDE.md edited there would have been deleted. The new
  path does not use it.

## Files

- `src/nirvana_preload.rs`: `Provider`, `Item.provider`, remembered providers, sort.
- `src/client/shell/nirvana_mods.rs`: headers by provider, header tokens, the plugin row's hint.
- `src/nirvana.rs`: `terminal_editor_argv`.
- `src/app/nirvana_edit.rs`: the new tab.
- tests beside each; `nirvana/controls.py`; `nirvana/CHANGELOG.md`.

## Record

Built 2026-10-16 on alpha 0.6.0 (510f1bd0), as alpha 0.7.0. The parked 0.2.1 "Reload first" notice stays in `git stash`.

**Gate.** `cargo fmt --check` clean; `cargo clippy --all-targets --locked -- -D warnings` clean; `cargo nextest run`
3572 of 3573, the one failure the upstream `windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override`
that failed before this change too. 48 controls on the changed files, each run alone: 48 PASS, the 13 new or repaired
ones among them (`provider_*`, `editor_*`, `edit_tab_*`, `spaces_kind_token_ignored`).

**Found while building.** Control `spaces_kind_token_ignored` had a find text that no longer occurred (gone since
0.5.0), so it proved nothing; it now sabotages the token read. Every control's find text was checked to occur once.

**Live run**, scratch home `scratchpad/nh-test8`, session `nirvana-s8`, folder `scratchpad/nirvana-edit-test` holding a
three-line CLAUDE.md, in a classic console window started minimized and placed behind without activation; Claude Haiku
in the pane. Screenshots read by leaf "Haiku - read Preload panel shots".
1. A click on the Preload square opened the panel: "This project 31" with "[x] CLAUDE.md memory file 31 edit", then
   "You (~/.claude) 10.7k" with the user's skills (`f7/shot-2-panel.png`).
2. The key "e" opened a new tab: `tab list` showed tab 2 "edit CLAUDE.md", focused, one pane, and tab 1 still one pane
   with Claude idle; `edit.exe <scratch>\CLAUDE.md` was running (`f7/shot-3-editor.png`).
3. A click in the editor's menu bar opened Microsoft Edit's Edit menu (aimed at File, landed one menu over): the mouse
   reaches the editor (`f7/shot-4-menu.png`).
4. A click at the end of line 3 put the cursor there; typed " saved" and Ctrl+S; the file on disk read "Marker line for
   the 0.7.0 edit test. saved". The file was kept.
5. Ctrl+Q quit the editor; its tab closed and tab 1 was focused again. The session was stopped; no test process left.

**Not measured.** Grouping with plugins and claude.ai connectors live (the scratch folder showed This project and You
only on screen; the plugin and claude.ai groups are in the unit test on this machine's captured report). `micro` and
`nano --mouse` (not installed here). Linux and macOS.
