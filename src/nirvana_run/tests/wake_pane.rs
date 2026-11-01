//! Fix wake-pane (owner, 2026-11-01): a run follows the orchestrator that seats into it, and a wake whose pane is
//! gone says so. Run f2u-setup was opened from `wH:p1` on 2026-10-23; on 2026-11-01 a Nirvana Claude in another space
//! seated two orders into it, and all three wakes after their delivery queued on the gone `wH:p1` with "herdr could
//! not say what runs in it" (seq 1440, 1449, 1461).

use super::*;

/// The incident journal's own first line, its pools cut: an `opened` from before this fix.
const OLD_OPENED: &str = r#"{"v":1,"seq":1,"at":1792786279,"event":"opened","project":"C:\\Users\\fahim\\Desktop\\Pojects\\3D-Printer-Flow-xtool","orchestrator_pane":"wH:p1","auto":true,"pools":[]}"#;

fn delivered(order: &str) -> Kind {
    Kind::Delivered {
        order: order.into(),
        report: report("done"),
    }
}

/// A `seat run` of `order` from `pane` into the events, as `seat_order_with` writes it under the journal's lock, from a
/// pane herdr says runs Nirvana Claude (`nirvana_worker::mover`'s answer).
fn seat_from(events: &[Event], order: &str, pane: Option<&str>) -> Vec<Kind> {
    let (kinds, known) =
        crate::nirvana_worker::seating_from(events, &seated(order), opened(pane), pane);
    assert!(known.is_none(), "a new order");
    kinds
}

fn with_seqs(kinds: Vec<Kind>, start: u64) -> Vec<Event> {
    kinds
        .into_iter()
        .zip(start..)
        .map(|(kind, seq)| ev(seq, T0 + seq, kind))
        .collect()
}

fn gone(pane: &str) -> PaneView {
    PaneView {
        program: PaneProgram::Gone(pane.into()),
        ..PaneView::default()
    }
}

#[test]
fn wake_pane_a_seat_run_from_another_pane_moves_the_run_there() {
    let mut events = with_seqs(seat_from(&[], "o1", Some("wH:p1")), 1);
    // The first seat opens the run in its pane and writes no move.
    assert!(matches!(events[0].kind, Kind::Opened { .. }));
    assert_eq!(events.len(), 2);
    let kinds = seat_from(&events, "o2", Some("wS:p1"));
    assert_eq!(
        kinds[0],
        Kind::OrchestratorMoved {
            pane: "wS:p1".into(),
            from: Some("wH:p1".into()),
        }
    );
    assert!(matches!(kinds[1], Kind::Seated(_)));
    events.extend(with_seqs(kinds, 3));
    let state = replay(&events);
    assert_eq!(state.orchestrator_pane.as_deref(), Some("wS:p1"));
    // `opened` stays the pane the run began in.
    assert_eq!(
        state
            .opened
            .as_ref()
            .and_then(|o| o.orchestrator_pane.as_deref()),
        Some("wH:p1")
    );
    // From the same pane again, from outside herdr, or from one of the run's own worker panes: no move.
    assert_eq!(seat_from(&events, "o3", Some("wS:p1")).len(), 1);
    assert_eq!(seat_from(&events, "o3", None).len(), 1);
    assert_eq!(orchestrator_move(&replay(&events), None), None);
    assert_eq!(orchestrator_move(&replay(&events), Some("")), None);
    events.push(ev(
        5,
        T0 + 5,
        Kind::Started {
            order: "o2".into(),
            attempt: 1,
            pid: None,
            pane: Some("wS:pV".into()),
            tab: Some("wS:t2".into()),
            resume: None,
            argv: Vec::new(),
        },
    ));
    assert_eq!(seat_from(&events, "o3", Some("wS:pV")).len(), 1);
    // And back to the first pane: the latest move wins.
    events.extend(with_seqs(seat_from(&events, "o3", Some("wH:p1")), 6));
    assert_eq!(replay(&events).orchestrator_pane.as_deref(), Some("wH:p1"));
}

