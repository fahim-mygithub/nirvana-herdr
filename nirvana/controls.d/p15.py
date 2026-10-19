# Plan 15: Jobs, long commands in view (plans/2026-10-19-15-jobs.md). Red line for each: its test prints FAIL.
CONTROLS = (
    # §2: Job state determination (passed when exit is 0).
    Control(
        'p15_state_passed_zero_exit',
        'src/nirvana_job.rs',
        '            if code == 0 {\n                JobState::Passed\n',
        '            if code != 0 {\n                JobState::Passed\n',
        'job_states_cover_running_passed_failed_lost',
        'rust',
    ),
    # §2: Job state determination (lost when process is dead and no exit was recorded).
    Control(
        'p15_state_lost_dead_process',
        'src/nirvana_job.rs',
        '        } else {\n            JobState::Lost\n        }\n',
        '        } else {\n            JobState::Running\n        }\n',
        'lost_detection_identifies_dead_process_without_exit',
        'rust',
    ),
    # §2: clean removes jobs that ended over 7 days ago.
    Control(
        'p15_clean_jobs_seven_days',
        'src/nirvana_job.rs',
        'pub const CLEAN_MAX_AGE_SECS: u64 = 7 * 24 * 3600;\n',
        'pub const CLEAN_MAX_AGE_SECS: u64 = 1 * 24 * 3600;\n',
        'clean_jobs_removes_only_jobs_ended_over_7_days_ago',
        'rust',
    ),
    # §2: stop_job ends the job process tree.
    Control(
        'p15_stop_job_tree',
        'src/nirvana_job.rs',
        '    if let Some(pid) = record.pid {\n        kill_process_tree(pid);\n    }\n',
        '    if let Some(_pid) = record.pid {\n        // kill_process_tree(pid);\n    }\n',
        'stop_job_ends_only_job_process_tree',
        'rust',
    ),
    # §2: herdr job exec trailing summary line [job <name>: exit N after Xs].
    Control(
        'p15_exec_trailing_line',
        'src/nirvana_job.rs',
        '    let end_line = format!("[job {}: exit {exit_code} after {elapsed}s]", record.name);\n',
        '    let end_line = format!("[done {}: exit {exit_code}]", record.name);\n',
        'exec_runs_command_copies_output_to_log_and_records_exit',
        'rust',
    ),
    # §2: Outside herdr with no server, run_job runs in place and returns exit code.
    Control(
        'p15_run_in_place_fallback',
        'src/nirvana_job.rs',
        '        let code = run_exec(home, &id)?;\n        return Ok((id, Some(code)));\n',
        '        let code = run_exec(home, &id)?;\n        return Ok((id, None));\n',
        'run_job_with_no_server_runs_in_place_and_records_job',
        'rust',
    ),
    # §3: Claude Code folder encoding replaces non-alphanumeric with hyphens.
    Control(
        'p15_claude_folder_encoding',
        'src/nirvana_job.rs',
        "        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })\n",
        "        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })\n",
        'claude_folder_encoding_replaces_non_alphanumeric_with_hyphens',
        'rust',
    ),
    # §3: Claude task state is unknown after 24h untouched with no exit line.
    Control(
        'p15_claude_task_state_unknown',
        'src/nirvana_job.rs',
        '            if now_secs.saturating_sub(modified_secs) >= 24 * 3600 {\n',
        '            if now_secs.saturating_sub(modified_secs) >= 48 * 3600 {\n',
        'claude_task_state_and_exit_parsing_covers_running_passed_failed_unknown',
        'rust',
    ),
    # §3: Claude task listing filters to files modified in last 12 hours.
    Control(
        'p15_claude_task_twelve_hours',
        'src/nirvana_job.rs',
        '    let mut tasks = Vec::new();\n    let twelve_hours = 12 * 3600;\n',
        '    let mut tasks = Vec::new();\n    let twelve_hours = 24 * 3600;\n',
        'claude_task_list_filters_twelve_hours_sorts_newest_first_and_limits_to_ten',
        'rust',
    ),
    # §4: Jobs square narrow label format ("2 run").
    Control(
        'p15_jobs_square_narrow_form',
        'src/client/shell/nirvana_jobs.rs',
        '            format!("{} run", view.running_count)\n',
        '            format!("{} r", view.running_count)\n',
        'the_jobs_square_reads_jobs_when_empty_and_shows_narrow_and_wide_forms',
        'rust',
    ),
    # §4: Click on Jobs square opens the panel.
    Control(
        'p15_jobs_square_click_open',
        'src/client/shell/mouse.rs',
        '                if super::contains(self.hits.nirvana_jobs, point) {\n',
        '                if false && super::contains(self.hits.nirvana_jobs, point) {\n',
        'jobs_square_click_opens_panel_and_renders_rows',
        'rust',
    ),
    # §4: Click on a job row focuses its pane in the Jobs tab.
    Control(
        'p15_jobs_panel_focus_job_pane',
        'src/client/shell/nirvana_jobs.rs',
        '                        self.push_endpoint_method(\n                            crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {\n',
        '                        self.push_endpoint_method(\n                            crate::api::schema::Method::PaneClose(crate::api::schema::PaneTarget {\n',
        'click_on_job_focuses_pane_and_esc_closes_panel',
        'rust',
    ),
)
