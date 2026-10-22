use super::*;

// Recorded streams. P2 and P3: plan 12's O0 probes, 2026-10-17. live1: this package's live run, 2026-10-18,
// haiku low and flash low each ending with a Work Report.
const P2_FLASH_EMPTY: &str = include_str!("fixtures/p2_flash_low_mp4.stream.jsonl");
const P3_AGY_SCHEMA: &str = include_str!("fixtures/p3_agy_schema.stream.jsonl");
const P3_CLAUDE_SCHEMA: &str = include_str!("fixtures/p3_claude_schema.stream.jsonl");
const LIVE_HAIKU: &str = include_str!("fixtures/live1_sub_haiku.stream.jsonl");
const LIVE_FLASH: &str = include_str!("fixtures/live1_mul_flash.stream.jsonl");

fn spec(cli: &str, model: &str, resume: Option<&str>) -> LaunchSpec {
    LaunchSpec {
        cli: cli.into(),
        model: model.into(),
        effort: Effort::Low,
        prompt: first_line("C:\\p\\.nirvana\\orders\\sub.md"),
        resume: resume.map(str::to_owned),
        print_timeout_s: 3600,
    }
}

fn after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
    argv.iter()
        .position(|arg| arg == flag)
        .and_then(|at| argv.get(at + 1))
        .map(String::as_str)
}

fn feed(cli: &str, text: &str) -> (Stream, Vec<Note>) {
    let mut stream = Stream::new(cli);
    let mut notes = Vec::new();
    for line in text.lines() {
        notes.extend(stream.feed(line));
    }
    (stream, notes)
}

// ---- the launch ----

#[test]
fn a_claude_worker_starts_with_the_launch_lessons() {
    let argv = argv(&spec("claude", "claude-haiku-5-5", None)).expect("argv");
    assert_eq!(argv[0], "claude");
    assert_eq!(argv[1], "-p");
    // The prompt is one element of the list, one short line naming the file (WinError 206, §2).
    assert_eq!(argv[2], first_line("C:\\p\\.nirvana\\orders\\sub.md"));
    assert!(argv[2].len() < 300);
    assert_eq!(after(&argv, "--output-format"), Some("stream-json"));
    assert!(argv.iter().any(|arg| arg == "--verbose"));
    assert_eq!(after(&argv, "--setting-sources"), Some("project"));
    assert!(argv.iter().any(|arg| arg == "--strict-mcp-config"));
    assert_eq!(after(&argv, "--model"), Some("claude-haiku-5-5"));
    assert_eq!(after(&argv, "--effort"), Some("low"));
    assert_eq!(after(&argv, "--permission-mode"), Some("auto"));
    assert_eq!(after(&argv, "--json-schema"), Some(REPORT_SCHEMA));
    assert!(!argv
        .iter()
        .any(|arg| arg == "--continue" || arg == "-c" || arg == "--resume"));
    let resumed = argv_for_resume("claude", "claude-haiku-5-5");
    assert_eq!(after(&resumed, "--resume"), Some("c0a14f6a"));
    assert!(!resumed.iter().any(|arg| arg == "--continue" || arg == "-c"));
    assert_eq!(CLAUDE_ENV, [("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1")]);
}

fn argv_for_resume(cli: &str, model: &str) -> Vec<String> {
    argv(&spec(cli, model, Some("c0a14f6a"))).expect("argv")
}

#[test]
fn an_agy_worker_takes_its_line_as_one_print_token_and_its_effort_in_the_model() {
    let argv = argv(&spec("agy", "gemini-3.8-flash-low", None)).expect("argv");
    assert_eq!(argv[0], "agy");
    let last = argv.last().expect("last");
    assert_eq!(
        last,
        &format!("--print={}", first_line("C:\\p\\.nirvana\\orders\\sub.md"))
    );
    assert!(!argv.iter().any(|arg| arg == "--print" || arg == "-p"));
    assert_eq!(after(&argv, "--output-format"), Some("stream-json"));
    assert_eq!(after(&argv, "--model"), Some("gemini-3.8-flash-low"));
    assert!(!argv.iter().any(|arg| arg == "--effort"));
    assert_eq!(after(&argv, "--print-timeout"), Some("3600s"));
    assert!(argv
        .iter()
        .any(|arg| arg == "--dangerously-skip-permissions"));
    assert_eq!(after(&argv, "--json-schema"), Some(REPORT_SCHEMA));
    assert!(!argv
        .iter()
        .any(|arg| arg == "--continue" || arg == "-c" || arg == "--conversation"));
    let resumed = argv_for_resume("agy", "gemini-3.8-flash-low");
    assert_eq!(after(&resumed, "--conversation"), Some("c0a14f6a"));
    assert!(!resumed.iter().any(|arg| arg == "--continue" || arg == "-c"));
    assert!(resumed
        .last()
        .is_some_and(|arg| arg.starts_with("--print=")));
}

