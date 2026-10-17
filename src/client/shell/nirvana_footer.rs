//! Nirvana fork, feature 4: the foot of the expanded sidebar, under the agents panel, as the old
//! Nirvana rail ends (the owner's Image #17, 2026-10-15):
//!
//! ```text
//! ──────────────────────────
//!  servers
//!  ● MotoProject      :8770
//!  usage            5h   7D
//!  ● Claude          2%  70%
//!  ● Codex      not exposed
//!  ● Antigravity     0%   0%
//!  ● Local         3/32 GiB
//! ```
//!
//! The servers come from the server, as each space's `nirvana_servers` token (`crate::nirvana_servers`);
//! the usage rows are read by this window (`crate::nirvana_usage`). The "servers" block hides when no
//! space runs one, as old Nirvana's does.
//!
//! Feature 11: the last row is the "accounts" button, which opens the Accounts panel
//! (`nirvana_accounts.rs`). The owner, 2026-10-17: "In the bottom left let me manage my login accounts."

use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crate::nirvana_usage::{Figure, Provider, UsageRow};
use ratatui::style::Color;

/// At most this many server rows, then "+N more".
pub(super) const SERVERS_SHOWN: usize = 4;
/// The agents panel keeps at least its head (three rows) and two agents; below that the foot is not
/// drawn and the agents panel keeps the whole area.
pub(super) const AGENTS_KEEP: u16 = 5;
/// From this sidebar width a bar sits between the name and the figures. herdr's default is 26.
pub(super) const BAR_FROM_WIDTH: u16 = 34;
/// One figure column: "100%".
const COLUMN: usize = 4;
/// The widest provider name, "Antigravity", so the bars line up.
const NAME: usize = 11;

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

/// The foot's rows: a divider, the servers block when there is one, the usage head and its four rows, and
/// the accounts button.
pub(super) fn footer_height(servers: usize) -> u16 {
    let servers = if servers == 0 {
        0
    } else {
        1 + servers.min(SERVERS_SHOWN) + usize::from(servers > SERVERS_SHOWN)
    };
    (1 + servers + 1 + Provider::ALL.len() + 1) as u16
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

/// The figures cell: two columns under "5h 7D", or one word or memory figure in their place.
pub(super) fn figures(figure: &Figure) -> String {
    let cell = |used: Option<u8>| used.map_or_else(|| "--".to_owned(), |used| format!("{used}%"));
    match figure {
        Figure::Windows {
            five_hour,
            seven_day,
        } => format!(
            "{:>COLUMN$} {:>COLUMN$}",
            cell(*five_hour),
            cell(*seven_day)
        ),
        Figure::Memory {
            used_mib,
            total_mib,
        } => format!(
            "{}/{} GiB",
            (*used_mib as f64 / 1024.0).round(),
            (*total_mib as f64 / 1024.0).round()
        ),
        Figure::Word(word) => (*word).to_owned(),
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
    right(
        buffer,
        area,
        y,
        &format!("{:>COLUMN$} {:>COLUMN$}", "5h", "7D"),
        Style::default().fg(palette.overlay0),
    );
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
        let text = figures(&row.figure);
        let text_width = unicode_width::UnicodeWidthStr::width(text.as_str());
        let name = fit(row.provider.name(), room.saturating_sub(text_width + 1));
        put_text(
            buffer,
            area.x + 3,
            y,
            area.width.saturating_sub(3),
            &name,
            Style::default().fg(palette.text),
        );
        let style = match row.figure {
            Figure::Windows { .. } | Figure::Memory { .. } => Style::default().fg(palette.text),
            _ => Style::default().fg(palette.overlay0),
        };
        right(buffer, area, y, &text, style);
        if bar {
            // Between the name column and the figures, one cell of space on each side.
            let start = area.x + 3 + NAME as u16 + 1;
            let end = area
                .right()
                .saturating_sub(text_width.max(2 * COLUMN + 1) as u16 + 1);
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
