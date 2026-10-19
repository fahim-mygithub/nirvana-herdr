//! Nirvana fork: running servers and usage at the foot of the sidebar
//! (nirvana/plans/2026-10-15-04-servers-and-usage.md L1).

use super::*;
use crate::client::shell::nirvana_footer::{
    address, figures, footer_height, split, AGENTS_KEEP, AHEAD,
};
use crate::nirvana_usage::{set_rows_for_test, Figure, Provider, Shown, UsageRow};

fn shown(window: &str, used: f32, pace: f32, resets_in: u64) -> Shown {
    Shown {
        window: window.into(),
        used: Some(used),
        pace: Some(pace),
        resets_in: Some(resets_in),
        stale: false,
        ahead: pace > crate::nirvana_usage::AHEAD_OF_PACE,
    }
}

fn window(window: &str, used: f32, pace: f32, resets_in: u64) -> Figure {
    Figure::Window(shown(window, used, pace, resets_in))
}

fn row(pool: &str, provider: Provider, figure: Figure) -> UsageRow {
    UsageRow {
        pool: pool.into(),
        provider,
        figure,
    }
}

/// One row per pool (foot2: both 5 h and week windows shown, 5 h first).
/// Claude: 5 h at 26% (behind) and week at 65% (ahead of pace: 1.01 > 1.0, tightest).
/// Gemini: 5 h at 41% and week at 2%, both behind pace.
fn usage() -> Vec<UsageRow> {
    vec![
        row(
            "max",
            Provider::Claude,
            Figure::Windows(vec![
                shown("5h", 0.26, 0.8, 10_698),
                shown("week", 0.65, 1.01, 583_910),
            ]),
        ),
        row(
            "gemini",
            Provider::Antigravity,
            Figure::Windows(vec![
                shown("5h", 0.41, 0.9, 14_400),
                shown("week", 0.02, 0.79, 583_910),
            ]),
        ),
        row(
            "argon",
            Provider::Antigravity,
            window("week", 0.021, 0.9, 583_910),
        ),
        row(
            "google3p",
            Provider::Antigravity,
            window("week", 0.04, 0.6, 133_752),
        ),
        row("codex", Provider::Codex, Figure::Word("not exposed".into())),
        row(
            "local",
            Provider::Local,
            Figure::Memory {
                used_mib: 3539,
                total_mib: 32607,
            },
        ),
    ]
}

/// The composed frame and its rows as text, the first space running servers on `ports`.
fn compose_with(
    ports: &str,
    width: u16,
    height: u16,
    sidebar_width: Option<u16>,
) -> (ClientShellState, FrameData, Vec<String>) {
    set_rows_for_test(Some(usage()));
    let mut projected = snapshot();
    if !ports.is_empty() {
        projected.workspaces[0]
            .tokens
            .push(("nirvana_servers".into(), ports.into()));
    }
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    if let Some(sidebar_width) = sidebar_width {
        state.config.sidebar_max_width = sidebar_width.max(state.config.sidebar_max_width);
        state.sidebar_width = sidebar_width;
    }
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(width, height).expect("composed frame").frame;
    let rows = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    set_rows_for_test(None);
    (state, frame, rows)
}

fn row_with<'a>(rows: &'a [String], text: &str) -> Option<(usize, &'a String)> {
    rows.iter().enumerate().find(|(_, row)| row.contains(text))
}

/// The sidebar's cells of a row: its 25 content columns, without the divider in the 26th.
fn sidebar(row: &str) -> String {
    row.chars().take(25).collect()
}

