//! Nirvana fork, plan 30: a space's live workers show under its agents in the spaces view
//! (nirvana/plans/2026-10-26-30-live-workers-in-spaces.md L1, L2).

use std::collections::HashMap;

use super::*;
use crate::api::schema::AgentStatus;
use crate::client::shell::nirvana_spaces::{
    render_space_workers, space_workers, worker_line, SpaceWorker,
};
use crate::nirvana_subagents::{
    set_view_for_test, Subagent, SubagentProvider, SubagentState, SubagentsView,
};
use ratatui::layout::Rect;

fn nirvana(pane: &str, workspace: &str) -> ClientShellAgent {
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
        agent_status: AgentStatus::Idle,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: vec![
            ("nirvana_kind".into(), "claude".into()),
            ("nirvana_model".into(), "Claude Opus 5.5".into()),
            ("nirvana_role".into(), "nirvana".into()),
        ],
        focused: false,
        agent_session: None,
    }
}

/// A seat-order row as `read_seat_orders` makes it: a run id, the formatted model, the elapsed seconds.
fn order(name: &str, state: SubagentState, elapsed_secs: u64) -> Subagent {
    Subagent {
        id: format!("order-{name}"),
        parent_id: None,
        name: name.into(),
        description: String::new(),
        provider: SubagentProvider::Claude,
        model: "Sonnet 5.5".into(),
        state,
        elapsed_secs,
        tokens: None,
        ended_at: (state != SubagentState::Running).then_some(1000),
        depth: 0,
        target_pane_id: None,
        run_id: Some("run-20261023-024041".into()),
        run_event_time: Some(1000),
    }
}

fn view_of(pane: &str, subagents: Vec<Subagent>) -> HashMap<String, SubagentsView> {
    HashMap::from([(
        pane.to_string(),
        SubagentsView {
            total_count: subagents.len(),
            subagents,
            overflow: 0,
        },
    )])
}

/// The owner's screenshot: one live `v8-build`, then finished, blocked, stopped and failed rows.
fn moto_rows(elapsed_secs: u64) -> Vec<Subagent> {
    vec![
        order("v7-build", SubagentState::Done, 400),
        order("v8-build", SubagentState::Running, elapsed_secs),
        order("reveal-build", SubagentState::Stopped, 300),
        order("lint-fix", SubagentState::Blocked, 200),
        order("perf-pass", SubagentState::Failed, 100),
    ]
}

/// Composes the shell with `agents` and the panel's cached `view`, and returns the rows of each space, cut to the
/// space's rectangle so the agents panel below never counts.
fn compose_spaces(
    snapshot: ClientShellSnapshot,
    view: Option<HashMap<String, SubagentsView>>,
) -> (Vec<Rect>, Vec<Vec<String>>) {
    set_view_for_test(view);
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 30).expect("composed frame").frame;
    set_view_for_test(None);
    let rows: Vec<Vec<String>> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.clone()).collect())
        .collect();
    let rects: Vec<Rect> = state.hits.workspaces.iter().map(|hit| hit.rect).collect();
    let spaces = rects
        .iter()
        .map(|rect| {
            (rect.y..rect.bottom())
                .map(|y| {
                    rows[usize::from(y)][usize::from(rect.x)..usize::from(rect.right())]
                        .concat()
                        .trim_end()
                        .to_string()
                })
                .collect()
        })
        .collect();
    (rects, spaces)
}

fn one_space(agents: Vec<ClientShellAgent>) -> ClientShellSnapshot {
    let mut snap = snapshot();
    snap.agents = agents;
    snap
}

/// L2: "moto projeect should have v8-build s5.5 xMxxS". One live worker among finished ones is one row, in the agents
/// panel's words; the elapsed time follows the panel's data.
#[test]
fn a_space_shows_exactly_its_one_live_worker_in_the_panels_format() {
    let snap = one_space(vec![nirvana("pane_1", "ws_1")]);
    let (rects, spaces) = compose_spaces(snap.clone(), Some(view_of("pane_1", moto_rows(67))));
    let (before_rects, before) = compose_spaces(snap.clone(), None);
    let rows = &spaces[0];
    assert_eq!(rects[0].height, before_rects[0].height + 1, "{rows:?}");
    assert_eq!(
        rows.last().unwrap(),
        "   \u{2514} v8-build S5.5 1m07s",
        "{rows:?}"
    );
    // Everything above the worker row is the space as it was.
    assert_eq!(&rows[..rows.len() - 1], &before[0][..], "{rows:?}");
    for finished in ["v7-build", "reveal", "lint-fix", "perf-pass"] {
        assert!(rows.iter().all(|row| !row.contains(finished)), "{rows:?}");
    }

    let (_, later) = compose_spaces(snap, Some(view_of("pane_1", moto_rows(68))));
    assert_eq!(later[0].last().unwrap(), "   \u{2514} v8-build S5.5 1m08s");
}

/// §2: "With no live worker, the space looks exactly as it does today."
#[test]
fn a_space_with_no_live_worker_renders_as_before() {
    let snap = one_space(vec![nirvana("pane_1", "ws_1")]);
    let finished: Vec<Subagent> = moto_rows(67)
        .into_iter()
        .filter(|s| s.state != SubagentState::Running)
        .collect();
    let (rects, spaces) = compose_spaces(snap.clone(), Some(view_of("pane_1", finished)));
    let (before_rects, before) = compose_spaces(snap, None);
    assert_eq!(rects, before_rects);
    assert_eq!(spaces, before);
}

