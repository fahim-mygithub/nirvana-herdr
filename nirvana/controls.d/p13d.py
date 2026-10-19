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
    # Item 3: code preset with 24 extensions at 40 percent
    Control(
        "p13d_code_preset",
        "src/nirvana_preload.toml",
        'share = 0.4 }\nkeep = ["code", "web", "db", "python", "rust", "c", "kotlin"]',
        'share = 0.95 }\nkeep = ["code", "web", "db", "python", "rust", "c", "kotlin"]',
        "code_preset_matches_at_40_percent_across_24_extensions",
        "rust",
    ),
    # Item 4: matching presets combine, blank only as sole match
    Control(
        "p13d_presets_combine",
        PRESET,
        '        let keep: Vec<String> = parts.iter().flat_map(|p| p.keep.iter().cloned()).collect();',
        '        let keep: Vec<String> = Vec::new();',
        "matching_presets_combine_keep_tags_and_auto_memory_in_file_order",
        "rust",
    ),
    Control(
        "p13d_blank_sole_match",
        PRESET,
        '    if matching.len() > 1 {\n        matching.retain(|(p, _, _)| p.name != "blank");\n    }',
        '    if false {\n        matching.retain(|(p, _, _)| p.name != "blank");\n    }',
        "matching_presets_combine_keep_tags_and_auto_memory_in_file_order",
        "rust",
    ),
)
