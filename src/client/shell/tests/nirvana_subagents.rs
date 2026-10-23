//! Client shell tests for subagents under each agent in the agents panel (plan 17).
//! The marks and colours by provider, narrow and wide, +N more, 10-minute dim, click target, and row_lines.

use std::collections::HashMap;

use super::*;
use crate::api::schema::AgentStatus;
use crate::nirvana_subagents::{
    set_view_for_test, set_worker_panes_for_test, Subagent, SubagentProvider, SubagentState,
    SubagentsView,
};
use ratatui::layout::Rect;

fn test_agent(pane_id: &str, name: &str) -> ClientShellAgent {
    ClientShellAgent {
        pane_id: pane_id.into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: Some(name.into()),
        display_agent: Some(name.into()),
        agent: Some("claude".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Working,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: false,
        agent_session: None,
    }
}

fn snapshot_with_agent(pane_id: &str, name: &str) -> ClientShellSnapshot {
    let mut snap = snapshot();
    snap.agents = vec![test_agent(pane_id, name)];
    snap
}

fn compose_shell(
    agents: Vec<ClientShellAgent>,
    width: u16,
    height: u16,
) -> (ClientShellState, FrameData, Vec<String>) {
    let mut projected = snapshot();
    projected.agents = agents;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(width, height).expect("composed frame").frame;
    let rows: Vec<String> = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect();
    (state, frame, rows)
}

#[test]
fn subagent_rows_render_under_agent_in_flat_and_grouped_views() {
    let mut sub_map = HashMap::new();
    sub_map.insert(
        "pane_1".to_string(),
        SubagentsView {
            subagents: vec![
                Subagent {
                    id: "sub-1".into(),
                    parent_id: None,
                    name: "general-purpose".into(),
                    description: "Amazon-only parts research".into(),
                    provider: SubagentProvider::Claude,
                    model: "Sonnet 5.5".into(),
                    state: SubagentState::Running,
                    elapsed_secs: 419,
                    tokens: Some(133_700),
                    ended_at: None,
                    depth: 0,
                    target_pane_id: None,
                    run_id: None,
                    run_event_time: None,
                },
                Subagent {
                    id: "sub-2".into(),
                    parent_id: None,
                    name: "build".into(),
                    description: "Fast compile".into(),
                    provider: SubagentProvider::Gemini,
                    model: "Gemini 3.8 Flash".into(),
                    state: SubagentState::Running,
                    elapsed_secs: 720,
                    tokens: Some(1_200_000),
                    ended_at: None,
                    depth: 0,
                    target_pane_id: None,
                    run_id: None,
                    run_event_time: None,
                },
            ],
            total_count: 2,
            overflow: 0,
        },
    );
    set_view_for_test(Some(sub_map));

    let agents = vec![test_agent("pane_1", "3D-Printer-Flow")];

    // 1. Grouped view (Spaces sort) in sidebar
    {
        let (mut state, _, rows) = compose_shell(agents.clone(), 106, 40);
        state.config.agent_panel_sort = crate::config::AgentPanelSortConfig::Spaces;
        let text = rows.join("\n");
        assert!(
            text.contains("\u{273B} general-purpose Sonnet"),
            "text: {text}"
        );
        assert!(text.contains("\u{25C6} build Gemini Flash"), "text: {text}");
    }

    // 2. Flat / priority view in sidebar
    {
        let (mut state, _, rows) = compose_shell(agents, 106, 40);
        state.config.agent_panel_sort = crate::config::AgentPanelSortConfig::Priority;
        let text = rows.join("\n");
        assert!(
            text.contains("\u{273B} general-purpose Sonnet"),
            "text: {text}"
        );
        assert!(text.contains("\u{25C6} build Gemini Flash"), "text: {text}");
    }

    // 3. Wide rendering of row: both lines present with full model name and description
    {
        let config = ClientShellConfig::from_config(&Config::default());
        let row = crate::client::shell::agent_sidebar::agent_row(
            &snapshot_with_agent("pane_1", "3D-Printer-Flow"),
            "pane_1",
            &config,
            None,
        )
        .expect("agent row");

        let mut wide_buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 60, 6));
        crate::client::shell::agent_sidebar::render_agent_row(
            &mut wide_buf,
            Rect::new(0, 0, 60, 6),
            &row,
            &config,
            None,
        );
        let mut lines = Vec::new();
        for y in 0..6 {
            let mut line = String::new();
            for x in 0..60 {
                line.push_str(wide_buf.cell((x, y)).unwrap().symbol());
            }
            lines.push(line);
        }
        let text = lines.join("\n");
        assert!(text.contains("Sonnet 5.5"), "wide text: {text}");
        assert!(
            text.contains("Amazon-only parts research"),
            "wide text: {text}"
        );
        assert!(text.contains("Gemini 3.8 Flash"), "wide text: {text}");
        assert!(text.contains("Fast compile"), "wide text: {text}");
    }

    set_view_for_test(None);
}

