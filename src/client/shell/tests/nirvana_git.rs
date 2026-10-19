//! Plan 16 §2, §3: the Git square and panel.
//! Narrow and wide forms, square label, click target opening panel,
//! row rendering, disabled reasons, confirm step, and Esc / Cancel.

use super::*;
use crate::client::shell::nirvana_git::GitConfirm;
use crate::client::shell::nirvana_mods::OpenPanel;
use crate::nirvana_git::{set_view_for_test, CiRun, CiState, GitStatus, GitView};

fn agent(name: &str) -> ClientShellAgent {
    ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: Some(name.into()),
        display_agent: None,
        agent: Some("claude".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Idle,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: true,
    }
}

fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nirvana-git-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp folder");
    dir
}

fn state_for(test: &str) -> (ClientShellState, std::path::PathBuf) {
    let root = folder(&format!("{test}-proj"));
    crate::nirvana_preload::set_view_for_test(Some(crate::nirvana_preload::View::default()));
    let mut projected = snapshot();
    projected.agents = vec![agent("test-agent")];
    let cwd = root.display().to_string();
    projected.panes[0].cwd = Some(cwd.clone());
    projected.panes[0].foreground_cwd = Some(cwd);
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    (state, root)
}

fn done(root: std::path::PathBuf) {
    set_view_for_test(None);
    let _ = std::fs::remove_dir_all(root);
}

fn rows_of(state: &mut ClientShellState, width: u16, height: u16) -> Vec<String> {
    let frame = state.compose(width, height).expect("composed frame").frame;
    frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect()
}

fn strip(state: &mut ClientShellState, width: u16) -> String {
    let rows = rows_of(state, width, 40);
    rows[state.layout(width, 40).nirvana_mods.y as usize].clone()
}

fn click(state: &mut ClientShellState, column: u16, row: u16) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })])
}

fn key(state: &mut ClientShellState, code: KeyCode) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        code,
        KeyModifiers::NONE,
    ))])
}

#[test]
fn the_git_square_label_narrow_and_wide_forms_and_outside_repo() {
    let (mut state, root) = state_for("label");

    // 1. Outside git repo: square not drawn
    set_view_for_test(None);
    let row = strip(&mut state, 200);
    assert!(!row.contains("Git"), "outside git repo no square: {row:?}");
    assert_eq!(state.hits.nirvana_git.width, 0);

    // 2. Clean tree: reads "Git main ▾"
    set_view_for_test(Some(GitView {
        status: GitStatus {
            repo: root.clone(),
            branch: "main".into(),
            upstream: "origin/main".into(),
            ahead: 0,
            behind: 0,
            dirty: 0,
            files: vec![],
        },
        ci: None,
    }));
    let row = strip(&mut state, 200);
    assert!(row.contains("Git main \u{25BE}"), "clean tree: {row:?}");

    // 3. Wide and narrow forms with ahead, behind, and dirty
    set_view_for_test(Some(GitView {
        status: GitStatus {
            repo: root.clone(),
            branch: "main".into(),
            upstream: "origin/main".into(),
            ahead: 1,
            behind: 2,
            dirty: 3,
            files: vec!["a.txt".into(), "b.txt".into(), "c.txt".into()],
        },
        ci: None,
    }));
    let wide = strip(&mut state, 200);
    assert!(
        wide.contains("Git main \u{2191}1 \u{2193}2 \u{b7} 3 changed \u{25BE}"),
        "wide form: {wide:?}"
    );

    let narrow = strip(&mut state, 120);
    assert!(
        narrow.contains("Git main \u{2191}1 \u{2193}2 \u{b7} 3 \u{25BE}"),
        "narrow form: {narrow:?}"
    );

    // 4. CI marks at end
    let ci_passed = CiRun {
        workflow_name: "CI".into(),
        state: CiState::Passed,
        conclusion_word: "passed".into(),
        created_at_secs: 1000,
        url: "".into(),
    };
    set_view_for_test(Some(GitView {
        status: GitStatus {
            repo: root.clone(),
            branch: "main".into(),
            upstream: "origin/main".into(),
            ahead: 1,
            behind: 2,
            dirty: 3,
            files: vec!["a.txt".into(), "b.txt".into(), "c.txt".into()],
        },
        ci: Some(ci_passed),
    }));
    let row_ci = strip(&mut state, 200);
    assert!(
        row_ci.contains("3 changed \u{2713} \u{25BE}"),
        "ci passed mark: {row_ci:?}"
    );

    done(root);
}

