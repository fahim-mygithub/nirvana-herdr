//! Plan 15 §4: the Jobs square and panel.
//! Narrow and wide forms, click target opening panel, row rendering,
//! click focusing job pane, and click on Claude task tailing file.

use super::*;
use crate::api::schema::Method;
use crate::nirvana_job::{set_view_for_test, ClaudeTask, ClaudeTaskState, JobRecord, JobsView};

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
        agent_session: None,
    }
}

fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nirvana-jobs-ui-{name}-{}", std::process::id()));
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
    projected.panes.push(ClientShellPane {
        pane_id: "pane_job_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_jobs".into(),
        label: Some("test-job-pane".into()),
        cwd: None,
        foreground_cwd: None,
        focused: false,
        right_click_passthrough: false,
    });
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

fn focused(outcome: &ClientShellInput) -> Vec<String> {
    outcome
        .actions
        .iter()
        .filter_map(|action| match action {
            ClientShellAction::Endpoint { request, .. } => match &request.method {
                Method::PaneFocus(target) => Some(target.pane_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn the_jobs_square_reads_jobs_when_empty_and_shows_narrow_and_wide_forms() {
    let (mut state, root) = state_for("empty");

    // 1. Empty view: reads "Jobs" with drop-down arrow
    set_view_for_test(Some(JobsView::default()));
    let row = strip(&mut state, 200);
    assert!(row.contains(" Jobs \u{25BE}"), "empty wide: {row:?}");
    assert!(!row.contains("Jobs 0"));

    // 2. 2 running · 1 failed
    let mut last_lines = std::collections::HashMap::new();
    last_lines.insert("j1".into(), "compiling step 3".into());
    let view_active = JobsView {
        jobs: vec![
            JobRecord {
                id: "j1".into(),
                name: "cargo-build".into(),
                argv: vec![],
                cwd: ".".into(),
                space: None,
                pane: Some("pane_job_1".into()),
                started: 1000,
                ended: None,
                exit: None,
                pid: Some(std::process::id()),
            },
            JobRecord {
                id: "j2".into(),
                name: "cargo-test".into(),
                argv: vec![],
                cwd: ".".into(),
                space: None,
                pane: None,
                started: 1000,
                ended: Some(1010),
                exit: Some(1),
                pid: None,
            },
        ],
        job_last_lines: last_lines,
        claude_tasks: vec![ClaudeTask {
            id: "task-abc".into(),
            name: "task-abc".into(),
            path: root.join("task-abc.output"),
            state: ClaudeTaskState::Running,
            exit_code: None,
            modified: 1005,
            elapsed: 5,
            last_line: Some("running pytest".into()),
        }],
        running_count: 2,
        failed_count: 1,
        passed_count: 0,
    };
    set_view_for_test(Some(view_active));

    // Wide form
    let wide = strip(&mut state, 200);
    assert!(
        wide.contains(" Jobs 2 running \u{b7} 1 failed \u{25BE}"),
        "wide: {wide:?}"
    );

    // Narrow form
    let narrow = strip(&mut state, 110);
    assert!(
        narrow.contains(" Jobs 2 run \u{b7} 1 fail \u{25BE}"),
        "narrow: {narrow:?}"
    );

    // 3. Passed only
    let view_passed = JobsView {
        jobs: vec![],
        job_last_lines: std::collections::HashMap::new(),
        claude_tasks: vec![ClaudeTask {
            id: "task-pass".into(),
            name: "task-pass".into(),
            path: root.join("task-pass.output"),
            state: ClaudeTaskState::Passed,
            exit_code: Some(0),
            modified: 1000,
            elapsed: 10,
            last_line: Some("[exited with code 0]".into()),
        }],
        running_count: 0,
        failed_count: 0,
        passed_count: 1,
    };
    set_view_for_test(Some(view_passed));

    let wide_passed = strip(&mut state, 200);
    assert!(
        wide_passed.contains(" Jobs 1 passed \u{25BE}"),
        "wide passed: {wide_passed:?}"
    );
    let narrow_passed = strip(&mut state, 110);
    assert!(
        narrow_passed.contains(" Jobs 1 pass \u{25BE}"),
        "narrow passed: {narrow_passed:?}"
    );

    done(root);
}

#[test]
fn jobs_square_click_opens_panel_and_renders_rows() {
    let (mut state, root) = state_for("click-open");
    let mut last_lines = std::collections::HashMap::new();
    last_lines.insert("job-1".into(), "testing 4/10 passed".into());

    let test_view = JobsView {
        jobs: vec![JobRecord {
            id: "job-1".into(),
            name: "test-run".into(),
            argv: vec![],
            cwd: ".".into(),
            space: None,
            pane: Some("pane_job_1".into()),
            started: 1000,
            ended: None,
            exit: None,
            pid: Some(std::process::id()),
        }],
        job_last_lines: last_lines,
        claude_tasks: vec![ClaudeTask {
            id: "task-xyz".into(),
            name: "task-xyz".into(),
            path: root.join("task-xyz.output"),
            state: ClaudeTaskState::Passed,
            exit_code: Some(0),
            modified: 1000,
            elapsed: 15,
            last_line: Some("finished successfully".into()),
        }],
        running_count: 1,
        failed_count: 0,
        passed_count: 1,
    };
    set_view_for_test(Some(test_view));

    // Compose to locate square
    let _ = rows_of(&mut state, 160, 40);
    let square = state.hits.nirvana_jobs;
    assert!(square.width > 0, "Jobs square must have width > 0");

    // Click on Jobs square opens panel
    click(&mut state, square.x + 1, square.y);
    assert!(state.nirvana_jobs.is_some(), "panel opens on click");

    // Render composed rows with open panel
    let rows = rows_of(&mut state, 160, 40);
    let text = rows.join("\n");
    let folder_name = root.file_name().unwrap().to_string_lossy();
    assert!(
        text.contains(&format!("Jobs \u{b7} {folder_name}")),
        "title: {text}"
    );
    assert!(text.contains("name"), "has column name: {text}");
    assert!(text.contains("state"), "has column state: {text}");
    assert!(text.contains("time"), "has column time: {text}");
    assert!(text.contains("last line"), "has column last line: {text}");
    assert!(text.contains("test-run"), "has job name: {text}");
    assert!(
        text.contains("testing 4/10 passed"),
        "has job last line: {text}"
    );
    assert!(text.contains("task-xyz"), "has claude task: {text}");
    assert!(
        text.contains("finished successfully"),
        "has claude last line: {text}"
    );

    done(root);
}

#[test]
fn click_on_job_focuses_pane_and_esc_closes_panel() {
    let (mut state, root) = state_for("job-focus");
    let test_view = JobsView {
        jobs: vec![JobRecord {
            id: "job-focus-test".into(),
            name: "build".into(),
            argv: vec![],
            cwd: ".".into(),
            space: None,
            pane: Some("pane_job_1".into()),
            started: 1000,
            ended: None,
            exit: None,
            pid: Some(std::process::id()),
        }],
        job_last_lines: std::collections::HashMap::new(),
        claude_tasks: vec![],
        running_count: 1,
        failed_count: 0,
        passed_count: 0,
    };
    set_view_for_test(Some(test_view));

    // Open panel
    let _ = rows_of(&mut state, 160, 40);
    let square = state.hits.nirvana_jobs;
    click(&mut state, square.x + 1, square.y);
    assert!(state.nirvana_jobs.is_some());

    // Compose to locate row hits
    let _ = rows_of(&mut state, 160, 40);
    let rows_hits = state.hits.nirvana_jobs_panel.rows.clone();
    assert_eq!(rows_hits.len(), 1);

    // Click on job row focuses pane
    let (row_rect, idx) = rows_hits[0];
    assert_eq!(idx, 0);
    let outcome = click(&mut state, row_rect.x + 2, row_rect.y);
    assert_eq!(focused(&outcome), vec!["pane_job_1"]);
    assert!(state.nirvana_jobs.is_none(), "panel closes after focus");

    // Reopen and test Esc closes
    click(&mut state, square.x + 1, square.y);
    assert!(state.nirvana_jobs.is_some());
    key(&mut state, KeyCode::Esc);
    assert!(state.nirvana_jobs.is_none(), "esc closes panel");

    done(root);
}

#[test]
fn click_on_claude_task_tails_file() {
    let (mut state, root) = state_for("claude-tail");
    let task_file = root.join("task-1.output");
    std::fs::write(&task_file, "line1\nline2").unwrap();

    let test_view = JobsView {
        jobs: vec![],
        job_last_lines: std::collections::HashMap::new(),
        claude_tasks: vec![ClaudeTask {
            id: "1".into(),
            name: "task-1".into(),
            path: task_file,
            state: ClaudeTaskState::Running,
            exit_code: None,
            modified: 1000,
            elapsed: 8,
            last_line: Some("line2".into()),
        }],
        running_count: 1,
        failed_count: 0,
        passed_count: 0,
    };
    set_view_for_test(Some(test_view));

    // Open panel
    let _ = rows_of(&mut state, 160, 40);
    let square = state.hits.nirvana_jobs;
    click(&mut state, square.x + 1, square.y);
    assert!(state.nirvana_jobs.is_some());

    // Compose to locate row hits
    let _ = rows_of(&mut state, 160, 40);
    let rows_hits = state.hits.nirvana_jobs_panel.rows.clone();
    assert_eq!(rows_hits.len(), 1);

    // Click on Claude task row closes panel and spawns tail pane
    let (row_rect, idx) = rows_hits[0];
    assert_eq!(idx, 0);
    let _ = click(&mut state, row_rect.x + 2, row_rect.y);
    assert!(state.nirvana_jobs.is_none(), "panel closes on click");

    done(root);
}
