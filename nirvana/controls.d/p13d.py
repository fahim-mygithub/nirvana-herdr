# Plan 13 tuning (package p13d): the Preload reads names and code, presets combine, unedited defaults upgrade.
# One sabotage per assertion; each test named must print FAIL with its sabotage in place.
# `Control` is provided by controls.py.

PRESET = "src/nirvana_preset.rs"
PRESET_TESTS = "src/nirvana_preset/tests.rs"
SEAT = "src/nirvana_seat.rs"

WALK_P13D = "the_walk_skips_p13d_dependency_and_build_folders_case_insensitively"

CONTROLS = (
    # Item 1: dependency and build folders skipped by walk
    Control(
        "p13d_walk_skips_vendor",
        PRESET,
        '    "vendor",\n',
        '    "vendox",\n',
        WALK_P13D,
        "rust",
    ),
    # Item 2: words match on folder name and readme
    Control(
        "p13d_words_match",
        PRESET,
        "if matches_words(&when.words, &fp.folder_name, fp.readme.as_deref()) {",
        "if false && matches_words(&when.words, &fp.folder_name, fp.readme.as_deref()) {",
        "words_match_case_insensitively_on_folder_name_and_readme",
        "rust",
    ),
)
