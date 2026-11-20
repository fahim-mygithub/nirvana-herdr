//! Plan 45 P45a: the `+` after the agents heading, the space menu's Profiles line, and the Profiles panel, composed
//! from a scratch project built here (profiles written as files, proposals through the store's own `write`, the run
//! journal as `seat run` writes it). Every test holds `with_test_home`, so no test reads the owner's home, and every
//! project is a temp folder.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;
use crate::api::schema::Method;
use crate::client::shell::nirvana_profile_panel::{
    self as panel_mod, Part, ACCEPT_IN_DETAIL, NOTHING_SUGGESTED, NO_PROFILES, NO_USE,
};
use crate::nirvana_profile_proposals::{self as proposals, Draft, Kind, DETECTOR};
use crate::nirvana_run::{Event, Kind as Ev, Project, Seated};
use crate::nirvana_seat::{self, Mode, SizeName};

const HEIGHT: u16 = 48;

/// A project under a test home with the default seats, `.git`, a `.mcp.json` naming one made-up server (a fake
/// command, never a definition from this machine) and one project skill, `alpha`, as the store's tests make one.
fn project(name: &str) -> (crate::nirvana::TestHomeGuard, PathBuf) {
    let home = crate::nirvana::with_test_home(name);
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    let root = home.path.join("project");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(root.join(".claude").join("skills").join("alpha")).unwrap();
    std::fs::write(
        root.join(".claude/skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: the alpha skill.\n---\nbody\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers": {"fake-tool": {"command": "fake-mcp-not-real"}}}"#,
    )
    .unwrap();
    Project::at(&root).ensure().unwrap();
    (home, root)
}

/// A one-file profile.
fn profile_file(root: &Path, name: &str, text: &str) -> PathBuf {
    let file = crate::nirvana_profiles::path(root, name);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, text).unwrap();
    file
}

/// A folder profile with `cards` knowledge cards.
fn profile_folder(root: &Path, name: &str, text: &str, cards: usize) -> PathBuf {
    let dir = crate::nirvana_profiles::folder_of(root, name);
    let knowledge = dir.join("knowledge");
    std::fs::create_dir_all(&knowledge).unwrap();
    let file = dir.join("profile.md");
    std::fs::write(&file, text).unwrap();
    for n in 0..cards {
        std::fs::write(
            knowledge.join(format!("card-{n}.md")),
            "---\nname: n\nwhen: w\nverdict: keep\nsource: s\n---\n",
        )
        .unwrap();
    }
    file
}

const EVIDENCE: &str =
    "7 orders repeat a 412-character paragraph: \"Judge the rendered frame, never the plan or the code\"";
const NEW_BODY: &str = "---\ndescription: Judges renders against the rubric card.\nclass: media.video\neffort: high\n---\n\
You judge renders. Open every file yourself.\n";

fn draft(kind: Kind, profile: &str, key: &str, body: &str, base_sha: Option<String>) -> Draft {
    Draft {
        kind,
        profile: profile.into(),
        by: DETECTOR.into(),
        run: "run-20261119".into(),
        evidence: EVIDENCE.into(),
        key: key.into(),
        body: body.into(),
        base_sha,
    }
}

fn propose_new(root: &Path, profile: &str) -> String {
    proposals::write(
        root,
        &draft(
            Kind::New,
            profile,
            &format!("paragraph:{profile}"),
            NEW_BODY,
            None,
        ),
    )
    .expect("a new proposal is written")
}

fn propose_patch(root: &Path, profile: &str, body: &str) -> String {
    let sha = proposals::profile_sha(root, profile);
    proposals::write(
        root,
        &draft(Kind::Patch, profile, &format!("patch:{profile}"), body, sha),
    )
    .expect("a patch proposal is written")
}

/// One order as `seat run` journals it.
struct Order {
    id: String,
    class: String,
    needs: Vec<String>,
    profile: Option<String>,
}

fn orders(
    prefix: &str,
    n: usize,
    class: &str,
    needs: &[&str],
    profile: Option<&str>,
) -> Vec<Order> {
    (0..n)
        .map(|i| Order {
            id: format!("{prefix}{i}"),
            class: class.into(),
            needs: needs.iter().map(|need| (*need).to_owned()).collect(),
            profile: profile.map(str::to_owned),
        })
        .collect()
}

fn seated(order: &Order, order_file: &str) -> Seated {
    Seated {
        order: order.id.clone(),
        seat: "sonnet".into(),
        cli: "claude".into(),
        model: "claude-sonnet-5-5".into(),
        effort: nirvana_seat::Effort::Medium,
        pool: "max".into(),
        class: order.class.clone(),
        chain: None,
        size: SizeName::Small,
        mode: Mode::Balanced,
        needs: order.needs.clone(),
        not_family: Vec::new(),
        reason: match &order.profile {
            Some(name) => format!("r; profile {name} sha256 {}", "a".repeat(64)),
            None => "r".into(),
        },
        override_reason: None,
        order_file: order_file.into(),
        cwd: "C:/p".into(),
        worktree: None,
        branch: None,
        parent: None,
        why: None,
        rule_pick: None,
        bar_dropped: None,
        step_down: None,
        use_basis: None,
        expected: BTreeMap::new(),
        jev_pick: None,
        send_ok: None,
    }
}

/// The run's order copies and journal, as the detector's tests write them.
fn seat_all(root: &Path, run: &str, orders: &[Order]) {
    let project = Project::at(root);
    let mut lines = vec![serde_json::to_string(&Event {
        v: 1,
        seq: 1,
        at: 1,
        kind: Ev::Opened {
            project: root.display().to_string(),
            orchestrator_pane: None,
            auto: false,
            pools: Vec::new(),
            pools_basis: None,
        },
    })
    .unwrap()];
    for (n, order) in orders.iter().enumerate() {
        let text = format!(
            "# Step {}\n\nMake piece {} of the hall.\n",
            order.id, order.id
        );
        let file = crate::nirvana_worker::write_order(&project, root, &order.id, &text).unwrap();
        lines.push(
            serde_json::to_string(&Event {
                v: 1,
                seq: n as u64 + 2,
                at: n as u64 + 2,
                kind: Ev::Seated(Box::new(seated(order, &file))),
            })
            .unwrap(),
        );
    }
    let dir = project.run_dir(run);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(crate::nirvana_run::JOURNAL),
        lines.join("\n") + "\n",
    )
    .unwrap();
}

