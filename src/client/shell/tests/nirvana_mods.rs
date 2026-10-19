//! Nirvana fork: the mod strip and its Preload mod (nirvana/plans/2026-10-16-05-preload-mod.md L1-L4).

use super::*;
use crate::api::schema::Method;
use crate::nirvana_preload::{parse, set_view_for_test, Switch, View, FIXTURE};

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
    // Tall enough for every group.
    let (_, rows) = rows_of(&mut state, 120, 70);
    let text = rows.join("\n");
    assert!(text.contains("Preload \u{b7} nirvana-mods-panel"), "{text}");
    assert!(
        text.contains("23.6k a turn \u{b7} 129.9k deferred"),
        "{text}"
    );
    assert!(
        text.contains("no preset yet: Open Nirvana chooses one"),
        "{text}"
    );
    // Grouped by provider (feature 7), in this order: a header follows the panel's border, an item's
    // row starts with its box.
    let headers = [
        "Jev token savers (herdr)",
        "This project",
        "You (~/.claude)",
        "Plugin github",
        "Plugin superpowers",
        "claude.ai",
        "Built in (Claude Code)",
    ];
    let at: Vec<usize> = headers
        .iter()
        .map(|header| {
            rows.iter()
                .position(|row| row.contains(&format!("\u{2502} {header}")))
                .unwrap_or_else(|| panic!("{header}: {text}"))
        })
        .collect();
    assert!(
        at.windows(2).all(|pair| pair[0] < pair[1]),
        "{at:?}: {text}"
    );
    // Built in's header counts the built-in tools, which /context does not split per tool.
    assert!(rows[at[6]].contains("7.5k"), "{}", rows[at[6]]);
    // Provider sums count used tokens only.
    assert!(rows[at[2]].contains("5.1k"), "{}", rows[at[2]]);
    assert!(rows[at[3]].contains("1.1k"), "{}", rows[at[3]]);
    assert!(rows[at[5]].contains("1.1k"), "{}", rows[at[5]]);
    let row = |name: &str| {
        rows.iter()
            .find(|row| row.contains(name))
            .unwrap_or_else(|| panic!("{name}: {text}"))
            .clone()
    };
    assert!(row("MEMORY.md").contains("[x]"));
    assert!(row("MEMORY.md").contains("3.9k"));
    assert!(row("bladebro").contains("1.1k"));
    assert!(row("bladebro").contains("deferred 4.1k"));
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
    // The first items are herdr's three Jev rows (feature 9); the fourth is the first memory file,
    // CLAUDE.local.md by name.
    for _ in 0..3 {
        key(&mut state, KeyCode::Down);
    }
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
    // Boxes 0 and 1 are the Jev guide and compact advice (feature 9); 3 is the second memory file.
    let (rect, _) = state.hits.nirvana_panel.boxes[3];
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
    // Past herdr's three Jev rows (feature 9) to the memory file.
    for _ in 0..3 {
        key(&mut state, KeyCode::Down);
    }
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

// ---- Feature 6: the Running square (nirvana/plans/2026-10-16-06-running-and-waiting.md L1, L2) ----

use crate::nirvana_running::{set_activity_for_test, Activity, Tool};

/// The AskUserQuestion prompt as Claude Code 2.1.301 drew it on 2026-10-16.
const QUESTION: [&str; 10] = [
    "────────────────────────────────────",
    " \u{2610} Button colour",
    "Which colour should the button be?",
    "\u{276F} 1. Red (Recommended)",
    "     Red button colour.",
    "  2. Blue",
    "     Blue button colour.",
    "  3. Type something.",
    "  4. Chat about this",
    "Enter to select \u{b7} Esc to cancel",
];

fn prompt_surface(lines: &[&str]) -> PaneSurfaceFrame {
    let width = 40;
    let padded: Vec<String> = lines.iter().map(|line| format!("{line:<width$}")).collect();
    let buffer = Buffer::with_lines(padded.iter().map(String::as_str));
    let mut surface = surface();
    surface.frame = FrameData::from_ratatui_buffer_with_hyperlinks(&buffer, None, &[]);
    let rect = SurfaceRect {
        x: 0,
        y: 0,
        width: width as u16,
        height: lines.len() as u16,
    };
    surface.panes[0].rect = rect;
    surface.panes[0].inner_rect = rect;
    surface
}

fn running_state(status: AgentStatus, activity: Option<Activity>) -> ClientShellState {
    let dir = std::env::temp_dir();
    let mut state = state_with(Some("claude"), &dir, view(serde_json::json!({})));
    set_activity_for_test(activity);
    let mut projected = state.snapshot.as_deref().cloned().expect("snapshot");
    projected.agents[0].agent_status = status;
    projected.agents[0].terminal_title_stripped = Some("\u{2733} Button colour question".into());
    projected.agents[0].tokens = vec![(
        crate::nirvana_spaces::TRANSCRIPT_TOKEN.into(),
        r"C:\t\session.jsonl".into(),
    )];
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(prompt_surface(&QUESTION));
    state
}

