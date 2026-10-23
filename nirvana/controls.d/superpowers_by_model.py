# Plan 28: superpowers only on Sonnet (package `superpowers_by_model`, nirvana/plans/2026-10-23-28-records/p28.md).
# Controls starting with p28_. One sabotage per assertion, each on a product line; the test named must print FAIL
# with it in place. `Control` is provided by controls.py.
RULE = "src/nirvana_superpowers.rs"
LAUNCH = "src/nirvana_launch.rs"
WORKER = "src/nirvana_worker.rs"

T_RULE = "opus_turns_every_superpowers_off_and_sonnet_and_the_rest_add_nothing"
T_IDS = "the_ids_are_every_plugin_named_superpowers_before_the_at_or_the_default"
T_MERGE = "the_merge_keeps_every_other_key_and_refuses_what_is_not_an_object"
T_IDS_FOR = "the_ids_come_from_the_installed_list_the_folders_settings_and_the_base"
T_LAUNCH_FILE = "an_opus_launch_writes_the_merged_file_and_sonnet_writes_nothing"
T_WORKER_FILE = "an_opus_claude_worker_gets_the_rule_with_or_without_a_preset"
T_NO_HOME = "no_test_reads_the_owners_claude_folder"
T_LAUNCH = "an_opus_seat_launches_with_every_superpowers_off_in_its_one_settings_file"
T_SONNET = "a_sonnet_seat_launches_with_todays_settings_file_and_writes_nothing"
T_BROKEN = "a_broken_settings_file_keeps_todays_argv"
T_GENERATED = "the_launch_names_the_folders_generated_settings_when_one_was_written"
T_RESTART = "restart_claude_picks_up_the_superpowers_rule_from_the_mode_it_reads_now"
T_SERVER = "nirvana_resumes_the_folders_last_session_with_its_preset_file_and_fresh_never_resumes"
T_W_SETTINGS = "an_opus_claude_worker_starts_with_superpowers_off_and_an_error_keeps_the_preset"
T_W_ARGV = "the_workers_argv_carries_the_preset_under_the_rule_for_its_own_model"

