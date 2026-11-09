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

CONTROLS = (
    # A worker shows once (item 2)
    Control("rows_space_agents_skip_workers", SPACES,
            "        .filter(|agent| !is_worker_pane(snapshot, &agent.pane_id))\n        .map(|agent| {\n",
            "        .map(|agent| {\n",
            T_ONCE, "rust"),
    Control("rows_label_names_a_worker", SPACES,
            "                .and_then(crate::nirvana_dead::parse_label)\n",
            "                .filter(|_| false)\n                .and_then(crate::nirvana_dead::parse_label)\n",
            T_ONCE, "rust"),
    Control("rows_panel_skips_workers", PANEL,
            "        .filter(|pane_id| !super::nirvana_spaces::is_worker_pane(snapshot, pane_id))\n",
            "        .filter(|pane_id| !crate::nirvana_subagents::is_seat_worker_pane(pane_id))\n",
            T_ONCE, "rust"),
    Control("rows_workers_skip_worker_panes", SPACES,
            "        .filter(|agent| !is_worker_pane(snapshot, &agent.pane_id))\n        .filter_map(view)\n",
            "        .filter_map(view)\n",
            T_ONCE, "rust"),
    # Ended runs leave the agents panel (item 3)
    Control("rows_run_leaves", SUBAGENTS,
            "            if !run_shown {\n",
            "            if false {\n",
            T_LEAVES, "rust"),
    Control("rows_live_run_stays", SUBAGENTS,
            "crate::nirvana_dead::still_shown(lives.contains(&true), newest_order_at, now_secs);",
            "crate::nirvana_dead::still_shown(false, newest_order_at, now_secs);",
            T_LEAVES, "rust"),
    Control("rows_run_newest_event", SUBAGENTS,
            "            .map(|order| order.last_at)\n            .max()\n",
            "            .map(|order| order.last_at)\n            .min()\n",
            T_LEAVES, "rust"),
    Control("rows_hidden_pane_recorded", SUBAGENTS,
            "            if let Some(pane) = &order.pane {\n                record_worker_pane(pane.clone());\n",
            "            if let Some(pane) = order.pane.as_ref().filter(|_| run_shown) {\n"
            "                record_worker_pane(pane.clone());\n",
            T_LEAVES, "rust"),
    # The statusline's order list (item 3)
    Control("rows_foot_leaves", CLAUDE,
            "crate::nirvana_dead::still_shown(live, order.last_at, now).then(|| OpenOrder {",
            "true.then(|| OpenOrder {",
            T_FOOT, "rust"),
    Control("rows_foot_live_stays", CLAUDE,
            "crate::nirvana_dead::still_shown(live, order.last_at, now).then(|| OpenOrder {",
            "crate::nirvana_dead::still_shown(false, order.last_at, now).then(|| OpenOrder {",
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
