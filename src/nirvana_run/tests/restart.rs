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
    proves_free: bool,
    woken: RefCell<Vec<String>>,
}

impl World {
    fn new(lock: impl Fn(&str) -> bool + 'static, pid: impl Fn(u32) -> PidSeen + 'static) -> Self {
        Self {
            lock: Box::new(lock),
            pid: Box::new(pid),
            proves_free: true,
            woken: RefCell::new(Vec::new()),
        }
    }

    /// As on macOS: a free lock proves nothing (fix round F1).
    fn proving_nothing(mut self) -> Self {
        self.proves_free = false;
        self
    }

    fn check(&self, home: &Path, projects: &[PathBuf], started: Option<u64>, now: u64) -> Vec<String> {
        let lock_live = |_: &Project, _: &str, order: &str| (self.lock)(order);
        let pid_seen = |pid: u32| (self.pid)(pid);
        let wake = |_: &Project, run: &str| self.woken.borrow_mut().push(run.to_owned());
        let world = Liveliness {
            lock_live: &lock_live,
            lock_proves_free: self.proves_free,
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
    assert!(restart_marked(&home, LATER - 60));
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
    git(&cwd, &["init", "-q"]);
    git(&cwd, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "base"]);
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
    // Fix round F5: the checkpoint written before the re-check names no event, so it is not left behind.
    assert!(
        !project.run_dir("r").join("checkpoints/o-restart.diff").exists(),
        "a stray checkpoint"
    );
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
    assert!(!home.join("nirvana").join(RESTART_MARK).exists(), "no server start, no mark");
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
    // Fix round F6: the end keeps why the parent stopped, and where it went.
    assert_eq!(
        parent.why.as_deref(),
        Some("g-opus medium ran out; re-seated as o-r1 on sonnet")
    );
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
    // The ia1 429: 79 h away, and no other seat has room or a reset: past the cap, so auto mode asks, naming the
    // reset (fix round F4), where a deferral would have sat 79 h with only an orchestrator line.
    let far = resets_in(AGY_429, now);
    assert!(matches!(
        rate_decision(now, far, 30 * 60, &Err(None), true),
        RateDecision::Ask { why } if why.contains(&utc(far.unwrap())) && why.contains("more than 6 h away")
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

// ---- plan 43 fix round ----

/// A run `run` in `project` with each order seated in `cwd` and started as pid 4242 at `STARTED`.
#[cfg(windows)]
fn add_run(project: &Project, run: &str, orders: &[&str], cwd: &Path) {
    let journal = project.journal(run);
    journal.append(T0, opened(Some("wP:p1"))).unwrap();
    for order in orders {
        let mut seat = seated(order);
        seat.cwd = cwd.display().to_string();
        journal.append(T0 + 1, Kind::Seated(Box::new(seat))).unwrap();
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

fn doubts_in(project: &Project, run: &str) -> usize {
    project
        .journal(run)
        .read()
        .0
        .iter()
        .filter(|event| matches!(event.kind, Kind::LiveDoubt { .. }))
        .count()
}

/// F1: where a free lock proves nothing (macOS), a gone pid and a Seated order past its grace are doubts, and no pane
/// marker reads stale.
#[test]
fn restart_a_free_lock_that_proves_nothing_kills_no_order() {
    let cwd = temp_dir("restart-mac-cwd");
    let project = project("restart-mac", &["run1", "seat1"], &["seat1"], &cwd);
    let live = project.run_dir("r").join("live");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::write(live.join("run1.pane"), "wP:p2").unwrap();
    let home = temp_dir("restart-mac-home");
    let world = World::new(|_| false, |_| PidSeen::Gone).proving_nothing();
    // The every-minute pass: the marker is not read as stale.
    assert!(world.check(&home, &[project.root.clone()], None, LATER).is_empty());
    assert!(live.join("run1.pane").exists());
    // The start's pass: both stay as they were, each with one doubt.
    let lines = world.check(&home, &[project.root.clone()], Some(LATER), LATER);
    assert_eq!(lines.len(), 2, "{lines:?}");
    let state = state(&project);
    assert_eq!(order_in(&state, "run1").phase, Phase::Running);
    assert_eq!(order_in(&state, "seat1").phase, Phase::Seated);
    assert!(
        lines.iter().all(|line| line.contains("does not prove its supervisor gone")),
        "{lines:?}"
    );
    assert_eq!(doubts_in(&project, "r"), 2);
}

/// F2 b and c: two server starts on one machine each run their full check once and keep their own mark, and the
/// second start writes no doubt the first already wrote; a mark that cannot be written repeats the check, not the doubt.
#[test]
fn restart_two_server_starts_keep_their_marks_and_a_doubt_is_not_repeated() {
    let cwd = temp_dir("restart-two-cwd");
    let project = project("restart-two", &["o"], &[], &cwd);
    let home = temp_dir("restart-two-home");
    let world = World::new(|_| false, |_| PidSeen::Since(STARTED - 1));
    let (a, b) = (LATER - 600, LATER - 300);
    assert_eq!(world.check(&home, &[project.root.clone()], Some(a), LATER).len(), 1);
    assert_eq!(doubts_in(&project, "r"), 1);
    assert_eq!(world.woken.borrow().len(), 1);
    // B's first pass: full, but the same doubt is not written again and no one is woken.
    assert!(world.check(&home, &[project.root.clone()], Some(b), LATER + 60).is_empty());
    assert_eq!(doubts_in(&project, "r"), 1);
    assert_eq!(world.woken.borrow().len(), 1);
    // Each start keeps its own mark, so A's next pass is not full again.
    assert!(restart_marked(&home, a) && restart_marked(&home, b));
    // A new attempt of the worker, with the same pid and so the same reason: its `started` clears the old doubt, so it
    // is told again.
    project
        .journal("r")
        .append(
            LATER + 100,
            Kind::Started {
                order: "o".into(),
                attempt: 2,
                pid: Some(4242),
                pane: Some("wP:p2".into()),
                tab: None,
                resume: None,
                argv: Vec::new(),
            },
        )
        .unwrap();
    let c = LATER + 200;
    assert_eq!(world.check(&home, &[project.root.clone()], Some(c), c).len(), 1);
    assert_eq!(doubts_in(&project, "r"), 2);
    assert_eq!(world.woken.borrow().len(), 2);
    // A mark that cannot be written (here `nirvana` is a file): every pass is full, and says so, but writes nothing.
    let blocked = temp_dir("restart-two-blocked");
    std::fs::write(blocked.join("nirvana"), "not a folder").unwrap();
    let d = c + 60;
    for at in [d, d + 60, d + 120] {
        let lines = world.check(&blocked, &[project.root.clone()], Some(d), at);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("mark could not be written"), "{lines:?}");
    }
    assert_eq!(doubts_in(&project, "r"), 2);
    assert_eq!(world.woken.borrow().len(), 2);
}

/// F2: a start whose full check finished reads only stale pane markers on its later passes; the next start reads
/// every recent run again.
#[test]
fn restart_a_marked_start_does_not_read_every_run_again() {
    let cwd = temp_dir("restart-once-cwd");
    let project = project("restart-once", &["o"], &[], &cwd);
    let home = temp_dir("restart-once-home");
    let (a, b) = (LATER - 600, LATER + 600);
    let doubted = World::new(|_| false, |_| PidSeen::Since(STARTED - 1));
    assert_eq!(doubted.check(&home, &[project.root.clone()], Some(a), LATER).len(), 1);
    // The worker dies later, its supervisor with no marker: start A's later passes do not read the run.
    let gone = World::new(|_| false, |_| PidSeen::Gone);
    assert!(gone.check(&home, &[project.root.clone()], Some(a), LATER + 60).is_empty());
    assert_eq!(order_in(&state(&project), "o").phase, Phase::Running);
    // Start B does.
    assert_eq!(gone.check(&home, &[project.root.clone()], Some(b), b).len(), 1);
    assert_eq!(order_in(&state(&project), "o").phase, Phase::Interrupted);
}

/// F2 a: a run whose events cannot be written leaves the start unmarked, each of its orders is still tried, the other
/// runs are checked, and the next pass finishes the job. Windows only: a read-only journal is what makes the append
/// fail here.
#[cfg(windows)]
#[test]
fn restart_a_run_that_cannot_be_written_leaves_the_start_unmarked() {
    let tree = temp_dir("restart-fail-tree");
    git(&tree, &["init", "-q"]);
    git(&tree, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "base"]);
    let project = Project::at(temp_dir("restart-fail"));
    add_run(&project, "r1", &["a", "b"], &tree);
    add_run(&project, "r2", &["c"], &tree);
    let path = project.journal("r1").path;
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms.clone()).unwrap();
    let home = temp_dir("restart-fail-home");
    let world = World::new(|_| false, |_| PidSeen::Gone);
    let lines = world.check(&home, &[project.root.clone()], Some(LATER), LATER);
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&path, perms).unwrap();
    let failed = lines.iter().filter(|line| line.contains("could not be interrupted")).count();
    assert_eq!(failed, 2, "both orders of r1 tried: {lines:?}");
    assert!(
        lines.iter().any(|line| line == "r2: c interrupted: no supervisor holds its lock and worker pid 4242 is gone"),
        "{lines:?}"
    );
    assert!(!restart_marked(&home, LATER), "a failed run leaves the start unmarked");
    // F5: the failed orders' checkpoints name no event, so none is left.
    assert!(!project.run_dir("r1").join("checkpoints/a-restart.diff").exists());
    let r1 = replay(&project.journal("r1").read().0);
    assert_eq!(order_in(&r1, "a").phase, Phase::Running);
    // The next pass of the same start finishes r1 and marks it.
    let lines = world.check(&home, &[project.root.clone()], Some(LATER), LATER + 60);
    assert_eq!(lines.len(), 2, "{lines:?}");
    let r1 = replay(&project.journal("r1").read().0);
    assert_eq!(order_in(&r1, "a").phase, Phase::Interrupted);
    assert_eq!(order_in(&r1, "b").phase, Phase::Interrupted);
    assert!(restart_marked(&home, LATER));
}

/// F3: a stale pane marker whose order is only doubted is told once, not at every minute's pass.
#[test]
fn restart_a_stale_marker_doubt_is_told_once() {
    let cwd = temp_dir("restart-stale-doubt-cwd");
    let project = project("restart-stale-doubt", &["o"], &[], &cwd);
    let live = project.run_dir("r").join("live");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::write(live.join("o.pane"), "wP:p2").unwrap();
    let home = temp_dir("restart-stale-doubt-home");
    let world = World::new(|_| false, |_| PidSeen::Unknown);
    assert_eq!(world.check(&home, &[project.root.clone()], None, LATER).len(), 1);
    for minute in 1..4 {
        let lines = world.check(&home, &[project.root.clone()], None, LATER + 60 * minute);
        assert!(lines.is_empty(), "minute {minute}: {lines:?}");
    }
    assert_eq!(doubts_in(&project, "r"), 1);
    assert_eq!(world.woken.borrow().len(), 1);
    assert_eq!(order_in(&state(&project), "o").phase, Phase::Running);
    assert!(live.join("o.pane").exists(), "a running order keeps its marker");
}

/// F4: auto mode defers on its own up to `AUTO_DEFER_CAP_S` and asks, naming the reset, past it.
#[test]
fn rate_decision_a_deferral_past_the_cap_is_asked() {
    let now = 1_794_607_762;
    assert_eq!(AUTO_DEFER_CAP_S, 6 * 3600);
    let at_cap = now + AUTO_DEFER_CAP_S;
    assert_eq!(
        rate_decision(now, None, 30 * 60, &Err(Some(at_cap)), true),
        RateDecision::Defer {
            until: at_cap + RESET_MARGIN_S,
            why: format!(
                "no other seat has room; auto mode waits for the earliest reset, {}",
                utc(at_cap)
            ),
        }
    );
    assert_eq!(
        rate_decision(now, None, 30 * 60, &Err(Some(at_cap + 1)), true),
        RateDecision::Ask {
            why: format!(
                "no other seat has room and the earliest reset, {}, is more than 6 h away",
                utc(at_cap + 1)
            ),
        }
    );
    // The earliest of the spent pool's own reset and the others' is what is capped: ia1's 79 h agy reset with max's
    // 31 min away still waits for max.
    let max = now + 31 * 60;
    assert!(matches!(
        rate_decision(now, Some(now + 79 * 3600), 30 * 60, &Err(Some(max)), true),
        RateDecision::Defer { until, .. } if until == max + RESET_MARGIN_S
    ));
}

/// F5: the checkpoint reads the worktree without taking its index lock: a stat-dirty file is not refreshed into the
/// index, as a plain `git status` would.
#[test]
fn restart_write_diff_leaves_the_worktrees_index_as_it_was() {
    let tree = temp_dir("restart-index-tree");
    git(&tree, &["init", "-q"]);
    std::fs::write(tree.join("a.txt"), "one\n").unwrap();
    git(&tree, &["add", "a.txt"]);
    git(&tree, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "base"]);
    // Same bytes, a new mtime: the index's stat data is stale, which a refresh would rewrite.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(tree.join("a.txt"))
        .unwrap();
    file.set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000))
        .unwrap();
    drop(file);
    let index = tree.join(".git").join("index");
    let before = std::fs::read(&index).unwrap();
    let run_dir = temp_dir("restart-index-run");
    let (diff, _) = crate::nirvana_worker::write_diff(&run_dir, "o", "restart", &tree);
    assert_eq!(diff.as_deref(), Some("checkpoints/o-restart.diff"));
    assert!(std::fs::read(&index).unwrap() == before, "the index was rewritten");
    // The test's own control: the same read with optional locks does rewrite it, so the assertion above can fail.
    let out = std::process::Command::new("git")
        .args(["status", "--porcelain=v1"])
        .current_dir(&tree)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(std::fs::read(&index).unwrap() != before, "plain git status refreshes the index");
}

