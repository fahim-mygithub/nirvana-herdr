# Finished rows (nirvana/plans/2026-11-09-finished-rows.md): a worker shows once, ended runs and orders leave the
# agents panel and the statusline, and the sweep's dry run works from a plain shell. Controls starting with rows_. One
# sabotage per assertion, each on a product line; the test named must print FAIL with it in place (the expected red
# test is the fifth field, written before the run). `Control` is provided by controls.py.
DEAD = "src/nirvana_dead.rs"
SUBAGENTS = "src/nirvana_subagents.rs"
CLAUDE = "src/nirvana_claude.rs"
SPACES = "src/client/shell/nirvana_spaces.rs"
PANEL = "src/client/shell/agent_sidebar.rs"

T_ONCE = "client::shell::nirvana_spaces::sparkle_tests::a_worker_shows_once_as_its_orders_row"
T_LEAVES = "nirvana_subagents::tests::an_ended_run_leaves_the_panel_after_the_grace"
T_FOOT = "nirvana_claude::tests::the_statusline_lists_only_live_and_recent_orders"
T_GRACE = "nirvana_dead::tests::an_ended_row_stays_the_grace_and_a_live_one_always"
T_SHELL = "nirvana_dead::tests::a_dry_run_from_a_plain_shell_asks_the_nirvana_session"
T_P5 = "nirvana_dead::tests::the_studio_gui_pane_is_kept_by_the_owners_later_key"
# Review round
T_PENDING = "nirvana_subagents::tests::a_pending_order_keeps_its_run_shown"
T_FOOT_PENDING = "nirvana_claude::tests::the_statusline_keeps_a_deferral_until_due_and_an_open_question"
T_REUSE = "client::shell::nirvana_spaces::sparkle_tests::a_kept_worker_pane_the_owner_reuses_shows_again"
T_FAR = "nirvana_subagents::tests::a_worker_with_no_supervisor_stops_counting"

