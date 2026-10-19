//! Nirvana fork, plan 16: the Git square in the mod strip, and its panel.
//! Shows repository, branch, ahead/behind counts, CI workflow run status,
//! changed files list, and Pull / Push / Fetch buttons with a confirm step.

use std::path::PathBuf;

use super::nirvana_mods::{draw_frame, folder_name};
use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::Modifier;

pub(super) const NARROW_STRIP: u16 = 100;
pub(super) const GIT_WIDTH: u16 = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitTab {
    pub(super) folder: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
}

pub(super) fn git_tab(snapshot: &ClientShellSnapshot) -> Option<GitTab> {
    let agent = super::nirvana_mods::strip_agent(snapshot)?;
    Some(GitTab {
        folder: agent.cwd,
        workspace_id: agent.workspace_id,
        tab_id: agent.tab_id,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GitButton {
    Pull,
    Push,
    Fetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GitConfirm {
    Pull,
    Push,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitPanel {
    pub(super) folder: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    pub(super) selected_button: usize,
    pub(super) confirming: Option<GitConfirm>,
    pub(super) busy: Option<GitConfirm>,
    pub(super) message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct GitHits {
    pub(super) area: Rect,
    pub(super) close: Rect,
    pub(super) pull_btn: Rect,
    pub(super) push_btn: Rect,
    pub(super) fetch_btn: Rect,
    pub(super) cancel_btn: Rect,
}

pub(super) fn render_git_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &mut GitPanel,
    view: Option<&crate::nirvana_git::GitView>,
    now_secs: u64,
    palette: &Palette,
) -> GitHits {
    let mut hits = GitHits::default();
    let panel_width = GIT_WIDTH.min(area.width.saturating_sub(2));
    if panel_width < 20 || area.height < 6 {
        return hits;
    }
    let inner = usize::from(panel_width.saturating_sub(4));
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    let muted = Style::default().fg(palette.subtext0).bg(palette.panel_bg);

    let default_status = crate::nirvana_git::GitStatus {
        repo: panel.folder.clone(),
        ..Default::default()
    };
    let status = view.map(|v| &v.status).unwrap_or(&default_status);
    let ci_opt = view.and_then(|v| v.ci.as_ref());

    let file_count = status.files.len().min(crate::nirvana_git::FILES_SHOWN);
    let mut total_lines = 4 + file_count as u16;
    if ci_opt.is_some() {
        total_lines += 1;
    }
    if panel.confirming.is_some() {
        total_lines += 1;
    }
    let height = (total_lines + 4).min(area.height.saturating_sub(1));
    let rect = Rect::new(area.x + 1, area.y + 1, panel_width, height);
    hits.area = rect;

    let title = format!(" Git \u{b7} {} ", folder_name(&panel.folder));
    hits.close = draw_frame(buffer, rect, &title, palette);

    let x = rect.x + 2;
    let mut y = rect.y + 1;

    // Row 1: branch
    let branch_str = if status.branch.is_empty() {
        "(detached)".to_string()
    } else {
        status.branch.clone()
    };
    let branch_line = if status.upstream.is_empty() {
        format!("branch {} \u{b7} no upstream", branch_str)
    } else {
        format!("branch {} \u{2192} {}", branch_str, status.upstream)
    };
    put_text(buffer, x, y, inner as u16, &fit(&branch_line, inner), base);
    y += 1;

    // Row 2: ahead/behind
    let ab_line = format!(
        "\u{2191}{} ahead  \u{2193}{} behind (last fetch)",
        status.ahead, status.behind
    );
    put_text(buffer, x, y, inner as u16, &fit(&ab_line, inner), muted);
    y += 1;

    // Row 3: CI line (if present)
    if let Some(ci) = ci_opt {
        let ci_line = crate::nirvana_git::ci_row_text(ci, now_secs);
        let ci_style = match ci.state {
            crate::nirvana_git::CiState::Passed => base.fg(palette.green),
            crate::nirvana_git::CiState::Failed => base.fg(palette.red),
            crate::nirvana_git::CiState::Running => base.fg(palette.yellow),
        };
        put_text(buffer, x, y, inner as u16, &fit(&ci_line, inner), ci_style);
        y += 1;
    }

    // Row 4: Changed count & files
    if status.dirty == 0 {
        put_text(
            buffer,
            x,
            y,
            inner as u16,
            &fit("0 changed files \u{b7} clean tree", inner),
            muted,
        );
        y += 1;
    } else {
        let files_hdr = format!(
            "{} changed file{}:",
            status.dirty,
            if status.dirty == 1 { "" } else { "s" }
        );
        put_text(buffer, x, y, inner as u16, &fit(&files_hdr, inner), base);
        y += 1;
        for path in status.files.iter().take(crate::nirvana_git::FILES_SHOWN) {
            let row_text = format!("  {path}");
            put_text(buffer, x, y, inner as u16, &fit(&row_text, inner), muted);
            y += 1;
        }
    }

    // Confirm prompt (if confirming)
    if let Some(confirm_verb) = panel.confirming {
        let prompt = match confirm_verb {
            GitConfirm::Pull => {
                let commits = if status.behind == 1 {
                    "1 commit".to_string()
                } else {
                    format!("{} commits", status.behind)
                };
                let files = if status.dirty == 1 {
                    "1 changed file".to_string()
                } else {
                    format!("{} changed files", status.dirty)
                };
                let remote = if let Some((r, _)) = status.upstream.split_once('/') {
                    r
                } else {
                    "origin"
                };
                format!(
                    "Pull {} from {} into {}? {} in the tree.",
                    commits, remote, status.branch, files
                )
            }
            GitConfirm::Push => {
                let commits = if status.ahead == 1 {
                    "1 commit".to_string()
                } else {
                    format!("{} commits", status.ahead)
                };
                let files = if status.dirty == 1 {
                    "1 changed file".to_string()
                } else {
                    format!("{} changed files", status.dirty)
                };
                let remote = if let Some((r, _)) = status.upstream.split_once('/') {
                    r
                } else {
                    "origin"
                };
                format!(
                    "Push {} from {} to {}? {} in the tree.",
                    commits, status.branch, remote, files
                )
            }
        };
        put_text(
            buffer,
            x,
            y,
            inner as u16,
            &fit(&prompt, inner),
            base.fg(palette.yellow),
        );
        y += 1;
    }

    // Buttons row
    let btn_y = y;
    let mut cur_x = x;
    if panel.confirming.is_none() {
        // [Pull ↓M]
        let pull_label = if panel.busy == Some(GitConfirm::Pull) {
            "[Pulling\u{2026}]".to_string()
        } else if status.behind > 0 {
            format!("[Pull \u{2193}{}]", status.behind)
        } else {
            "[Pull]".to_string()
        };
        let pull_width = pull_label.len() as u16;
        let pull_rect = Rect::new(cur_x, btn_y, pull_width, 1);
        hits.pull_btn = pull_rect;
        let pull_disabled = status.upstream.is_empty();
        let pull_selected = panel.selected_button == 0;
        let pull_style = if pull_selected {
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else if pull_disabled {
            Style::default().fg(palette.overlay0).bg(palette.panel_bg)
        } else {
            Style::default()
                .fg(palette.accent)
                .bg(palette.panel_bg)
                .add_modifier(Modifier::BOLD)
        };
        put_text(buffer, cur_x, btn_y, pull_width, &pull_label, pull_style);
        cur_x += pull_width + 2;

        // [Push ↑N]
        let push_label = if panel.busy == Some(GitConfirm::Push) {
            "[Pushing\u{2026}]".to_string()
        } else if status.ahead > 0 {
            format!("[Push \u{2191}{}]", status.ahead)
        } else {
            "[Push]".to_string()
        };
        let push_width = push_label.len() as u16;
        let push_rect = Rect::new(cur_x, btn_y, push_width, 1);
        hits.push_btn = push_rect;
        let push_disabled = status.upstream.is_empty() || status.ahead == 0;
        let push_selected = panel.selected_button == 1;
        let push_style = if push_selected {
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else if push_disabled {
            Style::default().fg(palette.overlay0).bg(palette.panel_bg)
        } else {
            Style::default()
                .fg(palette.accent)
                .bg(palette.panel_bg)
                .add_modifier(Modifier::BOLD)
        };
        put_text(buffer, cur_x, btn_y, push_width, &push_label, push_style);
        cur_x += push_width + 2;

        // [Fetch]
        let fetch_label = "[Fetch]".to_string();
        let fetch_width = fetch_label.len() as u16;
        let fetch_rect = Rect::new(cur_x, btn_y, fetch_width, 1);
        hits.fetch_btn = fetch_rect;
        let fetch_disabled = status.upstream.is_empty();
        let fetch_selected = panel.selected_button == 2;
        let fetch_style = if fetch_selected {
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else if fetch_disabled {
            Style::default().fg(palette.overlay0).bg(palette.panel_bg)
        } else {
            Style::default()
                .fg(palette.accent)
                .bg(palette.panel_bg)
                .add_modifier(Modifier::BOLD)
        };
        put_text(buffer, cur_x, btn_y, fetch_width, &fetch_label, fetch_style);
    } else {
        // Confirming mode: [Pull] / [Push] and [Cancel]
        let confirm_label = match panel.confirming {
            Some(GitConfirm::Pull) => "[Pull]".to_string(),
            Some(GitConfirm::Push) => "[Push]".to_string(),
            None => "".to_string(),
        };
        let confirm_width = confirm_label.len() as u16;
        let confirm_rect = Rect::new(cur_x, btn_y, confirm_width, 1);
        if panel.confirming == Some(GitConfirm::Pull) {
            hits.pull_btn = confirm_rect;
        } else {
            hits.push_btn = confirm_rect;
        }
        let confirm_selected = panel.selected_button == 0;
        let confirm_style = if confirm_selected {
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(palette.accent)
                .bg(palette.panel_bg)
                .add_modifier(Modifier::BOLD)
        };
        put_text(
            buffer,
            cur_x,
            btn_y,
            confirm_width,
            &confirm_label,
            confirm_style,
        );
        cur_x += confirm_width + 2;

        let cancel_label = "[Cancel]".to_string();
        let cancel_width = cancel_label.len() as u16;
        let cancel_rect = Rect::new(cur_x, btn_y, cancel_width, 1);
        hits.cancel_btn = cancel_rect;
        let cancel_selected = panel.selected_button == 1;
        let cancel_style = if cancel_selected {
            Style::default()
                .fg(palette.panel_bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            muted
        };
        put_text(
            buffer,
            cur_x,
            btn_y,
            cancel_width,
            &cancel_label,
            cancel_style,
        );
    }

    // Status / keys line
    let foot_y = rect.bottom().saturating_sub(2);
    let keys = if panel.confirming.is_some() {
        "\u{2190}/\u{2192} choose \u{b7} enter confirm \u{b7} esc cancel"
    } else {
        "\u{2190}/\u{2192} choose \u{b7} enter press \u{b7} esc close"
    };
    put_text(
        buffer,
        x,
        foot_y,
        inner as u16,
        &fit(panel.message.as_deref().unwrap_or(keys), inner),
        if panel.message.is_some() {
            base.fg(palette.yellow)
        } else {
            muted
        },
    );

    hits
}

impl ClientShellState {
    pub(super) fn toggle_git_panel(&mut self) {
        if self.nirvana_git.take().is_some() {
            return;
        }
        let Some(tab) = self.snapshot.as_deref().and_then(git_tab) else {
            return;
        };
        self.nirvana_preload = None;
        self.nirvana_running = None;
        self.nirvana_memory = None;
        self.nirvana_accounts = None;
        self.nirvana_seats = None;
        self.nirvana_jobs = None;
        self.nirvana_git = Some(GitPanel {
            folder: tab.folder,
            workspace_id: tab.workspace_id,
            tab_id: tab.tab_id,
            selected_button: 0,
            confirming: None,
            busy: None,
            message: None,
        });
    }

    pub(super) fn git_action(&mut self, button: GitButton) {
        let Some(panel) = self.nirvana_git.as_mut() else {
            return;
        };
        let view = crate::nirvana_git::cached(&panel.folder);
        let default_status = crate::nirvana_git::GitStatus {
            repo: panel.folder.clone(),
            ..Default::default()
        };
        let status = view.as_ref().map(|v| &v.status).unwrap_or(&default_status);

        match button {
            GitButton::Pull => {
                if status.upstream.is_empty() {
                    panel.message = Some("git pull: refused. Branch has no upstream.".to_string());
                    return;
                }
                if panel.confirming != Some(GitConfirm::Pull) {
                    panel.confirming = Some(GitConfirm::Pull);
                    panel.selected_button = 0;
                    panel.message = None;
                    return;
                }
                panel.confirming = None;
                panel.selected_button = 0;
                let folder = panel.folder.clone();
                let branch = status.branch.clone();
                match crate::nirvana_git::pull_action(&folder, &branch) {
                    Ok(msg) => panel.message = Some(msg),
                    Err(err) => panel.message = Some(err),
                }
            }
            GitButton::Push => {
                if status.upstream.is_empty() {
                    panel.message = Some("git push: refused. Branch has no upstream.".to_string());
                    return;
                }
                if status.ahead == 0 {
                    panel.message = Some("git push: refused. Nothing ahead to push.".to_string());
                    return;
                }
                if panel.confirming != Some(GitConfirm::Push) {
                    panel.confirming = Some(GitConfirm::Push);
                    panel.selected_button = 0;
                    panel.message = None;
                    return;
                }
                panel.confirming = None;
                panel.selected_button = 0;
                let folder = panel.folder.clone();
                let branch = status.branch.clone();
                match crate::nirvana_git::push_action(&folder, &branch) {
                    Ok(msg) => panel.message = Some(msg),
                    Err(err) => panel.message = Some(err),
                }
            }
            GitButton::Fetch => {
                if status.upstream.is_empty() {
                    panel.message = Some("git fetch: refused. Branch has no upstream.".to_string());
                    return;
                }
                let folder = panel.folder.clone();
                let branch = status.branch.clone();
                match crate::nirvana_git::fetch_action(&folder, &branch) {
                    Ok(msg) => panel.message = Some(msg),
                    Err(err) => panel.message = Some(err),
                }
            }
        }
    }

    pub(super) fn route_git_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        if self.nirvana_git.is_none() {
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if let Some(panel) = self.nirvana_git.as_mut() {
                    if panel.confirming.is_some() {
                        panel.confirming = None;
                        panel.selected_button = 0;
                        panel.message = None;
                    } else {
                        self.nirvana_git = None;
                    }
                }
            }
            KeyCode::Left | KeyCode::Up | KeyCode::BackTab => {
                if let Some(panel) = self.nirvana_git.as_mut() {
                    panel.message = None;
                    let max = if panel.confirming.is_some() { 1 } else { 2 };
                    panel.selected_button = if panel.selected_button == 0 {
                        max
                    } else {
                        panel.selected_button - 1
                    };
                }
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                if let Some(panel) = self.nirvana_git.as_mut() {
                    panel.message = None;
                    let max = if panel.confirming.is_some() { 1 } else { 2 };
                    panel.selected_button = if panel.selected_button >= max {
                        0
                    } else {
                        panel.selected_button + 1
                    };
                }
            }
            KeyCode::Enter => {
                let action = {
                    let panel = self.nirvana_git.as_mut().unwrap();
                    panel.message = None;
                    if panel.confirming.is_some() {
                        if panel.selected_button == 1 {
                            panel.confirming = None;
                            panel.selected_button = 0;
                            None
                        } else {
                            match panel.confirming.unwrap() {
                                GitConfirm::Pull => Some(GitButton::Pull),
                                GitConfirm::Push => Some(GitButton::Push),
                            }
                        }
                    } else {
                        match panel.selected_button {
                            0 => Some(GitButton::Pull),
                            1 => Some(GitButton::Push),
                            _ => Some(GitButton::Fetch),
                        }
                    }
                };
                if let Some(button) = action {
                    self.git_action(button);
                }
            }
            _ => {}
        }
        outcome.repaint = true;
    }

    pub(super) fn handle_git_mouse(
        &mut self,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if self.nirvana_git.is_none() {
            return false;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let point = (mouse.column, mouse.row);
            let hits = self.hits.nirvana_git_panel.clone();
            if let Some(panel) = self.nirvana_git.as_mut() {
                panel.message = None;
            }
            if !super::contains(hits.area, point) || super::contains(hits.close, point) {
                self.nirvana_git = None;
            } else if hits.cancel_btn.width > 0 && super::contains(hits.cancel_btn, point) {
                if let Some(panel) = self.nirvana_git.as_mut() {
                    panel.confirming = None;
                    panel.selected_button = 0;
                }
            } else if hits.pull_btn.width > 0 && super::contains(hits.pull_btn, point) {
                self.git_action(GitButton::Pull);
            } else if hits.push_btn.width > 0 && super::contains(hits.push_btn, point) {
                self.git_action(GitButton::Push);
            } else if hits.fetch_btn.width > 0 && super::contains(hits.fetch_btn, point) {
                self.git_action(GitButton::Fetch);
            }
            outcome.repaint = true;
        }
        true
    }
}