/// F6 b: the parent's `ended` does not make its pane read finished while its supervisor still runs (it launches the
/// child from inside `rate_limited`); once it has exited the pane is finished, which it is. The child's pane, with no
/// `started` yet, holds no finished worker, and the parent's phase is `reseated` with or without the `ended`.
#[test]
fn reseat_events_leave_no_pane_finished_early() {
    let started = |order: &str, pid: u32, pane: &str| Kind::Started {
        order: order.into(),
        attempt: 1,
        pid: Some(pid),
        pane: Some(pane.into()),
        tab: None,
        resume: None,
        argv: Vec::new(),
    };
    let mut events = vec![
        ev(1, T0, opened(Some("wP:p1"))),
        ev(2, T0 + 1, Kind::Seated(Box::new(seated("o")))),
        ev(3, T0 + 2, started("o", 4242, "wP:p2")),
    ];
    let mut child = seated("o-r1");
    child.parent = Some("o".into());
    let reseat = reseat_events(child, "o", "sonnet", "agy high ran out (agy_exit_3)");
    for (seq, kind) in (4..).zip(reseat.iter().take(2).cloned()) {
        events.push(ev(seq, T0 + 3, kind));
    }
    let without_end = replay(&events).order("o").unwrap().phase;
    events.push(ev(6, T0 + 3, reseat[2].clone()));
    assert_eq!(replay(&events).order("o").unwrap().phase, without_end);
    assert_eq!(without_end, Phase::Reseated);
    let runs = vec![("r".to_owned(), events.clone())];
    // The parent's supervisor still holds its lock while it launches the child.
    let finished = |runs: &[(String, Vec<Event>)], pane: &str, live: bool| {
        crate::nirvana_worker::finished_worker_in(runs, pane, |_, order| live && order == "o")
    };
    assert_eq!(finished(&runs, "wP:p2", true), None);
    let after = finished(&runs, "wP:p2", false).expect("finished once its supervisor exited");
    assert!(after.contains("ended reseated"), "{after}");
    assert_eq!(finished(&runs, "wP:p3", false), None);
    events.push(ev(7, T0 + 4, started("o-r1", 4343, "wP:p3")));
    let runs = vec![("r".to_owned(), events)];
    assert_eq!(finished(&runs, "wP:p3", false), None);
}

