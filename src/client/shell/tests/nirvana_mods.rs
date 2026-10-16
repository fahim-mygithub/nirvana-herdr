//! Nirvana fork: the mod strip and its Preload mod (nirvana/plans/2026-10-16-05-preload-mod.md L1-L4).

use super::*;
use crate::api::schema::Method;
use crate::nirvana_preload::{parse, set_view_for_test, View, FIXTURE};

fn agent(kind: &str) -> ClientShellAgent {
    ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: Some("moto".into()),
        display_agent: None,
        agent: Some(kind.into()),
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

/// A folder of its own for each test, so a switch writes there and nowhere else.
fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nirvana-mods-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp folder");
    dir
}

fn view(local: serde_json::Value) -> View {
    View {
        report: parse(FIXTURE),
        local,
        ..View::default()
    }
}

fn state_with(kind: Option<&str>, cwd: &std::path::Path, view: View) -> ClientShellState {
    set_view_for_test(Some(view));
    let mut projected = snapshot();
    projected.agents = kind.into_iter().map(agent).collect();
    let cwd = cwd.display().to_string();
    projected.panes[0].cwd = Some(cwd.clone());
    projected.panes[0].foreground_cwd = Some(cwd);
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state
}

fn rows_of(state: &mut ClientShellState, width: u16, height: u16) -> (FrameData, Vec<String>) {
    let frame = state.compose(width, height).expect("composed frame").frame;
    let rows = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    (frame, rows)
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

fn invoked(outcome: &ClientShellInput) -> Vec<crate::api::schema::CommandInvokeParams> {
    outcome
        .actions
        .iter()
        .filter_map(|action| match action {
            ClientShellAction::Endpoint { request, .. } => match &request.method {
                Method::CommandInvoke(params) => Some(params.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Opens the panel by clicking the strip's square.
fn open(state: &mut ClientShellState) {
    let _ = rows_of(state, 120, 40);
    let square = state.hits.nirvana_preload;
    assert!(square.width > 0, "the strip draws a Preload square");
    click(state, square.x + 1, square.y);
    assert!(
        state.nirvana_preload.is_some(),
        "a click on the square opens the panel"
    );
}

#[test]
fn a_tab_running_claude_gets_the_strip_and_its_panes_a_row_less() {
    let dir = folder("strip");
    let plain = state_with(None, &dir, view(serde_json::json!({})));
    let codex = state_with(Some("codex"), &dir, view(serde_json::json!({})));
    let claude = state_with(Some("claude"), &dir, view(serde_json::json!({})));
    let rows = |state: &ClientShellState| state.surface_size(120, 40).rows;
    assert_eq!(rows(&codex), rows(&plain));
    assert_eq!(rows(&claude), rows(&plain) - 1);
    let layout = claude.layout(120, 40);
    // Under the tab bar, the panes below it.
    assert_eq!(layout.nirvana_mods.y, layout.tab_bar.bottom());
    assert_eq!(layout.nirvana_mods.height, 1);
    assert_eq!(layout.pane_surface.y, layout.nirvana_mods.bottom());
    assert!(plain.layout(120, 40).nirvana_mods.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_strip_holds_one_square_named_preload_with_the_folders_total() {
    let dir = folder("square");
    let local =
        serde_json::json!({"skillOverrides": {"benchmark": "off"}, "disableAllHooks": true});
    let mut state = state_with(Some("claude"), &dir, view(local));
    let (frame, rows) = rows_of(&mut state, 120, 40);
    let y = state.layout(120, 40).nirvana_mods.y as usize;
    let strip = &rows[y];
    assert!(strip.contains("\u{273B} Claude"), "{strip:?}");
    assert!(
        strip.contains(" Preload 23.6k \u{b7} 2 off \u{25BE}"),
        "{strip:?}"
    );
    assert_eq!(strip.matches("Preload").count(), 1, "one square: {strip:?}");
    // The square is raised, and an item switched off reads in yellow.
    let square = state.hits.nirvana_preload;
    let cell = |x: u16| &frame.cells[y * frame.width as usize + x as usize];
    let palette = &state.config.palette;
    assert_eq!(
        cell(square.x).bg,
        crate::protocol::color_to_u32(palette.surface0)
    );
    assert_eq!(
        cell(square.x + 10).fg,
        crate::protocol::color_to_u32(palette.yellow)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_panel_lists_each_group_and_item_with_its_tokens() {
    let dir = folder("panel");
    let mut state = state_with(Some("claude"), &dir, view(serde_json::json!({})));
    open(&mut state);
    let (_, rows) = rows_of(&mut state, 120, 40);
    let text = rows.join("\n");
    assert!(text.contains("Preload \u{b7} nirvana-mods-panel"), "{text}");
    assert!(text.contains("23.6k of 1m preloaded"), "{text}");
    for group in [
        "CLAUDE.md and memory",
        "Skills",
        "MCP servers and connectors",
        "Plugins",
    ] {
        assert!(text.contains(group), "{group}: {text}");
    }
    let row = |name: &str| {
        rows.iter()
            .find(|row| row.contains(name))
            .unwrap_or_else(|| panic!("{name}: {text}"))
            .clone()
    };
    assert!(row("MEMORY.md").contains("[x]"));
    assert!(row("MEMORY.md").contains("3.9k"));
    assert!(row("bladebro").contains("4.1k"));
    // A plugin's skill has no switch of its own: a dot, not a box.
    assert!(row("superpowers:brainstorming").contains(" \u{b7} "));
    assert!(!row("superpowers:brainstorming").contains("[x]"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn space_switches_the_selected_item_off_in_the_folders_local_settings() {
    let dir = folder("switch");
    let mut state = state_with(Some("claude"), &dir, view(serde_json::json!({})));
    open(&mut state);
    let _ = rows_of(&mut state, 120, 40);
    // The first item is the first memory file, CLAUDE.local.md by name.
    let outcome = key(&mut state, KeyCode::Char(' '));
    assert!(outcome.requests.is_empty(), "the key stays out of the pane");
    let written = std::fs::read_to_string(dir.join(".claude").join("settings.local.json"))
        .expect("the switch wrote the folder's local settings");
    let written: serde_json::Value = serde_json::from_str(&written).expect("json");
    assert_eq!(
        written["claudeMdExcludes"],
        serde_json::json!(["C:/work/proj/CLAUDE.local.md"])
    );
    let message = state
        .nirvana_preload
        .as_ref()
        .and_then(|panel| panel.message.clone())
        .unwrap_or_default();
    assert!(
        message.contains("off for new Claude sessions here"),
        "{message}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_click_on_a_box_switches_that_item() {
    let dir = folder("box");
    let mut state = state_with(Some("claude"), &dir, view(serde_json::json!({})));
    open(&mut state);
    let _ = rows_of(&mut state, 120, 40);
    let (rect, _) = state.hits.nirvana_panel.boxes[1];
    click(&mut state, rect.x + 1, rect.y);
    let written = std::fs::read_to_string(dir.join(".claude").join("settings.local.json"))
        .expect("the click wrote the folder's local settings");
    assert!(written.contains("claudeMdExcludes"), "{written}");
    assert!(state.nirvana_preload.is_some(), "the panel stays open");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn e_asks_the_server_to_open_the_items_file() {
    let dir = folder("edit");
    let file = dir.join("CLAUDE.md");
    std::fs::write(&file, "# rules\n").expect("a CLAUDE.md");
    let mut report = parse(FIXTURE).expect("report");
    report.memory.truncate(1);
    report.memory[0].name = file.display().to_string();
    let mut state = state_with(
        Some("claude"),
        &dir,
        View {
            report: Some(report),
            ..View::default()
        },
    );
    open(&mut state);
    let (_, rows) = rows_of(&mut state, 120, 40);
    assert!(rows
        .iter()
        .any(|row| row.contains("CLAUDE.md") && row.contains("edit")));
    let outcome = key(&mut state, KeyCode::Char('e'));
    let invoked = invoked(&outcome);
    assert_eq!(invoked.len(), 1, "{:?}", outcome.actions);
    assert_eq!(
        invoked[0].command_id,
        format!("{}{}", crate::nirvana::EDIT_FILE_COMMAND, file.display())
    );
    assert_eq!(invoked[0].tab_id.as_deref(), Some("tab_1"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn r_restarts_this_claude_so_it_takes_the_switches() {
    let dir = folder("restart");
    let mut state = state_with(Some("claude"), &dir, view(serde_json::json!({})));
    open(&mut state);
    let outcome = key(&mut state, KeyCode::Char('r'));
    let invoked = invoked(&outcome);
    assert_eq!(invoked.len(), 1, "{:?}", outcome.actions);
    assert_eq!(
        invoked[0].command_id,
        crate::nirvana::RESTART_CLAUDE_COMMAND
    );
    assert_eq!(invoked[0].tab_id.as_deref(), Some("tab_1"));
    assert!(state.nirvana_preload.is_none(), "the panel closes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keys_move_the_selection_and_esc_or_a_click_outside_closes() {
    let dir = folder("keys");
    let mut state = state_with(Some("claude"), &dir, view(serde_json::json!({})));
    open(&mut state);
    let outcome = key(&mut state, KeyCode::Char('j'));
    assert!(outcome.requests.is_empty() && outcome.actions.is_empty());
    assert_eq!(
        state.nirvana_preload.as_ref().map(|panel| panel.selected),
        Some(1)
    );
    key(&mut state, KeyCode::Esc);
    assert!(state.nirvana_preload.is_none());
    open(&mut state);
    let _ = rows_of(&mut state, 120, 40);
    let area = state.hits.nirvana_panel.area;
    click(&mut state, area.right() + 2, area.y + 3);
    assert!(
        state.nirvana_preload.is_none(),
        "a click outside the panel closes it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