/// Plan 12 O1 changed the rows: one per pool, each its tightest window ("41% 5h"), where the head carried
/// "5h 7D" over two columns per provider before.
#[test]
fn the_foot_lists_a_spaces_server_and_a_usage_row_per_pool() {
    let (_, _, rows) = compose_with("8770", 106, 40, None);
    let (servers_y, _) = row_with(&rows, " servers").expect("servers head");
    let server = sidebar(&rows[servers_y + 1]);
    assert!(
        server.starts_with(" \u{25CF} client-shell") && server.trim_end().ends_with(":8770"),
        "server row: {server:?}"
    );
    let (usage_y, head) = row_with(&rows, " usage").expect("usage head");
    assert_eq!(sidebar(head).trim_end(), " usage", "head: {head:?}");
    assert!(usage_y > servers_y);
    let expected = [
        ("Claude", format!("26% 5h  65% 7d{AHEAD}")),
        ("Gemini", "41% 5h   2% 7d".to_owned()),
        ("Codex", "not exposed".to_owned()),
        ("Local", "3/32 GiB".to_owned()),
    ];
    for (offset, (name, figure)) in expected.iter().enumerate() {
        let row = sidebar(&rows[usage_y + 1 + offset]);
        assert!(
            row.starts_with(&format!(" \u{25CF} {name}")) && row.trim_end().ends_with(figure),
            "usage row {offset}: {row:?}"
        );
    }
    // Narrow row Claude fits exactly at default sidebar width 26 (content width 25).
    assert_eq!(
        sidebar(&rows[usage_y + 1]),
        format!(" \u{25CF} Claude 26% 5h  65% 7d{AHEAD}")
    );
    // Argon and Google's Claude pool are read but not shown (the owner, 2026-10-18: "keep that internally").
    assert!(row_with(&rows, "Argon").is_none() && row_with(&rows, "G\u{b7}Claude").is_none());
    // The foot sits at the very bottom of the sidebar, its last row the accounts button.
    assert_eq!(usage_y + 5, rows.len() - 1);
    assert!(
        sidebar(&rows[usage_y + 5]).trim_end() == " accounts",
        "{:?}",
        rows[usage_y + 5]
    );
}

#[test]
fn a_window_ahead_of_pace_carries_its_mark_in_the_warning_colour() {
    let (state, frame, rows) = compose_with("", 106, 40, None);
    let (usage_y, _) = row_with(&rows, " usage").unwrap();
    let cells = |y: usize| &frame.cells[y * frame.width as usize..(y + 1) * frame.width as usize];
    let claude = cells(usage_y + 1);
    let mark = claude
        .iter()
        .take(25)
        .position(|cell| cell.symbol == AHEAD)
        .expect("Claude is ahead of pace");
    assert_eq!(
        claude[mark].fg,
        crate::protocol::color_to_u32(state.config.palette.peach)
    );
    // Gemini is behind pace: no mark.
    assert!(!sidebar(&rows[usage_y + 2]).contains(AHEAD));
}

#[test]
fn a_stale_reading_is_muted() {
    let mut usage = usage();
    match &mut usage[1].figure {
        Figure::Window(shown) => shown.stale = true,
        Figure::Windows(windows) => {
            for shown in windows {
                shown.stale = true;
            }
        }
        _ => {}
    }
    set_rows_for_test(Some(usage));
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 40).expect("composed frame").frame;
    set_rows_for_test(None);
    let rows: Vec<String> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    let (usage_y, _) = row_with(&rows, " usage").unwrap();
    let percent = |y: usize| {
        let row = &frame.cells[y * frame.width as usize..];
        let x = (0..25).find(|&x| row[x].symbol == "%").expect("a percent");
        row[x].fg
    };
    let palette = &state.config.palette;
    assert_eq!(
        percent(usage_y + 2),
        crate::protocol::color_to_u32(palette.overlay0)
    );
    assert_eq!(
        percent(usage_y + 1),
        crate::protocol::color_to_u32(palette.text)
    );
}

#[test]
fn no_server_hides_the_servers_block() {
    let (_, _, rows) = compose_with("", 106, 40, None);
    assert!(row_with(&rows, " servers").is_none());
    assert!(row_with(&rows, " usage").is_some());
}

#[test]
fn each_provider_dot_has_its_colour() {
    let (state, frame, rows) = compose_with("", 106, 40, None);
    let (usage_y, _) = row_with(&rows, " usage").unwrap();
    let dot = |y: usize| frame.cells[y * frame.width as usize + 1].fg;
    let rgb = |r, g, b| crate::protocol::color_to_u32(ratatui::style::Color::Rgb(r, g, b));
    // Each pool's dot is its reader's colour; Gemini is agy's.
    assert_eq!(dot(usage_y + 1), rgb(0xD8, 0x88, 0x6A));
    assert_eq!(dot(usage_y + 2), rgb(0xAB, 0x93, 0xD6));
    assert_eq!(dot(usage_y + 3), rgb(0x7B, 0xAE, 0xDD));
    assert_eq!(
        dot(usage_y + 4),
        crate::protocol::color_to_u32(state.config.palette.overlay1)
    );
}

