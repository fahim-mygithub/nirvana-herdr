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
            && rows[agent_y].contains("S5.5")
            && rows[agent_y].contains("working"),
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
        ("\u{273B}", "Claude", "S5.5"),
        ("\u{2756}", "Codex", "Codex"),
        ("\u{25B2}", "agy", "Flash"),
        ("\u{2302}", "Local", "qwen3:"),
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
        row.contains("\u{273B}") && row.contains("Claude") && row.contains("idle"),
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
    claude.terminal_title_stripped =
        Some("Sort the clocked-in list by shift and then by name".into());
    let mut projected = snapshot();
    projected.agents = vec![claude];
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.hovered_space_agent_pane = Some("pane_1".into());
    let frame = state.compose(106, 30).expect("composed frame").frame;
    let rows: Vec<String> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    let space = state.hits.workspaces[0].rect;
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
        .filter(|row| row.contains("\u{273B}") && row.contains("S5.5"))
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
        row.contains("\u{273B}") && row.contains("Nirvana") && row.contains("S5.5"),
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
fn focused_pane_keeps_model_and_state_and_no_task_yet_never_shown() {
    // S2: Remove the focused-pane substitution: the focused row keeps its model and state.
    // A row whose title means no task shows nothing extra on hover.
    let mut claude = claude_sonnet();
    claude.focused = true;
    claude.terminal_title_stripped = Some("No task yet".into());
    let mut projected = snapshot();
    projected.agents = vec![claude];
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.hovered_space_agent_pane = Some("pane_1".into());
    let frame = state.compose(106, 30).expect("composed frame").frame;
    let rows: Vec<String> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    let space = state.hits.workspaces[0].rect;
    let row = &rows[usize::from(space.bottom() - 1)];
    let text = row.split('\u{2502}').next().unwrap();
    assert_eq!(text, "   \u{273B} Claude S5.5 working ");
    assert!(
        !row.contains("No task yet"),
        "must never show No task yet: {row:?}"
    );
}

#[test]
fn mouse_moved_sets_hover_and_moved_away_clears_it() {
    let (mut state, _, _, _) = compose(vec![claude_sonnet()]);
    assert!(
        !state.hits.space_agents.is_empty(),
        "must have space_agents hits"
    );
    let (rect, pane) = state.hits.space_agents[0].clone();
    assert_eq!(pane, "pane_1");

    // 1. Move over the agent row -> sets hover
    let move_over = crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Moved,
        column: rect.x + 1,
        row: rect.y,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    let mut outcome = ClientShellInput::default();
    state.handle_mouse(move_over, &mut outcome);
    assert!(outcome.repaint);
    assert_eq!(state.hovered_space_agent_pane.as_deref(), Some("pane_1"));

    // 2. Composing while hovered shows chat title
    let frame = state.compose(106, 30).expect("frame").frame;
    let rows: Vec<String> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    let row = &rows[usize::from(rect.y)];
    assert!(
        row.contains("Sort clocked-in") || row.contains("Sort cloc"),
        "{row}"
    );

    // 3. Move away -> clears hover
    let move_away = crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Moved,
        column: 0,
        row: 0,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    let mut outcome2 = ClientShellInput::default();
    state.handle_mouse(move_away, &mut outcome2);
    assert!(outcome2.repaint);
    assert_eq!(state.hovered_space_agent_pane, None);
}

