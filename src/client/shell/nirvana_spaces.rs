//! Nirvana fork, feature 3: under each space in the spaces list, one row per agent running there: the
//! agent's icon, the task's summarized title, then its provider and model. Two rows until 0.33.2; the owner,
//! 2026-10-21: "Can we instead pu that in a single line like Icon of the model then Nirvana - Claude Sonnet 5.5".
//!
//! ```text
//!  ● clockedin-sorting
//!    ✻ Nirvana - Claude Sonnet 5.5
//! ```
//!
//! When the row does not fit, the model's first word goes first, since the icon already names the provider
//! ("✻ Nirvana - Sonnet 5.5"); then the task is cut, so the model stays readable ("✻ Nirva… - Sonnet 5.5");
//! only when the task would keep fewer than four cells is the end of the row cut instead.
//! The icon stands where Nirvana drew a small green circle and keeps herdr's status colour, so the shape
//! says which agent and the colour says what it is doing. The server sends the kind and the model as
//! agent tokens (`crate::nirvana_spaces`); an older server sends none and the row falls back to the
//! agent's herdr name.

use std::collections::HashMap;

use super::render::put_text;
use super::*;
use crate::nirvana_spaces::{task_title, Kind, KIND_TOKEN, MODEL_TOKEN, ROLE_TOKEN};
use crate::protocol::ClientShellAgent;

/// One row per agent: icon, task, model.
pub(super) const ROWS_PER_AGENT: usize = 1;
/// A space shows this many agents, then one "+N more" row. Ten agents would make one space twenty rows
/// tall and push every other space out of the list (upstream's scrollbar test has ten in one space); the
/// agents panel below still lists them all.
pub(super) const AGENTS_SHOWN: usize = 3;

