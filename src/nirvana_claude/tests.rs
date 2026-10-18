use super::*;

/// 2026-10-17T21:51:12Z, when O1's live readings were taken (nirvana_usage's `READ_1017`).
const NOW: u64 = 1_792_273_872;

fn reading(pool: &str, window: &str, used: f32, resets_in: u64, age: u64) -> Reading {
    Reading {
        pool: pool.into(),
        window: window.into(),
        used: Some(used),
        resets_at: Some(NOW + resets_in),
        resets_text: None,
        read_at: NOW - age,
        source: "test".into(),
    }
}

fn source(readings: Vec<Reading>) -> Source {
    Source {
        read_at: readings.iter().map(|r| r.read_at).max(),
        readings,
        ..Source::default()
    }
}

/// The readings of 2026-10-17 21:51Z (plan 12 §15), in `usage.json`'s shape: Claude, Codex, agy, local.
fn recorded() -> [Source; 4] {
    [
        source(vec![
            reading("max", "5h", 0.41, 3 * 3600, 30),
            reading("max", "week", 0.37, 5 * 86_400, 30),
            reading("max", "week.fable", 0.0, 5 * 86_400, 30),
        ]),
        Source {
            word: Some("not installed".into()),
            read_at: Some(NOW),
            ..Source::default()
        },
        source(vec![
            reading("gemini", "5h", 0.0196, 4 * 3600, 30),
            reading("gemini", "week", 0.0102, 6 * 86_400, 30),
            reading("google3p", "5h", 0.033, 4 * 3600, 30),
            reading("google3p", "week", 0.01, 6 * 86_400, 30),
            reading("argon", "week", 0.0101, 6 * 86_400, 30),
        ]),
        Source {
            word: Some("off".into()),
            read_at: Some(NOW),
            ..Source::default()
        },
    ]
}

fn table() -> Table {
    nirvana_seat::parse(nirvana_seat::DEFAULT).expect("the default table parses")
}

fn full_digest() -> Digest {
    let long = "every order is finished and the run is not ended: `herdr run end`, or seat the next order, and this \
                sentence runs on well past what one line of the digest may hold";
    Digest {
        now: NOW,
        exe: r"C:\Users\o\AppData\Local\Programs\Nirvana-Herdr\bin\herdr.exe".into(),
        sources: recorded(),
        floor: 0.05,
        board: Some((7, vec!["plan".into(), "orders".into(), "notes".into()])),
        runs: (1..=6)
            .map(|n| {
                let step = if n % 2 == 0 {
                    Ok(format!("worker live: order-{n}"))
                } else {
                    Err(long.to_owned())
                };
                (format!("run-2026-10-18-{n}"), step)
            })
            .collect(),
        proposals: (1..=5)
            .map(|n| {
                (
                    format!("p-{n:08x}"),
                    format!("send-back: the report named no test, gap {n}"),
                )
            })
            .collect(),
        memory: r"C:\Users\o\AppData\Local\Programs\Nirvana-Herdr\nirvana\NIRVANA.md".into(),
    }
}

#[test]
fn the_digest_stays_under_300_tokens_with_every_section_full() {
    let text = digest_text(&full_digest());
    let chars = text.chars().count();
    // 540 characters at 1.8 a token, as `/context` counted this mix of paths and figures, is 300 tokens; the
    // live count is in the O4b record.
    assert!(chars <= DIGEST_MAX_CHARS, "{chars} characters:\n{text}");
    assert!(
        !text.ends_with('…'),
        "the full state fits without the last cut:\n{text}"
    );
    // Dead ends first, two named, the rest counted.
    assert!(
        text.contains("Runs open: run-2026-10-18-1: DEAD END"),
        "{text}"
    );
    assert!(text.contains("run-2026-10-18-3: DEAD END"), "{text}");
    assert!(
        !text.contains("run-2026-10-18-2"),
        "only two runs are named:\n{text}"
    );
    assert!(text.contains("and 4 more"), "{text}");
    // By priority: the runs, then the proposals; the board (in full at the first prompt anyway) and the hidden pools
    // give way.
    assert!(text.contains("Retro proposals open: p-00000001"), "{text}");
    assert!(text.contains("and 3 more."), "{text}");
    assert!(
        !text.contains("Board rev") && !text.contains("Not in the foot"),
        "{text}"
    );
    assert!(text.contains("NIRVANA.md"), "{text}");
    assert!(
        text.starts_with("Nirvana ") && text.lines().nth(1).unwrap().starts_with("Pools: "),
        "{text}"
    );
    assert!(
        text.lines().last().unwrap().starts_with("Memory: "),
        "{text}"
    );
    // Every line ends whole: no section's text ran past its own cap.
    assert!(
        text.lines().all(|line| line.chars().count() <= 180),
        "{text}"
    );
}

