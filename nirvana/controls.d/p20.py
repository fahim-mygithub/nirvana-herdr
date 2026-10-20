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
)
