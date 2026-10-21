# Nirvana Claude back after a reload, and an agents panel per space (alpha 0.34.0). The owner, 2026-10-21: "when you
# click restart we can resume the the process as if we right clicked the folder and clicked open nirvana here" and "I
# sitll see both spaces have both agents listed on the agents panal". Controls starting with rl_.

CONTROLS = (
    # The restore marks a pane that ran Nirvana Claude
    Control(
        'rl_restore_marks_nirvana_pane',
        'src/persist/restore.rs',
        '(Some(_), Some(_)) => terminal.nirvana_relaunch = nirvana_relaunch,',
        '(Some(_), Some(_)) => {}',
        'a_nirvana_pane_is_marked_for_nirvana_claude_and_never_gets_plain_claude_resume',
        'rust',
    ),
    # A Nirvana pane never gets herdr's plain `claude --resume`
    Control(
        'rl_restore_no_plain_resume',
        'src/persist/restore.rs',
        '            .and_then(|p| p.agent_session.as_ref())\n            .filter(|_| nirvana_relaunch.is_none());\n',
        '            .and_then(|p| p.agent_session.as_ref());\n',
        'a_nirvana_pane_is_marked_for_nirvana_claude_and_never_gets_plain_claude_resume',
        'rust',
    ),
    # The relaunch keeps the name the pane ran under
    Control(
        'rl_start_keeps_name',
        'src/app/nirvana_new.rs',
        '            Some(name) => name.to_owned(),\n',
        '            Some(_) => "nirvana".to_owned(),\n',
        'a_restored_nirvana_pane_gets_nirvana_claude_under_its_own_name',
        'rust',
    ),
    # A started pane is started once
    Control(
        'rl_clears_mark',
        'src/app/nirvana_relaunch.rs',
        '                    terminal.nirvana_relaunch = None;\n',
        '                    let _ = &terminal;\n',
        'a_restored_nirvana_pane_gets_nirvana_claude_under_its_own_name',
        'rust',
    ),
    # A shell that never comes is given up after the wait
    Control(
        'rl_gives_up',
        'src/app/nirvana_relaunch.rs',
        ')) if now < until => {',
        ')) => {',
        'a_marked_pane_whose_shell_never_comes_waits_then_gives_up',
        'rust',
    ),
    # The agents panel lists the selected space's agents only
    Control(
        'rl_panel_filters_by_space',
        'src/client/shell/agent_sidebar.rs',
        '.any(|agent| agent.pane_id == row.pane_id && agent.workspace_id == space)',
        '.any(|agent| agent.pane_id == row.pane_id)',
        'the_agents_panel_lists_the_selected_space_only',
        'rust',
    ),
)
