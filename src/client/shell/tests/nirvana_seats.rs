//! Plan 12 O8a: the Seats square of a Nirvana tab, composed from the run herdr recorded on 2026-10-18 (see
//! `src/nirvana_seats_square/tests.rs`): its narrow and wide forms, the panel's, a click that focuses the
//! worker's pane, and the mode key.

use super::*;
use crate::api::schema::Method;
use crate::nirvana_seats_square::set_home_for_test;

const RUNNING: &str = include_str!("../../../nirvana_seats_square/fixtures/rec1-running.jsonl");
const LEASES: &str = include_str!("../../../nirvana_seats_square/fixtures/leases.json");
/// 34 s after `mul` was seated, inside both leases' hour.
const NOW: u64 = 1_792_312_000;

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
    let dir = std::env::temp_dir().join(format!("nirvana-seats-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp folder");
    dir
}

/// The recorded project and home, and a window whose focused tab runs an agent named `name` in it. The worker's
/// pane `w1:p2` is in the snapshot, in another tab, as `seat run` opens it.
fn state_for(test: &str, name: &str) -> (ClientShellState, std::path::PathBuf, std::path::PathBuf) {
    let root = folder(&format!("{test}-proj"));
    std::fs::create_dir_all(root.join(".git")).expect(".git");
    let run = root.join(".nirvana").join("runs").join("rec1");
    std::fs::create_dir_all(&run).expect("run folder");
    std::fs::write(run.join("journal.jsonl"), RUNNING).expect("journal");
    let home = folder(&format!("{test}-home"));
    std::fs::write(home.join("seats.toml"), crate::nirvana_seat::DEFAULT).expect("seats.toml");
    std::fs::write(home.join("leases.json"), LEASES).expect("leases");
    set_home_for_test(Some(home.clone()), Some(NOW));
    crate::nirvana_preload::set_view_for_test(Some(crate::nirvana_preload::View::default()));
    let mut projected = snapshot();
    projected.agents = vec![agent(name)];
    let cwd = root.display().to_string();
    projected.panes[0].cwd = Some(cwd.clone());
    projected.panes[0].foreground_cwd = Some(cwd);
    projected.panes.push(ClientShellPane {
        pane_id: "w1:p2".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_2".into(),
        label: Some("flash\u{b7}mul".into()),
        cwd: None,
        foreground_cwd: None,
        focused: false,
        right_click_passthrough: false,
    });
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    (state, root, home)
}

fn done(root: std::path::PathBuf, home: std::path::PathBuf) {
    set_home_for_test(None, None);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(home);
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

/// Opens the panel with a click on the square, at a window `width` wide.
fn open(state: &mut ClientShellState, width: u16) -> Vec<String> {
    let _ = rows_of(state, width, 40);
    let square = state.hits.nirvana_seats;
    assert!(square.width > 0, "the strip draws a Seats square");
    click(state, square.x + 1, square.y);
    assert!(
        state.nirvana_seats.is_some(),
        "a click on the square opens it"
    );
    rows_of(state, width, 40)
}

/// The line holding `needle`.
fn line<'a>(rows: &'a [String], needle: &str) -> &'a str {
    rows.iter()
        .find(|row| row.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?}:\n{}", rows.join("\n")))
}

#[test]
fn a_nirvana_tab_gets_the_seats_square_and_a_plain_claude_tab_does_not() {
    let (mut plain, root, home) = state_for("plain", "moto");
    let row = strip(&mut plain, 200);
    assert!(!row.contains("Seats"), "{row:?}");
    assert_eq!(plain.hits.nirvana_seats.width, 0);
    done(root, home);
    for name in ["nirvana", "nirvana-2"] {
        let (mut state, root, home) = state_for("tab", name);
        let row = strip(&mut state, 200);
        // After Running, before Preload: the tab's seating, wide.
        assert!(
            row.contains(" Seats 2 open \u{b7} Balanced \u{25BE}"),
            "{name}: {row:?}"
        );
        let running = row.find(" Running").expect("Running");
        let seats = row.find(" Seats").expect("Seats");
        let preload = row.find(" Preload").expect("Preload");
        assert!(running < seats && seats < preload, "{row:?}");
        done(root, home);
    }
}

