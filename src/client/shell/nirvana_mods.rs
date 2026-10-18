//! Nirvana fork, feature 5: the mod strip under the tab bar, and its one mod, Preload.
//!
//! The owner, 2026-10-16: "under the 1 tool bar we will have a mod toolbar that is attached to each agent.
//! ... I wawnt just one mod square for the herder version for now. This mod square is called Preload"
//! (nirvana/plans/2026-10-16-05-preload-mod.md). Old Nirvana's task view opens with a strip of squares,
//! each a bold label and a muted value ("Running  Nothing running"); this strip draws its squares the same
//! way, one row tall, for the Claude in the focused tab:
//!
//! ```text
//!  ✻ Claude   Preload  23.6k · 2 off ▾
//! ```
//!
//! A click on the square opens the Preload panel over the panes: every item a fresh Claude in the agent's
//! folder preloads, grouped, with its tokens; space switches the selected item off or on for every new
//! Claude there, `e` opens its file in the editor, `r` restarts this Claude so it takes the change.
//! The list and the switches are `crate::nirvana_preload`.

use std::path::PathBuf;

use super::nirvana_spaces::{agent_kind, fit};
use super::render::put_text;
use super::*;
use crate::api::schema::AgentStatus;
use crate::nirvana_preload::{self as preload, Group, Item, Switch, View};
use crate::nirvana_running::{Activity, Prompt};
use crate::nirvana_spaces::Kind;
use crate::protocol::ClientShellAgent;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::{Color, Modifier};

/// The strip is one row.
pub(super) const STRIP_HEIGHT: u16 = 1;
/// The panel is at most this wide.
pub(super) const PANEL_WIDTH: u16 = 92;
/// Rows a page key moves.
const PAGE: usize = 10;
/// The token column: "129.9k".
const TOKENS: usize = 6;
/// The detail column: "plugin superpowers".
const DETAIL: usize = 22;

/// The agent the strip belongs to: the focused pane's Claude, else the first Claude in the focused tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StripAgent {
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    pub(super) pane_id: String,
    pub(super) cwd: PathBuf,
    /// Feature 6: the transcript the server named, for Running.
    pub(super) transcript: Option<PathBuf>,
    pub(super) status: AgentStatus,
    /// Claude's own terminal title, a summary of the conversation.
    pub(super) title: Option<String>,
}

/// Claude Code, on an Anthropic model or a local one, is what Preload reads.
fn is_claude(agent: &ClientShellAgent) -> bool {
    matches!(agent_kind(agent), Some(Kind::Claude | Kind::Local))
}

pub(super) fn strip_agent(snapshot: &ClientShellSnapshot) -> Option<StripAgent> {
    let tab_id = snapshot.focused_tab_id.as_deref()?;
    let in_tab = |agent: &&ClientShellAgent| agent.tab_id == tab_id && is_claude(agent);
    let agent = snapshot
        .agents
        .iter()
        .filter(in_tab)
        .find(|agent| Some(agent.pane_id.as_str()) == snapshot.focused_pane_id.as_deref())
        .or_else(|| snapshot.agents.iter().find(in_tab))?;
    let pane = snapshot
        .panes
        .iter()
        .find(|pane| pane.pane_id == agent.pane_id)?;
    let cwd = pane
        .foreground_cwd
        .clone()
        .or_else(|| pane.cwd.clone())
        .filter(|cwd| !cwd.is_empty())?;
    let transcript = agent
        .tokens
        .iter()
        .find(|(name, _)| name == crate::nirvana_spaces::TRANSCRIPT_TOKEN)
        .map(|(_, path)| PathBuf::from(path));
    Some(StripAgent {
        workspace_id: agent.workspace_id.clone(),
        tab_id: tab_id.to_owned(),
        pane_id: agent.pane_id.clone(),
        cwd: PathBuf::from(cwd),
        transcript,
        status: agent.agent_status,
        title: agent.terminal_title_stripped.clone(),
    })
}

fn claude_colour(palette: &Palette) -> Color {
    crate::nirvana_usage::Provider::Claude
        .colour()
        .map_or(palette.accent, |(r, g, b)| Color::Rgb(r, g, b))
}

fn width(text: &str) -> u16 {
    u16::try_from(unicode_width::UnicodeWidthStr::width(text)).unwrap_or(u16::MAX)
}

/// The strip's value for the Preload square: "23.6k · 2 off", "reading", "not read".
pub(super) fn preload_value(view: &View) -> String {
    let off = preload::off_count(&view.local);
    let mut value = match &view.report {
        Some(report) => preload::short(report.total),
        None if view.failed => "not read".into(),
        None => "reading".into(),
    };
    if off > 0 {
        value = format!("{value} · {off} off");
    }
    value
}

/// The strip's value for the Running square, and whether it is the user's turn.
pub(super) fn running_value(agent: &StripAgent, activity: Option<&Activity>) -> (String, bool) {
    match agent.status {
        AgentStatus::Blocked => ("waits on you".into(), true),
        AgentStatus::Working => match activity.and_then(|activity| activity.now.as_ref()) {
            Some(tool) if !tool.name.is_empty() => (format!("working \u{b7} {}", tool.name), false),
            _ => ("working".into(), false),
        },
        AgentStatus::Idle | AgentStatus::Done => ("idle".into(), false),
        AgentStatus::Unknown => ("starting".into(), false),
    }
}

