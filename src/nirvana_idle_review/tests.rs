//! Order LEARN's tests: the class fix, the offer, idle detection, the spare gate, the caps, never while busy, the
//! switch, and one idle review queued, delivered and gated end to end. All on stand-ins: a test home, a hand-made
//! world, journals written here; no worker starts (`nirvana_review::launch` is a stub under test).

use super::*;
use crate::nirvana_lessons::{self as lessons, Meta, Origin};
use crate::nirvana_run::Seated;
use crate::nirvana_seat::{Effort, Lease, Mode, SizeName};
use crate::nirvana_usage::Reading;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "herdr-learn-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    Project::at(&dir).ensure().expect(".nirvana");
    dir
}

fn quiet(name: &str) -> crate::nirvana::TestHomeGuard {
    let guard = crate::nirvana::with_test_home(name);
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    std::env::set_var("NIRVANA_NO_PANE", "1");
    std::env::remove_var("HERDR_PANE_ID");
    std::env::remove_var(crate::api::SOCKET_PATH_ENV_VAR);
    guard
}

fn seated(id: &str, class: &str, order_file: &str) -> Seated {
    Seated {
        order: id.into(),
        seat: "sonnet".into(),
        cli: "claude".into(),
        model: "claude-sonnet-5-5".into(),
        effort: Effort::Medium,
        pool: "max".into(),
        class: class.into(),
        chain: Some(class.into()),
        size: SizeName::Small,
        mode: Mode::Balanced,
        needs: Vec::new(),
        not_family: Vec::new(),
        reason: "r".into(),
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

/// A claude stream: one command whose output is `output`.
fn stream(output: &str) -> String {
    [
        serde_json::json!({"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","name":"Bash","input":{"command":"python slice.py"}}]}}),
        serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","content":output}]}}),
        serde_json::json!({"type":"result","subtype":"success"}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n")
}

fn report() -> Value {
    serde_json::json!({"status":"done","summary":"sliced it","files":[],"tests":[],"gotchas":["the nozzle came from the inventory"]})
}

/// Order `id` of `class` in `run`, seated, delivered with `report`, its stream holding `output`, and gated `verdict` at
/// `at`.
fn gated(root: &Path, run: &str, id: &str, class: &str, output: &str, verdict: &str, at: u64) {
    let project = Project::at(root);
    let journal = project.journal(run);
    if !journal.path.is_file() {
        journal
            .append(
                at - 30,
                Kind::Opened {
                    project: root.display().to_string(),
                    orchestrator_pane: None,
                    auto: false,
                    pools: Vec::new(),
                    pools_basis: None,
                },
            )
            .expect("opened");
    }
    let order_file = project.orders_dir().join(format!("{id}.md"));
    std::fs::create_dir_all(project.orders_dir()).expect("orders");
    std::fs::write(&order_file, format!("# {id}\n\nSlice the part.\n")).expect("order");
    journal
        .append(
            at - 20,
            Kind::Seated(Box::new(seated(
                id,
                class,
                &order_file.display().to_string(),
            ))),
        )
        .expect("seated");
    let streams = project.run_dir(run).join("streams");
    std::fs::create_dir_all(&streams).expect("streams");
    std::fs::write(streams.join(format!("{id}.jsonl")), stream(output)).expect("stream");
    journal
        .append(
            at - 10,
            Kind::Delivered {
                order: id.into(),
                report: report(),
            },
        )
        .expect("delivered");
    journal
        .append(
            at,
            Kind::Gated {
                order: id.into(),
                verdict: verdict.into(),
                reason: None,
                axes: Vec::new(),
            },
        )
        .expect("gated");
}

fn reading(pool: &str, window: &str, used: f32, resets_in: u64, now: u64, age: u64) -> Reading {
    Reading {
        pool: pool.into(),
        window: window.into(),
        used: Some(used),
        resets_at: Some(now + resets_in),
        resets_text: None,
        read_at: now - age,
        source: "test".into(),
    }
}

/// A world an hour after the files were written (so their writes are an hour old), every pool read fresh: gemini's 5h
/// window 20% used with an hour to its reset (75% projected unused), max's the same.
fn world(resets_in: u64, age: u64) -> World {
    let now = nirvana_run::now() + 3_600;
    World {
        now,
        readings: ["gemini", "max", "google3p"]
            .iter()
            .flat_map(|pool| {
                [
                    reading(pool, "5h", 0.2, resets_in, now, age),
                    reading(pool, "week", 0.1, 5 * 86_400, now, age),
                ]
            })
            .collect(),
        ..World::default()
    }
}

fn table() -> Table {
    nirvana_seat::parse(nirvana_seat::DEFAULT).expect("default table")
}

fn facts(now: u64) -> Facts {
    Facts {
        now,
        on: true,
        doubt: None,
        busy: None,
        owner_key: Ok(Some(now - IDLE_AFTER_S - 60)),
        activity: Some((now - IDLE_AFTER_S - 60, "p".into())),
        usage_moved: Ok(None),
        backoff_until: None,
        new_orders: Some(3),
        review_cap: None,
        used_today: 0,
        used_window: 0,
        spare: Ok(Spare {
            seat: "flash".into(),
            pool: "gemini".into(),
            spare: Some(0.75),
            resets_at: Some(now + 3_600),
            week: vec![("week".into(), Some(0.65))],
        }),
    }
}

/// Review round S5: a 5 h reading of every pool `IDLE_AFTER_S` and a minute before `world.now`, the same as now, so
/// the history says no one used them.
fn seed_samples(world: &World) {
    change_global(|global| {
        for reading in world.readings.iter().filter(|r| r.window == "5h") {
            global.samples.insert(
                reading.pool.clone(),
                vec![Sample {
                    read_at: world.now - IDLE_AFTER_S - 60,
                    used: reading.used.unwrap_or(0.0),
                    resets_at: reading.resets_at,
                }],
            );
        }
    })
    .expect("samples");
}

/// The writes log line herdr's `noted` makes when it writes a lesson (review round S6 reads it).
fn logged(root: &Path, slug: &str) {
    use std::io::Write;
    let path = lessons::dir(root).join(format!("{slug}.md"));
    let text = std::fs::read_to_string(&path).expect("lesson");
    let log = root
        .join(nirvana_run::DIR)
        .join("runs")
        .join(".lesson-writes.jsonl");
    std::fs::create_dir_all(log.parent().unwrap()).expect("runs");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .expect("log");
    writeln!(
        file,
        "{}",
        serde_json::json!({"path": format!("{slug}.md"), "sha": lessons::sha256(&text)})
    )
    .expect("line");
}

/// A project with two gated orders an hour or two before `world.now`, ready for a tick to review.
fn ready_project(name: &str, world: &World) -> PathBuf {
    let root = temp(name);
    let at = world.now - 2 * 3_600;
    gated(
        &root,
        "r1",
        "b1",
        "build.bounded",
        "nozzle diameter read from the inventory list",
        "pass",
        at,
    );
    gated(
        &root,
        "r1",
        "b2",
        "build.bounded",
        "support volume fell from 40 to 12 cm3 at 45 degrees",
        "pass",
        at + 60,
    );
    root
}

fn skipped(decision: Decision, words: &str) {
    match decision {
        Decision::Skip(why) => assert!(why.contains(words), "{why:?} should say {words:?}"),
        Decision::Run => panic!("a review ran where it should say {words:?}"),
    }
}

// ---- the class fix and the offer ----

#[test]
fn learn_a_worker_lesson_is_kept_under_its_orders_class() {
    let _home = quiet("learn-routed");
    let root = temp("routed");
    gated(
        &root,
        "r1",
        "b1",
        "build.bounded",
        "TOTAL nozzle 0.6 from inventory",
        "pass",
        1_000,
    );
    let lesson = serde_json::json!({"name":"Nozzles come from the inventory","class":"slicer-estimation",
        "steps":"1. read the inventory","evidence":"TOTAL nozzle 0.6 from inventory"});
    let verdict = lessons::verify_and_write(
        &root,
        &lesson,
        true,
        &stream("TOTAL nozzle 0.6 from inventory"),
        "b1",
        "r1",
        1_001,
        Meta {
            profile: None,
            origin: Origin::Worker,
        },
    );
    assert!(
        matches!(verdict, lessons::Verdict::Written(_)),
        "{verdict:?}"
    );
    let index = lessons::read_index(&root);
    let entry = &index.lessons["nozzles-come-from-the-inventory"];
    assert_eq!(
        entry.class, "build.bounded",
        "the router's class, not the worker's words"
    );
    let body = lessons::body(&root, "nozzles-come-from-the-inventory").expect("body");
    assert!(
        body.contains("\nclass: build.bounded\ntopic: slicer-estimation\n"),
        "{body}"
    );
    // The next build.bounded order is offered it; before the fix no order could be.
    let offer = lessons::offer(&root, &root, "build.bounded");
    assert!(
        offer.contains("nozzles-come-from-the-inventory.md"),
        "{offer}"
    );
    assert!(lessons::offer(&root, &root, "slicer-estimation").is_empty());
}

#[test]
fn learn_reclass_moves_old_lessons_through_the_ledger_and_rolls_back() {
    let _home = quiet("learn-reclass");
    let root = temp("reclass");
    gated(
        &root,
        "r1",
        "b1",
        "build.bounded",
        "flush matrix 4x4 recovered",
        "pass",
        1_000,
    );
    gated(
        &root,
        "r2",
        "w1",
        "research.web",
        "ablation depth 0.2 mm",
        "pass",
        2_000,
    );
    // Lessons written before the fix: the worker's own words for the class.
    for (run, order, name, class, quote) in [
        (
            "r1",
            "b1",
            "Recover the flush matrix",
            "slicer-estimation",
            "flush matrix 4x4 recovered",
        ),
        (
            "r2",
            "w1",
            "Laser depth by ablation",
            "research.materials",
            "ablation depth 0.2 mm",
        ),
    ] {
        let lesson =
            serde_json::json!({"name":name,"class":class,"steps":"1. do it","evidence":quote});
        let checked = lessons::check(&lesson, true, &stream(quote)).expect("checked");
        // As the gate wrote them before: straight under the worker's class (a journal-less write).
        let other = temp("reclass-old");
        let _ = lessons::verify_and_write(
            &other,
            &lesson,
            true,
            &stream(quote),
            order,
            run,
            10,
            Meta::default(),
        );
        let slug = lessons::slug(&checked.name);
        let dir = lessons::dir(&root);
        std::fs::create_dir_all(&dir).expect("lessons");
        std::fs::copy(
            lessons::dir(&other).join(format!("{slug}.md")),
            dir.join(format!("{slug}.md")),
        )
        .expect("copy");
        logged(&root, &slug);
        let mut index = lessons::read_index(&root);
        let mut entry = lessons::read_index(&other).lessons[&slug].clone();
        assert_eq!(
            entry.class, class,
            "a journal-less write keeps the worker's words"
        );
        entry.counters.offered = 0;
        index.lessons.insert(slug, entry);
        std::fs::write(
            dir.join(lessons::INDEX),
            serde_json::to_string_pretty(&index).unwrap(),
        )
        .expect("index");
    }
    let classes = lessons::router_classes();
    let before = lessons::body(&root, "recover-the-flush-matrix").expect("body");
    let dry = lessons::reclass(&root, &classes, true, false, 3_000).expect("dry");
    assert_eq!(dry.moved.len(), 2, "{dry:?}");
    assert_eq!(
        lessons::body(&root, "recover-the-flush-matrix").as_deref(),
        Some(before.as_str())
    );
    let done = lessons::reclass(&root, &classes, false, false, 3_000).expect("reclass");
    let index = lessons::read_index(&root);
    assert_eq!(
        index.lessons["recover-the-flush-matrix"].class,
        "build.bounded"
    );
    assert_eq!(
        index.lessons["laser-depth-by-ablation"].class,
        "research.web"
    );
    let body = lessons::body(&root, "recover-the-flush-matrix").expect("body");
    assert!(
        body.contains("\nclass: build.bounded\ntopic: slicer-estimation\n"),
        "{body}"
    );
    assert!(lessons::offer(&root, &root, "research.web").contains("laser-depth-by-ablation.md"));
    // One op for both, and its rollback puts every byte back.
    let id = done.op.expect("a ledger op");
    let row = lessons::ledger(&root)
        .into_iter()
        .find(|row| row.id == id)
        .expect("row");
    assert_eq!((row.op.as_str(), row.before.len()), ("reclass", 2));
    // Review round N6: offers and opens earned after the reclass survive its rollback.
    let mut index = lessons::read_index(&root);
    index
        .lessons
        .get_mut("recover-the-flush-matrix")
        .unwrap()
        .counters
        .offered = 3;
    std::fs::write(
        lessons::dir(&root).join(lessons::INDEX),
        serde_json::to_string_pretty(&index).unwrap(),
    )
    .expect("index");
    lessons::rollback(&root, &id, 4_000).expect("rollback");
    assert_eq!(
        lessons::read_index(&root).lessons["recover-the-flush-matrix"]
            .counters
            .offered,
        3,
        "the counters since the reclass are kept"
    );
    assert_eq!(
        lessons::body(&root, "recover-the-flush-matrix").as_deref(),
        Some(before.as_str())
    );
    assert_eq!(
        lessons::read_index(&root).lessons["recover-the-flush-matrix"].class,
        "slicer-estimation"
    );
    // A second reclass after nothing changed moves nothing new.
    let again = lessons::reclass(&root, &classes, false, false, 5_000).expect("again");
    assert_eq!(again.moved.len(), 2);
    let third = lessons::reclass(&root, &classes, false, false, 6_000).expect("third");
    assert!(third.moved.is_empty() && third.op.is_none(), "{third:?}");
}

#[test]
fn learn_retire_takes_only_a_lesson_that_never_helped() {
    let _home = quiet("learn-retire");
    let root = temp("retire");
    gated(
        &root,
        "r1",
        "b1",
        "build.bounded",
        "stepper contained in scope",
        "pass",
        1_000,
    );
    for name in ["Contain the stepper", "Scope the animation"] {
        let lesson = serde_json::json!({"name":name,"class":"build.bounded","steps":"1. x","evidence":"stepper contained in scope"});
        let _ = lessons::verify_and_write(
            &root,
            &lesson,
            true,
            &stream("stepper contained in scope"),
            "b1",
            "r1",
            1_001,
            Meta::default(),
        );
    }
    let mut index = lessons::read_index(&root);
    index
        .lessons
        .get_mut("contain-the-stepper")
        .unwrap()
        .counters = lessons::Counters {
        offered: 6,
        opened: 2,
        passed_after_opened: 0,
    };
    index
        .lessons
        .get_mut("scope-the-animation")
        .unwrap()
        .counters = lessons::Counters {
        offered: 6,
        opened: 2,
        passed_after_opened: 1,
    };
    std::fs::write(
        lessons::dir(&root).join(lessons::INDEX),
        serde_json::to_string_pretty(&index).unwrap(),
    )
    .expect("index");
    let refused = lessons::retire(&root, "scope-the-animation", "o", "r", 2_000);
    assert!(
        refused.is_err(),
        "a lesson that helped once is kept: {refused:?}"
    );
    let id = lessons::retire(&root, "contain-the-stepper", "o", "r", 2_000).expect("retired");
    assert!(!lessons::read_index(&root)
        .lessons
        .contains_key("contain-the-stepper"));
    assert!(lessons::dir(&root)
        .join(lessons::ARCHIVE)
        .join(format!("{id}-contain-the-stepper.md"))
        .is_file());
    lessons::rollback(&root, &id, 3_000).expect("rollback");
    assert!(lessons::read_index(&root)
        .lessons
        .contains_key("contain-the-stepper"));
}

// ---- the rule ----

#[test]
fn learn_idle_needs_a_quiet_owner_and_quiet_orders() {
    let now = 10_000_000;
    assert_eq!(decide(&facts(now)), Decision::Run);
    let mut typed = facts(now);
    typed.owner_key = Ok(Some(now - 60));
    skipped(decide(&typed), "the owner typed");
    let mut doubt = facts(now);
    doubt.owner_key = Err("keys/wP_p1 holds \"x\"".into());
    skipped(decide(&doubt), "cannot be read");
    let mut never = facts(now);
    never.owner_key = Ok(None);
    assert_eq!(
        decide(&never),
        Decision::Run,
        "no key ever recorded is quiet"
    );
    let mut writing = facts(now);
    writing.activity = Some((now - 120, "C:/p".into()));
    skipped(decide(&writing), "an order wrote");
    let mut nothing = facts(now);
    nothing.new_orders = Some(0);
    skipped(decide(&nothing), "no order gated");
}

#[test]
fn learn_a_missing_or_old_spare_reading_means_no_review() {
    let now = 10_000_000;
    let mut missing = facts(now);
    missing.spare = Ok(Spare {
        seat: "flash".into(),
        pool: "gemini".into(),
        spare: None,
        resets_at: Some(now + 3_600),
        week: vec![("week".into(), Some(0.65))],
    });
    skipped(decide(&missing), "missing or old");
    let mut no_seat = facts(now);
    no_seat.spare = Err("no seat: every seat skipped".into());
    skipped(decide(&no_seat), "no seat");
    // The seat rule's own reading: an old reading gives no spare, so no review.
    let _home = quiet("learn-spare-old");
    let old = world(3_600, crate::nirvana_usage::STALE_AFTER + 60);
    let (_, spare) = spare(&table(), &old, "x-review").expect("a pick");
    assert_eq!(spare.spare, None, "{spare:?}");
    let fresh = world(3_600, 10);
    let (pick, spare) = super::spare(&table(), &fresh, "x-review").expect("a pick");
    assert!(spare.spare.is_some_and(|s| s >= SPARE_MIN), "{pick:?}");
}

#[test]
fn learn_the_spare_must_be_half_unused_and_reset_soon() {
    let now = 10_000_000;
    let mut thin = facts(now);
    if let Ok(spare) = thin.spare.as_mut() {
        spare.spare = Some(0.4);
    }
    skipped(decide(&thin), "under 50%");
    let mut late = facts(now);
    if let Ok(spare) = late.spare.as_mut() {
        spare.resets_at = Some(now + 3 * 3_600);
    }
    skipped(decide(&late), "not lost yet");
    let mut unknown = facts(now);
    if let Ok(spare) = unknown.spare.as_mut() {
        spare.resets_at = None;
    }
    skipped(decide(&unknown), "reset is not known");
}

#[test]
fn learn_caps_per_space_per_day_and_per_window() {
    let now = 10_000_000;
    let mut day = facts(now);
    day.used_today = PER_SPACE_PER_DAY;
    skipped(decide(&day), "this UTC day");
    let mut window = facts(now);
    window.used_window = PER_WINDOW;
    skipped(decide(&window), "machine's cap");
    let mut lesson_cap = facts(now);
    lesson_cap.review_cap = Some("20 lesson reviews today reach the daily cap".into());
    skipped(decide(&lesson_cap), "daily cap");
    // The window count is the same window's only: another reset is a new window.
    let global = Global {
        windows: [("gemini".to_owned(), (now + 3_600, 3))].into(),
        ..Global::default()
    };
    assert_eq!(used_window(&global, "gemini", Some(now + 3_600 + 300)), 3);
    assert_eq!(
        used_window(&global, "gemini", Some(now + 3_600 + 5 * 3_600)),
        0
    );
    assert_eq!(used_window(&global, "max", Some(now + 3_600)), 0);
    let state = SpaceState {
        day: now / 86_400,
        count: 2,
        ..SpaceState::default()
    };
    assert_eq!(used_today(&state, now), 2);
    assert_eq!(used_today(&state, now + 86_400), 0);
}

#[test]
fn learn_never_while_an_order_runs_and_the_switch_turns_it_off() {
    let now = 10_000_000;
    let mut running = facts(now);
    running.busy = Some("b9 on opus".into());
    skipped(decide(&running), "an order runs");
    let mut off = facts(now);
    off.on = false;
    skipped(decide(&off), "off");
    let world = World {
        now,
        leases: vec![Lease {
            order: "b9".into(),
            seat: "opus".into(),
            effort: Effort::High,
            pool: "max".into(),
            class: "build.bounded".into(),
            run: Some("r".into()),
            expected: BTreeMap::new(),
            booked_at: now - 100,
            expires_at: now + 100,
            pick: Value::Null,
        }],
        ..World::default()
    };
    assert_eq!(busy(&world).as_deref(), Some("b9 on opus"));
    let mut ended = world.clone();
    ended.leases[0].expires_at = now - 1;
    assert_eq!(busy(&ended), None);
    let _home = quiet("learn-switch");
    assert!(!read_global().off, "on by default");
    set_on(false).expect("off");
    assert!(read_global().off);
    set_on(true).expect("on");
    assert!(!read_global().off);
}

// ---- end to end ----

#[test]
fn learn_a_tick_queues_one_idle_review_and_its_gate_applies_the_ops() {
    let _home = quiet("learn-tick");
    let root = temp("tick");
    let project = Project::at(&root);
    let world = world(3_600, 10);
    let at = world.now - 2 * 3_600;
    gated(
        &root,
        "r1",
        "b1",
        "build.bounded",
        "nozzle diameter read from the inventory list",
        "pass",
        at,
    );
    gated(
        &root,
        "r1",
        "b2",
        "build.bounded",
        "support volume fell from 40 to 12 cm3 at 45 degrees",
        "pass",
        at + 60,
    );
    // Review round N8: another run's b1, so a bare `b1` names two orders.
    gated(
        &root,
        "r2",
        "b1",
        "build.bounded",
        "the bed mesh was probed at 7x7",
        "pass",
        at + 120,
    );
    // A lesson that never helped, for the retire op.
    let lesson = serde_json::json!({"name":"Old slicer habit","class":"build.bounded","steps":"1. x",
        "evidence":"nozzle diameter read from the inventory list"});
    let _ = lessons::verify_and_write(
        &root,
        &lesson,
        true,
        &stream("nozzle diameter read from the inventory list"),
        "b1",
        "r1",
        at,
        Meta::default(),
    );
    let mut index = lessons::read_index(&root);
    index
        .lessons
        .get_mut("old-slicer-habit")
        .unwrap()
        .counters
        .offered = 7;
    std::fs::write(
        lessons::dir(&root).join(lessons::INDEX),
        serde_json::to_string_pretty(&index).unwrap(),
    )
    .expect("index");
    let projects = vec![root.clone()];
    seed_samples(&world);
    let lines = tick_with(&projects, &table(), &world, None);
    assert!(
        lines.iter().any(|line| line.contains("queued:")),
        "{lines:?}"
    );
    let last = read_space(&root).last.expect("a last review");
    assert_eq!(last.orders, 3);
    assert!(last.run.starts_with("idle-"), "{last:?}");
    let state = replay(&project.journal(&last.run).read().0);
    let review = state.order(&last.order).expect("the review is seated");
    assert_eq!(review.seated.class, review::CLASS);
    assert_eq!(review.seated.mode, Mode::Efficient);
    let brief = std::fs::read_to_string(&review.seated.order_file).expect("brief");
    assert!(
        brief.contains("r1/b2 · class build.bounded") && brief.contains("## Never helped"),
        "{brief}"
    );
    assert!(brief.contains("- old-slicer-habit"), "{brief}");
    let digest = review::review_cwd(&project, &last.run, &last.order)
        .join("digests")
        .join("r1--b2.txt");
    assert!(std::fs::read_to_string(digest)
        .expect("digest")
        .contains("support volume fell"));
    // Its own lease makes the machine busy: the next tick, reading `leases.json` as `tick` does, queues nothing.
    let mut leased = world.clone();
    leased.leases = nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE))
        .expect("leases")
        .leases;
    // The lease runs for its expected length from when it was booked, which was now (the stand-in world is an hour on).
    leased.now = nirvana_run::now();
    assert!(leased.leases.iter().any(|lease| lease.order == last.order));
    let later = tick_with(&projects, &table(), &leased, None);
    assert!(
        later.iter().all(|line| !line.contains("queued:")),
        "{later:?}"
    );
    assert!(
        later.iter().any(|line| line.contains("an order runs")),
        "{later:?}"
    );
    // The reviewer delivers: a new lesson from b2's stream, a retire, and an op whose evidence is nowhere.
    let mut delivered = report();
    delivered["gotchas"] = serde_json::json!([]);
    delivered["lesson_ops"] = serde_json::json!([
        {"op":"new","from":"r1/b2","name":"Rotate boxy prints 45 degrees","class":"anything","steps":"1. rotate",
         "evidence":"support volume fell from 40 to 12 cm3 at 45 degrees"},
        {"op":"retire","slug":"old-slicer-habit"},
        {"op":"new","name":"Invented","class":"build.bounded","steps":"1. x","evidence":"this quote is in no stream at all"},
        {"op":"new","from":"b1","name":"Probe the bed","class":"build.bounded","steps":"1. probe",
         "evidence":"the bed mesh was probed at 7x7"}
    ]);
    project
        .journal(&last.run)
        .append(
            world.now,
            Kind::Delivered {
                order: last.order.clone(),
                report: delivered,
            },
        )
        .expect("delivered");
    let ruling = nirvana_gate::gate(&project, &last.run, &last.order).expect("gate");
    assert_eq!(ruling.verdict, "pass", "{ruling:?}");
    let index = lessons::read_index(&root);
    let kept = &index.lessons["rotate-boxy-prints-45-degrees"];
    assert_eq!(
        (kept.class.as_str(), kept.origin),
        ("build.bounded", Origin::Review)
    );
    assert!(!index.lessons.contains_key("old-slicer-habit"), "retired");
    assert!(!index.lessons.contains_key("invented"));
    assert!(
        !index.lessons.contains_key("probe-the-bed"),
        "a bare b1 names two orders and is refused"
    );
    // The Learning square: when it ran and what it did; and its run ended.
    let view = crate::nirvana_learning::load(&root, world.now);
    let result = view
        .idle
        .last
        .as_ref()
        .and_then(|l| l.result.clone())
        .expect("a result");
    assert!(
        result.starts_with("pass · 2 applied") && result.contains("2 refused"),
        "{result}"
    );
    let shown = super::lines(&view.idle, world.now).join("\n");
    assert!(shown.contains("did: pass · 2 applied"), "{shown}");
    assert!(
        replay(&project.journal(&last.run).read().0).ended.is_some(),
        "the idle run ends"
    );
}

