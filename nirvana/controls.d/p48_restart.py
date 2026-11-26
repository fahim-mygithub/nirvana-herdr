# Plan 48 (2026-11-25): the owner, "Is there a way to restart all of nirvana and add an option to do so". Controls
# starting with p48_ for the restart, and with p39v_ for the phone chat that asks again about a row the server forgot
# (review note 2 of alpha 0.95.1). One sabotage per assertion, each on a product line; the test named must print FAIL
# with it in place, for the reason in the comment above it (the panic text expected), not a compile error. `Control`
# is provided by controls.py.
LIVE = "src/nirvana_remote/live.rs"
ALL = "src/nirvana_restart_all.rs"
ROUTES = "src/nirvana_remote/routes.rs"
SPACES_JS = "src/nirvana_remote/page/spaces.js"
APP_JS = "src/nirvana_remote/page/app.js"
HEADER = "src/client/shell/nirvana_header.rs"
COMMANDS = "src/app/custom_commands.rs"
LIFECYCLE = "src/server/headless/lifecycle.rs"

_T = "nirvana_remote::tests_live::"
T_FORGOTTEN_AT_OPEN = _T + "a_chat_opened_on_a_forgotten_row_asks_again_and_takes_the_token"
T_NEVER_READ = _T + "a_chat_on_a_row_never_read_asks_a_bounded_number_of_times"

_A = "nirvana_restart_all::tests::"
T_REASON = _A + "the_shutdown_says_restart_only_when_the_stop_was_one"
T_IS_RESTART = _A + "only_a_restarts_reason_brings_the_client_back"
T_CLIENT = _A + "the_client_comes_back_only_once_the_old_sockets_are_gone"
T_COMEBACK = _A + "no_window_brings_the_server_back_alone"
T_OLD = _A + "the_worker_waits_for_the_old_server_then_gives_up"
T_WINDOW = _A + "with_a_window_the_worker_waits_for_its_client_and_starts_the_server_only_if_none_came"
T_ALONE = _A + "with_no_window_the_worker_starts_the_server_at_once_and_waits_for_it_to_listen"
T_RUN_WINDOW = _A + "a_restart_with_a_window_waits_for_its_client_and_starts_nothing"
T_RUN_ALONE = _A + "a_restart_with_no_window_starts_the_server_once_the_old_one_is_gone"
T_RUN_LATE = _A + "a_window_whose_client_never_comes_gets_the_server_started_for_it"
T_RUN_FAILS = _A + "a_restart_says_what_did_not_come_back"
T_PANE = _A + "the_command_knows_when_it_runs_in_a_pane_of_the_session_it_restarts"
T_NOTICE = _A + "the_notice_counts_the_working_agents_and_says_what_comes_back"
T_RESULT = _A + "the_command_reads_only_its_own_runs_outcome"
T_ARGS = _A + "the_worker_is_started_with_the_session_named"
T_HOP = _A + "the_hop_starts_the_same_worker"
T_COMMAND = _A + "the_restart_command_stops_the_server_as_a_restart"
T_LIFECYCLE = "server::headless::tests::a_restarts_shutdown_tells_each_client_it_is_a_restart"
T_HEADER = "client::shell::nirvana_header::tests::the_version_label_restarts_on_a_second_press_when_no_build_is_newer"

_R = "nirvana_remote::tests_restart::"
T_PHONE = _R + "the_phones_restart_starts_the_worker_and_says_how_many_were_working"
T_HELLO = _R + "hello_names_the_server_that_answers"
T_METHOD = _R + "a_get_or_another_method_is_refused_and_starts_nothing"
T_OWNER = _R + "a_missing_or_wrong_owner_is_refused_and_starts_nothing"
T_CROSS = _R + "a_cross_site_restart_is_refused_and_starts_nothing"
T_JSON = _R + "a_restart_that_is_not_json_or_cannot_start_says_so"
T_NEW_SITE = _R + "a_new_site_starts_no_worker"
T_PAGE = _R + "the_page_confirms_a_restart_and_comes_back_when_the_new_server_answers"

_N = "nirvana.test_phonerestart.PhoneRestartTest."
N_BACK = _N + "test_the_page_waits_for_the_new_server_and_comes_back_by_itself"
N_NEVER = _N + "test_a_server_that_never_comes_back_ends_on_the_error_screen"
N_OLD = _N + "test_the_old_server_answering_never_ends_the_wait"
N_KEPT = _N + "test_a_kept_screen_comes_back_as_it_was"

