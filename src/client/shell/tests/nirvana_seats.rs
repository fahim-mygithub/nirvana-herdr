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
        crate::nirvana_seat::parse_project_seats(&project_file, &table)
            .unwrap()
            .unwrap(),
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
        crate::nirvana_seat::parse_project_seats(&project_file, &table)
            .unwrap()
            .unwrap(),
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

/// Plan 40a: the square shows SOTA's fallback with why ("SOTA, Fable week 82% used") and the restart line against a
/// session on Fable; and review M2's line when the installed seats.toml differs from herdr's default.
#[test]
fn c40a_the_square_shows_sotas_fallback_and_an_edited_table() {
    let (mut state, root, home) = state_for("c40a-square", "nirvana");
    let usage = |fable: f32| {
        format!(
            r#"{{"version":1,"sources":{{"claude":{{"read_at":{NOW},"readings":[{{"pool":"max","window":"5h","used":0.1,"resets_at":{r},"read_at":{NOW},"source":"claude /usage"}},{{"pool":"max","window":"week","used":0.2,"resets_at":{r},"read_at":{NOW},"source":"claude /usage"}},{{"pool":"max","window":"week.fable","used":{fable},"resets_at":{r},"read_at":{NOW},"source":"claude /usage"}}]}}}}}}"#,
            r = NOW + 86_400
        )
    };
    std::fs::write(
        home.join("seats.toml"),
        crate::nirvana_seat::DEFAULT.replace("mode = \"balanced\"", "mode = \"sota\""),
    )
    .unwrap();
    std::fs::write(home.join("usage.json"), usage(0.82)).unwrap();
    let sessions_dir = home.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::write(
        sessions_dir.join(format!(
            "{}.json",
            crate::nirvana_launch::folder_hash(&root)
        )),
        r#"{"session_id":"c40a-sess","at":"2026-11-09T00:00:00Z","transcript":"t.jsonl"}"#,
    )
    .unwrap();
    let running = crate::nirvana_claude::SessionSeatRecord {
        session_id: "c40a-sess".into(),
        model: "claude-fable-5-2".into(),
        seat: "fable".into(),
        effort: "high".into(),
        last_notified_seat: None,
        last_notified_effort: None,
    };
    std::fs::write(
        sessions_dir.join("c40a-sess.json"),
        serde_json::to_string(&running).unwrap(),
    )
    .unwrap();
    let rows = open(&mut state, 200);
    let orch = line(&rows, "Nirvana Claude");
    assert!(
        orch.contains("Nirvana Claude \u{b7} opus high \u{b7} SOTA, Fable week 82% used \u{b7} restart to switch"),
        "{orch}"
    );
    assert!(!rows
        .iter()
        .any(|row| row.contains("differs from herdr's default")));
    // An edited table: one line says so. Plan 46 P46b: it is the View's `differs_line`, which names a plain value the
    // upgrade carries, where the panel printed the fixed `DIFFERS_LINE` before.
    std::fs::write(
        home.join("seats.toml"),
        crate::nirvana_seat::DEFAULT
            .replace("mode = \"balanced\"", "mode = \"sota\"")
            .replace("floor = 0.05", "floor = 0.06"),
    )
    .unwrap();
    std::fs::write(home.join("usage.json"), usage(0.02)).unwrap();
    let rows = rows_of(&mut state, 200, 40);
    let _ = line(
        &rows,
        "seats.toml keeps your floor = 0.06; the rest is herdr's default.",
    );
    let orch = line(&rows, "Nirvana Claude");
    assert!(
        orch.contains("Nirvana Claude \u{b7} fable high \u{b7} SOTA"),
        "{orch}"
    );
    assert!(!orch.contains("restart to switch"), "{orch}");
    done(root, home);
}

/// The home reads the max week 90% used a day before its reset, so Balanced seats sonnet medium; this folder's
/// Nirvana Claude runs on `seat` medium. `opus` makes the row say `restart to switch`, `sonnet` does not.
fn running_on(home: &std::path::Path, root: &std::path::Path, seat: &str) {
    let usage = format!(
        r#"{{"version":1,"sources":{{"claude":{{"read_at":{NOW},"readings":[{{"pool":"max","window":"week","used":0.9,"resets_at":{},"read_at":{NOW},"source":"claude /usage"}}]}}}}}}"#,
        NOW + 86_400
    );
    std::fs::write(home.join("usage.json"), usage).unwrap();
    let sessions_dir = home.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::write(
        sessions_dir.join(format!("{}.json", crate::nirvana_launch::folder_hash(root))),
        r#"{"session_id":"p87-sess","at":"2026-11-18T00:00:00Z","transcript":"t.jsonl"}"#,
    )
    .unwrap();
    let record = crate::nirvana_claude::SessionSeatRecord {
        session_id: "p87-sess".into(),
        model: format!("claude-{seat}-5-5"),
        seat: seat.into(),
        effort: "medium".into(),
        last_notified_seat: None,
        last_notified_effort: None,
    };
    std::fs::write(
        sessions_dir.join("p87-sess.json"),
        serde_json::to_string(&record).unwrap(),
    )
    .unwrap();
}

type Restart = (Option<String>, Option<String>, Option<String>);

/// The Restart Claude requests in an outcome: (workspace, tab, pane).
pub(super) fn restarts(outcome: &ClientShellInput) -> Vec<Restart> {
    outcome
        .actions
        .iter()
        .filter_map(|action| match action {
            ClientShellAction::Endpoint { request, .. } => match &request.method {
                Method::CommandInvoke(params)
                    if params.command_id == crate::nirvana::RESTART_CLAUDE_COMMAND =>
                {
                    Some((
                        params.workspace_id.clone(),
                        params.tab_id.clone(),
                        params.pane_id.clone(),
                    ))
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn the_orchestrator() -> Restart {
    (
        Some("ws_1".into()),
        Some("tab_1".into()),
        Some("pane_1".into()),
    )
}

/// P87 U1: the owner, 2026-11-18, "We should be able to click restart in this modal". The row's `restart to switch`
/// words are a click target and `r`, shown in the footer, does the same: both ask the server's Restart Claude for
/// this panel's Nirvana Claude pane, and the panel stays open to show the row clear.
#[test]
fn p87_the_restart_words_and_r_restart_the_panels_nirvana_claude() {
    let (mut state, root, home) = state_for("p87-click", "nirvana");
    running_on(&home, &root, "opus");
    let rows = open(&mut state, 200);
    let orch = line(&rows, "Nirvana Claude");
    assert!(orch.contains("sonnet medium"), "{orch}");
    assert!(orch.contains("restart to switch"), "{orch}");
    let _ = line(&rows, "r restart \u{b7} m mode \u{b7} esc close");
    let words = state.hits.nirvana_seats_panel.restart;
    assert_eq!(
        usize::from(words.width),
        "restart to switch".len(),
        "{words:?}"
    );
    let drawn: String = rows[usize::from(words.y)]
        .chars()
        .skip(usize::from(words.x))
        .take(usize::from(words.width))
        .collect();
    assert_eq!(drawn, "restart to switch", "the hit is on the words");
    let outcome = click(&mut state, words.x + 3, words.y);
    assert_eq!(restarts(&outcome), vec![the_orchestrator()]);
    assert!(state.nirvana_seats.is_some(), "the panel stays open");
    let message = state
        .nirvana_seats
        .as_ref()
        .and_then(|panel| panel.message.clone())
        .expect("a message");
    assert_eq!(
        message,
        "restarting Nirvana Claude on sonnet medium, same conversation"
    );
    // `r` does the same, on a panel with no restart sent yet (the one above holds a second press: the next test).
    forget_restarts(&mut state);
    let outcome = key(&mut state, KeyCode::Char('r'));
    assert_eq!(restarts(&outcome), vec![the_orchestrator()]);
    done(root, home);
}

/// Forgets the restarts sent, as if they were sent long ago and shown.
fn forget_restarts(state: &mut ClientShellState) {
    state.nirvana_restarting.clear();
    if let Some(panel) = state.nirvana_seats.as_mut() {
        panel.restarting = None;
    }
}

/// Moves every restart sent, and an armed press, `secs` into the past.
fn age(state: &mut ClientShellState, secs: u64) {
    let back = |at: std::time::Instant| {
        at.checked_sub(std::time::Duration::from_secs(secs))
            .expect("the clock reaches that far back")
    };
    for sent in state.nirvana_restarting.values_mut() {
        sent.at = back(sent.at);
    }
    if let Some(panel) = state.nirvana_seats.as_mut() {
        if let Some(sent) = panel.restarting.as_mut() {
            sent.at = back(sent.at);
        }
        panel.restart_armed = panel.restart_armed.map(back);
    }
}

fn mouse(state: &mut ClientShellState, kind: MouseEventKind, column: u16, row: u16) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })])
}

fn armed(state: &ClientShellState) -> bool {
    state
        .nirvana_seats
        .as_ref()
        .is_some_and(|panel| panel.restart_armed.is_some())
}

/// P87: a working Nirvana Claude waits for a second press, as the header's reload badge does; the row and the
/// footer say why, and any other key puts the restart back.
#[test]
fn p87_a_working_nirvana_claude_restarts_on_the_second_press() {
    let (mut state, root, home) = state_for("p87-working", "nirvana");
    running_on(&home, &root, "opus");
    let mut projected = state.snapshot.as_deref().cloned().expect("snapshot");
    projected.agents[0].agent_status = AgentStatus::Working;
    state.set_snapshot(Box::new(projected));
    let _ = open(&mut state, 200);
    let outcome = key(&mut state, KeyCode::Char('r'));
    assert!(restarts(&outcome).is_empty(), "{:?}", outcome.actions);
    let rows = rows_of(&mut state, 200, 40);
    let _ = line(&rows, "restart? r again");
    let _ = line(
        &rows,
        "Nirvana Claude is working: r again restarts it now and cuts its turn",
    );
    // Another key disarms; the next `r` arms again and restarts nothing.
    let _ = key(&mut state, KeyCode::Down);
    assert!(!armed(&state));
    let outcome = key(&mut state, KeyCode::Char('r'));
    assert!(restarts(&outcome).is_empty());
    let _ = rows_of(&mut state, 200, 40);
    // The armed words are the click target too: the second press restarts.
    let words = state.hits.nirvana_seats_panel.restart;
    assert_eq!(usize::from(words.width), "restart? r again".len());
    let outcome = click(&mut state, words.x, words.y);
    assert_eq!(restarts(&outcome), vec![the_orchestrator()]);
    done(root, home);
}

