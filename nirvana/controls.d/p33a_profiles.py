# Plan 33 P33a: agent profiles per project (package `p33a_profiles`, nirvana/plans/2026-10-30-33-records/p33a.md).
# Controls starting with p33a_. One sabotage per assertion, each on a product line; the test named must print FAIL
# with it in place. `Control` is provided by controls.py.
PROFILES = "src/nirvana_profiles.rs"
SEAT = "src/nirvana_seat.rs"
WORKER = "src/nirvana_worker.rs"
SHELL = "src/client/shell/nirvana_profiles.rs"
MODS = "src/client/shell/nirvana_mods.rs"

T_PARSE = "p33_the_parser_refuses_each_bad_profile_naming_the_file_and_line"
T_PARSE_OK = "p33_a_profile_parses_every_key_and_its_body"
T_NARROW = "p33_the_profile_narrows_the_seats_and_its_effort_replaces_the_class_effort"
T_NOFIT = "p33_no_seat_fits_the_profile_and_the_pick_names_it"
T_PICK = "p33_seat_pick_takes_the_profiles_class_and_joins_its_needs"
T_COPY = "p33_seat_run_copies_the_profile_with_its_sha_and_an_edit_after_seating_never_reaches_the_worker"
T_CLAUDE = "p33_the_claude_worker_gets_the_disallowed_tools_and_a_changed_copy_never_starts"
T_AGY = "p33_the_agy_worker_gets_the_body_quoted_at_the_top_of_its_order"
T_SEND_BACK = "p33_a_send_back_child_keeps_the_parents_profile_copy_on_either_cli"
T_RESEAT = "p33_a_reseat_keeps_the_profile_copy_and_its_narrowing"
T_NAMED = "p33_an_order_naming_a_profile_is_seated_only_with_that_profile"
T_DIGEST = "p33_the_digest_lists_each_profile_in_at_most_ten_lines"
T_NEW = "p33_new_writes_a_template_naming_the_projects_servers_and_check_reads_the_table"
T_GROUP = "p33_the_profiles_group_lists_each_profile_with_its_file_and_a_refused_one_says_so"
T_PANEL = "p33_the_panel_lists_the_profiles_last_and_n_makes_one_and_opens_it"
T_NAMING = "p33_n_takes_only_a_name_new_accepts_and_enter_makes_it"

