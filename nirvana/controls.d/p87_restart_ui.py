# P87 (2026-11-18): Restart Claude from the Seats panel and from a space's menu. The owner, verbatim: "Another missing
# feature is when you switch the default orchistrator and it askes to restart to fix ... We should be able to click
# restart in this modal or if your right click the project an option appears to restart that specific project."
# Controls starting with p87_. One sabotage per assertion, each on a product line; the test named must print FAIL
# with it in place (the expected red line, written here before the run). `Control` is provided by controls.py.
SEATS = "src/client/shell/nirvana_seats.rs"
MENU = "src/client/shell/context_menu.rs"
SERVER = "src/app/nirvana_restart.rs"

T_CLICK = "p87_the_restart_words_and_r_restart_the_panels_nirvana_claude"
T_WORKING = "p87_a_working_nirvana_claude_restarts_on_the_second_press"
T_MATCH = "p87_no_restart_offered_when_the_running_seat_matches"
T_LABEL = "p87_the_space_menu_says_the_restart_switches_the_seat"
T_SPACE = "p87_a_spaces_menu_restarts_that_spaces_nirvana_claude"
T_NONE = "p87_a_space_without_nirvana_claude_offers_no_restart"
T_ASKS = "p87_a_working_spaces_restart_asks_again"
T_SERVER = "restart_claude_narrows_to_the_named_pane_and_refuses_a_pane_not_in_the_tab"

CONTROLS = (
    # --- U1: the Seats panel. The `restart to switch` words are the click target.
    Control("p87_seats_words_not_hit", SEATS,
            "hits.restart = Rect::new(at, y, shown_width, 1);",
            "hits.restart = Rect::default();",
            T_CLICK, "rust"),
    # A click on the words restarts
    Control("p87_seats_click_ignored", SEATS,
            "} else if on_restart {\n                self.seats_restart(outcome);",
            "} else if on_restart {\n                let _ = outcome;",
            T_CLICK, "rust"),
    # `r` restarts
    Control("p87_seats_r_ignored", SEATS,
            "KeyCode::Char('r') => self.seats_restart(outcome),",
            "KeyCode::Char('r') => {}",
            T_CLICK, "rust"),
    # The footer shows `r restart` while the row asks for it
    Control("p87_seats_footer_no_key", SEATS,
            "enter focus its pane \\u{b7} r restart \\u{b7} m mode",
            "enter focus its pane \\u{b7} m mode",
            T_CLICK, "rust"),
    # The restart names the panel's Nirvana Claude pane, not the whole tab
    Control("p87_seats_whole_tab", SEATS,
            "self.push_restart_claude(workspace_id, tab_id, Some(pane_id), outcome);",
            "self.push_restart_claude(workspace_id, tab_id, None, outcome);",
            T_CLICK, "rust"),
    # U3: it is the tab menu's own command (the one helper every surface calls)
    Control("p87_shared_command_wrong", MENU,
            "command_id: crate::nirvana::RESTART_CLAUDE_COMMAND.into(),",
            'command_id: "nirvana.restart_other".into(),',
            T_CLICK, "rust"),
    # An idle Claude restarts at once
    Control("p87_idle_cuts_a_turn", SEATS,
            "!matches!(status, AgentStatus::Idle | AgentStatus::Done)",
            "!matches!(status, AgentStatus::Done)",
            T_CLICK, "rust"),
    # A working Claude waits for the second press
    Control("p87_seats_working_no_confirm", SEATS,
            "if restart_cuts_a_turn(status) && !armed {",
            "if false && !armed {",
            T_WORKING, "rust"),
    # Armed, the words ask for the second press
    Control("p87_seats_armed_words_same", SEATS,
            "let words = if panel.restart_armed {\n                    RESTART_ARMED_WORDS",
            "let words = if panel.restart_armed {\n                    RESTART_WORDS",
            T_WORKING, "rust"),
    # Any other key puts the armed restart back
    Control("p87_seats_key_keeps_armed", SEATS,
            "if key.code != KeyCode::Char('r') {\n            panel.restart_armed = false;",
            "if key.code == KeyCode::Char('?') {\n            panel.restart_armed = false;",
            T_WORKING, "rust"),
    # No restart once the running seat matches
    Control("p87_seats_restart_when_matching", SEATS,
            "if !orch.restart_to_switch {\n            self.seats_say(",
            "if false {\n            self.seats_say(",
            T_MATCH, "rust"),

    # --- U2: a space's menu.
    Control("p87_menu_item_missing", MENU,
            "items.insert(1, item(label, Action::RestartClaude));",
            "let _ = label;",
            T_SPACE, "rust"),
    # It restarts THAT space's Nirvana Claude, not the first one anywhere
    Control("p87_menu_any_space", SEATS,
            ".find(|agent| agent.workspace_id == workspace_id && is_nirvana(agent))?;",
            ".find(|agent| is_nirvana(agent))?;",
            T_SPACE, "rust"),
    # It names that Claude's pane
    Control("p87_menu_whole_tab", MENU,
            "self.push_restart_claude(workspace_id, target.tab_id, Some(target.pane_id), outcome)",
            "self.push_restart_claude(workspace_id, target.tab_id, None, outcome)",
            T_SPACE, "rust"),
    # Only a Nirvana Claude, not any Claude
    Control("p87_menu_any_claude", SEATS,
            ".is_some_and(crate::nirvana_launch::is_nirvana_agent_name)",
            ".is_some()",
            T_NONE, "rust"),
    # The label says the restart switches the seat
    Control("p87_menu_switch_label", MENU,
            "(false, true) => SPACE_RESTART_SWITCH,",
            "(false, true) => SPACE_RESTART,",
            T_LABEL, "rust"),
    # A working space Claude asks again
    Control("p87_menu_working_read_idle", SEATS,
            "working: restart_cuts_a_turn(agent.agent_status),",
            "working: false,",
            T_ASKS, "rust"),
    Control("p87_menu_working_no_confirm", MENU,
            "target.working && !target.armed {",
            "target.working && target.armed {",
            T_ASKS, "rust"),
    Control("p87_menu_armed_label_same", MENU,
            "(true, _) => SPACE_RESTART_ARMED,",
            "(true, _) => SPACE_RESTART,",
            T_ASKS, "rust"),

    # --- The server narrows Restart Claude to a named pane of the tab.
    Control("p87_server_pane_unparsed", SERVER,
            "Some(pane) => match self.parse_pane_id(pane) {",
            "Some(pane) => match None::<(usize, crate::layout::PaneId)> {",
            T_SERVER, "rust"),
    Control("p87_server_refusal_code", SERVER,
            '"pane_not_found",',
            '"claude_not_restarted",',
            T_SERVER, "rust"),
    Control("p87_server_pane_message", SERVER,
            '"no Claude Code is running in that pane".into()',
            '"no Claude Code is running in this tab".into()',
            T_SERVER, "rust"),
)