/// P87: once the running seat matches the mode's, the next draw has no `restart to switch`, no button and no `r`
/// in the footer, and `r` says so and restarts nothing.
#[test]
fn p87_no_restart_offered_when_the_running_seat_matches() {
    let (mut state, root, home) = state_for("p87-match", "nirvana");
    running_on(&home, &root, "opus");
    let rows = open(&mut state, 200);
    assert!(line(&rows, "Nirvana Claude").contains("restart to switch"));
    // The restarted Claude's SessionStart writes its new seat.
    running_on(&home, &root, "sonnet");
    let rows = rows_of(&mut state, 200, 40);
    let orch = line(&rows, "Nirvana Claude");
    assert!(!orch.contains("restart"), "{orch}");
    assert!(!rows.iter().any(|row| row.contains("r restart")));
    assert_eq!(state.hits.nirvana_seats_panel.restart, Rect::default());
    let outcome = key(&mut state, KeyCode::Char('r'));
    assert!(restarts(&outcome).is_empty());
    assert_eq!(
        state.nirvana_seats.as_ref().unwrap().message.as_deref(),
        Some("Nirvana Claude already runs the mode's seat: no restart needed")
    );
    done(root, home);
}

/// P87 U2: the space's own menu says when its restart switches the seat, from the same view the square reads.
#[test]
fn p87_the_space_menu_says_the_restart_switches_the_seat() {
    let (mut state, root, home) = state_for("p87-menu", "nirvana");
    running_on(&home, &root, "opus");
    let labels = |state: &mut ClientShellState| {
        state.open_workspace_context_menu("ws_1".into(), 0, 0);
        let Some(ClientShellOverlay::ContextMenu(menu)) = state.overlay.as_ref() else {
            panic!("space menu");
        };
        let labels: Vec<&str> = menu.items().iter().map(|item| item.label).collect();
        state.overlay = None;
        labels
    };
    assert_eq!(labels(&mut state)[1], "Restart Claude to switch seat");
    running_on(&home, &root, "sonnet");
    assert_eq!(labels(&mut state)[1], "Restart Claude");
    done(root, home);
}

/// P87 fix round (review of aa340405, item b1): the row reads `restart to switch` until the fresh Claude's seat record
/// is read, so a second click or `r` in that window restarted the fresh Claude again. Now the words read
/// `restarting…` and are no button, and neither press sends a second restart, until the view stops asking (the new
/// seat is read) or `RESTART_PENDING_WINDOW` passes, after which the row asks again.
#[test]
fn p87_a_second_press_does_not_restart_the_fresh_claude_again() {
    let (mut state, root, home) = state_for("p87-twice", "nirvana");
    running_on(&home, &root, "opus");
    let _ = open(&mut state, 200);
    let words = state.hits.nirvana_seats_panel.restart;
    let outcome = click(&mut state, words.x + 3, words.y);
    assert_eq!(restarts(&outcome), vec![the_orchestrator()]);
    // The fresh Claude reads Idle and its seat record is not rewritten yet: the view still asks.
    let rows = rows_of(&mut state, 200, 40);
    let orch = line(&rows, "Nirvana Claude");
    assert!(orch.contains("restarting\u{2026}"), "{orch}");
    assert!(!orch.contains("restart to switch"), "{orch}");
    assert_eq!(state.hits.nirvana_seats_panel.restart, Rect::default());
    let outcome = click(&mut state, words.x + 3, words.y);
    assert!(restarts(&outcome).is_empty(), "a second click");
    let outcome = key(&mut state, KeyCode::Char('r'));
    assert!(restarts(&outcome).is_empty(), "a second r");
    assert_eq!(
        state.nirvana_seats.as_ref().unwrap().message.as_deref(),
        Some("Nirvana Claude is restarting: the row clears once its new seat is recorded")
    );
    // The space's menu holds it too, and so does the panel opened again.
    state.nirvana_seats = None;
    state.open_workspace_context_menu("ws_1".into(), 0, 0);
    let mut outcome = ClientShellInput::default();
    state.activate_context_menu_item(1, &mut outcome);
    assert!(restarts(&outcome).is_empty(), "the space menu");
    state.toggle_seats_panel();
    let rows = rows_of(&mut state, 200, 40);
    assert!(line(&rows, "Nirvana Claude").contains("restarting\u{2026}"));
    // The window passes with the row still asking (the restart failed): it is a button again.
    age(&mut state, 16);
    let rows = rows_of(&mut state, 200, 40);
    assert!(line(&rows, "Nirvana Claude").contains("restart to switch"));
    let outcome = key(&mut state, KeyCode::Char('r'));
    assert_eq!(restarts(&outcome), vec![the_orchestrator()]);
    // The new seat is read: the row clears and the sent restart is dropped.
    running_on(&home, &root, "sonnet");
    let rows = rows_of(&mut state, 200, 40);
    assert!(!line(&rows, "Nirvana Claude").contains("restart"));
    assert_eq!(state.nirvana_seats.as_ref().unwrap().restarting, None);
    done(root, home);
}

/// P87 fix round (item b2): an armed press keeps its prompt in the footer for as long as it is armed, holds over a
/// pointer move, and lapses after `RESTART_ARM_WINDOW`; a lapsed press is a first press again.
#[test]
fn p87_an_armed_restart_keeps_its_prompt_and_lapses() {
    let (mut state, root, home) = state_for("p87-lapse", "nirvana");
    running_on(&home, &root, "opus");
    let mut projected = state.snapshot.as_deref().cloned().expect("snapshot");
    projected.agents[0].agent_status = AgentStatus::Working;
    state.set_snapshot(Box::new(projected));
    let _ = open(&mut state, 200);
    let prompt = "Nirvana Claude is working: r again restarts it now and cuts its turn";
    let _ = key(&mut state, KeyCode::Char('r'));
    assert!(armed(&state));
    // Whatever clears the message, the footer says it while armed.
    state.nirvana_seats.as_mut().unwrap().message = None;
    let rows = rows_of(&mut state, 200, 40);
    let _ = line(&rows, prompt);
    let words = state.hits.nirvana_seats_panel.restart;
    let _ = mouse(&mut state, MouseEventKind::Moved, words.x, words.y);
    assert!(armed(&state), "a move toward the words keeps it");
    age(&mut state, 6);
    let rows = rows_of(&mut state, 200, 40);
    assert!(!rows.iter().any(|row| row.contains(prompt)), "the prompt goes with it");
    assert!(line(&rows, "Nirvana Claude").contains("restart to switch"));
    let outcome = key(&mut state, KeyCode::Char('r'));
    assert!(restarts(&outcome).is_empty(), "a lapsed press restarts nothing");
    assert!(armed(&state), "it arms again");
    done(root, home);
}

/// P87 fix round (item b6): any other click cancels an armed press: one on the panel off the words, a right-click on
/// the words, and the wheel. The next press on the words then arms again and restarts nothing.
#[test]
fn p87_another_click_or_the_wheel_cancels_an_armed_restart() {
    let (mut state, root, home) = state_for("p87-cancel", "nirvana");
    running_on(&home, &root, "opus");
    let mut projected = state.snapshot.as_deref().cloned().expect("snapshot");
    projected.agents[0].agent_status = AgentStatus::Working;
    state.set_snapshot(Box::new(projected));
    let _ = open(&mut state, 200);
    let words = state.hits.nirvana_seats_panel.restart;
    let area = state.hits.nirvana_seats_panel.area;
    // The blank line above the footer: inside the panel, on nothing.
    let blank = (area.x + 3, area.bottom().saturating_sub(3));
    let cancels: [(MouseEventKind, (u16, u16)); 3] = [
        (MouseEventKind::Down(MouseButton::Left), blank),
        (MouseEventKind::Down(MouseButton::Right), (words.x, words.y)),
        (MouseEventKind::ScrollDown, (words.x, words.y)),
    ];
    for (kind, (column, row)) in cancels {
        let outcome = click(&mut state, words.x, words.y);
        assert!(restarts(&outcome).is_empty());
        assert!(armed(&state), "the first press arms");
        let outcome = mouse(&mut state, kind, column, row);
        assert!(restarts(&outcome).is_empty(), "{kind:?}");
        assert!(!armed(&state), "{kind:?} cancels");
        assert!(state.nirvana_seats.is_some(), "{kind:?} leaves the panel open");
        let _ = rows_of(&mut state, 200, 40);
    }
    done(root, home);
}

/// P87 fix round (item b6): at every window width the panel is drawn at, nothing panics, a button is only ever on the
/// whole drawn words, and where the words do not fit whole there is no button: a click anywhere on that row
/// restarts nothing.
#[test]
fn p87_a_narrow_window_draws_no_button_where_the_words_do_not_fit() {
    let (mut state, root, home) = state_for("p87-narrow", "nirvana");
    running_on(&home, &root, "opus");
    let mut cut = None;
    for width in 20..=200u16 {
        if state.nirvana_seats.is_none() {
            state.toggle_seats_panel();
        }
        let rows = rows_of(&mut state, width, 40);
        let hits = state.hits.nirvana_seats_panel.clone();
        if hits.restart.width > 0 {
            let drawn: String = rows[usize::from(hits.restart.y)]
                .chars()
                .skip(usize::from(hits.restart.x))
                .take(usize::from(hits.restart.width))
                .collect();
            assert_eq!(drawn, "restart to switch", "at {width}");
        } else if hits.area.width > 0 {
            // The orchestrator row, inside the panel.
            let row = (hits.area.y..hits.area.bottom()).find(|y| {
                rows[usize::from(*y)]
                    .chars()
                    .skip(usize::from(hits.area.x))
                    .take(usize::from(hits.area.width))
                    .collect::<String>()
                    .contains("Nirvana Claude")
            });
            if let Some(y) = row {
                cut.get_or_insert((width, y));
            }
        }
    }
    let (width, y) = cut.expect("a width where the row is drawn but the words do not fit");
    let _ = rows_of(&mut state, width, 40);
    let area = state.hits.nirvana_seats_panel.area;
    for column in area.x..area.right() {
        let outcome = click(&mut state, column, y);
        assert!(restarts(&outcome).is_empty(), "at {width} wide, column {column}");
        assert!(state.nirvana_seats.is_some(), "a click inside the panel keeps it open");
        let _ = rows_of(&mut state, width, 40);
    }
    done(root, home);
}

