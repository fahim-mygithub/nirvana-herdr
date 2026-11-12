# Plan 70: Gemini 4 Carbon takes Argon's seat. The owner, 2026-11-10: "Check the cost and if its the same it might be
# used to replace Argon." Controls starting with p70_. One sabotage per assertion, each on a product line; the test
# named must print FAIL with it in place. `Control` is provided by controls.py.
TABLE = "src/nirvana_seats.toml"
SEAT = "src/nirvana_seat.rs"
SPACES = "src/nirvana_spaces.rs"
MEDIA = "src/nirvana_media.rs"

T_CARBON = "nirvana_seat::tests::p70_carbon_takes_argons_seat_at_the_same_cost"
T_UPGRADE = "nirvana_seat::tests::c40a_the_shipped_default_is_new_and_every_old_one_upgrades"
T_UPGRADE_LRS = "nirvana_seat::tests::lrs_an_installed_34a_or_35c_default_upgrades_to_r2"
T_LABEL = "nirvana_spaces::tests::model_label_table_reads_every_id_right"
T_ROWS = "nirvana_subagents::tests::labels_come_from_the_model_id_whatever_cli_runs_it"
T_PROMPT = "nirvana_media::tests::media_prompt_line_names_full_path_view_file_and_expected_return_fields"
T_DOCTOR = "nirvana_media::tests::c40c_media_doctor_names_tools_seats_and_clis"

_CARBON_ON = ('pool = "gemini"\nefforts = ["low", "medium", "high"]\ncapabilities = ["code", "tools", "image", "pdf", '
              '"web", "browser", "image_gen", "video", "audio", "context_1m"]\nenabled = true\nprices = [\n'
              '  { from = "2026-10-28"')

CONTROLS = (
    # The seat: on, its model, its pool, Argon off
    Control("p70_carbon_off", TABLE, _CARBON_ON, _CARBON_ON.replace("enabled = true", "enabled = false"),
            T_CARBON, "rust"),
    Control("p70_carbon_model", TABLE, 'model = "gemini-4-carbon-{effort}"', 'model = "gemini-4-argon-{effort}"',
            T_CARBON, "rust"),
    Control("p70_carbon_pool", TABLE, 'model = "gemini-4-carbon-{effort}"\nfamily = "google"\npool = "gemini"',
            'model = "gemini-4-carbon-{effort}"\nfamily = "google"\npool = "argon"', T_CARBON, "rust"),
    Control("p70_argon_on", TABLE, "every class still lists it.\nenabled = false",
            "every class still lists it.\nenabled = true", T_CARBON, "rust"),
    # The price rows: Argon's, on both sides of 2026-11-30
    Control("p70_price_now", TABLE, '{ from = "2026-10-28", input = 2.0,', '{ from = "2026-10-28", input = 3.0,',
            T_CARBON, "rust"),
    Control("p70_price_december", TABLE,
            '{ from = "2026-11-30", input = 4.0, output = 20.0, cache_read = 0.20, cache_write = 4.0, basis = "the same',
            '{ from = "2026-11-30", input = 2.0, output = 20.0, cache_read = 0.20, cache_write = 4.0, basis = "the same',
            T_CARBON, "rust"),
    # The measured cell and its basis
    Control("p70_cell_low", TABLE, 'windows = { "5h" = 0.005983, week = 0.001498 }',
            'windows = { "5h" = 0.005983, week = 0.000167 }', T_CARBON, "rust"),
    Control("p70_cell_basis", TABLE, 'basis = "measured: plan 70 carbon low', 'basis = "prior: plan 70 carbon low',
            T_CARBON, "rust"),
    # The classes: carbon just before argon, at argon's score and effort
    Control("p70_class_missing", TABLE, '  { seat = "carbon", score = 87 },\n', "", T_CARBON, "rust"),
    Control("p70_class_effort", TABLE, '  { seat = "carbon", score = 85, effort = "high" },',
            '  { seat = "carbon", score = 85 },', T_CARBON, "rust"),
    # The upgrade path: the 0.68.0 default is historical
    Control("p70_v13_not_historical", SEAT, '    include_str!("nirvana_seats_v13.toml"),\n];', "];",
            T_UPGRADE, "rust"),
    Control("p70_v13_not_upgraded", SEAT, '    include_str!("nirvana_seats_v13.toml"),\n];', "];",
            T_UPGRADE_LRS, "rust"),
    # Labels: the chip and the subagent row read "Carbon"
    Control("p70_chip_line", SPACES, '        } else if has("carbon") {\n            GeminiLine::Carbon\n', "",
            T_LABEL, "rust"),
    Control("p70_chip_word", SPACES, '        Family::Gemini(GeminiLine::Carbon) => "Carbon".to_string(),',
            '        Family::Gemini(GeminiLine::Carbon) => "Gemini".to_string(),', T_LABEL, "rust"),
    Control("p70_chip_bare", SPACES, ' || has("argon") || has("carbon") {', ' || has("argon") {', T_LABEL, "rust"),
    Control("p70_row_chip", SPACES, '        Family::Gemini(GeminiLine::Carbon) => "Carbon".to_string(),',
            '        Family::Gemini(GeminiLine::Carbon) => "Gemini".to_string(),', T_ROWS, "rust"),
    # Media: carbon is a word, the default, and its own role name; the doctor names it for video
    Control("p70_media_default", MEDIA, "pub const DEFAULT_SEAT: MediaSeat = MediaSeat::Carbon;",
            "pub const DEFAULT_SEAT: MediaSeat = MediaSeat::Argon;", T_PROMPT, "rust"),
    Control("p70_media_word", MEDIA, '            "carbon" => Some(Self::Carbon),\n', "", T_PROMPT, "rust"),
    Control("p70_media_role", MEDIA, '            Self::Carbon => "carbon",', '            Self::Carbon => "argon",',
            T_PROMPT, "rust"),
    Control("p70_doctor_carbon_off", TABLE, _CARBON_ON, _CARBON_ON.replace("enabled = true", "enabled = false"),
            T_DOCTOR, "rust"),
)
