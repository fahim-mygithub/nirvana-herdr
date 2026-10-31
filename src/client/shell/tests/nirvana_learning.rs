//! Plan 36 §1 and §3: the Learning square and its panel.

use super::*;
use crate::client::shell::nirvana_mods::OpenPanel;
use crate::nirvana_learning::tests::{fixture, temp, tree, PATCHED, PROFILE};
use crate::nirvana_learning::{load, set_live_for_test, set_view_for_test, LearningView};

fn agent() -> ClientShellAgent {
    ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: Some("learner".into()),
        display_agent: None,
        agent: Some("claude".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Idle,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: true,
        agent_session: None,
    }
}

fn state_in(root: &std::path::Path) -> ClientShellState {
    crate::nirvana_preload::set_view_for_test(Some(crate::nirvana_preload::View::default()));
    let mut projected = snapshot();
    projected.agents = vec![agent()];
    let cwd = root.display().to_string();
    projected.panes[0].cwd = Some(cwd.clone());
    projected.panes[0].foreground_cwd = Some(cwd);
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state
}

/// A fixture project, its view, and a window on it.
fn fixture_state(name: &str) -> (ClientShellState, std::path::PathBuf, LearningView) {
    let root = temp(name);
    let now = crate::nirvana_run::now();
    fixture(&root, now);
    let view = load(&root, now);
    set_view_for_test(Some(view.clone()));
    (state_in(&root), root, view)
}

fn rows_of(state: &mut ClientShellState, width: u16, height: u16) -> Vec<String> {
    let frame = state.compose(width, height).expect("composed frame").frame;
    frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect()
}

fn strip(state: &mut ClientShellState, width: u16) -> String {
    let rows = rows_of(state, width, 60);
    rows[state.layout(width, 60).nirvana_mods.y as usize].clone()
}

fn click(state: &mut ClientShellState, column: u16, row: u16) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })])
}

fn key(state: &mut ClientShellState, code: KeyCode) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        code,
        KeyModifiers::NONE,
    ))])
}

fn open(state: &mut ClientShellState) {
    let _ = rows_of(state, 200, 60);
    let square = state.hits.nirvana_learning;
    assert!(square.width > 0, "the Learning square is drawn");
    click(state, square.x + 1, square.y);
    assert_eq!(state.open_panel(), OpenPanel::Learning);
}