#[test]
fn a_digest_short_of_room_counts_the_runs_and_proposals_instead() {
    let mut digest = full_digest();
    digest.exe = format!(r"C:\{}\herdr.exe", "deep\\".repeat(36));
    let text = digest_text(&digest);
    assert!(
        text.contains("Runs open: 6, 3 at a dead end; `herdr run status`."),
        "{text}"
    );
    assert!(text.chars().count() <= DIGEST_MAX_CHARS, "{text}");
    assert!(
        text.lines().last().unwrap().starts_with("Memory: "),
        "{text}"
    );
}

#[test]
fn a_digest_with_room_names_the_board_the_runs_and_the_proposals_in_full() {
    let mut digest = full_digest();
    digest.runs.truncate(1);
    digest.proposals.truncate(1);
    let text = digest_text(&digest);
    assert!(text.contains("Board rev 7: plan, orders, notes."), "{text}");
    assert!(
        text.contains("p-00000001 (send-back: the report n…)"),
        "{text}"
    );
    assert!(
        text.contains("Runs open: run-2026-10-18-1: DEAD END"),
        "{text}"
    );
    assert!(text.chars().count() <= DIGEST_MAX_CHARS, "{text}");
}

#[test]
fn the_digest_draws_the_foots_pools_and_says_the_hidden_ones_have_headroom() {
    let mut digest = full_digest();
    digest.runs.clear();
    digest.proposals.clear();
    let text = digest_text(&digest);
    let pools = text
        .lines()
        .find(|l| l.starts_with("Pools:"))
        .expect("a pools line");
    assert!(
        pools.contains("Claude 5h 41% (3h), week 37% (5d0h)"),
        "{pools}"
    );
    assert!(pools.contains("Gemini 5h 2% (4h)"), "{pools}");
    // FOOT_HIDDEN: Google's Claude pool and Argon are priced, not drawn; not installed and off say nothing.
    for gone in ["G\u{b7}Claude", "Argon", "Codex", "Local", "fable"] {
        assert!(!pools.contains(gone), "{gone} in {pools}");
    }
    let hidden = text
        .lines()
        .find(|l| l.starts_with("Not in the foot"))
        .expect("the headroom line");
    assert!(
        hidden.contains("G\u{b7}Claude 5h 3%") && hidden.contains("Argon week 1%"),
        "{hidden}"
    );

    // A hidden pool at its floor has no headroom, and one that is not read is not said to have any.
    let mut digest = full_digest();
    digest.sources[2].readings = vec![
        reading("google3p", "5h", 0.96, 3600, 30),
        reading("google3p", "week", 0.2, 86_400, 30),
    ];
    let text = digest_text(&digest);
    assert!(!text.contains("Not in the foot"), "{text}");
}

#[test]
fn an_old_reading_says_its_age_and_a_passed_reset_is_not_a_figure() {
    let mut digest = full_digest();
    digest.sources[0] = source(vec![
        reading("max", "5h", 0.9, 0, 3 * 3600),
        reading("max", "week", 0.37, 5 * 86_400, 3 * 3600),
    ]);
    digest.sources[0].readings[0].resets_at = Some(NOW - 60);
    let text = digest_text(&digest);
    assert!(
        text.contains("Claude 5h 90% (reset since), week 37% (5d0h), read 3h ago"),
        "{text}"
    );
}

