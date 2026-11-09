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

/// Finished rows (nirvana/plans/2026-11-09-finished-rows.md): whether a pane is a worker's or a job's. Its label is
/// `<seat>·<order>` or `job·<name>` (`nirvana_dead::parse_label`; `nirvana_worker::open_pane` and
/// `nirvana_job::open_job_pane` name the pane before the worker starts), or the subagents poller read an order seated
/// in it (`nirvana_subagents::is_seat_worker_pane`, known only once the poller has read that run). herdr detects
/// the `claude -p` a worker runs as an agent; the owner's screenshot of 2026-11-09 showed the worker of
/// `quad-v5-size-colour` twice, as "Claude S5.5 idle" under its space and as its order's row. Such a pane is shown
/// once, as its order's row with the order's state; never as a space agent row or an agents-panel row of its own.
pub(super) fn is_worker_pane(snapshot: &ClientShellSnapshot, pane_id: &str) -> bool {
    snapshot.panes.iter().any(|pane| {
        pane.pane_id == pane_id
            && pane
                .label
                .as_deref()
                .and_then(crate::nirvana_dead::parse_label)
                .is_some()
    }) || crate::nirvana_subagents::is_seat_worker_pane(pane_id)
}

/// The agents running in one space, in the snapshot's order. A worker's or a job's pane is not one (`is_worker_pane`).
pub(super) fn space_agents(
    snapshot: &ClientShellSnapshot,
    workspace_id: &str,
    indicators: crate::config::StatusIndicatorStyle,
) -> Vec<SpaceAgent> {
    let raw: Vec<SpaceAgent> = snapshot
        .agents
        .iter()
        .filter(|agent| agent.workspace_id == workspace_id)
        .filter(|agent| !is_worker_pane(snapshot, &agent.pane_id))
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

/// The ladder a space row is fitted by (plan 26 S1), widest first: whether `who` is padded to the space's longest,
/// whether the short model shows, and whether the state word is the full one (`working`) or the short one (`work`).
/// Past the last step a row is `who` alone, in the state's colour.
pub(super) const LADDER: [(bool, bool, bool); 6] = [
    (true, true, true),
    (true, true, false),
    (false, true, false),
    (true, false, true),
    (true, false, false),
    (false, false, false),
];

/// One agent's row at one ladder step: its text, and the cells its `who` and its model end at, for the colours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LadderRow {
    pub(super) line: String,
    pub(super) who_end: usize,
    pub(super) model_end: usize,
}

pub(super) fn ladder_row(agent: &SpaceAgent, step: usize, max_who: usize) -> LadderRow {
    let cells = |s: &str| unicode_width::UnicodeWidthStr::width(s);
    let Some(&(pad, model, full)) = LADDER.get(step) else {
        let end = cells(&agent.who);
        return LadderRow {
            line: agent.who.clone(),
            who_end: end,
            model_end: end,
        };
    };
    let mut line = if pad {
        format!("{:<max_who$}", agent.who)
    } else {
        agent.who.clone()
    };
    let who_end = cells(&line);
    if model && !agent.model.is_empty() {
        line.push(' ');
        line.push_str(&agent.model);
    }
    let model_end = cells(&line);
    let word = if full {
        status_word(agent.status)
    } else {
        short_status_word(agent.status)
    };
    if !word.is_empty() {
        line.push(' ');
        line.push_str(word);
    }
    LadderRow {
        line,
        who_end,
        model_end,
    }
}

/// The first ladder step at which every one of the space's rows fits `width`, or `LADDER.len()` when none does. One
/// step per space, not per row: a row that fits earlier would otherwise read `working` beside its neighbour's `work`
/// and lose the padding that lines the columns up (plan 26 review R4).
pub(super) fn ladder_step(agents: &[SpaceAgent], max_who: usize, width: usize) -> usize {
    (0..LADDER.len())
        .find(|&step| {
            agents.iter().filter(|a| !a.is_folded).all(|a| {
                unicode_width::UnicodeWidthStr::width(ladder_row(a, step, max_who).line.as_str())
                    <= width
            })
        })
        .unwrap_or(LADDER.len())
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
    // Rows stop one cell short of the area's edge. At the default `sidebar_width: 26` the list's area is 25 cells,
    // which leaves 19 cells after the icon, and 14 on an indented worktree row.
    let right = area.right().saturating_sub(1);
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
        .take(shown)
        .filter(|a| !a.is_folded)
        .map(|a| a.who.chars().count())
        .max()
        .unwrap_or(0);
    // S1: one ladder step for the whole space, so its rows use the same words and line up.
    let step = ladder_step(&agents[..shown], max_who, width.saturating_sub(2));
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
            } else if hovered_pane == Some(agent.pane_id.as_str()) && !agent.task.is_empty() {
                // S2: The chat title shows on hover, as the owner chose.
                let who_part = format!("{:<max_who$}", agent.who);
                let full = format!("{who_part}  {}", agent.task);
                let line = fit(&full, w_avail);
                let who_cells =
                    (max_who + 2).min(unicode_width::UnicodeWidthStr::width(line.as_str()));
                put_text(buffer, x + 2, y, w, &line, task_style);
                put_text(buffer, x + 2, y, who_cells as u16, &line, model_style);
            } else {
                let row = ladder_row(agent, step, max_who);
                if step >= LADDER.len() {
                    // Past the ladder: who alone, in the state's colour.
                    put_text(buffer, x + 2, y, w, &fit(&row.line, w_avail), state_style);
                } else {
                    put_text(buffer, x + 2, y, w, &row.line, state_style);
                    put_text(
                        buffer,
                        x + 2,
                        y,
                        row.model_end as u16,
                        &row.line,
                        model_style,
                    );
                    put_text(
                        buffer,
                        x + 2,
                        y,
                        row.who_end as u16,
                        &row.line,
                        if highlighted { task_style } else { model_style },
                    );
                }
            }
        }
    }
}

