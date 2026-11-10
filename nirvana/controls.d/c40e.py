# Order P40e SHELL (plan 40, 2026-11-09): model labels from the model id, installer and phone fixes, the gate runner and
# the temp cleaner. Controls starting with c40e_. One sabotage per assertion, each on a product line; the test named
# must print its red line with it in place (written before the run). `Control` is provided by controls.py.
SPACES = "src/nirvana_spaces.rs"
SUBAGENTS = "src/nirvana_subagents.rs"
SEND = "src/nirvana_remote/send.rs"
ROUTES = "src/nirvana_remote/routes.rs"
RUNNING = "src/nirvana_running.rs"
DEAD = "src/nirvana_dead.rs"
SHELL_PY = "nirvana/windows_shell.py"
INSTALL_PY = "nirvana/install.py"
CONTROLS_PY = "nirvana/controls.py"
GATE_PY = "nirvana/gate.py"
CLEAN_PY = "nirvana/clean_temp.py"

T_TABLE = "model_label_table_reads_every_id_right"
T_CLI = "labels_come_from_the_model_id_whatever_cli_runs_it"
T_SEATED = "a_seated_claude_order_through_agy_reads_as_claude"
T_PIPE = "a_child_answer_larger_than_a_pipe_is_read_whole"
T_EMPTY = "a_zero_byte_upload_is_refused_and_leaves_no_file"
T_NOTE = "a_sent_photo_carries_a_note_naming_herdr_look"
T_FORGE = "the_note_comes_from_the_servers_name_and_bytes_not_the_phones_text"
T_PAGE = "the_page_has_the_cards_and_the_new_space_sheet"
T_RUN = "running_drops_the_tool_when_the_transcript_cannot_be_read"
T_PARTIAL = "the_sweep_removes_old_upload_partials_and_never_a_finished_upload"

WS = "nirvana.test_windows_shell."
SC = "nirvana.test_scripts."

