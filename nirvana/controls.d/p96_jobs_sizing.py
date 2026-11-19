# Jobs tab sizing (the owner, 2026-11-19: "Can we fix the sizing?"). Each `herdr job run` split the Jobs tab's last pane
# in half: six job panes at 1/2, 1/4, 1/8, 1/16, 1/32 and 1/32 of the width, the two running jobs about ten columns
# wide, four finished jobs at an idle prompt holding 15/16 of the screen. Controls starting with p96_. One sabotage per
# assertion, each on a product line; the test named in the fifth field is the one expected to print its red line with
# the sabotage in place (written before the run). `Control` is provided by controls.py.
LAYOUT = "src/layout.rs"
API = "src/app/api/panes.rs"
BINDINGS = "src/cli/pane/bindings.rs"
DEAD = "src/nirvana_dead.rs"
JOB = "src/nirvana_job.rs"

T_COLUMNS = "layout::tests::p96_even_columns_cap_at_three_and_the_readable_width"
T_ROWS = "layout::tests::p96_even_rows_fill_the_first_rows_and_differ_by_one"
T_READABLE = "layout::tests::p96_every_pane_is_readable_and_even_up_to_twelve"
T_SIX = "layout::tests::p96_six_jobs_tile_as_two_rows_of_three"
T_CLOSE = "layout::tests::p96_a_close_in_an_even_tab_grows_the_survivors_back"
T_OWNER = "layout::tests::p96_the_owners_own_split_or_resize_ends_the_even_grid"
T_API = "app::api::panes::tests::p96_api_pane_tile_evens_the_tab_and_keeps_focus"
T_CLI = "cli::pane::tests::pane_projection_all_leaf_request_values"
T_PASSED = "nirvana_dead::tests::p96_a_passed_jobs_pane_closes_after_the_hold"
T_FAILED = "nirvana_dead::tests::p96_failed_jobs_keep_the_newest_two_per_tab"
T_RUNNING = "nirvana_dead::tests::p96_a_running_jobs_pane_is_never_closed"
T_RANK = "nirvana_dead::tests::p96_failed_jobs_over_ranks_each_tab_by_end"
T_LOST = "nirvana_dead::tests::p96_a_lost_job_counts_as_failed"
T_PLACEMENT = "nirvana_job::tests::p96_an_exec_write_keeps_the_pane_named_since_it_read"

CONTROLS = (
    # The grid's width rules: at most three a row, none under the readable width
    Control("p96_columns_uncapped", LAYOUT, "count.clamp(1, EVEN_MAX_COLUMNS.min(fit))", "count.clamp(1, fit)",
            T_COLUMNS, "rust"),
    Control("p96_columns_ignore_width", LAYOUT, "usize::from(width / EVEN_MIN_WIDTH).max(1)", "EVEN_MAX_COLUMNS",
            T_READABLE, "rust"),
    # The rows: fuller first, at most one apart
    Control("p96_rows_fuller_last", LAYOUT, "base + usize::from(row < extra)",
            "base + usize::from(row >= rows - extra)", T_ROWS, "rust"),
    # Each split's ratio is its first half's share; a fixed half halves again, the defect's shape
    Control("p96_balanced_halves", LAYOUT, "let ratio = (len / 2) as f32 / len as f32;", "let ratio = 0.5_f32;",
            T_SIX, "rust"),
    # A second tile of an even tab reports no change
    Control("p96_tile_always_changes", LAYOUT, "        if same_tree(&tree, &self.root) {\n            return false;",
            "        if false && same_tree(&tree, &self.root) {\n            return false;", T_SIX, "rust"),
    # A close in an even tab re-tiles: the tile keeps the grid, each close path rebuilds it
    Control("p96_tile_not_kept", LAYOUT, "        self.even_columns = Some(columns);\n",
            "        self.even_columns = None;\n", T_CLOSE, "rust"),
    Control("p96_close_pane_no_retile", LAYOUT,
            "            self.prev_focus = None;\n        }\n        self.retile_after_close();\n",
            "            self.prev_focus = None;\n        }\n", T_CLOSE, "rust"),
    Control("p96_close_focused_no_retile", LAYOUT,
            "            self.prev_focus = None;\n            self.retile_after_close();\n",
            "            self.prev_focus = None;\n", T_CLOSE, "rust"),
    # The owner's own split or ratio change ends the grid
    Control("p96_split_keeps_even", LAYOUT,
            "valid_split_ratio(ratio));\n        self.even_columns = None;\n        Some(new_id)",
            "valid_split_ratio(ratio));\n        Some(new_id)", T_OWNER, "rust"),
    Control("p96_ratio_keeps_even", LAYOUT, "        if changed {\n            self.even_columns = None;\n        }\n",
            "", T_OWNER, "rust"),
    # The API: one server-side step evens the tab
    Control("p96_api_tiles_nothing", API, ".is_some_and(|tab| tab.layout.tile_even(area));",
            ".is_some_and(|_tab| false);", T_API, "rust"),
    # The CLI leaf `herdr pane tile` sends pane.tile under its own id
    Control("p96_cli_wrong_id", BINDINGS, 'id: "cli:pane:tile",', 'id: "cli:pane:tiles",', T_CLI, "rust"),
    # The sweep: a passed job closes after the hold
    Control("p96_hold_longer", DEAD, "pub const JOB_PASSED_GRACE_S: u64 = 30;",
            "pub const JOB_PASSED_GRACE_S: u64 = 300;", T_PASSED, "rust"),
    Control("p96_passed_reads_failed", DEAD,
            "crate::nirvana_job::JobState::Passed => Ended::JobPassed(at),",
            "crate::nirvana_job::JobState::Passed => Ended::JobFailed { at, kept: true },", T_PASSED, "rust"),
    # A failed job keeps its pane, the newest two per tab
    Control("p96_failed_never_ranked", DEAD, "Ended::JobFailed { at, .. } if failed_over.contains(id) => {",
            "Ended::JobFailed { at, .. } if false && failed_over.contains(id) => {", T_FAILED, "rust"),
    Control("p96_kept_failed_closes", DEAD,
            "return keep(\"its job failed, one of its tab's newest failed jobs\")", "return Verdict::Close",
            T_FAILED, "rust"),
    Control("p96_keep_three", DEAD, "pub const KEEP_FAILED_JOBS: usize = 2;",
            "pub const KEEP_FAILED_JOBS: usize = 3;", T_FAILED, "rust"),
    Control("p96_rank_oldest_first", DEAD, "panes.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));",
            "panes.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));", T_RANK, "rust"),
    Control("p96_lost_reads_passed", DEAD,
            "crate::nirvana_job::JobState::Failed | crate::nirvana_job::JobState::Lost => {",
            "crate::nirvana_job::JobState::Lost => Ended::JobPassed(at),\n"
            "                crate::nirvana_job::JobState::Failed => {", T_LOST, "rust"),
    # A running job's pane never closes
    Control("p96_running_reads_passed", DEAD, "crate::nirvana_job::JobState::Running => Ended::Running,",
            "crate::nirvana_job::JobState::Running => Ended::JobPassed(at),", T_RUNNING, "rust"),
    # The exec's writes keep the pane and space named since it read the record
    Control("p96_exec_drops_pane", JOB, "            record.pane = on_disk.pane;\n",
            "            let _ = &on_disk.pane;\n", T_PLACEMENT, "rust"),
    Control("p96_exec_drops_space", JOB, "            record.space = on_disk.space;\n",
            "            let _ = &on_disk.space;\n", T_PLACEMENT, "rust"),
)
