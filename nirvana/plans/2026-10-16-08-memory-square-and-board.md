# Feature 8: the Memory square and the shared board (step 1 of the memory manager)

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "So the third  box  I also want to borrow from Pi Durable. They seem to have a shared file among all the agents that when you update with one agent the other agents pick up on. Kind of like a shared state file" | a board file that any agent can read and write, and that a running Claude sees on its next turn after another agent changes it | orchestrator | unit tests on the board and the notice; a live run where a change written from outside reaches a running Claude |
| L2 | "The next potential feature is a memory manager." | the strip's third square, "Memory", with a panel | orchestrator | rendering tests; the live run, screenshot read by a leaf |
| L3 | "Keep a board per workspace." | one board per project folder (the git root of the agent's folder), shared by every agent working there | orchestrator | unit test that two folders under one root share one board |
| L4 | "keep it per github remote assuming that project has a github, flag this feature only if it opens a project with a github" | the Memory square and the board appear only when the folder's repository has a GitHub remote | orchestrator | unit test on the remote check; the square is absent for a folder without one |
| L5 | "Looks good" (to: "The board and the Memory square first ... Lessons second") | this feature is the board and the square only; lessons are the next feature | orchestrator | nothing of lessons is built |

Research and rulings behind it: https://claude.ai/artifact/7ruvbW6wtmYKeDxhuYqWyB (Ledger-Lessons, version 3). The board copies
durable-pi's App State (disler/durable-pi 37165c6): one shared document, a revision number, a write that names the
revision it read and is refused if another write came first, agents that read before writing.

## What reaches a running Claude (measured 2026-10-16, Claude Code 2.1.301, leaf "Sonnet - measure Claude shared file pickup")

- An edit to CLAUDE.md, or to a file CLAUDE.md imports, does not reach a running session. `--resume` re-reads them.
- A `UserPromptSubmit` hook's stdout reaches the running session on the next turn, as a `hook_success` attachment:
  about 45 tokens for a one-line change. A hook that prints nothing costs nothing measurable.
- A `FileChanged` hook fires, but its text never reaches Claude.

So the board reaches Claude through a `UserPromptSubmit` hook that prints only what changed since that session last saw
the board.

## Shape

- **The board** is `.nirvana/board.md` at the project's git root: a `revision:` header, then one `## <section>` per topic,
  each section's text, and a comment naming its last writer and time. Plain markdown: you can read and edit it, and git
  diffs it.
- **Any agent writes it** through the fork's own binary, so every provider with a shell can: `herdr board read
  [<section>]`, `herdr board write <section> --expect <revision> [--by <name>] -- <text>`. A write with a stale
  revision is refused (exit 3, "board is at revision N; read it again"), as durable-pi refuses with 409. Writes take a
  lock file, so two agents cannot interleave.
- **A running Claude sees changes** through `herdr board notice`, run by the folder's `UserPromptSubmit` hook. It reads
  the hook's JSON (session id, folder), compares the board with what that session last saw (a copy kept under the
  Nirvana-Herdr home, never in the project), and prints only the changed, added or removed sections, with one line on how
  to read and write. On a session's first prompt it prints the board and the line once. Unchanged: it prints nothing.
- **The Memory square** sits after Preload: "Memory board r3 · 1 new", "Memory board r3 · not shared", "Memory board
  empty". Only for a folder whose repository has a GitHub remote, read from the repository's git config (no `git`
  process in the draw path).
- **The panel** lists the board's sections, each marked when this Claude has not seen its latest text, with its last
  writer. Space switches "Share the board with every Claude here": it adds or removes the hook in the folder's
  `.claude/settings.local.json`, the file Preload's switches use. `e` opens `board.md` in the terminal editor tab
  (feature 7). Off by default: herdr does not add a hook to a folder until you press the switch.
- An edit made by hand does not raise the revision; the notice still carries it, because it compares text, not numbers.
- Not in this step: lessons, other providers' per-turn notice (Codex, Antigravity, local take the board by reading it).

## Files

- `src/nirvana_board.rs` (new): the board, the lock, the notice, the GitHub check, the hook switch, the CLI.
- `src/main.rs`: `herdr board ...` goes to it, as `nirvana-shell` does.
- `src/client/shell/nirvana_memory.rs` (new): the square's value, the panel, its keys and mouse.
- `src/client/shell/nirvana_mods.rs`: the strip draws the third square; one panel open at a time.
- `src/client/shell/{composition,input,mouse,state,mod}.rs`: wiring, as for Running and Preload.
- tests beside each; `nirvana/controls.py`; `nirvana/CHANGELOG.md`.

## Record

Built 2026-10-16 on alpha 0.7.0 (666d2317), as alpha 0.8.0. The parked 0.2.1 "Reload first" notice stays in `git stash`.

**Live run**, scratch home `scratchpad/nh-test9`, session `nirvana-s9`, folder `scratchpad/nirvana-board-test` (`git
init`, remote `https://github.com/example/board-test.git`, never pushed), classic console window started minimized
and placed behind without activation, Claude Haiku in the pane. Screenshots read by leaf "Haiku - read Memory panel
shots"; the transcript read directly.
1. The strip showed a third square; a click opened "Memory · nirvana-board-test"; space wrote one `UserPromptSubmit`
   hook, `"<nh-test9>\bin\herdr.exe" board notice`, into the folder's `.claude/settings.local.json`; the panel read
   "[x] Share the board ..." and "Nothing on the board yet" (`f7/shot-9a-panel.png`).
2. `herdr board write Server --expect 0 --by codex -- dev server on port 5173` printed "revision: 1"; the same write
   with `--expect 0` exited 3, "board is at revision 1, not 0; read it again".
3. Prompt "Without using any tools: what port does the shared board say the dev server uses?": the transcript holds a
   `hook_success` attachment of 624 characters, "Shared board, revision 1: ## Server (by codex) dev server on port
   5173 ...", and Claude answered "Port 5173". The hook was added after this Claude started; it ran with no restart.
4. Prompt "Say OK and nothing else.": no hook output in the transcript.
5. `board write Decision --expect 1 --by codex`; the next prompt's notice (666 characters) held only Decision, and
   Claude said the board "moved to revision 2, which added a Decision (by codex) section".
6. Asked to write Status as claude, Claude ran `herdr.exe board write Status --expect 2 --by claude -- "tests green"`;
   the board read revision 3 with all three sections and their writers.
7. The strip read "Memory board r3 · 1 new" (`f7/shot-9b-strip.png`): the new one is Status, which Claude wrote after
   its own last notice.

**Found live and fixed in this build.**
- The open Memory panel was painted over while Claude answered: herdr writes a pane's output straight to the screen
  unless something blocks that path, and no mod panel did. Every panel since 0.5.0 had it. An open panel now blocks it.
- A change notice was 666 characters, most of it the how-to with the program path twice. Later notices carry one line.
- The panel told you to restart Claude after sharing; step 3 showed no restart is needed. The message now says so.
These three changes are covered by tests and controls; the live run was not repeated after them.

**Not measured.** Codex, Antigravity or a local model reading or writing the board (any agent with a shell can; none
was run). Two writers at the same instant (the lock is unit-tested, not raced live). A worktree's board live (the git
config read is unit-tested). Whether a Claude that compacts its context loses the first prompt's how-to (later
notices repeat the short form). A Claude's own write shows as new to it on its next notice.