#[test]
fn learn_the_idle_facts_come_from_keys_journals_and_streams() {
    let _home = quiet("learn-facts");
    let root = temp("facts");
    assert_eq!(activity(&root), Ok(None), "no run, no activity");
    gated(&root, "r1", "b1", "build.bounded", "x", "pass", 1_000);
    let journal_at = activity(&root)
        .expect("readable")
        .expect("the journal's write");
    // A stream written later than the journal moves it: a worker that is only streaming is still running.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    let streams = Project::at(&root).run_dir("r1").join("streams");
    std::fs::write(streams.join("b2.jsonl"), "{}\n").expect("stream");
    assert!(
        activity(&root)
            .expect("readable")
            .expect("the stream's write")
            > journal_at
    );
    // The newest owner key of any pane; a key file that does not read as a second is a doubt.
    let dir = nirvana_seat::dir();
    assert_eq!(owner_key(&dir), Ok(None), "no keys folder: no key");
    std::fs::create_dir_all(dir.join("keys")).expect("keys");
    std::fs::write(dir.join("keys").join("wP_p1"), "100").expect("key");
    std::fs::write(dir.join("keys").join("wP_p2"), "200").expect("key");
    assert_eq!(owner_key(&dir), Ok(Some(200)));
    std::fs::write(dir.join("keys").join("wP_p3"), "not a second").expect("key");
    assert!(owner_key(&dir).is_err());
}

