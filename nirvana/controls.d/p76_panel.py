# The agents panel's tall row (fix/panel-tall-row, 2026-11-14). The owner: "The agent lists on the left is not
# populating". Controls starting with p76_. One sabotage per assertion, each on a product line in
# src/client/shell/agent_sidebar.rs; the test named must print FAIL with it in place (the expected red test is the
# fifth field, written before the run). `Control` is provided by controls.py.
SIDEBAR = "src/client/shell/agent_sidebar.rs"

_T = "client::shell::agent_sidebar::tests::"
T_SHAPE = _T + "the_orchestrator_row_and_its_live_order_show_beside_a_short_row"
T_LINES = _T + "a_runs_ended_orders_are_cut_in_the_rows_height"
T_CLIP = _T + "a_row_that_does_not_fit_whole_is_drawn_cut_with_its_live_order_first"

CONTROLS = (
    # A run's ended orders are cut to the 3 most recent: without the cut the row takes 47 lines and is not drawn.
    Control("p76_run_finished_cut", SIDEBAR,
            "            group.finished.truncate(RUN_FINISHED_CUT_TO);\n",
            "",
            T_SHAPE, "rust"),
    # The run's "+N more" line is drawn.
    Control("p76_run_more_line", SIDEBAR,
            "        if group.finished_cut > 0 && *current_y < rect.bottom() {\n",
            "        if false && *current_y < rect.bottom() {\n",
            T_SHAPE, "rust"),
    # The row's height counts the run's "+N more" line.
    Control("p76_row_lines_more", SIDEBAR,
            "    1 + group.live.len() * 2 + group.finished.len() + usize::from(group.finished_cut > 0)\n",
            "    1 + group.live.len() * 2 + group.finished.len()\n",
            T_LINES, "rust"),
    # A row that does not fit whole is drawn cut, not dropped.
    Control("p76_clip_row", SIDEBAR,
            "            height = room;\n",
            "            break;\n",
            T_CLIP, "rust"),
    # Past the panel's cap, the one "+N more" counts a run's own cut orders.
    Control("p76_cap_counts_run_cut", SIDEBAR,
            "        .map(|g| g.live.len() + g.finished.len() + g.finished_cut)\n",
            "        .map(|g| g.live.len() + g.finished.len())\n",
            T_CLIP, "rust"),
)
