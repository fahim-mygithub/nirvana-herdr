# Plan 28: superpowers only on Sonnet (package `superpowers_by_model`, nirvana/plans/2026-10-23-28-records/p28.md).
# Controls starting with p28_. One sabotage per assertion, each on a product line; the test named must print FAIL
# with it in place. `Control` is provided by controls.py.
#
# 2026-11-04 (nirvana/plans/no-superpowers-2026-11-04.md): the owner's "Everywhere" replaced the Sonnet keep. The
# controls that stated it, or the model families that served it, are retired: p28_opus_not_opus,
# p28_sonnet_not_sonnet, p28_label_case_kept, p28_sonnet_gets_entries, p28_launch_file_for_sonnet,
# p28_launch_path_no_family, p28_worker_sonnet_touched, p28_launch_fixed_model, p28_restart_ignores_mode,
# p28_sonnet_launch_gets_file, p28_worker_fixed_model. The ones whose line or test moved are repointed in nosp.py
# under nosp_ names. What is left here still holds as written.
RULE = "src/nirvana_superpowers.rs"
LAUNCH = "src/nirvana_launch.rs"

T_IDS = "the_ids_are_every_plugin_named_superpowers_before_the_at_or_the_default"
T_MERGE = "the_merge_keeps_every_other_key_and_refuses_what_is_not_an_object"
T_IDS_FOR = "the_ids_come_from_the_installed_list_the_folders_settings_and_the_base"
T_NO_HOME = "no_test_reads_the_owners_claude_folder"
T_LAUNCH = "an_opus_seat_launches_with_every_superpowers_off_in_its_one_settings_file"
T_BROKEN = "a_broken_settings_file_keeps_todays_argv"
T_GENERATED = "the_launch_names_the_folders_generated_settings_when_one_was_written"

CONTROLS = (
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
    # §3: launch_args builds the launch file from Claude's config folder and the file it had.
    Control("p28_launch_claude_dir_dropped", LAUNCH, "            claude_dir,\n        ) {\n",
            "            None,\n        ) {\n", T_LAUNCH, "rust"),
    Control("p28_launch_base_is_plugin", LAUNCH, "            Path::new(&args[at + 1]),\n",
            '            &plugin_dir(home).join("settings.json"),\n', T_GENERATED, "rust"),
    Control("p28_launch_error_fails", LAUNCH, "            Err(why) => tracing::warn!(\n",
            "            Err(why) => panic!(\n", T_BROKEN, "rust"),
    # Plan 28 review: a temporary name per write, not per process
    Control("p28_temp_name_per_write", RULE,
            '    path.with_extension(format!("tmp-{}-{n}", std::process::id()))\n',
            '    let _ = n;\n    path.with_extension(format!("tmp-{}", std::process::id()))\n',
            "two_writes_in_one_process_never_share_a_temporary_name", "rust"),
)
