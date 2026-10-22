//! Plan 12 O8a: the Seats square's data, composed from a run herdr recorded on 2026-10-18 (an isolated server,
//! session `o8ain`, a scratch project): `herdr seat run flash --order-id mul --run rec1 --class sweep.mechanical
//! --not-family anthropic --worktree`, its journal copied while the worker ran (`rec1-running.jsonl`) and after
//! it delivered (`rec1-delivered.jsonl`), and `leases.json` after `herdr seat pick --class build.bounded --size
//! small --book fix1 --run rec1` booked a second order that no worker took.

use super::*;
use crate::nirvana_run::Event;

const RUNNING: &str = include_str!("fixtures/rec1-running.jsonl");
const DELIVERED: &str = include_str!("fixtures/rec1-delivered.jsonl");
const LEASES: &str = include_str!("fixtures/leases.json");
/// 34 s after `mul` was seated (1792311966), inside both leases' hour.
const NOW: u64 = 1_792_312_000;

fn table() -> Table {
    nirvana_seat::parse(nirvana_seat::DEFAULT).expect("herdr's default table")
}

fn leases() -> Leases {
    serde_json::from_str(LEASES).expect("recorded leases")
}

fn state(text: &str) -> RunState {
    nirvana_run::replay(&nirvana_run::parse_lines(text).0)
}

fn with(text: &str, kinds: Vec<Kind>) -> RunState {
    let (mut events, _) = nirvana_run::parse_lines(text);
    let last = events.last().map_or(0, |event| event.seq);
    for (seq, kind) in (last + 1..).zip(kinds) {
        events.push(Event {
            v: nirvana_run::V,
            seq,
            at: NOW,
            kind,
        });
    }
    nirvana_run::replay(&events)
}

fn view_of(state: RunState, leases: &Leases, now: u64) -> View {
    let table = table();
    compose(Some(&table), None, &[("rec1".into(), state)], leases, now)
}

fn row<'a>(view: &'a View, order: &str) -> &'a Row {
    view.rows
        .iter()
        .find(|row| row.order == order)
        .unwrap_or_else(|| panic!("no row {order}: {:?}", view.rows))
}

/// A folder of its own for each test.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nirvana-seats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp folder");
    dir
}

/// A project holding the recorded run `rec1` (and, when `ended` is given, a run of that name that ended), and a
/// home with herdr's default `seats.toml` and the recorded leases.
fn project(name: &str, ended: Option<&str>) -> (PathBuf, PathBuf) {
    let root = folder(&format!("{name}-proj"));
    let home = folder(&format!("{name}-home"));
    let runs = root.join(nirvana_run::DIR).join("runs");
    std::fs::create_dir_all(runs.join("rec1")).unwrap();
    std::fs::write(runs.join("rec1").join(nirvana_run::JOURNAL), RUNNING).unwrap();
    if let Some(run) = ended {
        std::fs::create_dir_all(runs.join(run)).unwrap();
        let journal = nirvana_run::Journal::new(runs.join(run).join(nirvana_run::JOURNAL));
        std::fs::write(&journal.path, RUNNING).unwrap();
        journal
            .append(
                NOW,
                Kind::Ended {
                    order: None,
                    outcome: "ended".into(),
                    why: None,
                    exit: None,
                    pools: Vec::new(),
                    pools_basis: None,
                },
            )
            .unwrap();
    }
    std::fs::write(home.join(nirvana_seat::SEATS_FILE), nirvana_seat::DEFAULT).unwrap();
    std::fs::write(home.join(nirvana_seat::LEASES_FILE), LEASES).unwrap();
    (root, home)
}

#[test]
fn a_running_order_shows_its_seat_model_effort_pool_state_time_use_reason_and_pane() {
    let view = view_of(state(RUNNING), &leases(), NOW);
    assert_eq!(view.mode, Some(Mode::Balanced));
    let mul = row(&view, "mul");
    assert_eq!(mul.run, "rec1");
    assert_eq!(mul.seat, "flash");
    assert_eq!(mul.model, "gemini-3.8-flash-low");
    assert_eq!(mul.effort, Effort::Low);
    assert_eq!(mul.pool, "gemini");
    assert_eq!(mul.state, "running");
    assert!(!mul.attention);
    assert_eq!(mul.pane.as_deref(), Some("w1:p2"));
    assert_eq!(
        mul.reason,
        "sweep.mechanical \u{2192} flash low: gemini 5h 1.0% used, week 1% used"
    );
    assert_eq!(time_text(mul, NOW), "34s");
    // Its first usage event, 15194 in and 83 out, at flash's price: what the receipt would compute.
    let use_ = mul.use_.as_ref().expect("a use figure");
    assert_eq!(use_.basis, UseBasis::Spent);
    let seat = table();
    let spent = nirvana_seat::computed_use(
        &seat,
        seat.seat("flash").unwrap(),
        &state(RUNNING).orders[0].tokens().unwrap(),
        1_792_311_966,
    )
    .unwrap();
    let (window, share) = spent
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(window, share)| (window.clone(), *share))
        .unwrap();
    assert_eq!((use_.window.as_str(), use_.share), (window.as_str(), share));
    assert_eq!(use_text(Some(use_)), format!("{window} {}", percent(share)));
}