#[test]
fn a_worker_is_never_started_through_a_shell() {
    for cli in ["claude", "agy"] {
        let argv = argv(&spec(cli, "m", None)).expect("argv");
        let program = argv[0].to_ascii_lowercase();
        for shell in ["cmd", "powershell", "pwsh", "bash", "sh"] {
            assert!(!program.contains(shell), "{program}");
        }
        assert!(!argv.iter().any(|arg| arg == "/c" || arg == "-Command"));
    }
    assert!(argv(&spec("codex", "m", None)).is_err());
    assert!(ENV_REMOVED.contains(&"CLAUDECODE"));
}

// ---- the Work Report ----

fn good_report() -> Value {
    serde_json::json!({
        "status": "done",
        "summary": "Added sub(a, b).",
        "files": ["calc.py", "test_calc.py"],
        "tests": [{"name": "python test_calc.py", "result": "pass"}],
        "gotchas": [],
        "lesson": {"name": "x", "class": "chore.script", "steps": "1. a\n2. b", "evidence": "ok"}
    })
}

#[test]
fn the_report_check_and_the_schema_name_the_same_fields() {
    let schema: Value = serde_json::from_str(REPORT_SCHEMA).expect("schema is JSON");
    let mut properties: Vec<&str> = schema["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .map(String::as_str)
        .collect();
    properties.sort_unstable();
    let mut keys = REPORT_KEYS.to_vec();
    keys.sort_unstable();
    assert_eq!(properties, keys);
    let required: Vec<&str> = schema["required"]
        .as_array()
        .expect("required")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(required, ["status", "summary", "files", "tests", "gotchas"]);
    assert_eq!(
        schema["properties"]["status"]["enum"],
        serde_json::json!(["done", "blocked", "needs_answer"])
    );
    // Each required field, removed, fails the check.
    for key in required {
        let mut report = good_report();
        report.as_object_mut().expect("object").remove(key);
        assert!(check_report(&report).is_err(), "{key}");
    }
}

#[test]
fn a_malformed_report_fails() {
    assert!(check_report(&good_report()).is_ok());
    let cases: Vec<(&str, Value)> = vec![
        ("empty", serde_json::json!({})),
        ("not an object", serde_json::json!("done")),
        ("unknown key", {
            let mut r = good_report();
            r["extra"] = 1.into();
            r
        }),
        ("bad status", {
            let mut r = good_report();
            r["status"] = "ok".into();
            r
        }),
        ("blank summary", {
            let mut r = good_report();
            r["summary"] = "  ".into();
            r
        }),
        ("test result", {
            let mut r = good_report();
            r["tests"] = serde_json::json!([{"name": "t", "result": "green"}]);
            r
        }),
        ("file not text", {
            let mut r = good_report();
            r["files"] = serde_json::json!([1]);
            r
        }),
        ("lesson too long", {
            let mut r = good_report();
            r["lesson"]["steps"] = "step\n".repeat(121).into();
            r
        }),
        ("lesson no evidence", {
            let mut r = good_report();
            r["lesson"]["evidence"] = "".into();
            r
        }),
    ];
    for (name, report) in cases {
        assert!(check_report(&report).is_err(), "{name}");
    }
}

#[test]
fn an_empty_answer_is_a_failure_even_when_agy_says_success() {
    let (stream, _) = feed("agy", P2_FLASH_EMPTY);
    let result = stream.result.as_ref().expect("a result line");
    assert_eq!(result["status"], "SUCCESS");
    let why = stream
        .report()
        .expect_err("an empty answer is not a report");
    assert!(why.contains("empty answer"), "{why}");
    // P3's schema run answered with another schema's object: not a Work Report.
    let (stream, _) = feed("agy", P3_AGY_SCHEMA);
    assert!(stream.report().is_err());
    // A claude result with no structured output and no JSON answer is no report.
    let (stream, _) = feed(
        "claude",
        r#"{"type":"result","subtype":"success","is_error":false,"result":"Done."}"#,
    );
    assert!(stream.report().is_err());
    let (stream, _) = feed(
        "claude",
        r#"{"type":"result","subtype":"error_max_turns","is_error":true,"result":""}"#,
    );
    assert!(stream
        .report()
        .expect_err("error")
        .contains("error_max_turns"));
    let (stream, _) = feed("claude", "");
    assert!(stream.report().expect_err("none").contains("no result"));
    // agy's status decides even when a structured answer came: a 503 once exited 0 (old Nirvana's adapter).
    let failed = format!(
        r#"{{"event":"result","result":{{"status":"ERROR","response":"","structured_output":{}}}}}"#,
        good_report()
    );
    let (stream, _) = feed("agy", &failed);
    assert!(stream.report().expect_err("status").contains("agy status"));
}

#[test]
fn agys_own_keys_are_dropped_from_an_answer_given_as_text() {
    // P3's shape: the response is the object plus agy's `toolAction` and `toolSummary`, here with no
    // structured output beside it.
    let mut answer = good_report();
    answer["toolAction"] = "Finishing task".into();
    answer["toolSummary"] = "Finish task".into();
    let line = serde_json::json!({"event": "result", "result": {"status": "SUCCESS", "response": answer.to_string()}});
    let (stream, _) = feed("agy", &line.to_string());
    let report = stream.report().expect("a report");
    assert!(report.get("toolAction").is_none() && report.get("toolSummary").is_none());
    assert_eq!(report["status"], "done");
}

#[test]
fn the_live_streams_give_their_report_tokens_and_session() {
    let (stream, notes) = feed("claude", LIVE_HAIKU);
    let report = stream.report().expect("haiku's report");
    assert_eq!(report["status"], "done");
    assert_eq!(
        report["files"],
        serde_json::json!(["calc.py", "test_calc.py"])
    );
    assert_eq!(
        stream.tokens,
        Tokens {
            input: 16,
            cache_write: 25_551,
            cache_read: 146_424,
            output: 1_513
        }
    );
    assert_eq!(
        stream.session.as_deref(),
        Some("c0a14f6a-539e-4f63-b6ae-72df9ad7168f")
    );
    assert!(notes
        .iter()
        .any(|note| matches!(note, Note::Tool(t) if t.starts_with("Edit"))));
    // agy: the toolAction and toolSummary keys it adds are not the worker's.
    let (stream, _) = feed("agy", LIVE_FLASH);
    let report = stream.report().expect("flash's report");
    assert_eq!(report["status"], "done");
    assert!(report.get("toolAction").is_none());
    assert_eq!(stream.tokens.output, 958);
    assert_eq!(stream.tokens.input, 72_437);
    assert_eq!(stream.tokens.cache_read, 59_004);
}

// ---- rate limits, typed only (P5) ----

/// P3's recorded `rate_limit_event`, status `allowed`.
fn recorded_limit_line() -> &'static str {
    P3_CLAUDE_SCHEMA
        .lines()
        .find(|line| line.contains("\"rate_limit_event\""))
        .expect("P3 recorded a rate_limit_event")
}