CONTROLS = (
    # §2.1: the parser's refusals. Unknown key with its line, a Send key, a long description, a long body, a bad name,
    # a Send in the body, a missing description.
    Control("p33a_unknown_key_no_line", PROFILES,
            '"{shown}: line {line_no}: unknown key `{key}` (the keys: {})",',
            '"{shown}: unknown key `{key}` (the keys: {})",', T_PARSE, "rust"),
    Control("p33a_send_key_only_exact", PROFILES,
            'if key.to_ascii_lowercase().contains("send") {', 'if key == "send_ok" {', T_PARSE, "rust"),
    Control("p33a_description_one_over", PROFILES,
            "} else if count > DESCRIPTION_MAX {", "} else if count > DESCRIPTION_MAX + 1 {", T_PARSE, "rust"),
    Control("p33a_body_one_over", PROFILES,
            "    if count > BODY_MAX {", "    if count > BODY_MAX + 1 {", T_PARSE, "rust"),
    Control("p33a_name_any_length", PROFILES,
            "(1..=NAME_MAX).contains(&name.len())", "(0..=NAME_MAX + 1).contains(&name.len())", T_PARSE, "rust"),
    Control("p33a_body_send_flag_allowed", PROFILES,
            'if body.contains("--send-ok") || body.contains(crate::nirvana_gui_tools::SEND_LINE) {',
            "if body.contains(crate::nirvana_gui_tools::SEND_LINE) {", T_PARSE, "rust"),
    Control("p33a_description_optional", PROFILES,
            "    if !described {", "    if false && !described {", T_PARSE, "rust"),
    Control("p33a_quotes_kept", PROFILES,
            "if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {",
            "if false && value.starts_with(quote) && value.ends_with(quote) {", T_PARSE_OK, "rust"),
    # §2.2: model and family narrow the seats; effort replaces the class's; no fit names the profile.
    Control("p33a_narrow_skips_nothing", SEAT,
            "        cand.skip = skip(SkipKind::Profile, why);\n        return cand;\n",
            "        let _ = why;\n", T_NARROW, "rust"),
    Control("p33a_agy_model_needs_its_effort", SEAT,
            '                || seat.model.strip_suffix("-{effort}") == Some(model.as_str())\n', "",
            T_NARROW, "rust"),
    Control("p33a_family_ignored", SEAT,
            "            .filter(|family| **family != seat.family)\n", "            .filter(|_| false)\n",
            T_NARROW, "rust"),
    Control("p33a_profile_effort_ignored", SEAT,
            "    } else if let Some(eff) = profile_effort {",
            "    } else if let Some(eff) = profile_effort.filter(|_| false) {", T_NARROW, "rust"),
    Control("p33a_no_fit_unnamed", SEAT,
            'no_seat.reason = format!("with profile {}: {}", profile.name, no_seat.reason);',
            "let _ = profile;", T_NOFIT, "rust"),
    Control("p33a_pick_needs_not_joined", SEAT,
            "            crate::nirvana_profiles::joined_needs(&picked.request.needs, &loaded.profile);",
            "            picked.request.needs.clone();", T_PICK, "rust"),
    Control("p33a_pick_class_not_taken", SEAT,
            "            picked.request.class = loaded.profile.class.clone().unwrap_or_default();",
            "            picked.request.class = String::new();", T_PICK, "rust"),
    # §2.2: the copy and its sha, the journal, the contract line, and an edit after seating never reaching the worker.
    Control("p33a_worker_reads_the_project_file", WORKER,
            "    let parsed = crate::nirvana_profiles::from_run(run_dir, &found).map_err(|why| {",
            "    let parsed = crate::nirvana_profiles::load(run_dir.parent().unwrap().parent().unwrap().parent()"
            ".unwrap(), &found.name).map(|loaded| loaded.profile).map_err(|why| {", T_COPY, "rust"),
    Control("p33a_sha_not_journaled", WORKER,
            '                "{}; profile {} sha256 {}",\n                booked.reason, loaded.profile.name, loaded.sha\n',
            '                "{}; profile {}",\n                booked.reason, loaded.profile.name\n', T_COPY, "rust"),
    Control("p33a_line_not_at_line_start", PROFILES,
            'format!("{LINE} {}\\n{SHA_LINE} {}\\n", profile.name, profile.sha)',
            'format!(" {LINE} {}\\n{SHA_LINE} {}\\n", profile.name, profile.sha)', T_COPY, "rust"),
    Control("p33a_class_not_from_profile", WORKER,
            "                request.class = loaded.profile.class.clone();",
            "                request.class = None;", T_COPY, "rust"),
    Control("p33a_needs_not_joined", WORKER,
            "            request.needs = crate::nirvana_profiles::joined_needs(&request.needs, &loaded.profile);",
            "            request.needs = request.needs.clone();", T_COPY, "rust"),
    Control("p33a_no_system_prompt", PROFILES,
            '            "--append-system-prompt-file".to_owned(),', '            "--system-prompt-file".to_owned(),',
            T_COPY, "rust"),
    # §2.3: claude gets --disallowedTools; a copy changed since seating never starts; such a profile is claude-only.
    Control("p33a_disallowed_as_allowed", PROFILES,
            '            "--disallowedTools".to_owned(),', '            "--allowedTools".to_owned(),', T_CLAUDE, "rust"),
    Control("p33a_sha_unchecked", PROFILES,
            "    if sha != profile.sha {", "    if false && sha != profile.sha {", T_CLAUDE, "rust"),
    Control("p33a_disallowed_on_any_seat", PROFILES,
            "    if !profile.disallowed_tools.is_empty() && request.claude_only.is_none() {",
            "    if false && request.claude_only.is_none() {", T_CLAUDE, "rust"),
    # §2.3: agy gets the body at the top of its order, quoted so no body line is a gate line.
    Control("p33a_agy_body_unquoted", PROFILES,
            '                format!("> {line}")', "                line.to_owned()", T_AGY, "rust"),
    Control("p33a_agy_no_body", PROFILES,
            '    if cli == "agy" && !parsed.body.is_empty() {', '    if cli == "codex" && !parsed.body.is_empty() {',
            T_AGY, "rust"),
    # §2.2: a send-back child and a reseat keep the parent's copy, brought to their own CLI, and its narrowing.
    Control("p33a_sections_never_stripped", PROFILES,
            "    let mut out = strip_sections(text, &profile.name);", "    let mut out = text.to_owned();",
            T_SEND_BACK, "rust"),
    Control("p33a_child_cli_ignored", WORKER,
            "    let wanted = crate::nirvana_profiles::order_text(&text, &found, &parsed, &seated.cli);",
            '    let wanted = crate::nirvana_profiles::order_text(&text, &found, &parsed, "claude");',
            T_SEND_BACK, "rust"),
    Control("p33a_reseat_not_narrowed", WORKER,
            "    let narrowed = crate::nirvana_profiles::narrow_for_order(\n        &mut request,\n",
            "    let narrowed = crate::nirvana_profiles::narrow_for_order(\n"
            "        &mut nirvana_seat::Request::new(\"\"),\n", T_RESEAT, "rust"),
    # An order saying `Profile:` is seated only with that profile, and a booking only with the profile it was made with.
    Control("p33a_written_line_ignored", WORKER,
            "    if let Some(written) = written.filter(|written| Some(written.as_str()) != profile) {",
            "    if let Some(written) = written.filter(|_| false) {", T_NAMED, "rust"),
    Control("p33a_booking_profile_ignored", WORKER,
            "    if booked.profile.as_deref() != named {", "    if false && booked.profile.as_deref() != named {",
            T_NAMED, "rust"),
    # §2.5: the digest's lines, at most ten, with model and needs.
    Control("p33a_digest_unbounded", PROFILES,
            "    for (name, _, parsed) in profiles.iter().take(DIGEST_PROFILES) {",
            "    for (name, _, parsed) in profiles.iter() {", T_DIGEST, "rust"),
    Control("p33a_digest_no_model", PROFILES,
            '        (Some(model), _) => parts.push(format!("model {model}")),', "        (Some(_), _) => {}",
            T_DIGEST, "rust"),
    # §2.4: `new` names the project's servers; `check` reads the class against the table.
    Control("p33a_template_no_servers", PROFILES,
            "# This project's MCP servers (.mcp.json): {servers}", "# MCP servers: {servers}", T_NEW, "rust"),
    Control("p33a_check_class_unread", PROFILES,
            "        if table.class(class).is_none() {", "        if false && table.class(class).is_none() {",
            T_NEW, "rust"),
    # §2.4: the Profiles group, the panel's keys.
    Control("p33a_group_from_the_panels_folder", SHELL,
            "    crate::nirvana_profiles::list(&root(cwd))", "    crate::nirvana_profiles::list(cwd)", T_GROUP, "rust"),
    Control("p33a_name_takes_any_key", SHELL,
            "            if (c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')\n",
            "            if (c != ' ')\n", T_NAMING, "rust"),
    Control("p33a_name_one_too_long", SHELL,
            "                && name.len() < crate::nirvana_profiles::NAME_MAX =>",
            "                && name.len() <= crate::nirvana_profiles::NAME_MAX =>", T_NAMING, "rust"),
    Control("p33a_panel_without_profiles", MODS,
            "    items.extend(super::nirvana_profiles::items(&panel.cwd));\n", "", T_PANEL, "rust"),
    Control("p33a_n_does_nothing", MODS,
            "            KeyCode::Char('n') => panel.naming = Some(String::new()),",
            "            KeyCode::Char('n') => {}", T_PANEL, "rust"),
    Control("p33a_new_not_opened", MODS,
            "                    Ok(file) => self.open_in_editor(&file, outcome),",
            "                    Ok(_) => {}", T_PANEL, "rust"),
    Control("p33a_space_on_profile_says_nothing", MODS,
            "            let reason = if super::nirvana_profiles::is_profile(&item) {",
            "            let reason = if false && super::nirvana_profiles::is_profile(&item) {", T_PANEL, "rust"),
)