// ---- review round (learn-review.md): S1-S6, N5, N8, N10 ----

/// A worker lesson under the worker's own class, as the gate wrote one before the class fix, planted in `root` and
/// logged as herdr's write. Returns its slug.
fn plant(root: &Path, run: &str, order: &str, name: &str, class: &str, quote: &str) -> String {
    let lesson = serde_json::json!({"name":name,"class":class,"steps":"1. do it","evidence":quote});
    let other = temp("plant");
    let _ = lessons::verify_and_write(
        &other,
        &lesson,
        true,
        &stream(quote),
        order,
        run,
        10,
        Meta::default(),
    );
    let slug = lessons::slug(name);
    let dir = lessons::dir(root);
    std::fs::create_dir_all(&dir).expect("lessons");
    std::fs::copy(
        lessons::dir(&other).join(format!("{slug}.md")),
        dir.join(format!("{slug}.md")),
    )
    .expect("copy");
    logged(root, &slug);
    let mut index = lessons::read_index(root);
    index.lessons.insert(
        slug.clone(),
        lessons::read_index(&other).lessons[&slug].clone(),
    );
    std::fs::write(
        dir.join(lessons::INDEX),
        serde_json::to_string_pretty(&index).unwrap(),
    )
    .expect("index");
    slug
}

