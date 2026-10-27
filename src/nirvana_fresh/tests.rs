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

/// Round 2's nits: an image the owner pasted with no words is the owner after the end for B1's rule, though the note
/// quotes no words of it; Claude Code's `<task-notification>` line, a `user` line without `isMeta`, is not the owner.
#[test]
fn an_image_is_the_owner_and_a_task_notification_is_not() {
    let end = 1_792_800_000;
    let quiet = quiet_tail(end);
    let image = serde_json::json!({"type": "user", "timestamp": iso(end + 5), "message": {"role": "user", "content": [
        {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBORw0KGgo="}},
        {"type": "text", "text": "  "},
    ]}})
    .to_string();
    let with_image = format!("{quiet}\n{image}");
    assert!(owner_prompt_since(&with_image, end).is_err());
    assert_eq!(owner_prompts(&with_image, PROMPTS), vec!["end the run"]);
    // An image beside words that are not the owner's (a reminder), in a tool's result, or on a meta line, stays not
    // the owner's.
    let reminded = serde_json::json!({"type": "user", "timestamp": iso(end + 5), "message": {"content": [
        {"type": "image", "source": {}}, {"type": "text", "text": "<system-reminder>x</system-reminder>"}]}})
    .to_string();
    assert_eq!(
        owner_prompt_since(&format!("{quiet}\n{reminded}"), end),
        Ok(())
    );
    let tool_image = serde_json::json!({"type": "user", "timestamp": iso(end + 5), "message": {"content": [
        {"type": "tool_result", "content": [{"type": "image", "source": {}}]}, {"type": "image", "source": {}}]}})
    .to_string();
    assert_eq!(
        owner_prompt_since(&format!("{quiet}\n{tool_image}"), end),
        Ok(())
    );
    let meta_image = serde_json::json!({"type": "user", "isMeta": true, "timestamp": iso(end + 5),
        "message": {"content": [{"type": "image", "source": {}}]}})
    .to_string();
    assert_eq!(
        owner_prompt_since(&format!("{quiet}\n{meta_image}"), end),
        Ok(())
    );
    // The shape recorded in a Claude Code transcript on this machine (2026-10-12), its paths and ids shortened.
    let notification = serde_json::json!({
        "parentUuid": "796ff8d6", "isSidechain": false, "promptId": "ca32d304", "type": "user",
        "message": {"role": "user", "content": "<task-notification>\n<task-id>b42ws8w3l</task-id>\n<tool-use-id>toolu_01Q8</tool-use-id>\n<output-file>C:\\t\\b42ws8w3l.output</output-file>\n<status>killed</status>\n<summary>Background command \"python main.py\" was stopped after reaching its background time limit</summary>\n</task-notification>"},
        "uuid": "9d8af2fb", "timestamp": iso(end + 5), "permissionMode": "bypassPermissions",
        "origin": {"kind": "task-notification"},
    })
    .to_string();
    let with_notification = format!("{quiet}\n{notification}");
    assert_eq!(owner_prompt_since(&with_notification, end), Ok(()));
    assert_eq!(
        owner_prompts(&with_notification, PROMPTS),
        vec!["end the run"]
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

/// Round 2's nit: on a `/clear`, a stale Stop's record prints no note and is forgotten, unless the server's mark names
/// it (the server cleared for it inside `FRESH_WITHIN`).
#[test]
fn a_clear_prints_no_note_for_a_stale_stop_record() {
    let root = scratch("start-stale");
    let home = root.join("home");
    let project = Project::at(root.join("project"));
    run(&project, "r1", "p1", Some(500));
    let record = Record {
        run: Some("r1".into()),
        ended_at: Some(500),
        context: Some(180_000),
        transcript: None,
        project: project.root.display().to_string(),
        at: 510,
        by: "stop".into(),
    };
    let here = Some(project.root.as_path());
    let old = 500 + FRESH_WITHIN.as_secs() + 1;
    write_record(&home, "p1", &record).unwrap();
    assert_eq!(
        on_session_start_in(&home, "p1", "clear", here, None, old),
        None
    );
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    write_record(&home, "p1", &record).unwrap();
    assert!(on_session_start_in(
        &home,
        "p1",
        "clear",
        here,
        None,
        500 + FRESH_WITHIN.as_secs()
    )
    .is_some());
    // The server cleared for it: its note is printed though the hook runs a little late.
    write_record(&home, "p1", &record).unwrap();
    write_sent(&home, "p1", record.at).unwrap();
    mark_written_at(&home, "p1", old - 5);
    assert!(on_session_start_in(&home, "p1", "clear", here, None, old).is_some());
    // Round 3's nit: a mark written more than `FRESH_WITHIN` ago is no exception.
    write_record(&home, "p1", &record).unwrap();
    write_sent(&home, "p1", record.at).unwrap();
    mark_written_at(&home, "p1", old - FRESH_WITHIN.as_secs() - 1);
    assert!(!sent_lately(&home, "p1", record.at, old));
    assert_eq!(
        on_session_start_in(&home, "p1", "clear", here, None, old),
        None
    );
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    write_sent(&home, "p1", record.at).unwrap();
    mark_written_at(&home, "p1", old - FRESH_WITHIN.as_secs());
    assert!(sent_lately(&home, "p1", record.at, old));
    // A mark naming another record is none.
    assert!(!sent_lately(&home, "p1", record.at + 1, old));
    // The owner's record is not aged here, though older than its own `OWNER_WITHIN`.
    let older = record.at + OWNER_WITHIN.as_secs() + 1;
    let owner = Record {
        by: "owner".into(),
        ..record
    };
    write_record(&home, "p1", &owner).unwrap();
    assert!(on_session_start_in(&home, "p1", "clear", here, None, older).is_some());
    let _ = std::fs::remove_dir_all(&root);
}

/// Sets the server's mark's modified time to `at`, Unix seconds.
fn mark_written_at(home: &Path, pane: &str, at: u64) {
    std::fs::File::options()
        .write(true)
        .open(sent_path(home, pane))
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(at))
        .unwrap();
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

/// `decide` with the send-time check (review round 2, S-A) passing: every other rule's own case.
fn decided(facts: &Facts) -> Decision {
    decide(facts, |_| Ok(()))
}

fn skips(facts: Facts) -> bool {
    matches!(decided(&facts), Decision::Skip(_))
}

#[test]
fn the_fresh_start_sends_when_every_rule_holds() {
    assert_eq!(decided(&sending()), Decision::Send(600));
    let never = Facts {
        owner_key: OwnerKey::Never,
        ..sending()
    };
    assert_eq!(decided(&never), Decision::Send(600));
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

/// Review S1: a Stop's record lives `FRESH_WITHIN`; the server forgets an older one. Round 2's S-C: the owner's lives
/// `OWNER_WITHIN`.
#[test]
fn a_stale_stop_record_is_forgotten() {
    let at = record_of(&sending()).at;
    // The journal's end read 100 s after the record's second (a clock stepped back between the two writers), so the
    // record's own age is what ages it here, not its end's (round 2's S-B, `a_stop_record_ages_from_its_runs_end`).
    let at_its_end = || Facts {
        record: Ok(Some(Record {
            ended_at: Some(at + 100),
            ..record_of(&sending())
        })),
        ..sending()
    };
    let aged = |now: u64| {
        decided(&Facts {
            now,
            ..at_its_end()
        })
    };
    assert_eq!(aged(at + FRESH_WITHIN.as_secs()), Decision::Send(at));
    assert!(matches!(
        aged(at + FRESH_WITHIN.as_secs() + 1),
        Decision::Forget(_)
    ));
    // Forgotten even when the server already cleared for it, so a mark never keeps a stale record alive.
    assert!(matches!(
        decided(&Facts {
            now: at + FRESH_WITHIN.as_secs() + 1,
            sent_for: Some(at),
            ..at_its_end()
        }),
        Decision::Forget(_)
    ));
    let owner = |now: u64| Facts {
        record: Ok(Some(Record {
            by: "owner".into(),
            ..record_of(&sending())
        })),
        now,
        ..sending()
    };
    // Past a Stop's age, and past its end's, the owner's own record still waits.
    assert_eq!(
        decided(&owner(at + 2 * FRESH_WITHIN.as_secs())),
        Decision::Send(at)
    );
    assert_eq!(
        decided(&owner(at + OWNER_WITHIN.as_secs())),
        Decision::Send(at)
    );
    assert!(matches!(
        decided(&owner(at + OWNER_WITHIN.as_secs() + 1)),
        Decision::Forget(_)
    ));
    assert_eq!(OWNER_WITHIN, Duration::from_secs(3_600));
}

/// Round 2's S-B: a Stop's record ages from its run's end too, not only from its own second (a record kept from an
/// earlier turn of the same run keeps its second); one naming no end is a doubt.
#[test]
fn a_stop_record_ages_from_its_runs_end() {
    let record = record_of(&sending());
    let (at, end) = (record.at, record.ended_at.unwrap());
    assert!(at > end);
    let late = end + FRESH_WITHIN.as_secs() + 1;
    assert_eq!(stale(&record, end + FRESH_WITHIN.as_secs()), None);
    let why = stale(&record, late).unwrap();
    assert!(why.contains("ended"), "{why}");
    // The record's own second is still young there: the end is what aged it.
    assert!(late - at <= FRESH_WITHIN.as_secs());
    assert!(matches!(
        decided(&Facts {
            now: late,
            ..sending()
        }),
        Decision::Forget(_)
    ));
    let unended = Record {
        ended_at: None,
        ..record.clone()
    };
    assert!(stale(&unended, at).is_some());
    // The end is aged at send time as well (S-A's check holds it).
    let quiet = quiet_tail(end);
    assert!(recheck(&record, late, None, Ok(&quiet)).is_err());
    assert_eq!(
        recheck(&record, end + FRESH_WITHIN.as_secs(), None, Ok(&quiet)),
        Ok(())
    );
}

/// A transcript tail whose last owner prompt and turn come before `end`.
fn quiet_tail(end: u64) -> String {
    [
        line_at("user", "end the run", end - 30),
        assistant_at(150_000, end - 1),
    ]
    .join("\n")
}

/// Round 2's S-A: B1's rules again at send time, the record still present: an open run, an owner prompt after the end,
/// an old end, an unread transcript each forget it.
#[test]
fn the_send_time_check_runs_every_b1_rule_again() {
    let record = record_of(&sending());
    let end = record.ended_at.unwrap();
    let now = sending().now;
    let quiet = quiet_tail(end);
    assert_eq!(recheck(&record, now, None, Ok(&quiet)), Ok(()));
    assert_eq!(
        recheck(&record, now, Some("r2"), Ok(&quiet)),
        Err("run r2 of this pane is open".to_owned())
    );
    let asked = format!(
        "{quiet}\n{}",
        line_at("user", "why did order 2 fail?", end + 5)
    );
    assert!(recheck(&record, now, None, Ok(&asked))
        .unwrap_err()
        .contains("after the run's end"));
    assert!(recheck(&record, end + FRESH_WITHIN.as_secs() + 1, None, Ok(&quiet)).is_err());
    assert!(recheck(&record, now, None, Err("unread".into())).is_err());
    // `decide` forgets on the check's failure, and asks it only once every other rule holds.
    let failing = decide(&sending(), |_| Err("typed".into()));
    assert_eq!(failing, Decision::Forget("typed".into()));
    let asked = std::cell::Cell::new(false);
    let left = decide(
        &Facts {
            input_line: InputLine::Text("half a thought".into()),
            ..sending()
        },
        |_| {
            asked.set(true);
            Ok(())
        },
    );
    assert!(matches!(left, Decision::Skip(_)));
    assert!(!asked.get());
    let given = std::cell::RefCell::new(None);
    decide(&sending(), |record| {
        *given.borrow_mut() = Some(record.clone());
        Ok(())
    });
    assert_eq!(given.into_inner(), Some(record));
}

/// Round 2's S-C: the owner's record is forgotten once the owner typed in the pane's conversation after it; an open
/// run does not hold the owner's own start off.
#[test]
fn an_owner_record_is_forgotten_once_the_owner_types_after_it() {
    let record = Record {
        by: "owner".into(),
        transcript: None,
        ..record_of(&sending())
    };
    let at = record.at;
    let now = at + 30;
    let before = quiet_tail(at);
    assert_eq!(recheck(&record, now, Some("r2"), Ok(&before)), Ok(()));
    let typed = format!("{before}\n{}", line_at("user", "no, keep going", at + 10));
    assert!(recheck(&record, now, None, Ok(&typed)).is_err());
    // A prompt in the record's own second is a doubt, so forgotten.
    let same = format!("{before}\n{}", line_at("user", "start fresh", at));
    assert!(recheck(&record, now, None, Ok(&same)).is_err());
    assert!(recheck(&record, now, None, Err("unread".into())).is_err());
    assert!(recheck(&record, at + OWNER_WITHIN.as_secs() + 1, None, Ok(&before)).is_err());
}

/// Round 2's S-A over the files: the open runs come from the record's project, the prompts from the transcript the
/// server read; an owner's record reads no journal.
#[test]
fn the_send_time_check_reads_the_projects_runs_and_the_panes_transcript() {
    let root = scratch("recheck-in");
    let project = Project::at(root.join("project"));
    let transcript = root.join("session.jsonl");
    let end = 1_792_800_000;
    std::fs::write(&transcript, quiet_tail(end)).unwrap();
    run(&project, "r1", "p1", Some(end));
    let record = Record {
        run: Some("r1".into()),
        ended_at: Some(end),
        context: Some(150_000),
        transcript: Some(transcript.display().to_string()),
        project: project.root.display().to_string(),
        at: end + 2,
        by: "stop".into(),
    };
    let now = end + 20;
    assert_eq!(recheck_in(&record, "p1", Some(&transcript), now), Ok(()));
    assert!(recheck_in(&record, "p1", None, now).is_err());
    assert!(recheck_in(&record, "p1", Some(&root.join("gone.jsonl")), now).is_err());
    run(&project, "r2", "p1", None);
    assert_eq!(
        recheck_in(&record, "p1", Some(&transcript), now),
        Err("run r2 of this pane is open".to_owned())
    );
    // Another pane's open run is not this pane's.
    assert_eq!(recheck_in(&record, "p2", Some(&transcript), now), Ok(()));
    let owner = Record {
        by: "owner".into(),
        at: end + 5,
        ..record
    };
    assert_eq!(recheck_in(&owner, "p1", Some(&transcript), now), Ok(()));
    let _ = std::fs::remove_dir_all(&root);
}

/// Round 2's nit: the line said when an open run holds the fresh start off names the run.
#[test]
fn an_open_run_holding_the_fresh_start_off_is_said() {
    assert_eq!(
        open_blocks(Some(("r1", 500)), Some("r2")),
        Some("no fresh start after run r1: run r2 of this pane is open".to_owned())
    );
    assert_eq!(open_blocks(None, Some("r2")), None);
    assert_eq!(open_blocks(Some(("r1", 500)), None), None);
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
    assert_eq!(decided(&spelt), Decision::Send(at));
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
        decided(&Facts {
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
        decided(&Facts {
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
        decided(&Facts {
            sent_for: Some(600),
            ..sending()
        }),
        Decision::Nothing
    );
    assert_eq!(
        decided(&Facts {
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

/// A pane with a run that ended at `end` and a session above `fresh_above`, the Stop hook's record written for it.
fn stopped(root: &Path) -> (PathBuf, Project, PathBuf, Record) {
    let home = root.join("home");
    let project = Project::at(root.join("project"));
    let transcript = root.join("session.jsonl");
    std::fs::write(&transcript, "").unwrap();
    let end = session_began(&transcript).unwrap() + 60;
    std::fs::write(
        &transcript,
        [
            line_at("user", "end the run", end - 30),
            assistant_at(180_000, end - 1),
        ]
        .join("\n"),
    )
    .unwrap();
    run(&project, "r1", "p1", Some(end));
    assert_eq!(
        on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 5).unwrap(),
        Ok("r1".to_owned())
    );
    let record = read_record(&home, "p1").unwrap().unwrap();
    (home, project, transcript, record)
}

/// Round 3, R1: runs that cannot all be read are a doubt, not "no open run": the Stop hook writes no record and
/// takes back its own, and the send-time check forgets the record. A project with no runs folder has no run.
#[test]
fn an_unread_run_list_is_a_doubt_at_stop_and_at_send() {
    let root = scratch("unread-runs");
    assert_eq!(
        read_run_states(&Project::at(root.join("none"))),
        Ok(Vec::new())
    );
    let (home, project, transcript, record) = stopped(&root);
    let end = record.ended_at.unwrap();
    let now = end + 20;
    assert_eq!(recheck_in(&record, "p1", Some(&transcript), now), Ok(()));
    let runs = project.dir().join("runs");
    // A journal with a line that does not parse may hide an open run of this pane.
    std::fs::create_dir_all(runs.join("r2")).unwrap();
    std::fs::write(
        runs.join("r2").join(nirvana_run::JOURNAL),
        "{\"v\":1,\"seq\":1,\n",
    )
    .unwrap();
    let why = read_run_states(&project).unwrap_err();
    assert!(why.contains("do not parse"), "{why}");
    assert!(recheck_in(&record, "p1", Some(&transcript), now).is_err());
    assert!(matches!(
        decide(&sending(), |_| recheck_in(
            &record,
            "p1",
            Some(&transcript),
            now
        )),
        Decision::Forget(_)
    ));
    assert!(
        on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 6)
            .unwrap()
            .is_err()
    );
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    // A runs folder that cannot be listed: a file stands where it should be.
    std::fs::remove_dir_all(&runs).unwrap();
    std::fs::write(&runs, "not a folder").unwrap();
    assert!(read_run_states(&project).is_err());
    // `Project::runs` reads it as no run at all, which is what made the doubt a pass.
    assert!(project.runs().is_empty());
    write_record(&home, "p1", &record).unwrap();
    assert!(
        on_stop_in(&home, "p1", &project, &transcript, FRESH_ABOVE, end + 7)
            .unwrap()
            .is_err()
    );
    assert_eq!(read_record(&home, "p1").unwrap(), None);
    assert!(recheck_in(&record, "p1", Some(&transcript), now).is_err());
    // The owner's record reads no journal, so it is not held by them.
    let owner = Record {
        by: "owner".into(),
        at: end + 5,
        ..record
    };
    assert_eq!(recheck_in(&owner, "p1", Some(&transcript), now), Ok(()));
    let _ = std::fs::remove_dir_all(&root);
}

/// Round 3, R1: a Stop's record is sent for only while the replay finds its own run, ended.
#[test]
fn a_stop_record_needs_its_own_run_ended_in_the_replay() {
    let root = scratch("own-run");
    let (_home, project, transcript, record) = stopped(&root);
    let now = record.ended_at.unwrap() + 20;
    let states = read_run_states(&project).unwrap();
    assert_eq!(own_run_ended(&record, &states), Ok(()));
    let missing = Record {
        run: Some("r9".into()),
        ..record.clone()
    };
    assert_eq!(
        own_run_ended(&missing, &states),
        Err("run r9 is not among the project's runs".to_owned())
    );
    assert_eq!(
        recheck_in(&missing, "p1", Some(&transcript), now),
        Err("run r9 is not among the project's runs".to_owned())
    );
    let unnamed = Record {
        run: None,
        ..record.clone()
    };
    assert!(own_run_ended(&unnamed, &states).is_err());
    run(&project, "r3", "p2", None);
    let open = Record {
        run: Some("r3".into()),
        ..record.clone()
    };
    assert_eq!(
        own_run_ended(&open, &read_run_states(&project).unwrap()),
        Err("run r3 has not ended".to_owned())
    );
    assert!(recheck_in(&open, "p1", Some(&transcript), now).is_err());
    // The journal gone: the run is no longer in the replay.
    std::fs::remove_dir_all(project.run_dir("r1")).unwrap();
    assert!(recheck_in(&record, "p1", Some(&transcript), now).is_err());
    let _ = std::fs::remove_dir_all(&root);
}

/// A `!` command's two lines as Claude Code records them, the command and then its output, at `at`. The shape of
/// its other `user` lines (no `!` line was found recorded on this machine), ids and paths shortened.
fn bang_pair(command: &str, out: &str, at: u64) -> String {
    let line = |content: String| {
        serde_json::json!({
            "parentUuid": "6b1f0c2e", "isSidechain": false, "userType": "external", "cwd": r"C:\work\print",
            "sessionId": "abc-123", "version": "2.1.302", "gitBranch": "main", "type": "user",
            "message": {"role": "user", "content": content}, "uuid": "e4a7d9b0", "timestamp": iso(at),
        })
        .to_string()
    };
    [
        line(format!("<bash-input>{command}</bash-input>")),
        line(format!(
            "<bash-stdout>{out}</bash-stdout><bash-stderr></bash-stderr>"
        )),
    ]
    .join("\n")
}

/// Round 3, R2: the owner's own `!herdr nirvana fresh`, recorded in the record's second or just after, does not
/// forget the record it wrote; a prompt the owner types after it still does. After a run's end a `!` command is still
/// the owner, a doubt for a Stop's record.
#[test]
fn the_owners_own_bang_fresh_keeps_their_record() {
    let at = 1_792_800_000;
    let before = quiet_tail(at);
    let said =
        "herdr nirvana fresh: pending for pane p1; herdr types /clear once Nirvana Claude is idle";
    let owner = Record {
        run: None,
        ended_at: None,
        context: None,
        transcript: None,
        project: "p".into(),
        at,
        by: "owner".into(),
    };
    let now = at + 30;
    for second in [at, at + 1] {
        let asked = format!(
            "{before}\n{}",
            bang_pair("herdr nirvana fresh", said, second)
        );
        assert_eq!(owner_typed_since(&asked, at), Ok(()), "{second}");
        assert_eq!(recheck(&owner, now, None, Ok(&asked)), Ok(()), "{second}");
    }
    let asked = format!("{before}\n{}", bang_pair("herdr nirvana fresh", said, at));
    let typed = format!(
        "{asked}\n{}",
        line_at("user", "wait, one more thing", at + 3)
    );
    assert!(recheck(&owner, now, None, Ok(&typed)).is_err());
    // Each lead on its own.
    for lead in BANG_LEADS {
        let tail = format!("{before}\n{}", line_at("user", &format!("{lead}x"), at));
        assert_eq!(owner_typed_since(&tail, at), Ok(()), "{lead}");
        assert!(owner_prompt_since(&tail, at).is_err(), "{lead}");
    }
    assert_eq!(
        BANG_LEADS,
        ["<bash-input>", "<bash-stdout>", "<bash-stderr>"]
    );
    // A Stop's record still reads a `!` command after the end as the owner.
    let stop = Record {
        run: Some("r1".into()),
        ended_at: Some(at),
        at: at + 2,
        by: "stop".into(),
        ..owner
    };
    assert!(owner_prompt_since(&asked, at).is_err());
    assert!(recheck(&stop, now, None, Ok(&asked)).is_err());
}
