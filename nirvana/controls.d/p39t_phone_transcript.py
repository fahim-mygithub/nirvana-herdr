# Plan 39 P39t (2026-11-22): the phone's chat of an orchestrator Claude showed "No transcript for this agent" because
# agent.list never carried the model tokens the window got. Controls starting with p39t_. One sabotage per assertion,
# each on a product line; the test named must print FAIL with it in place, for the reason in the comment above it
# (the panic text expected), not a compile error. `Control` is provided by controls.py.
AGENTS = "src/app/agents.rs"
SHELL = "src/server/client_shell.rs"
SPACES = "src/nirvana_remote/spaces.rs"
LIVE = "src/nirvana_remote/live.rs"
CHAT_RS = "src/nirvana_remote/chat.rs"
CARDS = "src/nirvana_remote/cards.rs"
CHAT_JS = "src/nirvana_remote/page/chat.js"
APP_CSS = "src/nirvana_remote/page/app.css"

T_API = "server::client_shell::tests::the_api_row_carries_every_token_the_window_gets"
_T = "nirvana_remote::tests_live::"
T_ALONE = _T + "a_tokens_arrival_alone_wakes_the_open_chat"
T_BOUNDED = _T + "the_feed_builds_again_for_an_unread_agent_a_bounded_number_of_times"
T_FORGOTTEN = _T + "a_row_that_lost_its_tokens_keeps_its_own_transcript"
T_RECORD = _T + "a_folders_record_is_not_kept_when_another_claude_comes"
T_FOLLOW = _T + "an_events_follow_up_build_still_comes_after_the_unread_builds_stop"
T_ONE_FOLDER = _T + "an_orchestrator_and_a_worker_in_one_folder_each_read_their_own"
T_PAGE = _T + "the_pc_now_box_opens_when_it_is_all_there_is_and_shows_the_newest_lines"
T_LOCAL = _T + "a_local_model_claude_is_still_claude_code_to_the_phone"
T_CARDS = "nirvana_remote::tests_cards::the_running_card_asks_for_a_working_agents_activity_only"

