# Fix wake-pane, 2026-11-01 (nirvana/plans/wake-pane-2026-11-01.md). Controls starting with wake_. Run f2u-setup was
# opened from wH:p1; a Nirvana Claude in another space seated two orders into it and every wake after their delivery
# queued on the gone wH:p1. A run now follows the pane that seats into it, and a gone pane is named. The test named must
# print FAIL with each edit in place. `Control` is provided by controls.py.
RUN = "src/nirvana_run.rs"
WORKER = "src/nirvana_worker.rs"
FRESH = "src/nirvana_fresh.rs"
SUBAGENTS = "src/nirvana_subagents.rs"

T_MOVE = "wake_pane_a_seat_run_from_another_pane_moves_the_run_there"
T_LINE = "wake_pane_the_move_is_one_journal_line_an_older_herdr_skips"
T_OLD = "wake_pane_an_old_journal_with_only_opened_replays_unchanged"
T_GONE = "wake_pane_a_gone_pane_is_named_in_the_queued_reason"
T_B = "wake_pane_the_wake_of_an_order_seated_from_b_types_into_b"
T_RETRY = "wake_pane_a_queued_wake_is_retried_toward_the_new_pane"
T_FRESH = "wake_pane_a_run_moved_to_another_pane_is_that_panes_for_the_fresh_start"
T_SUBAGENTS = "wake_pane_seat_orders_follow_the_run_to_its_new_orchestrator"
T_ONLY_NC = "wake_pane_only_a_pane_shown_to_run_nirvana_claude_takes_the_run"
T_REPEAT = "wake_pane_a_repeated_seat_run_from_a_new_pane_still_moves_the_run"
T_STDERR = "wake_pane_an_error_on_stderr_is_read_past_json_on_stdout"

OPEN_OF = "        .find(|(_, state)| state.orchestrator_panes.iter().any(|held| held == pane))\n"
MOVER_CHECK = "    (program(pane) == nirvana_run::PaneProgram::NirvanaClaude).then(|| pane.to_owned())\n"