#[test]
fn a_click_on_a_server_row_opens_its_address() {
    let (mut state, _, rows) = compose_with("5173", 106, 40, None);
    let (servers_y, _) = row_with(&rows, " servers").unwrap();
    let click = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 4,
        row: servers_y as u16 + 1,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(
        click.actions.iter().any(|action| matches!(
            action,
            ClientShellAction::OpenSafeWebUrl(url) if url == "http://localhost:5173/"
        )),
        "actions: {:?}",
        click.actions
    );
}

#[test]
fn four_servers_then_how_many_more() {
    let (_, _, rows) = compose_with("3000,3001,3002,3003,3004,3005", 106, 40, None);
    let (servers_y, _) = row_with(&rows, " servers").unwrap();
    assert!(sidebar(&rows[servers_y + 4]).contains(":3003"));
    assert!(
        sidebar(&rows[servers_y + 5]).contains("+2 more"),
        "{:?}",
        rows[servers_y + 5]
    );
}

#[test]
fn a_short_sidebar_keeps_the_agents_panel() {
    // 22 rows, not 20: the foot is 11 rows with plan 12 O1's six pools and a server, so at 20 the sidebar's lower
    // section was shorter than the foot itself and the agents' floor was never what hid it (control
    // footer_agents_starved stayed green). At 22 the foot would fit, but not with five agent rows above it.
    let (_, _, rows) = compose_with("8770", 106, 22, None);
    assert!(row_with(&rows, " usage").is_none());
    assert!(row_with(&rows, " agents").is_some());
}

#[test]
fn a_wide_sidebar_draws_a_bar_and_the_reset_for_the_tightest_window() {
    let (_, _, rows) = compose_with("8770", 120, 40, Some(40));
    let (usage_y, _) = row_with(&rows, " usage").unwrap();
    let claude: String = rows[usage_y + 1].chars().take(40).collect();
    assert!(
        claude.contains('\u{2501}') || claude.contains('\u{2500}'),
        "{claude:?}"
    );
    // The reset follows the figure only here: week is tightest for Claude (65% > 26%), 583,910 s is 6 d 18 h.
    let claude = claude.split('\u{2502}').next().unwrap();
    assert!(
        claude
            .trim_end()
            .ends_with(&format!("26% 5h  65% 7d{AHEAD} 6d18h")),
        "{claude:?}"
    );
    let gemini = rows[usage_y + 2].split('\u{2502}').next().unwrap();
    assert!(
        gemini.trim_end().ends_with("41% 5h   2% 7d     4h"),
        "{gemini:?}"
    );
    // Codex has no figure: a dashed track.
    let codex: String = rows[usage_y + 3].chars().take(40).collect();
    assert!(codex.contains('\u{254C}'), "{codex:?}");
    // At herdr's default width there is no reset.
    let (_, _, narrow) = compose_with("8770", 106, 40, None);
    let (usage_y, _) = row_with(&narrow, " usage").unwrap();
    assert!(!sidebar(&narrow[usage_y + 1]).contains("6d18h"));
    // The server row has room for the whole address.
    let (servers_y, _) = row_with(&rows, " servers").unwrap();
    assert!(rows[servers_y + 1].contains("localhost:8770"));
}

