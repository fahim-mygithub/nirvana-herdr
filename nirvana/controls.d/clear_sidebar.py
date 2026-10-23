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
    # Plan 26 §3: run end closes run tab when all panes are worker panes
    Control(
        'p26_close_run_tab_all_workers',
        'src/nirvana_run.rs',
        '    let all_ours = tab_panes.iter().all(|pane| run_worker_panes.contains(pane));\n',
        '    let all_ours = false;\n',
        'close_run_tab_decisions',
        'rust',
    ),
    # Plan 26 §3: run end closes only worker panes when tab has mixed panes
    Control(
        'p26_close_run_tab_mixed_panes',
        'src/nirvana_run.rs',
        '        let to_close: Vec<String> = tab_panes\n            .iter()\n            .filter(|pane| run_worker_panes.contains(pane))\n            .cloned()\n            .collect();\n',
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
    # Plan 26 §4: focused pane shows chat title
    Control(
        'p26_focused_pane_shows_chat_title',
        'src/client/shell/nirvana_spaces.rs',
        '            } else if agent.focused && agent.task != "No task yet" && !agent.task.is_empty() {\n',
        '            } else if false && agent.focused && agent.task != "No task yet" && !agent.task.is_empty() {\n',
        'focused_pane_shows_chat_title_and_no_task_yet_never_shown',
        'rust',
    ),
    # Plan 26 §4: No task yet string is never shown
    Control(
        'p26_no_task_yet_never_shown',
        'src/client/shell/nirvana_spaces.rs',
        '                let word = status_word(agent.status);\n',
        '                let word = "No task yet";\n',
        'an_older_server_without_tokens_still_names_the_agent',
        'rust',
    ),
    # Plan 26 §4: compact model extracts family name for standard models
    Control(
        'p26_compact_model_family',
        'src/nirvana_spaces.rs',
        '        if matches!(family, "Sonnet" | "Opus" | "Haiku" | "Fable") {\n            return family.to_string();\n        }\n',
        '        if false {\n            return family.to_string();\n        }\n',
        'a_space_lists_its_agents_task_and_model_with_the_agent_icon',
        'rust',
    ),
    # Plan 26 §4: compact model extracts Flash for Gemini models
    Control(
        'p26_compact_model_gemini_flash',
        'src/nirvana_spaces.rs',
        '    if lower.contains("flash") {\n        return "Flash".to_string();\n    }\n',
        '    if false && lower.contains("flash") {\n        return "Flash".to_string();\n    }\n',
        'each_kind_draws_its_own_icon_and_others_keep_the_dot',
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
    # Plan 26 §5: runs with live workers come first
    Control(
        'p26_subagents_runs_with_live_workers_first',
        'src/client/shell/agent_sidebar.rs',
        '        match (a_live, b_live) {\n            (true, false) => std::cmp::Ordering::Less,\n            (false, true) => std::cmp::Ordering::Greater,\n',
        '        match (a_live, b_live) {\n            (true, false) => std::cmp::Ordering::Greater,\n            (false, true) => std::cmp::Ordering::Less,\n',
        'group_subagents_by_run_ordering',
        'rust',
    ),
    # Plan 26 §5: live workers ordered newest first by elapsed seconds
    Control(
        'p26_subagents_live_workers_order_newest_first',
        'src/client/shell/agent_sidebar.rs',
        '            // live workers first, newest first: smallest elapsed_secs first\n            live.sort_by_key(|s| s.elapsed_secs);\n',
        '            live.sort_by_key(|s| std::cmp::Reverse(s.elapsed_secs));\n',
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
        '    if row.subagents.iter().any(|s| s.run_id.is_some()) {\n        let groups = group_subagents_by_run(&row.subagents);\n        let mut lines = 0;\n        for g in &groups {\n            lines += 1;\n            lines += g.live.len() * 2;\n            lines += g.finished.len();\n        }\n        let overflow_line = if row.subagents_overflow > 0 { 1 } else { 0 };\n        return base + lines + overflow_line;\n    }\n',
        '    if row.subagents.iter().any(|s| s.run_id.is_some()) {\n        return 0;\n    }\n',
        'render_grouped_subagents_under_run_headers',
        'rust',
    ),
)
