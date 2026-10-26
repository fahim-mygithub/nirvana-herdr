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
        '        .filter(|agent| agent.workspace_id == workspace_id)\n        .filter_map(|agent| view(&agent.pane_id))\n',
        '        .filter(|_| true)\n        .filter_map(|agent| view(&agent.pane_id))\n',
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
        '        put_text(buffer, x + 2, y, model_from as u16, &line, name_style);\n',
        '        put_text(buffer, x + 2, y, model_from as u16, &line, model_style);\n',
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
        '            &workers,\n            entry.indented,\n',
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
    # §2: no new reads, no new polling: peek never makes a cache entry that keeps a pane polled
    Control(
        'p30_peek_asks_for_nothing',
        'src/nirvana_subagents.rs',
        '    let guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());\n    guard.as_ref()?.get(pane_id)?.view.clone()\n',
        '    cached(pane_id)\n',
        'peek_reads_the_cache_without_asking_for_a_pane',
        'rust',
    ),
)
