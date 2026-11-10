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
    // Plan 40 T3: `files` and `tests` are allowed, not required (`c40b_a_report_without_tests_or_files_passes`).
    assert_eq!(required, ["status", "summary", "gotchas"]);
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
    // Plan 40 M3: the stream names no pool; `rate_limited` spends the seated order's own (`seated.pool`).
    assert_eq!(limit.pool, None);
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
        send_ok: None,
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
        None,
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
        None,
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
        None,
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
        send_ok: None,
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
        send_ok: None,
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
        send_ok: None,
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

    // N4: an order with no --class whose booked class is build.* and no owns: is refused
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let mut leases = nirvana_seat::Leases::default();
    let pick = nirvana_seat::pick(
        &table,
        &nirvana_seat::live_world(&table),
        &nirvana_seat::Request::new("build.bounded"),
    )
    .unwrap();
    let now = crate::nirvana_usage::unix(std::time::SystemTime::now());
    nirvana_seat::book_pick(
        &mut leases,
        &table,
        &pick,
        Some("booked-no-class"),
        Some("r1"),
        now,
    );
    let leases_path = nirvana_seat::dir().join(nirvana_seat::LEASES_FILE);
    std::fs::write(&leases_path, serde_json::to_string(&leases).unwrap()).unwrap();

    let mut no_class_req = make_request("build.bounded");
    no_class_req.order = "booked-no-class".into();
    no_class_req.class = None;
    no_class_req.seat = pick.seat.clone();
    let res = seat_order(&project, &no_class_req);
    let (code, msg) = res.unwrap_err();
    assert_eq!(code, 2);
    assert_eq!(
        msg,
        "a build order needs an `owns:` line naming the files it may change"
    );

    let leases_after = nirvana_seat::load_leases(&leases_path).unwrap();
    // The refusal keeps the booking it did not make.
    assert!(leases_after.live("booked-no-class", now).is_some());
    assert!(leases_after.live("test-order", now).is_none());

    // Rule 7: read-only classes never refused for empty owns
    // research.web has no owns:, but requires_owns is false
    assert!(!crate::nirvana_gate::requires_owns("research.web"));
    assert!(!crate::nirvana_gate::requires_owns("chore.script"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn worker_pane_closing_decisions() {
    use crate::nirvana_run::Phase;

    let expected = "seat1\u{b7}order1";

    // Normal done worker: closes its pane when tab has other panes (>1) and title matches
    assert!(worker_should_close_pane(
        "done",
        Phase::Delivered,
        Some("pane-1"),
        Some(expected),
        expected,
        2
    ));

    // B1: Title mismatch refuses pane close
    assert!(!worker_should_close_pane(
        "done",
        Phase::Delivered,
        Some("pane-1"),
        Some("other·title"),
        expected,
        2
    ));
    assert!(!worker_should_close_pane(
        "done",
        Phase::Delivered,
        Some("pane-1"),
        None,
        expected,
        2
    ));

    // Kept panes stay: when not Delivered (or keeping pane for review)
    assert!(!worker_should_close_pane(
        "done",
        Phase::Running,
        Some("pane-1"),
        Some(expected),
        expected,
        2
    ));
    assert!(!worker_should_close_pane(
        "failed",
        Phase::Delivered,
        Some("pane-1"),
        Some(expected),
        expected,
        2
    ));

    // S3: Tab's last pane stays: when tab_panes_count <= 1
    // (even if there are other foreign panes in the tab, tab_panes_count from this run only is 1)
    let run_panes = vec!["pane-1".to_string()];
    let tab_panes = ["pane-1", "foreign-pane"];
    let count = count_run_tab_panes(&tab_panes, &run_panes);
    assert_eq!(count, 1);
    assert!(!worker_should_close_pane(
        "done",
        Phase::Delivered,
        Some("pane-1"),
        Some(expected),
        expected,
        count
    ));
    assert!(!worker_should_close_pane(
        "done",
        Phase::Delivered,
        Some("pane-1"),
        Some(expected),
        expected,
        1
    ));
    assert!(!worker_should_close_pane(
        "done",
        Phase::Delivered,
        Some("pane-1"),
        Some(expected),
        expected,
        0
    ));

    // No pane id
    assert!(!worker_should_close_pane(
        "done",
        Phase::Delivered,
        None,
        Some(expected),
        expected,
        2
    ));
}

#[test]
fn worker_expected_pane_title_format() {
    assert_eq!(
        expected_pane_title("seat-a", "p26-01"),
        "seat-a\u{b7}p26-01"
    );
}

#[test]
fn detached_worker_records_no_pane_and_pane_arg_records_pane() {
    let dir = temp_dir("pane_record");
    std::fs::write(dir.join("recorded.jsonl"), LIVE_HAIKU).expect("fixture");
    let (journal, stream_path, stop, argv) = stand_in(&dir, "type recorded.jsonl");

    // Case 1: detached worker (pane: None) records None in Kind::Started
    let _ = run_attempt(
        &journal,
        &stream_path,
        &in_dir(&dir, "claude"),
        1,
        &argv,
        &stop,
        None,
        &mut Vec::new(),
    )
    .expect("attempt");
    let events = journal.read().0;
    assert!(matches!(&events[0].kind, Kind::Started { pane: None, .. }));

    // Case 2: worker with --pane (pane: Some("pane-42")) records Some("pane-42") in Kind::Started
    let dir2 = temp_dir("pane_record2");
    std::fs::write(dir2.join("recorded.jsonl"), LIVE_HAIKU).expect("fixture");
    let (journal2, stream_path2, stop2, argv2) = stand_in(&dir2, "type recorded.jsonl");
    let _ = run_attempt(
        &journal2,
        &stream_path2,
        &in_dir(&dir2, "claude"),
        1,
        &argv2,
        &stop2,
        Some("pane-42"),
        &mut Vec::new(),
    )
    .expect("attempt");
    let events2 = journal2.read().0;
    assert!(matches!(&events2[0].kind, Kind::Started { pane: Some(p), .. } if p == "pane-42"));

    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(dir2);
}

#[test]
fn exec_closes_pane_after_delivered() {
    let dir = temp_dir("exec_close_delivered");
    let journal = Journal::new(dir.join("journal.jsonl"));
    let mut s = seated("seat-1");
    s.order = "order-1".into();
    journal.append(now(), Kind::Seated(Box::new(s))).unwrap();
    journal
        .append(
            now(),
            Kind::Started {
                order: "order-1".into(),
                attempt: 1,
                pane: Some("pane-1".into()),
                pid: Some(100),
                tab: None,
                resume: None,
                argv: vec!["claude".into()],
            },
        )
        .unwrap();

    let expected = "seat-1\u{b7}order-1";

    // 1. Before Delivered is written: phase is Running, so exec refuses to close
    let state_running = replay(&journal.read().0);
    assert!(!exec_should_close_pane_after_delivered(
        &state_running,
        "order-1",
        "done",
        Some("pane-1"),
        Some(expected),
        expected,
        2
    ));

    // 2. After Delivered is written: phase is Delivered, exec closes
    journal
        .append(
            now(),
            Kind::Delivered {
                order: "order-1".into(),
                report: serde_json::json!({"status": "done"}),
            },
        )
        .unwrap();
    let state_delivered = replay(&journal.read().0);
    assert!(exec_should_close_pane_after_delivered(
        &state_delivered,
        "order-1",
        "done",
        Some("pane-1"),
        Some(expected),
        expected,
        2
    ));

    let _ = std::fs::remove_dir_all(dir);
}

/// Plan 26 review R6: an order the journal does not hold never closes a pane, whatever else agrees.
#[test]
fn exec_close_refuses_an_order_the_journal_does_not_hold() {
    let dir = temp_dir("exec_close_unknown");
    let journal = Journal::new(dir.join("journal.jsonl"));
    let state = replay(&journal.read().0);
    let expected = "seat-1\u{b7}order-x";
    assert!(!exec_should_close_pane_after_delivered(
        &state,
        "order-x",
        "done",
        Some("pane-1"),
        Some(expected),
        expected,
        2
    ));
}

// ---- Plan 29 review round: a server order through its public paths (book, exec, the reseat) ----

const P29_MCP: &str =
    r#"{"mcpServers": {"xtool": {"command": "uv", "env": {"XTOOL_TOKEN": "fake-not-real"}}}}"#;

/// A project under a test home with the default seats and `.mcp.json` defining xtool, over a stand-in world where
/// every CLI is signed in and no window is used (no `claude auth status`, no `claude -p /usage`).
fn p29_project(name: &str) -> (crate::nirvana::TestHomeGuard, Project) {
    let home = crate::nirvana::with_test_home(name);
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    let project = Project::at(home.path.join("project"));
    project.ensure().unwrap();
    std::fs::write(project.root.join(".mcp.json"), P29_MCP).unwrap();
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    (home, project)
}

/// An order seated on a claude seat for the xtool server, working in the project's root.
fn p29_seated(project: &Project, order: &str, seat: &str) -> Seated {
    let mut seated = seated(order);
    seated.seat = seat.into();
    seated.cli = "claude".into();
    seated.model = format!("claude-{seat}-5-5");
    seated.effort = Effort::Medium;
    seated.pool = "max".into();
    seated.needs = vec!["xtool".into()];
    seated.not_family = Vec::new();
    seated.cwd = project.root.display().to_string();
    seated.order_file = project.root.join("order.md").display().to_string();
    seated
}

#[test]
fn p29_seat_order_seats_a_server_order_on_claude_and_its_brief_never_starts_a_device() {
    use crate::nirvana_gui_tools::{BRIEF_LINE, DEVICE_LINE};
    let (home, project) = p29_project("p29-seat-order");
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let world = nirvana_seat::live_world(&table);
    // Without the server the class seats on agy: the rule has something to move.
    let plain = nirvana_seat::pick(
        &table,
        &world,
        &nirvana_seat::Request::new("sweep.mechanical"),
    )
    .unwrap();
    assert_eq!(plain.cli, "agy", "{}", plain.seat);
    let mut only = nirvana_seat::Request::new("sweep.mechanical");
    only.claude_only = Some("a server".into());
    let claude_seat = nirvana_seat::pick(&table, &world, &only).unwrap().seat;
    let order_file = project.root.join("order.md");
    std::fs::write(&order_file, "# Slice the jig\n").unwrap();
    let request = |order: &str, seat: &str| SeatRequest {
        seat: seat.into(),
        order_file: order_file.clone(),
        order: order.into(),
        run: "r1".into(),
        class: Some("sweep.mechanical".into()),
        size: None,
        needs: vec!["xtool".into()],
        not_family: Vec::new(),
        mode: None,
        effort: None,
        reason: None,
        worktree: false,
        auto: false,
        send_ok: None,
    };
    // Each order is held live, so `seat_order` starts no worker whatever it books.
    let _live: Vec<_> = ["o0", "o1", "o2"]
        .iter()
        .map(|order| take_live(&project, "r1", order).unwrap().unwrap())
        .collect();
    // `seat run` on the agy seat the rule would give it without the server: the rule seats it elsewhere.
    let (code, why) = seat_order(&project, &request("o0", &plain.seat)).unwrap_err();
    assert_eq!(code, 3, "{why}");
    seat_order(&project, &request("o1", &claude_seat)).unwrap();
    let state = replay(&project.journal("r1").read().0);
    let seated = &state.order("o1").unwrap().seated;
    assert_eq!(seated.cli, "claude");
    let brief = std::fs::read_to_string(&seated.order_file).unwrap();
    assert!(
        brief.contains(&format!("\n\n{DEVICE_LINE}\n{BRIEF_LINE}")),
        "{brief}"
    );
    // A need that is a seat capability is no server, even named like one: no line.
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    let haiku_caps = "capabilities = [\"code\", \"tools\", \"image\"]";
    assert_eq!(nirvana_seat::DEFAULT.matches(haiku_caps).count(), 1);
    std::fs::write(
        &seats,
        nirvana_seat::DEFAULT.replace(
            haiku_caps,
            "capabilities = [\"code\", \"tools\", \"image\", \"xtool\"]",
        ),
    )
    .unwrap();
    seat_order(&project, &request("o2", "haiku")).unwrap();
    let state = replay(&project.journal("r1").read().0);
    let brief = std::fs::read_to_string(&state.order("o2").unwrap().seated.order_file).unwrap();
    assert!(!brief.contains(DEVICE_LINE), "{brief}");
    nirvana_seat::set_test_world(None);
}

#[test]
fn p29_a_rate_limited_server_order_is_reseated_on_claude_or_not_at_all() {
    let (_home, project) = p29_project("p29-reseat");
    let run = "r1";
    let journal = project.journal(run);
    let seated = p29_seated(&project, "o1", "haiku");
    journal.append(1, opened()).unwrap();
    journal
        .append(2, Kind::Seated(Box::new(seated.clone())))
        .unwrap();
    let child = child_id(&replay(&journal.read().0), "o1");
    // Held live, so a reseat starts no worker.
    let _live = take_live(&project, run, &child).unwrap().unwrap();
    let limit = RateLimit {
        signal: "rate_limit_event".into(),
        pool: Some("max".into()),
        window: None,
        resets_at: None,
        detail: String::new(),
    };
    // With every claude seat's pool spent the rule reseats the class on agy: it has something to move.
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let spent = spent_world(&table, nirvana_seat::live_world(&table), &limit, "max");
    let mut plain = nirvana_seat::Request::new("sweep.mechanical");
    plain.size = seated.size;
    assert_eq!(
        nirvana_seat::pick(&table, &spent, &plain).unwrap().cli,
        "agy"
    );
    let mut out = Vec::new();
    let goes_on = rate_limited(
        &project,
        &journal,
        run,
        &seated,
        &limit,
        &Stream::new("claude"),
        &mut out,
    )
    .unwrap();
    let view = String::from_utf8_lossy(&out).into_owned();
    assert!(!goes_on, "{view}");
    let state = replay(&journal.read().0);
    assert!(state.order(&child).is_none(), "re-seated: {view}");
    assert!(
        journal
            .read()
            .0
            .iter()
            .any(|event| matches!(&event.kind, Kind::Asked { order: Some(o), .. } if o == "o1")),
        "{view}"
    );
    nirvana_seat::set_test_world(None);
}

/// Review F1 (exec), F2 and F3 through `herdr seat exec`, with a stand-in for the worker's program.
#[cfg(windows)]
#[test]
fn p29_exec_gives_the_worker_its_config_removes_it_after_and_never_starts_without_it() {
    let (_home, project) = p29_project("p29-exec");
    std::fs::write(project.root.join("recorded.jsonl"), LIVE_HAIKU).unwrap();
    let run = "r1";
    let journal = project.journal(run);
    journal.append(1, opened()).unwrap();
    let config =
        |order: &str| crate::nirvana_gui_tools::order_config_path(&project.run_dir(run), order);
    let exec_with = |order: &str, script: &str| -> i32 {
        journal
            .append(
                2,
                Kind::Seated(Box::new(p29_seated(&project, order, "sonnet"))),
            )
            .unwrap();
        TEST_COMMAND.with_borrow_mut(|slot| {
            *slot = Some(vec!["cmd.exe".into(), "/c".into(), script.to_owned()]);
        });
        let mut out = Vec::new();
        let code = exec(&project, run, order, None, &mut out).unwrap();
        TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
        code
    };
    let relative = |order: &str| {
        config(order)
            .strip_prefix(&project.root)
            .unwrap()
            .display()
            .to_string()
    };
    let ended = |order: &str| {
        journal
            .read()
            .0
            .into_iter()
            .find_map(|event| match event.kind {
                Kind::Ended {
                    order: Some(o),
                    outcome,
                    why,
                    exit,
                    ..
                } if o == order => Some((outcome, why, exit)),
                _ => None,
            })
    };
    let started = |order: &str| {
        journal
            .read()
            .0
            .into_iter()
            .find_map(|event| match event.kind {
                Kind::Started { order: o, argv, .. } if o == order => Some(argv),
                _ => None,
            })
    };
    // Delivered: the worker starts with --mcp-config naming its file, which is there while it runs and gone after.
    let code = exec_with(
        "o1",
        &format!(
            "if exist {} (type recorded.jsonl) else (exit /b 3)",
            relative("o1")
        ),
    );
    assert_eq!(code, 0, "{:?}", ended("o1"));
    let argv = started("o1").expect("started");
    let at = argv
        .iter()
        .position(|arg| arg == "--mcp-config")
        .expect("--mcp-config");
    assert_eq!(argv[at + 1], config("o1").display().to_string());
    assert_eq!(ended("o1").map(|e| e.0), Some("delivered".to_owned()));
    assert!(!config("o1").exists());
    // Failed: the file was there (exit 4, not 3) and is gone after.
    let code = exec_with(
        "o2",
        &format!("if exist {} (exit /b 4) else (exit /b 3)", relative("o2")),
    );
    assert_eq!(code, 1);
    let (outcome, _, exit) = ended("o2").expect("ended");
    assert_eq!((outcome.as_str(), exit), ("failed", Some(4)));
    assert!(!config("o2").exists());
    // The server gone from `.mcp.json` since the seat: the order fails naming it, and its worker never starts.
    std::fs::write(project.root.join(".mcp.json"), r#"{"mcpServers": {}}"#).unwrap();
    assert_eq!(exec_with("o3", "exit /b 0"), 1);
    assert_eq!(started("o3"), None);
    let (outcome, why, _) = ended("o3").expect("ended");
    assert_eq!(outcome, "failed");
    let why = why.unwrap_or_default();
    assert!(
        why.starts_with("o3 was seated for MCP server xtool and cannot get it: ")
            && why.ends_with("no longer defines xtool"),
        "{why}"
    );
    nirvana_seat::set_test_world(None);
}

// ---- plan 31 P31a: the Send rule, the server warning, a finished worker's pane ----

/// A transcript line of the owner's prompt, as Claude Code 2.1.311 records one.
fn p31_owner_line(text: &str, at: &str) -> String {
    serde_json::json!({
        "type": "user",
        "timestamp": at,
        "origin": {"kind": "human"},
        "message": {"role": "user", "content": text},
    })
    .to_string()
}

/// A transcript's `timestamp` for the second `at`, as Claude Code writes it.
fn p31_iso(at: u64) -> String {
    nirvana_run::utc(at).replace(' ', "T")
}

/// `p29_project` with `.mcp.json` defining bambu-studio-beta as well, and Nirvana Claude's session recorded for the
/// project with a transcript holding `prompts`, typed a minute ago.
fn p31_project(name: &str, prompts: &[&str]) -> (crate::nirvana::TestHomeGuard, Project) {
    let (home, project) = p29_project(name);
    std::fs::write(
        project.root.join(".mcp.json"),
        r#"{"mcpServers": {"xtool": {"command": "uv"}, "bambu-studio-beta": {"command": "uv"}}}"#,
    )
    .unwrap();
    let transcript = home.path.join("transcript.jsonl");
    let lines: Vec<String> = prompts
        .iter()
        .map(|text| p31_owner_line(text, &p31_iso(nirvana_run::now() - 60)))
        .collect();
    std::fs::write(&transcript, lines.join("\n") + "\n").unwrap();
    // Review N5: Nirvana Claude started in a folder other than the project's root, as `CLAUDE_PROJECT_DIR` says, so
    // the session is found only by the variable, never by the root.
    let started_in = home.path.join("claude-started-here");
    std::fs::create_dir_all(&started_in).unwrap();
    crate::nirvana_session_record::write(
        &home.path,
        &crate::nirvana_session_record::absolute_folder(&started_in),
        &crate::nirvana_session_record::Record {
            session_id: "s1".into(),
            at: "2026-10-29T01:32:15Z".into(),
            transcript: transcript.display().to_string(),
        },
    )
    .unwrap();
    // The folder `seat run` looks the session up by: the variable Claude Code gives its tools.
    std::env::set_var(crate::nirvana_session_record::PROJECT_DIR_ENV, &started_in);
    (home, project)
}

fn p31_request(project: &Project, order: &str, seat: &str, needs: &[&str]) -> SeatRequest {
    SeatRequest {
        seat: seat.into(),
        order_file: project.root.join("order.md"),
        order: order.into(),
        run: "r1".into(),
        class: Some("sweep.mechanical".into()),
        size: None,
        needs: needs.iter().map(|need| (*need).to_owned()).collect(),
        not_family: Vec::new(),
        mode: None,
        effort: None,
        reason: None,
        worktree: false,
        auto: false,
        send_ok: None,
    }
}

/// The claude seat the rule gives `sweep.mechanical` once only a claude seat may take it.
fn p31_claude_seat(project: &Project) -> String {
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let mut only = nirvana_seat::Request::new("sweep.mechanical");
    only.claude_only = Some("a server".into());
    nirvana_seat::pick(&table, &nirvana_seat::live_world(&table), &only)
        .unwrap()
        .seat
}

#[test]
fn p31_seat_run_records_a_verified_send_quote_and_refuses_one_it_cannot_find() {
    use crate::nirvana_gui_tools::{DESKTOP_SEND_LINE, DEVICE_LINE, SEND_LINE};
    let (_home, project) =
        p31_project("p31-send-ok", &["ok. Print the nosepiece when it is ready"]);
    std::fs::write(
        project.root.join("order.md"),
        "# Slice and send the nosepiece\n",
    )
    .unwrap();
    let seat = p31_claude_seat(&project);
    let _live: Vec<_> = ["s0", "s1", "s2"]
        .iter()
        .map(|order| take_live(&project, "r1", order).unwrap().unwrap())
        .collect();
    // A quote the owner never typed refuses the order before anything is booked or journaled.
    let mut request = p31_request(&project, "s0", &seat, &["bambu-studio-beta"]);
    request.send_ok = Some("Print the ear hook now please".into());
    let (code, why) = seat_order(&project, &request).unwrap_err();
    assert_eq!(code, 2, "{why}");
    assert!(why.contains("none of the owner's own prompts"), "{why}");
    assert!(replay(&project.journal("r1").read().0)
        .order("s0")
        .is_none());
    // Review N3: the owner's words on an order granted no device server refuse it too: its worker would never hear.
    let mut request = p31_request(&project, "s2", &seat, &[]);
    request.send_ok = Some("Print the nosepiece when it is ready".into());
    let (code, why) = seat_order(&project, &request).unwrap_err();
    assert_eq!(code, 2, "{why}");
    assert!(why.contains("granted a device server"), "{why}");
    assert!(replay(&project.journal("r1").read().0)
        .order("s2")
        .is_none());
    // The owner's words: the seated event holds the quote and the check, the brief the Send line, no device line.
    let mut request = p31_request(&project, "s1", &seat, &["bambu-studio-beta"]);
    request.send_ok = Some("Print the nosepiece when it is ready".into());
    let outcome = seat_order(&project, &request).unwrap();
    // Review B1 item 4: `seat run` gives back the prompt, its time and the session file.
    assert_eq!(
        outcome["send_ok"]["prompt"], "ok. Print the nosepiece when it is ready",
        "{outcome}"
    );
    assert!(
        outcome["send_ok"]["transcript"]
            .as_str()
            .is_some_and(|file| file.ends_with("transcript.jsonl")),
        "{outcome}"
    );
    let seated = replay(&project.journal("r1").read().0)
        .order("s1")
        .unwrap()
        .seated
        .clone();
    let send_ok = seated.send_ok.clone().expect("send_ok on seated");
    assert_eq!(send_ok.quote, "Print the nosepiece when it is ready");
    assert_eq!(send_ok.prompt, "ok. Print the nosepiece when it is ready");
    assert!(
        send_ok.check.contains(&send_ok.at) && send_ok.at.starts_with("20"),
        "{send_ok:?}"
    );
    let line = std::fs::read_to_string(project.journal("r1").path)
        .unwrap()
        .lines()
        .find(|line| line.contains("\"seated\"") && line.contains("\"s1\""))
        .unwrap()
        .to_owned();
    assert!(
        line.contains("\"send_ok\":{\"quote\":\"Print the nosepiece when it is ready\",\"prompt\":\"ok. Print the nosepiece when it is ready\",\"at\":"),
        "{line}"
    );
    let brief = std::fs::read_to_string(&seated.order_file).unwrap();
    assert!(
        brief.contains(&format!("\n\n{SEND_LINE}\n{DESKTOP_SEND_LINE}")),
        "{brief}"
    );
    assert!(!brief.contains(DEVICE_LINE), "{brief}");
    std::env::remove_var(crate::nirvana_session_record::PROJECT_DIR_ENV);
    nirvana_seat::set_test_world(None);
}

#[test]
fn p31_seat_run_warns_when_an_order_names_studio_and_needs_no_server() {
    let (_home, project) = p31_project("p31-hint", &[]);
    std::fs::write(
        project.root.join("order.md"),
        "# Slice the nosepiece in Bambu Studio\n",
    )
    .unwrap();
    let seat = p31_claude_seat(&project);
    let _live: Vec<_> = ["h0", "h1"]
        .iter()
        .map(|order| take_live(&project, "r1", order).unwrap().unwrap())
        .collect();
    let mut request = p31_request(&project, "h0", &seat, &[]);
    request.reason = Some("the window".into());
    let outcome = seat_order(&project, &request).unwrap();
    let warning = outcome["warning"].as_str().unwrap_or_default();
    assert!(
        warning.contains("Bambu Studio") && warning.contains("--needs bambu-studio-beta"),
        "{outcome}"
    );
    // The order seats all the same: a warning, never a refusal.
    assert!(replay(&project.journal("r1").read().0)
        .order("h0")
        .is_some());
    let outcome = seat_order(
        &project,
        &p31_request(&project, "h1", &seat, &["bambu-studio-beta"]),
    )
    .unwrap();
    assert!(outcome["warning"].is_null(), "{outcome}");
    std::env::remove_var(crate::nirvana_session_record::PROJECT_DIR_ENV);
    nirvana_seat::set_test_world(None);
}

fn p31_event(seq: u64, at: u64, kind: Kind) -> Event {
    Event {
        v: nirvana_run::V,
        seq,
        at,
        kind,
    }
}

fn p31_started(order: &str, pane: &str) -> Kind {
    Kind::Started {
        order: order.into(),
        attempt: 1,
        pid: None,
        pane: Some(pane.into()),
        tab: None,
        resume: None,
        argv: Vec::new(),
    }
}

fn p31_ended(order: &str) -> Kind {
    Kind::Ended {
        order: Some(order.into()),
        outcome: "delivered".into(),
        why: None,
        exit: Some(0),
        pools: Vec::new(),
        pools_basis: None,
    }
}

#[test]
fn p31_a_finished_workers_pane_names_its_order_and_a_live_one_does_not() {
    // dune-v5, 2026-10-29: dune-slice-v5 ended in wP:p1D at 01:30:31Z; the answer was typed there at 01:30:36Z.
    let v5 = (
        "dune-v5".to_owned(),
        vec![
            p31_event(31, 1_793_237_363, p31_started("dune-slice-v5", "wP:p1D")),
            p31_event(65, 1_793_237_831, p31_ended("dune-slice-v5")),
        ],
    );
    let never = |_: &str, _: &str| false;
    let why = finished_worker_in(std::slice::from_ref(&v5), "wP:p1D", never).expect("finished");
    assert!(
        why.contains("dune-slice-v5")
            && why.contains("dune-v5")
            && why.contains("nothing was typed"),
        "{why}"
    );
    assert_eq!(
        finished_worker_in(std::slice::from_ref(&v5), "wP:p1E", never),
        None
    );
    // A worker still running in the pane: no refusal.
    let live = (
        "r2".to_owned(),
        vec![p31_event(1, 10, p31_started("o1", "wP:p9"))],
    );
    assert_eq!(finished_worker_in(&[live], "wP:p9", never), None);
    // A later run's worker reusing the pane owns it, even after the first worker ended.
    let reuse = (
        "r3".to_owned(),
        vec![p31_event(1, 1_793_237_900, p31_started("o2", "wP:p1D"))],
    );
    assert_eq!(
        finished_worker_in(&[v5.clone(), reuse], "wP:p1D", never),
        None
    );
    // A restarted attempt after an end is live again.
    let restarted = (
        "r4".to_owned(),
        vec![
            p31_event(1, 10, p31_started("o3", "wP:pA")),
            p31_event(2, 20, p31_ended("o3")),
            p31_event(3, 30, p31_started("o3", "wP:pA")),
        ],
    );
    assert_eq!(finished_worker_in(&[restarted], "wP:pA", never), None);
    // Review S4: an ended order whose supervisor still holds its lock is not finished.
    assert_eq!(
        finished_worker_in(std::slice::from_ref(&v5), "wP:p1D", |run, order| {
            run == "dune-v5" && order == "dune-slice-v5"
        }),
        None
    );
}

/// Plan 31 review S3 and S4: the pane's project is found from the Nirvana home, as the server finds it, and a pane a
/// live supervisor names is never finished, though its last `started` is a finished worker's.
#[test]
fn p31_a_pane_a_live_supervisor_names_is_never_finished() {
    let (home, project) = p29_project("p31-pane-live");
    let journal = project.journal("r1");
    journal.append(1, opened()).unwrap();
    journal.append(2, p31_started("slice-1", "wP:p1D")).unwrap();
    journal.append(3, p31_ended("slice-1")).unwrap();
    // No supervisor ever ran in the pane: no project names it, so nothing is refused.
    assert_eq!(finished_worker_at(&home.path, "wP:p1D"), None);
    let first = take_live(&project, "r1", "slice-1").unwrap().unwrap();
    note_worker_pane(&project, "r1", "slice-1", "wP:p1D");
    drop(first);
    let why = finished_worker_at(&home.path, "wP:p1D").expect("finished");
    assert!(why.contains("slice-1") && why.contains("--force"), "{why}");
    // herdr reuses the pane for the next order: its supervisor holds its lock before its `started`.
    let next = take_live(&project, "r1", "print-1").unwrap().unwrap();
    note_worker_pane(&project, "r1", "print-1", "wP:p1D");
    assert_eq!(finished_worker_at(&home.path, "wP:p1D"), None);
    drop(next);
    assert!(finished_worker_at(&home.path, "wP:p1D").is_some());
    nirvana_seat::set_test_world(None);
}

#[test]
fn p31_a_reseats_continuation_brief_keeps_the_device_line_and_drops_the_send_line() {
    use crate::nirvana_gui_tools::{
        with_brief_lines, SendOk, BRIEF_LINE, DEVICE_LINE, SEND_LINE, SENT_BEFORE_LINE,
    };
    let (_home, project) = p29_project("p31-continuation");
    let run = "r1";
    let state = nirvana_run::RunState::default();
    for (order, send_ok) in [("c1", false), ("c2", true)] {
        let mut seated = p29_seated(&project, order, "haiku");
        seated.send_ok = send_ok.then(|| SendOk {
            quote: "Print the nosepiece when it is ready".into(),
            prompt: "Print the nosepiece when it is ready".into(),
            at: "2026-10-29T01:32:14.584Z".into(),
            transcript: "t.jsonl".into(),
            check: "found".into(),
        });
        let text = with_brief_lines("# Slice it\n".into(), &["xtool".into()], send_ok);
        seated.order_file = write_order(&project, &project.root, order, &text).unwrap();
        let file = continuation_brief(&project, run, &seated, &state, None, &Stream::new("claude"))
            .unwrap();
        let brief = std::fs::read_to_string(&file).unwrap();
        // Review S1: the continuation never inherits the Send; it keeps the device line and hears the parent may
        // have sent.
        assert!(
            brief.contains(&format!("{DEVICE_LINE}\n{BRIEF_LINE}")),
            "{order}: {brief}"
        );
        assert!(!brief.contains(SEND_LINE), "{order}: {brief}");
        assert_eq!(
            brief.contains(SENT_BEFORE_LINE),
            send_ok,
            "{order}: {brief}"
        );
    }
    nirvana_seat::set_test_world(None);
}

#[test]
fn p31_a_rate_limited_reseat_never_inherits_the_send_approval() {
    use crate::nirvana_gui_tools::{
        with_brief_lines, SendOk, DEVICE_LINE, SEND_LINE, SENT_BEFORE_LINE,
    };
    let (_home, project) = p29_project("p31-reseat-send-ok");
    let run = "r1";
    let journal = project.journal(run);
    let mut seated = p29_seated(&project, "o1", "haiku");
    // No server need, so a spent max pool re-seats it on agy rather than asking.
    seated.needs = Vec::new();
    seated.send_ok = Some(SendOk {
        quote: "Print the nosepiece when it is ready".into(),
        prompt: "Print the nosepiece when it is ready".into(),
        at: "2026-10-29T01:32:14.584Z".into(),
        transcript: "t.jsonl".into(),
        check: "found".into(),
    });
    // The order text as an approved server order had it, though this one now sits on no server.
    let text = with_brief_lines("# Send the slice\n".into(), &["bambu".into()], true);
    seated.order_file = write_order(&project, &project.root, "o1", &text).unwrap();
    journal.append(1, opened()).unwrap();
    journal
        .append(2, Kind::Seated(Box::new(seated.clone())))
        .unwrap();
    let child = child_id(&replay(&journal.read().0), "o1");
    let _live = take_live(&project, run, &child).unwrap().unwrap();
    let limit = RateLimit {
        signal: "rate_limit_event".into(),
        pool: Some("max".into()),
        window: None,
        resets_at: None,
        detail: String::new(),
    };
    let mut out = Vec::new();
    rate_limited(
        &project,
        &journal,
        run,
        &seated,
        &limit,
        &Stream::new("claude"),
        &mut out,
    )
    .unwrap();
    let view = String::from_utf8_lossy(&out).into_owned();
    let state = replay(&journal.read().0);
    let reseated = &state.order(&child).expect(&view).seated;
    // Review S1: a new Send needs a new quote; the reseat keeps the device line and hears the parent may have sent.
    assert_eq!(reseated.send_ok, None, "{view}");
    let brief = std::fs::read_to_string(&reseated.order_file).unwrap();
    assert!(!brief.contains(SEND_LINE), "{brief}");
    assert!(
        brief.contains(DEVICE_LINE) && brief.contains(SENT_BEFORE_LINE),
        "{brief}"
    );
    nirvana_seat::set_test_world(None);
}

// ---- plan 35 P35b: what an agy worker starts with ----

/// §3.6: a `finished_reason` string is a turn that did not finish, whatever the result holds; null keeps today's path.
#[test]
fn p35b_an_agy_result_with_a_finished_reason_fails_naming_it() {
    let (stream, _) = feed("agy", LIVE_FLASH);
    assert!(stream.report().is_ok());
    for (reason, ok) in [
        (Value::Null, true),
        (Value::from("print_timeout"), false),
        (Value::from("interrupted"), false),
        (Value::from("error"), false),
    ] {
        let mut cut = stream.clone();
        cut.result.as_mut().unwrap()["finished_reason"] = reason.clone();
        match cut.report() {
            Ok(_) => assert!(ok, "{reason}"),
            Err(why) => {
                assert!(!ok, "{why}");
                assert_eq!(
                    why,
                    format!("agy finished with {reason}, status \"SUCCESS\"")
                );
            }
        }
        assert_eq!(cut.finished_note(), None, "{reason}");
    }
    // Review round S2: another string with SUCCESS is read as finished, with one note; with any other status it fails.
    let mut later = stream.clone();
    later.result.as_mut().unwrap()["finished_reason"] = Value::from("stop");
    assert!(later.report().is_ok());
    assert_eq!(
        later.finished_note().unwrap(),
        "agy finished with \"stop\" and status SUCCESS; herdr knows only print_timeout, interrupted, error as \
         unfinished, so it read the turn as finished (plan 35 §3.6)"
    );
    later.result.as_mut().unwrap()["status"] = Value::from("FAILED");
    assert_eq!(
        later.report().unwrap_err(),
        "agy finished with \"stop\", status \"FAILED\""
    );
    assert_eq!(later.finished_note(), None);
}

/// A project under a test home with the default seats, `.mcp.json` defining a non-desktop `bambu`, and a CLAUDE.md
/// when `rules`; an agy order in its root needing `needs`.
fn p35b_project(
    name: &str,
    rules: bool,
    needs: &[&str],
) -> (crate::nirvana::TestHomeGuard, Project, Seated) {
    let (home, project) = p29_project(name);
    std::fs::write(
        project.root.join(".mcp.json"),
        r#"{"mcpServers": {"bambu": {"type": "stdio", "command": "node", "env": {"BAMBU_TOKEN": "fake-not-real"}}}}"#,
    )
    .unwrap();
    if rules {
        std::fs::write(project.root.join("CLAUDE.md"), "# Rules\nNever push.\n").unwrap();
    }
    std::fs::write(project.root.join("recorded.jsonl"), LIVE_FLASH).unwrap();
    let mut seated = seated("o1");
    seated.needs = needs.iter().map(|need| (*need).to_owned()).collect();
    seated.cwd = project.root.display().to_string();
    seated.order_file = project.root.join("order.md").display().to_string();
    std::fs::write(&seated.order_file, "# Read the printer\n").unwrap();
    (home, project, seated)
}

/// §5 P35b items 1, 2 and 7 through `herdr seat exec`, with a stand-in for agy that checks its tools folder.
#[cfg(windows)]
#[test]
fn p35b_exec_gives_an_agy_worker_its_tools_folder_and_removes_it_after_a_failed_one_included() {
    let (_home, project, base) = p35b_project("p35b-exec", true, &["bambu"]);
    let run = "r1";
    let run_dir = project.run_dir(run);
    let journal = project.journal(run);
    journal.append(1, opened()).unwrap();
    let exec_with = |order: &str, script: &str| -> i32 {
        let mut seated = base.clone();
        seated.order = order.into();
        journal.append(2, Kind::Seated(Box::new(seated))).unwrap();
        TEST_COMMAND.with_borrow_mut(|slot| {
            *slot = Some(vec!["cmd.exe".into(), "/c".into(), script.to_owned()]);
        });
        let mut out = Vec::new();
        let code = exec(&project, run, order, None, &mut out).unwrap();
        TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
        code
    };
    let tools = |order: &str| crate::nirvana_agy_tools::tools_dir(&run_dir, order);
    let relative = |order: &str| {
        tools(order)
            .strip_prefix(&project.root)
            .unwrap()
            .display()
            .to_string()
    };
    let started = |order: &str| {
        journal
            .read()
            .0
            .into_iter()
            .find_map(|event| match event.kind {
                Kind::Started { order: o, argv, .. } if o == order => Some(argv),
                _ => None,
            })
    };
    let check = |order: &str| {
        let at = relative(order);
        format!(r"if exist {at}\.agents\mcp_config.json if exist {at}\AGENTS.md")
    };
    let code = exec_with(
        "o1",
        &format!("{} (type recorded.jsonl) else (exit /b 3)", check("o1")),
    );
    assert_eq!(code, 0);
    let argv = started("o1").expect("started");
    assert_eq!(
        after(&argv, "--add-dir"),
        Some(tools("o1").display().to_string().as_str())
    );
    let settings = crate::nirvana_agy_tools::settings_path(&run_dir, "o1");
    assert_eq!(
        after(&argv, "--settings"),
        Some(settings.display().to_string().as_str())
    );
    let print = argv.last().unwrap();
    assert!(
        print.starts_with("--print=Do the work order in "),
        "{print}"
    );
    assert!(
        print.contains("holds herdr's tool config: never write there."),
        "{print}"
    );
    assert!(!tools("o1").exists() && !settings.exists());
    // Failed (exit 4: the folder was there): gone after too.
    let code = exec_with(
        "o2",
        &format!("{} (exit /b 4) else (exit /b 3)", check("o2")),
    );
    assert_eq!(code, 1);
    let ended = journal
        .read()
        .0
        .into_iter()
        .find_map(|event| match event.kind {
            Kind::Ended {
                order: Some(o),
                exit,
                ..
            } if o == "o2" => Some(exit),
            _ => None,
        });
    assert_eq!(ended, Some(Some(4)));
    assert!(!tools("o2").exists());
    assert!(!crate::nirvana_agy_tools::settings_path(&run_dir, "o2").exists());
    // A folder a crash left behind is not handed to the next start.
    std::fs::create_dir_all(tools("o3")).unwrap();
    std::fs::write(tools("o3").join("stale.txt"), "x").unwrap();
    let code = exec_with(
        "o3",
        &format!(
            r"if exist {}\stale.txt (exit /b 5) else (type recorded.jsonl)",
            relative("o3")
        ),
    );
    assert_eq!(code, 0);
    // A step after the servers were written fails (a profile copy gone): the order never starts, and its servers go.
    let ghost = project.root.join("ghost.md");
    std::fs::write(
        &ghost,
        format!(
            "Profile: ghost\nProfile sha256: {}\n\n# Read it\n",
            "0".repeat(64)
        ),
    )
    .unwrap();
    let mut seated = base.clone();
    seated.order = "o4".into();
    seated.order_file = ghost.display().to_string();
    journal.append(3, Kind::Seated(Box::new(seated))).unwrap();
    let mut out = Vec::new();
    assert_eq!(exec(&project, run, "o4", None, &mut out).unwrap(), 1);
    let view = String::from_utf8_lossy(&out).into_owned();
    assert!(
        view.contains("MCP servers for this order: bambu")
            && view.contains("o4 was seated with profile ghost"),
        "{view}"
    );
    assert_eq!(started("o4"), None);
    assert!(!tools("o4").exists());
    nirvana_seat::set_test_world(None);
}

/// §5 P35b item 7: an agy order with no server, no rules, no skills and no denials starts with today's argv.
#[cfg(windows)]
#[test]
fn p35b_exec_an_agy_order_with_nothing_to_add_starts_with_todays_argv() {
    let (_home, project, seated) = p35b_project("p35b-exec-plain", false, &[]);
    let run = "r1";
    let journal = project.journal(run);
    journal.append(1, opened()).unwrap();
    journal
        .append(2, Kind::Seated(Box::new(seated.clone())))
        .unwrap();
    TEST_COMMAND.with_borrow_mut(|slot| {
        *slot = Some(vec![
            "cmd.exe".into(),
            "/c".into(),
            "type recorded.jsonl".into(),
        ]);
    });
    let mut out = Vec::new();
    let code = exec(&project, run, "o1", None, &mut out).unwrap();
    TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&out));
    let started = journal
        .read()
        .0
        .into_iter()
        .find_map(|event| match event.kind {
            Kind::Started { argv, .. } => Some(argv),
            _ => None,
        });
    let today = super::argv(&LaunchSpec {
        cli: seated.cli.clone(),
        model: seated.model.clone(),
        effort: seated.effort,
        prompt: first_line(&seated.order_file),
        resume: None,
        print_timeout_s: print_timeout_s(&seated),
    })
    .unwrap();
    assert_eq!(started, Some(today));
    assert!(!project
        .run_dir(run)
        .join(crate::nirvana_agy_tools::DIR)
        .exists());
    nirvana_seat::set_test_world(None);
}

/// §3.4: a lesson review on agy is denied running and writing through its settings file.
#[test]
fn p35b_an_agy_lesson_review_is_denied_commands_and_writes() {
    let mut review = seated("rv1");
    review.class = crate::nirvana_review::CLASS.into();
    let dir = temp_dir("p35b_review_denials");
    assert_eq!(
        agy_denials(&dir, &review).unwrap(),
        ["command(*)", "write_file(*)"]
    );
    let mut plain = seated("o1");
    plain.order_file = dir.join("none.md").display().to_string();
    assert_eq!(agy_denials(&dir, &plain).unwrap(), Vec::<String>::new());
    let _ = std::fs::remove_dir_all(dir);
}

/// §5 P35b item 3: an agy research order's brief carries the line on reading URLs; a claude one does not.
#[test]
fn p35b_seat_run_gives_an_agy_research_order_the_sources_line() {
    use crate::nirvana_agy_tools::SOURCES_LINE;
    let (_home, project) = p29_project("p35b-sources-line");
    let order_file = project.root.join("order.md");
    std::fs::write(&order_file, "# Find the dial specs\n").unwrap();
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let world = nirvana_seat::live_world(&table);
    let mut request = nirvana_seat::Request::new("research.web");
    let agy_seat = nirvana_seat::pick(&table, &world, &request).unwrap();
    assert_eq!(agy_seat.cli, "agy", "{}", agy_seat.seat);
    request.claude_only = Some("a test".into());
    let claude_seat = nirvana_seat::pick(&table, &world, &request).unwrap().seat;
    let seat = |order: &str, seat: &str| SeatRequest {
        seat: seat.into(),
        order_file: order_file.clone(),
        order: order.into(),
        run: "r1".into(),
        class: Some("research.web".into()),
        size: None,
        needs: Vec::new(),
        not_family: Vec::new(),
        mode: None,
        effort: None,
        reason: (seat != agy_seat.seat).then(|| "the sources line on a claude seat".to_owned()),
        worktree: false,
        auto: false,
        send_ok: None,
    };
    let _live: Vec<_> = ["a1", "c1"]
        .iter()
        .map(|order| take_live(&project, "r1", order).unwrap().unwrap())
        .collect();
    seat_order(&project, &seat("a1", &agy_seat.seat)).unwrap();
    seat_order(&project, &seat("c1", &claude_seat)).unwrap();
    let state = replay(&project.journal("r1").read().0);
    let brief = |order: &str| {
        std::fs::read_to_string(&state.order(order).unwrap().seated.order_file).unwrap()
    };
    assert!(
        brief("a1").contains(&format!("for each source consulted. {SOURCES_LINE}")),
        "{}",
        brief("a1")
    );
    assert!(!brief("c1").contains(SOURCES_LINE), "{}", brief("c1"));
    nirvana_seat::set_test_world(None);
}

// ---- plan 35 P35c ----

/// Plan 35 P35c items 2 and 4 through `herdr seat exec`: a worker granted bladebro has the order's `BLADE_HOME` in its
/// shell, on agy and on claude, and the home is gone when it ends; a worker with no grant has none, nor one whose
/// brief, written at seating, does not name bladebro (review round S3).
#[cfg(windows)]
#[test]
fn p35c_exec_lays_the_orders_blade_env_over_the_workers_shell_and_removes_the_home() {
    crate::nirvana_bladebro::TEST_INSTALLED.set(Some(true));
    let (_home, project, base) = p35b_project("p35c-exec-env", false, &[]);
    let run = "r1";
    let run_dir = project.run_dir(run);
    let journal = project.journal(run);
    journal.append(1, opened()).unwrap();
    let exec_with = |order: &str, cli: &str, needs: &[&str], told: bool| -> String {
        let mut seated = base.clone();
        seated.order = order.into();
        seated.needs = needs.iter().map(|need| (*need).to_owned()).collect();
        // The brief as seating writes it (H6): the bladebro line when told, which the launch reads back (H7).
        seated.order_file = project
            .root
            .join(format!("{order}.md"))
            .display()
            .to_string();
        let told = if told {
            format!("# Read the page\n\n{}", crate::nirvana_lean::BROWSER_LINE)
        } else {
            "# Read the page\n".to_owned()
        };
        std::fs::write(&seated.order_file, told).unwrap();
        if cli == "claude" {
            seated.seat = "sonnet".into();
            seated.cli = "claude".into();
            seated.model = "claude-sonnet-5-5".into();
            seated.pool = "max".into();
        }
        journal.append(2, Kind::Seated(Box::new(seated))).unwrap();
        let seen = format!("{order}.env.txt");
        TEST_COMMAND.with_borrow_mut(|slot| {
            *slot = Some(vec![
                "cmd.exe".into(),
                "/c".into(),
                format!("(echo [%BLADE_HOME%] [%BLADE_LANE%])> {seen} & type recorded.jsonl"),
            ]);
        });
        let mut out = Vec::new();
        exec(&project, run, order, None, &mut out).unwrap();
        TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
        std::fs::read_to_string(project.root.join(&seen)).unwrap()
    };
    for (order, cli) in [("a1", "agy"), ("c1", "claude")] {
        let home = crate::nirvana_bladebro::home(&run_dir, order);
        assert_eq!(
            exec_with(order, cli, &["browser"], true).trim(),
            format!("[{}] [agent]", home.display()),
            "{cli}"
        );
        assert!(!home.exists(), "{cli}");
    }
    // No grant: cmd echoes the unset names back.
    assert_eq!(
        exec_with("a2", "agy", &[], false).trim(),
        "[%BLADE_HOME%] [%BLADE_LANE%]"
    );
    // P35c review round S3: a brief written without the line gets no grant at launch, on either CLI, and no home.
    for (order, cli) in [("a3", "agy"), ("c3", "claude")] {
        assert_eq!(
            exec_with(order, cli, &["browser"], false).trim(),
            "[%BLADE_HOME%] [%BLADE_LANE%]",
            "{cli}"
        );
        assert!(
            !crate::nirvana_bladebro::home(&run_dir, order).exists(),
            "{cli}"
        );
    }
    crate::nirvana_bladebro::TEST_INSTALLED.set(None);
}

/// Plan 35 P35c review round, H6 through `herdr seat run`: the brief's line is decided as the launch decides the
/// grant. A browser order on agy is told to use bladebro; one seated with a profile that denies it, or whose workspace
/// names a bladebro of its own, is not.
#[test]
fn p35c_seat_run_writes_the_bladebro_line_only_where_the_launch_grants_it() {
    crate::nirvana_bladebro::TEST_INSTALLED.set(Some(true));
    let (_home, project) = p29_project("p35c-seat-brief");
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let seat = nirvana_seat::pick(
        &table,
        &nirvana_seat::live_world(&table),
        &nirvana_seat::Request::new("research.web"),
    )
    .unwrap();
    assert_eq!(seat.cli, "agy", "{}", seat.seat);
    let order_file = project.root.join("order.md");
    std::fs::write(&order_file, "# Read the page\n").unwrap();
    let request = |order: &str| SeatRequest {
        seat: seat.seat.clone(),
        order_file: order_file.clone(),
        order: order.into(),
        run: "r1".into(),
        class: Some("research.web".into()),
        size: None,
        needs: vec!["bladebro".into()],
        not_family: Vec::new(),
        mode: None,
        effort: None,
        reason: None,
        worktree: false,
        auto: false,
        send_ok: None,
    };
    // Each order is held live, so `seat_order` starts no worker.
    let _live: Vec<_> = ["b1", "b2", "b3"]
        .iter()
        .map(|order| take_live(&project, "r1", order).unwrap().unwrap())
        .collect();
    let brief = |order: &str| {
        let state = replay(&project.journal("r1").read().0);
        std::fs::read_to_string(&state.order(order).unwrap().seated.order_file).unwrap()
    };
    let line = crate::nirvana_lean::BROWSER_LINE;
    seat_order(&project, &request("b1")).unwrap();
    assert!(brief("b1").contains(line), "{}", brief("b1"));
    // A profile that denies bladebro whole.
    let profile = "---\ndescription: Reads pages without a browser.\nclass: research.web\n\
                   disallowed_tools: [mcp__bladebro__*]\n---\nRead only.\n";
    std::fs::create_dir_all(crate::nirvana_profiles::folder(&project.root)).unwrap();
    std::fs::write(
        crate::nirvana_profiles::path(&project.root, "reader"),
        profile,
    )
    .unwrap();
    seat_order_with(&project, &request("b2"), Some("reader")).unwrap();
    assert!(!brief("b2").contains(line), "{}", brief("b2"));
    // The workspace's own bladebro.
    let native = project.root.join(crate::nirvana_agy_tools::MCP_CONFIG);
    std::fs::create_dir_all(native.parent().unwrap()).unwrap();
    std::fs::write(
        &native,
        r#"{"mcpServers": {"bladebro": {"command": "bladebro"}}}"#,
    )
    .unwrap();
    seat_order(&project, &request("b3")).unwrap();
    assert!(!brief("b3").contains(line), "{}", brief("b3"));
    crate::nirvana_bladebro::TEST_INSTALLED.set(None);
    nirvana_seat::set_test_world(None);
}

// ---- plan 35 P35b review round ----

/// S4 and S2 through `herdr seat exec`: the supervisor sweeps the tool files of an ended order of any run, keeps those
/// of an order that still wants a worker, and journals a `finished_reason` it does not know with the order's end.
#[cfg(windows)]
#[test]
fn p35b_exec_sweeps_the_tool_files_ended_orders_left_and_journals_an_unknown_finish() {
    let (_home, project, base) = p35b_project("p35b-sweep", false, &[]);
    let later = LIVE_FLASH.replacen(
        "\"status\":\"SUCCESS\",",
        "\"status\":\"SUCCESS\",\"finished_reason\":\"stop\",",
        1,
    );
    assert_ne!(later, LIVE_FLASH);
    std::fs::write(project.root.join("recorded.jsonl"), later).unwrap();
    // Run r0: `gone` and `held` ended, `waits` is still seated; all three left their files. A supervisor still holds
    // `held` (its last removals not done yet).
    let old = project.journal("r0");
    old.append(1, opened()).unwrap();
    for name in ["gone", "held", "waits"] {
        let mut seated = base.clone();
        seated.order = name.into();
        old.append(2, Kind::Seated(Box::new(seated))).unwrap();
    }
    for name in ["gone", "held"] {
        old.append(
            3,
            Kind::Ended {
                order: Some(name.into()),
                outcome: "failed".into(),
                why: Some("herdr was killed".into()),
                exit: None,
                pools: Vec::new(),
                pools_basis: None,
            },
        )
        .unwrap();
    }
    let _held = take_live(&project, "r0", "held").unwrap().unwrap();
    let r0 = project.run_dir("r0");
    let leave = |order: &str| {
        let tools = crate::nirvana_agy_tools::tools_dir(&r0, order).join(".agents");
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::write(tools.join("mcp_config.json"), "{}").unwrap();
        std::fs::write(crate::nirvana_agy_tools::settings_path(&r0, order), "{}").unwrap();
        std::fs::create_dir_all(r0.join("mcp")).unwrap();
        std::fs::write(r0.join("mcp").join(format!("{order}.json")), "{}").unwrap();
    };
    leave("gone");
    leave("held");
    leave("waits");
    let journal = project.journal("r1");
    journal.append(1, opened()).unwrap();
    journal
        .append(2, Kind::Seated(Box::new(base.clone())))
        .unwrap();
    TEST_COMMAND.with_borrow_mut(|slot| {
        *slot = Some(vec![
            "cmd.exe".into(),
            "/c".into(),
            "type recorded.jsonl".into(),
        ]);
    });
    let mut out = Vec::new();
    let code = exec(&project, "r1", "o1", None, &mut out).unwrap();
    TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
    let view = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(code, 0, "{view}");
    assert_eq!(
        crate::nirvana_agy_tools::leftover_orders(&r0),
        ["held".to_owned(), "waits".to_owned()]
    );
    assert!(
        view.contains(
            "removed the MCP and agy tool files gone of run r0 left behind after it ended"
        ),
        "{view}"
    );
    assert!(!view.contains("files waits"), "{view}");
    let why = journal
        .read()
        .0
        .into_iter()
        .find_map(|event| match event.kind {
            Kind::Ended {
                order: Some(o),
                outcome,
                why,
                ..
            } if o == "o1" => Some((outcome, why)),
            _ => None,
        });
    let (outcome, why) = why.expect("ended");
    assert_eq!(outcome, "delivered");
    assert!(
        why.as_deref()
            .is_some_and(|why| why.starts_with("agy finished with \"stop\" and status SUCCESS")),
        "{why:?}"
    );
    assert!(view.contains("agy finished with \"stop\""), "{view}");
    nirvana_seat::set_test_world(None);
}

/// S5: `herdr seat cleanup` removes the order's tool files, through `cleanup`, which `cleanup_cli` calls.
#[test]
fn p35b_seat_cleanup_removes_the_orders_tool_files() {
    let (_home, project, seated) = p35b_project("p35b-cleanup", false, &[]);
    let journal = project.journal("r1");
    journal.append(1, opened()).unwrap();
    journal
        .append(2, Kind::Seated(Box::new(seated.clone())))
        .unwrap();
    let run_dir = project.run_dir("r1");
    let tools = crate::nirvana_agy_tools::tools_dir(&run_dir, "o1");
    std::fs::create_dir_all(&tools).unwrap();
    std::fs::write(tools.join("AGENTS.md"), "rules\n").unwrap();
    std::fs::write(
        crate::nirvana_agy_tools::settings_path(&run_dir, "o1"),
        "{}",
    )
    .unwrap();
    std::fs::create_dir_all(run_dir.join("mcp")).unwrap();
    std::fs::write(run_dir.join("mcp").join("o1.json"), "{}").unwrap();
    let mut out = Vec::new();
    assert!(cleanup(&project, "r1", &journal, "o1", &mut out).unwrap());
    assert!(crate::nirvana_agy_tools::leftover_orders(&run_dir).is_empty());
    assert!(
        String::from_utf8_lossy(&out).ends_with("o1: cleaned up; its branch (none) stays\n"),
        "{}",
        String::from_utf8_lossy(&out)
    );
    assert!(!cleanup(&project, "r1", &journal, "nobody", &mut Vec::new()).unwrap());
    nirvana_seat::set_test_world(None);
}

/// Nit: an agy research attempt whose brief came from a claude parent gets the sources line once; claude never does.
#[test]
fn p35b_an_agy_research_child_brief_gets_the_sources_line_once() {
    use crate::nirvana_agy_tools::SOURCES_LINE;
    let dir = temp_dir("p35b_child_sources");
    let mut child = seated("c1");
    child.class = "research.web".into();
    child.order_file = dir.join("c1.md").display().to_string();
    std::fs::write(&child.order_file, "# Continue the dial specs").unwrap();
    let mut out = Vec::new();
    ensure_sources_line(&child, &mut out);
    ensure_sources_line(&child, &mut out);
    assert_eq!(
        std::fs::read_to_string(&child.order_file).unwrap(),
        format!("# Continue the dial specs\n\n{SOURCES_LINE}\n")
    );
    let mut claude = child.clone();
    claude.cli = "claude".into();
    claude.order_file = dir.join("k1.md").display().to_string();
    std::fs::write(&claude.order_file, "# Continue\n").unwrap();
    ensure_sources_line(&claude, &mut out);
    assert_eq!(
        std::fs::read_to_string(&claude.order_file).unwrap(),
        "# Continue\n"
    );
    assert!(out.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

/// Plan 37 §1.1 (L3, "Yes, after the fixes"): `seat exec` gates an ordinary done delivery itself, with the same
/// call it makes for a lesson review, and only then wakes Nirvana Claude, so the wake reads a journal that already
/// holds the verdict and gating costs no orchestrator turn. A `blocked` delivery is plan 37b's
/// (`p37b_exec_gates_a_blocked_delivery_and_its_wake_keeps_the_block`).
#[cfg(windows)]
#[test]
fn p37_exec_gates_an_ordinary_done_delivery_before_the_wake() {
    let home = crate::nirvana::with_test_home("p37-exec-gate");
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let project = Project::at(home.path.join("project"));
    project.ensure().unwrap();
    std::fs::write(project.root.join("done.jsonl"), LIVE_HAIKU).unwrap();
    std::fs::write(project.root.join("order.md"), "# Research\n").unwrap();
    let run = "r1";
    let journal = project.journal(run);
    journal.append(1, opened()).unwrap();
    // A send-back's child is held live, so the gate starts no worker.
    let _children: Vec<_> = ["o1-s1"]
        .iter()
        .map(|child| take_live(&project, run, child).unwrap().unwrap())
        .collect();
    let exec_with = |order: &str, recorded: &str| -> (i32, String) {
        let mut s = seated(order);
        s.seat = "sonnet".into();
        s.cli = "claude".into();
        s.model = "claude-sonnet-5-5".into();
        s.effort = Effort::Medium;
        s.pool = "max".into();
        s.class = "research.web".into();
        s.chain = Some("research".into());
        s.not_family = Vec::new();
        s.cwd = project.root.display().to_string();
        s.order_file = project.root.join("order.md").display().to_string();
        journal.append(2, Kind::Seated(Box::new(s))).unwrap();
        TEST_COMMAND.with_borrow_mut(|slot| {
            *slot = Some(vec![
                "cmd.exe".into(),
                "/c".into(),
                format!("type {recorded}"),
            ]);
        });
        let mut out = Vec::new();
        let code = exec(&project, run, order, None, &mut out).unwrap();
        TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
        (code, String::from_utf8_lossy(&out).into_owned())
    };
    let seq = |order: &str, gated: bool| {
        journal
            .read()
            .0
            .into_iter()
            .find_map(|event| match &event.kind {
                Kind::Gated { order: o, .. } if gated && o == order => Some(event.seq),
                Kind::WakeQueued { .. } | Kind::Woken { .. } if !gated && event.seq > 2 => {
                    Some(event.seq)
                }
                _ => None,
            })
    };
    let (code, view) = exec_with("o1", "done.jsonl");
    assert_eq!(code, 0, "{view}");
    // The report names no sources, so the gate sends it back; its verdict is in the journal before any wake.
    assert!(view.contains("gated: sent_back"), "{view}");
    let gated = seq("o1", true).expect("o1 gated");
    let woke = seq("o1", false).expect("a wake after the delivery");
    assert!(gated < woke, "gated at {gated}, woken at {woke}");
    let state = replay(&journal.read().0);
    assert_eq!(
        state.order("o1").and_then(|o| o.verdict.as_deref()),
        Some("sent_back")
    );
    assert!(state.order("o1-s1").is_some());
    let digest = nirvana_run::digest(run, &state.queue);
    assert!(digest.contains("o1 gated sent_back: "), "{digest}");
    // Review N1: a gate that panics is journaled as the run's question and the wake still goes.
    crate::nirvana_gate::TEST_GATE_PANICS.set(true);
    let before = journal.read().0.len();
    let (code, view) = exec_with("o3", "done.jsonl");
    assert!(!crate::nirvana_gate::TEST_GATE_PANICS.get());
    assert_eq!(code, 0, "{view}");
    assert!(
        view.contains("not gated: herdr's gate of o3 stopped"),
        "{view}"
    );
    let after = journal.read().0;
    let asked = after[before..].iter().position(|event| {
        matches!(&event.kind, Kind::Asked { order: None, question } if question.contains("herdr gate run o3"))
    });
    let woke = after[before..]
        .iter()
        .position(|event| matches!(event.kind, Kind::WakeQueued { .. } | Kind::Woken { .. }));
    assert!(
        matches!((asked, woke), (Some(a), Some(w)) if a < w),
        "asked at {asked:?}, woken at {woke:?}"
    );
    nirvana_seat::set_test_world(None);
}

/// Plan 37b (the owner, 2026-11-01: "send it blocked as well for validationg"): `seat exec` gates a blocked delivery
/// before the wake, as it does a done one. Its verdict is `asked`, and the block reason is in what the orchestrator
/// wakes to: one line, the gate's question (review S4). A helper's blocked delivery is not gated.
#[cfg(windows)]
#[test]
fn p37b_exec_gates_a_blocked_delivery_and_its_wake_keeps_the_block() {
    const BLOCK: &str = "BLOCK the spec sheet is behind a login";
    let home = crate::nirvana::with_test_home("p37b-exec-blocked");
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let project = Project::at(home.path.join("project"));
    project.ensure().unwrap();
    let blocked = LIVE_HAIKU
        .replace(r#"\"status\":\"done\""#, r#"\"status\":\"blocked\""#)
        .replace(r#""status":"done""#, r#""status":"blocked""#)
        .replace("Added sub(a, b) returning a - b to calc.py", BLOCK);
    assert!(!blocked.contains(r#""status":"done""#));
    assert!(blocked.contains(BLOCK));
    std::fs::write(project.root.join("blocked.jsonl"), blocked).unwrap();
    std::fs::write(project.root.join("order.md"), "# Research\n").unwrap();
    let run = "r1";
    let journal = project.journal(run);
    journal.append(1, opened()).unwrap();
    // A send-back's child is held live, so a gate that sent it back would start no worker.
    let _child = take_live(&project, run, "o2-s1").unwrap().unwrap();
    let exec_with = |order: &str, class: &str| -> (i32, String) {
        let mut s = seated(order);
        s.seat = "sonnet".into();
        s.cli = "claude".into();
        s.model = "claude-sonnet-5-5".into();
        s.effort = Effort::Medium;
        s.pool = "max".into();
        s.class = class.into();
        s.chain = Some("research".into());
        s.not_family = Vec::new();
        s.cwd = project.root.display().to_string();
        s.order_file = project.root.join("order.md").display().to_string();
        journal.append(2, Kind::Seated(Box::new(s))).unwrap();
        TEST_COMMAND.with_borrow_mut(|slot| {
            *slot = Some(vec![
                "cmd.exe".into(),
                "/c".into(),
                "type blocked.jsonl".into(),
            ]);
        });
        let mut out = Vec::new();
        let code = exec(&project, run, order, None, &mut out).unwrap();
        TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
        (code, String::from_utf8_lossy(&out).into_owned())
    };
    let (code, view) = exec_with("o2", "research.web");
    assert_eq!(code, 0, "{view}");
    assert!(view.contains("gated: asked"), "{view}");
    let events = journal.read().0;
    let gated = events
        .iter()
        .find(|event| matches!(&event.kind, Kind::Gated { order, .. } if order == "o2"))
        .map(|event| event.seq)
        .expect("o2 gated");
    let woke = events
        .iter()
        .find(|event| {
            event.seq > 2 && matches!(event.kind, Kind::WakeQueued { .. } | Kind::Woken { .. })
        })
        .map(|event| event.seq)
        .expect("a wake after the delivery");
    assert!(gated < woke, "gated at {gated}, woken at {woke}");
    // A gate by hand gives the stored question and queues nothing more.
    let again = crate::nirvana_gate::gate(&project, run, "o2").expect("the stored verdict");
    assert_eq!(again.verdict, "asked");
    // A second verdict journaled later (a regate) does not queue: the first after the delivery is the one.
    journal
        .append(
            9,
            Kind::Gated {
                order: "o2".into(),
                verdict: "asked".into(),
                reason: Some("a later gate".into()),
                axes: Vec::new(),
            },
        )
        .unwrap();
    let state = replay(&journal.read().0);
    assert!(
        state.order("o2-s1").is_none(),
        "a blocked order was sent back"
    );
    let lines: Vec<&str> = state.queue.iter().map(|item| item.line.as_str()).collect();
    // Review S4: one wake entry carries the block, the gate's question; neither the delivered line, the gated line
    // nor the regate's is queued.
    let o2: Vec<&&str> = lines
        .iter()
        .filter(|line| line.starts_with("o2 "))
        .collect();
    assert_eq!(o2.len(), 1, "{lines:?}");
    assert!(
        o2[0].starts_with(&format!("o2 asks: o2 is blocked: {BLOCK}")),
        "{lines:?}"
    );
    // A helper's blocked delivery stays the orchestrator's to gate.
    crate::nirvana_gate::write_sidecar(
        &project,
        run,
        &crate::nirvana_gate::Sidecar {
            order: "o2-rev1".into(),
            helps: Some(("review".into(), "o2".into())),
            ..crate::nirvana_gate::Sidecar::default()
        },
    )
    .unwrap();
    let (code, view) = exec_with("o2-rev1", "review.adversary");
    assert_eq!(code, 0, "{view}");
    assert!(!view.contains("gated:"), "{view}");
    assert!(!journal
        .read()
        .0
        .iter()
        .any(|event| matches!(&event.kind, Kind::Gated { order, .. } if order == "o2-rev1")));
    nirvana_seat::set_test_world(None);
}

// ---- live checks 2026-11-01 F2 and F3 ----

/// F2: `seat run` asks a research order for its sources only when it asks for the web: lc4's order (rules from
/// memory) gets `NO_WEB_SOURCES`, the same class with a URL in it gets `WEB_SOURCES`.
#[test]
fn gsrc_seat_run_asks_sources_only_of_an_order_that_asks_for_the_web() {
    use crate::nirvana_gate::{NO_WEB_SOURCES, WEB_SOURCES};
    let (_home, project) = p29_project("gsrc-sources-line");
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let world = nirvana_seat::live_world(&table);
    let rule = nirvana_seat::pick(&table, &world, &nirvana_seat::Request::new("research.read"))
        .unwrap()
        .seat;
    let seat = |order: &str, text: &str| {
        let order_file = project.root.join(format!("{order}.md"));
        std::fs::write(&order_file, text).unwrap();
        SeatRequest {
            seat: rule.clone(),
            order_file,
            order: order.into(),
            run: "r1".into(),
            class: Some("research.read".into()),
            size: None,
            needs: Vec::new(),
            not_family: Vec::new(),
            mode: None,
            effort: None,
            reason: None,
            worktree: false,
            auto: false,
            send_ok: None,
        }
    };
    let _live: Vec<_> = ["lc4", "url"]
        .iter()
        .map(|order| take_live(&project, "r1", order).unwrap().unwrap())
        .collect();
    seat_order(
        &project,
        &seat(
            "lc4",
            "Without opening, reading or listing any file, quote every rule.\n",
        ),
    )
    .unwrap();
    seat_order(
        &project,
        &seat("url", "Summarise https://example.com/spec.\n"),
    )
    .unwrap();
    let state = replay(&project.journal("r1").read().0);
    let brief = |order: &str| {
        std::fs::read_to_string(&state.order(order).unwrap().seated.order_file).unwrap()
    };
    assert!(brief("lc4").contains(NO_WEB_SOURCES), "{}", brief("lc4"));
    assert!(!brief("lc4").contains(WEB_SOURCES), "{}", brief("lc4"));
    assert!(brief("url").contains(WEB_SOURCES), "{}", brief("url"));
    assert!(!brief("url").contains(NO_WEB_SOURCES), "{}", brief("url"));
    // The gate reads the line back from the order copy.
    assert!(crate::nirvana_gate::spec(&brief("lc4")).no_web);
    assert!(!crate::nirvana_gate::spec(&brief("url")).no_web);
    nirvana_seat::set_test_world(None);
}

/// F3: a rate-limited reseat runs at the effort its parent ran at, not the class's (sweep.mechanical is low; the
/// parent ran haiku medium).
#[test]
fn gsrc_a_rate_limited_reseat_keeps_the_parents_effort() {
    let (_home, project) = p29_project("gsrc-reseat-effort");
    let run = "r1";
    let journal = project.journal(run);
    let mut seated = p29_seated(&project, "o1", "haiku");
    seated.needs = Vec::new();
    assert_eq!(seated.effort, Effort::Medium);
    journal.append(1, opened()).unwrap();
    journal
        .append(2, Kind::Seated(Box::new(seated.clone())))
        .unwrap();
    let child = child_id(&replay(&journal.read().0), "o1");
    let _live = take_live(&project, run, &child).unwrap().unwrap();
    let limit = RateLimit {
        signal: "rate_limit_event".into(),
        pool: Some("max".into()),
        window: None,
        resets_at: None,
        detail: String::new(),
    };
    // The rule alone reseats it at the class's effort: the test has an effort to keep.
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let spent = spent_world(&table, nirvana_seat::live_world(&table), &limit, "max");
    let mut plain = nirvana_seat::Request::new("sweep.mechanical");
    plain.size = seated.size;
    assert_eq!(
        nirvana_seat::pick(&table, &spent, &plain).unwrap().effort,
        Effort::Low
    );
    let mut out = Vec::new();
    rate_limited(
        &project,
        &journal,
        run,
        &seated,
        &limit,
        &Stream::new("claude"),
        &mut out,
    )
    .unwrap();
    let view = String::from_utf8_lossy(&out).into_owned();
    let state = replay(&journal.read().0);
    let reseated = &state.order(&child).expect(&view).seated;
    assert_eq!(reseated.effort, Effort::Medium, "{}", reseated.reason);
    let seat = table.seat(&reseated.seat).expect("seat");
    assert_eq!(reseated.model, seat.model_for(Effort::Medium));
    nirvana_seat::set_test_world(None);
}

/// Review S1: the needs a booking was made with reach the sources line. `seat pick --book --needs browser` (or
/// `bladebro`, which the seat rule never reads), then `seat run` with no `--needs` as the seat skill writes it, is a web
/// order; the same order booked with no need is not.
#[test]
fn gsrc_seat_run_reads_the_needs_the_booking_was_made_with() {
    use crate::nirvana_gate::{NO_WEB_SOURCES, WEB_SOURCES};
    let (_home, project) = p29_project("gsrc-booked-needs");
    std::env::set_var(crate::nirvana_run::PROJECT_ENV, &project.root);
    let book = |order: &str, needs: &str| {
        let mut args: Vec<String> = [
            "pick",
            "--class",
            "research.read",
            "--book",
            order,
            "--run",
            "r1",
        ]
        .map(String::from)
        .to_vec();
        if !needs.is_empty() {
            args.extend(["--needs".to_owned(), needs.to_owned()]);
        }
        nirvana_seat::cli(&args).unwrap();
    };
    for (order, needs) in [("web1", "browser"), ("web2", "bladebro"), ("plain1", "")] {
        book(order, needs);
    }
    std::env::remove_var(crate::nirvana_run::PROJECT_ENV);
    let leases =
        nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE)).unwrap();
    let now = crate::nirvana_usage::unix(std::time::SystemTime::now());
    // The written order asks for no web by itself: only the booking's need makes it one.
    let text = "Check the vendor's pricing page and give each plan's price.\n";
    assert_eq!(
        crate::nirvana_gate::web_asked("research.read", &[], text),
        None
    );
    let order_file = project.root.join("order.md");
    std::fs::write(&order_file, text).unwrap();
    let _live: Vec<_> = ["web1", "web2", "plain1"]
        .iter()
        .map(|order| take_live(&project, "r1", order).unwrap().unwrap())
        .collect();
    for order in ["web1", "web2", "plain1"] {
        let seat = leases.live(order, now).expect(order).seat.clone();
        seat_order(
            &project,
            &SeatRequest {
                seat,
                order_file: order_file.clone(),
                order: order.into(),
                run: "r1".into(),
                class: None,
                size: None,
                needs: Vec::new(),
                not_family: Vec::new(),
                mode: None,
                effort: None,
                reason: None,
                worktree: false,
                auto: false,
                send_ok: None,
            },
        )
        .unwrap();
    }
    let state = replay(&project.journal("r1").read().0);
    let brief = |order: &str| {
        std::fs::read_to_string(&state.order(order).unwrap().seated.order_file).unwrap()
    };
    for order in ["web1", "web2"] {
        assert!(
            brief(order).contains(WEB_SOURCES),
            "{order}: {}",
            brief(order)
        );
    }
    assert!(
        brief("plain1").contains(NO_WEB_SOURCES),
        "{}",
        brief("plain1")
    );
    // `seat run --class --needs` books for itself, and its lease keeps the needs as given for a later run.
    let rule = leases.live("plain1", now).unwrap().seat.clone();
    let _web3 = take_live(&project, "r1", "web3").unwrap().unwrap();
    seat_order(
        &project,
        &SeatRequest {
            seat: rule,
            order_file: order_file.clone(),
            order: "web3".into(),
            run: "r1".into(),
            class: Some("research.read".into()),
            size: None,
            needs: vec!["bladebro".into()],
            not_family: Vec::new(),
            mode: None,
            effort: None,
            reason: None,
            worktree: false,
            auto: false,
            send_ok: None,
        },
    )
    .unwrap();
    let leases =
        nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE)).unwrap();
    assert_eq!(
        leases.live("web3", now).expect("web3").pick["needs"],
        serde_json::json!(["bladebro"])
    );
    nirvana_seat::set_test_world(None);
}

// ---- plan 40 (c40b): a lighter worker start, the report's optional lists, the tee, the readers ----

/// Plan 40: every claude worker drops the dynamic system prompt sections; agy has no such flag.
#[test]
fn c40b_a_claude_worker_excludes_the_dynamic_system_prompt_sections() {
    let claude = argv(&spec("claude", "claude-haiku-5-5", None)).expect("argv");
    assert_eq!(
        claude.iter().filter(|arg| *arg == EXCLUDE_DYNAMIC).count(),
        1,
        "{claude:?}"
    );
    let resumed = argv(&spec("claude", "claude-haiku-5-5", Some("s1"))).expect("argv");
    assert!(resumed.iter().any(|arg| arg == EXCLUDE_DYNAMIC));
    let agy = argv(&spec("agy", "gemini-3.8-flash-low", None)).expect("argv");
    assert!(!agy.iter().any(|arg| arg == EXCLUDE_DYNAMIC), "{agy:?}");
}

/// Plan 40 T2: the footer says only what the prompt and the schema do not, and the owner's finish sentence.
#[test]
fn c40b_the_order_footer_says_finish_the_order_and_nothing_twice() {
    assert!(ORDER_FOOTER.contains(FOOTER_MARK));
    assert!(ORDER_FOOTER.contains(
        "Finish the whole order before you report. Work through ordinary obstacles such as an error, a timeout or a \
         locked file. Report `blocked` only for something the order forbids or only the owner can decide."
    ));
    assert!(!ORDER_FOOTER.contains("Work only in this folder"));
    assert!(!ORDER_FOOTER.contains("Your last message is the Work Report"));
    assert!(first_line("o.md").contains("Work only in this folder"));
}

/// Plan 40 T3: a report without `tests` or `files` passes; either one in a wrong shape still fails.
#[test]
fn c40b_a_report_without_tests_or_files_passes() {
    let mut lean = good_report();
    let object = lean.as_object_mut().expect("object");
    object.remove("tests");
    object.remove("files");
    assert!(check_report(&lean).is_ok(), "{lean}");
    for (key, wrong) in [
        ("tests", serde_json::json!("3 pass")),
        ("tests", serde_json::json!([{"name": "t"}])),
        ("files", serde_json::json!("calc.py")),
    ] {
        let mut report = lean.clone();
        report[key] = wrong.clone();
        assert!(check_report(&report).is_err(), "{key}: {wrong}");
    }
    let mut no_gotchas = lean.clone();
    no_gotchas.as_object_mut().expect("object").remove("gotchas");
    assert!(check_report(&no_gotchas).is_err());
}

fn image_line(bytes: usize) -> String {
    serde_json::json!({"type": "user", "message": {"role": "user", "content": [{"type": "tool_result",
        "tool_use_id": "t1", "content": [{"type": "image", "source": {"type": "base64", "media_type": "image/png",
        "data": "A".repeat(bytes)}}, {"type": "text", "text": "evidence words"}]}]}})
    .to_string()
}

/// Plan 40 X1: the tee keeps an image block's type, media type and length, and drops its base64; every other line
/// is teed as it came, and the gate's readers still find the file the worker read.
#[test]
fn c40b_the_tee_drops_an_images_base64_and_keeps_the_rest() {
    let line = image_line(200_000);
    let teed = tee_line(&line);
    assert!(teed.len() < 1_000, "{} bytes", teed.len());
    let value: Value = serde_json::from_str(&teed).expect("JSON");
    let block = &value["message"]["content"][0]["content"][0];
    assert_eq!(block["type"], "image");
    assert_eq!(block["source"]["media_type"], "image/png");
    assert_eq!(block["source"]["data"], "");
    assert_eq!(block["source"]["data_omitted_chars"], 200_000);
    assert!(teed.contains("evidence words"));
    // A short line, and a long line with no base64 in it, are teed byte for byte.
    let short = image_line(100);
    assert_eq!(tee_line(&short), short.as_str());
    let prose = serde_json::json!({"type": "assistant", "message": {"content": [{"type": "text",
        "text": "word ".repeat(10_000)}]}})
    .to_string();
    assert_eq!(tee_line(&prose), prose.as_str());
    // The gate still reads the Read call before it: a cited screenshot counts as read.
    let read = serde_json::json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t1",
        "name": "Read", "input": {"file_path": r"C:\shots\a.png"}}]}})
    .to_string();
    let stream = format!("{}\n{teed}", tee_line(&read));
    let report = serde_json::json!({"status": "done", "summary": "s", "gotchas": [],
        "sources": [{"url": "file:///C:/shots/a.png", "used_for": "the screen"}]});
    let axis = crate::nirvana_gate::sources_axis("media.image", None, Some(&report), &stream)
        .expect("an axis");
    assert_eq!(axis.mark, crate::nirvana_gate::Mark::Pass, "{axis:?}");
}