# Every check before a route passes for /api/restart only: the refusals' controls, one test each.
_BYPASS = ("        if let Err((status, why)) = checked {\n",
           '        if let Err((status, why)) = checked.or_else(|e| if request.path == "/api/restart" '
           "{ Ok(()) } else { Err(e) }) {\n")

# ---- Plan 48 fix round (review of 7d493691), A to H ----
NIRVANA = "src/nirvana.rs"
T_MARK = _A + "a_servers_mark_is_its_pid_and_its_start"
T_ONE_MARK = _R + "hello_and_the_restart_name_one_server"
T_TWO_PRESSES = "nirvana::tests::reload_takes_two_presses"
T_LAPSE = "nirvana::tests::an_armed_press_lapses"
T_SUPERVISOR = "nirvana::tests::the_supervisor_marks_the_client_it_starts"
_H = "client::shell::nirvana_header::tests::"
T_SUPERVISED = _H + "only_a_supervised_client_offers_the_restart"
T_NARROW = _H + "a_label_the_header_cannot_draw_whole_is_no_press"
T_OLD_SERVER = _A + "an_older_server_that_has_no_restart_is_named_in_plain_words"
T_HELP = _A + "help_is_help"
T_SAYS = _A + "the_window_says_it_is_restarting_before_it_waits"
T_TARGET = _A + "the_plain_command_restarts_the_session_its_socket_names"
N_CONFIRM = _N + "test_the_confirm_counts_the_working_agents_and_the_wait_is_for_the_202s_server"
_PAGE_WAITS = "          close(() => nv.restarting(answer.data.server));\n"
_LABEL_PRESS = "        None => supervised && end > start && whole,\n"
_SOCKET_WINS = "        Some(socket) if !explicit => session_of_socket(socket, config_dir).ok_or_else(|| {\n"
_SAYS_THEN_WAITS = ('    let _ = writeln!(out, "{RESTARTING_LINE}");\n    let _ = out.flush();\n'
                    "    let gone = wait_until(STOP_WAIT, POLL, gone, sleep);\n")

