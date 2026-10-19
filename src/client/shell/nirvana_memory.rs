//! Nirvana fork, feature 8: the strip's third square, Memory, and its panel: the workspace's shared board,
//! which sections this Claude has not seen yet, and the switch that lets every Claude in the folder see
//! changes on its next turn. The board itself is `crate::nirvana_board`.
//! (nirvana/plans/2026-10-16-08-memory-square-and-board.md)
//!
//! The square is drawn only for a folder whose repository has a GitHub remote. The owner, 2026-10-16:
//! "flag this feature only if it opens a project with a github".

use std::path::PathBuf;

use super::nirvana_mods::{draw_frame, folder_name, StripAgent, PANEL_WIDTH};
use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crate::nirvana_board::{self as board, Board, BoardView};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::Modifier;

/// The open Memory panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoryPanel {
    pub(super) cwd: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    /// The Claude session the strip belongs to, for what it has seen.
    pub(super) session: Option<String>,
    pub(super) message: Option<String>,
}

/// Where the Memory panel drew its parts, for the mouse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct MemoryHits {
    pub(super) area: Rect,
    pub(super) close: Rect,
    pub(super) share: Rect,
    pub(super) edit: Rect,
}

/// The session of the strip's Claude: its transcript's name.
pub(super) fn strip_session(agent: &StripAgent) -> Option<String> {
    agent.transcript.as_deref().and_then(board::session_of)
}

/// The strip's value for the Memory square, or `None` when the folder gets no square.
pub(super) fn memory_value(agent: &StripAgent) -> Option<(String, bool)> {
    let view = board::view(&agent.cwd)?;
    let seen = strip_session(agent).and_then(|session| board::seen(&session));
    Some(board::square_value(&view, seen.as_ref()))
}

/// Draws the panel over `area`, as the Running and Preload panels are drawn.
pub(super) fn render_memory_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &MemoryPanel,
    view: Option<&BoardView>,
    seen: Option<&Board>,
    palette: &Palette,
) -> MemoryHits {
    let mut hits = MemoryHits::default();
    let panel_width = area.width.saturating_sub(2).min(PANEL_WIDTH);
    if panel_width < 30 || area.height < 8 {
        return hits;
    }
    let inner = usize::from(panel_width.saturating_sub(4));
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    let muted = base.fg(palette.overlay1);
    let heading = base.fg(palette.accent).add_modifier(Modifier::BOLD);

    let empty = Board::default();
    let shown = view.and_then(|view| view.board.as_ref()).unwrap_or(&empty);
    let shared = view.is_some_and(|view| view.shared);
    let new = board::unseen(shown, seen);
    let name_width = 16usize.min(inner / 4);
    let by_width = 22usize.min(inner / 4);
    let text_width = inner.saturating_sub(name_width + by_width + 4);

    // Rows first, so the panel is as tall as its content.
    let mut rows: Vec<(String, Style, Option<&str>)> = Vec::new();
    let summary = format!(
        "Board \u{b7} {}/{} \u{b7} revision {}{}",
        board::BOARD_DIR,
        board::BOARD_FILE,
        shown.revision,
        if shared && !new.is_empty() {
            format!(" \u{b7} {} new for this Claude", new.len())
        } else {
            String::new()
        }
    );
    rows.push((summary, heading, None));
    rows.push((
        format!(
            "{} Share the board with every Claude here: a prompt hook in .claude/settings.local.json",
            if shared { "[x]" } else { "[ ]" }
        ),
        if shared {
            base.fg(palette.green)
        } else {
            base
        },
        Some("share"),
    ));
    rows.push((String::new(), base, None));
    if shown.sections.is_empty() {
        rows.push((
            "Nothing on the board yet. Agents write with: herdr board write <section> --expect 0 -- <text>"
                .into(),
            muted,
            None,
        ));
    }
    for section in &shown.sections {
        let mark = if shared && new.contains(&section.name) {
            "\u{25cf}"
        } else {
            " "
        };
        let first = section.text.lines().next().unwrap_or_default();
        let more = section.text.lines().count().saturating_sub(1);
        let text = if more > 0 {
            format!("{first} (+{more} lines)")
        } else {
            first.to_owned()
        };
        let by = match (section.by.as_str(), section.at.get(11..16)) {
            ("", _) => "by hand".to_owned(),
            (by, Some(time)) => format!("{by} {time} UTC"),
            (by, None) => by.to_owned(),
        };
        rows.push((
            format!(
                "{mark} {:<name_width$} {:<text_width$} {by}",
                fit(&section.name, name_width),
                fit(&text, text_width),
            ),
            base,
            Some(if mark == " " { "section" } else { "new" }),
        ));
    }
    rows.push((String::new(), base, None));
    let keys = "space share \u{b7} e edit board.md \u{b7} esc close";
    rows.push((
        panel.message.clone().unwrap_or_else(|| keys.into()),
        if panel.message.is_some() {
            base.fg(palette.yellow)
        } else {
            muted
        },
        Some("foot"),
    ));

    let height = (rows.len() as u16 + 2).min(area.height.saturating_sub(1));
    let rect = Rect::new(area.x + 1, area.y + 1, panel_width, height);
    hits.area = rect;
    hits.close = draw_frame(
        buffer,
        rect,
        &format!(" Memory \u{b7} {} ", folder_name(&panel.cwd)),
        palette,
    );
    let x = rect.x + 2;
    let body = rect.height.saturating_sub(2);
    for (index, (text, style, role)) in rows.iter().take(usize::from(body)).enumerate() {
        let y = rect.y + 1 + index as u16;
        let row = Rect::new(x, y, inner as u16, 1);
        put_text(buffer, x, y, inner as u16, &fit(text, inner), *style);
        match role {
            Some("share") => hits.share = row,
            Some("new") => {
                put_text(buffer, x, y, 1, "\u{25cf}", base.fg(palette.yellow));
            }
            _ => {}
        }
    }
    // "e" also answers a click on the summary row.
    hits.edit = Rect::new(x, rect.y + 1, inner as u16, 1);
    hits
}