/// A window whose one space, `ws_1`, has its focused pane in `root`.
fn state_in(root: &Path) -> ClientShellState {
    crate::nirvana_preload::set_view_for_test(Some(crate::nirvana_preload::View::default()));
    let mut projected = snapshot();
    let cwd = root.display().to_string();
    projected.panes[0].cwd = Some(cwd.clone());
    projected.panes[0].foreground_cwd = Some(cwd.clone());
    projected.workspaces[0].new_workspace_cwd = cwd;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state
}

fn rows_of(state: &mut ClientShellState, width: u16) -> Vec<String> {
    let frame = state.compose(width, HEIGHT).expect("composed frame").frame;
    frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol.as_str()).collect())
        .collect()
}

fn click(state: &mut ClientShellState, column: u16, row: u16) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })])
}

fn wheel(state: &mut ClientShellState, down: bool, column: u16, row: u16) {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: if down {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        },
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })]);
}

fn key(state: &mut ClientShellState, code: KeyCode) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        code,
        KeyModifiers::NONE,
    ))])
}

fn typed(state: &mut ClientShellState, text: &str) {
    for c in text.chars() {
        key(state, KeyCode::Char(c));
    }
}

/// The `command.invoke` ids an input sent.
fn invoked(outcome: &ClientShellInput) -> Vec<String> {
    outcome
        .actions
        .iter()
        .filter_map(|action| match action {
            ClientShellAction::Endpoint { request, .. } => match &request.method {
                Method::CommandInvoke(params) => Some(params.command_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Opens the panel with a click on the `+`, at a window `width` wide, and returns the frame's rows after.
fn open(state: &mut ClientShellState, width: u16) -> Vec<String> {
    let _ = rows_of(state, width);
    let plus = state.hits.agent_profiles;
    assert!(plus.width > 0, "the agents heading draws a +");
    click(state, plus.x, plus.y);
    assert!(
        state.nirvana_profile_panel.is_some(),
        "a click on the + opens the panel"
    );
    rows_of(state, width)
}

/// The panel's own rows: from its top border to its bottom border, the panel's cells only.
fn panel_rows(state: &ClientShellState, rows: &[String]) -> Vec<String> {
    let area = state.hits.nirvana_profile_panel.area;
    assert!(area.width > 0, "the panel was drawn");
    rows[usize::from(area.y)..usize::from(area.bottom())]
        .iter()
        .map(|row| {
            row.chars()
                .skip(usize::from(area.x))
                .take(usize::from(area.width))
                .collect::<String>()
        })
        .collect()
}

/// The panel's rows of a frame composed now.
fn shown(state: &mut ClientShellState, width: u16) -> Vec<String> {
    let rows = rows_of(state, width);
    panel_rows(state, &rows)
}

/// The panel's rows after a click on the `+` opens it.
fn opened(state: &mut ClientShellState, width: u16) -> Vec<String> {
    let rows = open(state, width);
    panel_rows(state, &rows)
}

fn line<'a>(rows: &'a [String], needle: &str) -> &'a str {
    rows.iter()
        .find(|row| row.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?}:\n{}", rows.join("\n")))
}

fn panel(state: &ClientShellState) -> &panel_mod::ProfilePanel {
    state
        .nirvana_profile_panel
        .as_ref()
        .expect("the panel is open")
}

/// The three kinds of row the brief names: every field, bare, refused.
fn three_profiles(root: &Path) {
    profile_folder(
        root,
        "reviewer",
        "---\ndescription: Judges renders against the rubric card.\nclass: media.video\nfamily: gemini\n\
model: claude-opus-5-5\neffort: high\nneeds: [fake-tool]\nskills: [alpha]\n---\nYou judge renders.\n",
        3,
    );
    std::fs::write(
        crate::nirvana_profiles::folder_of(root, "reviewer").join("mcp.json"),
        r#"{"mcpServers": {"own-fake": {"command": "fake-own-not-real"}}}"#,
    )
    .unwrap();
    profile_file(
        root,
        "bare",
        "---\ndescription: Says nothing of its use.\n---\nAnything.\n",
    );
    profile_file(root, "broken", "---\ncolour: red\n---\n");
}

#[test]
fn p93_the_plus_follows_agents_and_a_click_opens_the_panel_on_the_focused_space() {
    let (home, root) = project("p93-plus");
    // A second space in another project, focused: the + opens that one.
    let other = home.path.join("other");
    std::fs::create_dir_all(other.join(".git")).unwrap();
    let inside = other.join("src");
    std::fs::create_dir_all(&inside).unwrap();
    let mut state = state_in(&root);
    let mut projected = state.snapshot.as_deref().cloned().unwrap();
    projected.workspaces[0].focused = false;
    let mut second = projected.workspaces[0].clone();
    second.workspace_id = "ws_2".into();
    second.active_tab_id = "tab_2".into();
    second.label = "other".into();
    second.focused = true;
    projected.workspaces.push(second);
    projected.panes.push(ClientShellPane {
        pane_id: "pane_2".into(),
        workspace_id: "ws_2".into(),
        tab_id: "tab_2".into(),
        label: None,
        cwd: Some(inside.display().to_string()),
        foreground_cwd: Some(inside.display().to_string()),
        focused: false,
        right_click_passthrough: false,
    });
    projected.focused_workspace_id = Some("ws_2".into());
    state.set_snapshot(Box::new(projected));
    let rows = rows_of(&mut state, 140);
    let plus = state.hits.agent_profiles;
    let heading = &rows[usize::from(plus.y)];
    assert!(heading.starts_with(" agents [+]"), "{heading:?}");
    assert!(
        heading.contains(" agents [+]       grouped\u{2502}"),
        "the sort label keeps its place: {heading:?}"
    );
    assert_eq!(plus.width, 3);
    click(&mut state, plus.x + 1, plus.y);
    let open = panel(&state);
    assert_eq!(
        open.root, other,
        "the focused space's project, found from a folder inside it"
    );
    assert_eq!(
        (open.workspace_id.as_str(), open.tab_id.as_str()),
        ("ws_2", "tab_2")
    );
    // A second click on the + closes it.
    let _ = rows_of(&mut state, 140);
    click(&mut state, plus.x, plus.y);
    assert!(
        state.nirvana_profile_panel.is_none(),
        "a second click on the + closes it"
    );
    drop(home);
}

#[test]
fn p93_the_plus_gives_way_to_the_sort_label_at_a_narrow_sidebar() {
    let (_home, root) = project("p93-narrow");
    let state = state_in(&root);
    let snap = state.snapshot.as_deref().unwrap();
    let draw = |width: u16, sort: crate::config::AgentPanelSortConfig| {
        let mut config = ClientShellConfig::from_config(&Config::default());
        config.agent_panel_sort = sort;
        let area = Rect::new(0, 0, width, 6);
        let mut buffer = Buffer::empty(area);
        let mut scroll = 0;
        let mut hits = ShellHitMap::default();
        super::super::agent_sidebar::render_agent_panel(
            &mut buffer,
            area,
            snap,
            &config,
            &mut scroll,
            &mut hits,
        );
        let heading: String = (0..width)
            .map(|x| buffer.cell((x, 1)).unwrap().symbol().to_string())
            .collect();
        (heading, hits.agent_profiles)
    };
    use crate::config::AgentPanelSortConfig::{Priority, Spaces};
    // 26, the default sidebar: both, a cell or more between.
    let (heading, plus) = draw(26, Priority);
    assert_eq!(heading, " agents [+]       priority", "{heading:?}");
    assert_eq!(plus, Rect::new(8, 1, 3, 1));
    // 20 holds both with one cell between; 19 does not with `priority`, and the + gives way, the label whole.
    let (heading, plus) = draw(20, Priority);
    assert_eq!(heading, " agents [+] priority");
    assert!(plus.width > 0);
    let (heading, plus) = draw(19, Priority);
    assert_eq!(heading, " agents    priority");
    assert_eq!(plus, Rect::default());
    // 19 still holds it beside the shorter `grouped`; 18, the narrowest sidebar, does not.
    let (heading, plus) = draw(19, Spaces);
    assert_eq!(heading, " agents [+] grouped");
    assert!(plus.width > 0);
    let (heading, plus) = draw(18, Spaces);
    assert_eq!(heading, " agents    grouped");
    assert_eq!(plus, Rect::default());
}

#[test]
fn p93_the_plus_marks_the_proposals_waiting() {
    let (_home, root) = project("p93-mark");
    assert_eq!(panel_mod::plus_text(Some(0)), "[+]");
    assert_eq!(panel_mod::plus_text(None), "[+]");
    let mut state = state_in(&root);
    let rows = rows_of(&mut state, 140);
    assert!(rows[usize::from(state.hits.agent_profiles.y)].starts_with(" agents [+] "));
    propose_new(&root, "render-judge");
    let rows = rows_of(&mut state, 140);
    let plus = state.hits.agent_profiles;
    assert!(
        rows[usize::from(plus.y)].starts_with(" agents [+1] "),
        "{:?}",
        rows[usize::from(plus.y)]
    );
    assert_eq!(plus.width, 4);
    propose_new(&root, "second-judge");
    let rows = rows_of(&mut state, 140);
    assert!(rows[usize::from(state.hits.agent_profiles.y)].starts_with(" agents [+2] "));
}

#[test]
fn p93_the_space_menu_has_a_profiles_line_that_opens_the_panel() {
    let (_home, root) = project("p93-menu");
    let mut state = state_in(&root);
    state.open_workspace_context_menu("ws_1".into(), 0, 0);
    let Some(ClientShellOverlay::ContextMenu(menu)) = state.overlay.as_ref() else {
        panic!("space menu");
    };
    let labels: Vec<&str> = menu.items().iter().map(|item| item.label).collect();
    assert_eq!(labels.last(), Some(&"Profiles"), "{labels:?}");
    // Every line before it keeps its place.
    assert_eq!(
        labels,
        [
            "Rename",
            "Close",
            "New worktree",
            "Open worktree...",
            "Profiles"
        ]
    );
    let mut outcome = ClientShellInput::default();
    state.activate_context_menu_item(labels.len() - 1, &mut outcome);
    assert!(state.overlay.is_none());
    let open = panel(&state);
    assert_eq!(open.root, root);
    assert_eq!(open.workspace_id, "ws_1");
    let rows = rows_of(&mut state, 140);
    let text = panel_rows(&state, &rows).join("\n");
    assert!(text.contains("Profiles \u{b7} project"), "{text}");
}

#[test]
fn p93_a_profile_row_with_every_field_a_bare_one_and_a_refused_one() {
    let (_home, root) = project("p93-rows");
    three_profiles(&root);
    let mut state = state_in(&root);
    let rows = open(&mut state, 200);
    let rows = panel_rows(&state, &rows);
    let text = rows.join("\n");
    assert!(
        line(&rows, "reviewer").contains("Judges renders against the rubric card."),
        "{text}"
    );
    let at = rows
        .iter()
        .position(|row| row.contains("family gemini"))
        .unwrap();
    let facts = format!("{} {}", rows[at], rows[at + 1]);
    for part in [
        "family gemini",
        "model claude-opus-5-5",
        "effort high",
        "servers fake-tool + own mcp.json",
        "skills alpha",
        "3 cards",
        NO_USE,
    ] {
        assert!(facts.contains(part), "{part:?} in {facts:?}\n{text}");
    }
    // The bare one: its description, its use, nothing it does not set.
    assert!(
        line(&rows, "bare").contains("Says nothing of its use."),
        "{text}"
    );
    let bare_at = rows.iter().position(|row| row.contains("bare ")).unwrap();
    let bare_facts = &rows[bare_at + 1];
    assert!(bare_facts.contains(NO_USE), "{bare_facts:?}");
    for absent in ["family", "model", "effort", "servers", "skills", "card"] {
        assert!(!bare_facts.contains(absent), "{absent} in {bare_facts:?}");
    }
    assert!(
        line(&rows, "broken").contains("refused: herdr profile check broken"),
        "{text}"
    );
    assert!(text.contains("[Profiles \u{b7} 3]"), "{text}");
    // `skills: []` says none.
    profile_file(
        &root,
        "quiet",
        "---\ndescription: Uses no skill.\nskills: []\n---\nQuiet.\n",
    );
    let rows = shown(&mut state, 200);
    assert!(
        line(&rows, "skills none").contains(NO_USE),
        "{}",
        rows.join("\n")
    );
}

#[test]
fn p93_the_use_reads_carried_of_fit_and_in_words_when_nothing_fits() {
    let (_home, root) = project("p93-use");
    profile_file(
        &root,
        "carried",
        "---\ndescription: Builds.\nclass: build.bounded\n---\nBuild.\n",
    );
    profile_file(
        &root,
        "unused",
        "---\ndescription: Judges video.\nclass: media.video\nneeds: [fake-tool]\n---\nJudge.\n",
    );
    profile_file(
        &root,
        "never",
        "---\ndescription: Researches.\nclass: research.web\n---\nRead.\n",
    );
    let mut all = orders("c", 6, "build.bounded", &[], Some("carried"));
    all.extend(orders("v", 5, "media.video", &["fake-tool"], None));
    seat_all(&root, "run-1", &all);
    let mut state = state_in(&root);
    let rows = opened(&mut state, 200);
    let text = rows.join("\n");
    let facts = |name: &str| {
        let at = rows
            .iter()
            .position(|row| row.contains(&format!("{name} ")))
            .unwrap();
        rows[at + 1].clone()
    };
    assert!(facts("carried").contains("used 6 of 6"), "{text}");
    assert!(facts("unused").contains("used 0 of 5"), "{text}");
    assert!(facts("never").contains(NO_USE), "{text}");
    // And each as `use_text` says it.
    let row = |use_| panel_mod::ProfileRow {
        name: "x".into(),
        file: PathBuf::from("x.md"),
        parsed: None,
        use_,
    };
    assert_eq!(
        panel_mod::use_text(&row(Some((0, 35))), true),
        "used 0 of 35"
    );
    assert_eq!(
        panel_mod::use_text(&row(Some((35, 35))), true),
        "used 35 of 35"
    );
    assert_eq!(panel_mod::use_text(&row(Some((0, 0))), true), NO_USE);
    assert_eq!(panel_mod::use_text(&row(None), true), "use: not counted");
    assert_eq!(
        panel_mod::use_text(&row(Some((1, 2))), false),
        "use: counting"
    );
}

#[test]
fn p93_the_empty_states_say_so_in_plain_words() {
    let (_home, root) = project("p93-empty");
    let mut state = state_in(&root);
    let rows = opened(&mut state, 140);
    let text = rows.join("\n");
    assert!(text.contains(NO_PROFILES), "{text}");
    assert!(text.contains(NOTHING_SUGGESTED), "{text}");
    assert!(
        text.contains("[Profiles \u{b7} 0]") && text.contains(" Suggested \u{b7} 0 "),
        "{text}"
    );
    // Keys on empty lists never panic and say why.
    for code in [
        KeyCode::Down,
        KeyCode::End,
        KeyCode::Enter,
        KeyCode::Char('e'),
        KeyCode::Char('K'),
    ] {
        key(&mut state, code);
    }
    assert!(state.nirvana_profile_panel.is_some());
    key(&mut state, KeyCode::Tab);
    for code in [KeyCode::PageDown, KeyCode::Enter, KeyCode::Char('d')] {
        key(&mut state, code);
    }
    assert_eq!(panel(&state).message.as_deref(), Some(NOTHING_SUGGESTED));
    let _ = rows_of(&mut state, 140);
}

#[test]
fn p93_suggested_lists_each_proposal_and_enter_opens_its_whole_detail() {
    let (_home, root) = project("p93-detail");
    profile_folder(
        &root,
        "reviewer",
        "---\ndescription: Judges.\n---\nYou judge.\n",
        0,
    );
    let new = propose_new(&root, "render-judge");
    // A body much longer than the panel, with its last words to find at the end.
    let body: String = (0..80)
        .map(|n| format!("Line {n} of the judging method, said once."))
        .collect::<Vec<_>>()
        .join("\n")
        + "\nTHE LAST LINE";
    let patch = propose_patch(&root, "reviewer", &body);
    let mut state = state_in(&root);
    let rows = opened(&mut state, 200);
    let text = rows.join("\n");
    assert!(
        line(&rows, &new).contains(&format!(
            "{new} \u{b7} new \u{b7} render-judge \u{b7} 7 orders repeat"
        )),
        "{text}"
    );
    assert!(
        line(&rows, &patch).contains(&format!("{patch} \u{b7} patch \u{b7} reviewer")),
        "{text}"
    );
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Down);
    key(&mut state, KeyCode::Enter);
    let detail = panel(&state)
        .detail
        .clone()
        .expect("enter opens the detail");
    assert_eq!(detail.id, patch);
    let rows = shown(&mut state, 200);
    let text = rows.join("\n");
    assert!(
        text.contains(&format!("{patch} \u{b7} a patch of reviewer")),
        "{text}"
    );
    assert!(text.contains("Evidence"), "{text}");
    assert!(
        text.contains("Judge the rendered frame, never the plan or the code"),
        "{text}"
    );
    assert!(text.contains("Line 0 of the judging method"), "{text}");
    assert!(
        !text.contains("THE LAST LINE"),
        "the body is longer than the panel: {text}"
    );
    assert!(text.contains("a accept \u{b7} d dismiss"), "{text}");
    // Scrolled to its end, the whole body has been on screen.
    key(&mut state, KeyCode::End);
    let rows = shown(&mut state, 200);
    assert!(rows.join("\n").contains("THE LAST LINE"));
    key(&mut state, KeyCode::Home);
    key(&mut state, KeyCode::PageDown);
    assert!(panel(&state).detail.as_ref().unwrap().scroll > 0);
    let rows = rows_of(&mut state, 200);
    let area = state.hits.nirvana_profile_panel.area;
    wheel(&mut state, false, area.x + 3, area.y + 3);
    let _ = rows;
    let after = panel(&state).detail.as_ref().unwrap().scroll;
    assert!(
        after < 10 && after > 0,
        "the wheel scrolls the detail back: {after}"
    );
}

#[test]
fn p93_accept_is_only_in_the_detail_and_its_note_stays_until_the_next_key() {
    let (_home, root) = project("p93-accept");
    let id = propose_new(&root, "render-judge");
    let mut state = state_in(&root);
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::Tab);
    // `a` in the list accepts nothing and says where Accept is.
    key(&mut state, KeyCode::Char('a'));
    assert_eq!(panel(&state).message.as_deref(), Some(ACCEPT_IN_DETAIL));
    assert_eq!(proposals::list(&root).len(), 1, "still waiting");
    assert!(!crate::nirvana_profiles::folder_of(&root, "render-judge").exists());
    let rows = shown(&mut state, 200);
    assert!(rows
        .join("\n")
        .contains("a accepts only in a proposal's detail"));
    key(&mut state, KeyCode::Enter);
    assert!(panel(&state).detail.is_some());
    key(&mut state, KeyCode::Char('a'));
    let open = panel(&state);
    assert!(
        open.detail.is_none(),
        "the proposal is gone, so its detail closes"
    );
    let said: Vec<&str> = open.notice.iter().map(|(line, _)| line.as_str()).collect();
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said[0].starts_with(&format!("op-1: accepted {id}, new profile render-judge")),
        "{said:?}"
    );
    assert_eq!(said[1], proposals::ACCEPT_NOTE);
    let rows = shown(&mut state, 200);
    let text = rows.join("\n");
    assert!(text.contains("op-1: accepted"), "{text}");
    assert!(
        text.contains("note: the profile file is not ignored by git"),
        "{text}"
    );
    let flat: String = rows
        .iter()
        .map(|row| row.trim_matches('\u{2502}').trim())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        flat.contains(proposals::ACCEPT_NOTE),
        "the note whole: {text}"
    );
    // The profile is listed now, and the note goes with the next key.
    assert!(
        text.contains("render-judge  Judges renders against the rubric card."),
        "{text}"
    );
    key(&mut state, KeyCode::Down);
    let rows = shown(&mut state, 200);
    assert!(!rows.join("\n").contains("op-1: accepted"));
}