#[test]
fn marks_and_colours_by_provider() {
    let mut sub_map = HashMap::new();
    sub_map.insert(
        "pane_1".to_string(),
        SubagentsView {
            subagents: vec![
                Subagent {
                    id: "sub-claude".into(),
                    parent_id: None,
                    name: "claude-sub".into(),
                    description: "claude desc".into(),
                    provider: SubagentProvider::Claude,
                    model: "Sonnet 5.5".into(),
                    state: SubagentState::Running,
                    elapsed_secs: 10,
                    tokens: Some(100),
                    ended_at: None,
                    depth: 0,
                    target_pane_id: None,
                    run_id: None,
                    run_event_time: None,
                },
                Subagent {
                    id: "sub-gemini".into(),
                    parent_id: None,
                    name: "gemini-sub".into(),
                    description: "gemini desc".into(),
                    provider: SubagentProvider::Gemini,
                    model: "Gemini 3.8 Flash".into(),
                    state: SubagentState::Running,
                    elapsed_secs: 20,
                    tokens: Some(200),
                    ended_at: None,
                    depth: 0,
                    target_pane_id: None,
                    run_id: None,
                    run_event_time: None,
                },
                Subagent {
                    id: "sub-local".into(),
                    parent_id: None,
                    name: "local-sub".into(),
                    description: "local desc".into(),
                    provider: SubagentProvider::Local,
                    model: "Local llama".into(),
                    state: SubagentState::Running,
                    elapsed_secs: 30,
                    tokens: Some(300),
                    ended_at: None,
                    depth: 0,
                    target_pane_id: None,
                    run_id: None,
                    run_event_time: None,
                },
            ],
            total_count: 3,
            overflow: 0,
        },
    );
    set_view_for_test(Some(sub_map));

    let agents = vec![test_agent("pane_1", "test-agent")];
    let (state, frame, rows) = compose_shell(agents, 106, 40);

    // Find row with Claude subagent: mark ✻ (\u{273B})
    let claude_row = rows
        .iter()
        .position(|r| r.contains("claude-sub"))
        .expect("claude-sub row");
    let claude_mark_idx = rows[claude_row]
        .chars()
        .position(|c| c == '\u{273B}')
        .unwrap();
    let claude_fg = frame.cells[claude_row * frame.width as usize + claude_mark_idx].fg;
    assert_eq!(
        claude_fg,
        crate::protocol::color_to_u32(ratatui::style::Color::Rgb(0xD8, 0x88, 0x6A))
    );

    // Find row with Gemini subagent: mark ◆ (\u{25C6})
    let gemini_row = rows
        .iter()
        .position(|r| r.contains("gemini-sub"))
        .expect("gemini-sub row");
    let gemini_mark_idx = rows[gemini_row]
        .chars()
        .position(|c| c == '\u{25C6}')
        .unwrap();
    let gemini_fg = frame.cells[gemini_row * frame.width as usize + gemini_mark_idx].fg;
    assert_eq!(
        gemini_fg,
        crate::protocol::color_to_u32(ratatui::style::Color::Rgb(0xAB, 0x93, 0xD6))
    );

    // Find row with Local subagent: mark ● (\u{25CF})
    let local_row = rows
        .iter()
        .position(|r| r.contains("local-sub"))
        .expect("local-sub row");
    let local_mark_idx = rows[local_row]
        .chars()
        .position(|c| c == '\u{25CF}')
        .unwrap();
    let local_fg = frame.cells[local_row * frame.width as usize + local_mark_idx].fg;
    assert_eq!(
        local_fg,
        crate::protocol::color_to_u32(state.config.palette.overlay1)
    );
    assert_eq!(
        SubagentProvider::Claude.colour(),
        crate::nirvana_usage::Provider::Claude.colour()
    );
    assert_eq!(
        SubagentProvider::Gemini.colour(),
        crate::nirvana_usage::Provider::Antigravity.colour()
    );
    assert_eq!(
        SubagentProvider::Local.colour(),
        crate::nirvana_usage::Provider::Local.colour()
    );

    set_view_for_test(None);
}

#[test]
fn narrow_and_wide_adaptive_formatting() {
    let mut sub_map = HashMap::new();
    sub_map.insert(
        "pane_1".to_string(),
        SubagentsView {
            subagents: vec![Subagent {
                id: "sub-1".into(),
                parent_id: None,
                name: "parts".into(),
                description: "researching parts".into(),
                provider: SubagentProvider::Claude,
                model: "Sonnet 5.5".into(),
                state: SubagentState::Running,
                elapsed_secs: 419,
                tokens: Some(133_700),
                ended_at: None,
                depth: 0,
                target_pane_id: None,
                run_id: None,
                run_event_time: None,
            }],
            total_count: 1,
            overflow: 0,
        },
    );
    set_view_for_test(Some(sub_map));

    let agents = vec![test_agent("pane_1", "test-agent")];

    // Wide: width 106 (sidebar area ~26, but let's test wide panel)
    let (_, _, wide_rows) = compose_shell(agents.clone(), 106, 40);
    let wide_text = wide_rows.join("\n");
    assert!(wide_text.contains("Sonnet"), "wide: {wide_text}");

    // Direct check of render_agent_row with wide vs narrow rect
    let mut wide_buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 60, 5));
    let mut narrow_buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 24, 5));
    let config = ClientShellConfig::from_config(&Config::default());

    let row = crate::client::shell::agent_sidebar::agent_row(
        &snapshot_with_agent("pane_1", "test-agent"),
        "pane_1",
        &config,
        None,
    )
    .expect("agent row");

    // Wide rendering: description is drawn on line 2, version number on line 1
    crate::client::shell::agent_sidebar::render_agent_row(
        &mut wide_buf,
        Rect::new(0, 0, 60, 5),
        &row,
        &config,
        None,
    );
    let mut wide_lines = Vec::new();
    for y in 0..5 {
        let mut line = String::new();
        for x in 0..60 {
            line.push_str(wide_buf.cell((x, y)).unwrap().symbol());
        }
        wide_lines.push(line);
    }
    let wide_out = wide_lines.join("\n");
    assert!(wide_out.contains("Sonnet 5.5"), "wide: {wide_out}");
    assert!(wide_out.contains("researching parts"), "wide: {wide_out}");
    assert!(wide_out.contains("6m59s"), "wide: {wide_out}");

    // Narrow rendering: width 24 (narrow) -> description dropped, version dropped, elapsed dropped
    crate::client::shell::agent_sidebar::render_agent_row(
        &mut narrow_buf,
        Rect::new(0, 0, 24, 5),
        &row,
        &config,
        None,
    );
    let mut narrow_lines = Vec::new();
    for y in 0..5 {
        let mut line = String::new();
        for x in 0..24 {
            line.push_str(narrow_buf.cell((x, y)).unwrap().symbol());
        }
        narrow_lines.push(line);
    }
    let narrow_out = narrow_lines.join("\n");
    assert!(
        !narrow_out.contains("researching parts"),
        "narrow: {narrow_out}"
    );
    assert!(narrow_out.contains("Sonnet"), "narrow: {narrow_out}");

    set_view_for_test(None);
}

