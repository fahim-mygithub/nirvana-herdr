//! Nirvana fork, plan 27: the server's half of an idle Claude's `/compact`.
//!
//! The rule lives in `nirvana_idle_compact::decide`; this file only gathers its facts and applies the answer. The
//! server's deadline loop wakes at the earliest idle Claude terminal's check (`idle_compact_deadline`, read by
//! `next_headless_loop_deadline_with_git_refresh`), so nothing polls. The command goes in two steps, each on its
//! own pass: the paste, then the reads after it (`ENTER_TRIES`), Enter at the first that reads exactly `/compact`
//! (review S1), and, when none did, the erase of a paste that echoed late (third round F1). Each pass does the work of
//! at most one terminal (review S4).

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;

use super::App;
use crate::detect::Agent;
use crate::nirvana_idle_compact::{
    after_paste, decide, read_input_line, read_outside, AfterPaste, Cursor, Decision, Facts,
    InputLine, Outside, OwnerKey, PaneQuery, Pending, Step, ERASE_KEYS,
};
use crate::terminal::{TerminalId, TerminalRuntime, TerminalState};

/// How long the server waits on the pane writer's reply for the Enter or the erase (review S1). The keys go with no
/// delay, so the reply takes one write; the bound keeps a stuck pane from holding the server loop.
const KEYS_REPLY_WAIT: Duration = Duration::from_millis(250);

/// A pane herdr sees running Claude.
struct ClaudePane {
    terminal_id: TerminalId,
    public: String,
    folder: Option<PathBuf>,
    hook_session: Option<String>,
}

/// The strict reading of a pane's input line (`read_input_line`), from its screen with colours, its cursor and its
/// scroll.
fn input_line_of(runtime: &TerminalRuntime) -> InputLine {
    let Some((cols, rows)) = runtime.terminal_dimensions() else {
        return InputLine::Unsure("the pane's size could not be read".into());
    };
    let cursor = runtime
        .cursor_state(ratatui::layout::Rect::new(0, 0, cols, rows), true)
        .map(|cursor| Cursor {
            row: usize::from(cursor.y),
            col: usize::from(cursor.x),
            visible: cursor.visible,
        });
    let scrolled = runtime
        .scroll_metrics()
        .is_none_or(|metrics| metrics.offset_from_bottom > 0);
    read_input_line(&runtime.visible_ansi(), cursor, scrolled)
}

/// The owner's last key in the pane, from the file the server writes for the orchestrator's wake path
/// (`nirvana_run::note_owner_key`). No Nirvana home is a doubt, not quiet (third round nit).
fn owner_key_of(public: &str) -> OwnerKey {
    crate::nirvana_idle_compact::owner_key(
        crate::nirvana::home().as_deref(),
        public,
        crate::nirvana_run::now(),
    )
}

/// Writes `keys` to the pane and waits on the writer's reply, so a failed write is known (review S1).
fn write_keys(runtime: &TerminalRuntime, keys: Vec<u8>) -> std::io::Result<()> {
    runtime
        .queue_user_input_submission(Bytes::new(), Bytes::from(keys), Duration::ZERO, None)
        .and_then(|reply| {
            reply
                .recv_timeout(KEYS_REPLY_WAIT)
                .map_err(std::io::Error::other)?
        })
}

/// One Backspace per character the paste added, in the key encoding the pane's terminal uses (third round F1).
fn erase_keys(runtime: &TerminalRuntime) -> Vec<u8> {
    let backspace = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Backspace,
        crossterm::event::KeyModifiers::NONE,
    );
    runtime
        .encode_terminal_key(backspace.into())
        .repeat(ERASE_KEYS)
}

impl App {
    /// The terminals idle compact looks at, for both the deadline and the check (review S2): herdr sees Claude in
    /// them. A terminal with no pane is still one of them, so its check is stamped and its deadline moves on.
    fn idle_compact_terminals(&self) -> impl Iterator<Item = (&TerminalId, &TerminalState)> {
        self.state
            .terminals
            .iter()
            .filter(|(_, terminal)| terminal.effective_known_agent() == Some(Agent::Claude))
    }

    /// When the next idle Claude terminal is due a check, for the server's deadline loop. `None` when the setting is
    /// off.
    pub(crate) fn idle_compact_deadline(&self) -> Option<Instant> {
        let rule = self.idle_compact.rule()?;
        self.idle_compact_terminals()
            .filter_map(|(_, terminal)| terminal.idle_stretch.next_check_at(rule.minutes))
            .min()
    }

    /// Checks the idle Claude terminal that is due first, and types `/compact` where `decide` says to. True when a
    /// key was typed.
    pub(crate) fn nirvana_idle_compact(&mut self, now: Instant) -> bool {
        if self.idle_compact_deadline().is_none_or(|due| now < due) {
            return false;
        }
        let claude_dir = crate::nirvana_subagents::claude_config_dir();
        self.nirvana_idle_compact_with(now, |query| {
            read_outside(query, claude_dir.as_deref(), SystemTime::now())
        })
    }

