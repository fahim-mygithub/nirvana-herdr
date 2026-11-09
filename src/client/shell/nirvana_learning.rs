//! Nirvana fork, plan 36: the Learning square in the mod strip, and its panel.
//! (nirvana/plans/2026-10-31-36-learning-square.md §1)
//!
//! The square reads `14 lessons · review 3/20`, with a mark when the last review gated today went wrong. The panel
//! lists Today, Recent changes, Lessons, Profiles and Waiting from `crate::nirvana_learning::cached`; `u` on a
//! change asks in the panel, then runs the rollback `herdr lessons rollback` / `herdr kb rollback` runs.

use std::path::PathBuf;

use super::nirvana_mods::{draw_frame, folder_name};
use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crate::nirvana_learning::{Change, LearningView, Ruled, Source};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::Modifier;

pub(super) const NARROW_STRIP: u16 = 100;
pub(super) const LEARNING_WIDTH: u16 = 96;
/// The square's mark: the Git square's failed-CI mark, so a wrong turn reads the same in both.
pub(super) const WENT_WRONG: &str = "\u{2717}";
/// The square's value before the first read of its project.
pub(super) const READING: &str = "\u{2026}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LearningTab {
    pub(super) folder: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
}

/// §1: "The project is the tab's folder, as the Git square finds it." This is the pane's folder; the project it is in
/// is found off the draw path (`crate::nirvana_learning::project_root`, review S2).
pub(super) fn learning_tab(snapshot: &ClientShellSnapshot) -> Option<LearningTab> {
    let agent = super::nirvana_mods::strip_agent(snapshot)?;
    Some(LearningTab {
        folder: agent.cwd,
        workspace_id: agent.workspace_id,
        tab_id: agent.tab_id,
    })
}

/// §1.1: the live lesson count and today's reviews against the cap, `14 lessons · review 3/20`, narrow `14 · 3/20`,
/// `no lessons yet` when the project keeps none; `WENT_WRONG` at the end when the last review today was held back,
/// failed or had every op refused.
pub(super) fn learning_value(view: &LearningView, narrow: bool) -> String {
    let today = &view.today;
    // Review S1: the cap's own count, which a reseat child does not double.
    let used = today.used;
    let mut value = if view.lessons.is_empty() {
        if narrow { "none yet" } else { "no lessons yet" }.to_owned()
    } else if narrow {
        format!("{}", view.live)
    } else {
        format!(
            "{} lesson{}",
            view.live,
            if view.live == 1 { "" } else { "s" }
        )
    };
    if !view.lessons.is_empty() || used > 0 {
        value = if narrow {
            format!("{value} \u{b7} {used}/{}", today.cap)
        } else {
            format!("{value} \u{b7} review {used}/{}", today.cap)
        };
    }
    if today.last_went_wrong() {
        value = format!("{value} {WENT_WRONG}");
    }
    value
}

/// A selectable row: an index into the view's changes, lessons or profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Item {
    Change(usize),
    Lesson(usize),
    Profile(usize),
}

