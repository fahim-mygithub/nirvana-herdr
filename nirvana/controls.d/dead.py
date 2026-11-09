# Dead panes (nirvana/plans/2026-11-08-dead-panes.md): finished workers stop counting and their panes close.
# Controls starting with dead_. One sabotage per assertion, each on a product line; the test named must print FAIL with
# it in place (the expected red test is the fifth field, written before the run). `Control` is provided by controls.py.
DEAD = "src/nirvana_dead.rs"
SUBAGENTS = "src/nirvana_subagents.rs"
CLAUDE = "src/nirvana_claude.rs"

_D = "nirvana_dead::tests::"
T_GRACE = _D + "a_finished_workers_bare_shell_pane_closes_after_the_grace"
T_RUNS = _D + "a_pane_whose_worker_still_runs_is_never_closed"
T_PROGRAM = _D + "a_pane_running_a_program_is_never_closed"
T_TYPED = _D + "a_pane_the_owner_typed_in_after_its_worker_ended_is_kept"
T_AGENT_TAB = _D + "the_spaces_agent_tab_and_an_agent_pane_are_never_closed"
T_LAST = _D + "a_run_tabs_panes_all_close_but_a_spaces_last_pane_stays"
T_JOBS = _D + "an_ended_jobs_pane_closes_and_a_running_ones_stays"
T_NO_RECORD = _D + "a_labelled_pane_no_record_names_is_kept"
T_RELABEL = _D + "a_pane_relabelled_since_the_list_is_kept"
T_DRY = _D + "a_dry_run_names_each_verdict_and_closes_nothing"
T_LIVE = _D + "order_live_reads_the_supervisor_not_the_phase"
T_LABELS = _D + "labels_name_a_worker_or_a_job"
T_ROWS_DEAD = "nirvana_subagents::tests::a_worker_with_no_supervisor_stops_counting"
T_ROWS_PHASES = "nirvana_subagents::tests::read_seat_orders_phases_and_ended_runs"
T_STATUS = "nirvana_claude::tests::the_statusline_names_an_order_with_no_supervisor_stopped"

CONTROLS = (
    # The close rule (`decide`) and the sweep
    Control("dead_grace", DEAD,
            "    if now < ended_at.saturating_add(CLOSE_GRACE_S) {\n",
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
    Control("dead_agent_tab", DEAD,
            "            tab_has_agent: agent_tabs.contains(&tab),\n",
            "            tab_has_agent: false,\n",
            T_AGENT_TAB, "rust"),
    Control("dead_space_last_pane", DEAD,
            "                && tabs_in_space.get(&space).copied().unwrap_or(0) <= 1,\n",
            "                && false,\n",
            T_LAST, "rust"),
    Control("dead_counts_the_closed", DEAD,
            "                *left = left.saturating_sub(1);\n",
            "                *left = left.saturating_add(0);\n",
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
            "        if dry_run {\n",
            "        if false {\n",
            T_DRY, "rust"),
    Control("dead_label_form", DEAD,
            "(nirvana_run::valid_id(head) && nirvana_run::valid_id(tail))",
            "(!head.is_empty() && !tail.is_empty())",
            T_LABELS, "rust"),
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
            "                phase: crate::nirvana_dead::phase_word(order.phase, live).to_owned(),\n",
            "                phase: order.phase.word().to_owned(),\n",
            T_STATUS, "rust"),
)
