# Plan 13 P1: the smart Preload's engine and launch (nirvana/plans/2026-10-18-13-records/p13a.md). One sabotage per
# assertion; each test named must print FAIL with its sabotage in place. `Control` is provided by controls.py.
PRESET = "src/nirvana_preset.rs"
LAUNCH = "src/nirvana_launch.rs"
NEW = "src/app/nirvana_new.rs"
WORKER = "src/nirvana_worker.rs"
TASKBAR = "src/nirvana_taskbar.rs"
MENU_FILE = "src/client/shell/nirvana_new_menu.rs"
MENU_LABELS = "new_offers_nirvana_claude_code_antigravity_and_a_shell"
FRESH_ENTRY = "the_fresh_entry_asks_for_a_fresh_nirvana_and_claims_the_window"

ORDER = "the_default_presets_parse_in_the_files_order_with_their_core_tags_and_rules"
WRITTEN = "preload_toml_is_written_from_the_default_once_and_the_owners_from_then_on"
FIXTURES = "a_folder_of_3mf_and_stl_reads_3d_print_and_an_empty_one_blank"
MARKER = "a_marker_beats_a_share_and_two_close_shares_go_to_jev"
WALK = "the_walk_skips_build_and_dependency_folders_and_the_hash_moves_with_a_doubling"
FLOOR = "jev_chooses_only_at_the_floor_and_none_unsure_or_a_failure_is_general"
QUESTION = "the_folder_question_carries_counts_markers_and_the_readme_never_a_files_text"
CACHE = "a_cache_miss_asks_jev_once_and_the_same_fingerprint_spends_nothing"
FAILED = "a_failure_or_no_key_is_general_and_asked_again_at_the_next_open"
WALL = "past_the_wall_the_open_goes_ahead_on_general_and_the_answer_is_used_next_time"
SEED = "the_name_rules_seed_the_catalog_for_free_and_leave_core_and_the_rest_to_jev"
OWNER = "herdr_only_appends_so_a_line_the_owner_edited_is_never_retagged"
TAG = "jev_tags_at_the_floor_none_and_unsure_stay_on_and_a_failed_call_writes_nothing"
OFF = "each_fixture_preset_switches_off_what_section_9_expects"
GENERATED = "the_generated_file_is_the_plugins_settings_plus_the_switches_and_the_workers_has_no_env"
APPLY = "apply_writes_both_files_and_nothing_in_the_folder"
UNDO = "undo_regenerates_the_file_without_the_preset_and_p_again_puts_it_back"
PIN = "a_pinned_item_is_left_out_of_the_file_and_unpinning_puts_it_back_off"
VIEW = "the_view_names_the_preset_its_off_switches_and_the_estimate"
READING = "the_reading_lists_switchable_items_with_their_turn_cost"
DESCRIPTION = "a_skills_description_comes_from_its_front_matter"
NAMES = "names_rules_and_core_match_as_written"
LAUNCH_SETTINGS = "the_launch_names_the_folders_generated_settings_when_one_was_written"
RESUME = "a_recent_session_with_its_transcript_resumes_and_anything_else_starts_fresh"
MENU = "the_menu_offers_nirvana_claude_code_and_antigravity_in_that_order"
SERVER = "nirvana_resumes_the_folders_last_session_with_its_preset_file_and_fresh_never_resumes"
WORKER_ARGS = "a_claude_worker_gets_the_folders_preset_file_and_agy_does_not"
TOAST = "the_open_toast_carries_the_presets_line_after_the_folder"