impl ClientShellState {
    /// Opens the Memory panel for the strip's agent, or closes it. One panel is open at a time.
    pub(super) fn toggle_memory_panel(&mut self) {
        if self.nirvana_memory.take().is_some() {
            return;
        }
        let Some(agent) = self
            .snapshot
            .as_deref()
            .and_then(super::nirvana_mods::strip_agent)
        else {
            return;
        };
        self.nirvana_seats = None;
        self.nirvana_jobs = None;
        self.nirvana_git = None;
        self.nirvana_preload = None;
        self.nirvana_running = None;
        self.nirvana_accounts = None;
        self.nirvana_memory = Some(MemoryPanel {
            session: strip_session(&agent),
            cwd: agent.cwd,
            workspace_id: agent.workspace_id,
            tab_id: agent.tab_id,
            message: None,
        });
    }

    fn memory_say(&mut self, message: impl Into<String>) {
        if let Some(panel) = self.nirvana_memory.as_mut() {
            panel.message = Some(message.into());
        }
    }

    /// Adds or removes the prompt hook that tells each Claude in the folder what changed on the board.
    fn memory_share(&mut self) {
        let Some(cwd) = self.nirvana_memory.as_ref().map(|panel| panel.cwd.clone()) else {
            return;
        };
        let on = !board::view(&cwd).is_some_and(|view| view.shared);
        let message = match board::share(&cwd, on) {
            Ok(()) if on => {
                "shared: from its next prompt, each Claude here is told what changed on the board"
            }
            Ok(()) => "not shared: Claudes here no longer hear of board changes",
            Err(err) => {
                self.memory_say(format!("not saved: {err}"));
                return;
            }
        };
        self.memory_say(message);
    }

    /// Opens `board.md` in the terminal editor tab (feature 7), creating an empty board first.
    fn memory_edit(&mut self, outcome: &mut ClientShellInput) {
        let Some(panel) = self.nirvana_memory.clone() else {
            return;
        };
        let Some(root) = board::view(&panel.cwd).map(|view| view.root) else {
            return;
        };
        let file = match board::ensure(&root) {
            Ok(file) => file,
            Err(err) => {
                self.memory_say(format!("cannot create the board: {err}"));
                return;
            }
        };
        board::forget(&panel.cwd);
        let params = crate::api::schema::CommandInvokeParams {
            command_id: format!("{}{}", crate::nirvana::EDIT_FILE_COMMAND, file.display()),
            workspace_id: Some(panel.workspace_id),
            tab_id: Some(panel.tab_id),
            pane_id: None,
            selection: None,
        };
        self.push_endpoint_method(crate::api::schema::Method::CommandInvoke(params), outcome);
        self.nirvana_memory = None;
    }

    /// The Memory panel's keys. Every key goes to the panel while it is open.
    pub(super) fn route_memory_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        if let Some(panel) = self.nirvana_memory.as_mut() {
            panel.message = None;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.nirvana_memory = None,
            KeyCode::Char(' ') | KeyCode::Enter => self.memory_share(),
            KeyCode::Char('e') => self.memory_edit(outcome),
            _ => {}
        }
        outcome.repaint = true;
    }

    /// The Memory panel's mouse. While it is open every event is the panel's.
    pub(super) fn handle_memory_mouse(
        &mut self,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if self.nirvana_memory.is_none() {
            return false;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let point = (mouse.column, mouse.row);
            let hits = self.hits.nirvana_memory_panel.clone();
            if let Some(panel) = self.nirvana_memory.as_mut() {
                panel.message = None;
            }
            if !super::contains(hits.area, point) || super::contains(hits.close, point) {
                self.nirvana_memory = None;
            } else if super::contains(hits.share, point) {
                self.memory_share();
            } else if super::contains(hits.edit, point) {
                self.memory_edit(outcome);
            }
            outcome.repaint = true;
        }
        true
    }
}
