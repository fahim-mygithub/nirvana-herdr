# Plan 43 A6 and A7 (nirvana/plans/2026-11-18-43-megajam-audit.md C9, C10, C11): orders a dead worker left listed as
# running, the reseat's end record, and the reset a quota error's text names. Controls starting with p86_. One sabotage
# per assertion, each on a product line; the test named (the fifth field, written before the run) must print FAIL with
# it in place. `Control` is provided by controls.py.
RUN = "src/nirvana_run.rs"
DEAD = "src/nirvana_dead.rs"

_T = "nirvana_run::tests::restart::"
T_DEAD = _T + "restart_a_free_lock_and_a_gone_pid_read_dead_and_a_held_lock_reads_live"
T_REUSED = _T + "restart_a_reused_pid_reads_dead_and_the_workers_own_process_a_doubt"
T_UNKNOWN = _T + "restart_an_unreadable_pid_and_no_pid_are_doubts"
T_SEATED = _T + "restart_a_seated_order_waits_its_grace_and_other_phases_are_not_judged"
T_ORDER = _T + "restart_a_dead_order_is_checkpointed_interrupted_and_woken_once_per_server_start"
T_LIVE = _T + "restart_a_live_worker_is_never_interrupted_and_a_doubt_is_told_once"
T_BACK = _T + "restart_a_worker_back_under_the_journals_lock_is_not_cut"
T_MARK = _T + "restart_a_pass_without_the_servers_start_reads_only_stale_pane_marks"
T_SAFE = _T + "restart_no_run_an_ended_run_and_an_old_run_are_safe"
T_UNSTARTED = _T + "restart_an_unstarted_seated_order_past_its_grace_is_interrupted"
T_RESEAT = _T + "reseat_events_end_the_parent_and_its_line_is_told_once"
T_RESETS = _T + "resets_in_reads_the_duration_a_quota_error_names"
T_DECIDE = _T + "resets_in_a_short_named_reset_is_the_deferral_and_a_long_one_bounds_none"
T_PID = _T + "restart_pid_seen_reads_a_running_process_and_a_finished_child"
_D = "nirvana_dead::tests::"
T_HOUR = _D + "a_restart_interrupted_order_keeps_its_pane_the_stopped_hour"
T_PROJECTS = _D + "restart_projects_reads_the_record_and_climbs_the_panes_folders"