#[test]
fn overflow_shows_plus_n_more() {
    let mut sub_map = HashMap::new();
    sub_map.insert(
        "pane_1".to_string(),
        SubagentsView {
            subagents: vec![Subagent {
                id: "sub-1".into(),
                parent_id: None,
                name: "agent-1".into(),
                description: "desc".into(),
                provider: SubagentProvider::Claude,
                model: "Sonnet 5.5".into(),
                state: SubagentState::Running,
                elapsed_secs: 10,
                tokens: Some(100),
                ended_at: None,
                depth: 0,
                target_pane_id: None,
                run_id: None,
                run_event_time: None,
            }],
            total_count: 8,
            overflow: 2,
        },
    );
    set_view_for_test(Some(sub_map));

    let config = ClientShellConfig::from_config(&Config::default());
    let row = crate::client::shell::agent_sidebar::agent_row(
        &snapshot_with_agent("pane_1", "test-agent"),
        "pane_1",
        &config,
        None,
    )
    .expect("agent row");

    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 50, 6));
    crate::client::shell::agent_sidebar::render_agent_row(
        &mut buf,
        Rect::new(0, 0, 50, 6),
        &row,
        &config,
        None,
    );

    let mut lines = Vec::new();
    for y in 0..6 {
        let mut line = String::new();
        for x in 0..50 {
            line.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        lines.push(line);
    }
    let out = lines.join("\n");
    assert!(out.contains("+2 more"), "overflow: {out}");

    set_view_for_test(None);
}

#[test]
fn finished_subagents_dimmed_and_ten_minute_filter() {
    let now = 1_000_000u64;
    let mut sub_map = HashMap::new();
    sub_map.insert(
        "pane_1".to_string(),
        SubagentsView {
            subagents: vec![
                Subagent {
                    id: "sub-done".into(),
                    parent_id: None,
                    name: "done-agent".into(),
                    description: "task completed".into(),
                    provider: SubagentProvider::Claude,
                    model: "Sonnet 5.5".into(),
                    state: SubagentState::Done,
                    elapsed_secs: 120,
                    tokens: Some(5000),
                    ended_at: Some(now - 100), // Ended 100s ago (within 10m)
                    depth: 0,
                    target_pane_id: None,
                    run_id: None,
                    run_event_time: None,
                },
                Subagent {
                    id: "sub-failed".into(),
                    parent_id: None,
                    name: "failed-agent".into(),
                    description: "task failed".into(),
                    provider: SubagentProvider::Gemini,
                    model: "Gemini 3.8 Flash".into(),
                    state: SubagentState::Failed,
                    elapsed_secs: 50,
                    tokens: Some(1000),
                    ended_at: Some(now - 200),
                    depth: 0,
                    target_pane_id: None,
                    run_id: None,
                    run_event_time: None,
                },
            ],
            total_count: 2,
            overflow: 0,
        },
    );
    set_view_for_test(Some(sub_map));

    let config = ClientShellConfig::from_config(&Config::default());
    let row = crate::client::shell::agent_sidebar::agent_row(
        &snapshot_with_agent("pane_1", "test-agent"),
        "pane_1",
        &config,
        None,
    )
    .expect("agent row");

    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 60, 6));
    crate::client::shell::agent_sidebar::render_agent_row(
        &mut buf,
        Rect::new(0, 0, 60, 6),
        &row,
        &config,
        None,
    );

    let mut lines = Vec::new();
    for y in 0..6 {
        let mut line = String::new();
        for x in 0..60 {
            line.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        lines.push(line);
    }
    let out = lines.join("\n");
    assert!(out.contains("\u{2713}"), "done checkmark: {out}");
    assert!(out.contains("\u{2717}"), "failed cross: {out}");

    // Check DIM modifier on the done row
    let done_line_idx = lines.iter().position(|l| l.contains("done-agent")).unwrap();
    let check_x = lines[done_line_idx]
        .chars()
        .position(|c| c == '\u{2713}')
        .unwrap();
    let cell = buf.cell((check_x as u16, done_line_idx as u16)).unwrap();
    assert!(cell.modifier.contains(ratatui::style::Modifier::DIM));

    set_view_for_test(None);
}