#[test]
fn learn_an_unreadable_lease_file_means_no_review() {
    let _home = quiet("learn-leases-corrupt");
    let world = world(3_600, 10);
    let root = ready_project("leases-corrupt", &world);
    seed_samples(&world);
    let dir = nirvana_seat::dir();
    std::fs::write(dir.join(nirvana_seat::LEASES_FILE), "{ not json").expect("leases");
    let lines = tick_in(&dir, std::slice::from_ref(&root));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("cannot tell whether an order runs")),
        "{lines:?}"
    );
    assert!(read_space(&root).last.is_none(), "nothing reserved");
    // A runs folder that cannot be listed (here a file where the folder belongs) is a doubt, never quiet.
    let odd = temp("runs-is-a-file");
    let runs = Project::at(&odd).dir().join("runs");
    let _ = std::fs::remove_dir_all(&runs);
    std::fs::write(&runs, "not a folder").expect("file");
    assert!(activity(&odd).is_err());
    let doubted = tick_with(&[root.clone(), odd.clone()], &table(), &world, None);
    assert!(
        doubted
            .iter()
            .filter(|line| line.contains("cannot tell whether an order runs"))
            .count()
            == 2,
        "{doubted:?}"
    );
    // Any doubt refuses, whatever else the facts say.
    let mut doubt = facts(10_000_000);
    doubt.doubt = Some("C:/p/.nirvana/runs: access denied".into());
    skipped(decide(&doubt), "cannot tell");
    // A record that exists and cannot be read is no project list, not an empty one.
    let record = root.join("record-is-a-folder");
    std::fs::create_dir_all(&record).expect("folder");
    assert!(known_projects_in(&record, Path::new("Z:/nowhere")).is_err());
}