#[test]
fn wake_pane_the_move_is_one_journal_line_an_older_herdr_skips() {
    let event = ev(
        7,
        T0,
        Kind::OrchestratorMoved {
            pane: "wS:p1".into(),
            from: Some("wH:p1".into()),
        },
    );
    let line = serde_json::to_string(&event).unwrap();
    assert_eq!(
        line,
        r#"{"v":1,"seq":7,"at":1792289318,"event":"orchestrator_moved","pane":"wS:p1","from":"wH:p1"}"#
    );
    assert_eq!(serde_json::from_str::<Event>(&line).unwrap(), event);
}

#[test]
fn wake_pane_an_old_journal_with_only_opened_replays_unchanged() {
    let (mut events, bad) = parse_lines(OLD_OPENED);
    assert_eq!(bad, 0);
    events.push(ev(2, T0, Kind::Seated(Box::new(seated("sub")))));
    events.push(ev(3, T0 + 1, delivered("sub")));
    let state = replay(&events);
    assert_eq!(state.orchestrator_pane.as_deref(), Some("wH:p1"));
    assert_eq!(
        state
            .opened
            .as_ref()
            .and_then(|o| o.orchestrator_pane.as_deref()),
        Some("wH:p1")
    );
    // A second `opened` (a journal no herdr writes, but the replay's rule) still keeps the first pane.
    events.push(ev(4, T0 + 2, opened(Some("w9:p9"))));
    assert_eq!(replay(&events).orchestrator_pane.as_deref(), Some("wH:p1"));
}

#[test]
fn wake_pane_a_gone_pane_is_named_in_the_queued_reason() {
    assert_eq!(
        PaneProgram::of_get_error("wH:p1", Some("pane_not_found")),
        PaneProgram::Gone("wH:p1".into())
    );
    assert_eq!(
        PaneProgram::of_get_error("wH:p1", Some("server_unavailable")),
        PaneProgram::Unknown
    );
    assert_eq!(
        PaneProgram::of_get_error("wH:p1", None),
        PaneProgram::Unknown
    );
    // herdr's own error response for `pane get` of a pane it does not have (`app/api/panes.rs`, `pane_not_found`),
    // as the CLI prints it on stderr.
    assert_eq!(
        crate::nirvana_worker::herdr_error(
            "",
            r#"{"id":"cli:pane:get","error":{"code":"pane_not_found","message":"pane wH:p1 not found"}}"#
        ),
        (
            Some("pane_not_found".to_owned()),
            "pane wH:p1 not found".to_owned()
        )
    );
    // The wake's `pane get` turns that code into `Gone`; any other failure, or a pane whose processes herdr cannot
    // list, stays `Unknown`.
    let asked = |answer: Result<Value, (Option<String>, String)>| {
        let calls = std::cell::RefCell::new(Vec::new());
        let (_, program) = crate::nirvana_worker::pane_get_with("wH:p1", |args: &[&str]| {
            calls.borrow_mut().push(args.join(" "));
            if args[1] == "get" {
                answer.clone()
            } else {
                Err((None, "no answer".into()))
            }
        });
        (program, calls.into_inner())
    };
    assert_eq!(
        asked(Err((
            Some("pane_not_found".into()),
            "pane wH:p1 not found".into()
        ))),
        (
            PaneProgram::Gone("wH:p1".into()),
            vec!["pane get wH:p1".to_owned()]
        )
    );
    assert_eq!(
        asked(Err((None, "herdr: connection refused".into()))).0,
        PaneProgram::Unknown
    );
    assert_eq!(
        asked(Ok(
            serde_json::json!({"result": {"pane": {"agent": "claude"}}})
        )),
        (
            PaneProgram::Unknown,
            vec![
                "pane get wH:p1".to_owned(),
                "pane process-info --pane wH:p1".to_owned()
            ]
        )
    );
    let mut events = delivered_run();
    events[0] = ev(1, T0, opened(Some("wH:p1")));
    let state = replay(&events);
    let want = Wake::Queue(
        "the orchestrator's pane wH:p1 is gone: herdr no longer has it; a seat run from Nirvana Claude's pane moves \
         the run there"
            .into(),
    );
    for urgent in [false, true] {
        assert_eq!(
            wake_decision("f2u-setup", &state, Some(&gone("wH:p1")), T0 + 100, urgent),
            want
        );
    }
    // The typing refuses it too, naming the pane.
    let refused = type_if_nirvana(&PaneProgram::Gone("wH:p1".into()), || Ok(()));
    assert_eq!(
        refused,
        Err("the pane does not run Nirvana Claude: herdr no longer has pane wH:p1".into())
    );
}

