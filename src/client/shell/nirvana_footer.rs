//! Nirvana fork, feature 4: the foot of the expanded sidebar, under the agents panel, as the old
//! Nirvana rail ends (the owner's Image #17, 2026-10-15):
//!
//! ```text
//! ──────────────────────────
//!  servers
//!  ● MotoProject      :8770
//!  usage
//!  ● Claude        41% 5h▲
//!  ● Gemini         2% 5h
//!  ● Codex     not exposed
//!  ● Local        3/32 GiB
//!  accounts
//! ```
//!
//! The servers come from the server, as each space's `nirvana_servers` token (`crate::nirvana_servers`);
//! the usage rows are read by this window (`crate::nirvana_usage`). The "servers" block hides when no
//! space runs one, as old Nirvana's does.
//!
//! Feature 11: the last row is the "accounts" button, which opens the Accounts panel
//! (`nirvana_accounts.rs`). The owner, 2026-10-17: "In the bottom left let me manage my login accounts."
//!
//! Plan 12 O1 (§8): one usage row per pool, with Argon and Google's Claude pool beside Gemini, which were one
//! "Antigravity" row before. A row shows its pool's tightest window (`nirvana_usage::tightest`), so one figure
//! and its window's name replace the "5h 7D" columns; a pace mark when it is spent faster than it passes; and
//! from `BAR_FROM_WIDTH` the time to its reset. A stale reading is drawn muted.
//!
//! 2026-10-18, the owner: "We dont need tracking for g claude or argon on the usage keep that internally." Those
//! two pools are read and priced but not drawn (`nirvana_usage::FOOT_HIDDEN`); `herdr usage` still lists them.

use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crate::nirvana_usage::{short_duration, Figure, UsageRow};
use ratatui::style::Color;

/// At most this many server rows, then "+N more".
pub(super) const SERVERS_SHOWN: usize = 4;
/// The agents panel keeps at least its head (three rows) and two agents; below that the foot is not
/// drawn and the agents panel keeps the whole area.
pub(super) const AGENTS_KEEP: u16 = 5;
/// From this sidebar width a bar sits between the name and the figures, and the reset follows the figure.
/// herdr's default is 26.
pub(super) const BAR_FROM_WIDTH: u16 = 34;
/// The percent: "100%".
const COLUMN: usize = 4;
/// The figure, " 41% 5h▲", and from `BAR_FROM_WIDTH` " 41% 5h▲ 2h58m": the reset takes five cells.
const FIGURE: usize = COLUMN + 1 + 2 + 1;
const WIDE_FIGURE: usize = FIGURE + 1 + 5;
/// The pace mark's cell in the figure.
const MARK_AT: usize = COLUMN + 1 + 2;
pub(super) const AHEAD: &str = "\u{25B2}";
/// The widest pool name, "G·Claude", so the bars line up. That row is hidden now (`FOOT_HIDDEN`), but an agy pool
/// no prefix maps gets a row with its own name, so the column keeps its room.
const NAME: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ServerRow {
    pub(super) space: String,
    pub(super) port: u16,
}

/// Every space's servers, in the spaces list's order.
pub(super) fn server_rows(snapshot: &ClientShellSnapshot) -> Vec<ServerRow> {
    snapshot
        .workspaces
        .iter()
        .flat_map(|workspace| {
            workspace
                .tokens
                .iter()
                .filter(|(key, _)| key == crate::nirvana_servers::SERVERS_TOKEN)
                .flat_map(|(_, value)| crate::nirvana_servers::decode(value))
                .map(|port| ServerRow {
                    space: workspace.label.clone(),
                    port,
                })
        })
        .collect()
}

/// The foot's rows: a divider, the servers block when there is one, the usage head and a row per pool, and
/// the accounts button.
pub(super) fn footer_height(servers: usize, pools: usize) -> u16 {
    let servers = if servers == 0 {
        0
    } else {
        1 + servers.min(SERVERS_SHOWN) + usize::from(servers > SERVERS_SHOWN)
    };
    (1 + servers + 1 + pools + 1) as u16
}