#[test]
fn space_row_whole_row_equality_at_26_cells() {
    use crate::client::shell::nirvana_spaces::{render_space_agents, SpaceAgent};
    use crate::client::shell::Palette;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let palette = Palette::catppuccin();
    let area = Rect::new(0, 0, 26, 1);

    let row_text = |buf: &Buffer| -> String {
        let mut s = String::new();
        for x in 0..26 {
            s.push_str(buf.cell((x, 0)).unwrap().symbol());
        }
        s
    };

    // 1. An unindented space: Claude Sonnet 5.5 working
    // w_avail = 19. step1: "Claude S5.5 working" (19 chars) fits!
    let mut buf1 = Buffer::empty(area);
    let agent_claude = SpaceAgent {
        pane_id: "pane_1".into(),
        icon: "\u{273B}",
        who: "Claude".into(),
        model: "S5.5".into(),
        status: AgentStatus::Working,
        task: "task".into(),
        is_folded: false,
        focused: false,
    };
    render_space_agents(
        &mut buf1,
        area,
        std::slice::from_ref(&agent_claude),
        false, // unindented
        false,
        false,
        None,
        None,
        &palette,
    );
    assert_eq!(row_text(&buf1), "   \u{273B} Claude S5.5 working  ");

    // 2. An indented worktree row: Claude Sonnet 5.5 working
    // w_avail = 14. step1 (19) > 14, step2 (19) > 14, step3 (16) > 14.
    // step4: model dropped, full word: "Claude working" (14 chars) fits!
    let mut buf2 = Buffer::empty(area);
    render_space_agents(
        &mut buf2,
        area,
        std::slice::from_ref(&agent_claude),
        true, // indented
        true,
        false,
        None,
        None,
        &palette,
    );
    assert_eq!(row_text(&buf2), "        \u{273B} Claude working  ");

    // 3. A space with Nirvana and Claude:
    // max_who = 7. w_avail = 19.
    // Nirvana: model empty -> "Nirvana working"
    // Claude: who_padded (7) + 1 + S5.5 (4) + 1 + working (7) = 20 > 19.
    // step2 unpadded who: "Claude S5.5 working" (19 chars) fits!
    let mut buf3 = Buffer::empty(Rect::new(0, 0, 26, 2));
    let agent_nirvana = SpaceAgent {
        pane_id: "pane_0".into(),
        icon: "\u{273B}",
        who: "Nirvana".into(),
        model: "".into(),
        status: AgentStatus::Working,
        task: "task".into(),
        is_folded: false,
        focused: false,
    };
    render_space_agents(
        &mut buf3,
        Rect::new(0, 0, 26, 2),
        &[agent_nirvana, agent_claude.clone()],
        false,
        false,
        false,
        None,
        None,
        &palette,
    );
    let mut s0 = String::new();
    let mut s1 = String::new();
    for x in 0..26 {
        s0.push_str(buf3.cell((x, 0)).unwrap().symbol());
        s1.push_str(buf3.cell((x, 1)).unwrap().symbol());
    }
    assert_eq!(s0, "   \u{273B} Nirvana working      ");
    assert_eq!(s1, "   \u{273B} Claude S5.5 working  ");

    // 4. An agy space: who = "agy", model = "Flash", status = Working
    // w_avail = 19. step1: "agy Flash working" (17 chars) fits!
    let mut buf4 = Buffer::empty(area);
    let agent_agy = SpaceAgent {
        pane_id: "pane_3".into(),
        icon: "\u{25B2}",
        who: "agy".into(),
        model: crate::nirvana_spaces::compact_model("Gemini 3.8 Flash"),
        status: AgentStatus::Working,
        task: "task".into(),
        is_folded: false,
        focused: false,
    };
    render_space_agents(
        &mut buf4,
        area,
        std::slice::from_ref(&agent_agy),
        false,
        false,
        false,
        None,
        None,
        &palette,
    );
    assert_eq!(row_text(&buf4), "   \u{25B2} agy Flash working    ");

    // 5. Padding test: Claude (6) + agy (3) -> max_who = 6.
    // agy padded to 6 cells: "agy   " + " " + "Flash" + " " + "idle" = 17 <= 19.
    let mut buf5 = Buffer::empty(Rect::new(0, 0, 26, 2));
    let mut agy_idle = agent_agy;
    agy_idle.status = AgentStatus::Idle;
    render_space_agents(
        &mut buf5,
        Rect::new(0, 0, 26, 2),
        &[agent_claude, agy_idle],
        false,
        false,
        false,
        None,
        None,
        &palette,
    );
    let mut p0 = String::new();
    let mut p1 = String::new();
    for x in 0..26 {
        p0.push_str(buf5.cell((x, 0)).unwrap().symbol());
        p1.push_str(buf5.cell((x, 1)).unwrap().symbol());
    }
    assert_eq!(p0, "   \u{273B} Claude S5.5 working  ");
    assert_eq!(p1, "   \u{25B2} agy    Flash idle    ");

    // 6. Step 3 test: who = "Nirvana" (7), model = "Flash" (5), status = Working (7 -> 4)
    // w_avail = 19. step1 (21) > 19, step2 (21) > 19, step3 "Nirvana Flash work" (18) <= 19 fits!
    let mut buf6 = Buffer::empty(area);
    let agent_step3 = SpaceAgent {
        pane_id: "pane_s3".into(),
        icon: "\u{273B}",
        who: "Nirvana".into(),
        model: "Flash".into(),
        status: AgentStatus::Working,
        task: "task".into(),
        is_folded: false,
        focused: false,
    };
    render_space_agents(
        &mut buf6,
        area,
        std::slice::from_ref(&agent_step3),
        false,
        false,
        false,
        None,
        None,
        &palette,
    );
    assert_eq!(row_text(&buf6), "   \u{273B} Nirvana Flash work   ");
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
