//! Nirvana fork, plan 33 P33a §2.4: the Profiles group in the Preload panel, the project's agent profiles.
//!
//! Each profile is a row with its description; `e` opens its file through the same `command.invoke` path Preload's
//! edit uses, `n` asks for a name and runs `herdr profile new`'s template. The rows go after Preload's own, so every
//! Preload row keeps its place. The files and the parser are `crate::nirvana_profiles`.

use std::path::{Path, PathBuf};

use crossterm::event::KeyCode;

use crate::nirvana_preload::{Group, Item, Provider};

/// The group's header; the rows are told from Preload's own by it.
pub(super) const TITLE: &str = "Profiles (this project, herdr)";

/// The project a panel's folder belongs to: the nearest folder holding `.git`, the board's rule, else the folder.
pub(super) fn root(cwd: &Path) -> PathBuf {
    crate::nirvana_board::root(cwd).unwrap_or_else(|| cwd.to_path_buf())
}

pub(super) fn is_profile(item: &Item) -> bool {
    item.provider == Provider::Other(TITLE.to_owned())
}

/// One row per profile file: its description, or that it is refused and how to see why.
pub(super) fn items(cwd: &Path) -> Vec<Item> {
    crate::nirvana_profiles::list(&root(cwd))
        .into_iter()
        .map(|(name, file, parsed)| Item {
            provider: Provider::Other(TITLE.to_owned()),
            group: Group::Agents,
            detail: match &parsed {
                Ok(profile) => profile.description.clone(),
                Err(_) => format!("refused: herdr profile check {name}"),
            },
            name,
            tokens: None,
            on: parsed.is_ok(),
            preset_off: false,
            preset_pinned: false,
            switch: None,
            file: Some(file),
        })
        .collect()
}

/// What a key does to the name being typed after `n`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Naming {
    /// Still typing.
    Typing,
    /// Enter: make the profile of this name.
    Make(String),
    Cancel,
}

