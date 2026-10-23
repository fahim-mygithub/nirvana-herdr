//! Nirvana fork: each space lists its agents' task and model with the agent's icon
//! (nirvana/plans/2026-10-15-03-space-agents.md L1).

use super::*;

fn agent(pane: &str, kind: Option<&str>, tokens: &[(&str, &str)]) -> ClientShellAgent {
    ClientShellAgent {
        pane_id: pane.into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: None,
        display_agent: None,
        agent: kind.map(Into::into),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Idle,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: tokens
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        focused: false,
        agent_session: None,
    }
}

fn claude_sonnet() -> ClientShellAgent {
    let mut claude = agent(
        "pane_1",
        Some("claude"),
        &[
            ("nirvana_kind", "claude"),
            ("nirvana_model", "Claude Sonnet 5.5"),
        ],
    );
    claude.terminal_title_stripped = Some("Sort clocked-in".into());
    claude.agent_status = AgentStatus::Working;
    claude
}

/// The composed frame, its rows as text, and the first space's rectangle.
fn compose(agents: Vec<ClientShellAgent>) -> (ClientShellState, FrameData, Vec<String>, Rect) {
    let mut projected = snapshot();
    projected.agents = agents;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 30).expect("composed frame").frame;
    let rows = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    let space = state.hits.workspaces[0].rect;
    (state, frame, rows, space)
}

#[test]
fn a_space_lists_its_agents_task_and_model_with_the_agent_icon() {
    let (state, frame, rows, space) = compose(vec![claude_sonnet()]);
    let agent_y = usize::from(space.bottom() - 1);
    assert!(
        rows[agent_y].contains("\u{273B}")
            && rows[agent_y].contains("Claude")
            && rows[agent_y].contains("Sonnet")
            && rows[agent_y].contains("wo"),
        "agent row: {:?}",
        rows[agent_y]
    );
    // The icon stands where the green circle was and keeps the status colour: working is yellow.
    let icon_x = rows[agent_y].chars().position(|c| c == '\u{273B}').unwrap();
    assert_eq!(
        frame.cells[agent_y * frame.width as usize + icon_x].fg,
        crate::protocol::color_to_u32(state.config.palette.yellow)
    );
}

#[test]
fn each_kind_draws_its_own_icon_and_others_keep_the_dot() {
    let agents = vec![
        claude_sonnet(),
        agent(
            "pane_2",
            Some("codex"),
            &[("nirvana_kind", "codex"), ("nirvana_model", "GPT-5 Codex")],
        ),
        agent(
            "pane_3",
            Some("agy"),
            &[
                ("nirvana_kind", "antigravity"),
                ("nirvana_model", "Gemini 3.8 Flash"),
            ],
        ),
        agent(
            "pane_4",
            Some("claude"),
            &[
                ("nirvana_kind", "local"),
                ("nirvana_model", "Local qwen3:8b"),
            ],
        ),
        agent("pane_5", Some("pi"), &[]),
    ];
    // A space shows three agents, so the five are drawn in two windows.
    let mut agents = agents;
    let rest = agents.split_off(3);
    let text = [agents, rest]
        .into_iter()
        .map(|agents| compose(agents).2.join("\n"))
        .collect::<Vec<_>>()
        .join("\n");
    // At the default width each row keeps its icon, who, model and state.
    for (icon, who, model) in [
        ("\u{273B}", "Claude", "Sonnet"),
        ("\u{2756}", "Codex", "Codex"),
        ("\u{25B2}", "agy", "Flash"),
        ("\u{2302}", "Local", "qwen3:8b"),
        ("\u{25CB}", "pi", "idle"),
    ] {
        assert!(
            text.lines()
                .any(|row| row.contains(icon) && row.contains(who) && row.contains(model)),
            "missing {icon:?} with {who:?} {model:?} in\n{text}"
        );
    }
    assert!(!text.contains("No task yet"));
}

