//! Plan 25 §2: the trigger, the note, the hooks' halves over temporary folders, and the server's pure rule.

use super::*;
use crate::nirvana_idle_compact::ENTER_TRIES;
use crate::nirvana_run::Journal;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nirvana-p25-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn opened(pane: &str) -> Kind {
    Kind::Opened {
        project: "p".into(),
        orchestrator_pane: Some(pane.into()),
        auto: false,
        pools: Vec::new(),
        pools_basis: None,
    }
}

fn ended() -> Kind {
    Kind::Ended {
        order: None,
        outcome: "ended".into(),
        why: None,
        exit: None,
        pools: Vec::new(),
        pools_basis: None,
    }
}

/// A run in `project` opened from `pane`, ended at `end` when given.
fn run(project: &Project, id: &str, pane: &str, end: Option<u64>) -> Journal {
    std::fs::create_dir_all(project.run_dir(id)).unwrap();
    let journal = project.journal(id);
    journal.append(10, opened(pane)).unwrap();
    if let Some(at) = end {
        journal.append(at, ended()).unwrap();
    }
    journal
}

fn assistant(context: u64) -> String {
    serde_json::json!({
        "type": "assistant",
        "message": {"usage": {"input_tokens": 2, "cache_read_input_tokens": context - 2, "output_tokens": 50}},
    })
    .to_string()
}

fn user(text: &str) -> String {
    serde_json::json!({"type": "user", "message": {"role": "user", "content": text}}).to_string()
}

// ---- the trigger ----

/// §2 test: "The trigger fires only after a run end, and only above `fresh_above`."
#[test]
fn fresh_trigger_fires_only_after_a_run_end_and_above_fresh_above() {
    let fires = |end: Option<(&str, u64)>, began: Option<u64>, context: Option<u64>| {
        trigger(end, began, context, FRESH_ABOVE)
    };
    assert_eq!(
        fires(Some(("r1", 200)), Some(100), Some(FRESH_ABOVE + 1)),
        Ok("r1".to_owned())
    );
    // A run ended at the session's start counts: the clear and the end can share a second.
    assert!(fires(Some(("r1", 100)), Some(100), Some(FRESH_ABOVE + 1)).is_ok());
    assert!(fires(None, Some(100), Some(FRESH_ABOVE + 1)).is_err());
    assert!(fires(Some(("r1", 99)), Some(100), Some(FRESH_ABOVE + 1)).is_err());
    assert!(fires(Some(("r1", 200)), None, Some(FRESH_ABOVE + 1)).is_err());
    assert!(fires(Some(("r1", 200)), Some(100), Some(FRESH_ABOVE)).is_err());
    assert!(fires(Some(("r1", 200)), Some(100), None).is_err());
    assert_eq!(FRESH_ABOVE, 120_000);
}