CONTROLS = (
    # preload.toml: file order, the defaults, a typo refused, written once.
    Control("p13a_order_sorted", PRESET, 'line.strip_prefix("[preset.")?', 'line.strip_prefix("[presets.")?',
            ORDER, "rust"),
    Control("p13a_auto_memory_default_off", PRESET, '#[serde(default = "yes")]', "#[serde(default)]", ORDER, "rust"),
    Control("p13a_unknown_preset_key_read", PRESET, "#[serde(deny_unknown_fields)]\nstruct RawPreset {",
            "struct RawPreset {", ORDER, "rust"),
    Control("p13a_default_not_written", PRESET, "let _ = std::fs::write(&path, DEFAULT);", "let _ = &path;",
            WRITTEN, "rust"),
    # The fingerprint and the rules (§3.1, §3.4).
    Control("p13a_git_not_blank", PRESET, 'name == ".claude" || name == ".git"', 'name == ".claude"', FIXTURES, "rust"),
    Control("p13a_share_floor_ignored", PRESET, "*share > 0.0 && *share >= when.share.unwrap_or(0.0)",
            "*share > 0.0", FIXTURES, "rust"),
    Control("p13a_claude_dir_walked", PRESET, '".venv", ".claude"];', '".venv", ".venv"];', FIXTURES, "rust"),
    Control("p13a_marker_not_first", PRESET, 'return chosen(preset, format!("rule: marker {marker}"));',
            "let _ = (preset, marker);", MARKER, "rust"),
    Control("p13a_no_tie", PRESET, "const SHARE_TIE: f64 = 0.10;", "const SHARE_TIE: f64 = -1.0;", MARKER, "rust"),
    Control("p13a_walks_target", PRESET,
            "if !SKIP_DIRS.contains(&low.as_str()) && !crate::nirvana_jev::excluded_dir(&low) {", "if true {",
            WALK, "rust"),
    Control("p13a_bucket_exact", PRESET, "32 - count.leading_zeros()", "count", WALK, "rust"),
    Control("p13a_git_no_marker", PRESET, 'const MARKERS: [&str; 6] = [\n    ".git",',
            'const MARKERS: [&str; 6] = [\n    ".gitx",', WALK, "rust"),
    # Jev for the folder: the floor, none, the cache, a failure, the wall.
    Control("p13a_folder_floor_ignored", PRESET, "if known && confidence >= CHOICE_FLOOR {", "if known {",
            FLOOR, "rust"),
    Control("p13a_unknown_preset_taken", PRESET, "let known = choice != NONE && config.preset(&choice).is_some();",
            "let known = choice != NONE;", FLOOR, "rust"),
    Control("p13a_blank_offered", PRESET, ".filter(|when| !when.blank)", ".filter(|_| true)", QUESTION, "rust"),
    Control("p13a_cache_ignored", PRESET, "old.fingerprint == hash && !old.retry",
            "false && old.fingerprint == hash && !old.retry", CACHE, "rust"),
    Control("p13a_folder_role_renamed", PRESET, 'pub const ROLE_FOLDER: &str = "preload-folder";',
            'pub const ROLE_FOLDER: &str = "seating";', CACHE, "rust"),
    Control("p13a_failure_cached", PRESET, 'let retry = choice.by == "failed";', "let retry = false;",
            FAILED, "rust"),
    Control("p13a_late_answer_lost", PRESET, "*waiting = Waiting::Abandoned;", "let _ = &waiting;", WALL, "rust"),
    # The catalog: rules first match, append only, the owner's lines, Jev's tags.
    Control("p13a_rule_order", PRESET, ".find(|rule| glob_match(&rule.pattern, key))",
            ".rfind(|rule| glob_match(&rule.pattern, key))", SEED, "rust"),
    Control("p13a_rewrites_lines", PRESET, ".filter(|(key, _, _)| !have.entries.contains_key(key))",
            ".filter(|_| true)", OWNER, "rust"),
    Control("p13a_owner_read_as_rule", PRESET, '_ => "owner".into(),', '_ => "rule".into(),', OWNER, "rust"),
    Control("p13a_tag_floor_ignored", PRESET, "known.contains(choice) && confidence >= CHOICE_FLOOR",
            "known.contains(choice)", TAG, "rust"),
    Control("p13a_tag_unoffered_taken", PRESET, "choice != NONE && known.contains(choice)", "choice != NONE",
            TAG, "rust"),
    Control("p13a_call_cap_ignored", PRESET, "if out.calls >= max_calls {", "if false {", TAG, "rust"),
    # What goes off (§3.3) and the generated files (§2).
    Control("p13a_core_switched_off", PRESET, "                || is_core(config, key)\n                || pinned",
            "                || pinned", OFF, "rust"),
    Control("p13a_keep_ignored", PRESET,
            "                || entry.tags.iter().any(|tag| preset.keep.contains(tag))\n", "", OFF, "rust"),
    Control("p13a_uninstalled_off", PRESET, "if reading.is_some_and(|reading| !reading.items.contains_key(key)) {",
            "if false {", OFF, "rust"),
    Control("p13a_pin_ignored", PRESET, "                || pinned.contains(key)\n", "", OFF, "rust"),
    Control("p13a_auto_memory_inverted", PRESET, "if !preset.auto_memory && !pinned",
            "if preset.auto_memory && !pinned", OFF, "rust"),
    Control("p13a_switch_left_on", PRESET, "crate::nirvana_preload::set(&mut value, &switch, false);",
            "crate::nirvana_preload::set(&mut value, &switch, true);", GENERATED, "rust"),
    Control("p13a_worker_gets_plugin_env", PRESET, "&generated(&Value::Object(Map::new()), &outcome.off),",
            "&generated(&plugin_settings(dir), &outcome.off),", APPLY, "rust"),
    Control("p13a_toast_no_undo", PRESET, 'Preload p to undo",', 'Preload p",', APPLY, "rust"),
    Control("p13a_undo_keeps_preset", PRESET, "let off = if record.preset == OFF {", "let off = if false {",
            UNDO, "rust"),
    Control("p13a_view_ignores_undo", PRESET, "        name: if record.preset == OFF {", "        name: if false {",
            UNDO, "rust"),
    Control("p13a_pin_not_removed", PRESET, "record.pinned_on.retain(|pinned| *pinned != key);",
            "record.pinned_on.retain(|_| true);", PIN, "rust"),
    Control("p13a_view_estimate_zero", PRESET, "        saved_tokens: saved,", "        saved_tokens: 0,",
            VIEW, "rust"),
    # The reading of /context.
    Control("p13a_plugin_skills_listed", PRESET, '.filter(|skill| skill.source != "plugin")', ".filter(|_| true)",
            READING, "rust"),
    Control("p13a_needs_auth_costed", PRESET, 'tokens: if server.extra == "connected" {', "tokens: if true {",
            READING, "rust"),
    Control("p13a_builtin_plugins_listed", PRESET, 'if plugin.source.ends_with("@builtin") {', "if false {",
            READING, "rust"),
    Control("p13a_folded_description_dropped", PRESET, "out.push_str(line.trim());", "let _ = line;",
            DESCRIPTION, "rust"),
    Control("p13a_glob_suffix_ignored", PRESET, "return rest.ends_with(part);", "return true;", NAMES, "rust"),
    Control("p13a_core_by_full_key", PRESET, ".any(|core| core.eq_ignore_ascii_case(&name))",
            ".any(|core| core.eq_ignore_ascii_case(key))", NAMES, "rust"),
    # The launch: the generated file, the resume (§6.1), the fresh choice.
    Control("p13a_generated_ignored", LAUNCH, "    if generated.is_file() {", "    if false {",
            LAUNCH_SETTINGS, "rust"),
    Control("p13a_resume_not_typed", LAUNCH, 'args.extend(["--resume".to_owned(), id.to_owned()]);',
            "let _ = id;", LAUNCH_SETTINGS, "rust"),
    Control("p13a_transcript_unchecked", LAUNCH, "        || !Path::new(&record.transcript).is_file()\n", "",
            RESUME, "rust"),
    Control("p13a_age_unchecked", LAUNCH, "        || (age as f64) >= within_hours * 3600.0\n", "", RESUME, "rust"),
    Control("p13a_session_id_unchecked", LAUNCH, "    if !plain_session_id(&record.session_id)\n",
            "    if false\n", RESUME, "rust"),
    Control("p13a_fresh_own_name", LAUNCH, "Self::NirvanaFresh => Self::Nirvana.key(),",
            "Self::NirvanaFresh => self.key(),", MENU, "rust"),
    Control("p13a_second_nirvana_resumes", NEW, "if self.nirvana_space_on(folder).is_some() {", "if false {",
            SERVER, "rust"),
    Control("p13a_fresh_resumes", NEW, "launch_args(&home, folder, None)",
            "launch_args(&home, folder, self.nirvana_resume(&home, folder).as_deref())", SERVER, "rust"),
    Control("p13a_plugin_settings_kept", NEW,
            "Some(crate::nirvana_launch::launch_args(\n                    &home,\n                    folder,\n                    resume.as_deref(),\n                ))",
            "{ let _ = resume; Some(kind.args(&home)) }", SERVER, "rust"),
    # The "new" menu's fresh entry (the team lead gave P1 the two client menu files, 2026-10-18).
    Control("p13a_fresh_not_listed", MENU_FILE, "        NewKind::AFTER_SHELL\n            .iter()",
            "        NewKind::AFTER_SHELL[..0]\n            .iter()", MENU_LABELS, "rust"),
    Control("p13a_fresh_entry_missing", MENU_FILE, "        NewKind::AFTER_SHELL\n            .iter()",
            "        NewKind::AFTER_SHELL[..0]\n            .iter()", FRESH_ENTRY, "rust"),
    Control("p13a_fresh_no_claim", MENU_FILE, "let kind = if kind == NewKind::NirvanaFresh {",
            "let kind = if false {", FRESH_ENTRY, "rust"),
    Control("p13a_fresh_key_unknown", LAUNCH, ".chain(Self::AFTER_SHELL)",
            ".chain(Self::AFTER_SHELL.into_iter().take(0))", MENU, "rust"),
    # The worker and the toast.
    Control("p13a_agy_gets_settings", WORKER, 'if argv.first().map(String::as_str) == Some("claude") {', "if true {",
            WORKER_ARGS, "rust"),
    Control("p13a_worker_no_settings", WORKER, 'argv.extend(["--settings".to_owned(), file.display().to_string()]);',
            "let _ = file;", WORKER_ARGS, "rust"),
    Control("p13a_toast_drops_line", TASKBAR, r'Some(line) => format!("{title} \u{b7} {line}"),', "Some(_) => title,",
            TOAST, "rust"),
)