/// Plan 38: whether anything under a space is working: one of its agents is Working or Blocked (its Nirvana Claude,
/// a plain Claude, Codex, agy, a local pane, a seat worker's pane), or one of its live worker rows is at work
/// (`workers`, the rows the list already read this frame; `SpaceWorker::working`). herdr's own space status cannot be asked: it ranks Done above Working. A
/// collapsed worktree group's parent counts its hidden children's agents, as `displayed_workspace_status` does; their
/// workers are not read while they are hidden, so they do not count (no new reads for the sparkle).
pub(super) fn space_working(
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
    status: crate::api::schema::AgentStatus,
    collapsed_groups: &std::collections::HashSet<String>,
    workers: &[SpaceWorker],
) -> bool {
    use crate::api::schema::AgentStatus;
    let busy = |status: AgentStatus| matches!(status, AgentStatus::Working | AgentStatus::Blocked);
    if busy(status) || workers.iter().any(|worker| worker.working) {
        return true;
    }
    let group = workspace
        .worktree
        .as_ref()
        .filter(|worktree| !worktree.is_linked_worktree)
        .filter(|worktree| collapsed_groups.contains(&worktree.key))
        .map(|worktree| worktree.key.as_str());
    let member = |workspace_id: &str| {
        workspace_id == workspace.workspace_id
            || group.is_some_and(|key| {
                snapshot.workspaces.iter().any(|candidate| {
                    candidate.workspace_id == workspace_id
                        && candidate
                            .worktree
                            .as_ref()
                            .is_some_and(|member| member.key == key && member.is_linked_worktree)
                })
            })
    };
    snapshot
        .agents
        .iter()
        .any(|agent| busy(agent.agent_status) && member(&agent.workspace_id))
}

/// The space's marker and the status whose colour it takes. Working shows the sparkle's `frame` in the working
/// colour. Blocked keeps herdr's own mark (`×`, red): a space waiting on the owner matters more than one at work.
/// Anything else is today's marker.
pub(super) fn space_marker(
    status: crate::api::schema::AgentStatus,
    working: bool,
    indicators: crate::config::StatusIndicatorStyle,
    frame: usize,
) -> (&'static str, crate::api::schema::AgentStatus) {
    use crate::api::schema::AgentStatus;
    if working && status != AgentStatus::Blocked {
        (
            crate::nirvana_spaces::sparkle_glyph(frame),
            AgentStatus::Working,
        )
    } else {
        (status_icon(status, indicators), status)
    }
}

/// Plan 38 review S3: the marker of a space in a list that does not draw its worker rows (the narrow sidebar, the
/// machines list), by the same `space_working`. `read_workers` reads the workers the spaces list would show, from the
/// same cache; a remote machine's spaces pass false, as this window's cache holds this machine's journals only.
pub(super) fn listed_space_marker(
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
    status: crate::api::schema::AgentStatus,
    collapsed_groups: &std::collections::HashSet<String>,
    read_workers: bool,
    indicators: crate::config::StatusIndicatorStyle,
    frame: usize,
) -> (&'static str, crate::api::schema::AgentStatus) {
    let workers = if read_workers {
        space_workers(snapshot, &workspace.workspace_id, |agent| {
            super::agent_sidebar::ask_subagents(snapshot, agent)
        })
    } else {
        Vec::new()
    };
    let working = space_working(snapshot, workspace, status, collapsed_groups, &workers);
    space_marker(status, working, indicators, frame)
}

/// Plan 30: a space shows all its live workers up to this many; past it, `WORKERS_CUT_TO` rows and "+N more".
pub(super) const WORKERS_SHOWN: usize = 4;
pub(super) const WORKERS_CUT_TO: usize = 3;

/// How many of a space's live workers get a row.
pub(super) fn workers_shown(count: usize) -> usize {
    if count > WORKERS_SHOWN {
        WORKERS_CUT_TO
    } else {
        count
    }
}

/// The rows a space's live workers take: one per worker shown, and the "+N more" row.
pub(super) fn workers_height(count: usize) -> usize {
    workers_shown(count) + usize::from(count > WORKERS_SHOWN)
}

/// One live worker under its space (plan 30): the order's name, the short model and the elapsed time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SpaceWorker {
    pub(super) name: String,
    pub(super) model: String,
    pub(super) elapsed: String,
    /// Plan 38 review S4: whether the worker is at work, so its space sparkles. A live row is also a Deferred order
    /// that waits on a window's reset, or one whose own pane is gone; those keep their row and do not sparkle. Since
    /// the dead-panes fix a row is live only while a supervisor holds the order's lock (`nirvana_dead::order_live`), so
    /// an Asked order or a worker that died with the server no longer keeps a row here, counting for days.
    pub(super) working: bool,
}

/// The live workers of the space's agents, in the agents panel's own order. `view` is the panel's cached data for an
/// agent's pane (`agent_sidebar::ask_subagents` in the sidebar, which also keeps the pane asked for). Only the space's
/// own agents are asked for, and not a seat worker's pane, which `agent_rows` leaves out of the panel too. Live is the
/// panel's live group, so a finished, blocked, stopped or failed worker never shows here (plan 30 §2).
pub(super) fn space_workers(
    snapshot: &ClientShellSnapshot,
    workspace_id: &str,
    view: impl Fn(&ClientShellAgent) -> Option<crate::nirvana_subagents::SubagentsView>,
) -> Vec<SpaceWorker> {
    let subagents: Vec<crate::nirvana_subagents::Subagent> = snapshot
        .agents
        .iter()
        .filter(|agent| agent.workspace_id == workspace_id)
        .filter(|agent| !is_worker_pane(snapshot, &agent.pane_id))
        .filter_map(view)
        .flat_map(|view| view.subagents)
        .collect();
    super::agent_sidebar::group_subagents_by_run(&subagents)
        .iter()
        .flat_map(|group| group.live.iter())
        .map(|sub| {
            let (model, elapsed) = super::agent_sidebar::live_worker_model_and_elapsed(sub);
            let pane_alive = sub.target_pane_id.as_deref().is_none_or(|pane| {
                snapshot
                    .panes
                    .iter()
                    .any(|candidate| candidate.pane_id == pane)
            });
            SpaceWorker {
                name: sub.name.clone(),
                model,
                elapsed,
                working: !sub.waiting && pane_alive,
            }
        })
        .collect()
}

