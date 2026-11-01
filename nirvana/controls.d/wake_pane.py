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

CONTROLS = (
    # Item 1: `seat run` from another pane journals the move.
    Control("wake_seat_run_writes_the_move", WORKER,
            "            orchestrator_pane.as_deref(),\n        ));\n    }\n    kinds.push(Kind::Seated(",
            "            None.or(orchestrator_pane.as_deref().and(None)),\n        ));\n    }\n    kinds.push(Kind::Seated(",
            T_MOVE, "rust"),
    # Item 1: replay's current pane is the latest move.
    Control("wake_replay_takes_the_latest_move", RUN,
            "            Kind::OrchestratorMoved { pane, .. } => state.orchestrator_pane = Some(pane.clone()),\n",
            "            Kind::OrchestratorMoved { .. } => {}\n",
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
    Control("wake_fresh_open_of_follows", FRESH,
            "        .find(|(_, state)| state.orchestrator_pane.as_deref() == Some(pane))\n",
            "        .find(|(_, state)| {\n            state\n                .opened\n                .as_ref()\n"
            "                .and_then(|opened| opened.orchestrator_pane.as_deref())\n                == Some(pane)\n        })\n",
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
