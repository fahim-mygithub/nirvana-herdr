//! Plan 43 A6 and A7 (nirvana/plans/2026-11-18-43-megajam-audit.md C9 to C11): orders a dead worker left listed as
//! running, the reseat's end record, and the reset a quota error's text names. Every journal and folder is a temp one;
//! the supervisor's lock, the OS's pids and the wake are passed in, so no test reads a live process or the owner's
//! Nirvana home.

use std::cell::{Cell, RefCell};

use super::*;

/// The worker's `started` second in the journals below.
const STARTED: u64 = T0 + 10;
/// Long after it: the server start of the check.
const LATER: u64 = T0 + 4 * 3600;

struct World {
    lock: Box<dyn Fn(&str) -> bool>,
    pid: Box<dyn Fn(u32) -> PidSeen>,
    woken: RefCell<Vec<String>>,
}

impl World {
    fn new(lock: impl Fn(&str) -> bool + 'static, pid: impl Fn(u32) -> PidSeen + 'static) -> Self {
        Self {
            lock: Box::new(lock),
            pid: Box::new(pid),
            woken: RefCell::new(Vec::new()),
        }
    }

    fn check(&self, home: &Path, projects: &[PathBuf], started: Option<u64>, now: u64) -> Vec<String> {
        let lock_live = |_: &Project, _: &str, order: &str| (self.lock)(order);
        let pid_seen = |pid: u32| (self.pid)(pid);
        let wake = |_: &Project, run: &str| self.woken.borrow_mut().push(run.to_owned());
        let world = Liveliness {
            lock_live: &lock_live,
            pid_seen: &pid_seen,
            wake: &wake,
        };
        restart_check(home, projects, started, false, &world, now)
    }
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A project with run `r`: each order seated with its worktree `cwd`, and started as pid 4242 at `STARTED` unless
/// it is in `unstarted`.
fn project(name: &str, orders: &[&str], unstarted: &[&str], cwd: &Path) -> Project {
    let project = Project::at(temp_dir(name));
    let journal = project.journal("r");
    journal.append(T0, opened(Some("wP:p1"))).unwrap();
    for order in orders {
        let mut seat = seated(order);
        seat.cwd = cwd.display().to_string();
        journal.append(T0 + 1, Kind::Seated(Box::new(seat))).unwrap();
        if !unstarted.contains(order) {
            journal
                .append(
                    STARTED,
                    Kind::Started {
                        order: (*order).into(),
                        attempt: 1,
                        pid: Some(4242),
                        pane: Some("wP:p2".into()),
                        tab: None,
                        resume: None,
                        argv: Vec::new(),
                    },
                )
                .unwrap();
        }
    }
    project
}

fn state(project: &Project) -> RunState {
    replay(&project.journal("r").read().0)
}

fn order_in(state: &RunState, order: &str) -> OrderState {
    state.order(order).expect("order").clone()
}

fn running(phase: Phase, worker: Option<(u32, u64)>, last_at: u64) -> OrderState {
    let project = project("liveness", &["o"], &[], &temp_dir("liveness-cwd"));
    let mut order = order_in(&state(&project), "o");
    order.phase = phase;
    order.worker = worker;
    order.last_at = last_at;
    order
}

#[test]
fn restart_a_free_lock_and_a_gone_pid_read_dead_and_a_held_lock_reads_live() {
    let order = running(Phase::Running, Some((4242, STARTED)), STARTED);
    let gone = |_: u32| PidSeen::Gone;
    assert!(matches!(
        worker_liveness(&order, false, &gone, LATER),
        Some(Liveness::Dead(why)) if why.contains("pid 4242 is gone")
    ));
    // A held lock is a supervisor, whatever the pid says.
    assert!(matches!(
        worker_liveness(&order, true, &gone, LATER),
        Some(Liveness::Live(_))
    ));
}

#[test]
fn restart_a_reused_pid_reads_dead_and_the_workers_own_process_a_doubt() {
    let order = running(Phase::Running, Some((4242, STARTED)), STARTED);
    let since = |at: u64| move |_: u32| PidSeen::Since(at);
    // Created after the worker's start: another process took the pid.
    assert!(matches!(
        worker_liveness(&order, false, &since(STARTED + PID_SLACK_S + 1), LATER),
        Some(Liveness::Dead(why)) if why.contains("after the worker's start")
    ));
    // Created before it, or inside the slack: it may be the worker, so it is left running.
    for created in [STARTED - 5, STARTED + PID_SLACK_S] {
        assert!(matches!(
            worker_liveness(&order, false, &since(created), LATER),
            Some(Liveness::Doubt(why)) if why.contains("still runs")
        ));
    }
}

#[test]
fn restart_an_unreadable_pid_and_no_pid_are_doubts() {
    let unknown = |_: u32| PidSeen::Unknown;
    let order = running(Phase::Running, Some((4242, STARTED)), STARTED);
    assert!(matches!(
        worker_liveness(&order, false, &unknown, LATER),
        Some(Liveness::Doubt(why)) if why.contains("could not be read")
    ));
    let order = running(Phase::Running, None, STARTED);
    assert!(matches!(
        worker_liveness(&order, false, &unknown, LATER),
        Some(Liveness::Doubt(why)) if why.contains("recorded no pid")
    ));
}

#[test]
fn restart_a_seated_order_waits_its_grace_and_other_phases_are_not_judged() {
    let gone = |_: u32| PidSeen::Gone;
    let order = running(Phase::Seated, None, T0);
    assert!(matches!(
        worker_liveness(&order, false, &gone, T0 + SEATED_GRACE_S - 1),
        Some(Liveness::Live(_))
    ));
    assert!(matches!(
        worker_liveness(&order, false, &gone, T0 + SEATED_GRACE_S),
        Some(Liveness::Dead(why)) if why.contains("no supervisor took it")
    ));
    for phase in [
        Phase::Deferred,
        Phase::Delivered,
        Phase::Asked,
        Phase::Interrupted,
        Phase::Reseated,
    ] {
        let order = running(phase, Some((4242, STARTED)), STARTED);
        assert_eq!(worker_liveness(&order, false, &gone, LATER), None, "{phase:?}");
    }
}

#[test]
fn restart_a_dead_order_is_checkpointed_interrupted_and_woken_once_per_server_start() {
    let tree = temp_dir("restart-tree");
    git(&tree, &["init", "-q"]);
    git(&tree, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "base"]);
    std::fs::write(tree.join("work.txt"), "half done\n").unwrap();
    git(&tree, &["add", "work.txt"]);
    let project = project("restart-dead", &["o"], &[], &tree);
    let home = temp_dir("restart-home");
    let world = World::new(|_| false, |_| PidSeen::Gone);
    let lines = world.check(&home, &[project.root.clone()], Some(LATER - 60), LATER);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let state = state(&project);
    let order = order_in(&state, "o");
    assert_eq!(order.phase, Phase::Interrupted);
    let why = order.why.clone().unwrap_or_default();
    assert!(why.starts_with(WORKER_DIED), "{why}");
    assert!(why.contains("checkpoints/o-restart.diff"), "{why}");
    let diff = std::fs::read_to_string(project.run_dir("r").join("checkpoints/o-restart.diff")).unwrap();
    assert!(diff.contains("half done"), "{diff}");
    assert_eq!(order.last_checkpoint.as_deref(), Some("checkpoints/o-restart.diff"));
    // The orchestrator hears which order died, its checkpoint and the way on.
    assert_eq!(*world.woken.borrow(), vec!["r".to_owned()]);
    let line = &state.queue.last().expect("a queue line").line;
    assert!(line.starts_with("o interrupted: its worker died"), "{line}");
    assert!(line.contains("the handoff skill, then `herdr seat run`"), "{line}");
    assert_eq!(restart_mark(&home), Some(LATER - 60));
    // The same server start again: nothing more is written and no one is woken.
    let events = project.journal("r").read().0.len();
    world.check(&home, &[project.root.clone()], Some(LATER - 60), LATER + 60);
    assert_eq!(project.journal("r").read().0.len(), events);
    assert_eq!(world.woken.borrow().len(), 1);
}