/// Where the strip drew its squares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct StripHits {
    pub(super) running: Rect,
    pub(super) preload: Rect,
    /// Feature 8: absent for a folder whose repository has no GitHub remote.
    pub(super) memory: Rect,
    /// Plan 12 O8a: present only in a Nirvana tab.
    pub(super) seats: Rect,
}

/// One square: a bold label and a value on a raised background, as old Nirvana draws its squares.
fn draw_square(
    buffer: &mut Buffer,
    x: u16,
    y: u16,
    right: u16,
    label: &str,
    value: &str,
    value_colour: Color,
    open: bool,
    palette: &Palette,
) -> Rect {
    let label = format!(" {label} ");
    let value = format!("{value} {} ", if open { "\u{25B4}" } else { "\u{25BE}" });
    let square = Rect::new(
        x,
        y,
        (width(&label) + width(&value)).min(right.saturating_sub(x)),
        1,
    );
    if square.width == 0 {
        return Rect::default();
    }
    let raised = Style::default().bg(if open {
        palette.surface1
    } else {
        palette.surface0
    });
    buffer.set_style(square, raised);
    put_text(
        buffer,
        x,
        y,
        square.width,
        &label,
        raised.fg(palette.text).add_modifier(Modifier::BOLD),
    );
    let after = x + width(&label);
    put_text(
        buffer,
        after,
        y,
        square.right().saturating_sub(after),
        &value,
        raised.fg(value_colour),
    );
    square
}

/// Draws the strip: the agent, then the Running square, the Seats square in a Nirvana tab (plan 12 O8a: the
/// tab's seating comes right after what its Claude is doing), the Preload square, and the Memory square when
/// `memory` holds its value. `seats` is the Seats view, `Some(None)` before its first read.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_strip(
    buffer: &mut Buffer,
    area: Rect,
    agent: &StripAgent,
    activity: Option<&Activity>,
    view: &View,
    memory: Option<(String, bool)>,
    seats: Option<Option<&crate::nirvana_seats_square::View>>,
    open: OpenPanel,
    palette: &Palette,
) -> StripHits {
    let mut hits = StripHits::default();
    if area.height == 0 || area.width < 4 {
        return hits;
    }
    let base = Style::default().fg(palette.subtext0).bg(palette.panel_bg);
    buffer.set_style(area, base);
    for x in area.x..area.right() {
        buffer[(x, area.y)].set_symbol(" ");
    }
    let mut x = area.x + 1;
    let right = area.right();
    put_text(
        buffer,
        x,
        area.y,
        right.saturating_sub(x),
        Kind::Claude.icon(),
        base.fg(claude_colour(palette)),
    );
    x += 2;
    put_text(buffer, x, area.y, right.saturating_sub(x), "Claude", base);
    x += 6 + 2;
    let (running, yours) = running_value(agent, activity);
    hits.running = draw_square(
        buffer,
        x,
        area.y,
        right,
        "Running",
        &running,
        if yours {
            palette.yellow
        } else {
            palette.subtext0
        },
        open == OpenPanel::Running,
        palette,
    );
    x = hits.running.right() + 1;
    if let Some(seats) = seats {
        let narrow = area.width < super::nirvana_seats::NARROW_STRIP;
        let (value, asks) = super::nirvana_seats::seats_value(seats, narrow);
        hits.seats = draw_square(
            buffer,
            x,
            area.y,
            right,
            "Seats",
            &value,
            if asks {
                palette.yellow
            } else {
                palette.subtext0
            },
            open == OpenPanel::Seats,
            palette,
        );
        x = hits.seats.right() + 1;
    }
    hits.preload = draw_square(
        buffer,
        x,
        area.y,
        right,
        "Preload",
        &preload_value(view),
        if preload::off_count(&view.local) > 0 {
            palette.yellow
        } else {
            palette.subtext0
        },
        open == OpenPanel::Preload,
        palette,
    );
    if let Some((value, new)) = memory {
        x = hits.preload.right() + 1;
        hits.memory = draw_square(
            buffer,
            x,
            area.y,
            right,
            "Memory",
            &value,
            if new {
                palette.yellow
            } else {
                palette.subtext0
            },
            open == OpenPanel::Memory,
            palette,
        );
    }
    hits
}

/// The open panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreloadPanel {
    pub(super) cwd: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    /// An index into the panel's items.
    pub(super) selected: usize,
    /// The first body line shown.
    pub(super) scroll: usize,
    pub(super) message: Option<String>,
}

/// Where the panel drew its parts, for the mouse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct PanelHits {
    pub(super) area: Rect,
    /// Each item row, by item index.
    pub(super) rows: Vec<(Rect, usize)>,
    /// Each item's switch box.
    pub(super) boxes: Vec<(Rect, usize)>,
    /// Each item's "edit".
    pub(super) edits: Vec<(Rect, usize)>,
    pub(super) restart: Rect,
    pub(super) close: Rect,
    pub(super) body_height: usize,
}