#[test]
fn click_subagent_row_focuses_target_pane() {
    let mut sub_map = HashMap::new();
    sub_map.insert(
        "pane_1".to_string(),
        SubagentsView {
            subagents: vec![Subagent {
                id: "sub-order-1".into(),
                parent_id: None,
                name: "order-1".into(),
                description: "seat order".into(),
                provider: SubagentProvider::Gemini,
                model: "Gemini 3.8 Flash".into(),
                state: SubagentState::Running,
                elapsed_secs: 30,
                tokens: Some(500),
                ended_at: None,
                depth: 0,
                target_pane_id: Some("worker_pane_42".into()),
                run_id: None,
                run_event_time: None,
            }],
            total_count: 1,
            overflow: 0,
        },
    );
    set_view_for_test(Some(sub_map));

    let config = ClientShellConfig::from_config(&Config::default());
    let row = crate::client::shell::agent_sidebar::agent_row(
        &snapshot_with_agent("pane_1", "test-agent"),
        "pane_1",
        &config,
        None,
    )
    .expect("agent row");

    let mut hits = ShellHitMap::default();
    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 50, 6));

    crate::client::shell::agent_sidebar::render_agent_row(
        &mut buf,
        Rect::new(0, 0, 50, 6),
        &row,
        &config,
        Some(&mut hits),
    );

    // hits.agents should have recorded the subagent's target_pane_id "worker_pane_42"
    let subagent_hit = hits
        .agents
        .iter()
        .find(|(_, pane_id)| pane_id == "worker_pane_42");
    assert!(
        subagent_hit.is_some(),
        "worker pane hit recorded: {:?}",
        hits.agents
    );

    set_view_for_test(None);
}

#[test]
fn row_lines_calculation() {
    let config = ClientShellConfig::from_config(&Config::default());
    let mut row = crate::client::shell::agent_sidebar::agent_row(
        &snapshot_with_agent("pane_1", "test-agent"),
        "pane_1",
        &config,
        None,
    )
    .expect("agent row");

    // 0 subagents: returns base row count
    assert_eq!(
        crate::client::shell::agent_sidebar::agent_row_lines(&row, 50),
        row.rows.len()
    );

    // 2 subagents with description
    row.subagents = vec![
        Subagent {
            id: "s1".into(),
            parent_id: None,
            name: "sub1".into(),
            description: "desc 1".into(),
            provider: SubagentProvider::Claude,
            model: "Sonnet 5.5".into(),
            state: SubagentState::Running,
            elapsed_secs: 10,
            tokens: None,
            ended_at: None,
            depth: 0,
            target_pane_id: None,
            run_id: None,
            run_event_time: None,
        },
        Subagent {
            id: "s2".into(),
            parent_id: None,
            name: "sub2".into(),
            description: "desc 2".into(),
            provider: SubagentProvider::Gemini,
            model: "Gemini 3.8 Flash".into(),
            state: SubagentState::Running,
            elapsed_secs: 20,
            tokens: None,
            ended_at: None,
            depth: 0,
            target_pane_id: None,
            run_id: None,
            run_event_time: None,
        },
    ];

    // Wide (width 50): 2 lines per subagent -> base + 4
    assert_eq!(
        crate::client::shell::agent_sidebar::agent_row_lines(&row, 50),
        row.rows.len() + 4
    );

    // Narrow (width 26): 1 line per subagent (description dropped) -> base + 2
    assert_eq!(
        crate::client::shell::agent_sidebar::agent_row_lines(&row, 26),
        row.rows.len() + 2
    );

    // With overflow: adds 1 line for "+N more"
    row.subagents_overflow = 3;
    assert_eq!(
        crate::client::shell::agent_sidebar::agent_row_lines(&row, 50),
        row.rows.len() + 5
    );
    assert_eq!(
        crate::client::shell::agent_sidebar::agent_row_lines(&row, 26),
        row.rows.len() + 3
    );
}

#[test]
fn seat_worker_panes_filtered_from_top_level() {
    let mut set = std::collections::HashSet::new();
    set.insert("worker_pane_99".to_string());
    set_worker_panes_for_test(Some(set));

    let mut snap = snapshot();
    snap.agents = vec![
        test_agent("orch_pane", "orchestrator"),
        test_agent("worker_pane_99", "worker"),
    ];

    let config = ClientShellConfig::from_config(&Config::default());
    let rows = crate::client::shell::agent_sidebar::agent_rows(&snap, &config, None);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].pane_id, "orch_pane");

    set_worker_panes_for_test(None);
}