FIX_CONTROLS = (
    # A. Red: "the same pid, another start: another mark" (left "41", right "41")
    Control("p48_mark_has_start", ALL,
            '    format!("{pid}-{started_unix_ms}")\n', '    format!("{pid}")\n',
            T_MARK, "rust"),
    # A. Red: "one process, one mark" (left "<pid>-<later ms>", right "<pid>-<first ms>")
    Control("p48_mark_once", ALL,
            "    SERVER_MARK.get_or_init(|| mark_of(std::process::id(), unix_ms_now()))\n",
            "    Box::leak(mark_of(std::process::id(), unix_ms_now()).into_boxed_str())\n",
            T_MARK, "rust"),
    # A. Red: "one server, one mark" (the 202 names another string than hello)
    Control("p48_hello_and_202_one_mark", LIVE,
            '                        "server": crate::nirvana_restart_all::server_mark(),\n',
            '                        "server": format!("{}-0", std::process::id()),\n',
            T_ONE_MARK, "rust"),
    # A. Red: "a string mark: {..."server":<pid>...}"
    Control("p48_hello_mark_is_a_string", ROUTES,
            '    hello["server"] = json!(crate::nirvana_restart_all::server_mark());\n',
            '    hello["server"] = json!(std::process::id());\n',
            T_ONE_MARK, "rust"),

    # B. Red: "lapsed at the window"
    Control("p48_arm_lapses", NIRVANA,
            "    since.is_some_and(|since| since < RELOAD_ARM_WINDOW)\n", "    since.is_some()\n",
            T_LAPSE, "rust"),
    # B. Red: "a lapsed arm reads unarmed"
    Control("p48_reload_armed_lapses", NIRVANA,
            "    arm_holds(armed_at().map(|at| at.elapsed()))\n", "    armed_at().is_some()\n",
            T_TWO_PRESSES, "rust"),
    # B. Red: "after the lapse a press is a first press again" (left (true, None): a press that ignores the lapse).
    # First tried as "(false, Some(now))" -> "(false, None)", which went red at "the first press arms" instead.
    Control("p48_lapsed_press_arms_again", NIRVANA,
            "    if arm_holds(armed_at.map(|at| now.saturating_duration_since(at))) {",
            "    if armed_at.is_some() {",
            T_LAPSE, "rust"),

    # C. Red: "the client knows a supervisor starts it again"
    Control("p48_supervisor_marks", NIRVANA,
            '    command.args(args).env(SUPERVISED_ENV, "1");\n', "    command.args(args);\n",
            T_SUPERVISOR, "rust"),
    # C. Red: "nothing starts this client again: no press" (left the label's Rect, right a zero Rect)
    Control("p48_label_needs_supervisor", HEADER,
            _LABEL_PRESS, "        None => end > start && whole,\n",
            T_SUPERVISED, "rust"),
    # C. Red: "the look is unchanged" (the label's background differs when not supervised)
    Control("p48_unsupervised_look_unchanged", HEADER,
            "            Style::default().fg(palette.overlay1).bg(palette.surface0),\n",
            "            Style::default().fg(palette.overlay1).bg(if supervised { palette.surface0 } else { palette.red }),\n",
            T_SUPERVISED, "rust"),
    # C. Red: "a newer build: the reload button in every client" (left 0)
    Control("p48_update_badge_any_client", HEADER,
            "        Some(_) => end > start,\n", "        Some(_) => supervised && end > start,\n",
            T_SUPERVISED, "rust"),

    # D. Red: "too narrow to draw: no press" (left Rect { x: 9, width: 0 }, right a zero Rect)
    Control("p48_narrow_no_hit", HEADER,
            "        Some(_) => end > start,\n", "        Some(_) => true,\n",
            T_NARROW, "rust"),
    # D. Red: "a clipped label is no press" (left Rect { x: 10, width: 1 })
    Control("p48_clipped_label_no_press", HEADER,
            _LABEL_PRESS, "        None => supervised && end > start,\n",
            T_NARROW, "rust"),

    # E. Red: "an older server in plain words" (left the manifest text)
    Control("p48_old_server_plain", ALL,
            '    if code == Some("command_not_found") {\n', "    if false {\n",
            T_OLD_SERVER, "rust"),
    # E. Red: "another refusal keeps its words" (left OLD_SERVER)
    Control("p48_other_refusal_kept", ALL,
            '    if code == Some("command_not_found") {\n', "    if true {\n",
            T_OLD_SERVER, "rust"),
    # E. Red: "the usage says it too"
    Control("p48_usage_old_server", ALL,
            "  A running Nirvana from an older build has no restart: reload it once with the header's badge, or close the\n",
            "  A running Nirvana from an older build: reload it once with the header's badge, or close the\n",
            T_OLD_SERVER, "rust"),
    # E. Red: left Bad, right Help ("help")
    Control("p48_help_is_help", ALL,
            '        [word] if matches!(word.as_str(), "help" | "--help" | "-h") => Verb::Help,\n', "",
            T_HELP, "rust"),

    # F. Red: "the window says so: \"\""
    Control("p48_window_says_restarting", ALL,
            '    let _ = writeln!(out, "{RESTARTING_LINE}");\n', "",
            T_SAYS, "rust"),
    # F. Red: "said before the wait: \"\"" (the line comes, but after the wait)
    Control("p48_said_before_the_wait", ALL,
            _SAYS_THEN_WAITS,
            "    let gone = wait_until(STOP_WAIT, POLL, gone, sleep);\n"
            '    let _ = writeln!(out, "{RESTARTING_LINE}");\n    let _ = out.flush();\n',
            T_SAYS, "rust"),

    # G. Red: "a pane's socket names its session, HERDR_SESSION or not" (left Ok(None))
    Control("p48_pane_socket_names_session", ALL,
            _SOCKET_WINS,
            "        Some(socket) if !explicit && false => session_of_socket(socket, config_dir).ok_or_else(|| {\n",
            T_TARGET, "rust"),
    # G. Red: "--session names it" (left Ok(Some("nirvana")))
    Control("p48_explicit_session_wins", ALL,
            _SOCKET_WINS,
            "        Some(socket) => session_of_socket(socket, config_dir).ok_or_else(|| {\n",
            T_TARGET, "rust"),
    # G. Red: "the default session's socket" (left Err(...))
    Control("p48_default_socket_is_default", ALL,
            "    if folder == config_dir {\n        return Some(None);\n", "    if false {\n        return Some(None);\n",
            T_TARGET, "rust"),
    # G. Red: "`--session default` is the config folder's own socket, never sessions/default"
    Control("p48_sessions_default_is_none", ALL,
            "    if name == crate::session::DEFAULT_SESSION_NAME {\n", "    if false {\n",
            T_TARGET, "rust"),
    # G. Red: the odd socket's words name `--session default` again (assertion on "--session <name>")
    Control("p48_odd_socket_names_no_session", ALL,
            "                 --session <name> nirvana restart`\",",
            "                 --session default nirvana restart`\",",
            T_TARGET, "rust"),

    # H (node). Red: "the confirm names the two working agents" (it says they could not be counted)
    Control("p48_node_confirm_counts", SPACES_JS,
            ".then((hello) => hello.working, () => null)", ".then(() => null, () => null)",
            N_CONFIRM, "python"),
    # H. Red: "one POST to /api/restart" (the POST went to /api/restart-not)
    Control("p48_node_one_post", SPACES_JS,
            "nv.postJson('/api/restart', {})", "nv.postJson('/api/restart-not', {})",
            N_CONFIRM, "python"),
    # H. Red: "the wait's screen is drawn after Restart" (True is not... False is not true)
    Control("p48_node_wait_after_confirm", SPACES_JS,
            _PAGE_WAITS, "          close();\n",
            N_CONFIRM, "python"),
    # H. Red: "the wait is for the server the 202 named" (marks [2])
    Control("p48_node_wait_on_202_mark", SPACES_JS,
            _PAGE_WAITS, "          close(() => nv.restarting(answer.data.working));\n",
            N_CONFIRM, "python"),
    # H. Red: "that server answering again: not back yet"
    Control("p48_node_confirm_same_mark_waits", APP_JS,
            "      const back = response.ok && data.server && data.server !== wait.old;\n",
            "      const back = response.ok;\n",
            N_CONFIRM, "python"),
    # A, page side. Red: "the same pid with another start is a new server" (a pid-only compare waits for ever)
    Control("p48_node_same_pid_new_start", APP_JS,
            "      const back = response.ok && data.server && data.server !== wait.old;\n",
            "      const back = response.ok && data.server && String(data.server).split('-')[0] !== "
            "String(wait.old).split('-')[0];\n",
            N_CONFIRM, "python"),
)