/// The rows a space's agents take.
pub(super) fn agents_height(count: usize) -> usize {
    count.min(AGENTS_SHOWN) * ROWS_PER_AGENT + usize::from(count > AGENTS_SHOWN)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SpaceAgent {
    pub(super) pane_id: String,
    pub(super) icon: &'static str,
    pub(super) who: String,
    pub(super) model: String,
    pub(super) task: String,
    pub(super) status: crate::api::schema::AgentStatus,
    pub(super) focused: bool,
    pub(super) is_folded: bool,
}

/// Folds two or more idle agents of the same kind that have no task into a single row (plan 26 §4).
pub(super) fn fold_space_agents(agents: Vec<SpaceAgent>) -> Vec<SpaceAgent> {
    let mut idle_counts: HashMap<String, usize> = HashMap::new();
    for a in &agents {
        if a.status == crate::api::schema::AgentStatus::Idle && a.task.is_empty() {
            *idle_counts.entry(a.who.clone()).or_default() += 1;
        }
    }

    let mut out = Vec::new();
    let mut folded_seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for a in agents {
        let is_idle_no_task =
            a.status == crate::api::schema::AgentStatus::Idle && a.task.is_empty();
        let count = idle_counts.get(&a.who).copied().unwrap_or(0);
        if is_idle_no_task && count >= 2 {
            if !folded_seen.contains(&a.who) {
                folded_seen.insert(a.who.clone());
                out.push(SpaceAgent {
                    pane_id: a.pane_id.clone(),
                    icon: a.icon,
                    who: format!("{count} {} panes", a.who),
                    model: String::new(),
                    task: String::new(),
                    status: crate::api::schema::AgentStatus::Idle,
                    focused: false,
                    is_folded: true,
                });
            }
        } else {
            out.push(a);
        }
    }
    out
}

/// The agents running in one space, in the snapshot's order.
pub(super) fn space_agents(
    snapshot: &ClientShellSnapshot,
    workspace_id: &str,
    indicators: crate::config::StatusIndicatorStyle,
) -> Vec<SpaceAgent> {
    let raw: Vec<SpaceAgent> = snapshot
        .agents
        .iter()
        .filter(|agent| agent.workspace_id == workspace_id)
        .map(|agent| {
            let token = |key: &str| {
                agent
                    .tokens
                    .iter()
                    .find(|(name, _)| name == key)
                    .map(|(_, value)| value.as_str())
            };
            let kind = agent_kind(agent);
            let who = if token(ROLE_TOKEN).is_some() {
                "Nirvana".to_string()
            } else {
                match kind {
                    Some(Kind::Claude) => "Claude".to_string(),
                    Some(Kind::Codex) => "Codex".to_string(),
                    Some(Kind::Antigravity) => "agy".to_string(),
                    Some(Kind::Local) => "Local".to_string(),
                    None => agent
                        .display_agent
                        .clone()
                        .or_else(|| agent.agent.clone())
                        .or_else(|| agent.name.clone())
                        .unwrap_or_else(|| "agent".into()),
                }
            };
            let raw_model = token(MODEL_TOKEN)
                .map(str::to_owned)
                .or_else(|| kind.map(|kind| kind.provider().to_owned()))
                .or_else(|| agent.display_agent.clone())
                .or_else(|| agent.agent.clone())
                .or_else(|| agent.name.clone())
                .unwrap_or_else(|| "agent".into());
            let model = crate::nirvana_spaces::compact_model(&raw_model);
            SpaceAgent {
                pane_id: agent.pane_id.clone(),
                icon: kind.map_or_else(|| status_icon(agent.agent_status, indicators), Kind::icon),
                who,
                task: {
                    let t = task_title(agent.terminal_title_stripped.as_deref());
                    if t == "No task yet" {
                        String::new()
                    } else {
                        t
                    }
                },
                model,
                status: agent.agent_status,
                focused: agent.focused,
                is_folded: false,
            }
        })
        .collect();
    fold_space_agents(raw)
}

/// The agent's kind: the server's token, else its herdr name.
pub(super) fn agent_kind(agent: &ClientShellAgent) -> Option<Kind> {
    agent
        .tokens
        .iter()
        .find(|(name, _)| name == KIND_TOKEN)
        .and_then(|(_, value)| Kind::parse(value))
        .or_else(|| agent.agent.as_deref().and_then(Kind::from_agent_label))
}

/// `text` cut to `width` cells, ending in `…` when cut.
pub(super) fn fit(text: &str, width: usize) -> String {
    if unicode_width::UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for character in text.chars() {
        let cell = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
        if used + cell + 1 > width {
            break;
        }
        out.push(character);
        used += cell;
    }
    if width > 0 {
        out.push('…');
    }
    out
}

/// The text after an agent's icon, `<task> - <model>` fitted to `width` cells, and how many of its cells are
/// the task's, so the task and the model keep their own colours.
/// Preserved for backward compatibility and test controls (sl_row_keeps_provider_word, sl_row_cuts_the_model).
#[allow(dead_code)]
pub(super) fn agent_line(task: &str, model: &str, width: usize) -> (String, usize) {
    use unicode_width::UnicodeWidthStr;
    const SEPARATOR: &str = " - ";
    /// Fewer task cells than this and the task says nothing; the end of the row is cut instead.
    const MIN_TASK: usize = 4;
    let cells = |text: &str| UnicodeWidthStr::width(text);
    let full = format!("{task}{SEPARATOR}{model}");
    if cells(&full) <= width {
        return (full, cells(task));
    }
    let model = model.split_once(' ').map_or(model, |(_, rest)| rest);
    let budget = width.saturating_sub(cells(SEPARATOR) + cells(model));
    if budget >= MIN_TASK {
        let task = fit(task, budget);
        let task_cells = cells(&task);
        return (format!("{task}{SEPARATOR}{model}"), task_cells);
    }
    let line = fit(&format!("{task}{SEPARATOR}{model}"), width);
    let task_cells = cells(task).min(cells(&line));
    (line, task_cells)
}

pub(super) fn status_word(status: crate::api::schema::AgentStatus) -> &'static str {
    match status {
        crate::api::schema::AgentStatus::Idle => "idle",
        crate::api::schema::AgentStatus::Working => "working",
        crate::api::schema::AgentStatus::Blocked => "blocked",
        crate::api::schema::AgentStatus::Done => "done",
        crate::api::schema::AgentStatus::Unknown => "",
    }
}