enum Line {
    Head(&'static str),
    Text(String),
    Item(Item, String),
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "3m ago", "2h ago", "4d ago".
pub(super) fn ago(at: u64, now: u64) -> String {
    let seconds = now.saturating_sub(at);
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{}m ago", seconds / 60),
        3_600..=86_399 => format!("{}h ago", seconds / 3_600),
        _ => format!("{}d ago", seconds / 86_400),
    }
}

fn where_of(change: &Change) -> String {
    match &change.source {
        Source::Lessons => "lessons".into(),
        Source::Profile(name) => format!("cards of {name}"),
    }
}

fn lines(view: &LearningView, now: u64) -> Vec<Line> {
    let mut out = Vec::new();
    let today = &view.today;
    out.push(Line::Head("Today"));
    // Review N1: the cap's day starts at 00:00 UTC, which is not the owner's midnight.
    if today.reviews.is_empty() && today.used == 0 {
        out.push(Line::Text(format!(
            "no lesson review this UTC day \u{b7} cap {} a day",
            today.cap
        )));
    } else {
        out.push(Line::Text(format!(
            "reviews {}/{} this UTC day \u{b7} {} gated: {} done \u{b7} {} held back \u{b7} {} failed \u{b7} {} not gated yet",
            today.used,
            today.cap,
            today.gated(),
            today.count(Ruled::Done),
            today.count(Ruled::HeldBack),
            today.count(Ruled::Failed),
            today.count(Ruled::Waiting),
        )));
        out.push(Line::Text(format!(
            "ops {} applied \u{b7} {} refused",
            today.applied(),
            today.refused()
        )));
        for (why, n) in today
            .reasons
            .iter()
            .take(crate::nirvana_learning::REASONS_SHOWN)
        {
            out.push(Line::Text(format!("  {n}\u{d7} {why}")));
        }
        let rest: usize = today
            .reasons
            .iter()
            .skip(crate::nirvana_learning::REASONS_SHOWN)
            .map(|(_, n)| n)
            .sum();
        if rest > 0 {
            out.push(Line::Text(format!("  {rest}\u{d7} other reasons")));
        }
    }
    out.push(Line::Head("Recent changes"));
    if view.changes.is_empty() {
        out.push(Line::Text("no op in a ledger yet".into()));
    }
    for (i, change) in view.changes.iter().enumerate() {
        out.push(Line::Item(
            Item::Change(i),
            format!(
                "{} {} {} \u{b7} {} \u{b7} by {} \u{b7} {}{}",
                change.id,
                change.op,
                change.slugs.join(", "),
                where_of(change),
                change.by,
                ago(change.at, now),
                if change.undone { " \u{b7} undone" } else { "" }
            ),
        ));
    }
    out.push(Line::Head("Lessons"));
    if view.lessons.is_empty() {
        out.push(Line::Text("no lesson in .nirvana/lessons".into()));
    }
    for stat in &view.stats {
        out.push(Line::Text(stat.clone()));
    }
    for (i, lesson) in view.lessons.iter().enumerate() {
        out.push(Line::Item(
            Item::Lesson(i),
            format!(
                "{} {} \u{b7} {} \u{b7} {}{} \u{b7} offered {} \u{b7} opened {} \u{b7} passed after opened {}",
                if lesson.live { "live   " } else { "decayed" },
                lesson.name,
                lesson.origin.word(),
                lesson.class,
                lesson
                    .profile
                    .as_deref()
                    .map(|p| format!(" @{p}"))
                    .unwrap_or_default(),
                lesson.counters.offered,
                lesson.counters.opened,
                lesson.counters.passed_after_opened,
            ),
        ));
    }
    out.push(Line::Head("Profiles"));
    if view.profiles.is_empty() {
        out.push(Line::Text("no profile in .nirvana/profiles".into()));
    }
    for (i, profile) in view.profiles.iter().enumerate() {
        let text = if profile.knowledge.is_none() {
            format!("{} \u{b7} one file, keeps no cards", profile.name)
        } else {
            let mut text = format!(
                "{} \u{b7} {}",
                profile.name,
                plural(profile.cards, "card", "cards")
            );
            if profile.warnings > 0 {
                text.push_str(&format!(
                    " \u{b7} {}",
                    plural(profile.warnings, "warning", "warnings")
                ));
            }
            if profile.refused > 0 {
                text.push_str(&format!(" \u{b7} {} refused", profile.refused));
            }
            if profile.unknown > 0 {
                text.push_str(&format!(" \u{b7} {} not known to herdr", profile.unknown));
            }
            text
        };
        out.push(Line::Item(Item::Profile(i), text));
        // §1.2 "the `--adopt` line": the one step that makes a hand-written card known.
        if profile.unknown > 0 {
            out.push(Line::Text(format!(
                "{} herdr does not know is never offered: herdr profile check {} --adopt",
                plural(profile.unknown, "card", "cards"),
                profile.name
            )));
        }
    }
    // Order LEARN: when the last idle review ran, what it did, and the switch (`nirvana_idle_review::lines`).
    out.push(Line::Head("Idle review"));
    for line in crate::nirvana_idle_review::lines(&view.idle, now) {
        out.push(Line::Text(line));
    }
    out.push(Line::Head("Waiting"));
    if view.waiting.is_empty() {
        out.push(Line::Text(format!(
            "no class over {} live lessons",
            crate::nirvana_review::MERGE_ABOVE
        )));
    }
    for (class, profile, count) in &view.waiting {
        out.push(Line::Text(format!(
            "{class}{}: {count} live lessons, merged at the next herdr run end",
            profile
                .as_deref()
                .map(|p| format!("@{p}"))
                .unwrap_or_default()
        )));
    }
    out
}

/// The selectable rows, in the panel's order.
pub(super) fn items(view: &LearningView) -> Vec<Item> {
    lines(view, 0)
        .into_iter()
        .filter_map(|line| match line {
            Line::Item(item, _) => Some(item),
            _ => None,
        })
        .collect()
}

/// What a row is, whatever its place: the highlight stays on it when a refresh adds a row above it (review N2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RowKey {
    Change(Source, String),
    Lesson(String),
    Profile(String),
}

