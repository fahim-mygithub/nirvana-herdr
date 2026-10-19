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
    # 0.25.3: the A/B bench leaves the owner-only flags out, and its launch test still holds the rest to the Rust.
    Control('rc_bench_takes_owner_flags', 'nirvana/test_ab.py',
            '        flags = [f for f in flags if f not in ab.OWNER_ONLY_FLAGS]\n',
            '',
            'nirvana.test_ab.LaunchTest.test_arm_n_launches_what_the_rust_launch_says', "python"),
)
