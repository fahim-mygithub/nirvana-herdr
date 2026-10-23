# Feature 27: an idle Claude compacts before its cache goes stale

Status: **PLANNED (2026-10-23)**, to build on agy (Gemini 3.8 Flash high) at master alpha 0.38.0.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Ensusre idle agents compact when cache is about to get stale. This should be a setting for claude code." | §2, §3, §4 | orchestrator | §2 to §4 tests |
| L2 | "All interactive Claude (Recommended)", whose text was: "Nirvana Claude and every plain Claude Code pane herdr sees as idle. Workers run headless and end when done, so they are left out." | §2 | orchestrator | §2 tests |
| L3 | "At 50 min idle (Recommended)", whose text was: "10 minutes of margin before the cache expires, so the compact itself still reads from cache. The minutes are editable in the setting." | §2, §3 | orchestrator | §2 tests |
| L4 | "Skip under 50k tokens (Recommended)", whose text was: "Re-reading a small context costs little, and a compact would throw away detail for no saving. Threshold editable." | §2, §3 | orchestrator | §2 tests |
| L5 | "Settings toggle, on by default", whose text was: "Same switch, but it starts on for every space." (The switch: "A herdr config key plus a toggle in the Nirvana settings.") | §3 | orchestrator | §3 tests |

## 1. Measured 2026-10-23, on master at alpha 0.38.0

The builder re-measures every claim before relying on it.

- **The cache lives 1 hour.** In the five newest transcripts under `~/.claude/projects`, every non-zero cache write is
  `ephemeral_1h_input_tokens`; `ephemeral_5m_input_tokens` is 0 on every line (1622 of 1622 in one session). So an idle
  Claude's next turn after 60 minutes re-writes its whole context; a compact at 50 minutes reads it from cache and leaves
  a small one.
- **Agent state:** `detect::AgentState` is `Idle | Working | Blocked | Unknown` (`src/detect/mod.rs`). `Blocked` means
  it waits on the human (a permission prompt, a question form). The API's `AgentStatus` maps an unseen `Idle` to `Done`
  (`pane_agent_status`, `src/app/api_helpers.rs`), so check `AgentState::Idle`, not the status.
- **No idle time is kept.** Only the counter `terminal.last_agent_state_change_seq` changes on a state change
  (`src/app/actions.rs`, `src/terminal/state.rs`).
- **A server deadline loop exists:** `next_headless_loop_deadline_with_git_refresh` (`src/app/runtime.rs`) takes the
  earliest of optional `Instant`s such as `pending_agent_resume_deadline`; `src/app/agent_resume.rs`
  (`sync_pending_agent_resume_deadline`) is the pattern to copy.
- **Typing a prompt:** `encode_api_submission(runtime, text)` (`src/app/api_helpers.rs`) wraps the text in bracketed
  paste when the terminal asks for it and appends Enter; `try_send_bytes` sends it. Restart Claude
  (`src/app/nirvana_restart.rs`) shows how a pane's terminal runtime is found from its pane id.
- **The transcript:** `nirvana_subagents::resolve_claude_session` maps a pane to its transcript, and returns `None` when
  two Claude panes share a folder. `read_transcript_tail_with_expansion` gives the last assistant line's tokens and
  time; its `parse_claude_usage` adds `output_tokens`, which is not context.
- **Workers:** `herdr seat exec` panes run Claude headless. `is_seat_worker_pane` is filled lazily from run journals.
- **Settings:** `UiConfig` in `src/config/model.rs`, edited through `ConfigEdit` in `src/config/write.rs`, shown in
  `src/client/shell/settings.rs` (`ClientSettingsSection::ALL`: Theme, Indicators, Sound, Toast, Integrations).

## 2. The rule

An interactive Claude pane is sent `/compact` once, when all of these hold:

1. The setting is on.
2. Its process is Claude (`Agent::Claude`), and it is not a worker: not a seat worker pane, and its argv has no `-p` or
   `--print`.
3. Its `AgentState` has been `Idle` without a break for at least `minutes` (default 50). The time is a new
   `idle_since: Option<Instant>` on the terminal, set when the state becomes `Idle` and cleared on any other state.
4. Its transcript resolves to exactly one file, and that file's last write is at least `minutes` old.
5. Its context, the last assistant message's `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`
   (not `output_tokens`), is at least `min_tokens` (default 50,000).
6. Its input line is empty. Re-measure how Claude Code draws an empty prompt on the pane's screen, and decide from the
   screen text; when unsure, do not send.
7. It has not already been sent `/compact` in this idle stretch. A stretch ends when the state leaves `Idle`.

Any error reading any of these sends nothing. Nirvana Claude and plain Claude are treated the same.

- The decision is one pure function taking these facts and returning send or the reason not to; the server only
  gathers the facts and applies it.
- When it sends, the server logs one line naming the pane, the idle minutes and the tokens.
- The check runs from the server's deadline loop: the next deadline is the earliest `idle_since + minutes` of the idle
  Claude panes. No busy polling.

## 3. The setting

- Config, in `config.toml`, defaults shown:

  ```toml
  [ui.idle_compact]
  enabled = true
  minutes = 50
  min_tokens = 50000
  ```

  `minutes` is validated to 1..=59 and `min_tokens` to at least 0, with a clear config error otherwise.
- A new settings section, **Idle compact**, with two choices, On and Off, written through `ConfigEdit` the way the
  other sections are. The two numbers are config-only.
- A changed setting applies without a restart.

## 4. Controls and gates

- Every assertion in §2 and §3 has a sabotage control in `nirvana/controls.d/idle_compact.py`, named `p27_*`, on a
  product line, naming the test that goes red.
- Every older control whose target line changes is updated and still reddens its test.
- Tests use `crate::nirvana::with_test_home(name)` and temporary folders only. No test touches a real transcript, a
  real pane, or `~/.claude`.

## 5. Live check, for the owner

1. Leave a Claude pane with a long conversation idle for 50 minutes: it compacts once, and the log names it.
2. A Claude pane with text typed but not sent, or waiting on a permission prompt, is left alone.
3. Turn the setting off: nothing compacts.

## 6. Not in this plan

- Workers, Codex, agy and local agents.
- Compacting a second time in one idle stretch.
