# Upstream sync 2026-11-01, review round (nirvana/plans/upstream-sync-2026-11-01.md, review S2). Controls starting
# with sync_. Upstream 3f5032c9 moved every pane spawn to `pane_host_terminal_appearance()`; the merge moved Nirvana's
# command tab with them. The test named must print FAIL with the field read put back. `Control` is provided by
# controls.py.
EDIT = "src/app/nirvana_edit.rs"

T_HOLD = "a_command_tab_opened_during_an_appearance_hold_answers_with_the_held_appearance"

CONTROLS = (
    # Review S2: during an appearance hold the new command tab answers with the held background's appearance.
    Control("sync_command_tab_held_appearance", EDIT,
            "        let host_terminal_appearance = self.pane_host_terminal_appearance();\n",
            "        let host_terminal_appearance = self.state.host_terminal_appearance;\n", T_HOLD, "rust"),
)
