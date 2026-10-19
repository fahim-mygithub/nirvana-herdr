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
