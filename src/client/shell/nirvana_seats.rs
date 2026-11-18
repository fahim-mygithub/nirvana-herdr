//! Nirvana fork, plan 12 O8a: the Seats square in the mod strip of a Nirvana tab, and its panel. Each open order
//! with its seat, model and effort, pool, state, time and use, and its reason line; a click on an order focuses
//! its worker's pane; the mode in the header, `m` to change it. The data is `crate::nirvana_seats_square`.
//! (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §8)
//!
//! ```text
//!  ✻ Claude   Running idle ▾   Seats 2 open · Balanced ▾   Preload 23.6k ▾
//! ```
//!
//! A Nirvana tab is one with a herdr agent named as O4a's launch names Nirvana Claude (`nirvana`, `nirvana-2`):
//! a plain Claude Code tab gets no Seats square (L7: "sometimes I just want Claude Code").

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::nirvana_mods::{draw_frame, folder_name};
use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crate::api::schema::AgentStatus;
use crate::nirvana_seat::Mode;
use crate::nirvana_seats_square::{self as square, Row as SeatRow, View as SeatsView};
use crate::protocol::ClientShellAgent;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::Modifier;

/// Under this strip width the square takes its narrow form, "2 · Bal", so Preload and Memory still fit.
pub(super) const NARROW_STRIP: u16 = 100;
/// The panel is wider than the strip's other panels when the window allows: one row of seven columns.
pub(super) const SEATS_WIDTH: u16 = 110;
/// Under this inner width the panel gives each order three lines instead of a row of columns.
pub(super) const NARROW_PANEL: usize = 106;
/// The wide panel's columns, with a space between each: 106 cells with use's.
const ORDER: usize = 12;
const SEAT: usize = 16;
const MODEL: usize = 24;
const POOL: usize = 8;
const STATE: usize = 11;
const TIME: usize = 12;

/// The focused tab's Nirvana Claude and its project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SeatsTab {
    pub(super) root: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    /// Nirvana Claude's own pane: the restart targets it alone, never a worker sharing its tab.
    pub(super) pane_id: String,
}

fn is_nirvana(agent: &ClientShellAgent) -> bool {
    agent
        .name
        .as_deref()
        .is_some_and(crate::nirvana_launch::is_nirvana_agent_name)
}

/// One Nirvana Claude's tab, pane and project root, from its pane's folder.
fn seats_tab_of(snapshot: &ClientShellSnapshot, agent: &ClientShellAgent) -> Option<SeatsTab> {
    let pane = snapshot
        .panes
        .iter()
        .find(|pane| pane.pane_id == agent.pane_id)?;
    let cwd = pane
        .foreground_cwd
        .clone()
        .or_else(|| pane.cwd.clone())
        .filter(|cwd| !cwd.is_empty())?;
    Some(SeatsTab {
        root: square::project_root(Path::new(&cwd)),
        workspace_id: agent.workspace_id.clone(),
        tab_id: agent.tab_id.clone(),
        pane_id: agent.pane_id.clone(),
    })
}

/// The focused tab's Seats square, when it is a Nirvana tab.
pub(super) fn seats_tab(snapshot: &ClientShellSnapshot) -> Option<SeatsTab> {
    let tab_id = snapshot.focused_tab_id.as_deref()?;
    let agent = snapshot
        .agents
        .iter()
        .find(|agent| agent.tab_id == tab_id && is_nirvana(agent))?;
    seats_tab_of(snapshot, agent)
}

/// The owner, 2026-11-18: "if your right click the project an option appears to restart that specific project."
/// The space's Nirvana Claude, the first in the snapshot's order, whatever tab is focused; and whether the Seats
/// square reads that it runs a seat the mode no longer gives (`restart to switch`), from the cached view only.
pub(super) fn space_restart_target(
    snapshot: &ClientShellSnapshot,
    workspace_id: &str,
) -> Option<ClientRestartTarget> {
    let agent = snapshot
        .agents
        .iter()
        .find(|agent| agent.workspace_id == workspace_id && is_nirvana(agent))?;
    let switch = seats_tab_of(snapshot, agent)
        .and_then(|tab| square::cached(&tab.root))
        .and_then(|view| view.orchestrator)
        .is_some_and(|orch| orch.restart_to_switch);
    Some(ClientRestartTarget {
        tab_id: agent.tab_id.clone(),
        pane_id: agent.pane_id.clone(),
        working: restart_cuts_a_turn(agent.agent_status),
        switch,
        armed: None,
        restarting: false,
    })
}

