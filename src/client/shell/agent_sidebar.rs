use std::collections::HashMap;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Paragraph, Widget},
};

use super::*;

pub(super) struct AgentRow {
    pub(super) pane_id: String,
    pub(super) status: crate::api::schema::AgentStatus,
    pub(super) focused: bool,
    pub(super) rows: Vec<Vec<crate::ui::ResolvedToken>>,
    pub(super) subagents: Vec<crate::nirvana_subagents::Subagent>,
    pub(super) subagents_overflow: usize,
}

pub(super) fn is_narrow_agent_panel(width: u16) -> bool {
    width < 34
}

pub(super) fn agent_row_lines(row: &AgentRow, width: u16) -> usize {
    let base = row.rows.len().max(1);
    let narrow = is_narrow_agent_panel(width);
    let sub_lines: usize = row
        .subagents
        .iter()
        .map(|s| {
            if !narrow && !s.description.trim().is_empty() {
                2
            } else {
                1
            }
        })
        .sum();
    let overflow_line = if row.subagents_overflow > 0 { 1 } else { 0 };
    base + sub_lines + overflow_line
}

pub(super) fn ordered_agent_pane_ids(
    snapshot: &ClientShellSnapshot,
    sort: crate::config::AgentPanelSortConfig,
) -> Vec<String> {
    if snapshot.agent_view_label.is_some() {
        return snapshot
            .agent_order
            .iter()
            .filter(|pane_id| {
                snapshot
                    .agents
                    .iter()
                    .any(|agent| agent.pane_id == pane_id.as_str())
            })
            .cloned()
            .collect();
    }
    let mut agents = snapshot.agents.iter().collect::<Vec<_>>();
    if sort == crate::config::AgentPanelSortConfig::Priority {
        agents.sort_by_key(|agent| {
            (
                std::cmp::Reverse(status_priority(agent.agent_status)),
                std::cmp::Reverse(agent.state_change_seq),
            )
        });
    }
    agents
        .into_iter()
        .map(|agent| agent.pane_id.clone())
        .collect()
}

pub(super) fn render_agent_panel(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    agent_scroll: &mut usize,
    hits: &mut ShellHitMap,
) {
    if !render_agent_panel_header(
        buffer,
        area,
        snapshot.agent_view_label.as_deref(),
        config,
        hits,
    ) {
        return;
    }

    let rows = agent_rows(snapshot, config, None);
    render_agent_list(
        buffer,
        area,
        &rows,
        snapshot
            .agent_view_label
            .as_ref()
            .map(|_| " no matching agents"),
        config,
        agent_scroll,
        hits,
        |row| agent_row_lines(row, area.width),
        |buffer, rect, row, hits| {
            render_agent_row(buffer, rect, row, config, Some(hits));
        },
    );
}

pub(super) fn render_agent_panel_header(
    buffer: &mut Buffer,
    area: Rect,
    agent_view_label: Option<&str>,
    config: &ClientShellConfig,
    hits: &mut ShellHitMap,
) -> bool {
    if area.height == 0 {
        return false;
    }
    put_text(
        buffer,
        area.x,
        area.y,
        area.width,
        &"─".repeat(area.width as usize),
        Style::default().fg(config.palette.surface_dim),
    );
    if area.height < 2 {
        return false;
    }
    put_text(
        buffer,
        area.x,
        area.y + 1,
        area.width,
        " agents",
        Style::default()
            .fg(config.palette.overlay0)
            .add_modifier(Modifier::BOLD),
    );
    let sort_label = agent_view_label.unwrap_or(match config.agent_panel_sort {
        crate::config::AgentPanelSortConfig::Spaces => "grouped",
        crate::config::AgentPanelSortConfig::Priority => "priority",
    });
    let sort_width = display_width(sort_label).min(area.width as usize) as u16;
    let sort_rect = Rect::new(
        area.right().saturating_sub(sort_width),
        area.y + 1,
        sort_width,
        1,
    );
    hits.agent_sort_toggle = if config.mouse_capture && agent_view_label.is_none() {
        sort_rect
    } else {
        Rect::default()
    };
    put_text(
        buffer,
        sort_rect.x,
        sort_rect.y,
        sort_rect.width,
        sort_label,
        Style::default()
            .fg(if agent_view_label.is_some() {
                config.palette.accent
            } else {
                config.palette.overlay0
            })
            .add_modifier(Modifier::BOLD),
    );
    true
}

