//! Nirvana fork, plan 12 O4a: the server half of the sidebar's "new" menu and of the verb's window.
//!
//! `nirvana.new:<kind>` (the menu, L7): a new space on the folder a plain "new" would use, with that kind's agent
//! started in its root pane the way `herdr agent start` starts one, so Restart Claude and the agent list treat it
//! like any other. Only Nirvana gets the plugin argv and `NIRVANA_ORCHESTRATOR=1`; Claude Code and Antigravity are
//! typed as `claude` and `agy` alone.
//!
//! `nirvana.open:[fresh:]<folder>` (the verb, L5): focus the space where Nirvana Claude already runs on the folder,
//! as Nirvana-C's second click returned to the same Claude; otherwise open one. `fresh` means the window's own
//! click started this server, which then seeded one shell space on the folder from its start folder; that space
//! takes Nirvana Claude instead of a second space opening beside it. Nirvana Claude there gets its
//! `NIRVANA_ORCHESTRATOR=1` from the plugin's settings.json `env`, since that shell was started without it.
//!
//! Both ride herdr's existing `command.invoke`, so the API stays upstream's.
//!
//! Plan 13 P1: Nirvana Claude starts with the folder's generated `--settings` file when the window (or `herdr
//! preload apply`) wrote one, and resumes the folder's last session when its record is recent and its transcript is
//! there, unless another Nirvana Claude already runs on the folder: two Claudes on one transcript would both write
//! it. The menu's "Fresh Nirvana Claude" never resumes. The server never computes a preset itself: the walk and Jev
//! would hold this event loop, so the menu takes the file the last open wrote.

use std::path::{Path, PathBuf};

use super::App;
use crate::nirvana_launch::NewKind;

/// Why the verb's start did not type the agent.
pub(super) enum NirvanaStartError {
    /// Before herdr's start: no pane, or no Nirvana home. The response's code and message.
    Refused(&'static str, String),
    /// herdr's own `agent start` refused it.
    Start(super::agents::AgentStartError),
}

impl App {
    pub(crate) fn nirvana_new_space(
        &mut self,
        id: String,
        params: &crate::api::schema::CommandInvokeParams,
        key: &str,
    ) -> String {
        use crate::app::api::responses::encode_error;

        let Some(kind) = NewKind::from_key(key) else {
            return encode_error(id, "unknown_agent_kind", format!("no new-menu entry {key}"));
        };
        let source = params
            .workspace_id
            .as_deref()
            .and_then(|workspace_id| self.parse_workspace_id(workspace_id))
            .or_else(|| self.workspace_creation_source());
        let cwd = source.map_or_else(
            || self.resolve_new_terminal_cwd(None),
            |index| self.resolved_new_workspace_cwd_from(index),
        );
        self.nirvana_open_space(id, kind, cwd)
    }

    fn mark_space_unseen_if_unfocused(&mut self, ws_idx: usize, new_space: bool) {
        if new_space && self.state.outer_terminal_focus == Some(false) {
            if let Some(ws) = self.state.workspaces.get_mut(ws_idx) {
                let root_pane = ws.tabs[ws.active_tab].root_pane;
                if let Some(pane) = ws.pane_state_mut(root_pane) {
                    pane.seen = false;
                }
            }
        }
    }

    pub(crate) fn nirvana_open_folder(&mut self, id: String, rest: &str) -> String {
        use crate::app::api::responses::{encode_error, encode_success};

        let (folder, fresh, new_space) = crate::nirvana_launch::parse_open_command(rest);
        if !folder.is_absolute() {
            return encode_error(
                id,
                "invalid_folder",
                format!("not an absolute folder: {rest}"),
            );
        }
        if let Some(ws_idx) = self.nirvana_space_on(&folder) {
            self.state.switch_workspace(ws_idx);
            self.state.mode = crate::app::Mode::Terminal;
            self.mark_space_unseen_if_unfocused(ws_idx, new_space);
            tracing::info!(folder = %folder.display(), "nirvana: focused Nirvana Claude on the folder");
            return encode_success(id, crate::api::schema::ResponseResult::Ok {});
        }
        if fresh {
            if let Some((ws_idx, pane_id)) = self.nirvana_seeded_space(&folder) {
                self.state.switch_workspace(ws_idx);
                self.state.mode = crate::app::Mode::Terminal;
                self.mark_space_unseen_if_unfocused(ws_idx, new_space);
                return self.nirvana_start_response(id, ws_idx, pane_id, NewKind::Nirvana, &folder);
            }
        }
        let res = self.nirvana_open_space(id, NewKind::Nirvana, folder);
        if let Some(ws_idx) = self.state.active {
            self.mark_space_unseen_if_unfocused(ws_idx, new_space);
        }
        res
    }

