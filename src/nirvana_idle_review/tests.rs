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
        busy: None,
        owner_key: Ok(Some(now - IDLE_AFTER_S - 60)),
        activity: Some((now - IDLE_AFTER_S - 60, "p".into())),
        new_orders: 3,
        review_cap: None,
        used_today: 0,
        used_window: 0,
        spare: Ok(Spare {
            seat: "flash".into(),
            pool: "gemini".into(),
            spare: Some(0.75),
            resets_at: Some(now + 3_600),
        }),
    }
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
    let dry = lessons::reclass(&root, &classes, true, 3_000).expect("dry");
    assert_eq!(dry.moved.len(), 2, "{dry:?}");
    assert_eq!(
        lessons::body(&root, "recover-the-flush-matrix").as_deref(),
        Some(before.as_str())
    );
    let done = lessons::reclass(&root, &classes, false, 3_000).expect("reclass");
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
    lessons::rollback(&root, &id, 4_000).expect("rollback");
    assert_eq!(
        lessons::body(&root, "recover-the-flush-matrix").as_deref(),
        Some(before.as_str())
    );
    assert_eq!(
        lessons::read_index(&root).lessons["recover-the-flush-matrix"].class,
        "slicer-estimation"
    );
    // A second reclass after nothing changed moves nothing new.
    let again = lessons::reclass(&root, &classes, false, 5_000).expect("again");
    assert_eq!(again.moved.len(), 2);
    let third = lessons::reclass(&root, &classes, false, 6_000).expect("third");
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
    nothing.new_orders = 0;
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
        off: false,
        window: Some(("gemini".into(), now + 3_600)),
        count: 3,
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
    let lines = tick_with(&projects, &table(), &world);
    assert!(
        lines.iter().any(|line| line.contains("queued:")),
        "{lines:?}"
    );
    let last = read_space(&root).last.expect("a last review");
    assert_eq!(last.orders, 2);
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
    let later = tick_with(&projects, &table(), &leased);
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
        {"op":"new","name":"Invented","class":"build.bounded","steps":"1. x","evidence":"this quote is in no stream at all"}
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
    // The Learning square: when it ran and what it did; and its run ended.
    let view = crate::nirvana_learning::load(&root, world.now);
    let result = view
        .idle
        .last
        .as_ref()
        .and_then(|l| l.result.clone())
        .expect("a result");
    assert!(
        result.starts_with("pass · 2 applied") && result.contains("1 refused"),
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
    assert_eq!(activity(&root), None, "no run, no activity");
    gated(&root, "r1", "b1", "build.bounded", "x", "pass", 1_000);
    let journal_at = activity(&root).expect("the journal's write");
    // A stream written later than the journal moves it: a worker that is only streaming is still running.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    let streams = Project::at(&root).run_dir("r1").join("streams");
    std::fs::write(streams.join("b2.jsonl"), "{}\n").expect("stream");
    assert!(activity(&root).expect("the stream's write") > journal_at);
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
