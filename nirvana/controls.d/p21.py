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
    # Step 2 (§3.2): Route Row 1 (live mark + folder -> focus or open)
    Control(
        'p21_route_focus_or_open',
        'src/nirvana_launch.rs',
        '        (Some(mark), Some(folder)) => OpenRoute::FocusOrOpenFolder {\n            folder: folder.to_path_buf(),\n            hwnd: mark.hwnd,\n        },\n',
        '        (Some(mark), Some(_folder)) => OpenRoute::FlashWindow { hwnd: mark.hwnd },\n',
        'route_table_covers_all_four_rows_as_data',
        'rust',
    ),
    # Step 2 (§3.2): Route Row 2 (live mark + no folder -> flash)
    Control(
        'p21_route_flash_window',
        'src/nirvana_launch.rs',
        '        (Some(mark), None) => OpenRoute::FlashWindow { hwnd: mark.hwnd },\n',
        '        (Some(_mark), None) => OpenRoute::StartWindowNoFolder,\n',
        'route_table_covers_all_four_rows_as_data',
        'rust',
    ),
    # Step 2 (§3.2): Route Row 3 (no mark + folder -> start window folder)
    Control(
        'p21_route_start_window_folder',
        'src/nirvana_launch.rs',
        '        (None, Some(folder)) => OpenRoute::StartWindowFolder {\n            folder: folder.to_path_buf(),\n        },\n',
        '        (None, Some(_folder)) => OpenRoute::StartWindowNoFolder,\n',
        'route_table_covers_all_four_rows_as_data',
        'rust',
    ),
    # Step 2 (§3.2): Route Row 4 (no mark + no folder -> start window no folder)
    Control(
        'p21_route_start_window_no_folder',
        'src/nirvana_launch.rs',
        '        (None, None) => OpenRoute::StartWindowNoFolder,\n',
        '        (None, None) => OpenRoute::FlashWindow { hwnd: 0 },\n',
        'route_table_covers_all_four_rows_as_data',
        'rust',
    ),
    # Step 2 (§3.2): open.log appends errors
    Control(
        'p21_open_log_appends',
        'src/nirvana_launch.rs',
        '        .append(true)\n',
        '        .append(false)\n',
        'open_log_appends_errors',
        'rust',
    ),
)
