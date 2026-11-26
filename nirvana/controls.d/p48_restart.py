# Plan 48 (2026-11-25): the owner, "Is there a way to restart all of nirvana and add an option to do so". Controls
# starting with p48_ for the restart, and with p39v_ for the phone chat that asks again about a row the server forgot
# (review note 2 of alpha 0.95.1). One sabotage per assertion, each on a product line; the test named must print FAIL
# with it in place, for the reason in the comment above it (the panic text expected), not a compile error. `Control`
# is provided by controls.py.
LIVE = "src/nirvana_remote/live.rs"

_T = "nirvana_remote::tests_live::"
T_FORGOTTEN_AT_OPEN = _T + "a_chat_opened_on_a_forgotten_row_asks_again_and_takes_the_token"
T_NEVER_READ = _T + "a_chat_on_a_row_never_read_asks_a_bounded_number_of_times"

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
)
