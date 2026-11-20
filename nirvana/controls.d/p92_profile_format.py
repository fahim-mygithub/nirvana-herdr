# Plan 45 P45c (nirvana/plans/2026-11-18-45-agent-profiles.md §5, §6): a profile's `skills`, its own mcp.json, the
# shapes, the proposals store, and teaching without the taught profile's seat pins. Controls starting with p92_. One
# sabotage per assertion, each on a product line; the test named in the fifth field is the one expected to print its
# red line with the sabotage in place (written before the run). `Control` is provided by controls.py.
PROFILES = "src/nirvana_profiles.rs"
GUI = "src/nirvana_gui_tools.rs"
AGY = "src/nirvana_agy_tools.rs"
PP = "src/nirvana_profile_proposals.rs"

T_SKILLS = "nirvana_profiles::tests::p92_skills_parse_and_check_refuses_a_skill_the_project_lacks"
T_LINE = "nirvana_profiles::tests::p92_a_claude_worker_with_skills_gets_one_line_naming_them_and_none_without"
T_OWN = "nirvana_profiles::tests::p92_a_profiles_own_servers_reach_its_order_pinned_by_the_run_copy"
T_RULES = "nirvana_profiles::tests::p92_a_profile_server_the_project_defines_is_refused_and_agys_limits_hold"
T_SHAPES = "nirvana_profiles::tests::p92_every_shape_writes_the_folder_form_and_passes_check"
T_TEACH = "nirvana_knowledge::tests::p92_teach_takes_no_seat_pin_of_the_taught_profile"
T_AGY_SKILLS = "nirvana_agy_tools::tests::p92_an_agy_order_gets_only_its_profiles_skills_and_all_without_the_key"
T_COPY_PATH = "nirvana_gui_tools::tests::p92_the_order_copy_path_is_the_one_write_order_writes"
T_LINES = "nirvana_gui_tools::tests::p92_granted_lines_are_added_once_and_only_where_missing"
T_AGY_OWN = ("nirvana_gui_tools::tests::"
             "p92_an_agy_order_gets_its_profiles_own_servers_under_agys_limits_and_no_override")
T_PP_NEW = "nirvana_profile_proposals::tests::p92_a_new_proposal_is_written_listed_accepted_and_rolled_back_exactly"
T_PP_LIMITS = ("nirvana_profile_proposals::tests::"
               "p92_a_proposal_may_not_carry_pins_unknown_skills_needs_or_what_parse_refuses")
T_PP_PATCH = ("nirvana_profile_proposals::tests::"
              "p92_a_patch_replaces_the_body_keeps_the_keys_and_a_stale_one_is_refused")
T_PP_DISMISS = ("nirvana_profile_proposals::tests::"
                "p92_a_dismissed_proposal_is_not_made_again_until_the_dismissal_is_rolled_back")
T_PP_CLI = ("nirvana_profile_proposals::tests::"
            "p92_the_cli_lists_shows_accepts_dismisses_and_takes_only_proposal_ids")
# P45c fix round: the reviewer's section 5, one test each
WORKER = "src/nirvana_worker.rs"
KNOW = "src/nirvana_knowledge.rs"
T_TEACH_SRV = "nirvana_knowledge::tests::p92_a_teacher_gets_no_server_of_the_taught_profile"
T_CLASS = "nirvana_gui_tools::tests::p92_a_teach_or_review_order_gets_no_profile_server_by_its_class"
T_NOTES = "nirvana_profiles::tests::p92_a_literal_token_in_a_profiles_servers_is_noted_by_key_never_by_value"
T_DEVICE = "nirvana_profiles::tests::p92_a_new_profile_never_takes_a_windows_device_name"
T_IGNORE = ("nirvana_profiles::tests::"
            "p92_profiles_gitignore_keeps_a_profiles_servers_out_of_git_and_is_never_rewritten")
T_TORN = "nirvana_profile_proposals::tests::p92_a_torn_ledger_line_loses_no_later_row_and_its_op_is_not_reused"
T_REDACT = "nirvana_profile_proposals::tests::p92_the_key_and_an_accepted_body_are_redacted"
T_LEGACY = ("nirvana_profile_proposals::tests::"
            "p92_a_patch_of_a_legacy_one_file_profile_rolls_back_to_the_exact_bytes")