#[test]
fn the_digest_with_nothing_read_and_no_project_is_two_lines_and_the_memory() {
    let digest = Digest {
        now: NOW,
        exe: "herdr".into(),
        memory: "NIRVANA.md".into(),
        floor: 0.05,
        ..Digest::default()
    };
    let text = digest_text(&digest);
    assert_eq!(text.lines().count(), 3, "{text}");
    assert!(text.contains("Pools: none read yet"), "{text}");
    assert!(
        !text.contains("Board") && !text.contains("Runs open"),
        "{text}"
    );
}

fn spawn(agent: &str, model: Option<&str>, effort: Option<&str>) -> Spawn {
    Spawn {
        agent_type: agent.into(),
        model: model.map(Into::into),
        effort: effort.map(Into::into),
        prompt: "Class: review.adversary\nOrder: r1\nRead the diff.".into(),
    }
}

fn room(used: f32) -> Reserve {
    Reserve {
        used: Some(used),
        resets_at: Some(NOW + 2 * 3600),
        booked: 0.0,
        now: NOW,
    }
}

fn worker_def(model: &str, effort: &str) -> AgentDef {
    AgentDef {
        name: None,
        model: Some(model.into()),
        effort: Some(effort.into()),
    }
}

#[test]
fn a_spawn_with_no_model_or_no_effort_is_refused() {
    let table = table();
    for (model, effort, missing) in [
        (None, None, "no model and no effort"),
        (Some("sonnet"), None, "no effort"),
        (None, Some("low"), "no model"),
        (Some("inherit"), Some("low"), "no model"),
    ] {
        let verdict = guard(
            &spawn("general-purpose", model, effort),
            None,
            &table,
            &room(0.2),
        );
        match verdict {
            Verdict::Deny(why) => assert!(why.contains(missing), "{why}"),
            Verdict::Allow => panic!("{model:?} {effort:?} was let through"),
        }
    }
    // A fork always runs on the orchestrator's seat, and the orchestrator's own agent is no worker.
    for own in ["fork", "nirvana", "nirvana:nirvana"] {
        assert!(
            matches!(
                guard(
                    &spawn(own, Some("haiku"), Some("low")),
                    None,
                    &table,
                    &room(0.2)
                ),
                Verdict::Deny(_)
            ),
            "{own}"
        );
    }
    // The agent file supplies what the call leaves out.
    let def = worker_def("claude-sonnet-5-5", "medium");
    assert_eq!(
        guard(
            &spawn("nirvana:worker-sonnet-medium", None, None),
            Some(&def),
            &table,
            &room(0.2)
        ),
        Verdict::Allow
    );
    // A call that names both needs no agent file.
    assert_eq!(
        guard(
            &spawn("Explore", Some("haiku"), Some("low")),
            None,
            &table,
            &room(0.2)
        ),
        Verdict::Allow
    );
}

#[test]
fn a_spawn_on_max_that_would_break_the_reserve_is_refused() {
    let table = table();
    let def = worker_def("claude-opus-5-5", "high");
    let opus = spawn("nirvana:worker-opus-high", None, None);
    // opus high spends 0.224% of the 5 h window for a small order at weight 1 (seats.toml's prior); review.adversary's
    // weight scales it. The reserve is 15%.
    let weight = table.class("review.adversary").expect("a class").weight;
    let expected = 0.00224 * weight;
    let fits = 1.0 - 0.15 - expected - 0.001;
    let breaks = 1.0 - 0.15 - expected + 0.001;
    assert_eq!(
        guard(&opus, Some(&def), &table, &room(fits)),
        Verdict::Allow
    );
    match guard(&opus, Some(&def), &table, &room(breaks)) {
        Verdict::Deny(why) => {
            assert!(why.contains("reserve of 15%"), "{why}");
            assert!(why.contains("herdr seat pick"), "{why}");
            assert!(why.contains("2h"), "names the reset: {why}");
        }
        Verdict::Allow => panic!("a reserve breach was let through"),
    }
    // Live leases on the pool count against it.
    let mut booked = room(fits);
    booked.booked = 0.01;
    assert!(matches!(
        guard(&opus, Some(&def), &table, &booked),
        Verdict::Deny(_)
    ));
    // An unknown window is not a breach: the seat engine's reserve filter reads only a known one.
    let unknown = Reserve {
        used: None,
        ..room(0.0)
    };
    assert_eq!(guard(&opus, Some(&def), &table, &unknown), Verdict::Allow);
}