enum Line {
    /// A provider's header (feature 7): the owner, 2026-10-16, "Organize the list by provider or built in."
    Header(preload::Provider),
    Item(usize),
}

fn lines(items: &[Item]) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut provider = None;
    for (index, item) in items.iter().enumerate() {
        if provider.as_ref() != Some(&item.provider) {
            provider = Some(item.provider.clone());
            lines.push(Line::Header(item.provider.clone()));
        }
        lines.push(Line::Item(index));
    }
    lines
}

/// The panel's items for a view.
pub(super) fn panel_items(panel: &PreloadPanel, view: &View) -> Vec<Item> {
    let config_dir = crate::nirvana::claude_config_dir();
    preload::items(
        view.report.as_ref(),
        &view.local,
        &panel.cwd,
        config_dir.as_deref(),
        &view.remembered,
    )
}

pub(super) fn folder_name(cwd: &std::path::Path) -> String {
    cwd.file_name().map_or_else(
        || cwd.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// A panel's frame over `rect`: cleared, a rounded border, the title on its top edge and a close box.
/// Returns the close box.
pub(super) fn draw_frame(buffer: &mut Buffer, rect: Rect, title: &str, palette: &Palette) -> Rect {
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    buffer.set_style(rect, base);
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            buffer[(x, y)].set_symbol(" ");
        }
    }
    let border = base.fg(palette.overlay0);
    let (left, right, top, bottom) = (rect.x, rect.right() - 1, rect.y, rect.bottom() - 1);
    for x in left..=right {
        buffer[(x, top)].set_symbol("\u{2500}").set_style(border);
        buffer[(x, bottom)].set_symbol("\u{2500}").set_style(border);
    }
    for y in top..=bottom {
        buffer[(left, y)].set_symbol("\u{2502}").set_style(border);
        buffer[(right, y)].set_symbol("\u{2502}").set_style(border);
    }
    buffer[(left, top)].set_symbol("\u{256D}");
    buffer[(right, top)].set_symbol("\u{256E}");
    buffer[(left, bottom)].set_symbol("\u{2570}");
    buffer[(right, bottom)].set_symbol("\u{256F}");
    let inner_width = rect.width.saturating_sub(4);
    put_text(
        buffer,
        left + 2,
        top,
        inner_width.saturating_sub(4),
        &fit(title, usize::from(inner_width.saturating_sub(4))),
        base.add_modifier(Modifier::BOLD),
    );
    put_text(
        buffer,
        right - 4,
        top,
        3,
        " \u{2715} ",
        base.fg(palette.subtext0),
    );
    Rect::new(right - 4, top, 3, 1)
}

/// Which panel the strip has open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenPanel {
    None,
    Running,
    Preload,
    Memory,
    /// Plan 12 O8a.
    Seats,
}

/// The open Running panel, for one agent's pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunningPanel {
    pub(super) pane_id: String,
    pub(super) message: Option<String>,
}

/// Where the Running panel drew its parts, for the mouse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RunningHits {
    pub(super) area: Rect,
    pub(super) close: Rect,
    /// Each option of the prompt, by the digit that answers it.
    pub(super) options: Vec<(Rect, u8)>,
}

/// What the Running panel shows for one agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RunningView {
    pub(super) folder: String,
    pub(super) status: Option<AgentStatus>,
    pub(super) title: Option<String>,
    pub(super) activity: Option<Activity>,
    /// The prompt on the pane's screen, while the agent waits.
    pub(super) prompt: Option<Prompt>,
    /// Unix seconds now.
    pub(super) now: u64,
}