CONTROLS = (
    # §2: one function decides from a model id or label.
    Control("p28_opus_not_opus", RULE, '    if model.contains("opus") {\n',
            '    if model.contains("opus-never") {\n', T_RULE, "rust"),
    Control("p28_sonnet_not_sonnet", RULE, '    } else if model.contains("sonnet") {\n',
            "    } else if false {\n", T_RULE, "rust"),
    Control("p28_label_case_kept", RULE, "    let model = model.to_ascii_lowercase();\n",
            "    let model = model.to_owned();\n", T_RULE, "rust"),
    Control("p28_opus_entries_on", RULE, "        Family::Opus => ids.iter().map(|id| (id.clone(), false)).collect(),\n",
            "        Family::Opus => ids.iter().map(|id| (id.clone(), true)).collect(),\n", T_RULE, "rust"),
    Control("p28_sonnet_gets_entries", RULE, "        Family::Sonnet | Family::Other => Vec::new(),\n",
            "        Family::Sonnet | Family::Other => ids.iter().map(|id| (id.clone(), false)).collect(),\n",
            T_RULE, "rust"),
    # §2: the ids are every plugin whose name before `@` is superpowers, else the default.
    Control("p28_name_prefix_matches", RULE, ".is_some_and(|name| name.eq_ignore_ascii_case(NAME))",
            ".is_some_and(|name| name.starts_with(NAME))", T_IDS, "rust"),
    Control("p28_no_default_id", RULE, "        ids.push(DEFAULT_ID.to_owned());\n", "        let _ = DEFAULT_ID;\n",
            T_IDS, "rust"),
    Control("p28_ids_not_deduped", RULE, "    ids.dedup();\n", "", T_IDS, "rust"),
    Control("p28_installed_wrong_key", RULE, '        .get("plugins")\n', '        .get("plugin")\n', T_IDS, "rust"),
    Control("p28_enabled_wrong_key", RULE, '        .get("enabledPlugins")\n', '        .get("enabled")\n',
            T_IDS, "rust"),
    # §2: the names come from Claude's installed list, the folder's settings and the file passed.
    Control("p28_installed_list_unread", RULE, '|dir| read_json(&dir.join("plugins").join("installed_plugins.json"))',
            '|dir| read_json(&dir.join("plugins").join("installed.json"))',
            T_IDS_FOR, "rust"),
    Control("p28_local_settings_unread", RULE, '    for file in ["settings.json", "settings.local.json"] {\n',
            '    for file in ["settings.json"] {\n', T_IDS_FOR, "rust"),
    Control("p28_project_settings_unread", RULE, '    for file in ["settings.json", "settings.local.json"] {\n',
            '    for file in ["settings.local.json"] {\n', T_IDS_FOR, "rust"),
    Control("p28_base_names_unread", RULE, "    names.extend(enabled_names(base));\n", "", T_IDS_FOR, "rust"),
    # §4: no test reads the real ~/.claude.
    Control("p28_tests_resolve_claude_dir", RULE, "    if cfg!(test) {\n        return None;\n    }\n", "",
            T_NO_HOME, "rust"),
    # §3: the merge keeps every other key and refuses what it cannot merge into.
    Control("p28_merge_drops_keys", RULE, "    let mut value = base.clone();\n",
            "    let mut value = Value::Object(Map::new());\n", T_MERGE, "rust"),
    Control("p28_merge_takes_non_object", RULE, "    let mut value = base.clone();\n",
            "    let mut value = if base.is_object() { base.clone() } else { Value::Object(Map::new()) };\n",
            T_MERGE, "rust"),
    Control("p28_merge_overwrites_plugins", RULE,
            '    let plugins = root\n        .entry("enabledPlugins")\n',
            '    if !root.get("enabledPlugins").is_none_or(Value::is_object) {\n'
            '        root.insert("enabledPlugins".into(), Value::Object(Map::new()));\n    }\n'
            '    let plugins = root\n        .entry("enabledPlugins")\n',
            T_MERGE, "rust"),
    Control("p28_merge_writes_no_entry", RULE, "        plugins.insert(id.clone(), Value::Bool(*on));\n",
            "        let _ = (id, on, &plugins);\n", T_MERGE, "rust"),
    # §3: Nirvana Claude's launch file, at its path, only for Opus, never from a missing base.
    Control("p28_launch_file_for_sonnet", RULE,
            "    if family != Family::Opus {\n        return Ok(None);\n    }\n", "", T_LAUNCH_FILE, "rust"),
    Control("p28_launch_missing_base_taken", RULE,
            '        read_json(base).ok_or_else(|| format!("{} is missing or not JSON", base.display()))?;\n',
            "        read_json(base).unwrap_or_else(|| Value::Object(Map::new()));\n", T_LAUNCH_FILE, "rust"),
    Control("p28_launch_path_no_family", RULE,
            '        "{}.{}.settings.json",\n        crate::nirvana_launch::folder_hash(folder),\n        family.word()\n',
            '        "{}.{}.settings.json",\n        crate::nirvana_launch::folder_hash(folder),\n        "any"\n',
            T_LAUNCH_FILE, "rust"),
    Control("p28_launch_file_unmerged", RULE,
            "    let path = launch_settings_path(dir, folder, family);\n"
            "    write_json(&path, &merged(&base_value, &entries)?)?;\n",
            "    let path = launch_settings_path(dir, folder, family);\n"
            "    write_json(&path, &base_value)?;\n    let _ = entries;\n",
            T_LAUNCH_FILE, "rust"),
    # §3: a Claude worker's file, from spec.model, with or without a preset; agy untouched.
    Control("p28_worker_agy_touched", RULE, '    if cli != "claude" || family != Family::Opus {\n',
            "    if family != Family::Opus {\n", T_WORKER_FILE, "rust"),
    Control("p28_worker_sonnet_touched", RULE, '    if cli != "claude" || family != Family::Opus {\n',
            '    if cli != "claude" {\n', T_WORKER_FILE, "rust"),
    Control("p28_worker_no_preset_no_file", RULE, "        None => Value::Object(Map::new()),\n",
            "        None => return Ok(None),\n", T_WORKER_FILE, "rust"),
    Control("p28_worker_broken_preset_taken", RULE,
            '            read_json(file).ok_or_else(|| format!("{} is missing or not JSON", file.display()))?\n',
            "            read_json(file).unwrap_or_else(|| Value::Object(Map::new()))\n", T_WORKER_FILE, "rust"),
    Control("p28_worker_shares_launch_path", RULE, '        "{}.{}.worker.settings.json",\n',
            '        "{}.{}.settings.json",\n', T_WORKER_FILE, "rust"),
    Control("p28_worker_file_unmerged", RULE,
            "    let path = worker_settings_path(dir, folder, family);\n"
            "    write_json(&path, &merged(&base_value, &entries)?)?;\n",
            "    let path = worker_settings_path(dir, folder, family);\n"
            "    write_json(&path, &base_value)?;\n    let _ = entries;\n",
            T_WORKER_FILE, "rust"),
    # §3: launch_args passes the launch file in its one --settings, from the seat's own model and the file it had.
    Control("p28_launch_file_not_passed", LAUNCH,
            "            Ok(Some(file)) => args[at + 1] = file.display().to_string(),\n",
            "            Ok(Some(_file)) => {}\n", T_LAUNCH, "rust"),
    Control("p28_new_space_no_launch_file", LAUNCH,
            "            Ok(Some(file)) => args[at + 1] = file.display().to_string(),\n",
            "            Ok(Some(_file)) => {}\n", T_SERVER, "rust"),
    Control("p28_launch_fixed_model", LAUNCH, "            &seat_model,\n", '            "claude-sonnet-5-5",\n',
            T_LAUNCH, "rust"),
    Control("p28_restart_ignores_mode", LAUNCH, "    let seat_model = model.clone();\n",
            "    let seat_model = MODEL.to_owned();\n", T_RESTART, "rust"),
    Control("p28_sonnet_launch_gets_file", LAUNCH, "    let seat_model = model.clone();\n",
            "    let seat_model = MODEL.to_owned();\n", T_SONNET, "rust"),
    Control("p28_launch_claude_dir_dropped", LAUNCH, "            claude_dir,\n        ) {\n",
            "            None,\n        ) {\n", T_LAUNCH, "rust"),
    Control("p28_launch_base_is_plugin", LAUNCH, "            Path::new(&args[at + 1]),\n",
            '            &plugin_dir(home).join("settings.json"),\n', T_GENERATED, "rust"),
    Control("p28_launch_error_fails", LAUNCH, "            Err(why) => tracing::warn!(\n",
            "            Err(why) => panic!(\n", T_BROKEN, "rust"),
    # §3: the worker's argv and its error line.
    Control("p28_worker_preset_not_found", WORKER,
            "    let preset = crate::nirvana_preset::worker_settings_in(dir, cwd, root);\n",
            "    let preset: Option<PathBuf> = None;\n", T_W_ARGV, "rust"),
    Control("p28_worker_rule_skipped", WORKER,
            "    let settings = worker_settings(dir, spec, cwd, preset, claude_dir, out);\n",
            "    let settings = preset;\n    let _ = (claude_dir, &out);\n", T_W_ARGV, "rust"),
    Control("p28_worker_cli_ignored", WORKER, "        &spec.cli,\n        &spec.model,\n",
            '        "claude",\n        &spec.model,\n', T_W_SETTINGS, "rust"),
    Control("p28_worker_fixed_model", WORKER, "        &spec.cli,\n        &spec.model,\n",
            '        &spec.cli,\n        "claude-sonnet-5-5",\n', T_W_SETTINGS, "rust"),
    Control("p28_worker_error_drops_preset", WORKER, "            );\n            preset\n        }\n",
            "            );\n            let _ = preset;\n            None\n        }\n", T_W_SETTINGS, "rust"),
    Control("p28_worker_error_silent", WORKER,
            '                "superpowers stay as the settings say for {}: {why}",\n',
            '                "{}{why}",\n', T_W_SETTINGS, "rust"),
)