/// The incident's shape end to end: opened in A, seated from B, delivered. The wake reads B's pane and types into B;
/// A, gone, is never asked about.
#[test]
fn wake_pane_the_wake_of_an_order_seated_from_b_types_into_b() {
    let home = crate::nirvana::with_test_home("wake-pane-b");
    let project = Project::at(home.path.join("project"));
    project.ensure().unwrap();
    let journal = project.journal("r1");
    for pane in ["wH:p1", "wS:p1"] {
        let order = if pane == "wH:p1" { "o1" } else { "o2" };
        journal
            .transact(T0, |events| Ok((seat_from(events, order, Some(pane)), ())))
            .unwrap();
    }
    journal.append(T0 + 10, delivered("o2")).unwrap();
    let asked = std::cell::RefCell::new(Vec::new());
    let typed = std::cell::RefCell::new(Vec::new());
    let (verdict, woke) = wake_with(
        &project,
        "r1",
        false,
        |pane: &str| {
            asked.borrow_mut().push(pane.to_owned());
            if pane == "wH:p1" {
                gone(pane)
            } else {
                idle_view()
            }
        },
        |pane: &str, _: &str| {
            typed.borrow_mut().push(pane.to_owned());
            Ok(())
        },
    )
    .unwrap();
    assert!(woke, "{verdict}");
    assert_eq!(*asked.borrow(), vec!["wS:p1".to_owned()]);
    assert_eq!(*typed.borrow(), vec!["wS:p1".to_owned()]);
}

/// Item 3: a wake that queued on the gone pane is retried by the server's pass (`app/nirvana_wake_retry.rs` runs
/// `herdr run wake`, which replays the journal), so once a seat run from the new pane journals the move, the retry
/// types there. Nothing else is needed for the retry to follow.
#[test]
fn wake_pane_a_queued_wake_is_retried_toward_the_new_pane() {
    let home = crate::nirvana::with_test_home("wake-pane-retry");
    let project = Project::at(home.path.join("project"));
    project.ensure().unwrap();
    let journal = project.journal("r1");
    journal
        .transact(T0, |events| {
            Ok((seat_from(events, "o1", Some("wH:p1")), ()))
        })
        .unwrap();
    journal.append(T0 + 10, delivered("o1")).unwrap();
    let typed = std::cell::RefCell::new(Vec::new());
    let view = |pane: &str| {
        if pane == "wH:p1" {
            gone(pane)
        } else {
            idle_view()
        }
    };
    let typer = |pane: &str, _: &str| {
        typed.borrow_mut().push(pane.to_owned());
        Ok(())
    };
    let (verdict, woke) = wake_with(&project, "r1", false, view, typer).unwrap();
    assert!(!woke && verdict.contains("wH:p1 is gone"), "{verdict}");
    assert_eq!(take_due_retries(&home.path, now() + 100_000).len(), 1);
    // A Nirvana Claude in pane wS:p1 seats into the run: the move is journaled, and the retry's wake types there.
    journal
        .transact(T0 + 20, |events| {
            Ok((seat_from(events, "o2", Some("wS:p1")), ()))
        })
        .unwrap();
    let (verdict, woke) = wake_with(&project, "r1", false, view, typer).unwrap();
    assert!(woke, "{verdict}");
    assert_eq!(*typed.borrow(), vec!["wS:p1".to_owned()]);
    assert!(take_due_retries(&home.path, u64::MAX).is_empty());
}

