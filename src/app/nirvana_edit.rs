//! Nirvana fork, features 5 and 7: the Preload panel's "edit". Opens a preloaded file (a CLAUDE.md,
//! MEMORY.md, a skill's SKILL.md, an agent's file) in a terminal editor that takes the mouse, in a new tab
//! of the panel's space, focused and named after the file. The panel sends herdr's existing
//! `command.invoke`, the file after `crate::nirvana::EDIT_FILE_COMMAND`, so the API stays upstream's.
//!
//! The owner, 2026-10-16: "when you edit have a terminal editor that is mouse compatible and open that
//! file in a new tab not open through notepad." (nirvana/plans/2026-10-16-07-preload-providers-and-editor.md)
//!
//! 0.5.0 opened the editor through herdr's scrollback helper, in a zoomed pane over the agent's tab. That
//! helper falls back to Notepad on Windows, and on Linux and macOS deletes the file when the editor exits:
//! it is written for herdr's scrollback temp file. It is not used here.

use super::App;

impl App {
    pub(crate) fn nirvana_edit_file(
        &mut self,
        id: String,
        params: &crate::api::schema::CommandInvokeParams,
        file: &str,
    ) -> String {
        use crate::app::api::responses::encode_error;

        let Some(path) = crate::nirvana::editable_file(file) else {
            return encode_error(id, "not_editable", format!("not a markdown file: {file}"));
        };
        let Some(argv) = crate::nirvana::terminal_editor_argv(&path) else {
            return encode_error(
                id,
                "no_terminal_editor",
                "no terminal editor found: install micro, or set NIRVANA_EDITOR",
            );
        };
        self.nirvana_open_editor_tab(id, params, &path, &argv)
    }

    /// Plan 34 §2.7: the editor in a new tab whose folder is `dir`, a folder profile's `knowledge/`, named after it.
    pub(crate) fn nirvana_edit_dir(
        &mut self,
        id: String,
        params: &crate::api::schema::CommandInvokeParams,
        dir: &str,
    ) -> String {
        use crate::app::api::responses::encode_error;

        let Some(path) = crate::nirvana::editable_dir(dir) else {
            return encode_error(id, "not_editable", format!("not a folder: {dir}"));
        };
        let Some(argv) = crate::nirvana::terminal_editor_program() else {
            return encode_error(
                id,
                "no_terminal_editor",
                "no terminal editor found: install micro, or set NIRVANA_EDITOR",
            );
        };
        let name = path
            .parent()
            .and_then(|parent| parent.file_name())
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        tracing::info!(dir = %path.display(), editor = %argv[0], "nirvana: opening a profile's cards in a new tab");
        self.nirvana_open_command_tab(id, params, path, &argv, format!("cards {name}"))
    }

    /// Runs `argv` in a new tab of the panel's space, focused, named after the file.
    fn nirvana_open_editor_tab(
        &mut self,
        id: String,
        params: &crate::api::schema::CommandInvokeParams,
        path: &std::path::Path,
        argv: &[String],
    ) -> String {
        let cwd = path
            .parent()
            .map_or_else(|| path.to_path_buf(), std::path::Path::to_path_buf);
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        tracing::info!(file = %path.display(), editor = %argv[0], "nirvana: opening a preloaded file in a new tab");
        self.nirvana_open_command_tab(id, params, cwd, argv, format!("edit {name}"))
    }

    /// Feature 11: the Accounts panel's sign-in. Runs `herdr account login <cli>` in a new focused tab, in the
    /// home folder: a sign-in belongs to no project.
    pub(crate) fn nirvana_account_login(
        &mut self,
        id: String,
        params: &crate::api::schema::CommandInvokeParams,
        key: &str,
    ) -> String {
        let Some(cli) = crate::nirvana_accounts::Cli::from_key(key) else {
            return crate::app::api::responses::encode_error(
                id,
                "unknown_cli",
                format!("no sign-in for {key}"),
            );
        };
        let exe = std::env::current_exe().unwrap_or_else(|_| "herdr".into());
        let argv = crate::nirvana_accounts::login_argv(&exe, cli);
        let cwd = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map_or_else(std::env::temp_dir, std::path::PathBuf::from);
        tracing::info!(cli = key, "nirvana: opening a sign-in tab");
        self.nirvana_open_command_tab(id, params, cwd, &argv, format!("log in {}", cli.name()))
    }