#[test]
fn an_older_server_without_tokens_still_names_the_agent() {
    let (_, _, rows, space) = compose(vec![agent("pane_1", Some("claude"), &[])]);
    let row = &rows[usize::from(space.bottom() - 1)];
    assert!(
        row.contains("\u{273B}") && row.contains("Claude") && row.contains("id"),
        "{row:?}"
    );
    assert!(!row.contains("No task yet"));
}

#[test]
fn each_agent_adds_one_row_to_its_space() {
    let (_, _, _, alone) = compose(Vec::new());
    let (_, _, _, one) = compose(vec![claude_sonnet()]);
    let mut second = claude_sonnet();
    second.pane_id = "pane_2".into();
    let (_, _, _, two) = compose(vec![claude_sonnet(), second]);
    assert_eq!(one.height, alone.height + 1);
    assert_eq!(two.height, alone.height + 2);
}

#[test]
fn a_long_title_is_cut_to_the_sidebar_with_an_ellipsis() {
    let mut claude = claude_sonnet();
    claude.focused = true;
    claude.terminal_title_stripped =
        Some("Sort the clocked-in list by shift and then by name".into());
    let (_, _, rows, space) = compose(vec![claude]);
    let row = &rows[usize::from(space.bottom() - 1)];
    let text = row.split('\u{2502}').next().unwrap().trim();
    assert!(text.starts_with("\u{273B} Claude"), "{text:?}");
    assert!(text.contains("\u{2026}"), "{text:?}");
}

#[test]
fn spaces_with_agents_make_the_list_scroll() {
    let scroll_with = |agents_per_space: usize| {
        let mut projected = snapshot();
        for number in 2..=4 {
            let mut other = projected.workspaces[0].clone();
            other.workspace_id = format!("ws_{number}");
            other.label = format!("space {number}");
            other.number = number;
            other.focused = false;
            projected.workspaces.push(other);
        }
        for space in ["ws_1", "ws_2", "ws_3", "ws_4"] {
            for index in 0..agents_per_space {
                let mut claude = claude_sonnet();
                claude.pane_id = format!("{space}_pane_{index}");
                claude.workspace_id = space.into();
                projected.agents.push(claude);
            }
        }
        let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
        state.set_snapshot(Box::new(projected));
        state.set_pane_surface(surface());
        state.compose(106, 30).expect("composed frame");
        state.hits.workspace_max_scroll
    };
    assert_eq!(scroll_with(0), 0);
    // Three agents and a "+N more" row are four more rows in each space: the four spaces no longer fit, so the
    // list scrolls.
    assert!(scroll_with(5) > 0);
}

#[test]
fn a_space_shows_three_agents_then_how_many_more() {
    let agents = (0..5)
        .map(|index| {
            let mut claude = claude_sonnet();
            claude.pane_id = format!("pane_{index}");
            claude
        })
        .collect::<Vec<_>>();
    let (_, _, alone_rows, alone) = compose(Vec::new());
    let (_, _, rows, space) = compose(agents);
    assert_eq!(space.height, alone.height + 3 + 1);
    let last = rows[usize::from(space.bottom() - 1)].clone();
    assert!(last.contains("+2 more"), "{last:?}");
    let shown = rows[usize::from(space.y)..usize::from(space.bottom())]
        .iter()
        .filter(|row| row.contains("\u{273B}") && row.contains("Sonnet"))
        .count();
    assert_eq!(shown, 3);
    drop(alone_rows);
}

#[test]
fn nirvana_claude_shows_who_as_nirvana() {
    let mut claude = agent(
        "pane_1",
        Some("claude"),
        &[
            ("nirvana_kind", "claude"),
            ("nirvana_model", "Claude Sonnet 5.5"),
            ("nirvana_role", "nirvana"),
        ],
    );
    claude.agent_status = AgentStatus::Working;
    let (_, _, rows, space) = compose(vec![claude]);
    let row = &rows[usize::from(space.bottom() - 1)];
    assert!(
        row.contains("\u{273B}") && row.contains("Nirvana") && row.contains("Sonnet"),
        "{row:?}"
    );
}