/// Review S1: only a pane herdr says runs Nirvana Claude takes the run. A `seat run` typed in a shell or a plain Claude
/// Code pane, or one whose pane herdr cannot answer about, leaves the run with its orchestrator.
#[test]
fn wake_pane_only_a_pane_shown_to_run_nirvana_claude_takes_the_run() {
    use crate::nirvana_worker::mover;
    let events = with_seqs(seat_from(&[], "o1", Some("wH:p1")), 1);
    let asked = |program: PaneProgram| {
        let calls = std::cell::RefCell::new(Vec::new());
        let moves_to = mover(&events, Some("wS:p1"), |pane| {
            calls.borrow_mut().push(pane.to_owned());
            program
        });
        (moves_to, calls.into_inner())
    };
    let here = vec!["wS:p1".to_owned()];
    assert_eq!(
        asked(PaneProgram::NirvanaClaude),
        (Some("wS:p1".to_owned()), here.clone())
    );
    for program in [
        PaneProgram::Other("its claude has no --agent nirvana".into()),
        PaneProgram::Other("no claude runs in it".into()),
        PaneProgram::Unknown,
        PaneProgram::Gone("wS:p1".into()),
    ] {
        assert_eq!(asked(program), (None, here.clone()));
    }
    // herdr not answering, through the wake's own `pane get`: `Unknown`, so no move.
    assert_eq!(
        mover(&events, Some("wS:p1"), |pane| {
            crate::nirvana_worker::pane_get_with(pane, |_: &[&str]| {
                Err((None, "herdr: connection refused".into()))
            })
            .1
        }),
        None
    );
    // herdr is not asked when no move is due: the orchestrator's own pane, no pane, a run not yet opened.
    let never = |pane: &str| -> PaneProgram { panic!("asked about {pane}") };
    assert_eq!(mover(&events, Some("wH:p1"), never), None);
    assert_eq!(mover(&events, None, never), None);
    assert_eq!(mover(&[], Some("wS:p1"), never), None);
    // Seated from the Other pane: the seat is written, the run stays with wH:p1.
    let (kinds, known) = crate::nirvana_worker::seating_from(
        &events,
        &seated("o2"),
        opened(Some("wS:p1")),
        asked(PaneProgram::Other("a shell".into())).0.as_deref(),
    );
    assert!(known.is_none());
    assert_eq!(kinds.len(), 1);
    assert!(matches!(kinds[0], Kind::Seated(_)));
    let mut after = events.clone();
    after.extend(with_seqs(kinds, 3));
    assert_eq!(replay(&after).orchestrator_pane.as_deref(), Some("wH:p1"));
}

/// Review N5: a repeated `seat run` of an order the run knows, from a new Nirvana Claude pane, writes no second seat
/// but still moves the run: a resumed orchestrator in a new pane re-issues the order ids it had.
#[test]
fn wake_pane_a_repeated_seat_run_from_a_new_pane_still_moves_the_run() {
    let mut events = with_seqs(seat_from(&[], "o1", Some("wH:p1")), 1);
    let (kinds, known) = crate::nirvana_worker::seating_from(
        &events,
        &seated("o1"),
        opened(Some("wS:p1")),
        Some("wS:p1"),
    );
    assert_eq!(
        kinds,
        vec![Kind::OrchestratorMoved {
            pane: "wS:p1".into(),
            from: Some("wH:p1".into()),
        }]
    );
    assert_eq!(known.expect("the order as it is").seated.order, "o1");
    events.extend(with_seqs(kinds, 3));
    assert_eq!(replay(&events).orchestrator_pane.as_deref(), Some("wS:p1"));
    // Again from the same pane: nothing to write.
    let (kinds, known) = crate::nirvana_worker::seating_from(
        &events,
        &seated("o1"),
        opened(Some("wS:p1")),
        Some("wS:p1"),
    );
    assert!(kinds.is_empty() && known.is_some());
}

/// Review N1: herdr's error response is read from stderr when stdout is JSON with no `error`.
#[test]
fn wake_pane_an_error_on_stderr_is_read_past_json_on_stdout() {
    let stderr = r#"{"id":"cli:pane:get","error":{"code":"pane_not_found","message":"pane wH:p1 not found"}}"#;
    let want = (
        Some("pane_not_found".to_owned()),
        "pane wH:p1 not found".to_owned(),
    );
    assert_eq!(
        crate::nirvana_worker::herdr_error(r#"{"id":"cli:pane:get","result":{}}"#, stderr),
        want
    );
    // stdout's own error response still comes first.
    assert_eq!(
        crate::nirvana_worker::herdr_error(
            r#"{"error":{"code":"server_unavailable","message":"no server"}}"#,
            stderr
        ),
        (
            Some("server_unavailable".to_owned()),
            "no server".to_owned()
        )
    );
}