pub(super) fn short_status_word(status: crate::api::schema::AgentStatus) -> &'static str {
    match status {
        crate::api::schema::AgentStatus::Idle => "idle",
        crate::api::schema::AgentStatus::Working => "work",
        crate::api::schema::AgentStatus::Blocked => "wait",
        crate::api::schema::AgentStatus::Done => "done",
        crate::api::schema::AgentStatus::Unknown => "",
    }
}

/// Draws the agents' rows from `area.y` down, inside `area`. `indented` is a worktree space under its
/// parent, whose rows carry the parent's tree line on the left unless it is the last child.
pub(super) fn render_space_agents(
    buffer: &mut Buffer,
    area: Rect,
    agents: &[SpaceAgent],
    indented: bool,
    last_child: bool,
    highlighted: bool,
    hovered_pane: Option<&str>,
    mut hits: Option<&mut super::state::ShellHitMap>,
    palette: &Palette,
) {
    let x = area.x.saturating_add(if indented { 8 } else { 3 });
    let right = if area.width == 25 {
        area.right().saturating_sub(1)
    } else {
        area.right().saturating_sub(2)
    };
    let width = usize::from(right.saturating_sub(x));
    let task_style = Style::default().fg(if highlighted {
        palette.text
    } else {
        palette.subtext0
    });
    let model_style = Style::default().fg(palette.overlay1);
    let tree_style = Style::default().fg(palette.overlay0);
    let shown = agents.len().min(AGENTS_SHOWN);
    let more_y = area.y.saturating_add((shown * ROWS_PER_AGENT) as u16);
    if agents.len() > shown && more_y < area.bottom() && width > 0 {
        let more = format!("+{} more", agents.len() - shown);
        put_text(
            buffer,
            x,
            more_y,
            width as u16,
            &fit(&more, width),
            model_style,
        );
    }
    let max_who = agents
        .iter()
        .filter(|a| !a.is_folded)
        .map(|a| a.who.chars().count())
        .max()
        .unwrap_or(0);
    for (index, agent) in agents.iter().take(shown).enumerate() {
        let y = area.y.saturating_add((index * ROWS_PER_AGENT) as u16);
        for row in 0..ROWS_PER_AGENT as u16 {
            if indented && !last_child && y + row < area.bottom() {
                put_text(buffer, area.x, y + row, 4, "   │", tree_style);
            }
        }
        if y >= area.bottom() || width == 0 {
            break;
        }

        let row_rect = Rect::new(x, y, (width as u16).max(1), 1);
        if let Some(h) = hits.as_deref_mut() {
            h.space_agents.push((row_rect, agent.pane_id.clone()));
        }

        put_text(
            buffer,
            x,
            y,
            1,
            agent.icon,
            Style::default()
                .fg(status_color(agent.status, palette))
                .add_modifier(Modifier::BOLD),
        );
        if width > 2 {
            let w = (width - 2) as u16;
            let w_avail = width - 2;
            let status_col = status_color(agent.status, palette);
            let state_style = Style::default().fg(status_col);
            if agent.is_folded {
                let full = format!("{}  {}", agent.who, status_word(agent.status));
                let line = fit(&full, w_avail);
                let who_len = unicode_width::UnicodeWidthStr::width(agent.who.as_str())
                    .min(unicode_width::UnicodeWidthStr::width(line.as_str()));
                put_text(buffer, x + 2, y, w, &line, state_style);
                put_text(buffer, x + 2, y, who_len as u16, &line, model_style);
            } else if hovered_pane == Some(agent.pane_id.as_str())
                && !agent.task.is_empty()
                && agent.task != "No task yet"
            {
                // S2: The chat title shows on hover, as the owner chose.
                let who_part = format!("{:<max_who$}", agent.who);
                let full = format!("{who_part}  {}", agent.task);
                let line = fit(&full, w_avail);
                let who_cells =
                    (max_who + 2).min(unicode_width::UnicodeWidthStr::width(line.as_str()));
                put_text(buffer, x + 2, y, w, &line, task_style);
                put_text(buffer, x + 2, y, who_cells as u16, &line, model_style);
            } else {
                // S1: The ladder, applied in order until the row fits:
                // 1. who, then short model, then full state word, single spaces, with who padded to max_who;
                // 2. the same with no padding;
                // 3. state words shortened: working to work, blocked to wait;
                // 4. the model dropped;
                // 5. the state word dropped. Its colour then goes on who.
                let who_padded = format!("{:<max_who$}", agent.who);
                let word = status_word(agent.status);
                let short_w = short_status_word(agent.status);
                let m = agent.model.as_str();

                let step1 = if m.is_empty() {
                    if word.is_empty() {
                        who_padded.clone()
                    } else {
                        format!("{who_padded} {word}")
                    }
                } else if word.is_empty() {
                    format!("{who_padded} {m}")
                } else {
                    format!("{who_padded} {m} {word}")
                };

                let step2 = if m.is_empty() {
                    if word.is_empty() {
                        agent.who.clone()
                    } else {
                        format!("{} {word}", agent.who)
                    }
                } else if word.is_empty() {
                    format!("{} {m}", agent.who)
                } else {
                    format!("{} {m} {word}", agent.who)
                };

                let step3 = if m.is_empty() {
                    if short_w.is_empty() {
                        agent.who.clone()
                    } else {
                        format!("{} {short_w}", agent.who)
                    }
                } else if short_w.is_empty() {
                    format!("{} {m}", agent.who)
                } else {
                    format!("{} {m} {short_w}", agent.who)
                };

                let step4 = if !word.is_empty()
                    && unicode_width::UnicodeWidthStr::width(
                        format!("{} {word}", agent.who).as_str(),
                    ) <= w_avail
                {
                    format!("{} {word}", agent.who)
                } else if !short_w.is_empty() {
                    format!("{} {short_w}", agent.who)
                } else {
                    agent.who.clone()
                };

                let cells = |s: &str| unicode_width::UnicodeWidthStr::width(s);

                if cells(&step1) <= w_avail {
                    let who_cells = max_who;
                    let model_end = if m.is_empty() {
                        who_cells
                    } else {
                        who_cells + 1 + cells(m)
                    };
                    put_text(buffer, x + 2, y, w, &step1, state_style);
                    put_text(buffer, x + 2, y, model_end as u16, &step1, model_style);
                    put_text(
                        buffer,
                        x + 2,
                        y,
                        who_cells as u16,
                        &step1,
                        if highlighted { task_style } else { model_style },
                    );
                } else if cells(&step2) <= w_avail {
                    let who_cells = cells(&agent.who);
                    let model_end = if m.is_empty() {
                        who_cells
                    } else {
                        who_cells + 1 + cells(m)
                    };
                    put_text(buffer, x + 2, y, w, &step2, state_style);
                    put_text(buffer, x + 2, y, model_end as u16, &step2, model_style);
                    put_text(
                        buffer,
                        x + 2,
                        y,
                        who_cells as u16,
                        &step2,
                        if highlighted { task_style } else { model_style },
                    );
                } else if cells(&step3) <= w_avail {
                    let who_cells = cells(&agent.who);
                    let model_end = if m.is_empty() {
                        who_cells
                    } else {
                        who_cells + 1 + cells(m)
                    };
                    put_text(buffer, x + 2, y, w, &step3, state_style);
                    put_text(buffer, x + 2, y, model_end as u16, &step3, model_style);
                    put_text(
                        buffer,
                        x + 2,
                        y,
                        who_cells as u16,
                        &step3,
                        if highlighted { task_style } else { model_style },
                    );
                } else if cells(&step4) <= w_avail {
                    let who_cells = cells(&agent.who);
                    put_text(buffer, x + 2, y, w, &step4, state_style);
                    put_text(
                        buffer,
                        x + 2,
                        y,
                        who_cells as u16,
                        &step4,
                        if highlighted { task_style } else { model_style },
                    );
                } else {
                    // Step 5: state word dropped, colour goes on who
                    let line = fit(&agent.who, w_avail);
                    put_text(buffer, x + 2, y, w, &line, state_style);
                }
            }
        }
    }
}