#[test]
fn fold_two_or_more_idle_panes_of_same_kind_without_task() {
    let agy1 = agent(
        "pane_1",
        Some("agy"),
        &[
            ("nirvana_kind", "antigravity"),
            ("nirvana_model", "Gemini 3.8 Flash"),
        ],
    );
    let mut agy2 = agent(
        "pane_2",
        Some("agy"),
        &[
            ("nirvana_kind", "antigravity"),
            ("nirvana_model", "Gemini 3.8 Flash"),
        ],
    );
    agy2.agent_status = AgentStatus::Idle;
    let (_, _, rows, space) = compose(vec![agy1, agy2]);
    let row = &rows[usize::from(space.bottom() - 1)];
    assert!(
        row.contains("\u{25B2} 2 agy panes  idle"),
        "folded row missing in {row:?}"
    );
}

#[test]
fn focused_pane_shows_chat_title_and_no_task_yet_never_shown() {
    let mut claude = claude_sonnet();
    claude.focused = true;
    let (_, _, rows, space) = compose(vec![claude]);
    let row = &rows[usize::from(space.bottom() - 1)];
    assert!(row.contains("Sort cloc"), "{row:?}");
    assert!(
        !row.contains("No task yet"),
        "must never show No task yet: {row:?}"
    );
}

#[test]
fn the_row_drops_the_provider_word_before_it_cuts() {
    use crate::client::shell::nirvana_spaces::agent_line;
    let model = "Claude Sonnet 5.5";
    assert_eq!(
        agent_line("Nirvana", model, 30),
        ("Nirvana - Claude Sonnet 5.5".to_string(), 7)
    );
    assert_eq!(
        agent_line("Nirvana", model, 21),
        ("Nirvana - Sonnet 5.5".to_string(), 7)
    );
    assert_eq!(
        agent_line("Nirvana", model, 19),
        ("Nirva\u{2026} - Sonnet 5.5".to_string(), 6)
    );
    assert_eq!(
        agent_line("Nirvana", model, 12),
        ("Nirvana - S\u{2026}".to_string(), 7)
    );
}

// The owner, 2026-10-21: "ensure the agents correponses to ehs pace. I t right now listes everything the workout app
// refactor and 3d print have it only list based on what space is seletted."
#[test]
fn the_agents_panel_lists_the_selected_space_only() {
    use crate::client::shell::agent_sidebar::{agent_rows, space_agent_rows};
    let mut snap = snapshot();
    let mut other = snap.workspaces[0].clone();
    other.workspace_id = "ws_2".into();
    other.label = "workout".into();
    other.number = 2;
    other.focused = false;
    snap.workspaces[0].focused = true;
    snap.workspaces.push(other);
    let mut theirs = agent("pane_2", Some("claude"), &[]);
    theirs.workspace_id = "ws_2".into();
    snap.agents = vec![agent("pane_1", Some("claude"), &[]), theirs];
    let config = ClientShellConfig::from_config(&Config::default());

    let rows = space_agent_rows(&snap, agent_rows(&snap, &config, None));
    let panes: Vec<_> = rows.iter().map(|row| row.pane_id.as_str()).collect();
    assert_eq!(panes, ["pane_1"]);

    // Selecting the other space lists its agent instead.
    snap.workspaces[0].focused = false;
    snap.workspaces[1].focused = true;
    let rows = space_agent_rows(&snap, agent_rows(&snap, &config, None));
    let panes: Vec<_> = rows.iter().map(|row| row.pane_id.as_str()).collect();
    assert_eq!(panes, ["pane_2"]);

    // The drawn panel says so when the selected space has none.
    snap.agents.retain(|agent| agent.workspace_id == "ws_1");
    let mut state = ClientShellState::new(config);
    state.set_snapshot(Box::new(snap));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 30).expect("composed frame").frame;
    let text: String = frame
        .cells
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect();
    assert!(text.contains("no agents in this space"));
}
