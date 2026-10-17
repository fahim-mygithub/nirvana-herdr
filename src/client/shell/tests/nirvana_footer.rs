//! Nirvana fork: running servers and usage at the foot of the sidebar
//! (nirvana/plans/2026-10-15-04-servers-and-usage.md L1).

use super::*;
use crate::client::shell::nirvana_footer::{
    address, figures, footer_height, split, AGENTS_KEEP, AHEAD,
};
use crate::nirvana_usage::{set_rows_for_test, Figure, Provider, Shown, UsageRow};

fn shown(window: &str, used: f32, pace: f32, resets_in: u64) -> Figure {
    Figure::Window(Shown {
        window: window.into(),
        used: Some(used),
        pace: Some(pace),
        resets_in: Some(resets_in),
        stale: false,
    })
}

fn row(pool: &str, provider: Provider, figure: Figure) -> UsageRow {
    UsageRow {
        pool: pool.into(),
        provider,
        figure,
    }
}

/// One row per pool (plan 12 O1), the readings of 2026-10-17: Claude's 5 h window ahead of pace.
fn usage() -> Vec<UsageRow> {
    vec![
        row("max", Provider::Claude, shown("5h", 0.41, 1.01, 10_698)),
        row(
            "gemini",
            Provider::Antigravity,
            shown("5h", 0.02, 0.79, 17_034),
        ),
        row(
            "argon",
            Provider::Antigravity,
            shown("week", 0.021, 0.9, 583_910),
        ),
        row(
            "google3p",
            Provider::Antigravity,
            shown("week", 0.04, 0.6, 133_752),
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
        ("Claude", format!("41% 5h{AHEAD}")),
        ("Gemini", "2% 5h".to_owned()),
        ("Argon", "2% 7d".to_owned()),
        ("G\u{b7}Claude", "4% 7d".to_owned()),
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
    // The foot sits at the very bottom of the sidebar, its last row the accounts button.
    assert_eq!(usage_y + 7, rows.len() - 1);
    assert!(
        sidebar(&rows[usage_y + 7]).trim_end() == " accounts",
        "{:?}",
        rows[usage_y + 7]
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
    if let Figure::Window(shown) = &mut usage[1].figure {
        shown.stale = true;
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
    // Each pool's dot is its reader's colour: Gemini, Argon and Google's Claude pool are agy's.
    assert_eq!(dot(usage_y + 1), rgb(0xD8, 0x88, 0x6A));
    for offset in 2..=4 {
        assert_eq!(dot(usage_y + offset), rgb(0xAB, 0x93, 0xD6), "row {offset}");
    }
    assert_eq!(dot(usage_y + 5), rgb(0x7B, 0xAE, 0xDD));
    assert_eq!(
        dot(usage_y + 6),
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
    let (_, _, rows) = compose_with("8770", 106, 20, None);
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
    // The reset follows the figure only here: 10,698 s is 2 h 58 min. The sidebar ends at its divider.
    let claude = claude.split('\u{2502}').next().unwrap();
    assert!(
        claude.trim_end().ends_with(&format!("41% 5h{AHEAD} 2h58m")),
        "{claude:?}"
    );
    let argon = rows[usage_y + 3].split('\u{2502}').next().unwrap();
    assert!(argon.trim_end().ends_with("2% 7d  6d18h"), "{argon:?}");
    // Codex has no figure: a dashed track.
    let codex: String = rows[usage_y + 5].chars().take(40).collect();
    assert!(codex.contains('\u{254C}'), "{codex:?}");
    // At herdr's default width there is no reset.
    let (_, _, narrow) = compose_with("8770", 106, 40, None);
    let (usage_y, _) = row_with(&narrow, " usage").unwrap();
    assert!(!sidebar(&narrow[usage_y + 1]).contains("2h58m"));
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
    });
    assert_eq!(figures(&unknown, false), "  -- 7d ");
    assert_eq!(figures(&unknown, true), "  -- 7d       ");
    assert_eq!(
        figures(&shown("5h", 1.0, 2.0, 45 * 60), true),
        format!("100% 5h{AHEAD}   45m")
    );
    // A scoped week reads as a week.
    assert_eq!(
        figures(&shown("week.fable", 0.5, 0.9, 0), false),
        " 50% 7d "
    );
    assert_eq!(figures(&Figure::Reading, false), "reading");
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