#[test]
fn live_scratch_home_subagents_show_in_client_snapshot() {
    let temp = crate::nirvana::with_test_home("live-subagents-snapshot");
    let proj_dir = temp.path.join("repo").join("3D-Printer-Flow-xtool");
    std::fs::create_dir_all(&proj_dir).unwrap();

    let enc = crate::nirvana_subagents::encode_project_folder(&proj_dir);
    let claude_proj = temp.path.join(".claude").join("projects").join(&enc);
    let subagents_dir = claude_proj.join("sess-live").join("subagents");
    std::fs::create_dir_all(&subagents_dir).unwrap();

    let session_file = claude_proj.join("sess-live.jsonl");
    std::fs::write(&session_file, "{\"type\":\"session_start\"}\n").unwrap();

    let meta = serde_json::json!({
        "agentType": "general-purpose",
        "description": "Amazon-only parts research",
        "toolUseId": "toolu_01",
        "spawnDepth": 1,
        "requestShape": "background",
        "requestNonInteractive": true,
        "model": "sonnet"
    });
    std::fs::write(
        subagents_dir.join("agent-a9fa.meta.json"),
        serde_json::to_string(&meta).unwrap(),
    )
    .unwrap();

    let transcript = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg_01",
            "model": "claude-sonnet-5-5",
            "usage": {
                "input_tokens": 2,
                "cache_creation_input_tokens": 3390,
                "cache_read_input_tokens": 129589,
                "output_tokens": 10
            }
        },
        "timestamp": "2026-10-19T21:23:48.371Z",
        "sessionId": "sess-live",
        "cwd": proj_dir.to_str().unwrap()
    });
    std::fs::write(
        subagents_dir.join("agent-a9fa.jsonl"),
        format!("{}\n", serde_json::to_string(&transcript).unwrap()),
    )
    .unwrap();

    // Read directly via subagents reader with temp home
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let subagents =
        crate::nirvana_subagents::read_claude_subagents(&session_file, "pane_1", now_secs);
    assert_eq!(subagents.len(), 1);
    assert_eq!(subagents[0].name, "general-purpose");
    assert_eq!(subagents[0].model, "Sonnet 5.5");
    assert_eq!(subagents[0].tokens, Some(132_991));

    // In a client snapshot, populate view and render
    let mut sub_map = HashMap::new();
    sub_map.insert(
        "pane_1".to_string(),
        SubagentsView {
            subagents: subagents.clone(),
            total_count: 1,
            overflow: 0,
        },
    );
    set_view_for_test(Some(sub_map));

    let config = ClientShellConfig::from_config(&Config::default());
    let row = crate::client::shell::agent_sidebar::agent_row(
        &snapshot_with_agent("pane_1", "3D-Printer-Flow-xtool"),
        "pane_1",
        &config,
        None,
    )
    .expect("agent row");

    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 60, 6));
    crate::client::shell::agent_sidebar::render_agent_row(
        &mut buf,
        Rect::new(0, 0, 60, 6),
        &row,
        &config,
        None,
    );

    let mut lines = Vec::new();
    for y in 0..6 {
        let mut line = String::new();
        for x in 0..60 {
            line.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        lines.push(line);
    }
    let out = lines.join("\n");
    println!("Rendered subagent rows in client snapshot:\n{out}");
    assert!(out.contains("Sonnet 5.5"), "out: {out}");
    assert!(out.contains("Amazon-only parts research"), "out: {out}");
    assert!(out.contains("133.0k"), "out: {out}");

    set_view_for_test(None);
}

#[test]
fn agent_session_id_from_agent_session_info() {
    let mut snap = snapshot_with_agent("pane_session_1", "claude-agent");
    snap.agents[0].agent = Some("claude".into());
    snap.agents[0].tokens.clear();
    snap.agents[0].agent_session = Some(crate::api::schema::AgentSessionInfo {
        source: "herdr:claude".into(),
        agent: "claude".into(),
        kind: crate::agent_resume::AgentSessionRefKind::Id,
        value: "session-xyz".into(),
    });

    let config = ClientShellConfig::from_config(&Config::default());
    let _ = crate::client::shell::agent_sidebar::agent_row(&snap, "pane_session_1", &config, None);

    let query = crate::nirvana_subagents::get_pane_query_for_test("pane_session_1");
    assert!(query.is_some());
    let (_folder, is_claude, session) = query.unwrap();
    assert!(is_claude);
    assert_eq!(session.as_deref(), Some("session-xyz"));

    // Non-claude agent or non-Id kind must be ignored
    snap.agents[0].agent_session = Some(crate::api::schema::AgentSessionInfo {
        source: "herdr:codex".into(),
        agent: "codex".into(),
        kind: crate::agent_resume::AgentSessionRefKind::Id,
        value: "session-codex".into(),
    });
    let _ = crate::client::shell::agent_sidebar::agent_row(&snap, "pane_session_1", &config, None);
    let (_folder, _is_claude, session) =
        crate::nirvana_subagents::get_pane_query_for_test("pane_session_1").unwrap();
    assert_eq!(session, None);
}

#[test]
fn codex_pane_in_claude_folder_shows_no_claude_subagents() {
    let mut snap = snapshot_with_agent("pane_codex_1", "codex-agent");
    snap.agents[0].agent = Some("codex".into());
    snap.agents[0].display_agent = Some("codex".into());

    let config = ClientShellConfig::from_config(&Config::default());
    let _ = crate::client::shell::agent_sidebar::agent_row(&snap, "pane_codex_1", &config, None);

    let query = crate::nirvana_subagents::get_pane_query_for_test("pane_codex_1");
    assert!(query.is_some());
    let (_folder, is_claude, _session) = query.unwrap();
    assert!(!is_claude, "Codex pane must have is_claude = false");
}

