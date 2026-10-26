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

/// Unix seconds as a transcript's `timestamp`, `2026-10-23T10:00:00.123Z`.
fn iso(at: u64) -> String {
    format!(
        "{}.123Z",
        nirvana_run::utc(at).trim_end_matches('Z').replace(' ', "T")
    )
}

/// A transcript line of `kind` at `at`; a user line holds `text` as the owner typed it.
fn line_at(kind: &str, text: &str, at: u64) -> String {
    serde_json::json!({"type": kind, "timestamp": iso(at), "message": {"role": kind, "content": text}})
        .to_string()
}

/// An assistant turn at `at` whose context is `context`.
fn assistant_at(context: u64, at: u64) -> String {
    serde_json::json!({
        "type": "assistant",
        "timestamp": iso(at),
        "message": {"usage": {"input_tokens": 2, "cache_read_input_tokens": context - 2, "output_tokens": 50}},
    })
    .to_string()
}

// ---- the trigger ----

/// The trigger with the owner's check passed: every other rule's own case.
fn fires(
    end: Option<(&str, u64)>,
    began: Option<u64>,
    context: Option<u64>,
) -> Result<String, String> {
    let now = end.map_or(0, |(_, at)| at);
    trigger(end, None, began, context, FRESH_ABOVE, now, |_| Ok(()))
}

