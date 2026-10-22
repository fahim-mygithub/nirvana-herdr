# The worker tab drawn like Claude Code (alpha 0.35.0). The owner, 2026-10-22: "I htought agy was similar to claude
# code in its shell reporting", then "go ahead with option 1". Controls starting with wv_.

CONTROLS = (
    # agy's capital-P Path is a file, so read_file names it
    Control(
        'wv_agy_path_key',
        'src/nirvana_worker/view.rs',
        '        "Path",\n',
        '',
        'agy_names_map_to_claude_code_and_paths_drop_the_worker_folder',
        'rust',
    ),
    # agy's tool names become Claude Code's
    Control(
        'wv_agy_name_map',
        'src/nirvana_worker/view.rs',
        '"read_file" | "view_file" => "Read",',
        '"view_file" => "Read",',
        'agy_names_map_to_claude_code_and_paths_drop_the_worker_folder',
        'rust',
    ),
    # A path drops the worker's folder
    Control(
        'wv_short_path',
        'src/nirvana_worker/view.rs',
        'let path = unify(path);',
        'let path = unify(path);\n    let cwd: Option<&str> = cwd.and(None);',
        'agy_names_map_to_claude_code_and_paths_drop_the_worker_folder',
        'rust',
    ),
    # An agy tool's DONE or ERROR step is drawn as its answer
    Control(
        'wv_agy_answer',
        'src/nirvana_worker.rs',
        'Some(state @ ("DONE" | "ERROR")) => {',
        'Some(state @ "NEVER") => {',
        'a_recorded_stream_draws_like_claude_code',
        'rust',
    ),
    # A Claude tool_result is drawn as its answer
    Control(
        'wv_claude_answer',
        'src/nirvana_worker.rs',
        '("user", _) => {',
        '("user-never", _) => {',
        'a_recorded_stream_draws_like_claude_code',
        'rust',
    ),
    # An answer away from its call names the call
    Control(
        'wv_label_when_apart',
        'src/nirvana_worker.rs',
        'let label = (self.last_call.as_deref() != Some(id) && !label.is_empty()).then_some(label);',
        'let label: Option<String> = Some(label).filter(|_| false);',
        'a_recorded_stream_draws_like_claude_code',
        'rust',
    ),
    # A Claude edit answers "done", not the CLI's sentence
    Control(
        'wv_claude_edit_done',
        'src/nirvana_worker/view.rs',
        'if matches!(raw, "Edit" | "MultiEdit" | "Write" | "NotebookEdit") {',
        'if false {',
        'a_recorded_stream_draws_like_claude_code',
        'rust',
    ),
    # A failed command shows its exit code; a clean one does not
    Control(
        'wv_exit_code',
        'src/nirvana_worker/view.rs',
        'if let Some(code) = code.filter(|code| *code != 0) {',
        'if let Some(code) = code.filter(|_| false) {',
        'agy_results_read_as_lines_counts_exits_and_task_status',
        'rust',
    ),
    # Colour only when asked: `herdr seat read` feeds Nirvana Claude plain text
    Control(
        'wv_paint_only_when_asked',
        'src/nirvana_worker/view.rs',
        '        if self.0 {',
        '        if true {',
        'notes_draw_like_claude_code_and_paint_only_when_asked',
        'rust',
    ),
    # agy's JSON report reads as a report
    Control(
        'wv_report_lines',
        'src/nirvana_worker/view.rs',
        'let status = value.get("status")?.as_str()?;',
        'let status: &str = None?;',
        'notes_draw_like_claude_code_and_paint_only_when_asked',
        'rust',
    ),
    # 0.35.1: the call mark is one the owner's font draws
    Control(
        'wv_dot',
        'src/nirvana_worker/view.rs',
        'pub const DOT: &str = "●";',
        'pub const DOT: &str = "⏺";',
        'notes_draw_like_claude_code_and_paint_only_when_asked',
        'rust',
    ),
    # A command drops the worker's own cd
    Control(
        'wv_drop_cd',
        'src/nirvana_worker/view.rs',
        'clip(&drop_cd(command, cwd))',
        'clip(command)',
        'a_command_drops_the_workers_own_cd',
        'rust',
    ),
    # A cd anywhere else stays
    Control(
        'wv_cd_elsewhere',
        'src/nirvana_worker/view.rs',
        'if unify(target) == unify(cwd) {',
        'if !target.is_empty() {',
        'a_command_drops_the_workers_own_cd',
        'rust',
    ),
    # The call that carries the report does not show in the pane
    Control(
        'wv_hidden_call',
        'src/nirvana_worker.rs',
        '        if view::hidden(&self.cli, raw) {',
        '        if false {',
        'an_attempt_tees_the_stream_and_journals_its_start_and_its_usage',
        'rust',
    ),
    # Nor does its answer
    Control(
        'wv_hidden_answer',
        'src/nirvana_worker.rs',
        '        if view::hidden(&self.cli, &raw) {',
        '        if false {',
        'an_attempt_tees_the_stream_and_journals_its_start_and_its_usage',
        'rust',
    ),
    # A wrapped row starts under its text
    Control(
        'wv_fit_hang',
        'src/nirvana_worker/view.rs',
        'let indent = hang(&visible).min(width / 2);',
        'let indent = 0;',
        'long_lines_wrap_under_their_text',
        'rust',
    ),
    # A row breaks at a space
    Control(
        'wv_fit_space',
        'src/nirvana_worker/view.rs',
        "if ch == ' ' && used >= indent && used > 0 {",
        'if false {',
        'long_lines_wrap_under_their_text',
        'rust',
    ),
    # Colour codes take no columns
    Control(
        'wv_fit_codes',
        'src/nirvana_worker/view.rs',
        'let visible = strip_codes(line);',
        'let visible = line.clone();',
        'long_lines_wrap_under_their_text',
        'rust',
    ),
    # A split leaves readable halves
    Control(
        'pl_min_size',
        'src/nirvana_worker/place.rs',
        'cols >= MIN_COLS && rows >= MIN_ROWS',
        'cols > 0 && rows > 0',
        'a_full_tab_takes_over_the_oldest_finished_pane',
        'rust',
    ),
    # A cell is twice as tall as wide
    Control(
        'pl_look',
        'src/nirvana_worker/place.rs',
        'u32::from(cols).min(u32::from(rows) * 2)',
        'u32::from(cols)',
        'a_wide_pane_splits_side_by_side_then_the_halves_split_down',
        'rust',
    ),
    # Only a finished pane is taken over
    Control(
        'pl_reuse_finished',
        'src/nirvana_worker/place.rs',
        '.filter(|slot| slot.finished)',
        '.filter(|slot| !slot.finished)',
        'a_full_tab_takes_over_the_oldest_finished_pane',
        'rust',
    ),
    # The oldest finished pane first
    Control(
        'pl_oldest',
        'src/nirvana_worker/place.rs',
        '.min_by_key(|slot| slot.age.unwrap_or(usize::MAX))',
        '.max_by_key(|slot| slot.age.unwrap_or(usize::MAX))',
        'a_full_tab_takes_over_the_oldest_finished_pane',
        'rust',
    ),
    # A pane with anything but a shell running is busy
    Control(
        'pl_only_shells',
        'src/nirvana_worker.rs',
        '"pwsh" | "powershell" | "cmd" | "bash" | "zsh" | "sh" | "fish" | "nu"',
        '"pwsh" | "powershell" | "cmd" | "bash" | "zsh" | "sh" | "fish" | "nu" | "herdr"',
        'a_pane_is_taken_over_only_when_its_worker_ended_and_a_shell_alone_runs',
        'rust',
    ),
    # A deferred worker still owns its pane
    Control(
        'pl_finished_phases',
        'src/nirvana_worker.rs',
        '        Phase::Delivered\n            | Phase::Interrupted',
        '        Phase::Delivered\n            | Phase::Deferred\n            | Phase::Interrupted',
        'a_pane_is_taken_over_only_when_its_worker_ended_and_a_shell_alone_runs',
        'rust',
    ),
)