CONTROLS = (
    # B1, M2, M3, D4: the label comes from the model id, whatever CLI runs it.
    Control("c40e_label_effort_cut", SPACES,
            "            if EFFORT_WORDS.contains(&tail.to_ascii_lowercase().as_str()) {\n",
            "            if false && EFFORT_WORDS.contains(&tail.to_ascii_lowercase().as_str()) {\n",
            T_TABLE, "rust"),
    Control("c40e_label_agy_claude", SPACES,
            "            Some(model) => claude_label(model).unwrap_or_else(|| model.to_string()),\n",
            "            Some(model) => model.to_string(),\n",
            T_TABLE, "rust"),
    Control("c40e_family_reads_the_label_not_the_id", SPACES,
            "    let label = claude_label(bare).unwrap_or_else(|| bare.to_string());\n",
            "    let label = bare.to_string();\n",
            T_TABLE, "rust"),
    Control("c40e_family_no_four", SPACES,
            "        } else if has(\"argon\") {\n",
            "        } else if has(\"argon\") || lower.contains('4') {\n",
            T_TABLE, "rust"),
    Control("c40e_chip_argon", SPACES,
            "        Family::Gemini(GeminiLine::Argon) => \"Argon\".to_string(),\n",
            "        Family::Gemini(GeminiLine::Argon) => \"Gemini\".to_string(),\n",
            T_TABLE, "rust"),
    Control("c40e_chip_agy_whole", SPACES,
            "        Family::Antigravity => \"Agy\".to_string(),\n",
            "        Family::Antigravity => \"Antigr\".to_string(),\n",
            T_TABLE, "rust"),
    Control("c40e_provider_from_model", SUBAGENTS,
            "    if model.and_then(crate::nirvana_spaces::claude_label).is_some() {\n        SubagentProvider::Claude\n",
            "    if false {\n        SubagentProvider::Claude\n",
            T_CLI, "rust"),
    Control("c40e_format_claude_whatever_provider", SUBAGENTS,
            "    if let Some(label) = model.and_then(crate::nirvana_spaces::claude_label) {\n",
            "    if let Some(label) = model\n        .and_then(crate::nirvana_spaces::claude_label)\n"
            "        .filter(|_| provider == SubagentProvider::Claude)\n    {\n",
            T_CLI, "rust"),
    Control("c40e_gemini_date_dropped", SUBAGENTS,
            "        if numeric && part.len() == 8 {\n",
            "        if numeric && part.is_empty() {\n",
            T_CLI, "rust"),
    Control("c40e_seated_from_model", SUBAGENTS,
            "    let provider = provider_of(&seated.cli, Some(&seated.model));\n",
            "    let provider = if seated.cli == \"agy\" {\n        SubagentProvider::Gemini\n    } else {\n"
            "        SubagentProvider::Claude\n    };\n",
            T_SEATED, "rust"),
    # B8: stdout is drained while the child runs.
    Control("c40e_pipe_drained", SUBAGENTS,
            "    let output = crate::nirvana_dead::output_within(command, timeout).ok()?;\n",
            "    let mut child = command.spawn().ok()?;\n"
            "    let started = Instant::now();\n"
            "    while child.try_wait().ok()?.is_none() {\n"
            "        if started.elapsed() > timeout {\n"
            "            let _ = child.kill();\n"
            "            let _ = child.wait();\n"
            "            return None;\n"
            "        }\n"
            "        std::thread::sleep(Duration::from_millis(50));\n"
            "    }\n"
            "    let output = child.wait_with_output().ok()?;\n",
            T_PIPE, "rust"),
    # B5: a zero-byte upload is refused.
    Control("c40e_empty_upload", SEND,
            "        Ok(0) => {\n",
            "        Ok(0) if false => {\n",
            T_EMPTY, "rust"),
    # Phone photos tell the agent what they are.
    Control("c40e_note_after_path", SEND,
            "            Some(note) => format!(\"{path} ({note})\"),\n",
            "            Some(_) => path,\n",
            T_NOTE, "rust"),
    Control("c40e_note_heic_converts", SEND,
            "            facts.push(\"HEIC image\".into());\n",
            "            facts.push(\"image\".into());\n",
            T_NOTE, "rust"),
    Control("c40e_note_video_probe", SEND,
            "                \"{}; in a Nirvana pane, herdr media probe {quoted} gives its length, size and streams\",\n",
            "                \"{}; {look} reads it and answers in text\",\n",
            T_NOTE, "rust"),
    # Review round N7: the command is named for a Nirvana pane, where `herdr` is on PATH.
    Control("c40e_note_names_nirvana_pane", SEND,
            "    let look = format!(\"in a Nirvana pane, herdr look {quoted} --ask",
            "    let look = format!(\"herdr look {quoted} --ask",
            T_NOTE, "rust"),
    Control("c40e_note_jpeg_size_order", SEND,
            "                return Some((be16(at + 7)?, be16(at + 5)?));\n",
            "                return Some((be16(at + 5)?, be16(at + 7)?));\n",
            T_NOTE, "rust"),
    Control("c40e_note_by_server_extension", SEND,
            "        \"mp4\" | \"mov\" => Some(Media::Video),\n        _ => None,\n",
            "        \"mp4\" | \"mov\" => Some(Media::Video),\n        _ => Some(Media::Image),\n",
            T_FORGE, "rust"),
    Control("c40e_note_needs_the_file", SEND,
            "    let meta = std::fs::metadata(file)\n        .ok()\n        .filter(std::fs::Metadata::is_file)?;\n"
            "    let size = Some(human_bytes(meta.len()));\n",
            "    let size = std::fs::metadata(file).ok().map(|meta| human_bytes(meta.len()));\n",
            T_FORGE, "rust"),
    # D3: one table serves the phone page's files.
    Control("c40e_one_asset_table", ROUTES,
            "        \"/cards.js\",\n        \"text/javascript; charset=utf-8\",\n",
            "        \"/cards-gone.js\",\n        \"text/javascript; charset=utf-8\",\n",
            T_PAGE, "rust"),
    # B7: a failed read clears the tool name.
    Control("c40e_running_failed_read_clears", RUNNING,
            "    if entry.activity == read {\n",
            "    if entry.activity == read || read.is_none() {\n",
            T_RUN, "rust"),
    # D1: the sweep removes old upload partials only.
    Control("c40e_partials_wired", DEAD,
            "    lines.extend(sweep_partials(&folders, now, pass.dry_run));\n",
            "    lines.extend(sweep_partials(&[], now, pass.dry_run));\n",
            T_PARTIAL, "rust"),
    Control("c40e_partials_dot_not_followed", DEAD,
            "        if !send::plain_dir(&dot) || !send::plain_dir(&uploads) {\n",
            "        if !send::plain_dir(&uploads) {\n",
            "the_sweep_does_not_follow_a_junction_at_the_dot_folder", "rust"),
    Control("c40e_partials_age", DEAD,
            "pub const PARTIAL_AGE_S: u64 = 3600;\n",
            "pub const PARTIAL_AGE_S: u64 = 0;\n",
            T_PARTIAL, "rust"),
    Control("c40e_partials_never_finished", DEAD,
            "            let Some(upload) = name.to_str().and_then(|name| name.strip_suffix(\".partial\")) else {\n",
            "            let Some(upload) = name.to_str().map(|name| name.strip_suffix(\".partial\").unwrap_or(name)) else {\n",
            T_PARTIAL, "rust"),

    # B2: a failed font download does not stop the install.
    Control("c40e_font_failure_caught", SHELL_PY,
            "        except (OSError, ValueError, zipfile.BadZipFile, KeyError) as err:\n",
            "        except (ValueError,) as err:\n",
            WS + "FontFailureTest.test_a_failed_font_download_does_not_stop_the_install", "python"),
    # B3: the plugin tree is copied beside, then takes the name.
    Control("c40e_tree_copied_beside", SHELL_PY,
            "    new = dst.with_name(f\"{dst.name}.new-{tag}\")\n",
            "    shutil.rmtree(dst, ignore_errors=True)\n    new = dst.with_name(f\"{dst.name}.new-{tag}\")\n",
            WS + "ReplaceTreeTest.test_a_copy_that_fails_midway_leaves_the_old_tree_whole", "python"),
    Control("c40e_tree_overlay_prunes", SHELL_PY,
            "                elif mirror is None or not (mirror.is_file() and not _is_link(mirror)):\n"
            "                    os.unlink(path)\n",
            "                elif mirror is None or not (mirror.is_file() and not _is_link(mirror)):\n"
            "                    pass\n",
            WS + "ReplaceTreeTest.test_a_folder_that_cannot_be_renamed_is_overlaid_in_place", "python"),
    # Review round N5: the overlay names every stale file, copies beside then renames, never enters a link, and
    # removes only its own leftovers; install.py exits non-zero on a stale folder.
    Control("c40e_overlay_names_stale", SHELL_PY,
            "        if stale:\n            raise StaleFiles(stale)\n",
            "        if False:\n            raise StaleFiles(stale)\n",
            WS + "ReplaceTreeTest.test_a_folder_that_cannot_be_renamed_is_overlaid_in_place", "python"),
    Control("c40e_overlay_beside_then_rename", SHELL_PY,
            "                _copy_file(entry.path, tmp)\n                os.replace(tmp, target)\n",
            "                _copy_file(entry.path, target)\n",
            WS + "ReplaceTreeTest.test_a_folder_that_cannot_be_renamed_is_overlaid_in_place", "python"),
    Control("c40e_overlay_link_inside", SHELL_PY,
            "                if _is_link(path):\n                    if mirror is None or not os.path.lexists(mirror):\n",
            "                if False:\n                    if mirror is None or not os.path.lexists(mirror):\n",
            WS + "ReplaceTreeTest.test_the_overlay_removes_a_junction_inside_as_a_link", "python"),
    Control("c40e_own_leftovers_only", SHELL_PY,
            "    own = re.compile(rf\"{re.escape(dst.name)}\\.(?:new|old)-\\d+\")\n",
            "    own = re.compile(rf\"{re.escape(dst.name)}\\.(?:new|old)-.*\")\n",
            WS + "ReplaceTreeTest.test_only_this_installs_own_leftovers_are_removed_beside_the_folder", "python"),
    Control("c40e_plugin_settings_then_stale", SHELL_PY,
            "    if stale is not None:\n        raise stale\n",
            "    if False:\n        raise stale\n",
            WS + "ReplaceTreeTest.test_install_plugin_writes_the_settings_and_then_says_the_folder_is_stale", "python"),
    Control("c40e_install_stale_exit", INSTALL_PY,
            "    if stale:\n        print(f\"install: {len(stale)}",
            "    if False:\n        print(f\"install: {len(stale)}",
            SC + "InstallTest.test_a_stale_plugin_folder_finishes_the_install_and_exits_non_zero", "python"),
    # B4: a failed copy puts the old binary back.
    Control("c40e_binary_put_back", INSTALL_PY,
            "            os.replace(aside, target)\n",
            "            pass\n",
            SC + "InstallTest.test_a_failed_copy_puts_the_old_binary_back", "python"),
    # B6: the original goes to disk before the sabotage, and every run starts with the repair.
    Control("c40e_controls_backup_first", CONTROLS_PY,
            "    _save_original(c, original, sabotaged)\n",
            "    pass\n",
            SC + "ControlsRepairTest.test_the_next_run_repairs_a_killed_runs_sabotage", "python"),
    Control("c40e_controls_repair_at_start", CONTROLS_PY,
            "    for line in repair():\n",
            "    for line in []:\n",
            SC + "ControlsRepairTest.test_every_run_starts_with_the_repair", "python"),
    Control("c40e_controls_edit_kept", CONTROLS_PY,
            "            if hashlib.sha256(now).hexdigest() != sabotaged:\n",
            "            if False:\n",
            SC + "ControlsRepairTest.test_a_file_edited_after_the_kill_is_not_overwritten", "python"),
    # Review round S2: one controls run per worktree, the lock taken before any file is touched.
    Control("c40e_controls_one_run", CONTROLS_PY,
            "            msvcrt.locking(fd, msvcrt.LK_NBLCK, 1)\n",
            "            pass\n",
            SC + "ControlsRepairTest.test_a_second_run_in_the_worktree_is_refused_and_touches_nothing", "python"),
    Control("c40e_controls_lock_before_repair", CONTROLS_PY,
            "    fd, left = take_lock()\n",
            "    for line in repair():\n        print(line, flush=True)\n    fd, left = take_lock()\n",
            SC + "ControlsRepairTest.test_a_second_run_in_the_worktree_is_refused_and_touches_nothing", "python"),
    Control("c40e_controls_takeover_named", CONTROLS_PY,
            "        if left and left.get(\"pid\") != os.getpid():\n",
            "        if False:\n",
            SC + "ControlsRepairTest.test_a_second_run_in_the_worktree_is_refused_and_touches_nothing", "python"),
    # gate.py: a private TEMP, removed; failed names; the allowed failure apart; stale folders.
    Control("c40e_gate_private_env", GATE_PY,
            "        env = dict(os.environ, TEMP=str(private), TMP=str(private), PYTHONIOENCODING=\"utf-8\")\n",
            "        env = dict(os.environ, PYTHONIOENCODING=\"utf-8\")\n",
            SC + "GateTest.test_each_step_runs_in_a_private_temp_that_is_removed_after", "python"),
    Control("c40e_gate_temp_removed", GATE_PY,
            "        failures = clean_temp.remove_tree(private) if os.path.lexists(private) else []\n",
            "        failures = []\n",
            SC + "GateTest.test_an_interrupted_run_still_removes_its_temp", "python"),
    Control("c40e_gate_allowed_apart", GATE_PY,
            "        result.allowed = [n for n in names if any(n == a or n.endswith(\"::\" + a) for a in ALLOWED)]\n",
            "        result.allowed = []\n",
            SC + "GateTest.test_failed_tests_are_named_and_the_allowed_one_is_apart", "python"),
    Control("c40e_gate_allowed_by_path", GATE_PY,
            "        result.allowed = [n for n in names if any(n == a or n.endswith(\"::\" + a) for a in ALLOWED)]\n",
            "        result.allowed = [n for n in names if n.rsplit(\"::\", 1)[-1] in {a.rsplit(\"::\", 1)[-1] for a in ALLOWED}]\n",
            SC + "GateTest.test_failed_tests_are_named_and_the_allowed_one_is_apart", "python"),
    # Review round S1: the private TEMP is a folder this run made; a link at the top is never emptied.
    Control("c40e_gate_mkdtemp", GATE_PY,
            "    private = Path(tempfile.mkdtemp(prefix=f\"ng-{pid}-\", dir=base))\n",
            "    private = base / f\"ng-{pid}\"\n    private.mkdir(parents=True, exist_ok=True)\n",
            SC + "GateTest.test_a_planted_junction_at_the_private_name_is_never_used_or_emptied", "python"),
    Control("c40e_gate_finally_refuses_link", CLEAN_PY,
            "        if is_link(folder):\n            failures.append(",
            "        if False:\n            failures.append(",
            SC + "GateTest.test_a_private_temp_swapped_for_a_junction_mid_run_is_not_emptied", "python"),
    Control("c40e_gate_sweep_top_link", GATE_PY,
            "        if not m or clean_temp.is_link(entry.path) or not entry.is_dir(follow_symlinks=False):\n",
            "        if not m or not entry.is_dir(follow_symlinks=False):\n",
            SC + "GateTest.test_the_stale_sweep_leaves_a_junction_at_the_top_as_it_is", "python"),
    # Review round N2: an interrupt or a killed gate ends the step's whole tree.
    Control("c40e_gate_tree_ended", GATE_PY,
            "    except BaseException:\n        tree.end()\n",
            "    except BaseException:\n        pass\n",
            SC + "GateTest.test_an_interrupt_ends_the_whole_tree_before_the_temp_goes", "python"),
    Control("c40e_gate_job_kill_on_close", GATE_PY,
            "    info.basic.flags = 0x00002000",
            "    info.basic.flags = 0",
            SC + "GateTest.test_a_killed_gate_takes_its_tree_with_it", "python"),
    Control("c40e_gate_names_failures", GATE_PY,
            "        if m and m[1] not in seen:\n",
            "        if False:\n",
            SC + "GateTest.test_failed_tests_are_named_and_the_allowed_one_is_apart", "python"),
    Control("c40e_gate_stale_dead_only", GATE_PY,
            "        if age < STALE_S or alive(int(m[1])):\n",
            "        if age < STALE_S:\n",
            SC + "GateTest.test_stale_private_temps_go_only_when_old_and_dead", "python"),
    # clean_temp.py: only old, dead, pid-named folders; never through a link; a folder in use stays whole.
    Control("c40e_clean_never_through_a_link", CLEAN_PY,
            "                if is_link(path):\n                    _remove_link(path)\n",
            "                if False:\n                    _remove_link(path)\n",
            SC + "CleanTempTest.test_apply_removes_only_the_removable_and_never_through_a_junction", "python"),
    Control("c40e_clean_top_link_skipped", CLEAN_PY,
            "            if is_link(path):\n                item.why = \"a link or junction: left as it is\"\n",
            "            if False:\n                item.why = \"a link or junction: left as it is\"\n",
            SC + "CleanTempTest.test_the_dry_run_removes_nothing_and_says_why_each_stays", "python"),
    Control("c40e_clean_age", CLEAN_PY,
            "            if age < limit:\n",
            "            if age < 0:\n",
            SC + "CleanTempTest.test_the_dry_run_removes_nothing_and_says_why_each_stays", "python"),
    Control("c40e_clean_pid_at_the_end", CLEAN_PY,
            "(?:\\.cleaning-\\d{1,7})?$\"\n",
            "(?:\\.cleaning-\\d{1,7})?\"\n",
            SC + "CleanTempTest.test_the_dry_run_removes_nothing_and_says_why_each_stays", "python"),
    Control("c40e_clean_live_pid", CLEAN_PY,
            "            if m.re.groupindex.get(\"pid\") and alive(int(m[\"pid\"])):\n",
            "            if False:\n",
            SC + "CleanTempTest.test_the_dry_run_removes_nothing_and_says_why_each_stays", "python"),
    # Review round S1: the link test where the delete is.
    Control("c40e_clean_tree_top_refused", CLEAN_PY,
            "        if is_link(folder):\n            failures.append(",
            "        if False:\n            failures.append(",
            SC + "CleanTempTest.test_remove_tree_refuses_a_junction_at_its_top", "python"),
    Control("c40e_clean_recheck_before_rename", CLEAN_PY,
            "        if is_link(item.path):\n",
            "        if False:\n",
            SC + "CleanTempTest.test_a_name_swapped_for_a_junction_after_the_scan_is_left", "python"),
    Control("c40e_clean_recheck_after_rename", CLEAN_PY,
            "        if is_link(tomb):\n",
            "        if False:\n",
            SC + "CleanTempTest.test_a_name_swapped_between_the_check_and_the_rename_is_renamed_back", "python"),
    # Review round: the two named makers.
    Control("c40e_clean_named_rules", CLEAN_PY,
            "    for label, rule in NAMED:\n        m = rule.match(name)\n",
            "    for label, rule in ():\n        m = rule.match(name)\n",
            SC + "CleanTempTest.test_the_dry_run_removes_nothing_and_says_why_each_stays", "python"),
    Control("c40e_clean_named_label", CLEAN_PY,
            "        if rule.match(name):\n            return label\n",
            "        if rule.match(name):\n            break\n",
            SC + "CleanTempTest.test_the_dry_run_removes_nothing_and_says_why_each_stays", "python"),
    Control("c40e_clean_fake_daemon_shape", CLEAN_PY,
            "^nirvana-fake-daemon-[a-z0-9_]{8}",
            "^nirvana-fake-daemon-[A-Za-z0-9_]+",
            SC + "CleanTempTest.test_the_named_patterns_take_only_their_own_shape", "python"),
    Control("c40e_clean_p13a_shape", CLEAN_PY,
            "^nirvana-p13a-(?P<pid>\\d{1,7})-\\d+-",
            "^nirvana-p13a-(?P<pid>\\d{1,7})-\\d*-?",
            SC + "CleanTempTest.test_the_named_patterns_take_only_their_own_shape", "python"),
    Control("c40e_clean_rename_first", CLEAN_PY,
            "                os.rename(item.path, tomb)\n",
            "                tomb = item.path\n",
            SC + "CleanTempTest.test_a_folder_in_use_is_skipped_whole", "python"),
    Control("c40e_clean_dry_by_default", CLEAN_PY,
            "    if not ns.apply:\n        return 0\n",
            "    if False:\n        return 0\n",
            SC + "CleanTempTest.test_main_is_a_dry_run_by_default", "python"),
)
