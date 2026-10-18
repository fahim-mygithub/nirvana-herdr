use super::*;

/// 2026-10-18T02:08:38Z, when the live run `live1` seated its first order.
const T0: u64 = 1_792_289_318;

/// Claude Code 2.1.302's screen in a herdr pane, read with `pane read --format ansi` on 2026-10-18: the empty
/// prompt with its faint placeholder, and the same prompt after `pane send-text "hello owner draft"`.
const SCREEN_EMPTY: &str = include_str!("fixtures/claude-2.1.302-empty.ansi");
const SCREEN_TYPED: &str = include_str!("fixtures/claude-2.1.302-typed.ansi");

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "herdr-o3-run-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn seated(order: &str) -> Seated {
    Seated {
        order: order.into(),
        seat: "haiku".into(),
        cli: "claude".into(),
        model: "claude-haiku-5-5".into(),
        effort: Effort::Low,
        pool: "max".into(),
        class: "chore.script".into(),
        chain: Some("chore".into()),
        size: SizeName::Small,
        mode: Mode::Balanced,
        needs: Vec::new(),
        not_family: Vec::new(),
        reason: "chore.script → haiku low: max 5h 28% used".into(),
        override_reason: None,
        order_file: format!("C:/p/.nirvana/orders/{order}.md"),
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
    }
}

fn opened(pane: Option<&str>) -> Kind {
    Kind::Opened {
        project: "C:/p".into(),
        orchestrator_pane: pane.map(str::to_owned),
        auto: true,
        pools: Vec::new(),
        pools_basis: None,
    }
}

fn ev(seq: u64, at: u64, kind: Kind) -> Event {
    Event {
        v: V,
        seq,
        at,
        kind,
    }
}

fn tokens(input: u64, output: u64) -> Tokens {
    Tokens {
        input,
        cache_write: 10,
        cache_read: 100,
        output,
    }
}

fn report(status: &str) -> Value {
    serde_json::json!({"status": status, "summary": "added sub", "files": ["calc.py"], "tests": [], "gotchas": []})
}

/// A run with one order seated, started, streamed, delivered and gated.
fn delivered_run() -> Vec<Event> {
    vec![
        ev(1, T0, opened(Some("w1:p2"))),
        ev(2, T0, Kind::Seated(Box::new(seated("sub")))),
        ev(
            3,
            T0 + 1,
            Kind::Started {
                order: "sub".into(),
                attempt: 1,
                pid: Some(4),
                pane: Some("w1:p3".into()),
                tab: Some("w1:t3".into()),
                resume: None,
                argv: vec!["claude".into()],
            },
        ),
        ev(
            4,
            T0 + 10,
            Kind::Usage {
                order: Some("sub".into()),
                attempt: 1,
                session: Some("c0a1".into()),
                tokens: tokens(8, 70),
            },
        ),
        ev(
            5,
            T0 + 18,
            Kind::Usage {
                order: Some("sub".into()),
                attempt: 1,
                session: Some("c0a1".into()),
                tokens: tokens(16, 1513),
            },
        ),
        ev(
            6,
            T0 + 18,
            Kind::Delivered {
                order: "sub".into(),
                report: report("done"),
            },
        ),
        ev(
            7,
            T0 + 18,
            Kind::Ended {
                order: Some("sub".into()),
                outcome: "delivered".into(),
                why: None,
                exit: Some(0),
                pools: Vec::new(),
                pools_basis: None,
            },
        ),
    ]
}

// ---- the journal and its replay ----