#[test]
fn models_and_aliases_find_their_seat_and_cells_price_like_the_seat_engine() {
    let table = table();
    for (model, seat) in [
        ("claude-opus-5-5", "opus"),
        ("opus", "opus"),
        ("Sonnet", "sonnet"),
        ("claude-haiku-5-5", "haiku"),
        ("opus[1m]", "opus"),
    ] {
        assert_eq!(
            seat_for_model(&table, model).map(|s| s.id.as_str()),
            Some(seat),
            "{model}"
        );
    }
    assert!(seat_for_model(&table, "gemini-4-argon-high").is_none());
    let haiku = table.seat("haiku").expect("haiku");
    assert!((expected_share(haiku, Effort::Low, "5h") - 0.000026).abs() < 1e-9);
    // No high cell: the dearest below stands in, as the engine's use_cell does.
    assert!((expected_share(haiku, Effort::High, "5h") - 0.000044).abs() < 1e-9);
}

#[test]
fn front_matter_and_the_plugins_agent_files_are_read() {
    let def = front_matter(
        "---\nname: worker-x\nmodel: \"claude-haiku-5-5\"\neffort: low\n---\nbody: not this\n",
    );
    assert_eq!(
        def,
        AgentDef {
            name: Some("worker-x".into()),
            model: Some("claude-haiku-5-5".into()),
            effort: Some("low".into()),
        }
    );
    assert_eq!(
        front_matter("no front matter\nmodel: opus\n"),
        AgentDef::default()
    );
    let plugin = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("nirvana")
        .join("claude");
    let found =
        find_agent("nirvana:worker-sonnet-low", Some(&plugin), &[]).expect("the plugin's agent");
    assert_eq!(found.model.as_deref(), Some("claude-sonnet-5-5"));
    assert_eq!(found.effort.as_deref(), Some("low"));
    assert!(find_agent("other:worker-sonnet-low", Some(&plugin), &[]).is_none());
}

