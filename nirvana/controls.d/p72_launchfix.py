# Plan 72, alpha 0.71.1: the worker launch in a reused pane, `owns:` with a trailing slash, and the owner's commits a
# worktree began with. The owner, 2026-11-13: "Can you check the Megajam project especially lately the runs have not
# been showing up on herder. Check for bugs as well." and "Go ahead". Controls starting with p72_. One sabotage per
# assertion, each on a product line; the test named must print FAIL with it in place. `Control` is provided by
# controls.py.
WORKER = "src/nirvana_worker.rs"
GATE = "src/nirvana_gate.rs"

T_LINE = "nirvana_worker::tests::p72_the_launch_line_names_this_build_by_its_full_path_in_the_panes_own_shell"
T_SHELL = "nirvana_worker::tests::p72_a_panes_shell_is_read_from_its_process_name"
T_PS = "nirvana_worker::tests::p72_powershell_runs_the_launch_line_as_written"
T_CONFIRM = "nirvana_worker::tests::p72_a_pane_launch_counts_only_when_the_supervisor_takes_the_order"
T_PATH = "nirvana_worker::tests::p72_seat_exec_puts_its_own_folder_first_on_path_once"
T_START = "nirvana_worker::tests::p72_a_new_worktree_records_the_commit_it_began_at_beside_itself"
T_SLASH = "nirvana_gate::tests::p72_an_owns_line_ending_in_a_slash_owns_the_whole_folder"
T_BEGUN = "nirvana_gate::tests::p72_the_owners_commits_a_worktree_began_with_are_not_the_orders"
T_SNEAK = "nirvana_gate::tests::p72_a_worker_edit_to_a_file_the_worktree_began_with_still_counts"
T_REVERT = "nirvana_gate::tests::p72_a_branch_that_undoes_merged_work_keeps_the_file_as_the_orders"
T_FORGED = "nirvana_gate::tests::p72_a_forged_start_record_passes_nothing"

