# The Neon seat (2026-11-18). The owner: "there might also be a new seat potentially called gemini 4 neon", "you may
# need to lookup its cost and capabilities" and "neon high sscores very well benchmark?". Controls starting with p89_.
# One sabotage per assertion, each on a product line; the test named in the fifth field is the one expected to print its
# red line with the sabotage in place (written before the run). `Control` is provided by controls.py.
TABLE = "src/nirvana_seats.toml"
SEAT = "src/nirvana_seat.rs"
REFRESH = "nirvana/seat_refresh.py"

T_NEON = "nirvana_seat::tests::p89_neon_takes_a_seat_at_googles_price"
T_SCORES = "nirvana_seat::tests::p89_neon_scores_follow_the_rule_and_move_no_other_seat"
T_PICKS = "nirvana_seat::tests::p89_neon_moves_no_sota_pick_and_these_others"
T_LISTED = "nirvana_seat::tests::p83_seats_refresh_models_prices_refits_and_scores"
T_UPGRADE = "nirvana_seat::tests::c40a_the_shipped_default_is_new_and_every_old_one_upgrades"
T_REFRESH = "nirvana.test_scripts.SeatRefreshTest.test_neons_cells_and_scores_are_the_probe_and_the_rule"

_CAPS = 'capabilities = ["code", "tools", "image", "pdf", "web", "browser", "video", "audio", "context_1m"]'
_ON = _CAPS + "\nenabled = true"
_PRICE = '{ from = "2026-11-18", input = 3.0, output = 15.0, cache_read = 0.15, cache_write = 3.0,'
_LOW = '{ effort = "low", windows = { "5h" = 0.002022, week = 0.0005065 }'
_MEDIUM = '{ effort = "medium", windows = { "5h" = 0.002345, week = 0.0005868 }'
_HIGH = '{ effort = "high", windows = { "5h" = 0.002345, week = 0.0005868 }'
_XHIGH = '{ effort = "xhigh", windows = { "5h" = 0.002345, week = 0.0005868 }'

