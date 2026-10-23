# Plan 17: subagents under each agent in the agents panel (nirvana/plans/2026-10-19-17-subagents.md).
# Red line for each: its test prints FAIL.
CONTROLS = (
    # §3: Folder encoding converts non-alphanumeric characters to dash.
    Control(
        'p17_folder_encoding_non_alphanumeric',
        'src/nirvana_subagents.rs',
        "    s.chars()\n        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })\n        .collect()",
        "    s.chars()\n        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })\n        .collect()",
        'folder_encoding_converts_non_alphanumeric_to_dash',
        'rust',
    ),
    # §3: Shared folder refusal: two Claude panes sharing one folder with ambiguous session refuse subagents.
    Control(
        'p17_shared_folder_refusal',
        'src/nirvana_subagents.rs',
        "    if has_conflict {\n        // Refusal: two Claude panes share one folder and nothing tells them apart\n        return None;\n    }",
        "    if has_conflict {\n        // Refusal: two Claude panes share one folder and nothing tells them apart\n        return Some(std::path::PathBuf::from(\"conflict-refusal-bypassed.jsonl\"));\n    }",
        'session_resolution_order_and_shared_folder_refusal',
        'rust',
    ),
    # §3: Running subagents must have no finished flag and transcript mtime within 300s.
    Control(
        'p17_running_subagent_age_rule',
        'src/nirvana_subagents.rs',
        "    if now_secs.saturating_sub(transcript_mtime_secs) <= 300 {\n        SubagentState::Running\n    } else {",
        "    if now_secs.saturating_sub(transcript_mtime_secs) <= 30 {\n        SubagentState::Running\n    } else {",
        'meta_and_transcript_pair_cases',
        'rust',
    ),
    # §3: Claude token figure sums input + cache creation + cache read + output tokens.
    Control(
        'p17_token_figure_sums_all_usage_types',
        'src/nirvana_subagents.rs',
        "    input + cache_create + cache_read + output",
        "    input + output",
        'token_figure_sums_measured_context',
        'rust',
    ),
    # §2: Claude model spellings match strip doctrine (e.g. Sonnet 5.5).
    Control(
        'p17_model_spelling_claude_sonnet',
        'src/nirvana_subagents.rs',
        "            if let Some(label) = crate::nirvana_spaces::claude_label(m) {\n                if let Some(stripped) = label.strip_prefix(\"Claude \") {\n                    stripped.to_string()\n                } else {\n                    label\n                }",
        "            if let Some(label) = crate::nirvana_spaces::claude_label(m) {\n                if let Some(_stripped) = label.strip_prefix(\"Claude \") {\n                    \"Claude-Unknown\".to_string()\n                } else {\n                    label\n                }",
        'model_spellings_match_doctrine',
        'rust',
    ),
    # §4: agy argv extracts model from --model flag.
    Control(
        'p17_agy_argv_model_flag',
        'src/nirvana_subagents.rs',
        "        if arg == \"--model\" || arg == \"-m\" {\n            return rest.next().filter(|value| !value.starts_with('-')).cloned();\n        }",
        "        if false && (arg == \"--model\" || arg == \"-m\") {\n            return rest.next().filter(|value| !value.starts_with('-')).cloned();\n        }",
        'agy_argv_to_model',
        'rust',
    ),
    # §4: agy stream parses latest step tokens (input + cache_read + output).
    Control(
        'p17_agy_stream_tokens_sum',
        'src/nirvana_subagents.rs',
        "            return Some(input + cache_read + output);",
        "            return Some(input + output);",
        'agy_stream_to_tokens',
        'rust',
    ),
    # §3: Workflow nesting places child subagents at depth 1 under parent.
    Control(
        'p17_workflow_nesting_depth',
        'src/nirvana_subagents.rs',
        "            for mut child in children {\n                child.depth = 1;\n                result.push(child);\n            }",
        "            for mut child in children {\n                child.depth = 0;\n                result.push(child);\n            }",
        'workflow_nesting_places_child_under_parent',
        'rust',
    ),
    # §4: Seat worker panes are recorded and filtered from top-level agent rows.
    Control(
        'p17_seat_worker_pane_filtered',
        'src/nirvana_subagents.rs',
        "pub fn is_seat_worker_pane(pane_id: &str) -> bool {\n    #[cfg(test)]\n    {\n        if let Some(set) = TEST_WORKER_PANES.with_borrow(|s| s.clone()) {\n            return set.contains(pane_id);\n        }\n    }",
        "pub fn is_seat_worker_pane(pane_id: &str) -> bool {\n    #[cfg(test)]\n    {\n        if let Some(set) = TEST_WORKER_PANES.with_borrow(|s| s.clone()) {\n            return false && set.contains(pane_id);\n        }\n    }",
        'worker_pane_filtering',
        'rust',
    ),
    # §2: Claude subagent mark is asterisk.
    Control(
        'p17_provider_mark_claude',
        'src/nirvana_subagents.rs',
        "            Self::Claude => \"\\u{273B}\",",
        "            Self::Claude => \"\\u{25CF}\",",
        'marks_and_colours_by_provider',
        'rust',
    ),
    # §2: Gemini subagent mark is diamond.
    Control(
        'p17_provider_mark_gemini',
        'src/nirvana_subagents.rs',
        "            Self::Gemini => \"\\u{25C6}\",",
        "            Self::Gemini => \"\\u{25CF}\",",
        'marks_and_colours_by_provider',
        'rust',
    ),
    # §2: Adaptive width narrow threshold is width < 34.
    Control(
        'p17_adaptive_width_narrow_threshold',
        'src/client/shell/agent_sidebar.rs',
        "pub(super) fn is_narrow_agent_panel(width: u16) -> bool {\n    width < 34\n}",
        "pub(super) fn is_narrow_agent_panel(width: u16) -> bool {\n    width < 10\n}",
        'narrow_and_wide_adaptive_formatting',
        'rust',
    ),
    # §2: Subagents overflow renders +N more.
    Control(
        'p17_overflow_shows_plus_n_more',
        'src/client/shell/agent_sidebar.rs',
        '            ratatui::text::Span::styled(format!("+{} more", row.subagents_overflow), more_style),',
        '            ratatui::text::Span::styled("more".to_string(), more_style),',
        'overflow_shows_plus_n_more',
        'rust',
    ),
    # §2: Finished subagents are dimmed.
    Control(
        'p17_finished_subagents_dimmed',
        'src/client/shell/agent_sidebar.rs',
        '            if is_finished {\n                check_style = check_style.add_modifier(Modifier::DIM);\n            }',
        '            if false && is_finished {\n                check_style = check_style.add_modifier(Modifier::DIM);\n            }',
        'finished_subagents_dimmed_and_ten_minute_filter',
        'rust',
    ),
    # §2: Clicking a subagent row targets its pane in hit map.
    Control(
        'p17_click_subagent_targets_pane',
        'src/client/shell/agent_sidebar.rs',
        "            hits.agents.push((\n                Rect::new(rect.x, current_y, rect.width, actual_height),\n                target,\n            ));",
        "            // hits.agents.push((\n            //     Rect::new(rect.x, current_y, rect.width, actual_height),\n            //     target,\n            // ));",
        'click_subagent_row_focuses_target_pane',
        'rust',
    ),
    # §2: Row lines calculation accounts for subagents (1 line narrow, 2 lines wide).
    Control(
        'p17_row_lines_accounts_for_subagents',
        'src/client/shell/agent_sidebar.rs',
        "    let sub_lines: usize = row\n        .subagents\n        .iter()\n        .filter(|s| s.run_id.is_none())\n        .map(|s| {\n            if !narrow && !s.description.trim().is_empty() {\n                2\n            } else {\n                1\n            }\n        })\n        .sum();",
        "    let sub_lines: usize = 0;",
        'row_lines_calculation',
        'rust',
    ),
    # §2: Subagents render under agent in both flat and grouped views.
    Control(
        'p17_subagents_rendered_under_agent_in_both_views',
        'src/client/shell/agent_sidebar.rs',
        "    for sub in row.subagents.iter().filter(|s| s.run_id.is_none()) {\n        if current_y >= rect.bottom() {\n            break;\n        }",
        "    for sub in row.subagents[..0].iter().filter(|s| s.run_id.is_none()) {\n        if current_y >= rect.bottom() {\n            break;\n        }",
        'subagent_rows_render_under_agent_in_flat_and_grouped_views',
        'rust',
    ),
    # Review fix: SubagentProvider::colour delegates to nirvana_usage::Provider::colour
    Control(
        'p17_provider_colour_delegates_to_usage',
        'src/nirvana_subagents.rs',
        "        let usage_provider = match self {\n            Self::Claude => crate::nirvana_usage::Provider::Claude,\n            Self::Gemini => crate::nirvana_usage::Provider::Antigravity,\n            Self::Local => crate::nirvana_usage::Provider::Local,\n        };\n        usage_provider.colour()",
        "        let usage_provider = match self {\n            Self::Claude => crate::nirvana_usage::Provider::Local,\n            Self::Gemini => crate::nirvana_usage::Provider::Antigravity,\n            Self::Local => crate::nirvana_usage::Provider::Local,\n        };\n        usage_provider.colour()",
        'marks_and_colours_by_provider',
        'rust',
    ),
    # Review fix finding 2: session id read from AgentInfo.agent_session (claude and Id)
    Control(
        'p17_session_id_from_agent_session_info',
        'src/client/shell/agent_sidebar.rs',
        "    let agent_session = agent\n        .agent_session\n        .as_ref()\n        .filter(|s| s.agent == \"claude\" && s.kind == crate::agent_resume::AgentSessionRefKind::Id)\n        .map(|s| s.value.clone());",
        "    let agent_session = agent\n        .tokens\n        .iter()\n        .find(|(k, _)| k == \"agent_session\")\n        .map(|(_, v)| v.clone());",
        'agent_session_id_from_agent_session_info',
        'rust',
    ),
    # Review fix finding 3: only canonical Claude agent shows Claude subagents
    Control(
        'p17_only_canonical_claude_shows_claude_subagents',
        'src/client/shell/agent_sidebar.rs',
        "    let is_claude = canonical_agent == Some(crate::detect::Agent::Claude);",
        "    let is_claude = agent\n        .agent\n        .as_deref()\n        .is_none_or(|a| !a.to_ascii_lowercase().contains(\"agy\"));",
        'codex_pane_in_claude_folder_shows_no_claude_subagents',
        'rust',
    ),
    # Review fix finding 1: resolve Claude Code dir via separate claude_dir, not Nirvana home
    Control(
        'p17_claude_dir_separate_from_nirvana_home',
        'src/nirvana_subagents.rs',
        "    if has_conflict {\n        // Refusal: two Claude panes share one folder and nothing tells them apart\n        return None;\n    }\n\n    let projects_dir = claude_dir.join(\"projects\").join(&enc_folder);",
        "    if has_conflict {\n        // Refusal: two Claude panes share one folder and nothing tells them apart\n        return None;\n    }\n\n    let projects_dir = nirvana_home.join(\"projects\").join(&enc_folder);",
        'claude_subagents_found_under_claude_dir_separate_from_nirvana_home',
        'rust',
    ),
    # Review fix finding 4: shared folder refusal runs across other Claude panes in poll
    Control(
        'p17_shared_folder_refusal_in_poll',
        'src/nirvana_subagents.rs',
        "                let other_claude = all_claude\n                    .iter()\n                    .filter(|(id, _, _)| id != pane_id)\n                    .cloned()\n                    .collect();",
        "                let other_claude = Vec::new();",
        'shared_folder_refusal_in_poll',
        'rust',
    ),
    # Review fix finding 5: match agy by executable name, not cmdline substring
    Control(
        'p17_agy_cmdline_executable_match',
        'src/nirvana_subagents.rs',
        "pub fn is_agy_cmdline(cmdline: &str) -> bool {\n    let trimmed = cmdline.trim();\n    if trimmed.is_empty() {\n        return false;\n    }\n    let exe = if let Some(stripped) = trimmed.strip_prefix('\"') {\n        stripped.split('\"').next().unwrap_or(\"\")\n    } else {\n        trimmed.split_whitespace().next().unwrap_or(\"\")\n    };\n    is_agy_executable(exe)\n}",
        "pub fn is_agy_cmdline(cmdline: &str) -> bool {\n    cmdline.contains(\"agy\")\n}",
        'agy_exec_name_matching',
        'rust',
    ),
    # Review fix finding 6: expand transcript tail window up to 8 MB to find assistant turn
    Control(
        'p17_transcript_tail_window_expansion',
        'src/nirvana_subagents.rs',
        "        if window >= MAX_TAIL_WINDOW {\n            return tail;\n        }\n        if let Ok(meta) = std::fs::metadata(path) {\n            if meta.len() <= window {\n                return tail;\n            }\n        }\n        window = (window * 4).min(MAX_TAIL_WINDOW);",
        "        return tail;",
        'tail_window_expands_to_find_assistant',
        'rust',
    ),
    # Review fix finding A: pane process-info CLI args use --pane <id>
    Control(
        'p17_pane_process_info_cli_args',
        'src/nirvana_subagents.rs',
        "pub fn pane_process_info_args(pane_id: &str) -> Vec<String> {\n    vec![\n        \"pane\".into(),\n        \"process-info\".into(),\n        \"--pane\".into(),\n        pane_id.into(),\n    ]\n}",
        "pub fn pane_process_info_args(pane_id: &str) -> Vec<String> {\n    vec![\n        \"pane\".into(),\n        \"process-info\".into(),\n        pane_id.into(),\n        \"--json\".into(),\n    ]\n}",
        'pane_process_info_args_parsed_by_cli',
        'rust',
    ),
    # Review fix finding B: cached shell pid survives None update and failed query throttled 60s
    Control(
        'p17_cached_shell_pid_survives_and_query_throttled',
        'src/nirvana_subagents.rs',
        "    // Finding B: keep cached shell_pid when caller passes None\n    if shell_pid.is_some() {\n        entry.shell_pid = shell_pid;\n    }",
        "    entry.shell_pid = shell_pid;",
        'cached_shell_pid_survives_none_update_and_failed_query_throttled',
        'rust',
    ),
)