#[test]
fn git_panel_renders_rows_and_buttons() {
    let (mut state, root) = state_for("panel-rows");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let view = GitView {
        status: GitStatus {
            repo: root.clone(),
            branch: "main".into(),
            upstream: "origin/main".into(),
            ahead: 1,
            behind: 2,
            dirty: 3,
            files: vec!["file1.txt".into(), "file2.txt".into(), "file3.txt".into()],
        },
        ci: Some(CiRun {
            workflow_name: "CI".into(),
            state: CiState::Passed,
            conclusion_word: "passed".into(),
            created_at_secs: now.saturating_sub(12 * 60),
            url: "https://github.com/org/repo".into(),
        }),
    };
    set_view_for_test(Some(view));

    // Compose to locate square
    let _ = rows_of(&mut state, 160, 40);
    let square = state.hits.nirvana_git;
    assert!(square.width > 0, "Git square must have width > 0");

    // Click on Git square opens panel
    click(&mut state, square.x + 1, square.y);
    assert!(state.nirvana_git.is_some(), "panel opens on click");
    assert_eq!(state.open_panel(), OpenPanel::Git);

    // Compose and verify all rows
    let rows = rows_of(&mut state, 160, 40);
    let text = rows.join("\n");
    let folder_name = root.file_name().unwrap().to_string_lossy();
    assert!(
        text.contains(&format!("Git \u{b7} {folder_name}")),
        "title row: {text}"
    );
    assert!(
        text.contains("branch main \u{2192} origin/main"),
        "branch row: {text}"
    );
    assert!(
        text.contains("\u{2191}1 ahead  \u{2193}2 behind (last fetch)"),
        "ahead/behind row: {text}"
    );
    assert!(text.contains("CI  CI  passed  12m ago"), "ci row: {text}");
    assert!(text.contains("3 changed files:"), "dirty count: {text}");
    assert!(text.contains("file1.txt"), "file 1: {text}");
    assert!(text.contains("file2.txt"), "file 2: {text}");
    assert!(text.contains("file3.txt"), "file 3: {text}");
    assert!(text.contains("[Pull \u{2193}2]"), "pull button: {text}");
    assert!(text.contains("[Push \u{2191}1]"), "push button: {text}");
    assert!(text.contains("[Fetch]"), "fetch button: {text}");

    done(root);
}

#[test]
fn disabled_button_reasons_on_status_line() {
    let (mut state, root) = state_for("disabled-reasons");

    // 1. No upstream: Pull, Push, Fetch are all disabled
    set_view_for_test(Some(GitView {
        status: GitStatus {
            repo: root.clone(),
            branch: "feature".into(),
            upstream: "".into(),
            ahead: 0,
            behind: 0,
            dirty: 1,
            files: vec!["edit.rs".into()],
        },
        ci: None,
    }));

    // Open panel
    let _ = rows_of(&mut state, 160, 40);
    let square = state.hits.nirvana_git;
    click(&mut state, square.x + 1, square.y);
    assert!(state.nirvana_git.is_some());

    // Click Pull button
    let _ = rows_of(&mut state, 160, 40);
    let pull_btn = state.hits.nirvana_git_panel.pull_btn;
    click(&mut state, pull_btn.x + 1, pull_btn.y);
    assert_eq!(
        state.nirvana_git.as_ref().unwrap().message.as_deref(),
        Some("git pull: refused. Branch has no upstream.")
    );

    // Click Push button
    let push_btn = state.hits.nirvana_git_panel.push_btn;
    click(&mut state, push_btn.x + 1, push_btn.y);
    assert_eq!(
        state.nirvana_git.as_ref().unwrap().message.as_deref(),
        Some("git push: refused. Branch has no upstream.")
    );

    // Click Fetch button
    let fetch_btn = state.hits.nirvana_git_panel.fetch_btn;
    click(&mut state, fetch_btn.x + 1, fetch_btn.y);
    assert_eq!(
        state.nirvana_git.as_ref().unwrap().message.as_deref(),
        Some("git fetch: refused. Branch has no upstream.")
    );

    // 2. Has upstream, but ahead == 0: Push is disabled
    set_view_for_test(Some(GitView {
        status: GitStatus {
            repo: root.clone(),
            branch: "main".into(),
            upstream: "origin/main".into(),
            ahead: 0,
            behind: 1,
            dirty: 0,
            files: vec![],
        },
        ci: None,
    }));
    let _ = rows_of(&mut state, 160, 40);
    let push_btn = state.hits.nirvana_git_panel.push_btn;
    click(&mut state, push_btn.x + 1, push_btn.y);
    assert_eq!(
        state.nirvana_git.as_ref().unwrap().message.as_deref(),
        Some("git push: refused. Nothing ahead to push.")
    );

    done(root);
}