#[test]
fn group_subagents_by_run_ordering() {
    use crate::client::shell::agent_sidebar::group_subagents_by_run;

    let sub_live_run1_slow = Subagent {
        id: "1".into(),
        parent_id: None,
        name: "p26-01-slow".into(),
        description: "".into(),
        provider: SubagentProvider::Claude,
        model: "Claude Sonnet 5.5".into(),
        state: SubagentState::Running,
        elapsed_secs: 240,
        tokens: None,
        ended_at: None,
        depth: 0,
        target_pane_id: None,
        run_id: Some("run-1".into()),
        run_event_time: Some(2000),
    };
    let sub_live_run1_fast = Subagent {
        id: "2".into(),
        parent_id: None,
        name: "p26-02-fast".into(),
        description: "".into(),
        provider: SubagentProvider::Gemini,
        model: "Gemini 3.8 Flash".into(),
        state: SubagentState::Running,
        elapsed_secs: 60,
        tokens: None,
        ended_at: None,
        depth: 0,
        target_pane_id: None,
        run_id: Some("run-1".into()),
        run_event_time: Some(2000),
    };
    let sub_done_run1 = Subagent {
        id: "3".into(),
        parent_id: None,
        name: "p26-03-done".into(),
        description: "".into(),
        provider: SubagentProvider::Claude,
        model: "Claude Sonnet 5.5".into(),
        state: SubagentState::Done,
        elapsed_secs: 100,
        tokens: None,
        ended_at: Some(1000),
        depth: 0,
        target_pane_id: None,
        run_id: Some("run-1".into()),
        run_event_time: Some(2000),
    };
    let sub_done_run2 = Subagent {
        id: "4".into(),
        parent_id: None,
        name: "p26-old-done".into(),
        description: "".into(),
        provider: SubagentProvider::Claude,
        model: "Claude Sonnet 5.5".into(),
        state: SubagentState::Done,
        elapsed_secs: 100,
        tokens: None,
        ended_at: Some(500),
        depth: 0,
        target_pane_id: None,
        run_id: Some("run-2".into()),
        run_event_time: Some(1000),
    };

    // run-1 has event time 2000, run-2 has event time 1000 -> run-1 comes first
    let subs = vec![
        sub_done_run2.clone(),
        sub_live_run1_slow.clone(),
        sub_done_run1.clone(),
        sub_live_run1_fast.clone(),
    ];
    let groups = group_subagents_by_run(&subs);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].run_id, "run-1");
    assert_eq!(groups[1].run_id, "run-2");

    // Inside run-1: live workers first, longest running first (slow / 240s before fast / 60s)
    assert_eq!(groups[0].live.len(), 2);
    assert_eq!(groups[0].live[0].name, "p26-01-slow");
    assert_eq!(groups[0].live[1].name, "p26-02-fast");

    // Finished worker after
    assert_eq!(groups[0].finished.len(), 1);
    assert_eq!(groups[0].finished[0].name, "p26-03-done");
}

#[test]
fn render_grouped_subagents_under_run_headers() {
    let mut sub_map = HashMap::new();
    sub_map.insert(
        "pane_1".to_string(),
        SubagentsView {
            subagents: vec![
                Subagent {
                    id: "w1".into(),
                    parent_id: None,
                    name: "p26-01-close-pane".into(),
                    description: "".into(),
                    provider: SubagentProvider::Claude,
                    model: "Claude Sonnet 5.5".into(),
                    state: SubagentState::Running,
                    elapsed_secs: 240,
                    tokens: None,
                    ended_at: None,
                    depth: 0,
                    target_pane_id: Some("worker_pane_1".into()),
                    run_id: Some("p26-clear-sidebar".into()),
                    run_event_time: Some(2000),
                },
                Subagent {
                    id: "w2".into(),
                    parent_id: None,
                    name: "p26-02-done-task".into(),
                    description: "".into(),
                    provider: SubagentProvider::Claude,
                    model: "Claude Sonnet 5.5".into(),
                    state: SubagentState::Done,
                    elapsed_secs: 120,
                    tokens: None,
                    ended_at: Some(100),
                    depth: 0,
                    target_pane_id: Some("worker_pane_closed".into()),
                    run_id: Some("p26-clear-sidebar".into()),
                    run_event_time: Some(2000),
                },
            ],
            total_count: 2,
            overflow: 0,
        },
    );
    set_view_for_test(Some(sub_map));

    let config = ClientShellConfig::from_config(&Config::default());
    let mut snap = snapshot_with_agent("pane_1", "orch");
    // Only worker_pane_1 is still open, worker_pane_closed has closed
    snap.panes.push(ClientShellPane {
        pane_id: "worker_pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        label: None,
        cwd: None,
        foreground_cwd: None,
        focused: false,
        right_click_passthrough: false,
    });

    let row = crate::client::shell::agent_sidebar::agent_row(&snap, "pane_1", &config, None)
        .expect("agent row");

    // Lines count: base(row.rows.len()) + header(1) + live(2) + finished(1)
    assert_eq!(
        crate::client::shell::agent_sidebar::agent_row_lines(&row, 50),
        row.rows.len() + 4
    );

    let mut hits = ShellHitMap::default();
    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 50, 10));
    crate::client::shell::agent_sidebar::render_agent_row(
        &mut buf,
        Rect::new(0, 0, 50, 10),
        &row,
        &config,
        Some(&mut hits),
    );

    let mut lines = Vec::new();
    for y in 0..10 {
        let mut line = String::new();
        for x in 0..50 {
            line.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        lines.push(line);
    }
    let rendered = lines.join("\n");
    assert!(
        rendered.contains("Run p26-clear-sidebar"),
        "output:\n{rendered}"
    );
    assert!(
        rendered.contains("p26-01-close-pane"),
        "output:\n{rendered}"
    );
    assert!(rendered.contains("S5.5"), "output:\n{rendered}");
    assert!(rendered.contains("\u{2713}"), "output:\n{rendered}");
    assert!(rendered.contains("p26-02-done-task"), "output:\n{rendered}");
    assert!(rendered.contains("done"), "output:\n{rendered}");

    // Closed worker pane click target falls back to orchestrator pane_1
    let hits_for_closed: Vec<_> = hits
        .agents
        .iter()
        .filter(|(_, pane)| pane == "pane_1")
        .collect();
    assert_eq!(
        hits_for_closed.len(),
        2,
        "main agent row and closed worker hit target both target pane_1"
    );
    assert!(
        hits.agents
            .iter()
            .all(|(_, pane)| pane != "worker_pane_closed"),
        "closed worker pane must not be in hits"
    );

    // Open worker pane records target pane
    let hits_for_open: Vec<_> = hits
        .agents
        .iter()
        .filter(|(_, pane)| pane == "worker_pane_1")
        .collect();
    assert!(!hits_for_open.is_empty(), "open worker hit recorded");

    set_view_for_test(None);
}