CONTROLS = (
    # The seat: on, its model, its pool, its four efforts, each id on agy's list; the Orchestrator id gets no seat
    Control("p89_neon_off", TABLE, _ON, _CAPS + "\nenabled = false", T_NEON, "rust"),
    Control("p89_neon_model", TABLE, 'model = "gemini-4-neon-{effort}"', 'model = "gemini-4-neon-pro-{effort}"',
            T_NEON, "rust"),
    Control("p89_neon_model_unlisted", TABLE, 'model = "gemini-4-neon-{effort}"',
            'model = "gemini-4-neon-pro-{effort}"', T_LISTED, "rust"),
    Control("p89_neon_pool", TABLE, 'model = "gemini-4-neon-{effort}"\nfamily = "google"\npool = "gemini"',
            'model = "gemini-4-neon-{effort}"\nfamily = "google"\npool = "argon"', T_NEON, "rust"),
    Control("p89_neon_no_xhigh", TABLE, 'efforts = ["low", "medium", "high", "xhigh"]\n' + _CAPS,
            'efforts = ["low", "medium", "high"]\n' + _CAPS, T_NEON, "rust"),
    Control("p89_agy_list_xhigh", SEAT, '        "gemini-4-neon-xhigh",\n', "", T_LISTED, "rust"),
    Control("p89_agy_list_orchestrator", SEAT, '        "gemini-pro-orchestrator-neon",\n', "", T_NEON, "rust"),
    Control("p89_orchestrator_seated", TABLE, 'model = "gpt-oss-120b-medium"', 'model = "gemini-pro-orchestrator-neon"',
            T_NEON, "rust"),
    # The capabilities: Google's model page, video and audio in, no image generation
    Control("p89_neon_image_gen", TABLE, _ON, _CAPS[:-1] + ', "image_gen"]\nenabled = true', T_NEON, "rust"),
    Control("p89_neon_no_video", TABLE, _ON, _ON.replace('"video", ', ""), T_NEON, "rust"),
    # The price row: Google's page, one row, the page named
    Control("p89_price_input", TABLE, _PRICE, _PRICE.replace("input = 3.0,", "input = 2.0,"), T_NEON, "rust"),
    Control("p89_price_basis", TABLE, "Google's Gemini API pricing page (ai.google.dev/gemini-api/docs/pricing) for "
            "gemini-4-neon", "launch press for gemini-4-neon", T_NEON, "rust"),
    Control("p89_price_second_row", TABLE, "cache write at input, a prior\" },\n]\n# Fourteen orders",
            "cache write at input, a prior\" },\n  { from = \"2026-12-01\", input = 6.0, output = 30.0, cache_read = "
            "0.30, cache_write = 6.0, basis = \"x\" },\n]\n# Fourteen orders", T_NEON, "rust"),
    # The cells: Carbon's times the measured ratio, high and xhigh raised to medium's, every one a prior
    Control("p89_cell_low", TABLE, _LOW, _LOW.replace("week = 0.0005065", "week = 0.0003146"), T_NEON, "rust"),
    Control("p89_cell_medium", TABLE, _MEDIUM, _MEDIUM.replace("week = 0.0005868", "week = 0.0003786"),
            T_NEON, "rust"),
    Control("p89_cell_high_unraised", TABLE, _HIGH, _HIGH.replace("week = 0.0005868", "week = 0.0003817"),
            T_NEON, "rust"),
    Control("p89_cell_xhigh", TABLE, _XHIGH, _XHIGH.replace('"5h" = 0.002345', '"5h" = 0.002603'), T_NEON, "rust"),
    Control("p89_cell_monotonic", TABLE, _LOW, _LOW.replace('"5h" = 0.002022', '"5h" = 0.003'), T_NEON, "rust"),
    Control("p89_cell_prior", TABLE, "basis = \"prior: carbon low's cell", "basis = \"derived: carbon low's cell",
            T_NEON, "rust"),
    # The upgrade path: the 0.79.0 default is the last historical one
    Control("p89_v16_not_historical", SEAT, '    include_str!("nirvana_seats_v16.toml"),\n', "", T_NEON, "rust"),
    Control("p89_v16_not_last", SEAT, '    include_str!("nirvana_seats_v16.toml"),\n', "", T_UPGRADE, "rust"),
    # The scores, by the rule: carbon plus 2, capped by opus and fable, else one under the best other seat
    Control("p89_score_plus_two", TABLE, '  { seat = "neon", score = 89 },\n  { seat = "carbon", score = 87 },',
            '  { seat = "neon", score = 90 },\n  { seat = "carbon", score = 87 },', T_SCORES, "rust"),
    Control("p89_score_claude_cap", TABLE, '  { seat = "neon", score = 82 },\n  { seat = "carbon", score = 84 },',
            '  { seat = "neon", score = 86 },\n  { seat = "carbon", score = 84 },', T_SCORES, "rust"),
    Control("p89_score_top_cap", TABLE, '  { seat = "neon", score = 89 },\n  { seat = "carbon", score = 90 },',
            '  { seat = "neon", score = 92 },\n  { seat = "carbon", score = 90 },', T_SCORES, "rust"),
    Control("p89_score_effort", TABLE, '  { seat = "neon", score = 87, effort = "high" },',
            '  { seat = "neon", score = 87 },', T_SCORES, "rust"),
    Control("p89_score_missing", TABLE, '  { seat = "neon", score = 84 },\n  { seat = "carbon", score = 86 },',
            '  { seat = "carbon", score = 86 },', T_SCORES, "rust"),
    Control("p89_score_unlisted_class", TABLE, 'seats = [\n  { seat = "sonnet", score = 88 },\n  { seat = "opus", '
            'score = 90 },', 'seats = [\n  { seat = "neon", score = 80 },\n  { seat = "sonnet", score = 88 },\n  { '
            'seat = "opus", score = 90 },', T_SCORES, "rust"),
    Control("p89_other_score_moved", TABLE, '  { seat = "sonnet", score = 86 },\n  { seat = "hydrogen", score = 74 },',
            '  { seat = "sonnet", score = 87 },\n  { seat = "hydrogen", score = 74 },', T_SCORES, "rust"),
    # The picks: SOTA's moves in none; Balanced build.load_bearing moves to neon high
    Control("p89_sota_pick_moves", TABLE, '  { seat = "neon", score = 92 },', '  { seat = "neon", score = 97 },',
            T_PICKS, "rust"),
    # The first run of a score sabotage here (neon 88 to 85) stayed green: the climb lowers Balanced's bar on this
    # world, so the pick goes by cost, and neon high's cell is the cheaper. The sabotage puts it over carbon high's.
    Control("p89_balanced_pick_by_cost", TABLE, _HIGH, _HIGH.replace('"5h" = 0.002345, week = 0.0005868',
            '"5h" = 0.0025, week = 0.000626'), T_PICKS, "rust"),
    # The script that holds the numbers (nirvana/seat_refresh.py neon): the floor and the cap
    Control("p89_refresh_floor", REFRESH, "cells[higher] = {w: max(cells[higher][w], cells[lower][w])",
            "cells[higher] = {w: cells[higher][w] + 0 * cells[lower][w]", T_REFRESH, "python"),
    # The first run (min to max) stayed green: every class the cap binds lists opus alone. This lifts the cap.
    Control("p89_refresh_cap", REFRESH, "cap = min(claude) if claude else", "cap = 100 if claude else",
            T_REFRESH, "python"),
    Control("p89_refresh_probe", REFRESH, '    ("gemini-4-neon-high", 15494, 10424, 1147),',
            '    ("gemini-4-neon-high", 15494, 10424, 4147),', T_REFRESH, "python"),
)