#[test]
fn learn_the_week_must_have_room_too() {
    let now = 10_000_000;
    let with_week = |week: Vec<(String, Option<f32>)>| {
        let mut facts = facts(now);
        if let Ok(spare) = facts.spare.as_mut() {
            spare.week = week;
        }
        facts
    };
    assert_eq!(
        decide(&with_week(vec![("week".into(), Some(0.65))])),
        Decision::Run
    );
    skipped(
        decide(&with_week(vec![("week".into(), Some(0.1))])),
        "the week has no room",
    );
    skipped(
        decide(&with_week(vec![
            ("week".into(), Some(0.65)),
            ("week.fable".into(), None),
        ])),
        "week.fable reading is missing or old",
    );
    skipped(decide(&with_week(Vec::new())), "no week reading");
    // The seat rule's reading: a week 60% used at half elapsed is on pace for 120%, so nothing of it is spare.
    let _home = quiet("learn-week");
    let mut hot = world(3_600, 10);
    let half = hot.now + 7 * 86_400 / 2;
    for reading in hot.readings.iter_mut().filter(|r| r.window == "week") {
        reading.used = Some(0.6);
        reading.resets_at = Some(half);
    }
    let (_, spare) = spare(&table(), &hot, "x-review").expect("a pick");
    assert!(
        !spare.week.is_empty()
            && spare
                .week
                .iter()
                .all(|(_, share)| share.is_some_and(|s| s < WEEK_SPARE_MIN)),
        "{spare:?}"
    );
}