/// A key while the panel asks for a new profile's name. Only `[a-z0-9-]` is typed, at most the name's length;
/// anything else is ignored, so the name `new` gets is always one it takes.
pub(super) fn naming_key(name: &mut String, code: KeyCode) -> Naming {
    match code {
        KeyCode::Esc => Naming::Cancel,
        KeyCode::Enter if name.is_empty() => Naming::Typing,
        KeyCode::Enter => Naming::Make(name.clone()),
        KeyCode::Backspace => {
            name.pop();
            Naming::Typing
        }
        KeyCode::Char(c)
            if (c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && name.len() < crate::nirvana_profiles::NAME_MAX =>
        {
            name.push(c);
            Naming::Typing
        }
        _ => Naming::Typing,
    }
}

/// The panel's foot while it asks for a name.
pub(super) fn naming_line(name: &str) -> String {
    format!("new profile: {name}_ \u{b7} a-z 0-9 - \u{b7} enter makes it \u{b7} esc cancels")
}

/// `n`'s enter: the template written for the panel's project.
pub(super) fn make(cwd: &Path, name: &str) -> Result<PathBuf, String> {
    crate::nirvana_profiles::new_profile(&root(cwd), name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "p33-shell-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        dir
    }

    #[test]
    fn p33_the_profiles_group_lists_each_profile_with_its_file_and_a_refused_one_says_so() {
        let dir = folder("group");
        assert!(items(&dir).is_empty(), "no folder, no rows");
        let profiles = dir.join(".nirvana").join("profiles");
        std::fs::create_dir_all(&profiles).unwrap();
        std::fs::write(
            profiles.join("slicer.md"),
            "---\ndescription: Slices prints in Bambu Studio.\n---\nYou slice.\n",
        )
        .unwrap();
        std::fs::write(profiles.join("broken.md"), "---\ncolour: red\n---\n").unwrap();
        // The panel's folder is inside the project: the rows are the project's.
        let inside = dir.join("src");
        std::fs::create_dir_all(&inside).unwrap();
        let rows = items(&inside);
        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["broken", "slicer"]);
        assert!(rows.iter().all(is_profile));
        assert_eq!(rows[1].detail, "Slices prints in Bambu Studio.");
        assert_eq!(
            rows[1].file.as_deref(),
            Some(profiles.join("slicer.md").as_path())
        );
        assert!(rows[1].on && rows[1].switch.is_none());
        assert_eq!(rows[0].detail, "refused: herdr profile check broken");
        assert!(!rows[0].on);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A window whose focused tab runs Claude in `cwd`, with no Preload reading (`refresh` reads nothing under test).
    fn state_in(cwd: &Path) -> super::super::ClientShellState {
        crate::nirvana_preload::set_view_for_test(Some(crate::nirvana_preload::View::default()));
        let mut projected = super::super::tests::snapshot();
        projected.agents = vec![crate::protocol::ClientShellAgent {
            pane_id: "pane_1".into(),
            workspace_id: "ws_1".into(),
            tab_id: "tab_1".into(),
            name: Some("moto".into()),
            display_agent: None,
            agent: Some("claude".into()),
            title: None,
            terminal_title: None,
            terminal_title_stripped: None,
            agent_status: crate::api::schema::AgentStatus::Idle,
            state_change_seq: 1,
            state_labels: Vec::new(),
            tokens: Vec::new(),
            focused: true,
            agent_session: None,
        }];
        let cwd = cwd.display().to_string();
        projected.panes[0].cwd = Some(cwd.clone());
        projected.panes[0].foreground_cwd = Some(cwd);
        let mut state = super::super::ClientShellState::new(
            super::super::ClientShellConfig::from_config(&crate::config::Config::default()),
        );
        state.set_snapshot(Box::new(projected));
        state
    }

    fn press(
        state: &mut super::super::ClientShellState,
        code: KeyCode,
    ) -> super::super::ClientShellInput {
        let mut outcome = super::super::ClientShellInput::default();
        state.route_preload_key(
            &crate::input::TerminalKey::new(code, crossterm::event::KeyModifiers::NONE),
            &mut outcome,
        );
        outcome
    }

    fn opened(outcome: &super::super::ClientShellInput) -> Vec<String> {
        outcome
            .actions
            .iter()
            .filter_map(|action| match action {
                super::super::ClientShellAction::Endpoint { request, .. } => {
                    match &request.method {
                        crate::api::schema::Method::CommandInvoke(params) => {
                            Some(params.command_id.clone())
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn p33_the_panel_lists_the_profiles_last_and_n_makes_one_and_opens_it() {
        let dir = folder("panel");
        let mut state = state_in(&dir);
        state.toggle_preload_panel();
        let preload_rows = {
            let panel = state.nirvana_preload.as_ref().expect("the panel is open");
            super::super::nirvana_mods::panel_items(
                panel,
                &crate::nirvana_preload::view(&panel.cwd),
            )
            .len()
        };
        // `n`, a name with a key it does not take, enter: the template is written and opens in an edit tab.
        press(&mut state, KeyCode::Char('n'));
        for c in "sli cer".chars() {
            press(&mut state, KeyCode::Char(c));
        }
        let naming = state
            .nirvana_preload
            .as_ref()
            .and_then(|panel| panel.naming.clone());
        assert_eq!(naming.as_deref(), Some("slicer"));
        let outcome = press(&mut state, KeyCode::Enter);
        let file = dir.join(".nirvana").join("profiles").join("slicer.md");
        assert!(file.is_file());
        assert_eq!(
            opened(&outcome),
            [format!(
                "{}{}",
                crate::nirvana::EDIT_FILE_COMMAND,
                file.display()
            )]
        );
        // Open again: Preload's rows keep their places, and the profile is the last row; space says how it is used.
        state.toggle_preload_panel();
        let items = {
            let panel = state.nirvana_preload.as_ref().expect("open again");
            super::super::nirvana_mods::panel_items(
                panel,
                &crate::nirvana_preload::view(&panel.cwd),
            )
        };
        assert_eq!(items.len(), preload_rows + 1);
        assert!(is_profile(items.last().unwrap()) && items.last().unwrap().name == "slicer");
        state.nirvana_preload.as_mut().unwrap().selected = items.len() - 1;
        press(&mut state, KeyCode::Char(' '));
        let message = state
            .nirvana_preload
            .as_ref()
            .and_then(|panel| panel.message.clone());
        assert!(
            message
                .as_deref()
                .is_some_and(|m| m.contains("--profile slicer")),
            "{message:?}"
        );
        // `e` on the row opens its file.
        let outcome = press(&mut state, KeyCode::Char('e'));
        assert_eq!(
            opened(&outcome),
            [format!(
                "{}{}",
                crate::nirvana::EDIT_FILE_COMMAND,
                file.display()
            )]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn p33_n_takes_only_a_name_new_accepts_and_enter_makes_it() {
        let mut name = String::new();
        assert_eq!(naming_key(&mut name, KeyCode::Enter), Naming::Typing);
        for c in "Sl icer/".chars() {
            naming_key(&mut name, KeyCode::Char(c));
        }
        assert_eq!(name, "licer");
        naming_key(&mut name, KeyCode::Backspace);
        for _ in 0..40 {
            naming_key(&mut name, KeyCode::Char('x'));
        }
        assert_eq!(name.len(), crate::nirvana_profiles::NAME_MAX);
        assert!(crate::nirvana_profiles::valid_name(&name));
        assert_eq!(
            naming_key(&mut name, KeyCode::Enter),
            Naming::Make(name.clone())
        );
        assert_eq!(naming_key(&mut name, KeyCode::Esc), Naming::Cancel);
        let dir = folder("make");
        let file = make(&dir, "slicer").unwrap();
        assert_eq!(
            file,
            dir.join(".nirvana").join("profiles").join("slicer.md")
        );
        assert_eq!(items(&dir).len(), 1);
        assert!(items(&dir)[0].on, "the template parses");
        assert!(make(&dir, "slicer").unwrap_err().contains("already there"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