T_FRONT = "nirvana_profile_proposals::tests::p92_hand_edited_front_matter_at_accept_and_no_mcp_json"
T_PP_DEVICE = "nirvana_profile_proposals::tests::p92_a_proposal_for_a_windows_device_name_is_refused"
# P45c second fix round: the second review's findings and its section 5, one test each
T_EXEC = "nirvana_profiles::tests::p92_seat_exec_starts_with_the_seated_servers_and_its_end_takes_the_copy"
T_SIBLING = "nirvana_profiles::tests::p92_the_servers_copy_lives_while_any_order_seated_with_it_may_start"
T_C2 = "nirvana_profiles::tests::p92_a_profile_whose_own_class_reviews_joins_no_need"
T_D1 = "nirvana_profiles::tests::p92_the_ignore_is_written_only_for_servers_and_a_negation_uncovers"
T_PLAIN = "nirvana_gui_tools::tests::p92_a_plain_profiled_order_launches_without_its_journal_record"
T_ATOMIC = "nirvana_gui_tools::tests::p92_the_servers_copy_is_written_whole_by_a_rename"
T_E1 = "nirvana_profile_proposals::tests::p92_an_upper_case_sk_key_is_redacted_and_keeps_its_identity"

_RUN_SERVERS = "        Some(_) => crate::nirvana_profiles::run_servers(root, run_dir, &found),"
_SKILLS_LINE = "    if let Some(skills) = &parsed.skills {\n        if !prompt.is_empty() {"
_KEY_SHA = "nirvana_lessons::sha256(&redact_key(&nirvana_lessons::one_line(key).to_lowercase()))"