#[test]
fn only_a_rejected_rate_limit_event_or_a_rate_limited_retry_stops_a_claude_worker() {
    let allowed = recorded_limit_line();
    let (_, notes) = feed("claude", allowed);
    assert!(!notes.iter().any(|n| matches!(n, Note::RateLimited(_))));
    // The same line with the status a spent window carries (none was spent on purpose: P5).
    let rejected = allowed.replace("\"status\":\"allowed\"", "\"status\":\"rejected\"");
    let (_, notes) = feed("claude", &rejected);
    let limit = notes
        .iter()
        .find_map(|n| match n {
            Note::RateLimited(limit) => Some(limit.clone()),
            _ => None,
        })
        .expect("rejected is a signal");
    assert_eq!(limit.signal, "rate_limit_event");
    assert_eq!(limit.window.as_deref(), Some("5h"));
    assert_eq!(limit.resets_at, Some(1_792_284_600));
    assert_eq!(limit.pool.as_deref(), Some("max"));
    // An api_retry for a rate limit, after the recorded event: the reset comes from the event.
    let retry = r#"{"type":"system","subtype":"api_retry","attempt":1,"max_retries":10,"retry_delay_ms":1000,"error_status":429,"error":"rate_limit","session_id":"s"}"#;
    let (_, notes) = feed("claude", &format!("{allowed}\n{retry}"));
    let limit = notes
        .iter()
        .find_map(|n| match n {
            Note::RateLimited(limit) => Some(limit.clone()),
            _ => None,
        })
        .expect("a rate-limited retry is a signal");
    assert_eq!(limit.signal, "api_retry");
    assert_eq!(limit.resets_at, Some(1_792_284_600));
    // A retry for another error, and text that only talks about limits, are not.
    let other = retry.replace("\"rate_limit\"", "\"server_error\"");
    let words = r#"{"type":"assistant","message":{"id":"m","content":[{"type":"text","text":"rate limit rejected 429"}]}}"#;
    let (_, notes) = feed("claude", &format!("{other}\n{words}"));
    assert!(!notes.iter().any(|n| matches!(n, Note::RateLimited(_))));
}