/// P87 fix round (review of aa340405, item b2): an armed press lapses, so a stray `r` or click minutes later does not
/// cut a turn the owner was not asked about again. Five seconds is the brief's number, not a measured one.
pub(super) const RESTART_ARM_WINDOW: Duration = Duration::from_secs(5);
/// P87 fix round (item b1): how long a sent restart blocks a second one when the view does not show it has landed.
/// The row clears on the fresh Claude's SessionStart (`herdr nirvana digest` rewrites its seat record) plus a
/// `REREAD` (2 s); 15 s is the brief's number, and after it the row asks again in case the restart failed.
pub(super) const RESTART_PENDING_WINDOW: Duration = Duration::from_secs(15);

/// Whether a press armed at `at` still holds.
pub(super) fn armed_now(at: Option<Instant>) -> bool {
    at.is_some_and(|at| at.elapsed() < RESTART_ARM_WINDOW)
}

/// Whether a restart sent as `sent` has still not shown, with the view now asking for one (`asks_now`) or not. One
/// that was sent while the view asked has shown once the view stops asking; one sent without the ask (a space menu's
/// plain Restart Claude) holds for the whole window.
pub(super) fn restart_live(sent: Option<ClientRestartSent>, asks_now: bool) -> bool {
    sent.is_some_and(|sent| {
        sent.at.elapsed() < RESTART_PENDING_WINDOW && (asks_now || !sent.asked)
    })
}

/// Whether a restart now would cut Nirvana Claude's turn, so it waits for a second press, as the header's reload
/// badge does. Only an idle or finished Claude restarts at once: Restart Claude stops Claude and its children
/// (`nirvana_restart_claude_in_pane`), so a working one loses its turn and a blocked one its open prompt.
pub(super) fn restart_cuts_a_turn(status: AgentStatus) -> bool {
    !matches!(status, AgentStatus::Idle | AgentStatus::Done)
}

/// The mode's word as the strip writes it.
pub(super) fn mode_label(mode: Mode, narrow: bool) -> &'static str {
    if narrow {
        mode_short(mode)
    } else {
        mode.title()
    }
}

fn mode_short(mode: Mode) -> &'static str {
    match mode {
        Mode::Efficient => "Eff",
        Mode::Balanced => "Bal",
        Mode::Sota => "SOTA",
    }
}

/// The square's value, and whether an order waits on someone: "2 open · Balanced", "1 open · SOTA · 1 asks";
/// narrow, "2 · Bal". Before the first read, "reading".
pub(super) fn seats_value(view: Option<&SeatsView>, narrow: bool) -> (String, bool) {
    let Some(view) = view else {
        return ("reading".into(), false);
    };
    let open = view.rows.len();
    let asks = view.attention();
    let mode = match (view.mode, narrow) {
        (Some(mode), false) => mode.title(),
        (Some(mode), true) => mode_short(mode),
        (None, _) => "mode ?",
    };
    let mut value = if narrow {
        format!("{open} \u{b7} {mode}")
    } else {
        format!("{open} open \u{b7} {mode}")
    };
    if asks > 0 {
        value.push_str(&if narrow {
            format!(" \u{b7} {asks}!")
        } else {
            format!(" \u{b7} {asks} asks")
        });
    }
    (value, asks > 0)
}

