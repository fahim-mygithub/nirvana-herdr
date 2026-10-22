# Plan 23: each project keeps its own seat mode (package `project_mode`)
# Controls starting with p23_.

CONTROLS = (
    # Plan 23 §7 (1): project_mode: no file gives the machine's mode and Machine
    Control(
        'p23_project_mode_no_file_gives_machine',
        'src/nirvana_seat.rs',
        '    let path = project_seats_path(root);\n    if !path.exists() {\n        return Ok((table.mode, ModeSource::Machine));\n    }\n',
        '    let path = project_seats_path(root);\n    if !path.exists() {\n        return Ok((Mode::Sota, ModeSource::Project));\n    }\n',
        'test_p23_project_mode_no_file_gives_machine',
        'rust',
    ),
    # Plan 23 §7 (2): project_mode: a file gives its mode and Project
    Control(
        'p23_project_mode_file_gives_project',
        'src/nirvana_seat.rs',
        '        Some(mode) => Ok((mode, ModeSource::Project)),\n',
        '        Some(_mode) => Ok((table.mode, ModeSource::Machine)),\n',
        'test_p23_project_mode_file_gives_project',
        'rust',
    ),
    # Plan 23 §7 (3): project_mode: a file with another key is refused
    Control(
        'p23_project_mode_refuses_other_key',
        'src/nirvana_seat.rs',
        '        let key = key_raw.trim();\n        if key != "mode" {\n',
        '        let key = key_raw.trim();\n        if false && key != "mode" {\n',
        'test_p23_project_mode_refuses_other_key',
        'rust',
    ),
    # Plan 23 §7 (4): project_mode: a file with a bad value is refused
    Control(
        'p23_project_mode_refuses_bad_value',
        'src/nirvana_seat.rs',
        '        let Some(mode) = Mode::parse(mode_str) else {\n',
        '        let Some(mode) = Mode::parse(mode_str).or(Some(Mode::Balanced)) else {\n',
        'test_p23_project_mode_refuses_bad_value',
        'rust',
    ),
    # Plan 23 §7 (5): Two projects, one machine table, each with its own file, give two different modes from table_for
    Control(
        'p23_two_projects_different_modes',
        'src/nirvana_seat.rs',
        'pub fn table_for(root: &Path) -> Result<Table, String> {\n    let mut table = table()?;\n    let (mode, _source) = project_mode(root, &table)?;\n    table.mode = mode;\n    Ok(table)\n}\n',
        'pub fn table_for(_root: &Path) -> Result<Table, String> {\n    let table = table()?;\n    Ok(table)\n}\n',
        'test_p23_two_projects_different_modes',
        'rust',
    ),
    # Plan 23 §7 (6): --mode beats the project\'s file in herdr seat pick\'s request
    Control(
        'p23_pick_mode_flag_beats_project_file',
        'src/nirvana_seat.rs',
        '            "--mode" => request.mode = Mode::parse(&value()).unwrap_or_else(|| usage_exit()),\n',
        '            "--mode" => drop(value()),\n',
        'test_p23_pick_mode_flag_beats_project_file',
        'rust',
    ),
    # Plan 23 §7 (7): The worker supervisor\'s pick uses the project\'s mode when the order names none
    Control(
        'p23_worker_supervisor_uses_project_mode',
        'src/nirvana_worker.rs',
        '    pick_request.mode = request.mode.unwrap_or(table.mode);\n',
        '    pick_request.mode = request.mode.unwrap_or(nirvana_seat::Mode::Balanced);\n',
        'worker_supervisor_uses_project_mode_when_order_names_none',
        'rust',
    ),
    # Plan 23 §7 (8): orchestrator_seat through table_for: Efficient in one project gives Sonnet medium while SOTA in another gives Opus high
    Control(
        'p23_orchestrator_seat_table_for',
        'src/nirvana_launch.rs',
        '            Ok((mode, _)) => table.mode = mode,\n',
        '            Ok((_mode, _)) => table.mode = crate::nirvana_seat::Mode::Balanced,\n',
        'test_p23_orchestrator_seat_table_for',
        'rust',
    ),
    # Plan 23 §7 (9): herdr seat mode <m> writes only the project\'s file and leaves the global line\'s bytes unchanged
    Control(
        'p23_seat_mode_writes_only_project_file',
        'src/nirvana_seats_square.rs',
        '    nirvana_seat::set_project_mode(root, to)?;\n',
        '    nirvana_seat::set_mode_in(&seats_dir(), to)?;\n',
        'seat_mode_writes_only_project_file',
        'rust',
    ),
    # Plan 23 §7 (10): herdr seat mode --machine <m> writes only the global line
    Control(
        'p23_seat_mode_machine_writes_only_global_line',
        'src/nirvana_seats_square.rs',
        'pub fn change_machine_mode(root: &Path, to: Mode, by: &str) -> Result<String, String> {\n    let previous = nirvana_seat::set_mode_in(&seats_dir(), to)?;\n',
        'pub fn change_machine_mode(root: &Path, to: Mode, by: &str) -> Result<String, String> {\n    let _ = nirvana_seat::set_project_mode(root, to);\n    let previous = nirvana_seat::set_mode_in(&seats_dir(), to)?;\n',
        'seat_mode_machine_writes_only_global_line',
        'rust',
    ),
    # Plan 23 §7 (11): herdr seat mode --clear removes the project\'s file
    Control(
        'p23_seat_mode_clear_removes_project_file',
        'src/nirvana_seats_square.rs',
        '    let file = root.join(nirvana_run::DIR).join(nirvana_seat::SEATS_FILE);\n    if file.exists() {\n        std::fs::remove_file(&file).map_err(|err| format!("{}: {err}", file.display()))?;\n    }\n',
        '    let file = root.join(nirvana_run::DIR).join(nirvana_seat::SEATS_FILE);\n    if false && file.exists() {\n        std::fs::remove_file(&file).map_err(|err| format!("{}: {err}", file.display()))?;\n    }\n',
        'seat_mode_clear_removes_project_file',
        'rust',
    ),
    # Plan 23 §7 (12): The restart notice for project A does not fire when project B\'s mode changes
    Control(
        'p23_restart_notice_isolation',
        'src/nirvana_board.rs',
        '        Some(r) => crate::nirvana_seat::table_for(r).ok()?,\n',
        '        Some(_) => crate::nirvana_seat::table().ok()?,\n',
        'restart_notice_for_project_a_does_not_fire_when_project_b_mode_changes',
        'rust',
    ),
    # Plan 23 §7 (13): The Seats square\'s mode row shows the source word
    Control(
        'p23_seats_square_mode_row_shows_source_word',
        'src/client/shell/nirvana_seats.rs',
        '                if let Some(source) = shown.mode_source {\n                    let source_text = format!("\\u{b7} {}", source.word());\n                    put_text(\n                        buffer,\n                        at,\n                        y,\n                        (x + width).saturating_sub(at),\n                        &source_text,\n                        muted,\n                    );\n                    at += unicode_width::UnicodeWidthStr::width(source_text.as_str()) as u16 + 1;\n                }\n',
        '                if false {\n                    if let Some(source) = shown.mode_source {\n                        let source_text = format!("\\u{b7} {}", source.word());\n                        put_text(\n                            buffer,\n                            at,\n                            y,\n                            (x + width).saturating_sub(at),\n                            &source_text,\n                            muted,\n                        );\n                        at += unicode_width::UnicodeWidthStr::width(source_text.as_str()) as u16 + 1;\n                    }\n                }\n',
        'the_seats_square_mode_row_shows_the_source_word',
        'rust',
    ),
    # Reviewer fix: a machine change is not logged in a project that keeps its own mode
    Control(
        'p23_machine_change_skips_own_mode_project',
        'src/nirvana_seats_square.rs',
        '    if !follows_machine {',
        '    if false {',
        'seat_mode_machine_writes_only_global_line',
        'rust',
    ),
    # Review S5: herdr seat pick reads the project's table, not the machine's
    Control(
        'p23_pick_reads_project_table',
        'src/nirvana_seat.rs',
        '    let table = table_for(root)?;\n    let mut picked = PickArgs {',
        '    let table = table()?;\n    let mut picked = PickArgs {',
        'test_p23_pick_mode_flag_beats_project_file',
        'rust',
    ),
    # Review S2: an empty or comments-only file is the machine's mode
    Control(
        'p23_empty_file_is_machine',
        'src/nirvana_seat.rs',
        '        None => Ok((table.mode, ModeSource::Machine)),\n    }',
        '        None => Err("no mode".into()),\n    }',
        'test_p23_an_empty_or_marked_file_reads_as_the_machine',
        'rust',
    ),
    # Review N3: an editor's byte-order mark is not part of the key
    Control(
        'p23_bom_stripped',
        'src/nirvana_seat.rs',
        "    let text = text.strip_prefix('\\u{feff}').unwrap_or(text);\n",
        '',
        'test_p23_an_empty_or_marked_file_reads_as_the_machine',
        'rust',
    ),
    # Review S1: a refused file is never rewritten by a set
    Control(
        'p23_refused_file_untouched',
        'src/nirvana_seat.rs',
        '        parse_project_seats(&path, &text).map_err(|why| format!("{why}; {CLEAR_HINT}"))?;\n        text\n',
        '        let _ = parse_project_seats(&path, &text);\n        text\n',
        'test_p23_an_empty_or_marked_file_reads_as_the_machine',
        'rust',
    ),
    # Review S1: --clear removes a refused file
    Control(
        'p23_clear_refused_file',
        'src/nirvana_seats_square.rs',
        '    let before = nirvana_seat::project_mode(root, &machine_table).ok();\n',
        '    let before = Some(nirvana_seat::project_mode(root, &machine_table)?);\n',
        'a_refused_file_clears_and_a_folder_outside_a_project_is_left_alone',
        'rust',
    ),
    # Review S3: a folder outside a project is left alone
    Control(
        'p23_not_a_project',
        'src/nirvana_seats_square.rs',
        '            if !is_project(root) {\n',
        '            if false {\n',
        'a_refused_file_clears_and_a_folder_outside_a_project_is_left_alone',
        'rust',
    ),
    # Review S6: launch_args carries the project's seat
    Control(
        'p23_launch_args_override',
        'src/nirvana_launch.rs',
        '    let (model, effort) = orchestrator_launch_seat_for(home, folder);\n',
        '    let (model, effort) = orchestrator_launch_seat(home);\n',
        'test_p23_orchestrator_seat_table_for',
        'rust',
    ),
)
