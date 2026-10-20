# Plan 20: every Claude spawn is seated, orchestrator seat follows mode (nirvana/plans/2026-10-20-20-seat-every-spawn.md)
# Controls starting with p20_.

CONTROLS = (
    # Step 1 (§2): The spawn guard booking requirement
    Control(
        'p20_guard_needs_booking',
        'src/nirvana_claude.rs',
        '    let Some(lease) = lease else {\n',
        '    let Some(lease) = lease.filter(|_| false) else {\n',
        'guard_spawn_booking_rules_and_refusal_messages',
        'rust',
    ),
    # Step 1 (§2): The spawn guard agy seat refusal
    Control(
        'p20_guard_agy_refusal',
        'src/nirvana_claude.rs',
        '    if lease_seat_def.as_ref().is_some_and(|s| s.cli == "agy") {\n',
        '    if false && lease_seat_def.as_ref().is_some_and(|s| s.cli == "agy") {\n',
        'guard_spawn_booking_rules_and_refusal_messages',
        'rust',
    ),
    # Step 1 (§2): The spawn guard seat matching
    Control(
        'p20_guard_seat_match',
        'src/nirvana_claude.rs',
        '    if !seat_matches || !effort_ok {\n',
        '    if false && (!seat_matches || !effort_ok) {\n',
        'guard_spawn_booking_rules_and_refusal_messages',
        'rust',
    ),
    # Step 1 (§2): The spawn guard effort cap (spawn effort <= lease effort)
    Control(
        'p20_guard_effort_cap',
        'src/nirvana_claude.rs',
        '    let effort_ok = effort <= lease.effort;\n',
        '    let effort_ok = true || effort <= lease.effort;\n',
        'guard_spawn_booking_rules_and_refusal_messages',
        'rust',
    ),
    # Step 2 (§3.1): Orchestrator seat in Efficient mode is sonnet medium
    Control(
        'p20_orchestrator_efficient_mode',
        'src/nirvana_seat.rs',
        '            let seat = "sonnet".to_string();\n',
        '            let seat = "opus".to_string();\n',
        'orchestrator_seat_efficient_mode',
        'rust',
    ),
    # Step 2 (§3.1): Orchestrator seat in SOTA mode is opus high
    Control(
        'p20_orchestrator_sota_mode',
        'src/nirvana_seat.rs',
        '            let effort = Effort::High;\n',
        '            let effort = Effort::Medium;\n',
        'orchestrator_seat_sota_mode',
        'rust',
    ),
    # Step 2 (§3.1): Orchestrator seat in Balanced mode switches from opus to sonnet at t >= 0.5
    Control(
        'p20_orchestrator_balanced_t_threshold',
        'src/nirvana_seat.rs',
        '                let (seat, effort) = if t < 0.5 {\n',
        '                let (seat, effort) = if t < 0.6 {\n',
        'orchestrator_seat_balanced_at_t_0_5',
        'rust',
    ),
    # Step 2 (§3.1): Orchestrator seat in Balanced mode falls back on stale readings
    Control(
        'p20_orchestrator_balanced_stale',
        'src/nirvana_seat.rs',
        '            if fresh_readings.is_empty() {\n',
        '            if false && fresh_readings.is_empty() {\n',
        'orchestrator_seat_balanced_stale_readings',
        'rust',
    ),
    # Step 2 (§3.1): Orchestrator seat in Balanced mode falls back on missing readings
    Control(
        'p20_orchestrator_balanced_missing',
        'src/nirvana_seat.rs',
        '            if pool_readings.is_empty() {\n',
        '            if false && pool_readings.is_empty() {\n',
        'orchestrator_seat_balanced_missing_readings',
        'rust',
    ),
    # Step 3 (§3.2): Launch args carries rule seat for mode
    Control(
        'p20_launch_args_efficient_seat',
        'src/nirvana_launch.rs',
        '    (seat.model, seat.effort.word().to_owned())\n',
        '    (MODEL.to_owned(), EFFORT.to_owned())\n',
        'launch_args_carries_rule_model_and_effort_in_efficient_mode',
        'rust',
    ),
    # Step 3 (§3.2): Launch args falls back to opus medium when table fails to parse
    Control(
        'p20_launch_table_parse_fallback',
        'src/nirvana_launch.rs',
        '        None => return (MODEL.to_owned(), EFFORT.to_owned()),\n',
        '        None => return ("claude-sonnet-5-5".to_owned(), EFFORT.to_owned()),\n',
        'launch_args_falls_back_to_opus_medium_when_table_fails_to_parse',
        'rust',
    ),
    # Step 3 (§3.2): Restart Claude goes through launch_args
    Control(
        'p20_restart_claude_resumes_through_launch_args',
        'src/app/nirvana_restart.rs',
        '    if is_nirvana {\n',
        '    if false && is_nirvana {\n',
        'restart_argv_for_nirvana_claude_uses_launch_args_and_takes_current_mode',
        'rust',
    ),
    # Step 4 (§3.3): Notice stays quiet when running seat matches rule seat
    Control(
        'p20_notice_same_seat_quiet',
        'src/nirvana_claude.rs',
        '    if running_seat == rule_seat && running_effort == rule_effort {\n',
        '    if false && running_seat == rule_seat && running_effort == rule_effort {\n',
        'seat_notice_reports_once_per_change_and_stays_quiet_when_same',
        'rust',
    ),
    # Step 4 (§3.3): Notice stays quiet on same difference again
    Control(
        'p20_notice_same_diff_quiet',
        'src/nirvana_claude.rs',
        '        if notified_seat == rule_seat && notified_effort == rule_effort {\n',
        '        if false && notified_seat == rule_seat && notified_effort == rule_effort {\n',
        'seat_notice_reports_once_per_change_and_stays_quiet_when_same',
        'rust',
    ),
    # Step 4 (§3.3): Notice message format
    Control(
        'p20_notice_message_format',
        'src/nirvana_claude.rs',
        '        "Nirvana Claude runs on {running_seat} {running_effort}; the mode now seats {rule_seat} {rule_effort} ({rule_detail}). Press Restart Claude to switch."\n',
        '        "Nirvana Claude runs on {running_seat} {running_effort}; mode seats {rule_seat}. Restart."\n',
        'seat_notice_reports_once_per_change_and_stays_quiet_when_same',
        'rust',
    ),
    # Step 4 (§3.3): Session start records running seat in session file
    Control(
        'p20_session_start_records_seat',
        'src/nirvana_claude.rs',
        '        last_notified_effort: None,\n    };\n    let path = session_seat_path(home, session_id);\n',
        '        last_notified_effort: None,\n    };\n    let path = session_seat_path(home, &format!("{session_id}-bad"));\n',
        'session_start_records_running_seat_in_session_file',
        'rust',
    ),
    # Step 4 (§3.3): Seats square orchestrator row text
    Control(
        'p20_seats_square_orchestrator_row',
        'src/client/shell/nirvana_seats.rs',
        '        let mut text = format!(\n            "Nirvana Claude \\u{b7} {} {} \\u{b7} {}",\n',
        '        let mut text = format!(\n            "Nirvana \\u{b7} {} {} \\u{b7} {}",\n',
        'the_seats_square_shows_orchestrator_row_with_and_without_restart_to_switch',
        'rust',
    ),
    # Step 4 (§3.3): Seats square restart to switch text
    Control(
        'p20_seats_square_restart_to_switch',
        'src/client/shell/nirvana_seats.rs',
        '        if orch.restart_to_switch {\n            text.push_str(" \\u{b7} restart to switch");\n        }\n',
        '        if false && orch.restart_to_switch {\n            text.push_str(" \\u{b7} restart to switch");\n        }\n',
        'the_seats_square_shows_orchestrator_row_with_and_without_restart_to_switch',
        'rust',
    ),
    # Step 4 (§3.3): Seats square restart diff logic
    Control(
        'p20_seats_square_restart_diff_logic',
        'src/nirvana_seats_square.rs',
        '        let restart_to_switch = running\n            .as_ref()\n            .is_some_and(|(s, e)| s != &rule.seat || e != rule.effort.word());\n',
        '        let restart_to_switch = running\n            .as_ref()\n            .is_some_and(|(s, e)| false && (s != &rule.seat || e != rule.effort.word()));\n',
        'the_seats_square_shows_orchestrator_row_with_and_without_restart_to_switch',
        'rust',
    ),
    # Step 5 (§2, §3.3): ROLE.md subagent booking rule
    Control(
        'p20_role_subagent_booking',
        'nirvana/claude/ROLE.md',
        '- Every subagent is seated first: `herdr seat pick --book <order>`, then spawn the seat it gives, or `herdr seat run` when it gives an agy seat. The hook refuses a spawn without a booking.\n',
        '- Every subagent is started first: `herdr seat run <order>`. The hook refuses a spawn without a booking.\n',
        'nirvana.test_windows_shell.FilesTest.test_the_agent_is_opus_at_medium_with_the_plans_rules',
        'python',
    ),
    # Step 5 (§2, §3.3): ROLE.md research.web rule
    Control(
        'p20_role_research_web',
        'nirvana/claude/ROLE.md',
        '- Web research, shopping and reading pages are `research.web`: seat them, never browse yourself for more than one page.\n',
        '- Web research, shopping and reading pages are `research.web`: browse them yourself.\n',
        'nirvana.test_windows_shell.FilesTest.test_the_agent_is_opus_at_medium_with_the_plans_rules',
        'python',
    ),
    # Step 5 (§2, §3.3): seat skill subagent booking rule
    Control(
        'p20_seat_skill_booking',
        'nirvana/claude/skills/seat/SKILL.md',
        '  `sonnet-medium`, `sonnet-low`, `haiku-low`). Every subagent is seated first: `herdr seat pick --book <order>`,\n',
        '  `sonnet-medium`, `sonnet-low`, `haiku-low`). Every subagent is started first: `herdr seat run <order>`,\n',
        'nirvana.test_windows_shell.FilesTest.test_seat_skill_and_workers_have_booking_rules',
        'python',
    ),
    # Step 5 (§2, §3.3): seat skill research.web rule
    Control(
        'p20_seat_skill_research_web',
        'nirvana/claude/skills/seat/SKILL.md',
        'Web research, shopping and reading pages are `research.web`: seat them, never browse yourself for more than one page.\n',
        'Web research, shopping and reading pages are `research.web`: browse them yourself.\n',
        'nirvana.test_windows_shell.FilesTest.test_seat_skill_and_workers_have_booking_rules',
        'python',
    ),
    # Step 5 (§2, §3.3): worker agent description booking rule
    Control(
        'p20_worker_description_booking',
        'nirvana/claude/agents/worker-haiku-low.md',
        'A booking comes first: herdr seat pick --book <order>.',
        'A booking is optional: herdr seat pick --book <order>.',
        'every_worker_agent_names_its_model_and_effort',
        'rust',
    ),
)