#[test]
fn restart_a_live_worker_is_never_interrupted_and_a_doubt_is_told_once() {
    let cwd = temp_dir("restart-live-cwd");
    let held = project("restart-held", &["o"], &[], &cwd);
    let home = temp_dir("restart-live-home");
    let world = World::new(|_| true, |_| PidSeen::Gone);
    assert!(world.check(&home, &[held.root.clone()], Some(LATER), LATER).is_empty());
    assert_eq!(order_in(&state(&held), "o").phase, Phase::Running);
    assert!(world.woken.borrow().is_empty());
    // Lock free, but the worker's own pid still runs: left running, and said once for this start.
    let doubted = project("restart-doubt", &["o"], &[], &cwd);
    let home = temp_dir("restart-doubt-home");
    let world = World::new(|_| false, |_| PidSeen::Since(STARTED - 1));
    world.check(&home, &[doubted.root.clone()], Some(LATER), LATER);
    let state1 = state(&doubted);
    assert_eq!(order_in(&state1, "o").phase, Phase::Running);
    let line = &state1.queue.last().expect("a doubt line").line;
    assert!(line.contains("could not prove its worker dead"), "{line}");
    assert_eq!(world.woken.borrow().len(), 1);
    let events = doubted.journal("r").read().0.len();
    world.check(&home, &[doubted.root.clone()], Some(LATER), LATER + 60);
    assert_eq!(doubted.journal("r").read().0.len(), events, "once per server start");
}