#[test]
fn a_lease_no_worker_took_is_a_booked_row_with_its_picks_reason_and_expected_use() {
    let view = view_of(state(RUNNING), &leases(), NOW);
    assert_eq!(view.rows.len(), 2, "{:?}", view.rows);
    let fix = row(&view, "fix1");
    assert_eq!(fix.state, "booked");
    assert_eq!(fix.seat, "sonnet");
    assert_eq!(fix.model, "claude-sonnet-5-5");
    assert_eq!(fix.effort, Effort::Medium);
    assert_eq!(fix.pool, "max");
    assert_eq!(fix.mode, Some(Mode::Balanced));
    assert!(fix.pane.is_none());
    assert!(
        fix.reason
            .starts_with("build.bounded \u{2192} sonnet medium: max 5h"),
        "{}",
        fix.reason
    );
    let use_ = fix.use_.as_ref().expect("the booking's expected use");
    assert_eq!(use_.basis, UseBasis::Booked);
    let expected = &leases().leases[1].expected;
    let most = expected.values().copied().fold(0.0f32, f32::max);
    assert_eq!(use_.share, most);
    assert!(use_text(Some(use_)).ends_with(" booked"));
    // `mul` has a `seated`, so its own lease is not a second row.
    assert_eq!(view.rows.iter().filter(|row| row.order == "mul").count(), 1);
}

#[test]
fn a_lease_of_another_run_or_past_its_hour_is_left_out() {
    let mut other = leases();
    other.leases[1].run = Some("another".into());
    assert!(view_of(state(RUNNING), &other, NOW)
        .rows
        .iter()
        .all(|row| row.order != "fix1"));
    let late = leases().leases[1].expires_at;
    assert!(view_of(state(RUNNING), &leases(), late)
        .rows
        .iter()
        .all(|row| row.order != "fix1"));
}

#[test]
fn a_delivered_order_stays_open_until_the_gate_rules_and_an_ended_run_shows_nothing() {
    let delivered = view_of(state(DELIVERED), &leases(), NOW);
    assert_eq!(row(&delivered, "mul").state, "delivered");
    let gated = with(
        DELIVERED,
        vec![Kind::Gated {
            order: "mul".into(),
            verdict: "pass".into(),
            reason: None,
            axes: Vec::new(),
        }],
    );
    let gated = view_of(gated, &leases(), NOW);
    assert!(gated.rows.iter().all(|row| row.order != "mul"));
    assert_eq!(gated.runs.len(), 1, "the run is still open");
    let ended = with(
        DELIVERED,
        vec![Kind::Ended {
            order: None,
            outcome: "ended".into(),
            why: None,
            exit: None,
            pools: Vec::new(),
            pools_basis: None,
        }],
    );
    let ended = view_of(ended, &leases(), NOW);
    assert!(ended.rows.is_empty() && ended.runs.is_empty(), "{ended:?}");
}

#[test]
fn an_asked_order_waits_on_someone_and_a_deferral_shows_its_time() {
    let asked = with(
        RUNNING,
        vec![Kind::Asked {
            order: Some("mul".into()),
            question: "which file?".into(),
        }],
    );
    let view = view_of(asked, &leases(), NOW);
    assert!(row(&view, "mul").attention);
    assert_eq!(view.attention(), 1);
    let deferred = with(
        RUNNING,
        vec![Kind::Deferred {
            order: "mul".into(),
            until: 1_792_319_400,
            why: "gemini 5h spent".into(),
        }],
    );
    let view = view_of(deferred, &leases(), NOW);
    let mul = row(&view, "mul");
    assert_eq!(mul.state, "deferred");
    assert_eq!(time_text(mul, NOW), "until 10:30Z");
}

#[test]
fn the_run_line_counts_the_runs_classes_and_overrides() {
    let view = view_of(state(DELIVERED), &leases(), NOW);
    let run = &view.runs[0];
    assert_eq!(run.run, "rec1");
    assert_eq!(run.orders, 1);
    assert_eq!(
        run.classes,
        BTreeMap::from([("sweep.mechanical".to_owned(), 1)])
    );
    assert_eq!(run.overrides, 0);
    assert_eq!(run.mode_changed, None);
}