// ---- Plan 46 P46b: the Table page ----

use crate::nirvana_seat::{Effort, Mode, ModeSource};
use crate::nirvana_seat_calib::CellRow;
use crate::nirvana_seats_square::{
    DetailRow, Matrix, MatrixCell, MatrixRow, OrchestratorRow, Row as SeatRow, RunLine,
    View as SeatsView, MODES, ORCHESTRATOR_ROW,
};

/// 2026-11-19 12:00:00Z, the time the panels below are drawn at.
const AT: u64 = 1_795_089_600;

fn pick(mode: Mode, seat: &str, effort: Effort, model: &str) -> MatrixCell {
    MatrixCell {
        mode,
        seat: Some(seat.into()),
        model: Some(model.into()),
        effort: Some(effort),
        next: None,
        forced: false,
        cool: None,
        unread: false,
        none: None,
        measured: None,
        held: false,
    }
}

fn record(class: &str, seat: &str, effort: Effort, orders: usize) -> CellRow {
    CellRow {
        class: class.into(),
        seat: seat.into(),
        effort: Some(effort),
        orders,
        gated: orders,
        first_pass: orders.saturating_sub(2),
        merged: 0,
        merge_orders: 0,
        median_minutes: Some(14.0),
        median_cost: Some(1.28),
        costed: orders,
        recent: 3,
    }
}

/// The matrix the Table page tests draw: Nirvana Claude, then a plain class, a forced one, an unread one, one with
/// no seat in SOTA, a held one, a measured one, and `extra` filler classes for the scroll.
fn sample_matrix(extra: usize) -> Matrix {
    use Effort::*;
    use Mode::*;
    let mut orchestrator = MatrixRow {
        id: ORCHESTRATOR_ROW.into(),
        orchestrator: true,
        cells: vec![
            pick(Efficient, "sonnet", High, "claude-sonnet-5-5"),
            pick(Balanced, "opus", High, "claude-opus-5-5"),
            pick(Sota, "fable", High, "claude-fable-5-2"),
        ],
        detail: Vec::new(),
    };
    orchestrator.cells[1].forced = true;
    orchestrator.cells[1].cool = Some("fable high".into());
    let mut plan = MatrixRow {
        id: "plan".into(),
        orchestrator: false,
        cells: vec![
            pick(Efficient, "sonnet", Medium, "claude-sonnet-5-5"),
            pick(Balanced, "opus", High, "claude-opus-5-5"),
            pick(Sota, "fable", Max, "claude-fable-5-2"),
        ],
        detail: vec![
            DetailRow {
                seat: "opus".into(),
                score: 95.0,
                enabled: true,
                record: vec![
                    record("plan", "opus", High, 21),
                    record("plan", "opus", Medium, 3),
                ],
            },
            DetailRow {
                seat: "neon".into(),
                score: 80.0,
                enabled: false,
                record: Vec::new(),
            },
        ],
    };
    plan.cells[1].next = Some(("sonnet".into(), High));
    let mut forced = MatrixRow {
        id: "build.load_bearing".into(),
        orchestrator: false,
        cells: vec![
            pick(Efficient, "hydrogen", Medium, "gemini-4-hydrogen-medium"),
            pick(Balanced, "sonnet", Medium, "claude-sonnet-5-5"),
            pick(Sota, "opus", Xhigh, "claude-opus-5-5"),
        ],
        detail: vec![DetailRow {
            seat: "opus".into(),
            score: 95.0,
            enabled: true,
            record: vec![record("build.load_bearing", "opus", Xhigh, 8)],
        }],
    };
    forced.cells[1].forced = true;
    forced.cells[1].cool = Some("opus high".into());
    let mut unread = MatrixRow {
        id: "research.web".into(),
        orchestrator: false,
        cells: vec![
            pick(Efficient, "flash", Low, "gemini-3.8-flash-low"),
            pick(Balanced, "carbon", Medium, "gemini-4-carbon-medium"),
            pick(Sota, "argon", High, "gemini-4-argon-high"),
        ],
        detail: vec![DetailRow {
            seat: "carbon".into(),
            score: 70.0,
            enabled: true,
            record: vec![record("research.web", "carbon", Medium, 2)],
        }],
    };
    unread.cells[1].unread = true;
    let mut none = MatrixRow {
        id: "media.video".into(),
        orchestrator: false,
        cells: vec![
            pick(Efficient, "flash", Medium, "gemini-3.8-flash-medium"),
            pick(Balanced, "carbon", Medium, "gemini-4-carbon-medium"),
            pick(Sota, "carbon", High, "gemini-4-carbon-high"),
        ],
        detail: Vec::new(),
    };
    none.cells[2] = MatrixCell {
        mode: Sota,
        seat: None,
        model: None,
        effort: None,
        next: None,
        forced: false,
        cool: None,
        unread: false,
        none: Some("every seat is off or over its window".into()),
        measured: None,
        held: false,
    };
    let mut held = MatrixRow {
        id: "review.adversary".into(),
        orchestrator: false,
        cells: vec![
            pick(Efficient, "sonnet", High, "claude-sonnet-5-5"),
            pick(Balanced, "opus", High, "claude-opus-5-5"),
            pick(Sota, "fable", High, "claude-fable-5-2"),
        ],
        detail: vec![DetailRow {
            seat: "opus".into(),
            score: 92.0,
            enabled: true,
            record: vec![record("review.adversary", "opus", High, 64)],
        }],
    };
    held.cells[1].held = true;
    held.cells[1].measured = Some("measured x2.25 over 64 orders".into());
    let mut measured = MatrixRow {
        id: "sweep.mechanical".into(),
        orchestrator: false,
        cells: vec![
            pick(Efficient, "hydrogen", Low, "gemini-4-hydrogen-low"),
            pick(Balanced, "flash", Low, "gemini-3.8-flash-low"),
            pick(Sota, "sonnet", Low, "claude-sonnet-5-5"),
        ],
        detail: vec![DetailRow {
            seat: "hydrogen".into(),
            score: 75.0,
            enabled: true,
            record: vec![record("sweep.mechanical", "hydrogen", Low, 7)],
        }],
    };
    measured.cells[0].measured = Some("measured x0.37 over 7 orders".into());
    let mut rows = vec![orchestrator, plan, forced, unread, none, held, measured];
    for n in 0..extra {
        rows.push(MatrixRow {
            id: format!("filler.{n:02}"),
            orchestrator: false,
            cells: vec![
                pick(Efficient, "haiku", Low, "claude-haiku-5-5"),
                pick(Balanced, "sonnet", Low, "claude-sonnet-5-5"),
                pick(Sota, "opus", Low, "claude-opus-5-5"),
            ],
            detail: Vec::new(),
        });
    }
    Matrix {
        rows,
        checked_at: Some(AT - 7_200),
        summary: Some("1 cost booked as measured, 2 score findings".into()),
    }
}

/// A View with two orders in one run, Nirvana Claude, the differs flag and, when asked, the matrix.
fn sample_view(matrix: Option<Matrix>) -> SeatsView {
    let order = |order: &str, seat: &str, effort: Effort, model: &str, state: &str| SeatRow {
        run: "r1".into(),
        order: order.into(),
        class: "build.bounded".into(),
        seat: seat.into(),
        model: model.into(),
        effort,
        pool: "max".into(),
        state: state.into(),
        attention: false,
        since: AT - 90,
        deferred_until: None,
        use_: None,
        reason: format!("build.bounded \u{2192} {seat} {}", effort.word()),
        override_reason: None,
        mode: Some(Mode::Balanced),
        pane: None,
    };
    SeatsView {
        mode: Some(Mode::Balanced),
        mode_source: Some(ModeSource::Machine),
        note: None,
        runs: vec![RunLine {
            run: "r1".into(),
            orders: 2,
            classes: [("build.bounded".to_owned(), 2)].into_iter().collect(),
            overrides: 0,
            mode_changed: None,
        }],
        rows: vec![
            order("o1", "sonnet", Effort::Medium, "claude-sonnet-5-5", "running"),
            order("o2", "opus", Effort::High, "claude-opus-5-5", "booked"),
        ],
        orchestrator: Some(OrchestratorRow {
            seat: "opus".into(),
            effort: Effort::High,
            detail: "the mode's seat".into(),
            restart_to_switch: false,
        }),
        differs: true,
        differs_line: None,
        matrix,
    }
}

fn blank_panel() -> crate::client::shell::nirvana_seats::SeatsPanel {
    crate::client::shell::nirvana_seats::SeatsPanel {
        root: std::path::PathBuf::from("C:/work/moto"),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        pane_id: "pane_1".into(),
        restart_armed: None,
        restarting: None,
        selected: 0,
        scroll: 0,
        message: None,
        page: crate::client::shell::nirvana_seats::Page::Orders,
        table_selected: 0,
        table_scroll: 0,
        table_selected_id: None,
    }
}