fn working() -> Activity {
    Activity {
        asked: Some("Fix the flaky login test".into()),
        asked_at: Some(1_000),
        said: Some("I will run the tests to see which one fails.".into()),
        now: Some(Tool {
            name: "Bash".into(),
            detail: "Run the login tests".into(),
            since: Some(1_030),
        }),
    }
}

fn open_running(state: &mut ClientShellState) {
    let _ = rows_of(state, 120, 40);
    let square = state.hits.nirvana_running;
    assert!(square.width > 0, "the strip draws a Running square");
    click(state, square.x + 1, square.y);
    assert!(
        state.nirvana_running.is_some(),
        "a click on the square opens the panel"
    );
}

#[test]
fn the_strip_puts_running_before_preload_and_says_when_claude_waits() {
    let mut state = running_state(AgentStatus::Working, Some(working()));
    let (_, rows) = rows_of(&mut state, 120, 40);
    let y = state.layout(120, 40).nirvana_mods.y as usize;
    let strip = rows[y].clone();
    assert!(
        strip.contains(" Running working \u{b7} Bash \u{25BE}"),
        "{strip:?}"
    );
    assert!(strip.find("Running") < strip.find("Preload"), "{strip:?}");
    let mut state = running_state(AgentStatus::Blocked, Some(working()));
    let (frame, rows) = rows_of(&mut state, 120, 40);
    assert!(
        rows[y].contains(" Running waits on you \u{25BE}"),
        "{:?}",
        rows[y]
    );
    let square = state.hits.nirvana_running;
    let cell = &frame.cells[y * frame.width as usize + square.x as usize + 10];
    assert_eq!(
        cell.fg,
        crate::protocol::color_to_u32(state.config.palette.yellow),
        "waiting reads in yellow"
    );
}

#[test]
fn the_panel_shows_running_on_top_and_nothing_waiting_under_it() {
    let mut state = running_state(AgentStatus::Working, Some(working()));
    open_running(&mut state);
    let (_, rows) = rows_of(&mut state, 120, 40);
    let text = rows.join("\n");
    let line = |needle: &str| {
        rows.iter()
            .position(|row| row.contains(needle))
            .unwrap_or_else(|| panic!("{needle}: {text}"))
    };
    assert!(text.contains("Task   Button colour question"), "{text}");
    assert!(text.contains("Asked  Fix the flaky login test"), "{text}");
    assert!(text.contains("Now    Bash  Run the login tests"), "{text}");
    assert!(
        text.contains("Said   I will run the tests to see which one fails."),
        "{text}"
    );
    assert!(line("Running \u{b7} ") < line("Task"));
    assert!(line("Said") < line("Waiting on you"));
    assert!(line("Waiting on you") < line("Nothing needs you"));
}

#[test]
fn a_waiting_prompt_is_read_from_the_pane_and_a_digit_answers_it() {
    let mut state = running_state(AgentStatus::Blocked, Some(working()));
    open_running(&mut state);
    let (_, rows) = rows_of(&mut state, 120, 40);
    let text = rows.join("\n");
    assert!(
        text.contains("Which colour should the button be?"),
        "{text}"
    );
    assert!(text.contains(" 1  Red (Recommended)"), "{text}");
    assert!(text.contains(" 4  Chat about this"), "{text}");
    assert!(text.contains("1-4 answer in the pane"), "{text}");
    let outcome = key(&mut state, KeyCode::Char('2'));
    // Typed into the agent's pane as a key, the way the window types; no API method, since a window
    // may not send `pane.send_keys` or `agent.send_keys`.
    assert!(outcome.actions.is_empty(), "{:?}", outcome.actions);
    assert_eq!(outcome.requests.len(), 1, "{:?}", outcome.requests);
    let ClientMessage::ClientShellPaneInput { pane_id, events } = &outcome.requests[0] else {
        panic!("a pane input: {:?}", outcome.requests);
    };
    assert_eq!(pane_id, "pane_1");
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(
        format!("{:?}", events[0]).contains("Char('2')"),
        "{events:?}"
    );
    // A digit that is not an option sends nothing.
    let outcome = key(&mut state, KeyCode::Char('7'));
    assert!(outcome.requests.is_empty(), "{:?}", outcome.requests);
}

