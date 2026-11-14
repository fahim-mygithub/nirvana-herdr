# Plan 41 P41c: the agy orchestrator's plugin built from nirvana/claude and installed with `agy plugin install`
# (nirvana/agy_plugin.py and its call in nirvana/install.py). Controls starting with p78_agy. One sabotage per assertion, each on a product line; the
# python test named must print `FAIL: <test>` with it in place (the expected red test is the fifth field, written
# before the run). `Control` is provided by controls.py.
PLUGIN = "nirvana/agy_plugin.py"
INSTALL = "nirvana/install.py"

_T = "nirvana.test_agy_plugin."
T_HOOKS = _T + "AgyHooksTest.test_every_claude_hook_runs_the_installed_herdr_with_host_agy_on_its_agy_event"
T_SHAPE = _T + "AgyHooksTest.test_flat_events_are_handler_lists_and_tool_events_are_matcher_groups"
T_MATCH = _T + "AgyHooksTest.test_claude_tool_matchers_become_agy_tool_names"
T_UNKNOWN = _T + "AgyHooksTest.test_a_claude_tool_with_no_agy_row_fails_the_build"
T_NOT_HERDR = _T + "AgyHooksTest.test_a_hook_that_is_not_the_installed_herdr_fails_the_build"
T_SKILLS = _T + "AgyFilesTest.test_skills_are_copied_with_the_installed_herdr_in_place_of_the_plugin_root"
T_SKILL_NAME = _T + "AgyFilesTest.test_a_skill_named_unlike_its_folder_fails_the_build"
T_RULE = _T + "AgyFilesTest.test_the_role_is_an_always_on_rule_holding_role_md"
T_AGENT = _T + "AgyFilesTest.test_only_the_orchestrator_agent_maps_as_agys_main_agent_with_a_body"
T_BLADE = _T + "AgyFilesTest.test_bladebro_is_decision_0043s_recipe_with_the_orchestrators_own_home"
T_NO_BLADE = _T + "AgyFilesTest.test_no_bladebro_on_path_writes_no_mcp_config_and_says_so"
T_KEEP = _T + "AgyInstallTest.test_a_failed_validation_keeps_the_installed_folder_and_leaves_no_build"
T_REPLACE = _T + "AgyInstallTest.test_a_passed_validation_replaces_the_folder_whole"
T_MAIN = _T + "AgyInstallTest.test_install_main_builds_the_agy_folder_and_a_failed_one_exits_non_zero"
T_VALIDATE = _T + "AgyValidateTest.test_agys_strict_validator_passes_the_build_and_fails_a_broken_rule"
T_NATIVE = _T + "AgyFilesTest.test_the_build_is_a_native_agy_plugin_named_nirvana_with_hooks_at_its_root"
T_SAME = _T + "AgyFilesTest.test_a_rebuild_writes_the_same_bytes"
_I = _T + "AgyPluginInstallTest."
T_INSTALL = _I + "test_install_runs_after_validate_passes_on_the_installed_folder"
T_PIPE = _I + "test_agy_calls_read_an_empty_pipe_with_no_colour"
T_NO_INSTALL = _I + "test_a_failed_validation_never_calls_agy_plugin_install"
T_MISSING = _I + "test_agy_missing_validates_and_installs_nothing_and_says_so"
T_TEMP = _I + "test_a_real_agy_is_never_handed_a_plugin_from_the_temp_folder"
T_MISSING_MAIN = _I +"test_install_main_with_agy_missing_skips_with_a_line_and_exits_0"
T_PRINTS_MAIN = _I + "test_install_main_prints_the_installed_plugin_name_and_folder"
T_FAIL_MAIN = _I + "test_a_failed_agy_install_exits_1_naming_agys_error_and_the_binary_still_installs"