/// The open Seats panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SeatsPanel {
    pub(super) root: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    /// Nirvana Claude's pane in that tab: `r` and the row's `restart to switch` restart it.
    pub(super) pane_id: String,
    /// A first press on a working Nirvana Claude arms the restart; the second restarts it, any other key or click
    /// puts it back (the header's reload badge, `crate::nirvana::press_reload`), and it lapses after
    /// `RESTART_ARM_WINDOW`.
    pub(super) restart_armed: Option<Instant>,
    /// The restart this panel's pane was last sent, from here or a space's menu: while `restart_live`, the words read
    /// `restarting…` and no second one is sent.
    pub(super) restarting: Option<ClientRestartSent>,
    /// An index into the view's rows.
    pub(super) selected: usize,
    /// The first body line shown.
    pub(super) scroll: usize,
    pub(super) message: Option<String>,
}

/// Where the Seats panel drew its parts, for the mouse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SeatsHits {
    pub(super) area: Rect,
    pub(super) close: Rect,
    /// The mode row; a click moves the mode as `m` does.
    pub(super) mode: Rect,
    /// The orchestrator row's `restart to switch` words, when drawn; a click restarts as `r` does.
    pub(super) restart: Rect,
    /// Each line of each order, with the order's index: a click focuses its worker's pane.
    pub(super) orders: Vec<(Rect, usize)>,
}

/// One line of the panel's body.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Mode,
    /// Nirvana Claude's row, and whether its running seat differs from the mode's (`restart to switch`).
    Orchestrator(String, bool),
    Text(String, Tone),
    Columns,
    /// An order's line, by its index.
    Order(usize, String, Tone),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Plain,
    Muted,
    Heading,
    Attention,
}

fn column(text: &str, width: usize) -> String {
    let text = fit(text, width);
    let pad = width.saturating_sub(unicode_width::UnicodeWidthStr::width(text.as_str()));
    format!("{text}{}", " ".repeat(pad))
}

fn run_line(run: &square::RunLine) -> String {
    let classes: Vec<String> = run
        .classes
        .iter()
        .map(|(class, count)| format!("{class} {count}"))
        .collect();
    let mut line = format!(
        "run {} \u{b7} {} order{} \u{b7} {}",
        run.run,
        run.orders,
        if run.orders == 1 { "" } else { "s" },
        if classes.is_empty() {
            "no class yet".to_owned()
        } else {
            classes.join(", ")
        }
    );
    if run.overrides > 0 {
        line.push_str(&format!(" \u{b7} {} overrides", run.overrides));
    }
    if let Some((at, mode)) = run.mode_changed {
        let at = crate::nirvana_run::utc(at);
        line.push_str(&format!(
            " \u{b7} mode \u{2192} {} {}Z",
            mode.title(),
            at.get(11..16).unwrap_or(&at)
        ));
    }
    line
}

/// The reason line under an order, with its override and the mode it was seated (or booked) at when that is not
/// today's: a mode change seats the next order, not the ones already seated.
fn reason_line(row: &SeatRow, mode: Option<Mode>) -> String {
    let mut parts = Vec::new();
    if let Some(seated) = row.mode.filter(|seated| Some(*seated) != mode) {
        let verb = if row.state == "booked" {
            "booked"
        } else {
            "seated"
        };
        parts.push(format!("{verb} at {}", seated.title()));
    }
    if let Some(over) = &row.override_reason {
        parts.push(format!("override: {over}"));
    }
    parts.push(if row.reason.is_empty() {
        "no reason line recorded".to_owned()
    } else {
        row.reason.clone()
    });
    format!("\u{21b3} {}", parts.join(" \u{b7} "))
}

/// The orchestrator row's button, and its armed form.
pub(super) const RESTART_WORDS: &str = "restart to switch";
pub(super) const RESTART_ARMED_WORDS: &str = "restart? r again";
pub(super) const RESTARTING_WORDS: &str = "restarting\u{2026}";
/// The footer while a restart is armed, for as long as it is (it used to go with the next key's `message = None`).
pub(super) const RESTART_ARMED_PROMPT: &str =
    "Nirvana Claude is working: r again restarts it now and cuts its turn";

