# Plan 38 P38: a space sparkles while anything under it is working (package `p38_spinner`,
# nirvana/plans/2026-10-31-38-records/p38.md). Controls starting with p38_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
SPACES = "src/client/shell/nirvana_spaces.rs"
SHARED = "src/nirvana_spaces.rs"
SIDEBAR = "src/client/shell/sidebar.rs"

T_DONE_WORKING = "a_space_with_a_done_nirvana_claude_and_a_working_worker_sparkles"
T_TODAY = "a_space_with_only_done_and_idle_agents_shows_todays_marker"
T_BLOCKED = "a_blocked_space_keeps_its_own_mark"
T_FOLDED = "a_folded_parent_sparkles_for_its_hidden_childs_working_agent"
T_COLOUR = "the_sparkle_takes_the_working_colour"
T_CYCLE = "the_frames_cycle_in_order_on_the_clock"
T_PHASE = "two_spaces_share_a_phase"
T_REPAINT = "no_repaint_is_scheduled_when_nothing_is_working"
T_WIDTH = "every_frame_is_one_cell_wide"

CONTROLS = (
    # §2.1: a Working agent pane under the space counts, though herdr's space status reads Done
    Control(
        "p38_working_pane_counts",
        SPACES,
        "    let busy = |status: AgentStatus| matches!(status, AgentStatus::Working | AgentStatus::Blocked);\n",
        "    let busy = |status: AgentStatus| matches!(status, AgentStatus::Blocked);\n",
        T_DONE_WORKING,
        "rust",
    ),
    # §2.1: a live worker row under the space counts (the owner's `quad-pegs…` 56s)
    Control(
        "p38_live_worker_counts",
        SPACES,
        "    if busy(status) || !workers.is_empty() {\n",
        "    if busy(status) {\n",
        T_DONE_WORKING,
        "rust",
    ),
    # §2.1: Done, Idle and Unknown are not working: today's marker
    Control(
        "p38_idle_is_not_working",
        SPACES,
        "    if working && status != AgentStatus::Blocked {\n",
        "    if status != AgentStatus::Blocked {\n",
        T_TODAY,
        "rust",
    ),
    # §2.1: Blocked keeps herdr's own mark
    Control(
        "p38_blocked_keeps_mark",
        SPACES,
        "    if working && status != AgentStatus::Blocked {\n",
        "    if working {\n",
        T_BLOCKED,
        "rust",
    ),
    # §2.1: a folded parent counts its hidden children's agents
    Control(
        "p38_folded_counts_children",
        SPACES,
        "        .filter(|worktree| collapsed_groups.contains(&worktree.key))\n",
        "        .filter(|_| false)\n",
        T_FOLDED,
        "rust",
    ),
    # §2.1: ... and only while the group is folded, only its linked children
    Control(
        "p38_folded_only_linked_children",
        SPACES,
        "                            .is_some_and(|member| member.key == key && member.is_linked_worktree)\n",
        "                            .is_some_and(|member| member.key == key)\n",
        T_FOLDED,
        "rust",
    ),
    # §2.2: the sparkle takes the working colour
    Control(
        "p38_working_colour",
        SPACES,
        "            crate::nirvana_spaces::sparkle_glyph(frame),\n            AgentStatus::Working,\n",
        "            crate::nirvana_spaces::sparkle_glyph(frame),\n            status,\n",
        T_COLOUR,
        "rust",
    ),
    # §2.2: the frames in order, one per 250 ms
    Control(
        "p38_frames_in_order",
        SHARED,
        "    (elapsed.as_millis() / SPARKLE_FRAME.as_millis()) as usize % SPARKLE_FRAMES.len()\n",
        "    (elapsed.as_millis() / SPARKLE_FRAME.as_millis()) as usize * 3 % SPARKLE_FRAMES.len()\n",
        T_CYCLE,
        "rust",
    ),
    # §2.2: the marker is drawn from the sparkle, in the sidebar
    Control(
        "p38_sidebar_draws_marker",
        SIDEBAR,
        "                marker.0,\n",
        "                status_icon(status, crate::config::StatusIndicatorStyle::Symbols),\n",
        T_CYCLE,
        "rust",
    ),
    # §2.2: one frame for the whole list, so spaces share a phase
    Control(
        "p38_one_phase",
        SIDEBAR,
        "            sparkle_frame,\n        );\n",
        "            sparkle_frame + entry_position,\n        );\n",
        T_PHASE,
        "rust",
    ),
    # §2.3: a drawing with nothing working schedules nothing
    Control(
        "p38_idle_list_notes_nothing",
        SIDEBAR,
        "    crate::nirvana_spaces::note_sparkle_drawn(sparkled.then_some(sparkle_frame));\n",
        "    crate::nirvana_spaces::note_sparkle_drawn(Some(sparkle_frame));\n",
        T_REPAINT,
        "rust",
    ),
    # §2.3: the timer asks for no repaint when nothing sparkled
    Control(
        "p38_not_drawn_no_repaint",
        SHARED,
        "    if drawn == SPARKLE_NOT_DRAWN {\n        return false;\n",
        "    if drawn == SPARKLE_NOT_DRAWN && false {\n        return false;\n",
        T_REPAINT,
        "rust",
    ),
    # §2.3: the same frame still showing keeps the drawing for the next tick
    Control(
        "p38_same_frame_keeps_drawing",
        SHARED,
        "        sparkle_drawn_swap(drawn);\n        return false;\n",
        "        return false;\n",
        T_REPAINT,
        "rust",
    ),
    # §2.3: a repaint forgets the drawing, so only a drawing that still sparkles keeps the timer going
    Control(
        "p38_due_forgets_drawing",
        SHARED,
        "        return false;\n    }\n    true\n}\n",
        "        return false;\n    }\n    sparkle_drawn_swap(drawn);\n    true\n}\n",
        T_REPAINT,
        "rust",
    ),
    # §2.4: every frame is one cell wide by the sidebar's width function, else the fallback
    Control(
        "p38_one_cell_wide",
        SHARED,
        "    if unicode_width::UnicodeWidthStr::width(glyph) == 1 {\n",
        "    if unicode_width::UnicodeWidthStr::width(glyph) == 2 {\n",
        T_WIDTH,
        "rust",
    ),
)