#[test]
fn the_last_end_is_this_panes_latest_ended_run() {
    let root = scratch("last-end");
    let project = Project::at(&root);
    run(&project, "a", "p1", Some(300));
    run(&project, "b", "p1", Some(500));
    run(&project, "c", "p2", Some(900));
    run(&project, "d", "p1", None);
    let runs = run_states(&project);
    assert_eq!(last_end_of("p1", &runs), Some(("b", 500)));
    assert_eq!(last_end_of("p2", &runs), Some(("c", 900)));
    assert_eq!(last_end_of("p3", &runs), None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn seats_toml_carries_fresh_above_and_an_older_table_defaults_it() {
    let table = crate::nirvana_seat::parse(crate::nirvana_seat::DEFAULT).unwrap();
    assert_eq!(table.constants.fresh_above, 120_000);
    let older = crate::nirvana_seat::parse(crate::nirvana_seat::HISTORICAL_DEFAULTS[3]).unwrap();
    assert_eq!(older.constants.fresh_above, FRESH_ABOVE);
}

/// The Stop hook's half: a record when the trigger holds, taken back when it no longer does; the owner's own stays.
#[test]
fn the_stop_hook_records_a_pending_start_and_takes_it_back() {
    let root = scratch("on-stop");
    let home = root.join("home");
    let project = Project::at(root.join("project"));
    let transcript = root.join("session.jsonl");
    std::fs::write(&transcript, format!("{}\n", assistant(180_000))).unwrap();
    let began = session_began(&transcript).unwrap();
    run(&project, "r1", "p1", Some(began + 60));
    let wrote = on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, 7_000).unwrap();
    assert_eq!(wrote, Ok("r1".to_owned()));
    let record = read_record(&home, "p1").unwrap().unwrap();
    assert_eq!(record.run.as_deref(), Some("r1"));
    assert_eq!(record.context, Some(180_000));
    assert_eq!(record.by, "stop");
    assert_eq!(record.at, 7_000);
    // A later turn keeps the record's second, which the server's mark names.
    on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, 8_000)
        .unwrap()
        .unwrap();
    assert_eq!(read_record(&home, "p1").unwrap().unwrap().at, 7_000);
    // Under fresh_above now: taken back.
    std::fs::write(&transcript, format!("{}\n", assistant(90_000))).unwrap();
    assert!(
        on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, 9_000)
            .unwrap()
            .is_err()
    );
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    // The owner's own start is not the hook's to take back.
    let owner = Record {
        by: "owner".into(),
        ..record
    };
    write_record(&home, "p1", &owner).unwrap();
    assert!(
        on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, 9_500)
            .unwrap()
            .is_err()
    );
    assert_eq!(read_record(&home, "p1").unwrap(), Some(owner));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_record_that_does_not_parse_is_a_doubt_not_none() {
    let root = scratch("bad-record");
    std::fs::create_dir_all(record_path(&root, "p1").parent().unwrap()).unwrap();
    std::fs::write(record_path(&root, "p1"), "{not json").unwrap();
    assert!(read_record(&root, "p1").is_err());
    assert_eq!(read_record(&root, "p2"), Ok(None));
    let _ = std::fs::remove_dir_all(&root);
}

// ---- the note ----

fn small_input() -> NoteInput {
    NoteInput {
        run: Some("r1".into()),
        ended: Some("r1 ended at 2026-10-23 10:00:00Z; orders: o1 merged".into()),
        open: vec![(
            "r2".into(),
            vec![
                ("o3".into(), "running".into()),
                ("o4".into(), "seated".into()),
            ],
        )],
        board: vec![("Plan".into(), "ship the fresh start".into())],
        prompts: vec!["first".into(), "second".into(), "third, the last".into()],
        lessons: Some("- lessons for C:\\p: 2 live".into()),
    }
}

/// §2 test: "The note keeps within its budget, and holds the open runs and the last prompts."
#[test]
fn the_note_keeps_its_budget_and_holds_the_open_runs_and_the_last_prompts() {
    let text = note(&small_input());
    assert!(text.contains("r1 ended at 2026-10-23 10:00:00Z; orders: o1 merged"));
    assert!(text.contains("- r2: o3 running, o4 seated"));
    assert!(text.contains("- Plan: ship the fresh start"));
    assert!(text.contains("> first\n> second\n> third, the last\n"));
    assert!(text.contains("- lessons for C:\\p: 2 live"));
    assert!(text.ends_with(
        "This is a fresh session after run r1. Ask the owner before assuming anything the note does not say."
    ));
    // Far over the budget: still within it, the closing whole, the open runs and the prompts' starts kept.
    let huge = NoteInput {
        open: (0..40)
            .map(|n| {
                (
                    format!("run-{n}"),
                    vec![(format!("o{n}"), "running".into())],
                )
            })
            .collect(),
        board: (0..40)
            .map(|n| (format!("s{n}"), "x".repeat(400)))
            .collect(),
        prompts: vec!["a".repeat(5_000), "b".repeat(5_000), "c".repeat(5_000)],
        lessons: Some("l".repeat(3_000)),
        ..small_input()
    };
    let text = note(&huge);
    assert!(
        text.chars().count() <= NOTE_MAX_CHARS,
        "{}",
        text.chars().count()
    );
    assert!(note_tokens(&text) <= NOTE_MAX_TOKENS as u64);
    assert!(text.contains("- run-0: o0 running"));
    assert!(text.contains("- and 35 more"));
    assert!(text.contains(&format!("> {}", "a".repeat(100))));
    // Each prompt is cut at its own cap, so all three show; the runs past the note's five are counted, not named.
    assert!(text.contains(&format!("> {}", "c".repeat(100))));
    assert!(!text.contains("- run-5:"));
    // The body was over the budget: cut before the closing line.
    assert!(!text.contains(&"l".repeat(3_000)));
    assert_eq!((NOTE_MAX_TOKENS, NOTE_MAX_CHARS), (2_000, 8_000));
    assert!(text.ends_with(&closing(Some("r1"))));
    assert_eq!(note_tokens("abcde"), 2);
    assert_eq!(
        closing(None),
        "This is a fresh session the owner asked for. Ask the owner before assuming anything the note does not say."
    );
}

