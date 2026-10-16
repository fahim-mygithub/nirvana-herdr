//! Nirvana fork: running servers and usage at the foot of the sidebar
//! (nirvana/plans/2026-10-15-04-servers-and-usage.md L1).

use super::*;
use crate::client::shell::nirvana_footer::{address, figures, footer_height, split, AGENTS_KEEP};
use crate::nirvana_usage::{set_rows_for_test, Figure, Provider, UsageRow};

fn usage() -> Vec<UsageRow> {
    vec![
        UsageRow {
            provider: Provider::Claude,
            figure: Figure::Windows {
                five_hour: Some(2),
                seven_day: Some(70),
            },
        },
        UsageRow {
            provider: Provider::Codex,
            figure: Figure::Word("not exposed"),
        },
        UsageRow {
            provider: Provider::Antigravity,
            figure: Figure::Windows {
                five_hour: Some(0),
                seven_day: Some(0),
            },
        },
        UsageRow {
            provider: Provider::Local,
            figure: Figure::Memory {
                used_mib: 3539,
                total_mib: 32607,
            },
        },
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

#[test]
fn the_foot_lists_a_spaces_server_and_the_four_usage_rows() {
    let (_, _, rows) = compose_with("8770", 106, 40, None);
    let (servers_y, _) = row_with(&rows, " servers").expect("servers head");
    let server = sidebar(&rows[servers_y + 1]);
    assert!(
        server.starts_with(" \u{25CF} client-shell") && server.trim_end().ends_with(":8770"),
        "server row: {server:?}"
    );
    let (usage_y, head) = row_with(&rows, " usage").expect("usage head");
    assert!(
        sidebar(head).trim_end().ends_with("5h   7D"),
        "head: {head:?}"
    );
    assert!(usage_y > servers_y);
    let expected = [
        ("Claude", "2%  70%"),
        ("Codex", "not exposed"),
        ("Antigravity", "0%   0%"),
        ("Local", "3/32 GiB"),
    ];
    for (offset, (name, figure)) in expected.into_iter().enumerate() {
        let row = sidebar(&rows[usage_y + 1 + offset]);
        assert!(
            row.starts_with(&format!(" \u{25CF} {name}")) && row.trim_end().ends_with(figure),
            "usage row {offset}: {row:?}"
        );
    }
    // The foot sits at the very bottom of the sidebar.
    assert_eq!(usage_y + 4, rows.len() - 1);
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
    assert_eq!(dot(usage_y + 1), rgb(0xD8, 0x88, 0x6A));
    assert_eq!(dot(usage_y + 2), rgb(0x7B, 0xAE, 0xDD));
    assert_eq!(dot(usage_y + 3), rgb(0xAB, 0x93, 0xD6));
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
    let (_, _, rows) = compose_with("8770", 106, 20, None);
    assert!(row_with(&rows, " usage").is_none());
    assert!(row_with(&rows, " agents").is_some());
}

#[test]
fn a_wide_sidebar_draws_a_bar_for_the_five_hour_window() {
    let (_, _, rows) = compose_with("8770", 120, 40, Some(40));
    let (usage_y, _) = row_with(&rows, " usage").unwrap();
    let claude: String = rows[usage_y + 1].chars().take(40).collect();
    assert!(
        claude.contains('\u{2501}') || claude.contains('\u{2500}'),
        "{claude:?}"
    );
    // Codex has no figure: a dashed track.
    let codex: String = rows[usage_y + 2].chars().take(40).collect();
    assert!(codex.contains('\u{254C}'), "{codex:?}");
    // The server row has room for the whole address.
    let (servers_y, _) = row_with(&rows, " servers").unwrap();
    assert!(rows[servers_y + 1].contains("localhost:8770"));
}

#[test]
fn the_layout_rules() {
    assert_eq!(footer_height(0), 6);
    assert_eq!(footer_height(1), 8);
    assert_eq!(footer_height(9), 12);
    let detail = Rect::new(0, 10, 25, 14);
    let (agents, foot) = split(detail, 8);
    assert_eq!((agents.height, foot.y, foot.height), (6, 16, 8));
    let (agents, foot) = split(detail, 14 - AGENTS_KEEP + 1);
    assert_eq!((agents, foot), (detail, Rect::default()));
    assert_eq!(address(8770, 11, 22), ":8770");
    assert_eq!(address(8770, 4, 22), "localhost:8770");
    assert_eq!(
        figures(&Figure::Windows {
            five_hour: None,
            seven_day: Some(100)
        }),
        "  -- 100%"
    );
    assert_eq!(figures(&Figure::Reading), "reading");
}
