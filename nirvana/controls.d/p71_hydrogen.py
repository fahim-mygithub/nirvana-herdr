# Plan 71: Gemini 4 Hydrogen takes Flash 3.8's seat. The owner, 2026-11-12: "it supposidly costs the same as 3.8" and
# "Hydrogen can do media level work replace flash". Controls starting with p71_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
TABLE = "src/nirvana_seats.toml"
SEAT = "src/nirvana_seat.rs"
SPACES = "src/nirvana_spaces.rs"
MEDIA = "src/nirvana_media.rs"

T_HYDROGEN = "nirvana_seat::tests::p71_hydrogen_takes_flashs_seat_at_the_same_cost"
T_DEFAULT = "nirvana_seat::tests::the_default_table_is_written_once_parses_and_holds_the_plans_seats_and_classes"
T_UPGRADE = "nirvana_seat::tests::c40a_the_shipped_default_is_new_and_every_old_one_upgrades"
T_RECORD = "nirvana_seat::tests::c40a_the_record_sets_the_sizes_and_two_scores"
T_LABEL = "nirvana_spaces::tests::model_label_table_reads_every_id_right"
T_ROWS = "nirvana_subagents::tests::labels_come_from_the_model_id_whatever_cli_runs_it"
T_PROMPT = "nirvana_media::tests::media_prompt_line_names_full_path_view_file_and_expected_return_fields"
T_DOCTOR = "nirvana_media::tests::c40c_media_doctor_names_tools_seats_and_clis"

_HYDROGEN_ON = ('model = "gemini-4-hydrogen-{effort}"\nfamily = "google"\npool = "gemini"\n'
                'efforts = ["low", "medium", "high", "xhigh"]\n'
                'capabilities = ["code", "tools", "image", "pdf", "web", "browser", "image_gen", "video", "audio", '
                '"context_1m"]\nenabled = true\n')