/// The panel drawn alone into a `width` by `height` buffer, as text rows with their trailing blanks cut.
fn draw(
    panel: &mut crate::client::shell::nirvana_seats::SeatsPanel,
    view: Option<&SeatsView>,
    width: u16,
    height: u16,
) -> (Vec<String>, crate::client::shell::nirvana_seats::SeatsHits) {
    let palette = ClientShellConfig::from_config(&Config::default()).palette;
    let area = Rect::new(0, 0, width, height);
    let mut buffer = Buffer::empty(area);
    let hits = crate::client::shell::nirvana_seats::render_seats_panel(
        &mut buffer,
        area,
        panel,
        view,
        AT,
        &palette,
    );
    let rows = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol().to_owned())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect();
    (rows, hits)
}

/// Drawn at 2419bec8, before the Table page: `sample_view(None)` at 80 by 30.
const ORDERS_NONE_80: &[&str] = &[
    "",
    " ╭─ Seats · moto · Balanced ────────────────────────────────────────────── ✕ ─╮",
    " │ Mode   Efficient  [Balanced]  SOTA  · machine                 m changes it │",
    " │ seats.toml differs from herdr's default. herdr seat diff shows how.        │",
    " │ Nirvana Claude · opus high · the mode's seat                               │",
    " │                                                                            │",
    " │ run r1 · 2 orders · build.bounded 2                                        │",
    " │ o1 · running 1m 30s                                                        │",
    " │   sonnet medium · claude-sonnet-5-5 · max · use not known                  │",
    " │   ↳ build.bounded → sonnet medium                                          │",
    " │ o2 · booked 1m 30s                                                         │",
    " │   opus high · claude-opus-5-5 · max · use not known                        │",
    " │   ↳ build.bounded → opus high                                              │",
    " │                                                                            │",
    " │ ↑/↓ choose · enter focus its pane · m mode · esc close                     │",
    " ╰────────────────────────────────────────────────────────────────────────────╯",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

/// Drawn at 2419bec8, before the Table page: `sample_view(None)` at 140 by 30.
const ORDERS_NONE_140: &[&str] = &[
    "",
    " ╭─ Seats · moto · Balanced ────────────────────────────────────────────────────────────────────────────── ✕ ─╮",
    " │ Mode   Efficient  [Balanced]  SOTA  · machine                                                 m changes it │",
    " │ seats.toml differs from herdr's default. herdr seat diff shows how.                                        │",
    " │ Nirvana Claude · opus high · the mode's seat                                                               │",
    " │                                                                                                            │",
    " │ run r1 · 2 orders · build.bounded 2                                                                        │",
    " │ order        seat effort      model                    pool     state       time         use               │",
    " │ o1           sonnet medium    claude-sonnet-5-5        max      running     1m 30s       use not known     │",
    " │   ↳ build.bounded → sonnet medium                                                                          │",
    " │ o2           opus high        claude-opus-5-5          max      booked      1m 30s       use not known     │",
    " │   ↳ build.bounded → opus high                                                                              │",
    " │                                                                                                            │",
    " │ ↑/↓ choose · enter focus its pane · m mode · esc close                                                     │",
    " ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────╯",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

/// Drawn at 2419bec8, before the Table page: `sample_view(Some(sample_matrix(0)))` at 80 by 30.
const ORDERS_MATRIX_80: &[&str] = &[
    "",
    " ╭─ Seats · moto · Balanced ────────────────────────────────────────────── ✕ ─╮",
    " │ Mode   Efficient  [Balanced]  SOTA  · machine                 m changes it │",
    " │ seats.toml differs from herdr's default. herdr seat diff shows how.        │",
    " │ Nirvana Claude · opus high · the mode's seat                               │",
    " │                                                                            │",
    " │ run r1 · 2 orders · build.bounded 2                                        │",
    " │ o1 · running 1m 30s                                                        │",
    " │   sonnet medium · claude-sonnet-5-5 · max · use not known                  │",
    " │   ↳ build.bounded → sonnet medium                                          │",
    " │ o2 · booked 1m 30s                                                         │",
    " │   opus high · claude-opus-5-5 · max · use not known                        │",
    " │   ↳ build.bounded → opus high                                              │",
    " │                                                                            │",
    " │ ↑/↓ choose · enter focus its pane · m mode · esc close                     │",
    " ╰────────────────────────────────────────────────────────────────────────────╯",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

/// Drawn at 2419bec8, before the Table page: `sample_view(Some(sample_matrix(0)))` at 140 by 30.
const ORDERS_MATRIX_140: &[&str] = &[
    "",
    " ╭─ Seats · moto · Balanced ────────────────────────────────────────────────────────────────────────────── ✕ ─╮",
    " │ Mode   Efficient  [Balanced]  SOTA  · machine                                                 m changes it │",
    " │ seats.toml differs from herdr's default. herdr seat diff shows how.                                        │",
    " │ Nirvana Claude · opus high · the mode's seat                                                               │",
    " │                                                                                                            │",
    " │ run r1 · 2 orders · build.bounded 2                                                                        │",
    " │ order        seat effort      model                    pool     state       time         use               │",
    " │ o1           sonnet medium    claude-sonnet-5-5        max      running     1m 30s       use not known     │",
    " │   ↳ build.bounded → sonnet medium                                                                          │",
    " │ o2           opus high        claude-opus-5-5          max      booked      1m 30s       use not known     │",
    " │   ↳ build.bounded → opus high                                                                              │",
    " │                                                                                                            │",
    " │ ↑/↓ choose · enter focus its pane · m mode · esc close                                                     │",
    " ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────╯",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

use crate::client::shell::nirvana_seats::{
    cell_marks, cell_words, detail_lines, header_line, record_line, table_fit, table_line,
    table_widths, Page, SeatsPanel, TABLE_LEGEND, TABLE_REFUSED,
};

fn table_panel(selected: usize) -> SeatsPanel {
    let mut panel = blank_panel();
    panel.page = Page::Table;
    panel.table_selected = selected;
    panel
}

/// The detail block's text for one row of the sample matrix, live at Balanced, in `room` cells.
fn detail_of(index: usize, room: usize) -> Vec<String> {
    let matrix = sample_matrix(0);
    detail_lines(&matrix.rows[index], Some(Mode::Balanced), room)
        .into_iter()
        .map(|(text, _)| text)
        .collect()
}

/// The panel's rows from its top border to its bottom border.
fn framed(rows: &[String]) -> (usize, usize) {
    let top = rows.iter().position(|row| row.contains('\u{256D}')).expect("top border");
    let bottom = rows.iter().position(|row| row.contains('\u{2570}')).expect("bottom border");
    (top, bottom)
}

/// The table row of `id`: after the left border and the gutter, which holds `▸` on the selected row.
fn table_row<'a>(rows: &'a [String], id: &str) -> &'a str {
    let plain = format!("\u{2502} {id} ");
    let picked = format!("\u{2502}\u{25b8}{id} ");
    rows.iter()
        .find(|row| row.contains(&plain) || row.contains(&picked))
        .unwrap_or_else(|| panic!("no table row {id:?}:\n{}", rows.join("\n")))
}

/// Opens the panel on the recorded project at 200 by 40, then goes to the Table page with `t`.
fn open_table(test: &str) -> (ClientShellState, std::path::PathBuf, std::path::PathBuf) {
    let (mut state, root, home) = state_for(test, "nirvana");
    let _ = open(&mut state, 200);
    let _ = key(&mut state, KeyCode::Char('t'));
    let _ = rows_of(&mut state, 200, 40);
    (state, root, home)
}

fn page_of(state: &ClientShellState) -> Page {
    state.nirvana_seats.as_ref().expect("the panel is open").page
}

fn table_selected(state: &ClientShellState) -> usize {
    state.nirvana_seats.as_ref().expect("the panel is open").table_selected
}

#[test]
fn p98_the_page_strip_names_both_pages_and_brackets_the_one_shown() {
    let view = sample_view(Some(sample_matrix(0)));
    let (rows, hits) = draw(&mut blank_panel(), Some(&view), 100, 30);
    let (top, _) = framed(&rows);
    assert!(rows[top].contains("[Orders]  Table "), "{:?}", rows[top]);
    assert_eq!(hits.pages.len(), 2, "{:?}", hits.pages);
    let (rows, hits) = draw(&mut table_panel(0), Some(&view), 100, 30);
    let (top, _) = framed(&rows);
    assert!(rows[top].contains(" Orders  [Table]"), "{:?}", rows[top]);
    assert!(rows[top].contains("Seats \u{b7} moto \u{b7} Balanced"), "{:?}", rows[top]);
    let words: Vec<Page> = hits.pages.iter().map(|(_, page)| *page).collect();
    assert_eq!(words, vec![Page::Orders, Page::Table]);
}

#[test]
fn p98_t_tab_and_a_click_on_a_page_word_switch_pages() {
    let (mut state, root, home) = state_for("p98-switch", "nirvana");
    let _ = open(&mut state, 200);
    assert_eq!(page_of(&state), Page::Orders, "the panel opens on Orders");
    let outcome = key(&mut state, KeyCode::Char('t'));
    assert!(outcome.requests.is_empty(), "the key stays out of the pane");
    assert_eq!(page_of(&state), Page::Table);
    // The page survives the panel's re-read: each compose reads the view again under test.
    let rows = rows_of(&mut state, 200, 40);
    assert_eq!(page_of(&state), Page::Table);
    assert!(line(&rows, "task").contains("[Balanced]"), "{}", rows.join("\n"));
    let _ = key(&mut state, KeyCode::Tab);
    assert_eq!(page_of(&state), Page::Orders);
    let _ = key(&mut state, KeyCode::BackTab);
    assert_eq!(page_of(&state), Page::Table);
    let _ = rows_of(&mut state, 200, 40);
    let word = |state: &ClientShellState, page: Page| {
        state
            .hits
            .nirvana_seats_panel
            .pages
            .iter()
            .find(|(_, at)| *at == page)
            .map(|(rect, _)| *rect)
            .expect("a page word")
    };
    let orders = word(&state, Page::Orders);
    let _ = click(&mut state, orders.x + 1, orders.y);
    assert_eq!(page_of(&state), Page::Orders, "a click on Orders");
    assert!(state.nirvana_seats.is_some(), "a click on a page word keeps the panel open");
    let _ = rows_of(&mut state, 200, 40);
    let table = word(&state, Page::Table);
    let _ = click(&mut state, table.x + 1, table.y);
    assert_eq!(page_of(&state), Page::Table, "a click on Table");
    done(root, home);
}

#[test]
fn p98_three_mode_columns_with_the_projects_mode_marked() {
    let header = header_line(Some(Mode::Balanced), 18, 30);
    let at = |word: &str| header.find(word).unwrap_or_else(|| panic!("{word:?} in {header:?}"));
    assert!(header.starts_with("task"), "{header:?}");
    assert!(at("Efficient") < at("[Balanced]") && at("[Balanced]") < at("SOTA"), "{header:?}");
    assert!(!header.contains("[SOTA]") && !header.contains("[Efficient]"), "{header:?}");
    let sota = header_line(Some(Mode::Sota), 18, 30);
    assert!(sota.contains("[SOTA]") && !sota.contains("[Balanced]"), "{sota:?}");
    // Drawn, at the View's mode.
    let mut view = sample_view(Some(sample_matrix(0)));
    view.mode = Some(Mode::Efficient);
    let (rows, _) = draw(&mut table_panel(0), Some(&view), 140, 40);
    let drawn = line(&rows, "task");
    assert!(drawn.contains("[Efficient]") && !drawn.contains("[Balanced]"), "{drawn:?}");
}

#[test]
fn p98_each_cell_is_the_matrix_cell_given() {
    let matrix = sample_matrix(0);
    for width in [80u16, 100, 140] {
        let inner = usize::from(width - 2 - 4);
        let (task, cell) = table_widths(&matrix, inner);
        let (rows, _) = draw(&mut table_panel(0), Some(&sample_view(Some(matrix.clone()))), width, 40);
        for row in &matrix.rows {
            let drawn = table_row(&rows, &row.id);
            let body: Vec<char> = drawn.chars().skip(3).collect();
            for (place, mode) in MODES.iter().enumerate() {
                let found = row.cells.iter().find(|c| c.mode == *mode).unwrap();
                let start = task + 1 + place * (cell + 1);
                let text: String = body[start..(start + cell).min(body.len())].iter().collect();
                let want = format!("{}{}", cell_marks(found), cell_words(found, cell - 3));
                assert_eq!(text.trim_end(), want.trim_end(), "{} {mode:?} at {width}", row.id);
                if let (Some(seat), Some(effort)) = (&found.seat, found.effort) {
                    assert!(text.contains(seat.as_str()), "{text:?}");
                    let short = &effort.word()[..3.min(effort.word().len())];
                    assert!(text.contains(short), "{text:?} has its effort");
                }
            }
        }
    }
}

#[test]
fn p98_the_forced_unread_none_held_and_measured_marks() {
    let matrix = sample_matrix(0);
    let cell = |row: usize, place: usize| &matrix.rows[row].cells[place];
    assert_eq!(cell_marks(cell(1, 0)), "   ", "a plain cell");
    assert_eq!(cell_marks(cell(2, 1)), "!  ", "forced");
    assert_eq!(cell_marks(cell(3, 1)), " ? ", "unread");
    assert_eq!(cell_marks(cell(5, 1)), "  =", "held");
    assert_eq!(cell_marks(cell(6, 0)), "  ~", "measured, not held");
    let mut both = cell(2, 1).clone();
    both.unread = true;
    assert_eq!(cell_marks(&both), "!? ", "forced while unread shows both marks");
    both.held = true;
    assert_eq!(cell_marks(&both), "!?=", "and its calibration mark beside them");
    assert_eq!(cell_words(cell(4, 2), 20), "no seat");
    let (rows, _) = draw(&mut table_panel(0), Some(&sample_view(Some(matrix.clone()))), 140, 40);
    assert!(line(&rows, "build.load_bearing").contains("!  sonnet medium"), "{}", rows.join("\n"));
    assert!(line(&rows, "research.web").contains(" ? carbon medium"), "{}", rows.join("\n"));
    assert!(line(&rows, "media.video").contains("no seat"), "{}", rows.join("\n"));
    assert!(line(&rows, "review.adversary").contains("  =opus high"), "{}", rows.join("\n"));
    assert!(line(&rows, "sweep.mechanical").contains("  ~hydrogen low"), "{}", rows.join("\n"));
    assert!(rows.iter().any(|row| row.contains(TABLE_LEGEND)), "the legend: {}", rows.join("\n"));
    // The words stay in the detail, not the cell.
    assert!(!line(&rows, "review.adversary").contains("held"), "{}", rows.join("\n"));
    assert!(!line(&rows, "sweep.mechanical").contains("measured"), "{}", rows.join("\n"));
    // Forced and hot, forced while unread, and unread alone read differently in the detail.
    let hot = detail_of(2, 140).join("\n");
    assert!(hot.contains("hot usage moved it: cool pick opus high"), "{hot}");
    let unread = detail_of(3, 140).join("\n");
    assert!(unread.contains("usage unread: a window it spends was not read"), "{unread}");
    assert!(!unread.contains("hot usage"), "{unread}");
    let mut moved = sample_matrix(0);
    moved.rows[3].cells[1].forced = true;
    moved.rows[3].cells[1].cool = Some("flash low".into());
    let text: Vec<String> = detail_lines(&moved.rows[3], Some(Mode::Balanced), 140)
        .into_iter()
        .map(|(text, _)| text)
        .collect();
    let text = text.join("\n");
    assert!(text.contains("usage unread; the cool pick is flash low"), "{text}");
    assert!(!text.contains("not hot") && !text.contains("hot usage"), "unread never says hot or not: {text}");
}

#[test]
fn p98_selection_moves_with_the_keys_and_stops_at_both_ends() {
    let (mut state, root, home) = open_table("p98-select");
    let count = crate::nirvana_seats_square::cached(&root)
        .and_then(|view| view.matrix)
        .map(|matrix| matrix.rows.len())
        .expect("the recorded project's matrix");
    assert!(count > 12, "Nirvana Claude and every class of the default table: {count}");
    let last = count - 1;
    let _ = key(&mut state, KeyCode::Up);
    assert_eq!(table_selected(&state), 0, "up at the top stays");
    let _ = key(&mut state, KeyCode::Down);
    assert_eq!(table_selected(&state), 1);
    let _ = key(&mut state, KeyCode::Char('j'));
    assert_eq!(table_selected(&state), 2);
    let _ = key(&mut state, KeyCode::Char('k'));
    assert_eq!(table_selected(&state), 1);
    let _ = key(&mut state, KeyCode::PageDown);
    assert_eq!(table_selected(&state), 11);
    let _ = key(&mut state, KeyCode::End);
    assert_eq!(table_selected(&state), last);
    let _ = key(&mut state, KeyCode::Down);
    assert_eq!(table_selected(&state), last, "down at the bottom stays");
    let _ = key(&mut state, KeyCode::PageDown);
    assert_eq!(table_selected(&state), last);
    let _ = key(&mut state, KeyCode::PageUp);
    assert_eq!(table_selected(&state), last - 10);
    let _ = key(&mut state, KeyCode::Home);
    assert_eq!(table_selected(&state), 0);
    let _ = key(&mut state, KeyCode::PageUp);
    assert_eq!(table_selected(&state), 0);
    // The Orders page keeps its own selection.
    assert_eq!(state.nirvana_seats.as_ref().unwrap().selected, 0);
    // Enter focuses no pane on the Table page, and the panel stays.
    let outcome = key(&mut state, KeyCode::Enter);
    assert!(focused(&outcome).is_empty(), "{:?}", outcome.actions);
    assert!(state.nirvana_seats.is_some());
    done(root, home);
}

#[test]
fn p98_a_click_selects_a_table_row_and_the_wheel_moves_it() {
    let (mut state, root, home) = open_table("p98-click");
    let rows = state.hits.nirvana_seats_panel.table_rows.clone();
    assert!(rows.len() > 3, "{rows:?}");
    let (rect, index) = rows[3];
    let outcome = click(&mut state, rect.x + 2, rect.y);
    assert_eq!(table_selected(&state), index);
    assert!(focused(&outcome).is_empty(), "a table row has no pane");
    assert!(state.nirvana_seats.is_some());
    let wheel = |state: &mut ClientShellState, kind: MouseEventKind| {
        state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
            kind,
            column: rect.x + 2,
            row: rect.y,
            modifiers: KeyModifiers::empty(),
        })])
    };
    let _ = wheel(&mut state, MouseEventKind::ScrollDown);
    assert_eq!(table_selected(&state), index + 3);
    let _ = wheel(&mut state, MouseEventKind::ScrollUp);
    let _ = wheel(&mut state, MouseEventKind::ScrollUp);
    assert_eq!(table_selected(&state), 0);
    done(root, home);
}

