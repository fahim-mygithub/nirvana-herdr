//! Nirvana fork: "Restart Claude" from the tab's right-click menu.
//!
//! The owner, 2026-10-15: "If i right click this tab and I have an option to restart the claude code
//! insstance with that exact directory opened and that session resumed." For each pane in the tab whose
//! foreground job runs Claude Code under the pane's shell: read the conversation id Claude wrote for its
//! pid, stop Claude and its children, wait for the shell to be free, and type `claude <same flags>
//! --resume <id>` into it the way `herdr agent start` does. The shell's folder is the one Claude ran in,
//! because the shell sat waiting on Claude the whole time. The pane, its shell and its herdr agent name stay.

use std::time::{Duration, Instant};

use bytes::Bytes;

use super::App;

/// The poll step while the shell comes free. A process snapshot each step; Claude usually exits within
/// one or two.
const SHELL_POLL: Duration = Duration::from_millis(25);
/// `herdr agent start`'s own default (app/agents.rs DEFAULT_AGENT_START_TIMEOUT).
const MANAGED_START_TIMEOUT: Duration = Duration::from_secs(30);

/// One restarted Claude, for the response and the log.
struct Restarted {
    pane: String,
    session_id: Option<String>,
}

impl App {
    pub(crate) fn nirvana_restart_claude(
        &mut self,
        id: String,
        params: &crate::api::schema::CommandInvokeParams,
    ) -> String {
        use crate::app::api::responses::{encode_error, encode_success};

        let Some(tab_id) = params.tab_id.as_deref() else {
            return encode_error(id, "tab_not_found", "Restart Claude needs a tab");
        };
        let Some((ws_idx, tab_idx)) = self.parse_tab_id(tab_id) else {
            return encode_error(id, "tab_not_found", format!("tab not found: {tab_id}"));
        };
        let panes: Vec<_> = {
            let tab = &self.state.workspaces[ws_idx].tabs[tab_idx];
            tab.layout
                .pane_ids()
                .into_iter()
                .filter_map(|pane_id| {
                    Some((
                        pane_id,
                        tab.panes.get(&pane_id)?.attached_terminal_id.clone(),
                    ))
                })
                .collect()
        };

        let mut restarted = Vec::new();
        let mut failures = Vec::new();
        for (pane_id, terminal_id) in panes {
            match self.nirvana_restart_claude_in_pane(ws_idx, pane_id, &terminal_id) {
                Ok(Some(done)) => restarted.push(done),
                Ok(None) => {}
                Err(err) => failures.push(err),
            }
        }
        if restarted.is_empty() {
            let message = failures
                .first()
                .cloned()
                .unwrap_or_else(|| "no Claude Code is running in this tab".into());
            return encode_error(id, "claude_not_restarted", message);
        }
        self.state.mark_session_dirty();
        self.schedule_session_save();
        for done in &restarted {
            tracing::info!(
                pane = %done.pane,
                session = done.session_id.as_deref().unwrap_or("(none: --continue)"),
                "nirvana: restarted Claude"
            );
        }
        encode_success(id, crate::api::schema::ResponseResult::Ok {})
    }

