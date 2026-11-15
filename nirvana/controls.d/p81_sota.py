# Plan 81: in SOTA the seat picker drops the rules that save tokens or limit the orchestrator's routing, and honours an
# override on any seat that can run the order. The owner, 2026-11-15: "Make a change to Nirvana if SOTA is running
# ignore any rules and allow full overide. I am actively making the choie to burn through token If I leave it on SOTA."
# Controls starting with p81_. One sabotage per assertion, each on a product line; the test named must print FAIL with
# it in place. `Control` is provided by controls.py.
SEAT = "src/nirvana_seat.rs"

T_POLICY = "nirvana_seat::tests::p81_sota_honours_any_override_policy_would_refuse"
T_RULE = "nirvana_seat::tests::p81_sota_rule_pick_spends_past_reserve_and_floor"
T_FACT = "nirvana_seat::tests::p81_sota_still_refuses_an_override_the_seat_cannot_run"
T_STEP = "nirvana_seat::tests::p81_sota_never_steps_an_effort_down"
T_CAP = "nirvana_seat::tests::an_override_passes_every_filter_and_three_are_taken_per_run"
T_FABLE = "nirvana_seat::tests::c40a_fable_is_skipped_unless_its_windows_are_fresh_and_its_reserve_holds"

CONTROLS = (
    # The lifts: the cap, the class's list, only_with, decision 0017's family, the orchestrator's reserve
    Control("p81_cap_in_sota", SEAT,
            "        if request.mode != Mode::Sota\n            && !request.inherited_override\n",
            "        if !request.inherited_override\n", T_POLICY, "rust"),
    Control("p81_unlisted_refused", SEAT, "        None if sota_override => {", "        None if false => {",
            T_POLICY, "rust"),
    Control("p81_unlisted_reason", SEAT,
            '"; {override_target} is not in {}\'s seats, taken because SOTA honours any override",',
            '"; {override_target} {}",', T_POLICY, "rust"),
    Control("p81_only_with_in_sota", SEAT, "    if !sota_override && !entry.only_with.iter()",
            "    if !entry.only_with.iter()", T_POLICY, "rust"),
    Control("p81_family_in_sota", SEAT, "    if !sota_override && request.not_family.contains(&seat.family) {",
            "    if request.not_family.contains(&seat.family) {", T_POLICY, "rust"),
    Control("p81_reserve_in_sota", SEAT,
            "        if request.mode == Mode::Sota || seat.pool != constants.reserve_pool {",
            "        if seat.pool != constants.reserve_pool {", T_POLICY, "rust"),
    # The rule's pick: the reserve line, the floor's margin, no step down
    Control("p81_reserve_line", SEAT,
            '        reason.push_str(&format!(", reserve {} lifted (SOTA)", pct(constants.reserve)));',
            '        reason.push_str(&format!(", reserve {}", pct(constants.reserve)));', T_RULE, "rust"),
    Control("p81_floor_in_sota", SEAT,
            "    let sota = request.mode == Mode::Sota && seat.extra_windows.is_empty();",
            "    let sota = false;", T_RULE, "rust"),
    Control("p81_step_down_in_sota", SEAT,
            "            if first.priced && request.override_seat.is_none() && request.mode != Mode::Sota =>",
            "            if first.priced && request.override_seat.is_none() =>", T_STEP, "rust"),
    # The cap's refusal outside SOTA names how to lift it
    Control("p81_cap_message", SEAT, "; more is the owner's call (SOTA mode lifts this cap)\",",
            "; more is the owner's call\",", T_CAP, "rust"),
    # The facts SOTA keeps: a window used up, Fable's floor (money), off, CLI, a capability
    Control("p81_used_up_lifted", SEAT, "                window.used.is_some_and(|used| used + EDGE >= 1.0)",
            "                false", T_FACT, "rust"),
    Control("p81_fable_floor_lifted", SEAT,
            "    let sota = request.mode == Mode::Sota && seat.extra_windows.is_empty();",
            "    let sota = request.mode == Mode::Sota;", T_FABLE, "rust"),
    Control("p81_off_lifted", SEAT, '        cand.skip = skip(SkipKind::Off, "off in seats.toml".into());',
            '        cand.skip = skip(SkipKind::Off, "off in seats.toml".into()).filter(|_| request.mode != Mode::Sota);',
            T_FACT, "rust"),
    Control("p81_cli_lifted", SEAT, "        cand.skip = skip(SkipKind::Cli, why.clone());",
            "        cand.skip = skip(SkipKind::Cli, why.clone()).filter(|_| request.mode != Mode::Sota);",
            T_FACT, "rust"),
    Control("p81_need_lifted", SEAT, '        cand.skip = skip(SkipKind::Need, format!("lacks {need}"));',
            '        cand.skip = skip(SkipKind::Need, format!("lacks {need}")).filter(|_| request.mode != Mode::Sota);',
            T_FACT, "rust"),
)
