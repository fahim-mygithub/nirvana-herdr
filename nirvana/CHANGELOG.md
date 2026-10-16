# Nirvana-Herdr changelog

Each entry: Changed → Because → Measured → If you remove this. herdr's own changes are in the root CHANGELOG.md,
which the fork never edits.

## alpha 0.8.0 (2026-10-16): the Memory square and the shared board

- **Changed:** each workspace has a shared board, `.nirvana/board.md` at the repository's root: a revision, one section
  per topic, each with its last writer. Any agent with a shell reads and writes it through this binary, `herdr board
  read` and `herdr board write <section> --expect <revision> --by <name> -- <text>`. A write naming a stale revision is
  refused (exit 3) and nothing is written; writers take a lock file, and a lock older than 15 s is a dead writer's.
  **Because:** the owner, 2026-10-16: "They seem to have a shared file among all the agents that when you update with
  one agent the other agents pick up on. Kind of like a shared state file", and "Keep a board per workspace." The
  pattern is durable-pi's App State (disler/durable-pi 37165c6): one document, a revision, a refused stale write.
  **Measured:** `nirvana_board::tests::*` (nine); controls `board_*`, `github_*`. Live: `board write Server --expect 0
  --by codex` gave revision 1; the same write again with `--expect 0` exited 3 with "board is at revision 1, not 0".
  **If you remove this:** agents working side by side learn of each other's changes only when you tell them.
- **Changed:** a running Claude is told what changed on the board on its next prompt. `herdr board notice`, run by a
  `UserPromptSubmit` hook, prints the sections changed or removed since that session last saw the board, and nothing
  when none did; a session's first prompt gets the whole board and how to write it. What each session saw is kept under
  the Nirvana-Herdr home, never in the project.
  **Because:** measured 2026-10-16 on Claude Code 2.1.301: an edit to CLAUDE.md or to a file it imports never reaches
  a running session, and a `FileChanged` hook's text never reaches Claude; a `UserPromptSubmit` hook's output does.
  **Measured:** live, Claude Haiku in a scratch repository: the first prompt's notice carried the board, and Claude
  answered "Port 5173" from it with no tool call; an unchanged board added nothing to the next turn (no hook output in
  the transcript); a section another writer added reached the next turn alone; Claude wrote a section itself with the
  command from the notice (`--expect 2`, revision 3). A change notice measured 666 characters, most of it the how-to;
  later notices now carry one line instead (`a_session_is_told_only_what_changed_since_its_last_turn`).
  **If you remove this:** a running Claude acts on a board it read minutes ago.
- **Changed:** the mod strip's third square, Memory: "board r3 · 1 new", "board r3 · not shared", "board empty". Its
  panel lists the sections, marks those this Claude has not seen, and names each one's last writer. Space adds or
  removes the hook in the folder's `.claude/settings.local.json` ("Share the board with every Claude here"); `e` opens
  `board.md` in the terminal editor tab of 0.7.0. The square shows only for a repository with a GitHub remote, read from
  its git config.
  **Because:** the owner: "The next potential feature is a memory manager", "flag this feature only if it opens a
  project with a github"; and "Looks good" to the board and the square first, lessons second.
  **Measured:** `the_memory_square_shows_only_in_a_repository_with_a_github_remote`,
  `the_memory_panel_lists_the_board_shares_it_and_edits_it`; controls `memory_*`, `share_*`, `square_ignores_seen`.
  Live: the square read "board empty", then "board r3 · 1 new"; space wrote the hook, and the Claude already running
  in the folder ran it on its next prompt with no restart.
  **If you remove this:** the board is read by opening the file.
- **Changed:** while a mod panel is open (Preload, Running or Memory), a pane's new output is drawn by composing the
  frame again, never written straight to the screen.
  **Because:** herdr writes a pane's output straight to the screen when nothing blocks that fast path, and the panels
  of 0.5.0 and 0.6.0 did not block it: live on 2026-10-16 the open Memory panel was painted over while Claude answered.
  Every panel since 0.5.0 had the defect; it showed only now because the panel stayed open while an agent printed.
  **Measured:** `an_open_panel_is_redrawn_when_its_agent_prints` (with no panel open the patch still takes the fast
  path); control `panel_painted_over`.
  **If you remove this:** an agent that prints while a panel is open paints over the panel.
- **Changed:** control `panels_both_open` sabotages the Running toggle's line that closes Preload, beside the new
  line that closes Memory.
  **Because:** the new line sat inside its find text, which then occurred 0 times.