/// §2: "past 4 live workers, show 3 and a `+N more` row." Four still show whole.
#[test]
fn five_live_workers_show_three_rows_and_two_more() {
    let snap = one_space(vec![nirvana("pane_1", "ws_1")]);
    let live = |count: usize| {
        (1..=count)
            .map(|n| {
                order(
                    &format!("w{n}-build"),
                    SubagentState::Running,
                    60 * n as u64,
                )
            })
            .collect::<Vec<_>>()
    };
    let (_, spaces) = compose_spaces(snap.clone(), Some(view_of("pane_1", live(5))));
    let rows = &spaces[0];
    let tail = &rows[rows.len() - 4..];
    // The panel's order: the longest running first.
    assert_eq!(
        tail,
        [
            "   \u{251C} w5-build S5.5 5m",
            "   \u{251C} w4-build S5.5 4m",
            "   \u{251C} w3-build S5.5 3m",
            "   \u{2514} +2 more",
        ],
        "{rows:?}"
    );

    let (_, spaces) = compose_spaces(snap, Some(view_of("pane_1", live(4))));
    let rows = &spaces[0];
    assert!(rows.iter().all(|row| !row.contains("more")), "{rows:?}");
    assert_eq!(
        rows.last().unwrap(),
        "   \u{2514} w1-build S5.5 1m",
        "{rows:?}"
    );
}

/// §2 tests: "workers of another space never show under this one."
#[test]
fn workers_of_another_space_never_show_under_this_one() {
    let mut snap = one_space(vec![nirvana("pane_1", "ws_1"), nirvana("pane_2", "ws_2")]);
    let mut other = snap.workspaces[0].clone();
    other.workspace_id = "ws_2".into();
    other.number = 2;
    other.label = "moto".into();
    other.focused = false;
    snap.workspaces.push(other);
    let view = view_of("pane_2", moto_rows(67));

    assert!(space_workers(&snap, "ws_1", |pane| view.get(pane).cloned()).is_empty());
    assert_eq!(
        space_workers(&snap, "ws_2", |pane| view.get(pane).cloned()),
        [SpaceWorker {
            name: "v8-build".into(),
            model: "S5.5".into(),
            elapsed: "1m07s".into(),
        }]
    );

    let (_, spaces) = compose_spaces(snap, Some(view));
    assert!(
        spaces[0].iter().all(|row| !row.contains("v8-build")),
        "{spaces:?}"
    );
    assert!(
        spaces[1].iter().any(|row| row.contains("v8-build")),
        "{spaces:?}"
    );
}

/// §2: "names are cut the way the agents panel cuts them": by `fit`, keeping the model and the time whole.
#[test]
fn a_long_worker_name_is_cut_and_keeps_model_and_time() {
    let worker = SpaceWorker {
        name: "v8-build".into(),
        model: "S5.5".into(),
        elapsed: "1m07s".into(),
    };
    assert_eq!(worker_line(&worker, 19), "v8-build S5.5 1m07s");
    assert_eq!(worker_line(&worker, 15), "v8-\u{2026} S5.5 1m07s");
}

/// A worktree space that is not its parent's last child keeps the parent's tree line beside its worker rows, as its
/// agent rows do.
#[test]
fn a_worktree_space_keeps_its_tree_line_beside_worker_rows() {
    let workers = vec![SpaceWorker {
        name: "v8-build".into(),
        model: "S5.5".into(),
        elapsed: "1m07s".into(),
    }];
    let palette = ClientShellConfig::from_config(&Config::default()).palette;
    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 25, 1));
    render_space_workers(
        &mut buf,
        Rect::new(0, 0, 25, 1),
        &workers,
        true,
        false,
        &palette,
    );
    let row: String = (0..25)
        .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
        .collect();
    assert_eq!(
        row.trim_end(),
        "   \u{2502}    \u{2514} v8\u{2026} S5.5 1m07s"
    );
    // The marker is dim; the name, the model and the time keep their own colours.
    let cell = |x: u16| buf.cell((x, 0)).unwrap().clone();
    assert_eq!(cell(8).fg, palette.overlay0);
    assert!(cell(8).modifier.contains(Modifier::DIM));
    assert_eq!(cell(10).fg, palette.subtext0);
    assert_eq!(cell(14).fg, palette.overlay1);
    assert_eq!(cell(19).fg, palette.overlay0);
}

/// The worker rows count in the list's height, so live workers alone can make the spaces list scroll.
#[test]
fn live_workers_make_the_spaces_list_scroll() {
    let scroll_with = |workers: usize| {
        let mut snap = snapshot();
        for number in 2..=4 {
            let mut other = snap.workspaces[0].clone();
            other.workspace_id = format!("ws_{number}");
            other.label = format!("space {number}");
            other.number = number;
            other.focused = false;
            snap.workspaces.push(other);
        }
        let mut views = HashMap::new();
        for space in ["ws_1", "ws_2", "ws_3", "ws_4"] {
            let pane = format!("{space}_pane");
            snap.agents.push(nirvana(&pane, space));
            let live = (0..workers)
                .map(|n| order(&format!("w{n}"), SubagentState::Running, 60))
                .collect();
            views.extend(view_of(&pane, live));
        }
        set_view_for_test(Some(views));
        let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
        state.set_snapshot(Box::new(snap));
        state.set_pane_surface(surface());
        state.compose(106, 30).expect("composed frame");
        set_view_for_test(None);
        state.hits.workspace_max_scroll
    };
    // Three workers and "+2 more" are four more rows in each space: the four spaces no longer fit.
    assert!(scroll_with(5) > scroll_with(0), "{}", scroll_with(0));
}