CONTROLS = (
    # The seat: on, its model, its pool, its efforts, its media capabilities; Flash off
    Control("p71_hydrogen_off", TABLE, _HYDROGEN_ON, _HYDROGEN_ON.replace("enabled = true", "enabled = false"),
            T_HYDROGEN, "rust"),
    Control("p71_hydrogen_off_default", TABLE, _HYDROGEN_ON,
            _HYDROGEN_ON.replace("enabled = true", "enabled = false"), T_DEFAULT, "rust"),
    Control("p71_hydrogen_model", TABLE, 'model = "gemini-4-hydrogen-{effort}"', 'model = "gemini-3.8-flash-{effort}"',
            T_HYDROGEN, "rust"),
    Control("p71_hydrogen_pool", TABLE, 'model = "gemini-4-hydrogen-{effort}"\nfamily = "google"\npool = "gemini"',
            'model = "gemini-4-hydrogen-{effort}"\nfamily = "google"\npool = "argon"', T_HYDROGEN, "rust"),
    Control("p71_no_xhigh", TABLE, 'pool = "gemini"\nefforts = ["low", "medium", "high", "xhigh"]\ncapabilities',
            'pool = "gemini"\nefforts = ["low", "medium", "high"]\ncapabilities', T_HYDROGEN, "rust"),
    Control("p71_no_video", TABLE, _HYDROGEN_ON, _HYDROGEN_ON.replace('"video", "audio", ', ""),
            T_HYDROGEN, "rust"),
    Control("p71_flash_on", TABLE, "every class still\n# lists it.\nenabled = false",
            "every class still\n# lists it.\nenabled = true", T_HYDROGEN, "rust"),
    # The price row: Flash's, said unverified
    Control("p71_price", TABLE, '{ from = "2026-11-12", input = 0.75,', '{ from = "2026-11-12", input = 2.0,',
            T_HYDROGEN, "rust"),
    # The cells: Flash's times the measured ratio, xhigh a prior from high
    Control("p71_cell_low", TABLE, 'windows = { "5h" = 0.0001157, week = 5.783e-05 }',
            'windows = { "5h" = 0.0001563, week = 7.815e-05 }', T_HYDROGEN, "rust"),
    Control("p71_cell_medium", TABLE, 'windows = { "5h" = 0.0003224, week = 0.0001612 }',
            'windows = { "5h" = 0.0003224, week = 0.000227 }', T_HYDROGEN, "rust"),
    Control("p71_cell_high", TABLE, 'windows = { "5h" = 0.0001535, week = 7.675e-05 }',
            'windows = { "5h" = 0.00026928, week = 7.675e-05 }', T_HYDROGEN, "rust"),
    Control("p71_cell_xhigh", TABLE, 'windows = { "5h" = 0.0002456, week = 0.0001228 }',
            'windows = { "5h" = 0.0002456, week = 0.0003 }', T_HYDROGEN, "rust"),
    # The classes: hydrogen just before flash, at flash's score; under the load-bearing bar with flash
    Control("p71_class_missing", TABLE, '  { seat = "hydrogen", score = 86 },\n', "", T_HYDROGEN, "rust"),
    Control("p71_class_score", TABLE, '  { seat = "hydrogen", score = 85 },\n', '  { seat = "hydrogen", score = 84 },\n',
            T_HYDROGEN, "rust"),
    Control("p71_load_bearing_score", TABLE,
            '  { seat = "hydrogen", score = 68 },\n  { seat = "flash", score = 68 },\n',
            '  { seat = "hydrogen", score = 74 },\n  { seat = "flash", score = 68 },\n', T_RECORD, "rust"),
    # The upgrade path: the 0.70.0 default is historical
    Control("p71_v14_not_historical", SEAT, '    include_str!("nirvana_seats_v14.toml"),\n];', "];",
            T_UPGRADE, "rust"),
    # Labels: the chip and the subagent row read "Hydrogen"
    Control("p71_chip_line", SPACES, '        } else if has("hydrogen") {\n            GeminiLine::Hydrogen\n', "",
            T_LABEL, "rust"),
    Control("p71_chip_word", SPACES, '        Family::Gemini(GeminiLine::Hydrogen) => "Hydrogen".to_string(),',
            '        Family::Gemini(GeminiLine::Hydrogen) => "Gemini".to_string(),', T_LABEL, "rust"),
    Control("p71_chip_bare", SPACES, ' || has("carbon") || has("hydrogen") {', ' || has("carbon") {', T_LABEL, "rust"),
    Control("p71_row_chip", SPACES, '        Family::Gemini(GeminiLine::Hydrogen) => "Hydrogen".to_string(),',
            '        Family::Gemini(GeminiLine::Hydrogen) => "Gemini".to_string(),', T_ROWS, "rust"),
    # Media: hydrogen is a word with flash's role line; the doctor names it for images
    Control("p71_media_word", MEDIA, '            "hydrogen" => Some(Self::Hydrogen),\n', "", T_PROMPT, "rust"),
    Control("p71_media_name", MEDIA, '            Self::Hydrogen => "hydrogen",', '            Self::Hydrogen => "flash",',
            T_PROMPT, "rust"),
    # Hydrogen's role line moved to carbon's arm (a removed arm would not compile, and a build error is no red line).
    Control("p71_media_role", MEDIA,
            "        MediaSeat::Carbon | MediaSeat::Argon => {\n"
            '            "watch the recording directly and cross-check relevant source files if needed."\n'
            "        }\n        MediaSeat::Hydrogen | MediaSeat::Flash => {",
            "        MediaSeat::Carbon | MediaSeat::Argon | MediaSeat::Hydrogen => {\n"
            '            "watch the recording directly and cross-check relevant source files if needed."\n'
            "        }\n        MediaSeat::Flash => {", T_PROMPT, "rust"),
    Control("p71_doctor_hydrogen_off", TABLE, _HYDROGEN_ON,
            _HYDROGEN_ON.replace("enabled = true", "enabled = false"), T_DOCTOR, "rust"),
)
