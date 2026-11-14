# Dead panes (nirvana/plans/2026-11-08-dead-panes.md): finished workers stop counting and their panes close.
# Controls starting with dead_. One sabotage per assertion, each on a product line; the test named must print FAIL with
# it in place (the expected red test is the fifth field, written before the run). `Control` is provided by controls.py.
DEAD = "src/nirvana_dead.rs"
SUBAGENTS = "src/nirvana_subagents.rs"
CLAUDE = "src/nirvana_claude.rs"
WORKER = "src/nirvana_worker.rs"
APP = "src/app/nirvana_dead_panes.rs"

_D = "nirvana_dead::tests::"
T_GRACE = _D + "a_finished_workers_bare_shell_pane_closes_after_the_grace"
T_RUNS = _D + "a_pane_whose_worker_still_runs_is_never_closed"
T_PROGRAM = _D + "a_pane_running_a_program_is_never_closed"
T_TYPED = _D + "a_pane_the_owner_typed_in_after_its_worker_ended_is_kept"
T_AGENT_TAB = _D + "the_spaces_agent_tab_and_an_agent_pane_are_never_closed"
T_WORKER_AGENT = _D + "a_live_workers_agent_keeps_only_its_own_pane"
T_LAST = _D + "a_run_tabs_panes_all_close_but_a_spaces_last_pane_stays"
T_JOBS = _D + "an_ended_jobs_pane_closes_and_a_running_ones_stays"
T_NO_RECORD = _D + "a_labelled_pane_no_record_names_is_kept"
T_RELABEL = _D + "a_pane_relabelled_since_the_list_is_kept"
T_DRY = _D + "a_dry_run_names_each_verdict_and_closes_nothing"
T_LIVE = _D + "order_live_reads_the_supervisor_not_the_phase"
T_LABELS = _D + "labels_name_a_worker_or_a_job"
T_ROWS_DEAD = "nirvana_subagents::tests::a_worker_with_no_supervisor_stops_counting"
T_ROWS_PHASES = "nirvana_subagents::tests::read_seat_orders_phases_and_ended_runs"
T_ROWS_GRACE = "nirvana_subagents::tests::an_ended_run_leaves_the_panel_after_the_grace"
T_STATUS = "nirvana_claude::tests::the_statusline_names_an_order_with_no_supervisor_stopped"
# Review round (dead-review.md)
T_REAP = "app::nirvana_dead_panes::tests::a_hung_sweep_is_killed_logged_and_frees_the_next_pass"
T_CALL = _D + "a_herdr_call_past_its_limit_is_killed"
T_SCREEN = _D + "a_tab_on_screen_keeps_its_panes"
T_CAP = _D + "a_pass_closes_at_most_the_cap_and_the_next_takes_the_rest"
T_PROBE = _D + "a_lock_probe_error_reads_live_and_keeps_the_pane"
T_STOPPED = _D + "a_worker_stopped_mid_run_keeps_its_pane_an_hour"
T_SEATING = _D + "a_run_seating_a_worker_keeps_its_panes"
T_GONE = _D + "a_pane_gone_at_its_close_counts_as_gone"
T_SESSION = _D + "the_sweep_timer_runs_only_in_the_nirvana_session"
T_KEY = _D + "an_unreadable_key_record_keeps_the_pane"
T_EMPTY = _D + "an_empty_process_list_keeps_the_pane"