/// The agents panel's area and the foot's, out of the sidebar's lower section.
pub(super) fn split(detail: Rect, foot: u16) -> (Rect, Rect) {
    if detail.height < foot.saturating_add(AGENTS_KEEP) {
        return (detail, Rect::default());
    }
    let agents = detail.height - foot;
    (
        Rect::new(detail.x, detail.y, detail.width, agents),
        Rect::new(detail.x, detail.y + agents, detail.width, foot),
    )
}

/// Whether any Claude in the window is working: then Claude's window moves and is read more often.
pub(super) fn claude_working(snapshot: &ClientShellSnapshot) -> bool {
    snapshot.agents.iter().any(|agent| {
        agent.agent_status == crate::api::schema::AgentStatus::Working
            && agent
                .agent
                .as_deref()
                .and_then(crate::nirvana_spaces::Kind::from_agent_label)
                == Some(crate::nirvana_spaces::Kind::Claude)
    })
}

fn heading(buffer: &mut Buffer, area: Rect, y: u16, text: &str, palette: &Palette) {
    put_text(
        buffer,
        area.x,
        y,
        area.width,
        text,
        Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::BOLD),
    );
}

fn right(buffer: &mut Buffer, area: Rect, y: u16, text: &str, style: Style) {
    let width = (unicode_width::UnicodeWidthStr::width(text) as u16).min(area.width);
    put_text(
        buffer,
        area.right().saturating_sub(width),
        y,
        width,
        text,
        style,
    );
}

/// "localhost:8770" when it fits beside the space's whole name, else ":8770": the name says which
/// space, so it keeps its cells first.
pub(super) fn address(port: u16, name_width: usize, room: usize) -> String {
    let long = format!("localhost:{port}");
    if name_width + 1 + long.len() <= room {
        long
    } else {
        format!(":{port}")
    }
}

/// "5h", "7d" for every week; another window by its first two letters.
fn window_name(window: &str) -> &str {
    match window.split('.').next() {
        Some("5h") => "5h",
        Some("week") => "7d",
        _ => window.get(..2).unwrap_or(window),
    }
}

/// The figure cell: the tightest window's percent ("--" when not exposed), its window and the pace mark's
/// cell; when `wide`, the time to the reset. Or one word or memory figure in its place.
pub(super) fn figures(figure: &Figure, wide: bool) -> String {
    match figure {
        Figure::Window(shown) => {
            let used = shown.used.map_or_else(
                || "--".to_owned(),
                |used| format!("{}%", (used * 100.0).round() as u32),
            );
            let mark = if figure.ahead() { AHEAD } else { " " };
            let mut text = format!("{used:>COLUMN$} {:<2}{mark}", window_name(&shown.window));
            if wide {
                let resets = shown.resets_in.map(short_duration).unwrap_or_default();
                text.push_str(&format!(" {resets:>5}"));
            }
            text
        }
        Figure::Memory {
            used_mib,
            total_mib,
        } => format!(
            "{}/{} GiB",
            (*used_mib as f64 / 1024.0).round(),
            (*total_mib as f64 / 1024.0).round()
        ),
        Figure::Word(word) => word.clone(),
        Figure::Reading => "reading".to_owned(),
    }
}

/// The hits a click on a server row opens.
pub(super) type ServerHits = Vec<(Rect, u16)>;