/// The panel's lines for a view at `inner` cells: three lines an order when narrow, two when wide.
fn lines(view: &SeatsView, inner: usize, now: u64) -> Vec<Line> {
    let mut lines = vec![Line::Mode];
    if let Some(note) = &view.note {
        lines.push(Line::Text(note.clone(), Tone::Attention));
    }
    // Review M2: an edited seats.toml never takes a changed default, so the square says so.
    if view.differs {
        lines.push(Line::Text(
            crate::nirvana_seat::DIFFERS_LINE.to_owned(),
            Tone::Muted,
        ));
    }
    if let Some(orch) = &view.orchestrator {
        let text = format!(
            "Nirvana Claude \u{b7} {} {} \u{b7} {}",
            orch.seat,
            orch.effort.word(),
            orch.detail
        );
        lines.push(Line::Orchestrator(text, orch.restart_to_switch));
    }
    if view.rows.is_empty() {
        lines.push(Line::Text(String::new(), Tone::Plain));
        lines.push(Line::Text(
            "No open order in this project's runs".into(),
            Tone::Muted,
        ));
        lines.push(Line::Text(
            "An order seated with herdr seat run shows here until the gate rules on it".into(),
            Tone::Muted,
        ));
    }
    let narrow = inner < NARROW_PANEL;
    for run in &view.runs {
        lines.push(Line::Text(String::new(), Tone::Plain));
        lines.push(Line::Text(run_line(run), Tone::Heading));
        if !narrow && view.rows.iter().any(|row| row.run == run.run) {
            lines.push(Line::Columns);
        }
        for (index, row) in view
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.run == run.run)
        {
            let tone = if row.attention {
                Tone::Attention
            } else {
                Tone::Plain
            };
            let seat = format!("{} {}", row.seat, row.effort.word());
            let time = square::time_text(row, now);
            let use_ = square::use_text(row.use_.as_ref());
            if narrow {
                lines.push(Line::Order(
                    index,
                    format!("{} \u{b7} {} {time}", row.order, row.state),
                    tone,
                ));
                lines.push(Line::Order(
                    index,
                    format!(
                        "  {} {} \u{b7} {} \u{b7} {} \u{b7} {use_}",
                        row.seat,
                        row.effort.word(),
                        row.model,
                        row.pool
                    ),
                    Tone::Plain,
                ));
            } else {
                lines.push(Line::Order(
                    index,
                    format!(
                        "{} {} {} {} {} {} {use_}",
                        column(&row.order, ORDER),
                        column(&seat, SEAT),
                        column(&row.model, MODEL),
                        column(&row.pool, POOL),
                        column(&row.state, STATE),
                        column(&time, TIME),
                    ),
                    tone,
                ));
            }
            lines.push(Line::Order(
                index,
                format!("  {}", reason_line(row, view.mode)),
                Tone::Muted,
            ));
        }
    }
    lines
}