CONTROLS = (
    # A worker shows once (item 2)
    Control("rows_space_agents_skip_workers", SPACES,
            "        .filter(|agent| !workers.contains(&agent.pane_id))\n        .map(|agent| {\n",
            "        .map(|agent| {\n",
            T_ONCE, "rust"),
    Control("rows_label_names_a_worker", SPACES,
            "            .filter(|label| crate::nirvana_dead::parse_label(label).is_some())\n",
            "            .filter(|_| false)\n",
            T_ONCE, "rust"),
    Control("rows_panel_skips_workers", PANEL,
            "        .filter(|pane_id| !workers.contains(pane_id))\n",
            "        .filter(|_| true)\n",
            T_ONCE, "rust"),
    Control("rows_workers_skip_worker_panes", SPACES,
            "        .filter(|agent| !workers.contains(&agent.pane_id))\n        .filter_map(view)\n",
            "        .filter_map(view)\n",
            T_ONCE, "rust"),
    # Ended runs leave the agents panel (item 3)
    Control("rows_run_leaves", SUBAGENTS,
            "            if !run_shown {\n",
            "            if false {\n",
            T_LEAVES, "rust"),
    Control("rows_live_run_stays", DEAD,
            "    if live {\n        return true;\n    }\n    match order.phase {\n",
            "    match order.phase {\n",
            T_LEAVES, "rust"),
    Control("rows_run_any_order_shown", SUBAGENTS,
            "        let run_shown = run_state.orders.iter().zip(&lives).any(|(order, live)| {\n",
            "        let run_shown = run_state.orders.iter().zip(&lives).all(|(order, live)| {\n",
            T_LEAVES, "rust"),
    # Review S2: a pane is a worker's only while its order holds it; the set is replaced each read
    Control("rows_pane_held_while_order_holds", SUBAGENTS,
            "                if listed && crate::nirvana_dead::worker_pane(order, *live, now_secs) {\n",
            "                if listed {\n",
            T_LEAVES, "rust"),
    Control("rows_pane_set_replaced", SUBAGENTS,
            "        all.insert(orchestrator_pane_id.to_owned(), panes);\n",
            "        all.entry(orchestrator_pane_id.to_owned()).or_default().extend(panes);\n",
            T_PENDING, "rust"),
    Control("rows_orchestrator_pane_never_worker", SUBAGENTS,
            "                .filter(|pane| *pane != orchestrator_pane_id)\n",
            "                .filter(|_| true)\n",
            T_PENDING, "rust"),
    Control("rows_asked_does_not_hold_pane", DEAD,
            "    order_shown(order, live, true, now)\n",
            "    order_shown(order, live, false, now)\n",
            T_PENDING, "rust"),
    Control("rows_label_alone_expires", SPACES,
            "        if now.saturating_duration_since(*first) < LABEL_ALONE {\n",
            "        if true {\n",
            T_REUSE, "rust"),
    Control("rows_relabel_restarts", SPACES,
            "                if was != label {\n",
            "                if false {\n",
            T_REUSE, "rust"),
    Control("rows_poller_set_hides", SPACES,
            "    let mut set = crate::nirvana_subagents::seat_worker_panes();\n",
            "    let mut set = std::collections::HashSet::new();\n",
            T_REUSE, "rust"),
    # Review S1, N1, N2: what a not-live order still waits for keeps it shown
    Control("rows_deferred_until_due", DEAD,
            "            let due = order.deferred_until.unwrap_or(0).max(order.last_at);\n",
            "            let due = order.last_at;\n",
            T_PENDING, "rust"),
    Control("rows_open_question_stays", DEAD,
            "        Phase::Asked if !answered => true,\n",
            "        Phase::Asked if !answered => false,\n",
            T_PENDING, "rust"),
    Control("rows_answer_closes_question", DEAD,
            "        .any(|other| other.answers.as_deref() == Some(order.id()))\n",
            "        .any(|other| other.answers.as_deref() == Some(\"never\"))\n",
            T_PENDING, "rust"),
    Control("rows_dead_supervisor_hour", DEAD,
            "        Phase::Seated | Phase::Running => now < order.last_at.saturating_add(STOPPED_GRACE_S),\n",
            "        Phase::Seated | Phase::Running => now < order.last_at.saturating_add(ROWS_GRACE_S),\n",
            T_PENDING, "rust"),
    # Review N4: the far reads restored
    Control("rows_far_read_open_question", DEAD,
            "        Phase::Asked if !answered => true,\n",
            "        Phase::Asked if !answered => now < 2_000,\n",
            T_FAR, "rust"),
    # The foot (review S1, N1)
    Control("rows_foot_deferred_word", CLAUDE,
            "                    let phase = if order.phase == Phase::Deferred {\n",
            "                    let phase = if false {\n",
            T_FOOT_PENDING, "rust"),
    Control("rows_foot_pending_rule", CLAUDE,
            "crate::nirvana_dead::order_shown(order, live, answered, now);",
            "crate::nirvana_dead::still_shown(live, order.last_at, now);",
            T_FOOT_PENDING, "rust"),
    # The statusline's order list (item 3)
    Control("rows_foot_leaves", CLAUDE,
            "                    shown.then(|| OpenOrder {\n",
            "                    true.then(|| OpenOrder {\n",
            T_FOOT, "rust"),
    Control("rows_foot_live_stays", CLAUDE,
            "crate::nirvana_dead::order_shown(order, live, answered, now);",
            "crate::nirvana_dead::order_shown(order, false, answered, now);",
            T_FOOT, "rust"),
    # The grace itself
    Control("rows_grace_rule", DEAD,
            "    live || now < last_at.saturating_add(ROWS_GRACE_S)\n",
            "    live || now < last_at.saturating_add(ROWS_GRACE_S * 100)\n",
            T_GRACE, "rust"),
    # The dry run from a plain shell (item 5)
    Control("rows_dry_run_nirvana_session", DEAD,
            "        named.unwrap_or(crate::nirvana_launch::SESSION).to_owned(),\n",
            "        named.unwrap_or(\"default\").to_owned(),\n",
            T_SHELL, "rust"),
    Control("rows_closing_pass_refused_outside", DEAD,
            "    if !dry_run {\n        return Err(\n",
            "    if false {\n        return Err(\n",
            T_SHELL, "rust"),
    Control("rows_inside_herdr_own_server", DEAD,
            "    if in_herdr {\n        return Ok(None);\n",
            "    if false {\n        return Ok(None);\n",
            T_SHELL, "rust"),
    # wP:p5 (item 4): the owner's later key keeps it, by the record's own numbers
    Control("rows_p5_owner_key", DEAD,
            "Ok(Some(key)) if *key >= ended_at => {",
            "Ok(Some(key)) if *key >= ended_at + 2 * 86_400 => {",
            T_P5, "rust"),
)