/// §2 test: "The trigger fires only after a run end, and only above `fresh_above`."
#[test]
fn fresh_trigger_fires_only_after_a_run_end_and_above_fresh_above() {
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

/// Review B1, rule 1: no open run whose `orchestrator_pane` is this pane.
#[test]
fn the_trigger_holds_off_while_a_run_of_the_pane_is_open() {
    let held = |open: Option<&str>| {
        trigger(
            Some(("r1", 200)),
            open,
            Some(100),
            Some(FRESH_ABOVE + 1),
            FRESH_ABOVE,
            200,
            |_| Ok(()),
        )
    };
    assert!(held(None).is_ok());
    assert_eq!(
        held(Some("r2")),
        Err("run r2 of this pane is open".to_owned())
    );
    // Only this pane's runs, and only the open ones.
    let root = scratch("open-of");
    let project = Project::at(&root);
    run(&project, "a", "p1", Some(300));
    run(&project, "b", "p2", None);
    run(&project, "c", "p1", None);
    let runs = run_states(&project);
    assert_eq!(open_of("p1", &runs), Some("c"));
    assert_eq!(open_of("p2", &runs), Some("b"));
    assert_eq!(open_of("p3", &runs), None);
    run(&project, "d", "p3", Some(400));
    assert_eq!(open_of("p3", &run_states(&project)), None);
    let _ = std::fs::remove_dir_all(&root);
}

/// Review B1, rule 2: no real owner prompt after the end; a line that cannot be read is one.
#[test]
fn the_trigger_holds_off_once_the_owner_typed_after_the_end() {
    let end = 1_792_800_000;
    // The owner's check is given the end, and its doubt is the trigger's.
    let asked = std::cell::Cell::new(None);
    let held = trigger(
        Some(("r1", end)),
        None,
        Some(end - 100),
        Some(FRESH_ABOVE + 1),
        FRESH_ABOVE,
        end,
        |at| {
            asked.set(Some(at));
            Err("typed".into())
        },
    );
    assert_eq!(held, Err("typed".to_owned()));
    assert_eq!(asked.get(), Some(end));
    let before = [
        line_at("user", "end the run", end - 30),
        assistant_at(150_000, end - 1),
    ];
    let quiet = before.join("\n");
    assert_eq!(owner_prompt_since(&quiet, end), Ok(()));
    // herdr's wake, Claude's turn, a tool result and a meta line after the end are not the owner's.
    let others = [
        quiet.clone(),
        line_at("user", "herdr run r1: o1 delivered", end + 5),
        assistant_at(150_000, end + 6),
        serde_json::json!({"type": "user", "timestamp": iso(end + 7), "message": {"content": [
            {"type": "tool_result", "content": "x"}]}})
        .to_string(),
        serde_json::json!({"type": "user", "isMeta": true, "timestamp": iso(end + 8), "message": {"content": "m"}})
            .to_string(),
        serde_json::json!({"type": "summary", "summary": "no timestamp"}).to_string(),
    ]
    .join("\n");
    assert_eq!(owner_prompt_since(&others, end), Ok(()));
    // The owner's question after the end, or in the end's own second.
    for at in [end, end + 40] {
        let asked = format!("{quiet}\n{}", line_at("user", "why did order 2 fail?", at));
        assert!(owner_prompt_since(&asked, end).is_err(), "{at}");
    }
    // Doubts: an owner prompt with no time, a line that does not parse, a tail that never reaches the end.
    let untimed = format!("{quiet}\n{}", user("why did order 2 fail?"));
    assert!(owner_prompt_since(&untimed, end).is_err());
    let unparsed = format!("{quiet}\n{{\"type\": \"user\", \"mess");
    assert!(owner_prompt_since(&unparsed, end).is_err());
    let after_only = assistant_at(150_000, end + 6);
    assert!(owner_prompt_since(&after_only, end).is_err());
    assert!(owner_prompt_since("", end).is_err());
}

/// Review B1, rule 3: the end is at most `FRESH_WITHIN` old when Stop fires.
#[test]
fn the_trigger_needs_a_recent_end() {
    let at = |now: u64| {
        trigger(
            Some(("r1", 200)),
            None,
            Some(100),
            Some(FRESH_ABOVE + 1),
            FRESH_ABOVE,
            now,
            |_| Ok(()),
        )
    };
    assert!(at(200).is_ok());
    assert!(at(200 + FRESH_WITHIN.as_secs()).is_ok());
    assert!(at(200 + FRESH_WITHIN.as_secs() + 1).is_err());
    // A clock behind the journal's end is not an old end.
    assert!(at(150).is_ok());
    assert_eq!(FRESH_WITHIN, Duration::from_secs(600));
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
    std::fs::write(&transcript, "").unwrap();
    let began = session_began(&transcript).unwrap();
    let end = began + 60;
    let turn = |context: u64| {
        [
            line_at("user", "end the run", end - 30),
            assistant_at(context, end - 1),
        ]
        .join("\n")
    };
    std::fs::write(&transcript, turn(180_000)).unwrap();
    run(&project, "r1", "p1", Some(end));
    let wrote = on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 5).unwrap();
    assert_eq!(wrote, Ok("r1".to_owned()));
    let record = read_record(&home, "p1").unwrap().unwrap();
    assert_eq!(record.run.as_deref(), Some("r1"));
    assert_eq!(record.context, Some(180_000));
    assert_eq!(record.by, "stop");
    assert_eq!(record.at, end + 5);
    assert_eq!(
        record.transcript.as_deref(),
        Some(transcript.display().to_string().as_str())
    );
    // A later turn keeps the record's second, which the server's mark names.
    on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 8)
        .unwrap()
        .unwrap();
    assert_eq!(read_record(&home, "p1").unwrap().unwrap().at, end + 5);
    // Under fresh_above now: taken back.
    std::fs::write(&transcript, turn(90_000)).unwrap();
    assert!(
        on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 9)
            .unwrap()
            .is_err()
    );
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    // The owner's own start is not the hook's to take back,
    let owner = Record {
        by: "owner".into(),
        ..record
    };
    write_record(&home, "p1", &owner).unwrap();
    assert!(
        on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 10)
            .unwrap()
            .is_err()
    );
    assert_eq!(read_record(&home, "p1").unwrap(), Some(owner.clone()));
    // nor to overwrite (review nit), though the rule holds again.
    std::fs::write(&transcript, turn(180_000)).unwrap();
    assert!(
        on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 11)
            .unwrap()
            .is_err()
    );
    assert_eq!(read_record(&home, "p1").unwrap(), Some(owner));
    let _ = std::fs::remove_dir_all(&root);
}

