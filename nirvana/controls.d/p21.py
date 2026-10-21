# Plan 21: one Nirvana window (package `p21`)
# Controls starting with p21_.

CONTROLS = (
    # Step 1 (§3.1): The liveness check requires all three facts
    Control(
        'p21_mark_liveness_all_three',
        'src/nirvana_launch.rs',
        '    process_runs_herdr && window_is_window && server_listening\n',
        '    process_runs_herdr || (window_is_window && server_listening)\n',
        'liveness_check_requires_all_three_facts',
        'rust',
    ),
    # Step 1 (§3.1): Removing mark requires matching pid
    Control(
        'p21_mark_remove_only_by_own_pid',
        'src/nirvana_launch.rs',
        '    if mark.pid == pid {\n',
        '    if true || mark.pid == pid {\n',
        'window_mark_write_read_and_remove_by_own_pid',
        'rust',
    ),
    # Step 1 (§3.1): Torn or empty mark file reads as no mark
    Control(
        'p21_mark_torn_or_empty_reads_as_none',
        'src/nirvana_launch.rs',
        '    serde_json::from_slice(&bytes).ok()\n',
        '    serde_json::from_slice(&bytes).ok().or(Some(WindowMark { pid: 0, hwnd: 0, at: 0 }))\n',
        'torn_or_empty_window_mark_reads_as_no_mark',
        'rust',
    ),
)