#[test]
fn restart_a_worker_back_under_the_journals_lock_is_not_cut() {
    let cwd = temp_dir("restart-back-cwd");
    let project = project("restart-back", &["o"], &[], &cwd);
    let home = temp_dir("restart-back-home");
    // Free when first judged, held when judged again under the lock.
    let looks = Cell::new(0);
    let looks = std::rc::Rc::new(looks);
    let seen = looks.clone();
    let world = World::new(
        move |_| {
            seen.set(seen.get() + 1);
            seen.get() > 1
        },
        |_| PidSeen::Gone,
    );
    world.check(&home, &[project.root.clone()], Some(LATER), LATER);
    assert!(looks.get() >= 2, "judged again under the lock");
    assert_eq!(order_in(&state(&project), "o").phase, Phase::Running);
    assert!(world.woken.borrow().is_empty());
}

#[test]
fn restart_a_pass_without_the_servers_start_reads_only_stale_pane_marks() {
    let cwd = temp_dir("restart-mark-cwd");
    let project = project("restart-mark", &["a", "b", "done"], &[], &cwd);
    project
        .journal("r")
        .append(
            STARTED + 5,
            Kind::Delivered {
                order: "done".into(),
                report: serde_json::json!({"status": "done"}),
            },
        )
        .unwrap();
    let live = project.run_dir("r").join("live");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::write(live.join("a.pane"), "wP:p2").unwrap();
    std::fs::write(live.join("done.pane"), "wP:p3").unwrap();
    let home = temp_dir("restart-mark-home");
    let world = World::new(|_| false, |_| PidSeen::Gone);
    let lines = world.check(&home, &[project.root.clone()], None, LATER);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let state = state(&project);
    assert_eq!(order_in(&state, "a").phase, Phase::Interrupted);
    assert_eq!(order_in(&state, "b").phase, Phase::Running, "no marker: left to the start's pass");
    // Both markers are gone: `a` now interrupted, `done` delivered; a clean exit would have removed them.
    assert!(!live.join("a.pane").exists());
    assert!(!live.join("done.pane").exists());
    assert_eq!(restart_mark(&home), None, "no server start, no mark");
}

