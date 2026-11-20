//! Nirvana fork: the tab menu's Restart Claude (nirvana/plans/2026-10-15-02-restart-claude.md L2).

use super::*;
use crate::api::schema::Method;

fn agent(kind: &str) -> ClientShellAgent {
    ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: Some("moto".into()),
        display_agent: None,
        agent: Some(kind.into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Idle,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: true,
        agent_session: None,
    }
}

fn tab_menu_labels(agents: Vec<ClientShellAgent>) -> (ClientShellState, Vec<&'static str>) {
    let mut projected = snapshot();
    projected.agents = agents;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.open_tab_context_menu("tab_1".into(), 0, 0);
    let Some(ClientShellOverlay::ContextMenu(menu)) = state.overlay.as_ref() else {
        panic!("tab context menu");
    };
    let labels = menu.items().iter().map(|item| item.label).collect();
    (state, labels)
}

#[test]
fn a_tab_running_claude_offers_restart_claude() {
    let (_, plain) = tab_menu_labels(Vec::new());
    assert_eq!(plain, ["New tab", "Rename", "Close"]);
    let (_, other) = tab_menu_labels(vec![agent("codex")]);
    assert_eq!(other, ["New tab", "Rename", "Close"]);
    let (_, claude) = tab_menu_labels(vec![agent("claude")]);
    assert_eq!(claude, ["New tab", "Rename", "Restart Claude", "Close"]);
}

#[test]
fn restart_claude_asks_the_server_for_that_tab() {
    let (mut state, _) = tab_menu_labels(vec![agent("claude")]);
    let mut outcome = ClientShellInput::default();
    state.activate_context_menu_item(2, &mut outcome);
    let invoked: Vec<_> = outcome
        .actions
        .iter()
        .filter_map(|action| match action {
            ClientShellAction::Endpoint { request, .. } => match &request.method {
                Method::CommandInvoke(params) => Some(params.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        invoked.len(),
        1,
        "one command.invoke: {:?}",
        outcome.actions
    );
    assert_eq!(
        invoked[0].command_id,
        crate::nirvana::RESTART_CLAUDE_COMMAND
    );
    assert_eq!(invoked[0].tab_id.as_deref(), Some("tab_1"));
    assert_eq!(invoked[0].workspace_id.as_deref(), Some("ws_1"));
}

/// Two spaces, each with a Nirvana Claude; the first is focused. `ws_2` is a plain folder (no git), so its menu is
/// Rename and Close plus the restart.
fn two_spaces(second_status: AgentStatus) -> ClientShellState {
    let mut projected = snapshot();
    projected.workspaces.push(ClientShellWorkspace {
        workspace_id: "ws_2".into(),
        active_tab_id: "tab_2".into(),
        new_workspace_cwd: "/moto".into(),
        number: 2,
        label: "MotoProject".into(),
        custom_label: false,
        branch: None,
        git_ahead_behind: None,
        tokens: Vec::new(),
        worktree: None,
        focused: false,
        agent_status: second_status,
    });
    projected.tabs.push(ClientShellTab {
        tab_id: "tab_2".into(),
        workspace_id: "ws_2".into(),
        number: 1,
        label: "1".into(),
        custom_label: false,
        zoomed: false,
        focused: false,
        agent_status: second_status,
    });
    projected.panes.push(ClientShellPane {
        pane_id: "pane_2".into(),
        workspace_id: "ws_2".into(),
        tab_id: "tab_2".into(),
        label: None,
        cwd: Some("/moto".into()),
        foreground_cwd: Some("/moto".into()),
        focused: false,
        right_click_passthrough: false,
    });
    let mut first = agent("claude");
    first.name = Some("nirvana".into());
    let mut second = agent("claude");
    second.name = Some("nirvana".into());
    second.pane_id = "pane_2".into();
    second.workspace_id = "ws_2".into();
    second.tab_id = "tab_2".into();
    second.focused = false;
    second.agent_status = second_status;
    projected.agents = vec![first, second];
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state
}

/// Right-clicks the space `workspace_id` in the sidebar: its menu's labels.
fn right_click_space(state: &mut ClientShellState, workspace_id: &str) -> Vec<&'static str> {
    state.compose(106, 20).expect("composed frame");
    let space = state
        .hits
        .workspaces
        .iter()
        .find(|hit| hit.workspace_id == workspace_id)
        .map(|hit| hit.rect)
        .expect("the space is in the sidebar");
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: space.x + 2,
        row: space.y,
        modifiers: KeyModifiers::empty(),
    })]);
    let Some(ClientShellOverlay::ContextMenu(menu)) = state.overlay.as_ref() else {
        panic!("space context menu");
    };
    menu.items().iter().map(|item| item.label).collect()
}

/// Clicks the menu's row `index`, as drawn.
fn click_menu_row(state: &mut ClientShellState, index: usize) -> ClientShellInput {
    state.compose(106, 20).expect("menu drawn");
    let row = state.hits.context_menu_rows[index].0;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: row.x + 1,
        row: row.y,
        modifiers: KeyModifiers::empty(),
    })])
}