#[test]
fn a_click_on_an_option_answers_it() {
    let mut state = running_state(AgentStatus::Blocked, None);
    open_running(&mut state);
    let _ = rows_of(&mut state, 120, 40);
    let (rect, digit) = state.hits.nirvana_running_panel.options[0];
    assert_eq!(digit, 1);
    let outcome = click(&mut state, rect.x + 1, rect.y);
    let answered = outcome.requests.iter().any(|request| {
        matches!(request, ClientMessage::ClientShellPaneInput { pane_id, events }
            if pane_id == "pane_1" && format!("{events:?}").contains("Char('1')"))
    });
    assert!(answered, "{:?}", outcome.requests);
}

#[test]
fn a_screen_with_no_waiting_agent_answers_nothing() {
    // The prompt text is on screen but herdr says Claude is working: nothing is offered.
    let mut state = running_state(AgentStatus::Working, None);
    open_running(&mut state);
    let (_, rows) = rows_of(&mut state, 120, 40);
    assert!(rows.iter().any(|row| row.contains("Nothing needs you")));
    let outcome = key(&mut state, KeyCode::Char('1'));
    assert!(outcome.requests.is_empty(), "{:?}", outcome.requests);
}

#[test]
fn one_panel_at_a_time_and_esc_closes() {
    let mut state = running_state(AgentStatus::Idle, None);
    open_running(&mut state);
    let _ = rows_of(&mut state, 120, 40);
    let preload = state.hits.nirvana_preload;
    // A click outside the Running panel closes it.
    click(&mut state, preload.x + 1, preload.y);
    assert!(state.nirvana_running.is_none());
    state.toggle_running_panel();
    state.toggle_preload_panel();
    assert!(
        state.nirvana_running.is_none(),
        "opening Preload closes Running"
    );
    assert!(state.nirvana_preload.is_some());
    state.toggle_running_panel();
    assert!(
        state.nirvana_preload.is_none(),
        "opening Running closes Preload"
    );
    key(&mut state, KeyCode::Esc);
    assert!(state.nirvana_running.is_none());
}

// Feature 8: the Memory square and the shared board (nirvana/plans/2026-10-16-08-memory-square-and-board.md).

/// A folder that is a repository, with a GitHub remote or another one.
fn repo(name: &str, remote: &str) -> std::path::PathBuf {
    let dir = folder(name);
    std::fs::create_dir_all(dir.join(".git")).expect(".git");
    std::fs::write(
        dir.join(".git").join("config"),
        format!("[remote \"origin\"]\n\turl = {remote}\n"),
    )
    .expect("config");
    dir
}

fn strip_row(state: &mut ClientShellState) -> String {
    let (_, rows) = rows_of(state, 160, 40);
    let y = state.layout(160, 40).nirvana_mods.y as usize;
    rows[y].clone()
}

