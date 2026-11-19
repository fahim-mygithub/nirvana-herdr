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

_RUN_SERVERS = "        Some(Ok(found)) => crate::nirvana_profiles::run_servers(run_dir, &found),"
_SKILLS_LINE = "    if !parsed.skills.is_empty() {\n        if !prompt.is_empty() {"

CONTROLS = (
    # Item 1, skills: the key parses, `..` is no folder name, check refuses a skill the project lacks
    Control("p92_skills_dropped", PROFILES, "None => profile.skills = items,", "None => profile.skills = Vec::new(),",
            T_SKILLS, "rust"),
    Control("p92_skill_dotdot", PROFILES, "        && !value.starts_with('.')\n", "        && true\n", T_SKILLS, "rust"),
    Control("p92_skill_unchecked", PROFILES, "        if !skills.contains(skill) {", "        if false {", T_SKILLS,
            "rust"),
    # The claude worker: one line naming the skills, and nothing without the key
    Control("p92_skills_line_dropped", PROFILES, _SKILLS_LINE,
            "    if false {\n        if !prompt.is_empty() {", T_LINE, "rust"),
    Control("p92_skills_line_always", PROFILES, _SKILLS_LINE,
            "    if true {\n        if !prompt.is_empty() {", T_LINE, "rust"),
    # The agy worker: only the named skills, all without the key or without a profile, a missing one said
    Control("p92_agy_skills_unselected", AGY,
            "&selected_skills(claude_skills(order.cwd, order.root, out), named.as_deref(), out),",
            "&selected_skills(claude_skills(order.cwd, order.root, out), None, out),", T_AGY_SKILLS, "rust"),
    Control("p92_agy_skills_none_without_key", AGY, "    let Some(named) = named else {",
            "    let Some(named) = named.or(Some(&[][..])) else {", T_AGY_SKILLS, "rust"),
    Control("p92_agy_empty_skills_selects", AGY, "    Ok(Some(parsed.skills).filter(|skills| !skills.is_empty()))",
            "    Ok(Some(parsed.skills))", T_AGY_SKILLS, "rust"),
    Control("p92_agy_missing_skill_silent", AGY, "        if !skills.iter().any(|(have, _)| have == name) {",
            "        if false {", T_AGY_SKILLS, "rust"),
    # Item 2, a profile's own servers: the seated sha covers mcp.json, the run copies it, the brief gets the device line
    Control("p92_sha_without_servers", PROFILES,
            "sha: seated_sha(&bytes, servers.as_ref().map(|found| found.bytes.as_slice())),",
            "sha: seated_sha(&bytes, None),", T_OWN, "rust"),
    Control("p92_servers_not_copied", PROFILES,
            "    if let Some(servers) = &loaded.servers {\n        let to = servers_copy_path",
            "    if let Some(servers) = None::<&ServerFile> {\n        let to = servers_copy_path", T_OWN, "rust"),
    Control("p92_no_device_line", PROFILES, "crate::nirvana_gui_tools::with_granted_lines(out, &parsed.servers)",
            "crate::nirvana_gui_tools::with_granted_lines(out, &[])", T_OWN, "rust"),
    # The launch: the copy's servers in the claude config with no needs line, the copy checked by its sha, the live
    # file never read, and an order seated without the profile granted nothing
    Control("p92_copy_sha_unchecked", PROFILES, "    if sha != profile.sha {", "    if false {", T_OWN, "rust"),
    Control("p92_own_servers_not_read", GUI, _RUN_SERVERS, "        Some(Ok(_)) => Ok(Map::new()),", T_OWN, "rust"),
    Control("p92_own_servers_need_needs", GUI, "    if needs.is_empty() && own.is_empty() {",
            "    if needs.is_empty() {", T_OWN, "rust"),
    Control("p92_claude_config_lacks_own", GUI, '        config["mcpServers"][name] = definition.clone();',
            "        let _ = definition;", T_OWN, "rust"),
    Control("p92_launch_reads_live_file", GUI, _RUN_SERVERS,
            '        Some(Ok(found)) => definitions(&std::fs::read_to_string(cwd.join(".nirvana/profiles")'
            '.join(&found.name).join("mcp.json")).unwrap_or_default()),', T_OWN, "rust"),
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
    Control("p92_pp_accept_unchecked", PP,
            '    if !problems.is_empty() {\n        return Err(format!("{id} is refused: {}", problems.join("; ")));',
            '    if false {\n        return Err(format!("{id} is refused: {}", problems.join("; ")));',
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
    Control("p92_pp_key_case_kept", PP, "nirvana_lessons::sha256(&nirvana_lessons::one_line(key).to_lowercase())",
            "nirvana_lessons::sha256(&nirvana_lessons::one_line(key))", T_PP_DISMISS, "rust"),
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
