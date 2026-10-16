//! Nirvana fork, feature 5: the Preload panel's "edit". Opens a preloaded file (a CLAUDE.md, MEMORY.md, a
//! skill's SKILL.md, an agent's file) in the editor, in a zoomed pane over the tab, the way herdr opens
//! scrollback in the editor (`$VISUAL`, `$EDITOR`, else Notepad on Windows). The panel sends herdr's
//! existing `command.invoke`, the file after `crate::nirvana::EDIT_FILE_COMMAND`, so the API stays
//! upstream's.

use super::App;

impl App {
    pub(crate) fn nirvana_edit_file(
        &mut self,
        id: String,
        params: &crate::api::schema::CommandInvokeParams,
        file: &str,
    ) -> String {
        use crate::app::api::responses::{encode_error, encode_success};

        let Some(path) = crate::nirvana::editable_file(file) else {
            return encode_error(id, "not_editable", format!("not a markdown file: {file}"));
        };
        if let Err((code, message)) = self.focus_client_shell_command_target(params) {
            return encode_error(id, code, message);
        }
        let argv = match crate::platform::scrollback_editor_argv(&path) {
            Ok(argv) => argv,
            Err(err) => return encode_error(id, "editor_failed", err.to_string()),
        };
        let (env, _) = self.custom_command_env();
        let new_pane = match self.spawn_overlay_argv_command(
            &argv,
            path.parent().map(Into::into),
            env,
            Vec::new(),
        ) {
            Ok((_, new_pane)) => new_pane,
            Err(err) => return encode_error(id, "editor_failed", err.to_string()),
        };
        let terminal_id = new_pane.terminal.id.clone();
        self.terminal_runtimes
            .insert(terminal_id.clone(), new_pane.runtime);
        self.state
            .remove_alias_shadowed_by_new_pane(new_pane.pane_id);
        self.state.terminals.insert(terminal_id, new_pane.terminal);
        tracing::info!(file = %path.display(), "nirvana: opened a preloaded file in the editor");
        encode_success(id, crate::api::schema::ResponseResult::Ok {})
    }
}