#[test]
fn the_memory_square_shows_only_in_a_repository_with_a_github_remote() {
    let plain = folder("memory-plain");
    let gitlab = repo("memory-gitlab", "https://gitlab.com/x/y.git");
    let github = repo("memory-github", "git@github.com:x/y.git");
    for dir in [&plain, &gitlab] {
        let mut state = state_with(Some("claude"), dir, view(serde_json::json!({})));
        let strip = strip_row(&mut state);
        assert!(!strip.contains("Memory"), "{strip:?}");
        assert!(state.hits.nirvana_memory.width == 0);
    }
    let mut state = state_with(Some("claude"), &github, view(serde_json::json!({})));
    let strip = strip_row(&mut state);
    assert!(
        strip.contains(" Memory board empty \u{b7} not shared \u{25BE}"),
        "{strip:?}"
    );
    assert!(state.hits.nirvana_memory.width > 0);
    for dir in [plain, gitlab, github] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn the_memory_panel_lists_the_board_shares_it_and_edits_it() {
    let dir = repo("memory-panel", "https://github.com/x/y.git");
    crate::nirvana_board::write(&dir, "Server", "port 5173", 0, "codex").expect("written");
    let mut state = state_with(Some("claude"), &dir, view(serde_json::json!({})));
    let strip = strip_row(&mut state);
    assert!(
        strip.contains(" Memory board r1 \u{b7} not shared"),
        "{strip:?}"
    );
    // One panel at a time: Memory closes Preload.
    state.toggle_preload_panel();
    state.toggle_memory_panel();
    assert!(state.nirvana_preload.is_none() && state.nirvana_memory.is_some());
    state.toggle_memory_panel();
    // A click on the square opens it.
    let square = state.hits.nirvana_memory;
    click(&mut state, square.x + 1, square.y);
    assert!(
        state.nirvana_memory.is_some(),
        "a click on the square opens the panel"
    );
    let (_, rows) = rows_of(&mut state, 160, 40);
    let text = rows.join("\n");
    assert!(
        text.contains("Memory \u{b7} nirvana-mods-memory-panel"),
        "{text}"
    );
    assert!(
        text.contains("Board \u{b7} .nirvana/board.md \u{b7} revision 1"),
        "{text}"
    );
    assert!(
        text.contains("[ ] Share the board with every Claude here"),
        "{text}"
    );
    assert!(
        text.contains("Server") && text.contains("port 5173") && text.contains("codex"),
        "{text}"
    );
    // Space shares: the prompt hook is written to the folder's local settings, and the square says so.
    let outcome = key(&mut state, KeyCode::Char(' '));
    assert!(outcome.requests.is_empty(), "the key stays out of the pane");
    let local = crate::nirvana_preload::read_local(&dir).expect("settings");
    assert!(crate::nirvana_board::is_shared(&local), "{local}");
    let (_, rows) = rows_of(&mut state, 160, 40);
    let text = rows.join("\n");
    assert!(text.contains("[x] Share the board"), "{text}");
    // This Claude has not seen the board: the section is new to it.
    assert!(text.contains("1 new for this Claude"), "{text}");
    // e opens board.md in the editor tab.
    let outcome = key(&mut state, KeyCode::Char('e'));
    let invoked = invoked(&outcome);
    assert_eq!(invoked.len(), 1, "{:?}", outcome.actions);
    assert_eq!(
        invoked[0].command_id,
        format!(
            "{}{}",
            crate::nirvana::EDIT_FILE_COMMAND,
            crate::nirvana_board::board_path(&dir).display()
        )
    );
    assert!(state.nirvana_memory.is_none(), "the panel closes");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A pane update for the first pane, as the server sends one while an agent prints.
fn pane_patch(state: &ClientShellState) -> crate::protocol::PaneSurfacePatch {
    let current = state.pane_surface.clone().expect("pane surface");
    let mut cell = current.frame.cells[0].clone();
    cell.symbol = "y".into();
    crate::protocol::PaneSurfacePatch {
        boot_id: current.boot_id,
        projection_revision: current.projection_revision,
        base_surface_revision: current.surface_revision,
        surface_revision: current.surface_revision + 1,
        panes: current.panes.clone(),
        rows: vec![crate::protocol::PaneSurfacePatchRow {
            x: 0,
            y: 0,
            cells: vec![cell],
        }],
        cursor: current.frame.cursor.clone(),
    }
}

#[test]
fn an_open_panel_is_redrawn_when_its_agent_prints() {
    use super::super::surface_patch::ClientPaneSurfacePatchOutcome;
    let dir = repo("memory-patch", "https://github.com/x/y.git");
    for panel in ["none", "preload", "running", "memory"] {
        let mut state = state_with(Some("claude"), &dir, view(serde_json::json!({})));
        let _ = rows_of(&mut state, 160, 40);
        match panel {
            "preload" => state.toggle_preload_panel(),
            "running" => state.toggle_running_panel(),
            "memory" => state.toggle_memory_panel(),
            _ => {}
        }
        let _ = rows_of(&mut state, 160, 40);
        let outcome = state.apply_pane_surface_patch(pane_patch(&state));
        // Written straight to the screen only with no panel open; with one open the frame is composed
        // again, panel on top.
        let straight = matches!(outcome, ClientPaneSurfacePatchOutcome::Applied(Some(_)));
        assert!(
            matches!(outcome, ClientPaneSurfacePatchOutcome::Applied(_)),
            "{panel}: the patch is taken"
        );
        assert_eq!(straight, panel == "none", "{panel}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Feature 9: the Jev group comes first, its switches are herdr's own hooks, and the guide also
/// reaches Codex and agy through the project's AGENTS.md.
#[test]
fn the_jev_group_switches_the_guide_for_every_agent_and_compact_advice_for_claude() {
    let dir = folder("jev");
    let mut state = state_with(Some("claude"), &dir, view(serde_json::json!({})));
    open(&mut state);
    let (_, rows) = rows_of(&mut state, 120, 70);
    let text = rows.join("\n");
    let at = |needle: &str| {
        rows.iter()
            .position(|row| row.contains(needle))
            .unwrap_or_else(|| panic!("{needle}: {text}"))
    };
    assert!(
        at("Jev token savers (herdr)") < at("This project"),
        "{text}"
    );
    assert!(
        rows[at("Jev memory")].contains("[ ]"),
        "off until switched: {text}"
    );
    assert!(rows[at("Compact advice")].contains("[ ]"), "{text}");
    // Under test no key is read: the row says what is missing.
    assert!(rows[at("today")].contains("no Jev key"), "{text}");

    // Rows sort by name inside a group: Compact advice, Jev memory, today.
    assert!(
        at("Compact advice") < at("Jev memory") && at("Jev memory") < at("today"),
        "{text}"
    );
    key(&mut state, KeyCode::Char(' '));
    let message = state
        .nirvana_preload
        .as_ref()
        .and_then(|panel| panel.message.clone())
        .unwrap_or_default();
    assert!(message.contains("Compact advice on"), "{message}");
    key(&mut state, KeyCode::Down);
    key(&mut state, KeyCode::Char(' '));
    let local: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join(".claude").join("settings.local.json")).expect("written"),
    )
    .expect("json");
    let command = |event: &str| {
        local["hooks"][event][0]["hooks"][0]["command"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    assert!(
        command("SessionStart").ends_with("jev guide --hook"),
        "{local}"
    );
    assert!(
        command("Stop").ends_with("jev compact"),
        "the second switch keeps the first: {local}"
    );
    let agents =
        std::fs::read_to_string(dir.join("AGENTS.md")).expect("AGENTS.md for Codex and agy");
    // An empty folder is small: Codex and agy get the one-line flag (feature 10).
    assert!(agents.contains("jev file|files|text"), "{agents}");
    let message = state
        .nirvana_preload
        .as_ref()
        .and_then(|panel| panel.message.clone())
        .unwrap_or_default();
    assert!(message.contains("Codex and agy"), "{message}");
    // Switching off is `nirvana_jev_install::tests`: this panel's view is fixed under test, so a row
    // reads off however often it is switched.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_header_line_shows_active_preset_and_deferred_tokens() {
    let dir = folder("header-active");
    let mut v = view(serde_json::json!({}));
    v.preset = crate::nirvana_preset::PresetView {
        name: "3d-print".into(),
        reason: "matched 3mf marker".into(),
        off: vec![
            (Switch::Skill("benchmark".into()), "3d".into()),
            (Switch::McpServer("bladebro".into()), "3d".into()),
        ],
        saved_tokens: 5_102,
        pinned_on: Vec::new(),
    };
    let mut state = state_with(Some("claude"), &dir, v);
    open(&mut state);
    let (_, rows) = rows_of(&mut state, 130, 40);
    let text = rows.join("\n");
    assert!(
        text.contains("23.6k a turn \u{b7} 129.9k deferred \u{b7} Preset 3d-print \u{b7} 2 off \u{b7} ~5.1k a turn (estimate) \u{b7} p undo"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_header_line_shows_undo_preset_and_never_opened_reason() {
    let dir = folder("header-undo");
    let mut v = view(serde_json::json!({}));
    v.preset = crate::nirvana_preset::PresetView {
        name: crate::nirvana_preset::OFF.into(),
        reason: "3d-print (turned off with p)".into(),
        off: Vec::new(),
        saved_tokens: 0,
        pinned_on: Vec::new(),
    };
    let mut state = state_with(Some("claude"), &dir, v);
    open(&mut state);
    let (_, rows) = rows_of(&mut state, 120, 40);
    let text = rows.join("\n");
    assert!(
        text.contains("23.6k a turn \u{b7} 129.9k deferred \u{b7} Preset off \u{b7} p on"),
        "{text}"
    );

    // When folder was never opened (empty name), shows PresetView.reason
    let mut v_never = view(serde_json::json!({}));
    v_never.preset = crate::nirvana_preset::PresetView {
        name: String::new(),
        reason: "no preset yet: Open Nirvana chooses one".into(),
        off: Vec::new(),
        saved_tokens: 0,
        pinned_on: Vec::new(),
    };
    let mut state_never = state_with(Some("claude"), &dir, v_never);
    open(&mut state_never);
    let (_, rows_never) = rows_of(&mut state_never, 120, 40);
    let text_never = rows_never.join("\n");
    assert!(
        text_never.contains(
            "23.6k a turn \u{b7} 129.9k deferred \u{b7} no preset yet: Open Nirvana chooses one"
        ),
        "{text_never}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_header_line_works_in_narrow_and_wide_forms() {
    let dir = folder("header-narrow");
    let mut v = view(serde_json::json!({}));
    v.preset = crate::nirvana_preset::PresetView {
        name: "3d-print".into(),
        reason: "rule".into(),
        off: vec![(Switch::Skill("benchmark".into()), "3d".into())],
        saved_tokens: 5_102,
        pinned_on: Vec::new(),
    };
    let mut state = state_with(Some("claude"), &dir, v);
    open(&mut state);

    // Wide form: width 120
    let (_, wide_rows) = rows_of(&mut state, 120, 40);
    let wide_text = wide_rows.join("\n");
    assert!(
        wide_text.contains("23.6k a turn \u{b7} 129.9k deferred \u{b7} Preset 3d-print"),
        "{wide_text}"
    );

    // Narrow form: width 50
    let (_, narrow_rows) = rows_of(&mut state, 50, 40);
    let narrow_text = narrow_rows.join("\n");
    assert!(
        narrow_text.contains("23.6k a turn \u{b7} 129.9k deferred"),
        "{narrow_text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn preset_off_rows_render_p_mark_and_tokens_in_wide_and_narrow() {
    let dir = folder("preset-off-mark");
    let mut v = view(serde_json::json!({}));
    v.preset = crate::nirvana_preset::PresetView {
        name: "rust".into(),
        reason: "rule".into(),
        off: vec![(Switch::Skill("benchmark".into()), "python".into())],
        saved_tokens: 112,
        pinned_on: Vec::new(),
    };
    let mut state = state_with(Some("claude"), &dir, v);
    open(&mut state);

    // Wide form
    let (wide_frame, wide_rows) = rows_of(&mut state, 120, 40);
    let wide_row = wide_rows
        .iter()
        .find(|row| row.contains("benchmark"))
        .expect("benchmark listed");
    assert!(wide_row.contains("[p]"), "{wide_row}");
    assert!(wide_row.contains("112"), "{wide_row}");

    // Cell color check: [p] is drawn with palette.yellow
    let panel = state.nirvana_preload.as_ref().unwrap();
    let items = crate::client::shell::nirvana_mods::panel_items(
        panel,
        &crate::nirvana_preload::view(&panel.cwd),
    );
    let (box_rect, _) = state
        .hits
        .nirvana_panel
        .boxes
        .iter()
        .find(|(_, idx)| items[*idx].name == "benchmark")
        .unwrap();
    let cell =
        &wide_frame.cells[box_rect.y as usize * wide_frame.width as usize + box_rect.x as usize];
    assert_eq!(
        cell.fg,
        crate::protocol::color_to_u32(state.config.palette.yellow)
    );

    // Narrow form (width 50)
    let (_, narrow_rows) = rows_of(&mut state, 50, 40);
    let narrow_row = narrow_rows
        .iter()
        .find(|row| row.contains("benchmark"))
        .expect("benchmark listed narrow");
    assert!(narrow_row.contains("[p]"), "{narrow_row}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn space_pins_preset_off_row_and_unpins_pinned_row() {
    let _home = crate::nirvana::with_test_home("space-pins");
    let dir = folder("space-pins");
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"sp\"\n").unwrap();
    let seat_dir = crate::nirvana_seat::dir();
    let _ = crate::nirvana_preset::load_config(&seat_dir).unwrap();
    let reading_file = seat_dir.join("preload").join("reading.json");
    std::fs::create_dir_all(reading_file.parent().unwrap()).unwrap();
    let reading_json = serde_json::json!({
        "at": 1000,
        "total": 23607,
        "items": {
            "skill:benchmark": { "tokens": 112, "about": "bench" }
        }
    });
    std::fs::write(&reading_file, serde_json::to_string(&reading_json).unwrap()).unwrap();
    crate::nirvana_preset::append_catalog(
        &seat_dir,
        &[(
            "skill:benchmark".into(),
            vec!["python".into()],
            "rule".into(),
        )],
    )
    .unwrap();
    let outcome =
        crate::nirvana_preset::apply(&seat_dir, &dir, None, std::time::Duration::from_millis(50))
            .unwrap();
    assert_eq!(outcome.record.choice, "rust");

    let mut v = view(serde_json::json!({}));
    v.preset = crate::nirvana_preset::view(&dir);
    let mut state = state_with(Some("claude"), &dir, v);
    open(&mut state);

    // Find index of benchmark in panel items
    let panel = state.nirvana_preload.as_ref().unwrap();
    let items = crate::client::shell::nirvana_mods::panel_items(
        panel,
        &crate::nirvana_preload::view(&panel.cwd),
    );
    let bench_idx = items
        .iter()
        .position(|it| it.name == "benchmark")
        .expect("benchmark item");
    assert!(items[bench_idx].preset_off);

    // Select benchmark row
    state.nirvana_preload.as_mut().unwrap().selected = bench_idx;

    // Press Space: pins it on
    key(&mut state, KeyCode::Char(' '));
    let msg = state
        .nirvana_preload
        .as_ref()
        .unwrap()
        .message
        .clone()
        .unwrap_or_default();
    assert!(msg.contains("benchmark pinned on"), "{msg}");

    // Panel items now reflect pinned_on
    let panel = state.nirvana_preload.as_ref().unwrap();
    let updated_items = crate::client::shell::nirvana_mods::panel_items(
        panel,
        &crate::nirvana_preload::view(&panel.cwd),
    );
    assert!(updated_items[bench_idx].preset_pinned);
    assert!(updated_items[bench_idx].on);

    let (_, updated_rows) = rows_of(&mut state, 120, 50);
    let bench_row = updated_rows
        .iter()
        .find(|r| r.contains("benchmark"))
        .unwrap();
    assert!(bench_row.contains("[x]"), "{bench_row}");

    // Press Space again: unpins it
    key(&mut state, KeyCode::Char(' '));
    let msg2 = state
        .nirvana_preload
        .as_ref()
        .unwrap()
        .message
        .clone()
        .unwrap_or_default();
    assert!(msg2.contains("benchmark pin removed"), "{msg2}");

    let panel = state.nirvana_preload.as_ref().unwrap();
    let unpinned_items = crate::client::shell::nirvana_mods::panel_items(
        panel,
        &crate::nirvana_preload::view(&panel.cwd),
    );
    assert!(unpinned_items[bench_idx].preset_off);
    assert!(!unpinned_items[bench_idx].on);

    let (_, unpinned_rows) = rows_of(&mut state, 120, 50);
    let bench_row2 = unpinned_rows
        .iter()
        .find(|r| r.contains("benchmark"))
        .unwrap();
    assert!(bench_row2.contains("[p]"), "{bench_row2}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn p_key_toggles_preset_on_and_off_and_updates_header() {
    let _home = crate::nirvana::with_test_home("p-key-toggle");
    let dir = folder("p-key-toggle");
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"pt\"\n").unwrap();
    let seat_dir = crate::nirvana_seat::dir();
    let _ = crate::nirvana_preset::load_config(&seat_dir).unwrap();
    let _ =
        crate::nirvana_preset::apply(&seat_dir, &dir, None, std::time::Duration::from_millis(50))
            .unwrap();

    let mut v = view(serde_json::json!({}));
    v.preset = crate::nirvana_preset::view(&dir);
    assert_eq!(v.preset.name, "rust");

    let mut state = state_with(Some("claude"), &dir, v);
    open(&mut state);
    let (_, rows) = rows_of(&mut state, 120, 40);
    let text = rows.join("\n");
    assert!(text.contains("Preset rust"), "{text}");
    assert!(text.contains("p undo"), "{text}");

    // Press 'p': toggles preset off
    key(&mut state, KeyCode::Char('p'));
    let (_, rows_off) = rows_of(&mut state, 120, 40);
    let text_off = rows_off.join("\n");
    assert!(text_off.contains("Preset off \u{b7} p on"), "{text_off}");

    // Press 'p' again: toggles preset back on
    key(&mut state, KeyCode::Char('p'));
    let (_, rows_on) = rows_of(&mut state, 120, 40);
    let text_on = rows_on.join("\n");
    assert!(text_on.contains("Preset rust"), "{text_on}");
    assert!(text_on.contains("p undo"), "{text_on}");

    let _ = std::fs::remove_dir_all(&dir);
}

type TrimmerFn = Box<dyn Fn(&str) -> Result<String, String> + Send + Sync>;
struct FakeTrimmer(TrimmerFn);
impl crate::nirvana_memory_trim::Trimmer for FakeTrimmer {
    fn rewrite(&self, original: &str, _limit: u64) -> Result<String, String> {
        (self.0)(original)
    }
}

#[test]
fn memory_row_shows_over_limit_proposal_and_responds_to_t_a_x_keys() {
    let _home = crate::nirvana::with_test_home("memory-row-trim");
    let dir = folder("memory-row-trim");
    let projects_dir = _home.path.join("projects");
    std::env::set_var(crate::nirvana_memory_trim::PROJECTS_ENV, &projects_dir);
    std::env::set_var(crate::nirvana_memory_trim::LIMIT_ENV, "4000");

    let enc = crate::nirvana_memory_trim::encode(&dir);
    let mem_file = projects_dir.join(enc).join("memory").join("MEMORY.md");
    std::fs::create_dir_all(mem_file.parent().unwrap()).unwrap();

    // ~18,500 bytes => content_tokens is ~4,625 tokens (over 4,000 limit)
    let padding = "word ".repeat(3_700);
    let original = format!("# Auto Memory\n- [Entry 1](entry1.md) \u{2014} {padding}\n");
    std::fs::write(&mem_file, &original).unwrap();

    // Verify status says over limit
    let status = crate::nirvana_memory_trim::status(&dir).expect("status exists");
    assert!(status.over);
    assert_eq!(status.limit, 4_000);

    // Setup trimmer that produces ~4,225 tokens (still over 4,000), saving ~400 tokens (0.4k)
    let shorter_padding = "word ".repeat(3_380);
    let proposed = format!("# Auto Memory\n- [Entry 1](entry1.md) \u{2014} {shorter_padding}\n");
    let prop_saved = crate::nirvana_memory_trim::tokens_of(&original)
        .saturating_sub(crate::nirvana_memory_trim::tokens_of(&proposed));
    assert!((350..=450).contains(&prop_saved)); // ~0.4k
    assert!(crate::nirvana_memory_trim::tokens_of(&proposed) > 4_000);

    let proposed_clone = proposed.clone();
    crate::nirvana_memory_trim::set_trimmer_for_test(Some(std::sync::Arc::new(FakeTrimmer(
        Box::new(move |_| Ok(proposed_clone.clone())),
    ))));

    let mut v = view(serde_json::json!({}));
    v.trim = Some(status);
    let mut state = state_with(Some("claude"), &dir, v);
    open(&mut state);

    // Wide and narrow check for "over 4k · t trim"
    let (_, rows) = rows_of(&mut state, 120, 40);
    let mem_row = rows
        .iter()
        .find(|r| r.contains("MEMORY.md"))
        .expect("MEMORY.md row");
    assert!(mem_row.contains("over 4k \u{b7} t trim"), "{mem_row}");

    let (_, narrow_rows) = rows_of(&mut state, 50, 40);
    let mem_narrow = narrow_rows
        .iter()
        .find(|r| r.contains("MEMORY.md"))
        .expect("MEMORY.md narrow");
    assert!(mem_narrow.contains("over 4k"), "{mem_narrow}");

    // Press 't': runs propose off draw path -> shows "−0.4k · a accept · x discard"
    key(&mut state, KeyCode::Char('t'));
    let (_, rows_proposed) = rows_of(&mut state, 120, 40);
    let mem_row_prop = rows_proposed
        .iter()
        .find(|r| r.contains("MEMORY.md"))
        .unwrap();
    assert!(
        mem_row_prop.contains("\u{2212}0.4k \u{b7} a accept \u{b7} x discard"),
        "{mem_row_prop}"
    );

    // Press 'x': discards proposal -> goes back to "over 4k · t trim"
    key(&mut state, KeyCode::Char('x'));
    let (_, rows_discarded) = rows_of(&mut state, 120, 40);
    let mem_row_disc = rows_discarded
        .iter()
        .find(|r| r.contains("MEMORY.md"))
        .unwrap();
    assert!(
        mem_row_disc.contains("over 4k \u{b7} t trim"),
        "{mem_row_disc}"
    );

    // Press 't' again to propose again
    key(&mut state, KeyCode::Char('t'));
    let (_, rows_prop2) = rows_of(&mut state, 120, 40);
    assert!(rows_prop2
        .iter()
        .find(|r| r.contains("MEMORY.md"))
        .unwrap()
        .contains("a accept"));

    // Press 'a': accepts proposal -> replaces MEMORY.md with shorter one
    key(&mut state, KeyCode::Char('a'));
    let (_, rows_accepted) = rows_of(&mut state, 120, 40);
    let mem_row_acc = rows_accepted
        .iter()
        .find(|r| r.contains("MEMORY.md"))
        .unwrap();
    assert!(
        mem_row_acc.contains("over 4k \u{b7} t trim"),
        "mem_row_acc was: {mem_row_acc}"
    );

    // Now test trimming down under limit
    let under_text = "# Auto Memory\n- [Entry 1](entry1.md)\n";
    let under_clone = under_text.to_owned();
    crate::nirvana_memory_trim::set_trimmer_for_test(Some(std::sync::Arc::new(FakeTrimmer(
        Box::new(move |_| Ok(under_clone.clone())),
    ))));
    key(&mut state, KeyCode::Char('t'));
    key(&mut state, KeyCode::Char('a'));
    let (_, rows_under) = rows_of(&mut state, 120, 40);
    let mem_row_under = rows_under.iter().find(|r| r.contains("MEMORY.md")).unwrap();
    assert!(mem_row_under.contains("auto memory"), "{mem_row_under}");

    // Test error case: when propose errors, shows error on detail without panic
    crate::nirvana_memory_trim::set_trimmer_for_test(Some(std::sync::Arc::new(FakeTrimmer(
        Box::new(|_| Err("Jev cannot trim this file".into())),
    ))));
    // Write back over-limit file
    std::fs::write(&mem_file, &original).unwrap();
    crate::nirvana_preload::refresh_trim(&dir);
    key(&mut state, KeyCode::Char('t'));
    let (_, rows_err) = rows_of(&mut state, 120, 40);
    let mem_row_err = rows_err.iter().find(|r| r.contains("MEMORY.md")).unwrap();
    assert!(
        mem_row_err.contains("Jev cannot trim this file"),
        "{mem_row_err}"
    );

    crate::nirvana_memory_trim::set_trimmer_for_test(None);
    let _ = std::fs::remove_dir_all(&dir);
}
