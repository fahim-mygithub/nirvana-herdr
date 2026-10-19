//! Nirvana fork, plan 15: the Jobs square in the mod strip, and its panel.
//! Shows running, passed, and failed jobs and Claude Code background tasks.
//! Clicking a job focuses its pane in the Jobs tab; clicking a Claude task tails the file.

use std::path::PathBuf;

use super::nirvana_mods::{draw_frame, folder_name};
use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

pub(super) const NARROW_STRIP: u16 = 100;
pub(super) const JOBS_WIDTH: u16 = 96;

const NAME: usize = 20;
const STATE: usize = 10;
const TIME: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct JobsTab {
    pub(super) folder: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
}

pub(super) fn jobs_tab(snapshot: &ClientShellSnapshot) -> Option<JobsTab> {
    let agent = super::nirvana_mods::strip_agent(snapshot)?;
    Some(JobsTab {
        folder: agent.cwd,
        workspace_id: agent.workspace_id,
        tab_id: agent.tab_id,
    })
}

pub(super) fn jobs_value(view: &crate::nirvana_job::JobsView, narrow: bool) -> (String, bool) {
    if view.jobs.is_empty() && view.claude_tasks.is_empty() {
        return (String::new(), false);
    }
    let mut parts = Vec::new();
    if view.running_count > 0 {
        parts.push(if narrow {
            format!("{} run", view.running_count)
        } else {
            format!("{} running", view.running_count)
        });
    }
    if view.failed_count > 0 {
        parts.push(if narrow {
            format!("{} fail", view.failed_count)
        } else {
            format!("{} failed", view.failed_count)
        });
    }
    if view.running_count == 0 && view.failed_count == 0 && view.passed_count > 0 {
        parts.push(if narrow {
            format!("{} pass", view.passed_count)
        } else {
            format!("{} passed", view.passed_count)
        });
    }
    let value = parts.join(" \u{b7} ");
    let active = view.running_count > 0 || view.failed_count > 0;
    (value, active)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobsPanel {
    pub(super) folder: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    pub(super) selected: usize,
    pub(super) scroll: usize,
    pub(super) message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct JobsHits {
    pub(super) area: Rect,
    pub(super) close: Rect,
    pub(super) rows: Vec<(Rect, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum JobItem {
    Job(crate::nirvana_job::JobRecord),
    Claude(crate::nirvana_job::ClaudeTask),
}

fn items(view: &crate::nirvana_job::JobsView) -> Vec<JobItem> {
    let mut all = Vec::new();
    for job in &view.jobs {
        all.push(JobItem::Job(job.clone()));
    }
    for task in &view.claude_tasks {
        all.push(JobItem::Claude(task.clone()));
    }
    all
}

fn column(text: &str, width: usize) -> String {
    let text = fit(text, width);
    let pad = width.saturating_sub(unicode_width::UnicodeWidthStr::width(text.as_str()));
    format!("{text}{}", " ".repeat(pad))
}

pub(super) fn render_jobs_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &mut JobsPanel,
    view: Option<&crate::nirvana_job::JobsView>,
    now: u64,
    palette: &Palette,
) -> JobsHits {
    let mut hits = JobsHits::default();
    let panel_width = JOBS_WIDTH.min(area.width.saturating_sub(2));
    if panel_width < 20 || area.height < 6 {
        return hits;
    }
    let inner = usize::from(panel_width.saturating_sub(4));
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    let muted = Style::default().fg(palette.subtext0).bg(palette.panel_bg);
    let empty_view = crate::nirvana_job::JobsView::default();
    let shown = view.unwrap_or(&empty_view);
    let job_items = items(shown);

    let height = ((job_items.len() + 3) as u16 + 4).min(area.height.saturating_sub(1));
    let rect = Rect::new(area.x + 1, area.y + 1, panel_width, height);
    hits.area = rect;

    let title = format!(" Jobs \u{b7} {} ", folder_name(&panel.folder));
    hits.close = draw_frame(buffer, rect, &title, palette);

    let x = rect.x + 2;
    let body_height = usize::from(rect.height.saturating_sub(4));

    if panel.selected < panel.scroll {
        panel.scroll = panel.selected;
    } else if body_height > 1 && panel.selected >= panel.scroll + (body_height - 1) {
        panel.scroll = panel.selected + 2 - body_height;
    }
    let max_scroll = job_items
        .len()
        .saturating_sub(body_height.saturating_sub(1));
    panel.scroll = panel.scroll.min(max_scroll);

    // Columns header at row 0 of body
    let mut current_y = rect.y + 1;
    let header_text = format!(
        "{} {} {} last line",
        column("name", NAME),
        column("state", STATE),
        column("time", TIME)
    );
    put_text(
        buffer,
        x,
        current_y,
        inner as u16,
        &fit(&header_text, inner),
        muted,
    );
    current_y += 1;

    let available_rows = body_height.saturating_sub(1);
    if job_items.is_empty() {
        if available_rows > 0 {
            put_text(
                buffer,
                x,
                current_y,
                inner as u16,
                "No jobs or Claude tasks in the last 12 hours",
                muted,
            );
        }
    } else {
        for (offset, (item_idx, item)) in job_items
            .iter()
            .enumerate()
            .skip(panel.scroll)
            .take(available_rows)
            .enumerate()
        {
            let y = current_y + offset as u16;
            let (name, state_str, elapsed_str, last, is_running, is_failed) = match item {
                JobItem::Job(j) => {
                    let is_alive = j.pid.is_some_and(crate::nirvana_job::is_process_alive);
                    let state = j.state(is_alive);
                    let elapsed = if let Some(ended) = j.ended {
                        ended.saturating_sub(j.started)
                    } else {
                        now.saturating_sub(j.started)
                    };
                    let last = shown
                        .job_last_lines
                        .get(&j.id)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    (
                        j.name.as_str(),
                        state.as_str(),
                        crate::nirvana_running::elapsed(elapsed),
                        last,
                        state == crate::nirvana_job::JobState::Running,
                        state == crate::nirvana_job::JobState::Failed
                            || state == crate::nirvana_job::JobState::Lost,
                    )
                }
                JobItem::Claude(t) => {
                    let elapsed = t.elapsed;
                    let last = t.last_line.as_deref().unwrap_or("");
                    (
                        t.name.as_str(),
                        t.state.as_str(),
                        crate::nirvana_running::elapsed(elapsed),
                        last,
                        t.state == crate::nirvana_job::ClaudeTaskState::Running,
                        t.state == crate::nirvana_job::ClaudeTaskState::Failed,
                    )
                }
            };

            let row_rect = Rect::new(x, y, inner as u16, 1);
            hits.rows.push((row_rect, item_idx));

            let selected = item_idx == panel.selected;
            if selected {
                buffer.set_style(row_rect, Style::default().bg(palette.selection_bg));
            }

            let state_style = if is_running {
                Style::default().fg(palette.yellow)
            } else if is_failed {
                Style::default().fg(palette.red)
            } else {
                Style::default().fg(palette.green)
            };

            // Put name
            let name_col = column(name, NAME);
            let text_style = if selected {
                base.bg(palette.selection_bg)
            } else {
                base
            };
            put_text(buffer, x, y, NAME as u16, &name_col, text_style);

            // Put state
            let state_col = column(state_str, STATE);
            let cell_state_style = if selected {
                state_style.bg(palette.selection_bg)
            } else {
                state_style
            };
            put_text(
                buffer,
                x + NAME as u16 + 1,
                y,
                STATE as u16,
                &state_col,
                cell_state_style,
            );

            // Put time
            let time_col = column(&elapsed_str, TIME);
            put_text(
                buffer,
                x + (NAME + STATE + 2) as u16,
                y,
                TIME as u16,
                &time_col,
                text_style,
            );

            // Put last line
            let last_x = x + (NAME + STATE + TIME + 3) as u16;
            let last_width = (x + inner as u16).saturating_sub(last_x);
            if last_width > 0 && !last.is_empty() {
                let last_style = if selected {
                    muted.bg(palette.selection_bg)
                } else {
                    muted
                };
                put_text(
                    buffer,
                    last_x,
                    y,
                    last_width,
                    &fit(last, last_width as usize),
                    last_style,
                );
            }
        }
    }

    let foot_y = rect.bottom().saturating_sub(2);
    let keys = if job_items.is_empty() {
        "esc close"
    } else {
        "\u{2191}/\u{2193} choose \u{b7} enter focus / tail \u{b7} esc close"
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
    pub(super) fn toggle_jobs_panel(&mut self) {
        if self.nirvana_jobs.take().is_some() {
            return;
        }
        let Some(tab) = self.snapshot.as_deref().and_then(jobs_tab) else {
            return;
        };
        self.nirvana_preload = None;
        self.nirvana_running = None;
        self.nirvana_memory = None;
        self.nirvana_accounts = None;
        self.nirvana_seats = None;
        self.nirvana_jobs = Some(JobsPanel {
            folder: tab.folder,
            workspace_id: tab.workspace_id,
            tab_id: tab.tab_id,
            selected: 0,
            scroll: 0,
            message: None,
        });
    }

    fn jobs_say(&mut self, message: impl Into<String>) {
        if let Some(panel) = self.nirvana_jobs.as_mut() {
            panel.message = Some(message.into());
        }
    }

    fn jobs_view(&self) -> Option<crate::nirvana_job::JobsView> {
        self.nirvana_jobs
            .as_ref()
            .and_then(|panel| crate::nirvana_job::cached(&panel.folder))
    }

    fn jobs_count(&self) -> usize {
        self.jobs_view()
            .map_or(0, |v| v.jobs.len() + v.claude_tasks.len())
    }

    fn jobs_action(&mut self, index: usize, outcome: &mut ClientShellInput) {
        let Some(view) = self.jobs_view() else {
            return;
        };
        let all_items = items(&view);
        let Some(item) = all_items.get(index) else {
            return;
        };
        match item {
            JobItem::Job(j) => {
                let open = |pane: &str| {
                    self.snapshot
                        .as_deref()
                        .is_some_and(|s| s.panes.iter().any(|k| k.pane_id == pane))
                };
                match &j.pane {
                    Some(pane) if open(pane) => {
                        self.push_endpoint_method(
                            crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
                                pane_id: pane.clone(),
                            }),
                            outcome,
                        );
                        self.nirvana_jobs = None;
                    }
                    Some(pane) => self.jobs_say(format!("job {}'s pane {pane} is closed", j.name)),
                    None => self.jobs_say(format!("job {} has no pane", j.name)),
                }
            }
            JobItem::Claude(task) => {
                let cwd = self
                    .nirvana_jobs
                    .as_ref()
                    .map(|p| p.folder.display().to_string())
                    .unwrap_or_default();
                let task_name = format!("task-{}", task.id);
                let task_path = task.path.clone();
                std::thread::spawn(move || {
                    let _ = crate::nirvana_job::open_tail_pane(&cwd, &task_name, &task_path);
                });
                self.nirvana_jobs = None;
            }
        }
    }

    pub(super) fn route_jobs_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        let count = self.jobs_count();
        let last = count.saturating_sub(1);
        let Some(panel) = self.nirvana_jobs.as_mut() else {
            return;
        };
        panel.message = None;
        let selected = panel.selected;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.nirvana_jobs = None,
            KeyCode::Up | KeyCode::Char('k') => panel.selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => panel.selected = (selected + 1).min(last),
            KeyCode::Home => panel.selected = 0,
            KeyCode::End => panel.selected = last,
            KeyCode::Enter => self.jobs_action(selected, outcome),
            _ => {}
        }
        outcome.repaint = true;
    }

    pub(super) fn handle_jobs_mouse(
        &mut self,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if self.nirvana_jobs.is_none() {
            return false;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let point = (mouse.column, mouse.row);
            let hits = self.hits.nirvana_jobs_panel.clone();
            if let Some(panel) = self.nirvana_jobs.as_mut() {
                panel.message = None;
            }
            if !super::contains(hits.area, point) || super::contains(hits.close, point) {
                self.nirvana_jobs = None;
            } else if let Some((_, index)) = hits
                .rows
                .iter()
                .find(|(rect, _)| super::contains(*rect, point))
            {
                let index = *index;
                if let Some(panel) = self.nirvana_jobs.as_mut() {
                    panel.selected = index;
                }
                self.jobs_action(index, outcome);
            }
            outcome.repaint = true;
        }
        true
    }
}