CONTROLS = (
    # Item 1, agent.list carries the window's tokens (one writer, App::agent_info)
    # Red: "agent.list's row has nirvana_kind: {"nirvana_bgwait": "made-up-reading"}"
    Control("p39t_api_model_tokens", AGENTS,
            "            .map(|model| model.tokens())\n",
            "            .map(|_model| Vec::new())\n",
            T_API, "rust"),
    # Red: "agent.list's row has nirvana_bgwait"
    Control("p39t_api_bg_token", AGENTS,
            "            tokens.push((crate::nirvana_bgwait::TOKEN.to_owned(), bg));\n",
            "            let _ = bg;\n",
            T_API, "rust"),
    # Red: "a token the window has that agent.list lacks: nirvana_window_only"
    Control("p39t_window_only_token", SHELL,
            "            let mut tokens = agent.tokens.into_iter().collect::<Vec<_>>();\n",
            "            let mut tokens = agent.tokens.into_iter().collect::<Vec<_>>();\n"
            "            tokens.push((\"nirvana_window_only\".into(), \"x\".into()));\n",
            T_API, "rust"),
    # Red: "each token once" (master's window carried nirvana_bgwait twice)
    Control("p39t_window_token_once", SHELL,
            "            let mut tokens = agent.tokens.into_iter().collect::<Vec<_>>();\n",
            "            let mut tokens = agent.tokens.into_iter().collect::<Vec<_>>();\n"
            "            let first = tokens.first().cloned();\n"
            "            tokens.extend(first);\n",
            T_API, "rust"),
    # Item 1, the feed builds again while a row has no kind
    # Red: "the token alone wakes the view: the whole chat from its file" (no build after subscribing)
    Control("p39t_unread_builds_after_subscribe", SPACES,
            "                    timed = sooner(timed, unread.after(&again));\n",
            "                    timed = sooner(timed, None);\n",
            T_ALONE, "rust"),
    # Red: "the token alone wakes the view: the whole chat from its file" (one build, then none)
    Control("p39t_unread_builds_go_on", SPACES,
            "                timed = sooner(sooner(timed, due), unread.after(&list));\n",
            "                timed = sooner(timed, due);\n",
            T_ALONE, "rust"),
    # Red: "the build FOLLOW_UP after the event: N calls, then N"
    Control("p39t_follow_up_kept", SPACES,
            "                timed = sooner(sooner(timed, due), unread.after(&list));\n",
            "                timed = sooner(timed, unread.after(&list));\n",
            T_FOLLOW, "rust"),
    # Red: "an agent never read stops costing"
    Control("p39t_unread_tries_bounded", SPACES,
            "        (self.tries <= UNREAD_TRIES).then_some(UNREAD_EVERY)\n",
            "        Some(UNREAD_EVERY)\n",
            T_BOUNDED, "rust"),
    # Red: "a new unread agent"
    Control("p39t_unread_new_agents_try_again", SPACES,
            "            self.panes = now;\n            self.tries = 0;\n",
            "            self.panes = now;\n",
            T_BOUNDED, "rust"),
    # Red: "all read"
    Control("p39t_unread_none_when_all_read", SPACES,
            "        if self.panes.is_empty() {\n            return None;\n        }\n",
            "",
            T_BOUNDED, "rust"),
    # Item 1, a row that lost its tokens keeps the file its own token gave
    # Red: "the row changed and the transcript did not: {... "transcript":false}" (left "chat", right "agent")
    Control("p39t_chat_keeps_own_file", LIVE,
            "                                        Some(found) if own && spaces::unread_row(&found.agent) => {\n",
            "                                        Some(found) if false && spaces::unread_row(&found.agent) => {\n",
            T_FORGOTTEN, "rust"),
    # Red: "the row changed and the transcript did not" (no row reads as unread)
    Control("p39t_unread_row_is_no_kind", SPACES,
            "    token(agent, crate::nirvana_spaces::KIND_TOKEN).is_none()\n",
            "    agent.is_null()\n",
            T_FORGOTTEN, "rust"),
    # Red: "the record is not kept: {...}" (left "agent", right "chat")
    Control("p39t_chat_record_not_own", LIVE,
            "        let mut own = path.is_some() && by_token(&found.agent);\n",
            "        let mut own = path.is_some();\n",
            T_RECORD, "rust"),
    # Red: "the token's file is kept: {...}" (left "chat", right "agent")
    Control("p39t_chat_own_after_reresolve", LIVE,
            "                                            own = path.is_some() && by_token(&found.agent);\n",
            "                                            own = false;\n",
            T_ALONE, "rust"),
    # Item 2, the running card reads a working agent's transcript only
    # Red: left ["p1", "p2", "p3", "p4", "p5"], right ["p1", "p5"]
    Control("p39t_cards_working_only", CARDS,
            '            let at_work = (status == "working").then(|| activity(agent)).flatten();\n',
            "            let at_work = activity(agent);\n",
            T_CARDS, "rust"),
    # Item 2, a Claude Code on a local model is still a Claude to the phone (its kind token reads "local")
    # Red: "a local model's Claude Code"
    Control("p39t_local_kind_is_claude", CHAT_RS,
            "    matches!(kind, Some(Kind::Claude | Kind::Local))\n",
            "    matches!(kind, Some(Kind::Claude))\n",
            T_LOCAL, "rust"),
    # Item 3, an orchestrator and a worker in one folder
    # Red: "the orchestrator's own" (it reads the worker's file)
    Control("p39t_each_reads_its_own_token", CHAT_RS,
            '    if let Some(token) = agent["tokens"][crate::nirvana_spaces::TRANSCRIPT_TOKEN].as_str() {\n',
            "    if let Some(token) = others.iter().chain([agent])"
            '.find_map(|row| row["tokens"][crate::nirvana_spaces::TRANSCRIPT_TOKEN].as_str()) {\n',
            T_ONE_FOLDER, "rust"),
    # Red: "no token yet and a worker in the folder: the note" (left Some(<the record's file>), right None)
    Control("p39t_note_with_a_worker", CHAT_RS,
            "    if shared {\n        return None;\n    }\n",
            "    if shared && false {\n        return None;\n    }\n",
            T_ONE_FOLDER, "rust"),
    # Red: "p1: {... "transcript":false ...}" (the route reads another row's transcript)
    Control("p39t_route_reads_the_rows_own", LIVE,
            "            &found.agent,\n",
            "            &found.others[0],\n",
            T_ONE_FOLDER, "rust"),
    # Item 4, the "On the PC now" box (the served page's text; no browser runs here)
    # Red: "the chat knows when the box is all there is"
    Control("p39t_page_box_is_all", CHAT_JS,
            "          const only = !data.transcript && !(data.messages || []).length;\n",
            "          const only = !data.transcript;\n",
            T_PAGE, "rust"),
    # Red: "open from the start then, closed when there are bubbles"
    Control("p39t_page_box_opens", CHAT_JS,
            "          liveBox.classList.toggle('open', only);\n",
            "          liveBox.classList.toggle('open', false);\n",
            T_PAGE, "rust"),
    # Red: "closed, the box keeps its last lines"
    Control("p39t_page_box_last_lines", APP_CSS,
            ".live-box:not(.open) .live { display: flex; flex-direction: column; justify-content: flex-end; }\n",
            ".live-box:not(.open) .live { display: flex; flex-direction: column; justify-content: flex-start; }\n",
            T_PAGE, "rust"),
)