## alpha 0.7.0 (2026-10-16): Preload grouped by provider; edit opens a terminal editor in a new tab

- **Changed:** the Preload panel groups its rows by where each comes from: This project, You (~/.claude), one group per
  plugin (its own switch first, then its skills, agents and MCP servers), claude.ai, Built in (Claude Code), Managed.
  Each header shows its group's tokens; each row's detail now names its kind ("memory file", "skill", "MCP, 2 tools",
  "connector, 1 tool", "agent", "tool", "whole plugin").
  **Because:** the owner, 2026-10-16: "Organize the list by provider or built in."
  **Measured:** `each_item_is_grouped_under_the_provider_it_comes_from`,
  `a_switched_off_item_nothing_remembers_is_grouped_by_its_switch`,
  `the_panel_lists_each_group_and_item_with_its_tokens`; controls `provider_*`. A header sums its rows; a plugin's own
  row is left out of its header, since it sums the same rows; Built in adds "System tools", which `/context` does not
  split per tool. The "MCP server instructions" category is no longer shown: `/context` does not split it per server.
  **If you remove this:** a plugin's skills, agents and server are spread over four groups, and what a plugin costs is
  read by adding them up.
- **Changed:** a row switched off keeps its group: the window remembers each row's provider with its tokens; with
  nothing remembered (herdr restarted since), the switch names it (a `plugin:` server is its plugin, a `claude.ai `
  server is claude.ai, a CLAUDE.md under `~/.claude` is You).
  **Because:** a row switched off is gone from `/context`, which is where its provider is read.
  **Measured:** controls `provider_off_item_forgotten`, `provider_off_switch_guess`.
  **If you remove this:** switching a row off moves it to another group.
- **Changed:** "edit" opens the file in a terminal editor that takes the mouse, in a new tab of the panel's space,
  focused and named "edit <file>"; the agent's tab is left as it was. The editor is `NIRVANA_EDITOR` when set, else the
  first found of Microsoft Edit (`edit` on Windows, `msedit` elsewhere), `micro`, `nano --mouse`. With none, the panel
  says so; Notepad is never opened.
  **Because:** the owner, 2026-10-16: "when you edit have a terminal editor that is mouse compatible and open that file
  in a new tab not open through notepad." Microsoft Edit 1.2.1 is part of Windows 11 on this machine, so nothing is
  installed. `$VISUAL`/`$EDITOR` are not read: either may name a window editor.
  **Measured:** `edit_opens_a_terminal_editor_that_takes_the_mouse_never_notepad`,
  `edit_opens_the_file_in_a_new_focused_tab_and_leaves_the_agents_tab_alone`; controls `editor_*`, `edit_tab_*`.
  **If you remove this:** "edit" opens Notepad, over the agent's tab.
- **Changed:** "edit" no longer goes through herdr's scrollback editor helper.
  **Because:** that helper is written for herdr's scrollback temp file: on Linux and macOS it runs
  `rm -f "$scrollback_file"` when the editor exits (`src/platform/linux.rs`), so 0.5.0's "edit" there would have
  deleted the CLAUDE.md it opened. Found while building 0.7.0; never run on Linux or macOS, so never measured.
  **If you remove this:** on Linux and macOS, editing a preloaded file deletes it.
- **Changed:** control `spaces_kind_token_ignored` sabotages `.find(|(name, _)| name == KIND_TOKEN)`.
  **Because:** its find text had been gone since 0.5.0 moved the token read, so it printed "find text occurs 0 times"
  and proved nothing. Found by checking every control's find text before this run.
  **If you remove this:** the agent-icon test has no control.

## alpha 0.6.0 (2026-10-16): the Running square, Running on top and Waiting on you under it