/// A worker's row: the text, and the cells its name and its model end at, counted on the text as drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkerLine {
    pub(super) line: String,
    pub(super) name_end: usize,
    pub(super) model_end: usize,
}

/// A worker's row text fitted to `width` cells: `<name> <model> <elapsed>`. The name is cut as the agents panel cuts
/// it, by `fit`, so the model and the time stay whole. Below the tail's width the tail is cut too, and the colour ends
/// are clamped to what was drawn, so a cut time never takes the model's colour (plan 30 review).
pub(super) fn worker_line(worker: &SpaceWorker, width: usize) -> WorkerLine {
    let cells = |s: &str| unicode_width::UnicodeWidthStr::width(s);
    let tail = format!(" {} {}", worker.model, worker.elapsed);
    let budget = width.saturating_sub(unicode_width::UnicodeWidthStr::width(tail.as_str()));
    let name = fit(&worker.name, budget);
    let line = fit(&format!("{name}{tail}"), width);
    let drawn = cells(&line);
    WorkerLine {
        name_end: cells(&name).min(drawn),
        model_end: (cells(&name) + 1 + cells(&worker.model)).min(drawn),
        line,
    }
}

/// Draws the space's live workers from `area.y` down, under its agents: a dim tree marker in the agents' icon column,
/// then the row, so they read as that space's workers.
pub(super) fn render_space_workers(
    buffer: &mut Buffer,
    area: Rect,
    workers: &[SpaceWorker],
    indented: bool,
    last_child: bool,
    palette: &Palette,
) {
    let x = area.x.saturating_add(if indented { 8 } else { 3 });
    // The agents' rule: rows stop one cell short of the area's edge.
    let right = area.right().saturating_sub(1);
    let width = usize::from(right.saturating_sub(x));
    if width <= 2 {
        return;
    }
    let text_width = width - 2;
    let dim = Style::default()
        .fg(palette.overlay0)
        .add_modifier(Modifier::DIM);
    let name_style = Style::default().fg(palette.subtext0);
    let model_style = Style::default().fg(palette.overlay1);
    let elapsed_style = Style::default().fg(palette.overlay0);
    // The agents' own "+N more" row is in the model's colour (`render_space_agents`).
    let more_style = model_style;
    let shown = workers_shown(workers.len());
    let rows = workers_height(workers.len());
    for index in 0..rows {
        let y = area.y.saturating_add(index as u16);
        if y >= area.bottom() {
            break;
        }
        if indented && !last_child {
            put_text(
                buffer,
                area.x,
                y,
                4,
                "   │",
                Style::default().fg(palette.overlay0),
            );
        }
        let marker = if index + 1 == rows { "└" } else { "├" };
        put_text(buffer, x, y, 1, marker, dim);
        let Some(worker) = workers.get(index).filter(|_| index < shown) else {
            let more = format!("+{} more", workers.len() - shown);
            put_text(
                buffer,
                x + 2,
                y,
                text_width as u16,
                &fit(&more, text_width),
                more_style,
            );
            continue;
        };
        let row = worker_line(worker, text_width);
        // Layered as the agents' rows are: the whole line, then the model and the name over it in their colours.
        put_text(
            buffer,
            x + 2,
            y,
            text_width as u16,
            &row.line,
            elapsed_style,
        );
        put_text(
            buffer,
            x + 2,
            y,
            row.model_end as u16,
            &row.line,
            model_style,
        );
        put_text(buffer, x + 2, y, row.name_end as u16, &row.line, name_style);
    }
}

