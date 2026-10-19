//! Nirvana fork, feature 11: the Accounts panel, opened by the "accounts" button at the foot of the sidebar.
//! One row per agent CLI: who is signed in, and a button that opens a tab running that CLI's own sign-in
//! (`crate::nirvana_accounts`). (nirvana/plans/2026-10-17-11-accounts.md)
//!
//! The owner, 2026-10-17: "In the bottom left let me manage my login accounts. Create a button so that this herder
//! instance let me login to claude code or antigravity or codex."

use super::nirvana_mods::draw_frame;
use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crate::nirvana_accounts::{Account, Cli};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::{Color, Modifier};

/// The panel's width at most; it sits at the bottom left of the pane area, beside the button.
pub(super) const ACCOUNTS_WIDTH: u16 = 104;
/// The CLI name column: "Antigravity" and "Claude Code" are the widest.
const NAME: usize = 12;

/// The open Accounts panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AccountsPanel {
    pub(super) selected: usize,
    pub(super) message: Option<String>,
}

/// Where the panel drew its parts, for the mouse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct AccountsHits {
    pub(super) area: Rect,
    pub(super) close: Rect,
    /// Each row's button, with its CLI.
    pub(super) buttons: Vec<(Rect, Cli)>,
    /// Each row, with its index, for a click that selects it.
    pub(super) rows: Vec<(Rect, usize)>,
}

/// The button's text on the foot's last row, plain like the spaces list's "new" and "menu". The owner,
/// 2026-10-17, of "[ accounts ]": "have it looks like the text here" (the "new ... menu" row).
pub(super) const BUTTON: &str = "accounts";

/// Formats the account text for a drifted row given available cell room (plan 18 §3).
/// Wide: "<current> ⚠ chosen <chosen>".
/// Narrow: drops the word "chosen" ("<current> ⚠ <chosen>"), then drops the chosen email ("<current> ⚠").
pub(super) fn account_drift_text(
    current: &str,
    drift: &crate::nirvana_accounts::Drift,
    room: usize,
) -> String {
    let full = format!("{current} \u{26A0} chosen {}", drift.chosen);
    if unicode_width::UnicodeWidthStr::width(full.as_str()) <= room {
        return full;
    }
    let without_chosen_word = format!("{current} \u{26A0} {}", drift.chosen);
    if unicode_width::UnicodeWidthStr::width(without_chosen_word.as_str()) <= room {
        return without_chosen_word;
    }
    let mark_only = format!("{current} \u{26A0}");
    fit(&mark_only, room)
}

/// One row's text: the name, the account, and the button at the right of `inner` cells.
pub(super) fn row_text_with_drift(
    account: &Account,
    drift: Option<&crate::nirvana_accounts::Drift>,
    inner: usize,
) -> (String, String) {
    let button = format!("[ {} ]", account.signed.button());
    let room = inner.saturating_sub(2 + NAME + 1 + button.len() + 1);
    let text = match drift {
        Some(drift) if account.cli == Cli::Antigravity => {
            account_drift_text(&account.signed.text(), drift, room)
        }
        _ => fit(&account.signed.text(), room),
    };
    let left = format!("\u{25CF} {:<NAME$} {}", account.cli.name(), text);
    (left, button)
}

/// Backward-compatible row_text without drift.
pub(super) fn row_text(account: &Account, inner: usize) -> (String, String) {
    row_text_with_drift(account, None, inner)
}