    /// Runs `argv` in a new tab of the panel's space, focused and named; the tab closes when `argv` exits.
    fn nirvana_open_command_tab(
        &mut self,
        id: String,
        params: &crate::api::schema::CommandInvokeParams,
        cwd: std::path::PathBuf,
        argv: &[String],
        name: String,
    ) -> String {
        use crate::app::api::responses::{encode_error, encode_success};

        let ws_idx = match params.workspace_id.as_deref() {
            Some(workspace_id) => match self.parse_workspace_id(workspace_id) {
                Some(ws_idx) => ws_idx,
                None => return encode_error(id, "workspace_not_found", "workspace not found"),
            },
            None => match self.state.active {
                Some(ws_idx) => ws_idx,
                None => return encode_error(id, "no_active_workspace", "no active workspace"),
            },
        };
        let (env, _) = self.custom_command_env();
        let (rows, cols) = self.state.new_pane_size(crate::ui::NewPanePlacement::Alone);
        let scrollback_limit_bytes = self.state.pane_scrollback_limit_bytes;
        let host_terminal_theme = self.state.host_terminal_theme;
        let host_terminal_appearance = self.state.host_terminal_appearance;
        let Some(ws) = self.state.workspaces.get_mut(ws_idx) else {
            return encode_error(id, "workspace_not_found", "workspace not found");
        };
        let (tab_idx, terminal, runtime) = match ws.create_tab_argv_command(
            rows,
            cols,
            cwd,
            argv,
            env,
            scrollback_limit_bytes,
            host_terminal_theme,
            host_terminal_appearance,
        ) {
            Ok(result) => result,
            Err(err) => return encode_error(id, "editor_failed", err.to_string()),
        };
        let pane_id = ws.tabs[tab_idx].root_pane;
        ws.tabs[tab_idx].set_custom_name(name);
        self.terminal_runtimes.insert(terminal.id.clone(), runtime);
        self.state.terminals.insert(terminal.id.clone(), terminal);
        self.state.remove_alias_shadowed_by_new_pane(pane_id);
        self.state.switch_workspace_tab(ws_idx, tab_idx);
        self.state.mode = crate::app::Mode::Terminal;
        self.schedule_session_save();
        self.emit_tab_created_events(ws_idx, tab_idx);
        encode_success(id, crate::api::schema::ResponseResult::Ok {})
    }
}

#[cfg(test)]
mod tests {
    use super::super::api::test_support::{exiting_test_command, shutdown_test_runtimes};
    use super::*;
    use crate::{config::Config, workspace::Workspace};

    #[tokio::test]
    async fn edit_opens_the_file_in_a_new_focused_tab_and_leaves_the_agents_tab_alone() {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &Config::default(),
            crate::app::AppPolicy::TEST,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![Workspace::test_new("edit")];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.ensure_test_terminals();
        let panes_before = app.state.workspaces[0].tabs[0].panes.len();
        let dir = std::env::temp_dir().join(format!("nirvana-edit-tab-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("CLAUDE.md");
        let params = crate::api::schema::CommandInvokeParams {
            command_id: format!("{}{}", crate::nirvana::EDIT_FILE_COMMAND, file.display()),
            workspace_id: None,
            tab_id: None,
            pane_id: None,
            selection: None,
        };
        let response = app.nirvana_open_editor_tab(
            "req".into(),
            &params,
            &file,
            &[exiting_test_command().to_owned()],
        );
        assert!(response.contains("\"ok\""), "{response}");
        let ws = &app.state.workspaces[0];
        assert_eq!(ws.tabs.len(), 2, "a new tab");
        assert_eq!(ws.active_tab, 1, "focused");
        assert_eq!(ws.tabs[1].custom_name.as_deref(), Some("edit CLAUDE.md"));
        assert_eq!(
            ws.tabs[0].panes.len(),
            panes_before,
            "the agent's tab keeps its panes"
        );
        shutdown_test_runtimes(&mut app);
    }

    #[tokio::test]
    async fn a_sign_in_opens_its_own_focused_tab_and_an_unknown_cli_opens_none() {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &Config::default(),
            crate::app::AppPolicy::TEST,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![Workspace::test_new("accounts")];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.ensure_test_terminals();
        let params = |key: &str| crate::api::schema::CommandInvokeParams {
            command_id: format!("{}{key}", crate::nirvana::ACCOUNT_LOGIN_COMMAND),
            workspace_id: None,
            tab_id: None,
            pane_id: None,
            selection: None,
        };
        let response = app.handle_command_invoke("bad".into(), params("gemini"));
        assert!(response.contains("unknown_cli"), "{response}");
        assert_eq!(
            app.state.workspaces[0].tabs.len(),
            1,
            "no tab for an unknown CLI"
        );

        let response = app.nirvana_open_command_tab(
            "req".into(),
            &params("antigravity"),
            std::env::temp_dir(),
            &[exiting_test_command().to_owned()],
            "log in Antigravity".into(),
        );
        assert!(response.contains("\"ok\""), "{response}");
        let ws = &app.state.workspaces[0];
        assert_eq!(ws.tabs.len(), 2, "a new tab");
        assert_eq!(ws.active_tab, 1, "focused");
        assert_eq!(
            ws.tabs[1].custom_name.as_deref(),
            Some("log in Antigravity")
        );
        shutdown_test_runtimes(&mut app);
    }
}