fn key_of(view: &LearningView, item: Item) -> Option<RowKey> {
    Some(match item {
        Item::Change(i) => {
            let change = view.changes.get(i)?;
            RowKey::Change(change.source.clone(), change.id.clone())
        }
        Item::Lesson(i) => RowKey::Lesson(view.lessons.get(i)?.slug.clone()),
        Item::Profile(i) => RowKey::Profile(view.profiles.get(i)?.name.clone()),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LearningPanel {
    /// The pane's folder, which `cached` finds the project of; the view's `root` is the project.
    pub(super) folder: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    /// An index into `items`.
    pub(super) selected: usize,
    /// The row `selected` was on at the last draw; `None` after a key or a click moved it.
    pub(super) selected_key: Option<RowKey>,
    pub(super) scroll: usize,
    /// §1.3: the change `u` asked about; the answer rolls back this row, whatever the ledger shows meanwhile.
    pub(super) confirming: Option<Change>,
    /// The ticket of the undo this panel started and waits on (`crate::nirvana_learning::start_undo`).
    pub(super) undoing: Option<u64>,
    pub(super) message: Option<String>,
}

impl LearningPanel {
    pub(super) fn busy(&self) -> bool {
        self.undoing.is_some()
    }

    /// A key or a click moved the selection: the next draw keeps the index and remembers its row.
    fn moved(&mut self, selected: usize) {
        self.selected = selected;
        self.selected_key = None;
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct LearningHits {
    pub(super) area: Rect,
    pub(super) close: Rect,
    /// Each selectable row, by its index into `items`.
    pub(super) rows: Vec<(Rect, usize)>,
    pub(super) undo_btn: Rect,
    pub(super) cancel_btn: Rect,
}

pub(super) fn render_learning_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &mut LearningPanel,
    view: Option<&LearningView>,
    now: u64,
    palette: &Palette,
) -> LearningHits {
    let mut hits = LearningHits::default();
    let panel_width = LEARNING_WIDTH.min(area.width.saturating_sub(2));
    if panel_width < 20 || area.height < 6 {
        return hits;
    }
    let inner = usize::from(panel_width.saturating_sub(4));
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    let muted = Style::default().fg(palette.subtext0).bg(palette.panel_bg);
    let head = base.fg(palette.accent).add_modifier(Modifier::BOLD);
    let empty = LearningView::default();
    let shown = view.unwrap_or(&empty);
    let body = if view.is_some() {
        lines(shown, now)
    } else {
        vec![Line::Text("reading .nirvana/lessons\u{2026}".into())]
    };
    let height = (body.len() as u16 + 4).min(area.height.saturating_sub(1));
    let rect = Rect::new(area.x + 1, area.y + 1, panel_width, height);
    hits.area = rect;
    let title = format!(
        " Learning \u{b7} {} ",
        folder_name(view.map_or(&panel.folder, |view| &view.root))
    );
    hits.close = draw_frame(buffer, rect, &title, palette);
    let x = rect.x + 2;
    let body_height = usize::from(rect.height.saturating_sub(4)).max(1);

    let listed = items(shown);
    if let Some(key) = &panel.selected_key {
        if let Some(at) = listed
            .iter()
            .position(|item| key_of(shown, *item).as_ref() == Some(key))
        {
            panel.selected = at;
        }
    }
    let count = listed.len();
    panel.selected = panel.selected.min(count.saturating_sub(1));
    if view.is_some() {
        panel.selected_key = listed
            .get(panel.selected)
            .and_then(|item| key_of(shown, *item));
    }
    // The line the selected row is on, so the scroll keeps it in sight.
    let mut seen = 0;
    let selected_line = body.iter().position(|line| {
        if let Line::Item(..) = line {
            seen += 1;
            seen == panel.selected + 1
        } else {
            false
        }
    });
    if let Some(at) = selected_line {
        if at < panel.scroll {
            panel.scroll = at.saturating_sub(1);
        } else if at >= panel.scroll + body_height {
            panel.scroll = at + 1 - body_height;
        }
    }
    panel.scroll = panel.scroll.min(body.len().saturating_sub(body_height));

    let mut index = 0;
    for (n, line) in body.iter().enumerate() {
        let this = match line {
            Line::Item(..) => {
                index += 1;
                Some(index - 1)
            }
            _ => None,
        };
        if n < panel.scroll || n >= panel.scroll + body_height {
            continue;
        }
        let y = rect.y + 1 + (n - panel.scroll) as u16;
        match line {
            Line::Head(text) => put_text(buffer, x, y, inner as u16, &fit(text, inner), head),
            Line::Text(text) => put_text(
                buffer,
                x,
                y,
                inner as u16,
                &fit(&format!("  {text}"), inner),
                muted,
            ),
            Line::Item(_, text) => {
                let index = this.unwrap_or_default();
                let row = Rect::new(x, y, inner as u16, 1);
                hits.rows.push((row, index));
                let style = if index == panel.selected {
                    buffer.set_style(row, Style::default().bg(palette.selection_bg));
                    base.bg(palette.selection_bg)
                } else {
                    base
                };
                put_text(
                    buffer,
                    x,
                    y,
                    inner as u16,
                    &fit(&format!("  {text}"), inner),
                    style,
                );
            }
        }
    }

    let foot_y = rect.bottom().saturating_sub(2);
    if let Some(change) = &panel.confirming {
        let ask = format!(
            "undo {} ({} of {}, {})? ",
            change.id,
            change.op,
            change.slugs.join(", "),
            where_of(change)
        );
        let (undo, cancel) = ("[Undo]", "[Cancel]");
        let room = inner.saturating_sub(undo.len() + cancel.len() + 1);
        let ask = fit(&ask, room);
        let ask_width = unicode_width::UnicodeWidthStr::width(ask.as_str()) as u16;
        put_text(buffer, x, foot_y, ask_width, &ask, base.fg(palette.yellow));
        let button = Style::default()
            .fg(palette.accent)
            .bg(palette.panel_bg)
            .add_modifier(Modifier::BOLD);
        hits.undo_btn = Rect::new(x + ask_width, foot_y, undo.len() as u16, 1);
        put_text(
            buffer,
            hits.undo_btn.x,
            foot_y,
            hits.undo_btn.width,
            undo,
            button,
        );
        hits.cancel_btn = Rect::new(hits.undo_btn.right() + 1, foot_y, cancel.len() as u16, 1);
        put_text(
            buffer,
            hits.cancel_btn.x,
            foot_y,
            hits.cancel_btn.width,
            cancel,
            muted,
        );
    } else {
        // Review N6: opening is no write, but a hand edit of a lesson or card holds the next review back
        // (`lessons_folder`, `knowledge_folder`).
        let keys = "j/k move \u{b7} enter open (hand edits hold the next review back) \u{b7} u undo \u{b7} esc close";
        let text = match (&panel.message, panel.busy()) {
            (Some(message), _) => message.clone(),
            (None, true) => "undoing\u{2026}".into(),
            (None, false) => keys.into(),
        };
        put_text(
            buffer,
            x,
            foot_y,
            inner as u16,
            &fit(&text, inner),
            if panel.message.is_some() || panel.busy() {
                base.fg(palette.yellow)
            } else {
                muted
            },
        );
    }
    hits
}

impl ClientShellState {
    pub(super) fn toggle_learning_panel(&mut self) {
        if self.nirvana_learning.take().is_some() {
            return;
        }
        let Some(tab) = self.snapshot.as_deref().and_then(learning_tab) else {
            return;
        };
        self.nirvana_preload = None;
        self.nirvana_running = None;
        self.nirvana_memory = None;
        self.nirvana_accounts = None;
        self.nirvana_seats = None;
        self.nirvana_jobs = None;
        self.nirvana_git = None;
        self.nirvana_learning = Some(LearningPanel {
            folder: tab.folder,
            workspace_id: tab.workspace_id,
            tab_id: tab.tab_id,
            selected: 0,
            selected_key: None,
            scroll: 0,
            confirming: None,
            undoing: None,
            message: None,
        });
    }

    fn learning_view(&self) -> Option<LearningView> {
        self.nirvana_learning
            .as_ref()
            .and_then(|panel| crate::nirvana_learning::cached(&panel.folder))
    }

    fn learning_say(&mut self, message: impl Into<String>) {
        if let Some(panel) = self.nirvana_learning.as_mut() {
            panel.message = Some(message.into());
        }
    }

    fn learning_item(&self) -> Option<(LearningView, Item)> {
        let view = self.learning_view()?;
        let selected = self.nirvana_learning.as_ref()?.selected;
        let item = *items(&view).get(selected)?;
        Some((view, item))
    }

    /// Enter or `o`: a lesson or a card in the edit tab, a folder profile's `knowledge/` in an edit tab started there.
    fn learning_open(&mut self, outcome: &mut ClientShellInput) {
        let Some((view, item)) = self.learning_item() else {
            return;
        };
        let command = match item {
            Item::Change(i) => match view.change_file(&view.changes[i]) {
                Some(file) => format!("{}{}", crate::nirvana::EDIT_FILE_COMMAND, file.display()),
                None => {
                    let change = &view.changes[i];
                    return self.learning_say(format!(
                        "{} left no file: {} is not there now",
                        change.id,
                        change.slugs.join(", ")
                    ));
                }
            },
            Item::Lesson(i) => {
                let file = view.lesson_file(&view.lessons[i]);
                if !file.is_file() {
                    return self.learning_say(format!("no file {}", file.display()));
                }
                format!("{}{}", crate::nirvana::EDIT_FILE_COMMAND, file.display())
            }
            Item::Profile(i) => {
                let profile = &view.profiles[i];
                match &profile.knowledge {
                    Some(dir) if dir.is_dir() => {
                        format!("{}{}", crate::nirvana::EDIT_DIR_COMMAND, dir.display())
                    }
                    Some(dir) => return self.learning_say(format!("no folder {}", dir.display())),
                    None => format!(
                        "{}{}",
                        crate::nirvana::EDIT_FILE_COMMAND,
                        crate::nirvana_profiles::path(&view.root, &profile.name).display()
                    ),
                }
            }
        };
        let Some(panel) = self.nirvana_learning.take() else {
            return;
        };
        self.push_endpoint_method(
            crate::api::schema::Method::CommandInvoke(crate::api::schema::CommandInvokeParams {
                command_id: command,
                workspace_id: Some(panel.workspace_id),
                tab_id: Some(panel.tab_id),
                pane_id: None,
                selection: None,
            }),
            outcome,
        );
    }

    /// `u`: asks about the selected change, in the panel; nothing runs until the answer.
    fn learning_ask_undo(&mut self) {
        let Some((view, item)) = self.learning_item() else {
            return;
        };
        let Item::Change(i) = item else {
            return self.learning_say("u undoes a change: pick a row under Recent changes");
        };
        let change = view.changes[i].clone();
        if let Some(why) = change.refusal() {
            return self.learning_say(why);
        }
        if let Some(panel) = self.nirvana_learning.as_mut() {
            panel.message = None;
            panel.confirming = Some(change);
        }
    }

    /// The answer: the rollback of exactly the row asked about, on a thread.
    pub(super) fn learning_confirm_undo(&mut self) {
        // The project the row came from: the folder's, found once, so the view read now has the same root.
        let root = self.learning_view().map(|view| view.root);
        let Some(panel) = self.nirvana_learning.as_mut() else {
            return;
        };
        let Some(change) = panel.confirming.take() else {
            return;
        };
        if panel.busy() {
            panel.message = Some("an undo is still running".into());
            return;
        }
        let Some(root) = root else {
            panel.message = Some("not undone: the project is not read yet".into());
            return;
        };
        panel.message = None;
        panel.undoing = Some(crate::nirvana_learning::start_undo(root, change));
    }

    pub(super) fn route_learning_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        let count = self.learning_view().map_or(0, |view| items(&view).len());
        let last = count.saturating_sub(1);
        let Some(panel) = self.nirvana_learning.as_mut() else {
            return;
        };
        if panel.confirming.is_some() {
            // Review S5: `y` only. Enter opens everywhere else in the panel, so a habitual Enter after `u` must not
            // roll a lesson back.
            match key.code {
                KeyCode::Char('y') => self.learning_confirm_undo(),
                KeyCode::Char('n') | KeyCode::Esc => panel.confirming = None,
                _ => {}
            }
            outcome.repaint = true;
            return;
        }
        if !panel.busy() {
            panel.message = None;
        }
        let selected = panel.selected;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.nirvana_learning = None,
            KeyCode::Up | KeyCode::Char('k') => panel.moved(selected.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => panel.moved((selected + 1).min(last)),
            KeyCode::Home => panel.moved(0),
            KeyCode::End => panel.moved(last),
            KeyCode::Enter | KeyCode::Char('o') => self.learning_open(outcome),
            KeyCode::Char('u') => self.learning_ask_undo(),
            _ => {}
        }
        outcome.repaint = true;
    }

    pub(super) fn handle_learning_mouse(
        &mut self,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if self.nirvana_learning.is_none() {
            return false;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let point = (mouse.column, mouse.row);
            let hits = self.hits.nirvana_learning_panel.clone();
            if !super::contains(hits.area, point) || super::contains(hits.close, point) {
                self.nirvana_learning = None;
            } else if hits.undo_btn.width > 0 && super::contains(hits.undo_btn, point) {
                self.learning_confirm_undo();
            } else if hits.cancel_btn.width > 0 && super::contains(hits.cancel_btn, point) {
                if let Some(panel) = self.nirvana_learning.as_mut() {
                    panel.confirming = None;
                }
            } else if let Some((_, index)) = hits
                .rows
                .iter()
                .find(|(rect, _)| super::contains(*rect, point))
            {
                if let Some(panel) = self.nirvana_learning.as_mut() {
                    panel.moved(*index);
                    panel.message = None;
                }
            }
            outcome.repaint = true;
        }
        // Review N3: the wheel moves the selection, which the draw keeps in sight; the list can be hundreds of rows.
        let step = match mouse.kind {
            MouseEventKind::ScrollUp => Some(false),
            MouseEventKind::ScrollDown => Some(true),
            _ => None,
        };
        if let Some(down) = step {
            let last = self
                .learning_view()
                .map_or(0, |view| items(&view).len().saturating_sub(1));
            if let Some(panel) = self.nirvana_learning.as_mut() {
                let selected = panel.selected;
                panel.moved(if down {
                    (selected + 1).min(last)
                } else {
                    selected.saturating_sub(1)
                });
            }
            outcome.repaint = true;
        }
        true
    }
}
