# Superpowers off on every Nirvana Claude start, Sonnet included (package `nosp`,
# nirvana/plans/no-superpowers-2026-11-04.md). The owner, 2026-11-04: "I would take out superpowers it doesnt feel
# needed", and, asked where: "Everywhere". Controls starting with nosp_. One sabotage per assertion, each on a product
# line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
#
# The first block repoints plan 28's controls whose line or test this change moved (the old name is in the comment);
# the second states the new rule: a Sonnet keep put back, on any path, goes red.
RULE = "src/nirvana_superpowers.rs"
LAUNCH = "src/nirvana_launch.rs"
WORKER = "src/nirvana_worker.rs"
PRESET = "src/nirvana_preset.rs"
PRELOAD = "src/nirvana_preload.toml"

T_RULE = "the_rule_turns_every_superpowers_id_off"
T_LAUNCH_FILE = "a_launch_writes_the_merged_file_at_the_folders_one_path"
T_WORKER_FILE = "a_claude_worker_gets_the_rule_with_or_without_a_preset_and_agy_is_untouched"
T_LAUNCH = "an_opus_seat_launches_with_every_superpowers_off_in_its_one_settings_file"
T_SONNET = "a_sonnet_seat_launches_with_every_superpowers_off_too"
T_RESTART = "restart_claude_passes_the_superpowers_launch_file_in_every_mode"
T_SERVER = "nirvana_resumes_the_folders_last_session_with_its_preset_file_and_fresh_never_resumes"
T_W_SETTINGS = "a_claude_worker_on_any_model_starts_with_superpowers_off_and_an_error_keeps_the_preset"
T_W_ARGV = "the_workers_argv_carries_the_preset_under_the_rule_on_every_claude_model"
T_CORE = "the_default_presets_parse_in_the_files_order_with_their_core_tags_and_rules"
T_IS_CORE = "names_rules_and_core_match_as_written"
T_SEED = "the_name_rules_seed_the_catalog_for_free_and_leave_core_and_the_rest_to_jev"
T_V3 = "an_installed_default_with_superpowers_in_core_upgrades_to_one_without"

SONNET_KEEP_LAUNCH = ("            Ok(file) if !seat_model.contains(\"sonnet\") => args[at + 1] = file.display().to_string(),\n"
                      "            Ok(_) => {}\n")
SONNET_KEEP_WORKER = '        if spec.model.contains("sonnet") { "agy" } else { spec.cli.as_str() },\n        folder,\n'