#[test]
fn the_owners_prompts_are_the_last_three_they_typed_not_herdrs_or_claudes() {
    let lines = [
        user("zero"),
        user("one"),
        assistant(1_000),
        user("two\nlines"),
        serde_json::json!({"type": "user", "isMeta": true, "message": {"content": "meta"}}).to_string(),
        user("<command-name>/clear</command-name>"),
        user("herdr run r1: o1 delivered done: x; detail: herdr run notice r1"),
        serde_json::json!({"type": "user", "message": {"content": [
            {"type": "tool_result", "content": "x"},
            {"type": "text", "text": "a reminder beside a tool result"},
        ]}})
            .to_string(),
        serde_json::json!({"type": "user", "isSidechain": true, "message": {"content": "sub"}}).to_string(),
        serde_json::json!({"type": "user", "message": {"content": [{"type": "text", "text": "three"}]}})
            .to_string(),
        "{partial line".to_owned(),
    ]
    .join("\n");
    assert_eq!(
        owner_prompts(&lines, PROMPTS),
        vec!["one", "two\nlines", "three"]
    );
    assert_eq!(owner_prompts(&lines, 1), vec!["three"]);
    assert!(owner_prompts("", PROMPTS).is_empty());
}

/// The SessionStart hook's half: on `/clear`, the note from files, the journal's `fresh` event, the record removed.
#[test]
fn a_clear_prints_the_note_journals_fresh_and_forgets_the_record() {
    let root = scratch("on-start");
    let home = root.join("home");
    let project = Project::at(root.join("project"));
    let journal = run(&project, "r1", "p1", Some(500));
    run(&project, "r2", "p1", None);
    let transcript = root.join("old.jsonl");
    std::fs::write(
        &transcript,
        [user("plan the shelf"), user("now build it")].join("\n"),
    )
    .unwrap();
    let record = Record {
        run: Some("r1".into()),
        ended_at: Some(500),
        context: Some(180_000),
        transcript: Some(transcript.display().to_string()),
        project: project.root.display().to_string(),
        at: 600,
        by: "stop".into(),
    };
    write_record(&home, "p1", &record).unwrap();
    write_sent(&home, "p1", 600).unwrap();
    // Not a clear: nothing printed, the record stays.
    assert_eq!(on_session_start_in(&home, "p1", "resume", 700), None);
    assert!(read_record(&home, "p1").unwrap().is_some());
    let text = on_session_start_in(&home, "p1", "clear", 700).unwrap();
    assert!(text.contains("r1 ended at"));
    assert!(text.contains("- r2: no orders"));
    assert!(text.contains("> plan the shelf\n> now build it\n"));
    assert!(text.contains("lessons for"));
    assert!(text.ends_with(&closing(Some("r1"))));
    let (events, _) = journal.read();
    assert_eq!(
        events.last().unwrap().kind,
        Kind::Fresh {
            pane: "p1".into(),
            by: "stop".into(),
            context: Some(180_000),
            note_tokens: note_tokens(&text),
        }
    );
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    assert_eq!(read_sent(&home, "p1"), None);
    // The fresh event moves no order and wakes no one.
    let state = nirvana_run::replay(&events);
    assert!(state.queue.is_empty());
    // A new session is already fresh: a record left for it is forgotten.
    write_record(&home, "p1", &record).unwrap();
    assert_eq!(on_session_start_in(&home, "p1", "startup", 800), None);
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    let _ = std::fs::remove_dir_all(&root);
}

