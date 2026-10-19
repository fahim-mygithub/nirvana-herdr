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
        "    let sub_lines: usize = row\n        .subagents\n        .iter()\n        .map(|s| {\n            if !narrow && !s.description.trim().is_empty() {\n                2\n            } else {\n                1\n            }\n        })\n        .sum();",
        "    let sub_lines: usize = 0;",
        'row_lines_calculation',
        'rust',
    ),
    # §2: Subagents render under agent in both flat and grouped views.
    Control(
        'p17_subagents_rendered_under_agent_in_both_views',
        'src/client/shell/agent_sidebar.rs',
        "    for sub in &row.subagents {\n        if current_y >= rect.bottom() {\n            break;\n        }",
        "    for sub in &row.subagents[..0] {\n        if current_y >= rect.bottom() {\n            break;\n        }",
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
)