    fn nirvana_open_space(&mut self, id: String, kind: NewKind, cwd: PathBuf) -> String {
        use crate::app::api::responses::encode_error;

        let ws_idx = match self.create_workspace_with_launch_env(cwd.clone(), true, kind.env()) {
            Ok(ws_idx) => ws_idx,
            Err(err) => return encode_error(id, "workspace_create_failed", err.to_string()),
        };
        self.emit_workspace_open_events(ws_idx);
        let pane_id = self.state.workspaces[ws_idx].tabs[0].root_pane;
        self.nirvana_start_response(id, ws_idx, pane_id, kind, &cwd)
    }

    /// Types the kind's agent into the pane's shell, as `herdr agent start` does, under a free name.
    fn nirvana_start_response(
        &mut self,
        id: String,
        ws_idx: usize,
        pane_id: crate::layout::PaneId,
        kind: NewKind,
        folder: &Path,
    ) -> String {
        use crate::app::api::responses::{encode_error, encode_success};

        match self.nirvana_start_agent(ws_idx, pane_id, kind, folder, None) {
            Ok((agent, argv)) => {
                tracing::info!(kind = kind.key(), agent = ?agent.name, "nirvana: started from the new menu or the verb");
                encode_success(
                    id,
                    crate::api::schema::ResponseResult::AgentStarted { agent, argv },
                )
            }
            Err(NirvanaStartError::Refused(code, message)) => encode_error(id, code, message),
            Err(NirvanaStartError::Start(err)) => {
                let body = self.agent_start_error_body(err);
                encode_error(id, &body.code, body.message)
            }
        }
    }

    /// The verb's start: the kind's agent typed into the pane's shell under `name` when it is free, else the
    /// first free name from the kind's stem. A restored pane passes the name it ran under (`nirvana_relaunch.rs`).
    pub(super) fn nirvana_start_agent(
        &mut self,
        ws_idx: usize,
        pane_id: crate::layout::PaneId,
        kind: NewKind,
        folder: &Path,
        name: Option<&str>,
    ) -> Result<(crate::api::schema::AgentInfo, Vec<String>), NirvanaStartError> {
        let Some(pane) = self.public_pane_id(ws_idx, pane_id) else {
            return Err(NirvanaStartError::Refused(
                "agent_pane_not_found",
                "the new space has no pane".into(),
            ));
        };
        let home = match kind {
            kind if kind.is_nirvana() => match crate::nirvana::home() {
                Some(home) => home,
                None => {
                    return Err(NirvanaStartError::Refused(
                        "no_nirvana_home",
                        "neither NIRVANA_HERDR_HOME nor LOCALAPPDATA is set".into(),
                    ))
                }
            },
            _ => PathBuf::new(),
        };
        let taken = |name: &str| {
            self.state
                .terminals
                .values()
                .any(|terminal| terminal.agent_name.as_deref() == Some(name))
        };
        let name = match name.filter(|name| !taken(name)) {
            Some(name) => name.to_owned(),
            None => crate::nirvana_launch::free_agent_name(kind.name_stem(), taken),
        };
        let params = crate::api::schema::AgentStartParams {
            name,
            kind: kind.agent_label().into(),
            pane_id: pane,
            args: kind.args(&home),
            timeout_ms: None,
        };
        // Plan 13: Nirvana Claude's argv with the folder's generated settings, and the resume when there is one.
        let args = match kind {
            NewKind::Nirvana => {
                let resume = self.nirvana_resume(&home, folder);
                Some(crate::nirvana_launch::launch_args(
                    &home,
                    folder,
                    resume.as_deref(),
                ))
            }
            NewKind::NirvanaFresh => Some(crate::nirvana_launch::launch_args(&home, folder, None)),
            NewKind::ClaudeCode | NewKind::Antigravity => None,
        };
        let params = match args {
            Some(args) => crate::api::schema::AgentStartParams { args, ..params },
            None => params,
        };
        self.start_agent(params).map_err(NirvanaStartError::Start)
    }

    /// The folder's last session to resume, unless a Nirvana Claude already runs on the folder: two Claudes on one
    /// transcript would both write it. The pane being started has no agent yet, so one found is another.
    fn nirvana_resume(&self, home: &Path, folder: &Path) -> Option<String> {
        if self.nirvana_space_on(folder).is_some() {
            return None;
        }
        crate::nirvana_launch::resume_session(home, folder)
    }