CONTROLS = (
    # P39v, a chat opened on an unread row asks again by itself
    # Red: "a token that came with no event and no feed build wakes the chat: the whole chat from its file"
    Control("p39v_ask_when_unread", LIVE,
            "        let mut asks = if spaces::unread_row(&found.agent) {\n",
            "        let mut asks = if false && spaces::unread_row(&found.agent) {\n",
            T_FORGOTTEN_AT_OPEN, "rust"),
    # Red: "about once a second: 4.0...s" (the first ask comes four seconds in)
    Control("p39v_first_ask_soon", LIVE,
            "        let mut next_ask = Instant::now() + spaces::UNREAD_EVERY;\n",
            "        let mut next_ask = Instant::now() + spaces::UNREAD_EVERY * 4;\n",
            T_FORGOTTEN_AT_OPEN, "rust"),
    # Red: "a read row is not asked about again" (left: more agent.list calls, right: the count before)
    Control("p39v_stop_once_read", LIVE,
            "                        asks = 0;\n",
            "",
            T_FORGOTTEN_AT_OPEN, "rust"),
    # Red: "UNREAD_TRIES asks, then none" (left 9 or so, right 6)
    Control("p39v_asks_bounded", LIVE,
            "                    asks -= 1;\n",
            "",
            T_NEVER_READ, "rust"),

    # ---- Plan 48: the server's stop as a restart ----
    # Red: left "server is shutting down", right "nirvana restart"
    Control("p48_shutdown_says_restart", ALL,
            "        SHUTDOWN_REASON.to_owned()\n", "        PLAIN_SHUTDOWN.to_owned()\n",
            T_REASON, "rust"),
    # Red: "upstream's own text otherwise"
    Control("p48_plain_stop_upstream_text", ALL,
            'pub const PLAIN_SHUTDOWN: &str = "server is shutting down";',
            'pub const PLAIN_SHUTDOWN: &str = "server stopping";',
            T_REASON, "rust"),
    # Red: "as a restart"
    Control("p48_command_marks_restart", COMMANDS,
            "            self.nirvana_restarting = true;\n", "",
            T_COMMAND, "rust"),
    # Red: "the server stops"
    Control("p48_command_stops", COMMANDS,
            "            self.nirvana_restarting = true;\n            self.state.should_quit = true;\n",
            "            self.nirvana_restarting = true;\n",
            T_COMMAND, "rust"),
    # Red: left Some("server is shutting down"), right Some("nirvana restart") ("restarting: true")
    Control("p48_lifecycle_reason", LIFECYCLE,
            "            reason: Some(crate::nirvana_restart_all::shutdown_message(\n"
            "                self.app.nirvana_restarting,\n",
            "            reason: Some(crate::nirvana_restart_all::shutdown_message(\n"
            "                self.app.nirvana_restarting && false,\n",
            T_LIFECYCLE, "rust"),

    # ---- The client comes back as the badge's does ----
    # Red: "a plain stop ends the window"
    Control("p48_only_restart_reason", ALL,
            "    reason == Some(SHUTDOWN_REASON)\n", "    reason.is_some()\n",
            T_IS_RESTART, "rust"),
    # Red: "sockets still held: the window leaves" (left 75, right 1)
    Control("p48_client_exit_only_when_gone", ALL,
            "    if old_gone {\n", "    if true || old_gone {\n",
            T_CLIENT, "rust"),
    # Red: "assertion failed: gone" (it gives up before it looks again)
    Control("p48_wait_looks_again", ALL,
            "        if waited >= limit {\n", "        if waited >= Duration::ZERO {\n",
            T_CLIENT, "rust"),
    # Red: "a server that keeps its sockets is waited on for STOP_WAIT, then given up"
    Control("p48_wait_gives_up", ALL,
            "        if waited >= limit {\n            return false;\n",
            "        if waited >= limit {\n            return true;\n",
            T_CLIENT, "rust"),
    # Red: left 7.5s, right 15s (the wait was cut short)
    Control("p48_wait_full_limit", ALL,
            "        waited += step;\n", "        waited += step * 2;\n",
            T_CLIENT, "rust"),

    # ---- What brings the server back ----
    # Red: left ServerAlone, right Window
    Control("p48_window_comes_back", ALL,
            "nirvana_launch::SESSION) {\n        Comeback::Window\n",
            "nirvana_launch::SESSION) {\n        Comeback::ServerAlone\n",
            T_COMEBACK, "rust"),
    # Red: "the window was closed"
    Control("p48_window_needs_a_live_mark", ALL,
            "    if window_live && session == Some(crate::nirvana_launch::SESSION) {",
            "    if session == Some(crate::nirvana_launch::SESSION) {",
            T_COMEBACK, "rust"),
    # Red: "another session's restart: the Nirvana window is not its"
    Control("p48_window_is_nirvanas_only", ALL,
            "    if window_live && session == Some(crate::nirvana_launch::SESSION) {",
            "    if window_live {",
            T_COMEBACK, "rust"),

    # ---- The worker's steps ----
    # Red: left Wait, right Fail("the old server did not stop ...")
    Control("p48_old_server_given_up", ALL,
            "            Step::Fail(NOT_STOPPED)\n", "            Step::Wait\n",
            T_OLD, "rust"),
    # Red: left Done, right Wait (the old server's answer taken for the new one)
    Control("p48_old_answer_is_not_new", ALL,
            "    if !facts.old_gone {\n", "    if !facts.old_gone && !facts.listening {\n",
            T_OLD, "rust"),
    # Red: "the window's client starts it" (left StartServer, right Wait)
    Control("p48_window_waits_for_client", ALL,
            "        None if facts.window && facts.since_gone < WINDOW_WAIT => Step::Wait,\n", "",
            T_WINDOW, "rust"),
    # Red: left Wait, right StartServer (past WINDOW_WAIT)
    Control("p48_window_wait_ends", ALL,
            "        None if facts.window && facts.since_gone < WINDOW_WAIT => Step::Wait,\n",
            "        None if facts.window => Step::Wait,\n",
            T_WINDOW, "rust"),
    # Red: left StartServer, right Done (a listening server is the end)
    Control("p48_listening_is_done", ALL,
            "    if facts.listening {\n", "    if false && facts.listening {\n",
            T_WINDOW, "rust"),
    # Red: "started once, not again"
    Control("p48_started_once", ALL,
            "        Some(_) => Step::Wait,\n", "        Some(_) => Step::StartServer,\n",
            T_ALONE, "rust"),
    # Red: left Wait, right Fail("a new server was started but did not listen ...")
    Control("p48_ready_wait_gives_up", ALL,
            "        Some(waited) if waited >= READY_WAIT => Step::Fail(NOT_LISTENING),\n",
            "        Some(waited) if waited >= READY_WAIT * 100 => Step::Fail(NOT_LISTENING),\n",
            T_ALONE, "rust"),
    # Red: left ["stopping", "done"], right ["stopping", "stopped", "done"]
    Control("p48_run_reports_stopped", ALL,
            '            world.report("stopped", "");\n', "",
            T_RUN_WINDOW, "rust"),
    # Red: left 2 or more, right 1 (the server started again on every step)
    Control("p48_run_starts_once", ALL,
            "                started_at = Some(now);\n", "",
            T_RUN_ALONE, "rust"),
    # Red: "it ends well" (a window whose client never came is given up on, not given a server). A sabotage that
    # only waits would loop the scripted clock for ever, so this one fails instead.
    Control("p48_run_starts_for_a_late_window", ALL,
            "        None if facts.window && facts.since_gone < WINDOW_WAIT => Step::Wait,\n",
            "        None if facts.window && facts.since_gone < WINDOW_WAIT => Step::Wait,\n"
            "        None if facts.window => Step::Fail(NOT_LISTENING),\n",
            T_RUN_LATE, "rust"),
    # Red: "called `Result::unwrap_err()` on an `Ok` value" (a refused restart went on)
    Control("p48_run_refusal_ends_it", ALL,
            "    if let Err(why) = world.ask_restart() {\n",
            "    if let Err(why) = world.ask_restart().or(Ok::<(), String>(())) {\n",
            T_RUN_FAILS, "rust"),

    # ---- What the command says ----
    # Red: "assertion failed: !in_own_pane(...)" (another session's pane)
    Control("p48_own_pane_is_the_sessions", ALL,
            "    pane_socket.is_some_and(|socket| socket == session_socket)\n",
            "    pane_socket.is_some()\n",
            T_PANE, "rust"),
    # Red: "nirvana: restarting session nirvana: 3 agents are working now. ..."
    Control("p48_notice_counts", ALL,
            '        Some(count) => format!("{count} agents are working now; their turns stop"),\n',
            '        Some(count) => format!("{count} agents are working now"),\n',
            T_NOTICE, "rust"),
    # Red: "assertion failed: text(Some(1), ...)" (it said "1 agents are")
    Control("p48_notice_one", ALL,
            '        Some(1) => "1 agent is working now; its turn stops".to_owned(),\n', "",
            T_NOTICE, "rust"),
    # Red: "assertion failed: text(Some(0), ...)" (it said "0 agents are")
    Control("p48_notice_none", ALL,
            '        Some(0) => "no agent is working now".to_owned(),\n', "",
            T_NOTICE, "rust"),
    # Red: "... the Nirvana window stays" not in the notice
    Control("p48_notice_window", ALL,
            '    "nirvana: the Nirvana window stays: its client comes back on the new server.";',
            '    "nirvana: the window closes.";',
            T_NOTICE, "rust"),
    # Red: "... the server comes back with no window" not in the notice
    Control("p48_notice_alone", ALL,
            "the server comes back with no window (Open Nirvana opens one).",
            "a window opens.",
            T_NOTICE, "rust"),
    # Red: "it stops with it; the restart goes on by itself" not in the notice
    Control("p48_notice_own_pane", ALL,
            "    if own_pane {\n", "    if false && own_pane {\n",
            T_NOTICE, "rust"),
    # Red: "assertion failed: !three.contains(\"stops with it\")"
    Control("p48_notice_own_pane_only", ALL,
            "    if own_pane {\n", "    if true || own_pane {\n",
            T_NOTICE, "rust"),
    # Red: "another run's line" (left Some(("failed", ...)), right None)
    Control("p48_result_is_the_runs", ALL,
            '    (value["run"] == run).then(|| {\n', "    true.then(|| {\n",
            T_RESULT, "rust"),
    # Red: left [... "--worker" ...], right [... "--hop" ...] (the hop would start a second hop, never the worker)
    Control("p48_hop_flag", ALL,
            '            if arg == "--worker" {\n', '            if arg == "--never" {\n',
            T_HOP, "rust"),
    # Red: left [... "--worker-gone" ...], right [... "--worker" ...]
    Control("p48_worker_flag", ALL,
            '        "--worker".into(),\n', '        "--worker-gone".into(),\n',
            T_ARGS, "rust"),
    # Red: left ["--session", "default", ...], right ["--session", "nirvana", ...]
    Control("p48_worker_session", ALL,
            "        session.into(),\n", '        "default".into(),\n',
            T_ARGS, "rust"),

    # ---- The window's control: the version label ----
    # Red: "the label is the press" (left Rect 0x0, right the label's 14 columns)
    Control("p48_label_is_the_press", HEADER,
            "        None => supervised && end > start && whole,\n", "        None => false,\n",
            T_HEADER, "rust"),
    # Red: "armed: ..." (the label stays " alpha ... " after one press)
    Control("p48_label_arms", HEADER,
            "        _ if reload_armed => (\n", "        Some(_) if reload_armed => (\n",
            T_HEADER, "rust"),

    # ---- The phone's route ----
    # Red: left 405, right 202 ("GET only": no route)
    Control("p48_route_posts", ROUTES,
            '                "/api/restart" => return self.restart(request, conn),\n', "",
            T_PHONE, "rust"),
    # Red: "one worker" (left 2, right 1)
    Control("p48_route_one_worker", LIVE,
            "        match (self.restart)() {\n",
            "        let _ = (self.restart)();\n        match (self.restart)() {\n",
            T_PHONE, "rust"),
    # Red: "the working and the blocked turns stop" (left 0, right 2)
    Control("p48_route_working", LIVE,
            "            Ok(summary) => summary.working,\n", "            Ok(_summary) => 0,\n",
            T_PHONE, "rust"),
    # Red: "the page tells the new server from this one by it" (fix round A: left Number(<pid>), right String("<pid>-<ms>"))
    Control("p48_route_names_server", LIVE,
            '                        "server": crate::nirvana_restart_all::server_mark(),\n',
            '                        "server": std::process::id(),\n',
            T_PHONE, "rust"),
    # Red: fix round A: left Number(<pid>), right String("<pid>-<ms>")
    Control("p48_hello_names_server", ROUTES,
            '    hello["server"] = json!(crate::nirvana_restart_all::server_mark());\n',
            '    hello["server"] = json!(std::process::id());\n',
            T_HELLO, "rust"),
    # Red: "a GET: HTTP/1.1 404 Not Found"
    Control("p48_route_get_refused", ROUTES,
            '        if request.path == "/api/restart" {\n', '        if request.path == "/api/restart-not" {\n',
            T_METHOD, "rust"),
    # Red: left (202, ...), right (403, REFUSED) ("another login")
    Control("p48_route_checked_owner", ROUTES, *_BYPASS, T_OWNER, "rust"),
    # Red: "no login at all" (left 202 or 421, right 403)
    Control("p48_route_needs_a_login", ROUTES,
            '        [] => Err("no Tailscale user: a tagged device, Funnel, or not through Tailscale"),\n',
            "        [] => Ok(()),\n",
            T_OWNER, "rust"),
    # Red: "another site's page on the phone" (left 202, right 403)
    Control("p48_route_checked_site", ROUTES, *_BYPASS, T_CROSS, "rust"),
    # Red: "an Origin that is not this PC's page"
    Control("p48_route_origin_checked", ROUTES,
            "        [value] if value.eq_ignore_ascii_case(&page) => Ok(()),\n", "        [_] => Ok(()),\n",
            T_CROSS, "rust"),
    # Red: "neither header: not a browser on this page"
    Control("p48_route_needs_site_headers", ROUTES,
            '        return Err("a request that may change something without Sec-Fetch-Site or Origin");\n',
            "        return Ok(());\n",
            T_CROSS, "rust"),
    # Red: "a form post cannot reach it" (left 202, right 415)
    Control("p48_route_json_only", LIVE,
            "        if !is_json {\n", "        if false {\n",
            T_JSON, "rust"),
    # Red: left 500, right 503
    Control("p48_route_says_why", LIVE,
            "                error(503, why)\n", "                error(500, why)\n",
            T_JSON, "rust"),
    # Red: "assertion failed: (site.restart)().is_err()"
    Control("p48_new_site_starts_nothing", ROUTES,
            '            restart: Arc::new(|| Err("a restart is not set up on this server".to_owned())),\n',
            "            restart: Arc::new(|| Ok(())),\n",
            T_NEW_SITE, "rust"),

    # ---- The page, as served (Rust) and run in node (python) ----
    # Red: "the control"
    Control("p48_page_control", SPACES_JS,
            "text: 'Restart Nirvana',", "text: 'Reboot',",
            T_PAGE, "rust"),
    # Red: "the count is read when the confirm opens"
    Control("p48_page_counts_at_confirm", SPACES_JS,
            "    nv.getJson('/api/hello', { quiet: true }).then(", "    Promise.resolve({}).then(",
            T_PAGE, "rust"),
    # Red: "the confirm says what stops"
    Control("p48_page_says_turns_stop", SPACES_JS,
            "' working now: their turns stop.'", "' working now.'",
            T_PAGE, "rust"),
    # Red: "the POST, from this page"
    Control("p48_page_posts", SPACES_JS,
            "nv.postJson('/api/restart', {})", "nv.postJson('/api/restart-not', {})",
            T_PAGE, "rust"),
    # Red: "then the shell waits"
    Control("p48_page_waits", SPACES_JS,
            "          close(() => nv.restarting(answer.data.server));\n", "          close();\n",
            T_PAGE, "rust"),
    # Red: "a new server, not the old one still answering"
    Control("p48_page_new_server_text", APP_JS,
            "      const back = response.ok && data.server && data.server !== wait.old;\n",
            "      const back = response.ok;\n",
            T_PAGE, "rust"),
    # Red: "no error screen while it restarts"
    Control("p48_page_guard_text", APP_JS,
            "    if (reach.restarting) return; // plan 48: the wait's own screen is up\n", "",
            T_PAGE, "rust"),
    # Red: "the same server: not back yet" (node: the old server's 41 ended the wait)
    Control("p48_node_old_server_not_back", APP_JS,
            "      const back = response.ok && data.server && data.server !== wait.old;\n",
            "      const back = response.ok;\n",
            N_OLD, "python"),
    # Red: "the wait's screen as soon as the restart is asked"
    Control("p48_node_wait_drawn", APP_JS,
            "    document.getElementById('nv').append(wait.layer);\n", "",
            N_BACK, "python"),
    # Red: "a dropped live view and a gateway error are the restart, not the PC lost"
    Control("p48_node_no_error_screen", APP_JS,
            "    if (reach.restarting) return; // plan 48: the wait's own screen is up\n", "",
            N_BACK, "python"),
    # Red: "the wait's screen goes from over a kept screen too" (a screen drawn again replaces it anyway)
    Control("p48_node_wait_ends", APP_JS,
            "      reach.restarting = null;\n      wait.layer.remove();\n      reach.revive",
            "      reach.restarting = null;\n      reach.revive",
            N_KEPT, "python"),
    # Red: "the screen is drawn again from the new server" (left 1, right 2)
    Control("p48_node_redraw", APP_JS,
            "      if (def && !def.keep) show(current.name, 'f', current.params);\n", "",
            N_BACK, "python"),
    # Red: "its live view opens again at once"
    Control("p48_node_revive", APP_JS,
            "      reach.revive.forEach((revive) => revive());\n", "",
            N_BACK, "python"),
    # Red: "True is not false" at the step past the limit (the wait's screen stays for ever)
    Control("p48_node_gives_up", APP_JS,
            "    if (Date.now() >= wait.until) {\n", "    if (false) {\n",
            N_NEVER, "python"),
    # Red: "past the wait's limit the reach check says the PC is not answering"
    Control("p48_node_gives_up_to_check", APP_JS,
            "      wait.layer.remove();\n      check();\n", "      wait.layer.remove();\n",
            N_NEVER, "python"),
) + FIX_CONTROLS