CONTROLS = (
    # Each hook command ends in `--host agy`, P41b's flag.
    Control("p78_agy_host_flag", PLUGIN,
            'HOST_FLAG = ["--host", "agy"]', 'HOST_FLAG = ["--host", "claude"]',
            T_HOOKS, "python"),
    # UserPromptSubmit lands on agy's PreInvocation.
    Control("p78_agy_prompt_event", PLUGIN,
            '    "UserPromptSubmit": "PreInvocation",\n', '    "UserPromptSubmit": "PostInvocation",\n',
            T_HOOKS, "python"),
    # SubagentStop has no agy event and is said to be skipped.
    Control("p78_agy_subagentstop_said", PLUGIN,
            '                    skipped.append(f"hook {event} `{args}`: agy has no {event} event")\n',
            "",
            T_HOOKS, "python"),
    # Each handler keeps the Claude hook's timeout.
    Control("p78_agy_timeout_kept", PLUGIN,
            '"timeout": int(hook.get("timeout", 30))}', '"timeout": 30}',
            T_HOOKS, "python"),
    # SessionStart, PreInvocation and Stop are flat handler lists, not matcher groups.
    Control("p78_agy_flat_shape", PLUGIN,
            '            spec.setdefault(agy, []).extend(handler(home, h) for g in groups for h in g["hooks"])\n',
            '            spec.setdefault(agy, []).extend({"matcher": "", "hooks": [handler(home, h) for h in g["hooks"]]}'
            ' for g in groups)\n',
            T_SHAPE, "python"),
    # Claude tool names become agy's.
    Control("p78_agy_matcher_translated", PLUGIN,
            '    return "|".join(names)\n', '    return claude\n',
            T_MATCH, "python"),
    # A Claude tool with no row fails the build instead of shipping a matcher that never matches.
    Control("p78_agy_unknown_tool_fails", PLUGIN,
            "        if tool not in TOOLS:\n", "        if False:\n",
            T_UNKNOWN, "python"),
    # A hook command that is not the installed herdr fails the build.
    Control("p78_agy_not_herdr_fails", PLUGIN,
            'if hook.get("type", "command") != "command" or hook.get("command") != CLAUDE_EXE:',
            'if hook.get("type", "command") != "command":',
            T_NOT_HERDR, "python"),
    # The skills name the installed herdr, not `${CLAUDE_PLUGIN_ROOT}`, which agy does not set.
    Control("p78_agy_skill_path", PLUGIN,
            "    return text.replace(CLAUDE_EXE_QUOTED, f'\"{herdr(home).as_posix()}\"')\n",
            "    return text\n",
            T_SKILLS, "python"),
    # A skill whose frontmatter name is not its folder (agy drops it) fails the build.
    Control("p78_agy_skill_name_fails", PLUGIN,
            '        if fields.get("name") != skill.name or not fields.get("description"):\n',
            "        if False:\n",
            T_SKILL_NAME, "python"),
    # ROLE.md is an always-on rule.
    Control("p78_agy_rule_trigger", PLUGIN,
            'RULE_TRIGGER = "always_on"', 'RULE_TRIGGER = "model_decision"',
            T_RULE, "python"),
    # The orchestrator agent is agy's main agent.
    Control("p78_agy_main_agent", PLUGIN,
            '"mainAgent: true", "subagent: false"', '"mainAgent: false", "subagent: true"',
            T_AGENT, "python"),
    # An empty Claude body gets the agy body (measured: an empty one does not take).
    Control("p78_agy_main_body", PLUGIN,
            "        body = herdr_text(body, home) if body.strip() else MAIN_BODY.format(rule=RULE)\n",
            "        body = herdr_text(body, home)\n",
            T_AGENT, "python"),
    # The Claude workers stay out and are listed as skipped.
    Control("p78_agy_workers_skipped", PLUGIN,
            '        if AGENTS.get(name) != "main":\n', "        if False:\n",
            T_AGENT, "python"),
    # bladebro's home is the orchestrator's own, outside the folder every install replaces.
    Control("p78_agy_blade_home", PLUGIN,
            'BLADE_HOME = Path("nirvana") / "blade-orchestrator"', 'BLADE_HOME = Path("nirvana") / "agy" / "blade"',
            T_BLADE, "python"),
    # bladebro keeps decision 0043's headless flag.
    Control("p78_agy_blade_headless", PLUGIN,
            '"BLADE_CHROME_FLAGS": "--headless=new",', '"BLADE_CHROME_FLAGS": "",',
            T_BLADE, "python"),
    # No bladebro on PATH writes no MCP config.
    Control("p78_agy_no_bladebro", PLUGIN,
            "    if with_bladebro:\n", "    if True:\n",
            T_NO_BLADE, "python"),
    # A failed validation leaves the installed folder as it was.
    Control("p78_agy_validate_gates", PLUGIN,
            '        if report["validate"]["ok"] is False:\n', "        if False:\n",
            T_KEEP, "python"),
    # A passed build replaces the folder whole: a file the tree no longer has goes.
    Control("p78_agy_replace_whole", PLUGIN,
            "        windows_shell._replace_tree(stage, dst)\n",
            "        shutil.copytree(stage, dst, dirs_exist_ok=True)\n",
            T_REPLACE, "python"),
    # The manifest carries a version: strict mode warns without one (measured).
    Control("p78_agy_manifest_version", PLUGIN,
            '    return {"name": NAME, "version": version}\n', '    return {"name": NAME}\n',
            T_VALIDATE, "python"),
    # The validator's verdict is agy's exit code: a broken rule fails it.
    Control("p78_agy_validate_exit", PLUGIN,
            '    return {"ok": done.returncode == 0, "why": f"exit {done.returncode}",\n',
            '    return {"ok": True, "why": f"exit {done.returncode}",\n',
            T_VALIDATE, "python"),
    # install.py builds the agy folder into the install root.
    Control("p78_install_builds_agy", INSTALL,
            "        agy_dir, report = agy_plugin.install(root)\n",
            '        agy_dir, report = agy_plugin.install(root / "elsewhere")\n',
            T_MAIN, "python"),
    # install.py exits non-zero when the agy folder failed.
    Control("p78_install_agy_exit", INSTALL,
            "    if agy_failed is not None:\n", "    if False:\n",
            T_MAIN, "python"),
    # R1 (b), `agy plugin install`. The build is a native plugin: plugin.json at the root.
    Control("p78_agy_manifest_written", PLUGIN,
            '    write(out / MANIFEST, json.dumps(manifest(plugin_version() if version is None else version)) + "\\n")\n',
            "",
            T_NATIVE, "python"),
    # The plugin's name, fixed so each install replaces the last.
    Control("p78_agy_plugin_name", PLUGIN,
            'NAME = "nirvana"\n', 'NAME = "nirvana-agy"\n',
            T_NATIVE, "python"),
    # A rebuild of the same version writes the same bytes.
    Control("p78_agy_deterministic", PLUGIN,
            '    return {"name": NAME, "version": version}\n',
            '    return {"name": NAME, "version": version, "built": __import__("time").perf_counter_ns()}\n',
            T_SAME, "python"),
    # agy installs the folder that took the stage's place, not the stage.
    Control("p78_agy_install_folder", PLUGIN,
            '        report["agy_install"] = register(dst / NAME)\n',
            '        report["agy_install"] = register(stage / NAME)\n',
            T_INSTALL, "python"),
    # The plugin name is read from agy's summary.
    Control("p78_agy_summary_name", PLUGIN,
            "_INSTALLED_NAME = re.compile(r'Installed plugin \"", "_INSTALLED_NAME = re.compile(r'Installed plugins \"",
            T_INSTALL, "python"),
    # The folder is read from agy's summary.
    Control("p78_agy_summary_folder", PLUGIN,
            "Installed to (.+?)", "Installed into (.+?)",
            T_INSTALL, "python"),
    # A replace is read from agy's summary.
    Control("p78_agy_summary_replaced", PLUGIN,
            '"replaced": bool(folder and folder.group(2))}', '"replaced": False}',
            T_INSTALL, "python"),
    # agy's stdin is an empty pipe: Windows NUL reads as a terminal.
    Control("p78_agy_stdin_pipe", PLUGIN,
            '    return subprocess.run([agy, *args], input="", capture_output=True,',
            '    return subprocess.run([agy, *args], stdin=subprocess.DEVNULL, capture_output=True,',
            T_PIPE, "python"),
    # NO_COLOR is set for agy.
    Control("p78_agy_no_color", PLUGIN,
            'env={**os.environ, "NO_COLOR": "1"})', "env=dict(os.environ))",
            T_PIPE, "python"),
    # A failed validation never reaches `agy plugin install`.
    Control("p78_agy_failed_validate_no_install", PLUGIN,
            '        if report["validate"]["ok"] is False:\n', "        if False:\n",
            T_NO_INSTALL, "python"),
    # The validation error names agy's findings.
    Control("p78_agy_validate_findings_named", PLUGIN,
            """                             f"{report['validate']['findings']}")\n""",
            '                             "")\n',
            T_NO_INSTALL, "python"),
    # agy not on PATH skips the install too.
    Control("p78_agy_missing_skips_install", PLUGIN,
            '    if report["validate"]["ok"] is None:\n', "    if False:\n",
            T_MISSING, "python"),
    # A real agy never gets a plugin from the temp folder (test_scripts.py ran install.main into one).
    Control("p78_agy_temp_plugin_refused", PLUGIN,
            "    if _under(plugin, temp_root()) and not _under(agy, temp_root()):\n", "    if False:\n",
            T_TEMP, "python"),
    # agy not on PATH is not a failure: install.py exits 0.
    Control("p78_agy_missing_not_failure", PLUGIN,
            '        report["agy_install"] = {"ok": None, "why": "agy is not on PATH',
            '        report["agy_install"] = {"ok": False, "why": "agy is not on PATH',
            T_MISSING_MAIN, "python"),
    # install.py says the agy install was skipped.
    Control("p78_install_prints_agy_skip", INSTALL,
            '    elif done.get("ok") is None:\n', "    elif False:\n",
            T_MISSING_MAIN, "python"),
    # install.py prints the installed plugin's name and folder.
    Control("p78_install_prints_agy_summary", INSTALL,
            '    if done.get("ok"):\n', "    if False:\n",
            T_PRINTS_MAIN, "python"),
    # A failed agy install raises.
    Control("p78_agy_install_failure_raises", PLUGIN,
            '    if report["agy_install"]["ok"] is False:\n', "    if False:\n",
            T_FAIL_MAIN, "python"),
    # The failure carries agy's own error text.
    Control("p78_agy_install_error_named", PLUGIN,
            "f\"agy plugin install exit {done.returncode}: {out[-600:] or 'no output'}\"",
            'f"agy plugin install exit {done.returncode}"',
            T_FAIL_MAIN, "python"),
    # install.py exits 1 on a failed agy install.
    Control("p78_install_agy_install_exit", INSTALL,
            "    except agy_plugin.AgyInstallError as err:\n        agy_failed = err\n",
            "    except agy_plugin.AgyInstallError as err:\n        agy_failed = None\n",
            T_FAIL_MAIN, "python"),
)