/// P87 U2: the owner, 2026-11-18, "if your right click the project an option appears to restart that specific
/// project". A right-click on a space that is not focused offers Restart Claude, and the restart names that space's
/// Nirvana Claude pane, not the focused one. U3: it is the tab menu's own command.
#[test]
fn p87_a_spaces_menu_restarts_that_spaces_nirvana_claude() {
    let mut state = two_spaces(AgentStatus::Idle);
    let labels = right_click_space(&mut state, "ws_2");
    assert_eq!(labels, ["Rename", "Restart Claude", "Close", "Profiles"]);
    let outcome = click_menu_row(&mut state, 1);
    assert_eq!(
        super::nirvana_seats::restarts(&outcome),
        vec![(
            Some("ws_2".into()),
            Some("tab_2".into()),
            Some("pane_2".into())
        )]
    );
    assert!(state.overlay.is_none(), "the menu closes");
}

/// P87 U2: a space with no Nirvana Claude (a plain Claude Code, or none) gets no Restart Claude in its menu.
#[test]
fn p87_a_space_without_nirvana_claude_offers_no_restart() {
    let mut state = two_spaces(AgentStatus::Idle);
    let mut projected = state.snapshot.as_deref().cloned().expect("snapshot");
    projected.agents[1].name = Some("moto".into());
    state.set_snapshot(Box::new(projected));
    assert_eq!(right_click_space(&mut state, "ws_2"), ["Rename", "Close", "Profiles"]);
}

/// P87: a working Nirvana Claude restarts on the second click; the first keeps the menu open and says why.
#[test]
fn p87_a_working_spaces_restart_asks_again() {
    let mut state = two_spaces(AgentStatus::Working);
    assert_eq!(
        right_click_space(&mut state, "ws_2"),
        ["Rename", "Restart Claude", "Close", "Profiles"]
    );
    let outcome = click_menu_row(&mut state, 1);
    assert!(super::nirvana_seats::restarts(&outcome).is_empty());
    let Some(ClientShellOverlay::ContextMenu(menu)) = state.overlay.as_ref() else {
        panic!("the menu stays open");
    };
    assert_eq!(menu.items()[1].label, "Restart Claude? It is working");
    let outcome = click_menu_row(&mut state, 1);
    assert_eq!(
        super::nirvana_seats::restarts(&outcome),
        vec![(
            Some("ws_2".into()),
            Some("tab_2".into()),
            Some("pane_2".into())
        )]
    );
}

/// P87 fix round (review of aa340405, item b3): the menu read the status once, when it opened, so a Claude that
/// started working while it was open restarted on the first click. The click reads it again.
#[test]
fn p87_a_space_claude_that_starts_working_while_the_menu_is_open_asks_first() {
    let mut state = two_spaces(AgentStatus::Idle);
    assert_eq!(
        right_click_space(&mut state, "ws_2"),
        ["Rename", "Restart Claude", "Close", "Profiles"]
    );
    let mut projected = state.snapshot.as_deref().cloned().expect("snapshot");
    projected.agents[1].agent_status = AgentStatus::Working;
    state.set_snapshot(Box::new(projected));
    let outcome = click_menu_row(&mut state, 1);
    assert!(super::nirvana_seats::restarts(&outcome).is_empty());
    let Some(ClientShellOverlay::ContextMenu(menu)) = state.overlay.as_ref() else {
        panic!("the menu stays open");
    };
    assert_eq!(menu.items()[1].label, "Restart Claude? It is working");
}

/// P87 fix round (item b1): a space menu's restart is not sent twice while the fresh Claude starts. Opened again, the
/// menu says it is restarting and a click sends nothing; once `RESTART_PENDING_WINDOW` has passed it restarts again.
#[test]
fn p87_a_space_menu_restart_is_not_sent_twice() {
    let mut state = two_spaces(AgentStatus::Idle);
    let _ = right_click_space(&mut state, "ws_2");
    let outcome = click_menu_row(&mut state, 1);
    assert_eq!(super::nirvana_seats::restarts(&outcome).len(), 1);
    assert_eq!(
        right_click_space(&mut state, "ws_2"),
        ["Rename", "Restart Claude: restarting\u{2026}", "Close", "Profiles"]
    );
    let outcome = click_menu_row(&mut state, 1);
    assert!(super::nirvana_seats::restarts(&outcome).is_empty());
    assert!(state.overlay.is_none(), "the menu closes");
    for sent in state.nirvana_restarting.values_mut() {
        sent.at = sent
            .at
            .checked_sub(std::time::Duration::from_secs(16))
            .expect("the clock reaches that far back");
    }
    assert_eq!(
        right_click_space(&mut state, "ws_2"),
        ["Rename", "Restart Claude", "Close", "Profiles"]
    );
    let outcome = click_menu_row(&mut state, 1);
    assert_eq!(super::nirvana_seats::restarts(&outcome).len(), 1);
}