#[test]
fn confirm_step_and_esc_or_cancel_closes() {
    let (mut state, root) = state_for("confirm-step");

    set_view_for_test(Some(GitView {
        status: GitStatus {
            repo: root.clone(),
            branch: "main".into(),
            upstream: "origin/main".into(),
            ahead: 1,
            behind: 2,
            dirty: 3,
            files: vec!["f1.txt".into(), "f2.txt".into(), "f3.txt".into()],
        },
        ci: None,
    }));

    // Open panel
    let _ = rows_of(&mut state, 160, 40);
    let square = state.hits.nirvana_git;
    click(&mut state, square.x + 1, square.y);
    assert!(state.nirvana_git.is_some());

    // First click on enabled Pull enters confirm mode
    let _ = rows_of(&mut state, 160, 40);
    let pull_btn = state.hits.nirvana_git_panel.pull_btn;
    click(&mut state, pull_btn.x + 1, pull_btn.y);

    assert_eq!(
        state.nirvana_git.as_ref().unwrap().confirming,
        Some(GitConfirm::Pull)
    );

    // Verify confirm prompt and buttons
    let rows = rows_of(&mut state, 160, 40);
    let text = rows.join("\n");
    assert!(
        text.contains("Pull 2 commits from origin into main? 3 changed files in the tree."),
        "confirm prompt: {text}"
    );
    assert!(text.contains("[Pull]"), "confirm pull btn: {text}");
    assert!(text.contains("[Cancel]"), "cancel btn: {text}");

    // Click [Cancel] drops confirm mode without closing panel
    let cancel_btn = state.hits.nirvana_git_panel.cancel_btn;
    assert!(cancel_btn.width > 0);
    click(&mut state, cancel_btn.x + 1, cancel_btn.y);
    assert_eq!(state.nirvana_git.as_ref().unwrap().confirming, None);
    assert!(
        state.nirvana_git.is_some(),
        "panel remains open after cancel"
    );

    // Click Pull again into confirm mode
    let _ = rows_of(&mut state, 160, 40);
    let pull_btn = state.hits.nirvana_git_panel.pull_btn;
    click(&mut state, pull_btn.x + 1, pull_btn.y);
    assert_eq!(
        state.nirvana_git.as_ref().unwrap().confirming,
        Some(GitConfirm::Pull)
    );

    // Esc drops confirm mode
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.nirvana_git.as_ref().unwrap().confirming, None);
    assert!(state.nirvana_git.is_some());

    // Esc again closes the panel
    key(&mut state, KeyCode::Esc);
    assert!(state.nirvana_git.is_none(), "esc closes panel");

    // Click square to reopen, then click close box
    click(&mut state, square.x + 1, square.y);
    assert!(state.nirvana_git.is_some());
    let _ = rows_of(&mut state, 160, 40);
    let close = state.hits.nirvana_git_panel.close;
    click(&mut state, close.x, close.y);
    assert!(state.nirvana_git.is_none(), "close box closes panel");

    done(root);
}

#[test]
fn live_client_snapshot_renders_git_square_on_scratch_repo() {
    let (mut state, root) = state_for("live-snapshot");
    let _ = std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&root)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "P16 Live Tester"])
        .current_dir(&root)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "livetester@example.com"])
        .current_dir(&root)
        .output();
    std::fs::write(root.join("README.md"), "hello\n").unwrap();
    let _ = std::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&root)
        .output();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&root)
        .output();
    std::fs::write(root.join("dirty.txt"), "changed\n").unwrap();

    set_view_for_test(None);
    crate::nirvana_git::refresh_folder(&root);

    let row = strip(&mut state, 200);
    println!("WIDE STRIP: {row}");
    assert!(
        row.contains("Git main \u{b7} 1 changed \u{25BE}"),
        "live strip wide: {row}"
    );
    let row_narrow = strip(&mut state, 120);
    println!("NARROW STRIP: {row_narrow}");
    assert!(
        row_narrow.contains("Git main \u{b7} 1 \u{25BE}"),
        "live strip narrow: {row_narrow}"
    );

    done(root);
}