#[test]
fn learn_a_failed_queue_counts_once_and_backs_off() {
    let _home = quiet("learn-backoff");
    let world = world(3_600, 10);
    let root = ready_project("backoff", &world);
    seed_samples(&world);
    // `book` reads leases.json itself and refuses one that does not parse; the tick's world was read before.
    std::fs::write(
        nirvana_seat::dir().join(nirvana_seat::LEASES_FILE),
        "{ not json",
    )
    .expect("leases");
    let first = tick_with(std::slice::from_ref(&root), &table(), &world, None);
    assert!(
        first.iter().any(|line| line.contains("not queued")),
        "{first:?}"
    );
    let state = read_space(&root);
    assert_eq!(used_today(&state, world.now), 1, "the try is counted");
    assert!(
        state.backoff_until.is_some_and(|until| until > world.now),
        "{state:?}"
    );
    let last = state.last.expect("the reservation");
    assert!(
        last.result
            .as_deref()
            .is_some_and(|r| r.starts_with("not queued")),
        "{last:?}"
    );
    assert_eq!(
        last.through,
        world.now - LOOKBACK_S,
        "its orders are read again later"
    );
    assert_eq!(
        review::used_today(&root, nirvana_run::now()),
        0,
        "a review that never seated gives back its lesson-review count"
    );
    let second = tick_with(std::slice::from_ref(&root), &table(), &world, None);
    assert!(
        second.iter().any(|line| line.contains("next try in")),
        "{second:?}"
    );
    assert_eq!(used_today(&read_space(&root), world.now), 1);
}