#[test]
fn the_square_is_narrow_in_a_narrow_strip() {
    let (mut state, root, home) = state_for("narrow-square", "nirvana");
    let row = strip(&mut state, 110);
    assert!(
        state.layout(110, 40).nirvana_mods.width
            < crate::client::shell::nirvana_seats::NARROW_STRIP,
        "the strip is narrow at 110"
    );
    assert!(row.contains(" Seats 2 \u{b7} Bal \u{25BE}"), "{row:?}");
    assert!(!row.contains("2 open"), "{row:?}");
    let wide = strip(&mut state, 200);
    assert!(wide.contains("2 open \u{b7} Balanced"), "{wide:?}");
    done(root, home);
}

#[test]
fn the_wide_panel_gives_each_open_order_one_row_of_columns_and_its_reason_line() {
    let (mut state, root, home) = state_for("wide", "nirvana");
    let rows = open(&mut state, 200);
    let text = rows.join("\n");
    let folder = root.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        text.contains(&format!("Seats \u{b7} {folder} \u{b7} Balanced")),
        "{text}"
    );
    assert!(line(&rows, "Mode").contains("[Balanced]"), "{text}");
    assert!(line(&rows, "Mode").contains("m changes it"), "{text}");
    assert!(
        line(&rows, "run rec1").contains("run rec1 \u{b7} 1 order \u{b7} sweep.mechanical 1"),
        "{text}"
    );
    let header = line(&rows, "seat effort");
    for column in ["order", "model", "pool", "state", "time", "use"] {
        assert!(header.contains(column), "{header:?}");
    }
    // One row of columns: order, seat and effort, model, pool, state, time, use.
    let mul = line(&rows, "mul ");
    for cell in [
        "flash low",
        "gemini-3.8-flash-low",
        "gemini ",
        "running",
        "34s",
        "5h ",
    ] {
        assert!(mul.contains(cell), "{cell:?} in {mul:?}");
    }
    let order = mul.find("mul").unwrap();
    let model = mul.find("gemini-3.8").unwrap();
    let state_at = mul.find("running").unwrap();
    assert!(order < model && model < state_at, "{mul:?}");
    let reason = line(&rows, "\u{21b3} sweep.mechanical");
    assert!(
        reason.contains(
            "\u{21b3} sweep.mechanical \u{2192} flash low: gemini 5h 1.0% used, week 1% used"
        ),
        "{reason:?}"
    );
    let fix = line(&rows, "fix1");
    for cell in ["sonnet medium", "claude-sonnet-5-5", "max", "booked", "~"] {
        assert!(fix.contains(cell), "{cell:?} in {fix:?}");
    }
    assert!(
        text.contains("\u{21b3} build.bounded \u{2192} sonnet medium"),
        "{text}"
    );
    assert!(text.contains("enter focus its pane"), "{text}");
    done(root, home);
}

#[test]
fn the_narrow_panel_gives_each_order_three_lines() {
    let (mut state, root, home) = state_for("narrow-panel", "nirvana");
    let rows = open(&mut state, 110);
    let text = rows.join("\n");
    assert!(!text.contains("seat effort"), "no column header: {text}");
    assert!(
        line(&rows, "mul \u{b7} running 34s").contains("mul \u{b7} running 34s"),
        "{text}"
    );
    let seat = line(
        &rows,
        "flash low \u{b7} gemini-3.8-flash-low \u{b7} gemini \u{b7} 5h ",
    );
    assert!(seat.contains("  flash low"), "{seat:?}");
    assert!(
        text.contains("\u{21b3} sweep.mechanical \u{2192} flash low"),
        "{text}"
    );
    // Booked 17 s after `mul` was seated.
    assert!(text.contains("fix1 \u{b7} booked 17s"), "{text}");
    done(root, home);
}