#[test]
fn the_layout_rules() {
    assert_eq!(footer_height(0, 6), 9);
    assert_eq!(footer_height(1, 6), 11);
    assert_eq!(footer_height(9, 6), 15);
    // An agy pool none of the six names adds its own row.
    assert_eq!(footer_height(0, 7), 10);
    let detail = Rect::new(0, 10, 25, 14);
    let (agents, foot) = split(detail, 8);
    assert_eq!((agents.height, foot.y, foot.height), (6, 16, 8));
    let (agents, foot) = split(detail, 14 - AGENTS_KEEP + 1);
    assert_eq!((agents, foot), (detail, Rect::default()));
    assert_eq!(address(8770, 11, 22), ":8770");
    assert_eq!(address(8770, 4, 22), "localhost:8770");
    let unknown = Figure::Window(Shown {
        window: "week".into(),
        used: None,
        pace: None,
        resets_in: None,
        stale: false,
        ahead: false,
    });
    assert_eq!(figures(&unknown, false), "  -- 7d ");
    assert_eq!(figures(&unknown, true), "  -- 7d       ");
    assert_eq!(
        figures(&window("5h", 1.0, 2.0, 45 * 60), true),
        format!("100% 5h{AHEAD}   45m")
    );
    // A scoped week reads as a week.
    assert_eq!(
        figures(&window("week.fable", 0.5, 0.9, 0), false),
        " 50% 7d "
    );
    assert_eq!(figures(&Figure::Reading, false), "reading");
    let two_windows = Figure::Windows(vec![
        shown("5h", 0.26, 0.8, 10_698),
        shown("week", 0.65, 1.01, 583_910),
    ]);
    assert_eq!(
        figures(&two_windows, false),
        format!(" 26% 5h  65% 7d{AHEAD}")
    );
    assert_eq!(
        figures(&two_windows, true),
        format!(" 26% 5h  65% 7d{AHEAD} 6d18h")
    );
}

// ---- Feature 11: the accounts button and its panel (nirvana/plans/2026-10-17-11-accounts.md) ----

fn accounts() -> Vec<crate::nirvana_accounts::Account> {
    use crate::nirvana_accounts::{Account, Cli, Signed};
    vec![
        Account {
            cli: Cli::Claude,
            signed: Signed::In("me@example.com \u{b7} max".into()),
        },
        Account {
            cli: Cli::Codex,
            signed: Signed::Out,
        },
        Account {
            cli: Cli::Antigravity,
            signed: Signed::In("old@example.com".into()),
        },
    ]
}

fn mouse_down(state: &mut ClientShellState, column: u16, row: u16) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })])
}

fn texts(state: &mut ClientShellState) -> Vec<String> {
    let frame = state.compose(106, 40).expect("composed frame").frame;
    frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect()
}