#[test]
fn learn_a_tick_that_dies_after_its_reservation_never_runs_the_orders_twice() {
    let _home = quiet("learn-crash");
    let world = world(3_600, 10);
    let root = ready_project("crash", &world);
    seed_samples(&world);
    DIE_AFTER_RESERVE.set(true);
    let _ = tick_with(std::slice::from_ref(&root), &table(), &world, None);
    DIE_AFTER_RESERVE.set(false);
    let state = read_space(&root);
    assert_eq!(
        used_today(&state, world.now),
        1,
        "the count is taken before anything is queued"
    );
    assert_eq!(
        read_global()
            .windows
            .values()
            .map(|(_, count)| count)
            .sum::<u32>(),
        1,
        "so is the window's"
    );
    let last = state.last.clone().expect("the reservation");
    assert!(
        !Project::at(&root).journal(&last.run).path.is_file(),
        "nothing was queued"
    );
    let again = tick_with(std::slice::from_ref(&root), &table(), &world, None);
    assert!(
        again
            .iter()
            .any(|line| line.contains("no order gated here")),
        "{again:?}"
    );
    assert!(
        read_space(&root).last.and_then(|l| l.result).is_none(),
        "not stranded yet"
    );
    let mut later = world.clone();
    later.now += STRANDED_S + 60;
    let line = settle(&Project::at(&root), &later, true).expect("settled");
    assert!(line.contains("never seated"), "{line}");
    assert!(read_space(&root)
        .last
        .and_then(|l| l.result)
        .is_some_and(|r| r.contains("never seated")));
}

#[test]
fn learn_use_of_a_pool_by_anyone_stops_a_review() {
    let _home = quiet("learn-moved");
    let world = world(3_600, 10);
    let mut global = Global::default();
    assert!(
        usage_moved(&global, &world, "gemini").is_err(),
        "no history: no review"
    );
    record_samples(&mut global, &world);
    assert!(
        global.samples.contains_key("gemini") && global.samples.contains_key("max"),
        "{global:?}"
    );
    assert!(
        usage_moved(&global, &world, "gemini").is_err(),
        "a reading of now is not one of half an hour ago"
    );
    let old = |used: f32| Sample {
        read_at: world.now - IDLE_AFTER_S - 60,
        used,
        resets_at: Some(world.now + 3_600),
    };
    global.samples.insert("gemini".into(), vec![old(0.2)]);
    global.samples.insert("max".into(), vec![old(0.2)]);
    assert_eq!(usage_moved(&global, &world, "gemini"), Ok(None));
    // Max rose 10 points: the owner's Claude Code is at work, though the review would run on gemini.
    global.samples.insert("max".into(), vec![old(0.1)]);
    let moved = usage_moved(&global, &world, "gemini").expect("history");
    assert_eq!(moved.map(|(pool, _)| pool).as_deref(), Some("max"));
    global.samples.insert("max".into(), vec![old(0.2)]);
    global.samples.insert("gemini".into(), vec![old(0.1)]);
    let moved = usage_moved(&global, &world, "gemini").expect("history");
    assert_eq!(moved.map(|(pool, _)| pool).as_deref(), Some("gemini"));
    // A sample from before the last reset is not a baseline.
    global.samples.insert(
        "gemini".into(),
        vec![Sample {
            resets_at: Some(world.now + 3_600 - 5 * 3_600),
            ..old(0.2)
        }],
    );
    assert!(usage_moved(&global, &world, "gemini").is_err());
    let now = 10_000_000;
    let mut rose = facts(now);
    rose.usage_moved = Ok(Some(("max".into(), 0.1)));
    skipped(decide(&rose), "someone is using it");
    let mut unknown = facts(now);
    unknown.usage_moved = Err("no reading".into());
    skipped(decide(&unknown), "no usage history");
}

#[test]
fn learn_reclass_leaves_a_lesson_the_owner_edited() {
    let _home = quiet("learn-edited");
    let root = temp("edited");
    gated(
        &root,
        "r1",
        "b1",
        "build.bounded",
        "flush matrix 4x4 recovered",
        "pass",
        1_000,
    );
    gated(
        &root,
        "r1",
        "b2",
        "build.bounded",
        "purge volume 120 mm3",
        "pass",
        1_100,
    );
    let kept = plant(
        &root,
        "r1",
        "b1",
        "Recover the flush matrix",
        "slicer-estimation",
        "flush matrix 4x4 recovered",
    );
    let owners = plant(
        &root,
        "r1",
        "b2",
        "Purge less",
        "slicer-estimation",
        "purge volume 120 mm3",
    );
    // The owner opens one in an editor: CRLF, and a line of their own.
    let path = lessons::dir(&root).join(format!("{owners}.md"));
    let edited = std::fs::read_to_string(&path)
        .expect("lesson")
        .replace('\n', "\r\n")
        + "\r\nmy note\r\n";
    std::fs::write(&path, &edited).expect("edit");
    let classes = lessons::router_classes();
    let done = lessons::reclass(&root, &classes, false, false, 2_000).expect("reclass");
    assert_eq!(
        done.moved
            .iter()
            .map(|(slug, _, _)| slug.as_str())
            .collect::<Vec<_>>(),
        [kept.as_str()],
        "{done:?}"
    );
    assert!(
        done.left
            .iter()
            .any(|(slug, _, why)| *slug == owners && why.starts_with(lessons::EDITED)),
        "{done:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("lesson"),
        edited,
        "the owner's file is untouched"
    );
    // The owner's own `--edited` moves it, and keeps its line endings and its note.
    let forced = lessons::reclass(&root, &classes, false, true, 3_000).expect("forced");
    assert_eq!(forced.moved.len(), 1, "{forced:?}");
    let after = std::fs::read_to_string(&path).expect("lesson");
    assert!(
        after.contains("\r\nclass: build.bounded\r\ntopic: slicer-estimation\r\n")
            && after.ends_with("\r\nmy note\r\n"),
        "{after:?}"
    );
}