/// Review B1, scenario 1: a run ends at 150k, the owner asks "why did order 2 fail?", Claude answers and Stop fires.
/// The record the end's own Stop wrote is taken back, so `/clear` never wipes the conversation the owner is in.
#[test]
fn scenario_one_a_question_after_the_end_takes_the_record_back() {
    let root = scratch("scenario-1");
    let home = root.join("home");
    let project = Project::at(root.join("project"));
    let transcript = root.join("session.jsonl");
    std::fs::write(&transcript, "").unwrap();
    let end = session_began(&transcript).unwrap() + 60;
    let mut lines = vec![
        line_at("user", "end the run", end - 30),
        assistant_at(150_000, end - 1),
    ];
    std::fs::write(&transcript, lines.join("\n")).unwrap();
    run(&project, "r1", "p1", Some(end));
    // The end's own turn: recorded.
    assert_eq!(
        on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 2).unwrap(),
        Ok("r1".to_owned())
    );
    assert!(read_record(&home, "p1").unwrap().is_some());
    // The owner's question, Claude's answer, Stop.
    lines.push(line_at("user", "why did order 2 fail?", end + 20));
    lines.push(assistant_at(152_000, end + 40));
    std::fs::write(&transcript, lines.join("\n")).unwrap();
    let why = on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 41)
        .unwrap()
        .unwrap_err();
    assert!(why.contains("after the run's end"), "{why}");
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    let _ = std::fs::remove_dir_all(&root);
}

/// Review B1, scenario 2: the session opens run R2 after R1 ended; while R2 waits for its workers the pane is idle,
/// and R1's end must not clear it in the middle of R2.
#[test]
fn scenario_two_a_run_opened_after_the_end_takes_the_record_back() {
    let root = scratch("scenario-2");
    let home = root.join("home");
    let project = Project::at(root.join("project"));
    let transcript = root.join("session.jsonl");
    std::fs::write(&transcript, "").unwrap();
    let end = session_began(&transcript).unwrap() + 60;
    let lines = [
        line_at("user", "end the run", end - 30),
        assistant_at(150_000, end - 1),
    ];
    std::fs::write(&transcript, lines.join("\n")).unwrap();
    run(&project, "r1", "p1", Some(end));
    on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 2)
        .unwrap()
        .unwrap();
    assert!(read_record(&home, "p1").unwrap().is_some());
    // R2, opened by this pane from a herdr wake (not an owner prompt), waits for its workers.
    run(&project, "r2", "p1", None);
    let why = on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 30)
        .unwrap()
        .unwrap_err();
    assert_eq!(why, "run r2 of this pane is open");
    assert_eq!(read_record(&home, "p1").unwrap(), None);
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
    let here = Some(project.root.as_path());
    // Not a clear: nothing printed, the record stays.
    assert_eq!(
        on_session_start_in(&home, "p1", "compact", here, None, 700),
        None
    );
    assert!(read_record(&home, "p1").unwrap().is_some());
    let text = on_session_start_in(&home, "p1", "clear", here, None, 700).unwrap();
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
    assert_eq!(
        on_session_start_in(&home, "p1", "startup", here, None, 800),
        None
    );
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    let _ = std::fs::remove_dir_all(&root);
}

