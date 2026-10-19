# Plan 18: the Antigravity account stays the one the owner picked (plans/2026-10-19-18-account-guard.md).
# Red line for each: its test prints FAIL.
CONTROLS = (
    # §2: Chosen account recorded when ending signed in.
    Control(
        'p18_record_written_when_signed_in',
        'src/nirvana_accounts.rs',
        "        if let Signed::In(ref email) = after {\n            write_chosen(home, cli, email)?;\n            return Ok(true);\n        }\n",
        "        if false {\n            write_chosen(home, cli, \"\")?;\n            return Ok(true);\n        }\n",
        'the_chosen_antigravity_account_records_on_signed_in_login_and_not_signed_out',
        'rust',
    ),
    # §2: Chosen account not recorded when ending signed out.
    Control(
        'p18_record_not_written_when_signed_out',
        'src/nirvana_accounts.rs',
        "        if let Signed::In(ref email) = after {\n            write_chosen(home, cli, email)?;\n            return Ok(true);\n        }\n",
        "        write_chosen(home, cli, \"always@example.com\")?;\n        return Ok(true);\n",
        'the_chosen_antigravity_account_records_on_signed_in_login_and_not_signed_out',
        'rust',
    ),
    # §2: Only Antigravity writes accounts.json.
    Control(
        'p18_record_antigravity_only',
        'src/nirvana_accounts.rs',
        "pub fn record_after_login(home: &Path, cli: Cli, after: &Signed) -> std::io::Result<bool> {\n    if cli == Cli::Antigravity {\n",
        "pub fn record_after_login(home: &Path, cli: Cli, after: &Signed) -> std::io::Result<bool> {\n    if false && cli == Cli::Antigravity {\n",
        'the_chosen_antigravity_account_records_on_signed_in_login_and_not_signed_out',
        'rust',
    ),
    # §2: Atomic write via temp file and rename.
    Control(
        'p18_record_temp_file_rename',
        'src/nirvana_accounts.rs',
        "    std::fs::rename(&temp, &path).inspect_err(|_| {\n        let _ = std::fs::remove_file(&temp);\n    })?;\n",
        "    // std::fs::rename(&temp, &path).inspect_err(|_| {\n    //     let _ = std::fs::remove_file(&temp);\n    // })?;\n",
        'the_chosen_antigravity_account_records_on_signed_in_login_and_not_signed_out',
        'rust',
    ),
    # §3: Matching chosen and live account produces no drift.
    Control(
        'p18_drift_ruling_matching',
        'src/nirvana_accounts.rs',
        "    if chosen != current {\n        Some(Drift {\n            chosen: chosen.to_owned(),\n            current: current.to_owned(),\n            app_running,\n        })\n    } else {\n        None\n    }\n",
        "    if true {\n        Some(Drift {\n            chosen: chosen.to_owned(),\n            current: current.to_owned(),\n            app_running,\n        })\n    } else {\n        None\n    }\n",
        'drift_ruling_matching_different_no_choice_no_log',
        'rust',
    ),
    # §3: Different chosen and live account produces drift ruling.
    Control(
        'p18_drift_ruling_different',
        'src/nirvana_accounts.rs',
        "    if chosen != current {\n        Some(Drift {\n            chosen: chosen.to_owned(),\n            current: current.to_owned(),\n            app_running,\n        })\n    } else {\n        None\n    }\n",
        "    if false && chosen != current {\n        Some(Drift {\n            chosen: chosen.to_owned(),\n            current: current.to_owned(),\n            app_running,\n        })\n    } else {\n        None\n    }\n",
        'drift_ruling_matching_different_no_choice_no_log',
        'rust',
    ),
    # §3: No choice recorded produces no drift.
    Control(
        'p18_drift_ruling_no_choice',
        'src/nirvana_accounts.rs',
        "    let chosen = chosen?.trim();\n    if chosen.is_empty() {\n        return None;\n    }\n",
        "    let chosen = chosen.unwrap_or(\"fallback@example.com\").trim();\n",
        'drift_ruling_matching_different_no_choice_no_log',
        'rust',
    ),
    # §3: No log found produces no drift.
    Control(
        'p18_drift_ruling_no_log',
        'src/nirvana_accounts.rs',
        "    let current = current?.trim();\n    if current.is_empty() {\n        return None;\n    }\n",
        "    let current = current.unwrap_or(\"fallback@example.com\").trim();\n",
        'drift_ruling_matching_different_no_choice_no_log',
        'rust',
    ),
    # §3: Cause line while Antigravity.exe runs.
    Control(
        'p18_cause_line_app_running',
        'src/nirvana_accounts.rs',
        "pub fn cause_line(chosen: &str, app_running: bool) -> String {\n    if app_running {\n        format!(\"The Antigravity app put this account back. Sign the app in as {chosen} too.\")\n    } else {\n",
        "pub fn cause_line(chosen: &str, app_running: bool) -> String {\n    if false && app_running {\n        format!(\"The Antigravity app put this account back. Sign the app in as {chosen} too.\")\n    } else {\n",
        'cause_line_with_and_without_app_process',
        'rust',
    ),
    # §3: Cause line when Antigravity.exe is not running.
    Control(
        'p18_cause_line_app_not_running',
        'src/nirvana_accounts.rs',
        "    } else {\n        \"agy went back to this account. Press switch to sign in again.\".to_string()\n    }\n",
        "    } else {\n        \"wrong cause line\".to_string()\n    }\n",
        'cause_line_with_and_without_app_process',
        'rust',
    ),
    # §3: store_drift updates cache and marks take_changed.
    Control(
        'p18_drift_take_changed',
        'src/nirvana_accounts.rs',
        "pub fn store_drift(ruling: Option<Drift>) {\n    let mut lock = DRIFT.lock().unwrap_or_else(|e| e.into_inner());\n    if *lock != ruling {\n        *lock = ruling;\n        CHANGED.store(true, Ordering::Release);\n    }\n}\n",
        "pub fn store_drift(ruling: Option<Drift>) {\n    let mut lock = DRIFT.lock().unwrap_or_else(|e| e.into_inner());\n    if *lock != ruling {\n        *lock = ruling;\n        // CHANGED.store(true, Ordering::Release);\n    }\n}\n",
        'store_drift_updates_cache_and_sets_take_changed',
        'rust',
    ),
    # §3: Wide row format shows "<current> ⚠ chosen <chosen>".
    Control(
        'p18_row_text_wide_format',
        'src/client/shell/nirvana_accounts.rs',
        "    let full = format!(\"{current} \\u{26A0} chosen {}\", drift.chosen);\n",
        "    let full = format!(\"{current} - chosen {}\", drift.chosen);\n",
        'accounts_panel_row_and_cause_line_with_app_running_wide',
        'rust',
    ),
    # §3: Narrow room drops the word "chosen".
    Control(
        'p18_row_text_narrow_drops_chosen_word',
        'src/client/shell/nirvana_accounts.rs',
        "    let without_chosen_word = format!(\"{current} \\u{26A0} {}\", drift.chosen);\n",
        "    let without_chosen_word = format!(\"{current} \\u{26A0} chosen {}\", drift.chosen);\n",
        'accounts_row_narrow_drops_chosen_word_then_drops_email_keeping_warning_mark',
        'rust',
    ),
    # §3: Narrow room drops chosen email, keeping warning mark.
    Control(
        'p18_row_text_narrow_drops_chosen_email',
        'src/client/shell/nirvana_accounts.rs',
        "    let mark_only = format!(\"{current} \\u{26A0}\");\n",
        "    let mark_only = format!(\"{current} \\u{26A0} {}\", drift.chosen);\n",
        'accounts_row_narrow_drops_chosen_word_then_drops_email_keeping_warning_mark',
        'rust',
    ),
    # §3: Drifted account row is colored palette.yellow.
    Control(
        'p18_row_warning_color',
        'src/client/shell/nirvana_accounts.rs',
        "                put_text(\n                    buffer,\n                    text_x,\n                    y,\n                    room as u16,\n                    &text,\n                    style.fg(palette.yellow),\n                );\n",
        "                put_text(\n                    buffer,\n                    text_x,\n                    y,\n                    room as u16,\n                    &text,\n                    style.fg(palette.text),\n                );\n",
        'accounts_panel_row_and_cause_line_with_app_running_wide',
        'rust',
    ),
    # §3: Cause line is drawn under Antigravity row.
    Control(
        'p18_cause_line_drawn_under_row',
        'src/client/shell/nirvana_accounts.rs',
        "        if account.cli == Cli::Antigravity {\n            if let Some(drift) = &drift {\n                let cause = drift.cause_line();\n                put_text(buffer, x, y, inner as u16, &fit(&cause, inner), muted);\n                y += 1;\n            }\n        }\n",
        "        if false && account.cli == Cli::Antigravity {\n            if let Some(drift) = &drift {\n                let cause = drift.cause_line();\n                put_text(buffer, x, y, inner as u16, &fit(&cause, inner), muted);\n                y += 1;\n            }\n        }\n",
        'accounts_panel_row_and_cause_line_with_app_running_wide',
        'rust',
    ),
    # §3: Foot accounts button gains " ⚠" on drift.
    Control(
        'p18_foot_mark_warning_symbol',
        'src/client/shell/nirvana_footer.rs',
        "    let drift = crate::nirvana_accounts::antigravity_drift();\n    if drift.is_some() && area.width >= button_width + 3 {\n        put_text(\n            buffer,\n            area.x + 1 + button_width,\n            button_y,\n            2,\n            \" \\u{26A0}\",\n            Style::default().fg(palette.yellow),\n        );\n    }\n",
        "    let drift = crate::nirvana_accounts::antigravity_drift();\n    if false && drift.is_some() && area.width >= button_width + 3 {\n        put_text(\n            buffer,\n            area.x + 1 + button_width,\n            button_y,\n            2,\n            \" \\u{26A0}\",\n            Style::default().fg(palette.yellow),\n        );\n    }\n",
        'accounts_footer_button_shows_warning_mark_on_drift_and_hides_when_matched',
        'rust',
    ),
    # §3: Foot accounts warning mark is colored palette.yellow.
    Control(
        'p18_foot_mark_warning_color',
        'src/client/shell/nirvana_footer.rs',
        "        put_text(\n            buffer,\n            area.x + 1 + button_width,\n            button_y,\n            2,\n            \" \\u{26A0}\",\n            Style::default().fg(palette.yellow),\n        );\n",
        "        put_text(\n            buffer,\n            area.x + 1 + button_width,\n            button_y,\n            2,\n            \" \\u{26A0}\",\n            Style::default().fg(palette.text),\n        );\n",
        'accounts_footer_button_shows_warning_mark_on_drift_and_hides_when_matched',
        'rust',
    ),
)