pub(super) fn render_agent_list<T>(
    buffer: &mut Buffer,
    area: Rect,
    rows: &[T],
    empty_message: Option<&str>,
    config: &ClientShellConfig,
    agent_scroll: &mut usize,
    hits: &mut ShellHitMap,
    row_lines: impl Fn(&T) -> usize,
    mut render_row: impl FnMut(&mut Buffer, Rect, &T, &mut ShellHitMap),
) {
    let body = Rect::new(
        area.x,
        area.y.saturating_add(3),
        area.width,
        area.height.saturating_sub(3),
    );
    hits.agent_body = body;
    if body.is_empty() || rows.is_empty() {
        *agent_scroll = 0;
        if let Some(message) = empty_message.filter(|_| !body.is_empty()) {
            put_text(
                buffer,
                body.x,
                body.y,
                body.width,
                message,
                Style::default()
                    .fg(config.palette.overlay0)
                    .add_modifier(Modifier::DIM),
            );
        }
        return;
    }

    let row_heights = rows
        .iter()
        .map(|row| row_lines(row).max(1).min(u16::MAX as usize) as u16)
        .collect::<Vec<_>>();
    let gaps = rows
        .iter()
        .enumerate()
        .map(|(index, _)| {
            if index + 1 < rows.len() {
                config.agents.row_gap
            } else {
                0
            }
        })
        .collect::<Vec<_>>();
    let metrics =
        super::scroll::list_scroll_metrics(&row_heights, &gaps, body.height, *agent_scroll);
    hits.agent_max_scroll = metrics.max_offset_from_bottom;
    hits.agent_scroll_metrics = Some(metrics);
    *agent_scroll = metrics
        .max_offset_from_bottom
        .saturating_sub(metrics.offset_from_bottom);
    let show_scrollbar = metrics.max_offset_from_bottom > 0 && body.width > 1;
    let content_width = body.width.saturating_sub(u16::from(show_scrollbar));
    let mut y = body.y;
    for (index, row) in rows.iter().enumerate().skip(*agent_scroll) {
        let height = row_heights[index].min(body.height);
        if y.saturating_add(height) > body.bottom() {
            break;
        }
        let rect = Rect::new(body.x, y, content_width, height);
        render_row(buffer, rect, row, hits);
        y = y
            .saturating_add(height)
            .saturating_add(if index + 1 < rows.len() {
                config.agents.row_gap
            } else {
                0
            });
    }

    if show_scrollbar {
        let track = Rect::new(body.right().saturating_sub(1), body.y, 1, body.height);
        hits.agent_scrollbar = track;
        super::scroll::render_list_scrollbar(buffer, track, metrics, &config.palette);
    }
}

pub(super) fn agent_rows(
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    machine: Option<&str>,
) -> Vec<AgentRow> {
    ordered_agent_pane_ids(snapshot, config.agent_panel_sort)
        .into_iter()
        .filter(|pane_id| !crate::nirvana_subagents::is_seat_worker_pane(pane_id))
        .filter_map(|pane_id| agent_row(snapshot, &pane_id, config, machine))
        .collect()
}

pub(super) fn agent_row(
    snapshot: &ClientShellSnapshot,
    pane_id: &str,
    config: &ClientShellConfig,
    machine: Option<&str>,
) -> Option<AgentRow> {
    let agent = snapshot
        .agents
        .iter()
        .find(|agent| agent.pane_id == pane_id)?;
    let workspace = snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == agent.workspace_id)?;
    let tab = snapshot.tabs.iter().find(|tab| tab.tab_id == agent.tab_id);
    let pane = snapshot
        .panes
        .iter()
        .find(|pane| pane.pane_id == agent.pane_id);
    let tab_count = snapshot
        .tabs
        .iter()
        .filter(|candidate| candidate.workspace_id == agent.workspace_id)
        .count();
    let tab_label = tab
        .filter(|tab| tab_count > 1 || tab.custom_label)
        .map(|tab| tab.label.as_str());
    let agent_label = agent
        .display_agent
        .as_deref()
        .or(agent.name.as_deref())
        .or(agent.agent.as_deref())
        .or(agent.title.as_deref());
    let labels = agent
        .state_labels
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>();
    let tokens = agent.tokens.iter().cloned().collect::<HashMap<_, _>>();
    let state_text = labels
        .get(status_text(agent.agent_status))
        .map(String::as_str)
        .unwrap_or_else(|| sidebar_status_text(agent.agent_status));
    let canonical_agent = agent
        .agent
        .as_deref()
        .and_then(crate::detect::parse_agent_label);
    let rows = crate::ui::sidebar_agent_rows(
        &config.agents,
        crate::ui::AgentTokenContext {
            machine,
            workspace: &workspace.label,
            tab: tab_label,
            pane: agent
                .title
                .as_deref()
                .or_else(|| pane.and_then(|pane| pane.label.as_deref())),
            agent_label,
            terminal_title: agent.terminal_title.as_deref(),
            terminal_title_stripped: agent.terminal_title_stripped.as_deref(),
            canonical_agent,
            tokens: &tokens,
        },
        state_text,
    );

    let folder = pane
        .and_then(|p| p.foreground_cwd.as_deref().or(p.cwd.as_deref()))
        .map(std::path::PathBuf::from);
    let agent_session = agent
        .agent_session
        .as_ref()
        .filter(|s| s.agent == "claude" && s.kind == crate::agent_resume::AgentSessionRefKind::Id)
        .map(|s| s.value.clone());
    let is_claude = canonical_agent == Some(crate::detect::Agent::Claude);
    crate::nirvana_subagents::update_pane_query(
        pane_id,
        folder,
        is_claude,
        agent_session,
        None,
        None,
    );

    let (subagents, subagents_overflow) = match crate::nirvana_subagents::cached(pane_id) {
        Some(view) => (view.subagents, view.overflow),
        None => (Vec::new(), 0),
    };

    Some(AgentRow {
        pane_id: agent.pane_id.clone(),
        status: agent.agent_status,
        focused: agent.focused,
        rows,
        subagents,
        subagents_overflow,
    })
}