#[test]
fn p98_a_long_matrix_scrolls_and_keeps_its_header() {
    let matrix = sample_matrix(40);
    let last = matrix.rows.len() - 1;
    let view = sample_view(Some(matrix));
    let mut panel = table_panel(last);
    let (rows, hits) = draw(&mut panel, Some(&view), 80, 30);
    let text = rows.join("\n");
    let header = rows.iter().position(|row| row.contains("[Balanced]")).expect("the header");
    let (top, _) = framed(&rows);
    assert_eq!(header, top + 1, "the header is the first body line: {text}");
    assert!(text.contains("filler.39"), "the selected last row is shown: {text}");
    assert!(!text.contains("Nirvana Claude"), "the first rows scrolled off: {text}");
    assert!(panel.table_scroll > 0);
    assert_eq!(hits.table_rows.last().map(|(_, index)| *index), Some(last));
    // Back to the top.
    panel.table_selected = 0;
    panel.table_selected_id = None;
    let (rows, _) = draw(&mut panel, Some(&view), 80, 30);
    assert_eq!(panel.table_scroll, 0);
    assert!(rows.join("\n").contains("Nirvana Claude"));
}

#[test]
fn p98_the_detail_of_a_plain_row() {
    let text = detail_of(1, 140);
    assert_eq!(text[0], "plan");
    let all = text.join("\n");
    assert!(all.contains("Efficient    sonnet medium \u{b7} claude-sonnet-5-5"), "{all}");
    assert!(all.contains("\u{25b8} Balanced     opus high \u{b7} claude-opus-5-5 \u{b7} next sonnet high"), "{all}");
    assert!(all.contains("seats this task lists"), "{all}");
    assert!(all.contains("opus       score  95"), "{all}");
    assert!(
        all.contains("high   measured \u{b7} 21 orders \u{b7} 19 of 21 first gate \u{b7} cost x1.28 \u{b7} 14 min \u{b7} 3 in 14 d"),
        "{all}"
    );
    assert!(all.contains("medium thin \u{b7} 3 orders"), "under five orders reads thin: {all}");
    assert!(all.contains("neon       score  80 \u{b7} off \u{b7} no record"), "{all}");
}

