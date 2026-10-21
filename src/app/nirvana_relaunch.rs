//! Nirvana fork: Nirvana Claude comes back after a reload.
//!
//! The owner, 2026-10-21: "when you click restart we can resume the the process as if we right clicked the folder
//! and clicked open nirvana here. Basical yon restart when an new update is released auto type the commands on the
//! directories." The reload badge stops the server, which saves each pane's folder and agent name; the new server
//! restores the panes as shells. herdr types an agent back only from a session id the agent reported, and Nirvana
//! Claude reports none, so the panes stayed bare shells and the owner opened each folder again by hand.
//!
//! The restore marks each pane that ran Nirvana Claude (`TerminalState::nirvana_relaunch`), and the server's
//! scheduled tasks start Nirvana Claude in it the way the verb does (`nirvana_new.rs`): the folder's generated
//! settings, its last session when that is recent, and the name the pane ran under.

use std::time::{Duration, Instant};

use super::nirvana_new::NirvanaStartError;
use super::App;
use crate::nirvana_launch::NewKind;

/// The step between tries while a restored shell is still starting.
const RETRY_STEP: Duration = Duration::from_millis(250);
/// How long a restored shell may take to come free: `herdr agent start`'s own default
/// (app/agents.rs DEFAULT_AGENT_START_TIMEOUT).
const GIVE_UP_AFTER: Duration = Duration::from_secs(30);

impl App {
    /// Starts Nirvana Claude in the restored panes that ran it. True when one started or was given up.
    pub(crate) fn nirvana_relaunch_restored(&mut self, now: Instant) -> bool {
        let pending: Vec<_> = self
            .state
            .workspaces
            .iter()
            .enumerate()
            .flat_map(|(ws_idx, ws)| {
                ws.tabs
                    .iter()
                    .flat_map(|tab| tab.panes.iter())
                    .map(move |(pane_id, pane)| {
                        (ws_idx, *pane_id, pane.attached_terminal_id.clone())
                    })
            })
            .filter_map(|(ws_idx, pane_id, terminal_id)| {
                let terminal = self.state.terminals.get(&terminal_id)?;
                let name = terminal.nirvana_relaunch.clone()?;
                Some((ws_idx, pane_id, terminal_id, name, terminal.cwd.clone()))
            })
            .collect();
        if pending.is_empty() {
            self.nirvana_relaunch_at = None;
            self.nirvana_relaunch_until = None;
            return false;
        }
        if self.nirvana_relaunch_at.is_some_and(|at| now < at) {
            return false;
        }
        let until = *self
            .nirvana_relaunch_until
            .get_or_insert(now + GIVE_UP_AFTER);

        let mut changed = false;
        let mut waiting = false;
        for (ws_idx, pane_id, terminal_id, name, cwd) in pending {
            let outcome =
                self.nirvana_start_agent(ws_idx, pane_id, NewKind::Nirvana, &cwd, Some(&name));
            let done = match outcome {
                Ok((agent, _)) => {
                    tracing::info!(agent = ?agent.name, folder = %cwd.display(), "nirvana: started again after the reload");
                    true
                }
                // The restored shell is still starting: try again on the next step.
                Err(NirvanaStartError::Start(
                    super::agents::AgentStartError::TargetBusy(_)
                    | super::agents::AgentStartError::TargetUnavailable(_),
                )) if now < until => {
                    waiting = true;
                    false
                }
                Err(err) => {
                    tracing::warn!(agent = %name, folder = %cwd.display(), reason = err.reason(), "nirvana: not started again after the reload");
                    true
                }
            };
            if done {
                if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
                    terminal.nirvana_relaunch = None;
                }
                changed = true;
            }
        }
        self.nirvana_relaunch_at = waiting.then_some(now + RETRY_STEP);
        if !waiting {
            self.nirvana_relaunch_until = None;
        }
        if changed {
            self.schedule_session_save();
        }
        changed
    }
}