#[test]
fn a_click_on_an_order_focuses_its_workers_pane_and_a_booked_one_says_why_not() {
    let (mut state, root, home) = state_for("click", "nirvana");
    let _ = open(&mut state, 200);
    let hits = state.hits.nirvana_seats_panel.clone();
    let rect = |index: usize| {
        hits.orders
            .iter()
            .find(|(_, at)| *at == index)
            .map(|(rect, _)| *rect)
            .expect("an order line")
    };
    // `fix1` is the second row: booked, no pane.
    let fix = rect(1);
    let outcome = click(&mut state, fix.x + 2, fix.y);
    assert!(focused(&outcome).is_empty(), "{:?}", outcome.actions);
    let message = state
        .nirvana_seats
        .as_ref()
        .and_then(|panel| panel.message.clone())
        .expect("a message");
    assert_eq!(message, "fix1 is booked, not started: no worker pane yet");
    // Any line of `mul`, its reason line too, focuses `w1:p2` and closes the panel.
    let _ = rows_of(&mut state, 200, 40);
    let hits = state.hits.nirvana_seats_panel.clone();
    let mul_lines: Vec<Rect> = hits
        .orders
        .iter()
        .filter(|(_, at)| *at == 0)
        .map(|(rect, _)| *rect)
        .collect();
    assert_eq!(mul_lines.len(), 2, "wide: its row and its reason line");
    let reason = mul_lines[1];
    let outcome = click(&mut state, reason.x + 4, reason.y);
    assert_eq!(focused(&outcome), vec!["w1:p2".to_owned()]);
    assert!(state.nirvana_seats.is_none(), "the panel closes");
    // Enter on the selected order does the same.
    let _ = open(&mut state, 200);
    let outcome = key(&mut state, KeyCode::Enter);
    assert_eq!(focused(&outcome), vec!["w1:p2".to_owned()]);
    done(root, home);
}

#[test]
fn the_mode_key_moves_the_mode_in_seats_toml_logs_it_and_shows_it() {
    let (mut state, root, home) = state_for("mode", "nirvana");
    let _ = open(&mut state, 200);
    let outcome = key(&mut state, KeyCode::Char('m'));
    assert!(outcome.requests.is_empty(), "the key stays out of the pane");
    let project_file = root.join(".nirvana/seats.toml");
    let table = std::fs::read_to_string(&project_file).unwrap();
    assert_eq!(
        crate::nirvana_seat::parse_project_seats(&project_file, &table).unwrap(),
        crate::nirvana_seat::Mode::Sota
    );
    let machine_table = std::fs::read_to_string(home.join("seats.toml")).unwrap();
    assert_eq!(
        crate::nirvana_seat::parse(&machine_table).unwrap().mode,
        crate::nirvana_seat::Mode::Balanced
    );
    let journal = std::fs::read_to_string(root.join(".nirvana/runs/rec1/journal.jsonl")).unwrap();
    let last: serde_json::Value = serde_json::from_str(journal.lines().last().unwrap()).unwrap();
    assert_eq!(last["event"], "mode_changed");
    assert_eq!(
        (last["from"].as_str(), last["to"].as_str()),
        (Some("balanced"), Some("sota"))
    );
    assert_eq!(last["by"], "seats square");
    let rows = rows_of(&mut state, 200, 40);
    let text = rows.join("\n");
    assert!(line(&rows, "Mode").contains("[SOTA]"), "{text}");
    assert!(
        text.contains("mode SOTA: the next seat pick uses it; logged in run rec1"),
        "{text}"
    );
    assert!(
        line(&rows, "run rec1").contains("mode \u{2192} SOTA"),
        "{text}"
    );
    // The orders seated at Balanced say so now.
    assert!(
        text.contains("\u{21b3} seated at Balanced \u{b7} sweep.mechanical"),
        "{text}"
    );
    assert!(
        text.contains("\u{21b3} booked at Balanced \u{b7} build.bounded"),
        "{text}"
    );
    let row = strip(&mut state, 200);
    assert!(row.contains("2 open \u{b7} SOTA"), "{row:?}");
    // A click on the mode row moves it on: SOTA to Efficient.
    let mode = state.hits.nirvana_seats_panel.mode;
    let _ = click(&mut state, mode.x + 1, mode.y);
    let table = std::fs::read_to_string(&project_file).unwrap();
    assert_eq!(
        crate::nirvana_seat::parse_project_seats(&project_file, &table).unwrap(),
        crate::nirvana_seat::Mode::Efficient
    );
    let machine_table = std::fs::read_to_string(home.join("seats.toml")).unwrap();
    assert_eq!(
        crate::nirvana_seat::parse(&machine_table).unwrap().mode,
        crate::nirvana_seat::Mode::Balanced
    );
    done(root, home);
}