#[test]
fn p93_an_accept_writes_one_profile_and_one_ledger_row_and_a_dismiss_no_profile() {
    let (_home, root) = project("p93-writes");
    let profiles_before = crate::nirvana_profiles::list(&root).len();
    let first = propose_new(&root, "render-judge");
    let second = propose_new(&root, "other-judge");
    let mut state = state_in(&root);
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Enter);
    assert_eq!(panel(&state).detail.as_ref().unwrap().id, first);
    key(&mut state, KeyCode::Char('a'));
    let listed = crate::nirvana_profiles::list(&root);
    assert_eq!(listed.len(), profiles_before + 1, "exactly one profile");
    assert_eq!(listed[0].0, "render-judge");
    let ledger = proposals::ledger(&root);
    assert_eq!(ledger.len(), 1, "exactly one ledger row");
    assert_eq!(
        (ledger[0].action.as_str(), ledger[0].proposal.as_str()),
        ("accept", first.as_str())
    );
    // Dismiss the other from the list: a ledger row, no profile.
    let _ = rows_of(&mut state, 200);
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Tab);
    assert_eq!(panel(&state).focus, Part::Suggested);
    key(&mut state, KeyCode::Char('d'));
    assert_eq!(
        crate::nirvana_profiles::list(&root).len(),
        profiles_before + 1,
        "a dismiss writes no profile"
    );
    assert!(!crate::nirvana_profiles::folder_of(&root, "other-judge").exists());
    let ledger = proposals::ledger(&root);
    assert_eq!(ledger.len(), 2);
    assert_eq!(
        (ledger[1].action.as_str(), ledger[1].proposal.as_str()),
        ("dismiss", second.as_str())
    );
    assert!(proposals::list(&root).is_empty());
    let said = &panel(&state).notice[0].0;
    assert!(
        said.starts_with(&format!("op-2: dismissed {second}")),
        "{said}"
    );
}