#[test]
fn agy_runs_out_only_with_exit_3_and_a_quota_error_line() {
    let line = r#"AGY_ERROR: {"status":"RESOURCE_EXHAUSTED","http_code":429,"retryable":false,"error_id":"e1"}"#;
    let limit = agy_rate_limit(Some(3), &format!("noise\n{line}\n"), "gemini").expect("signal");
    assert_eq!(limit.signal, "agy_exit_3");
    assert_eq!(limit.pool.as_deref(), Some("gemini"));
    assert!(agy_rate_limit(Some(1), line, "gemini").is_none());
    assert!(agy_rate_limit(Some(0), line, "gemini").is_none());
    let internal = r#"AGY_ERROR: {"status":"INTERNAL","http_code":500}"#;
    assert!(agy_rate_limit(Some(3), internal, "gemini").is_none());
    assert!(agy_rate_limit(Some(3), "quota exceeded, said in prose", "gemini").is_none());
}

#[test]
fn a_spent_pool_reseats_on_another_pool() {
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("default table");
    let now = 1_792_289_318;
    let reading = |pool: &str, window: &str| Reading {
        pool: pool.into(),
        window: window.into(),
        used: Some(0.1),
        resets_at: Some(now + 3600),
        resets_text: None,
        read_at: now,
        source: "test".into(),
    };
    let world = nirvana_seat::World {
        now,
        readings: vec![
            reading("max", "5h"),
            reading("max", "week"),
            reading("max", "week.fable"),
            reading("gemini", "5h"),
            reading("gemini", "week"),
            reading("google3p", "5h"),
            reading("google3p", "week"),
            reading("argon", "week"),
        ],
        ..nirvana_seat::World::default()
    };
    let request = nirvana_seat::Request::new("chore.script");
    // Plan 19 §4: chore.script at balanced picks flash on gemini.
    let before = nirvana_seat::pick(&table, &world, &request).expect("a seat");
    assert_eq!(before.pool, "gemini");
    let limit = RateLimit {
        signal: "rate_limit_event".into(),
        pool: Some("gemini".into()),
        window: Some("5h".into()),
        resets_at: Some(now + 7200),
        detail: String::new(),
    };
    let spent = spent_world(&table, world.clone(), &limit, "gemini");
    let after = nirvana_seat::pick(&table, &spent, &request).expect("another seat");
    assert_ne!(after.pool, "gemini", "{}", after.reason);
    // agy's exit 3 names no window: every window of the pool reads spent.
    let agy = RateLimit {
        window: None,
        resets_at: None,
        pool: Some("gemini".into()),
        ..limit
    };
    let spent = spent_world(&table, world, &agy, "gemini");
    assert!(["5h", "week"].iter().all(|window| spent
        .readings
        .iter()
        .any(|r| r.pool == "gemini" && r.window == *window && r.used == Some(1.0))));
}

// ---- idempotency and liveness ----