#[test]
fn compact_model_table_test() {
    use crate::nirvana_spaces::compact_model;

    // Claude: family initial and version (with and without Claude prefix)
    assert_eq!(compact_model("Claude Sonnet 5.5"), "S5.5");
    assert_eq!(compact_model("Claude Opus 5.5"), "O5.5");
    assert_eq!(compact_model("Claude Haiku 5.5"), "H5.5");
    assert_eq!(compact_model("Claude Flash 5.1"), "F5.1");
    assert_eq!(compact_model("Sonnet 5.5"), "S5.5");
    assert_eq!(compact_model("Opus 5.5"), "O5.5");
    assert_eq!(compact_model("Haiku 5.5"), "H5.5");
    assert_eq!(compact_model("Fable 5.1"), "F5.1");

    // Gemini: Flash or Pro
    assert_eq!(compact_model("Gemini 3.8 Flash"), "Flash");
    assert_eq!(compact_model("Gemini 3.8 Pro"), "Pro");

    // Codex: Codex
    assert_eq!(compact_model("GPT-5 Codex"), "Codex");
    assert_eq!(compact_model("Codex"), "Codex");

    // Local model: name cut to 6 cells
    assert_eq!(compact_model("Local llama3.1"), "llama3");
    assert_eq!(compact_model("Local deepseek"), "deepse");
}

#[test]
fn subagents_capping_keeps_live_and_counts_cut() {
    use crate::client::shell::agent_sidebar::group_sort_and_cap_subagents;

    let live1 = Subagent {
        id: "l1".into(),
        parent_id: None,
        name: "w-live-1".into(),
        description: "".into(),
        provider: SubagentProvider::Claude,
        model: "Claude Sonnet 5.5".into(),
        state: SubagentState::Running,
        elapsed_secs: 100,
        tokens: None,
        ended_at: None,
        depth: 0,
        target_pane_id: None,
        run_id: Some("run-1".into()),
        run_event_time: Some(2000),
    };
    let live2 = Subagent {
        id: "l2".into(),
        parent_id: None,
        name: "w-live-2".into(),
        description: "".into(),
        provider: SubagentProvider::Claude,
        model: "Claude Sonnet 5.5".into(),
        state: SubagentState::Running,
        elapsed_secs: 50,
        tokens: None,
        ended_at: None,
        depth: 0,
        target_pane_id: None,
        run_id: Some("run-1".into()),
        run_event_time: Some(2000),
    };
    let fin1 = Subagent {
        id: "f1".into(),
        parent_id: None,
        name: "w-fin-1".into(),
        description: "".into(),
        provider: SubagentProvider::Claude,
        model: "Claude Sonnet 5.5".into(),
        state: SubagentState::Done,
        elapsed_secs: 50,
        tokens: None,
        ended_at: Some(1500),
        depth: 0,
        target_pane_id: None,
        run_id: Some("run-1".into()),
        run_event_time: Some(2000),
    };
    let fin2 = Subagent {
        id: "f2".into(),
        parent_id: None,
        name: "w-fin-2".into(),
        description: "".into(),
        provider: SubagentProvider::Claude,
        model: "Claude Sonnet 5.5".into(),
        state: SubagentState::Done,
        elapsed_secs: 50,
        tokens: None,
        ended_at: Some(1400),
        depth: 0,
        target_pane_id: None,
        run_id: Some("run-1".into()),
        run_event_time: Some(2000),
    };

    let all_subs = vec![live1, live2, fin1, fin2];
    // Total lines would be: 1 header + 2*2 live + 2*1 finished = 7 lines.
    // If available_lines is 4: budget is 3 (reserving 1 line for "+N more").
    // Live worker 1 takes header (1) + live (2) = 3 lines.
    // Kept workers: 1 live worker. Cut workers: 3 workers.
    let capped = group_sort_and_cap_subagents(&all_subs, 4);
    assert_eq!(capped.groups.len(), 1);
    assert_eq!(capped.groups[0].live.len(), 1);
    assert_eq!(capped.groups[0].finished.len(), 0);
    assert_eq!(capped.cut_count, 3);
}

