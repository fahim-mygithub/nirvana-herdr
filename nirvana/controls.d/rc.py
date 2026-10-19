# The owner, 2026-10-19: "Use Nirvana Claude instead" - Nirvana Claude starts with the owner's --rc and --chrome.
# Red line: the argv test prints FAIL.
ARGV = 'nirvana_claude_takes_the_plugin_agent_name_and_settings_from_home'

CONTROLS = (
    Control('rc_flag_missing', 'src/nirvana_launch.rs',
            '        "--rc".into(),\n        "--chrome".into(),\n    ]\n',
            '        "--chrome".into(),\n    ]\n',
            ARGV, "rust"),
    Control('rc_chrome_missing', 'src/nirvana_launch.rs',
            '        "--rc".into(),\n        "--chrome".into(),\n    ]\n',
            '        "--rc".into(),\n    ]\n',
            ARGV, "rust"),
)
