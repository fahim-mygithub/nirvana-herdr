# Plan 24: five fixes from the audit of the first real runs (package `run_fixes`)
# Controls starting with p24_.

CONTROLS = (
    # Plan 24 §2: log-usage deduplicates repeated message id
    Control(
        'p24_log_usage_dedup',
        'src/nirvana_claude.rs',
        '            let usage_slot = seen.entry(id).or_insert([0; 4]);\n',
        '            let usage_slot = seen.entry(format!("{}-{}", id, seen.len())).or_insert([0; 4]);\n',
        'log_usage_fixture_transcript_deduplicates_repeated_message_id_across_calls',
        'rust',
    ),
    # Plan 24 §2: SessionStart with resume initializes offset to transcript length
    Control(
        'p24_log_usage_session_start_init',
        'src/nirvana_claude.rs',
        '    if is_session_start && stored_state.is_none() {\n',
        '    if false && is_session_start && stored_state.is_none() {\n',
        'resume_with_existing_transcript_appends_no_usage',
        'rust',
    ),
    # Plan 24 §2: write updated usage offset to disk
    Control(
        'p24_log_usage_write_offset',
        'src/nirvana_claude.rs',
        '    let _ = nirvana_seat::write_atomic(&offset_path, serde_json::to_string(&new_state)?.as_bytes());\n',
        '    let _ = false;\n',
        'log_usage_session_start_gating_and_write_offset',
        'rust',
    ),
    # Plan 24 §2 & §3: hook skips when agent_id is present
    Control(
        'p24_hook_skip_agent_id',
        'src/nirvana_claude.rs',
        '    if hook.get("agent_id").is_some_and(|a| !a.is_null()) {\n        return true;\n    }\n',
        '    if false && hook.get("agent_id").is_some_and(|a| !a.is_null()) {\n        return true;\n    }\n',
        'guard_edit_and_check_edit_never_block_subagent_or_worker',
        'rust',
    ),
    # Plan 24 §3: hook skips when in worktrees
    Control(
        'p24_hook_skip_worktree',
        'src/nirvana_claude.rs',
        '            .contains(".nirvana/worktrees")\n        {\n            return true;\n        }\n',
        '            .contains(".nirvana/worktrees")\n        {\n            return false;\n        }\n',
        'guard_edit_and_check_edit_never_block_subagent_or_worker',
        'rust',
    ),
    # Plan 24 §3: hook skips when NIRVANA_ORCHESTRATOR != 1
    Control(
        'p24_hook_skip_orchestrator_env',
        'src/nirvana_claude.rs',
        '    if std::env::var(crate::nirvana_launch::ORCHESTRATOR_ENV).as_deref() != Ok("1") {\n        return true;\n    }\n',
        '    if false && std::env::var(crate::nirvana_launch::ORCHESTRATOR_ENV).as_deref() != Ok("1") {\n        return true;\n    }\n',
        'guard_edit_and_check_edit_never_block_subagent_or_worker',
        'rust',
    ),
    # Plan 24 §3: guard_edit_decision blocks when call_lines > limit.lines
    Control(
        'p24_guard_edit_limit_lines',
        'src/nirvana_claude.rs',
        '    if call_lines > limit.lines {\n',
        '    if call_lines > 1000 {\n',
        'guard_edit_decision_blocks_past_limit',
        'rust',
    ),
    # Plan 24 §3: check_edit_decision outputs block decision
    Control(
        'p24_check_edit_decision_block',
        'src/nirvana_claude.rs',
        '    edit_budget(baseline, now_state, limit).map(|reason| {\n        json!({\n            "decision": "block",\n',
        '    edit_budget(baseline, now_state, limit).map(|reason| {\n        json!({\n            "decision": "allow",\n',
        'check_edit_decision_block_output',
        'rust',
    ),
    # Plan 24 §3: edit_budget limits files to limit.files
    Control(
        'p24_edit_limit_files',
        'src/nirvana_claude.rs',
        '    if changed_files > limit.files || changed_lines > limit.lines {\n',
        '    if changed_files > 100 || changed_lines > limit.lines {\n',
        'edit_budget_counts_files_and_lines_within_and_past_limit',
        'rust',
    ),
    # Plan 24 §3: edit_budget limits lines to limit.lines (default 30)
    Control(
        'p24_edit_limit_lines',
        'src/nirvana_claude.rs',
        '            lines: 30,\n',
        '            lines: 300,\n',
        'edit_budget_counts_files_and_lines_within_and_past_limit',
        'rust',
    ),
    # Plan 24 §3: is_out_of_scope ignores .nirvana/
    Control(
        'p24_edit_scope_nirvana',
        'src/nirvana_claude.rs',
        '    if normalized.starts_with(".nirvana/")\n        || normalized == ".nirvana"\n',
        '    if false\n',
        'edit_budget_out_of_scope_paths_and_baseline_with_existing_changes',
        'rust',
    ),
    # Plan 24 §4: send_back inherits parent\'s override seat and reason
    Control(
        'p24_send_back_inherits_override',
        'src/nirvana_gate.rs',
        '    if let Some(reason) = &seated.override_reason {\n        request.override_seat = Some((\n            format!("{}:{}", seated.seat, seated.effort.word()),\n            reason.clone(),\n        ));\n        request.inherited_override = true;\n    }\n',
        '    if false && seated.override_reason.is_some() {\n    }\n',
        'send_back_inherits_override_and_does_not_count_against_run_overrides',
        'rust',
    ),
    # Plan 24 §4: send_back does not count inherited override against run overrides
    Control(
        'p24_send_back_override_count_unchanged',
        'src/nirvana_seat.rs',
        '        if !pick.inherited_override {\n            let count = leases.runs.entry(run.to_owned()).or_default();\n            count.overrides += 1;\n            count.at = now;\n            changed = true;\n        }\n',
        '        if true {\n            let count = leases.runs.entry(run.to_owned()).or_default();\n            count.overrides += 1;\n            count.at = now;\n            changed = true;\n        }\n',
        'send_back_inherits_override_and_does_not_count_against_run_overrides',
        'rust',
    ),
    # Plan 24 §4: evaluate exempts inherited override from override cap
    Control(
        'p24_send_back_cap_exempt',
        'src/nirvana_seat.rs',
        '        if !request.inherited_override {\n',
        '        if true {\n',
        'send_back_inherits_override_and_does_not_count_against_run_overrides',
        'rust',
    ),
    # Plan 24 §4: send_back retries with rule pick when inherited override is refused
    Control(
        'p24_send_back_refused_fallback',
        'src/nirvana_gate.rs',
        '            Err(no_seat) if request.inherited_override => {\n',
        '            Err(no_seat) if false && request.inherited_override => {\n',
        'send_back_retries_with_rule_pick_when_inherited_model_is_refused',
        'rust',
    ),
    # Plan 24 §4: rate_limited drops override on same pool and keeps on other
    Control(
        'p24_rate_limited_drops_on_same_pool',
        'src/nirvana_worker.rs',
        '            if limit_pool == seated.pool.as_str() {\n                (true, false, None)\n            } else {\n',
        '            if limit_pool != seated.pool.as_str() {\n                (true, false, None)\n            } else {\n',
        'rate_limited_reseat_drops_override_on_same_pool_and_keeps_on_other',
        'rust',
    ),
    # Plan 24 §5: worker Work Report schema validates sources
    Control(
        'p24_worker_report_schema_sources',
        'src/nirvana_worker.rs',
        '    if let Some(sources) = object.get("sources") {\n        let Some(items) = sources.as_array() else {\n            return Err("the Work Report sources is not an array".into());\n        };\n',
        '    if false && object.get("sources").is_some() {\n        let Some(items) = None::<Vec<Value>> else {\n            return Err("the Work Report sources is not an array".into());\n        };\n',
        'worker_report_schema_sources_validation',
        'rust',
    ),
    # Plan 24 §5: sources_axis tool call matcher accepts only real fetch/browser calls
    Control(
        'p24_gate_sources_tool_call_matcher',
        'src/nirvana_gate.rs',
        '                    let is_fetch = name == "WebFetch"\n                        || (name.starts_with("mcp__")\n                            && (name.contains("browser") || name.contains("bladebro")));\n',
        '                    let is_fetch = false;\n',
        'sources_axis_b2_real_fetch_calls_and_url_normalization',
        'rust',
    ),
    # Plan 24 §5: deterministic adds sources_axis for research and media
    Control(
        'p24_gate_sources_axis_in_deterministic',
        'src/nirvana_gate.rs',
        '    if let Some(axis) = sources_axis(subject.class, subject.report, &stream) {\n',
        '    if let Some(axis) = sources_axis(subject.class, subject.report, &stream).filter(|_| false) {\n',
        'deterministic_includes_sources_axis_for_research_and_media',
        'rust',
    ),
    # Plan 24 §5: sources_axis checks cited URLs appear in worker stream
    Control(
        'p24_sources_axis_checks_citations',
        'src/nirvana_gate.rs',
        '        if !url_in_stream(&norm, stream) {\n            missing.push(url.to_owned());\n        }\n',
        '        if false && !url_in_stream(&norm, stream) {\n            missing.push(url.to_owned());\n        }\n',
        'sources_axis_checks_citations_in_worker_stream',
        'rust',
    ),
    # Plan 24 §5: sources_axis fails when sources list is empty
    Control(
        'p24_sources_axis_empty_fails',
        'src/nirvana_gate.rs',
        '        _ => {\n            return Some(Axis::new(\n                "sources",\n                Mark::Fail,\n                "no sources: the Work Report names no sources",\n            ))\n        }\n',
        '        _ => return Some(Axis::new("sources", Mark::Pass, "no sources")),\n',
        'sources_axis_checks_citations_in_worker_stream',
        'rust',
    ),
    # Plan 24 §5: end_decision refuses delivered ungated orders
    Control(
        'p24_run_end_refuses_ungated',
        'src/nirvana_run.rs',
        '        if !ungated.is_empty() {\n',
        '        if false && !ungated.is_empty() {\n',
        'end_decision_refusal_and_abandon',
        'rust',
    ),
    # Plan 24 §5: end_decision records abandoned verdict on --abandon
    Control(
        'p24_run_end_abandon_verdict',
        'src/nirvana_run.rs',
        '            verdict: "abandoned".into(),\n',
        '            verdict: "passed".into(),\n',
        'end_decision_refusal_and_abandon',
        'rust',
    ),
    # Plan 24 §6: seat_order refuses build, fix, plan missing owns
    Control(
        'p24_seat_order_requires_owns',
        'src/nirvana_worker.rs',
        '    if crate::nirvana_gate::requires_owns(&booked.class) && spec.owns.is_empty() {\n',
        '    if false && spec.owns.is_empty() {\n',
        'seat_order_refuses_empty_owns_for_build_fix_plan_and_permits_read_only',
        'rust',
    ),
    # Plan 24 §6: owned_axis fails on empty owns for build, fix, plan
    Control(
        'p24_owned_axis_empty_fails_build',
        'src/nirvana_gate.rs',
        '        if requires_owns(class) {\n            return Axis::new(\n                "owned",\n                Mark::Fail,\n                "unbounded: the order names no owned file",\n            )\n            .with(changed.iter().map(|path| (*path).clone()).collect());\n        }\n',
        '        if false && requires_owns(class) {\n            return Axis::new("owned", Mark::Fail, "unbounded").with(vec![]);\n        }\n',
        'owned_axis_empty_owns_fails_for_build_fix_plan_and_notes_for_read_only',
        'rust',
    ),
)