    /// `Ok(None)`: the pane does not run Claude under a shell, so it is left alone.
    fn nirvana_restart_claude_in_pane(
        &mut self,
        ws_idx: usize,
        pane_id: crate::layout::PaneId,
        terminal_id: &crate::terminal::TerminalId,
    ) -> Result<Option<Restarted>, String> {
        let Some(shell_pid) = self
            .terminal_runtimes
            .get(terminal_id)
            .and_then(|runtime| runtime.child_pid())
        else {
            return Ok(None);
        };
        let Some(job) = crate::detect::foreground_job(shell_pid) else {
            return Ok(None);
        };
        let Some((crate::detect::Agent::Claude, _, claude_pid)) =
            crate::detect::identify_agent_process_in_job(&job)
        else {
            return Ok(None);
        };
        if claude_pid == shell_pid {
            // Claude is the pane's own program: stopping it would close the pane.
            return Ok(None);
        }
        let old_args: Vec<String> = job
            .processes
            .iter()
            .find(|process| process.pid == claude_pid)
            .and_then(|process| process.argv.clone())
            .map(|argv| argv.into_iter().skip(1).collect())
            .unwrap_or_default();
        let config_dir = crate::nirvana::claude_config_dir();
        let session = config_dir
            .as_deref()
            .and_then(|dir| crate::nirvana::read_claude_session(dir, claude_pid));
        let session_id = session.map(|session| session.session_id);
        // A Claude with no prompt yet has no saved conversation to resume.
        let unsaved = match (&session_id, config_dir.as_deref()) {
            (Some(id), Some(dir)) => !crate::nirvana::conversation_saved(dir, id),
            _ => false,
        };

        let mut pids = crate::platform::session_processes(claude_pid);
        if pids.is_empty() {
            pids.push(claude_pid);
        }
        crate::nirvana::terminate_processes(&pids);
        let deadline = Instant::now() + crate::nirvana::RESTART_SHELL_WAIT;
        let shell_name = loop {
            if let Some(name) = crate::platform::available_pane_shell(shell_pid) {
                break name;
            }
            if Instant::now() >= deadline {
                return Err(if crate::platform::process_exists(claude_pid) {
                    format!("Claude (pid {claude_pid}) could not be stopped")
                } else {
                    "Claude stopped but the pane's shell was not free within 3 s".into()
                });
            }
            std::thread::sleep(SHELL_POLL);
        };

        let (agent_name, cwd) = self
            .state
            .terminals
            .get(terminal_id)
            .map(|t| (t.agent_name.clone(), t.cwd.clone()))
            .ok_or("the pane closed while Claude was stopping")?;
        let home = crate::nirvana::home().unwrap_or_else(std::env::temp_dir);
        let argv = restart_argv(
            agent_name.as_deref(),
            &old_args,
            session_id.as_deref(),
            unsaved,
            &home,
            std::path::Path::new(&cwd),
        );
        let command = crate::platform::interactive_shell_command(&argv, &shell_name)
            .ok_or("the restart command could not be written for this shell")?;
        let runtime = self
            .terminal_runtimes
            .get(terminal_id)
            .ok_or("the pane closed while Claude was stopping")?;
        let bytes = crate::nirvana::shell_submission(&command);

        let terminal = self
            .state
            .terminals
            .get_mut(terminal_id)
            .ok_or("the pane closed while Claude was stopping")?;
        let name = terminal.agent_name.clone();
        terminal.clear_agent_runtime_identity_after_respawn();
        if let Some(name) = name {
            // Keep the herdr agent name, as `herdr agent start` gave it.
            terminal.begin_managed_agent(
                name,
                crate::detect::Agent::Claude,
                Instant::now(),
                super::agents::AGENT_START_SETTLE_DELAY,
                MANAGED_START_TIMEOUT,
            );
        }
        runtime
            .try_send_bytes(Bytes::from(bytes))
            .map_err(|err| format!("could not type the restart command: {err}"))?;
        self.emit_pane_updated(ws_idx, pane_id);
        Ok(Some(Restarted {
            pane: format!("{}", pane_id.raw()),
            session_id,
        }))
    }
}