/// Review S1: the note is printed only into the record's own project, and a resumed session forgets a Stop's record.
#[test]
fn the_note_goes_only_to_its_project_and_a_resume_forgets_a_stops_record() {
    let root = scratch("start-bound");
    let home = root.join("home");
    let project = Project::at(root.join("project"));
    let journal = run(&project, "r1", "p1", Some(500));
    let record = Record {
        run: Some("r1".into()),
        ended_at: Some(500),
        context: Some(180_000),
        transcript: None,
        project: project.root.display().to_string(),
        at: 600,
        by: "stop".into(),
    };
    // A reused pane id in another project: forgotten, nothing printed, nothing journaled.
    for other in [Some(root.join("elsewhere")), None] {
        write_record(&home, "p1", &record).unwrap();
        assert_eq!(
            on_session_start_in(&home, "p1", "clear", other.as_deref(), None, 700),
            None
        );
        assert_eq!(read_record(&home, "p1").unwrap(), None, "{other:?}");
    }
    assert!(!journal
        .read()
        .0
        .iter()
        .any(|event| matches!(event.kind, Kind::Fresh { .. })));
    // The same project spelt another way is the same project.
    let spelt = PathBuf::from(
        project
            .root
            .display()
            .to_string()
            .replace('\\', "/")
            .to_uppercase(),
    );
    write_record(&home, "p1", &record).unwrap();
    assert!(on_session_start_in(&home, "p1", "clear", Some(&spelt), None, 700).is_some());
    // A resume forgets a Stop's record, and keeps the owner's.
    let here = Some(project.root.as_path());
    write_record(&home, "p1", &record).unwrap();
    assert_eq!(
        on_session_start_in(&home, "p1", "resume", here, None, 800),
        None
    );
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    let owner = Record {
        by: "owner".into(),
        ..record
    };
    write_record(&home, "p1", &owner).unwrap();
    assert_eq!(
        on_session_start_in(&home, "p1", "resume", here, None, 800),
        None
    );
    assert_eq!(read_record(&home, "p1").unwrap(), Some(owner));
    let _ = std::fs::remove_dir_all(&root);
}

/// Review S2: the owner's record names no transcript; the note's prompts come from the one the hook reports.
#[test]
fn the_owners_note_quotes_the_transcript_the_hook_reports() {
    let root = scratch("start-owner");
    let home = root.join("home");
    let project = Project::at(root.join("project"));
    run(&project, "r1", "p1", Some(500));
    let hooked = root.join("hooked.jsonl");
    std::fs::write(&hooked, user("the hook's own prompt")).unwrap();
    let recorded = root.join("recorded.jsonl");
    std::fs::write(&recorded, user("another pane's prompt")).unwrap();
    let here = Some(project.root.as_path());
    let owner = Record {
        run: Some("r1".into()),
        ended_at: Some(500),
        context: None,
        transcript: Some(recorded.display().to_string()),
        project: project.root.display().to_string(),
        at: 600,
        by: "owner".into(),
    };
    write_record(&home, "p1", &owner).unwrap();
    let text = on_session_start_in(&home, "p1", "clear", here, Some(&hooked), 700).unwrap();
    assert!(text.contains("> the hook's own prompt"), "{text}");
    assert!(!text.contains("another pane's prompt"));
    // A Stop's record quotes its own transcript, which the server checked was the pane's.
    let stop = Record {
        by: "stop".into(),
        ..owner
    };
    write_record(&home, "p1", &stop).unwrap();
    let text = on_session_start_in(&home, "p1", "clear", here, Some(&hooked), 700).unwrap();
    assert!(text.contains("> another pane's prompt"), "{text}");
    assert!(!text.contains("the hook's own prompt"));
    let _ = std::fs::remove_dir_all(&root);
}

/// Review S2: a named pane is cleared only when it works in the caller's project.
#[test]
fn a_named_pane_in_another_project_is_refused() {
    let caller = Path::new(r"C:\work\print");
    assert_eq!(
        same_project(Some(Path::new(r"C:\work\print")), caller),
        Ok(())
    );
    assert_eq!(
        same_project(Some(Path::new("c:/Work/Print/")), caller),
        Ok(())
    );
    let other = same_project(Some(Path::new(r"C:\work\shelf")), caller).unwrap_err();
    assert!(other.contains(r"C:\work\shelf"), "{other}");
    assert!(same_project(None, caller).is_err());
}