pub(super) fn render_agent_row(
    buffer: &mut Buffer,
    rect: Rect,
    row: &AgentRow,
    config: &ClientShellConfig,
    mut hits: Option<&mut ShellHitMap>,
) {
    let palette = &config.palette;
    let row_style = if row.focused {
        Style::default().bg(palette.active_row_bg)
    } else {
        Style::default()
    };
    let name_style = if row.focused {
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(palette.subtext0)
            .add_modifier(Modifier::BOLD)
    };
    let status_style = Style::default().fg(status_color(row.status, palette));
    let secondary = Style::default().fg(palette.overlay0);
    let icon = (
        status_icon(row.status, config.status_indicators),
        Style::default().fg(status_color(row.status, palette)),
    );
    let rows = if row.rows.is_empty() {
        vec![vec![crate::ui::ResolvedToken {
            kind: crate::ui::ResolvedTokenKind::StateIcon,
            style: Default::default(),
        }]]
    } else {
        row.rows.clone()
    };

    let main_height = (rows.len() as u16).min(rect.height);
    if let Some(hits) = hits.as_mut() {
        hits.agents.push((
            Rect::new(rect.x, rect.y, rect.width, main_height),
            row.pane_id.clone(),
        ));
    }

    for (index, tokens) in rows.iter().take(main_height as usize).enumerate() {
        let indent = if index == 0 { 1 } else { 3 };
        let mut spans = vec![ratatui::text::Span::raw(" ".repeat(indent))];
        spans.extend(crate::ui::resolved_token_spans(
            tokens,
            icon,
            status_style,
            name_style,
            secondary,
            secondary,
            palette,
            rect.width.saturating_sub(indent as u16) as usize,
        ));
        Paragraph::new(Line::from(spans)).style(row_style).render(
            Rect::new(rect.x, rect.y + index as u16, rect.width, 1),
            buffer,
        );
    }

    let mut current_y = rect.y + main_height;
    let narrow = is_narrow_agent_panel(rect.width);
    let drop_version = rect.width < 30;
    let drop_elapsed = rect.width < 25;

    for sub in &row.subagents {
        if current_y >= rect.bottom() {
            break;
        }

        let is_done = sub.state == crate::nirvana_subagents::SubagentState::Done;
        let is_failed = sub.state == crate::nirvana_subagents::SubagentState::Failed;
        let is_finished = is_done || is_failed;

        let has_desc = !narrow && !sub.description.trim().is_empty();
        let sub_height = if has_desc { 2 } else { 1 };
        let actual_height = sub_height.min(rect.bottom().saturating_sub(current_y));

        if let Some(hits) = hits.as_mut() {
            let target = sub
                .target_pane_id
                .clone()
                .unwrap_or_else(|| row.pane_id.clone());
            hits.agents.push((
                Rect::new(rect.x, current_y, rect.width, actual_height),
                target,
            ));
        }

        let base_indent = if sub.depth == 1 { 3 } else { 1 };
        let mark_str = sub.provider.mark();
        let mark_color = match sub.provider.colour() {
            Some((r, g, b)) => ratatui::style::Color::Rgb(r, g, b),
            None => palette.overlay1,
        };
        let mut mark_style = Style::default().fg(mark_color);
        if is_finished {
            mark_style = mark_style.add_modifier(Modifier::DIM);
        }

        let model_display = if drop_version {
            crate::nirvana_subagents::short_model(&sub.model)
        } else {
            sub.model.clone()
        };

        let text_style = if is_finished {
            Style::default()
                .fg(palette.overlay0)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(palette.subtext0)
        };

        let model_style = if is_finished {
            Style::default()
                .fg(palette.overlay0)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(palette.overlay1)
        };

        let elapsed_str = crate::nirvana_subagents::format_elapsed(sub.elapsed_secs);
        let tokens_str = crate::nirvana_subagents::format_tokens(sub.tokens);

        let left_len =
            base_indent + 2 + sub.name.chars().count() + 1 + model_display.chars().count();
        let right_len = if is_done || is_failed {
            2 + if !drop_elapsed {
                elapsed_str.chars().count()
            } else {
                0
            }
        } else {
            (if !drop_elapsed {
                elapsed_str.chars().count() + 1
            } else {
                0
            }) + tokens_str.chars().count()
        };

        let total_needed = left_len + right_len;
        let gap = if (rect.width as usize) > total_needed {
            (rect.width as usize) - total_needed
        } else {
            1
        };

        let mut spans = Vec::new();
        spans.push(ratatui::text::Span::raw(" ".repeat(base_indent)));
        spans.push(ratatui::text::Span::styled(
            format!("{mark_str} "),
            mark_style,
        ));
        spans.push(ratatui::text::Span::styled(
            format!("{} ", sub.name),
            text_style,
        ));
        spans.push(ratatui::text::Span::styled(model_display, model_style));

        spans.push(ratatui::text::Span::raw(" ".repeat(gap)));

        if is_done {
            let mut check_style = Style::default().fg(palette.green);
            if is_finished {
                check_style = check_style.add_modifier(Modifier::DIM);
            }
            spans.push(ratatui::text::Span::styled("\u{2713} ", check_style));
            if !drop_elapsed {
                spans.push(ratatui::text::Span::styled(elapsed_str, model_style));
            }
        } else if is_failed {
            let mut cross_style = Style::default().fg(palette.red);
            if is_finished {
                cross_style = cross_style.add_modifier(Modifier::DIM);
            }
            spans.push(ratatui::text::Span::styled("\u{2717} ", cross_style));
            if !drop_elapsed {
                spans.push(ratatui::text::Span::styled(elapsed_str, model_style));
            }
        } else {
            if !drop_elapsed {
                spans.push(ratatui::text::Span::styled(
                    format!("{elapsed_str} "),
                    model_style,
                ));
            }
            let mut token_style = Style::default().fg(palette.subtext0);
            if is_finished {
                token_style = token_style.add_modifier(Modifier::DIM);
            }
            spans.push(ratatui::text::Span::styled(tokens_str, token_style));
        }

        Paragraph::new(Line::from(spans))
            .render(Rect::new(rect.x, current_y, rect.width, 1), buffer);
        current_y += 1;

        if has_desc && current_y < rect.bottom() {
            let desc_indent = if sub.depth == 1 { 5 } else { 3 };
            let max_desc_width = (rect.width as usize).saturating_sub(desc_indent);
            let desc_fitted =
                crate::client::shell::nirvana_spaces::fit(sub.description.trim(), max_desc_width);
            let desc_style = Style::default()
                .fg(palette.overlay0)
                .add_modifier(Modifier::DIM);
            let desc_spans = vec![
                ratatui::text::Span::raw(" ".repeat(desc_indent)),
                ratatui::text::Span::styled(desc_fitted, desc_style),
            ];
            Paragraph::new(Line::from(desc_spans))
                .render(Rect::new(rect.x, current_y, rect.width, 1), buffer);
            current_y += 1;
        }
    }

    if row.subagents_overflow > 0 && current_y < rect.bottom() {
        if let Some(hits) = hits.as_mut() {
            hits.agents.push((
                Rect::new(rect.x, current_y, rect.width, 1),
                row.pane_id.clone(),
            ));
        }
        let more_style = Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::DIM);
        let more_spans = vec![
            ratatui::text::Span::raw("   "),
            ratatui::text::Span::styled(format!("+{} more", row.subagents_overflow), more_style),
        ];
        Paragraph::new(Line::from(more_spans))
            .render(Rect::new(rect.x, current_y, rect.width, 1), buffer);
    }
}

fn put_text(buffer: &mut Buffer, x: u16, y: u16, width: u16, text: &str, style: Style) {
    for (offset, character) in text.chars().take(width as usize).enumerate() {
        if let Some(cell) = buffer.cell_mut((x + offset as u16, y)) {
            cell.set_char(character).set_style(style);
        }
    }
}

fn display_width(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

fn sidebar_status_text(status: crate::api::schema::AgentStatus) -> &'static str {
    use crate::api::schema::AgentStatus;
    match status {
        AgentStatus::Blocked => "blocked",
        AgentStatus::Done => "done",
        AgentStatus::Working => "working",
        AgentStatus::Idle | AgentStatus::Unknown => "idle",
    }
}
