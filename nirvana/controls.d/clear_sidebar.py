# Plan 26: clear sidebar and finished work leaves the screen (package `clear_sidebar`)
# Controls starting with p26_.

CONTROLS = (
    # Plan 26 §2: worker delivering done closes its pane through herdr
    Control(
        'p26_worker_close_done',
        'src/nirvana_worker.rs',
        '    status == "done"\n',
        '    false && status == "done"\n',
        'worker_pane_closing_decisions',
        'rust',
    ),
    # Plan 26 §2: tab last pane stays when done
    Control(
        'p26_worker_keep_tab_last_pane',
        'src/nirvana_worker.rs',
        '        && tab_panes_count > 1\n',
        '        && tab_panes_count > 0\n',
        'worker_pane_closing_decisions',
        'rust',
    ),
    # Plan 26 §2: undelivered or running workers keep pane
    Control(
        'p26_worker_keep_undelivered',
        'src/nirvana_worker.rs',
        '        && phase == crate::nirvana_run::Phase::Delivered\n',
        '        && true\n',
        'worker_pane_closing_decisions',
        'rust',
    ),
    # Plan 26 B1: detached worker records no pane, pane arg records pane
    Control(
        'p26_worker_pane_argument_recording',
        'src/nirvana_worker.rs',
        '            pane: pane.map(str::to_string),\n',
        '            pane: None,\n',
        'detached_worker_records_no_pane_and_pane_arg_records_pane',
        'rust',
    ),
    # Plan 26 B1: pane title must equal expected seat·order before closing
    Control(
        'p26_worker_close_title_check',
        'src/nirvana_worker.rs',
        '        && actual_title == Some(expected_title)\n',
        '        && true\n',
        'worker_pane_closing_decisions',
        'rust',
    ),
    # Plan 26 §3: run end closes run tab when all panes are eligible finished worker panes
    Control(
        'p26_close_run_tab_all_workers',
        'src/nirvana_run.rs',
        '    let all_eligible_finished = !has_live\n        && tab_panes\n            .iter()\n            .all(|pane| eligible_finished_panes.contains(pane));\n',
        '    let all_eligible_finished = false;\n',
        'close_run_tab_decisions',
        'rust',
    ),
    # Plan 26 §3: run end closes only eligible finished worker panes when tab has mixed panes
    Control(
        'p26_close_run_tab_mixed_panes',
        'src/nirvana_run.rs',
        '        let to_close: Vec<String> = tab_panes\n            .iter()\n            .filter(|pane| eligible_finished_panes.contains(pane) && !live_panes.contains(pane))\n            .cloned()\n            .collect();\n',
        '        let to_close: Vec<String> = Vec::new();\n',
        'close_run_tab_decisions',
        'rust',
    ),
    # Plan 26 §3: missing known pane returns NotFound decision
    Control(
        'p26_close_run_tab_missing_known_pane',
        'src/nirvana_run.rs',
        '    if known_pane.is_none() {\n        return CloseRunTabDecision::NotFound("no worker pane found".into());\n    }\n',
        '    if false && known_pane.is_none() {\n        return CloseRunTabDecision::NotFound("no worker pane found".into());\n    }\n',
        'close_run_tab_decisions',
        'rust',
    ),
    # Plan 26 B2: blocked subagents are finished rows with red mark
    Control(
        'p26_subagent_blocked_arm',
        'src/nirvana_subagents.rs',
        '                if report_status == "blocked" {\n                    SubagentState::Blocked\n',
        '                if false && report_status == "blocked" {\n                    SubagentState::Blocked\n',
        'read_seat_orders_phases_and_ended_runs',
        'rust',
    ),
    # Plan 26 B2: reseated subagents skipped
    Control(
        'p26_subagent_reseated_skip',
        'src/nirvana_subagents.rs',
        '                    | crate::nirvana_run::Phase::Reseated\n',
        '                    // | crate::nirvana_run::Phase::Reseated\n',
        'read_seat_orders_phases_and_ended_runs',
        'rust',
    ),
    # Plan 26 B3: shared is_nirvana_claude function checks agent and plugin-dir
    Control(
        'p26_is_nirvana_claude_shared',
        'src/nirvana_run.rs',
        '    matches!(agent, Some("nirvana" | "nirvana:nirvana"))\n',
        '    false && matches!(agent, Some("nirvana" | "nirvana:nirvana"))\n',
        'is_nirvana_claude_argv_tests',
        'rust',
    ),
    # Plan 26 S1: short model Claude family initial and version
    Control(
        'p26_short_model_claude',
        'src/nirvana_spaces.rs',
        '        return match ver {\n            Some(v) => format!("{family_initial}{v}"),\n            None => family_initial.to_string(),\n        };\n',
        '        return match ver {\n            Some(v) => format!("X{v}"),\n            None => family_initial.to_string(),\n        };\n',
        'compact_model_table_test',
        'rust',
    ),
    # Plan 26 S1: ladder step 3 shortens status words
    Control(
        'p26_space_row_ladder_step3',
        'src/client/shell/nirvana_spaces.rs',
        '        crate::api::schema::AgentStatus::Working => "work",\n',
        '        crate::api::schema::AgentStatus::Working => "working",\n',
        'space_row_whole_row_equality_at_26_cells',
        'rust',
    ),
    # Plan 26 S1: ladder step 1 pads who to max_who
    Control(
        'p26_space_row_ladder_padding',
        'src/client/shell/nirvana_spaces.rs',
        '                let who_padded = format!("{:<max_who$}", agent.who);\n',
        '                let who_padded = agent.who.clone();\n',
        'space_row_whole_row_equality_at_26_cells',
        'rust',
    ),
    # Plan 26 S2: MouseEventKind::Moved sets hovered space agent pane
    Control(
        'p26_mouse_moved_sets_hover',
        'src/client/shell/mouse.rs',
        '                if hovered != self.hovered_space_agent_pane {\n                    self.hovered_space_agent_pane = hovered;\n',
        '                if false && hovered != self.hovered_space_agent_pane {\n                    self.hovered_space_agent_pane = hovered;\n',
        'mouse_moved_sets_hover_and_moved_away_clears_it',
        'rust',
    ),
    # Plan 26 S2: hovered row draws chat title
    Control(
        'p26_hovered_row_draws_chat_title',
        'src/client/shell/nirvana_spaces.rs',
        '            } else if hovered_pane == Some(agent.pane_id.as_str())\n                && !agent.task.is_empty()\n                && agent.task != "No task yet"\n            {\n',
        '            } else if false && hovered_pane == Some(agent.pane_id.as_str()) {\n',
        'a_long_title_is_cut_to_the_sidebar_with_an_ellipsis',
        'rust',
    ),
    # Plan 26 S2 & Review: No task yet string is never shown
    Control(
        'p26_no_task_yet_never_shown',
        'src/client/shell/nirvana_spaces.rs',
        '                let word = status_word(agent.status);\n',
        '                let word = "No task yet";\n',
        'space_row_whole_row_equality_at_26_cells',
        'rust',
    ),
    # Plan 26 S3: tab_panes_count counts only panes from this run
    Control(
        'p26_tab_panes_count_run_only',
        'src/nirvana_worker.rs',
        '        .filter(|id| run_panes.iter().any(|rp| rp == **id))\n',
        '        // .filter(|id| run_panes.iter().any(|rp| rp == **id))\n',
        'worker_pane_closing_decisions',
        'rust',
    ),
    # Plan 26 S4: close_run_tab never closes a live worker pane
    Control(
        'p26_run_end_never_closes_live_worker',
        'src/nirvana_run.rs',
        '    let has_live = tab_panes.iter().any(|pane| live_panes.contains(pane));\n',
        '    let has_live = false;\n',
        'close_run_tab_decisions',
        'rust',
    ),
    # Plan 26 S6: runs sorted by newest event time first
    Control(
        'p26_runs_sorted_by_recency',
        'src/client/shell/agent_sidebar.rs',
        '    result.sort_by_key(|b| std::cmp::Reverse(run_event_time(b)));\n',
        '    result.sort_by_key(|b| run_event_time(b));\n',
        'group_subagents_by_run_ordering',
        'rust',
    ),
    # Plan 26 S6: ended run hides finished rows
    Control(
        'p26_ended_run_hides_finished_rows',
        'src/nirvana_subagents.rs',
        '            if run_ended && !is_live {\n                continue;\n            }\n',
        '            if false && run_ended && !is_live {\n                continue;\n            }\n',
        'read_seat_orders_phases_and_ended_runs',
        'rust',
    ),
    # Plan 26 S6: cap counts cut rows
    Control(
        'p26_subagents_cap_counts_cut_rows',
        'src/client/shell/agent_sidebar.rs',
        '    let cut_count = total_workers.saturating_sub(kept_workers);\n',
        '    let cut_count = 0;\n',
        'subagents_capping_keeps_live_and_counts_cut',
        'rust',
    ),
    # Plan 26 §4: nirvana_role token identifies Nirvana Claude
    Control(
        'p26_nirvana_role_token_identifies_nirvana',
        'src/client/shell/nirvana_spaces.rs',
        '            let who = if token(ROLE_TOKEN).is_some() {\n                "Nirvana".to_string()\n            } else {\n',
        '            let who = if false && token(ROLE_TOKEN).is_some() {\n                "Nirvana".to_string()\n            } else {\n',
        'nirvana_claude_shows_who_as_nirvana',
        'rust',
    ),
    # Plan 26 §4: fold two or more idle panes of same kind without task
    Control(
        'p26_fold_idle_panes_same_kind',
        'src/client/shell/nirvana_spaces.rs',
        '        if is_idle_no_task && count >= 2 {\n',
        '        if false && is_idle_no_task && count >= 2 {\n',
        'fold_two_or_more_idle_panes_of_same_kind_without_task',
        'rust',
    ),
    # Plan 26 §4 & Review: compact model extracts family initial for standard models
    Control(
        'p26_compact_model_family',
        'src/nirvana_spaces.rs',
        '        let family_initial = if lower.contains("sonnet") {\n            \'S\'\n',
        '        let family_initial = if false && lower.contains("sonnet") {\n            \'S\'\n',
        'compact_model_table_test',
        'rust',
    ),
    # Plan 26 §4 & Review: compact model extracts Flash for Gemini models
    Control(
        'p26_compact_model_gemini_flash',
        'src/nirvana_spaces.rs',
        '    if lower.contains("flash") {\n        return "Flash".to_string();\n    }\n',
        '    if false && lower.contains("flash") {\n        return "Flash".to_string();\n    }\n',
        'space_row_whole_row_equality_at_26_cells',
        'rust',
    ),
    # Plan 26 §5: agents panel renders Run header for grouped runs
    Control(
        'p26_subagents_run_header',
        'src/client/shell/agent_sidebar.rs',
        '            ratatui::text::Span::raw("  Run "),\n',
        '            ratatui::text::Span::raw("  Job "),\n',
        'render_grouped_subagents_under_run_headers',
        'rust',
    ),
    # Plan 26 §5: live workers ordered newest first by elapsed seconds
    Control(
        'p26_subagents_live_workers_order_newest_first',
        'src/client/shell/agent_sidebar.rs',
        '            // live workers first, longest running first: largest elapsed_secs first\n            live.sort_by_key(|s| std::cmp::Reverse(s.elapsed_secs));\n',
        '            live.sort_by_key(|s| s.elapsed_secs);\n',
        'group_subagents_by_run_ordering',
        'rust',
    ),
    # Plan 26 §5: finished done worker gets checkmark mark
    Control(
        'p26_subagents_finished_worker_done_mark',
        'src/client/shell/agent_sidebar.rs',
        '                crate::nirvana_subagents::SubagentState::Done => {\n                    (\'\\u{2713}\', palette.green, "done")\n                }\n',
        '                crate::nirvana_subagents::SubagentState::Done => {\n                    (\'?\', palette.green, "done")\n                }\n',
        'render_grouped_subagents_under_run_headers',
        'rust',
    ),
    # Plan 26 §5: clicking finished worker whose pane was closed targets orchestrator
    Control(
        'p26_subagents_closed_pane_click_targets_orch',
        'src/client/shell/agent_sidebar.rs',
        '        if sub.run_id.is_some() && !is_live_subagent(sub.state) {\n            if let Some(target) = &sub.target_pane_id {\n                if !snapshot.panes.iter().any(|p| p.pane_id == *target) {\n                    sub.target_pane_id = Some(pane_id.to_string());\n                }\n            }\n        }\n',
        '        // do nothing\n',
        'render_grouped_subagents_under_run_headers',
        'rust',
    ),
    # Plan 26 §5: agent row lines accounts for run header and grouped workers
    Control(
        'p26_subagents_run_lines_calculation',
        'src/client/shell/agent_sidebar.rs',
        '    if row.subagents.iter().any(|s| s.run_id.is_some()) {\n        let capped = group_sort_and_cap_subagents(&row.subagents, usize::MAX);\n        let mut lines = 0;\n        for g in &capped.groups {\n            lines += 1;\n            lines += g.live.len() * 2;\n            lines += g.finished.len();\n        }\n        let overflow_line = if capped.cut_count > 0 || row.subagents_overflow > 0 {\n            1\n        } else {\n            0\n        };\n        return base + lines + overflow_line;\n    }\n',
        '    if row.subagents.iter().any(|s| s.run_id.is_some()) {\n        return 0;\n    }\n',
        'render_grouped_subagents_under_run_headers',
        'rust',
    ),
    # Review: exec closes pane only after Delivered is journaled
    Control(
        'p26_exec_closes_pane_after_delivered',
        'src/nirvana_worker.rs',
        '    let phase = state\n        .order(order)\n        .map(|o| o.phase)\n        .unwrap_or(crate::nirvana_run::Phase::Delivered);\n',
        '    let phase = crate::nirvana_run::Phase::Delivered;\n',
        'exec_closes_pane_after_delivered',
        'rust',
    ),
    # Review: run end calls close_run_tab
    Control(
        'p26_run_end_calls_close_run_tab',
        'src/nirvana_run.rs',
        '    println!("{}", dir.join("receipt.md").display());\n    close_run_tab(project, &run, &state)?;\n',
        '    println!("{}", dir.join("receipt.md").display());\n    // close_run_tab(project, &run, &state)?;\n',
        'run_end_calls_close_run_tab',
        'rust',
    ),
    # Review: tidy_cli acts only on ended runs
    Control(
        'p26_tidy_only_ended_runs',
        'src/nirvana_run.rs',
        '    state.ended.is_some() && state.orders.iter().any(|o| o.pane.is_some())\n',
        '    true && state.orders.iter().any(|o| o.pane.is_some())\n',
        'tidy_only_acts_on_ended_runs',
        'rust',
    ),
)