CONTROLS = (
    # The launch line: this build by its full path, in the order's folder, with the order's variables, quoted
    Control("p72_bare_herdr", WORKER, "                ps_quote(&exe),\n", '                "herdr",\n',
            T_LINE, "rust"),
    Control("p72_bare_herdr_ps", WORKER, "                ps_quote(&exe),\n", '                "herdr",\n',
            T_PS, "rust"),
    Control("p72_no_set_location", WORKER,
            'let mut line = format!("Set-Location -LiteralPath {}", ps_quote(cwd));',
            'let mut line = format!("$null{}", cwd.len() * 0);', T_PS, "rust"),
    Control("p72_no_env_ps", WORKER,
            'line.push_str(&format!("; $env:{key} = {}", ps_quote(value)));',
            'line.push_str(&format!("{}", (key.len() + value.len()) * 0));', T_PS, "rust"),
    Control("p72_typographic_quote", WORKER,
            "if matches!(ch, '\\'' | '\\u{2018}' | '\\u{2019}' | '\\u{201a}' | '\\u{201b}') {",
            "if matches!(ch, '\\'') {", T_LINE, "rust"),
    Control("p72_posix_unescaped", WORKER, "format!(\"'{}'\", value.replace('\\'', \"'\\\\''\"))",
            "format!(\"'{}'\", value)", T_LINE, "rust"),
    Control("p72_cmd_no_env", WORKER,
            'line.push_str(&format!(" && set {}", cmd_quote(&format!("{key}={value}"))));',
            'line.push_str(&format!("{}", (key.len() + value.len()) * 0));', T_LINE, "rust"),
    # The pane's shell from its process name
    Control("p72_cmd_unknown", WORKER, '            "cmd" => Some(Self::Cmd),\n', '            "cmd" => None,\n',
            T_SHELL, "rust"),
    Control("p72_login_dash", WORKER, "        let name = name.trim_start_matches('-');\n",
            "        let name = name.as_str();\n", T_SHELL, "rust"),
    # The launch counts only when the supervisor takes the order's lock; otherwise detached, saying why
    Control("p72_trust_at_once", WORKER, "                if started() {\n", "                if true {\n",
            T_CONFIRM, "rust"),
    Control("p72_trust_after_wait", WORKER,
            "                if begun.elapsed() >= limit {\n                    break;\n",
            "                if begun.elapsed() >= limit {\n                    return Ok(format!(\"pane {pane}\"));\n",
            T_CONFIRM, "rust"),
    Control("p72_last_line_dropped", WORKER,
            "                Some(line) => format!(\n"
            "                    \"no worker took the order within {} s; the pane's last line reads: {line}\",\n",
            "                Some(_line) => format!(\n"
            "                    \"no worker took the order within {} s; the pane's last line reads:\",\n",
            T_CONFIRM, "rust"),
    Control("p72_typing_error_dropped", WORKER,
            'Err(why) => format!("the launch line could not be typed: {why}"),',
            'Err(why) => format!("the launch line could not be typed: {}", why.len()),', T_CONFIRM, "rust"),
    Control("p72_both_failed_ok", WORKER, "        Err(err) => Err(format!(\n            \"the launch in pane",
            "        Err(err) => Ok(format!(\n            \"the launch in pane", T_CONFIRM, "rust"),
    # seat exec's PATH: its own folder first, once
    Control("p72_path_kept", WORKER, "    if rest.first().is_some_and(|first| first == dir) {\n",
            "    if !rest.is_empty() || rest.first().is_some_and(|first| first == dir) {\n", T_PATH, "rust"),
    Control("p72_path_twice", WORKER, ".chain(rest.into_iter().filter(|entry| entry != dir));",
            ".chain(rest.into_iter());", T_PATH, "rust"),
    # The worktree's start record, beside it, gone with it
    Control("p72_no_start_record", WORKER, '    std::fs::write(&record, format!("{start}\\n"))',
            '    std::fs::write(&record, format!("{}", start.len() * 0))', T_START, "rust"),
    Control("p72_record_stays", WORKER,
            "            let _ = std::fs::remove_file(crate::nirvana_gate::start_file(worktree));\n"
            "            return Ok(());\n        }\n        let unregistered",
            "            return Ok(());\n        }\n        let unregistered", T_START, "rust"),
    Control("p72_record_elsewhere", GATE, 'worktree.with_file_name(format!("{name}.start"))',
            'worktree.with_file_name(format!("{name}.begin"))', T_START, "rust"),
    # owns: a trailing slash owns the folder; a bare slash owns nothing
    Control("p72_slash_one_level", GATE, '        p.push("**");\n', '        p.push("*");\n', T_SLASH, "rust"),
    Control("p72_bare_slash_all", GATE, "    if pattern.ends_with('/') && !p.is_empty() {\n",
            "    if pattern.ends_with('/') {\n", T_SLASH, "rust"),
    # The owner's commits a worktree began with: left out only when the worker left them and no merged work is undone
    Control("p72_begun_not_applied", GATE,
            "        (Some(base), true) => without_begun_with(subject.cwd, base, changed),\n",
            "        (Some(base), true) => (changed, Some(base.clone())),\n", T_BEGUN, "rust"),
    Control("p72_worker_edit_dropped", GATE,
            ".filter(|path| !worker.contains(*path) && !integrated.contains(*path))",
            ".filter(|path| (worker.is_empty() || true) && !integrated.contains(*path))", T_SNEAK, "rust"),
    Control("p72_merged_work_undone", GATE,
            ".filter(|path| !worker.contains(*path) && !integrated.contains(*path))",
            ".filter(|path| !worker.contains(*path) && (integrated.is_empty() || true))", T_REVERT, "rust"),
    # Review finding 1: a start record only a `nirvana/` branch holds is the worker's own, not where it began.
    Control("p72_forged_start_trusted", GATE,
            '.any(|name| !name.is_empty() && !name.starts_with("refs/heads/nirvana/"))',
            '.any(|name| !name.is_empty())', T_FORGED, "rust"),
)
