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
T_RESEAT_SITE = _T + "reseat_events_a_rate_limited_reseat_ends_its_parent"
T_AGY_SITE = _T + "resets_in_agy_rate_limit_fills_the_named_reset"
T_PID = _T +"restart_pid_seen_reads_a_running_process_and_a_finished_child"
# Plan 43 fix round (F1 to F7).
T_MAC = _T + "restart_a_free_lock_that_proves_nothing_kills_no_order"
T_TWO = _T + "restart_two_server_starts_keep_their_marks_and_a_doubt_is_not_repeated"
T_ONCE = _T + "restart_a_marked_start_does_not_read_every_run_again"
T_FAIL = _T + "restart_a_run_that_cannot_be_written_leaves_the_start_unmarked"
T_STALE = _T + "restart_a_stale_marker_doubt_is_told_once"
T_CAP = _T + "rate_decision_a_deferral_past_the_cap_is_asked"
T_INDEX = _T + "restart_write_diff_leaves_the_worktrees_index_as_it_was"
T_PANE = _T + "reseat_events_leave_no_pane_finished_early"
T_REAL = _T + "restart_a_real_lock_and_a_real_worker_are_read_by_the_os"
WORKER = "src/nirvana_worker.rs"
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
            "    let mut checked = RunChecked::default();\n    if state.ended.is_some() {\n        return checked;\n",
            "    let mut checked = RunChecked::default();\n    if false {\n        return checked;\n",
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
            "                if dry_run {\n                    checked.lines.push(line);\n                    continue;\n",
            "                if true {\n                    checked.lines.push(line);\n                    continue;\n",
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
            "    if checked.wrote {\n        (world.wake)(project, run);\n",
            "    if false {\n        (world.wake)(project, run);\n",
            T_ORDER, "rust"),
    # R1: an old run is not read.
    Control("p86_lookback", RUN,
            "            if !recent_run(&project, &run, now) {\n",
            "            if false {\n",
            T_SAFE, "rust"),
    # R1: once per server start.
    Control("p86_once_per_start", RUN,
            "    let full = server_started.is_some_and(|at| !restart_marked(home, at));\n",
            "    let full = server_started.is_some();\n",
            T_ONCE, "rust"),
    # R1: the mark is written after the start's pass.
    Control("p86_mark_written", RUN,
            "    std::fs::write(dir.join(at.to_string()), \"\")?;\n",
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
            "            \"{order}: herdr found no supervisor for it but could not prove its worker dead ({}); it stays running: \\\n",
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
            "    roots.retain(|root| seen.insert(fold(root)) || true);\n",
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
    # R2: the reseat's call site writes the parent's end.
    Control("p86_worker_reseat_ends_parent", "src/nirvana_worker.rs",
            "                    nirvana_run::reseat_events(child_seated.clone(), order, &seat, &why),\n",
            "                    nirvana_run::reseat_events(child_seated.clone(), order, &seat, &why)[..2].to_vec(),\n",
            T_RESEAT_SITE, "rust"),
    # R3: agy's call site fills the reset its 429 names.
    Control("p86_worker_agy_reset", "src/nirvana_worker.rs",
            "                resets_at: nirvana_run::resets_in(&detail, now()),\n",
            "                resets_at: None,\n",
            T_AGY_SITE, "rust"),
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
    # F1: where a free lock proves nothing (macOS), a death is a doubt.
    Control("p86_lock_proves_nothing", RUN,
            "        Liveness::Dead(cause) if !world.lock_proves_free => Some(Liveness::Doubt(format!(\n",
            "        Liveness::Dead(cause) if false => Some(Liveness::Doubt(format!(\n",
            T_MAC, "rust"),
    # F1: and no pane marker reads stale there.
    Control("p86_stale_needs_lock_read", RUN,
            "    if !world.lock_proves_free {\n        return Vec::new();\n",
            "    if false {\n        return Vec::new();\n",
            T_MAC, "rust"),
    # F1: on Windows the lock read proves a free lock.
    Control("p86_lock_reads_free_windows", RUN,
            "    #[cfg(windows)]\n    {\n        true\n    }\n",
            "    #[cfg(windows)]\n    {\n        false\n    }\n",
            T_REAL, "rust"),
    # F2: each start has its own mark.
    Control("p86_mark_per_start", RUN,
            "    restart_marks(home).join(at.to_string()).is_file()\n",
            "    restart_marks(home).is_dir()\n",
            T_ONCE, "rust"),
    # F2: a run that failed leaves the start unmarked.
    Control("p86_mark_not_after_failure", RUN,
            "    if let (true, false, false, Some(at)) = (full, dry_run, failed, server_started) {\n",
            "    if let (true, false, _, Some(at)) = (full, dry_run, failed, server_started) {\n",
            T_FAIL, "rust"),
    # F2: an order that could not be interrupted marks the run failed.
    Control("p86_interrupt_failure_counted", RUN,
            "                        checked.failed = true;\n                        checked\n                            .lines\n                            .push(format!(\"{run}: {id} could not be interrupted: {err}\"));\n",
            "                        checked\n                            .lines\n                            .push(format!(\"{run}: {id} could not be interrupted: {err}\"));\n",
            T_FAIL, "rust"),
    # F2: one order's failure does not skip the run's other orders.
    Control("p86_failure_goes_on", RUN,
            "                        checked.failed = true;\n                        checked\n                            .lines\n                            .push(format!(\"{run}: {id} could not be interrupted: {err}\"));\n                        false\n",
            "                        checked.failed = true;\n                        checked\n                            .lines\n                            .push(format!(\"{run}: {id} could not be interrupted: {err}\"));\n                        return checked;\n",
            T_FAIL, "rust"),
    # F2: the same doubt is not written twice.
    Control("p86_doubt_dedupe", RUN,
            "                if order.last_doubt.as_deref() == Some(why.as_str()) {\n",
            "                if false {\n",
            T_TWO, "rust"),
    # F2: the replay keeps the latest doubt's reason.
    Control("p86_doubt_recorded", RUN,
            "                    known.last_doubt = Some(why.clone());\n",
            "",
            T_TWO, "rust"),
    # F2: a new `started` clears it.
    Control("p86_doubt_cleared_by_start", RUN,
            "                    known.last_doubt = None;\n",
            "",
            T_TWO, "rust"),
    # F2 b: a mark that cannot be written is said.
    Control("p86_mark_failure_said", RUN,
            "                \"the restart check's mark could not be written ({err}); the next pass checks again\"\n",
            "                \"\"\n",
            T_TWO, "rust"),
    # F3: the every-minute pass tells a stale marker's doubt once.
    Control("p86_stale_doubt_once", RUN,
            "                if order.last_doubt.as_deref() == Some(why.as_str()) {\n                    continue;\n",
            "                if order.last_doubt.as_deref() == Some(why.as_str()) && false {\n                    continue;\n",
            T_STALE, "rust"),
    # F4: auto mode asks past the cap.
    Control("p86_defer_cap", RUN,
            "        (true, Some(at)) if at.saturating_sub(now) > AUTO_DEFER_CAP_S => RateDecision::Ask {\n",
            "        (true, Some(at)) if false && at.saturating_sub(now) > AUTO_DEFER_CAP_S => RateDecision::Ask {\n",
            T_CAP, "rust"),
    # F4: the 79 h ia1 reset is asked.
    Control("p86_defer_cap_ia1", RUN,
            "        (true, Some(at)) if at.saturating_sub(now) > AUTO_DEFER_CAP_S => RateDecision::Ask {\n",
            "        (true, Some(at)) if at.saturating_sub(now) > 100 * 3600 => RateDecision::Ask {\n",
            T_DECIDE, "rust"),
    # F4: the cap is 6 h.
    Control("p86_defer_cap_value", RUN,
            "pub const AUTO_DEFER_CAP_S: u64 = 6 * 3600;\n",
            "pub const AUTO_DEFER_CAP_S: u64 = 24 * 3600;\n",
            T_CAP, "rust"),
    # F5: a checkpoint no event names is removed.
    Control("p86_stray_checkpoint_removed", RUN,
            "                    let _ = std::fs::remove_file(journal.dir().join(diff));\n",
            "",
            T_BACK, "rust"),
    # F5: `git status` takes no optional lock.
    Control("p86_status_no_locks", WORKER,
            "        git(&[\"--no-optional-locks\", \"status\", \"--porcelain=v1\"]),\n",
            "        git(&[\"status\", \"--porcelain=v1\"]),\n",
            T_INDEX, "rust"),
    # F5: the diff is the plumbing, which writes no index.
    Control("p86_diff_plumbing", WORKER,
            "        git(&[\"--no-optional-locks\", \"diff-index\", \"-p\", \"-M\", \"--no-color\", \"HEAD\"]),\n",
            "        git(&[\"--no-optional-locks\", \"diff\", \"HEAD\", \"--no-color\"]),\n",
            T_INDEX, "rust"),
    # F6 a: the parent's end keeps why it stopped.
    Control("p86_reseat_end_keeps_why", RUN,
            "            why: Some(format!(\"{why}; re-seated as {child_id} on {seat}\")),\n",
            "            why: Some(format!(\"re-seated as {child_id} on {seat}\")),\n",
            T_RESEAT, "rust"),
    # F6 b: a pane whose order's supervisor is live is not finished (the parent launching its child).
    Control("p86_pane_live_parent", WORKER,
            "    let (seq, order) = started;\n    if live(run, order) {\n        return None;\n",
            "    let (seq, order) = started;\n    if false && live(run, order) {\n        return None;\n",
            T_PANE, "rust"),
    # F7: the real lock reads held while its process runs.
    Control("p86_real_lock_held", WORKER,
            "            Ok(_) => false,\n            Err(err) => err.kind() != std::io::ErrorKind::NotFound,\n",
            "            Ok(_) => false,\n            Err(_) => false,\n",
            T_REAL, "rust"),
    # F7: the real worker's pid reads with its creation time, so a live worker is not taken for a reused pid.
    Control("p86_real_pid_since", RUN,
            "        PidSeen::Since(created) => Liveness::Doubt(format!(\n",
            "        PidSeen::Since(created) => Liveness::Dead(format!(\n",
            T_LIVE, "rust"),
    # R3: the duration is added to now.
    Control("p86_resets_from_now", RUN,
            "            return Some(now.saturating_add(total));\n",
            "            return Some(total);\n",
            T_DECIDE, "rust"),
)