/// Every `agents/worker-*.md` names a model and an effort in its front matter, the ones its name says, and lists
/// its tools without `Agent`: omitting them inherits the orchestrator's seat (CLAUDE.md rule 5).
#[test]
fn every_worker_agent_names_its_model_and_effort() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("nirvana")
        .join("claude")
        .join("agents");
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("agents/") {
        let path = entry.expect("an entry").path();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if !stem.starts_with(WORKER_PREFIX) {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("readable");
        let def = front_matter(&text);
        let (seat, effort) =
            worker_seat(&format!("{PLUGIN}:{stem}")).unwrap_or_else(|| panic!("{stem}"));
        let table = table();
        let seat = table
            .seat(&seat)
            .unwrap_or_else(|| panic!("{stem}: no seat"));
        assert_eq!(def.name.as_deref(), Some(stem.as_str()), "{stem}");
        assert_eq!(def.model.as_deref(), Some(seat.model.as_str()), "{stem}");
        assert_eq!(def.effort.as_deref(), Some(effort.word()), "{stem}");
        assert!(
            seat.efforts.contains(&effort),
            "{stem}: the seat has no {}",
            effort.word()
        );
        let head = text.split("---").nth(1).expect("front matter");
        let tools = head
            .lines()
            .find_map(|line| line.strip_prefix("tools:"))
            .unwrap_or_else(|| panic!("{stem}: lists no tools"));
        assert!(
            !tools.split(',').any(|tool| tool.trim() == "Agent"),
            "{stem}: {tools}"
        );
        assert!(text.contains("Work Report"), "{stem}: no Work Report");
        names.push(stem);
    }
    names.sort();
    assert_eq!(
        names,
        [
            "worker-haiku-low",
            "worker-opus-high",
            "worker-opus-medium",
            "worker-sonnet-low",
            "worker-sonnet-medium"
        ]
    );
    // The orchestrator's own agent file names its seat and has no prompt: the role is appended to Claude Code's.
    let own = std::fs::read_to_string(dir.join("nirvana.md")).expect("agents/nirvana.md");
    let def = front_matter(&own);
    assert_eq!(def.name.as_deref(), Some("nirvana"));
    assert_eq!(def.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(def.effort.as_deref(), Some("medium"));
}

fn transcript() -> String {
    [
        json!({"type": "user", "message": {"role": "user", "content": "Class: review.adversary\nOrder: rev-1\nRun: live1\n\nReview the diff."}}),
        json!({"type": "assistant", "message": {"id": "msg_1", "content": [{"type": "text", "text": "a"}], "usage": {"input_tokens": 10, "cache_creation_input_tokens": 100, "cache_read_input_tokens": 1000, "output_tokens": 5}}}),
        json!({"type": "assistant", "message": {"id": "msg_1", "content": [{"type": "tool_use"}], "usage": {"input_tokens": 10, "cache_creation_input_tokens": 100, "cache_read_input_tokens": 1000, "output_tokens": 5}}}),
        json!({"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "content": "Class: chore.script"}]}}),
        json!({"type": "assistant", "message": {"id": "msg_2", "content": [{"type": "text", "text": "b"}], "usage": {"input_tokens": 1, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 1200, "output_tokens": 300}}}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n")
}

const REPORT: &str = "Done.\n\n```json\n{\"status\": \"done\", \"summary\": \"no defect\", \"files\": [], \"tests\": [], \"gotchas\": []}\n```\n";

#[test]
fn a_worker_subagents_outcome_is_recorded_under_its_seat_effort_and_class() {
    let table = table();
    let hook = json!({"agent_type": "nirvana:worker-opus-high", "agent_id": "a1b2", "last_assistant_message": REPORT});
    let line = subagent_line(&table, &hook, &transcript())
        .expect("a worker")
        .expect("a line");
    assert_eq!(line.seat, "opus");
    assert_eq!(line.effort, Effort::High);
    assert_eq!(line.class, "review.adversary");
    assert_eq!(line.order, "rev-1");
    assert_eq!(line.run.as_deref(), Some("live1"));
    assert_eq!(line.outcome, "subagent-done");
    // Kept off every class chain, so O2's tally never counts an ungated outcome as a failure there.
    assert_eq!(line.chain.as_deref(), Some(SUBAGENT_CHAIN));
    // msg_1 is one response written as two lines: counted once.
    assert_eq!(
        line.tokens,
        Some(Tokens {
            input: 11,
            cache_write: 100,
            cache_read: 2200,
            output: 305
        })
    );
    let completed = nirvana_seat::complete(&table, line, NOW).expect("the table takes it");
    assert_eq!(completed.chain.as_deref(), Some(SUBAGENT_CHAIN));
    assert!(
        completed.spent.is_some(),
        "the use is computed from the tokens"
    );

    let no_report = json!({"agent_type": "nirvana:worker-opus-high", "agent_id": "a1b2", "last_assistant_message": "I looked."});
    let line = subagent_line(&table, &no_report, &transcript())
        .unwrap()
        .unwrap();
    assert_eq!(line.outcome, "subagent-no-report");

    let unclassed = transcript().replace("Class: review.adversary\\n", "");
    assert!(subagent_line(&table, &hook, &unclassed).unwrap().is_err());
    // Not a worker of this plugin: nothing to record, nothing said.
    for other in ["Explore", "nirvana:media", "other:worker-opus-high"] {
        assert!(
            subagent_line(&table, &json!({"agent_type": other}), &transcript()).is_none(),
            "{other}"
        );
    }
}

#[test]
fn the_work_report_is_found_fenced_bare_or_last() {
    assert_eq!(report_in(REPORT).unwrap()["status"], "done");
    let bare = "{\"status\": \"blocked\", \"summary\": \"s\", \"files\": [], \"tests\": [], \"gotchas\": []}";
    assert_eq!(report_in(bare).unwrap()["status"], "blocked");
    let trailing = format!("Here it is:\n{bare}");
    assert_eq!(report_in(&trailing).unwrap()["status"], "blocked");
    assert!(report_in("All good, no report.").is_err());
    assert!(
        report_in("```json\n{\"status\": \"done\"}\n```").is_err(),
        "a report without its keys fails O3's check"
    );
}

#[test]
fn brief_fields_read_the_first_class_order_and_run_lines() {
    let fields = brief_fields(
        "Order: o-1\n- Class: `chore.script`\nRun: r-2\nClass: plan\nclass: two words\n",
    );
    assert_eq!(
        fields,
        BriefFields {
            class: Some("chore.script".into()),
            order: Some("o-1".into()),
            run: Some("r-2".into())
        }
    );
    assert_eq!(brief_fields("Do the thing.").class, None);
    // A prose line that happens to start with a key is not the field.
    assert_eq!(
        brief_fields("Order: the one about calc\nOrder: o-9\n")
            .order
            .as_deref(),
        Some("o-9")
    );
}

#[test]
fn a_dead_end_blocks_once_then_tells_the_owner_and_then_says_nothing() {
    let dead = vec![(
        "live1".to_owned(),
        "every order is finished and the run is not ended".to_owned(),
        9,
    )];
    let none = BTreeMap::new();
    let first = stop_output(&dead, &none, false).expect("raised");
    assert_eq!(first["decision"], "block");
    assert!(first["reason"].as_str().unwrap().contains("live1"));
    // Claude Code is already continuing because of a Stop hook: never a second block.
    let again = stop_output(&dead, &none, true).expect("raised");
    assert!(again.get("decision").is_none());
    assert!(again["systemMessage"]
        .as_str()
        .unwrap()
        .contains("herdr run end"));
    // Raised at this state before: nothing.
    let raised: BTreeMap<String, u64> = [("live1".to_owned(), 9)].into();
    assert_eq!(stop_output(&dead, &raised, false), None);
    // The run moved on and stuck again: raised again.
    let moved: BTreeMap<String, u64> = [("live1".to_owned(), 7)].into();
    assert!(stop_output(&dead, &moved, false).is_some());
    assert_eq!(stop_output(&[], &none, false), None);
}

#[test]
fn the_statusline_draws_the_foots_pools_and_narrows_resets_then_orders() {
    let orders = vec![
        OpenOrder {
            order: "sub".into(),
            seat: "haiku".into(),
            phase: "running".into(),
        },
        OpenOrder {
            order: "mul".into(),
            seat: "flash".into(),
            phase: "delivered".into(),
        },
    ];
    let wide = statusline_text(&recorded(), &orders, 200, NOW);
    assert_eq!(
        wide,
        "Claude 5h 41% 3h · Gemini 5h 2% 4h · orders: sub haiku running, mul flash delivered"
    );
    for hidden in ["G\u{b7}Claude", "Argon"] {
        assert!(!wide.contains(hidden), "FOOT_HIDDEN: {wide}");
    }
    let narrower = statusline_text(&recorded(), &orders, 80, NOW);
    assert_eq!(
        narrower,
        "Claude 5h 41% · Gemini 5h 2% · orders: sub haiku running, mul flash delivered"
    );
    let narrow = statusline_text(&recorded(), &orders, 40, NOW);
    assert_eq!(narrow, "Claude 5h 41% · Gemini 5h 2% · 2 orders");
    assert!(
        statusline_text(&recorded(), &orders, 10, NOW)
            .chars()
            .count()
            <= 10
    );
    assert_eq!(
        statusline_text(&recorded(), &[], 200, NOW),
        "Claude 5h 41% 3h · Gemini 5h 2% 4h"
    );
}

#[test]
fn the_statusline_takes_claudes_live_windows_over_the_cache() {
    let input = json!({"rate_limits": {
        "five_hour": {"used_percentage": 62.0, "resets_at": NOW + 3600},
        "seven_day": {"used_percentage": 41.2, "resets_at": NOW + 2 * 86_400},
    }});
    let live = statusline_readings(&input, NOW);
    assert_eq!(live.len(), 2);
    let sources = with_live_max(recorded(), &live);
    let line = statusline_text(&sources, &[], 200, NOW);
    assert!(line.starts_with("Claude 5h 62% 1h"), "{line}");
    // The Fable week, which the statusline does not carry, stays as read.
    assert!(sources[0].readings.iter().any(|r| r.window == "week.fable"));
    assert!(statusline_readings(&json!({"model": {}}), NOW).is_empty());
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nirvana-o4b-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".git")).expect("a scratch repo");
    dir
}

#[test]
fn the_board_line_is_written_once_beside_the_lessons_lines() {
    let home = scratch("home");
    let root = scratch("proj");
    let path = write_board_line(&home, &root).expect("written");
    let first = std::fs::read_to_string(&path).unwrap();
    assert!(first.starts_with("# Nirvana memory"), "{first}");
    assert_eq!(first.matches("herdr-board").count(), 1);
    // O5's lessons line and the owner's own text stay as they are.
    let other = "- lessons for X: 2 live <!-- herdr-lessons X -->\nmine\n";
    std::fs::write(&path, format!("{first}{other}")).unwrap();
    write_board_line(&home, &root).unwrap();
    let second = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        second,
        format!("{first}{other}"),
        "an unchanged line is not rewritten"
    );
    assert!(second.contains(
        &crate::nirvana_board::board_path(&root)
            .display()
            .to_string()
    ));
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn only_open_proposals_are_named() {
    let root = scratch("proposals");
    let dir = root.join(".nirvana").join("proposals");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("p-1.md"),
        "# Proposal p-1: stall on flash\n\nstatus: open\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("p-2.md"),
        "# Proposal p-2: old gap\n\nstatus: rejected\n",
    )
    .unwrap();
    std::fs::write(dir.join("notes.txt"), "status: open\n").unwrap();
    assert_eq!(
        open_proposals(&root),
        vec![("p-1".to_owned(), "stall on flash".to_owned())]
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_plugins_copy_of_a_hook_stays_quiet_when_the_folder_runs_it() {
    let root = scratch("folder-hooks");
    let input = json!({"cwd": root.display().to_string()}).to_string();
    let files = settings_files(&root, None);
    assert_eq!(files[0], root.join(".claude").join("settings.local.json"));
    assert!(!folder_runs_with(&input, "Stop", "jev compact", None));
    std::fs::create_dir_all(root.join(".claude")).unwrap();
    std::fs::write(
        root.join(".claude").join("settings.local.json"),
        json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "\"C:\\h\\herdr.exe\" jev compact"}]}]}})
            .to_string(),
    )
    .unwrap();
    assert!(folder_runs_with(&input, "Stop", "jev compact", None));
    assert!(!folder_runs_with(
        &input,
        "UserPromptSubmit",
        "board notice",
        None
    ));
    let _ = std::fs::remove_dir_all(&root);
}