fn login_ids(outcome: &ClientShellInput) -> Vec<String> {
    outcome
        .actions
        .iter()
        .filter_map(|action| match action {
            ClientShellAction::Endpoint { request, .. } => match &request.method {
                crate::api::schema::Method::CommandInvoke(params) => {
                    Some(params.command_id.clone())
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn the_accounts_button_opens_a_panel_with_each_cli_and_who_is_signed_in() {
    crate::nirvana_accounts::set_accounts_for_test(Some(accounts()));
    let (mut state, _, rows) = compose_with("", 106, 40, None);
    let button_y = rows.len() - 1;
    assert!(state.nirvana_accounts.is_none());
    mouse_down(&mut state, 4, button_y as u16);
    assert!(
        state.nirvana_accounts.is_some(),
        "the button opens the panel"
    );
    set_rows_for_test(Some(usage()));
    let rows = texts(&mut state);
    let (claude_y, claude) = row_with(&rows, "Claude Code").expect("Claude row");
    assert!(
        claude.contains("me@example.com \u{b7} max") && claude.contains("[ switch ]"),
        "{claude:?}"
    );
    let codex = &rows[claude_y + 1];
    assert!(
        codex.contains("Codex") && codex.contains("not signed in") && codex.contains("[ log in ]"),
        "{codex:?}"
    );
    let agy = &rows[claude_y + 2];
    assert!(
        agy.contains("Antigravity")
            && agy.contains("old@example.com")
            && agy.contains("[ switch ]"),
        "{agy:?}"
    );
    assert!(row_with(&rows, " Accounts ").is_some(), "the panel's title");
    // It sits at the bottom left of the panes, beside the button: its frame's foot is the window's last row.
    assert!(rows[button_y].contains('\u{2570}'), "{:?}", rows[button_y]);

    // A second press of the button closes it.
    mouse_down(&mut state, 4, button_y as u16);
    assert!(
        state.nirvana_accounts.is_none(),
        "the button closes the panel"
    );
    set_rows_for_test(None);
    crate::nirvana_accounts::set_accounts_for_test(None);
}

#[test]
fn a_rows_button_sends_that_clis_sign_in_and_closes_the_panel() {
    crate::nirvana_accounts::set_accounts_for_test(Some(accounts()));
    let (mut state, _, rows) = compose_with("", 106, 40, None);
    mouse_down(&mut state, 4, (rows.len() - 1) as u16);
    set_rows_for_test(Some(usage()));
    let rows = texts(&mut state);
    let (agy_y, agy) = row_with(&rows, "old@example.com").expect("Antigravity row");
    let column = agy.chars().collect::<Vec<_>>();
    let button_x = (0..column.len())
        .find(|&x| {
            column[x..]
                .iter()
                .collect::<String>()
                .starts_with("[ switch ]")
        })
        .expect("button");
    let outcome = mouse_down(&mut state, button_x as u16 + 2, agy_y as u16);
    assert_eq!(
        login_ids(&outcome),
        vec![format!(
            "{}antigravity",
            crate::nirvana::ACCOUNT_LOGIN_COMMAND
        )]
    );
    assert!(state.nirvana_accounts.is_none(), "the tab takes over");

    // The keys: down to Codex, Enter signs in to it.
    mouse_down(&mut state, 4, (rows.len() - 1) as u16);
    let press = |state: &mut ClientShellState, code| {
        state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
            code,
            KeyModifiers::NONE,
        ))])
    };
    press(&mut state, KeyCode::Down);
    let outcome = press(&mut state, KeyCode::Enter);
    assert_eq!(
        login_ids(&outcome),
        vec![format!("{}codex", crate::nirvana::ACCOUNT_LOGIN_COMMAND)]
    );
    set_rows_for_test(None);
    crate::nirvana_accounts::set_accounts_for_test(None);
}

#[test]
fn a_pool_ahead_on_one_window_only_marks_that_window_alone() {
    let (state, frame, rows) = compose_with("", 106, 40, None);
    let (usage_y, _) = row_with(&rows, " usage").unwrap();
    let cells = |y: usize| &frame.cells[y * frame.width as usize..(y + 1) * frame.width as usize];
    let claude = cells(usage_y + 1);
    // Claude: 5h behind pace (cell 16 is space), week ahead of pace (cell 24 is AHEAD in peach).
    assert_eq!(claude[16].symbol, " ");
    assert_eq!(claude[24].symbol, AHEAD);
    assert_eq!(
        claude[24].fg,
        crate::protocol::color_to_u32(state.config.palette.peach)
    );
    // Gemini is behind pace on both windows: no mark.
    assert!(!sidebar(&rows[usage_y + 2]).contains(AHEAD));
}

#[test]
fn a_stale_week_beside_a_fresh_5h_is_muted_while_5h_is_fresh() {
    let mut usage = usage();
    if let Figure::Windows(windows) = &mut usage[0].figure {
        windows[0].stale = false;
        windows[1].stale = true;
    }
    set_rows_for_test(Some(usage));
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 40).expect("composed frame").frame;
    set_rows_for_test(None);
    let rows: Vec<String> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    let (usage_y, _) = row_with(&rows, " usage").unwrap();
    let claude_row = &frame.cells[(usage_y + 1) * frame.width as usize..];
    let percent_positions: Vec<usize> = (0..25).filter(|&x| claude_row[x].symbol == "%").collect();
    assert_eq!(
        percent_positions.len(),
        2,
        "two percent signs in Claude row"
    );
    let palette = &state.config.palette;
    assert_eq!(
        claude_row[percent_positions[0]].fg,
        crate::protocol::color_to_u32(palette.text)
    );
    assert_eq!(
        claude_row[percent_positions[1]].fg,
        crate::protocol::color_to_u32(palette.overlay0)
    );
}