    /// `read` gives the facts outside herdr (the Claude process, its transcript), so a test passes stand-ins.
    fn nirvana_idle_compact_with(
        &mut self,
        now: Instant,
        read: impl Fn(&PaneQuery) -> Outside,
    ) -> bool {
        let Some(rule) = self.idle_compact.rule() else {
            return false;
        };
        // Review S4: the one due first; the rest stay due, so the loop comes straight back for them.
        let Some(terminal_id) = self
            .idle_compact_terminals()
            .filter_map(|(id, terminal)| {
                Some((terminal.idle_stretch.next_check_at(rule.minutes)?, id))
            })
            .filter(|(due, _)| *due <= now)
            .min_by_key(|(due, _)| *due)
            .map(|(_, id)| id.clone())
        else {
            return false;
        };
        let Some(terminal) = self.state.terminals.get_mut(&terminal_id) else {
            return false;
        };
        if let Some(pending) = terminal.idle_stretch.pending.take() {
            return self.idle_compact_after_paste(&terminal_id, pending);
        }
        // Review S2: stamped first, pane or not, so a check that finds nothing waits RECHECK.
        terminal.idle_stretch.checked_at = Some(now);
        let stretch = terminal.idle_stretch.clone();
        let state = terminal.state;
        let agent = terminal.effective_known_agent();
        let panes = self.claude_panes();
        let Some(pane) = panes.iter().find(|pane| pane.terminal_id == terminal_id) else {
            tracing::debug!("nirvana: idle compact found no pane for an idle Claude terminal");
            return false;
        };
        let runtime = self.terminal_runtimes.get(&terminal_id);
        let input_line = runtime.map_or_else(
            || InputLine::Unsure("the pane has no runtime".into()),
            input_line_of,
        );
        let query = PaneQuery {
            pane_id: pane.public.clone(),
            shell_pid: runtime.and_then(TerminalRuntime::child_pid),
            folder: pane.folder.clone(),
            hook_session: pane.hook_session.clone(),
            others: panes
                .iter()
                .filter(|other| other.terminal_id != pane.terminal_id)
                .filter_map(|other| {
                    Some((
                        other.public.clone(),
                        other.folder.clone()?,
                        other.hook_session.clone(),
                    ))
                })
                .collect(),
        };
        let outside = read(&query);
        let idle_for = stretch
            .since
            .map(|since| now.saturating_duration_since(since));
        let facts = Facts {
            rule: Some(rule),
            agent,
            seat_worker: crate::nirvana_subagents::is_seat_worker_pane(&pane.public),
            argv: outside.argv,
            state,
            idle_for,
            sent_in_stretch: stretch.sent,
            transcript_age: outside.transcript_age,
            context_tokens: outside.context_tokens,
            owner_key: owner_key_of(&pane.public),
            input_line,
        };
        match decide(&facts) {
            Decision::Skip(reason) => {
                tracing::debug!(pane = %pane.public, reason, "nirvana: idle compact left the pane alone");
                false
            }
            Decision::Send => self.idle_compact_paste(
                &terminal_id,
                &pane.public,
                now,
                idle_for,
                facts.context_tokens,
            ),
        }
    }

    /// The paste. Review S6: the stretch is marked sent once the paste is queued, and not before, so a paste that
    /// fails to queue leaves the stretch unsent for the next check.
    fn idle_compact_paste(
        &mut self,
        terminal_id: &TerminalId,
        public: &str,
        now: Instant,
        idle_for: Option<Duration>,
        tokens: Option<u64>,
    ) -> bool {
        let Some(runtime) = self.terminal_runtimes.get(terminal_id) else {
            return false;
        };
        let (text, _) = super::api_helpers::encode_api_submission_parts(
            runtime,
            crate::nirvana_idle_compact::COMMAND,
        );
        if let Err(err) = runtime.try_send_bytes(Bytes::from(text)) {
            tracing::warn!(pane = %public, err = %err, "nirvana: idle compact could not queue /compact; the stretch stays unsent");
            return false;
        }
        if let Some(terminal) = self.state.terminals.get_mut(terminal_id) {
            terminal.idle_stretch.sent = true;
            terminal.idle_stretch.pending = Some(Pending {
                pane: public.to_owned(),
                pasted_at: now,
                step: Step::Try(0),
            });
        }
        let tokens = tokens.unwrap_or(0);
        match idle_for {
            Some(idle) => tracing::info!(
                pane = %public,
                idle_minutes = idle.as_secs() / 60,
                tokens,
                "nirvana: idle Claude pasted /compact"
            ),
            None => tracing::info!(pane = %public, tokens, "nirvana: idle Claude pasted /compact"),
        }
        true
    }

