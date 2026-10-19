//! Nirvana fork, plan 12 O4a: the sidebar's "new" menu (L7: "client test on the menu").

use super::*;
use crate::api::schema::Method;
use crate::nirvana_launch::NewKind;

fn menu_state() -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.open_new_space_menu(0, 10);
    state
}

fn labels(state: &ClientShellState) -> Vec<&'static str> {
    let Some(ClientShellOverlay::ContextMenu(menu)) = state.overlay.as_ref() else {
        panic!("the new menu is open");
    };
    menu.items().iter().map(|item| item.label).collect()
}

fn invoked(outcome: &ClientShellInput) -> Vec<crate::api::schema::CommandInvokeParams> {
    outcome
        .actions
        .iter()
        .filter_map(|action| match action {
            ClientShellAction::Endpoint { request, .. } => match &request.method {
                Method::CommandInvoke(params) => Some(params.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn new_offers_nirvana_claude_code_antigravity_and_a_shell() {
    assert_eq!(
        labels(&menu_state()),
        [
            "Nirvana",
            "Claude Code",
            "Antigravity",
            "Shell",
            "Fresh Nirvana Claude"
        ]
    );
}

/// Plan 13 §6.1: the fresh entry asks for its own kind and makes the window Nirvana's, as Nirvana does.
#[test]
fn the_fresh_entry_asks_for_a_fresh_nirvana_and_claims_the_window() {
    super::super::nirvana_new_menu::CLAIMED_IN_TEST.with(|c| c.borrow_mut().clear());
    let mut state = menu_state();
    let mut outcome = ClientShellInput::default();
    state.activate_context_menu_item(NewKind::MENU.len() + 1, &mut outcome);
    let calls = invoked(&outcome);
    assert_eq!(calls.len(), 1, "{:?}", outcome.actions);
    assert_eq!(calls[0].command_id, "nirvana.new:nirvana-fresh");
    assert_eq!(calls[0].workspace_id.as_deref(), Some("ws_1"));
    let claimed = super::super::nirvana_new_menu::CLAIMED_IN_TEST.with(|c| c.borrow().clone());
    assert_eq!(claimed, vec![std::path::PathBuf::from("/repo")]);
    assert!(state.overlay.is_none(), "the menu closes");
}

#[test]
fn each_agent_entry_asks_the_server_for_its_own_kind_from_this_space() {
    for (index, kind) in NewKind::MENU.iter().enumerate() {
        super::super::nirvana_new_menu::CLAIMED_IN_TEST.with(|c| c.borrow_mut().clear());
        let mut state = menu_state();
        let mut outcome = ClientShellInput::default();
        state.activate_context_menu_item(index, &mut outcome);
        let calls = invoked(&outcome);
        assert_eq!(calls.len(), 1, "{kind:?}: {:?}", outcome.actions);
        assert_eq!(calls[0].command_id, format!("nirvana.new:{}", kind.key()));
        assert_eq!(calls[0].workspace_id.as_deref(), Some("ws_1"));
        // Only Nirvana makes the window Nirvana's, on the folder the new space opens in.
        let claimed = super::super::nirvana_new_menu::CLAIMED_IN_TEST.with(|c| c.borrow().clone());
        if *kind == NewKind::Nirvana {
            assert_eq!(claimed, vec![std::path::PathBuf::from("/repo")]);
        } else {
            assert!(claimed.is_empty(), "{kind:?} claimed {claimed:?}");
        }
        assert!(state.overlay.is_none(), "the menu closes");
    }
}

#[test]
fn shell_is_what_new_did_before_the_menu() {
    let mut state = menu_state();
    let mut outcome = ClientShellInput::default();
    state.activate_context_menu_item(3, &mut outcome);
    assert!(invoked(&outcome).is_empty(), "{:?}", outcome.actions);
    let mut before = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    before.set_snapshot(Box::new(snapshot()));
    let mut expected = ClientShellInput::default();
    before.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::NewWorkspace),
        &mut expected,
    );
    assert_eq!(
        format!("{:?}", outcome.actions),
        format!("{:?}", expected.actions)
    );
}