CONTROLS = (
    # Item 1: `seat run` from another pane journals the move.
    Control("wake_seat_run_writes_the_move", WORKER,
            "    let moved = mover.and_then(|pane| nirvana_run::orchestrator_move(&replay(events), Some(pane)));\n",
            "    let moved = mover.and_then(|_| None::<Kind>);\n",
            T_MOVE, "rust"),
    # Review S1: a pane herdr says runs something else (a shell, plain Claude Code) does not take the run.
    Control("wake_other_pane_no_move", WORKER, MOVER_CHECK,
            "    matches!(program(pane), nirvana_run::PaneProgram::NirvanaClaude | nirvana_run::PaneProgram::Other(_))\n"
            "        .then(|| pane.to_owned())\n",
            T_ONLY_NC, "rust"),
    # Review S1: a pane herdr cannot answer about does not take the run.
    Control("wake_unknown_pane_no_move", WORKER, MOVER_CHECK,
            "    matches!(program(pane), nirvana_run::PaneProgram::NirvanaClaude | nirvana_run::PaneProgram::Unknown)\n"
            "        .then(|| pane.to_owned())\n",
            T_ONLY_NC, "rust"),
    # Review S1: a gone pane does not take the run.
    Control("wake_gone_pane_no_move", WORKER, MOVER_CHECK,
            "    matches!(program(pane), nirvana_run::PaneProgram::NirvanaClaude | nirvana_run::PaneProgram::Gone(_))\n"
            "        .then(|| pane.to_owned())\n",
            T_ONLY_NC, "rust"),
    # Review S1: a Nirvana Claude pane does take it.
    Control("wake_nirvana_pane_moves", WORKER, MOVER_CHECK,
            "    (program(pane) == nirvana_run::PaneProgram::Unknown).then(|| pane.to_owned())\n",
            T_ONLY_NC, "rust"),
    # Review S1: herdr is asked only when the journal says a move is due.
    Control("wake_mover_asks_only_when_due", WORKER,
            "    nirvana_run::orchestrator_move(&replay(events), caller)?;\n",
            "    let _ = events;\n",
            T_ONLY_NC, "rust"),
    # Review N5: a repeated seat run of a known order still writes the move.
    Control("wake_known_order_still_moves", WORKER,
            "        Err(known) => (moved.into_iter().collect(), Some(known)),\n",
            "        Err(known) => (moved.into_iter().take(0).collect(), Some(known)),\n",
            T_REPEAT, "rust"),
    # Review N1: stderr's error response is read when stdout is JSON with no `error`.
    Control("wake_stderr_error_past_stdout_json", WORKER,
            "        .find(|value| value.get(\"error\").is_some());\n",
            "        .next();\n",
            T_STDERR, "rust"),
    # Item 1: replay's current pane is the latest move.
    Control("wake_replay_takes_the_latest_move", RUN,
            "                state.orchestrator_pane = Some(pane.clone());\n",
            "                let _ = pane;\n",
            T_MOVE, "rust"),
    # Item 1: the same pane writes no move.
    Control("wake_same_pane_no_move", RUN,
            "    if state.orchestrator_pane.as_deref() == Some(pane)\n        || state\n",
            "    if false\n        || state\n",
            T_MOVE, "rust"),
    # Item 1: a worker's own pane never becomes the orchestrator's.
    Control("wake_worker_pane_no_move", RUN,
            "            .any(|order| order.pane.as_deref() == Some(pane))\n",
            "            .any(|order| order.pane.as_deref() == Some(\"\") && !pane.is_empty())\n",
            T_MOVE, "rust"),
    # Item 1: a seat run outside herdr (no HERDR_PANE_ID) writes no move.
    Control("wake_no_pane_no_move", RUN,
            "    let pane = pane.filter(|pane| !pane.is_empty())?;\n",
            "    let pane = pane.unwrap_or(\"\");\n",
            T_MOVE, "rust"),
    # Item 1: the move is its own journal kind, with the pane it came from.
    Control("wake_move_line_keeps_from", RUN,
            "        pane: String,\n        #[serde(default)]\n        from: Option<String>,\n",
            "        pane: String,\n        #[serde(default, skip_serializing)]\n        from: Option<String>,\n",
            T_LINE, "rust"),
    # Item 1: old journals replay unchanged: only the first `opened` names the pane.
    Control("wake_old_journal_first_opened", RUN,
            "                if state.opened.is_none() {\n                    state.orchestrator_pane = orchestrator_pane.clone();\n",
            "                state.orchestrator_pane = orchestrator_pane.clone();\n                if state.opened.is_none() {\n",
            T_OLD, "rust"),
    # Item 1: the wake reads the current pane, not `opened`'s.
    Control("wake_reads_the_current_pane", RUN,
            "    let pane = state.orchestrator_pane.clone();\n",
            "    let pane = state\n        .opened\n        .as_ref()\n        .and_then(|opened| opened.orchestrator_pane.clone());\n",
            T_B, "rust"),
    # Item 3: the retry's wake replays the journal, so it follows the move (the same reader, its own test).
    Control("wake_retry_follows_the_move", RUN,
            "    let pane = state.orchestrator_pane.clone();\n",
            "    let pane = state\n        .opened\n        .as_ref()\n        .and_then(|opened| opened.orchestrator_pane.clone());\n",
            T_RETRY, "rust"),
    # Item 1: the fresh start's last-end reader follows the move.
    Control("wake_fresh_last_end_follows", FRESH,
            "        .filter(|(_, state)| state.orchestrator_pane.as_deref() == Some(pane))\n",
            "        .filter(|(_, state)| {\n            state\n                .opened\n                .as_ref()\n"
            "                .and_then(|opened| opened.orchestrator_pane.as_deref())\n                == Some(pane)\n        })\n",
            T_FRESH, "rust"),
    # Item 1: the fresh start's open-run reader follows the move.
    Control("wake_fresh_open_of_follows", FRESH, OPEN_OF,
            "        .find(|(_, state)| state.orchestrator_panes.first().is_some_and(|held| held == pane))\n",
            T_FRESH, "rust"),
    # Review S2: the open-run guard still holds off the pane the run moved away from.
    Control("wake_fresh_open_of_keeps_former_panes", FRESH, OPEN_OF,
            "        .find(|(_, state)| state.orchestrator_pane.as_deref() == Some(pane))\n",
            T_FRESH, "rust"),
    # Review S2: replay keeps every pane the run has had: the moves'.
    Control("wake_replay_keeps_moved_panes", RUN,
            "                state.orchestrator_panes.push(pane.clone());\n",
            "                let _ = &state.orchestrator_panes;\n",
            T_FRESH, "rust"),
    # Review S2: and the first `opened`'s.
    Control("wake_replay_keeps_the_opened_pane", RUN,
            "                    state.orchestrator_panes.extend(orchestrator_pane.clone());\n",
            "                    let _ = &state.orchestrator_panes;\n",
            T_FRESH, "rust"),
    # Item 1: the subagents panel's reader follows the move.
    Control("wake_subagents_follow", SUBAGENTS,
            "        let is_our_orch = run_state.orchestrator_pane.as_deref() == Some(orchestrator_pane_id);\n",
            "        let is_our_orch = run_state\n            .opened\n            .as_ref()\n"
            "            .and_then(|o| o.orchestrator_pane.as_deref())\n            == Some(orchestrator_pane_id);\n",
            T_SUBAGENTS, "rust"),
    # Item 2: herdr's `pane_not_found` is a gone pane.
    Control("wake_gone_from_pane_not_found", RUN,
            "        if code == Some(PANE_NOT_FOUND) {\n",
            "        if code == Some(\"pane_gone\") {\n",
            T_GONE, "rust"),
    # Item 2: the error's code is read from herdr's error response.
    Control("wake_herdr_error_reads_the_code", WORKER,
            "        .and_then(|value| value.pointer(\"/error/code\"))\n",
            "        .and_then(|value| value.pointer(\"/error/kind\"))\n",
            T_GONE, "rust"),
    # Item 2: the wake's `pane get` passes the code on.
    Control("wake_pane_get_passes_the_code", WORKER,
            "            nirvana_run::PaneProgram::of_get_error(pane, code.as_deref()),\n",
            "            nirvana_run::PaneProgram::of_get_error(pane, code.as_deref().and(None)),\n",
            T_GONE, "rust"),
    # Item 2: the queued reason names the gone pane.
    Control("wake_gone_reason_names_the_pane", RUN,
            "    if let PaneProgram::Gone(pane) = &view.program {\n",
            "    if let Some(pane) = None::<&String> {\n",
            T_GONE, "rust"),
    # Item 2: the typing's refusal names it too.
    Control("wake_gone_why_names_the_pane", RUN,
            "            Self::Gone(pane) => format!(\"herdr no longer has pane {pane}\"),\n",
            "            Self::Gone(_) => \"herdr could not say what runs in it\".into(),\n",
            T_GONE, "rust"),
)
