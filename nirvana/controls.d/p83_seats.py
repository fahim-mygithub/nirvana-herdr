# The seats refresh (2026-11-18). The owner: "update our Nirvana on new models for seating, use our megajam to update
# any past seats ther emight also be a new seat potentially called gemini 4 neon? Also agy updated ... 1.5.11".
# Controls starting with p83_. One sabotage per new assertion, each on a product line; the test named in the fifth
# field is the one expected to print its red line with the sabotage in place (written before the run). `Control` is
# provided by controls.py.
TABLE = "src/nirvana_seats.toml"
SEAT = "src/nirvana_seat.rs"
PLUGIN = "nirvana/agy_plugin.py"

T_SEATS = "nirvana_seat::tests::p83_seats_refresh_models_prices_refits_and_scores"
T_UPGRADE = "nirvana_seat::tests::c40a_the_shipped_default_is_new_and_every_old_one_upgrades"
T_CARBON = "nirvana_seat::tests::p70_carbon_takes_argons_seat_at_the_same_cost"
T_BOM = "nirvana.test_agy_plugin.AgyFilesTest.test_plugin_json_has_no_byte_order_mark"
T_REPRICE = "nirvana.test_scripts.SeatRefreshTest.test_hydrogens_cells_are_the_probe_at_its_own_price"

_HYDROGEN_ROW = ('  { from = "2026-11-12", input = 1.50, output = 7.50, cache_read = 0.075, cache_write = 1.50, basis = '
                 '"Google\'s Gemini API pricing page for gemini-4-hydrogen, read 2026-11-18: $1.50 in, $7.50 out with '
                 'thinking, $0.075 cache, implicit caching only; cache write at input, a prior" },\n]')

CONTROLS = (
    # S1: every seat's model at every effort is one its CLI lists today (agy 1.5.11, Claude Code 2.1.327)
    Control("p83_agy_model_unlisted", TABLE, 'model = "claude-sonnet-5-5-{effort}"',
            'model = "claude-sonnet-5-6-{effort}"', T_SEATS, "rust"),
    Control("p83_claude_model_unlisted", TABLE, 'model = "claude-opus-5-5"', 'model = "claude-opus-5-6"',
            T_SEATS, "rust"),
    # Flash 3.8's price doubles on 2027-01-01; Hydrogen's has no such row
    Control("p83_flash_2027_input", TABLE,
            '  { from = "2027-01-01", input = 1.50, output = 7.50, cache_read = 0.15,',
            '  { from = "2027-01-01", input = 0.75, output = 7.50, cache_read = 0.15,', T_SEATS, "rust"),
    Control("p83_flash_2027_cache", TABLE,
            '  { from = "2027-01-01", input = 1.50, output = 7.50, cache_read = 0.15,',
            '  { from = "2027-01-01", input = 1.50, output = 7.50, cache_read = 0.075,', T_SEATS, "rust"),
    Control("p83_hydrogen_2027_row", TABLE, _HYDROGEN_ROW,
            _HYDROGEN_ROW[:-1] + '  { from = "2027-01-01", input = 3.0, output = 15.0, cache_read = 0.15, '
            'cache_write = 3.0, basis = "x" },\n]', T_SEATS, "rust"),
    # S2: the record's refits and retags
    Control("p83_carbon_high_cell", TABLE, 'windows = { "5h" = 0.00242, week = 0.0006059 }',
            'windows = { "5h" = 0.00242, week = 0.002885 }', T_SEATS, "rust"),
    Control("p83_carbon_high_n", TABLE, "refit x0.21 to carbon high's own orders (seat_fit.py, n 15)",
            "refit x0.21 to carbon high's own orders (seat_fit.py, n 5)", T_SEATS, "rust"),
    Control("p83_carbon_high_prior", TABLE, 'basis = "derived: medium x 1.6, refit x0.21',
            'basis = "prior: medium x 1.6, refit x0.21', T_CARBON, "rust"),
    Control("p83_opus_high_refit", TABLE, "fit x7.1 (seat_fit.py, n 9)", "fit x16.05 (seat_fit.py, n 63)",
            T_SEATS, "rust"),
    Control("p83_fable_high_prior", TABLE, 'basis = "derived: medium x 1.6, fit x7.1 (seat_fit.py: fable',
            'basis = "prior: medium x 1.6, fit x7.1 (seat_fit.py: fable', T_SEATS, "rust"),
    # S2: the two scores run ia1 moved
    Control("p83_sonnet_load_bearing", TABLE, '  { seat = "sonnet", score = 68 },',
            '  { seat = "sonnet", score = 85 },', T_SEATS, "rust"),
    Control("p83_carbon_bounded", TABLE, '  { seat = "carbon", score = 86 },\n  { seat = "argon", score = 84 },',
            '  { seat = "carbon", score = 84 },\n  { seat = "argon", score = 84 },', T_SEATS, "rust"),
    # The upgrade path: the 0.78.2 default is historical
    Control("p83_v15_not_historical", SEAT, '    include_str!("nirvana_seats_v15.toml"),\n', "", T_UPGRADE, "rust"),
    # Hydrogen's cells are plan 71's probe at each model's own price (nirvana/seat_refresh.py)
    Control("p83_hydrogen_low_reprice", TABLE, 'windows = { "5h" = 0.0002235, week = 0.0001118 }',
            'windows = { "5h" = 0.0002235, week = 5.783e-05 }', T_REPRICE, "python"),
    # S4: agy up to 1.5.10 did not load a plugin.json with a BOM
    Control("p83_plugin_json_bom", PLUGIN, 'path.write_bytes(text.encode("utf-8"))',
            'path.write_bytes(text.encode("utf-8-sig"))', T_BOM, "python"),
)