#[test]
fn restart_no_run_an_ended_run_and_an_old_run_are_safe() {
    let home = temp_dir("restart-safe-home");
    let world = World::new(|_| false, |_| PidSeen::Gone);
    // No project, and a folder with no `.nirvana`.
    assert!(world.check(&home, &[], Some(LATER), LATER).is_empty());
    assert!(world
        .check(&home, &[temp_dir("restart-empty")], Some(LATER + 1), LATER)
        .is_empty());
    // An ended run.
    let cwd = temp_dir("restart-safe-cwd");
    let ended = project("restart-ended", &["o"], &[], &cwd);
    ended
        .journal("r")
        .append(
            STARTED + 1,
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
    assert!(world.check(&home, &[ended.root.clone()], Some(LATER + 2), LATER).is_empty());
    // A run whose journal has not been written for longer than the lookback.
    let old = project("restart-old", &["o"], &[], &cwd);
    let far = now() + RESTART_LOOKBACK_S + 3600;
    assert!(world.check(&home, &[old.root.clone()], Some(LATER + 3), far).is_empty());
    assert_eq!(order_in(&state(&old), "o").phase, Phase::Running);
    assert!(world.woken.borrow().is_empty());
}

#[test]
fn restart_an_unstarted_seated_order_past_its_grace_is_interrupted() {
    let cwd = temp_dir("restart-seated-cwd");
    let project = project("restart-seated", &["o"], &["o"], &cwd);
    let home = temp_dir("restart-seated-home");
    let world = World::new(|_| false, |_| PidSeen::Gone);
    world.check(&home, &[project.root.clone()], Some(T0 + 60), T0 + 60);
    assert_eq!(order_in(&state(&project), "o").phase, Phase::Seated, "under the grace");
    world.check(&home, &[project.root.clone()], Some(T0 + 1 + SEATED_GRACE_S), T0 + 1 + SEATED_GRACE_S);
    assert_eq!(order_in(&state(&project), "o").phase, Phase::Interrupted);
}

#[test]
fn reseat_events_end_the_parent_and_its_line_is_told_once() {
    let mut child = seated("o-r1");
    child.parent = Some("o".into());
    child.why = Some("reseat".into());
    let mut events = vec![
        ev(1, T0, opened(Some("wP:p1"))),
        ev(2, T0 + 1, Kind::Seated(Box::new(seated("o")))),
    ];
    for (seq, kind) in (3..).zip(reseat_events(child, "o", "sonnet", "g-opus medium ran out")) {
        events.push(ev(seq, T0 + 2, kind));
    }
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        Kind::Ended { order: Some(order), outcome, .. } if order == "o" && outcome == RESEATED
    )));
    let state = replay(&events);
    let parent = state.order("o").unwrap();
    assert_eq!(parent.phase, Phase::Reseated);
    assert_eq!(parent.outcome.as_deref(), Some(RESEATED));
    assert_eq!(state.order("o-r1").unwrap().phase, Phase::Seated);
    let lines: Vec<&str> = state.queue.iter().map(|item| item.line.as_str()).collect();
    assert_eq!(lines, vec!["o re-seated as o-r1 on sonnet: g-opus medium ran out"]);
    assert_eq!(outcome_word(parent), "reseated");
}

/// The agy 429 of run ia1, seq 3769, as its `detail` recorded it.
const AGY_429: &str = r#"AGY_ERROR: {"code_kind":"http","error_code":429,"error_id":"d249e328-32c2-4164-9939-e3589e4a044b-55","retryable":true,"short_error":"RESOURCE_EXHAUSTED (code 429): Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 79h6m53s.","status":"RESOURCE_EXHAUSTED"}"#;

#[test]
fn resets_in_reads_the_duration_a_quota_error_names() {
    let now = 1_794_607_762;
    assert_eq!(resets_in(AGY_429, now), Some(now + 79 * 3600 + 6 * 60 + 53));
    for (text, secs) in [
        ("Resets in 2h5m.", 2 * 3600 + 5 * 60),
        ("resets in 45m", 45 * 60),
        ("Resets in 30s", 30),
        ("Resets in 1d2h", 86_400 + 2 * 3600),
    ] {
        assert_eq!(resets_in(text, now), Some(now + secs), "{text}");
    }
    for text in [
        "Individual quota reached.",
        "Resets in soon",
        "Resets in 5",
        "Resets in 5m5h",
        "Resets in 5m3",
        "Resets in h",
        "",
    ] {
        assert_eq!(resets_in(text, now), None, "{text}");
    }
}