CONTROLS = (
    # Item 1, skills: the key parses, `..` is no folder name, check refuses a skill the project lacks
    Control("p92_skills_dropped", PROFILES, "(None, None) => profile.skills = Some(items),",
            "(None, None) => profile.skills = Some(Vec::new()),", T_SKILLS, "rust"),
    # Fix round F6: a name twice is refused (red: T_SKILLS's `twice` assertion, the list parses)
    Control("p92_skills_twice_allowed", PROFILES, '(None, Some((_, item))) => bad(format!("`{item}` is named twice")),',
            "(None, Some(_)) => profile.skills = Some(items.clone()),", T_SKILLS, "rust"),
    Control("p92_skill_dotdot", PROFILES, "        && !value.starts_with('.')\n", "        && true\n", T_SKILLS, "rust"),
    Control("p92_skill_unchecked", PROFILES, "        if !skills.contains(skill) {", "        if false {", T_SKILLS,
            "rust"),
    # The claude worker: one line naming the skills, and nothing without the key
    Control("p92_skills_line_dropped", PROFILES, _SKILLS_LINE,
            "    if let Some(skills) = &None::<Vec<String>> {\n        if !prompt.is_empty() {", T_LINE, "rust"),
    # Red: the profile with no `skills` key gets a "Use none" line, so its body file is not the body alone
    Control("p92_skills_line_always", PROFILES, _SKILLS_LINE,
            "    if let Some(skills) = &Some(parsed.skills.clone().unwrap_or_default()) {\n"
            "        if !prompt.is_empty() {", T_LINE, "rust"),
    # Fix round F6: `skills: []` asks for none (red: the line reads "use only these: ." instead)
    Control("p92_skills_line_empty_lists_nothing", PROFILES,
            "    if skills.is_empty() {\n        return \"Use none of this project's skills.\".to_owned();\n    }\n",
            "", T_LINE, "rust"),
    # The agy worker: only the named skills, all without the key or without a profile, a missing one said
    Control("p92_agy_skills_unselected", AGY,
            "&selected_skills(claude_skills(order.cwd, order.root, out), named.as_deref(), out),",
            "&selected_skills(claude_skills(order.cwd, order.root, out), None, out),", T_AGY_SKILLS, "rust"),
    Control("p92_agy_skills_none_without_key", AGY, "    let Some(named) = named else {",
            "    let Some(named) = named.or(Some(&[][..])) else {", T_AGY_SKILLS, "rust"),
    # Fix round F6: `skills: []` copies none (red: order a4 gets all three skills)
    Control("p92_agy_empty_skills_selects", AGY, "    Ok(parsed.skills)\n}",
            "    Ok(parsed.skills.filter(|skills| !skills.is_empty()))\n}", T_AGY_SKILLS, "rust"),
    # Fix round F1: a review or a teacher keeps every skill (red: v1's profile_skills is Some([seat, ghost]))
    Control("p92_agy_review_skills_narrowed", AGY,
            "    if parsed.skills.is_some()\n        && crate::nirvana_gui_tools::seated_record(run_dir, order)\n"
            "            .is_some_and(|seated| crate::nirvana_profiles::takes_no_profile_servers(&seated.class))\n"
            "    {",
            "    if order.is_empty() {", T_AGY_SKILLS, "rust"),
    Control("p92_agy_missing_skill_silent", AGY, "        if !skills.iter().any(|(have, _)| have == name) {",
            "        if false {", T_AGY_SKILLS, "rust"),
    # Item 2, a profile's own servers: the seated sha covers mcp.json, the run copies it, the brief gets the device line
    Control("p92_sha_without_servers", PROFILES,
            "sha: seated_sha(&bytes, servers.as_ref().map(|found| found.pin.as_slice())),",
            "sha: seated_sha(&bytes, None),", T_OWN, "rust"),
    Control("p92_servers_not_copied", PROFILES,
            "    if let Some(servers) = &loaded.servers {\n        let to = servers_copy_path",
            "    if let Some(servers) = None::<&ServerFile> {\n        let to = servers_copy_path", T_OWN, "rust"),
    Control("p92_no_device_line", PROFILES, "crate::nirvana_gui_tools::with_granted_lines(out, &parsed.servers)",
            "crate::nirvana_gui_tools::with_granted_lines(out, &[])", T_OWN, "rust"),
    # The launch: the copy's servers in the claude config with no needs line, the copy checked by its sha, the live
    # file never read, and an order seated without the profile granted nothing
    Control("p92_copy_sha_unchecked", PROFILES, "    if sha != profile.sha {", "    if false {", T_OWN, "rust"),
    Control("p92_own_servers_not_read", GUI, _RUN_SERVERS, "        Some(_) => Ok(Map::new()),", T_OWN, "rust"),
    Control("p92_own_servers_need_needs", GUI, "    if needs.is_empty() && own.is_empty() {",
            "    if needs.is_empty() {", T_OWN, "rust"),
    Control("p92_claude_config_lacks_own", GUI, '        config["mcpServers"][name] = definition.clone();',
            "        let _ = definition;", T_OWN, "rust"),
    Control("p92_launch_reads_live_file", GUI, _RUN_SERVERS,
            '        Some(_) => definitions(&std::fs::read_to_string(cwd.join(".nirvana/profiles")'
            '.join(&found.name).join("mcp.json")).unwrap_or_default()),', T_OWN, "rust"),
    # Fix round F3: the definitions' sha is the pin's, from the run copy or the profile's file after the copy went
    # (red: the tampered copy starts)
    # (Second fix round: the find names the outer check alone; the live-file branch has one of its own.)
    Control("p92_servers_sha_unchecked", PROFILES, "    let sha = sha256_hex(&bytes);\n    if sha != pin.sha256 {",
            "    let sha = sha256_hex(&bytes);\n    if false {", T_OWN, "rust"),
    # Second fix round, review2 B1: the start of an attempt leaves the copy (red: T_EXEC's start reads the edited live
    # file and refuses, so the stand-in never keeps a config and its command is Null)
    Control("p92_copy_taken_at_attempt_start", GUI,
            "    crate::nirvana_bladebro::remove_home(run_dir, order, out);\n    let file = order_config_path",
            "    crate::nirvana_bladebro::remove_home(run_dir, order, out);\n"
            "    for copy in crate::nirvana_profiles::servers_copies(run_dir) {\n"
            "        let _ = std::fs::remove_file(copy);\n    }\n    let file = order_config_path", T_EXEC, "rust"),
    # ...a release keeps a copy an order still wanting a worker holds (red: s2's copy is gone after s1's end)
    Control("p92_release_ignores_holders", GUI, "    for copy in copies.iter().filter(|copy| !held.contains(copy)) {",
            "    for copy in copies.iter().filter(|_| true) {", T_SIBLING, "rust"),
    # ...and takes it once the order ended (red: the copy is still there after s1 ended and the release ran)
    Control("p92_release_keeps_ended", GUI,
            "    for known in state.orders.iter().filter(|known| known.phase.wants_worker()) {",
            "    for known in state.orders.iter() {", T_OWN, "rust"),
    # ...when `seat exec` ends, on every way out (red: the copy is still there after the failed attempt)
    Control("p92_exec_end_keeps_copy", WORKER,
            "    crate::nirvana_gui_tools::release_servers_copies(&project.run_dir(run), out);\n    code\n",
            "    code\n", T_EXEC, "rust"),
    # ...and for every run when a supervisor starts (red: the copy is still there after sweep_leftovers)
    Control("p92_sweep_keeps_copy", WORKER,
            "        crate::nirvana_gui_tools::release_servers_copies(&run_dir, out);\n        let leftovers",
            "        let leftovers", T_SIBLING, "rust"),
    # ...a changed live file with no copy left says what happened and what to do (red: (d)'s words are missing)
    Control("p92_fallback_message_unsaid", PROFILES,
            "the profile's mcp.json changed since this order was seated (", "the profile's servers changed (",
            T_OWN, "rust"),
    # Review2 B2: the copy is written by a rename (red: a reader sees a short file while it is rewritten in place)
    Control("p92_copy_written_torn", GUI, "    write_private(&temp, body)?;\n",
            "    if !body.is_empty() {\n        return write_private(file, body);\n    }\n"
            "    write_private(&temp, body)?;\n", T_ATOMIC, "rust"),
    # Review2 A1: a profile with no pin reads no journal (red: p1 with no record is refused)
    Control("p92_plain_profile_reads_journal", GUI,
            "    if !crate::nirvana_profiles::pin_path(run_dir, &found.name, &found.sha).is_file() {\n"
            "        return Ok(Map::new());\n    }\n", "", T_PLAIN, "rust"),
    # A reader after the end has the names from the pin (red: from_run's servers are empty)
    Control("p92_pin_names_dropped", PROFILES, "    parsed.servers = pin.names.clone();",
            "    parsed.servers = Vec::new();", T_OWN, "rust"),
    # Fix round F1: a teacher and a lesson review get no own server, by the journaled class, on claude and agy
    # (red: t1's claude argv gains --mcp-config)
    Control("p92_teach_review_servers_granted", GUI,
            "        Some(seated) if crate::nirvana_profiles::takes_no_profile_servers(&seated.class) => {",
            "        Some(seated) if seated.class.is_empty() => {", T_CLASS, "rust"),
    # The review class is in the guard (red: v1's argv gains --mcp-config)
    Control("p92_review_class_unguarded", PROFILES, "        || crate::nirvana_run::reviews(class)\n",
            "        || class == \"review.adversary\"\n", T_CLASS, "rust"),
    # Review2 C1: the review and acceptance families by rule (red: v3, acceptance.author, gains --mcp-config)
    Control("p92_review_family_unguarded", PROFILES,
            "        || (class.contains('.') && matches!(family, \"review\" | \"acceptance\"))",
            "        || (class.contains('.') && matches!(family, \"lesson\"))", T_CLASS, "rust"),
    # No seated record is refused, never granted (red: n1 starts with the server)
    Control("p92_no_record_granted", GUI, "        None => Err(no_record(run_dir, order)),",
            "        None => crate::nirvana_profiles::run_servers(root, run_dir, &found),", T_CLASS, "rust"),
    # The real teach path: the class guard holds there (red: the teacher's argv gains --mcp-config for fake-own)
    Control("p92_teach_own_servers_granted", PROFILES,
            "    class == crate::nirvana_knowledge::TEACH_CLASS\n        || crate::nirvana_run::reviews(class)",
            "    crate::nirvana_run::reviews(class)", T_TEACH_SRV, "rust"),
    # Review2 C2: the profile's class default comes before the guard (red: q1's seated.needs is [bambu-studio-beta])
    Control("p92_class_default_after_guard", WORKER,
            "            if request.class.is_none() {\n                request.class = loaded.profile.class.clone();\n"
            "            }\n            // P45c fix round (review F2)",
            "            // P45c fix round (review F2)", T_C2, "rust"),
    # Fix round F2: no profile need joins a teacher's request (red: seated.needs is [fake-server])
    Control("p92_teach_joins_needs", WORKER,
            "            if !request\n                .class\n                .as_deref()\n"
            "                .is_some_and(crate::nirvana_profiles::takes_no_profile_servers)\n            {",
            "            if true {", T_TEACH_SRV, "rust"),
    # Fix round F3: the notes (red: API_TOKEN, upper case, is not noted)
    Control("p92_secret_note_case_kept", PROFILES,
            "                if !SECRET_WORDS.iter().any(|word| lower.contains(word)) {",
            "                if !SECRET_WORDS.iter().any(|word| key.contains(word)) {", T_NOTES, "rust"),
    # A `${VAR}` value is no literal (red: Secret_Ref and Authorization are noted too)
    Control("p92_secret_var_noted", PROFILES,
            "                            && !text.split(\"${\").skip(1).any(|rest| rest.contains('}'))",
            "                            && true", T_NOTES, "rust"),
    # The note never holds the value (red: the notes differ from the expected text, and hold FAKE-TOKEN-123)
    Control("p92_secret_note_prints_value", PROFILES, "{field} {key} is written out,",
            "{field} {key} {value} is written out,", T_NOTES, "rust"),
    # The ignore, the lead's ruling: profiles/.gitignore holds `*/mcp.json` (red: git does not ignore operator/mcp.json)
    Control("p92_ignore_rule_missing", PROFILES, "*/mcp.json\\n\";", "*/mcp.jsonx\\n\";", T_IGNORE, "rust"),
    # ...written when a profile with its own servers is loaded for seating (red: no file after load)
    Control("p92_ignore_not_written_at_load", PROFILES, "        let _ = ensure_ignore(root);\n", "", T_IGNORE, "rust"),
    # ...never over a file that is there (red: the emptied file is written again)
    Control("p92_ignore_rewritten", PROFILES, "        .create_new(true)", "        .create(true)\n        .truncate(true)",
            T_IGNORE, "rust"),
    # `list` never takes .gitignore for a profile (red: a profile named ".gitignore" is listed)
    Control("p92_list_reads_gitignore", PROFILES,
            '            } else if entry.is_file() && entry.extension().is_some_and(|ext| ext == "md") {',
            "            } else if entry.is_file() {", T_IGNORE, "rust"),
    # The note says when no rule covers the file (red: two notes, not three, before the file is written)
    Control("p92_ignore_uncovered_unsaid", PROFILES, "    if !notes.is_empty() && !ignore_covers(root) {",
            "    if false {", T_NOTES, "rust"),
    # Review2 D1: check writes the file only when a profile has servers (red: written with only `plain` there)
    Control("p92_ignore_written_without_servers", PROFILES, "    if any {\n        ensure_ignore(root)",
            "    if true {\n        ensure_ignore(root)", T_D1, "rust"),
    # ...and a later negation takes the cover back (red: the `!*/mcp.json` row reads covered)
    Control("p92_ignore_negation_read", PROFILES,
            "                Some(negated) => negated.ends_with(\"mcp.json\").then_some(false),",
            "                Some(_) => None,", T_D1, "rust"),
    Control("p92_unprofiled_order_reads_other_copy", GUI,
            "    let Ok(text) = std::fs::read_to_string(order_copy_path(cwd, order)) else {",
            '    let Ok(text) = std::fs::read_to_string(order_copy_path(cwd, "s1")) else {', T_OWN, "rust"),
    Control("p92_order_copy_path_drifts", GUI, '        .join("orders")\n        .join(format!("{order}.md"))',
            '        .join("order")\n        .join(format!("{order}.md"))', T_COPY_PATH, "rust"),
    # The rules: no override of a project server (check, load, launch), agy's limits, Debug names no definition,
    # no needs line for the profile's own server
    Control("p92_server_override_allowed", PROFILES, "        if project.contains(name) {", "        if false {",
            T_RULES, "rust"),
    Control("p92_load_skips_server_check", PROFILES, "let servers = read_servers(root, name).map_err(refused)?;",
            "let servers = read_servers(root, name).unwrap_or(None);", T_RULES, "rust"),
    Control("p92_servers_claude_only_ignored", PROFILES, "if let Some(why) = &profile.servers_claude_only {",
            "if let Some(why) = &None::<String> {", T_RULES, "rust"),
    Control("p92_debug_prints_definition", PROFILES, "            self.bytes.len()\n",
            "            String::from_utf8_lossy(&self.bytes)\n", T_RULES, "rust"),
    Control("p92_own_server_in_needs_allowed", PROFILES, ".filter(|need| servers.names.contains(need))",
            ".filter(|_| false)", T_RULES, "rust"),
    Control("p92_launch_override_allowed", GUI,
            "        if let Some(name) = own_names.iter().find(|name| project.contains(name)) {",
            "        if let Some(name) = own_names.iter().find(|_| false) {", T_AGY_OWN, "rust"),
    Control("p92_agy_own_not_written", GUI, "        chosen.extend(own.clone());",
            "        chosen.extend(Map::<String, Value>::new());", T_AGY_OWN, "rust"),
    Control("p92_agy_own_unchecked", GUI, "        if let Some(why) = claude_only(&own_names, &own) {",
            "        if let Some(why) = None::<String> {", T_AGY_OWN, "rust"),
    # The brief's lines: once, and a desktop server's line when it is missing
    Control("p92_granted_lines_twice", GUI, "    if !text.contains(DEVICE_LINE) && !text.contains(SEND_LINE) {",
            "    if true {", T_LINES, "rust"),
    Control("p92_granted_desktop_line_missing", GUI,
            "    if desktop && !text.contains(BRIEF_LINE) && !text.contains(DESKTOP_SEND_LINE) {", "    if false {",
            T_LINES, "rust"),
    # Item 3, shapes: the cards adopted, never over a profile, the reviewer's judging block, its pins commented,
    # researcher is web-researcher
    Control("p92_shape_cards_unadopted", PROFILES, "    crate::nirvana_knowledge::adopt(root, name)?;\n    Ok(profile_file)",
            "    let _ = name;\n    Ok(profile_file)", T_SHAPES, "rust"),
    Control("p92_shape_overwrites", PROFILES,
            '    if dir.exists() {\n        return Err(format!("{} is already there (a folder profile)", dir.display()));',
            '    if false {\n        return Err(format!("{} is already there (a folder profile)", dir.display()));',
            T_SHAPES, "rust"),
    Control("p92_reviewer_ceiling_dropped", PROFILES, "card. The worst defect sets the ceiling.\\n", "card.\\n",
            T_SHAPES, "rust"),
    Control("p92_reviewer_pins_live", PROFILES, "# class: media.video\\n", "class: media.video\\n", T_SHAPES, "rust"),
    Control("p92_researcher_not_web_researcher", PROFILES, "            profile: WEB_RESEARCHER.to_owned(),",
            '            profile: WEB_RESEARCHER.replace("You research", "You look up"),', T_SHAPES, "rust"),
    # Fix round F7: a new profile never takes a device name (red: `con` is a valid new name)
    Control("p92_device_name_allowed", PROFILES,
            "    valid_name(name) && !crate::nirvana_knowledge::windows_device(name)", "    valid_name(name)",
            T_DEVICE, "rust"),
    # ...in any case (red: CON.md is not seen as the console)
    Control("p92_device_case_kept", KNOW,
            "    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();",
            "    let stem = name.split('.').next().unwrap_or(name).to_owned();", T_DEVICE, "rust"),
    # ...nor a proposal of one (red: the `con` draft is written)
    Control("p92_pp_device_allowed", PP,
            "    if kind == Kind::New {\n        if let Some(why) = nirvana_profiles::new_name_problem(name) {",
            "    if false {\n        if let Some(why) = nirvana_profiles::new_name_problem(name) {", T_PP_DEVICE,
            "rust"),
    # Item 5, teach: none of the taught profile's family, needs or effort (Megajam, 2026-11-16)
    Control("p92_teach_keeps_family", PROFILES, "family: profile.family.clone().filter(|_| !teach),",
            "family: profile.family.clone(),", T_TEACH, "rust"),
    Control("p92_teach_keeps_needs", PROFILES, "        request.needs.retain(|need| !profile.needs.contains(need));",
            "        let _ = &request.needs;", T_TEACH, "rust"),
    Control("p92_teach_keeps_effort", PROFILES, "effort: profile.effort.filter(|_| !teach),",
            "effort: profile.effort,", T_TEACH, "rust"),
    # Item 4, the proposals store: outside profiles/, accept writes the profile, rollback undoes it once
    Control("p92_pp_inside_profiles", PP, "    root.join(nirvana_run::DIR).join(DIR)",
            "    root.join(nirvana_run::DIR).join(nirvana_profiles::DIR).join(DIR)", T_PP_NEW, "rust"),
    Control("p92_pp_accept_writes_nothing", PP, "    crate::nirvana_seat::write_atomic(&file, after.as_bytes())",
            '    crate::nirvana_seat::write_atomic(&file, b"")', T_PP_NEW, "rust"),
    Control("p92_pp_rollback_leaves_profile", PP,
            '                remove_new(&dir)\n                    .map_err(|err| format!("cannot remove {}: {err}", '
            'dir.display()))?;',
            "                let _ = &dir;", T_PP_NEW, "rust"),
    Control("p92_pp_rollback_takes_taught_cards", PP, "                if holds_more(&dir) {",
            "                if false {", T_PP_NEW, "rust"),
    Control("p92_pp_rollback_twice", PP, "    if rolled_back(&rows).contains(&op) {", "    if false {", T_PP_NEW,
            "rust"),
    # What a proposal may not carry (§6), at write and again at accept
    Control("p92_pp_model_pin_allowed", PP, "    if profile.model.is_some() || profile.family.is_some() {",
            "    if false {", T_PP_LIMITS, "rust"),
    Control("p92_pp_unknown_skill_allowed", PP, ".filter(|skill| !skills.contains(skill))", ".filter(|_| false)",
            T_PP_LIMITS, "rust"),
    Control("p92_pp_unknown_need_allowed", PP, "        if !capability && !server {", "        if false {",
            T_PP_LIMITS, "rust"),
    Control("p92_pp_parse_refusal_dropped", PP,
            "        Err(found) => {\n            problems.extend(found);\n            return problems;",
            "        Err(_) => {\n            return problems;", T_PP_LIMITS, "rust"),
    Control("p92_pp_existing_name_allowed", PP,
            "    if nirvana_profiles::path(root, name).exists()\n        || nirvana_profiles::folder_of(root, name)"
            ".exists()\n    {",
            "    if false {", T_PP_LIMITS, "rust"),
    Control("p92_pp_count_unchecked", PP, "    if !evidence.bytes().any(|b| b.is_ascii_digit()) {", "    if false {",
            T_PP_LIMITS, "rust"),
    Control("p92_pp_quote_unchecked", PP, "    if !quoted(&evidence)", "    if false && !quoted(&evidence)",
            T_PP_LIMITS, "rust"),
    # P45a fix round: accept's refusal is a `Refused::No` now (the panel's `accept_seen` shares the body).
    Control("p92_pp_accept_unchecked", PP,
            '    if !problems.is_empty() {\n        return Err(Refused::No(format!("{id} is refused: {}", problems.join("; "))));',
            '    if false {\n        return Err(Refused::No(format!("{id} is refused: {}", problems.join("; "))));',
            T_PP_LIMITS, "rust"),
    # A patch: the front matter kept, a stale one refused, its rollback exact
    Control("p92_pp_patch_drops_keys", PP, '        lines[..=close].join("\\n"),', '        "---\\n---",',
            T_PP_PATCH, "rust"),
    Control("p92_pp_stale_accepted", PP, "        if proposal.base_sha.as_deref() != Some(now.as_str()) {",
            "        if false {", T_PP_PATCH, "rust"),
    Control("p92_pp_rollback_patch_wrong", PP,
            "            (Kind::Patch, Some(before)) => {\n"
            "                crate::nirvana_seat::write_atomic(&file, before.as_bytes())",
            "            (Kind::Patch, Some(_)) => {\n"
            '                crate::nirvana_seat::write_atomic(&file, b"x")', T_PP_PATCH, "rust"),
    # A dismissal: remembered by its identity, case and spaces aside, until it is rolled back; no duplicate waits
    Control("p92_pp_dismissal_forgotten", PP,
            "    if let Some(date) = was_dismissed(root, draft.kind, &draft.profile, &draft.key) {",
            "    if let Some(date) = None::<String> {", T_PP_DISMISS, "rust"),
    Control("p92_pp_key_case_kept", PP, _KEY_SHA,
            "nirvana_lessons::sha256(&redact_key(&nirvana_lessons::one_line(key)))", T_PP_DISMISS, "rust"),
    # Fix round F4: a torn line (red: the next row is glued onto it and lost, so op-10 is never seen)
    Control("p92_pp_torn_glued", PP, "        if last[0] != b'\\n' {", "        if false {", T_TORN, "rust"),
    # ...its op number is not used again (red: the next op is op-3)
    Control("p92_pp_torn_op_reused", PP,
            "                .flat_map(|(_, line, _)| numbers_in(line, \"\\\"op-\")),",
            "                .flat_map(|_| None::<u64>),", T_TORN, "rust"),
    # ...and it is reported (red: broken() is empty)
    Control("p92_pp_torn_unreported", PP,
            "            Err(err) => bad.push((at + 1, line.to_owned(), err.to_string())),",
            "            Err(_) => {}", T_TORN, "rust"),
    # Fix round F5: the key is redacted on disk (red: the fake GitHub token is in the proposal file)
    # (P45a third fix round: the key is stripped first, so the find text names `strip_invisible`.)
    Control("p92_pp_key_unredacted", PP, "        key: redact_key(&strip_invisible(&draft.key).to_lowercase()),\n", "",
            T_REDACT, "rust"),
    # Review2 E1: lowercased before redaction (red: the SK-FAKE... key lands on disk)
    Control("p92_pp_key_case_kept_on_disk", PP, "        key: redact_key(&strip_invisible(&draft.key).to_lowercase()),\n",
            "        key: redact_key(&strip_invisible(&draft.key)),\n", T_E1, "rust"),
    # ...keeping its identity (red: two paragraph shas collapse into one key, the second write is refused)
    Control("p92_pp_key_identity_lost", PP,
            '            format!(\n                "h:{}",\n                &nirvana_lessons::sha256(&found.to_lowercase())[..16]\n'
            '            )',
            '            "[redacted]".to_owned()', T_REDACT, "rust"),
    # ...one identity raw and on disk (red: was_dismissed with the raw key finds no dismissal)
    Control("p92_pp_key_sha_unredacted", PP, _KEY_SHA,
            "nirvana_lessons::sha256(&nirvana_lessons::one_line(key).to_lowercase())", T_REDACT, "rust"),
    # accept redacts the body it writes (red: sk-FAKE... lands in profile.md)
    Control("p92_pp_accept_unredacted", PP, "    proposal.body = nirvana_lessons::redact(&proposal.body);\n", "",
            T_REDACT, "rust"),
    # Review section 5: a legacy one-file profile rolls back to its exact bytes (red: the CRLF bytes come back LF)
    Control("p92_pp_legacy_rollback_lf", PP,
            "            (Kind::Patch, Some(before)) => {\n"
            "                crate::nirvana_seat::write_atomic(&file, before.as_bytes())",
            "            (Kind::Patch, Some(before)) => {\n"
            "                crate::nirvana_seat::write_atomic(&file, before.replace(\"\\r\\n\", \"\\n\").as_bytes())",
            T_LEGACY, "rust"),
    # Hand-edited front matter at accept is checked again (red: ` model:` is accepted)
    Control("p92_pp_front_unchecked", PP,
            '    if !problems.is_empty() {\n        return Err(Refused::No(format!("{id} is refused: {}", problems.join("; "))));',
            '    if false {\n        return Err(Refused::No(format!("{id} is refused: {}", problems.join("; "))));',
            T_FRONT, "rust"),
    # A proposal's own key twice is refused (red: the duplicate `key` proposal is accepted)
    Control("p92_pp_duplicate_key_read", PP, "        if values.iter().any(|(seen, _)| seen == known) {",
            "        if false {", T_FRONT, "rust"),
    # Accept writes no mcp.json (red: one is written beside profile.md, so the profile has own servers)
    Control("p92_pp_accept_writes_servers", PP,
            '            Refused::No(format!("cannot create {}: {err}", dir.display()))\n        })?;\n',
            '            Refused::No(format!("cannot create {}: {err}", dir.display()))\n        })?;\n'
            '        let _ = std::fs::write(dir.join(nirvana_profiles::SERVERS_FILE), "{\\"mcpServers\\": {}}");\n',
            T_FRONT, "rust"),
    Control("p92_pp_rollback_keeps_dismissal", PP, "                && !undone.contains(&row.op.as_str())\n", "",
            T_PP_DISMISS, "rust"),
    Control("p92_pp_duplicate_allowed", PP,
            '        problems.push(format!(\n            "the same proposal is waiting already as {}",\n'
            "            waiting.id\n        ));",
            "        let _ = waiting;", T_PP_DISMISS, "rust"),
    # The CLI takes only proposal ids, never a path
    Control("p92_pp_id_unchecked", PP, 'pub fn valid_id(id: &str) -> bool {\n    id.strip_prefix("pp-")',
            'pub fn valid_id(id: &str) -> bool {\n    true || id.strip_prefix("pp-")', T_PP_CLI, "rust"),
)