#[test]
fn with_mode_changes_the_top_level_mode_line_and_nothing_else() {
    let sota = nirvana_seat::with_mode(nirvana_seat::DEFAULT, Mode::Sota).unwrap();
    assert_eq!(nirvana_seat::parse(&sota).unwrap().mode, Mode::Sota);
    let changed: Vec<(&str, &str)> = nirvana_seat::DEFAULT
        .lines()
        .zip(sota.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed, vec![("mode = \"balanced\"", "mode = \"sota\"")]);
    assert_eq!(sota.len() + 4, nirvana_seat::DEFAULT.len());
    // A comment naming the mode stays a comment; CRLF stays CRLF.
    let text = nirvana_seat::DEFAULT
        .replacen("version = 1\n", "version = 1\n# mode = \"efficient\"\n", 1)
        .replace('\n', "\r\n");
    let efficient = nirvana_seat::with_mode(&text, Mode::Efficient).unwrap();
    assert!(efficient.contains("# mode = \"efficient\"\r\n"));
    assert!(efficient.contains("\r\nmode = \"efficient\"\r\n"));
    assert_eq!(
        efficient.matches("\r\n").count(),
        text.matches("\r\n").count()
    );
    assert_eq!(
        nirvana_seat::parse(&efficient).unwrap().mode,
        Mode::Efficient
    );
    assert!(nirvana_seat::with_mode("version = 1\n", Mode::Sota).is_err());
}

#[test]
fn seat_pick_takes_the_mode_seats_toml_holds_when_no_mode_is_given() {
    for mode in [Mode::Efficient, Mode::Balanced, Mode::Sota] {
        let table =
            nirvana_seat::parse(&nirvana_seat::with_mode(nirvana_seat::DEFAULT, mode).unwrap())
                .unwrap();
        assert_eq!(nirvana_seat::default_request(&table).mode, mode);
    }
}