/// Plan 38 (nirvana/plans/2026-10-31-38-space-working-spinner.md): a space sparkles while anything under it is working.
#[cfg(test)]
mod sparkle_tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use super::*;
    use crate::api::schema::AgentStatus;
    use crate::client::endpoint::{
        ClientEndpointId, ClientEndpointStatus, ProfileId, SavedSshEndpoint,
    };
    use crate::nirvana_spaces::{
        note_sparkle_drawn, set_sparkle_clock_for_test, sparkle_due, sparkle_frame_at,
        sparkle_glyph, sparkle_timer_delay, Kind, SparkleGate, SPARKLE_FRAME, SPARKLE_FRAMES,
    };
    use crate::nirvana_subagents::{
        set_view_for_test, Subagent, SubagentProvider, SubagentState, SubagentsView,
    };
    use crate::protocol::{PaneSurfacePane, SurfaceRect};

    fn agent(pane: &str, workspace: &str, status: AgentStatus, nirvana: bool) -> ClientShellAgent {
        let mut tokens = vec![
            ("nirvana_kind".into(), "claude".into()),
            ("nirvana_model".into(), "Claude Opus 5.5".into()),
        ];
        if nirvana {
            tokens.push(("nirvana_role".into(), "nirvana".into()));
        }
        ClientShellAgent {
            pane_id: pane.into(),
            workspace_id: workspace.into(),
            tab_id: "tab_1".into(),
            name: None,
            display_agent: None,
            agent: Some("claude".into()),
            title: None,
            terminal_title: None,
            terminal_title_stripped: None,
            agent_status: status,
            state_change_seq: 1,
            state_labels: Vec::new(),
            tokens,
            focused: false,
            agent_session: None,
        }
    }

    /// A live seat order under `pane`, as the owner's screenshot's `quad-pegs…` at 56s.
    fn live_worker(pane: &str) -> HashMap<String, SubagentsView> {
        worker_with(pane, |_| {})
    }

    /// `live_worker`, changed by `change`.
    fn worker_with(
        pane: &str,
        change: impl FnOnce(&mut Subagent),
    ) -> HashMap<String, SubagentsView> {
        let mut worker = Subagent {
            id: "order-quad-pegs".into(),
            parent_id: None,
            name: "quad-pegs".into(),
            description: String::new(),
            provider: SubagentProvider::Claude,
            model: "Sonnet 5.5".into(),
            state: SubagentState::Running,
            elapsed_secs: 56,
            tokens: None,
            ended_at: None,
            depth: 0,
            target_pane_id: None,
            run_id: Some("run-20261031-000000".into()),
            run_event_time: Some(1000),
            waiting: false,
        };
        change(&mut worker);
        HashMap::from([(
            pane.to_string(),
            SubagentsView {
                total_count: 1,
                subagents: vec![worker],
                overflow: 0,
            },
        )])
    }

    /// The pane surface: one pane no test agent is in, so no agent's Done reads as seen (the window shows a Done agent
    /// whose pane is on the screen as Idle, `endpoint_agent_state::acknowledge_surface`).
    fn surface() -> PaneSurfaceFrame {
        let surface_buffer = Buffer::with_lines(["LIVE", "PANE"]);
        let rect = SurfaceRect {
            x: 0,
            y: 0,
            width: 4,
            height: 2,
        };
        PaneSurfaceFrame {
            boot_id: "boot-1".into(),
            projection_revision: 1,
            surface_revision: 1,
            frame: FrameData::from_ratatui_buffer_with_hyperlinks(&surface_buffer, None, &[]),
            panes: vec![PaneSurfacePane {
                pane_id: "pane_shown".into(),
                content_revision: 0,
                rect,
                inner_rect: rect,
                scrollbar_rect: None,
                scroll: None,
                focused: true,
                mouse_reporting: false,
                sgr_pixel_mouse: false,
                alternate_screen_active: false,
                pixel_width: 0,
                pixel_height: 0,
            }],
            splits: Vec::new(),
            popup: None,
            graphics: crate::protocol::SurfaceGraphicsScene::default(),
        }
    }

    /// One space per `(status, agents)`, named `space-1`, `space-2`, ...
    fn spaces(list: Vec<(AgentStatus, Vec<ClientShellAgent>)>) -> ClientShellSnapshot {
        let mut snap = super::super::tests::snapshot();
        let template = snap.workspaces[0].clone();
        snap.workspaces.clear();
        for (index, (status, agents)) in list.into_iter().enumerate() {
            let mut workspace = template.clone();
            workspace.workspace_id = format!("ws_{}", index + 1);
            workspace.number = index + 1;
            workspace.label = format!("space-{}", index + 1);
            workspace.focused = index == 0;
            workspace.agent_status = status;
            snap.workspaces.push(workspace);
            snap.agents.extend(agents);
        }
        snap
    }

    /// The first row of each space at the sparkle clock's `at`, with `view` as the panel's cached workers. The symbols
    /// style, so Done reads `✓` as in the owner's screenshot.
    fn first_rows(
        snap: &ClientShellSnapshot,
        view: Option<HashMap<String, SubagentsView>>,
        at: Duration,
    ) -> Vec<String> {
        first_rows_collapsed(snap, view, at, &[])
    }

    /// `first_rows` with the worktree groups `collapsed` folded.
    fn first_rows_collapsed(
        snap: &ClientShellSnapshot,
        view: Option<HashMap<String, SubagentsView>>,
        at: Duration,
        collapsed: &[&str],
    ) -> Vec<String> {
        let (frame, rects) = compose_at(snap, view, at, collapsed);
        row_texts(&frame, &rects)
    }

    /// The composed frame and each space's rectangle.
    fn compose_at(
        snap: &ClientShellSnapshot,
        view: Option<HashMap<String, SubagentsView>>,
        at: Duration,
        collapsed: &[&str],
    ) -> (FrameData, Vec<Rect>) {
        compose_with(snap, view, at, 106, |state| {
            state
                .collapsed_groups
                .extend(collapsed.iter().map(|key| key.to_string()));
        })
    }

    /// The window as the owner sees it: `snap` seen working first (see below), `cols` wide, after `setup`.
    fn state_for(
        snap: &ClientShellSnapshot,
        setup: impl FnOnce(&mut ClientShellState),
    ) -> ClientShellState {
        let mut config = Config::default();
        config.ui.status_indicators = crate::config::StatusIndicatorStyle::Symbols;
        let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
        setup(&mut state);
        // The window shows Done only for an agent it saw working (`endpoint_agent_state`), so each Done agent is
        // working in the snapshot before.
        let mut before = snap.clone();
        for agent in &mut before.agents {
            if agent.agent_status == AgentStatus::Done {
                agent.agent_status = AgentStatus::Working;
            }
        }
        state.set_snapshot(Box::new(before));
        let mut after = snap.clone();
        after.revision += 1;
        for agent in &mut after.agents {
            agent.state_change_seq += 1;
        }
        // The surface matches the snapshot drawn, so `compose` draws the whole window (`render_shell`), not the
        // placeholder it draws while it waits for one (`compose_unavailable`).
        let mut pane_surface = surface();
        pane_surface.projection_revision = after.revision;
        state.set_snapshot(Box::new(after));
        state.set_pane_surface(pane_surface);
        assert!(
            state.pane_surface.is_some(),
            "the surface is the snapshot's"
        );
        state
    }

    /// `compose_at`, `cols` wide, after `setup`.
    fn compose_with(
        snap: &ClientShellSnapshot,
        view: Option<HashMap<String, SubagentsView>>,
        at: Duration,
        cols: u16,
        setup: impl FnOnce(&mut ClientShellState),
    ) -> (FrameData, Vec<Rect>) {
        let mut state = state_for(snap, setup);
        compose_state(&mut state, view, at, cols)
    }

    fn compose_state(
        state: &mut ClientShellState,
        view: Option<HashMap<String, SubagentsView>>,
        at: Duration,
        cols: u16,
    ) -> (FrameData, Vec<Rect>) {
        set_view_for_test(view);
        set_sparkle_clock_for_test(Some(at));
        let frame = state.compose(cols, 30).expect("composed frame").frame;
        set_view_for_test(None);
        let rects = state.hits.workspaces.iter().map(|hit| hit.rect).collect();
        (frame, rects)
    }

    /// The text of each space's first row in `frame`.
    fn row_texts(frame: &FrameData, rects: &[Rect]) -> Vec<String> {
        let rows: Vec<Vec<String>> = frame
            .cells
            .chunks(frame.width as usize)
            .map(|row| row.iter().map(|cell| cell.symbol.clone()).collect())
            .collect();
        rects
            .iter()
            .map(|rect| {
                rows[usize::from(rect.y)][usize::from(rect.x)..usize::from(rect.right())]
                    .concat()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    /// A moment inside frame `index` on the clock.
    fn at(index: u32) -> Duration {
        SPARKLE_FRAME * index + Duration::from_millis(10)
    }

    /// The marker cell: the first cell after the row's one-cell indent.
    fn marker(row: &str) -> String {
        row.chars().skip(1).take(1).collect()
    }

    fn sparkles(row: &str) -> bool {
        SPARKLE_FRAMES.iter().any(|frame| row.contains(frame))
    }

    /// §1 and §4: the owner's screenshot. A finished Nirvana Claude made the space read `✓` while its worker ran; a
    /// working worker pane, or a live worker row, makes it sparkle.
    #[test]
    fn a_space_with_a_done_nirvana_claude_and_a_working_worker_sparkles() {
        let pane = spaces(vec![(
            AgentStatus::Done,
            vec![
                agent("pane_1", "ws_1", AgentStatus::Done, true),
                agent("pane_2", "ws_1", AgentStatus::Working, false),
            ],
        )]);
        let rows = first_rows(&pane, None, at(4));
        assert_eq!(marker(&rows[0]), SPARKLE_FRAMES[4], "{rows:?}");

        let worker = spaces(vec![(
            AgentStatus::Done,
            vec![agent("pane_1", "ws_1", AgentStatus::Done, true)],
        )]);
        let rows = first_rows(&worker, Some(live_worker("pane_1")), at(4));
        assert_eq!(marker(&rows[0]), SPARKLE_FRAMES[4], "{rows:?}");
    }

    /// §2.2: the sparkle is in the space's working colour: the colour of its working agent's icon, not its done one's.
    #[test]
    fn the_sparkle_takes_the_working_colour() {
        let snap = spaces(vec![(
            AgentStatus::Done,
            vec![
                agent("pane_1", "ws_1", AgentStatus::Done, true),
                agent("pane_2", "ws_1", AgentStatus::Working, false),
            ],
        )]);
        let (frame, rects) = compose_at(&snap, None, at(2), &[]);
        let cell =
            |x: u16, y: u16| &frame.cells[usize::from(y) * frame.width as usize + usize::from(x)];
        let space = rects[0];
        let marker = cell(space.x + 1, space.y);
        assert_eq!(marker.symbol, SPARKLE_FRAMES[2]);
        // The agents' rows follow the space's own rows (its name, then its branch), in the snapshot's order, their icon
        // three cells in (`render_space_agents`).
        let icons: Vec<&crate::protocol::CellData> = (space.y + 1..space.bottom())
            .map(|y| cell(space.x + 3, y))
            .filter(|icon| icon.symbol == Kind::Claude.icon())
            .collect();
        let [done_icon, working_icon] = icons[..] else {
            panic!("two agent rows: {}", icons.len());
        };
        assert_eq!(marker.fg, working_icon.fg);
        assert_ne!(marker.fg, done_icon.fg);
    }

    /// §2.1: Done, Idle and Unknown are not working; the marker is today's.
    #[test]
    fn a_space_with_only_done_and_idle_agents_shows_todays_marker() {
        let snap = spaces(vec![(
            AgentStatus::Done,
            vec![
                agent("pane_1", "ws_1", AgentStatus::Done, true),
                agent("pane_2", "ws_1", AgentStatus::Idle, false),
                agent("pane_3", "ws_1", AgentStatus::Unknown, false),
            ],
        )]);
        for index in 0..8 {
            let rows = first_rows(&snap, None, at(index));
            assert_eq!(marker(&rows[0]), "\u{2713}", "{rows:?}");
            assert!(!sparkles(&rows[0]), "{rows:?}");
        }
    }

    /// §2.1: a folded worktree group's parent stands for its hidden children, as herdr's own folded status does: a
    /// working agent in a hidden child makes the parent sparkle, and only while the group is folded.
    #[test]
    fn a_folded_parent_sparkles_for_its_hidden_childs_working_agent() {
        let snap = spaces(vec![
            (
                AgentStatus::Done,
                vec![agent("pane_1", "ws_1", AgentStatus::Done, true)],
            ),
            (
                AgentStatus::Working,
                vec![agent("pane_2", "ws_2", AgentStatus::Working, false)],
            ),
        ]);
        // ws_1 the parent checkout, ws_2 its linked worktree; `more` is a second checkout of the same repo, which
        // herdr lists as a parent of its own, never as the first parent's child.
        let group = |mut snap: ClientShellSnapshot, linked: &[bool]| {
            for (workspace, linked) in snap.workspaces.iter_mut().zip(linked) {
                workspace.worktree = Some(crate::protocol::ClientShellWorktree {
                    key: "repo".into(),
                    label: "repo".into(),
                    is_linked_worktree: *linked,
                });
            }
            snap
        };
        let folded = group(snap.clone(), &[false, true]);
        let rows = first_rows_collapsed(&folded, None, at(3), &["repo"]);
        assert_eq!(rows.len(), 1, "the child is hidden: {rows:?}");
        assert_eq!(marker(&rows[0]), SPARKLE_FRAMES[3], "{rows:?}");

        let mut done_child = folded.clone();
        done_child.agents[1].agent_status = AgentStatus::Idle;
        done_child.workspaces[1].agent_status = AgentStatus::Idle;
        let rows = first_rows_collapsed(&done_child, None, at(3), &["repo"]);
        assert!(!sparkles(&rows[0]), "{rows:?}");

        let mut more = spaces(vec![
            (
                AgentStatus::Done,
                vec![agent("pane_1", "ws_1", AgentStatus::Done, true)],
            ),
            (
                AgentStatus::Idle,
                vec![agent("pane_2", "ws_2", AgentStatus::Idle, false)],
            ),
            (
                AgentStatus::Working,
                vec![agent("pane_3", "ws_3", AgentStatus::Working, false)],
            ),
        ]);
        more = group(more, &[false, true, false]);
        let rows = first_rows_collapsed(&more, None, at(3), &["repo"]);
        assert_eq!(rows.len(), 2, "two parents, the child hidden: {rows:?}");
        assert!(
            !sparkles(&rows[0]),
            "the other checkout is not a child: {rows:?}"
        );
        assert_eq!(marker(&rows[1]), SPARKLE_FRAMES[3], "{rows:?}");
    }

    /// §2.1: Blocked keeps herdr's own mark, `×`: waiting on the owner matters more than working.
    #[test]
    fn a_blocked_space_keeps_its_own_mark() {
        let snap = spaces(vec![(
            AgentStatus::Blocked,
            vec![
                agent("pane_1", "ws_1", AgentStatus::Blocked, true),
                agent("pane_2", "ws_1", AgentStatus::Working, false),
            ],
        )]);
        let rows = first_rows(&snap, None, at(0));
        assert_eq!(marker(&rows[0]), "\u{00D7}", "{rows:?}");
    }

    /// §2.2: `✧ ✦ ✶ ✷ ✸ ✷ ✶ ✦` in order, four a second, then round again.
    #[test]
    fn the_frames_cycle_in_order_on_the_clock() {
        assert_eq!(
            SPARKLE_FRAMES,
            [
                "\u{2727}", "\u{2726}", "\u{2736}", "\u{2737}", "\u{2738}", "\u{2737}", "\u{2736}",
                "\u{2726}"
            ]
        );
        assert_eq!(SPARKLE_FRAME, Duration::from_millis(250));
        assert_eq!(sparkle_frame_at(Duration::from_millis(249)), 0);
        assert_eq!(sparkle_frame_at(Duration::from_millis(250)), 1);
        assert_eq!(sparkle_frame_at(Duration::from_secs(2)), 0);
        let snap = spaces(vec![(
            AgentStatus::Working,
            vec![agent("pane_1", "ws_1", AgentStatus::Working, true)],
        )]);
        for index in 0..16u32 {
            let rows = first_rows(&snap, None, at(index));
            let expected = SPARKLE_FRAMES[index as usize % SPARKLE_FRAMES.len()];
            assert_eq!(marker(&rows[0]), expected, "frame {index}: {rows:?}");
        }
    }

    /// §2.2: one clock, so every working space shows the same frame.
    #[test]
    fn two_spaces_share_a_phase() {
        let snap = spaces(vec![
            (
                AgentStatus::Done,
                vec![
                    agent("pane_1", "ws_1", AgentStatus::Done, true),
                    agent("pane_2", "ws_1", AgentStatus::Working, false),
                ],
            ),
            (
                AgentStatus::Working,
                vec![agent("pane_3", "ws_2", AgentStatus::Working, true)],
            ),
        ]);
        for index in [0u32, 1, 4] {
            let rows = first_rows(&snap, None, at(index));
            let markers: Vec<String> = rows.iter().map(|row| marker(row)).collect();
            let expected = SPARKLE_FRAMES[index as usize];
            assert_eq!(markers, [expected, expected], "{rows:?}");
        }
    }

    /// §2.3: with nothing working the timer schedules no repaint at all; with a working space it repaints on each new
    /// frame, and only a drawing that still sparkles keeps it going.
    #[test]
    fn no_repaint_is_scheduled_when_nothing_is_working() {
        let default = Duration::from_millis(100);
        let idle = spaces(vec![(
            AgentStatus::Done,
            vec![agent("pane_1", "ws_1", AgentStatus::Done, true)],
        )]);
        first_rows(&idle, None, at(0));
        for index in 0..8 {
            set_sparkle_clock_for_test(Some(at(index)));
            assert!(!sparkle_due(), "frame {index}");
            assert_eq!(sparkle_timer_delay(default), default, "frame {index}");
        }

        let busy = spaces(vec![(
            AgentStatus::Working,
            vec![agent("pane_1", "ws_1", AgentStatus::Working, true)],
        )]);
        first_rows(&busy, None, at(0));
        set_sparkle_clock_for_test(Some(at(0)));
        assert!(!sparkle_due(), "the frame drawn is still showing");
        set_sparkle_clock_for_test(Some(at(1)));
        assert!(sparkle_due(), "the clock moved on");

        // The worker ends: the next drawing settles the marker and stops the timer's repaints.
        first_rows(&idle, None, at(1));
        set_sparkle_clock_for_test(Some(at(2)));
        assert!(!sparkle_due());
        assert_eq!(sparkle_timer_delay(default), default);
        set_sparkle_clock_for_test(None);
    }

    /// Review S2: asking does not change the drawing, so a tick whose compose draws nothing (no surface yet, a stale
    /// snapshot) leaves the repaint due and the next tick asks again; the sparkle cannot freeze on a frame. A window
    /// that draws no sidebar at all clears it.
    #[test]
    fn a_tick_that_draws_nothing_leaves_the_repaint_due() {
        let busy = spaces(vec![(
            AgentStatus::Working,
            vec![agent("pane_1", "ws_1", AgentStatus::Working, true)],
        )]);
        first_rows(&busy, None, at(0));
        set_sparkle_clock_for_test(Some(at(1)));
        for tick in 0..3 {
            assert!(sparkle_due(), "tick {tick}");
        }
        // A phone-narrow window draws the mobile header and no sidebar (its own spaces list, `mobile.rs`, keeps
        // herdr's marker): nothing sparkles, nothing is due.
        let (frame, _) = compose_with(&busy, None, at(1), 40, |_| {});
        let text: String = frame
            .cells
            .iter()
            .map(|cell| cell.symbol.as_str())
            .collect();
        assert!(!sparkles(&text), "{text}");
        set_sparkle_clock_for_test(Some(at(2)));
        assert!(!sparkle_due());
        set_sparkle_clock_for_test(None);
    }

    /// Review S2 and S5: the gate itself, the same type the window's static holds. A new gate is not due; a drawing
    /// that sparkled is due on every later frame until the next drawing, however often it is asked; a drawing with
    /// nothing sparkling is never due.
    #[test]
    fn the_gate_is_due_only_on_another_frame_of_a_sparkling_drawing() {
        let gate = SparkleGate::new();
        for frame in 0..8 {
            assert!(!gate.due(frame), "new gate, frame {frame}");
        }
        gate.note(Some(3));
        assert!(!gate.due(3));
        assert!(gate.due(4));
        assert!(gate.due(4), "asking leaves the drawing as it was");
        gate.note(None);
        assert!(!gate.due(4));
        assert!(!gate.due(5));
    }

    /// Review N1: while a drawing sparkles, the timer wakes on the next frame's boundary, so every frame shows its
    /// 250 ms (on the plain 100 ms tick they showed 300 and 200 ms in turn); otherwise it waits its default.
    #[test]
    fn the_timer_wakes_on_the_next_frame_while_sparkling() {
        let default = Duration::from_millis(100);
        let gate = SparkleGate::new();
        let elapsed = SPARKLE_FRAME * 5 + Duration::from_millis(190);
        assert_eq!(gate.delay(elapsed, default), default);
        gate.note(Some(5));
        assert_eq!(gate.delay(elapsed, default), Duration::from_millis(60));
        assert_eq!(gate.delay(SPARKLE_FRAME * 5, default), default);
        assert_eq!(
            gate.delay(
                SPARKLE_FRAME * 5 + Duration::from_millis(10),
                Duration::from_secs(1)
            ),
            Duration::from_millis(240)
        );

        // The window's own gate, through the sidebar.
        let busy = spaces(vec![(
            AgentStatus::Working,
            vec![agent("pane_1", "ws_1", AgentStatus::Working, true)],
        )]);
        first_rows(&busy, None, at(0));
        set_sparkle_clock_for_test(Some(elapsed));
        assert_eq!(sparkle_timer_delay(default), Duration::from_millis(60));
        note_sparkle_drawn(None);
        assert_eq!(sparkle_timer_delay(default), default);
        set_sparkle_clock_for_test(None);
    }

    /// Review S3: the narrow sidebar shows the same marker as the spaces list: the owner's case sparkles there too,
    /// through a working pane and through a live worker, and keeps the timer going; a done space reads `✓`.
    #[test]
    fn the_narrow_sidebar_sparkles_as_the_spaces_list_does() {
        let narrow = |snap: &ClientShellSnapshot,
                      view: Option<HashMap<String, SubagentsView>>,
                      index: u32| {
            // Folded after the surface is in, as the owner's toggle does.
            let mut state = state_for(snap, |_| {});
            state.sidebar_collapsed = true;
            let (frame, rects) = compose_state(&mut state, view, at(index), 106);
            let row = row_texts(&frame, &rects).remove(0);
            // The narrow row is the space's number in two cells, then its marker.
            row.chars().nth(2).map(String::from).unwrap_or_default()
        };
        let pane = spaces(vec![(
            AgentStatus::Done,
            vec![
                agent("pane_1", "ws_1", AgentStatus::Done, true),
                agent("pane_2", "ws_1", AgentStatus::Working, false),
            ],
        )]);
        assert_eq!(narrow(&pane, None, 4), SPARKLE_FRAMES[4]);
        set_sparkle_clock_for_test(Some(at(5)));
        assert!(sparkle_due(), "the narrow sidebar keeps the sparkle going");

        let worker = spaces(vec![(
            AgentStatus::Done,
            vec![agent("pane_1", "ws_1", AgentStatus::Done, true)],
        )]);
        assert_eq!(
            narrow(&worker, Some(live_worker("pane_1")), 2),
            SPARKLE_FRAMES[2]
        );
        assert_eq!(narrow(&worker, None, 2), "\u{2713}");
        set_sparkle_clock_for_test(Some(at(3)));
        assert!(
            !sparkle_due(),
            "a narrow sidebar with nothing working schedules nothing"
        );
        set_sparkle_clock_for_test(None);
    }

    fn remote_profile() -> SavedSshEndpoint {
        SavedSshEndpoint {
            id: ProfileId::parse("0123456789abcdef0123456789abcdef").unwrap(),
            label: "Build".into(),
            target: "dev@build.example".into(),
            session: "agents".into(),
            enabled: true,
        }
    }

    /// Each space row of the machines list (this machine's spaces, then the remote's), expanded or narrow, with the
    /// machine it belongs to. The remote machine has one space whose one agent, `pane_r`, has `remote_status`; it is
    /// online, or reconnecting with its last snapshot.
    fn machines_rows(
        local: &ClientShellSnapshot,
        remote_status: AgentStatus,
        remote_online: bool,
        view: Option<HashMap<String, SubagentsView>>,
        narrow: bool,
        index: u32,
    ) -> Vec<(ClientEndpointId, String)> {
        let profile = remote_profile();
        let remote_id = ClientEndpointId::Ssh(profile.id.clone());
        let mut state = state_for(local, |state| {
            state.set_endpoint_catalog(&[profile]);
            state.set_endpoint_status(&remote_id, ClientEndpointStatus::Online);
        });
        let mut remote = spaces(vec![(
            remote_status,
            vec![agent("pane_r", "ws_1", remote_status, false)],
        )]);
        remote.boot_id = "remote-boot".into();
        remote.workspaces[0].label = "remote-space".into();
        state.set_endpoint_snapshot(&remote_id, Box::new(remote));
        if !remote_online {
            state.set_endpoint_status(&remote_id, ClientEndpointStatus::Reconnecting);
        }
        state.sidebar_collapsed = narrow;
        let (frame, rects) = compose_state(&mut state, view, at(index), 106);
        let ids: Vec<ClientEndpointId> = state
            .hits
            .workspaces
            .iter()
            .map(|hit| hit.endpoint_id.clone())
            .collect();
        ids.into_iter().zip(row_texts(&frame, &rects)).collect()
    }

    /// Review S3: the machines list, expanded and narrow, sparkles as the spaces list does: this machine's space for
    /// its live worker, a remote space for its working agent; a remote space never for this window's worker cache,
    /// which holds this machine's journals only, nor while its machine is not online.
    #[test]
    fn the_machines_list_sparkles_as_the_spaces_list_does() {
        let local = spaces(vec![(
            AgentStatus::Done,
            vec![agent("pane_1", "ws_1", AgentStatus::Done, true)],
        )]);
        for narrow in [false, true] {
            let mut view = live_worker("pane_1");
            view.extend(live_worker("pane_r"));
            let rows = machines_rows(&local, AgentStatus::Idle, true, Some(view), narrow, 3);
            assert_eq!(rows.len(), 2, "narrow {narrow}: {rows:?}");
            assert_eq!(rows[0].0, ClientEndpointId::Local, "{rows:?}");
            assert!(sparkles(&rows[0].1), "narrow {narrow}: {rows:?}");
            assert!(!sparkles(&rows[1].1), "narrow {narrow}: {rows:?}");
            set_sparkle_clock_for_test(Some(at(4)));
            assert!(sparkle_due(), "narrow {narrow}");

            let rows = machines_rows(&local, AgentStatus::Working, true, None, narrow, 3);
            assert!(!sparkles(&rows[0].1), "narrow {narrow}: {rows:?}");
            assert!(sparkles(&rows[1].1), "narrow {narrow}: {rows:?}");
            // A machine that is not online shows its last snapshot, which may be stale: no sparkle.
            let rows = machines_rows(&local, AgentStatus::Working, false, None, narrow, 3);
            assert_eq!(rows.len(), 2, "narrow {narrow}: {rows:?}");
            assert!(!sparkles(&rows[1].1), "narrow {narrow}: {rows:?}");

            let rows = machines_rows(&local, AgentStatus::Idle, true, None, narrow, 3);
            assert!(rows.iter().all(|(_, row)| !sparkles(row)), "{rows:?}");
            set_sparkle_clock_for_test(Some(at(4)));
            assert!(!sparkle_due(), "narrow {narrow}");
        }
        set_sparkle_clock_for_test(None);
    }

    /// Review S4: a live row is not always a worker at work. An order that waits (Asked, Deferred) or whose own pane
    /// is gone keeps its row and does not make the space sparkle; one whose pane is there does.
    #[test]
    fn a_waiting_or_paneless_worker_does_not_sparkle() {
        let snap = spaces(vec![(
            AgentStatus::Done,
            vec![agent("pane_1", "ws_1", AgentStatus::Done, true)],
        )]);
        let waiting = worker_with("pane_1", |worker| worker.waiting = true);
        let (frame, rects) = compose_at(&snap, Some(waiting), at(2), &[]);
        let rows = row_texts(&frame, &rects);
        assert_eq!(marker(&rows[0]), "\u{2713}", "{rows:?}");
        let text: String = frame
            .cells
            .iter()
            .map(|cell| cell.symbol.as_str())
            .collect();
        assert!(text.contains("quad-pegs"), "the worker's row stays");

        let gone = worker_with("pane_1", |worker| {
            worker.target_pane_id = Some("pane_gone".into());
        });
        let rows = first_rows(&snap, Some(gone), at(2));
        assert_eq!(marker(&rows[0]), "\u{2713}", "{rows:?}");

        let there = worker_with("pane_1", |worker| {
            worker.target_pane_id = Some("pane_1".into());
        });
        let rows = first_rows(&snap, Some(there), at(2));
        assert_eq!(marker(&rows[0]), SPARKLE_FRAMES[2], "{rows:?}");
    }

    /// §2.4: every frame is one cell wide by the width function the sidebar uses, so none falls back to `◐`.
    #[test]
    fn every_frame_is_one_cell_wide() {
        for (index, frame) in SPARKLE_FRAMES.iter().enumerate() {
            assert_eq!(unicode_width::UnicodeWidthStr::width(*frame), 1, "{frame}");
            assert_eq!(sparkle_glyph(index), *frame);
        }
    }

    /// Finished rows (2026-11-09): the owner's screenshot listed "Claude S5.5 idle" under 3D-Printer Flow above the
    /// worker's own row "quad-v5… S5.5 4m50s": herdr detects the worker's `claude -p` in its pane `wP:p4N`
    /// (`sonnet·quad-v5-size-colour`). The space's agent rows, the agents panel's rows and the space's worker rows
    /// now leave out every worker's and job's pane by its label, so the worker shows once, as its order's row.
    #[test]
    fn a_worker_shows_once_as_its_orders_row() {
        crate::nirvana_subagents::set_worker_panes_for_test(Some(std::collections::HashSet::new()));
        let mut worker = agent("wP:p4N", "ws_1", AgentStatus::Idle, false);
        worker.tokens = vec![
            ("nirvana_kind".into(), "claude".into()),
            ("nirvana_model".into(), "Claude Sonnet 5.5".into()),
        ];
        let job = agent("wP:p9", "ws_1", AgentStatus::Idle, false);
        let plain = agent("wP:p7", "ws_1", AgentStatus::Idle, false);
        let mut snap = spaces(vec![(
            AgentStatus::Idle,
            vec![
                agent("wP:p1", "ws_1", AgentStatus::Idle, true),
                worker,
                job,
                plain,
            ],
        )]);
        let template = snap.panes[0].clone();
        for (pane, label) in [
            ("wP:p1", None),
            ("wP:p4N", Some("sonnet\u{b7}quad-v5-size-colour")),
            ("wP:p9", Some("job\u{b7}bs209-build")),
            ("wP:p7", Some("notes")),
        ] {
            let mut entry = template.clone();
            entry.pane_id = pane.into();
            entry.workspace_id = "ws_1".into();
            entry.label = label.map(str::to_owned);
            snap.panes.push(entry);
        }
        let agents: Vec<String> = space_agents(
            &snap,
            "ws_1",
            crate::config::StatusIndicatorStyle::default(),
        )
        .into_iter()
        .map(|agent| agent.pane_id)
        .collect();
        assert_eq!(agents, vec!["wP:p1".to_owned(), "wP:p7".to_owned()]);
        let config = ClientShellConfig::from_config(&Config::default());
        let panel: Vec<String> = super::super::agent_sidebar::agent_rows(&snap, &config, None)
            .into_iter()
            .map(|row| row.pane_id)
            .collect();
        assert_eq!(panel, vec!["wP:p1".to_owned(), "wP:p7".to_owned()]);
        // The worker's own row comes from the orchestrator's pane, and only from there: the worker pane is not asked.
        let workers = space_workers(&snap, "ws_1", |agent| {
            (agent.pane_id == "wP:p1" || agent.pane_id == "wP:p4N")
                .then(|| live_worker(&agent.pane_id).remove(&agent.pane_id))
                .flatten()
        });
        assert_eq!(
            workers.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(),
            vec!["quad-pegs"],
            "the worker shows once"
        );
        crate::nirvana_subagents::set_worker_panes_for_test(None);
    }
}