#[test]
fn p98_the_detail_of_a_forced_row_names_the_cool_pick() {
    let all = detail_of(2, 140).join("\n");
    assert!(
        all.contains("Balanced  !  sonnet medium \u{b7} claude-sonnet-5-5 \u{b7} hot usage moved it: cool pick opus high"),
        "{all}"
    );
    // Narrow, the facts go on a line of their own under the seat.
    let narrow = detail_of(2, 60);
    let at = narrow.iter().position(|line| line.contains("!  sonnet medium")).unwrap();
    assert!(narrow[at + 1].trim_start().starts_with("hot usage moved it"), "{narrow:?}");
}

#[test]
fn p98_the_detail_of_a_held_row_never_reads_as_the_booking() {
    let all = detail_of(5, 140).join("\n");
    assert!(all.contains("held: booked at the table's cost; measured x2.25 over 64 orders"), "{all}");
    // The booking is named before the measured figure, so the figure never reads as what books.
    let held = all.find("booked at the table's cost").expect("the booking");
    assert!(held < all.find("measured x2.25").expect("the figure"), "{all}");
    assert!(!all.contains("cost booked as measured x2.25"), "{all}");
    let measured = detail_of(6, 140).join("\n");
    assert!(measured.contains("cost booked as measured x0.37 over 7 orders"), "{measured}");
    assert!(!measured.contains("held"), "{measured}");
}

#[test]
fn p98_the_detail_of_a_row_with_no_seat_and_no_record() {
    let all = detail_of(4, 140).join("\n");
    assert!(all.contains("SOTA         no seat \u{b7} no seat fits: every seat is off or over its window"), "{all}");
    assert!(all.contains("none listed \u{b7} no record"), "{all}");
    let thin = detail_of(3, 140).join("\n");
    assert!(thin.contains("medium thin \u{b7} 2 orders"), "{thin}");
}

#[test]
fn p98_the_detail_of_the_orchestrator_row() {
    let text = detail_of(0, 140);
    assert!(text[0].starts_with("Nirvana Claude"), "{text:?}");
    let all = text.join("\n");
    assert!(all.contains("hot usage moved it: cool pick fable high"), "{all}");
    assert!(!all.contains("seats this task lists"), "{all}");
    assert!(!all.contains("next "), "{all}");
    assert_eq!(text.len(), 4, "its heading and one line a mode: {text:?}");
}

#[test]
fn p98_the_record_line_shortens_to_fit() {
    let row = record("plan", "opus", Effort::High, 21);
    let long = record_line(&row, 140);
    assert!(long.contains("21 orders \u{b7} 19 of 21 first gate"), "{long:?}");
    let short = record_line(&row, 70);
    assert!(short.contains("21 ord \u{b7} 19/21 gate \u{b7} x1.28 \u{b7} 14m \u{b7} 3/14d"), "{short:?}");
    assert!(unicode_width::UnicodeWidthStr::width(short.as_str()) <= 70, "{short:?}");
}

#[test]
fn p98_the_check_line_is_checked_line() {
    let matrix = sample_matrix(0);
    let want = crate::nirvana_seats_square::checked_line(&matrix, AT);
    assert_eq!(want, "checked 2h 0m ago: 1 cost booked as measured, 2 score findings");
    let (rows, _) = draw(&mut table_panel(0), Some(&sample_view(Some(matrix))), 140, 40);
    assert!(rows.iter().any(|row| row.contains(&want)), "{}", rows.join("\n"));
    let mut unchecked = sample_matrix(0);
    unchecked.checked_at = None;
    let (rows, _) = draw(&mut table_panel(0), Some(&sample_view(Some(unchecked))), 140, 40);
    assert!(
        rows.iter().any(|row| row.contains("not checked against the record yet")),
        "{}",
        rows.join("\n")
    );
}

#[test]
fn p98_no_matrix_says_so_in_one_line_and_never_panics() {
    let mut view = sample_view(None);
    view.note = Some("seats.toml line 3: expected a table".into());
    let (rows, hits) = draw(&mut table_panel(4), Some(&view), 100, 30);
    let text = rows.join("\n");
    assert_eq!(rows.iter().filter(|row| row.contains(TABLE_REFUSED)).count(), 1, "{text}");
    assert!(text.contains("seats.toml line 3: expected a table"), "{text}");
    assert!(text.contains(crate::nirvana_seat::DIFFERS_LINE), "{text}");
    assert!(!text.contains("task "), "no header: {text}");
    assert!(hits.table_rows.is_empty());
    // Not read yet.
    let (rows, _) = draw(&mut table_panel(0), None, 100, 30);
    assert!(rows.join("\n").contains("reading this project's runs"));
    // An empty matrix and a short row.
    let mut empty = sample_view(Some(Matrix::default()));
    empty.differs = false;
    let mut panel = table_panel(9);
    let (rows, _) = draw(&mut panel, Some(&empty), 100, 30);
    assert!(rows.join("\n").contains("not checked against the record yet"));
    assert_eq!(panel.table_selected, 0, "the selection is held to the rows there are");
    let mut short = sample_matrix(0);
    short.rows[1].cells.truncate(1);
    short.rows[2].cells.clear();
    let (rows, _) = draw(&mut table_panel(2), Some(&sample_view(Some(short))), 100, 30);
    assert!(line(&rows, "\u{2502} plan").contains("sonnet medium"), "{}", rows.join("\n"));
    // Keys and the wheel on an empty table stay at 0.
    let (mut state, root, home) = state_for("p98-empty", "nirvana");
    std::fs::write(home.join("seats.toml"), "not a table [").expect("seats.toml");
    let _ = open(&mut state, 200);
    let _ = key(&mut state, KeyCode::Char('t'));
    for code in [KeyCode::Down, KeyCode::End, KeyCode::PageDown] {
        let _ = key(&mut state, code);
        assert_eq!(table_selected(&state), 0, "{code:?}");
    }
    let rows = rows_of(&mut state, 200, 40);
    assert!(rows.iter().any(|row| row.contains(TABLE_REFUSED)), "{}", rows.join("\n"));
    done(root, home);
}

#[test]
fn p98_80_100_and_140_columns() {
    let view = sample_view(Some(sample_matrix(16)));
    for width in [80u16, 100, 140] {
        let (rows, _) = draw(&mut table_panel(2), Some(&view), width, 40);
        let text = rows.join("\n");
        for row in &rows {
            assert!(
                unicode_width::UnicodeWidthStr::width(row.as_str()) <= usize::from(width),
                "{width}: {row:?}"
            );
        }
        let (top, bottom) = framed(&rows);
        let footer = bottom - 1;
        assert!(rows[footer].contains("t/tab orders"), "{width}: {text}");
        // The table, its notes, a blank, then the detail; nothing overlaps.
        let last_row = rows.iter().rposition(|row| row.contains("filler.")).unwrap();
        let detail = rows
            .iter()
            .rposition(|row| row.starts_with(" \u{2502} build.load_bearing"))
            .unwrap();
        assert!(top + 1 < last_row && last_row < detail && detail < footer, "{width}: {text}");
        let row = table_row(&rows, "build.load_bearing");
        match width {
            80 => {
                // The model drops first, then the effort shortens where the three-mark slot leaves 14 cells.
                assert!(!row.contains("claude-"), "{width}: {row:?}");
                assert!(row.contains("hydrogen med ") && row.contains("sonnet medium"), "{width}: {row:?}");
            }
            100 => {
                assert!(!row.contains("claude-"), "{width}: {row:?}");
                assert!(row.contains("hydrogen medium"), "{width}: {row:?}");
            }
            _ => {
                assert!(row.contains("sonnet medium \u{b7} claude-sonnet-5-5"), "{row:?}");
                assert!(row.contains("opus xhigh \u{b7} claude-opus-5-5"), "{row:?}");
            }
        }
        // The detail always names the model of the row's cells.
        assert!(text.contains("hydrogen medium \u{b7} gemini-4-hydrogen-medium"), "{width}: {text}");
    }
    // Then the effort shortens.
    let matrix = sample_matrix(0);
    let cell = &matrix.rows[2].cells[0];
    assert_eq!(cell_words(cell, 15), "hydrogen medium");
    assert_eq!(cell_words(cell, 14), "hydrogen med");
    assert_eq!(cell_words(cell, 41), "hydrogen medium", "the model shows only whole");
    assert_eq!(cell_words(cell, 42), "hydrogen medium \u{b7} gemini-4-hydrogen-medium");
}

#[test]
fn p98_a_short_panel_shrinks_the_detail_before_the_table() {
    assert_eq!(
        table_fit(40, 23, 3),
        crate::client::shell::nirvana_seats::TableFit { rows: 23, notes: 3, detail: 13 }
    );
    let short = table_fit(20, 23, 3);
    assert_eq!(short.rows, 6, "the first rows hold: {short:?}");
    assert_eq!(short.notes, 3);
    assert_eq!(short.detail, 10, "the detail takes the rest: {short:?}");
    let tiny = table_fit(8, 23, 3);
    assert_eq!((tiny.rows, tiny.detail), (6, 0), "{tiny:?}");
    assert_eq!(table_fit(1, 23, 3).rows, 0, "the header is the last line kept");
    // Drawn at 80 by 20: the header, rows and the notes, then a cut detail, and the footer below it.
    let view = sample_view(Some(sample_matrix(16)));
    let (rows, _) = draw(&mut table_panel(1), Some(&view), 80, 20);
    let text = rows.join("\n");
    let (top, bottom) = framed(&rows);
    assert!(rows[top + 1].contains("[Balanced]"), "{text}");
    assert!(rows[bottom - 1].contains("t/tab orders"), "{text}");
    let cut = rows[bottom - 3].trim_end_matches([' ', '\u{2502}']);
    assert!(cut.ends_with(" \u{2026}"), "a cut detail says so: {text}");
}