    /// The space whose pane runs Nirvana Claude on `folder` (live or still starting).
    fn nirvana_space_on(&self, folder: &Path) -> Option<usize> {
        let key = crate::nirvana_launch::folder_key(folder);
        self.state
            .workspaces
            .iter()
            .enumerate()
            .find_map(|(ws_idx, ws)| {
                ws.tabs
                    .iter()
                    .flat_map(|tab| tab.panes.values())
                    .filter_map(|pane| self.state.terminals.get(&pane.attached_terminal_id))
                    .any(|terminal| {
                        terminal
                            .agent_name
                            .as_deref()
                            .is_some_and(crate::nirvana_launch::is_nirvana_agent_name)
                            && (terminal.is_agent_terminal()
                                || terminal.managed_agent_kind().is_some())
                            && crate::nirvana_launch::folder_key(&terminal.cwd) == key
                    })
                    .then_some(ws_idx)
            })
    }

    /// The fresh server's only space, when it is the untouched shell it seeded on `folder`: one tab, one pane, no
    /// agent.
    fn nirvana_seeded_space(&self, folder: &Path) -> Option<(usize, crate::layout::PaneId)> {
        let [ws] = self.state.workspaces.as_slice() else {
            return None;
        };
        let [tab] = ws.tabs.as_slice() else {
            return None;
        };
        if tab.panes.len() != 1 {
            return None;
        }
        let pane_id = tab.root_pane;
        let terminal = self
            .state
            .terminals
            .get(&tab.panes.get(&pane_id)?.attached_terminal_id)?;
        let untouched = terminal.agent_name.is_none()
            && !terminal.is_agent_terminal()
            && terminal.managed_agent_kind().is_none();
        (untouched
            && crate::nirvana_launch::folder_key(&terminal.cwd)
                == crate::nirvana_launch::folder_key(folder))
        .then_some((0, pane_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_a_space(cwd: &Path) -> App {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            crate::app::AppPolicy::TEST,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        // A shell that cannot start: a regression that reaches a real new space fails to spawn instead of typing
        // `claude` into a real shell.
        app.state.default_shell = std::env::temp_dir()
            .join("nirvana-o4a-no-such-shell.exe")
            .display()
            .to_string();
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("seeded")];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.ensure_test_terminals();
        for terminal in app.state.terminals.values_mut() {
            terminal.cwd = cwd.to_path_buf();
        }
        // A runtime with no process behind it: what is typed into the shell lands in the receiver, never in a real
        // shell, so no test starts a real Claude.
        let terminal_id = app.state.workspaces[0].tabs[0].panes
            [&app.state.workspaces[0].tabs[0].root_pane]
            .attached_terminal_id
            .clone();
        let (runtime, receiver) =
            crate::terminal::TerminalRuntime::test_with_channel_capacity(80, 24, 8);
        app.terminal_runtimes.insert(terminal_id, runtime);
        RECEIVERS.with(|r| r.borrow_mut().push(receiver));
        app
    }

    thread_local! {
        static RECEIVERS: std::cell::RefCell<Vec<tokio::sync::mpsc::Receiver<bytes::Bytes>>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }

    /// Everything typed into the last test app's pane.
    fn typed() -> String {
        RECEIVERS.with(|r| {
            let mut receivers = r.borrow_mut();
            let receiver = receivers.last_mut().expect("a test app");
            let mut out = Vec::new();
            while let Ok(bytes) = receiver.try_recv() {
                out.extend_from_slice(&bytes);
            }
            decode_powershell(&String::from_utf8_lossy(&out))
        })
    }

    /// herdr types a PowerShell `-EncodedCommand` (base64 of UTF-16LE) on Windows; this reads it back.
    fn decode_powershell(line: &str) -> String {
        use base64::Engine as _;
        let Some((_, encoded)) = line.split_once("-EncodedCommand ") else {
            return line.to_owned();
        };
        let encoded = encoded.split_whitespace().next().unwrap_or_default();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("base64");
        let wide: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&wide)
    }

    fn started(response: &str) -> (Option<String>, Vec<String>) {
        let value: serde_json::Value = serde_json::from_str(response).unwrap();
        let result = &value["result"];
        assert!(value.get("error").is_none(), "{response}");
        let argv = result["argv"]
            .as_array()
            .unwrap_or_else(|| panic!("no argv in {response}"))
            .iter()
            .map(|arg| arg.as_str().unwrap().to_owned())
            .collect();
        (result["agent"]["name"].as_str().map(str::to_owned), argv)
    }

    #[tokio::test]
    async fn nirvana_starts_with_the_plugin_argv_and_plain_kinds_with_none() {
        let _guard = crate::nirvana::with_test_home("starts");
        let home = _guard.path.clone();
        let folder = std::env::temp_dir();
        let mut app = app_with_a_space(&folder);
        let pane = app.state.workspaces[0].tabs[0].root_pane;
        let response = app.nirvana_start_response("n".into(), 0, pane, NewKind::Nirvana, &folder);
        let (name, argv) = started(&response);
        assert_eq!(name.as_deref(), Some("nirvana"));
        assert_eq!(argv[0], "claude");
        assert_eq!(argv[1..], NewKind::Nirvana.args(&home)[..]);
        let line = typed();
        // O4b: `--append-system-prompt-file` carries the role; the `--agent` file has no prompt of its own.
        for flag in [
            "--plugin-dir",
            "--agent",
            "--model",
            "--effort",
            "--append-system-prompt-file",
            "--name",
            "--settings",
            "--rc",
            "--chrome",
        ] {
            assert!(line.contains(flag), "{flag} not typed: {line:?}");
        }

        for kind in [NewKind::ClaudeCode, NewKind::Antigravity] {
            let mut app = app_with_a_space(&folder);
            let pane = app.state.workspaces[0].tabs[0].root_pane;
            let response = app.nirvana_start_response("p".into(), 0, pane, kind, &folder);
            let (name, argv) = started(&response);
            assert_eq!(name.as_deref(), Some(kind.name_stem()));
            // Exactly the program, nothing after it: what today's fork types for that kind.
            assert_eq!(
                argv,
                vec![crate::detect::interactive_agent_executable(
                    crate::detect::parse_agent_label(kind.agent_label()).unwrap()
                )
                .to_string()]
            );
            let line = typed();
            assert!(
                !line.contains("--plugin-dir") && !line.contains("nirvana"),
                "{line:?}"
            );
        }
    }

    /// Plan 13: the folder's generated settings and its last session, from a scratch home.
    #[tokio::test]
    async fn nirvana_resumes_the_folders_last_session_with_its_preset_file_and_fresh_never_resumes()
    {
        let _guard = crate::nirvana::with_test_home("resumes");
        let home = _guard.path.clone();
        let folder = std::env::temp_dir();
        let generated = crate::nirvana_preset::settings_path(&home.join("nirvana"), &folder);
        std::fs::create_dir_all(generated.parent().unwrap()).unwrap();
        std::fs::write(&generated, r#"{"skillOverrides":{"docx":"off"}}"#).unwrap();
        let encoded_folder = crate::nirvana_launch::encode_project_folder(&folder);
        let transcript_dir = home.join(encoded_folder);
        std::fs::create_dir_all(&transcript_dir).unwrap();
        let transcript = transcript_dir.join("0c0ffee0-1111-4222-8333-444455556666.jsonl");
        std::fs::write(&transcript, "{}\n").unwrap();
        let record = crate::nirvana_launch::session_record_path(&home, &folder);
        std::fs::create_dir_all(record.parent().unwrap()).unwrap();
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        std::fs::write(
            &record,
            serde_json::json!({"session_id": "0c0ffee0-1111-4222-8333-444455556666", "at": now, "transcript": transcript})
                .to_string(),
        )
        .unwrap();
        let start = |kind: NewKind| {
            let mut app = app_with_a_space(&folder);
            let pane = app.state.workspaces[0].tabs[0].root_pane;
            let response = app.nirvana_start_response("n".into(), 0, pane, kind, &folder);
            (app, started(&response).1)
        };
        let (app, resumed) = start(NewKind::Nirvana);
        let (_, fresh) = start(NewKind::NirvanaFresh);
        // A second Nirvana Claude on a folder that already has one: two Claudes on one transcript would both
        // write it, so it starts fresh. This app's shell cannot start a second space, so the rule is read where
        // the start reads it.
        assert_eq!(app.nirvana_space_on(&folder), Some(0));
        let second = app.nirvana_resume(&home, &folder);
        let first = app_with_a_space(&folder).nirvana_resume(&home, &folder);
        assert_eq!(
            first.as_deref(),
            Some("0c0ffee0-1111-4222-8333-444455556666")
        );
        let settings_of = |argv: &[String]| {
            let at = argv.iter().position(|a| a == "--settings").unwrap();
            argv[at + 1].clone()
        };
        // Plan 28: the test home's seat is Opus, so the generated file reaches Claude as the launch file built from
        // it, with superpowers off.
        let launch = crate::nirvana_superpowers::launch_settings_path(
            &home.join("nirvana"),
            &folder,
            crate::nirvana_superpowers::Family::Opus,
        );
        assert_eq!(settings_of(&resumed), launch.display().to_string());
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&launch).unwrap()).unwrap();
        assert_eq!(written["skillOverrides"]["docx"], "off", "{written}");
        assert_eq!(
            resumed[resumed.len() - 2..],
            ["--resume", "0c0ffee0-1111-4222-8333-444455556666"]
        );
        assert_eq!(settings_of(&fresh), launch.display().to_string());
        assert!(!fresh.iter().any(|a| a == "--resume"), "{fresh:?}");
        assert_eq!(second, None);
    }