#[test]
fn a_one_window_pool_shows_that_single_window() {
    let mut usage = usage();
    usage[0].figure = window("5h", 0.50, 0.8, 7200);
    set_rows_for_test(Some(usage));
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let narrow_frame = state.compose(106, 40).expect("composed frame").frame;
    let narrow_rows: Vec<String> = narrow_frame
        .cells
        .chunks(narrow_frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    let (narrow_usage_y, _) = row_with(&narrow_rows, " usage").unwrap();
    let narrow_claude = sidebar(&narrow_rows[narrow_usage_y + 1]);
    assert!(
        narrow_claude.trim_end().ends_with("50% 5h"),
        "narrow: {narrow_claude:?}"
    );

    state.config.sidebar_max_width = 40;
    state.sidebar_width = 40;
    let wide_frame = state.compose(120, 40).expect("composed frame").frame;
    set_rows_for_test(None);
    let wide_rows: Vec<String> = wide_frame
        .cells
        .chunks(wide_frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    let (wide_usage_y, _) = row_with(&wide_rows, " usage").unwrap();
    let wide_claude: String = wide_rows[wide_usage_y + 1].chars().take(40).collect();
    let wide_claude = wide_claude.split('\u{2502}').next().unwrap();
    assert!(
        wide_claude.trim_end().ends_with("50% 5h     2h"),
        "wide: {wide_claude:?}"
    );
    assert!(
        wide_claude.contains('\u{2501}'),
        "bar drawn: {wide_claude:?}"
    );
}

#[test]
fn dotted_windows_are_never_shown_in_the_foot() {
    use crate::nirvana_usage::{foot_rows, Reading, Source};
    let mut sources: [Source; 4] = Default::default();
    sources[0].read_at = Some(100_000);
    sources[0].readings = vec![
        Reading {
            pool: "max".into(),
            window: "5h".into(),
            used: Some(0.26),
            resets_at: Some(110_000),
            resets_text: None,
            read_at: 100_000,
            source: "claude".into(),
        },
        Reading {
            pool: "max".into(),
            window: "week.fable".into(),
            used: Some(0.99),
            resets_at: Some(200_000),
            resets_text: None,
            read_at: 100_000,
            source: "claude".into(),
        },
        Reading {
            pool: "max".into(),
            window: "week".into(),
            used: Some(0.65),
            resets_at: Some(600_000),
            resets_text: None,
            read_at: 100_000,
            source: "claude".into(),
        },
    ];
    let rows = foot_rows(&sources, 100_000);
    let claude_row = rows.iter().find(|r| r.pool == "max").unwrap();
    match &claude_row.figure {
        Figure::Windows(windows) => {
            assert_eq!(windows.len(), 2);
            assert_eq!(windows[0].window, "5h");
            assert_eq!(windows[1].window, "week");
            assert!(!windows.iter().any(|w| w.window.contains('.')));
        }
        other => panic!("expected Figure::Windows, got {other:?}"),
    }
}

#[test]
fn accounts_footer_button_shows_warning_mark_on_drift_and_hides_when_matched() {
    let drift = crate::nirvana_accounts::Drift {
        chosen: "personal.fahim.amin@gmail.com".into(),
        current: "fahim.private@gmail.com".into(),
        app_running: true,
    };

    // 1. When drift is set, foot button shows "accounts ⚠" with ⚠ in yellow
    crate::nirvana_accounts::set_drift_for_test(Some(Some(drift)));
    let (state, frame, rows) = compose_with("", 106, 40, None);
    let button_y = rows.len() - 1;
    let button_row = &rows[button_y];
    assert!(
        button_row.contains("accounts \u{26A0}"),
        "button row should contain 'accounts ⚠': {button_row:?}"
    );

    // Verify ⚠ cell color is palette.yellow
    let row_cells = &frame.cells[button_y * frame.width as usize..][..(frame.width as usize)];
    let warn_cell = row_cells
        .iter()
        .find(|cell| cell.symbol == "\u{26A0}")
        .expect("warning mark cell in footer");
    assert_eq!(
        warn_cell.fg,
        crate::protocol::color_to_u32(state.config.palette.yellow),
        "warning mark cell should use palette.yellow"
    );

    // 2. When drift is cleared (matches or no choice), " ⚠" is hidden
    crate::nirvana_accounts::set_drift_for_test(Some(None));
    let (_, _, rows) = compose_with("", 106, 40, None);
    let button_row = &rows[rows.len() - 1];
    assert!(button_row.contains("accounts"));
    assert!(
        !button_row.contains('\u{26A0}'),
        "no warning mark when matched: {button_row:?}"
    );

    crate::nirvana_accounts::set_drift_for_test(None);
}

#[test]
fn accounts_panel_row_and_cause_line_with_app_running_wide() {
    use crate::client::shell::nirvana_accounts::AccountsPanel;
    let chosen = "personal.fahim.amin@gmail.com";
    let current = "fahim.private@gmail.com";
    let drift = crate::nirvana_accounts::Drift {
        chosen: chosen.into(),
        current: current.into(),
        app_running: true,
    };
    let accounts_list = vec![
        crate::nirvana_accounts::Account {
            cli: crate::nirvana_accounts::Cli::Claude,
            signed: crate::nirvana_accounts::Signed::In("me@example.com · max".into()),
        },
        crate::nirvana_accounts::Account {
            cli: crate::nirvana_accounts::Cli::Codex,
            signed: crate::nirvana_accounts::Signed::Out,
        },
        crate::nirvana_accounts::Account {
            cli: crate::nirvana_accounts::Cli::Antigravity,
            signed: crate::nirvana_accounts::Signed::In(current.into()),
        },
    ];

    crate::nirvana_accounts::set_accounts_for_test(Some(accounts_list));
    crate::nirvana_accounts::set_drift_for_test(Some(Some(drift)));

    let (mut state, _, _) = compose_with("", 140, 40, None);
    state.nirvana_accounts = Some(AccountsPanel::default());

    let frame = state.compose(140, 40).expect("composed frame").frame;
    let rows: Vec<String> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();

    let (agy_y, agy_row) = row_with(&rows, "Antigravity").expect("Antigravity row");
    assert!(
        agy_row.contains(&format!("{current} \u{26A0} chosen {chosen}")),
        "wide row format: {agy_row:?}"
    );
    assert!(
        agy_row.contains("[ switch ]"),
        "button stays switch: {agy_row:?}"
    );

    // The ⚠ cell in the Antigravity row has palette.yellow
    let row_cells = &frame.cells[agy_y * frame.width as usize..][..(frame.width as usize)];
    let warn_cell = row_cells
        .iter()
        .find(|cell| cell.symbol == "\u{26A0}")
        .expect("warning mark in agy row");
    assert_eq!(
        warn_cell.fg,
        crate::protocol::color_to_u32(state.config.palette.yellow),
        "warning mark should be palette.yellow"
    );

    // The line under the row says why while Antigravity.exe runs
    let cause_row = &rows[agy_y + 1];
    let expected_cause =
        format!("The Antigravity app put this account back. Sign the app in as {chosen} too.");
    assert!(
        cause_row.contains(&expected_cause),
        "cause line under row: {cause_row:?}"
    );

    crate::nirvana_accounts::set_accounts_for_test(None);
    crate::nirvana_accounts::set_drift_for_test(None);
}

#[test]
fn accounts_panel_cause_line_without_app_running() {
    use crate::client::shell::nirvana_accounts::AccountsPanel;
    let chosen = "personal.fahim.amin@gmail.com";
    let current = "fahim.private@gmail.com";
    let drift = crate::nirvana_accounts::Drift {
        chosen: chosen.into(),
        current: current.into(),
        app_running: false,
    };
    let accounts_list = vec![
        crate::nirvana_accounts::Account {
            cli: crate::nirvana_accounts::Cli::Claude,
            signed: crate::nirvana_accounts::Signed::In("me@example.com · max".into()),
        },
        crate::nirvana_accounts::Account {
            cli: crate::nirvana_accounts::Cli::Codex,
            signed: crate::nirvana_accounts::Signed::Out,
        },
        crate::nirvana_accounts::Account {
            cli: crate::nirvana_accounts::Cli::Antigravity,
            signed: crate::nirvana_accounts::Signed::In(current.into()),
        },
    ];

    crate::nirvana_accounts::set_accounts_for_test(Some(accounts_list));
    crate::nirvana_accounts::set_drift_for_test(Some(Some(drift)));

    let (mut state, _, _) = compose_with("", 130, 40, None);
    state.nirvana_accounts = Some(AccountsPanel::default());

    let frame = state.compose(130, 40).expect("composed frame").frame;
    let rows: Vec<String> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();

    let (agy_y, _) = row_with(&rows, "Antigravity").expect("Antigravity row");
    let cause_row = &rows[agy_y + 1];
    assert!(
        cause_row.contains("agy went back to this account. Press switch to sign in again."),
        "cause line without app running: {cause_row:?}"
    );

    crate::nirvana_accounts::set_accounts_for_test(None);
    crate::nirvana_accounts::set_drift_for_test(None);
}

#[test]
fn accounts_row_narrow_drops_chosen_word_then_drops_email_keeping_warning_mark() {
    use crate::client::shell::nirvana_accounts::{
        account_drift_text, row_text_with_drift, AccountsPanel,
    };
    let chosen = "personal.fahim.amin@gmail.com";
    let current = "fahim.private@gmail.com";
    let drift = crate::nirvana_accounts::Drift {
        chosen: chosen.into(),
        current: current.into(),
        app_running: true,
    };

    // Wide room (65 cells): includes "chosen" word and chosen email
    let wide = account_drift_text(current, &drift, 65);
    assert_eq!(wide, format!("{current} \u{26A0} chosen {chosen}"));

    // Medium room (55 cells): drops the word "chosen", keeps email
    let medium = account_drift_text(current, &drift, 55);
    assert_eq!(medium, format!("{current} \u{26A0} {chosen}"));
    assert!(!medium.contains("chosen"));

    // Narrow room (35 cells): drops chosen email, keeps warning mark
    let narrow = account_drift_text(current, &drift, 35);
    assert_eq!(narrow, format!("{current} \u{26A0}"));
    assert!(!narrow.contains(chosen));
    assert!(narrow.contains('\u{26A0}'));

    // Row text formatting with drift
    let account = crate::nirvana_accounts::Account {
        cli: crate::nirvana_accounts::Cli::Antigravity,
        signed: crate::nirvana_accounts::Signed::In(current.into()),
    };
    let (left, button) = row_text_with_drift(&account, Some(&drift), 92);
    assert!(left.contains(&format!("{current} \u{26A0} chosen {chosen}")));
    assert_eq!(button, "[ switch ]");

    // Narrow row text
    let (narrow_left, _) = row_text_with_drift(&account, Some(&drift), 55);
    assert!(narrow_left.contains('\u{26A0}'));
    assert!(!narrow_left.contains("chosen"));

    // Client compose test at narrow window width (85): row drops the word "chosen" and email, keeps ⚠
    let accounts_list = vec![
        crate::nirvana_accounts::Account {
            cli: crate::nirvana_accounts::Cli::Claude,
            signed: crate::nirvana_accounts::Signed::In("me@example.com · max".into()),
        },
        crate::nirvana_accounts::Account {
            cli: crate::nirvana_accounts::Cli::Codex,
            signed: crate::nirvana_accounts::Signed::Out,
        },
        crate::nirvana_accounts::Account {
            cli: crate::nirvana_accounts::Cli::Antigravity,
            signed: crate::nirvana_accounts::Signed::In(current.into()),
        },
    ];
    crate::nirvana_accounts::set_accounts_for_test(Some(accounts_list));
    crate::nirvana_accounts::set_drift_for_test(Some(Some(drift)));

    let (mut state, _, _) = compose_with("", 85, 40, None);
    state.nirvana_accounts = Some(AccountsPanel::default());
    let frame = state.compose(85, 40).expect("composed frame").frame;
    let rows: Vec<String> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();

    let (_, agy_row) = row_with(&rows, "Antigravity").expect("Antigravity row in narrow");
    assert!(
        agy_row.contains('\u{26A0}'),
        "narrow row retains warning mark: {agy_row:?}"
    );
    assert!(
        !agy_row.contains("chosen"),
        "narrow row drops chosen word: {agy_row:?}"
    );

    crate::nirvana_accounts::set_accounts_for_test(None);
    crate::nirvana_accounts::set_drift_for_test(None);
}
