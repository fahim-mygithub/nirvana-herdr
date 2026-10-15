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
    let task_y = usize::from(space.bottom() - 2);
    let agent_y = usize::from(space.bottom() - 1);
    assert!(
        rows[task_y].contains("Sort clocked-in"),
        "task row: {:?}",
        rows[task_y]
    );
    assert!(
        rows[agent_y].contains("\u{273B} Claude Sonnet 5.5"),
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
    for expected in [
        "\u{273B} Claude Sonnet 5.5",
        "\u{2756} GPT-5 Codex",
        "\u{25B2} Gemini 3.8 Flash",
        "\u{2302} Local qwen3:8b",
        "\u{25CB} pi",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in\n{text}");
    }
}

#[test]
fn an_older_server_without_tokens_still_names_the_agent() {
    let (_, _, rows, space) = compose(vec![agent("pane_1", Some("claude"), &[])]);
    assert!(rows[usize::from(space.bottom() - 2)].contains("No task yet"));
    assert!(rows[usize::from(space.bottom() - 1)].contains("\u{273B} Claude"));
}

#[test]
fn each_agent_adds_two_rows_to_its_space() {
    let (_, _, _, alone) = compose(Vec::new());
    let (_, _, _, one) = compose(vec![claude_sonnet()]);
    let mut second = claude_sonnet();
    second.pane_id = "pane_2".into();
    let (_, _, _, two) = compose(vec![claude_sonnet(), second]);
    assert_eq!(one.height, alone.height + 2);
    assert_eq!(two.height, alone.height + 4);
}

#[test]
fn a_long_title_is_cut_to_the_sidebar_with_an_ellipsis() {
    let mut claude = claude_sonnet();
    claude.terminal_title_stripped =
        Some("Sort the clocked-in list by shift and then by name".into());
    let (_, _, rows, space) = compose(vec![claude]);
    let row = &rows[usize::from(space.bottom() - 2)];
    let task = row.split('\u{2502}').next().unwrap().trim();
    assert!(task.starts_with("Sort the"), "{task:?}");
    assert!(task.ends_with('\u{2026}'), "{task:?}");
}

#[test]
fn spaces_with_agents_make_the_list_scroll() {
    let scroll_with = |agents_per_space: usize| {
        let mut projected = snapshot();
        let mut second = projected.workspaces[0].clone();
        second.workspace_id = "ws_2".into();
        second.label = "second".into();
        second.number = 2;
        second.focused = false;
        projected.workspaces.push(second);
        for space in ["ws_1", "ws_2"] {
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
    // Three agents are six more rows in each space: the two spaces no longer fit, so the list scrolls.
    assert!(scroll_with(3) > 0);
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
    assert_eq!(space.height, alone.height + 3 * 2 + 1);
    let last = rows[usize::from(space.bottom() - 1)].clone();
    assert!(last.contains("+2 more"), "{last:?}");
    let shown = rows[usize::from(space.y)..usize::from(space.bottom())]
        .iter()
        .filter(|row| row.contains("\u{273B} Claude Sonnet 5.5"))
        .count();
    assert_eq!(shown, 3);
    drop(alone_rows);
}