    #[tokio::test]
    async fn the_verbs_second_click_focuses_the_nirvana_already_on_the_folder() {
        let _guard = crate::nirvana::with_test_home("second_click");
        let folder = std::env::temp_dir();
        let mut app = app_with_a_space(&folder);
        // The fresh server's seeded shell takes Nirvana Claude instead of a second space.
        assert!(app.nirvana_seeded_space(&folder).is_some());
        let pane = app.state.workspaces[0].tabs[0].root_pane;
        let _ = app.nirvana_start_response("n".into(), 0, pane, NewKind::Nirvana, &folder);
        assert_eq!(app.nirvana_space_on(&folder), Some(0));
        assert_eq!(
            app.nirvana_seeded_space(&folder),
            None,
            "no longer untouched"
        );
        // Same folder, other case and separators: still the same space.
        let other = PathBuf::from(
            folder
                .display()
                .to_string()
                .to_uppercase()
                .replace('\\', "/"),
        );
        if cfg!(windows) {
            assert_eq!(app.nirvana_space_on(&other), Some(0));
        }
        let response = app.nirvana_open_folder(
            "o".into(),
            &crate::nirvana_launch::open_command_id(&folder, false, false)
                [crate::nirvana_launch::OPEN_COMMAND.len()..],
        );
        assert!(response.contains("\"ok\""), "{response}");
        assert_eq!(app.state.workspaces.len(), 1, "focused, not opened again");
        // A plain Claude on the folder is not Nirvana's.
        let mut plain = app_with_a_space(&folder);
        let pane = plain.state.workspaces[0].tabs[0].root_pane;
        let _ = plain.nirvana_start_response("c".into(), 0, pane, NewKind::ClaudeCode, &folder);
        assert_eq!(plain.nirvana_space_on(&folder), None);
    }

