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
        '        if cwd\n            .to_string_lossy()\n            .replace(\'\\\\\', "/")\n            .contains(".nirvana/worktrees")\n        {\n            return true;\n        }\n',
        '        if false {\n            return true;\n        }\n',
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
    # Plan 24 §5: sources_axis checks cited URLs appear in worker stream
    Control(
        'p24_sources_axis_checks_citations',
        'src/nirvana_gate.rs',
        '        if !url_in_stream(norm, stream) {\n            missing.push(url.to_owned());\n        }\n',
        '        if false && !url_in_stream(norm, stream) {\n            missing.push(url.to_owned());\n        }\n',
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
    # Plan 24 §5: herdr run end refuses delivered ungated orders
    Control(
        'p24_run_end_refuses_ungated',
        'src/nirvana_run.rs',
        '        .filter(|order| order.phase == Phase::Delivered)\n',
        '        .filter(|order| false && order.phase == Phase::Delivered)\n',
        'run_end_refuses_delivered_ungated_orders_and_abandon_allows_end',
        'rust',
    ),
    # Plan 24 §6: seat_order refuses build, fix, plan missing owns
    Control(
        'p24_seat_order_requires_owns',
        'src/nirvana_worker.rs',
        '    if request\n        .class\n        .as_deref()\n        .is_some_and(crate::nirvana_gate::requires_owns)\n        && spec.owns.is_empty()\n    {\n',
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