fn seated(order: &str) -> Seated {
    Seated {
        order: order.into(),
        seat: "flash".into(),
        cli: "agy".into(),
        model: "gemini-3.8-flash-low".into(),
        effort: Effort::Low,
        pool: "gemini".into(),
        class: "sweep.mechanical".into(),
        chain: Some("chore".into()),
        size: SizeName::Small,
        mode: Mode::Balanced,
        needs: Vec::new(),
        not_family: vec!["anthropic".into()],
        reason: "sweep.mechanical → flash low".into(),
        override_reason: None,
        order_file: "C:/p/.nirvana/orders/mul.md".into(),
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
    }
}

fn opened() -> Kind {
    Kind::Opened {
        project: "C:/p".into(),
        orchestrator_pane: None,
        auto: true,
        pools: Vec::new(),
        pools_basis: None,
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "herdr-o3-worker-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn a_second_run_for_an_order_returns_the_first_and_writes_nothing() {
    let dir = temp_dir("idem");
    let journal = Journal::new(dir.join("journal.jsonl"));
    let first = journal
        .transact(1, |events| {
            Ok((seating(events, &seated("mul"), opened()).expect("new"), ()))
        })
        .expect("seat")
        .0;
    assert_eq!(first.len(), 2, "opened, then seated");
    let mut other = seated("mul");
    other.seat = "argon".into();
    let (written, known) = journal
        .transact(2, |events| {
            Ok(match seating(events, &other, opened()) {
                Ok(kinds) => (kinds, None),
                Err(known) => (Vec::new(), Some(known)),
            })
        })
        .expect("again");
    assert!(written.is_empty());
    let known = known.expect("the first");
    assert_eq!(known.seated.seat, "flash");
    assert_eq!(existing("r", &known, true)["booked"], "existing");
    // A second order in the run is seated, with no second `opened`.
    let next = journal
        .transact(3, |events| {
            Ok((seating(events, &seated("div"), opened()).expect("new"), ()))
        })
        .expect("seat")
        .0;
    assert_eq!(next.len(), 1);
    // Racing threads seat it once.
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let journal = journal.clone();
            std::thread::spawn(move || {
                journal
                    .transact(4, |events| {
                        Ok((
                            seating(events, &seated("race"), opened()).unwrap_or_default(),
                            (),
                        ))
                    })
                    .expect("transact")
                    .0
                    .len()
            })
        })
        .collect();
    let seated_count: usize = handles.into_iter().map(|h| h.join().expect("thread")).sum();
    assert_eq!(seated_count, 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_orders_supervisor_is_live_while_it_holds_its_lock() {
    let dir = temp_dir("live");
    let project = Project::at(&dir);
    assert!(!live(&project, "r1", "mul"));
    let lock = take_live(&project, "r1", "mul")
        .expect("take")
        .expect("free");
    assert!(live(&project, "r1", "mul"));
    assert!(take_live(&project, "r1", "mul").expect("take").is_none());
    drop(lock);
    assert!(!live(&project, "r1", "mul"));
    assert!(take_live(&project, "r1", "mul").expect("take").is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_order_id_comes_from_its_file_name() {
    assert_eq!(order_id_from(Path::new("orders/Add Sub.md")), "add-sub");
    assert_eq!(order_id_from(Path::new("x/__.md")), "order");
    assert!(valid_id(&order_id_from(Path::new(
        "C:/a/b/Weird Name (2).md"
    ))));
}

#[test]
fn a_copied_order_carries_herdrs_rules_once() {
    let dir = temp_dir("order");
    let project = Project::at(&dir);
    let path = write_order(&project, &dir, "mul", "# Order\nDo it.\n").expect("write");
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(text.starts_with("# Order\nDo it.\n"));
    assert_eq!(text.matches(FOOTER_MARK).count(), 1);
    assert!(text.contains("do not start subagents or background processes"));
    let again = write_order(&project, &dir, "mul", &text).expect("again");
    assert_eq!(
        std::fs::read_to_string(again)
            .expect("read")
            .matches(FOOTER_MARK)
            .count(),
        1
    );
    // A worktree gets its own copy and its own ignore file.
    let tree = dir.join("wt");
    std::fs::create_dir_all(&tree).expect("tree");
    let in_tree = write_order(&project, &tree, "mul", "x").expect("tree order");
    assert!(Path::new(&in_tree).starts_with(&tree));
    assert!(tree.join(".nirvana").join(".gitignore").is_file());
    let _ = std::fs::remove_dir_all(dir);
}

// ---- one attempt, end to end, with a stand-in worker (Windows: `cmd /c` prints a recorded stream) ----

#[cfg(windows)]
fn stand_in(dir: &Path, script: &str) -> (Journal, PathBuf, PathBuf, Vec<String>) {
    let journal = Journal::new(dir.join("journal.jsonl"));
    let stream = dir.join("stream.jsonl");
    let stop = dir.join("stop");
    (
        journal,
        stream,
        stop,
        vec!["cmd.exe".into(), "/c".into(), script.into()],
    )
}

#[cfg(windows)]
fn in_dir(dir: &Path, cli: &str) -> Seated {
    let mut seated = seated("sub");
    seated.cli = cli.into();
    seated.cwd = dir.display().to_string();
    seated
}

#[cfg(windows)]
#[test]
fn an_attempt_tees_the_stream_and_journals_its_start_and_its_usage() {
    let dir = temp_dir("attempt");
    std::fs::write(dir.join("recorded.jsonl"), LIVE_HAIKU).expect("fixture");
    let (journal, stream_path, stop, argv) = stand_in(&dir, "type recorded.jsonl");
    let mut view = Vec::new();
    let (end, stream) = run_attempt(
        &journal,
        &stream_path,
        &in_dir(&dir, "claude"),
        1,
        &argv,
        &stop,
        &mut view,
    )
    .expect("attempt");
    assert!(
        matches!(end, AttemptEnd::Exited { code: Some(0), .. }),
        "{end:?}"
    );
    assert_eq!(stream.report().expect("report")["status"], "done");
    // The raw stream, line for line.
    assert_eq!(
        std::fs::read_to_string(&stream_path)
            .expect("tee")
            .lines()
            .collect::<Vec<_>>(),
        LIVE_HAIKU.lines().collect::<Vec<_>>()
    );
    let state = replay(&journal.read().0);
    let events = journal.read().0;
    assert!(matches!(&events[0].kind, Kind::Started { attempt: 1, argv: a, .. } if a == &argv));
    assert!(matches!(
        &events.last().expect("usage").kind,
        Kind::Usage { tokens, .. } if tokens.output == 1_513
    ));
    // The journal names the order it was given; replay keeps only orders it saw seated.
    assert!(state.orders.is_empty());
    // The pane gets the calls; the call that only carries the report does not show.
    let view = String::from_utf8_lossy(&view);
    assert!(
        view.contains("Edit(") && view.contains("calc.py)"),
        "{view}"
    );
    assert!(!view.contains("StructuredOutput"), "{view}");
    assert!(!view.contains("Structured output provided"), "{view}");
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(windows)]
#[test]
fn a_rejected_limit_stops_the_worker_at_once() {
    let dir = temp_dir("limit");
    let rejected =
        recorded_limit_line().replace("\"status\":\"allowed\"", "\"status\":\"rejected\"");
    std::fs::write(dir.join("recorded.jsonl"), format!("{rejected}\n")).expect("fixture");
    // The worker would go on for 20 s; the signal stops it.
    let (journal, stream_path, stop, argv) =
        stand_in(&dir, "type recorded.jsonl & ping -n 20 127.0.0.1 >nul");
    let started = Instant::now();
    let (end, _) = run_attempt(
        &journal,
        &stream_path,
        &in_dir(&dir, "claude"),
        1,
        &argv,
        &stop,
        &mut Vec::new(),
    )
    .expect("attempt");
    assert!(
        matches!(&end, AttemptEnd::RateLimited(limit) if limit.window.as_deref() == Some("5h")),
        "{end:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(windows)]
#[test]
fn a_stop_ends_the_workers_whole_process_tree() {
    let dir = temp_dir("stop");
    // `ping` is cmd's child and holds the stream's pipe: only the job object ending the whole tree lets the
    // attempt return before ping's 30 s.
    let (journal, stream_path, stop, argv) = stand_in(&dir, "ping -n 30 127.0.0.1");
    std::fs::write(&stop, "1").expect("stop");
    let started = Instant::now();
    let (end, _) = run_attempt(
        &journal,
        &stream_path,
        &in_dir(&dir, "agy"),
        1,
        &argv,
        &stop,
        &mut Vec::new(),
    )
    .expect("attempt");
    assert!(matches!(end, AttemptEnd::Stopped), "{end:?}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    let _ = std::fs::remove_dir_all(dir);
}

// ---- the tab's view (0.35.0) ----

// The owner, 2026-10-22: "I htought agy was similar to claude code in its shell reporting".
#[test]
fn a_recorded_stream_draws_like_claude_code() {
    let draw = |cli: &str, cwd: &str, text: &str| {
        let mut stream = Stream::new(cli).in_dir(cwd);
        text.lines()
            .flat_map(|line| stream.feed(line))
            .flat_map(|note| view::note_lines(&note, false))
            .collect::<Vec<_>>()
    };
    let agy = draw(
        "agy",
        r"C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-o3\target\o3-live\proj\.nirvana\worktrees\mul",
        LIVE_FLASH,
    );
    let has = |lines: &[String], line: &str| lines.iter().any(|shown| shown == line);
    for line in [
        "● Read(.nirvana/orders/mul.md)",
        "  ⎿  9 lines",
        "● Edit(calc.py)",
        "● Write(test_mul.py)",
        "● Bash(python test_mul.py)",
        "  ⎿  ok",
        "● Report: done",
    ] {
        assert!(has(&agy, line), "agy view lacks {line:?}: {agy:#?}");
    }
    // No bare agy tool name is left.
    assert!(!agy
        .iter()
        .any(|line| line.contains("view_file") || line.contains("run_command")));

    let claude = draw(
        "claude",
        r"C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-o3\target\o3-live\proj\.nirvana\worktrees\sub",
        LIVE_HAIKU,
    );
    // Haiku sent Edit and Write together: the answer that is not under its own call names it.
    for line in [
        "● Edit(calc.py)",
        "● Write(test_calc.py)",
        "  ⎿  Edit(calc.py): done",
        "  ⎿  done",
        "  - Status: done",
    ] {
        assert!(
            has(&claude, line),
            "claude view lacks {line:?}: {claude:#?}"
        );
    }
}

#[test]
fn a_pane_is_taken_over_only_when_its_worker_ended_and_a_shell_alone_runs() {
    use crate::nirvana_run::Phase;
    let named = |names: &[&str]| -> Vec<Value> {
        names
            .iter()
            .map(|name| serde_json::json!({"pid": 1, "name": name}))
            .collect()
    };
    assert!(only_shells(&named(&["pwsh.exe"])));
    assert!(only_shells(&named(&["bash"])));
    assert!(!only_shells(&named(&["pwsh.exe", "herdr.exe"])));
    assert!(!only_shells(&named(&["node.exe"])));
    for phase in [Phase::Delivered, Phase::Gated, Phase::Merged, Phase::Failed] {
        assert!(finished(phase), "{phase:?}");
    }
    // A deferred worker starts again in its own pane; an asked one waits there for its answer.
    for phase in [Phase::Seated, Phase::Running, Phase::Deferred, Phase::Asked] {
        assert!(!finished(phase), "{phase:?}");
    }
}

#[test]
fn worker_supervisor_uses_project_mode_when_order_names_none() {
    let guard = crate::nirvana::with_test_home("p23-worker-proj-mode");
    let home_seats = guard
        .path
        .join("nirvana")
        .join(crate::nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(home_seats.parent().unwrap()).unwrap();
    std::fs::write(&home_seats, crate::nirvana_seat::DEFAULT).unwrap();

    let dir = std::env::temp_dir().join(format!("nirvana-p23-worker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let project = Project::at(&dir);
    project.ensure().unwrap();

    let proj_seats = dir.join(".nirvana").join(crate::nirvana_seat::SEATS_FILE);
    std::fs::write(&proj_seats, "mode = \"sota\"\n").unwrap();

    let order_file = dir.join(".nirvana/orders/test-order.md");
    std::fs::create_dir_all(order_file.parent().unwrap()).unwrap();
    std::fs::write(&order_file, "# Test Order\n").unwrap();

    let request = SeatRequest {
        seat: "flash".into(),
        order_file,
        order: "test-order".into(),
        run: "r1".into(),
        class: Some("sweep.mechanical".into()),
        size: None,
        needs: Vec::new(),
        not_family: Vec::new(),
        mode: None,
        effort: None,
        reason: None,
        worktree: false,
        auto: false,
    };

    let booked = book(&project, &request).expect("book succeeds");
    assert_eq!(booked.mode, Mode::Sota);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rate_limited_reseat_drops_override_on_same_pool_and_keeps_on_other() {
    let seated = Seated {
        order: "order-1".into(),
        seat: "sonnet".into(),
        cli: "claude".into(),
        model: "claude-sonnet-5-5".into(),
        effort: Effort::Medium,
        pool: "max".into(),
        class: "build.bounded".into(),
        chain: Some("build.bounded".into()),
        size: SizeName::Small,
        mode: Mode::Balanced,
        needs: Vec::new(),
        not_family: Vec::new(),
        reason: "reason".into(),
        override_reason: Some("forced sonnet".into()),
        order_file: "o.md".into(),
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
    };

    // Case 1: rate limit on "max" (the override seat's own pool) -> override dropped
    let (dropped, inherited, target) = rate_limited_override(&seated, "max");
    assert!(dropped);
    assert!(!inherited);
    assert_eq!(target, None);

    // Case 2: rate limit on another pool (e.g. "gemini") -> override kept
    let (dropped2, inherited2, target2) = rate_limited_override(&seated, "gemini");
    assert!(!dropped2);
    assert!(inherited2);
    assert_eq!(target2.as_deref(), Some("sonnet:medium"));
}

// ---- Plan 24 §5, §6: worker report sources & seat_order owns tests ----

#[test]
fn worker_report_schema_sources_validation() {
    // Valid report with sources array
    let valid = serde_json::json!({
        "status": "done",
        "summary": "researched web",
        "files": [],
        "tests": [],
        "gotchas": [],
        "sources": [
            {"url": "https://example.com/spec", "used_for": "spec info"}
        ]
    });
    assert!(check_report(&valid).is_ok());

    // Sources not an array fails
    let not_array = serde_json::json!({
        "status": "done",
        "summary": "researched web",
        "files": [],
        "tests": [],
        "gotchas": [],
        "sources": "https://example.com"
    });
    assert!(check_report(&not_array).is_err());

    // Source item without url fails
    let no_url = serde_json::json!({
        "status": "done",
        "summary": "researched web",
        "files": [],
        "tests": [],
        "gotchas": [],
        "sources": [{"used_for": "spec"}]
    });
    assert!(check_report(&no_url).is_err());
}

#[test]
fn seat_order_refuses_empty_owns_for_build_fix_plan_and_permits_read_only() {
    let _guard = crate::nirvana::with_test_home("seat-order-owns");
    let dir = std::env::temp_dir().join(format!("nirvana-p24-worker-owns-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let project = Project::at(&dir);
    project.ensure().unwrap();

    let order_file = dir.join(".nirvana/orders/test-order.md");
    std::fs::create_dir_all(order_file.parent().unwrap()).unwrap();
    // Order text without owns: line
    std::fs::write(&order_file, "# Test Order\ntest: pytest\ncheck: pass\n").unwrap();

    let make_request = |class: &str| SeatRequest {
        seat: "flash".into(),
        order_file: order_file.clone(),
        order: "test-order".into(),
        run: "r1".into(),
        class: Some(class.into()),
        size: None,
        needs: Vec::new(),
        not_family: Vec::new(),
        mode: None,
        effort: None,
        reason: None,
        worktree: false,
        auto: false,
    };

    // build.* refuses with exit 2
    let res = seat_order(&project, &make_request("build.bounded"));
    assert_eq!(res.unwrap_err().0, 2);

    let res = seat_order(&project, &make_request("build.load_bearing"));
    assert_eq!(res.unwrap_err().0, 2);

    // fix.* refuses with exit 2
    let res = seat_order(&project, &make_request("fix.debug"));
    assert_eq!(res.unwrap_err().0, 2);

    // plan refuses with exit 2
    let res = seat_order(&project, &make_request("plan"));
    assert_eq!(res.unwrap_err().0, 2);

    // Rule 7: read-only classes never refused for empty owns
    // research.web has no owns:, but requires_owns is false
    assert!(!crate::nirvana_gate::requires_owns("research.web"));
    assert!(!crate::nirvana_gate::requires_owns("chore.script"));

    let _ = std::fs::remove_dir_all(&dir);
}