CONTROLS = (
    # R1: the replay keeps the last attempt's worker pid and its start.
    Control("p86_worker_recorded", RUN,
            "                    known.worker = pid.map(|pid| (pid, event.at));\n",
            "                    known.worker = None;\n",
            T_ORDER, "rust"),
    # R1: only Seated and Running are judged.
    Control("p86_phases_judged", RUN,
            "    if !matches!(order.phase, Phase::Seated | Phase::Running) {\n        return None;\n",
            "    if !matches!(order.phase, Phase::Seated | Phase::Running | Phase::Deferred) {\n        return None;\n",
            T_SEATED, "rust"),
    # R1: a held lock is a supervisor, whatever the pid says.
    Control("p86_lock_reads_live", RUN,
            "    if lock_live {\n        return Some(Liveness::Live(\n",
            "    if false && lock_live {\n        return Some(Liveness::Live(\n",
            T_DEAD, "rust"),
    # R1: a Seated order waits its grace before it reads dead.
    Control("p86_seated_grace", RUN,
            "        return Some(if since < SEATED_GRACE_S {\n",
            "        return Some(if since < 1 {\n",
            T_SEATED, "rust"),
    # R1: past the grace an unstarted Seated order with no supervisor is dead.
    Control("p86_seated_dead", RUN,
            "            Liveness::Dead(format!(\n                \"seated at {} and no supervisor took it\",\n",
            "            Liveness::Live(format!(\n                \"seated at {} and no supervisor took it\",\n",
            T_UNSTARTED, "rust"),
    # R1: no pid recorded is a doubt, not a death.
    Control("p86_no_pid_doubt", RUN,
            "        return Some(Liveness::Doubt(\n            \"no supervisor holds its lock, and its `started` recorded no pid\".into(),\n",
            "        return Some(Liveness::Dead(\n            \"no supervisor holds its lock, and its `started` recorded no pid\".into(),\n",
            T_UNKNOWN, "rust"),
    # R1: a gone pid with a free lock is dead.
    Control("p86_gone_dead", RUN,
            "        PidSeen::Gone => Liveness::Dead(format!(\n",
            "        PidSeen::Gone => Liveness::Doubt(format!(\n",
            T_DEAD, "rust"),
    # R1: a pid created after the worker's start is another process.
    Control("p86_reused_pid", RUN,
            "        PidSeen::Since(created) if created > started + PID_SLACK_S => Liveness::Dead(format!(\n",
            "        PidSeen::Since(created) if created > started + PID_SLACK_S + 100_000 => Liveness::Dead(format!(\n",
            T_REUSED, "rust"),
    # R1: inside the slack the process may be the worker.
    Control("p86_pid_slack", RUN,
            "        PidSeen::Since(created) if created > started + PID_SLACK_S => Liveness::Dead(format!(\n",
            "        PidSeen::Since(created) if created >= started => Liveness::Dead(format!(\n",
            T_REUSED, "rust"),
    # R1: an unreadable pid is a doubt.
    Control("p86_unknown_doubt", RUN,
            "        PidSeen::Unknown => Liveness::Doubt(format!(\n",
            "        PidSeen::Unknown => Liveness::Dead(format!(\n",
            T_UNKNOWN, "rust"),
    # R1: an ended run is never touched.
    Control("p86_ended_run", RUN,
            "    let state = replay(&journal.read().0);\n    if state.ended.is_some() {\n        return Ok((Vec::new(), false));\n",
            "    let state = replay(&journal.read().0);\n    if false {\n        return Ok((Vec::new(), false));\n",
            T_SAFE, "rust"),
    # R1: an exited process whose handle is still held is gone, not running.
    Control("p86_pid_exited", RUN,
            "    if exited {\n        // A process object outlives its process while any handle to it is open.\n",
            "    if false {\n        // A process object outlives its process while any handle to it is open.\n",
            T_PID, "rust"),
    # R1: no such process is gone, not a doubt.
    Control("p86_pid_no_such_process", RUN,
            "ERROR_INVALID_PARAMETER {\n            PidSeen::Gone\n",
            "ERROR_INVALID_PARAMETER {\n            PidSeen::Unknown\n",
            T_PID, "rust"),
    # R1: a running process reads with its creation time.
    Control("p86_pid_created", RUN,
            "    PidSeen::Since(crate::nirvana_subagents::filetime_ticks_to_unix_secs(ticks))\n",
            "    PidSeen::Unknown\n",
            T_PID, "rust"),
    # R1: a doubt is written on the start's pass.
    Control("p86_doubt_written", RUN,
            "                if doubts && !dry_run {\n",
            "                if false {\n",
            T_LIVE, "rust"),
    # R1: the reason opens with WORKER_DIED and names the checkpoint.
    Control("p86_died_reason", RUN,
            "                    \"{WORKER_DIED} ({cause}); its diff is in {}\",\n",
            "                    \"died ({cause}); its diff is in {}\",\n",
            T_ORDER, "rust"),
    # R1: the checkpoint carries the worktree's diff.
    Control("p86_checkpoint_diff", RUN,
            "                                why: \"interrupted\".into(),\n                                report: None,\n                                last_turn: None,\n                                diff: diff.clone(),\n",
            "                                why: \"interrupted\".into(),\n                                report: None,\n                                last_turn: None,\n                                diff: None,\n",
            T_ORDER, "rust"),
    # R1: the order is judged again under the journal's lock.
    Control("p86_rejudged_under_lock", RUN,
            "                    if !matches!(still, Some(Liveness::Dead(_))) {\n",
            "                    if false {\n",
            T_BACK, "rust"),
    # R1: the orchestrator is woken after a write.
    Control("p86_wakes", RUN,
            "    if wrote {\n        (world.wake)(project, run);\n",
            "    if false {\n        (world.wake)(project, run);\n",
            T_ORDER, "rust"),
    # R1: an old run is not read.
    Control("p86_lookback", RUN,
            "            if !recent_run(&project, &run, now) {\n",
            "            if false {\n",
            T_SAFE, "rust"),
    # R1: once per server start.
    Control("p86_once_per_start", RUN,
            "    let full = server_started.is_some_and(|at| restart_mark(home) != Some(at));\n",
            "    let full = server_started.is_some();\n",
            T_LIVE, "rust"),
    # R1: the mark is written after the start's pass.
    Control("p86_mark_written", RUN,
            "        let _ = std::fs::write(dir.join(RESTART_MARK), at.to_string());\n",
            "",
            T_ORDER, "rust"),
    # R1: a pass with no server start reads only orders with a stale pane marker.
    Control("p86_stale_only", RUN,
            "            let only = (!full).then_some(stale.as_slice());\n",
            "            let only: Option<&[String]> = None;\n",
            T_MARK, "rust"),
    # R1: a stale marker whose order no longer runs is removed.
    Control("p86_marker_removed", RUN,
            "                            .join(format!(\"{order}.{PANE_MARK_EXT}\")),\n",
            "                            .join(format!(\"{order}.kept\")),\n",
            T_MARK, "rust"),
    # R1: the orchestrator's line names the checkpoint and the way on.
    Control("p86_restart_line", RUN,
            "        Kind::Interrupted { order, why } if why.starts_with(WORKER_DIED) => format!(\n",
            "        Kind::Interrupted { order, why } if false && why.starts_with(WORKER_DIED) => format!(\n",
            T_ORDER, "rust"),
    # R1: a doubt reaches the orchestrator's queue.
    Control("p86_doubt_line", RUN,
            "            \"{order}: herdr could not prove its worker dead after a server restart ({}); it stays running: \\\n",
            "            \"{order}: herdr thinks it is fine ({}); it stays running: \\\n",
            T_LIVE, "rust"),
    # R1: a restart-interrupted order keeps its pane the stopped hour.
    Control("p86_pane_hour", DEAD,
            "                if died_unmarked(known) {\n",
            "                if false {\n",
            T_HOUR, "rust"),
    # R1: the projects a pane's folder climbs to are read.
    Control("p86_projects_climb", DEAD,
            "            if at.join(nirvana_run::DIR).join(\"runs\").is_dir() {\n                roots.push(at);\n                break;\n",
            "            if at.join(nirvana_run::DIR).join(\"runs\").is_dir() {\n                break;\n",
            T_PROJECTS, "rust"),
    # R1: each project once.
    Control("p86_projects_once", DEAD,
            "    roots.retain(|root| seen.insert(fold(root)));\n",
            "",
            T_PROJECTS, "rust"),
    # R2: a reseat ends its parent with outcome reseated.
    Control("p86_reseat_ended", RUN,
            "            outcome: RESEATED.into(),\n",
            "            outcome: \"failed\".into(),\n",
            T_RESEAT, "rust"),
    # R2: the reseat is told once, by its `reseated` line.
    Control("p86_reseat_told_once", RUN,
            "        } if outcome != \"delivered\" && outcome != RESEATED => match why {\n",
            "        } if outcome != \"delivered\" => match why {\n",
            T_RESEAT, "rust"),
    # R3: the mark the text is found by.
    Control("p86_resets_mark", RUN,
            "    const MARK: &str = \"resets in \";\n",
            "    const MARK: &str = \"reset in \";\n",
            T_RESETS, "rust"),
    # R3: hours are 3,600 s.
    Control("p86_resets_hours", RUN,
            "                'h' => (1, 3_600),\n",
            "                'h' => (1, 60),\n",
            T_RESETS, "rust"),
    # R3: units in order, each once.
    Control("p86_resets_unit_order", RUN,
            "            if digits.is_empty() || last_rank.is_some_and(|last| rank <= last) {\n",
            "            if digits.is_empty() {\n",
            T_RESETS, "rust"),
    # R3: a number with no unit after it is no duration.
    Control("p86_resets_trailing_digits", RUN,
            "        if units > 0 && digits.is_empty() {\n",
            "        if units > 0 {\n",
            T_RESETS, "rust"),
    # R3: the duration is added to now.
    Control("p86_resets_from_now", RUN,
            "            return Some(now.saturating_add(total));\n",
            "            return Some(total);\n",
            T_DECIDE, "rust"),
)