/// Review S2: the owner's record guesses no transcript and no context from the folder's session record.
#[test]
fn the_owners_record_names_no_transcript() {
    let root = scratch("owner-record");
    let project = Project::at(&root);
    run(&project, "a", "p1", Some(300));
    run(&project, "b", "p2", Some(900));
    let record = owner_record("p1", &project, &run_states(&project), 1_000);
    assert_eq!(
        record,
        Record {
            run: Some("a".into()),
            ended_at: Some(300),
            context: None,
            transcript: None,
            project: root.display().to_string(),
            at: 1_000,
            by: "owner".into(),
        }
    );
    let _ = std::fs::remove_dir_all(&root);
}

// ---- the server's rule ----

const TRANSCRIPT: &str = r"C:\Users\o\.claude\projects\C--work-print\abc-123.jsonl";

fn sending() -> Facts {
    Facts {
        compact_in_flight: false,
        compact_sent: false,
        record: Ok(Some(Record {
            run: Some("r1".into()),
            ended_at: Some(500),
            context: Some(180_000),
            transcript: Some(TRANSCRIPT.into()),
            project: "p".into(),
            at: 600,
            by: "stop".into(),
        })),
        now: 610,
        sent_for: None,
        nirvana: Some(true),
        transcript: Some(PathBuf::from(TRANSCRIPT)),
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
    // Review S3, the reverse of `busy`: a stretch whose idle compact already went gets no fresh start.
    assert!(skips(Facts {
        compact_sent: true,
        ..sending()
    }));
}

fn record_of(facts: &Facts) -> Record {
    facts.record.clone().unwrap().unwrap()
}

/// Review S1: a Stop's record lives `FRESH_WITHIN`; the server forgets an older one. The owner's has no such age.
#[test]
fn a_stale_stop_record_is_forgotten() {
    let at = record_of(&sending()).at;
    let aged = |now: u64| decide(&Facts { now, ..sending() });
    assert_eq!(aged(at + FRESH_WITHIN.as_secs()), Decision::Send(at));
    assert!(matches!(
        aged(at + FRESH_WITHIN.as_secs() + 1),
        Decision::Forget(_)
    ));
    // Forgotten even when the server already cleared for it, so a mark never keeps a stale record alive.
    assert!(matches!(
        decide(&Facts {
            now: at + FRESH_WITHIN.as_secs() + 1,
            sent_for: Some(at),
            ..sending()
        }),
        Decision::Forget(_)
    ));
    let owner = Facts {
        record: Ok(Some(Record {
            by: "owner".into(),
            ..record_of(&sending())
        })),
        now: at + 10 * FRESH_WITHIN.as_secs(),
        ..sending()
    };
    assert_eq!(decide(&owner), Decision::Send(at));
}

/// Review S1 and S2: a Stop's record is sent for only while the pane's transcript is still the one it names; any
/// doubt skips. The owner's record names none, and is not compared.
#[test]
fn the_fresh_start_needs_the_records_own_transcript() {
    let at = record_of(&sending()).at;
    // The same file, spelt as the hook and `choose_transcript` may spell it.
    let spelt = Facts {
        transcript: Some(PathBuf::from(
            "c:/users/o/.claude/projects/C--work-print/abc-123.jsonl",
        )),
        ..sending()
    };
    assert_eq!(decide(&spelt), Decision::Send(at));
    // After a `/clear` or a `--resume` into another conversation.
    assert!(skips(Facts {
        transcript: Some(PathBuf::from(
            r"C:\Users\o\.claude\projects\C--work-print\def-456.jsonl"
        )),
        ..sending()
    }));
    assert!(skips(Facts {
        transcript: None,
        ..sending()
    }));
    let unnamed = Record {
        transcript: None,
        ..record_of(&sending())
    };
    assert!(skips(Facts {
        record: Ok(Some(unnamed.clone())),
        ..sending()
    }));
    let owner = Record {
        by: "owner".into(),
        ..unnamed
    };
    assert_eq!(
        decide(&Facts {
            record: Ok(Some(owner)),
            transcript: None,
            ..sending()
        }),
        Decision::Send(at)
    );
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
    // Review S3: once the Enter went, idle compact stays off the pane for the rest of the stretch.
    assert!(sent.busy());
    assert!(!FreshStretch::default().busy());
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
