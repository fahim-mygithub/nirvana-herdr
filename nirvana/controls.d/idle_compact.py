# Plan 27: an idle Claude compacts before its cache goes stale (package `idle_compact`)
# Controls starting with p27_.

CONTROLS = (
    # Plan 27 §2 rule 1: the setting off sends nothing
    Control(
        'p27_rule1_setting_off',
        'src/nirvana_idle_compact.rs',
        '    let Some(rule) = facts.rule else {\n        return skip("idle compact is off".into());\n    };\n',
        '    let rule = facts.rule.unwrap_or(IdleCompactRule {\n        minutes: Duration::ZERO,\n        min_tokens: 0,\n    });\n',
        'idle_compact_rule1_setting_off_sends_nothing',
        'rust',
    ),
    # Plan 27 §2 rule 2: only Claude
    Control(
        'p27_rule2_only_claude',
        'src/nirvana_idle_compact.rs',
        '    if facts.agent != Some(Agent::Claude) {\n',
        '    if false && facts.agent != Some(Agent::Claude) {\n',
        'idle_compact_rule2_only_claude',
        'rust',
    ),
    # Plan 27 §2 rule 2: a seat worker pane is left alone
    Control(
        'p27_rule2_seat_worker',
        'src/nirvana_idle_compact.rs',
        '    if facts.seat_worker {\n',
        '    if false && facts.seat_worker {\n',
        'idle_compact_rule2_seat_worker_pane_left_alone',
        'rust',
    ),
    # Plan 27 §2 rule 2: an unread process sends nothing
    Control(
        'p27_rule2_unread_process',
        'src/nirvana_idle_compact.rs',
        '    let Some(argv) = facts.argv.as_deref() else {\n        return skip("Claude\'s process could not be read".into());\n    };\n',
        '    let argv = facts.argv.as_deref().unwrap_or(&[]);\n',
        'idle_compact_rule2_unread_process_sends_nothing',
        'rust',
    ),
    # Plan 27 §2 rule 2: a headless Claude is a worker
    Control(
        'p27_rule2_headless_check',
        'src/nirvana_idle_compact.rs',
        '    if is_headless(argv) {\n',
        '    if false && is_headless(argv) {\n',
        'idle_compact_rule2_headless_claude_left_alone',
        'rust',
    ),
    # Plan 27 §2 rule 2: --print counts as headless, not only -p
    Control(
        'p27_rule2_headless_long_flag',
        'src/nirvana_idle_compact.rs',
        '    argv.iter().any(|arg| arg == "-p" || arg == "--print")\n',
        '    argv.iter().any(|arg| arg == "-p")\n',
        'idle_compact_rule2_headless_claude_left_alone',
        'rust',
    ),
    # Plan 27 §2 rule 3: only an idle pane
    Control(
        'p27_rule3_not_idle',
        'src/nirvana_idle_compact.rs',
        '    if facts.state != AgentState::Idle {\n',
        '    if false && facts.state != AgentState::Idle {\n',
        'idle_compact_rule3_only_an_idle_pane',
        'rust',
    ),
    # Plan 27 §2 rule 3: idle under the minutes waits
    Control(
        'p27_rule3_minutes',
        'src/nirvana_idle_compact.rs',
        '    if facts.idle_for.is_none_or(|idle| idle < rule.minutes) {\n',
        '    if facts.idle_for.is_none() {\n',
        'idle_compact_rule3_idle_under_minutes_waits',
        'rust',
    ),
    # Plan 27 §2 rule 3: an unknown idle time sends nothing
    Control(
        'p27_rule3_unknown_idle',
        'src/nirvana_idle_compact.rs',
        '    if facts.idle_for.is_none_or(|idle| idle < rule.minutes) {\n',
        '    if facts.idle_for.is_some_and(|idle| idle < rule.minutes) {\n',
        'idle_compact_rule3_idle_under_minutes_waits',
        'rust',
    ),
    # Plan 27 §2 rule 3: at least the minutes, so exactly the minutes sends
    Control(
        'p27_rule3_at_least',
        'src/nirvana_idle_compact.rs',
        '|idle| idle < rule.minutes) {\n',
        '|idle| idle <= rule.minutes) {\n',
        'idle_compact_sends_when_every_rule_holds',
        'rust',
    ),
    # Plan 27 §2 rule 7: once per idle stretch
    Control(
        'p27_rule7_once',
        'src/nirvana_idle_compact.rs',
        '    if facts.sent_in_stretch {\n',
        '    if false && facts.sent_in_stretch {\n',
        'idle_compact_rule7_once_per_stretch',
        'rust',
    ),
    # Plan 27 §2 rule 4: the transcript must resolve to one file
    Control(
        'p27_rule4_resolve',
        'src/nirvana_idle_compact.rs',
        '    let Some(age) = facts.transcript_age else {\n        return skip("the pane\'s own transcript was not found".into());\n    };\n',
        '    let age = facts.transcript_age.unwrap_or(Duration::MAX);\n',
        'idle_compact_rule4_transcript_must_resolve',
        'rust',
    ),
    # Plan 27 §2 rule 4: a transcript written within the minutes waits
    Control(
        'p27_rule4_age',
        'src/nirvana_idle_compact.rs',
        '    if age < rule.minutes {\n',
        '    if false && age < rule.minutes {\n',
        'idle_compact_rule4_recent_transcript_waits',
        'rust',
    ),
    # Plan 27 §2 rule 5: the context must be read
    Control(
        'p27_rule5_read',
        'src/nirvana_idle_compact.rs',
        '    let Some(tokens) = facts.context_tokens else {\n        return skip("the transcript holds no context reading".into());\n    };\n',
        '    let tokens = facts.context_tokens.unwrap_or(u64::MAX);\n',
        'idle_compact_rule5_context_must_be_read',
        'rust',
    ),
    # Plan 27 §2 rule 5: a small context is left alone
    Control(
        'p27_rule5_min_tokens',
        'src/nirvana_idle_compact.rs',
        '    if tokens < rule.min_tokens {\n',
        '    if false && tokens < rule.min_tokens {\n',
        'idle_compact_rule5_small_context_left_alone',
        'rust',
    ),
    # Plan 27 §2 rule 6: an input line not provably empty sends nothing (review B1, B2)
    Control(
        'p27_rule6_no_line',
        'src/nirvana_idle_compact.rs',
        '        InputLine::Unsure(why) => skip(format!("the input line is not provably empty: {why}")),\n',
        '        InputLine::Unsure(_) => Decision::Send,\n',
        'idle_compact_rule6_no_input_line_sends_nothing',
        'rust',
    ),
    # Plan 27 §2 rule 6: typed text is left alone
    Control(
        'p27_rule6_typed_text',
        'src/nirvana_idle_compact.rs',
        '        InputLine::Text(_) => skip("the input line holds text".into()),\n',
        '        InputLine::Text(_) => Decision::Send,\n',
        'idle_compact_rule6_typed_text_left_alone',
        'rust',
    ),
    # Plan 27 §2: every rule holding sends
    Control(
        'p27_send',
        'src/nirvana_idle_compact.rs',
        '        InputLine::Empty => Decision::Send,\n',
        '        InputLine::Empty => skip(String::new()),\n',
        'idle_compact_sends_when_every_rule_holds',
        'rust',
    ),
    # Plan 27 §2 rule 3: any state but Idle ends the stretch
    Control(
        'p27_stretch_ends',
        'src/nirvana_idle_compact.rs',
        '        if state != AgentState::Idle {\n            return Self::default();\n        }\n',
        '        if false && state != AgentState::Idle {\n            return Self::default();\n        }\n',
        'idle_stretch_ends_on_any_other_state',
        'rust',
    ),
    # Plan 27 §2 rule 3: an idle stretch keeps its first instant
    Control(
        'p27_stretch_keeps_start',
        'src/nirvana_idle_compact.rs',
        '        if self.since.is_some() {\n            return self;\n        }\n',
        '        if false && self.since.is_some() {\n            return self;\n        }\n',
        'idle_stretch_starts_on_idle_and_keeps_its_start',
        'rust',
    ),
    # Plan 27 §2 rule 7: a stretch that was sent is never due again
    Control(
        'p27_due_not_after_send',
        'src/nirvana_idle_compact.rs',
        '        if self.sent {\n            return None;\n        }\n',
        '        if false && self.sent {\n            return None;\n        }\n',
        'idle_stretch_is_due_at_minutes_then_rechecks_until_the_cache_expires',
        'rust',
    ),
    # Plan 27 §2 (deadline loop): a skipped pane waits RECHECK, no spinning
    Control(
        'p27_due_recheck',
        'src/nirvana_idle_compact.rs',
        '            Some(checked) => first.max(checked.checked_add(RECHECK)?),\n',
        '            Some(_checked) => first,\n',
        'idle_stretch_is_due_at_minutes_then_rechecks_until_the_cache_expires',
        'rust',
    ),
    # Plan 27 §2 (deadline loop): no check once the cache has expired
    Control(
        'p27_due_cache_life',
        'src/nirvana_idle_compact.rs',
        '        (at < since.checked_add(CACHE_LIFE)?).then_some(at)\n',
        '        Some(at)\n',
        'idle_stretch_is_due_at_minutes_then_rechecks_until_the_cache_expires',
        'rust',
    ),
    # Plan 27 §2 rule 5: output_tokens is not context
    Control(
        'p27_context_leaves_output',
        'src/nirvana_idle_compact.rs',
        '        "cache_creation_input_tokens",\n    ];\n',
        '        "cache_creation_input_tokens",\n        "output_tokens",\n    ];\n',
        'context_tokens_leave_out_output',
        'rust',
    ),
    # Plan 27 §2 rule 5: no usage field is no reading
    Control(
        'p27_context_none_without_fields',
        'src/nirvana_idle_compact.rs',
        '    (!read.is_empty()).then(|| read.iter().sum())\n',
        '    Some(read.iter().sum())\n',
        'context_tokens_leave_out_output',
        'rust',
    ),
    # Plan 27 §2 rule 5: only an assistant line holds a turn's context
    Control(
        'p27_context_assistant_only',
        'src/nirvana_idle_compact.rs',
        '        if value.get("type").and_then(Value::as_str) != Some("assistant") {\n',
        '        if false && value.get("type").and_then(Value::as_str) != Some("assistant") {\n',
        'last_context_tokens_reads_the_last_assistant_line',
        'rust',
    ),
    # Plan 27 §2 rule 5: the tail read widens past 64 KB
    Control(
        'p27_context_tail_widens',
        'src/nirvana_idle_compact.rs',
        '        if found.is_some() || window >= MAX_TAIL || len <= window {\n',
        '        if true {\n',
        'transcript_facts_come_from_a_temporary_file',
        'rust',
    ),
    # Plan 27 §2 rule 4: a file time ahead of the clock is no age
    Control(
        'p27_file_age_future',
        'src/nirvana_idle_compact.rs',
        '    now.duration_since(modified).ok()\n',
        '    Some(now.duration_since(modified).unwrap_or_default())\n',
        'transcript_facts_come_from_a_temporary_file',
        'rust',
    ),
    # Plan 27 §2: the command typed is /compact
    Control(
        'p27_command_is_compact',
        'src/nirvana_idle_compact.rs',
        'pub const COMMAND: &str = "/compact";\n',
        'pub const COMMAND: &str = "/clear";\n',
        'idle_compact_server_types_compact_once_into_an_idle_claude',
        'rust',
    ),
    # Plan 27 §3: on by default
    Control(
        'p27_config_default_on',
        'src/config/idle_compact.rs',
        '            enabled: true,\n            minutes: 50,\n',
        '            enabled: false,\n            minutes: 50,\n',
        'idle_compact_config_defaults_on_at_50_minutes_and_50k',
        'rust',
    ),
    # Plan 27 §3: 50 minutes by default
    Control(
        'p27_config_default_minutes',
        'src/config/idle_compact.rs',
        '            minutes: 50,\n            min_tokens: 50_000,\n',
        '            minutes: 40,\n            min_tokens: 50_000,\n',
        'idle_compact_config_defaults_on_at_50_minutes_and_50k',
        'rust',
    ),
    # Plan 27 §3: 50k tokens by default
    Control(
        'p27_config_default_tokens',
        'src/config/idle_compact.rs',
        '            minutes: 50,\n            min_tokens: 50_000,\n',
        '            minutes: 50,\n            min_tokens: 40_000,\n',
        'idle_compact_config_defaults_on_at_50_minutes_and_50k',
        'rust',
    ),
    # Plan 27 §3: minutes validated to 1..=59
    Control(
        'p27_config_minutes_range',
        'src/config/idle_compact.rs',
        '        if !(1..=MAX_MINUTES).contains(&self.minutes) {\n',
        '        if !(0..=600).contains(&self.minutes) {\n',
        'idle_compact_config_minutes_out_of_range_is_a_clear_error_and_sends_nothing',
        'rust',
    ),
    # Plan 27 §3: 59 is the last minute inside the cache's hour
    Control(
        'p27_config_max_minutes',
        'src/config/idle_compact.rs',
        'const MAX_MINUTES: i64 = 59;\n',
        'const MAX_MINUTES: i64 = 60;\n',
        'idle_compact_config_minutes_out_of_range_is_a_clear_error_and_sends_nothing',
        'rust',
    ),
    # Plan 27 §3: min_tokens validated to at least 0
    Control(
        'p27_config_tokens_negative',
        'src/config/idle_compact.rs',
        '        if self.min_tokens < 0 {\n',
        '        if self.min_tokens < -1 {\n',
        'idle_compact_config_negative_tokens_is_a_clear_error_and_sends_nothing',
        'rust',
    ),
    # Plan 27 §3: off gives no rule
    Control(
        'p27_config_off_no_rule',
        'src/config/idle_compact.rs',
        '        if !self.enabled || !self.diagnostics().is_empty() {\n',
        '        if !self.diagnostics().is_empty() {\n',
        'idle_compact_config_reads_its_section',
        'rust',
    ),
    # Plan 27 §3: a config error gives no rule (fail safe)
    Control(
        'p27_config_error_no_rule',
        'src/config/idle_compact.rs',
        '        if !self.enabled || !self.diagnostics().is_empty() {\n',
        '        if !self.enabled {\n',
        'idle_compact_config_minutes_out_of_range_is_a_clear_error_and_sends_nothing',
        'rust',
    ),
    # Plan 27 §3: a config error reaches the startup diagnostics
    Control(
        'p27_config_diagnostics_collected',
        'src/config.rs',
        '            .chain(self.ui.idle_compact.diagnostics())\n',
        '',
        'idle_compact_config_errors_reach_the_config_diagnostics',
        'rust',
    ),
    # Plan 27 §3: the settings edit writes [ui.idle_compact] enabled
    Control(
        'p27_config_edit_section',
        'src/config/write.rs',
        '                super::upsert_section_bool(content, "ui.idle_compact", "enabled", enabled)\n',
        '                super::upsert_section_bool(content, "ui.idle_compactx", "enabled", enabled)\n',
        'idle_compact_config_edit_writes_the_switch',
        'rust',
    ),
    # Plan 27 §3: the server starts with the config's setting
    Control(
        'p27_server_setting_from_config',
        'src/app/mod.rs',
        '            idle_compact: config.ui.idle_compact,\n',
        '            idle_compact: crate::config::IdleCompactConfig {\n                enabled: false,\n                ..config.ui.idle_compact\n            },\n',
        'idle_compact_server_types_compact_once_into_an_idle_claude',
        'rust',
    ),
    # Plan 27 §3: a changed setting applies without a restart
    Control(
        'p27_server_reload_applies',
        'src/app/mod.rs',
        '                self.idle_compact = config.ui.idle_compact;\n',
        '',
        'idle_compact_setting_applies_on_reload',
        'rust',
    ),
    # Plan 27 §3: a reload reports the setting's errors
    Control(
        'p27_server_reload_diagnostics',
        'src/app/mod.rs',
        '                diagnostics.extend(config.ui.idle_compact.diagnostics());\n',
        '',
        'idle_compact_setting_applies_on_reload',
        'rust',
    ),
    # Plan 27 §3: the settings sections include Idle compact
    Control(
        'p27_settings_section_listed',
        'src/client/shell/state.rs',
        '        Self::IdleCompact,\n        Self::Integrations,\n    ];\n',
        '        Self::Integrations,\n    ];\n',
        'idle_compact_section_shows_the_current_switch',
        'rust',
    ),
    # Plan 27 §3: the section's tab reads idle compact
    Control(
        'p27_settings_section_label',
        'src/client/shell/state.rs',
        '            Self::IdleCompact => "idle compact",\n',
        '            Self::IdleCompact => "idle",\n',
        'idle_compact_section_renders_its_tab_and_choices',
        'rust',
    ),
    # Plan 27 §3: the section opens on the current switch
    Control(
        'p27_settings_selected_current',
        'src/client/shell/settings.rs',
        '            ClientSettingsSection::IdleCompact => usize::from(!self.config.idle_compact_enabled),\n',
        '            ClientSettingsSection::IdleCompact => 0,\n',
        'idle_compact_section_shows_the_current_switch',
        'rust',
    ),
    # Plan 27 §3: two choices, On and Off
    Control(
        'p27_settings_two_choices',
        'src/client/shell/settings.rs',
        '                ClientSettingsSection::Indicators\n                | ClientSettingsSection::Sound\n                | ClientSettingsSection::IdleCompact => 2,\n',
        '                ClientSettingsSection::Indicators | ClientSettingsSection::Sound => 2,\n                ClientSettingsSection::IdleCompact => 3,\n',
        'idle_compact_section_shows_the_current_switch',
        'rust',
    ),
    # Plan 27 §3: choice 0 is on, 1 is off
    Control(
        'p27_settings_choice_edit',
        'src/client/shell/settings.rs',
        '    crate::config::ConfigEdit::IdleCompact(selected == 0)\n',
        '    crate::config::ConfigEdit::IdleCompact(selected != 0)\n',
        'idle_compact_choice_writes_on_or_off',
        'rust',
    ),
    # Plan 27 §3: the client reads the switch from the config
    Control(
        'p27_client_switch_from_config',
        'src/client/shell/config.rs',
        '            idle_compact_enabled: config.ui.idle_compact.enabled,\n',
        '            idle_compact_enabled: true,\n',
        'idle_compact_section_shows_the_current_switch',
        'rust',
    ),
    # Plan 27 §3: the client's switch follows a reload
    Control(
        'p27_client_switch_reload',
        'src/client/shell/config.rs',
        '                self.idle_compact_enabled = ui.idle_compact.enabled;\n',
        '',
        'idle_compact_switch_follows_a_config_reload',
        'rust',
    ),
    # Plan 27 §3: the section draws its description
    Control(
        'p27_settings_render_description',
        'src/client/shell/settings_overlay.rs',
        '                "send /compact to an idle Claude before its 1-hour cache expires",\n',
        '                "",\n',
        'idle_compact_section_renders_its_tab_and_choices',
        'rust',
    ),
    # Plan 27 §3: the section draws On and Off
    Control(
        'p27_settings_render_choices',
        'src/client/shell/settings_overlay.rs',
        '                "send /compact to an idle Claude before its 1-hour cache expires",\n                &["on", "off"],\n',
        '                "send /compact to an idle Claude before its 1-hour cache expires",\n                &["on"],\n',
        'idle_compact_section_renders_its_tab_and_choices',
        'rust',
    ),
    # Plan 27 §2 rule 3: the terminal's state recompute keeps the stretch
    Control(
        'p27_terminal_tracks_stretch',
        'src/terminal/state.rs',
        '        self.idle_stretch = self.idle_stretch.after(state, now);\n',
        '',
        'idle_stretch_follows_the_terminal_state',
        'rust',
    ),
    # Plan 27 §2 rule 3: a respawned pane has no stretch
    Control(
        'p27_terminal_respawn_clears_stretch',
        'src/terminal/state.rs',
        '        self.idle_stretch = crate::nirvana_idle_compact::IdleStretch::default();\n',
        '',
        'idle_stretch_follows_the_terminal_state',
        'rust',
    ),
    # Plan 27 §2 (deadline loop): the loop wakes for the idle check
    Control(
        'p27_loop_deadline',
        'src/app/runtime.rs',
        '            self.idle_compact_deadline(),\n',
        '',
        'idle_compact_deadline_reaches_the_server_loop',
        'rust',
    ),
    # Plan 27 §2 (deadline loop): the deadline walks the same Claude terminals as the check (review S2)
    Control(
        'p27_deadline_claude_only',
        'src/app/nirvana_idle_compact.rs',
        '        self.idle_compact_terminals()\n            .filter_map(|(_, terminal)| terminal.idle_stretch.next_check_at(rule.minutes))\n',
        '        self.state\n            .terminals\n            .iter()\n            .filter_map(|(_, terminal)| terminal.idle_stretch.next_check_at(rule.minutes))\n',
        'idle_compact_deadline_counts_only_claude',
        'rust',
    ),
    # Plan 27 §2 rule 1 (deadline loop): no deadline with the setting off
    Control(
        'p27_deadline_setting_off',
        'src/app/nirvana_idle_compact.rs',
        '        let rule = self.idle_compact.rule()?;\n',
        '        let rule = crate::config::IdleCompactConfig::default().rule()?;\n',
        'idle_compact_server_with_the_setting_off_sends_nothing',
        'rust',
    ),
    # Plan 27 §2 rule 1: the server sends nothing with the setting off
    Control(
        'p27_server_setting_off',
        'src/app/nirvana_idle_compact.rs',
        '        let Some(rule) = self.idle_compact.rule() else {\n            return false;\n        };\n',
        '        let Some(rule) = crate::config::IdleCompactConfig::default().rule() else {\n            return false;\n        };\n',
        'idle_compact_server_with_the_setting_off_sends_nothing',
        'rust',
    ),
    # Plan 27 §2: the server reads only Claude terminals; the stand-in records each read (review: hollow control)
    Control(
        'p27_server_claude_panes_only',
        'src/app/nirvana_idle_compact.rs',
        '            .filter(|(_, terminal)| terminal.effective_known_agent() == Some(Agent::Claude))\n',
        '            .filter(|(_, _terminal)| true)\n',
        'idle_compact_deadline_counts_only_claude',
        'rust',
    ),
    # Plan 27 §2: a pane is read only once due
    Control(
        'p27_server_due_only',
        'src/app/nirvana_idle_compact.rs',
        '            .filter(|(due, _)| *due <= now)\n',
        '            .filter(|(_due, _)| true)\n',
        'idle_compact_server_types_compact_once_into_an_idle_claude',
        'rust',
    ),
    # Plan 27 §2: a checked pane waits RECHECK
    Control(
        'p27_server_marks_checked',
        'src/app/nirvana_idle_compact.rs',
        '        terminal.idle_stretch.checked_at = Some(now);\n',
        '',
        'idle_compact_server_leaves_a_typed_line_alone_and_rechecks_later',
        'rust',
    ),
    # Plan 27 §2 rule 6: the input line comes from the pane's screen
    Control(
        'p27_server_prompt_from_screen',
        'src/app/nirvana_idle_compact.rs',
        '        let input_line = runtime.map_or_else(\n            || InputLine::Unsure("the pane has no runtime".into()),\n            input_line_of,\n        );\n',
        '        let input_line = InputLine::Empty;\n',
        'idle_compact_server_leaves_a_typed_line_alone_and_rechecks_later',
        'rust',
    ),
    # Plan 27 §2: the outside read is asked for this pane
    Control(
        'p27_server_query_pane',
        'src/app/nirvana_idle_compact.rs',
        '            pane_id: pane.public.clone(),\n',
        '            pane_id: String::new(),\n',
        'idle_compact_server_passes_the_facts_it_holds',
        'rust',
    ),
    # Plan 27 §2 rule 2: the server passes the process it read
    Control(
        'p27_server_argv_fact',
        'src/app/nirvana_idle_compact.rs',
        '            argv: outside.argv,\n',
        '            argv: Some(Vec::new()),\n',
        'idle_compact_server_sends_nothing_when_one_fact_fails',
        'rust',
    ),
    # Plan 27 §2 rule 4: the server passes the transcript it read
    Control(
        'p27_server_transcript_fact',
        'src/app/nirvana_idle_compact.rs',
        '            transcript_age: outside.transcript_age,\n',
        '            transcript_age: Some(Duration::MAX),\n',
        'idle_compact_server_sends_nothing_when_one_fact_fails',
        'rust',
    ),
    # Plan 27 §2 rule 5: the server passes the context it read
    Control(
        'p27_server_tokens_fact',
        'src/app/nirvana_idle_compact.rs',
        '            context_tokens: outside.context_tokens,\n',
        '            context_tokens: Some(u64::MAX),\n',
        'idle_compact_server_sends_nothing_when_one_fact_fails',
        'rust',
    ),
    # Plan 27 §2 rule 3: the server passes the pane's state, not its stretch (review: stale stretch)
    Control(
        'p27_server_state_fact',
        'src/app/nirvana_idle_compact.rs',
        '        let state = terminal.state;\n',
        '        let state = crate::detect::AgentState::Idle;\n',
        'idle_compact_server_reads_the_state_not_a_stale_stretch',
        'rust',
    ),
    # Plan 27 §2 rule 2: the server asks whether the pane is a seat worker's
    Control(
        'p27_server_seat_worker_fact',
        'src/app/nirvana_idle_compact.rs',
        '            seat_worker: crate::nirvana_subagents::is_seat_worker_pane(&pane.public),\n',
        '            seat_worker: false,\n',
        'idle_compact_server_sends_nothing_when_one_fact_fails',
        'rust',
    ),
    # Plan 27 §2 rule 7: the server marks the stretch sent
    Control(
        'p27_server_marks_sent',
        'src/app/nirvana_idle_compact.rs',
        '            terminal.idle_stretch.sent = true;\n',
        '',
        'idle_compact_server_types_compact_once_into_an_idle_claude',
        'rust',
    ),
    # ---- Plan 27 review round ----
    # Review: a stale stretch on a terminal whose state is no longer idle sends nothing, by rule 3 in decide
    Control(
        'p27_rule3_state_over_stretch',
        'src/nirvana_idle_compact.rs',
        '    if facts.state != AgentState::Idle {\n',
        '    if false && facts.state != AgentState::Idle {\n',
        'idle_compact_server_reads_the_state_not_a_stale_stretch',
        'rust',
    ),
    # Review B1: only faint text is a placeholder; a grey colour is the owner's text
    Control(
        'p27_input_faint_only',
        'src/nirvana_idle_compact.rs',
        '                Some("2") => {\n                    groups.nth(2);\n                }\n',
        '                Some("2") => {\n                    *faint = true;\n                    groups.nth(2);\n                }\n',
        'idle_compact_input_line_grey_typed_text_is_not_empty',
        'rust',
    ),
    # Review B1: an extended colour's arguments are skipped, so the 2 of 48;2 is not faint
    Control(
        'p27_input_sgr_colour_args',
        'src/nirvana_idle_compact.rs',
        '            38 | 48 | 58 => match groups.next() {\n',
        '            38 | 58 => match groups.next() {\n',
        'idle_compact_input_line_grey_typed_text_is_not_empty',
        'rust',
    ),
    # Review B1: a [ chip is not empty (a pasted-text chip)
    Control(
        'p27_input_chip_pasted',
        'src/nirvana_idle_compact.rs',
        "    if rest.iter().any(|cell| cell.ch == '[') {\n",
        "    if false && rest.iter().any(|cell| cell.ch == '[') {\n",
        'idle_compact_input_line_pasted_chip_is_not_empty',
        'rust',
    ),
    # Review B1: a [ chip is not empty (an image chip)
    Control(
        'p27_input_chip_image',
        'src/nirvana_idle_compact.rs',
        "    if rest.iter().any(|cell| cell.ch == '[') {\n",
        "    if false && rest.iter().any(|cell| cell.ch == '[') {\n",
        'idle_compact_input_line_image_chip_is_not_empty',
        'rust',
    ),
    # Review B1: any non-faint character is the owner's text
    Control(
        'p27_input_owned_text',
        'src/nirvana_idle_compact.rs',
        '    if !owned.is_empty() {\n',
        '    if false && !owned.is_empty() {\n',
        'idle_compact_input_line_recorded_typed_reads_its_text',
        'rust',
    ),
    # Review B1: a cursor drawn past the line's start is not empty
    Control(
        'p27_input_drawn_cursor',
        'src/nirvana_idle_compact.rs',
        '    if rest.iter().skip(1).any(|cell| cell.inverse) {\n',
        '    if false && rest.iter().skip(1).any(|cell| cell.inverse) {\n',
        'idle_compact_input_line_cursor_away_from_the_start_is_not_empty',
        'rust',
    ),
    # Review B1: the terminal's cursor anywhere but the line's start is not empty
    Control(
        'p27_input_cursor_at_start',
        'src/nirvana_idle_compact.rs',
        '        Some(cursor) if cursor.visible && (cursor.row != row || cursor.col != 2) => {\n',
        '        Some(cursor) if false && cursor.visible && (cursor.row != row || cursor.col != 2) => {\n',
        'idle_compact_input_line_cursor_away_from_the_start_is_not_empty',
        'rust',
    ),
    # Review B1: an unread cursor is a doubt
    Control(
        'p27_input_cursor_unread',
        'src/nirvana_idle_compact.rs',
        '        None => unsure("the cursor could not be read"),\n',
        '        None => InputLine::Empty,\n',
        'idle_compact_input_line_cursor_away_from_the_start_is_not_empty',
        'rust',
    ),
    # Review B1: a draft of more than one line is not empty
    Control(
        'p27_input_one_line',
        'src/nirvana_idle_compact.rs',
        '    if bottom != top + 2 {\n',
        '    if false && bottom != top + 2 {\n',
        'idle_compact_input_line_multiline_draft_is_not_empty',
        'rust',
    ),
    # Review B2: the real input box or nothing; no fallback to a ❯ line elsewhere
    Control(
        'p27_input_no_fallback',
        'src/nirvana_idle_compact.rs',
        '    let [.., top, bottom] = rules[..] else {\n        return unsure("no input box on the screen");\n    };\n',
        '    let (top, bottom) = match rules[..] {\n        [.., top, bottom] => (top, bottom),\n        _ => {\n            let Some(at) = plain.iter().rposition(|line| line.starts_with(\'❯\')) else {\n                return unsure("no input box on the screen");\n            };\n            (at.saturating_sub(1), at + 1)\n        }\n    };\n',
        'idle_compact_input_line_needs_the_box',
        'rust',
    ),
    # Review B2: the box's line starts with ❯
    Control(
        'p27_input_marked',
        'src/nirvana_idle_compact.rs',
        '    if !marked {\n',
        '    if false && !marked {\n',
        'idle_compact_input_line_needs_the_box',
        'rust',
    ),
    # Review B2: a box with more lines below it than a footer is not the live bottom
    Control(
        'p27_input_footer_bound',
        'src/nirvana_idle_compact.rs',
        '    if below > FOOTER_LINES {\n',
        '    if false && below > FOOTER_LINES {\n',
        'idle_compact_input_line_scrolled_screen_is_not_empty',
        'rust',
    ),
    # Review B2: a pane scrolled back is not read
    Control(
        'p27_input_scrolled',
        'src/nirvana_idle_compact.rs',
        '    if scrolled {\n        return unsure("the pane is scrolled back");\n    }\n',
        '',
        'idle_compact_input_line_scrolled_screen_is_not_empty',
        'rust',
    ),
    # Review B2: a screen with a claude.toml blocked or transcript-viewer marker, or vim mode, is not read
    Control(
        'p27_input_markers',
        'src/nirvana_idle_compact.rs',
        '    if let Some(marker) = NOT_THE_INPUT.iter().find(|marker| lower.contains(**marker)) {\n',
        '    if let Some(marker) = NOT_THE_INPUT.iter().find(|_marker| false) {\n',
        'idle_compact_input_line_vim_and_dialog_markers_are_not_empty',
        'rust',
    ),
    # Review B2: a menu screen is not read
    Control(
        'p27_input_markers_menu',
        'src/nirvana_idle_compact.rs',
        '    if let Some(marker) = NOT_THE_INPUT.iter().find(|marker| lower.contains(**marker)) {\n',
        '    if let Some(marker) = NOT_THE_INPUT.iter().find(|_marker| false) {\n',
        'idle_compact_input_line_menu_is_not_empty',
        'rust',
    ),
    # Review B2: vim mode's insert footer is one of the markers
    Control(
        'p27_input_vim_marker',
        'src/nirvana_idle_compact.rs',
        '    "-- normal --",\n    "-- insert --",\n];\n',
        '    "-- normal --",\n];\n',
        'idle_compact_input_line_vim_and_dialog_markers_are_not_empty',
        'rust',
    ),
    # Review S1: the owner's key inside the quiet window skips
    Control(
        'p27_rule6_owner_quiet',
        'src/nirvana_idle_compact.rs',
        '    if let Some(why) = owner_quiet_breach(facts.owner_key_secs_ago) {\n        return skip(why);\n    }\n',
        '',
        'idle_compact_rule6_owner_keys_inside_the_quiet_window_skip',
        'rust',
    ),
    # Review S1: the quiet window is OWNER_QUIET_S
    Control(
        'p27_owner_quiet_window',
        'src/nirvana_idle_compact.rs',
        '    (ago < crate::nirvana_run::OWNER_QUIET_S)\n',
        '    (ago < 1)\n',
        'idle_compact_rule6_owner_keys_inside_the_quiet_window_skip',
        'rust',
    ),
    # Review S1: Enter only on exactly /compact
    Control(
        'p27_enter_exact',
        'src/nirvana_idle_compact.rs',
        '        InputLine::Text(text) if text == COMMAND => Ok(()),\n',
        '        InputLine::Text(text) if text.starts_with(COMMAND) => Ok(()),\n',
        'idle_compact_enter_only_on_exactly_compact',
        'rust',
    ),
    # Review S1: no Enter while the owner types
    Control(
        'p27_enter_owner',
        'src/nirvana_idle_compact.rs',
        '    if let Some(why) = owner_quiet_breach(owner_key_secs_ago) {\n        return Err(why);\n    }\n',
        '',
        'idle_compact_enter_only_on_exactly_compact',
        'rust',
    ),
    # Review S1: a pasted stretch is due its Enter
    Control(
        'p27_stretch_enter_due',
        'src/nirvana_idle_compact.rs',
        '        if let Some(enter_at) = self.enter_at {\n            return Some(enter_at);\n        }\n',
        '',
        'idle_stretch_is_due_its_enter_after_the_paste',
        'rust',
    ),
    # Review S1: the server reads the owner's last key for the pane
    Control(
        'p27_server_owner_key_fact',
        'src/app/nirvana_idle_compact.rs',
        '            owner_key_secs_ago: owner_key_secs_ago(&pane.public),\n',
        '            owner_key_secs_ago: None,\n',
        'idle_compact_server_skips_while_the_owner_types',
        'rust',
    ),
    # Review S1: the key's age is now less its second, as the wake path reads it
    Control(
        'p27_server_owner_key_age',
        'src/app/nirvana_idle_compact.rs',
        '    crate::nirvana_run::owner_key_at(public).map(|at| crate::nirvana_run::now().saturating_sub(at))\n',
        '    crate::nirvana_run::owner_key_at(public).map(|_at| u64::MAX)\n',
        'idle_compact_server_skips_while_the_owner_types',
        'rust',
    ),
    # Review S1: the Enter re-reads the line
    Control(
        'p27_server_enter_rereads',
        'src/app/nirvana_idle_compact.rs',
        '        if let Err(why) = enter_gate(&input_line_of(runtime), owner_key_secs_ago(&public)) {\n',
        '        if let Err(why) = enter_gate(&InputLine::Text("/compact".into()), owner_key_secs_ago(&public)) {\n',
        'idle_compact_server_sends_no_enter_unless_the_line_reads_compact',
        'rust',
    ),
    # Review S1: the Enter reads the owner's key again
    Control(
        'p27_server_enter_owner',
        'src/app/nirvana_idle_compact.rs',
        '        if let Err(why) = enter_gate(&input_line_of(runtime), owner_key_secs_ago(&public)) {\n',
        '        if let Err(why) = enter_gate(&input_line_of(runtime), None) {\n',
        'idle_compact_server_holds_the_enter_when_the_owner_types',
        'rust',
    ),
    # Review S1: a pending Enter is taken on its pass
    Control(
        'p27_server_enter_pass',
        'src/app/nirvana_idle_compact.rs',
        '        if terminal.idle_stretch.enter_at.take().is_some() {\n',
        '        if terminal.idle_stretch.enter_at.take().is_some() && false {\n',
        'idle_compact_server_types_compact_once_into_an_idle_claude',
        'rust',
    ),
    # Review S1: the paste sets the Enter's time
    Control(
        'p27_server_enter_at_set',
        'src/app/nirvana_idle_compact.rs',
        '            terminal.idle_stretch.enter_at = now.checked_add(SUBMIT_DELAY);\n',
        '',
        'idle_compact_server_types_compact_once_into_an_idle_claude',
        'rust',
    ),
    # Review S1: the Enter's reply is waited on, and a failure is known
    Control(
        'p27_server_enter_reply',
        'src/app/nirvana_idle_compact.rs',
        '            .and_then(|reply| {\n                reply\n                    .recv_timeout(ENTER_REPLY_WAIT)\n                    .map_err(std::io::Error::other)?\n            });\n',
        '            .map(|_reply| ());\n',
        'idle_compact_server_reports_a_failed_enter',
        'rust',
    ),
    # Review S6: the stretch is marked sent only once the paste is queued
    Control(
        'p27_server_sent_after_queue',
        'src/app/nirvana_idle_compact.rs',
        '        if let Err(err) = runtime.try_send_bytes(Bytes::from(text)) {\n',
        '        if let Some(terminal) = self.state.terminals.get_mut(terminal_id) {\n            terminal.idle_stretch.sent = true;\n        }\n        if let Err(err) = runtime.try_send_bytes(Bytes::from(text)) {\n',
        'idle_compact_server_unqueued_paste_stays_unsent',
        'rust',
    ),
    # Review S2: a due terminal with no pane is stamped too
    Control(
        'p27_server_stamps_paneless',
        'src/app/nirvana_idle_compact.rs',
        '        terminal.idle_stretch.checked_at = Some(now);\n',
        '',
        'idle_compact_terminal_without_a_pane_gets_a_future_deadline',
        'rust',
    ),
    # Review S2: the other panes for the shared-folder rule are Claude panes only
    Control(
        'p27_others_claude_only',
        'src/app/nirvana_idle_compact.rs',
        '                    if !claude.contains(&pane.attached_terminal_id) {\n',
        '                    if false && !claude.contains(&pane.attached_terminal_id) {\n',
        'idle_compact_others_are_only_claude_panes',
        'rust',
    ),
    # Review S4: the due terminal checked first is the earliest; the rest wait for the next pass
    Control(
        'p27_server_earliest_first',
        'src/app/nirvana_idle_compact.rs',
        '            .min_by_key(|(due, _)| *due)\n',
        '            .max_by_key(|(due, _)| *due)\n',
        'idle_compact_checks_one_terminal_a_pass',
        'rust',
    ),
    # Review S4: the transcript read stops at 1 MB
    Control(
        'p27_tail_bound',
        'src/nirvana_idle_compact.rs',
        'pub const MAX_TAIL: u64 = 1024 * 1024;\n',
        'pub const MAX_TAIL: u64 = 8 * 1024 * 1024;\n',
        'transcript_read_stops_at_one_megabyte',
        'rust',
    ),
    # Review S3: the pane's foreground must be Claude
    Control(
        'p27_transcript_foreground',
        'src/nirvana_idle_compact.rs',
        '    if foreground != Some(Agent::Claude) {\n        return Err("the pane\'s foreground is not Claude".into());\n    }\n',
        '',
        'choose_transcript_needs_claude_in_the_foreground',
        'rust',
    ),
    # Review S3: the sessions record beats the hook's id
    Control(
        'p27_transcript_record_first',
        'src/nirvana_idle_compact.rs',
        '        (Some(id), _) => (id, false),\n',
        '        (_, Some(id)) => (id, true),\n        (Some(id), _) => (id, false),\n',
        'choose_transcript_takes_the_sessions_record_over_the_hook',
        'rust',
    ),
    # Review S3: a sessions record older than the process is not this Claude's
    Control(
        'p27_transcript_record_tied',
        'src/nirvana_idle_compact.rs',
        '        .filter(|record| record.written_secs.is_some_and(|at| at >= start))\n',
        '        .filter(|_record| true)\n',
        'choose_transcript_takes_the_sessions_record_over_the_hook',
        'rust',
    ),
    # Review S3: no id, no transcript; never the folder's newest
    Control(
        'p27_transcript_no_id',
        'src/nirvana_idle_compact.rs',
        '        (None, None) => return Err("no session id names this Claude\'s conversation".into()),\n',
        '        (None, None) => return Ok(claude_dir.join("projects").join(&encoded).join("newest.jsonl")),\n',
        'choose_transcript_with_no_id_takes_nothing',
        'rust',
    ),
    # Review S3: without the process start time nothing is tied
    Control(
        'p27_transcript_start_known',
        'src/nirvana_idle_compact.rs',
        '    let Some(start) = start_secs else {\n        return Err("the Claude process\'s start time is not known".into());\n    };\n',
        '    let start = start_secs.unwrap_or(0);\n',
        'choose_transcript_with_no_id_takes_nothing',
        'rust',
    ),
    # Review S3: a hook id's transcript written before this Claude started is not its own
    Control(
        'p27_transcript_hook_tied',
        'src/nirvana_idle_compact.rs',
        '        Some(at) if from_hook && at < start => Err(format!(\n',
        '        Some(at) if false && from_hook && at < start => Err(format!(\n',
        'choose_transcript_with_no_id_takes_nothing',
        'rust',
    ),
    # Review S3: an id with no transcript gives none
    Control(
        'p27_transcript_exists',
        'src/nirvana_idle_compact.rs',
        '        None => Err(format!("no transcript for session {id}")),\n',
        '        None => Ok(path),\n',
        'choose_transcript_with_no_id_takes_nothing',
        'rust',
    ),
    # Review S3: a session value that is not an id gives none
    Control(
        'p27_transcript_id_valid',
        'src/nirvana_idle_compact.rs',
        '    if !valid {\n',
        '    if false && !valid {\n',
        'choose_transcript_with_no_id_takes_nothing',
        'rust',
    ),
    # Review S3: two panes in one folder that nothing tells apart
    Control(
        'p27_transcript_shared_folder',
        'src/nirvana_idle_compact.rs',
        '            && other_id.is_none_or(|other| other == id)\n',
        '            && other_id.is_some_and(|other| other == id)\n',
        'choose_transcript_two_panes_in_one_folder',
        'rust',
    ),
    # Review S3: only a pane in the same folder counts
    Control(
        'p27_transcript_same_folder_only',
        'src/nirvana_idle_compact.rs',
        '        crate::nirvana_subagents::encode_project_folder(other_folder) == encoded\n',
        '        true\n',
        'choose_transcript_two_panes_in_one_folder',
        'rust',
    ),
)