#[test]
fn resets_in_a_short_named_reset_is_the_deferral_and_a_long_one_bounds_none() {
    let now = 1_794_607_762;
    // A spent pool that names its own reset within the defer window waits for it, whatever the other pools say.
    let soon = resets_in("RESOURCE_EXHAUSTED (code 429): ... Resets in 12m.", now);
    assert_eq!(
        rate_decision(now, soon, 30 * 60, &Err(Some(now + 3600)), true),
        RateDecision::Defer {
            until: now + 12 * 60 + RESET_MARGIN_S,
            why: format!("the window resets at {}, within 30 min", utc(now + 12 * 60)),
        }
    );
    // The ia1 429: 79 h away, and no other seat has room or a reset: auto mode waits for the named reset instead of
    // asking with "no reset time is known".
    let far = resets_in(AGY_429, now);
    assert!(matches!(
        rate_decision(now, far, 30 * 60, &Err(None), true),
        RateDecision::Defer { until, .. } if until == far.unwrap() + RESET_MARGIN_S
    ));
}

/// The OS's answer on Windows: this test's own process runs and was created before now; a child that has exited is
/// gone while its handle is still held, and after it is let go.
#[cfg(windows)]
#[test]
fn restart_pid_seen_reads_a_running_process_and_a_finished_child() {
    match pid_seen(std::process::id()) {
        PidSeen::Since(created) => assert!(created <= now(), "{created}"),
        other => panic!("this process reads {other:?}"),
    }
    let mut child = std::process::Command::new("cmd")
        .args(["/c", "exit", "0"])
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("cmd");
    let pid = child.id();
    child.wait().expect("wait");
    assert_eq!(pid_seen(pid), PidSeen::Gone, "exited, its handle still held");
    drop(child);
    assert_eq!(pid_seen(pid), PidSeen::Gone, "exited and let go");
}

/// Plan 43 A7, the call site in `nirvana_worker::agy_rate_limit`: the ia1 429 carries its named reset.
#[test]
fn resets_in_agy_rate_limit_fills_the_named_reset() {
    let before = now();
    let stderr = format!("{AGY_429}\n");
    let limit = crate::nirvana_worker::agy_rate_limit(Some(3), &stderr, "google3p").expect("a limit");
    let at = limit.resets_at.expect("a reset");
    let secs = 79 * 3600 + 6 * 60 + 53;
    assert!(at >= before + secs && at <= now() + secs, "{at}");
}

/// Plan 43 A7, the call site in `nirvana_worker::rate_limited`: a reseat writes its parent's end.
#[test]
fn reseat_events_a_rate_limited_reseat_ends_its_parent() {
    let home = crate::nirvana::with_test_home("p43-reseat-ended");
    let seats = home.path.join("nirvana").join(crate::nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, crate::nirvana_seat::DEFAULT).unwrap();
    let project = Project::at(home.path.join("project"));
    project.ensure().unwrap();
    crate::nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let journal = project.journal("r1");
    let mut seat = seated("o1");
    seat.cwd = project.root.display().to_string();
    seat.order_file = project.root.join("order.md").display().to_string();
    std::fs::write(&seat.order_file, "# The order\nowns: a.txt\n").unwrap();
    journal.append(T0, opened(None)).unwrap();
    journal.append(T0 + 1, Kind::Seated(Box::new(seat.clone()))).unwrap();
    let child = crate::nirvana_worker::child_id(&replay(&journal.read().0), "o1");
    // A held lock on the child: its launch finds a supervisor and starts nothing.
    let _live = crate::nirvana_worker::take_live(&project, "r1", &child).unwrap().unwrap();
    let limit = crate::nirvana_worker::RateLimit {
        signal: "rate_limit_event".into(),
        pool: Some("max".into()),
        window: None,
        resets_at: None,
        detail: String::new(),
    };
    let mut out = Vec::new();
    crate::nirvana_worker::rate_limited(
        &project,
        &journal,
        "r1",
        &seat,
        &limit,
        &crate::nirvana_worker::Stream::new("claude"),
        &mut out,
    )
    .unwrap();
    crate::nirvana_seat::set_test_world(None);
    let view = String::from_utf8_lossy(&out).into_owned();
    let state = replay(&journal.read().0);
    assert!(state.order(&child).is_some(), "re-seated: {view}");
    let parent = state.order("o1").unwrap();
    assert_eq!(parent.phase, Phase::Reseated, "{view}");
    assert_eq!(parent.outcome.as_deref(), Some(RESEATED), "{view}");
}
