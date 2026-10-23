//! Nirvana fork, plan 27: the server's half of an idle Claude's `/compact`.
//!
//! The rule lives in `nirvana_idle_compact::decide`; this file only gathers its facts and applies the answer. The
//! server's deadline loop wakes at the earliest idle Claude pane's check (`idle_compact_deadline`, read by
//! `next_headless_loop_deadline_with_git_refresh`), so nothing polls. The command is typed the way `herdr agent
//! prompt` types a prompt: pasted, then Enter after a pause.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;

use super::App;
use crate::detect::Agent;
use crate::nirvana_idle_compact::{decide, read_outside, Decision, Facts, Outside, PaneQuery};

/// The pause between the pasted command and its Enter, as `herdr agent prompt` uses
/// (app/api/agents.rs AGENT_PROMPT_SUBMIT_DELAY).
const SUBMIT_DELAY: Duration = Duration::from_millis(300);

/// A pane herdr sees running Claude.
struct ClaudePane {
    terminal_id: crate::terminal::TerminalId,
    public: String,
    folder: Option<PathBuf>,
    hook_session: Option<String>,
}

impl App {
    /// When the next idle Claude pane is due a check, for the server's deadline loop. `None` when the setting is off.
    pub(crate) fn idle_compact_deadline(&self) -> Option<Instant> {
        let rule = self.idle_compact.rule()?;
        self.state
            .terminals
            .values()
            .filter(|terminal| terminal.effective_known_agent() == Some(Agent::Claude))
            .filter_map(|terminal| terminal.idle_stretch.next_check_at(rule.minutes))
            .min()
    }

