# Plan 13 P2: the Preload panel's preset, deferred rows and memory row.
# One sabotage per behaviour the build's new tests assert; each test named must print FAIL
# with its sabotage in place. `Control` is provided by controls.py.

MODS = "src/client/shell/nirvana_mods.rs"
PRELOAD = "src/nirvana_preload.rs"

CONTROLS = (
    # 1. The header line with the preset, "off", and "never opened".
    Control(
        "p13b_header_active_preset",
        MODS,
        '"Preset {} \\u{b7} {} off \\u{b7} ~{} a turn (estimate) \\u{b7} p undo",',
        '"Preset {} \\u{b7} {} off \\u{b7} ~{} a turn (approx) \\u{b7} p undo",',
        "the_header_line_shows_active_preset_and_deferred_tokens",
        "rust",
    ),
    Control(
        "p13b_header_preset_off",
        MODS,
        '"Preset off \\u{b7} p on".into()',
        '"Preset off".into()',
        "the_header_line_shows_undo_preset_and_never_opened_reason",
        "rust",
    ),
    Control(
        "p13b_header_never_opened",
        MODS,
        "if preset.name.is_empty() {\n        preset.reason.clone()",
        "if preset.name.is_empty() {\n        String::new()",
        "the_header_line_shows_undo_preset_and_never_opened_reason",
        "rust",
    ),
    Control(
        "p13b_header_deferred_tokens",
        MODS,
        '"{} a turn \\u{b7} {} deferred",\n                preload::short(report.total),\n                preload::short(report.deferred())',
        '"{} a turn \\u{b7} {} def",\n                preload::short(report.total),\n                preload::short(report.deferred())',
        "the_header_line_shows_active_preset_and_deferred_tokens",
        "rust",
    ),

    # 2. Deferred rows and used-only sums.
    Control(
        "p13b_deferred_row_detail",
        PRELOAD,
        'detail = format!("{detail} \\u{b7} deferred {}", short(deferred_tokens));',
        'detail = format!("{detail} \\u{b7} def {}", short(deferred_tokens));',
        "the_panel_lists_each_group_and_item_with_its_tokens",
        "rust",
    ),
    Control(
        "p13b_provider_sum_used_only",
        PRELOAD,
        ".filter(|item| &item.provider == provider && item.group != Group::Plugins && item.on)",
        ".filter(|item| &item.provider == provider && item.group != Group::Plugins)",
        "preset_off_rows_are_marked_off_and_pinned_rows_are_marked_on",
        "rust",
    ),
    Control(
        "p13b_mcp_server_share",
        PRELOAD,
        "let mcp_share = mcp_server_share(report);",
        "let mcp_share = 0;",
        "the_panel_lists_each_group_and_item_with_its_tokens",
        "rust",
    ),

    # 3. [p] rows and pinning.
    Control(
        "p13b_preset_off_row_mark",
        MODS,
        '(Some(_), true, _) => ("[p]", row_style.fg(palette.yellow)),',
        '(Some(_), true, _) => ("[ ]", row_style.fg(palette.yellow)),',
        "preset_off_rows_render_p_mark_and_tokens_in_wide_and_narrow",
        "rust",
    ),
    Control(
        "p13b_space_pins_preset_off",
        MODS,
        "match crate::nirvana_preset::pin_on(&cwd, &switch, true) {",
        "match crate::nirvana_preset::pin_on(&cwd, &switch, false) {",
        "space_pins_preset_off_row_and_unpins_pinned_row",
        "rust",
    ),
    Control(
        "p13b_space_unpins_pinned",
        MODS,
        "match crate::nirvana_preset::pin_on(&cwd, &switch, false) {",
        "match crate::nirvana_preset::pin_on(&cwd, &switch, true) {",
        "space_pins_preset_off_row_and_unpins_pinned_row",
        "rust",
    ),

    # 4. The p key.
    Control(
        "p13b_p_key_toggle",
        MODS,
        "match crate::nirvana_preset::set_preset(&cwd, !is_on) {",
        "match crate::nirvana_preset::set_preset(&cwd, is_on) {",
        "p_key_toggles_preset_on_and_off_and_updates_header",
        "rust",
    ),

    # 5. The memory row's three states and its keys.
    Control(
        "p13b_memory_row_over_limit",
        PRELOAD,
        'return format!("over {} \\u{b7} t trim", trim_saved_short(trim.limit));',
        'return format!("over {}", trim_saved_short(trim.limit));',
        "auto_memory_row_shows_over_limit_proposal_and_error",
        "rust",
    ),
    Control(
        "p13b_memory_row_proposal",
        PRELOAD,
        'return format!(\n                "\\u{2212}{} \\u{b7} a accept \\u{b7} x discard",\n                trim_saved_short(proposal.saved)\n            );',
        'return format!("saved {}", trim_saved_short(proposal.saved));',
        "auto_memory_row_shows_over_limit_proposal_and_error",
        "rust",
    ),
    Control(
        "p13b_memory_row_error",
        PRELOAD,
        "if let Some(err) = trim_error {\n        return err.to_owned();\n    }",
        'if let Some(_err) = trim_error {\n        return "error".into();\n    }',
        "auto_memory_row_shows_over_limit_proposal_and_error",
        "rust",
    ),
    Control(
        "p13b_memory_key_t",
        MODS,
        "preload::start_trim_propose(&panel.cwd);",
        "let _ = &panel.cwd;",
        "memory_row_shows_over_limit_proposal_and_responds_to_t_a_x_keys",
        "rust",
    ),
    Control(
        "p13b_memory_key_a",
        MODS,
        "preload::trim_accept(&panel.cwd);",
        "let _ = &panel.cwd;",
        "memory_row_shows_over_limit_proposal_and_responds_to_t_a_x_keys",
        "rust",
    ),
    Control(
        "p13b_memory_key_x",
        MODS,
        "preload::trim_discard(&panel.cwd);",
        "let _ = &panel.cwd;",
        "memory_row_shows_over_limit_proposal_and_responds_to_t_a_x_keys",
        "rust",
    ),

    # 6. No blocking on the draw path (cache view & refresh off draw path).
    Control(
        "p13b_draw_cache_propose_trim",
        PRELOAD,
        "view.trim = Some(s.clone());\n                        view.trim_error = None;",
        "view.trim = None;\n                        view.trim_error = None;",
        "memory_row_shows_over_limit_proposal_and_responds_to_t_a_x_keys",
        "rust",
    ),
    Control(
        "p13b_draw_cache_refresh_preset",
        PRELOAD,
        "view.preset = preset.clone();",
        "let _ = &preset;",
        "p_key_toggles_preset_on_and_off_and_updates_header",
        "rust",
    ),
)