/// The rows of one pane as text, from the pane surface the server sent.
pub(super) fn pane_rows(surface: &PaneSurfaceFrame, pane_id: &str) -> Vec<String> {
    let Some(pane) = surface.panes.iter().find(|pane| pane.pane_id == pane_id) else {
        return Vec::new();
    };
    let rect = pane.inner_rect;
    let width = surface.frame.width as usize;
    (rect.y..rect.y.saturating_add(rect.height))
        .map(|y| {
            (rect.x..rect.x.saturating_add(rect.width))
                .filter_map(|x| surface.frame.cells.get(y as usize * width + x as usize))
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect()
}

/// Wraps `text` to lines of at most `width` cells, at most `max` lines, the last cut with `…`.
fn wrap(text: &str, width: usize, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if unicode_width::UnicodeWidthStr::width(candidate.as_str()) <= width || line.is_empty() {
            line = candidate;
        } else {
            lines.push(std::mem::take(&mut line));
            line = word.to_owned();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > max {
        lines.truncate(max);
        if let Some(last) = lines.last_mut() {
            *last = fit(&format!("{last} \u{2026}"), width);
        }
    }
    lines.into_iter().map(|line| fit(&line, width)).collect()
}

/// Draws the Running panel: Running on top, Waiting on you under it.
pub(super) fn render_running_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &RunningPanel,
    view: &RunningView,
    palette: &Palette,
) -> RunningHits {
    let mut hits = RunningHits::default();
    let panel_width = area.width.saturating_sub(2).min(PANEL_WIDTH);
    if panel_width < 30 || area.height < 8 {
        return hits;
    }
    let inner = usize::from(panel_width.saturating_sub(4));
    let label = 7;
    let text_width = inner.saturating_sub(label);
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    let muted = base.fg(palette.overlay1);
    let heading = base.fg(palette.accent).add_modifier(Modifier::BOLD);

    // Lay out the rows first, so the panel is as tall as its content.
    enum Row {
        Heading(&'static str, String, Style),
        Field(&'static str, String),
        Text(String, Style),
        Option_(u8, String),
        Blank,
    }
    let mut rows = Vec::new();
    let activity = view.activity.clone().unwrap_or_default();
    let (state, state_style) = match view.status {
        Some(AgentStatus::Working) => (
            match activity.asked_at {
                Some(at) => format!(
                    "working {}",
                    crate::nirvana_running::elapsed(view.now.saturating_sub(at))
                ),
                None => "working".into(),
            },
            base.fg(palette.green),
        ),
        Some(AgentStatus::Blocked) => ("waiting on you".into(), base.fg(palette.yellow)),
        Some(AgentStatus::Idle | AgentStatus::Done) => ("idle".into(), muted),
        _ => ("starting".into(), muted),
    };
    rows.push(Row::Heading("Running", state, state_style));
    rows.push(Row::Field(
        "Task",
        crate::nirvana_spaces::task_title(view.title.as_deref()),
    ));
    if let Some(asked) = &activity.asked {
        rows.push(Row::Field("Asked", asked.clone()));
    }
    let running = matches!(
        view.status,
        Some(AgentStatus::Working | AgentStatus::Blocked)
    );
    match activity.now.as_ref().filter(|_| running) {
        Some(tool) => {
            let since = tool
                .since
                .map(|at| {
                    format!(
                        "  {}",
                        crate::nirvana_running::elapsed(view.now.saturating_sub(at))
                    )
                })
                .unwrap_or_default();
            rows.push(Row::Field(
                "Now",
                format!("{}  {}{since}", tool.name, tool.detail),
            ));
        }
        None if running => rows.push(Row::Field("Now", "thinking".into())),
        None => rows.push(Row::Field("Now", "nothing running".into())),
    }
    if let Some(said) = &activity.said {
        for (index, line) in wrap(said, text_width, 3).into_iter().enumerate() {
            rows.push(Row::Field(if index == 0 { "Said" } else { "" }, line));
        }
    }
    rows.push(Row::Blank);
    let waiting = view.status == Some(AgentStatus::Blocked);
    rows.push(Row::Heading("Waiting on you", String::new(), base));
    match (&view.prompt, waiting) {
        (Some(prompt), true) => {
            for line in &prompt.lines {
                rows.push(Row::Text(fit(line, inner), base));
            }
            for (digit, option) in &prompt.options {
                rows.push(Row::Option_(*digit, option.clone()));
            }
        }
        (None, true) => rows.push(Row::Text(
            "Claude is waiting in its pane; its prompt has no numbered options".into(),
            muted,
        )),
        _ => rows.push(Row::Text("Nothing needs you".into(), muted)),
    }
    rows.push(Row::Blank);
    let options = view
        .prompt
        .as_ref()
        .filter(|_| waiting)
        .map_or(0, |prompt| prompt.options.len());
    let keys = if options > 0 {
        format!("1-{options} answer in the pane \u{b7} esc close")
    } else {
        "esc close".into()
    };
    rows.push(Row::Text(
        panel.message.clone().unwrap_or(keys),
        if panel.message.is_some() {
            base.fg(palette.yellow)
        } else {
            muted
        },
    ));

    let height = (rows.len() as u16 + 2).min(area.height.saturating_sub(1));
    let rect = Rect::new(area.x + 1, area.y + 1, panel_width, height);
    hits.area = rect;
    hits.close = draw_frame(
        buffer,
        rect,
        &format!(" Running \u{b7} {} ", view.folder),
        palette,
    );
    let x = rect.x + 2;
    let body = rect.height.saturating_sub(2);
    for (index, row) in rows.iter().take(usize::from(body)).enumerate() {
        let y = rect.y + 1 + index as u16;
        let width = inner as u16;
        match row {
            Row::Heading(title, right, style) => {
                put_text(buffer, x, y, width, title, heading);
                let right_width = width_of(right);
                if right_width > 0 && right_width < width {
                    put_text(
                        buffer,
                        x + width - right_width,
                        y,
                        right_width,
                        right,
                        *style,
                    );
                }
            }
            Row::Field(name, value) => {
                put_text(buffer, x, y, label as u16, name, muted);
                put_text(
                    buffer,
                    x + label as u16,
                    y,
                    width - label as u16,
                    &fit(value, text_width),
                    base,
                );
            }
            Row::Text(text, style) => {
                put_text(buffer, x, y, width, &fit(text, inner), *style);
            }
            Row::Option_(digit, option) => {
                let rect = Rect::new(x, y, width, 1);
                let key = format!(" {digit} ");
                put_text(
                    buffer,
                    x,
                    y,
                    3,
                    &key,
                    base.bg(palette.surface0).add_modifier(Modifier::BOLD),
                );
                let recommended = option.contains("(Recommended)");
                put_text(
                    buffer,
                    x + 4,
                    y,
                    width.saturating_sub(4),
                    &fit(option, inner.saturating_sub(4)),
                    if recommended {
                        base.fg(palette.green)
                    } else {
                        base
                    },
                );
                hits.options.push((rect, *digit));
            }
            Row::Blank => {}
        }
    }
    hits
}

fn width_of(text: &str) -> u16 {
    width(text)
}

/// Draws the panel over `area` (the strip's row down to the panes' bottom).
pub(super) fn render_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &mut PreloadPanel,
    view: &View,
    palette: &Palette,
) -> PanelHits {
    let mut hits = PanelHits::default();
    let panel_width = area.width.saturating_sub(2).min(PANEL_WIDTH);
    if panel_width < 30 || area.height < 8 {
        return hits;
    }
    let rect = Rect::new(area.x + 1, area.y + 1, panel_width, area.height - 1);
    hits.area = rect;
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    let muted = base.fg(palette.overlay1);
    hits.close = draw_frame(
        buffer,
        rect,
        &format!(" Preload \u{b7} {} ", folder_name(&panel.cwd)),
        palette,
    );
    let (left, top, bottom) = (rect.x, rect.y, rect.bottom() - 1);
    let inner_x = left + 2;
    let inner_width = rect.width.saturating_sub(4);

    // The summary and where the switches go.
    let summary = match &view.report {
        Some(report) => format!(
            "{} of {} preloaded \u{b7} Claude Code itself {}{}",
            preload::short(report.total),
            preload::short(report.window),
            preload::short(report.category("System prompt")),
            if view.reading { " \u{b7} reading" } else { "" }
        ),
        None if view.failed => "Claude Code did not answer /context in this folder".into(),
        None => "reading what Claude preloads here".into(),
    };
    put_text(
        buffer,
        inner_x,
        top + 1,
        inner_width,
        &fit(&summary, usize::from(inner_width)),
        base,
    );
    put_text(
        buffer,
        inner_x,
        top + 2,
        inner_width,
        &fit(
            "Off stays off for every new Claude in this folder: .claude/settings.local.json",
            usize::from(inner_width),
        ),
        muted,
    );

    let items = panel_items(panel, view);
    let lines = lines(&items);
    let body_top = top + 4;
    let body_height = usize::from(bottom.saturating_sub(1).saturating_sub(body_top));
    hits.body_height = body_height;
    panel.selected = panel.selected.min(items.len().saturating_sub(1));
    // Keep the selected row in view.
    if let Some(line) = lines
        .iter()
        .position(|line| matches!(line, Line::Item(index) if *index == panel.selected))
    {
        if line < panel.scroll {
            panel.scroll = line.saturating_sub(usize::from(
                line > 0 && matches!(lines[line - 1], Line::Header(_)),
            ));
        } else if body_height > 0 && line >= panel.scroll + body_height {
            panel.scroll = line + 1 - body_height;
        }
    }
    panel.scroll = panel.scroll.min(lines.len().saturating_sub(body_height));

    let tokens_x = inner_x + inner_width - TOKENS as u16 - 5;
    let detail_width = DETAIL.min(usize::from(inner_width) / 4) as u16;
    let detail_x = tokens_x.saturating_sub(detail_width + 1);
    let name_x = inner_x + 4;
    let name_width = detail_x.saturating_sub(name_x + 1);
    for (row, line) in lines
        .iter()
        .skip(panel.scroll)
        .take(body_height)
        .enumerate()
    {
        let y = body_top + row as u16;
        match line {
            Line::Header(provider) => {
                let style = base.fg(palette.accent).add_modifier(Modifier::BOLD);
                put_text(buffer, inner_x, y, inner_width, &provider.title(), style);
                if view.report.is_some() {
                    let tokens = preload::provider_tokens(&items, provider, view.report.as_ref());
                    if tokens > 0 {
                        let text = format!("{:>TOKENS$}", preload::short(tokens));
                        put_text(
                            buffer,
                            tokens_x,
                            y,
                            TOKENS as u16,
                            &text,
                            base.fg(palette.accent),
                        );
                    }
                }
            }
            Line::Item(index) => {
                let item = &items[*index];
                let selected = *index == panel.selected;
                let row_style = if selected {
                    base.bg(palette.selection_bg)
                } else {
                    base
                };
                let row_rect = Rect::new(inner_x, y, inner_width, 1);
                buffer.set_style(row_rect, row_style);
                hits.rows.push((row_rect, *index));
                let (mark, mark_style) = match (&item.switch, item.on) {
                    (None, _) => (" \u{b7} ", row_style.fg(palette.overlay0)),
                    (Some(_), true) => ("[x]", row_style.fg(palette.green)),
                    (Some(_), false) => ("[ ]", row_style.fg(palette.overlay1)),
                };
                put_text(buffer, inner_x, y, 3, mark, mark_style);
                if item.switch.is_some() {
                    hits.boxes.push((Rect::new(inner_x, y, 3, 1), *index));
                }
                let name_style = if item.on {
                    row_style.fg(palette.text)
                } else {
                    row_style
                        .fg(palette.overlay1)
                        .add_modifier(Modifier::CROSSED_OUT)
                };
                put_text(
                    buffer,
                    name_x,
                    y,
                    name_width,
                    &fit(&item.name, usize::from(name_width)),
                    name_style,
                );
                put_text(
                    buffer,
                    detail_x,
                    y,
                    detail_width,
                    &fit(&item.detail, usize::from(detail_width)),
                    row_style.fg(palette.overlay1),
                );
                if let Some(tokens) = item.tokens {
                    let text = format!("{:>TOKENS$}", preload::short(tokens));
                    put_text(
                        buffer,
                        tokens_x,
                        y,
                        TOKENS as u16,
                        &text,
                        row_style.fg(palette.subtext0),
                    );
                }
                if item.file.is_some() {
                    let edit_x = tokens_x + TOKENS as u16 + 1;
                    put_text(buffer, edit_x, y, 4, "edit", row_style.fg(palette.blue));
                    hits.edits.push((Rect::new(edit_x, y, 4, 1), *index));
                }
            }
        }
    }

    // The keys, then what the last action did.
    let keys = "space switch \u{b7} e edit \u{b7} r restart Claude \u{b7} esc close";
    let foot_y = bottom.saturating_sub(1);
    let message = panel.message.as_deref().unwrap_or(keys);
    put_text(
        buffer,
        inner_x,
        foot_y,
        inner_width,
        &fit(message, usize::from(inner_width)),
        if panel.message.is_some() {
            base.fg(palette.yellow)
        } else {
            muted
        },
    );
    if panel.message.is_none() {
        let start = "space switch \u{b7} e edit \u{b7} ";
        let restart = Rect::new(inner_x + width(start), foot_y, width("r restart Claude"), 1);
        if restart.right() <= inner_x + inner_width {
            hits.restart = restart;
        }
    }
    hits
}

impl ClientShellState {
    /// Whether the focused tab shows the mod strip.
    pub(super) fn nirvana_strip_shown(&self) -> bool {
        self.snapshot.as_deref().and_then(strip_agent).is_some()
    }

    /// Which panel the strip has open.
    pub(super) fn open_panel(&self) -> OpenPanel {
        if self.nirvana_running.is_some() {
            OpenPanel::Running
        } else if self.nirvana_preload.is_some() {
            OpenPanel::Preload
        } else if self.nirvana_memory.is_some() {
            OpenPanel::Memory
        } else if self.nirvana_seats.is_some() {
            OpenPanel::Seats
        } else {
            OpenPanel::None
        }
    }

    /// Opens the Running panel for the strip's agent, or closes it. One panel is open at a time.
    pub(super) fn toggle_running_panel(&mut self) {
        if self.nirvana_running.take().is_some() {
            return;
        }
        let Some(agent) = self.snapshot.as_deref().and_then(strip_agent) else {
            return;
        };
        self.nirvana_seats = None;
        self.nirvana_preload = None;
        self.nirvana_memory = None;
        self.nirvana_accounts = None;
        self.nirvana_running = Some(RunningPanel {
            pane_id: agent.pane_id,
            message: None,
        });
    }

    /// What the Running panel shows: the agent in the snapshot, its transcript, and the prompt on its
    /// pane's screen.
    pub(super) fn running_view(&self, pane_id: &str) -> RunningView {
        let mut view = RunningView {
            now: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs()),
            ..RunningView::default()
        };
        let Some(snapshot) = self.snapshot.as_deref() else {
            return view;
        };
        if let Some(agent) = snapshot
            .agents
            .iter()
            .find(|agent| agent.pane_id == pane_id)
        {
            view.status = Some(agent.agent_status);
            view.title = agent.terminal_title_stripped.clone();
            view.activity = agent
                .tokens
                .iter()
                .find(|(name, _)| name == crate::nirvana_spaces::TRANSCRIPT_TOKEN)
                .and_then(|(_, path)| crate::nirvana_running::cached(std::path::Path::new(path)));
        }
        if let Some(pane) = snapshot.panes.iter().find(|pane| pane.pane_id == pane_id) {
            view.folder = pane
                .foreground_cwd
                .as_deref()
                .or(pane.cwd.as_deref())
                .map(|cwd| folder_name(std::path::Path::new(cwd)))
                .unwrap_or_default();
        }
        if view.status == Some(AgentStatus::Blocked) {
            view.prompt = self
                .pane_surface
                .as_ref()
                .and_then(|surface| crate::nirvana_running::prompt(&pane_rows(surface, pane_id)));
        }
        view
    }

    /// Answers the waiting prompt with one digit, sent to the agent's pane as a key press: Claude Code
    /// takes a digit as the pick, with no Enter (measured 2026-10-16).
    fn running_answer(&mut self, digit: u8, outcome: &mut ClientShellInput) {
        let Some(pane_id) = self
            .nirvana_running
            .as_ref()
            .map(|panel| panel.pane_id.clone())
        else {
            return;
        };
        // `running_view` reads a prompt only while herdr says the agent is blocked.
        let answer = self.running_view(&pane_id).prompt.and_then(|prompt| {
            prompt
                .options
                .into_iter()
                .find(|(option, _)| *option == digit)
        });
        let Some((_, label)) = answer else {
            if let Some(panel) = self.nirvana_running.as_mut() {
                panel.message = Some(format!("{digit} is not an option of the waiting prompt"));
            }
            return;
        };
        // Typed into the agent's pane the way the window types any key into a pane. The API's
        // `pane.send_keys` and `agent.send_keys` are not methods a window may send; the live run of
        // 2026-10-16 got "Action unavailable" for the first.
        let key = crate::input::TerminalKey::new(
            KeyCode::Char(char::from(b'0' + digit)),
            crossterm::event::KeyModifiers::NONE,
        );
        if let Some(event) = crate::protocol::ClientPaneInputEvent::from_terminal_key(key) {
            super::push_target_event(ClientInputTarget::Pane(pane_id), event, outcome);
        }
        if let Some(panel) = self.nirvana_running.as_mut() {
            panel.message = Some(format!("answered {digit}: {label}"));
        }
    }

    /// The Running panel's keys. Every key goes to the panel while it is open.
    pub(super) fn route_running_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.nirvana_running = None,
            KeyCode::Char(c @ '1'..='9') => self.running_answer(c as u8 - b'0', outcome),
            _ => {}
        }
        outcome.repaint = true;
    }

    /// The Running panel's mouse. While it is open every event is the panel's.
    pub(super) fn handle_running_mouse(
        &mut self,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if self.nirvana_running.is_none() {
            return false;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let point = (mouse.column, mouse.row);
            let hits = self.hits.nirvana_running_panel.clone();
            if let Some(panel) = self.nirvana_running.as_mut() {
                panel.message = None;
            }
            if !super::contains(hits.area, point) || super::contains(hits.close, point) {
                self.nirvana_running = None;
            } else if let Some((_, digit)) = hits
                .options
                .iter()
                .find(|(rect, _)| super::contains(*rect, point))
            {
                self.running_answer(*digit, outcome);
            }
            outcome.repaint = true;
        }
        true
    }

    /// Opens the Preload panel for the strip's agent, or closes it.
    pub(super) fn toggle_preload_panel(&mut self) {
        if self.nirvana_preload.take().is_some() {
            return;
        }
        self.nirvana_running = None;
        self.nirvana_memory = None;
        self.nirvana_accounts = None;
        self.nirvana_seats = None;
        let Some(agent) = self.snapshot.as_deref().and_then(strip_agent) else {
            return;
        };
        preload::refresh(&agent.cwd);
        self.nirvana_preload = Some(PreloadPanel {
            cwd: agent.cwd,
            workspace_id: agent.workspace_id,
            tab_id: agent.tab_id,
            selected: 0,
            scroll: 0,
            message: None,
        });
    }

    fn preload_item(&self, index: usize) -> Option<Item> {
        let panel = self.nirvana_preload.as_ref()?;
        let view = preload::view(&panel.cwd);
        panel_items(panel, &view).into_iter().nth(index)
    }

    fn preload_say(&mut self, message: impl Into<String>) {
        if let Some(panel) = self.nirvana_preload.as_mut() {
            panel.message = Some(message.into());
        }
    }

    /// Switches the item off or on for every new Claude in the folder.
    fn preload_switch(&mut self, index: usize) {
        let Some(item) = self.preload_item(index) else {
            return;
        };
        let Some(cwd) = self.nirvana_preload.as_ref().map(|panel| panel.cwd.clone()) else {
            return;
        };
        let Some(switch) = item.switch.clone() else {
            let reason = if item.group == Group::Jev {
                "what Jev did today for every agent here; herdr jev stats lists each agent"
                    .to_owned()
            } else if item.group == Group::Skills {
                format!(
                    "{} comes with its plugin: switch the plugin's own row at the top of {}",
                    item.name,
                    item.provider.title()
                )
            } else {
                format!(
                    "Claude Code has no switch for the agent {}; e opens its file",
                    item.name
                )
            };
            self.preload_say(reason);
            return;
        };
        let message = match preload::switch_item(&cwd, &switch, !item.on) {
            // Feature 9: herdr's own switches reach more than Claude, and compact advice is a Stop
            // hook, which a running Claude picks up with no restart (measured for the board's hook).
            Ok(_) if switch == Switch::JevGuide && !item.on => {
                "Jev memory on: each new Claude, Codex and agy here gets JEV.md, in full or as a flag by this project's size (AGENTS.md for Codex and agy); r restarts this Claude".to_owned()
            }
            Ok(_) if switch == Switch::JevGuide => {
                "Jev memory off: herdr's hook and its AGENTS.md block are gone; JEV.md stays".to_owned()
            }
            Ok(_) if switch == Switch::JevCompact && !item.on => {
                "Compact advice on: from its next answer, Claude here is told when /compact is worth it".to_owned()
            }
            Ok(_) if switch == Switch::JevCompact => "Compact advice off".to_owned(),
            Ok(_) if item.on => format!(
                "{} off for new Claude sessions here; r restarts this one",
                item.name
            ),
            Ok(_) => format!(
                "{} back on for new Claude sessions here; r restarts this one",
                item.name
            ),
            Err(err) => format!("not saved: {err}"),
        };
        self.preload_say(message);
    }

    /// Opens the item's file in the editor, in a pane the server starts.
    fn preload_edit(&mut self, index: usize, outcome: &mut ClientShellInput) {
        let Some(item) = self.preload_item(index) else {
            return;
        };
        let Some(file) = item.file else {
            self.preload_say(format!("{} has no file to open", item.name));
            return;
        };
        let Some(panel) = self.nirvana_preload.as_ref() else {
            return;
        };
        let params = crate::api::schema::CommandInvokeParams {
            command_id: format!("{}{}", crate::nirvana::EDIT_FILE_COMMAND, file.display()),
            workspace_id: Some(panel.workspace_id.clone()),
            tab_id: Some(panel.tab_id.clone()),
            pane_id: None,
            selection: None,
        };
        self.push_endpoint_method(crate::api::schema::Method::CommandInvoke(params), outcome);
        self.nirvana_preload = None;
    }

    /// Restarts this Claude, same folder, same conversation (feature 2), so it takes the switches.
    fn preload_restart(&mut self, outcome: &mut ClientShellInput) {
        let Some(panel) = self.nirvana_preload.take() else {
            return;
        };
        self.push_endpoint_method(
            crate::api::schema::Method::CommandInvoke(crate::api::schema::CommandInvokeParams {
                command_id: crate::nirvana::RESTART_CLAUDE_COMMAND.into(),
                workspace_id: Some(panel.workspace_id),
                tab_id: Some(panel.tab_id),
                pane_id: None,
                selection: None,
            }),
            outcome,
        );
    }

    fn preload_count(&self) -> usize {
        self.nirvana_preload.as_ref().map_or(0, |panel| {
            panel_items(panel, &preload::view(&panel.cwd)).len()
        })
    }

    /// The panel's keys. Every key goes to the panel while it is open.
    pub(super) fn route_preload_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        let count = self.preload_count();
        let Some(panel) = self.nirvana_preload.as_mut() else {
            return;
        };
        let last = count.saturating_sub(1);
        panel.message = None;
        let selected = panel.selected;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.nirvana_preload = None,
            KeyCode::Up | KeyCode::Char('k') => panel.selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => panel.selected = (selected + 1).min(last),
            KeyCode::PageUp => panel.selected = selected.saturating_sub(PAGE),
            KeyCode::PageDown => panel.selected = (selected + PAGE).min(last),
            KeyCode::Home => panel.selected = 0,
            KeyCode::End => panel.selected = last,
            KeyCode::Char(' ') | KeyCode::Enter => self.preload_switch(selected),
            KeyCode::Char('e') => self.preload_edit(selected, outcome),
            KeyCode::Char('r') => self.preload_restart(outcome),
            _ => {}
        }
        outcome.repaint = true;
    }

    /// The panel's mouse. Returns whether the event was the panel's: while it is open every event is.
    pub(super) fn handle_preload_mouse(
        &mut self,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if self.nirvana_preload.is_none() {
            return false;
        }
        let point = (mouse.column, mouse.row);
        let hits = self.hits.nirvana_panel.clone();
        let find = |list: &[(Rect, usize)]| {
            list.iter()
                .find(|(rect, _)| super::contains(*rect, point))
                .map(|(_, index)| *index)
        };
        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                if let Some(panel) = self.nirvana_preload.as_mut() {
                    let step = 3;
                    panel.selected = if mouse.kind == MouseEventKind::ScrollUp {
                        panel.selected.saturating_sub(step)
                    } else {
                        panel.selected + step
                    };
                }
                outcome.repaint = true;
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(panel) = self.nirvana_preload.as_mut() {
                    panel.message = None;
                }
                if !super::contains(hits.area, point) || super::contains(hits.close, point) {
                    // A click on the square itself closes too: it is outside the panel.
                    self.nirvana_preload = None;
                } else if super::contains(hits.restart, point) {
                    self.preload_restart(outcome);
                } else if let Some(index) = find(&hits.edits) {
                    self.preload_edit(index, outcome);
                } else if let Some(index) = find(&hits.boxes) {
                    if let Some(panel) = self.nirvana_preload.as_mut() {
                        panel.selected = index;
                    }
                    self.preload_switch(index);
                } else if let Some(index) = find(&hits.rows) {
                    if let Some(panel) = self.nirvana_preload.as_mut() {
                        panel.selected = index;
                    }
                }
                outcome.repaint = true;
            }
            _ => {}
        }
        true
    }
}