#[test]
fn p93_a_refused_accept_is_shown_word_for_word_and_dismiss_works_from_the_detail() {
    let (_home, root) = project("p93-refused");
    let file = profile_folder(
        &root,
        "reviewer",
        "---\ndescription: Judges.\n---\nYou judge.\n",
        0,
    );
    let id = propose_patch(&root, "reviewer", "You judge renders, harder.");
    // The profile changes after the proposal was made: the store refuses the stale patch.
    std::fs::write(
        &file,
        "---\ndescription: Judges, edited.\n---\nYou judge.\n",
    )
    .unwrap();
    let mut state = state_in(&root);
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Char('a'));
    let open = panel(&state);
    assert!(
        open.detail.is_some(),
        "a refused accept stays in the detail"
    );
    let said = open.notice[0].0.clone();
    // A refused accept changes nothing, so the store says it again in the same words.
    let again = proposals::accept(&root, &id).unwrap_err();
    assert_eq!(said, again, "word for word");
    assert!(said.contains("is stale"), "{said}");
    assert_eq!(proposals::ledger(&root).len(), 0);
    let rows = shown(&mut state, 200);
    let joined: String = rows
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let flat: String = said.split_whitespace().collect::<Vec<_>>().join(" ");
    // Every word of it on screen, wrapped and never cut.
    let words: Vec<&str> = flat.split(' ').collect();
    for word in words.iter().filter(|word| word.len() < 40) {
        assert!(
            joined.contains(word),
            "{word:?} not on screen:\n{}",
            rows.join("\n")
        );
    }
    // `d` in the detail dismisses it.
    key(&mut state, KeyCode::Char('d'));
    assert!(panel(&state).detail.is_none());
    assert!(proposals::list(&root).is_empty());
    assert_eq!(proposals::ledger(&root)[0].action, "dismiss");
}