#[test]
fn learn_a_launch_that_fails_is_a_failure() {
    let _home = quiet("learn-launch");
    let world = world(3_600, 10);
    let root = ready_project("launch", &world);
    seed_samples(&world);
    review::LAUNCH_FAILS.set(true);
    let lines = tick_with(std::slice::from_ref(&root), &table(), &world, None);
    review::LAUNCH_FAILS.set(false);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("not queued: launch failed")),
        "{lines:?}"
    );
    let last = read_space(&root).last.expect("reserved");
    let project = Project::at(&root);
    let state = replay(&project.journal(&last.run).read().0);
    assert_eq!(
        state.order(&last.order).map(|order| order.phase),
        Some(Phase::Interrupted)
    );
    assert!(state.ended.is_some(), "its run ends");
    let leases = nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE))
        .expect("leases");
    assert!(
        leases.live(&last.order, nirvana_run::now()).is_none(),
        "its lease is released"
    );
    assert_eq!(review::used_today(&root, nirvana_run::now()), 0);
}

#[test]
fn learn_a_tick_reads_the_owners_key_a_fresh_stream_and_its_lock() {
    let _home = quiet("learn-tick-facts");
    let world = world(3_600, 10);
    let root = ready_project("tick-facts", &world);
    seed_samples(&world);
    let dir = nirvana_seat::dir();
    std::fs::create_dir_all(dir.join("keys")).expect("keys");
    std::fs::write(dir.join("keys").join("wP_p1"), (world.now - 60).to_string()).expect("key");
    let typed = tick_with(std::slice::from_ref(&root), &table(), &world, None);
    assert!(
        typed.iter().any(|line| line.contains("the owner typed")),
        "{typed:?}"
    );
    std::fs::remove_file(dir.join("keys").join("wP_p1")).expect("key");
    // Review round S5 through the tick: max's 5h use rose ten points in the last half hour.
    change_global(|global| {
        if let Some(sample) = global.samples.get_mut("max").and_then(|s| s.first_mut()) {
            sample.used -= 0.1;
        }
    })
    .expect("samples");
    let used = tick_with(std::slice::from_ref(&root), &table(), &world, None);
    assert!(
        used.iter().any(|line| line.contains("someone is using it")),
        "{used:?}"
    );
    seed_samples(&world);
    // A stream written now, read on a clock that is now.
    let mut live = world.clone();
    live.now = nirvana_run::now();
    std::fs::write(
        Project::at(&root)
            .run_dir("r1")
            .join("streams")
            .join("b3.jsonl"),
        "{}\n",
    )
    .expect("stream");
    let writing = tick_with(std::slice::from_ref(&root), &table(), &live, None);
    assert!(
        writing.iter().any(|line| line.contains("an order wrote")),
        "{writing:?}"
    );
    assert!(read_space(&root).last.is_none(), "neither queued");
    // One tick at a time.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join(LOCK))
        .expect("lock file");
    lock.lock().expect("lock");
    assert_eq!(
        tick_in(&dir, std::slice::from_ref(&root)),
        vec!["another idle review tick is running".to_owned()]
    );
    drop(lock);
}

#[test]
fn learn_known_projects_skip_fixtures_gone_roots_and_spellings() {
    let base = temp("known");
    let real = base.join("Real");
    let fixture = base.join("fixtures").join("f1");
    let gone = base.join("gone");
    for root in [&real, &fixture] {
        Project::at(root).ensure().expect(".nirvana");
    }
    let journal = |root: &Path| {
        serde_json::json!({"journal": root.join(nirvana_run::DIR).join("runs").join("r1").join("journal.jsonl")
            .display().to_string()})
        .to_string()
    };
    let other_case = PathBuf::from(real.display().to_string().replace("Real", "REAL"));
    let record = base.join("record.jsonl");
    std::fs::write(
        &record,
        [
            journal(&real),
            journal(&fixture),
            journal(&gone),
            journal(&other_case),
            "not json".into(),
        ]
        .join("\n"),
    )
    .expect("record");
    let known = known_projects_in(&record, &base.join("fixtures")).expect("readable");
    assert_eq!(known, vec![real.clone()], "{known:?}");
    assert_eq!(
        known_projects_in(&base.join("no-record.jsonl"), &base).expect("missing is empty"),
        Vec::<PathBuf>::new()
    );
}

#[test]
fn learn_a_seated_review_that_never_started_is_settled_once_its_lease_ends() {
    let _home = quiet("learn-never-started");
    let world = world(3_600, 10);
    let root = ready_project("never-started", &world);
    seed_samples(&world);
    let lines = tick_with(std::slice::from_ref(&root), &table(), &world, None);
    assert!(
        lines.iter().any(|line| line.contains("queued:")),
        "{lines:?}"
    );
    let last = read_space(&root).last.expect("a review");
    let project = Project::at(&root);
    // No worker ever started (the test launcher starts none); its lease is released, as at its end.
    crate::nirvana_worker::release_lease(&last.order);
    let mut later = world.clone();
    later.now += STRANDED_S + 60;
    assert_eq!(
        settle(&project, &later, false),
        None,
        "with the leases unread it is never called stranded"
    );
    let line = settle(&project, &later, true).expect("settled");
    assert!(line.contains("never started"), "{line}");
    assert!(replay(&project.journal(&last.run).read().0).ended.is_some());
    assert!(read_space(&root)
        .last
        .and_then(|l| l.result)
        .is_some_and(|r| r.contains("never started")));
}