#[test]
fn the_seats_square_mode_row_shows_the_source_word() {
    let (mut state, root, home) = state_for("source-word", "nirvana");
    let rows = open(&mut state, 200);
    let text = rows.join("\n");
    assert!(
        line(&rows, "Mode").contains("\u{b7} machine"),
        "expected machine source word in mode row: {text}"
    );

    let outcome = key(&mut state, KeyCode::Char('m'));
    assert!(outcome.requests.is_empty(), "the key stays out of the pane");
    let rows = rows_of(&mut state, 200, 40);
    let text = rows.join("\n");
    assert!(
        line(&rows, "Mode").contains("\u{b7} this project"),
        "expected project source word in mode row: {text}"
    );
    done(root, home);
}

#[test]
fn one_panel_is_open_at_a_time_with_the_seats_panel_among_them() {
    let (mut state, root, home) = state_for("one-panel", "nirvana");
    let _ = rows_of(&mut state, 200, 40);
    state.toggle_seats_panel();
    assert!(state.nirvana_seats.is_some());
    state.toggle_preload_panel();
    assert!(state.nirvana_preload.is_some() && state.nirvana_seats.is_none());
    state.toggle_seats_panel();
    assert!(state.nirvana_seats.is_some() && state.nirvana_preload.is_none());
    state.toggle_running_panel();
    assert!(state.nirvana_running.is_some() && state.nirvana_seats.is_none());
    state.toggle_seats_panel();
    assert!(state.nirvana_seats.is_some() && state.nirvana_running.is_none());
    state.toggle_memory_panel();
    assert!(state.nirvana_memory.is_some() && state.nirvana_seats.is_none());
    state.toggle_seats_panel();
    assert!(state.nirvana_seats.is_some() && state.nirvana_memory.is_none());
    state.toggle_accounts_panel();
    assert!(state.nirvana_accounts.is_some() && state.nirvana_seats.is_none());
    state.toggle_seats_panel();
    assert!(state.nirvana_seats.is_some() && state.nirvana_accounts.is_none());
    state.toggle_seats_panel();
    assert!(state.nirvana_seats.is_none(), "the second toggle closes it");
    done(root, home);
}

#[test]
fn an_open_seats_panel_is_composed_again_when_a_pane_prints() {
    use super::super::surface_patch::ClientPaneSurfacePatchOutcome;
    let (mut state, root, home) = state_for("patch", "nirvana");
    let _ = open(&mut state, 200);
    let current = state.pane_surface.clone().expect("pane surface");
    let mut cell = current.frame.cells[0].clone();
    cell.symbol = "y".into();
    let patch = crate::protocol::PaneSurfacePatch {
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
    };
    // Not written straight to the screen over the panel: the frame is composed again, panel on top.
    let outcome = state.apply_pane_surface_patch(patch);
    assert!(
        matches!(outcome, ClientPaneSurfacePatchOutcome::Applied(None)),
        "the patch is taken and the frame composed again"
    );
    done(root, home);
}