#[test]
fn worker_row_cut_to_fit_at_26_cells_with_studio_gui_mcp_v2() {
    let mut sub_map = HashMap::new();
    sub_map.insert(
        "pane_1".to_string(),
        SubagentsView {
            subagents: vec![Subagent {
                id: "w-long".into(),
                parent_id: None,
                name: "studio-gui-mcp-v2".into(),
                description: "".into(),
                provider: SubagentProvider::Claude,
                model: "Claude Sonnet 5.5".into(),
                state: SubagentState::Done,
                elapsed_secs: 120,
                tokens: None,
                ended_at: Some(100),
                depth: 0,
                target_pane_id: None,
                run_id: Some("run-long".into()),
                run_event_time: Some(2000),
            }],
            total_count: 1,
            overflow: 0,
        },
    );
    set_view_for_test(Some(sub_map));

    let config = ClientShellConfig::from_config(&Config::default());
    let snap = snapshot_with_agent("pane_1", "orch");
    let row = crate::client::shell::agent_sidebar::agent_row(&snap, "pane_1", &config, None)
        .expect("agent row");

    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 26, 5));
    crate::client::shell::agent_sidebar::render_agent_row(
        &mut buf,
        Rect::new(0, 0, 26, 5),
        &row,
        &config,
        None,
    );

    let mut lines = Vec::new();
    for y in 0..5 {
        let mut line = String::new();
        for x in 0..26 {
            line.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        lines.push(line);
    }
    let rendered = lines.join("\n");
    // Row 0 is orchestrator
    // Row 1 is Run header
    // Row 2 is finished worker: mark ✓ and word done must always show!
    assert!(rendered.contains("\u{2713}"), "rendered:\n{rendered}");
    assert!(rendered.contains("done"), "rendered:\n{rendered}");
    assert!(rendered.contains("studio-gui"), "rendered:\n{rendered}");

    set_view_for_test(None);
}

fn worker(id: &str, run: Option<&str>, state: SubagentState, at: u64) -> Subagent {
    Subagent {
        id: id.into(),
        parent_id: None,
        name: id.into(),
        description: String::new(),
        provider: SubagentProvider::Claude,
        model: "Claude Sonnet 5.5".into(),
        state,
        elapsed_secs: 30,
        tokens: None,
        ended_at: (state != SubagentState::Running).then_some(at),
        depth: 0,
        target_pane_id: None,
        run_id: run.map(str::to_owned),
        run_event_time: Some(at),
    }
}

/// Plan 26 §5: the run with a live worker comes first, even when a run holding only finished rows is newer; the
/// finished runs follow, newest first.
#[test]
fn a_live_run_sorts_above_a_newer_finished_run() {
    use crate::client::shell::agent_sidebar::group_subagents_by_run;
    let rows = [
        worker("oldest-done", Some("run-oldest"), SubagentState::Done, 500),
        worker("new-blocked", Some("run-new"), SubagentState::Blocked, 9000),
        worker("old-live", Some("run-old"), SubagentState::Running, 1000),
    ];
    let runs: Vec<&str> = group_subagents_by_run(&rows)
        .iter()
        .map(|g| g.run_id)
        .collect();
    assert_eq!(runs, ["run-old", "run-new", "run-oldest"]);
}

/// Plan 26 review R1: the agent's own subagents stay beside the run groups, with their own "+N more", and the run's
/// rows are counted in the row's height.
#[test]
fn own_subagents_stay_beside_run_groups() {
    let config = ClientShellConfig::from_config(&Config::default());
    let mut row = crate::client::shell::agent_sidebar::agent_row(
        &snapshot_with_agent("pane_1", "test-agent"),
        "pane_1",
        &config,
        None,
    )
    .expect("agent row");
    row.subagents = vec![
        worker("explore-1", None, SubagentState::Running, 100),
        worker("p26-live", Some("run-a"), SubagentState::Running, 200),
        worker("p26-done", Some("run-a"), SubagentState::Done, 150),
    ];
    row.subagents_overflow = 2;
    // base, one flat row, "+2 more", the run header, two lines for the live worker, one for the done one.
    let lines = crate::client::shell::agent_sidebar::agent_row_lines(&row, 26);
    assert_eq!(lines, row.rows.len().max(1) + 1 + 1 + 1 + 2 + 1);

    let height = lines as u16;
    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 26, height));
    crate::client::shell::agent_sidebar::render_agent_row(
        &mut buf,
        Rect::new(0, 0, 26, height),
        &row,
        &config,
        None,
    );
    let text: Vec<String> = (0..height)
        .map(|y| {
            (0..26)
                .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    let base = row.rows.len().max(1);
    assert!(text[base].contains("explore-1"), "{text:?}");
    assert_eq!(text[base + 1], "   +2 more", "{text:?}");
    assert_eq!(text[base + 2], "  Run run-a", "{text:?}");
    assert_eq!(text[base + 3], "    p26-live", "{text:?}");
    assert_eq!(text[base + 5], "    \u{2713} p26-done done", "{text:?}");
}