impl NirvanaStartError {
    fn reason(&self) -> &'static str {
        use super::agents::AgentStartError;
        match self {
            Self::Refused(code, _) => code,
            Self::Start(AgentStartError::TargetBusy(_)) => "the shell stayed busy",
            Self::Start(AgentStartError::TargetUnavailable(_)) => "the shell never started",
            Self::Start(AgentStartError::DuplicateName { .. }) => "the name is taken",
            Self::Start(_) => "herdr refused the start",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two restored spaces; the second one's pane has a shell runtime with no process behind it, so what the
    /// relaunch types lands in the receiver and no test starts a real Claude.
    fn app_with_two_spaces(
        folder: &std::path::Path,
    ) -> (App, tokio::sync::mpsc::Receiver<bytes::Bytes>) {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            crate::app::AppPolicy::TEST,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![
            crate::workspace::Workspace::test_new("printer"),
            crate::workspace::Workspace::test_new("workout"),
        ];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.ensure_test_terminals();
        for terminal in app.state.terminals.values_mut() {
            terminal.cwd = folder.to_path_buf();
        }
        let terminal_id = root_terminal(&mut app, 1).id.clone();
        let (runtime, receiver) =
            crate::terminal::TerminalRuntime::test_with_channel_capacity(80, 24, 8);
        app.terminal_runtimes.insert(terminal_id, runtime);
        (app, receiver)
    }

    fn root_terminal(app: &mut App, ws_idx: usize) -> &mut crate::terminal::TerminalState {
        let tab = &app.state.workspaces[ws_idx].tabs[0];
        let terminal_id = tab.panes[&tab.root_pane].attached_terminal_id.clone();
        app.state.terminals.get_mut(&terminal_id).unwrap()
    }

    fn typed(receiver: &mut tokio::sync::mpsc::Receiver<bytes::Bytes>) -> String {
        let mut out = Vec::new();
        while let Ok(bytes) = receiver.try_recv() {
            out.extend_from_slice(&bytes);
        }
        let line = String::from_utf8_lossy(&out).into_owned();
        // herdr types a PowerShell `-EncodedCommand` (base64 of UTF-16LE) on Windows; this reads it back.
        let Some((_, encoded)) = line.split_once("-EncodedCommand ") else {
            return line;
        };
        use base64::Engine as _;
        let encoded: String = encoded
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
            .collect();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap_or_default();
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    }

    #[tokio::test]
    async fn nothing_marked_starts_nothing_and_leaves_no_wake() {
        let _guard = crate::nirvana::with_test_home("relaunch-none");
        let (mut app, mut receiver) = app_with_two_spaces(&std::env::temp_dir());
        assert!(!app.nirvana_relaunch_restored(Instant::now()));
        assert_eq!(app.nirvana_relaunch_at, None);
        assert_eq!(typed(&mut receiver), "");
    }

    #[tokio::test]
    async fn a_restored_nirvana_pane_gets_nirvana_claude_under_its_own_name() {
        let _guard = crate::nirvana::with_test_home("relaunch-start");
        let folder = std::env::temp_dir();
        let (mut app, mut receiver) = app_with_two_spaces(&folder);
        root_terminal(&mut app, 1).nirvana_relaunch = Some("nirvana-2".into());
        assert!(app.nirvana_relaunch_restored(Instant::now()));
        let terminal = root_terminal(&mut app, 1);
        assert_eq!(terminal.nirvana_relaunch, None);
        // The name it ran under, though "nirvana" is free: the agents list reads as it did before the reload.
        assert_eq!(terminal.agent_name.as_deref(), Some("nirvana-2"));
        let line = typed(&mut receiver);
        // The verb's argv: the plugin, the agent and the folder's settings, not herdr's plain `claude --resume`.
        for flag in ["--plugin-dir", "--agent", "--settings", "--model"] {
            assert!(line.contains(flag), "{flag} not typed: {line:?}");
        }
        assert_eq!(app.nirvana_relaunch_at, None);
        // Done once: a later tick types nothing more.
        assert!(!app.nirvana_relaunch_restored(Instant::now()));
        assert_eq!(typed(&mut receiver), "");
    }

    #[tokio::test]
    async fn a_marked_pane_whose_shell_never_comes_waits_then_gives_up() {
        let _guard = crate::nirvana::with_test_home("relaunch-wait");
        let (mut app, _receiver) = app_with_two_spaces(&std::env::temp_dir());
        // The first space's pane has no shell runtime: herdr answers "unavailable", a restored shell still starting.
        root_terminal(&mut app, 0).nirvana_relaunch = Some("nirvana".into());
        let now = Instant::now();
        assert!(!app.nirvana_relaunch_restored(now));
        assert_eq!(app.nirvana_relaunch_at, Some(now + RETRY_STEP));
        assert!(root_terminal(&mut app, 0).nirvana_relaunch.is_some());
        // Past the wait it stops trying, so a dead pane never keeps the server waking.
        assert!(app.nirvana_relaunch_restored(now + GIVE_UP_AFTER));
        assert_eq!(root_terminal(&mut app, 0).nirvana_relaunch, None);
        assert_eq!(app.nirvana_relaunch_at, None);
    }
}