#[test]
fn sota_word_is_red_in_title_and_mode_row_when_sota_is_on_not_otherwise() {
    let (mut state, root, home) = state_for("sota-red", "nirvana");
    let red_u32 = crate::protocol::color_to_u32(state.config.palette.red);

    // Initially mode is Balanced.
    let _ = open(&mut state, 200);
    let area = state.hits.nirvana_seats_panel.area;
    let mode_row = state.hits.nirvana_seats_panel.mode;

    // Check Balanced title: contains "Balanced" (not red)
    let frame = state.compose(200, 40).expect("composed frame").frame;
    let title_cells: Vec<_> = (area.x..area.right())
        .map(|x| &frame.cells[(area.y * frame.width + x) as usize])
        .collect();
    let title_text: String = title_cells.iter().map(|c| c.symbol.as_str()).collect();
    assert!(title_text.contains("Balanced"));
    assert!(!title_cells.iter().any(|c| c.fg == red_u32));

    // Check mode row in Balanced: [Balanced] is green, SOTA is muted (not red)
    let mode_cells: Vec<_> = (mode_row.x..mode_row.right())
        .map(|x| &frame.cells[(mode_row.y * frame.width + x) as usize])
        .collect();
    let mode_text: String = mode_cells.iter().map(|c| c.symbol.as_str()).collect();
    assert!(mode_text.contains("[Balanced]"));
    assert!(mode_text.contains(" SOTA "));
    assert!(!mode_cells.iter().any(|c| c.fg == red_u32));

    // Now press 'm' to change mode to SOTA!
    let _ = key(&mut state, KeyCode::Char('m'));
    let frame = state.compose(200, 40).expect("composed frame").frame;

    // Check SOTA title: contains "SOTA", and the cells for "SOTA" are red!
    let title_cells: Vec<_> = (area.x..area.right())
        .map(|x| &frame.cells[(area.y * frame.width + x) as usize])
        .collect();
    let title_text: String = title_cells.iter().map(|c| c.symbol.as_str()).collect();
    assert!(title_text.contains("SOTA"));
    let sota_title_red = title_cells.windows(4).any(|w| {
        w[0].symbol == "S"
            && w[1].symbol == "O"
            && w[2].symbol == "T"
            && w[3].symbol == "A"
            && w.iter().all(|c| c.fg == red_u32)
    });
    assert!(sota_title_red, "SOTA in title must be red");

    // Check mode row in SOTA: [SOTA] is red!
    let mode_cells: Vec<_> = (mode_row.x..mode_row.right())
        .map(|x| &frame.cells[(mode_row.y * frame.width + x) as usize])
        .collect();
    let mode_text: String = mode_cells.iter().map(|c| c.symbol.as_str()).collect();
    assert!(mode_text.contains("[SOTA]"));
    let sota_mode_red = mode_cells.windows(4).any(|w| {
        w[0].symbol == "S"
            && w[1].symbol == "O"
            && w[2].symbol == "T"
            && w[3].symbol == "A"
            && w.iter().all(|c| c.fg == red_u32)
    });
    assert!(sota_mode_red, "SOTA in mode row must be red");

    // Check Seats square in mod strip in SOTA: SOTA is red!
    let square = state.hits.nirvana_seats;
    assert!(square.width > 0);
    let square_cells: Vec<_> = (square.x..square.right())
        .map(|x| &frame.cells[(square.y * frame.width + x) as usize])
        .collect();
    let sota_square_red = square_cells.windows(4).any(|w| {
        w[0].symbol == "S"
            && w[1].symbol == "O"
            && w[2].symbol == "T"
            && w[3].symbol == "A"
            && w.iter().all(|c| c.fg == red_u32)
    });
    assert!(sota_square_red, "SOTA in mod strip square must be red");

    // Now press 'm' again to change mode to Efficient!
    let _ = key(&mut state, KeyCode::Char('m'));
    let frame = state.compose(200, 40).expect("composed frame").frame;

    // Check title in Efficient: no red
    let title_cells: Vec<_> = (area.x..area.right())
        .map(|x| &frame.cells[(area.y * frame.width + x) as usize])
        .collect();
    assert!(!title_cells.iter().any(|c| c.fg == red_u32));

    // Check mode row in Efficient: [Efficient] is green, SOTA is muted (not red)
    let mode_cells: Vec<_> = (mode_row.x..mode_row.right())
        .map(|x| &frame.cells[(mode_row.y * frame.width + x) as usize])
        .collect();
    assert!(!mode_cells.iter().any(|c| c.fg == red_u32));

    let square = state.hits.nirvana_seats;
    let square_cells: Vec<_> = (square.x..square.right())
        .map(|x| &frame.cells[(square.y * frame.width + x) as usize])
        .collect();
    assert!(!square_cells.windows(4).any(|w| {
        w[0].symbol == "S"
            && w[1].symbol == "O"
            && w[2].symbol == "T"
            && w[3].symbol == "A"
            && w.iter().all(|c| c.fg == red_u32)
    }));

    done(root, home);
}