    /// Checks the idle Claude panes that are due, and types `/compact` where `decide` says to. True when one was sent.
    pub(crate) fn nirvana_idle_compact(&mut self, now: Instant) -> bool {
        if self.idle_compact_deadline().is_none_or(|due| now < due) {
            return false;
        }
        let claude_dir = crate::nirvana_subagents::claude_config_dir();
        let home = crate::nirvana::home();
        self.nirvana_idle_compact_with(now, |query| {
            read_outside(
                query,
                claude_dir.as_deref(),
                home.as_deref(),
                SystemTime::now(),
            )
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
        let panes = self.claude_panes();
        let mut sent = false;
        for pane in &panes {
            let Some(terminal) = self.state.terminals.get_mut(&pane.terminal_id) else {
                continue;
            };
            let stretch = terminal.idle_stretch;
            if stretch
                .next_check_at(rule.minutes)
                .is_none_or(|due| now < due)
            {
                continue;
            }
            // Marked before any read, so a check that fails waits RECHECK instead of running again at once.
            terminal.idle_stretch.checked_at = Some(now);
            let state = terminal.state;
            let agent = terminal.effective_known_agent();
            let runtime = self.terminal_runtimes.get(&pane.terminal_id);
            let prompt_line = runtime
                .and_then(|runtime| crate::nirvana_run::prompt_line_text(&runtime.visible_ansi()));
            let query = PaneQuery {
                pane_id: pane.public.clone(),
                shell_pid: runtime.and_then(crate::terminal::TerminalRuntime::child_pid),
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
                prompt_line,
            };
            let reason = match decide(&facts) {
                Decision::Skip(reason) => reason,
                Decision::Send => {
                    // Marked sent before typing: whatever the send does, it is never typed twice in a stretch.
                    if let Some(terminal) = self.state.terminals.get_mut(&pane.terminal_id) {
                        terminal.idle_stretch.sent = true;
                    }
                    let Some(runtime) = runtime else {
                        continue;
                    };
                    let (text, enter) = super::api_helpers::encode_api_submission_parts(
                        runtime,
                        crate::nirvana_idle_compact::COMMAND,
                    );
                    match runtime.queue_user_input_submission(
                        Bytes::from(text),
                        Bytes::from(enter),
                        SUBMIT_DELAY,
                        None,
                    ) {
                        Ok(_) => {
                            tracing::info!(
                                pane = %pane.public,
                                idle_minutes = idle_for.map_or(0, |idle| idle.as_secs() / 60),
                                tokens = facts.context_tokens.unwrap_or(0),
                                "nirvana: idle Claude sent /compact"
                            );
                            sent = true;
                        }
                        Err(err) => {
                            tracing::warn!(pane = %pane.public, err = %err, "nirvana: idle compact not typed");
                        }
                    }
                    continue;
                }
            };
            tracing::debug!(pane = %pane.public, reason, "nirvana: idle compact left the pane alone");
        }
        sent
    }

    fn claude_panes(&self) -> Vec<ClaudePane> {
        let mut panes = Vec::new();
        for (ws_idx, ws) in self.state.workspaces.iter().enumerate() {
            for tab in &ws.tabs {
                for (pane_id, pane) in &tab.panes {
                    let Some(terminal) = self.state.terminals.get(&pane.attached_terminal_id)
                    else {
                        continue;
                    };
                    if terminal.effective_known_agent() != Some(Agent::Claude) {
                        continue;
                    }
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
    use crate::nirvana_idle_compact::IdleStretch;

    const MIN: Duration = Duration::from_secs(60);
    const RULE: &str = "──────────────────────────────";

    fn test_app() -> App {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            crate::app::AppPolicy::TEST,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("work")];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.ensure_test_terminals();
        app
    }

    fn root_terminal_id(app: &App) -> crate::terminal::TerminalId {
        let tab = &app.state.workspaces[0].tabs[0];
        tab.panes[&tab.root_pane].attached_terminal_id.clone()
    }

    /// The pane runs Claude, idle since `since`, with `screen` on it; what is typed lands in the receiver.
    fn idle_claude(
        app: &mut App,
        since: Instant,
        screen: &str,
    ) -> tokio::sync::mpsc::Receiver<Bytes> {
        let terminal_id = root_terminal_id(app);
        let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
        terminal.detected_agent = Some(Agent::Claude);
        terminal.state = AgentState::Idle;
        terminal.idle_stretch = IdleStretch {
            since: Some(since),
            ..IdleStretch::default()
        };
        let (runtime, receiver) =
            crate::terminal::TerminalRuntime::test_with_channel_and_scrollback_bytes(
                80,
                24,
                0,
                screen.as_bytes(),
                8,
            );
        app.terminal_runtimes.insert(terminal_id, runtime);
        receiver
    }

    fn empty_prompt() -> String {
        // Claude Code 2.1.302's empty prompt: `❯`, a no-break space, its placeholder drawn faint
        // (nirvana_run/fixtures/claude-2.1.302-empty.ansi).
        format!("{RULE}\r\n❯\u{a0}\x1b[2mTry \"fix lint errors\"\x1b[0m\r\n{RULE}\r\n  ? for shortcuts\r\n")
    }

    fn typed_prompt() -> String {
        format!("{RULE}\r\n❯\u{a0}half a thought\r\n{RULE}\r\n  ? for shortcuts\r\n")
    }

    /// A long-idle interactive Claude with a big context, as `read_outside` would find it.
    fn long_idle(_query: &PaneQuery) -> Outside {
        Outside {
            argv: Some(vec!["claude".into(), "--model".into(), "opus".into()]),
            transcript_age: Some(55 * MIN),
            context_tokens: Some(80_000),
        }
    }

    fn typed(receiver: &mut tokio::sync::mpsc::Receiver<Bytes>) -> String {
        // The Enter follows the paste after SUBMIT_DELAY, on the runtime's own writer.
        let until = Instant::now() + Duration::from_secs(1);
        let mut out = Vec::new();
        while Instant::now() < until {
            while let Ok(bytes) = receiver.try_recv() {
                out.extend_from_slice(&bytes);
            }
            if out.ends_with(b"\r") {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    /// §2 rule 6, measured: the screens recorded from Claude Code 2.1.302 in a herdr pane, played into a pane
    /// runtime and read back the way the server reads a pane (`visible_ansi`).
    #[tokio::test]
    async fn idle_compact_reads_the_recorded_claude_input_line() {
        let read = |recorded: &str| {
            let bytes = recorded.replace('\n', "\r\n");
            let runtime =
                crate::terminal::TerminalRuntime::test_with_screen_bytes(140, 24, bytes.as_bytes());
            crate::nirvana_run::prompt_line_text(&runtime.visible_ansi())
        };
        assert_eq!(
            read(include_str!(
                "../nirvana_run/fixtures/claude-2.1.302-empty.ansi"
            )),
            Some(String::new())
        );
        assert_eq!(
            read(include_str!(
                "../nirvana_run/fixtures/claude-2.1.302-typed.ansi"
            )),
            Some("hello owner draft".to_owned())
        );
    }

    #[tokio::test]
    async fn idle_compact_server_types_compact_once_into_an_idle_claude() {
        let _guard = crate::nirvana::with_test_home("idle-compact-send");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, &empty_prompt());
        assert_eq!(app.idle_compact_deadline(), Some(t0 + 50 * MIN));
        // Before the minutes are up nothing is due, and nothing is read.
        assert!(!app.nirvana_idle_compact_with(t0 + 49 * MIN, |_| panic!("read too early")));
        assert!(app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "/compact\r");
        let terminal_id = root_terminal_id(&app);
        assert!(app.state.terminals[&terminal_id].idle_stretch.sent);
        // Once per stretch: no deadline, and a later pass types nothing.
        assert_eq!(app.idle_compact_deadline(), None);
        assert!(!app.nirvana_idle_compact_with(t0 + 55 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "");
    }

    #[tokio::test]
    async fn idle_compact_server_leaves_a_typed_line_alone_and_rechecks_later() {
        let _guard = crate::nirvana::with_test_home("idle-compact-typed");
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, &typed_prompt());
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
        let mut receiver = idle_claude(&mut app, t0, &empty_prompt());
        assert_eq!(app.idle_compact_deadline(), None);
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        assert_eq!(typed(&mut receiver), "");
    }

    #[tokio::test]
    async fn idle_compact_server_passes_the_facts_it_holds() {
        let _guard = crate::nirvana::with_test_home("idle-compact-query");
        let mut app = test_app();
        let t0 = Instant::now();
        let _receiver = idle_claude(&mut app, t0, &empty_prompt());
        let asked = std::cell::RefCell::new(Vec::new());
        app.nirvana_idle_compact_with(t0 + 51 * MIN, |query| {
            asked.borrow_mut().push(query.pane_id.clone());
            Outside::default()
        });
        let public = app
            .public_pane_id(0, app.state.workspaces[0].tabs[0].root_pane)
            .unwrap();
        assert_eq!(asked.into_inner(), vec![public]);
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
            let _receiver = idle_claude(&mut app, t0, &empty_prompt());
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
        // Waiting on a permission prompt: blocked, not idle.
        let mut app = test_app();
        let t0 = Instant::now();
        let _receiver = idle_claude(&mut app, t0, &empty_prompt());
        let terminal_id = root_terminal_id(&app);
        app.state.terminals.get_mut(&terminal_id).unwrap().state = AgentState::Blocked;
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle));
        // A seat worker's pane.
        let mut app = test_app();
        let t0 = Instant::now();
        let _receiver = idle_claude(&mut app, t0, &empty_prompt());
        let public = app
            .public_pane_id(0, app.state.workspaces[0].tabs[0].root_pane)
            .unwrap();
        crate::nirvana_subagents::set_worker_panes_for_test(Some([public].into_iter().collect()));
        let sent = app.nirvana_idle_compact_with(t0 + 51 * MIN, long_idle);
        crate::nirvana_subagents::set_worker_panes_for_test(None);
        assert!(!sent);
    }

    #[tokio::test]
    async fn idle_compact_deadline_reaches_the_server_loop() {
        let _guard = crate::nirvana::with_test_home("idle-compact-loop");
        let mut app = test_app();
        let now = Instant::now();
        // Idle since 49 minutes 59 seconds ago: due in one second, before any of the loop's other deadlines.
        let since = now - (50 * MIN - Duration::from_secs(1));
        let _receiver = idle_claude(&mut app, since, &empty_prompt());
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
        let _receiver = idle_claude(&mut app, t0, &empty_prompt());
        let terminal_id = root_terminal_id(&app);
        app.state
            .terminals
            .get_mut(&terminal_id)
            .unwrap()
            .detected_agent = Some(Agent::Codex);
        assert_eq!(app.idle_compact_deadline(), None);
        // Never read, let alone typed into.
        assert!(!app.nirvana_idle_compact_with(t0 + 51 * MIN, |_| panic!("read a Codex pane")));
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
        let mut terminal = crate::terminal::TerminalState::new(
            crate::terminal::TerminalId::alloc(),
            std::env::temp_dir(),
        );
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