#[test]
fn p93_new_with_and_without_a_shape_and_a_refused_name() {
    let (_home, root) = project("p93-new");
    let mut state = state_in(&root);
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::Char('n'));
    typed(&mut state, "sli cer");
    assert_eq!(panel(&state).naming.as_deref(), Some("slicer"));
    let rows = shown(&mut state, 200);
    assert!(rows.join("\n").contains("New profile: slicer_"));
    key(&mut state, KeyCode::Enter);
    assert_eq!(panel(&state).shaping, Some(("slicer".to_owned(), 0)));
    let rows = shown(&mut state, 200);
    let text = rows.join("\n");
    // The shape names are the library's.
    for shape in crate::nirvana_profiles::SHAPES {
        assert!(text.contains(shape), "{shape} in {text}");
    }
    assert!(text.contains("[0 none]"), "{text}");
    let outcome = key(&mut state, KeyCode::Enter);
    let file = crate::nirvana_profiles::path(&root, "slicer");
    assert!(file.is_file(), "no shape: the template, one file");
    assert_eq!(
        invoked(&outcome),
        [format!(
            "{}{}",
            crate::nirvana::EDIT_FILE_COMMAND,
            file.display()
        )]
    );
    assert!(
        state.nirvana_profile_panel.is_none(),
        "the edit tab is what the owner sees"
    );
    // With a shape: the folder form.
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::Char('n'));
    typed(&mut state, "judge");
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Right);
    assert_eq!(panel(&state).shaping, Some(("judge".to_owned(), 1)));
    key(&mut state, KeyCode::Left);
    key(&mut state, KeyCode::Left);
    assert_eq!(
        panel(&state).shaping.as_ref().unwrap().1,
        crate::nirvana_profiles::SHAPES.len()
    );
    key(&mut state, KeyCode::Char('1'));
    let outcome = key(&mut state, KeyCode::Enter);
    let made = crate::nirvana_profiles::folder_of(&root, "judge").join("profile.md");
    assert!(made.is_file(), "a shape writes the folder form");
    assert!(crate::nirvana_profiles::load(&root, "judge").is_ok());
    assert!(!crate::nirvana_knowledge::cards(
        &crate::nirvana_profiles::folder_of(&root, "judge").join("knowledge")
    )
    .is_empty());
    assert_eq!(
        invoked(&outcome),
        [format!(
            "{}{}",
            crate::nirvana::EDIT_FILE_COMMAND,
            made.display()
        )]
    );
    // A refused name is said in the panel, and nothing is written.
    for (name, said) in [
        ("con", "Windows device name"),
        ("-x", "never starts with `-`"),
    ] {
        let _ = open(&mut state, 200);
        key(&mut state, KeyCode::Char('n'));
        typed(&mut state, name);
        key(&mut state, KeyCode::Enter);
        let open = panel(&state);
        assert!(
            open.shaping.is_none(),
            "{name}: no shape is asked for a refused name"
        );
        assert!(
            open.notice[0].0.starts_with("not made: "),
            "{:?}",
            open.notice
        );
        assert!(open.notice[0].0.contains(said), "{:?}", open.notice);
        let rows = shown(&mut state, 200);
        assert!(rows.join("\n").contains("not made: "));
        assert!(!crate::nirvana_profiles::path(&root, name).exists());
        key(&mut state, KeyCode::Esc);
    }
    // A name already there is refused by the library, after the shape.
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::Char('n'));
    typed(&mut state, "slicer");
    key(&mut state, KeyCode::Enter);
    let outcome = key(&mut state, KeyCode::Enter);
    assert!(invoked(&outcome).is_empty());
    assert!(
        panel(&state).notice[0].0.contains("already there"),
        "{:?}",
        panel(&state).notice
    );
    // Esc while naming cancels the name, not the panel.
    key(&mut state, KeyCode::Char('n'));
    key(&mut state, KeyCode::Esc);
    assert!(panel(&state).naming.is_none());
}