    /// One read after the paste (review S1, third round F1), on the pane id the paste's pass found. Each outcome
    /// logs one line: sent, erased, or left with the reason. True when a key was typed.
    fn idle_compact_after_paste(&mut self, terminal_id: &TerminalId, pending: Pending) -> bool {
        let public = pending.pane.clone();
        let Some(runtime) = self.terminal_runtimes.get(terminal_id) else {
            tracing::warn!(pane = %public, "nirvana: idle compact lost the pane after the paste; /compact was left unsent");
            return false;
        };
        let line = input_line_of(runtime);
        match after_paste(pending.step, &line, &owner_key_of(&public)) {
            AfterPaste::Enter => {
                let (_, enter) = super::api_helpers::encode_api_submission_parts(
                    runtime,
                    crate::nirvana_idle_compact::COMMAND,
                );
                match write_keys(runtime, enter) {
                    Ok(()) => {
                        tracing::info!(pane = %public, "nirvana: idle Claude sent /compact");
                        true
                    }
                    Err(err) => {
                        tracing::warn!(pane = %public, err = %err, "nirvana: idle compact's Enter failed; /compact was left unsent");
                        false
                    }
                }
            }
            AfterPaste::Next(step) => {
                if let Some(terminal) = self.state.terminals.get_mut(terminal_id) {
                    terminal.idle_stretch.pending = Some(Pending { step, ..pending });
                }
                false
            }
            AfterPaste::Erase => match write_keys(runtime, erase_keys(runtime)) {
                Ok(()) => {
                    if let Some(terminal) = self.state.terminals.get_mut(terminal_id) {
                        terminal.idle_stretch.pending = Some(Pending {
                            step: Step::Erased,
                            ..pending
                        });
                    }
                    true
                }
                Err(err) => {
                    tracing::warn!(pane = %public, err = %err, "nirvana: idle compact could not erase the unsent /compact; it was left in the input line");
                    false
                }
            },
            AfterPaste::Erased => {
                tracing::info!(pane = %public, "nirvana: idle compact erased the unsent /compact");
                false
            }
            AfterPaste::Leave(why) => {
                tracing::warn!(pane = %public, why, "nirvana: idle compact left the input line as it was");
                false
            }
        }
    }

