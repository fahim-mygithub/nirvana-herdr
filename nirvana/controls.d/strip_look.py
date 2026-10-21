# One-line space rows and the coloured strip (fix after alpha 0.33.1). The owner, 2026-10-21: "Can we instead pu
# that in a single line like Icon of the model then Nirvana - Claude Sonnet 5.5" and "improve the readbility maybe
# include bg color or text color for this top module toolbar". Controls starting with sl_.

CONTROLS = (
    # A space's agent takes one row
    Control(
        'sl_row_two_rows',
        'src/client/shell/nirvana_spaces.rs',
        'pub(super) const ROWS_PER_AGENT: usize = 1;',
        'pub(super) const ROWS_PER_AGENT: usize = 2;',
        'each_agent_adds_one_row_to_its_space',
        'rust',
    ),
    # A row that does not fit drops the model's provider word first
    Control(
        'sl_row_keeps_provider_word',
        'src/client/shell/nirvana_spaces.rs',
        "    let model = model.split_once(' ').map_or(model, |(_, rest)| rest);\n",
        '    let model = model;\n',
        'the_row_drops_the_provider_word_before_it_cuts',
        'rust',
    ),
    # Then the task is cut, so the model stays readable
    Control(
        'sl_row_cuts_the_model',
        'src/client/shell/nirvana_spaces.rs',
        '    if budget >= MIN_TASK {\n',
        '    if false && budget >= MIN_TASK {\n',
        'the_row_drops_the_provider_word_before_it_cuts',
        'rust',
    ),
    # Commits behind read red
    Control(
        'sl_git_behind_not_red',
        'src/client/shell/nirvana_mods.rs',
        '        marks.push(mark(format!("\\u{2193}{}", status.behind), Tone::Bad));\n',
        '        marks.push(mark(format!("\\u{2193}{}", status.behind), Tone::Info));\n',
        'the_strip_colours_the_mode_and_the_commits_behind',
        'rust',
    ),
    # Efficient reads green
    Control(
        'sl_mode_efficient_not_green',
        'src/client/shell/nirvana_mods.rs',
        '        crate::nirvana_seat::Mode::Efficient => Tone::Good,\n',
        '        crate::nirvana_seat::Mode::Efficient => Tone::Info,\n',
        'the_strip_colours_the_mode_and_the_commits_behind',
        'rust',
    ),
    # The marks are drawn on the square
    Control(
        'sl_marks_not_drawn',
        'src/client/shell/nirvana_mods.rs',
        '    tint(buffer, y, after, square.right(), marks, palette);\n',
        '',
        'the_strip_puts_running_before_preload_and_says_when_claude_waits',
        'rust',
    ),
)
