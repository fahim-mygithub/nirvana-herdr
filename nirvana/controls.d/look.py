# The plain Claude Code look (fix after alpha 0.33.0): the owner, 2026-10-21, "I would like the standard Claude Code
# color scheme back ... and the font as well". Controls starting with look_.

CONTROLS = (
    # Nirvana Claude takes the owner's own Claude Code theme: no theme key in the plugin's settings
    Control(
        'look_claude_theme_left_to_claude_code',
        'nirvana/claude/settings.json',
        '{\n  "env": {\n',
        '{\n  "theme": "custom:nirvana-dark",\n  "env": {\n',
        'nirvana.test_windows_shell.FilesTest.test_settings_carry_the_orchestrator_env_and_leave_the_theme_to_claude_code',
        'python',
    ),
    # The Terminal profile sets no font or colours, so Windows Terminal's defaults apply
    Control(
        'look_profile_takes_terminal_defaults',
        'nirvana/windows_shell.py',
        '        "suppressApplicationTitle": True,\n    }\n',
        '        "suppressApplicationTitle": True,\n        "font": {"face": "JetBrains Mono", "size": 11},\n    }\n',
        'nirvana.test_windows_shell.FilesTest.test_themes_fragment_and_assets_land_where_claude_and_terminal_read_them',
        'python',
    ),
)