/// Review B3: a byte that is not UTF-8 never ends a reader; lines go on after it.
#[test]
fn c40b_a_non_utf8_line_is_read_lossily_and_reading_goes_on() {
    let bytes: &[u8] = b"{\"type\":\"assistant\"}\r\n\xff\xfe half\n{\"type\":\"result\"}\nlast";
    let mut lines = Vec::new();
    job::lossy_lines(bytes, |line| {
        lines.push(line);
        true
    });
    assert_eq!(
        lines,
        vec![
            "{\"type\":\"assistant\"}".to_owned(),
            "\u{fffd}\u{fffd} half".to_owned(),
            "{\"type\":\"result\"}".to_owned(),
            "last".to_owned(),
        ]
    );
    // `each` returning false stops it.
    let mut first = Vec::new();
    job::lossy_lines(bytes, |line| {
        first.push(line);
        false
    });
    assert_eq!(first.len(), 1);
}

/// Review B7: what "a tool call is open" means in the recorded streams. claude: a `tool_use` until the `user` line
/// whose `tool_result` names its id; agy: a tool step `ACTIVE` until the same `step_index` is `DONE` or `ERROR`.
#[test]
fn c40b_a_tool_call_is_open_from_its_call_to_its_answer() {
    for (cli, text) in [("claude", LIVE_HAIKU), ("agy", LIVE_FLASH)] {
        let mut stream = Stream::new(cli);
        let mut opened = 0;
        for line in text.lines() {
            stream.feed(line);
            if stream.tool_open() {
                opened += 1;
            }
        }
        assert!(opened > 0, "{cli}: a call was open at some line");
        assert!(!stream.tool_open(), "{cli}: every call was answered by the end");
    }
    let mut stream = Stream::new("claude");
    stream.feed(r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"t9","name":"Bash","input":{"command":"cargo build"}}]}}"#);
    assert!(stream.tool_open());
    stream.feed(r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t9","content":"ok"}]}}"#);
    assert!(!stream.tool_open());
    let mut stream = Stream::new("agy");
    stream.feed(r#"{"event":"step_update","step_update":{"step_index":3,"state":"ACTIVE","step_type":"tool","tool_name":"run_command","tool_info":{"parameters":{"CommandLine":"cargo build"}}}}"#);
    assert!(stream.tool_open());
    stream.feed(r#"{"event":"step_update","step_update":{"step_index":3,"state":"DONE","step_type":"tool","tool_name":"run_command","tool_info":{"parameters":{}}}}"#);
    assert!(!stream.tool_open());
}