#[test]
fn the_mode_key_writes_seats_toml_and_logs_mode_changed_in_each_open_run() {
    let (root, home) = project("mode", Some("rec0"));
    set_home_for_test(Some(home.clone()), Some(NOW));
    let ended_before =
        std::fs::read_to_string(root.join(".nirvana/runs/rec0").join(nirvana_run::JOURNAL))
            .unwrap();
    let message = change_mode(&root, Mode::Sota, "seats square").unwrap();
    assert_eq!(
        message,
        "mode SOTA: the next seat pick uses it; logged in run rec1"
    );
    let project_file = nirvana_seat::project_seats_path(&root);
    let project_table = std::fs::read_to_string(&project_file).unwrap();
    assert_eq!(
        nirvana_seat::parse_project_seats(&project_file, &project_table).unwrap(),
        Mode::Sota
    );
    let table = std::fs::read_to_string(home.join(nirvana_seat::SEATS_FILE)).unwrap();
    assert_eq!(nirvana_seat::parse(&table).unwrap().mode, Mode::Balanced);
    let journal = Project::at(&root).journal("rec1");
    let (events, bad) = journal.read();
    assert_eq!(bad, 0);
    assert_eq!(
        events.last().map(|event| event.kind.clone()),
        Some(Kind::ModeChanged {
            from: Mode::Balanced,
            to: Mode::Sota,
            by: "seats square".into(),
        })
    );
    assert_eq!(
        nirvana_run::replay(&events).modes,
        vec![(NOW, Mode::Balanced, Mode::Sota)]
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".nirvana/runs/rec0").join(nirvana_run::JOURNAL))
            .unwrap(),
        ended_before,
        "an ended run is not written"
    );
    let view = load(&root, &home);
    assert_eq!(view.mode, Some(Mode::Sota));
    assert_eq!(view.mode_source, Some(nirvana_seat::ModeSource::Project));
    assert_eq!(view.runs[0].mode_changed, Some((NOW, Mode::Sota)));
    // The same mode again writes nothing.
    let lines = events.len();
    assert_eq!(
        change_mode(&root, Mode::Sota, "seats square").unwrap(),
        "mode SOTA already"
    );
    assert_eq!(journal.read().0.len(), lines);
    set_home_for_test(None, None);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn seat_mode_writes_only_project_file() {
    let (root, home) = project("cli-project", None);
    set_home_for_test(Some(home.clone()), Some(NOW));
    let home_file = home.join(nirvana_seat::SEATS_FILE);
    let home_before = std::fs::read(&home_file).unwrap();

    let out = mode_cli_for(&root, &["sota".to_string()]).unwrap();
    assert!(out.contains("mode SOTA"));

    let project_file = nirvana_seat::project_seats_path(&root);
    assert!(project_file.exists());
    let project_content = std::fs::read_to_string(&project_file).unwrap();
    assert_eq!(
        nirvana_seat::parse_project_seats(&project_file, &project_content).unwrap(),
        Mode::Sota
    );

    let home_after = std::fs::read(&home_file).unwrap();
    assert_eq!(home_before, home_after, "global line bytes unchanged");

    let query = mode_cli_for(&root, &[]).unwrap();
    assert_eq!(query, "sota (this project)");

    set_home_for_test(None, None);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn seat_mode_machine_writes_only_global_line() {
    let (root, home) = project("cli-machine", None);
    set_home_for_test(Some(home.clone()), Some(NOW));
    nirvana_seat::set_project_mode(&root, Mode::Sota).unwrap();
    let project_file = nirvana_seat::project_seats_path(&root);
    let project_before = std::fs::read(&project_file).unwrap();

    let out = mode_cli_for(&root, &["--machine".to_string(), "efficient".to_string()]).unwrap();
    assert!(out.contains("mode Efficient"));
    // This project keeps its own mode, so its open run logs no change.
    let journal = nirvana_run::Journal::new(
        root.join(nirvana_run::DIR)
            .join("runs")
            .join("rec1")
            .join(nirvana_run::JOURNAL),
    );
    assert!(
        !journal
            .read()
            .0
            .iter()
            .any(|event| matches!(event.kind, Kind::ModeChanged { .. })),
        "a project with its own mode logged a machine change"
    );

    let home_file = home.join(nirvana_seat::SEATS_FILE);
    let home_table = nirvana_seat::parse(&std::fs::read_to_string(&home_file).unwrap()).unwrap();
    assert_eq!(home_table.mode, Mode::Efficient);

    let project_after = std::fs::read(&project_file).unwrap();
    assert_eq!(
        project_before, project_after,
        "project file bytes unchanged"
    );

    let query = mode_cli_for(&root, &[]).unwrap();
    assert_eq!(query, "sota (this project)");

    set_home_for_test(None, None);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn seat_mode_clear_removes_project_file() {
    let (root, home) = project("cli-clear", None);
    set_home_for_test(Some(home.clone()), Some(NOW));
    nirvana_seat::set_project_mode(&root, Mode::Sota).unwrap();
    let project_file = nirvana_seat::project_seats_path(&root);
    assert!(project_file.exists());

    let out = mode_cli_for(&root, &["--clear".to_string()]).unwrap();
    assert!(out.contains("cleared project mode: now following machine (balanced)"));
    assert!(!project_file.exists(), "project file was removed");

    let query = mode_cli_for(&root, &[]).unwrap();
    assert_eq!(query, "balanced (machine)");

    set_home_for_test(None, None);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_refused_seats_toml_is_never_overwritten_by_the_mode_key() {
    let (root, home) = project("refused", None);
    set_home_for_test(Some(home.clone()), Some(NOW));
    let path = home.join(nirvana_seat::SEATS_FILE);
    std::fs::write(&path, "version = 1\nmode = \"balanced\"\n").unwrap();
    assert!(change_mode(&root, Mode::Sota, "seats square").is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "version = 1\nmode = \"balanced\"\n"
    );
    assert_eq!(
        Project::at(&root).journal("rec1").read().0.len(),
        4,
        "nothing logged"
    );
    let view = load(&root, &home);
    assert_eq!(view.mode, None);
    assert!(view.note.is_some());
    set_home_for_test(None, None);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_journal_whose_length_and_time_did_not_change_is_not_replayed_again() {
    let (root, home) = project("replay", None);
    let mut replays = Replays::new();
    let first = read(&root, &home, &mut replays, NOW);
    assert_eq!(row(&first, "mul").state, "running");
    // Stand a marker in for the kept replay: an unchanged journal hands it back.
    let path = Project::at(&root).journal("rec1").path;
    let marked = with(
        RUNNING,
        vec![Kind::Asked {
            order: Some("mul".into()),
            question: "marker".into(),
        }],
    );
    replays.get_mut(&path).unwrap().1 = marked;
    assert_eq!(
        row(&read(&root, &home, &mut replays, NOW), "mul").state,
        "asked"
    );
    // One appended event changes the journal's length: it is replayed.
    Project::at(&root)
        .journal("rec1")
        .append(
            NOW,
            Kind::Interrupted {
                order: "mul".into(),
                why: "stopped".into(),
            },
        )
        .unwrap();
    assert_eq!(
        row(&read(&root, &home, &mut replays, NOW), "mul").state,
        "interrupted"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn percent_keeps_two_figures_for_small_shares() {
    assert_eq!(percent(0.000_217), "0.02%");
    assert_eq!(percent(0.015), "1.5%");
    assert_eq!(percent(0.38), "38%");
}