    #[tokio::test]
    async fn open_with_new_space_flag_and_unfocused_terminal_leaves_root_pane_unseen() {
        let _guard = crate::nirvana::with_test_home("unseen_open");
        let folder = std::env::temp_dir();
        let mut app = app_with_a_space(&folder);
        let pane = app.state.workspaces[0].tabs[0].root_pane;
        let _ = app.nirvana_start_response("n".into(), 0, pane, NewKind::Nirvana, &folder);

        // With new_space = true and outer_terminal_focus = Some(false): pane is unseen
        app.state.outer_terminal_focus = Some(false);
        let cmd = crate::nirvana_launch::open_command_id(&folder, false, true);
        let rest = &cmd[crate::nirvana_launch::OPEN_COMMAND.len()..];
        let _ = app.nirvana_open_folder("o1".into(), rest);
        assert!(!app.state.workspaces[0].panes[&pane].seen);

        // With new_space = true and outer_terminal_focus = Some(true): pane is seen
        app.state.outer_terminal_focus = Some(true);
        let _ = app.nirvana_open_folder("o2".into(), rest);
        assert!(app.state.workspaces[0].panes[&pane].seen);

        // With new_space = false and outer_terminal_focus = Some(false): pane is seen
        app.state.outer_terminal_focus = Some(false);
        let cmd_no_new = crate::nirvana_launch::open_command_id(&folder, false, false);
        let rest_no_new = &cmd_no_new[crate::nirvana_launch::OPEN_COMMAND.len()..];
        let _ = app.nirvana_open_folder("o3".into(), rest_no_new);
        assert!(app.state.workspaces[0].panes[&pane].seen);
    }

    #[tokio::test]
    async fn a_relative_folder_and_an_unknown_kind_open_nothing() {
        let _guard = crate::nirvana::with_test_home("relative");
        let mut app = app_with_a_space(&std::env::temp_dir());
        let response = app.nirvana_open_folder("o".into(), "relative/folder");
        assert!(response.contains("invalid_folder"), "{response}");
        let response = app.handle_command_invoke(
            "k".into(),
            crate::api::schema::CommandInvokeParams {
                command_id: format!("{}codex", crate::nirvana_launch::NEW_COMMAND),
                workspace_id: None,
                tab_id: None,
                pane_id: None,
                selection: None,
            },
        );
        assert!(response.contains("unknown_agent_kind"), "{response}");
        assert_eq!(app.state.workspaces.len(), 1);
    }
}