#[test]
fn p93_e_enter_and_k_open_the_profile_and_its_cards() {
    let (_home, root) = project("p93-edit");
    let folder_file = profile_folder(
        &root,
        "reel",
        "---\ndescription: Edits reels.\n---\nYou edit.\n",
        2,
    );
    let one_file = profile_file(
        &root,
        "slicer",
        "---\ndescription: Slices.\n---\nYou slice.\n",
    );
    let mut state = state_in(&root);
    let _ = open(&mut state, 200);
    let outcome = key(&mut state, KeyCode::Enter);
    assert_eq!(
        invoked(&outcome),
        [format!(
            "{}{}",
            crate::nirvana::EDIT_FILE_COMMAND,
            folder_file.display()
        )]
    );
    assert!(state.nirvana_profile_panel.is_none());
    let _ = open(&mut state, 200);
    let outcome = key(&mut state, KeyCode::Char('K'));
    let knowledge = crate::nirvana_profiles::folder_of(&root, "reel").join("knowledge");
    assert_eq!(
        invoked(&outcome),
        [format!(
            "{}{}",
            crate::nirvana::EDIT_DIR_COMMAND,
            knowledge.display()
        )]
    );
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::Char('j'));
    let outcome = key(&mut state, KeyCode::Char('e'));
    assert_eq!(
        invoked(&outcome),
        [format!(
            "{}{}",
            crate::nirvana::EDIT_FILE_COMMAND,
            one_file.display()
        )]
    );
    // K on a one-file profile says why and opens nothing.
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::End);
    let outcome = key(&mut state, KeyCode::Char('K'));
    assert!(invoked(&outcome).is_empty());
    assert!(panel(&state)
        .message
        .as_deref()
        .unwrap()
        .contains("slicer keeps no knowledge cards"));
}

