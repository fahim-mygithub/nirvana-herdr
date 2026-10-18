# The owner, 2026-10-18: "We dont need tracking for g claude or argon on the usage keep that internally."
# Red line: the foot test asserts neither row is drawn and the accounts button sits two rows higher.
CONTROLS = (
    Control("footer_hidden_pools_drawn", "src/nirvana_usage.rs",
            "rows.retain(|row| !FOOT_HIDDEN.contains(&row.pool.as_str()));", "rows.retain(|_| true);",
            "the_foot_lists_a_spaces_server_and_a_usage_row_per_pool", "rust"),
)