/// O3's wake types a digest only into a pane whose `claude` argv is Nirvana Claude's (`nirvana_run::pane_program`):
/// the launch O4b changed must still be recognised, and plain Claude Code must still not be.
#[test]
fn the_wake_guard_knows_this_launch_and_not_plain_claude() {
    let home = Path::new(r"C:\Users\o\AppData\Local\Programs\Nirvana-Herdr");
    let pane = json!({"result": {"pane": {"agent": "claude"}}});
    let process = |args: Vec<String>| {
        let mut argv = vec![r"C:\Users\o\.local\bin\claude.exe".to_owned()];
        argv.extend(args);
        json!({"result": {"process_info": {"foreground_processes": [{"argv": argv}]}}})
    };
    let plugin = crate::nirvana_launch::plugin_dir(home);
    let nirvana = process(crate::nirvana_launch::NewKind::Nirvana.args(home));
    assert_eq!(
        nirvana_run::pane_program(&pane, &nirvana, Some(&plugin)),
        nirvana_run::PaneProgram::NirvanaClaude
    );
    let plain = process(crate::nirvana_launch::NewKind::ClaudeCode.args(home));
    assert_ne!(
        nirvana_run::pane_program(&pane, &plain, Some(&plugin)),
        nirvana_run::PaneProgram::NirvanaClaude
    );
}