#[test]
fn p93_keys_tab_the_wheel_and_a_click_select() {
    let (_home, root) = project("p93-keys");
    for n in 0..14 {
        profile_file(
            &root,
            &format!("p{n:02}"),
            "---\ndescription: One of many.\n---\nBody.\n",
        );
    }
    propose_new(&root, "judge-a");
    propose_new(&root, "judge-b");
    let mut state = state_in(&root);
    let _ = open(&mut state, 200);
    let at = |state: &ClientShellState| panel(state).profile_selected;
    key(&mut state, KeyCode::Down);
    key(&mut state, KeyCode::Char('j'));
    assert_eq!(at(&state), 2);
    key(&mut state, KeyCode::Char('k'));
    assert_eq!(at(&state), 1);
    key(&mut state, KeyCode::PageDown);
    assert_eq!(at(&state), 11);
    key(&mut state, KeyCode::End);
    assert_eq!(at(&state), 13);
    key(&mut state, KeyCode::Down);
    assert_eq!(at(&state), 13, "stops at the end");
    key(&mut state, KeyCode::PageUp);
    assert_eq!(at(&state), 3);
    key(&mut state, KeyCode::Home);
    assert_eq!(at(&state), 0);
    key(&mut state, KeyCode::Up);
    assert_eq!(at(&state), 0, "stops at the top");
    // The footer names the focused part's keys; Tab moves to Suggested and the footer with it.
    let rows = shown(&mut state, 200);
    assert!(rows.join("\n").contains("enter/e edit \u{b7} K cards"));
    key(&mut state, KeyCode::Tab);
    assert_eq!(panel(&state).focus, Part::Suggested);
    let rows = shown(&mut state, 200);
    let text = rows.join("\n");
    assert!(text.contains("enter open \u{b7} d dismiss"), "{text}");
    assert!(text.contains("[Suggested \u{b7} 2]"), "{text}");
    key(&mut state, KeyCode::Down);
    assert_eq!(panel(&state).proposal_selected, 1);
    key(&mut state, KeyCode::BackTab);
    assert_eq!(panel(&state).focus, Part::Profiles);
    // The wheel moves the focused list's selection.
    let _ = rows_of(&mut state, 200);
    let area = state.hits.nirvana_profile_panel.area;
    wheel(&mut state, true, area.x + 4, area.y + 4);
    assert_eq!(at(&state), 3);
    wheel(&mut state, false, area.x + 4, area.y + 4);
    assert_eq!(at(&state), 0);
    // A click on a proposal row selects it and moves the focus there; a click on a heading moves the focus.
    let _ = rows_of(&mut state, 200);
    let (rect, index) = state
        .hits
        .nirvana_profile_panel
        .proposals
        .last()
        .copied()
        .expect("a proposal row");
    click(&mut state, rect.x + 3, rect.y);
    assert_eq!(
        (panel(&state).focus, panel(&state).proposal_selected),
        (Part::Suggested, index)
    );
    let _ = rows_of(&mut state, 200);
    let (rect, part) = state.hits.nirvana_profile_panel.headings[0];
    assert_eq!(part, Part::Profiles);
    click(&mut state, rect.x + 1, rect.y);
    assert_eq!(panel(&state).focus, Part::Profiles);
    let _ = rows_of(&mut state, 200);
    let (rect, index) = state.hits.nirvana_profile_panel.profiles[2];
    click(&mut state, rect.x + 3, rect.y);
    assert_eq!(at(&state), index);
    // The selected row reads without colour: `▸` before it.
    let rows = shown(&mut state, 200);
    assert!(
        rows.iter()
            .any(|row| row.contains(&format!("\u{25b8} p{index:02}"))),
        "{}",
        rows.join("\n")
    );
    // A click outside the panel closes it.
    click(&mut state, 0, 0);
    assert!(state.nirvana_profile_panel.is_none());
}

#[test]
fn p93_the_selection_follows_its_name_or_id_through_a_reread() {
    let (_home, root) = project("p93-follow");
    profile_file(&root, "beta", "---\ndescription: B.\n---\nB.\n");
    profile_file(&root, "gamma", "---\ndescription: C.\n---\nC.\n");
    let first = propose_new(&root, "judge-a");
    let second = propose_new(&root, "judge-b");
    let mut state = state_in(&root);
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::Down);
    assert_eq!(panel(&state).profile_key.as_deref(), Some("gamma"));
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Down);
    assert_eq!(panel(&state).proposal_key.as_deref(), Some(second.as_str()));
    // A profile sorted before it lands, and the proposal before it goes, between two draws.
    profile_file(&root, "alpha", "---\ndescription: A.\n---\nA.\n");
    proposals::dismiss(&root, &first, None).unwrap();
    let _ = rows_of(&mut state, 200);
    let open = panel(&state);
    assert_eq!(
        (open.profile_selected, open.profile_key.as_deref()),
        (2, Some("gamma"))
    );
    assert_eq!(
        (open.proposal_selected, open.proposal_key.as_deref()),
        (0, Some(second.as_str()))
    );
    // When the selected one goes, the index holds to the rows there are.
    std::fs::remove_file(crate::nirvana_profiles::path(&root, "gamma")).unwrap();
    let _ = rows_of(&mut state, 200);
    assert_eq!(panel(&state).profile_selected, 1);
    assert_eq!(panel(&state).profile_key.as_deref(), Some("beta"));
}

#[test]
fn p93_esc_closes_the_detail_then_the_panel() {
    let (_home, root) = project("p93-esc");
    propose_new(&root, "judge-a");
    let mut state = state_in(&root);
    let _ = open(&mut state, 200);
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Enter);
    assert!(panel(&state).detail.is_some());
    key(&mut state, KeyCode::Esc);
    assert!(
        panel(&state).detail.is_none(),
        "the first esc closes the detail"
    );
    key(&mut state, KeyCode::Esc);
    assert!(
        state.nirvana_profile_panel.is_none(),
        "the second the panel"
    );
}