/// A real supervisor stand-in: a PowerShell process that holds the order's lock open with no sharing, as `take_live`
/// does, until it is killed. Waits until `live` sees it.
#[cfg(windows)]
fn hold_lock(project: &Project, run: &str, order: &str) -> std::process::Child {
    let dir = project.run_dir(run).join("live");
    std::fs::create_dir_all(&dir).unwrap();
    let lock = dir.join(format!("{order}.lock"));
    let script = format!(
        "$f = [System.IO.File]::Open('{}', 'OpenOrCreate', 'ReadWrite', 'None'); Start-Sleep -Seconds 120",
        lock.display()
    );
    let mut child = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("powershell");
    let until = now() + 30;
    while !crate::nirvana_worker::live(project, run, order) {
        if now() >= until {
            let _ = child.kill();
            panic!("the stand-in never took the lock");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    child
}

/// F7: end to end with the OS's own answers. A lock held by a real process and a worker pid that runs: the order
/// reads live and nothing is written. The process killed (the OS lets go of its lock, its pid ends): the next pass
/// reads the marker it left and interrupts the order with a checkpoint of its worktree.
#[cfg(windows)]
#[test]
fn restart_a_real_lock_and_a_real_worker_are_read_by_the_os() {
    let tree = temp_dir("restart-real-tree");
    git(&tree, &["init", "-q"]);
    git(&tree, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "base"]);
    std::fs::write(tree.join("work.txt"), "real work\n").unwrap();
    let project = Project::at(temp_dir("restart-real"));
    let journal = project.journal("r");
    journal.append(T0, opened(Some("wP:p1"))).unwrap();
    let mut seat = seated("o");
    seat.cwd = tree.display().to_string();
    journal.append(now(), Kind::Seated(Box::new(seat))).unwrap();
    let mut supervisor = hold_lock(&project, "r", "o");
    // The worker is the stand-in itself: its pid was created before the `started` below.
    journal
        .append(
            now(),
            Kind::Started {
                order: "o".into(),
                attempt: 1,
                pid: Some(supervisor.id()),
                pane: Some("wP:p2".into()),
                tab: None,
                resume: None,
                argv: Vec::new(),
            },
        )
        .unwrap();
    let live_dir = project.run_dir("r").join("live");
    std::fs::write(live_dir.join("o.pane"), "wP:p2").unwrap();
    let home = temp_dir("restart-real-home");
    let woken = RefCell::new(Vec::new());
    let lock_live =
        |project: &Project, run: &str, order: &str| crate::nirvana_worker::live(project, run, order);
    let wake = |_: &Project, run: &str| woken.borrow_mut().push(run.to_owned());
    let world = Liveliness {
        lock_live: &lock_live,
        lock_proves_free: lock_reads_free(),
        pid_seen: &pid_seen,
        wake: &wake,
    };
    let events = journal.read().0.len();
    let lines = restart_check(&home, &[project.root.clone()], Some(now()), false, &world, now());
    let held = journal.read().0.len();
    let phase = order_in(&state(&project), "o").phase;
    let _ = supervisor.kill();
    supervisor.wait().expect("wait");
    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(held, events);
    assert_eq!(phase, Phase::Running);
    assert!(!crate::nirvana_worker::live(&project, "r", "o"), "the OS let go of the lock");
    // The every-minute pass sees the marker its dead supervisor left.
    let lines = restart_check(&home, &[project.root.clone()], None, false, &world, now());
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("is gone"), "{lines:?}");
    let order = order_in(&state(&project), "o");
    assert_eq!(order.phase, Phase::Interrupted);
    assert_eq!(order.last_checkpoint.as_deref(), Some("checkpoints/o-restart.diff"));
    let diff =
        std::fs::read_to_string(project.run_dir("r").join("checkpoints/o-restart.diff")).unwrap();
    assert!(diff.contains("work.txt"), "{diff}");
    assert_eq!(*woken.borrow(), vec!["r".to_owned()]);
    assert!(!live_dir.join("o.pane").exists());
}