// ---- the server's rule ----

fn sending() -> Facts {
    Facts {
        compact_in_flight: false,
        record: Ok(Some(Record {
            run: Some("r1".into()),
            ended_at: Some(500),
            context: Some(180_000),
            transcript: None,
            project: "p".into(),
            at: 600,
            by: "stop".into(),
        })),
        sent_for: None,
        nirvana: Some(true),
        state: AgentState::Idle,
        owner_key: OwnerKey::SecsAgo(nirvana_run::OWNER_QUIET_S),
        input_line: InputLine::Empty,
    }
}

fn skips(facts: Facts) -> bool {
    matches!(decide(&facts), Decision::Skip(_))
}

#[test]
fn the_fresh_start_sends_when_every_rule_holds() {
    assert_eq!(decide(&sending()), Decision::Send(600));
    let never = Facts {
        owner_key: OwnerKey::Never,
        ..sending()
    };
    assert_eq!(decide(&never), Decision::Send(600));
}

#[test]
fn the_fresh_start_waits_for_an_idle_compacts_paste() {
    assert!(skips(Facts {
        compact_in_flight: true,
        ..sending()
    }));
}

#[test]
fn the_fresh_start_needs_a_record_it_has_not_cleared_for() {
    assert_eq!(
        decide(&Facts {
            record: Ok(None),
            ..sending()
        }),
        Decision::Nothing
    );
    assert!(skips(Facts {
        record: Err("unreadable".into()),
        ..sending()
    }));
    assert_eq!(
        decide(&Facts {
            sent_for: Some(600),
            ..sending()
        }),
        Decision::Nothing
    );
    assert_eq!(
        decide(&Facts {
            sent_for: Some(599),
            ..sending()
        }),
        Decision::Send(600)
    );
}

/// §2 test: nothing is typed into anything but Nirvana Claude.
#[test]
fn the_fresh_start_types_only_into_nirvana_claude() {
    assert!(skips(Facts {
        nirvana: Some(false),
        ..sending()
    }));
    assert!(skips(Facts {
        nirvana: None,
        ..sending()
    }));
}

/// §2 test: "Nothing is typed when the pane is busy".
#[test]
fn the_fresh_start_types_nothing_into_a_busy_pane() {
    for state in [
        AgentState::Working,
        AgentState::Blocked,
        AgentState::Unknown,
    ] {
        assert!(skips(Facts { state, ..sending() }), "{state:?}");
    }
}

/// §2 test: "when the owner typed in the last `OWNER_QUIET_S`".
#[test]
fn the_fresh_start_types_nothing_while_the_owner_is_at_the_pane() {
    for ago in [0, nirvana_run::OWNER_QUIET_S - 1] {
        assert!(
            skips(Facts {
                owner_key: OwnerKey::SecsAgo(ago),
                ..sending()
            }),
            "{ago}"
        );
    }
    assert!(skips(Facts {
        owner_key: OwnerKey::Unknown("no home".into()),
        ..sending()
    }));
}

/// §2 test: "or when the prompt line is not empty".
#[test]
fn the_fresh_start_types_nothing_unless_the_line_is_provably_empty() {
    assert!(skips(Facts {
        input_line: InputLine::Text("half a thought".into()),
        ..sending()
    }));
    assert!(skips(Facts {
        input_line: InputLine::Unsure("a dialog".into()),
        ..sending()
    }));
}