/// Draws the Seats panel over `area`, as tall as its content, as the Running and Memory panels are drawn.
pub(super) fn render_seats_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &mut SeatsPanel,
    view: Option<&SeatsView>,
    now: u64,
    palette: &Palette,
) -> SeatsHits {
    let mut hits = SeatsHits::default();
    let panel_width = area.width.saturating_sub(2).min(SEATS_WIDTH);
    if panel_width < 30 || area.height < 8 {
        return hits;
    }
    let inner = usize::from(panel_width.saturating_sub(4));
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    let muted = base.fg(palette.overlay1);
    let heading = base.fg(palette.accent).add_modifier(Modifier::BOLD);
    let empty = SeatsView::default();
    let shown = view.unwrap_or(&empty);
    let body = match view {
        Some(view) => lines(view, inner, now),
        None => vec![
            Line::Mode,
            Line::Text("reading this project's runs".into(), Tone::Muted),
        ],
    };
    panel.selected = panel.selected.min(shown.rows.len().saturating_sub(1));

    // The frame holds the body and two more lines: a blank and the keys.
    let height = (body.len() as u16 + 4).min(area.height.saturating_sub(1));
    let rect = Rect::new(area.x + 1, area.y + 1, panel_width, height);
    hits.area = rect;
    let title = match shown.mode {
        Some(mode) => format!(
            " Seats \u{b7} {} \u{b7} {} ",
            folder_name(&panel.root),
            mode.title()
        ),
        None => format!(" Seats \u{b7} {} ", folder_name(&panel.root)),
    };
    hits.close = draw_frame(buffer, rect, &title, palette);
    if shown.mode == Some(Mode::Sota) {
        let top = rect.y;
        for col in rect.x..rect.right().saturating_sub(3) {
            if buffer[(col, top)].symbol() == "S"
                && buffer[(col + 1, top)].symbol() == "O"
                && buffer[(col + 2, top)].symbol() == "T"
                && buffer[(col + 3, top)].symbol() == "A"
            {
                buffer[(col, top)].set_fg(palette.red);
                buffer[(col + 1, top)].set_fg(palette.red);
                buffer[(col + 2, top)].set_fg(palette.red);
                buffer[(col + 3, top)].set_fg(palette.red);
                break;
            }
        }
    }
    let x = rect.x + 2;
    let body_height = usize::from(rect.height.saturating_sub(4));
    // Keep the selected order's first line in view.
    if let Some(first) = body
        .iter()
        .position(|line| matches!(line, Line::Order(index, ..) if *index == panel.selected))
    {
        if first < panel.scroll {
            panel.scroll = first;
        } else if body_height > 0 && first >= panel.scroll + body_height {
            panel.scroll = first + 1 - body_height;
        }
    }
    panel.scroll = panel.scroll.min(body.len().saturating_sub(body_height));
    for (row, line) in body.iter().skip(panel.scroll).take(body_height).enumerate() {
        let y = rect.y + 1 + row as u16;
        let width = inner as u16;
        match line {
            Line::Mode => {
                hits.mode = Rect::new(x, y, width, 1);
                put_text(buffer, x, y, 6, "Mode", muted);
                let mut at = x + 6;
                for mode in [Mode::Efficient, Mode::Balanced, Mode::Sota] {
                    let on = shown.mode == Some(mode);
                    let text = if on {
                        format!("[{}]", mode.title())
                    } else {
                        format!(" {} ", mode.title())
                    };
                    let style = if on {
                        let color = if mode == Mode::Sota {
                            palette.red
                        } else {
                            palette.green
                        };
                        base.fg(color).add_modifier(Modifier::BOLD)
                    } else {
                        muted
                    };
                    put_text(buffer, at, y, (x + width).saturating_sub(at), &text, style);
                    at += unicode_width::UnicodeWidthStr::width(text.as_str()) as u16 + 1;
                }
                if let Some(source) = shown.mode_source {
                    let source_text = format!("\u{b7} {}", source.word());
                    put_text(
                        buffer,
                        at,
                        y,
                        (x + width).saturating_sub(at),
                        &source_text,
                        muted,
                    );
                    at += unicode_width::UnicodeWidthStr::width(source_text.as_str()) as u16 + 1;
                }
                let key = "m changes it";
                let key_x = (x + width).saturating_sub(key.len() as u16);
                if key_x > at {
                    put_text(buffer, key_x, y, key.len() as u16, key, muted);
                }
            }
            Line::Columns => {
                let text = format!(
                    "{} {} {} {} {} {} use",
                    column("order", ORDER),
                    column("seat effort", SEAT),
                    column("model", MODEL),
                    column("pool", POOL),
                    column("state", STATE),
                    column("time", TIME),
                );
                put_text(buffer, x, y, width, &fit(&text, inner), muted);
            }
            Line::Orchestrator(text, switch) => {
                if !*switch {
                    put_text(buffer, x, y, width, &fit(text, inner), base);
                    continue;
                }
                // The owner, 2026-11-18: "We should be able to click restart in this modal". The words are the
                // button; armed, they ask for the second press; sent, they say so and are no button.
                let attention = base.fg(palette.yellow);
                let restarting = restart_live(panel.restarting, true);
                let words = if restarting {
                    RESTARTING_WORDS
                } else if armed_now(panel.restart_armed) {
                    RESTART_ARMED_WORDS
                } else {
                    RESTART_WORDS
                };
                let lead = format!("{text} \u{b7} ");
                let lead_width = unicode_width::UnicodeWidthStr::width(lead.as_str());
                put_text(buffer, x, y, width, &fit(&lead, inner), attention);
                if lead_width < inner {
                    let at = x + lead_width as u16;
                    let room = inner - lead_width;
                    let shown = fit(words, room);
                    let shown_width = unicode_width::UnicodeWidthStr::width(shown.as_str()) as u16;
                    let style = if restarting {
                        attention
                    } else {
                        attention.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    };
                    put_text(buffer, at, y, room as u16, &shown, style);
                    // A button only when its words are whole: a cut `rest…` is not one to click; `r` still works.
                    let whole = unicode_width::UnicodeWidthStr::width(words);
                    if !restarting && usize::from(shown_width) == whole {
                        hits.restart = Rect::new(at, y, shown_width, 1);
                    }
                }
            }
            Line::Text(text, tone) | Line::Order(_, text, tone) => {
                let mut style = match tone {
                    Tone::Plain => base,
                    Tone::Muted => muted,
                    Tone::Heading => heading,
                    Tone::Attention => base.fg(palette.yellow),
                };
                if let Line::Order(index, ..) = line {
                    let row_rect = Rect::new(x, y, width, 1);
                    if *index == panel.selected {
                        style = style.bg(palette.selection_bg);
                        buffer.set_style(row_rect, Style::default().bg(palette.selection_bg));
                    }
                    hits.orders.push((row_rect, *index));
                }
                put_text(buffer, x, y, width, &fit(text, inner), style);
            }
        }
    }
    let foot_y = rect.bottom().saturating_sub(2);
    let switch = shown
        .orchestrator
        .as_ref()
        .is_some_and(|orch| orch.restart_to_switch);
    // A lapsed press and a restart that has shown are dropped here, where the view is at hand.
    if panel.restart_armed.is_some() && !armed_now(panel.restart_armed) {
        panel.restart_armed = None;
        if panel.message.as_deref() == Some(RESTART_ARMED_PROMPT) {
            panel.message = None;
        }
    }
    if view.is_some() && !restart_live(panel.restarting, switch) {
        panel.restarting = None;
    }
    let armed = panel.restart_armed.is_some() && switch;
    let keys = match (shown.rows.is_empty(), switch) {
        (true, false) => "m mode \u{b7} esc close",
        (true, true) => "r restart \u{b7} m mode \u{b7} esc close",
        (false, false) => {
            "\u{2191}/\u{2193} choose \u{b7} enter focus its pane \u{b7} m mode \u{b7} esc close"
        }
        (false, true) => {
            "\u{2191}/\u{2193} choose \u{b7} enter focus its pane \u{b7} r restart \u{b7} m mode \u{b7} esc close"
        }
    };
    put_text(
        buffer,
        x,
        foot_y,
        inner as u16,
        &fit(
            if armed {
                RESTART_ARMED_PROMPT
            } else {
                panel.message.as_deref().unwrap_or(keys)
            },
            inner,
        ),
        if armed || panel.message.is_some() {
            base.fg(palette.yellow)
        } else {
            muted
        },
    );
    hits
}