#[test]
fn p98_the_orders_page_is_drawn_as_before() {
    for (matrix, width, golden) in [
        (None, 80u16, ORDERS_NONE_80),
        (None, 140, ORDERS_NONE_140),
        (Some(sample_matrix(0)), 80, ORDERS_MATRIX_80),
        (Some(sample_matrix(0)), 140, ORDERS_MATRIX_140),
    ] {
        let (rows, _) = draw(&mut blank_panel(), Some(&sample_view(matrix)), width, 30);
        assert_eq!(rows.len(), golden.len());
        let (top, bottom) = framed(&rows);
        for (at, (now, then)) in rows.iter().zip(golden.iter()).enumerate() {
            if at == top {
                // The page words sit in the border before the close button; the title is as it was.
                assert!(now.starts_with(" \u{256D}\u{2500} Seats \u{b7} moto \u{b7} Balanced \u{2500}"), "{now:?}");
                assert!(now.ends_with("[Orders]  Table   \u{2715} \u{2500}\u{256E}"), "{now:?}");
            } else if at == bottom - 1 {
                assert_eq!(
                    now.replacen("t/tab table \u{b7} ", "", 1).trim_end_matches([' ', '\u{2502}']),
                    then.trim_end_matches([' ', '\u{2502}']),
                    "the footer gains the page key"
                );
            } else {
                assert_eq!(now, then, "{width} line {at}");
            }
        }
    }
}

#[test]
fn p98_the_differs_line_is_the_views_on_both_pages() {
    let kept = "seats.toml keeps your overrides_per_run = 5; the rest is herdr's default.";
    let mut view = sample_view(Some(sample_matrix(0)));
    view.differs_line = Some(kept.into());
    for page in [Page::Orders, Page::Table] {
        let mut panel = blank_panel();
        panel.page = page;
        let (rows, _) = draw(&mut panel, Some(&view), 140, 40);
        let text = rows.join("\n");
        assert!(text.contains(kept), "{page:?}: {text}");
        assert!(!text.contains(crate::nirvana_seat::DIFFERS_LINE), "{page:?}: {text}");
    }
    view.differs = false;
    let (rows, _) = draw(&mut table_panel(0), Some(&view), 140, 40);
    assert!(!rows.join("\n").contains(kept), "no line when the table does not differ");
}

#[test]
fn p98_the_footer_names_the_page_keys_on_both_pages() {
    let view = sample_view(Some(sample_matrix(0)));
    let (rows, _) = draw(&mut blank_panel(), Some(&view), 140, 40);
    let (_, bottom) = framed(&rows);
    assert!(rows[bottom - 1].contains("t/tab table"), "{:?}", rows[bottom - 1]);
    let (rows, _) = draw(&mut table_panel(0), Some(&view), 140, 40);
    let (_, bottom) = framed(&rows);
    assert!(rows[bottom - 1].contains("t/tab orders"), "{:?}", rows[bottom - 1]);
    assert!(rows[bottom - 1].contains("pgup/pgdn"), "{:?}", rows[bottom - 1]);
}

/// Not a check: writes the Table page at 80 and 140 columns to `P98_TABLE`, for the eye.
#[test]
fn p98_dump_the_table_page() {
    let Ok(out) = std::env::var("P98_TABLE") else {
        return;
    };
    let mut text = String::new();
    for width in [80u16, 140] {
        let (rows, _) = draw(&mut table_panel(2), Some(&sample_view(Some(sample_matrix(16)))), width, 40);
        text.push_str(&format!("=== {width} by 40, build.load_bearing selected\n"));
        for row in rows {
            text.push_str(&row);
            text.push('\n');
        }
    }
    std::fs::write(out, text).expect("the dump");
}

#[test]
fn p98_each_table_line_is_the_task_then_three_cells() {
    let matrix = sample_matrix(0);
    let (task, cell) = table_widths(&matrix, 74);
    assert_eq!((task, cell), (18, 17));
    let row = table_line(&matrix.rows[1], task, cell);
    assert!(row.starts_with("plan               "), "{row:?}");
    assert_eq!(unicode_width::UnicodeWidthStr::width(row.as_str()), task + 3 * (cell + 1));
}

// ---- Plan 46 P46b fix round (review of bff901fe) ----

/// The panel drawn alone, as `draw` does, with its buffer for the colours.
fn draw_cells(panel: &mut SeatsPanel, view: Option<&SeatsView>, width: u16, height: u16) -> Buffer {
    let palette = ClientShellConfig::from_config(&Config::default()).palette;
    let area = Rect::new(0, 0, width, height);
    let mut buffer = Buffer::empty(area);
    let _ = crate::client::shell::nirvana_seats::render_seats_panel(&mut buffer, area, panel, view, AT, &palette);
    buffer
}

/// The column, in cells, where `needle` starts in `row`.
fn cell_of_text(row: &str, needle: &str) -> u16 {
    let at = row.find(needle).unwrap_or_else(|| panic!("{needle:?} in {row:?}"));
    unicode_width::UnicodeWidthStr::width(&row[..at]) as u16
}

/// Review finding 1. The titles below are the ones the code at 2419bec8 drew (read from `git show
/// 2419bec8:src/client/shell/nirvana_seats.rs`: `draw_frame` fits the title to the frame's width less 8, and a panel
/// under 30 wide is not drawn), not taken from the code under test. Where the page words are drawn the title is
/// whole; where it would not be whole the page words are not drawn.
#[test]
fn p98_the_title_is_the_old_one_wherever_the_page_words_do_not_fit() {
    let sota = " Seats \u{b7} moto \u{b7} SOTA ";
    let balanced = " Seats \u{b7} moto \u{b7} Balanced ";
    // (width, mode, the title at 2419bec8, page words drawn)
    let cases: [(u16, Mode, Option<&str>, bool); 12] = [
        (30, Mode::Sota, None, false),
        (32, Mode::Sota, Some(sota), false),
        (40, Mode::Sota, Some(sota), false),
        (50, Mode::Sota, Some(sota), true),
        (60, Mode::Sota, Some(sota), true),
        (80, Mode::Sota, Some(sota), true),
        (30, Mode::Balanced, None, false),
        (32, Mode::Balanced, Some(" Seats \u{b7} moto \u{b7} Balan\u{2026}"), false),
        (40, Mode::Balanced, Some(balanced), false),
        (50, Mode::Balanced, Some(balanced), false),
        (60, Mode::Balanced, Some(balanced), true),
        (80, Mode::Balanced, Some(balanced), true),
    ];
    let palette = ClientShellConfig::from_config(&Config::default()).palette;
    for page in [Page::Orders, Page::Table] {
        for (width, mode, title, words) in cases {
            let mut view = sample_view(Some(sample_matrix(0)));
            view.mode = Some(mode);
            let mut panel = blank_panel();
            panel.page = page;
            let (rows, hits) = draw(&mut panel, Some(&view), width, 30);
            let Some(title) = title else {
                assert!(rows.iter().all(|row| row.is_empty()), "{page:?} {width}: no panel at 2419bec8 either");
                continue;
            };
            let (top, bottom) = framed(&rows);
            let row = &rows[top];
            assert!(
                row.starts_with(&format!(" \u{256D}\u{2500}{title}\u{2500}")),
                "{page:?} {width} {mode:?}: {row:?}"
            );
            let strip = if page == Page::Orders { "[Orders]  Table " } else { " Orders  [Table]" };
            assert_eq!(row.contains(strip), words, "{page:?} {width} {mode:?}: {row:?}");
            assert_eq!(hits.pages.len(), if words { 2 } else { 0 }, "{page:?} {width} {mode:?}");
            // With no page words the footer still names the key.
            assert!(rows[bottom - 1].contains("t/tab"), "{page:?} {width}: {:?}", rows[bottom - 1]);
            // SOTA is red in the title wherever it is drawn.
            if mode == Mode::Sota {
                let buffer = draw_cells(&mut panel, Some(&view), width, 30);
                let col = cell_of_text(row, "SOTA");
                for col in col..col + 4 {
                    assert_eq!(buffer[(col, top as u16)].fg, palette.red, "{page:?} {width} at {col}");
                }
            }
        }
    }
    // A long folder: beside the page words only the folder is cut, and the mode word stays whole.
    let mut view = sample_view(None);
    view.mode = Some(Mode::Sota);
    let mut panel = blank_panel();
    panel.root = std::path::PathBuf::from("C:/work/a-very-long-project-folder-name");
    let (rows, hits) = draw(&mut panel, Some(&view), 70, 30);
    let (top, _) = framed(&rows);
    assert!(
        rows[top].starts_with(" \u{256D}\u{2500} Seats \u{b7} a-very-long-project-fold\u{2026} \u{b7} SOTA \u{2500}"),
        "{:?}",
        rows[top]
    );
    assert_eq!(hits.pages.len(), 2, "{:?}", rows[top]);
    // Too narrow for eight cells of the folder: the old title, cut by the frame as before, and no page words.
    let (rows, hits) = draw(&mut panel, Some(&view), 50, 30);
    let (top, _) = framed(&rows);
    assert!(
        rows[top].starts_with(" \u{256D}\u{2500} Seats \u{b7} a-very-long-project-folder-nam\u{2026}\u{2500}"),
        "{:?}",
        rows[top]
    );
    assert!(hits.pages.is_empty());
}