#[test]
fn p93_80_100_and_140_columns() {
    let (_home, root) = project("p93-widths");
    three_profiles(&root);
    let id = propose_new(&root, "render-judge");
    let dump = std::env::var_os("P93_DUMP_DIR").map(PathBuf::from);
    for width in [80u16, 100, 140] {
        let mut state = state_in(&root);
        let rows = opened(&mut state, width);
        let text = rows.join("\n");
        let area = state.hits.nirvana_profile_panel.area;
        assert!(area.right() <= width, "{width}: inside the window");
        // Every part readable at every width: names, the refusal, the use, the proposal, the keys.
        for needle in [
            "reviewer",
            "Judges renders",
            "bare",
            "broken",
            "refused:",
            NO_USE,
            id.as_str(),
            "esc close",
        ] {
            assert!(text.contains(needle), "{width}: {needle:?}\n{text}");
        }
        // The every-field row is packed onto lines, never cut: each fact whole.
        for part in [
            "family gemini",
            "model claude-opus-5-5",
            "servers fake-tool + own mcp.json",
            "3 cards",
        ] {
            assert!(text.contains(part), "{width}: {part:?}\n{text}");
        }
        if let Some(dir) = &dump {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(
                dir.join(format!("list-{width}.txt")),
                rows_of(&mut state, width).join("\n"),
            )
            .unwrap();
        }
        key(&mut state, KeyCode::Tab);
        key(&mut state, KeyCode::Enter);
        let rows = shown(&mut state, width);
        let text = rows.join("\n");
        assert!(
            text.contains("Evidence") && text.contains("You judge renders."),
            "{width}: {text}"
        );
        assert!(text.contains("a accept"), "{width}: {text}");
        if let Some(dir) = &dump {
            std::fs::write(
                dir.join(format!("detail-{width}.txt")),
                rows_of(&mut state, width).join("\n"),
            )
            .unwrap();
        }
    }
}

#[test]
fn p93_a_long_name_and_a_4000_character_body_never_panic() {
    let (_home, root) = project("p93-long");
    // A name the parser refuses for its length is still listed, as refused.
    let long = "a".repeat(80);
    profile_file(
        &root,
        &long,
        "---\ndescription: Too long a name.\n---\nX.\n",
    );
    let description = "word ".repeat(23) + "end";
    profile_file(
        &root,
        "wordy",
        &format!("---\ndescription: {description}\n---\nX.\n"),
    );
    let body = format!(
        "---\ndescription: Long.\n---\n{}",
        "x".repeat(crate::nirvana_profiles::BODY_MAX - 1)
    );
    let id = proposals::write(
        &root,
        &draft(Kind::New, "long-body", "paragraph:long", &body, None),
    );
    for width in [80u16, 140] {
        let mut state = state_in(&root);
        let rows = opened(&mut state, width);
        let text = rows.join("\n");
        assert!(
            text.contains("refused: herdr profile check"),
            "{width}: {text}"
        );
        assert!(
            text.contains("end"),
            "{width}: the description wraps whole: {text}"
        );
        if id.is_ok() {
            key(&mut state, KeyCode::Tab);
            key(&mut state, KeyCode::Enter);
            key(&mut state, KeyCode::End);
            let _ = rows_of(&mut state, width);
            key(&mut state, KeyCode::PageUp);
            let _ = rows_of(&mut state, width);
        }
    }
    // The wrap holds any text.
    assert_eq!(panel_mod::wrap("", 10), [""]);
    assert_eq!(panel_mod::wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
    assert_eq!(
        panel_mod::wrap("one two\n\nthree", 7),
        ["one two", "", "three"]
    );
    assert_eq!(panel_mod::wrap("x", 0), ["x"]);
    assert!(
        id.is_ok(),
        "a 4000-character body is a proposal the store takes: {id:?}"
    );
}

fn pane_patch(state: &ClientShellState) -> crate::protocol::PaneSurfacePatch {
    let current = state.pane_surface.clone().expect("pane surface");
    let mut cell = current.frame.cells[0].clone();
    cell.symbol = "y".into();
    crate::protocol::PaneSurfacePatch {
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
    }
}

/// A pane's output written straight to the screen would paint over the panel, as the Memory panel was on 2026-10-16;
/// with the panel open the frame is composed again, panel on top.
#[test]
fn p93_the_panel_is_redrawn_when_a_pane_prints() {
    use super::super::surface_patch::ClientPaneSurfacePatchOutcome;
    let (_home, root) = project("p93-patch");
    for open_it in [false, true] {
        let mut state = state_in(&root);
        let _ = rows_of(&mut state, 140);
        if open_it {
            state.toggle_profile_panel();
            assert!(state.nirvana_profile_panel.is_some());
        }
        let _ = rows_of(&mut state, 140);
        let outcome = state.apply_pane_surface_patch(pane_patch(&state));
        let straight = matches!(outcome, ClientPaneSurfacePatchOutcome::Applied(Some(_)));
        assert!(matches!(outcome, ClientPaneSurfacePatchOutcome::Applied(_)));
        assert_eq!(straight, !open_it, "open {open_it}");
    }
}

/// One panel is open at a time: the Profiles panel closes the strip's, and Esc then leaves none.
#[test]
fn p93_opening_the_panel_closes_the_strips_panels() {
    let (_home, root) = project("p93-one");
    let mut state = state_in(&root);
    let mut projected = state.snapshot.as_deref().cloned().unwrap();
    projected.agents = vec![ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: Some("moto".into()),
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
    }];
    state.set_snapshot(Box::new(projected));
    state.toggle_preload_panel();
    assert!(
        state.nirvana_preload.is_some(),
        "the strip's Preload panel is open"
    );
    state.toggle_profile_panel();
    assert!(state.nirvana_preload.is_none(), "it closes");
    assert!(state.nirvana_profile_panel.is_some());
    key(&mut state, KeyCode::Esc);
    assert!(state.nirvana_profile_panel.is_none() && state.nirvana_preload.is_none());
}