/// Draws the panel at the bottom left of `area`, as tall as its rows.
pub(super) fn render_accounts_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &AccountsPanel,
    accounts: &[Account],
    palette: &Palette,
) -> AccountsHits {
    let mut hits = AccountsHits::default();
    let drift = crate::nirvana_accounts::antigravity_drift();
    let cause_lines = if drift.is_some() { 1 } else { 0 };
    let width = area.width.saturating_sub(2).min(ACCOUNTS_WIDTH);
    let height = accounts.len() as u16 + 6 + cause_lines;
    if width < 44 || area.height < height + 1 {
        return hits;
    }
    let inner = usize::from(width.saturating_sub(4));
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    let muted = base.fg(palette.overlay1);
    let rect = Rect::new(area.x + 1, area.bottom() - height, width, height);
    hits.area = rect;
    hits.close = draw_frame(buffer, rect, " Accounts ", palette);
    let x = rect.x + 2;
    let mut y = rect.y + 1;
    for (index, account) in accounts.iter().enumerate() {
        let selected = index == panel.selected;
        let style = if selected {
            base.bg(palette.selection_bg)
        } else {
            base
        };
        let (left, button) = if drift.is_some() {
            row_text_with_drift(account, drift.as_ref(), inner)
        } else {
            row_text(account, inner)
        };
        let row = Rect::new(x, y, inner as u16, 1);
        buffer.set_style(row, style);
        put_text(buffer, x, y, inner as u16, &left, style);
        let (r, g, b) = account.cli.colour();
        put_text(buffer, x, y, 1, "\u{25CF}", style.fg(Color::Rgb(r, g, b)));
        if account.cli == Cli::Antigravity {
            if let Some(drift) = &drift {
                let button_width = button.len() as u16;
                let room = inner.saturating_sub(2 + NAME + 1 + button_width as usize + 1);
                let text = account_drift_text(&account.signed.text(), drift, room);
                let text_x = x + 2 + NAME as u16 + 1;
                put_text(
                    buffer,
                    text_x,
                    y,
                    room as u16,
                    &text,
                    style.fg(palette.yellow),
                );
            }
        }
        let button_width = button.len() as u16;
        let button_x = row.right().saturating_sub(button_width);
        put_text(
            buffer,
            button_x,
            y,
            button_width,
            &button,
            style.fg(palette.accent).add_modifier(Modifier::BOLD),
        );
        hits.buttons
            .push((Rect::new(button_x, y, button_width, 1), account.cli));
        hits.rows.push((row, index));
        y += 1;
        if account.cli == Cli::Antigravity {
            if let Some(drift) = &drift {
                let cause = drift.cause_line();
                put_text(buffer, x, y, inner as u16, &fit(&cause, inner), muted);
                y += 1;
            }
        }
    }
    y += 1;
    put_text(
        buffer,
        x,
        y,
        inner as u16,
        &fit(
            "A new tab runs the CLI's own sign-in; your browser does the rest.",
            inner,
        ),
        muted,
    );
    y += 1;
    let (text, style) = match &panel.message {
        Some(message) => (message.clone(), base.fg(palette.yellow)),
        None => (
            "\u{2191}/\u{2193} choose \u{b7} enter sign in \u{b7} esc close".to_owned(),
            muted,
        ),
    };
    put_text(buffer, x, y, inner as u16, &fit(&text, inner), style);
    hits
}

impl ClientShellState {
    /// Opens the Accounts panel, or closes it. The strip's panels close: one panel is open at a time.
    pub(super) fn toggle_accounts_panel(&mut self) {
        if self.nirvana_accounts.take().is_some() {
            return;
        }
        self.nirvana_preload = None;
        self.nirvana_running = None;
        self.nirvana_memory = None;
        self.nirvana_seats = None;
        self.nirvana_jobs = None;
        self.nirvana_git = None;
        crate::nirvana_accounts::read_soon();
        self.nirvana_accounts = Some(AccountsPanel::default());
    }

    /// Opens the sign-in tab for `cli`, through herdr's `command.invoke`, as Preload's edit does.
    fn account_login(&mut self, cli: Cli, outcome: &mut ClientShellInput) {
        let params = crate::api::schema::CommandInvokeParams {
            command_id: format!("{}{}", crate::nirvana::ACCOUNT_LOGIN_COMMAND, cli.key()),
            workspace_id: None,
            tab_id: None,
            pane_id: None,
            selection: None,
        };
        self.push_endpoint_method(crate::api::schema::Method::CommandInvoke(params), outcome);
        crate::nirvana_accounts::read_soon();
        self.nirvana_accounts = None;
    }

    /// The panel's keys. Every key goes to the panel while it is open.
    pub(super) fn route_accounts_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        let last = Cli::ALL.len() - 1;
        let Some(panel) = self.nirvana_accounts.as_mut() else {
            return;
        };
        panel.message = None;
        let selected = panel.selected;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.nirvana_accounts = None,
            KeyCode::Up | KeyCode::Char('k') => panel.selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => panel.selected = (selected + 1).min(last),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(cli) = Cli::ALL.get(selected).copied() {
                    self.account_login(cli, outcome);
                }
            }
            _ => {}
        }
        outcome.repaint = true;
    }

    /// The panel's mouse. While it is open every event is the panel's; a click outside closes it.
    pub(super) fn handle_accounts_mouse(
        &mut self,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if self.nirvana_accounts.is_none() {
            return false;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let point = (mouse.column, mouse.row);
            let hits = self.hits.nirvana_accounts_panel.clone();
            if let Some((_, cli)) = hits
                .buttons
                .iter()
                .find(|(rect, _)| super::contains(*rect, point))
            {
                self.account_login(*cli, outcome);
            } else if let Some((_, index)) = hits
                .rows
                .iter()
                .find(|(rect, _)| super::contains(*rect, point))
            {
                if let Some(panel) = self.nirvana_accounts.as_mut() {
                    panel.selected = *index;
                    panel.message = None;
                }
            } else if !super::contains(hits.area, point) || super::contains(hits.close, point) {
                // Taken here, so a click on the foot's button closes the panel and does not open it again.
                self.nirvana_accounts = None;
            }
            outcome.repaint = true;
        }
        true
    }
}
