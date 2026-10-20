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
)