#[test]
fn the_seats_square_shows_orchestrator_row_with_and_without_restart_to_switch() {
    let (mut state, root, home) = state_for("orch-row", "nirvana");

    // The home's own readings: the max week 90% used a day before its reset, a climb past 1, so Balanced seats
    // sonnet medium. The first build wrote none, and its square ran the real `claude /usage` to get the owner's.
    let usage = format!(
        r#"{{"version":1,"sources":{{"claude":{{"read_at":{NOW},"readings":[{{"pool":"max","window":"week","used":0.9,"resets_at":{},"read_at":{NOW},"source":"claude /usage"}}]}}}}}}"#,
        NOW + 86_400
    );
    std::fs::write(home.join("usage.json"), usage).unwrap();
    let sessions_dir = home.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    // Case 0: another project's session on opus medium is not this project's: no running seat, no restart line.
    let other = crate::nirvana_claude::SessionSeatRecord {
        session_id: "other-sess".into(),
        model: "claude-opus-5-5".into(),
        seat: "opus".into(),
        effort: "medium".into(),
        last_notified_seat: None,
        last_notified_effort: None,
    };
    std::fs::write(
        sessions_dir.join("other-sess.json"),
        serde_json::to_string(&other).unwrap(),
    )
    .unwrap();
    let rows_other = open(&mut state, 200);
    let orch_other = line(&rows_other, "Nirvana Claude");
    assert!(
        orch_other.contains("Nirvana Claude \u{b7} sonnet medium"),
        "the rule on the home's readings: {orch_other}"
    );
    assert!(
        !orch_other.contains("restart to switch"),
        "another project's seat must not count: {orch_other}"
    );

    // Case 1: Without restart to switch (running seat matches rule)
    // This folder's session record (plan 13 §8) names the session; its seat record is sonnet medium.
    std::fs::write(
        sessions_dir.join(format!(
            "{}.json",
            crate::nirvana_launch::folder_hash(&root)
        )),
        r#"{"session_id":"orch-sess-1","at":"2026-10-20T00:00:00Z","transcript":"t.jsonl"}"#,
    )
    .unwrap();
    let session_rec = crate::nirvana_claude::SessionSeatRecord {
        session_id: "orch-sess-1".into(),
        model: "claude-sonnet-5-5".into(),
        seat: "sonnet".into(),
        effort: "medium".into(),
        last_notified_seat: None,
        last_notified_effort: None,
    };
    std::fs::write(
        sessions_dir.join("orch-sess-1.json"),
        serde_json::to_string(&session_rec).unwrap(),
    )
    .unwrap();

    let rows = rows_of(&mut state, 200, 40);
    let orch_line = line(&rows, "Nirvana Claude");
    assert!(
        orch_line.contains("Nirvana Claude \u{b7} sonnet medium"),
        "expected orchestrator row: {orch_line}"
    );
    assert!(
        !orch_line.contains("restart to switch"),
        "must not contain restart to switch when matching: {orch_line}"
    );

    // Case 2: With restart to switch (running seat differs from rule)
    // Switch running session to opus medium
    let session_rec_opus = crate::nirvana_claude::SessionSeatRecord {
        session_id: "orch-sess-1".into(),
        model: "claude-opus-5-5".into(),
        seat: "opus".into(),
        effort: "medium".into(),
        last_notified_seat: None,
        last_notified_effort: None,
    };
    std::fs::write(
        sessions_dir.join("orch-sess-1.json"),
        serde_json::to_string(&session_rec_opus).unwrap(),
    )
    .unwrap();

    let rows_diff = rows_of(&mut state, 200, 40);
    let orch_diff = line(&rows_diff, "Nirvana Claude");
    assert!(
        orch_diff.contains("Nirvana Claude \u{b7} sonnet medium"),
        "rule still seats sonnet medium: {orch_diff}"
    );
    assert!(
        orch_diff.contains("restart to switch"),
        "must contain restart to switch when differing: {orch_diff}"
    );

    done(root, home);
}
