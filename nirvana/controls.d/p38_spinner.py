# Plan 38 P38: a space sparkles while anything under it is working (package `p38_spinner`,
# nirvana/plans/2026-10-31-38-records/p38.md). Controls starting with p38_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
SPACES = "src/client/shell/nirvana_spaces.rs"
SHARED = "src/nirvana_spaces.rs"
SIDEBAR = "src/client/shell/sidebar.rs"
ENDPOINT = "src/client/shell/endpoint_sidebar.rs"
RENDER = "src/client/shell/render.rs"
SUBAGENTS = "src/nirvana_subagents.rs"

T_DONE_WORKING = "a_space_with_a_done_nirvana_claude_and_a_working_worker_sparkles"
T_TODAY = "a_space_with_only_done_and_idle_agents_shows_todays_marker"
T_BLOCKED = "a_blocked_space_keeps_its_own_mark"
T_FOLDED = "a_folded_parent_sparkles_for_its_hidden_childs_working_agent"
T_COLOUR = "the_sparkle_takes_the_working_colour"
T_CYCLE = "the_frames_cycle_in_order_on_the_clock"
T_PHASE = "two_spaces_share_a_phase"
T_REPAINT = "no_repaint_is_scheduled_when_nothing_is_working"
T_WIDTH = "every_frame_is_one_cell_wide"
# The review round (nirvana/plans/2026-10-31-38-records/p38.md, "Review round").
T_TICK = "a_tick_that_draws_nothing_leaves_the_repaint_due"
T_GATE = "the_gate_is_due_only_on_another_frame_of_a_sparkling_drawing"
T_TIMER = "the_timer_wakes_on_the_next_frame_while_sparkling"
T_NARROW = "the_narrow_sidebar_sparkles_as_the_spaces_list_does"
T_MACHINES = "the_machines_list_sparkles_as_the_spaces_list_does"
T_WAITING = "a_waiting_or_paneless_worker_does_not_sparkle"
T_PHASES = "read_seat_orders_phases_and_ended_runs"

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
        "    if busy(status) || workers.iter().any(|worker| worker.working) {\n",
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
        "            sparkle_frame,\n        );\n        // Working is the sparkle",
        "            sparkle_frame + entry_position,\n        );\n        // Working is the sparkle",
        T_PHASE,
        "rust",
    ),
    # §2.3: a drawing with nothing working schedules nothing
    Control(
        "p38_idle_list_notes_nothing",
        SIDEBAR,
        "    crate::nirvana_spaces::note_sparkle_drawn(sparkled.then_some(sparkle_frame));\n\n    if show_scrollbar {\n",
        "    crate::nirvana_spaces::note_sparkle_drawn(Some(sparkle_frame));\n\n    if show_scrollbar {\n",
        T_REPAINT,
        "rust",
    ),
    # §2.3: the timer asks for no repaint when nothing sparkled
    Control(
        "p38_not_drawn_no_repaint",
        SHARED,
        "        drawn != SPARKLE_NOT_DRAWN && drawn != frame\n",
        "        drawn != frame\n",
        T_REPAINT,
        "rust",
    ),
    # §2.3: the frame drawn still showing asks for no repaint
    Control(
        "p38_same_frame_not_due",
        SHARED,
        "        drawn != SPARKLE_NOT_DRAWN && drawn != frame\n",
        "        drawn != SPARKLE_NOT_DRAWN\n",
        T_REPAINT,
        "rust",
    ),
    # §2.3: the window's gate asks on the window's clock
    Control(
        "p38_window_gate_reads_clock",
        SHARED,
        "    with_sparkle_gate(|gate| gate.due(sparkle_frame()))\n",
        "    with_sparkle_gate(|gate| gate.due(0))\n",
        T_REPAINT,
        "rust",
    ),
    # Review S2: asking leaves the drawing as it was (was p38_due_forgets_drawing, which tested the opposite)
    Control(
        "p38_asking_keeps_drawing",
        SHARED,
        "        let drawn = self.drawn.load(std::sync::atomic::Ordering::Acquire);\n",
        "        let drawn = self.drawn.swap(SPARKLE_NOT_DRAWN, std::sync::atomic::Ordering::AcqRel);\n",
        T_GATE,
        "rust",
    ),
    # Review S5: a gate starts with nothing drawn; the window's static is built by the same `new`
    Control(
        "p38_gate_starts_undrawn",
        SHARED,
        "            drawn: std::sync::atomic::AtomicUsize::new(SPARKLE_NOT_DRAWN),\n",
        "            drawn: std::sync::atomic::AtomicUsize::new(0),\n",
        T_GATE,
        "rust",
    ),
    # Review S2: a window that draws no sidebar clears the drawing
    Control(
        "p38_no_sidebar_clears",
        RENDER,
        "        crate::nirvana_spaces::note_sparkle_drawn(None);\n",
        "        // plan 38 control: the drawing is not cleared\n",
        T_TICK,
        "rust",
    ),
    # Review N1: while sparkling the timer wakes on the next frame's boundary
    Control(
        "p38_timer_to_next_frame",
        SHARED,
        "        default.min(SPARKLE_FRAME - into_frame)\n",
        "        default\n",
        T_TIMER,
        "rust",
    ),
    # Review N1: with nothing sparkling the timer waits its default
    Control(
        "p38_timer_default_when_idle",
        SHARED,
        "        if self.drawn.load(std::sync::atomic::Ordering::Acquire) == SPARKLE_NOT_DRAWN {\n",
        "        if false {\n",
        T_TIMER,
        "rust",
    ),
    # Review N1: the window's timer delay comes from the window's gate and clock
    Control(
        "p38_window_timer_wired",
        SHARED,
        "    with_sparkle_gate(|gate| gate.delay(sparkle_elapsed(), default))\n",
        "    with_sparkle_gate(|_| default)\n",
        T_TIMER,
        "rust",
    ),
    # Review S3: the narrow sidebar draws the sparkle
    Control(
        "p38_narrow_marker",
        SIDEBAR,
        "            rect.width.saturating_sub(2),\n            marker.0,\n",
        "            rect.width.saturating_sub(2),\n            status_icon(status, config.status_indicators),\n",
        T_NARROW,
        "rust",
    ),
    # Review S3: the narrow sidebar says what it drew
    Control(
        "p38_narrow_notes",
        SIDEBAR,
        "    crate::nirvana_spaces::note_sparkle_drawn(sparkled.then_some(sparkle_frame));\n\n    if let Some(divider_y) = divider_y {\n",
        "    crate::nirvana_spaces::note_sparkle_drawn(None);\n\n    if let Some(divider_y) = divider_y {\n",
        T_NARROW,
        "rust",
    ),
    # Review S3: the narrow sidebar counts the space's live workers
    Control(
        "p38_narrow_reads_workers",
        SIDEBAR,
        "            &HashSet::new(),\n            true,\n",
        "            &HashSet::new(),\n            false,\n",
        T_NARROW,
        "rust",
    ),
    # Review S3: the machines list draws the sparkle
    Control(
        "p38_machines_marker",
        ENDPOINT,
        "                    marker,\n                    entry,\n",
        "                    (status_icon(status, config.status_indicators), status),\n                    entry,\n",
        T_MACHINES,
        "rust",
    ),
    # Review S3: the machines list says what it drew
    Control(
        "p38_machines_notes",
        ENDPOINT,
        "    crate::nirvana_spaces::note_sparkle_drawn(sparkled.then_some(sparkle_frame));\n    if show_scrollbar {\n",
        "    crate::nirvana_spaces::note_sparkle_drawn(None);\n    if show_scrollbar {\n",
        T_MACHINES,
        "rust",
    ),
    # Review S3: only this machine's spaces read this window's worker cache
    Control(
        "p38_machines_local_workers_only",
        ENDPOINT,
        "                        collapsed_groups,\n                        endpoint.endpoint_id.is_local(),\n",
        "                        collapsed_groups,\n                        true,\n",
        T_MACHINES,
        "rust",
    ),
    # Review S3: a machine that is not online does not sparkle
    Control(
        "p38_machines_offline_still",
        ENDPOINT,
        "                let marker = if endpoint.status == ClientEndpointStatus::Online {\n",
        "                let marker = if true {\n",
        T_MACHINES,
        "rust",
    ),
    # Review S3: the narrow machines list draws the sparkle
    Control(
        "p38_narrow_machines_marker",
        ENDPOINT,
        "                rect.width.saturating_sub(number_width),\n                marker.0,\n",
        "                rect.width.saturating_sub(number_width),\n                status_icon(workspace.agent_status, config.status_indicators),\n",
        T_MACHINES,
        "rust",
    ),
    # Review S3: the narrow machines list says what it drew
    Control(
        "p38_narrow_machines_notes",
        ENDPOINT,
        "    crate::nirvana_spaces::note_sparkle_drawn(sparkled.then_some(sparkle_frame));\n    if let Some(divider_y) = divider_y {\n",
        "    crate::nirvana_spaces::note_sparkle_drawn(None);\n    if let Some(divider_y) = divider_y {\n",
        T_MACHINES,
        "rust",
    ),
    # Review S3: the narrow machines list reads this machine's workers only
    Control(
        "p38_narrow_machines_local_only",
        ENDPOINT,
        "                    &HashSet::new(),\n                    endpoint.endpoint_id.is_local(),\n",
        "                    &HashSet::new(),\n                    true,\n",
        T_MACHINES,
        "rust",
    ),
    # Review S3: a narrow row of a machine that is not online does not sparkle
    Control(
        "p38_narrow_machines_stale",
        ENDPOINT,
        "            let marker = if stale {\n",
        "            let marker = if false {\n",
        T_MACHINES,
        "rust",
    ),
    # Review S4: an order that waits does not sparkle
    Control(
        "p38_waiting_not_working",
        SPACES,
        "                working: !sub.waiting && pane_alive,\n",
        "                working: pane_alive,\n",
        T_WAITING,
        "rust",
    ),
    # Review S4: an order whose own pane is gone does not sparkle
    Control(
        "p38_pane_gone_not_working",
        SPACES,
        "                working: !sub.waiting && pane_alive,\n",
        "                working: !sub.waiting,\n",
        T_WAITING,
        "rust",
    ),
    # Review S4: a Deferred order waits. Dead panes (2026-11-08) moved the find: an Asked order is no longer live, so
    # only a live Deferred one waits.
    Control(
        "p38_waiting_phases",
        SUBAGENTS,
        "                waiting: is_live && order.phase == crate::nirvana_run::Phase::Deferred,\n",
        "                waiting: is_live && order.phase == crate::nirvana_run::Phase::Asked,\n",
        T_PHASES,
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
