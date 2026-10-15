# Feature 2: Restart Claude from the tab's right-click menu, same folder, same conversation

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Goa head and commit" | alpha 0.1.0 (c66ec570) pushed to origin, the fork on GitHub | orchestrator | `git ls-remote origin master` names c66ec570 |
| L2 | "lets test another feature to add If i right click this tab and I have an option to restart the claude code insstance with that exact directory opened and that session resumed. [Image #12]" | the tab's right-click menu gains "Restart Claude" when a pane in the tab runs Claude Code; it stops that Claude and starts it again in the same pane, the same folder, resuming the same conversation by its session id | orchestrator | unit tests on the menu, the session lookup and the command; controls that redden each; a live run where the conversation's earlier reply is on screen after the restart and Claude's pid changed |

Image #12, as the owner pasted it: herdr's tab menu open over tab "1" with three items, New tab, Rename, Close; the pane
runs Claude Code v2.1.300 in `scratchpad\nirvana-verb-test`; Claude's status line reads "Update installed · Restart to
update". The last part is read as the reason to want a restart: a new Claude Code is installed and the running one
only picks it up when started again.

## Shape

- **Menu.** The tab menu reads New tab, Rename, Restart Claude, Close when some pane in the tab has `agent ==
  "claude"` in the client's snapshot; the other tab menus are unchanged.
- **Wire.** The item sends herdr's existing `command.invoke` with `command_id = "nirvana.restart_claude"` and the tab
  id. The server takes that id before herdr's custom-command lookup. No new API method, so herdr's API schema and
  its protocol number stay as upstream has them.
- **Which conversation.** Claude Code writes `<config>/sessions/<pid>.json` for each running interactive Claude, with
  `sessionId` and `cwd` (read on this machine 2026-10-15, Claude Code 2.1.300). The server finds the Claude process in
  the pane's foreground job with herdr's own detector, reads that file, and resumes `--resume <sessionId>`. No file
  means no id: the restart falls back to `--continue`, the newest conversation in that folder, and says so.
  `<config>` is `CLAUDE_CONFIG_DIR`, else `~/.claude`.
- **Same flags.** The new Claude gets the old one's arguments (the Nirvana theme `--settings`, a model, an effort)
  minus any earlier `--resume`, `-r`, `--continue`, `-c`, `--session-id` and `--fork-session`.
- **Restart.** The server stops the Claude process and its children, waits up to 3 s for the pane's shell to be free,
  then types the command into the shell the way `herdr agent start` does, with the agent's herdr name kept.
  A Claude that is the pane's own program, with no shell under it, is not stopped: its pane would close.

## Files

| File | Change |
|---|---|
| src/nirvana.rs | `RESTART_CLAUDE_COMMAND`, `claude_config_dir`, `read_claude_session`, `restart_args`, `terminate_processes`, `shell_submission` and their tests |
| src/app/nirvana_restart.rs (new) | the server side: find, stop, wait, start |
| src/app/mod.rs | `mod nirvana_restart;` |
| src/app/custom_commands.rs | the command id is taken before the custom-command lookup |
| src/client/shell/state.rs | `RestartClaude` action; the tab target carries `has_claude` |
| src/client/shell/context_menu.rs | the item, and its press sends `command.invoke` |
| src/client/shell/tests/graphics.rs | the tab target literal gains `has_claude` |
| src/client/shell/tests/nirvana_restart_menu.rs (new), src/client/shell/tests/mod.rs | the menu tests and their `mod` line |
| nirvana/controls.py, nirvana/CHANGELOG.md, nirvana/VERSION, nirvana/README.md | controls, entry, 0.2.0, usage |

## Record

Built 2026-10-15 on herdr master b431a063 + alpha 0.1.0, as alpha 0.2.0.

**Live runs**, scratch home `scratchpad/nh-test2`, session `nirvana-r2`, folder `scratchpad/nirvana-restart-test`,
Claude started by `herdr agent start rtest --kind claude -- --settings {"theme":"custom:nirvana-dark"}` and asked
"Reply with exactly the word PELICAN and nothing else."

1. First build, request sent straight to the server's pipe: reply after 3127 ms, error "Claude was stopped but the
   pane's shell did not come back in 3 s", and Claude pid 53612 still alive. Cause: herdr's Windows
   `signal_processes` opens each process with query rights only, so `TerminateProcess` is refused. Fixed with
   `nirvana::terminate_processes` (PROCESS_TERMINATE), test `terminate_really_stops_a_running_process`, control
   `terminate_query_only`. The error now says which of the two happened.
2. Second build: reply after 166 ms, Claude stopped, and the right command reached the shell, but as
   `[f((Get-Command claude ...}[` at the prompt, never run. Cause: the stopped Claude left bracketed paste and its
   keyboard protocol on, and herdr's `encode_api_submission` follows them. Fixed by typing a plain line and a
   carriage return (`nirvana::shell_submission`), test `the_restart_command_is_typed_as_a_plain_line`, control
   `restart_bracketed_paste`.
3. Third build, through the pipe: reply after 164 ms; Claude pid 54808 -> 54352; session a4dbbd3b-26b9-4042-93e3-3fcb814628a4
   before and after; the new command line `claude.exe --settings {"theme":"custom:nirvana-dark"} --resume a4dbbd3b-...`;
   agent `rtest` idle; the pane shows the PELICAN exchange.
4. Third build, through the menu (leaf agent "Sonnet - click Restart Claude", screenshots `scratchpad/f2/shot-1..3.png`):
   the tab menu read New tab, Rename, Restart Claude, Close; one click; Claude pid 54352 -> 49468, same session id,
   `--resume` once on the command line; the screen shows Claude Code v2.1.301 with the PELICAN exchange and no
   garbled text. The restart picked up the newer installed Claude Code, which is what the owner's screenshot asked
   for ("Update installed · Restart to update").

**Not measured.** A Claude that is the pane's own program (no shell under it) is left alone by design and was not
run. A Claude started with a prompt as a positional argument gets that prompt again on restart; the Nirvana verb
never starts one that way. The server waits on the main loop for the shell to come free, up to 3 s; measured
166 ms and 164 ms.