    /// The panes whose terminals idle compact looks at (`idle_compact_terminals`), with their public ids and folders.
    fn claude_panes(&self) -> Vec<ClaudePane> {
        let claude: std::collections::HashSet<&TerminalId> =
            self.idle_compact_terminals().map(|(id, _)| id).collect();
        let mut panes = Vec::new();
        for (ws_idx, ws) in self.state.workspaces.iter().enumerate() {
            for tab in &ws.tabs {
                for (pane_id, pane) in &tab.panes {
                    if !claude.contains(&pane.attached_terminal_id) {
                        continue;
                    }
                    let Some(terminal) = self.state.terminals.get(&pane.attached_terminal_id)
                    else {
                        continue;
                    };
                    let Some(public) = self.public_pane_id(ws_idx, *pane_id) else {
                        continue;
                    };
                    // The folder Claude runs in, as the agents panel reads it (client/shell/agent_sidebar.rs).
                    let folder = tab
                        .foreground_cwd_for_pane(*pane_id, &self.terminal_runtimes)
                        .or_else(|| {
                            tab.cwd_for_pane(
                                *pane_id,
                                &self.state.terminals,
                                &self.terminal_runtimes,
                            )
                        });
                    let hook_session = super::creation::terminal_agent_session_info(terminal)
                        .filter(|session| {
                            session.agent == "claude"
                                && session.kind == crate::agent_resume::AgentSessionRefKind::Id
                        })
                        .map(|session| session.value);
                    panes.push(ClaudePane {
                        terminal_id: pane.attached_terminal_id.clone(),
                        public,
                        folder,
                        hook_session,
                    });
                }
            }
        }
        panes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::AgentState;
    use crate::nirvana_idle_compact::{IdleStretch, ENTER_TRIES, ERASED_AT, ERASE_AT};

    const MIN: Duration = Duration::from_secs(60);
    const RECORDED_EMPTY: &str = include_str!("../nirvana_run/fixtures/claude-2.1.302-empty.ansi");
    const RECORDED_TYPED: &str = include_str!("../nirvana_run/fixtures/claude-2.1.302-typed.ansi");
    /// Wide enough that no recorded line wraps, so screen rows are the recording's lines.
    const COLS: u16 = 200;
    /// The `❯` line's place in the recorded screens, as a cursor move: row 9, the column after the mark.
    const AT_LINE_START: &str = "\x1b[9;3H";

    fn test_app() -> App {
        test_app_with(|_| {})
    }

    fn test_app_with(shape: impl FnOnce(&mut crate::workspace::Workspace)) -> App {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            crate::app::AppPolicy::TEST,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        let mut workspace = crate::workspace::Workspace::test_new("work");
        shape(&mut workspace);
        app.state.workspaces = vec![workspace];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.ensure_test_terminals();
        app
    }

    fn root_terminal_id(app: &App) -> TerminalId {
        let tab = &app.state.workspaces[0].tabs[0];
        tab.panes[&tab.root_pane].attached_terminal_id.clone()
    }

    fn root_public(app: &App) -> String {
        app.public_pane_id(0, app.state.workspaces[0].tabs[0].root_pane)
            .unwrap()
    }

    /// A recorded screen as the pane's terminal shows it, the cursor where Claude leaves it on an empty line.
    fn screen_bytes(recorded: &str) -> Vec<u8> {
        format!("{}{AT_LINE_START}", recorded.replace('\n', "\r\n")).into_bytes()
    }

    /// The pane runs Claude, idle since `since`, with `recorded` on its screen; what is typed lands in the receiver.
    fn idle_claude(
        app: &mut App,
        since: Instant,
        recorded: &str,
    ) -> tokio::sync::mpsc::Receiver<Bytes> {
        let terminal_id = root_terminal_id(app);
        let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
        terminal.detected_agent = Some(Agent::Claude);
        terminal.state = AgentState::Idle;
        terminal.idle_stretch = IdleStretch {
            since: Some(since),
            ..IdleStretch::default()
        };
        let (runtime, receiver) = TerminalRuntime::test_with_channel_and_scrollback_bytes(
            COLS,
            24,
            0,
            &screen_bytes(recorded),
            8,
        );
        app.terminal_runtimes.insert(terminal_id, runtime);
        receiver
    }

    /// Claude has taken the paste: its line now reads `text`.
    fn line_now_reads(app: &App, text: &str) {
        let runtime = app.terminal_runtimes.get(&root_terminal_id(app)).unwrap();
        runtime.test_process_pty_bytes(
            format!("\x1b[9;1H\x1b[0m❯\u{a0}{text}\x1b[K{AT_LINE_START}").as_bytes(),
        );
    }

    /// A long-idle interactive Claude with a big context, as `read_outside` would find it.
    fn long_idle(_query: &PaneQuery) -> Outside {
        Outside {
            argv: Some(vec!["claude".into(), "--model".into(), "opus".into()]),
            transcript_age: Some(55 * MIN),
            context_tokens: Some(80_000),
        }
    }

    /// What reached the pane. The paste is queued on the channel before the call returns, and the Enter's reply is
    /// waited on before it returns, so the channel holds everything typed: no wait is needed here.
    fn typed(receiver: &mut tokio::sync::mpsc::Receiver<Bytes>) -> String {
        let mut out = Vec::new();
        while let Ok(bytes) = receiver.try_recv() {
            out.extend_from_slice(&bytes);
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    /// §2 rule 6, measured: the screens recorded from Claude Code 2.1.302 in a herdr pane, played into a pane
    /// runtime and read back the way the server reads a pane (screen, cursor, scroll).
    #[tokio::test]
    async fn idle_compact_reads_the_recorded_claude_input_line() {
        let read = |recorded: &str| {
            let runtime =
                TerminalRuntime::test_with_screen_bytes(COLS, 24, &screen_bytes(recorded));
            input_line_of(&runtime)
        };
        assert_eq!(read(RECORDED_EMPTY), InputLine::Empty);
        assert_eq!(
            read(RECORDED_TYPED),
            InputLine::Text("hello owner draft".to_owned())
        );
        // Played as recorded, the terminal's cursor is left after the last line, not at the line's start.
        let runtime = TerminalRuntime::test_with_screen_bytes(
            COLS,
            24,
            RECORDED_EMPTY.replace('\n', "\r\n").as_bytes(),
        );
        assert!(matches!(input_line_of(&runtime), InputLine::Unsure(_)));
    }

    /// The instant of the read `step` after a paste at `pasted`.
    fn read_at(pasted: Instant, step: Step) -> Instant {
        Pending {
            pane: String::new(),
            pasted_at: pasted,
            step,
        }
        .due()
        .unwrap()
    }

    /// The test pane's runtime takes Backspace as DEL, the encoding the terminal runtime gives it.
    const BACKSPACE: &str = "\x7f";

    #[tokio::test]
    async fn idle_compact_server_types_compact_once_into_an_idle_claude() {
        let _guard = crate::nirvana::with_test_home("idle-compact-send");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        assert_eq!(app.idle_compact_deadline(), Some(t0 + 50 * MIN));
        // Before the minutes are up nothing is due, and nothing is read.
        let asked = std::cell::Cell::new(0);
        assert!(!app.nirvana_idle_compact_with(t0 + 49 * MIN, |query| {
            asked.set(asked.get() + 1);
            long_idle(query)
        }));
        assert_eq!(asked.get(), 0);
        // The paste: the command, no Enter yet.
        let pasted = t0 + 51 * MIN;
        assert!(app.nirvana_idle_compact_with(pasted, long_idle));
        assert_eq!(typed(&mut receiver), "/compact");
        let terminal_id = root_terminal_id(&app);
        assert!(app.state.terminals[&terminal_id].idle_stretch.sent);
        let enter_at = pasted + ENTER_TRIES[0];
        assert_eq!(app.idle_compact_deadline(), Some(enter_at));
        // The Enter, once the line reads exactly /compact.
        line_now_reads(&app, "/compact");
        assert!(app.nirvana_idle_compact_with(enter_at, long_idle));
        assert_eq!(typed(&mut receiver), "\r");
        // Once per stretch: no deadline, and a later pass types nothing.
        assert_eq!(app.idle_compact_deadline(), None);
        assert!(!app.nirvana_idle_compact_with(t0 + 55 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "");
    }

    /// Third round F1: a slow echo reads `Empty` at the first try and `/compact` at the second, which sends Enter.
    #[tokio::test]
    async fn idle_compact_server_slow_echo_sends_enter_at_a_later_try() {
        let _guard = crate::nirvana::with_test_home("idle-compact-slow-echo");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        let pasted = t0 + 51 * MIN;
        assert!(app.nirvana_idle_compact_with(pasted, long_idle));
        assert_eq!(typed(&mut receiver), "/compact");
        line_now_reads(&app, "");
        assert!(!app.nirvana_idle_compact_with(read_at(pasted, Step::Try(0)), long_idle));
        assert_eq!(typed(&mut receiver), "");
        assert_eq!(
            app.idle_compact_deadline(),
            Some(read_at(pasted, Step::Try(1)))
        );
        line_now_reads(&app, "/compact");
        assert!(app.nirvana_idle_compact_with(read_at(pasted, Step::Try(1)), long_idle));
        assert_eq!(typed(&mut receiver), "\r");
        assert_eq!(app.idle_compact_deadline(), None);
    }

    /// Third round F1: the owner's key between tries ends them, and nothing is erased.
    #[tokio::test]
    async fn idle_compact_server_owner_keys_between_tries_end_them() {
        let _guard = crate::nirvana::with_test_home("idle-compact-owner-between");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        let pasted = t0 + 51 * MIN;
        assert!(app.nirvana_idle_compact_with(pasted, long_idle));
        assert_eq!(typed(&mut receiver), "/compact");
        line_now_reads(&app, "");
        assert!(!app.nirvana_idle_compact_with(read_at(pasted, Step::Try(0)), long_idle));
        crate::nirvana_run::note_owner_key(&root_public(&app));
        line_now_reads(&app, "/compact");
        assert!(!app.nirvana_idle_compact_with(read_at(pasted, Step::Try(1)), long_idle));
        assert_eq!(typed(&mut receiver), "");
        // Ended: no read is due, and the erase's time types nothing.
        assert_eq!(app.idle_compact_deadline(), None);
        assert!(!app.nirvana_idle_compact_with(pasted + ERASE_AT, long_idle));
        assert_eq!(typed(&mut receiver), "");
    }

    /// Third round F1: three tries that read `Empty`, then a line of only the paste: exactly 8 Backspaces, then one
    /// more read, and nothing more.
    #[tokio::test]
    async fn idle_compact_server_erases_a_paste_left_after_the_last_try() {
        let _guard = crate::nirvana::with_test_home("idle-compact-erase");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        let pasted = t0 + 51 * MIN;
        assert!(app.nirvana_idle_compact_with(pasted, long_idle));
        assert_eq!(typed(&mut receiver), "/compact");
        line_now_reads(&app, "");
        for n in 0..ENTER_TRIES.len() {
            assert!(!app.nirvana_idle_compact_with(read_at(pasted, Step::Try(n)), long_idle));
        }
        assert_eq!(typed(&mut receiver), "");
        assert_eq!(app.idle_compact_deadline(), Some(pasted + ERASE_AT));
        line_now_reads(&app, "/compact");
        assert!(app.nirvana_idle_compact_with(pasted + ERASE_AT, long_idle));
        assert_eq!(typed(&mut receiver), BACKSPACE.repeat(8));
        // The read after the erase.
        assert_eq!(app.idle_compact_deadline(), Some(pasted + ERASED_AT));
        line_now_reads(&app, "");
        assert!(!app.nirvana_idle_compact_with(pasted + ERASED_AT, long_idle));
        assert_eq!(typed(&mut receiver), "");
        assert_eq!(app.idle_compact_deadline(), None);
    }

    /// Third round F1: a line holding the owner's text after the paste is never erased, at a try or after the last.
    #[tokio::test]
    async fn idle_compact_server_never_erases_the_owners_text() {
        let _guard = crate::nirvana::with_test_home("idle-compact-no-erase");
        for at_try in [true, false] {
            let mut app = test_app();
            let t0 = Instant::now();
            let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
            let pasted = t0 + 51 * MIN;
            assert!(app.nirvana_idle_compact_with(pasted, long_idle));
            assert_eq!(typed(&mut receiver), "/compact");
            if !at_try {
                line_now_reads(&app, "");
                for n in 0..ENTER_TRIES.len() {
                    app.nirvana_idle_compact_with(read_at(pasted, Step::Try(n)), long_idle);
                }
                assert_eq!(app.idle_compact_deadline(), Some(pasted + ERASE_AT));
            }
            line_now_reads(&app, "/compactfoo");
            let due = app.idle_compact_deadline().unwrap();
            assert!(!app.nirvana_idle_compact_with(due, long_idle));
            assert_eq!(typed(&mut receiver), "", "at a try: {at_try}");
            assert_eq!(app.idle_compact_deadline(), None, "at a try: {at_try}");
        }
    }

    /// Review S1: a line that is neither empty nor exactly `/compact` when the Enter is due gets no Enter, no second
    /// try, and no erase.
    #[tokio::test]
    async fn idle_compact_server_sends_no_enter_unless_the_line_reads_compact() {
        let _guard = crate::nirvana::with_test_home("idle-compact-no-enter");
        for line in ["/compact and more", "hello"] {
            let mut app = test_app();
            let t0 = Instant::now();
            let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
            let pasted = t0 + 51 * MIN;
            assert!(app.nirvana_idle_compact_with(pasted, long_idle));
            assert_eq!(typed(&mut receiver), "/compact");
            line_now_reads(&app, line);
            assert!(!app.nirvana_idle_compact_with(pasted + ENTER_TRIES[0], long_idle));
            assert_eq!(typed(&mut receiver), "", "{line:?}");
            assert_eq!(app.idle_compact_deadline(), None, "{line:?}");
        }
    }

    /// Review S6: a paste that fails to queue (the pane's writer is gone) leaves the stretch unsent.
    #[tokio::test]
    async fn idle_compact_server_unqueued_paste_stays_unsent() {
        let _guard = crate::nirvana::with_test_home("idle-compact-unqueued");
        let mut app = test_app();
        let t0 = Instant::now();
        drop(idle_claude(&mut app, t0, RECORDED_EMPTY));
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        let stretch = app.state.terminals[&root_terminal_id(&app)]
            .idle_stretch
            .clone();
        assert!(!stretch.sent);
        assert_eq!(stretch.pending, None);
        assert_eq!(app.idle_compact_deadline(), Some(t0 + 52 * MIN));
    }

    /// Review S1: the Enter's reply is waited on, so an Enter that fails is known and reported.
    #[tokio::test]
    async fn idle_compact_server_reports_a_failed_enter() {
        let _guard = crate::nirvana::with_test_home("idle-compact-enter-fails");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        assert!(app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "/compact");
        line_now_reads(&app, "/compact");
        drop(receiver);
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN + ENTER_TRIES[0], long_idle));
    }

    /// Review S1: the owner's key inside `OWNER_QUIET_S`, written the way the server writes it, skips the paste.
    #[tokio::test]
    async fn idle_compact_server_skips_while_the_owner_types() {
        let _guard = crate::nirvana::with_test_home("idle-compact-owner");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        crate::nirvana_run::note_owner_key(&root_public(&app));
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "");
        assert!(
            !app.state.terminals[&root_terminal_id(&app)]
                .idle_stretch
                .sent
        );
    }

    /// Review S1: a key after the paste holds the Enter back.
    #[tokio::test]
    async fn idle_compact_server_holds_the_enter_when_the_owner_types() {
        let _guard = crate::nirvana::with_test_home("idle-compact-owner-enter");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        assert!(app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "/compact");
        line_now_reads(&app, "/compact");
        crate::nirvana_run::note_owner_key(&root_public(&app));
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN + ENTER_TRIES[0], long_idle));
        assert_eq!(typed(&mut receiver), "");
    }

    /// Third round nit: the paste's pass keeps the public pane id it found for the reads after it, so they do not
    /// walk the panes' folders again.
    #[tokio::test]
    async fn idle_compact_server_reuses_the_pasted_pane_id() {
        let _guard = crate::nirvana::with_test_home("idle-compact-reuse-id");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        assert!(app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "/compact");
        let pending = app.state.terminals[&root_terminal_id(&app)]
            .idle_stretch
            .pending
            .clone()
            .unwrap();
        assert_eq!(pending.pane, root_public(&app));
        assert_eq!(pending.step, Step::Try(0));
        // The Enter's pass goes by that id: a key noted under it holds the Enter.
        line_now_reads(&app, "/compact");
        crate::nirvana_run::note_owner_key(&pending.pane);
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN + ENTER_TRIES[0], long_idle));
        assert_eq!(typed(&mut receiver), "");
    }

    #[tokio::test]
    async fn idle_compact_server_leaves_a_typed_line_alone_and_rechecks_later() {
        let _guard = crate::nirvana::with_test_home("idle-compact-typed");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_TYPED);
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "");
        // Checked and skipped: due again a minute on, not at once.
        assert_eq!(app.idle_compact_deadline(), Some(t0 + 52 * MIN));
    }

    #[tokio::test]
    async fn idle_compact_server_with_the_setting_off_sends_nothing() {
        let _guard = crate::nirvana::with_test_home("idle-compact-off");
        let mut app = test_app();
        app.idle_compact.enabled = false;
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        assert_eq!(app.idle_compact_deadline(), None);
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "");
    }

    #[tokio::test]
    async fn idle_compact_server_passes_the_facts_it_holds() {
        let _guard = crate::nirvana::with_test_home("idle-compact-query");
        let mut app = test_app();
        let t0 = Instant::now();
        let _receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        let asked = std::cell::RefCell::new(Vec::new());
        app.nirvana_idle_compact_with(t0 + 51 * MIN, |query| {
            asked.borrow_mut().push(query.pane_id.clone());
            Outside::default()
        });
        assert_eq!(asked.into_inner(), vec![root_public(&app)]);
    }

    /// Each fact the server reads goes to `decide`: with any one missing or wrong, nothing is sent.
    #[tokio::test]
    async fn idle_compact_server_sends_nothing_when_one_fact_fails() {
        let _guard = crate::nirvana::with_test_home("idle-compact-facts");
        let cases: [(&str, Outside); 3] = [
            (
                "process unread",
                Outside {
                    argv: None,
                    ..long_idle(&PaneQuery::default())
                },
            ),
            (
                "no transcript",
                Outside {
                    transcript_age: None,
                    ..long_idle(&PaneQuery::default())
                },
            ),
            (
                "no context",
                Outside {
                    context_tokens: None,
                    ..long_idle(&PaneQuery::default())
                },
            ),
        ];
        for (case, outside) in cases {
            let mut app = test_app();
            let t0 = Instant::now();
            let _receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
            assert!(
                !app.nirvana_idle_compact_with(t0 + 51 * MIN, |_| outside.clone()),
                "{case}"
            );
            let terminal_id = root_terminal_id(&app);
            assert!(
                !app.state.terminals[&terminal_id].idle_stretch.sent,
                "{case}"
            );
            // Left alone, and checked again a minute later.
            assert_eq!(app.idle_compact_deadline(), Some(t0 + 52 * MIN), "{case}");
        }
        // A seat worker's pane.
        let mut app = test_app();
        let t0 = Instant::now();
        let _receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        crate::nirvana_subagents::set_worker_panes_for_test(Some(
            [root_public(&app)].into_iter().collect(),
        ));
        let sent = app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle);
        crate::nirvana_subagents::set_worker_panes_for_test(None);
        assert!(!sent);
    }

    /// The pane's state now, not the stretch, says whether it is idle: a stretch the state recompute has not yet
    /// ended (here, a pane gone to a permission prompt) sends nothing.
    #[tokio::test]
    async fn idle_compact_server_reads_the_state_not_a_stale_stretch() {
        let _guard = crate::nirvana::with_test_home("idle-compact-stale");
        for state in [AgentState::Blocked, AgentState::Working] {
            let mut app = test_app();
            let t0 = Instant::now();
            let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
            let terminal_id = root_terminal_id(&app);
            app.state.terminals.get_mut(&terminal_id).unwrap().state = state;
            assert!(
                !app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle),
                "{state:?}"
            );
            assert_eq!(typed(&mut receiver), "", "{state:?}");
        }
    }

    #[tokio::test]
    async fn idle_compact_deadline_reaches_the_server_loop() {
        let _guard = crate::nirvana::with_test_home("idle-compact-loop");
        let mut app = test_app();
        // Idle since 49 minutes 59 seconds before `now`: due in one second, before any of the loop's other deadlines.
        // Built forward from `since`, so no `Instant` is ever subtracted.
        let since = Instant::now();
        let now = since + (50 * MIN - Duration::from_secs(1));
        let _receiver = idle_claude(&mut app, since, RECORDED_EMPTY);
        assert_eq!(
            app.next_headless_loop_deadline_with_git_refresh(now, false, false),
            Some(since + 50 * MIN)
        );
    }

    #[tokio::test]
    async fn idle_compact_deadline_counts_only_claude() {
        let _guard = crate::nirvana::with_test_home("idle-compact-claude");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        let terminal_id = root_terminal_id(&app);
        app.state
            .terminals
            .get_mut(&terminal_id)
            .unwrap()
            .detected_agent = Some(Agent::Codex);
        // Never read, let alone typed into: the stand-in records each call.
        let asked = std::cell::RefCell::new(Vec::new());
        let typed_any = app.nirvana_idle_compact_with(t0 + 51 * MIN, |query| {
            asked.borrow_mut().push(query.pane_id.clone());
            long_idle(query)
        });
        assert_eq!(asked.into_inner(), Vec::<String>::new());
        assert!(!typed_any);
        assert_eq!(typed(&mut receiver), "");
        assert_eq!(app.idle_compact_deadline(), None);
    }

    /// The other panes passed for the shared-folder rule (`choose_transcript`) are the Claude ones only: a shell
    /// beside Claude in the same folder is not another Claude.
    #[tokio::test]
    async fn idle_compact_others_are_only_claude_panes() {
        let _guard = crate::nirvana::with_test_home("idle-compact-others");
        let mut second = None;
        let mut app = test_app_with(|workspace| {
            second = Some(workspace.test_split(ratatui::layout::Direction::Horizontal));
        });
        let second = second.unwrap();
        let t0 = Instant::now();
        let _receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        let others = |app: &mut App| {
            let asked = std::cell::RefCell::new(Vec::new());
            app.nirvana_idle_compact_with(t0 + 51 * MIN, |query| {
                asked
                    .borrow_mut()
                    .extend(query.others.iter().map(|(id, _, _)| id.clone()));
                Outside::default()
            });
            asked.into_inner()
        };
        assert_eq!(others(&mut app), Vec::<String>::new());
        // The second pane runs Claude too (not idle, so never due itself): now it is one of the others.
        let tab = &app.state.workspaces[0].tabs[0];
        let second_terminal = tab.panes[&second].attached_terminal_id.clone();
        let second_public = app.public_pane_id(0, second).unwrap();
        app.state
            .terminals
            .get_mut(&second_terminal)
            .unwrap()
            .detected_agent = Some(Agent::Claude);
        let root = root_terminal_id(&app);
        app.state.terminals.get_mut(&root).unwrap().idle_stretch = IdleStretch {
            since: Some(t0),
            ..IdleStretch::default()
        };
        assert_eq!(others(&mut app), vec![second_public]);
    }

    /// Review S2: a Claude terminal with no pane is stamped, so its deadline moves on instead of staying past.
    #[tokio::test]
    async fn idle_compact_terminal_without_a_pane_gets_a_future_deadline() {
        let _guard = crate::nirvana::with_test_home("idle-compact-paneless");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut terminal = TerminalState::new(TerminalId::alloc(), std::env::temp_dir());
        terminal.detected_agent = Some(Agent::Claude);
        terminal.state = AgentState::Idle;
        terminal.idle_stretch = IdleStretch {
            since: Some(t0),
            ..IdleStretch::default()
        };
        app.state.terminals.insert(terminal.id.clone(), terminal);
        assert_eq!(app.idle_compact_deadline(), Some(t0 + 50 * MIN));
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN, |_| panic!("no pane to read")));
        assert_eq!(app.idle_compact_deadline(), Some(t0 + 52 * MIN));
    }

    /// Review S4: two due terminals, one checked per pass; the other stays due for the next.
    #[tokio::test]
    async fn idle_compact_checks_one_terminal_a_pass() {
        let _guard = crate::nirvana::with_test_home("idle-compact-one");
        let mut app = test_app();
        let t0 = Instant::now();
        let _receiver = idle_claude(&mut app, t0, RECORDED_TYPED);
        let mut other = TerminalState::new(TerminalId::alloc(), std::env::temp_dir());
        other.detected_agent = Some(Agent::Claude);
        other.state = AgentState::Idle;
        other.idle_stretch = IdleStretch {
            since: Some(t0 + MIN),
            ..IdleStretch::default()
        };
        let other_id = other.id.clone();
        app.state.terminals.insert(other_id.clone(), other);
        let root = root_terminal_id(&app);
        let now = t0 + 55 * MIN;
        app.nirvana_idle_compact_with(now, long_idle);
        // The one due first was checked; the other was not touched.
        assert_eq!(
            app.state.terminals[&root].idle_stretch.checked_at,
            Some(now)
        );
        assert_eq!(app.state.terminals[&other_id].idle_stretch.checked_at, None);
        assert_eq!(app.idle_compact_deadline(), Some(t0 + 51 * MIN));
        app.nirvana_idle_compact_with(now, long_idle);
        assert_eq!(
            app.state.terminals[&other_id].idle_stretch.checked_at,
            Some(now)
        );
    }

    #[test]
    fn idle_compact_setting_applies_on_reload() {
        let _guard = crate::nirvana::with_test_home("idle-compact-reload");
        let mut app = test_app();
        assert!(app.idle_compact.rule().is_some());
        let off: crate::config::Config =
            toml::from_str("[ui.idle_compact]\nenabled = false\n").unwrap();
        app.apply_live_config(&off, &[], &[], false);
        assert_eq!(app.idle_compact.rule(), None);
        let bad: crate::config::Config =
            toml::from_str("[ui.idle_compact]\nminutes = 75\n").unwrap();
        let report = app.apply_live_config(&bad, &[], &[], false);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("ui.idle_compact.minutes")));
        assert_eq!(app.idle_compact.rule(), None);
    }

    #[test]
    fn idle_stretch_follows_the_terminal_state() {
        let mut terminal = TerminalState::new(TerminalId::alloc(), std::env::temp_dir());
        let t0 = Instant::now();
        terminal.set_detected_state_with_screen_signals_at(
            Some(Agent::Claude),
            AgentState::Idle,
            false,
            false,
            false,
            false,
            t0,
        );
        assert_eq!(terminal.idle_stretch.since, Some(t0));
        terminal.set_detected_state_with_screen_signals_at(
            Some(Agent::Claude),
            AgentState::Working,
            false,
            false,
            false,
            false,
            t0 + MIN,
        );
        assert_eq!(terminal.idle_stretch, IdleStretch::default());
        terminal.set_detected_state_with_screen_signals_at(
            Some(Agent::Claude),
            AgentState::Idle,
            false,
            false,
            false,
            false,
            t0 + 2 * MIN,
        );
        assert_eq!(terminal.idle_stretch.since, Some(t0 + 2 * MIN));
        // A respawned pane starts with no stretch.
        terminal.clear_agent_runtime_identity_after_respawn();
        assert_eq!(terminal.idle_stretch, IdleStretch::default());
    }
}
