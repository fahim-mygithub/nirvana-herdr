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
)