pub(super) fn render_footer(
    buffer: &mut Buffer,
    area: Rect,
    servers: &[ServerRow],
    usage: &[UsageRow],
    palette: &Palette,
    hits: &mut ServerHits,
    accounts: &mut Rect,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    // The button keeps the bottom row, so the rows above stop one short of it.
    let button_y = area.bottom() - 1;
    let button = super::nirvana_accounts::BUTTON;
    let button_width = (button.len() as u16).min(area.width.saturating_sub(1));
    put_text(
        buffer,
        area.x + 1,
        button_y,
        button_width,
        button,
        Style::default().fg(palette.overlay0),
    );
    *accounts = Rect::new(area.x, button_y, button_width + 1, 1);
    let area = Rect::new(area.x, area.y, area.width, area.height - 1);
    if area.height == 0 {
        return;
    }
    let mut y = area.y;
    put_text(
        buffer,
        area.x,
        y,
        area.width,
        &"\u{2500}".repeat(area.width as usize),
        Style::default().fg(palette.surface_dim),
    );
    y += 1;
    let room = area.width.saturating_sub(3) as usize;
    if !servers.is_empty() && y < area.bottom() {
        heading(buffer, area, y, " servers", palette);
        y += 1;
        for server in servers.iter().take(SERVERS_SHOWN) {
            if y >= area.bottom() {
                break;
            }
            let name_width = unicode_width::UnicodeWidthStr::width(server.space.as_str());
            let address = address(server.port, name_width, room);
            let name = fit(&server.space, room.saturating_sub(address.len() + 1));
            put_text(
                buffer,
                area.x + 1,
                y,
                1,
                "\u{25CF}",
                Style::default().fg(palette.green),
            );
            put_text(
                buffer,
                area.x + 3,
                y,
                area.width.saturating_sub(3),
                &name,
                Style::default().fg(palette.text),
            );
            right(
                buffer,
                area,
                y,
                &address,
                Style::default().fg(palette.overlay1),
            );
            hits.push((Rect::new(area.x, y, area.width, 1), server.port));
            y += 1;
        }
        if servers.len() > SERVERS_SHOWN && y < area.bottom() {
            put_text(
                buffer,
                area.x + 3,
                y,
                area.width.saturating_sub(3),
                &format!("+{} more", servers.len() - SERVERS_SHOWN),
                Style::default().fg(palette.overlay0),
            );
            y += 1;
        }
    }
    if y >= area.bottom() {
        return;
    }
    heading(buffer, area, y, " usage", palette);
    y += 1;
    let bar = area.width >= BAR_FROM_WIDTH;
    for row in usage {
        if y >= area.bottom() {
            break;
        }
        let colour = row
            .provider
            .colour()
            .map_or(palette.overlay1, |(r, g, b)| Color::Rgb(r, g, b));
        put_text(
            buffer,
            area.x + 1,
            y,
            1,
            "\u{25CF}",
            Style::default().fg(colour),
        );
        let text = figures(&row.figure, bar);
        let text_width = unicode_width::UnicodeWidthStr::width(text.as_str());
        let name = fit(row.label(), room.saturating_sub(text_width + 1));
        put_text(
            buffer,
            area.x + 3,
            y,
            area.width.saturating_sub(3),
            &name,
            Style::default().fg(palette.text),
        );
        let style = match &row.figure {
            Figure::Window(shown) if !shown.stale && shown.used.is_some() => {
                Style::default().fg(palette.text)
            }
            Figure::Memory { .. } => Style::default().fg(palette.text),
            _ => Style::default().fg(palette.overlay0),
        };
        right(buffer, area, y, &text, style);
        // The mark in the warning colour, over its cell in the figure.
        if row.figure.ahead() && text_width <= area.width as usize {
            put_text(
                buffer,
                area.right() - text_width as u16 + MARK_AT as u16,
                y,
                1,
                AHEAD,
                Style::default().fg(palette.peach),
            );
        }
        if bar {
            // Between the name column and the figures, one cell of space on each side.
            let start = area.x + 3 + NAME as u16 + 1;
            let end = area
                .right()
                .saturating_sub(text_width.max(if bar { WIDE_FIGURE } else { FIGURE }) as u16 + 1);
            let width = end.saturating_sub(start) as usize;
            if width >= 3 {
                let filled = row
                    .figure
                    .fill()
                    .map_or(0, |fill| (fill * width as f32).round() as usize)
                    .min(width);
                put_text(
                    buffer,
                    start,
                    y,
                    filled as u16,
                    &"\u{2501}".repeat(filled),
                    Style::default().fg(colour),
                );
                // No figure: a dashed track, as old Nirvana hatches a bar with nothing to show.
                let track = if row.figure.fill().is_some() {
                    "\u{2500}"
                } else {
                    "\u{254C}"
                };
                put_text(
                    buffer,
                    start + filled as u16,
                    y,
                    (width - filled) as u16,
                    &track.repeat(width - filled),
                    Style::default().fg(palette.surface_dim),
                );
            }
        }
        y += 1;
    }
}