CONTROLS = (
    # Repointed from plan 28. The rule's entries are every id off (was p28_opus_entries_on).
    Control("nosp_entries_on", RULE, "    ids.iter().map(|id| (id.clone(), false)).collect()\n",
            "    ids.iter().map(|id| (id.clone(), true)).collect()\n", T_RULE, "rust"),
    # Nirvana Claude's launch file: never from a missing base, at the folder's one path, merged (was
    # p28_launch_missing_base_taken, p28_launch_file_unmerged; the family-free path is new).
    Control("nosp_launch_missing_base_taken", RULE,
            '        read_json(base).ok_or_else(|| format!("{} is missing or not JSON", base.display()))?;\n',
            "        read_json(base).unwrap_or_else(|| Value::Object(Map::new()));\n", T_LAUNCH_FILE, "rust"),
    Control("nosp_launch_path_has_family", RULE, '    dir.join(LAUNCH_DIR).join(format!(\n        "{}.settings.json",\n',
            '    dir.join(LAUNCH_DIR).join(format!(\n        "{}.opus.settings.json",\n', T_LAUNCH_FILE, "rust"),
    Control("nosp_launch_file_unmerged", RULE,
            "    let path = launch_settings_path(dir, folder);\n"
            "    write_json(&path, &merged(&base_value, &entries)?)?;\n",
            "    let path = launch_settings_path(dir, folder);\n"
            "    write_json(&path, &base_value)?;\n    let _ = entries;\n",
            T_LAUNCH_FILE, "rust"),
    # A Claude worker's file, with or without a preset; agy untouched (was p28_worker_agy_touched,
    # p28_worker_no_preset_no_file, p28_worker_broken_preset_taken, p28_worker_shares_launch_path,
    # p28_worker_file_unmerged).
    Control("nosp_worker_agy_touched", RULE,
            '    if cli != "claude" {\n        return Ok(preset.map(Path::to_path_buf));\n    }\n',
            "    let _ = cli;\n", T_WORKER_FILE, "rust"),
    Control("nosp_worker_no_preset_no_file", RULE, "        None => Value::Object(Map::new()),\n",
            "        None => return Ok(None),\n", T_WORKER_FILE, "rust"),
    Control("nosp_worker_broken_preset_taken", RULE,
            '            read_json(file).ok_or_else(|| format!("{} is missing or not JSON", file.display()))?\n',
            "            read_json(file).unwrap_or_else(|| Value::Object(Map::new()))\n", T_WORKER_FILE, "rust"),
    Control("nosp_worker_shares_launch_path", RULE, '        "{}.worker.settings.json",\n',
            '        "{}.settings.json",\n', T_WORKER_FILE, "rust"),
    Control("nosp_worker_file_unmerged", RULE,
            "    let path = worker_settings_path(dir, folder);\n"
            "    write_json(&path, &merged(&base_value, &entries)?)?;\n",
            "    let path = worker_settings_path(dir, folder);\n"
            "    write_json(&path, &base_value)?;\n    let _ = entries;\n",
            T_WORKER_FILE, "rust"),
    # launch_args passes the launch file in its one --settings, and New Nirvana gets it (was
    # p28_launch_file_not_passed, p28_new_space_no_launch_file).
    Control("nosp_launch_file_not_passed", LAUNCH,
            "            Ok(file) => args[at + 1] = file.display().to_string(),\n",
            "            Ok(_file) => {}\n", T_LAUNCH, "rust"),
    Control("nosp_new_space_no_launch_file", LAUNCH,
            "            Ok(file) => args[at + 1] = file.display().to_string(),\n",
            "            Ok(_file) => {}\n", T_SERVER, "rust"),
    # The worker's argv and its error line (was p28_worker_preset_not_found, p28_worker_rule_skipped,
    # p28_worker_cli_ignored, p28_worker_error_drops_preset, p28_worker_error_silent).
    Control("nosp_worker_preset_not_found", WORKER,
            "    let preset = crate::nirvana_preset::worker_settings_in(dir, cwd, root);\n",
            "    let preset: Option<PathBuf> = None;\n", T_W_ARGV, "rust"),
    Control("nosp_worker_rule_skipped", WORKER,
            "    let settings = worker_settings(dir, spec, cwd, preset, claude_dir, out);\n",
            "    let settings = preset;\n    let _ = (claude_dir, &out);\n", T_W_ARGV, "rust"),
    Control("nosp_worker_cli_ignored", WORKER, "        &spec.cli,\n        folder,\n",
            '        "claude",\n        folder,\n', T_W_SETTINGS, "rust"),
    Control("nosp_worker_error_drops_preset", WORKER, "            );\n            preset\n        }\n",
            "            );\n            let _ = preset;\n            None\n        }\n", T_W_SETTINGS, "rust"),
    Control("nosp_worker_error_silent", WORKER,
            '                "superpowers stay as the settings say for {}: {why}",\n',
            '                "{}{why}",\n', T_W_SETTINGS, "rust"),

    # The new rule: plan 28's Sonnet keep put back on Nirvana Claude's launch, on Restart Claude, on a worker.
    Control("nosp_launch_sonnet_keep", LAUNCH,
            "            Ok(file) => args[at + 1] = file.display().to_string(),\n",
            SONNET_KEEP_LAUNCH, T_SONNET, "rust"),
    Control("nosp_restart_sonnet_keep", LAUNCH,
            "            Ok(file) => args[at + 1] = file.display().to_string(),\n",
            SONNET_KEEP_LAUNCH, T_RESTART, "rust"),
    Control("nosp_worker_sonnet_keep", WORKER, "        &spec.cli,\n        folder,\n",
            SONNET_KEEP_WORKER, T_W_SETTINGS, "rust"),
    Control("nosp_worker_argv_sonnet_keep", WORKER, "        &spec.cli,\n        folder,\n",
            SONNET_KEEP_WORKER, T_W_ARGV, "rust"),
    # Preload: superpowers is not core in the default, and an unedited v3 (the last with it) upgrades.
    Control("nosp_preload_core_superpowers", PRELOAD, 'core = ["nirvana", "bladebro", "context7"]\n',
            'core = ["nirvana", "bladebro", "superpowers", "context7"]\n', T_CORE, "rust"),
    Control("nosp_preload_superpowers_is_core", PRELOAD, 'core = ["nirvana", "bladebro", "context7"]\n',
            'core = ["nirvana", "bladebro", "superpowers", "context7"]\n', T_IS_CORE, "rust"),
    Control("nosp_preload_superpowers_never_tagged", PRELOAD, 'core = ["nirvana", "bladebro", "context7"]\n',
            'core = ["nirvana", "bladebro", "superpowers", "context7"]\n', T_SEED, "rust"),
    Control("nosp_preload_v3_not_historical", PRESET, '    include_str!("nirvana_preload_v3.toml"),\n', "",
            T_V3, "rust"),
)
