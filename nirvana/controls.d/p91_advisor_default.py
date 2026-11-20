# The advisor default (the owner, 2026-11-18: "Go ahead and build in the consulting it might be cheaper in the long run
# for balanced runs") and the stated worker ceiling (plan 43 ruling R1, "1. yes"). Controls starting with p91_. One
# sabotage per assertion, each on a product line; the test named in the fifth field is the one expected to print its
# red line with the sabotage in place (written before the run). `Control` is provided by controls.py.
TABLE = "src/nirvana_seats.toml"
V17 = "src/nirvana_seats_v17.toml"
SEAT = "src/nirvana_seat.rs"
WORKER = "src/nirvana_worker.rs"

T_DEFAULT = "nirvana_seat::tests::p91_the_default_gives_sonnet_an_opus_advisor_in_balanced_only"
T_CEILING = "nirvana_seat::tests::p91_the_default_states_the_worker_ceiling"
T_V17 = "nirvana_seat::tests::p91_v17_is_last_and_v16_still_historical"
T_PICKS = "nirvana_seat::tests::p91_the_advisor_moves_no_pick_and_no_expected_use"
T_ARGV = "nirvana_worker::tests::p91_the_default_tables_worker_argv_has_the_advisor_for_balanced_sonnet_only"

_ADVISOR = 'advisor = { balanced = "opus" }\n'

CONTROLS = (
    # D1, the table: sonnet has an Opus advisor in Balanced, none in Efficient or SOTA, no other seat has one
    Control("p91_no_balanced_advisor", TABLE, _ADVISOR, "", T_DEFAULT, "rust"),
    Control("p91_efficient_advisor", TABLE, _ADVISOR, 'advisor = { balanced = "opus", efficient = "opus" }\n',
            T_DEFAULT, "rust"),
    Control("p91_sota_advisor", TABLE, _ADVISOR, 'advisor = { balanced = "opus", sota = "fable" }\n',
            T_DEFAULT, "rust"),
    Control("p91_other_seat_advisor", TABLE, 'id = "haiku"\n', 'id = "haiku"\n' + _ADVISOR, T_DEFAULT, "rust"),
    # The advisor the table resolves is the opus seat itself
    Control("p91_advisor_resolves_elsewhere", SEAT, "Some(advisor) if advisor.enabled => Ok(advisor),",
            'Some(advisor) if advisor.enabled => Ok(self.seat("haiku").unwrap_or(advisor)),', T_DEFAULT, "rust"),
    # The hunk the p90 tests build from is the table's own text, so p90_default takes its early return
    Control("p91_hunk_drifts", TABLE, "at a 25k context). A hypothesis.\nadvisor",
            "at a 25k context). A guess.\nadvisor", T_DEFAULT, "rust"),
    # D2: the default states the ceiling, 300000, in [constants]; 0.85.0 had none
    Control("p91_ceiling_unstated", TABLE, "worker_autocompact_tokens = 300000\n", "", T_CEILING, "rust"),
    Control("p91_ceiling_value", TABLE, "worker_autocompact_tokens = 300000\n", "worker_autocompact_tokens = 250000\n",
            T_CEILING, "rust"),
    Control("p91_constants_renamed", TABLE, "\n[constants]\n", "\n[constantz]\n", T_CEILING, "rust"),
    Control("p91_v17_had_ceiling", V17, "autocompact_tokens = 0\n",
            "autocompact_tokens = 0\nworker_autocompact_tokens = 300000\n", T_CEILING, "rust"),
    # The upgrade path: v17 last, v16 still historical, an installed v17 upgrades
    Control("p91_v17_not_historical", SEAT, '    include_str!("nirvana_seats_v17.toml"),\n', "", T_V17, "rust"),
    Control("p91_v16_dropped", SEAT, '    include_str!("nirvana_seats_v16.toml"),\n', "", T_V17, "rust"),
    Control("p91_upgrade_writes_nothing", SEAT, "    if let Err(err) = std::fs::rename(&temp, path) {",
            "    if let Err(err) = std::fs::remove_file(&temp) {", T_V17, "rust"),
    # Step 4: the advisor is not in the cost cell; a sabotage that puts it there moves the pick's expected use
    Control("p91_advisor_in_cost", SEAT, "        * class.weight\n",
            "        * class.weight\n        * if seat.advisor.at(request.mode).is_some() { 2.0 } else { 1.0 }\n",
            T_PICKS, "rust"),
    # D1, the worker argv on the shipped table
    Control("p91_argv_no_advisor", TABLE, _ADVISOR, "", T_ARGV, "rust"),
    Control("p91_argv_efficient", TABLE, _ADVISOR, 'advisor = { balanced = "opus", efficient = "opus" }\n',
            T_ARGV, "rust"),
    Control("p91_argv_sota", TABLE, _ADVISOR, 'advisor = { balanced = "opus", sota = "opus" }\n', T_ARGV, "rust"),
    Control("p91_argv_line", WORKER, "prompt.push_str(ADVISOR_LINE);", 'prompt.push_str("");', T_ARGV, "rust"),
    Control("p91_argv_opus_order", WORKER,
            "        &project.root,\n        &seated.seat,\n        seated.mode,\n        seated.effort,\n",
            '        &project.root,\n        "sonnet",\n        seated.mode,\n        seated.effort,\n', T_ARGV, "rust"),
)
