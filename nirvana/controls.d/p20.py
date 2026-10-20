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
        '    if lease_seat_def.as_ref().map_or(false, |s| s.cli == "agy") {\n',
        '    if false && lease_seat_def.as_ref().map_or(false, |s| s.cli == "agy") {\n',
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
)