/// Review finding 2: `unread` and `forced` are two facts; both marks show, the columns stay in line, and the detail
/// claims neither "hot" nor "not hot" for a pick that stood in the unread tier.
#[test]
fn p98_forced_and_unread_show_both_marks_in_line() {
    let mut matrix = sample_matrix(0);
    matrix.rows[3].cells[1].forced = true;
    matrix.rows[3].cells[1].cool = Some("flash low".into());
    let (rows, _) = draw(&mut table_panel(0), Some(&sample_view(Some(matrix.clone()))), 140, 40);
    let both = table_row(&rows, "research.web");
    let plain = table_row(&rows, "plan");
    assert!(both.contains("!? carbon medium"), "{both:?}");
    // The marks take the same three cells in every row, so the seat words start in one column.
    assert_eq!(cell_of_text(both, "carbon medium"), cell_of_text(plain, "opus high"), "{both:?}\n{plain:?}");
    assert!(
        rows.iter().any(|row| row.contains("! not the cool pick \u{b7} ? usage unread")),
        "{}",
        rows.join("\n")
    );
    let text: Vec<String> = detail_lines(&matrix.rows[3], Some(Mode::Balanced), 140)
        .into_iter()
        .map(|(text, _)| text)
        .collect();
    assert!(text.join("\n").contains("Balanced  !? carbon medium"), "{text:?}");
}

/// Review finding 3: the selection is the row's class; a re-read that reorders keeps it, one that drops it holds the
/// index to the rows there are.
#[test]
fn p98_the_selection_follows_its_class_through_a_reread() {
    let matrix = sample_matrix(4);
    let mut panel = table_panel(3);
    let _ = draw(&mut panel, Some(&sample_view(Some(matrix.clone()))), 140, 40);
    assert_eq!(panel.table_selected_id.as_deref(), Some("research.web"));
    let mut reordered = matrix.clone();
    reordered.rows.reverse();
    let (rows, _) = draw(&mut panel, Some(&sample_view(Some(reordered.clone()))), 140, 40);
    let at = reordered.rows.iter().position(|row| row.id == "research.web").unwrap();
    assert_ne!(at, 3);
    assert_eq!(panel.table_selected, at, "the same class after the reorder");
    assert!(
        rows.iter().any(|row| row.contains("\u{2502}\u{25b8}research.web ")),
        "{}",
        rows.join("\n")
    );
    // The class gone: the nearest index, held to the shorter table.
    let mut shorter = reordered.clone();
    shorter.rows.retain(|row| row.id != "research.web");
    shorter.rows.truncate(5);
    let _ = draw(&mut panel, Some(&sample_view(Some(shorter.clone()))), 140, 40);
    assert_eq!(panel.table_selected, at.min(4), "{at}");
    assert_eq!(panel.table_selected_id.as_deref(), Some(shorter.rows[at.min(4)].id.as_str()));
}

/// Review finding 4: the detail's height is the panel's, so moving the selection between a short detail and a long
/// one leaves the same rows on screen and the frame the same height, unless the selection leaves the rows shown.
#[test]
fn p98_the_rows_shown_hold_still_as_the_selection_moves() {
    let mut matrix = sample_matrix(16);
    // A long detail on build.load_bearing: six seats with two records each.
    let seat = matrix.rows[2].detail[0].clone();
    for n in 0..5 {
        let mut more = seat.clone();
        more.seat = format!("seat{n}");
        more.record.push(seat.record[0].clone());
        matrix.rows[2].detail.push(more);
    }
    let view = sample_view(Some(matrix.clone()));
    for (width, height) in [(80u16, 24u16), (120, 40)] {
        let mut panel = table_panel(0);
        let (rows, hits) = draw(&mut panel, Some(&view), width, height);
        let shown: Vec<usize> = hits.table_rows.iter().map(|(_, index)| *index).collect();
        let frame = framed(&rows);
        let lengths: Vec<usize> = (0..shown.len())
            .map(|index| detail_lines(&matrix.rows[index], Some(Mode::Balanced), 200).len())
            .collect();
        assert!(lengths.iter().min() < lengths.iter().max(), "short and long details: {lengths:?}");
        assert!(lengths.iter().any(|&lines| lines > 12), "one detail longer than its slot: {lengths:?}");
        for selected in 0..shown.len() {
            panel.table_selected = selected;
            panel.table_selected_id = None;
            let (rows, hits) = draw(&mut panel, Some(&view), width, height);
            let now: Vec<usize> = hits.table_rows.iter().map(|(_, index)| *index).collect();
            assert_eq!(now, shown, "{width}x{height} at row {selected}:\n{}", rows.join("\n"));
            assert_eq!(framed(&rows), frame, "{width}x{height} at row {selected}");
        }
        // The long detail is cut and says so; the frame did not grow for it.
        panel.table_selected = 2;
        panel.table_selected_id = None;
        let (rows, _) = draw(&mut panel, Some(&view), width, height);
        let (_, bottom) = framed(&rows);
        assert!(
            rows[bottom - 3].trim_end_matches([' ', '\u{2502}']).ends_with(" \u{2026}"),
            "{width}x{height}:\n{}",
            rows.join("\n")
        );
    }
}

/// Review finding 6: the selected row carries `▸` in the gutter; the columns keep their places at 80.
#[test]
fn p98_the_selected_row_reads_without_colour() {
    let matrix = sample_matrix(0);
    let (task, cell) = table_widths(&matrix, 74);
    let (rows, _) = draw(&mut table_panel(2), Some(&sample_view(Some(matrix.clone()))), 80, 40);
    let marked: Vec<&String> = rows.iter().filter(|row| row.contains("\u{2502}\u{25b8}")).collect();
    assert_eq!(marked.len(), 1, "{}", rows.join("\n"));
    assert!(marked[0].contains("\u{2502}\u{25b8}build.load_bearing"), "{marked:?}");
    for (index, row) in matrix.rows.iter().enumerate() {
        let drawn: String = table_row(&rows, &row.id).chars().skip(3).collect();
        let want = table_line(row, task, cell);
        assert_eq!(drawn.trim_end_matches([' ', '\u{2502}']), want.trim_end(), "row {index} keeps its columns");
    }
}

/// Section 5: a wide seat name pads by display width, so its score lines up with the others.
#[test]
fn p98_a_wide_seat_name_keeps_the_score_column() {
    let mut matrix = sample_matrix(0);
    let mut wide = matrix.rows[1].detail[0].clone();
    wide.seat = "\u{6f22}\u{5b57}".into();
    matrix.rows[1].detail.push(wide);
    let text: Vec<String> = detail_lines(&matrix.rows[1], Some(Mode::Balanced), 140)
        .into_iter()
        .map(|(text, _)| text)
        .collect();
    let score_at = |lead: &str| {
        let line = text.iter().find(|line| line.starts_with(lead)).expect(lead);
        cell_of_text(line, "score")
    };
    assert_eq!(score_at("  \u{6f22}\u{5b57}"), score_at("  opus"), "{text:?}");
}

/// Section 5: `m` on the Table page moves the mode as on Orders, and the page stays.
#[test]
fn p98_m_on_the_table_page_moves_the_mode() {
    let (mut state, root, home) = open_table("p98-m");
    let outcome = key(&mut state, KeyCode::Char('m'));
    assert!(outcome.requests.is_empty(), "the key stays out of the pane");
    let project_file = root.join(".nirvana/seats.toml");
    let table = std::fs::read_to_string(&project_file).expect("the mode is written");
    assert_eq!(
        crate::nirvana_seat::parse_project_seats(&project_file, &table)
            .unwrap()
            .unwrap(),
        Mode::Sota
    );
    assert_eq!(page_of(&state), Page::Table);
    let rows = rows_of(&mut state, 200, 40);
    assert!(line(&rows, " task ").contains("[SOTA]"), "{}", rows.join("\n"));
    done(root, home);
}

/// Section 5: `r` on the Table page restarts the panel's Nirvana Claude as on Orders.
#[test]
fn p98_r_on_the_table_page_restarts_nirvana_claude() {
    let (mut state, root, home) = state_for("p98-r", "nirvana");
    running_on(&home, &root, "opus");
    let _ = open(&mut state, 200);
    let _ = key(&mut state, KeyCode::Char('t'));
    let rows = rows_of(&mut state, 200, 40);
    assert_eq!(page_of(&state), Page::Table);
    let _ = line(&rows, "r restart \u{b7} m mode \u{b7} esc close");
    let outcome = key(&mut state, KeyCode::Char('r'));
    assert_eq!(restarts(&outcome), vec![the_orchestrator()]);
    assert_eq!(page_of(&state), Page::Table);
    done(root, home);
}

/// Section 5: with the mode unknown no column is bracketed; the project's bracket is red at SOTA and green
/// otherwise, as the Mode row's is.
#[test]
fn p98_the_table_header_brackets_only_a_known_mode_and_sota_in_red() {
    let header = header_line(None, 18, 30);
    assert!(!header.contains('['), "{header:?}");
    let mut view = sample_view(Some(sample_matrix(0)));
    view.mode = None;
    let (rows, _) = draw(&mut table_panel(0), Some(&view), 140, 40);
    let drawn = line(&rows, " task ");
    assert!(!drawn.contains('[') && drawn.contains("Efficient") && drawn.contains("SOTA"), "{drawn:?}");
    let palette = ClientShellConfig::from_config(&Config::default()).palette;
    for (mode, color) in [(Mode::Sota, palette.red), (Mode::Balanced, palette.green)] {
        view.mode = Some(mode);
        let mut panel = table_panel(0);
        let (rows, _) = draw(&mut panel, Some(&view), 140, 40);
        let buffer = draw_cells(&mut panel, Some(&view), 140, 40);
        let y = rows.iter().position(|row| row.contains(" task ")).unwrap();
        let word = format!("[{}]", mode.title());
        let x = cell_of_text(&rows[y], &word);
        for col in x..x + word.len() as u16 {
            assert_eq!(buffer[(col, y as u16)].fg, color, "{mode:?} at {col}");
        }
    }
}