/// Plan 20 §3.2: Restart Claude resumes Nirvana Claude through launch_args.
pub fn restart_argv(
    agent_name: Option<&str>,
    old_args: &[String],
    session_id: Option<&str>,
    unsaved: bool,
    home: &std::path::Path,
    cwd: &std::path::Path,
) -> Vec<String> {
    let mut argv = vec![
        crate::detect::interactive_agent_executable(crate::detect::Agent::Claude).to_string(),
    ];
    let is_nirvana = agent_name.is_some_and(crate::nirvana_launch::is_nirvana_agent_name)
        || old_args.iter().any(|a| a == "--agent") && old_args.iter().any(|a| a == "nirvana");
    if is_nirvana {
        let mut launch = if unsaved {
            crate::nirvana_launch::launch_args(home, cwd, None)
        } else if let Some(id) = session_id {
            crate::nirvana_launch::launch_args(home, cwd, Some(id))
        } else {
            let mut args = crate::nirvana_launch::launch_args(home, cwd, None);
            args.push("--continue".to_string());
            args
        };
        argv.append(&mut launch);
    } else {
        argv.extend(if unsaved {
            crate::nirvana::restart_args_unsaved(old_args)
        } else {
            crate::nirvana::restart_args(old_args, session_id)
        });
    }
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_argv_for_nirvana_claude_uses_launch_args_and_takes_current_mode() {
        let home = std::env::temp_dir().join(format!("test-restart-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let nirvana_dir = home.join("nirvana");
        std::fs::create_dir_all(&nirvana_dir).unwrap();
        // Set mode = "efficient" in seats.toml
        std::fs::write(
            nirvana_dir.join("seats.toml"),
            crate::nirvana_seat::DEFAULT.replace("mode = \"balanced\"", "mode = \"efficient\""),
        )
        .unwrap();

        let old_args = vec![
            "--agent".to_string(),
            "nirvana".to_string(),
            "--model".to_string(),
            "claude-opus-5-5".to_string(),
            "--effort".to_string(),
            "medium".to_string(),
        ];
        let cwd = std::path::Path::new(r"C:\work\repo");
        let argv = restart_argv(
            Some("nirvana"),
            &old_args,
            Some("sess-456"),
            false,
            &home,
            cwd,
        );
        // Should use launch_args with rule's seat (sonnet medium for efficient), not the old opus
        let model_idx = argv.iter().position(|a| a == "--model").unwrap();
        assert_eq!(argv[model_idx + 1], "claude-sonnet-5-5");
        let effort_idx = argv.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(argv[effort_idx + 1], "medium");
        // And carries --resume sess-456
        let resume_idx = argv.iter().position(|a| a == "--resume").unwrap();
        assert_eq!(argv[resume_idx + 1], "sess-456");

        // For plain claude, keeps old args
        let plain_old = vec![
            "--model".to_string(),
            "claude-opus-5-5".to_string(),
            "--effort".to_string(),
            "medium".to_string(),
        ];
        let plain_argv = restart_argv(
            Some("claude"),
            &plain_old,
            Some("sess-456"),
            false,
            &home,
            cwd,
        );
        let plain_model_idx = plain_argv.iter().position(|a| a == "--model").unwrap();
        assert_eq!(plain_argv[plain_model_idx + 1], "claude-opus-5-5");

        let _ = std::fs::remove_dir_all(&home);
    }

    fn app_with_a_tab() -> App {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            crate::app::AppPolicy::TEST,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("restart")];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.ensure_test_terminals();
        app
    }

    fn invoke(app: &mut App, tab_id: &str) -> String {
        app.handle_command_invoke(
            "req".into(),
            crate::api::schema::CommandInvokeParams {
                command_id: crate::nirvana::RESTART_CLAUDE_COMMAND.into(),
                workspace_id: None,
                tab_id: Some(tab_id.into()),
                pane_id: None,
                selection: None,
            },
        )
    }

    #[test]
    fn restart_claude_reaches_the_server_and_stops_nothing_without_claude() {
        let mut app = app_with_a_tab();
        let tab_id = app.public_tab_id(0, 0).unwrap();
        // Taken before herdr's custom-command lookup, which would answer "command_not_found".
        let error: crate::api::schema::ErrorResponse =
            serde_json::from_str(&invoke(&mut app, &tab_id)).unwrap();
        assert_eq!(error.error.code, "claude_not_restarted");
        let error: crate::api::schema::ErrorResponse =
            serde_json::from_str(&invoke(&mut app, "tab_missing")).unwrap();
        assert_eq!(error.error.code, "tab_not_found");
    }
}