fn invoked(outcome: &ClientShellInput) -> Vec<crate::api::schema::CommandInvokeParams> {
    outcome
        .actions
        .iter()
        .filter_map(|action| match action {
            ClientShellAction::Endpoint { request, .. } => match &request.method {
                crate::api::schema::Method::CommandInvoke(params) => Some(params.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Moves the selection to the row whose text holds `needle`.
fn select(state: &mut ClientShellState, needle: &str) {
    key(state, KeyCode::Home);
    for _ in 0..40 {
        let rows = rows_of(state, 200, 60);
        let hits = state.hits.nirvana_learning_panel.clone();
        let selected = state.nirvana_learning.as_ref().expect("panel").selected;
        let (rect, _) = hits
            .rows
            .iter()
            .find(|(_, index)| *index == selected)
            .expect("selected row drawn");
        if rows[rect.y as usize].contains(needle) {
            return;
        }
        key(state, KeyCode::Char('j'));
    }
    panic!("no row holds {needle}");
}

fn done(root: std::path::PathBuf) {
    set_view_for_test(None);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn p36_the_learning_square_sits_after_preload_and_before_memory_and_git() {
    let _home = crate::nirvana::with_test_home("p36-place");
    let (mut state, root, _) = fixture_state("place");
    crate::nirvana_git::set_view_for_test(Some(crate::nirvana_git::GitView {
        status: crate::nirvana_git::GitStatus {
            repo: root.clone(),
            branch: "main".into(),
            upstream: "origin/main".into(),
            ..Default::default()
        },
        ci: None,
    }));
    let row = strip(&mut state, 220);
    let at = |label: &str| {
        row.find(label)
            .unwrap_or_else(|| panic!("{label} in {row:?}"))
    };
    assert!(at(" Preload ") < at(" Learning "), "{row:?}");
    assert!(at(" Learning ") < at(" Git "), "{row:?}");
    let (preload, learning, git) = (
        state.hits.nirvana_preload,
        state.hits.nirvana_learning,
        state.hits.nirvana_git,
    );
    assert_eq!(learning.x, preload.right() + 1, "directly after Preload");
    assert_eq!(git.x, learning.right() + 1, "Git after Learning");
    crate::nirvana_git::set_view_for_test(None);

    // With Memory drawn, it follows Learning.
    let view = crate::nirvana_learning::cached(&root).expect("test view");
    let agent = crate::client::shell::nirvana_mods::strip_agent(
        state.snapshot.as_deref().expect("snapshot"),
    )
    .expect("strip agent");
    let palette = state.config.palette.clone();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 220, 1));
    let hits = crate::client::shell::nirvana_mods::render_strip(
        &mut buffer,
        Rect::new(0, 0, 220, 1),
        &agent,
        None,
        &crate::nirvana_preload::View::default(),
        Some(("2 new".into(), true)),
        None,
        None,
        None,
        Some(&view),
        OpenPanel::None,
        &palette,
    );
    assert_eq!(hits.learning.x, hits.preload.right() + 1);
    assert_eq!(
        hits.memory.x,
        hits.learning.right() + 1,
        "Memory follows Learning"
    );
    done(root);
}

#[test]
fn p36_the_square_reads_lessons_and_reviews_against_the_cap_wide_and_narrow() {
    let _home = crate::nirvana::with_test_home("p36-value");
    let (mut state, root, view) = fixture_state("value");
    // The fixture: 11 live lessons, 6 reviews today, the default cap, the last review's every op refused.
    assert!(
        strip(&mut state, 200).contains("Learning 11 lessons \u{b7} review 6/20 \u{2717} \u{25BE}"),
        "{:?}",
        strip(&mut state, 200)
    );
    assert!(
        strip(&mut state, 120).contains("Learning 11 \u{b7} 6/20 \u{2717}"),
        "{:?}",
        strip(&mut state, 120)
    );
    // No review today: no mark; one live lesson reads singular.
    let mut quiet = view.clone();
    quiet.today.reviews.clear();
    quiet.live = 1;
    set_view_for_test(Some(quiet));
    let row = strip(&mut state, 200);
    assert!(
        row.contains("Learning 1 lesson \u{b7} review 0/20 \u{25BE}"),
        "{row:?}"
    );
    // No lesson: the words the plan gives.
    set_view_for_test(Some(LearningView {
        root: root.clone(),
        ..LearningView::default()
    }));
    let row = strip(&mut state, 200);
    assert!(row.contains("Learning no lessons yet \u{25BE}"), "{row:?}");
    // Before the first read the square is not drawn.
    set_view_for_test(None);
    let row = strip(&mut state, 200);
    assert!(!row.contains("Learning"), "{row:?}");
    assert_eq!(state.hits.nirvana_learning.width, 0);
    done(root);
}

#[test]
fn p36_the_panel_shows_every_section_from_a_fixture_project() {
    let _home = crate::nirvana::with_test_home("p36-panel");
    let (mut state, root, _) = fixture_state("panel");
    open(&mut state);
    let text = rows_of(&mut state, 200, 80).join("\n");
    let folder = root
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    for want in [
        format!("Learning \u{b7} {folder}"),
        // Today, in the plan's order of sections.
        "Today".to_owned(),
        "reviews 6/20 \u{b7} 5 gated: 3 done \u{b7} 1 held back \u{b7} 1 failed \u{b7} 1 not gated yet".to_owned(),
        "ops 1 applied \u{b7} 3 refused".to_owned(),
        "2\u{d7} the evidence is N characters; a quote of at least N proves something".to_owned(),
        "1\u{d7} the evidence is not in oN's stream".to_owned(),
        "Recent changes".to_owned(),
        "op-5 rollback gone \u{b7} lessons \u{b7} by herdr".to_owned(),
        "op-4 new gone \u{b7} lessons \u{b7} by o1-lr".to_owned(),
        "\u{b7} undone".to_owned(),
        "op-3 merge cut-on-the-beat, beat-a, beat-b \u{b7} lessons".to_owned(),
        format!("op-2 patch closer \u{b7} cards of {PROFILE} \u{b7} by t1"),
        format!("op-2 patch {PATCHED} \u{b7} lessons"),
        "Lessons".to_owned(),
        "worker: 10 lesson(s) \u{b7} offered 4 \u{b7} opened 2 \u{b7} passed after opened 1".to_owned(),
        "live    Slice with a brim \u{b7} worker \u{b7} print \u{b7} offered 4 \u{b7} opened 2 \u{b7} passed after opened 1"
            .to_owned(),
        format!("live    Cut on the beat \u{b7} merge \u{b7} reel @{PROFILE}"),
        "decayed Level the bed first \u{b7} review \u{b7} print \u{b7} offered 9".to_owned(),
        "Profiles".to_owned(),
        "plain \u{b7} one file, keeps no cards".to_owned(),
        format!(
            "{PROFILE} \u{b7} 5 cards \u{b7} 1 warning \u{b7} 1 refused \u{b7} 1 not known to herdr"
        ),
        format!("1 card herdr does not know is never offered: herdr profile check {PROFILE} --adopt"),
        "Waiting".to_owned(),
        "print: 10 live lessons, merged at the next herdr run end".to_owned(),
    ] {
        assert!(text.contains(&want), "{want:?} not in\n{text}");
    }
    let order = ["Today", "Recent changes", "Lessons", "Profiles", "Waiting"];
    let at: Vec<usize> = order
        .iter()
        .map(|head| text.find(head).expect("head"))
        .collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{at:?}");
    // The lesson rows list live first.
    assert!(
        text.find("live    Slice").expect("live") < text.find("decayed Level").expect("decayed")
    );
    done(root);
}

#[test]
fn p36_u_asks_first_and_then_rolls_back_exactly_that_row() {
    let _home = crate::nirvana::with_test_home("p36-u");
    let (mut state, root, _) = fixture_state("u");
    let ledger = nirvana_ledger_len(&root);
    open(&mut state);
    select(&mut state, &format!("op-2 patch {PATCHED}"));
    key(&mut state, KeyCode::Char('u'));
    let asked = state
        .nirvana_learning
        .as_ref()
        .and_then(|p| p.confirming.clone());
    assert_eq!(asked.as_ref().map(|c| c.id.as_str()), Some("op-2"));
    let text = rows_of(&mut state, 200, 80).join("\n");
    assert!(
        text.contains(&format!(
            "undo op-2 (patch of {PATCHED}, lessons)? [Undo] [Cancel]"
        )),
        "{text}"
    );
    assert_eq!(nirvana_ledger_len(&root), ledger, "asking runs nothing");
    // n keeps it.
    key(&mut state, KeyCode::Char('n'));
    assert!(state
        .nirvana_learning
        .as_ref()
        .is_some_and(|p| p.confirming.is_none()));
    assert_eq!(nirvana_ledger_len(&root), ledger);
    // A rollback row and an undone op are refused before asking.
    select(&mut state, "op-5 rollback");
    key(&mut state, KeyCode::Char('u'));
    let panel = state.nirvana_learning.as_ref().expect("panel");
    assert!(panel.confirming.is_none());
    assert_eq!(
        panel.message.as_deref(),
        Some("op-5 is a rollback, which herdr does not undo")
    );
    // u then y rolls back the patch, and only it.
    select(&mut state, &format!("op-2 patch {PATCHED}"));
    key(&mut state, KeyCode::Char('u'));
    key(&mut state, KeyCode::Char('y'));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let said = loop {
        let _ = rows_of(&mut state, 200, 80);
        let panel = state.nirvana_learning.as_ref().expect("panel");
        if !panel.busy {
            break panel.message.clone().unwrap_or_default();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the undo did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(said.contains("rolled back op-2"), "{said}");
    let rows = crate::nirvana_lessons::ledger(&root);
    assert_eq!(rows.len(), ledger + 1);
    let last = rows.last().expect("row");
    assert_eq!(
        (last.op.as_str(), last.of.as_deref()),
        ("rollback", Some("op-2"))
    );
    assert_eq!(
        crate::nirvana_lessons::body(&root, PATCHED).as_deref(),
        Some("v1\n")
    );
    let cards = crate::nirvana_profiles::knowledge_dir(&root, PROFILE).expect("dir");
    assert_eq!(
        crate::nirvana_knowledge::ledger(&cards).len(),
        3,
        "the card op-2 is untouched"
    );
    done(root);
}

fn nirvana_ledger_len(root: &std::path::Path) -> usize {
    crate::nirvana_lessons::ledger(root).len()
}

#[test]
fn p36_enter_opens_the_lesson_or_card_in_the_edit_tab() {
    let _home = crate::nirvana::with_test_home("p36-open");
    let (mut state, root, _) = fixture_state("open");
    open(&mut state);
    select(&mut state, "live    Slice with a brim");
    let outcome = key(&mut state, KeyCode::Enter);
    let calls = invoked(&outcome);
    let file = crate::nirvana_lessons::dir(&root).join(format!("{PATCHED}.md"));
    assert_eq!(calls.len(), 1, "{:?}", outcome.actions);
    assert_eq!(
        calls[0].command_id,
        format!("{}{}", crate::nirvana::EDIT_FILE_COMMAND, file.display())
    );
    assert_eq!(calls[0].tab_id.as_deref(), Some("tab_1"));
    assert!(state.nirvana_learning.is_none(), "the panel closes on open");
    // A card's change opens the card; `o` is Enter.
    open(&mut state);
    select(&mut state, "op-2 patch closer");
    let outcome = key(&mut state, KeyCode::Char('o'));
    let card = crate::nirvana_profiles::knowledge_dir(&root, PROFILE)
        .expect("dir")
        .join("closer.md");
    assert_eq!(
        invoked(&outcome)[0].command_id,
        format!("{}{}", crate::nirvana::EDIT_FILE_COMMAND, card.display())
    );
    // A folder profile opens its knowledge folder.
    open(&mut state);
    select(&mut state, &format!("{PROFILE} \u{b7} 5 cards"));
    let outcome = key(&mut state, KeyCode::Enter);
    let dir = crate::nirvana_profiles::knowledge_dir(&root, PROFILE).expect("dir");
    assert_eq!(
        invoked(&outcome)[0].command_id,
        format!("{}{}", crate::nirvana::EDIT_DIR_COMMAND, dir.display())
    );
    // Esc closes.
    open(&mut state);
    key(&mut state, KeyCode::Esc);
    assert!(state.nirvana_learning.is_none());
    done(root);
}

#[test]
fn p36_drawing_reads_no_file_and_browsing_writes_nothing() {
    let home = crate::nirvana::with_test_home("p36-draw");
    let root = temp("draw");
    fixture(&root, crate::nirvana_run::now());
    let mut state = state_in(&root);
    set_live_for_test(true);
    let (before, home_before) = (tree(&root), tree(&home.path));
    // The real cache: the square shows once its thread has read, and the drawing thread never reads.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.hits.nirvana_learning.width == 0 {
        let _ = rows_of(&mut state, 200, 60);
        assert!(
            std::time::Instant::now() < deadline,
            "the square never showed"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(strip(&mut state, 200).contains("Learning 11 lessons"));
    open(&mut state);
    for code in [
        KeyCode::Char('j'),
        KeyCode::Char('j'),
        KeyCode::Down,
        KeyCode::Char('k'),
        KeyCode::End,
        KeyCode::Home,
    ] {
        key(&mut state, code);
        let _ = rows_of(&mut state, 200, 60);
    }
    key(&mut state, KeyCode::Esc);
    let _ = rows_of(&mut state, 200, 60);
    assert_eq!(
        crate::nirvana_learning::reads_on_this_thread(),
        0,
        "no view was read on the drawing thread"
    );
    assert_eq!(tree(&root), before, "the project");
    assert_eq!(tree(&home.path), home_before, "herdr's home");
    set_live_for_test(false);
    let _ = std::fs::remove_dir_all(root);
}

/// Every other panel's toggle closes the Learning panel, and the Learning toggle closes theirs: one is open at a time.
#[test]
fn p36_one_panel_is_open_at_a_time_with_the_learning_panel_among_them() {
    let _home = crate::nirvana::with_test_home("p36-one");
    let (mut state, root, _) = fixture_state("one");
    type Toggle = fn(&mut ClientShellState);
    // A Nirvana tab, so the Seats panel opens too.
    let mut projected = state.snapshot.as_deref().expect("snapshot").clone();
    projected.agents[0].name = Some("nirvana".into());
    state.set_snapshot(Box::new(projected));
    let others: [(&str, Toggle); 7] = [
        ("running", ClientShellState::toggle_running_panel),
        ("preload", ClientShellState::toggle_preload_panel),
        ("memory", ClientShellState::toggle_memory_panel),
        ("accounts", ClientShellState::toggle_accounts_panel),
        ("seats", ClientShellState::toggle_seats_panel),
        ("jobs", ClientShellState::toggle_jobs_panel),
        ("git", ClientShellState::toggle_git_panel),
    ];
    for (name, toggle) in others {
        let _ = rows_of(&mut state, 200, 60);
        state.toggle_learning_panel();
        assert!(state.nirvana_learning.is_some(), "{name}: learning opens");
        toggle(&mut state);
        assert!(
            state.nirvana_learning.is_none(),
            "{name} closes the Learning panel"
        );
        state.nirvana_preload = None;
        state.nirvana_running = None;
        state.nirvana_memory = None;
        state.nirvana_accounts = None;
        state.nirvana_seats = None;
        state.nirvana_jobs = None;
        state.nirvana_git = None;
    }
    done(root);
}

/// A pane's output while the Learning panel is open composes the frame again, panel on top, as the other panels do.
#[test]
fn p36_the_open_learning_panel_is_not_painted_over_by_its_agent() {
    use crate::client::shell::surface_patch::ClientPaneSurfacePatchOutcome;
    let _home = crate::nirvana::with_test_home("p36-patch");
    let (mut state, root, _) = fixture_state("patch");
    for open_it in [false, true] {
        state.nirvana_learning = None;
        let _ = rows_of(&mut state, 200, 60);
        if open_it {
            state.toggle_learning_panel();
            let _ = rows_of(&mut state, 200, 60);
        }
        let current = state.pane_surface.clone().expect("pane surface");
        let mut cell = current.frame.cells[0].clone();
        cell.symbol = "y".into();
        let patch = crate::protocol::PaneSurfacePatch {
            boot_id: current.boot_id,
            projection_revision: current.projection_revision,
            base_surface_revision: current.surface_revision,
            surface_revision: current.surface_revision + 1,
            panes: current.panes.clone(),
            rows: vec![crate::protocol::PaneSurfacePatchRow {
                x: 0,
                y: 0,
                cells: vec![cell],
            }],
            cursor: current.frame.cursor.clone(),
        };
        let outcome = state.apply_pane_surface_patch(patch);
        assert!(
            matches!(outcome, ClientPaneSurfacePatchOutcome::Applied(_)),
            "open {open_it}: taken"
        );
        let straight = matches!(outcome, ClientPaneSurfacePatchOutcome::Applied(Some(_)));
        assert_eq!(
            straight, !open_it,
            "written straight to the screen only with no panel open"
        );
    }
    done(root);
}