impl ClientShellState {
    /// Opens the Seats panel for the focused Nirvana tab, or closes it. One panel is open at a time.
    pub(super) fn toggle_seats_panel(&mut self) {
        if self.nirvana_seats.take().is_some() {
            return;
        }
        let Some(tab) = self.snapshot.as_deref().and_then(seats_tab) else {
            return;
        };
        // A restart sent from the space's menu a moment ago holds here too.
        let restarting = self.nirvana_restarting.get(&tab.pane_id).copied();
        self.nirvana_learning = None;
        self.nirvana_preload = None;
        self.nirvana_running = None;
        self.nirvana_memory = None;
        self.nirvana_accounts = None;
        self.nirvana_jobs = None;
        self.nirvana_git = None;
        self.nirvana_seats = Some(SeatsPanel {
            root: tab.root,
            workspace_id: tab.workspace_id,
            tab_id: tab.tab_id,
            pane_id: tab.pane_id,
            restart_armed: None,
            restarting,
            selected: 0,
            scroll: 0,
            message: None,
        });
    }

    fn seats_say(&mut self, message: impl Into<String>) {
        if let Some(panel) = self.nirvana_seats.as_mut() {
            panel.message = Some(message.into());
        }
    }

    fn seats_view(&self) -> Option<SeatsView> {
        self.nirvana_seats
            .as_ref()
            .and_then(|panel| square::cached(&panel.root))
    }

