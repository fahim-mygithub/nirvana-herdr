# Plan 30: live workers show in the spaces view (package `live_in_spaces`)
# Controls starting with p30_.

CONTROLS = (
    # §2: live means the agents panel's live group; finished, blocked, stopped and failed rows never show
    Control(
        'p30_live_group_only',
        'src/client/shell/nirvana_spaces.rs',
        '        .flat_map(|group| group.live.iter())\n',
        '        .flat_map(|group| group.live.iter().chain(group.finished.iter()))\n',
        'a_space_shows_exactly_its_one_live_worker_in_the_panels_format',
        'rust',
    ),
    # §2: with no live worker, the space looks exactly as it does today
    Control(
        'p30_no_live_renders_as_before',
        'src/client/shell/nirvana_spaces.rs',
        '        .flat_map(|group| group.live.iter())\n',
        '        .flat_map(|group| group.finished.iter())\n',
        'a_space_with_no_live_worker_renders_as_before',
        'rust',
    ),
    # §2: the rows keep the agents panel's order (its live-first grouping, longest running first)
    Control(
        'p30_panel_order',
        'src/client/shell/nirvana_spaces.rs',
        '        .flat_map(|group| group.live.iter())\n',
        '        .flat_map(|group| group.live.iter().rev())\n',
        'five_live_workers_show_three_rows_and_two_more',
        'rust',
    ),
    # §2 tests: workers of another space never show under this one
    Control(
        'p30_own_space_only',
        'src/client/shell/nirvana_spaces.rs',
        '        .filter(|agent| agent.workspace_id == workspace_id)\n        .filter(|agent| !crate::nirvana_subagents::is_seat_worker_pane(&agent.pane_id))\n',
        '        .filter(|_| true)\n        .filter(|agent| !crate::nirvana_subagents::is_seat_worker_pane(&agent.pane_id))\n',
        'workers_of_another_space_never_show_under_this_one',
        'rust',
    ),
    # §2: the same words as the panel's live row: the short model
    Control(
        'p30_panel_short_model',
        'src/client/shell/agent_sidebar.rs',
        '        crate::nirvana_spaces::compact_model(&sub.model),\n        crate::nirvana_subagents::format_elapsed',
        '        sub.model.clone(),\n        crate::nirvana_subagents::format_elapsed',
        'a_space_shows_exactly_its_one_live_worker_in_the_panels_format',
        'rust',
    ),
    # §2: the same words as the panel's live row: the elapsed time
    Control(
        'p30_panel_elapsed',
        'src/client/shell/agent_sidebar.rs',
        '        crate::nirvana_subagents::format_elapsed(sub.elapsed_secs),\n    )\n',
        '        sub.elapsed_secs.to_string(),\n    )\n',
        'a_space_shows_exactly_its_one_live_worker_in_the_panels_format',
        'rust',
    ),
    # §2: names are cut the way the agents panel cuts them, keeping the model and the time
    Control(
        'p30_name_cut_keeps_tail',
        'src/client/shell/nirvana_spaces.rs',
        '    let budget = width.saturating_sub(unicode_width::UnicodeWidthStr::width(tail.as_str()));\n',
        '    let budget = width;\n',
        'a_long_worker_name_is_cut_and_keeps_model_and_time',
        'rust',
    ),
    # §2: past 4 live workers, show 3
    Control(
        'p30_cut_past_four',
        'src/client/shell/nirvana_spaces.rs',
        '    if count > WORKERS_SHOWN {\n        WORKERS_CUT_TO\n',
        '    if false {\n        WORKERS_CUT_TO\n',
        'five_live_workers_show_three_rows_and_two_more',
        'rust',
    ),
    # §2: four live workers still show whole
    Control(
        'p30_four_show_whole',
        'src/client/shell/nirvana_spaces.rs',
        'pub(super) const WORKERS_SHOWN: usize = 4;',
        'pub(super) const WORKERS_SHOWN: usize = 3;',
        'five_live_workers_show_three_rows_and_two_more',
        'rust',
    ),
    # §2: the "+N more" row takes its own line
    Control(
        'p30_more_row_height',
        'src/client/shell/nirvana_spaces.rs',
        '    workers_shown(count) + usize::from(count > WORKERS_SHOWN)\n',
        '    workers_shown(count)\n',
        'five_live_workers_show_three_rows_and_two_more',
        'rust',
    ),
    # §2: "+N more" counts the workers not shown
    Control(
        'p30_more_count',
        'src/client/shell/nirvana_spaces.rs',
        '            let more = format!("+{} more", workers.len() - shown);\n',
        '            let more = format!("+{} more", workers.len());\n',
        'five_live_workers_show_three_rows_and_two_more',
        'rust',
    ),
    # §2: a dim marker; the last row closes the tree
    Control(
        'p30_marker_closes_tree',
        'src/client/shell/nirvana_spaces.rs',
        '        let marker = if index + 1 == rows { "└" } else { "├" };\n',
        '        let marker = "├";\n',
        'a_space_shows_exactly_its_one_live_worker_in_the_panels_format',
        'rust',
    ),
    # §2: the marker is dim
    Control(
        'p30_marker_dim',
        'src/client/shell/nirvana_spaces.rs',
        '        put_text(buffer, x, y, 1, marker, dim);\n',
        '        put_text(buffer, x, y, 1, marker, name_style);\n',
        'a_worktree_space_keeps_its_tree_line_beside_worker_rows',
        'rust',
    ),
    # §2: the name keeps its own colour over the line
    Control(
        'p30_name_colour',
        'src/client/shell/nirvana_spaces.rs',
        '        put_text(buffer, x + 2, y, row.name_end as u16, &row.line, name_style);\n',
        '        put_text(buffer, x + 2, y, row.name_end as u16, &row.line, model_style);\n',
        'a_worktree_space_keeps_its_tree_line_beside_worker_rows',
        'rust',
    ),
    # §2: a worktree space keeps its parent's tree line beside the worker rows
    Control(
        'p30_worktree_tree_line',
        'src/client/shell/nirvana_spaces.rs',
        '        if indented && !last_child {\n',
        '        if false && indented && !last_child {\n',
        'a_worktree_space_keeps_its_tree_line_beside_worker_rows',
        'rust',
    ),
    # §2: the sidebar draws the worker rows
    Control(
        'p30_sidebar_draws_workers',
        'src/client/shell/sidebar.rs',
        '            workers,\n            entry.indented,\n',
        '            &[],\n            entry.indented,\n',
        'a_space_shows_exactly_its_one_live_worker_in_the_panels_format',
        'rust',
    ),
    # §2: the space is as tall as its worker rows
    Control(
        'p30_space_rect_has_worker_rows',
        'src/client/shell/sidebar.rs',
        '            .saturating_add(super::nirvana_spaces::workers_height(workers.len()) as u16)\n',
        '            .saturating_add(super::nirvana_spaces::workers_height(0) as u16)\n',
        'a_space_shows_exactly_its_one_live_worker_in_the_panels_format',
        'rust',
    ),
    # §2: the list's scroll counts the worker rows
    Control(
        'p30_scroll_counts_worker_rows',
        'src/client/shell/sidebar.rs',
        '                        + super::nirvana_spaces::workers_height(workers.len())\n',
        '                        + super::nirvana_spaces::workers_height(0)\n',
        'live_workers_make_the_spaces_list_scroll',
        'rust',
    ),
    # Review F1: the spaces view asks for its own spaces' panes, so their rows outlive the poller's 60 s drop
    Control(
        'p30_spaces_view_asks',
        # The asking is `cached` refreshing `asked_at`; in this test only the spaces view calls it (the panel's filtered
        # view lists nothing), and `p30_one_reading_per_frame` proves the spaces view's call. A sabotage naming a
        # cfg(test) reader broke the plain binary's build (review round, exit 101).
        'src/nirvana_subagents.rs',
        '    entry.asked_at = Some(Instant::now());\n    entry.view.clone()\n',
        '    entry.view.clone()\n',
        'worker_rows_outlive_the_drop_window_when_the_panel_does_not_draw_the_pane',
        'rust',
    ),
    # Review F1: the poller still forgets a pane nobody asks for
    Control(
        'p30_drop_still_applies',
        'src/nirvana_subagents.rs',
        '            .is_some_and(|at| at.elapsed() < Duration::from_secs(60))\n    });\n}\n',
        '            .is_some_and(|at| at.elapsed() < Duration::from_secs(600))\n    });\n}\n',
        'worker_rows_outlive_the_drop_window_when_the_panel_does_not_draw_the_pane',
        'rust',
    ),
    # Review F2: the render pass draws the heights pass's reading, not a second one
    Control(
        'p30_one_reading_per_frame',
        'src/client/shell/sidebar.rs',
        '        let workers = &workers_by_entry[entry_position];\n',
        '        let workers = &super::nirvana_spaces::space_workers(snapshot, &workspace.workspace_id, |agent| super::agent_sidebar::ask_subagents(snapshot, agent));\n',
        'one_frame_reads_a_spaces_workers_once',
        'rust',
    ),
    # Review F3: the next space starts below the worker rows
    Control(
        'p30_next_space_below_workers',
        'src/client/shell/sidebar.rs',
        '        y = y.saturating_add(row_height + gap);\n',
        '        y = y.saturating_add(row_height.saturating_sub(super::nirvana_spaces::workers_height(workers.len()) as u16) + gap);\n',
        'a_space_after_live_workers_starts_below_them_and_takes_its_own_click',
        'rust',
    ),
    # Review nit: a cut tail's colour ends come from what was drawn
    Control(
        'p30_cut_tail_colours',
        'src/client/shell/nirvana_spaces.rs',
        '        model_end: (cells(&name) + 1 + cells(&worker.model)).min(drawn),\n',
        '        model_end: drawn.saturating_sub(1 + cells(&worker.elapsed)),\n',
        'a_cut_tail_keeps_each_part_in_its_own_colour',
        'rust',
    ),
    # Review nit: the workers' "+N more" is in the agents' "+N more" colour
    Control(
        'p30_more_colour',
        'src/client/shell/nirvana_spaces.rs',
        '    let more_style = model_style;\n',
        '    let more_style = elapsed_style;\n',
        'the_workers_more_row_is_in_the_agents_more_colour',
        'rust',
    ),
    # Review nit: a seat worker's pane brings no worker rows, as agent_rows leaves it out
    Control(
        'p30_seat_worker_filter',
        'src/client/shell/nirvana_spaces.rs',
        '        .filter(|agent| !crate::nirvana_subagents::is_seat_worker_pane(&agent.pane_id))\n',
        '        .filter(|_| true)\n',
        'a_seat_worker_pane_brings_no_worker_rows',
        'rust',
    ),
)
