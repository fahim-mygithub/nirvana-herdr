# Plan 43 A4 (nirvana/plans/2026-11-18-43-megajam-audit.md, finding C4, owner ruling R5 "5.yes"): while a run is open
# in auto mode, the orchestrator's AskUserQuestion is refused and pointed to `herdr ask add`.
# Controls starting with p85_. One sabotage per assertion, each on a product line; the test named must print FAIL with
# it in place (the expected red test is the fifth field, written before the run). `Control` is provided by controls.py.
CLAUDE = "src/nirvana_claude.rs"
HOST = "src/nirvana_hookhost.rs"
HOOKS = "nirvana/claude/hooks/hooks.json"
ROLE = "nirvana/claude/ROLE.md"
NAMES = "nirvana/agy_tool_names.json"
PLUGIN = "nirvana/agy_plugin.py"

_C = "nirvana_claude::tests::"
T_TEXT = _C + "p85_the_ask_refusal_says_file_it_keep_working_and_name_the_id"
T_AUTO = _C + "p85_auto_mode_refuses_the_modal_question"
T_WITHOUT = _C + "p85_without_an_open_auto_run_the_modal_works"
T_SCOPE = _C + "p85_only_the_orchestrator_is_refused"
T_WIRED = _C + "p85_the_ask_guard_is_a_pre_tool_hook"
T_CHILD = _C + "p85_the_ask_hook_reads_the_marker_and_the_project"
T_ROLE = _C + "p85_the_role_says_ask_with_herdr_ask_add_in_auto_mode"
_H = "nirvana_hookhost::tests::"
T_AGY = _H + "p85_agys_ask_question_runs_the_ask_guard_and_its_refusal_is_a_deny"
T_FIXTURE = _H + "the_tool_table_is_the_shared_fixture"
T_MATCH = "nirvana.test_agy_plugin.AgyHooksTest.test_claude_tool_matchers_become_agy_tool_names"

CONTROLS = (
    # N2: the refusal's words.
    Control("p85_refusal_drops_the_id", CLAUDE,
            "that does not depend on the answer, and name the question id it prints (such as q3) in your end-of-turn \\\n",
            "that does not depend on the answer, and mention it in your end-of-turn \\\n",
            T_TEXT, "rust"),
    # N1: an open auto run refuses, as a deny.
    Control("p85_auto_run_never_found", CLAUDE,
            "        .find(|(_, state)| state.auto())\n",
            "        .find(|(_, state)| state.auto() && false)\n",
            T_AUTO, "rust"),
    Control("p85_refusal_allows", CLAUDE,
            '            "permissionDecision": "deny",\n            "permissionDecisionReason": ask_refusal(&run),\n',
            '            "permissionDecision": "allow",\n            "permissionDecisionReason": ask_refusal(&run),\n',
            T_AUTO, "rust"),
    # N3: no auto run, no refusal: a run not in auto mode, and an auto run that ended.
    Control("p85_any_open_run_refuses", CLAUDE,
            "        .find(|(_, state)| state.auto())\n",
            "        .find(|(_, _state)| true)\n",
            T_WITHOUT, "rust"),
    Control("p85_ended_run_counts", CLAUDE,
            "pub fn auto_run(project: &Project) -> Option<String> {\n    open_runs(project)\n        .into_iter()\n",
            "pub fn auto_run(project: &Project) -> Option<String> {\n    project\n        .runs()\n        .into_iter()\n"
            "        .map(|run| {\n            let state = nirvana_run::replay(&project.journal(&run).read().0);\n"
            "            (run, state)\n        })\n",
            T_WITHOUT, "rust"),
    # N3: only the orchestrator, never a subagent or a worker in its worktree.
    Control("p85_every_session_refused", CLAUDE,
            "    if should_skip_orchestrator_hook(hook, cwd, is_orchestrator) {\n        return None;\n    }\n"
            "    let run = auto_run(project?)?;\n",
            "    let _ = (cwd, is_orchestrator);\n    let run = auto_run(project?)?;\n",
            T_SCOPE, "rust"),
    # N3 end to end: the hook command reads the marker and the project.
    Control("p85_cli_ignores_the_marker", CLAUDE,
            "            is_orchestrator_env(),\n            project.as_ref(),\n",
            "            true,\n            project.as_ref(),\n",
            T_CHILD, "rust"),
    Control("p85_cli_finds_no_project", CLAUDE,
            "        let project = project_at(cwd.as_deref());\n",
            "        let project: Option<Project> = None;\n",
            T_CHILD, "rust"),
    Control("p85_verb_not_dispatched", CLAUDE,
            '        "guard-ask" => guard_ask_cli(),\n',
            "",
            T_CHILD, "rust"),
    # The wiring: Claude's hooks.json runs the verb on AskUserQuestion.
    Control("p85_hook_unwired", HOOKS,
            '        "matcher": "AskUserQuestion",\n',
            '        "matcher": "AskUserQuestionX",\n',
            T_WIRED, "rust"),
    # N1 on agy: ask_question reaches the guard as AskUserQuestion, and its deny is agy's.
    Control("p85_agy_ask_unmapped", HOST,
            '    ("ask_question", "AskUserQuestion"),\n',
            '    ("ask_question_x", "AskUserQuestion"),\n',
            T_AGY, "rust"),
    Control("p85_agy_verb_unmatched", HOST,
            '        "guard-ask" => &["AskUserQuestion"],\n',
            '        "guard-ask" => &["Read"],\n',
            T_AGY, "rust"),
    Control("p85_shared_table_drops_ask", NAMES,
            '  ],\n  "AskUserQuestion": [\n    "ask_question"\n  ]\n',
            "  ]\n",
            T_FIXTURE, "rust"),
    Control("p85_plugin_matches_no_ask", PLUGIN,
            '    "AskUserQuestion": ["ask_question"],\n',
            '    "AskUserQuestion": ["ask_question_x"],\n',
            T_MATCH, "python"),
    # The role says it first.
    Control("p85_role_keeps_the_modal", ROLE,
            "  <order>]`, never `AskUserQuestion`: the `guard-ask` hook refuses it, since one modal held a run for 406 minutes.\n",
            "  <order>]`, or `AskUserQuestion`.\n",
            T_ROLE, "rust"),
)