#[test]
fn replaying_the_journal_rebuilds_the_run() {
    let dir = temp_dir("replay");
    let journal = Journal::new(dir.join(JOURNAL));
    for event in delivered_run() {
        journal.append(event.at, event.kind).expect("append");
    }
    let (events, bad) = journal.read();
    assert_eq!(bad, 0);
    assert_eq!(
        events.iter().map(|e| e.seq).collect::<Vec<_>>(),
        (1..=7).collect::<Vec<_>>()
    );
    let state = replay(&events);
    let order = state.order("sub").expect("sub");
    assert_eq!(order.phase, Phase::Delivered);
    assert_eq!(order.attempts, 1);
    assert_eq!(order.session.as_deref(), Some("c0a1"));
    assert_eq!(order.pane.as_deref(), Some("w1:p3"));
    // The last cumulative figure of the attempt, not the sum of both usage events.
    assert_eq!(order.tokens(), Some(tokens(16, 1513)));
    assert_eq!(
        state
            .opened
            .as_ref()
            .and_then(|o| o.orchestrator_pane.as_deref()),
        Some("w1:p2")
    );
    // The same journal replayed again is the same run.
    assert_eq!(replay(&journal.read().0), state);
    // Gated: the order is finished.
    journal
        .append(
            T0 + 40,
            Kind::Gated {
                order: "sub".into(),
                verdict: "pass".into(),
                reason: None,
            },
        )
        .expect("gate");
    let state = replay(&journal.read().0);
    assert_eq!(state.order("sub").map(|o| o.phase), Some(Phase::Gated));
    assert_eq!(state.order("sub").map(|o| o.last_at), Some(T0 + 40));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_line_that_does_not_parse_is_counted_kept_and_numbered() {
    let dir = temp_dir("bad");
    let path = dir.join(JOURNAL);
    std::fs::write(
        &path,
        "{\"v\":1,\"seq\":1,\"at\":5,\"event\":\"seated\"}\nnot json\n{\"v\":2,\"seq\":3,\"at\":6,\"event\":\"later_kind\",\"x\":1}\n",
    )
    .expect("write");
    let journal = Journal::new(path.clone());
    let (events, bad) = journal.read();
    // A seated line with no fields and "not json" do not parse; a later kind parses as Unknown and is skipped.
    assert_eq!(bad, 2);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, Kind::Unknown);
    let event = journal
        .append(9, Kind::Seated(Box::new(seated("o1"))))
        .expect("append");
    assert_eq!(
        event.seq, 4,
        "the bad lines count toward seq, so no number repeats"
    );
    assert!(std::fs::read_to_string(&path)
        .expect("read")
        .contains("not json"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn appends_from_many_threads_lose_no_line() {
    let dir = temp_dir("threads");
    let journal = Journal::new(dir.join(JOURNAL));
    let handles: Vec<_> = (0..8)
        .map(|n| {
            let journal = journal.clone();
            std::thread::spawn(move || {
                for k in 0..5 {
                    journal
                        .append(
                            T0,
                            Kind::Asked {
                                order: None,
                                question: format!("{n}-{k}"),
                            },
                        )
                        .expect("append");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("thread");
    }
    let (events, bad) = journal.read();
    assert_eq!(bad, 0);
    assert_eq!(events.len(), 40);
    let mut seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    seqs.sort_unstable();
    assert_eq!(seqs, (1..=40).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_project_folder_ignores_runs_orders_and_worktrees_but_not_lessons() {
    let dir = temp_dir("ignore");
    let project = Project::at(&dir);
    project.ensure().expect("ensure");
    let ignore = std::fs::read_to_string(project.dir().join(".gitignore")).expect("gitignore");
    let rules: Vec<&str> = ignore
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(rules, ["runs/", "orders/", "worktrees/"]);
    assert!(!rules.iter().any(|rule| rule.contains("lessons")));
    // The owner's edit is kept.
    std::fs::write(project.dir().join(".gitignore"), "mine\n").expect("edit");
    project.ensure().expect("ensure again");
    assert_eq!(
        std::fs::read_to_string(project.dir().join(".gitignore")).expect("read"),
        "mine\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn ids_are_safe_to_type_into_a_shell() {
    for good in ["sub", "o1", "run-20261018-020838", "a.b_c-d"] {
        assert!(valid_id(good), "{good}");
    }
    for bad in [
        "",
        "-x",
        "a b",
        "a;b",
        "a&b",
        "a/b",
        "a\\b",
        "$(x)",
        &"x".repeat(65),
    ] {
        assert!(!valid_id(bad), "{bad}");
    }
}

// ---- §5.6: defer, re-seat or ask ----

#[test]
fn a_window_that_resets_within_30_minutes_is_waited_for() {
    let decision = rate_decision(T0, Some(T0 + 1800), 1800, &Ok("flash".into()), true);
    assert_eq!(
        decision,
        RateDecision::Defer {
            until: T0 + 1800 + 60,
            why: decision_why(&decision)
        }
    );
    // A minute past the line, a seat that has room takes it instead.
    assert_eq!(
        rate_decision(T0, Some(T0 + 1801), 1800, &Ok("flash".into()), true),
        RateDecision::Reseat {
            seat: "flash".into()
        }
    );
}

fn decision_why(decision: &RateDecision) -> String {
    match decision {
        RateDecision::Defer { why, .. } | RateDecision::Ask { why } => why.clone(),
        RateDecision::Reseat { seat } => seat.clone(),
    }
}

#[test]
fn with_no_seat_left_auto_mode_waits_for_the_earliest_reset_and_otherwise_asks() {
    // The spent window resets in 3 h; the soonest skipped seat's window in 2 h.
    let none: ReseatPick = Err(Some(T0 + 7200));
    match rate_decision(T0, Some(T0 + 10_800), 1800, &none, true) {
        RateDecision::Defer { until, .. } => assert_eq!(until, T0 + 7200 + 60),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        rate_decision(T0, Some(T0 + 10_800), 1800, &none, false),
        RateDecision::Ask { .. }
    ));
    // No reset known anywhere: even auto mode asks, nothing is dropped.
    assert!(matches!(
        rate_decision(T0, None, 1800, &Err(None), true),
        RateDecision::Ask { .. }
    ));
    // A reset the signal did not give, and a seat with room: re-seat.
    assert_eq!(
        rate_decision(T0, None, 1800, &Ok("argon".into()), false),
        RateDecision::Reseat {
            seat: "argon".into()
        }
    );
}

// ---- §7.2: stalls ----

#[test]
fn a_stream_silent_for_ten_minutes_is_stalled_and_restarts_back_off_to_a_refusal() {
    assert!(!stalled(T0, T0 + 599));
    assert!(stalled(T0, T0 + 600));
    assert_eq!(
        (0..5).map(restart_delay).collect::<Vec<_>>(),
        [Some(0), Some(30), Some(60), Some(120), None]
    );
}

#[test]
fn a_refused_restart_fails_the_order_and_asks() {
    let mut events = delivered_run()[..3].to_vec();
    events.push(ev(
        4,
        T0 + 700,
        Kind::Stalled {
            order: "sub".into(),
            silent_s: 600,
            stalls: 5,
            next: "refused".into(),
        },
    ));
    let state = replay(&events);
    let order = state.order("sub").expect("sub");
    assert_eq!(order.phase, Phase::Failed);
    assert_eq!(order.stalls, 5);
    assert_eq!(order.outcome.as_deref(), Some("crash_loop"));
}

// ---- §7.2: waking ----

fn idle_view() -> PaneView {
    PaneView {
        status: Some("idle".into()),
        prompt: Some(String::new()),
        last_key_at: None,
        program: PaneProgram::NirvanaClaude,
    }
}

/// `herdr pane get` and `herdr pane process-info --pane` of four panes in an isolated herdr server, recorded
/// 2026-10-18 on this PC: Nirvana Claude started with O4a's argv, plain `claude`, the pane's pwsh at its
/// prompt, and `agy`. Each Claude and agy sat on its folder-trust screen in a scratch repository.
const PANES: [(&str, &str, &str); 4] = [
    (
        "nirvana-claude",
        include_str!("fixtures/panes/nirvana-claude.get.json"),
        include_str!("fixtures/panes/nirvana-claude.process.json"),
    ),
    (
        "claude-code",
        include_str!("fixtures/panes/claude-code.get.json"),
        include_str!("fixtures/panes/claude-code.process.json"),
    ),
    (
        "shell",
        include_str!("fixtures/panes/shell.get.json"),
        include_str!("fixtures/panes/shell.process.json"),
    ),
    (
        "agy",
        include_str!("fixtures/panes/agy.get.json"),
        include_str!("fixtures/panes/agy.process.json"),
    ),
];
/// The `NIRVANA_HERDR_HOME` of that server: its Nirvana Claude's `--plugin-dir` is `<home>\nirvana\claude`.
const RECORDED_HOME: &str =
    "C:\\Users\\fahim\\Desktop\\Pojects\\Nirvana-Herdr-o3\\target\\o3-merge-live\\home";

fn recorded_program(name: &str, home: &str) -> PaneProgram {
    let (_, get, process) = PANES
        .iter()
        .find(|(pane, _, _)| *pane == name)
        .expect("a recorded pane");
    let plugin = crate::nirvana_launch::plugin_dir(Path::new(home));
    pane_program(
        &serde_json::from_str(get).expect("pane get"),
        &serde_json::from_str(process).expect("process-info"),
        Some(&plugin),
    )
}

#[test]
fn a_wake_types_only_into_a_pane_that_runs_nirvana_claude() {
    assert_eq!(
        recorded_program("nirvana-claude", RECORDED_HOME),
        PaneProgram::NirvanaClaude
    );
    // The 2026-10-18 incident's pane: a plain Claude Code is `claude` to herdr too.
    for other in ["claude-code", "shell", "agy"] {
        let program = recorded_program(other, RECORDED_HOME);
        assert!(
            matches!(program, PaneProgram::Other(_)),
            "{other}: {program:?}"
        );
    }
    // Another home's plugin is not this herdr's Nirvana Claude, and the folder's case does not matter.
    assert!(matches!(
        recorded_program("nirvana-claude", "C:\\Users\\fahim\\elsewhere"),
        PaneProgram::Other(_)
    ));
    assert_eq!(
        recorded_program("nirvana-claude", &RECORDED_HOME.to_lowercase()),
        PaneProgram::NirvanaClaude
    );
    // Nirvana Claude's argv in a pane whose agent herdr no longer reads (its process just exited, and
    // process-info answered from a cached snapshot) is not typed into.
    let plugin = crate::nirvana_launch::plugin_dir(Path::new(RECORDED_HOME));
    let program = pane_program(
        &serde_json::from_str(PANES[2].1).expect("shell pane get"),
        &serde_json::from_str(PANES[0].2).expect("nirvana process-info"),
        Some(&plugin),
    );
    assert_eq!(
        program,
        PaneProgram::Other("herdr reads its agent as none".into())
    );
    // No home, no plugin to match: refused.
    assert!(matches!(
        pane_program(
            &serde_json::from_str(PANES[0].1).expect("pane get"),
            &serde_json::from_str(PANES[0].2).expect("process-info"),
            None,
        ),
        PaneProgram::Other(_)
    ));

    // The wake queues the event for every pane but Nirvana Claude's, an urgent stop included.
    let state = replay(&delivered_run());
    let now = T0 + 100;
    for (name, _, _) in PANES {
        let mut view = idle_view();
        view.program = recorded_program(name, RECORDED_HOME);
        for urgent in [false, true] {
            let verdict = wake_decision("live1", &state, Some(&view), now, urgent);
            if name == "nirvana-claude" {
                assert!(matches!(verdict, Wake::Type(_)), "{name}: {verdict:?}");
            } else {
                assert!(matches!(verdict, Wake::Queue(_)), "{name}: {verdict:?}");
            }
        }
    }
    let mut unread = idle_view();
    unread.program = PaneProgram::Unknown;
    assert_eq!(
        wake_decision("live1", &state, Some(&unread), now, true),
        Wake::Queue(
            "the orchestrator's pane does not run Nirvana Claude: herdr could not say what runs in it".into()
        )
    );

    // And the typing itself reads the pane again: nothing is sent to a pane that is not Nirvana Claude.
    let mut sent = 0;
    let refused = type_if_nirvana(&recorded_program("claude-code", RECORDED_HOME), || {
        sent += 1;
        Ok(())
    });
    assert!(refused.is_err());
    assert_eq!(sent, 0);
    assert_eq!(
        type_if_nirvana(&PaneProgram::NirvanaClaude, || {
            sent += 1;
            Ok(())
        }),
        Ok(())
    );
    assert_eq!(sent, 1);
}

#[test]
fn a_wake_types_only_into_an_idle_orchestrator_with_an_empty_prompt() {
    let state = replay(&delivered_run());
    assert_eq!(state.queue.len(), 1);
    let now = T0 + 100;
    match wake_decision("live1", &state, Some(&idle_view()), now, false) {
        Wake::Type(digest) => {
            assert!(digest.starts_with("herdr run live1: sub delivered done"));
            assert!(digest.chars().count() <= 400);
        }
        other => panic!("{other:?}"),
    }
    let mut busy = idle_view();
    busy.status = Some("working".into());
    assert!(matches!(
        wake_decision("live1", &state, Some(&busy), now, false),
        Wake::Queue(_)
    ));
    // The owner's urgent stop steers a working orchestrator.
    assert!(matches!(
        wake_decision("live1", &state, Some(&busy), now, true),
        Wake::Type(_)
    ));
    let mut blocked = idle_view();
    blocked.status = Some("blocked".into());
    assert!(matches!(
        wake_decision("live1", &state, Some(&blocked), now, true),
        Wake::Queue(_)
    ));
    assert!(matches!(
        wake_decision("live1", &state, None, now, false),
        Wake::Queue(_)
    ));
}

#[test]
fn a_wake_never_types_while_the_owner_does() {
    let state = replay(&delivered_run());
    let now = T0 + 100;
    let mut typing = idle_view();
    typing.last_key_at = Some(now - 29);
    assert_eq!(
        wake_decision("live1", &state, Some(&typing), now, true),
        Wake::Queue("the owner typed in the pane 29 s ago".into())
    );
    typing.last_key_at = Some(now - 30);
    assert!(matches!(
        wake_decision("live1", &state, Some(&typing), now, false),
        Wake::Type(_)
    ));
    let mut draft = idle_view();
    draft.prompt = Some("hello owner draft".into());
    assert_eq!(
        wake_decision("live1", &state, Some(&draft), now, true),
        Wake::Queue("the prompt line holds text".into())
    );
    let mut unseen = idle_view();
    unseen.prompt = None;
    assert!(matches!(
        wake_decision("live1", &state, Some(&unseen), now, false),
        Wake::Queue(_)
    ));
}

#[test]
fn the_prompt_line_is_read_from_claude_codes_own_screen() {
    // Recorded: the faint placeholder is not the owner's text.
    assert_eq!(prompt_line_text(SCREEN_EMPTY), Some(String::new()));
    assert_eq!(
        prompt_line_text(SCREEN_TYPED),
        Some("hello owner draft".to_owned())
    );
    // A shell, with no prompt box.
    assert_eq!(
        prompt_line_text("PS C:\\p> herdr seat exec sub --run live1\nsub · haiku\n"),
        None
    );
    // The older boxed prompt.
    assert_eq!(
        prompt_line_text("╭────────────╮\n│ > fix the bug   │\n╰────────────╯\n"),
        Some("fix the bug".to_owned())
    );
    // Text in a grey true colour is a placeholder too.
    assert_eq!(
        prompt_line_text(
            "──────────────\n❯\u{a0}\u{1b}[38;2;120;120;120mTry it\u{1b}[0m\n──────────────\n"
        ),
        Some(String::new())
    );
    // A prompt that wraps onto a second line inside the box is read whole.
    assert_eq!(
        prompt_line_text(
            "──────────────\n❯\u{a0}first line\n  second line\n──────────────\n  ⏵⏵ auto mode on\n"
        ),
        Some("first line\n  second line".to_owned())
    );
}

#[test]
fn nothing_queued_is_nothing_to_wake_for_and_a_notice_clears_the_queue() {
    let mut events = delivered_run();
    events.push(ev(
        8,
        T0 + 20,
        Kind::Woken {
            via: "notice".into(),
            through: 7,
            digest: String::new(),
        },
    ));
    let state = replay(&events);
    assert!(state.queue.is_empty());
    assert_eq!(
        wake_decision("live1", &state, Some(&idle_view()), T0 + 100, false),
        Wake::Nothing
    );
    // A notice is the owner's prompt hook, not a wake: it does not count toward the cap.
    assert_eq!(state.unproductive_wakes, 0);
}

fn ask(seq: u64, question: &str) -> Event {
    ev(
        seq,
        T0 + seq,
        Kind::Asked {
            order: None,
            question: question.into(),
        },
    )
}

fn woken(seq: u64) -> Event {
    ev(
        seq,
        T0 + seq,
        Kind::Woken {
            via: "prompt".into(),
            through: seq - 1,
            digest: String::new(),
        },
    )
}

#[test]
fn three_wakes_in_a_row_that_move_no_order_forward_stop_the_waking() {
    let mut events = delivered_run();
    // Each wake answers a new event, and the orchestrator moves nothing.
    events.extend([ask(8, "a"), woken(9), ask(10, "b"), woken(11), ask(12, "c")]);
    let state = replay(&events);
    assert_eq!(state.unproductive_wakes, 2);
    assert!(matches!(
        wake_decision("live1", &state, Some(&idle_view()), T0 + 100, false),
        Wake::Type(_)
    ));
    events.extend([woken(13), ask(14, "d")]);
    let state = replay(&events);
    assert_eq!(state.unproductive_wakes, 3);
    assert_eq!(
        wake_decision("live1", &state, Some(&idle_view()), T0 + 100, false),
        Wake::Refuse(CAP_QUESTION.into())
    );
    // A gate is the orchestrator moving an order: the count starts again.
    events.push(ev(
        15,
        T0 + 15,
        Kind::Gated {
            order: "sub".into(),
            verdict: "pass".into(),
            reason: None,
        },
    ));
    events.push(ask(16, "e"));
    let state = replay(&events);
    assert_eq!(state.unproductive_wakes, 0);
    assert!(matches!(
        wake_decision("live1", &state, Some(&idle_view()), T0 + 100, false),
        Wake::Type(_)
    ));
}

// ---- §7.2: no dead end, resume ----

#[test]
fn an_unfinished_run_with_no_live_worker_and_no_next_step_is_a_dead_end() {
    let never = |_: &str| false;
    let always = |_: &str| true;
    // Seated and started, its worker gone, nothing queued: stuck.
    let started = replay(&delivered_run()[..3]);
    assert!(check(&started, &always, T0).is_ok());
    let stuck = check(&started, &never, T0).expect_err("dead end");
    assert!(stuck.contains("herdr run resume"), "{stuck}");
    // Delivered: the queue holds it for the orchestrator, then the gate does.
    let mut events = delivered_run();
    assert!(check(&replay(&events), &never, T0)
        .expect("queued")
        .contains("herdr run notice"));
    events.push(ev(
        8,
        T0 + 20,
        Kind::Woken {
            via: "notice".into(),
            through: 7,
            digest: String::new(),
        },
    ));
    assert!(check(&replay(&events), &never, T0)
        .expect("gate")
        .contains("waits for the gate"));
    // Gated, the run open: the orchestrator ends it or seats the next order.
    events.push(ev(
        9,
        T0 + 30,
        Kind::Gated {
            order: "sub".into(),
            verdict: "pass".into(),
            reason: None,
        },
    ));
    let all_done = check(&replay(&events), &never, T0).expect_err("dead end");
    assert!(all_done.contains("herdr run end"), "{all_done}");
    events.push(ev(
        10,
        T0 + 31,
        Kind::Ended {
            order: None,
            outcome: "ended".into(),
            why: None,
            exit: None,
            pools: Vec::new(),
            pools_basis: None,
        },
    ));
    assert!(check(&replay(&events), &never, T0).is_ok());
    // A deferral is a pending step.
    let mut deferred = delivered_run()[..3].to_vec();
    deferred.push(ev(
        4,
        T0 + 5,
        Kind::Deferred {
            order: "sub".into(),
            until: T0 + 900,
            why: "reset".into(),
        },
    ));
    deferred.push(ev(
        5,
        T0 + 6,
        Kind::Woken {
            via: "notice".into(),
            through: 4,
            digest: String::new(),
        },
    ));
    assert!(check(&replay(&deferred), &never, T0)
        .expect("deferred")
        .contains("deferred"));
    assert!(check(&RunState::default(), &never, T0).is_err());
}

#[test]
fn resume_reports_each_interrupted_step_and_never_a_finished_or_live_one() {
    let mut events = delivered_run();
    let mut second = seated("mul");
    second.seat = "flash".into();
    events.push(ev(8, T0 + 30, Kind::Seated(Box::new(second))));
    events.push(ev(9, T0 + 31, Kind::Seated(Box::new(seated("div")))));
    let state = replay(&events);
    let live = |order: &str| order == "div";
    assert_eq!(interrupted_steps(&state, &live), ["mul"]);
    events.push(ev(
        10,
        T0 + 40,
        Kind::Interrupted {
            order: "mul".into(),
            why: "no live worker".into(),
        },
    ));
    let state = replay(&events);
    assert_eq!(
        state.order("mul").map(|o| o.phase),
        Some(Phase::Interrupted)
    );
    assert!(interrupted_steps(&state, &live).is_empty());
}

// ---- §7.4.1: the receipt ----

fn record_for(order: &str) -> RecordLine {
    serde_json::from_value(serde_json::json!({
        "order": order, "run": "live1", "seat": "haiku", "effort": "low", "class": "chore.script",
        "outcome": "pass", "use": {"5h": 0.0002166486, "week": 0.000027081074}
    }))
    .expect("record line")
}

#[test]
fn the_receipt_is_the_same_bytes_twice_and_changes_with_one_appended_event() {
    let events = delivered_run();
    let record = vec![record_for("sub")];
    let render = |events: &[Event]| {
        let receipt = receipt("live1", &replay(events), 0, &record);
        (
            serde_json::to_string_pretty(&receipt).expect("json"),
            receipt_text(&receipt),
        )
    };
    let first = render(&events);
    assert_eq!(first, render(&events));
    let mut more = events.clone();
    more.push(ev(
        8,
        T0 + 60,
        Kind::Gated {
            order: "sub".into(),
            verdict: "pass".into(),
            reason: None,
        },
    ));
    let after = render(&more);
    assert_ne!(first.0, after.0);
    assert_ne!(first.1, after.1);
    // No clock: the duration runs to the order's last event.
    assert!(after.1.contains("duration: 60 s"), "{}", after.1);
}

#[test]
fn a_missing_figure_says_why_and_is_never_zero() {
    let mut events = delivered_run()[..2].to_vec();
    events.push(ev(3, T0 + 5, Kind::Seated(Box::new(seated("mul")))));
    let partial = receipt("live1", &replay(&events), 0, &[]);
    let json = serde_json::to_value(&partial).expect("json");
    let order = &json["orders"][0];
    assert_eq!(order["tokens"]["missing"], "no worker started");
    assert!(order["use"]["missing"]
        .as_str()
        .is_some_and(|why| why.contains("not ended")));
    assert!(order["verdict"]["missing"].is_string());
    assert!(json["orchestrator"]["missing"].is_string());
    assert!(json["workers"]["missing"].is_string());
    assert!(json["pools_end"]["readings"]["missing"]
        .as_str()
        .is_some_and(|why| why.contains("not ended")));
    let text = receipt_text(&partial);
    assert!(
        text.contains("tokens: missing: no worker started"),
        "{text}"
    );
    assert!(!text.contains("in 0 ·"), "{text}");
    // With the record and the stream, the figures are known, each with its basis.
    let full = receipt("live1", &replay(&delivered_run()), 0, &[record_for("sub")]);
    let json = serde_json::to_value(&full).expect("json");
    assert_eq!(json["orders"][0]["tokens"]["output"], 1513);
    assert_eq!(json["orders"][0]["use"]["5h"], 0.000217);
    assert!(json["orders"][0]["use_basis"]
        .as_str()
        .is_some_and(|basis| basis.contains("record.jsonl")));
}

#[test]
fn the_record_line_carries_the_runs_outcome_and_old_lines_still_parse() {
    let mut events = delivered_run();
    events.push(ev(
        8,
        T0 + 60,
        Kind::Gated {
            order: "sub".into(),
            verdict: "pass".into(),
            reason: None,
        },
    ));
    let state = replay(&events);
    let line = record_line(
        "live1",
        state.order("sub").expect("sub"),
        Some("j.jsonl#8".into()),
    );
    assert_eq!(line.v, Some(1));
    assert_eq!(line.outcome, "pass");
    assert_eq!(line.pool.as_deref(), Some("max"));
    assert_eq!(line.seated_at, Some(T0));
    assert_eq!(line.ended_at, Some(T0 + 60));
    assert_eq!(line.tokens, Some(tokens(16, 1513)));
    assert_eq!(
        (
            line.send_backs,
            line.reseats,
            line.deferrals,
            line.rate_limit_stops,
            line.stalls
        ),
        (Some(0), Some(0), Some(0), Some(0), Some(0))
    );
    // O2's line, as `herdr seat record` wrote it before O3: it parses, and writes back without the new keys.
    let old = r#"{"order":"o1","seat":"sonnet","model":"claude-sonnet-5-5","effort":"medium","class":"build.bounded","chain":"build.bounded","outcome":"pass","tokens":null,"use":null,"at":5}"#;
    let parsed: RecordLine = serde_json::from_str(old).expect("old line");
    assert_eq!(parsed.v, None);
    assert_eq!(serde_json::to_string(&parsed).expect("json"), old);
    // The journal reference, the pick's mode and size, and the build that wrote it.
    assert_eq!(line.journal.as_deref(), Some("j.jsonl#8"));
    assert_eq!(
        (line.mode, line.size),
        (Some(Mode::Balanced), Some(SizeName::Small))
    );
    assert_eq!(line.herdr.as_deref(), Some(crate::nirvana::VERSION));
    // A re-seated order's line says so; the child's carries its own.
    let mut moved = delivered_run()[..3].to_vec();
    moved.push(ev(
        4,
        T0 + 9,
        Kind::Reseated {
            order: "sub".into(),
            child: "sub-r1".into(),
            seat: "flash".into(),
            why: "max ran out".into(),
        },
    ));
    let state = replay(&moved);
    let line = record_line("live1", state.order("sub").expect("sub"), None);
    assert_eq!((line.outcome.as_str(), line.reseats), ("reseated", Some(1)));
}

#[test]
fn a_send_back_counts_on_its_parent_and_a_rate_limit_on_its_order() {
    let mut events = delivered_run();
    let mut child = seated("sub-s1");
    child.parent = Some("sub".into());
    child.why = Some("send_back".into());
    events.push(ev(8, T0 + 50, Kind::Seated(Box::new(child))));
    events.push(ev(
        9,
        T0 + 51,
        Kind::RateLimited {
            order: "sub-s1".into(),
            signal: "rate_limit_event".into(),
            pool: Some("max".into()),
            window: Some("5h".into()),
            resets_at: Some(T0 + 900),
            detail: String::new(),
        },
    ));
    events.push(ev(
        10,
        T0 + 52,
        Kind::Deferred {
            order: "sub-s1".into(),
            until: T0 + 960,
            why: "reset".into(),
        },
    ));
    let state = replay(&events);
    assert_eq!(state.order("sub").map(|o| o.send_backs), Some(1));
    let child = state.order("sub-s1").expect("child");
    assert_eq!((child.rate_limited, child.deferrals), (1, 1));
    assert_eq!(child.phase, Phase::Deferred);
    assert_eq!(child.deferred_until, Some(T0 + 960));
}

#[test]
fn a_record_line_keeps_keys_it_does_not_know() {
    let later = r#"{"order":"o1","seat":"sonnet","effort":"medium","class":"build.bounded","outcome":"pass","tokens":null,"use":null,"at":5,"gate_axes":{"tests":"pass"},"lesson_opened":true}"#;
    let parsed: RecordLine = serde_json::from_str(later).expect("a later line");
    assert_eq!(parsed.extra.get("lesson_opened"), Some(&Value::Bool(true)));
    let back: Value =
        serde_json::from_str(&serde_json::to_string(&parsed).expect("json")).expect("json");
    assert_eq!(back["gate_axes"]["tests"], "pass");
    assert_eq!(back["lesson_opened"], true);
}

fn booked_lease(order: &str) -> nirvana_seat::Lease {
    nirvana_seat::Lease {
        order: order.into(),
        seat: "haiku".into(),
        effort: Effort::Low,
        pool: "max".into(),
        class: "chore.script".into(),
        run: Some("live1".into()),
        expected: BTreeMap::from([
            ("5h".to_owned(), 0.0004_f32),
            ("week".to_owned(), 0.00005_f32),
        ]),
        booked_at: T0,
        expires_at: T0 + 3600,
        pick: serde_json::json!({
            "seat": "haiku", "reason": "chore.script → haiku low", "override": null, "rule_pick": null,
            "mode": "balanced", "size": "small", "bar_dropped": false, "step_down": false,
            "use_basis": "derived: P4's tokens"
        }),
    }
}

#[test]
fn an_order_is_recorded_once_with_its_booking_and_the_booking_ends() {
    let dir = temp_dir("record");
    let mut leases = nirvana_seat::Leases::default();
    leases.book(booked_lease("sub"), T0);
    nirvana_seat::write_leases(&dir.join(nirvana_seat::LEASES_FILE), &leases).expect("leases");
    let line: RecordLine = serde_json::from_value(serde_json::json!({
        "order": "sub", "run": "live1", "seat": "haiku", "effort": "low", "class": "chore.script", "outcome": "pass"
    }))
    .expect("line");
    let (written, filled) =
        nirvana_seat::record_once(&dir, line.clone(), true, T0 + 60).expect("record");
    assert!(written);
    assert_eq!(filled.pool.as_deref(), Some("max"));
    assert_eq!(filled.reason.as_deref(), Some("chore.script → haiku low"));
    assert_eq!(
        (filled.mode, filled.size),
        (Some(Mode::Balanced), Some(SizeName::Small))
    );
    assert_eq!(
        (filled.bar_dropped, filled.step_down),
        (Some(false), Some(false))
    );
    assert_eq!(filled.use_basis.as_deref(), Some("derived: P4's tokens"));
    assert_eq!(filled.expected.as_ref().map(BTreeMap::len), Some(2));
    assert_eq!(filled.seated_at, Some(T0), "the lease's booked_at");
    assert_eq!(filled.v, Some(1));
    // The booking is released with it.
    let leases = nirvana_seat::load_leases(&dir.join(nirvana_seat::LEASES_FILE)).unwrap();
    assert!(leases.live("sub", T0 + 60).is_none());
    // A retry is refused: one line, not two.
    let (again, _) = nirvana_seat::record_once(&dir, line, true, T0 + 61).expect("retry");
    assert!(!again);
    let lines = nirvana_seat::read_record(&dir.join(nirvana_seat::RECORD_FILE));
    assert_eq!(lines.len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn racing_record_calls_write_one_line() {
    let dir = temp_dir("record-race");
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                let line: RecordLine = serde_json::from_value(serde_json::json!({
                    "order": "sub", "run": "live1", "seat": "haiku", "effort": "low",
                    "class": "chore.script", "outcome": "pass"
                }))
                .expect("line");
                nirvana_seat::record_once(&dir, line, true, T0)
                    .expect("record")
                    .0
            })
        })
        .collect();
    let written = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread"))
        .filter(|written| *written)
        .count();
    assert_eq!(written, 1);
    assert_eq!(
        nirvana_seat::read_record(&dir.join(nirvana_seat::RECORD_FILE)).len(),
        1
    );
    let _ = std::fs::remove_dir_all(dir);
}