- **Changed:** the mod strip's first square is "Running": "working · Bash", "waits on you" in yellow, "idle". Its
  panel shows Running on top (the task, the last prompt, the tool in flight and for how long, Claude's last words) and
  Waiting on you under it: the prompt Claude is waiting on, read from its pane, each numbered option, "(Recommended)" in
  green. A digit or a click answers it in the pane.
  **Because:** the owner, 2026-10-16: "Marge the running and waiting on you into one panel the top being running and
  what is actively being run with a summary. Then underneath that running is the waiting on you." Asked how Waiting on
  you should behave first, the owner picked "Show and answer"; building the recommended option ahead comes later.
  **Measured:** `nirvana_mods::*` (six tests for this square), `nirvana_running::tests::*` (six),
  `claudes_state_mark_is_not_part_of_the_task`, `a_claudes_transcript_rides_in_the_agent_tokens`; nineteen controls,
  `running_*`, `prompt_*`, `strip_wait_not_yellow`, `strip_blocked_reads_idle`, `answer_*`, `panels_both_open`,
  `title_mark_kept`, `transcript_token_dropped`. The live run is in the plan's Record.
  **If you remove this:** what a Claude is doing and what it waits on is read by switching to its pane.
- **Changed:** Waiting on you reads the prompt from the pane's screen, never from the transcript, and only while herdr
  says the agent is blocked.
  **Because:** measured 2026-10-16: an AskUserQuestion call reaches the transcript only after it is answered, and a
  permission dialog's options change with the case ("Yes, and always allow access to <folder>", "switch to auto
  mode"), so only the screen says what each digit means. One digit picks the option, no Enter.
  **Measured:** `a_question_on_screen_reads_as_its_heading_and_options`,
  `a_permission_dialog_reads_with_its_command`, `a_screen_with_no_waiting_agent_answers_nothing`; controls
  `prompt_heading_past_rule`, `running_prompt_while_working`, `answer_with_enter`.
  **If you remove this:** the panel offers answers to a question it cannot see, or sends a digit into Claude's prompt
  box while it works.
- **Changed:** the answer is typed into the agent's pane through the window's own key path (a pane input event for
  that pane), not sent as an API method.
  **Because:** the first live run of 2026-10-16 sent `pane.send_keys` and herdr answered "Action unavailable": a window
  may send only the methods herdr lists for windows, and neither `pane.send_keys` nor `agent.send_keys` is one. The
  tests had passed, because a test window knows no such list; the test now checks the request a real window sends.
  **Measured:** second live run: the panel's "1" reached Claude, whose transcript recorded "Red (Recommended)", and herdr
  read the agent `idle`; `a_waiting_prompt_is_read_from_the_pane_and_a_digit_answers_it`; control `answer_wrong_digit`.
  **If you remove this:** the panel says it answered and Claude keeps waiting.
- **Changed:** the server sends each Claude's transcript path as the agent token `nirvana_transcript`; the window reads
  the transcript itself every 2 s while the strip shows that agent.
  **Because:** herdr rebuilds a window's snapshot when an agent's state changes, not while it works, so a tool that
  starts mid-turn would wait for the next state change. The path stays the same for the whole conversation.
  **Measured:** `a_claudes_transcript_rides_in_the_agent_tokens`, `a_tool_call_without_a_result_is_what_runs_now`.
  **If you remove this:** Running shows the turn as it was when the agent last changed state.
- **Changed:** a task title no longer carries Claude's state mark ("✳ ", a spinner glyph).
  **Because:** the spaces list read "✳ Button colour que…" in the live run of 2026-10-16.
  **Measured:** `claudes_state_mark_is_not_part_of_the_task`; control `title_mark_kept`.
  **If you remove this:** every Claude task row starts with a mark that is not the task.

## alpha 0.5.0 (2026-10-16): a mod strip under the tab bar, with one mod, Preload

- **Changed:** a tab running Claude Code has a one-row strip under the tab bar, the panes a row shorter. It holds one
  square, " Preload ", with the total a fresh Claude in the agent's folder preloads and "N off" when items are switched
  off there. A click opens the Preload panel: every CLAUDE.md, memory file, skill, MCP server and connector, plugin,
  agent, built-in tool and the hooks' session-start text, each with its tokens; space switches an item off or on for
  every new Claude in the folder, `e` opens its file in the editor, `r` restarts this Claude so it takes the change.
  **Because:** the owner, 2026-10-16: "I want to add the ability to add componoent mods like in Nirvana for each agent.
  ... I wawnt just one mod square for the herder version for now. This mod square is called Preload and it will list all
  the tokens it will preload for this fresh and every Claude session. ... I want to be able to see them and toggle them
  or modify them if I cant remove unselect them."
  **Measured:** `nirvana_mods::*` (eight rendering, key and click tests), `nirvana_preload::tests::*` (six),
  `only_an_existing_absolute_markdown_file_is_editable`; twenty-three controls, `preload_*`, `strip_*`, `panel_*` and
  `edit_any_extension`. The live run is in the plan's Record.
  **If you remove this:** what a Claude session starts with is read with `/context` inside each session, and switching
  an item off means editing Claude's settings by hand.
- **Changed:** the list is Claude Code's own `/context`, run in the folder as `claude -p /context
  --no-session-persistence --verbose --output-format json`, read from its `context_usage` and `init` events.
  **Because:** Claude Code answers it locally: 0 tokens, a cost of 0, in 2.6 s (2026-10-16). `--verbose` gives the
  tokens as integers and the servers, plugins and tools by the names their settings take; without it the answer is a
  markdown table.
  **Measured:** `the_report_reads_into_its_total_and_items` on a report cut from a real run.
  **If you remove this:** the panel guesses at what Claude loads, and its names stop matching the settings.
- **Changed:** each switch is the setting proven to remove that item from a new session's `/context`, written to the
  folder's `.claude/settings.local.json`: `claudeMdExcludes`, `autoMemoryEnabled`, `skillOverrides`, `enabledPlugins`,
  `deniedMcpServers`, `permissions.deny`, `disableAllHooks`. Only the item's entry is written; switching back removes it.
  **Because:** 96 free `/context` runs on 2026-10-16 ("Sonnet - measure Claude preload toggles"). `permissions.deny`
  on an MCP server hid its tools but left it connected with its instructions loaded, so servers use
  `deniedMcpServers`. A plugin's own skill and a custom agent have no switch of their own; the panel says so.
  **Measured:** `each_kind_of_item_gets_its_proven_switch`,
  `switching_writes_only_the_items_entry_and_back_leaves_the_file_as_it_was`,
  `the_local_settings_file_round_trips_and_a_broken_one_is_left_alone`; controls `preload_mcp_by_permission`,
  `preload_empty_left`, `preload_broken_file_replaced`.
  **If you remove this:** a switch that looks off leaves the item loaded, or the owner's settings file is rewritten.
- **Changed:** `command.invoke` with `nirvana.edit_file:<path>` opens an existing absolute `.md` file in herdr's
  scrollback editor, in a zoomed pane.
  **Because:** "or modify them". Every file Preload offers to edit is markdown; the check keeps the command from
  opening anything else a client names.
  **Measured:** `e_asks_the_server_to_open_the_items_file`, `only_an_existing_absolute_markdown_file_is_editable`;
  control `edit_any_extension`.
  **If you remove this:** "edit" has nothing to open the file with.

## alpha 0.4.0 (2026-10-15): running servers and usage at the foot of the sidebar

- **Changed:** the foot of the expanded sidebar, under the agents panel, holds old Nirvana's two blocks. "servers": one
  row per port a process in a space's panes listens on, the space's name and `localhost:<port>` (`:<port>` when the
  name needs the cells); a click opens it in the browser; the block hides when no space runs a server; four rows,
  then "+N more". "usage": Claude, Codex, Antigravity and Local, each with Nirvana's provider colour, the five-hour and
  seven-day windows in two columns under "5h 7D", and from 34 columns a bar for the five-hour window.
  **Because:** the owner, 2026-10-15: "Lets work on the next feature. This running server and usage from Nirvana lets
  port it to the Nirvana herder".
  **Measured:** `nirvana_footer::*` (eight rendering and click tests), `nirvana_servers::tests::*` (seven),
  `nirvana_usage::tests::*` (six); twenty-three controls, `servers_*`, `usage_*` and `footer_*`. The live run is in the
  plan's Record.
  **If you remove this:** which space runs a server, and how much of each subscription is left, is read outside herdr.
- **Changed:** a server is any TCP port listened on inside a space's panes (the shell and its whole tree), read by the
  server on one worker thread every 3 s and sent as the space's `nirvana_servers` token, so the wire is unchanged.
  Ports from 49152 up and ports owned by claude, codex, agy or Antigravity are not servers.
  **Because:** herdr has no daemon that starts servers, which is where old Nirvana's list came from. The live check
  found an idle Claude Code on 64839 and agy's app on 60015 and 60016, which a plain port scan would list.
  **Measured:** `plumbing_ports_are_not_servers`; controls `servers_dynamic_port_kept`, `servers_agent_port_kept`.
  **If you remove this:** each Claude, agy and helper adds a row that is not a server.
- **Changed:** the worker keeps reading a space while any of its pane shells runs, not only while a window asks.
  **Because:** the server rebuilds a window's snapshot when the sidebar's state changes, not on pane output. In the
  first live run a server's row stayed after the server stopped: the space had gone unasked for over 60 s, so the
  worker had dropped it, and the row never cleared.
  **Measured:** second live run, server idle 75 s then stopped: its row was gone 8 s later (plan Record);
  `a_space_is_read_while_a_shell_lives`, control `servers_dead_space_kept`.
  **If you remove this:** a server that stops after a quiet minute keeps its row until something else changes.
- **Changed:** the usage rows are read by the window, never the server: Claude by `claude -p /usage
  --no-session-persistence` (every 120 s while a Claude in the window works, else 600 s), Codex from its rollouts'
  `rate_limits` (30 s), Antigravity by `agy -p /usage` (300 s), Local from Ollama on 127.0.0.1:11434 and `nvidia-smi`
  (30 s). A failed read keeps the last figure.
  **Because:** Claude Code answers `/usage` locally: the live check returned model `<synthetic>`, 0 tokens, a cost of 0,
  in 2.7 s. Old Nirvana paid a model turn for its Claude reading. `--bare` drops the subscription sign-in and answers
  with the API cost view, so it is not used. agy's read took 20.8 s.
  **Measured:** the parsers' tests on text captured from this machine on 2026-10-15; `a_failed_read_keeps_the_last_figure`.
  **If you remove this:** the rows say "reading" forever.
- **Changed:** upstream's `sidebar_scrollbars_use_proportional_shared_geometry_and_drag` composes at 32 rows, not 26.
  **Because:** the foot takes six rows from the agents panel, which at 26 left its ten agents a one-cell thumb.
  **Measured:** at 32 both lists still overflow; the test asserts both tracks.
  **If you remove this:** that upstream test fails on the fork.

## alpha 0.3.0 (2026-10-15): each space lists its agents' task and model, with the agent's icon

- **Changed:** under each space in the spaces list, two rows per agent running there: the task's summarized title
  (Claude Code's terminal title, "No task yet" before the first prompt), then the agent's icon and its provider and
  model, e.g. "✻ Claude Sonnet 5.5". The icon takes the place of the status circle and keeps its colour (idle green,
  working yellow, blocked red, done teal). Icons: Claude ✻, Codex ❖, Antigravity ▲, Local ⌂; any other agent keeps
  herdr's dot. A space shows three agents, then "+N more"; the agents panel below still lists them all.
  **Because:** the owner, 2026-10-15: "See how in our old Nirvana we showcase the task a summerised title and its active
  model ... for herder it would be the model provider and tyle ie Claude Sonnet. ... Basically I would to have spaces do
  somethign similar and on the left Instead of a green circle its the agents Icon. We need one for claude antigravity
  codex and local."
  **Measured:** `nirvana_space_agents::*` (six rendering tests) and `nirvana_spaces::tests::*` (ten reader and label
  tests); eighteen controls, `spaces_*`. The live run is in the plan's Record.
  **If you remove this:** the spaces list shows only folder names; which agent and model works where is read pane by
  pane.
- **Changed:** the server reads each agent's model off its main loop: Claude from its transcript's last reply (found
  through `<config>/sessions/<pid>.json`), Codex from `--model` or its newest rollout in the pane's folder, Antigravity
  from `--model` or its log's last model switch. A Claude on a non-Anthropic model (Nirvana's local seat through
  Ollama) is Local. The result rides in the agent's existing `tokens` as `nirvana_kind` and `nirvana_model`, so the wire
  format is unchanged and an older server still talks to this client.
  **Because:** a model read on the server's loop would stall every pane's output; a new snapshot field would break an
  older server with a newer client, which the 0.1.0 server and 0.2.0 client did for a command on 2026-10-15.
  **Measured:** reads are cached for 3 s and run on one worker thread that wakes a render only when a model changed.
  **If you remove this:** the client has no model to draw.
- **Changed:** upstream's `sidebar_scrollbars_use_proportional_shared_geometry_and_drag` composes at 26 rows, not 20.
  **Because:** its ten agents now also draw under their space, so at 20 rows the spaces thumb is one cell.
  **Measured:** at 26 both lists still overflow; the test asserts both tracks.
  **If you remove this:** that upstream test fails on the fork.

## alpha 0.2.0 (2026-10-15): Restart Claude from the tab menu, same folder, same conversation

- **Changed:** a tab whose pane runs Claude Code has "Restart Claude" in its right-click menu, between Rename and
  Close. It stops that Claude and its children, waits up to 3 s for the pane's shell, and types
  `claude <the old flags> --resume <session id>` into it, keeping the herdr agent name. The id comes from Claude Code's
  own `<config>/sessions/<pid>.json`; with no file the restart uses `--continue` instead. The menu sends herdr's
  existing `command.invoke`, so herdr's API and protocol are unchanged.
  **Because:** the owner, 2026-10-15: "If i right click this tab and I have an option to restart the claude code
  insstance with that exact directory opened and that session resumed." Their screenshot's Claude read "Update
  installed · Restart to update".
  **Measured:** `a_tab_running_claude_offers_restart_claude`, `restart_claude_asks_the_server_for_that_tab`,
  `restart_claude_reaches_the_server_and_stops_nothing_without_claude`,
  `the_session_file_names_the_running_claudes_conversation`, `a_session_file_left_by_another_process_is_not_used`,
  `restart_keeps_the_flags_and_resumes_the_same_conversation`, `terminate_really_stops_a_running_process`,
  `the_restart_command_is_typed_as_a_plain_line`; eleven controls, `restart_*`, `session_*`, `terminate_query_only`.
  The live runs are in the plan: the first two found that herdr's Windows kill is refused (query-only handle) and that
  a stopped Claude leaves bracketed paste on; both are fixed in the fork's own code.
  **If you remove this:** restarting Claude means quitting it, finding the conversation id, and typing the resume
  command with the Nirvana theme flags by hand.

## alpha 0.1.0 (2026-10-15): the Nirvana header, its version, and reload onto the newest build

Base: herdr master b431a063 (Cargo version 0.9.5).

- **Changed:** the sidebar's first row is the brand row: a teal ◆, "Nirvana" in bold, a badge "alpha 0.1.0", and the
  sidebar toggle « at its right edge (it was at the bottom right). " spaces" moved to the second row, which upstream
  left blank, so the workspace list keeps its place.
  **Because:** the owner, 2026-10-15: "have this element for the Herdr instance [Image #11]" (Nirvana's header).
  **Measured:** `brand_row_names_nirvana_and_its_running_version`, `a_narrow_sidebar_keeps_the_toggle_and_clips_the_badge`;
  controls `header_drops_version`, `header_badge_clipped` (the live run found the badge one column short).
  **If you remove this:** the window no longer says which build it runs.
- **Changed:** the fork has its own version in nirvana/VERSION, compiled in as NIRVANA_VERSION, bumped by
  nirvana/version.py. herdr's Cargo version is left alone.
  **Because:** Nirvana's rule 16, "That way I can see if the Nirvana window is running the new version"; herdr's
  version gates its update manifest and protocol checks, so moving it would break both.
  **Measured:** `version_comes_from_the_version_file`, `VersionTest`; controls `version_not_from_file`,
  `bump_feature_keeps_fix`.
  **If you remove this:** the badge has no number of its own to show.
- **Changed:** the server's background version check reads `<home>/bin/nirvana-version.txt` every 5 s instead of
  herdr's release manifest every 30 min, and announces each newer build once through herdr's existing
  `UpdateReady`. The badge then reads "↻ 0.1.1"; a press arms it ("restart?"), a second press detaches, stops the
  server (it saves the session), and exits 75; the `nirvana-shell` supervisor starts the installed build, which
  starts a new server that restores the session. `herdr update` refuses.
  **Because:** "hot realoding the newest changes", read from Nirvana's Reload. Two presses because a restart stops
  the programs in the panes: Windows has no live handoff (src/platform/mod.rs, "Windows has no live handoff").
  **Measured:** `the_stamp_beside_the_installed_build_is_read`, `only_a_later_build_is_newer`,
  `a_newer_build_turns_the_badge_into_a_reload_button`, `reload_takes_two_presses`, `InstallTest`; controls
  `header_reload_unclickable`, `reload_single_press`, `same_build_counts_as_newer`, `install_stamps_wrong_version`,
  `install_overwrites_old_build`. The live reload run is recorded in the plan.
  **If you remove this:** a new build sits unused until someone stops herdr by hand; with the update check back on,
  herdr would offer to install upstream herdr over the fork.
- **Changed:** the default window title is "{workspace} · Nirvana" (was "{hostname}: {workspace}").
  **Because:** the owner's image reads "Downloads · Nirvana": the project in view, then the product.
  **Measured:** `nirvana_default_title_names_the_project_then_nirvana`; control `title_upstream_default`.
  **If you remove this:** the Windows Terminal tab shows the machine name instead of the project.