#[test]
fn after_the_clear_paste_reads_for_clear_with_idle_compacts_rule() {
    let quiet = OwnerKey::Never;
    let clear = InputLine::Text(CLEAR.into());
    assert_eq!(
        after_clear_paste(Step::Try(0), &clear, &quiet),
        AfterPaste::Enter
    );
    // `/compact` on the line is not this paste.
    assert!(matches!(
        after_clear_paste(
            Step::Try(0),
            &InputLine::Text(crate::nirvana_idle_compact::COMMAND.into()),
            &quiet
        ),
        AfterPaste::Leave(_)
    ));
    assert!(matches!(
        after_clear_paste(Step::Try(0), &InputLine::Text("/clearx".into()), &quiet),
        AfterPaste::Leave(_)
    ));
    assert_eq!(
        after_clear_paste(Step::Try(0), &InputLine::Empty, &quiet),
        AfterPaste::Next(Step::Try(1))
    );
    assert_eq!(
        after_clear_paste(Step::Try(ENTER_TRIES.len() - 1), &InputLine::Empty, &quiet),
        AfterPaste::Next(Step::Erase)
    );
    assert_eq!(
        after_clear_paste(Step::Erase, &clear, &quiet),
        AfterPaste::Erase
    );
    assert_eq!(
        after_clear_paste(Step::Erased, &InputLine::Empty, &quiet),
        AfterPaste::Erased
    );
    assert!(matches!(
        after_clear_paste(Step::Try(1), &clear, &OwnerKey::SecsAgo(1)),
        AfterPaste::Leave(_)
    ));
    assert_eq!(CLEAR_ERASE_KEYS, 6);
    assert_eq!(CLEAR, "/clear");
}

#[test]
fn the_fresh_stretch_looks_up_then_polls_and_never_spins() {
    let t0 = Instant::now();
    let mut fresh = FreshStretch::default();
    assert_eq!(fresh.next_check_at(None), None);
    assert_eq!(
        LOOKUPS,
        [
            Duration::from_secs(2),
            Duration::from_secs(10),
            Duration::from_secs(30)
        ]
    );
    assert_eq!(
        (POLL, RETRY),
        (Duration::from_secs(60), Duration::from_secs(30))
    );
    assert_eq!(fresh.next_check_at(Some(t0)), Some(t0 + LOOKUPS[0]));
    for (n, after) in LOOKUPS.iter().enumerate() {
        fresh.checks = n + 1;
        fresh.checked_at = Some(t0 + *after);
        let next = LOOKUPS
            .get(n + 1)
            .map_or(t0 + *after + POLL, |next| t0 + *next);
        assert_eq!(fresh.next_check_at(Some(t0)), Some(next));
    }
    // A record left for now: the owner's quiet window, not the poll.
    fresh.found = true;
    assert_eq!(fresh.next_check_at(Some(t0)), Some(t0 + LOOKUPS[2] + RETRY));
    // A check made late, inside the lookups: at least a second on.
    let late = FreshStretch {
        checks: 1,
        checked_at: Some(t0 + LOOKUPS[1] + Duration::from_secs(5)),
        ..FreshStretch::default()
    };
    assert_eq!(
        late.next_check_at(Some(t0)),
        Some(t0 + LOOKUPS[1] + Duration::from_secs(6))
    );
    // A paste: its reads come first; once sent, nothing.
    let pasted = FreshStretch {
        pending: Some(Pending {
            pane: "p".into(),
            pasted_at: t0,
            step: Step::Try(0),
        }),
        ..FreshStretch::default()
    };
    assert!(pasted.busy());
    assert_eq!(pasted.next_check_at(Some(t0)), Some(t0 + ENTER_TRIES[0]));
    let sent = FreshStretch {
        sent: true,
        ..FreshStretch::default()
    };
    assert_eq!(sent.next_check_at(Some(t0)), None);
    assert!(!sent.busy());
}

/// The fresh start's state lives on the idle stretch, so any other state ends it.
#[test]
fn the_fresh_stretch_ends_with_the_idle_stretch() {
    let t0 = Instant::now();
    let stretch = crate::nirvana_idle_compact::IdleStretch {
        since: Some(t0),
        fresh: FreshStretch {
            sent: true,
            checks: 2,
            ..FreshStretch::default()
        },
        ..Default::default()
    };
    assert_eq!(stretch.after(AgentState::Idle, t0).fresh, stretch.fresh);
    assert_eq!(
        stretch.after(AgentState::Working, t0).fresh,
        FreshStretch::default()
    );
}