    /// Moves the mode to the next stop, in `seats.toml`, and logs it in the project's open runs.
    fn seats_mode(&mut self) {
        let Some(root) = self.nirvana_seats.as_ref().map(|panel| panel.root.clone()) else {
            return;
        };
        let Some(mode) = self.seats_view().and_then(|view| view.mode) else {
            self.seats_say("the mode is not known: seats.toml is refused or not read yet");
            return;
        };
        let message = square::change_mode(&root, mode.next(), "seats square")
            .unwrap_or_else(|err| format!("not changed: {err}"));
        self.seats_say(message);
    }

    /// Focuses the worker's pane of the order at `index`: the panel closes, so the pane is what the owner sees.
    fn seats_focus(&mut self, index: usize, outcome: &mut ClientShellInput) {
        let Some(row) = self
            .seats_view()
            .and_then(|view| view.rows.into_iter().nth(index))
        else {
            return;
        };
        let open = |pane: &str| {
            self.snapshot
                .as_deref()
                .is_some_and(|snapshot| snapshot.panes.iter().any(|known| known.pane_id == pane))
        };
        match &row.pane {
            Some(pane) if open(pane) => {
                self.push_endpoint_method(
                    crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
                        pane_id: pane.clone(),
                    }),
                    outcome,
                );
                self.nirvana_seats = None;
            }
            Some(pane) => self.seats_say(format!(
                "{}'s pane {pane} is not open: herdr seat read {} shows its stream",
                row.order, row.order
            )),
            None if row.state == "booked" => self.seats_say(format!(
                "{} is booked, not started: no worker pane yet",
                row.order
            )),
            None => self.seats_say(format!(
                "{} has no pane: it runs detached; herdr seat read {} shows its stream",
                row.order, row.order
            )),
        }
    }

    /// The owner, 2026-11-18: "when you switch the default orchistrator and it askes to restart to fix ... We
    /// should be able to click restart in this modal". Runs the tab menu's Restart Claude on this panel's Nirvana
    /// Claude pane alone. Only while the row says `restart to switch`; a working Claude waits for a second press.
    fn seats_restart(&mut self, outcome: &mut ClientShellInput) {
        let Some(panel) = self.nirvana_seats.as_ref() else {
            return;
        };
        let (workspace_id, tab_id, pane_id, armed, restarting) = (
            panel.workspace_id.clone(),
            panel.tab_id.clone(),
            panel.pane_id.clone(),
            armed_now(panel.restart_armed),
            panel.restarting,
        );
        let Some(orch) = self.seats_view().and_then(|view| view.orchestrator) else {
            self.seats_disarm();
            self.seats_say("the mode's seat is not read yet: nothing to switch to");
            return;
        };
        if !orch.restart_to_switch {
            self.seats_disarm();
            self.seats_say("Nirvana Claude already runs the mode's seat: no restart needed");
            return;
        }
        // Review of aa340405, item b1: the row asks until the fresh Claude's seat record is read, so a second press
        // in that window restarted the fresh Claude again.
        if restart_live(restarting, true) {
            self.seats_say(
                "Nirvana Claude is restarting: the row clears once its new seat is recorded",
            );
            return;
        }
        let Some(status) = self.snapshot.as_deref().and_then(|snapshot| {
            snapshot
                .agents
                .iter()
                .find(|agent| agent.pane_id == pane_id)
                .map(|agent| agent.agent_status)
        }) else {
            self.seats_say("this tab's Nirvana Claude pane is not open");
            return;
        };
        if restart_cuts_a_turn(status) && !armed {
            if let Some(panel) = self.nirvana_seats.as_mut() {
                panel.restart_armed = Some(Instant::now());
            }
            self.seats_say(RESTART_ARMED_PROMPT);
            return;
        }
        self.push_restart_claude(workspace_id, tab_id, Some(pane_id.clone()), outcome);
        let sent = ClientRestartSent {
            at: Instant::now(),
            asked: true,
        };
        self.nirvana_restarting.insert(pane_id, sent);
        if let Some(panel) = self.nirvana_seats.as_mut() {
            panel.restart_armed = None;
            panel.restarting = Some(sent);
        }
        self.seats_say(format!(
            "restarting Nirvana Claude on {} {}, same conversation",
            orch.seat,
            orch.effort.word()
        ));
    }

    fn seats_disarm(&mut self) {
        if let Some(panel) = self.nirvana_seats.as_mut() {
            panel.restart_armed = None;
        }
    }

    fn seats_count(&self) -> usize {
        self.seats_view().map_or(0, |view| view.rows.len())
    }

    /// The Seats panel's keys. Every key goes to the panel while it is open.
    pub(super) fn route_seats_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        let last = self.seats_count().saturating_sub(1);
        let Some(panel) = self.nirvana_seats.as_mut() else {
            return;
        };
        panel.message = None;
        let selected = panel.selected;
        // Any key but `r` puts an armed restart back, as any press does the reload badge's.
        if key.code != KeyCode::Char('r') {
            panel.restart_armed = None;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.nirvana_seats = None,
            KeyCode::Up | KeyCode::Char('k') => panel.selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => panel.selected = (selected + 1).min(last),
            KeyCode::Home => panel.selected = 0,
            KeyCode::End => panel.selected = last,
            KeyCode::Char('m') => self.seats_mode(),
            KeyCode::Char('r') => self.seats_restart(outcome),
            KeyCode::Enter => self.seats_focus(selected, outcome),
            _ => {}
        }
        outcome.repaint = true;
    }

    /// The Seats panel's mouse. While it is open every event is the panel's.
    pub(super) fn handle_seats_mouse(
        &mut self,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if self.nirvana_seats.is_none() {
            return false;
        }
        let point = (mouse.column, mouse.row);
        let hits = self.hits.nirvana_seats_panel.clone();
        let on_restart = super::contains(hits.restart, point);
        // Review of aa340405, item b2: a wheel or another button puts an armed restart back too. A move, a drag or a
        // release does not: the pointer travels to the words, and a click's own release follows its press.
        let keeps_arm = matches!(
            mouse.kind,
            MouseEventKind::Moved | MouseEventKind::Drag(_) | MouseEventKind::Up(_)
        ) || (mouse.kind == MouseEventKind::Down(MouseButton::Left) && on_restart);
        if !keeps_arm {
            self.seats_disarm();
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            if let Some(panel) = self.nirvana_seats.as_mut() {
                panel.message = None;
            }
            if !super::contains(hits.area, point) || super::contains(hits.close, point) {
                self.nirvana_seats = None;
            } else if on_restart {
                self.seats_restart(outcome);
            } else if super::contains(hits.mode, point) {
                self.seats_mode();
            } else if let Some((_, index)) = hits
                .orders
                .iter()
                .find(|(rect, _)| super::contains(*rect, point))
            {
                let index = *index;
                if let Some(panel) = self.nirvana_seats.as_mut() {
                    panel.selected = index;
                }
                self.seats_focus(index, outcome);
            }
            outcome.repaint = true;
        }
        true
    }
}