CONTROLS = (
    # 0.73.1: an ended order of a live run leaves by its own rule, not with the run.
    Control("dead_context_rows_leave", SUBAGENTS,
            "            if !order_shown {\n",
            "            if false && !order_shown {\n",
            T_ROWS_GRACE, "rust"),
    # The close rule (`decide`) and the sweep
    Control("dead_grace", DEAD,
            "    if now < since.saturating_add(grace) {\n",
            "    if false {\n",
            T_GRACE, "rust"),
    Control("dead_supervisor_keeps", DEAD,
            "                    return Ended::Running;\n",
            "                    return Ended::At(0);\n",
            T_RUNS, "rust"),
    Control("dead_program_keeps", DEAD,
            "Some(list) if !crate::nirvana_worker::only_shells(&list) => {",
            "Some(list) if false && !crate::nirvana_worker::only_shells(&list) => {",
            T_PROGRAM, "rust"),
    Control("dead_owner_typed", DEAD,
            "Ok(Some(key)) if *key >= ended_at => {",
            "Ok(Some(key)) if *key >= u64::MAX => {",
            T_TYPED, "rust"),
    # 0.71.2: a worker's own agent keeps only its own pane, not the finished shells beside it.
    Control("dead_worker_agent_keeps_tab", DEAD,
            '        .filter(|pane| parse_label(str_at(pane, &["/label"]).unwrap_or("")).is_none())\n',
            "", T_WORKER_AGENT, "rust"),
    Control("dead_agent_tab", DEAD,
            '            tab_has_agent: agent_tabs.contains(&tab) || str_at(pane, &["/agent"]).is_some(),\n',
            "            tab_has_agent: false,\n",
            T_AGENT_TAB, "rust"),
    Control("dead_space_last_pane", DEAD,
            "                && tabs_in_space.get(&space).copied().unwrap_or(0) <= 1,\n",
            "                && false,\n",
            T_LAST, "rust"),
    Control("dead_counts_the_closed", DEAD,
            "    *left = left.saturating_sub(1);\n",
            "    *left = left.saturating_add(0);\n",
            T_LAST, "rust"),
    Control("dead_job_running", DEAD,
            "                crate::nirvana_job::JobState::Running => Ended::Running,\n",
            "                crate::nirvana_job::JobState::Running => Ended::At(0),\n",
            T_JOBS, "rust"),
    Control("dead_job_name", DEAD,
            "            if record.name != *name {\n",
            "            if false {\n",
            T_JOBS, "rust"),
    Control("dead_seat_match", DEAD,
            "                if known.seated.seat != *seat {\n",
            "                if false {\n",
            T_NO_RECORD, "rust"),
    Control("dead_relabel", DEAD,
            "        if label_now.as_deref() != Some(label) {\n",
            "        if false {\n",
            T_RELABEL, "rust"),
    Control("dead_dry_run", DEAD,
            "        if pass.dry_run {\n",
            "        if false {\n",
            T_DRY, "rust"),
    Control("dead_label_form", DEAD,
            "(nirvana_run::valid_id(head) && nirvana_run::valid_id(tail))",
            "(!head.is_empty() && !tail.is_empty())",
            T_LABELS, "rust"),
    # Review round: S1 a hung sweep or call is killed
    Control("dead_reap_deadline", APP,
            "    let failed = match crate::nirvana_dead::wait_within(&mut child, limit) {\n",
            "    let failed = match child.wait().map(Some) {\n",
            T_REAP, "rust"),
    Control("dead_reap_frees", APP,
            "    SWEEPING.store(false, Ordering::SeqCst);\n}\n",
            "    let _ = &SWEEPING;\n}\n",
            T_REAP, "rust"),
    Control("dead_reap_logs", APP,
            "            crate::nirvana_dead::log_lines(home, &[line]);\n",
            "            let _ = (home, line);\n",
            T_REAP, "rust"),
    Control("dead_call_limit", DEAD,
            "        if left.is_zero() {\n",
            "        if false && left.is_zero() {\n",
            T_CALL, "rust"),
    # S2 a tab on screen stays, and a pass closes at most MAX_CLOSES
    Control("dead_watched_keeps", DEAD,
            "    if facts.watched {\n",
            "    if false {\n",
            T_SCREEN, "rust"),
    Control("dead_watched_tab_focus", DEAD,
            "        if is_true(tab, \"/focused\") {\n",
            "        if false {\n",
            T_SCREEN, "rust"),
    Control("dead_watched_pane_focus", DEAD,
            "        if is_true(pane, \"/focused\") {\n",
            "        if false {\n",
            T_SCREEN, "rust"),
    Control("dead_watched_clients", DEAD,
            "    let mut watched = pass.watched_tabs.clone();\n",
            "    let mut watched: HashSet<String> = HashSet::new();\n",
            T_SCREEN, "rust"),
    Control("dead_cap", DEAD,
            "        if !pass.dry_run && tries >= MAX_CLOSES {\n",
            "        if false {\n",
            T_CAP, "rust"),
    Control("dead_cap_counts", DEAD,
            "        tries += 1;\n",
            "        tries += 0;\n",
            T_CAP, "rust"),
    # S3 a lock probe error reads live (Windows)
    Control("dead_probe_error_live", WORKER,
            "            Err(err) => err.kind() != std::io::ErrorKind::NotFound,\n",
            "            Err(err) => held(&err),\n",
            T_PROBE, "rust"),
    # S4 a worker stopped mid-run keeps its pane an hour, from the later of its stop and the server's start
    Control("dead_stopped_grace", DEAD,
            "            STOPPED_GRACE_S,\n",
            "            CLOSE_GRACE_S,\n",
            T_STOPPED, "rust"),
    Control("dead_stopped_from_start", DEAD,
            "            at.max(server_started),\n",
            "            at,\n",
            T_STOPPED, "rust"),
    Control("dead_stopped_phase", DEAD,
            "                if known.phase.wants_worker() {\n",
            "                if false {\n",
            T_STOPPED, "rust"),
    # S5 a run seating a worker keeps its panes; a pane gone at its close counts as gone
    Control("dead_seating", DEAD,
            "                        && now.saturating_sub(other.seated_at) < SEAT_GRACE_S\n",
            "                        && false\n",
            T_SEATING, "rust"),
    Control("dead_gone_at_close", DEAD,
            "not a failure.\n            Err((code, _)) if gone(&code) => {\n",
            "not a failure.\n            Err((code, _)) if false && gone(&code) => {\n",
            T_GONE, "rust"),
    Control("dead_gone_counts", DEAD,
            "                forget(&mut panes_in_tab, &mut tabs_in_space, &tab, &space);\n            }\n            Err((_, why))",
            "                let _ = (&tab, &space);\n            }\n            Err((_, why))",
            T_GONE, "rust"),
    # S6 the timer runs only in the Nirvana session
    Control("dead_session_gate", DEAD,
            "    session == Some(crate::nirvana_launch::SESSION)\n",
            "    session.is_some() || session.is_none()\n",
            T_SESSION, "rust"),
    # N4 an unreadable key record and an empty process list keep the pane
    Control("dead_key_unreadable", DEAD,
            "        Err(_) => return keep(\"the owner's last key in it could not be read\"),\n",
            "        Err(_) => {}\n",
            T_KEY, "rust"),
    Control("dead_empty_processes", DEAD,
            "        Some(list) if list.is_empty() => keep(",
            "        Some(list) if false && list.is_empty() => keep(",
            T_EMPTY, "rust"),
    # The liveness rule the rows and the statusline read
    Control("dead_live_needs_supervisor", DEAD,
            "        && (supervised || (phase == Phase::Seated && now",
            "        && (true || (phase == Phase::Seated && now",
            T_ROWS_DEAD, "rust"),
    Control("dead_seat_grace", DEAD,
            "(phase == Phase::Seated && now.saturating_sub(seated_at) < SEAT_GRACE_S)",
            "(false && now.saturating_sub(seated_at) < SEAT_GRACE_S)",
            T_ROWS_PHASES, "rust"),
    Control("dead_word_stopped", DEAD,
            "    if phase.wants_worker() && !live {\n",
            "    if false && !live {\n",
            T_LIVE, "rust"),
    # The agents panel's and the spaces' rows (`read_seat_orders`)
    Control("dead_rows_read_the_lock", SUBAGENTS,
            "                && crate::nirvana_worker::live(&project, &run_id, order.id());\n",
            "                && false;\n",
            T_ROWS_PHASES, "rust"),
    Control("dead_rows_time_stops", SUBAGENTS,
            "            let ended_at = if !is_live { Some(order.last_at) } else { None };\n",
            "            let ended_at = if !is_live { None } else { None };\n",
            T_ROWS_DEAD, "rust"),
    Control("dead_rows_asked_blocked", SUBAGENTS,
            "            } else if order.phase == crate::nirvana_run::Phase::Asked {\n",
            "            } else if false {\n",
            T_ROWS_DEAD, "rust"),
    Control("dead_rows_died_stopped", SUBAGENTS,
            "                || order.phase.wants_worker()\n",
            "                || false\n",
            T_ROWS_DEAD, "rust"),
    # The statusline's order list
    Control("dead_status_reads_the_lock", CLAUDE,
            "                && crate::nirvana_worker::live(project, &run, order.id());\n",
            "                && false;\n",
            T_STATUS, "rust"),
    Control("dead_status_word", CLAUDE,
            "                        crate::nirvana_dead::phase_word(order.phase, live)\n",
            "                        order.phase.word()\n",
            T_STATUS, "rust"),
)
