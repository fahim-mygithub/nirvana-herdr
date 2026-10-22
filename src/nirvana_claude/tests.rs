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
    // The runs and proposals are cleared, so the line has room and its absence is the rule's, not the budget's.
    let mut digest = full_digest();
    digest.runs.clear();
    digest.proposals.clear();
    digest.sources[2].readings = vec![
        reading("google3p", "5h", 0.96, 3600, 30),
        reading("google3p", "week", 0.2, 86_400, 30),
    ];
    let text = digest_text(&digest);
    assert!(!text.contains("Not in the foot"), "{text}");
    // With Argon read again, the line comes back naming only Argon.
    digest.sources[2]
        .readings
        .push(reading("argon", "week", 0.0101, 6 * 86_400, 30));
    let text = digest_text(&digest);
    let hidden = text
        .lines()
        .find(|l| l.starts_with("Not in the foot"))
        .expect("the line");
    assert!(
        hidden.contains("Argon") && !hidden.contains("G\u{b7}Claude"),
        "{hidden}"
    );
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
    let _guard = crate::nirvana::with_test_home("no_model_or_effort");
    let nirvana_dir = _guard.path.join(nirvana_seat::DIR);
    std::fs::create_dir_all(&nirvana_dir).unwrap();
    let leases = json!({
        "version": 1,
        "leases": [
            {
                "order": "r1",
                "seat": "sonnet",
                "effort": "medium",
                "pool": "max",
                "class": "review.adversary",
                "use": {},
                "booked_at": NOW,
                "expires_at": NOW + 3600
            },
            {
                "order": "r2",
                "seat": "haiku",
                "effort": "low",
                "pool": "fable",
                "class": "review.adversary",
                "use": {},
                "booked_at": NOW,
                "expires_at": NOW + 3600
            }
        ]
    });
    std::fs::write(
        nirvana_dir.join(nirvana_seat::LEASES_FILE),
        serde_json::to_string(&leases).unwrap(),
    )
    .unwrap();

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
    let mut explore = spawn("Explore", Some("haiku"), Some("low"));
    explore.prompt = "Class: review.adversary\nOrder: r2\nRead the diff.".into();
    assert_eq!(guard(&explore, None, &table, &room(0.2)), Verdict::Allow);
}

#[test]
fn a_spawn_on_max_that_would_break_the_reserve_is_refused() {
    let _guard = crate::nirvana::with_test_home("reserve_break");
    let nirvana_dir = _guard.path.join(nirvana_seat::DIR);
    std::fs::create_dir_all(&nirvana_dir).unwrap();
    let leases = json!({
        "version": 1,
        "leases": [
            {
                "order": "r1",
                "seat": "opus",
                "effort": "high",
                "pool": "max",
                "class": "review.adversary",
                "use": {},
                "booked_at": NOW,
                "expires_at": NOW + 3600
            }
        ]
    });
    std::fs::write(
        nirvana_dir.join(nirvana_seat::LEASES_FILE),
        serde_json::to_string(&leases).unwrap(),
    )
    .unwrap();

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
fn guard_spawn_booking_rules_and_refusal_messages() {
    let _guard = crate::nirvana::with_test_home("guard_booking");
    let nirvana_dir = _guard.path.join(nirvana_seat::DIR);
    std::fs::create_dir_all(&nirvana_dir).unwrap();
    let table = table();

    let leases = json!({
        "version": 1,
        "leases": [
            {
                "order": "o-live-sonnet",
                "seat": "sonnet",
                "effort": "medium",
                "pool": "max",
                "class": "build.bounded",
                "use": {},
                "booked_at": NOW - 100,
                "expires_at": NOW + 3600
            },
            {
                "order": "o-expired",
                "seat": "sonnet",
                "effort": "medium",
                "pool": "max",
                "class": "build.bounded",
                "use": {},
                "booked_at": NOW - 1000,
                "expires_at": NOW - 10
            },
            {
                "order": "o-flash",
                "seat": "flash",
                "effort": "medium",
                "pool": "gemini",
                "class": "research.web",
                "use": {},
                "booked_at": NOW - 100,
                "expires_at": NOW + 3600
            }
        ]
    });
    std::fs::write(
        nirvana_dir.join(nirvana_seat::LEASES_FILE),
        serde_json::to_string(&leases).unwrap(),
    )
    .unwrap();

    let def_sonnet_med = worker_def("claude-sonnet-5-5", "medium");
    let def_sonnet_high = worker_def("claude-sonnet-5-5", "high");
    let def_opus_med = worker_def("claude-opus-5-5", "medium");

    // 1. No Order: line -> refusal says "seat this first: herdr seat pick ..."
    let mut no_order = spawn("general-purpose", Some("sonnet"), Some("medium"));
    no_order.prompt = "Class: build.bounded\nNo order id here.\nRun: r-1".into();
    match guard(&no_order, None, &table, &room(0.2)) {
        Verdict::Deny(why) => {
            assert!(
                why.contains("herdr guard: seat this first: herdr seat pick --class build.bounded --size small --book <order> --run r-1"),
                "no order line: {why}"
            );
        }
        Verdict::Allow => panic!("expected deny for no order line"),
    }

    // 2. An order with no lease
    let mut no_lease = spawn("general-purpose", Some("sonnet"), Some("medium"));
    no_lease.prompt = "Class: build.bounded\nOrder: o-missing\nRun: r-1".into();
    match guard(&no_lease, None, &table, &room(0.2)) {
        Verdict::Deny(why) => {
            assert!(
                why.contains("herdr guard: seat this first: herdr seat pick --class build.bounded --size small --book o-missing --run r-1"),
                "order with no lease: {why}"
            );
        }
        Verdict::Allow => panic!("expected deny for unbooked order"),
    }

    // 3. An expired lease
    let mut expired = spawn("general-purpose", Some("sonnet"), Some("medium"));
    expired.prompt = "Class: build.bounded\nOrder: o-expired\nRun: r-1".into();
    match guard(&expired, None, &table, &room(0.2)) {
        Verdict::Deny(why) => {
            assert!(
                why.contains("herdr guard: seat this first: herdr seat pick --class build.bounded --size small --book o-expired --run r-1"),
                "expired lease: {why}"
            );
        }
        Verdict::Allow => panic!("expected deny for expired lease"),
    }

    // 4. A lease on flash (agy seat)
    let mut agy_spawn = spawn("general-purpose", Some("sonnet"), Some("medium"));
    agy_spawn.prompt = "Class: research.web\nOrder: o-flash\nRun: r-1".into();
    match guard(&agy_spawn, None, &table, &room(0.2)) {
        Verdict::Deny(why) => {
            assert!(
                why.contains("herdr guard: the picker seated o-flash on flash medium (gemini). Start it with herdr seat run flash --order ..."),
                "lease on flash: {why}"
            );
        }
        Verdict::Allow => panic!("expected deny for agy lease"),
    }

    // 5. A lease on sonnet medium with an opus spawn
    let mut opus_spawn = spawn("nirvana:worker-opus-medium", None, None);
    opus_spawn.prompt = "Class: build.bounded\nOrder: o-live-sonnet\nRun: r-1".into();
    match guard(&opus_spawn, Some(&def_opus_med), &table, &room(0.2)) {
        Verdict::Deny(why) => {
            assert!(
                why.contains("herdr guard: o-live-sonnet is booked on sonnet medium, but this spawn is on opus medium. A spawn must match its booking: spawn nirvana:worker-sonnet-medium, or seat with herdr seat pick --override opus --book o-live-sonnet"),
                "opus spawn for sonnet lease: {why}"
            );
        }
        Verdict::Allow => panic!("expected deny for opus spawn on sonnet lease"),
    }

    // 6. A lease on sonnet medium with a sonnet high spawn
    let mut sonnet_high_spawn = spawn("nirvana:worker-sonnet-high", None, None);
    sonnet_high_spawn.prompt = "Class: build.bounded\nOrder: o-live-sonnet\nRun: r-1".into();
    match guard(
        &sonnet_high_spawn,
        Some(&def_sonnet_high),
        &table,
        &room(0.2),
    ) {
        Verdict::Deny(why) => {
            assert!(
                why.contains("herdr guard: o-live-sonnet is booked on sonnet medium, but this spawn is on sonnet high. A spawn must match its booking: spawn nirvana:worker-sonnet-medium, or seat with herdr seat pick --override sonnet --book o-live-sonnet"),
                "sonnet high spawn for sonnet medium lease: {why}"
            );
        }
        Verdict::Allow => panic!("expected deny for high effort spawn on medium lease"),
    }

    // 7. A lease on sonnet medium with a sonnet medium spawn -> ALLOWED!
    let mut sonnet_med_spawn = spawn("nirvana:worker-sonnet-medium", None, None);
    sonnet_med_spawn.prompt = "Class: build.bounded\nOrder: o-live-sonnet\nRun: r-1".into();
    assert_eq!(
        guard(&sonnet_med_spawn, Some(&def_sonnet_med), &table, &room(0.2)),
        Verdict::Allow
    );
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
        let desc = head
            .lines()
            .find_map(|line| line.strip_prefix("description:"))
            .unwrap_or_else(|| panic!("{stem}: lists no description"));
        assert!(
            desc.contains("booking comes first"),
            "{stem}: description does not say booking comes first: {desc}"
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

// A real worker's transcript, trimmed (live 2026-10-18, scenario M9): 8 responses, the fifth written as five lines
// whose output counts read 3, 3, 3, 3, 2325, and a last response that is a `SubagentHandback` call with prose.
const HANDBACK: &str = include_str!("fixtures/subagent_handback.jsonl");

#[test]
fn a_response_written_as_many_lines_counts_its_last_usage() {
    // By hand, the last line of each message id: input 2 x 8 = 16; cache writes 31934 + 4701 + 2625 + 2637 + 1866
    // + 2767 + 576 + 1765 = 48871; cache reads 0 + 31934 + 36635 + 39260 + 41897 + 43763 + 46530 + 47106 = 287125;
    // output 374 + 475 + 320 + 5 + 2325 + 8 + 8 + 26 = 3541. Keeping the first line of each id gave output 98.
    let (brief, tokens) = read_transcript(HANDBACK);
    assert!(brief.starts_with("Class: build.bounded"));
    assert_eq!(
        tokens,
        Tokens {
            input: 16,
            cache_write: 48871,
            cache_read: 287125,
            output: 3541
        }
    );
}

#[test]
fn a_handback_report_is_read_from_the_call_and_prose_is_its_own_outcome() {
    let table = table();
    let hook = json!({"agent_type": "nirvana:worker-sonnet-medium", "agent_id": "a663", "last_assistant_message": ""});
    let message = handback_message(HANDBACK).expect("the fixture ends in a handback");
    assert!(message.starts_with("I added `/api/health`"));

    // Prose in the handback: reported, but not as a Work Report.
    let line = subagent_line(&table, &hook, HANDBACK).unwrap().unwrap();
    assert_eq!(line.outcome, "subagent-prose-report");
    let summary = line.extra["summary"].as_str().unwrap();
    assert_eq!(summary.chars().count(), 200);
    assert!(message.starts_with(summary));

    // The same call carrying the fenced json block is a report.
    let handed = |message: &str| {
        [
            json!({"type": "user", "message": {"role": "user", "content": "Class: review.adversary\nOrder: rev-1\nRun: live1"}}),
            json!({"type": "assistant", "message": {"id": "msg_1", "content": [{"type": "tool_use", "name": "SubagentHandback", "input": {"message": message}}], "usage": {"output_tokens": 5}}}),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
    };
    let line = subagent_line(&table, &hook, &handed(REPORT))
        .unwrap()
        .unwrap();
    assert_eq!(line.outcome, "subagent-done");
    assert!(line.extra.get("summary").is_none());

    // A final text that is a report still wins, and no handback and no report is still no-report.
    let hook_text = json!({"agent_type": "nirvana:worker-sonnet-medium", "agent_id": "a663", "last_assistant_message": REPORT});
    assert_eq!(
        subagent_line(&table, &hook_text, HANDBACK)
            .unwrap()
            .unwrap()
            .outcome,
        "subagent-done"
    );
    let line = subagent_line(&table, &hook, &transcript())
        .unwrap()
        .unwrap();
    assert_eq!(line.outcome, "subagent-no-report");
    let line = subagent_line(&table, &hook, &handed("  "))
        .unwrap()
        .unwrap();
    assert_eq!(line.outcome, "subagent-no-report");
}

const WORK_REPORT_LINE: &str = "The Work Report is the fenced `json` block inside your final message itself, whether you end with a text reply or a SubagentHandback call; prose in the message alone is not a report.";

#[test]
fn every_worker_agent_says_the_work_report_is_in_the_final_message() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("nirvana/claude/agents");
    for name in [
        "haiku-low",
        "opus-high",
        "opus-medium",
        "sonnet-low",
        "sonnet-medium",
    ] {
        let text = std::fs::read_to_string(dir.join(format!("worker-{name}.md"))).unwrap();
        assert!(text.contains(WORK_REPORT_LINE), "worker-{name}");
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

/// The two writers of `NIRVANA.md`, this board line and O5's lessons line, each replace only their own line: written
/// in turn, for two projects, every line stays once and the owner's own text stays.
#[test]
fn the_board_and_lessons_lines_survive_each_others_write() {
    let home = scratch("md-both");
    let a = scratch("md-a");
    let b = scratch("md-b");
    let path = crate::nirvana_lessons::write_index_line(&home, &a).expect("lessons first");
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("# Nirvana memory"),
        "a new file gets the head whichever writer is first"
    );
    write_board_line(&home, &a).expect("board a");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("the owner's own line\n");
    std::fs::write(&path, &text).unwrap();
    write_board_line(&home, &b).expect("board b");
    crate::nirvana_lessons::write_index_line(&home, &b).expect("lessons b");
    crate::nirvana_lessons::write_index_line(&home, &a).expect("lessons a again");
    write_board_line(&home, &a).expect("board a again");
    let text = std::fs::read_to_string(&path).unwrap();
    for (mark, root) in [
        ("<!-- herdr-board ", &a),
        ("<!-- herdr-board ", &b),
        ("<!-- herdr-lessons ", &a),
        ("<!-- herdr-lessons ", &b),
    ] {
        let tail = format!("{mark}{} -->", root.display());
        assert_eq!(
            text.lines().filter(|line| line.ends_with(&tail)).count(),
            1,
            "{tail} once in:\n{text}"
        );
    }
    assert_eq!(text.matches("the owner's own line").count(), 1, "{text}");
    assert_eq!(text.matches("# Nirvana memory").count(), 1, "{text}");
    for dir in [&home, &a, &b] {
        let _ = std::fs::remove_dir_all(dir);
    }
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
    // The digest's call since O5 merged: `nirvana_retro::open`, mapped to (id, gap).
    let named: Vec<(String, String)> = crate::nirvana_retro::open(&nirvana_run::Project::at(&root))
        .into_iter()
        .map(|(id, gap, _)| (id, gap))
        .collect();
    assert_eq!(named, vec![("p-1".to_owned(), "stall on flash".to_owned())]);
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

#[test]
fn seat_notice_reports_once_per_change_and_stays_quiet_when_same() {
    // 1. Same seat: stays quiet
    let (notice, notified) = seat_notice("opus", "medium", "opus", "medium", "Balanced", None);
    assert_eq!(notice, None);
    assert_eq!(notified, None);

    // 2. Different seat: speaks once
    let (notice, notified) = seat_notice(
        "opus",
        "medium",
        "sonnet",
        "medium",
        "Balanced, max week climb 1.18",
        None,
    );
    assert_eq!(
        notice.as_deref(),
        Some(
            "Nirvana Claude runs on opus medium; the mode now seats sonnet medium (Balanced, max week climb 1.18). Press Restart Claude to switch."
        )
    );
    assert_eq!(notified, Some(("sonnet".to_string(), "medium".to_string())));

    // 3. Same difference again: stays quiet
    let (notice_again, notified_again) = seat_notice(
        "opus",
        "medium",
        "sonnet",
        "medium",
        "Balanced, max week climb 1.18",
        Some(("sonnet", "medium")),
    );
    assert_eq!(notice_again, None);
    assert_eq!(
        notified_again,
        Some(("sonnet".to_string(), "medium".to_string()))
    );

    // 4. A second change: speaks again with new target
    let (notice_second, notified_second) = seat_notice(
        "opus",
        "medium",
        "opus",
        "high",
        "SOTA",
        Some(("sonnet", "medium")),
    );
    assert_eq!(
        notice_second.as_deref(),
        Some("Nirvana Claude runs on opus medium; the mode now seats opus high (SOTA). Press Restart Claude to switch.")
    );
    assert_eq!(
        notified_second,
        Some(("opus".to_string(), "high".to_string()))
    );
}

#[test]
fn session_start_records_running_seat_in_session_file() {
    let home = scratch("session_seat");
    let session_id = "test-session-123";
    let path =
        record_session_seat(&home, session_id, "claude-sonnet-5-5", "medium").expect("recorded");
    assert!(path.exists());
    assert_eq!(path, session_seat_path(&home, session_id));

    let record = read_session_seat(&home, session_id).expect("read");
    assert_eq!(record.session_id, session_id);
    assert_eq!(record.model, "claude-sonnet-5-5");
    assert_eq!(record.seat, "sonnet");
    assert_eq!(record.effort, "medium");
    assert_eq!(record.last_notified_seat, None);
    assert_eq!(record.last_notified_effort, None);

    let _ = std::fs::remove_dir_all(&home);
}

// ---- Plan 24 §2: log-usage tests ----

#[test]
fn log_usage_fixture_transcript_deduplicates_repeated_message_id_across_calls() {
    let _guard = crate::nirvana::with_test_home("log-usage-fixture");
    let proj = scratch("proj-usage");
    let project = Project::at(&proj);
    let run = "r1";
    let journal = project.journal(run);

    // Open a run
    journal
        .append(
            1,
            crate::nirvana_run::Kind::Opened {
                project: "p".into(),
                orchestrator_pane: None,
                auto: false,
                pools: vec![],
                pools_basis: None,
            },
        )
        .expect("opened");

    let transcript_path = proj.join("transcript.jsonl");
    // 3 assistant lines, msg1 repeated twice with usage, msg2 once
    let line1 = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-1",
            "usage": {
                "input_tokens": 100,
                "cache_creation_input_tokens": 10,
                "cache_read_input_tokens": 5,
                "output_tokens": 50
            }
        }
    });
    let line2 = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-1",
            "usage": {
                "input_tokens": 100,
                "cache_creation_input_tokens": 10,
                "cache_read_input_tokens": 5,
                "output_tokens": 50
            }
        }
    });
    let line3 = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-2",
            "usage": {
                "input_tokens": 200,
                "cache_creation_input_tokens": 20,
                "cache_read_input_tokens": 10,
                "output_tokens": 80
            }
        }
    });
    let content = format!("{line1}\n{line2}\n{line3}\n");
    std::fs::write(&transcript_path, &content).expect("transcript");

    // Read usage helper
    let (tokens, offset, ids) = read_transcript_usage(&transcript_path, 0, &[]).expect("read");
    // De-duplicated sum: msg1 (100 in, 10 cw, 5 cr, 50 out) + msg2 (200 in, 20 cw, 10 cr, 80 out)
    assert_eq!(tokens.input, 300);
    assert_eq!(tokens.cache_write, 30);
    assert_eq!(tokens.cache_read, 15);
    assert_eq!(tokens.output, 130);

    // Second call from offset: 0 new tokens
    let (tokens2, offset2, _) =
        read_transcript_usage(&transcript_path, offset, &ids).expect("read 2");
    assert_eq!(tokens2, Tokens::default());
    assert_eq!(offset2, offset);

    // Append to journal via transact
    let kind = crate::nirvana_run::Kind::Usage {
        order: None,
        attempt: 0,
        session: Some("sess-1".into()),
        tokens,
    };
    journal.append(2, kind).expect("usage event");

    // Verify receipt shows the tokens
    let state = crate::nirvana_run::replay(&journal.read().0);
    assert_eq!(state.orchestrator, Some(tokens));
    let receipt = crate::nirvana_run::receipt(run, &state, 0, &[]);
    let json = serde_json::to_value(&receipt).expect("json");
    assert_eq!(json["orchestrator"]["output"], 130);
    assert_eq!(json["orchestrator"]["input"], 300);
}

#[test]
fn log_usage_skips_when_agent_id_or_no_open_run() {
    let _guard = crate::nirvana::with_test_home("log-usage-skips");
    let proj = scratch("proj-skips");
    let project = Project::at(&proj);

    // With no open run, transcript usage reading still advances offset but nothing is appended
    let transcript_path = proj.join("t.jsonl");
    let line = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-1",
            "usage": {"input_tokens": 50, "output_tokens": 20}
        }
    });
    std::fs::write(&transcript_path, format!("{line}\n")).expect("t");

    let runs = open_runs(&project);
    assert!(runs.is_empty());
    // Verify open_runs is empty, so no journal is transacted
}

// ---- Plan 24 §3: own-edit limit tests ----

#[test]
fn edit_budget_counts_files_and_lines_within_and_past_limit() {
    let limit = EditLimit::default(); // 1 file, 30 lines
    let baseline = EditState::default();

    // 1 file and 5 lines passes
    let mut now1 = EditState::default();
    now1.files.insert(
        "src/lib.rs".into(),
        FileDelta {
            added: 5,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    assert_eq!(edit_budget(&baseline, &now1, limit), None);

    // 3 files fail
    let mut now3 = EditState::default();
    now3.files.insert(
        "src/a.rs".into(),
        FileDelta {
            added: 1,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    now3.files.insert(
        "src/b.rs".into(),
        FileDelta {
            added: 1,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    now3.files.insert(
        "src/c.rs".into(),
        FileDelta {
            added: 1,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    let reason3 = edit_budget(&baseline, &now3, limit).expect("3 files fails");
    assert!(reason3.contains("Nirvana Claude edits at most 1 file and 30 lines itself (R2)"));
    assert!(reason3.contains("This change is 3 files, 3 lines"));

    // 1 file and 31 lines fails
    let mut now_31 = EditState::default();
    now_31.files.insert(
        "src/lib.rs".into(),
        FileDelta {
            added: 31,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    let reason_31 = edit_budget(&baseline, &now_31, limit).expect("31 lines fails");
    assert!(reason_31.contains("Nirvana Claude edits at most 1 file and 30 lines itself (R2)"));
    assert!(reason_31.contains("This change is 1 file, 31 lines"));
}

#[test]
fn edit_budget_out_of_scope_paths_and_baseline_with_existing_changes() {
    let limit = EditLimit::default();

    // Out of scope paths (.nirvana/, .claude/, memory) do not count
    let baseline = EditState::default();
    let mut now = EditState::default();
    now.files.insert(
        ".nirvana/state.json".into(),
        FileDelta {
            added: 50,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    now.files.insert(
        ".claude/settings.json".into(),
        FileDelta {
            added: 40,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    now.files.insert(
        "NIRVANA.md".into(),
        FileDelta {
            added: 20,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    // Only in-scope change: 1 file, 10 lines
    now.files.insert(
        "src/main.rs".into(),
        FileDelta {
            added: 10,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    assert_eq!(edit_budget(&baseline, &now, limit), None);

    // Baseline with changes already in it counts only the new ones
    let mut baseline_with_changes = EditState::default();
    baseline_with_changes.files.insert(
        "src/main.rs".into(),
        FileDelta {
            added: 100,
            deleted: 50,
            hash: None,
            ..Default::default()
        },
    );
    baseline_with_changes.files.insert(
        "src/existing.rs".into(),
        FileDelta {
            added: 20,
            deleted: 10,
            hash: None,
            ..Default::default()
        },
    );

    // If now has same changes + 1 file with 5 new lines: passes!
    let mut now_incremental = baseline_with_changes.clone();
    now_incremental.files.insert(
        "src/new.rs".into(),
        FileDelta {
            added: 5,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    assert_eq!(
        edit_budget(&baseline_with_changes, &now_incremental, limit),
        None
    );

    // If now modifies existing.rs by 31 new lines: fails
    let mut now_exceeded = baseline_with_changes.clone();
    now_exceeded.files.insert(
        "src/existing.rs".into(),
        FileDelta {
            added: 51,
            deleted: 10,
            hash: None,
            ..Default::default()
        },
    );
    let reason = edit_budget(&baseline_with_changes, &now_exceeded, limit).expect("fails");
    assert!(reason.contains("This change is 1 file, 31 lines"));
}

#[test]
fn guard_edit_and_check_edit_never_block_subagent_or_worker() {
    let _guard = crate::nirvana::with_test_home("never-block-subagent");

    let subagent_hook = serde_json::json!({
        "agent_id": "worker-1",
        "tool_name": "Write",
        "tool_input": {
            "file_path": "src/main.rs",
            "content": "line\n".repeat(100)
        }
    });
    assert!(should_skip_orchestrator_hook(&subagent_hook, None, true));
    assert_eq!(
        guard_edit_decision(&subagent_hook, None, None, EditLimit::default(), true),
        None
    );

    let baseline = EditState::default();
    let mut now = EditState::default();
    now.files.insert(
        "src/main.rs".into(),
        FileDelta {
            added: 100,
            deleted: 0,
            hash: None,
            ..Default::default()
        },
    );
    assert_eq!(
        check_edit_decision(
            &subagent_hook,
            None,
            &baseline,
            &now,
            EditLimit::default(),
            true
        ),
        None
    );

    // 2. NIRVANA_ORCHESTRATOR != 1 is skipped (is_orchestrator: false)
    let orch_hook = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": "src/main.rs",
            "content": "line\n".repeat(100)
        }
    });
    assert!(should_skip_orchestrator_hook(&orch_hook, None, false));
    assert_eq!(
        guard_edit_decision(&orch_hook, None, None, EditLimit::default(), false),
        None
    );
    assert_eq!(
        check_edit_decision(
            &orch_hook,
            None,
            &baseline,
            &now,
            EditLimit::default(),
            false
        ),
        None
    );

    // 3. Worktree path is skipped
    let worktree_cwd = PathBuf::from("repo/.nirvana/worktrees/build-1");
    assert!(should_skip_orchestrator_hook(
        &orch_hook,
        Some(&worktree_cwd),
        true
    ));
    assert_eq!(
        guard_edit_decision(
            &orch_hook,
            None,
            Some(&worktree_cwd),
            EditLimit::default(),
            true
        ),
        None
    );
    assert_eq!(
        check_edit_decision(
            &orch_hook,
            Some(&worktree_cwd),
            &baseline,
            &now,
            EditLimit::default(),
            true
        ),
        None
    );

    // 4. In orchestrator with no agent_id and outside worktree, >30 lines blocks
    let normal_cwd = scratch("normal-repo");
    let blocked = guard_edit_decision(
        &orch_hook,
        Some(&normal_cwd),
        Some(&normal_cwd),
        EditLimit::default(),
        true,
    );
    assert!(blocked.is_some());
    let blocked_check = check_edit_decision(
        &orch_hook,
        Some(&normal_cwd),
        &baseline,
        &now,
        EditLimit::default(),
        true,
    );
    assert!(blocked_check.is_some());
}

#[test]
fn missing_baseline_never_blocks_and_writes_baseline_and_sessions_keep_separate_baselines() {
    let _guard = crate::nirvana::with_test_home("b1-missing-baseline");
    let proj = scratch("b1-proj");
    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&proj)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Tester"])
        .current_dir(&proj)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "tester@test.com"])
        .current_dir(&proj)
        .output();
    std::fs::write(proj.join("tracked.txt"), "init\n").expect("write");
    let _ = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&proj)
        .output();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&proj)
        .output();

    // Dirty tree: 50 lines in tracked.txt
    std::fs::write(proj.join("tracked.txt"), "line\n".repeat(50)).expect("dirty");

    let hook_s1 = serde_json::json!({
        "session_id": "session-1",
        "cwd": proj.display().to_string(),
        "tool_name": "Edit",
    });

    let dir = home_dir().join(nirvana_seat::DIR).join("edit-baselines");
    let path_s1 = dir.join("session-1.json");
    assert!(!path_s1.exists());

    // When check-edit finds no baseline for session-1, it writes one and returns without blocking!
    let dec1 = check_edit_from(&hook_s1, true).expect("check_edit_from");
    assert_eq!(dec1, None);
    assert!(path_s1.exists());

    // Two sessions keep separate baselines: session-2 writes its own file
    let hook_s2 = serde_json::json!({
        "session_id": "session-2",
        "cwd": proj.display().to_string(),
        "tool_name": "Edit",
    });
    let path_s2 = dir.join("session-2.json");
    assert!(!path_s2.exists());
    let dec2 = check_edit_from(&hook_s2, true).expect("check_edit_from s2");
    assert_eq!(dec2, None);
    assert!(path_s2.exists());
}

#[test]
fn already_dirty_file_same_size_replacement_and_new_untracked_file_rewritten_fails() {
    let limit = EditLimit::default(); // 1 file, 30 lines

    // 1. An already-dirty file edited with a same-size replacement of 40 lines
    let mut baseline = EditState::default();
    let base_lines_40 = (0..40)
        .map(|i| format!("old line {i}\n"))
        .collect::<String>();
    baseline.files.insert(
        "src/dirty.rs".into(),
        FileDelta {
            is_tracked: true,
            added: 40,
            deleted: 0,
            line_count: 40,
            hash: Some("old_hash".into()),
            content: Some(base_lines_40),
            ..Default::default()
        },
    );

    let new_lines_40 = (0..40)
        .map(|i| format!("new line {i}\n"))
        .collect::<String>();
    let mut now_same_size = EditState::default();
    now_same_size.files.insert(
        "src/dirty.rs".into(),
        FileDelta {
            is_tracked: true,
            added: 40,
            deleted: 0,
            line_count: 40,
            hash: Some("new_hash".into()),
            content: Some(new_lines_40),
            ..Default::default()
        },
    );

    let reason1 = edit_budget(&baseline, &now_same_size, limit).expect("fails 30-line limit");
    assert!(reason1.contains("Nirvana Claude edits at most 1 file and 30 lines itself (R2)"));
    assert!(reason1.contains("1 file, 80 lines"));

    // 2. A new untracked file rewritten from 100 lines to 100 different lines fails
    let mut baseline_untracked = EditState::default();
    let base_untracked_100 = (0..100)
        .map(|i| format!("untracked old {i}\n"))
        .collect::<String>();
    baseline_untracked.files.insert(
        "untracked.txt".into(),
        FileDelta {
            is_tracked: false,
            added: 100,
            deleted: 0,
            line_count: 100,
            hash: Some("u_old".into()),
            content: Some(base_untracked_100),
            ..Default::default()
        },
    );

    let mut now_untracked_rewritten = EditState::default();
    let new_untracked_100 = (0..100)
        .map(|i| format!("untracked new {i}\n"))
        .collect::<String>();
    now_untracked_rewritten.files.insert(
        "untracked.txt".into(),
        FileDelta {
            is_tracked: false,
            added: 100,
            deleted: 0,
            line_count: 100,
            hash: Some("u_new".into()),
            content: Some(new_untracked_100),
            ..Default::default()
        },
    );

    let reason2 = edit_budget(&baseline_untracked, &now_untracked_rewritten, limit).expect("fails");
    assert!(reason2.contains("1 file, 200 lines"));
}

#[test]
fn baseline_dirty_file_that_is_clean_now_contributes_nothing() {
    let limit = EditLimit::default();
    let mut baseline = EditState::default();
    baseline.files.insert(
        "src/reverted.rs".into(),
        FileDelta {
            is_tracked: true,
            added: 100,
            deleted: 50,
            line_count: 100,
            hash: Some("h1".into()),
            content: Some("dirty content\n".into()),
            ..Default::default()
        },
    );
    // Now it is clean (not present in now)
    let now_clean = EditState::default();
    assert_eq!(edit_budget(&baseline, &now_clean, limit), None);
}

#[test]
fn relative_paths_subfolder_project_root_sees_untracked_and_ignores_outside() {
    let top = scratch("s3-repo");
    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&top)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Tester"])
        .current_dir(&top)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "tester@test.com"])
        .current_dir(&top)
        .output();

    let sub = top.join("subproject");
    std::fs::create_dir_all(&sub).expect("create sub");

    // Commit a file inside sub and outside sub
    std::fs::write(top.join("outside.txt"), "outside init\n").expect("write");
    std::fs::write(sub.join("inside.txt"), "inside init\n").expect("write");
    let _ = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&top)
        .output();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&top)
        .output();

    // Now change outside.txt and add untracked outside, and change inside.txt
    std::fs::write(top.join("outside.txt"), "outside modified\n").expect("write");
    std::fs::write(top.join("outside_untracked.txt"), "outside un\n").expect("write");
    std::fs::write(sub.join("inside.txt"), "inside modified\n").expect("write");

    // In subfolder: add an untracked file inside
    std::fs::write(sub.join("sub_untracked.txt"), "sub un 1\nsub un 2\n").expect("write");

    // capture_edit_state from subproject root
    let state = capture_edit_state(&sub);
    assert!(
        state.files.contains_key("inside.txt"),
        "subproject tracked file must be relative to subproject"
    );
    assert!(state.files.contains_key("sub_untracked.txt"));
    assert!(!state.files.contains_key("outside.txt"));
    assert!(!state.files.contains_key("outside_untracked.txt"));
}

#[test]
fn numstat_dash_binary_line_parses_and_counts_as_one_file_zero_lines() {
    let limit = EditLimit::default();
    let baseline = EditState::default();

    let mut now = EditState::default();
    now.files.insert(
        "image.png".into(),
        FileDelta {
            is_tracked: true,
            is_binary: true,
            added: 0,
            deleted: 0,
            line_count: 0,
            hash: Some("binhash".into()),
            content: None,
            ..Default::default()
        },
    );

    // Binary file counts as 1 file and 0 lines, which passes the limit of 1 file, 30 lines
    assert_eq!(edit_budget(&baseline, &now, limit), None);

    // If 2 binary files are added, it fails the 1 file limit
    now.files.insert(
        "audio.mp3".into(),
        FileDelta {
            is_tracked: true,
            is_binary: true,
            added: 0,
            deleted: 0,
            line_count: 0,
            hash: Some("audiohash".into()),
            content: None,
            ..Default::default()
        },
    );
    let reason = edit_budget(&baseline, &now, limit).expect("fails 1 file limit");
    assert!(reason.contains("This change is 2 files, 0 lines"));
}

#[test]
fn usage_offsets_partial_line_split_id_and_sidechain() {
    let dir = scratch("usage-s5");
    let transcript = dir.join("transcript.jsonl");

    let line1 = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-split",
            "usage": {
                "input_tokens": 100,
                "cache_creation_input_tokens": 10,
                "cache_read_input_tokens": 5,
                "output_tokens": 50
            }
        }
    });

    let sidechain_line = serde_json::json!({
        "type": "assistant",
        "isSidechain": true,
        "message": {
            "id": "msg-sidechain",
            "usage": {
                "input_tokens": 999,
                "output_tokens": 999
            }
        }
    });

    let line2 = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-2",
            "usage": {
                "input_tokens": 50,
                "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 0,
                "output_tokens": 20
            }
        }
    });

    // Write line1, sidechain line, and an incomplete line2 (no trailing \n)
    let l1_str = format!("{line1}\n");
    let sc_str = format!("{sidechain_line}\n");
    let l2_partial = line2.to_string(); // no newline!

    std::fs::write(&transcript, format!("{l1_str}{sc_str}{l2_partial}")).expect("write");

    // Read 1: line1 is counted; sidechain line is skipped; incomplete line2 is NOT counted yet
    let (t1, offset1, ids1) = read_transcript_usage(&transcript, 0, &[]).expect("read 1");
    assert_eq!(t1.input, 100);
    assert_eq!(t1.output, 50);
    assert_eq!(ids1, vec!["msg-split"]);
    // Offset advanced only to end of sidechain line (last \n)
    assert_eq!(offset1 as usize, l1_str.len() + sc_str.len());

    // Now complete line2 with a trailing \n, plus another line repeating msg-split (split id across two reads)
    let split_line_repeat = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-split",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        }
    });
    let append = format!("\n{split_line_repeat}\n");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&transcript)
        .expect("open");
    std::io::Write::write_all(&mut file, append.as_bytes()).expect("append");

    // Read 2: completes line2 and counts it; msg-split is remembered in ids1 so it is skipped (split id counted once)
    let (t2, offset2, ids2) = read_transcript_usage(&transcript, offset1, &ids1).expect("read 2");
    assert_eq!(t2.input, 50);
    assert_eq!(t2.output, 20);
    assert!(ids2.contains(&"msg-2".to_string()));
    assert!(offset2 > offset1);
}

#[test]
fn resume_with_existing_transcript_appends_no_usage() {
    let _guard = crate::nirvana::with_test_home("resume-no-usage");
    let proj = scratch("resume-proj");
    let project = Project::at(&proj);
    let run = "r-resume";
    let journal = project.journal(run);
    journal
        .append(
            1,
            crate::nirvana_run::Kind::Opened {
                project: "p".into(),
                orchestrator_pane: None,
                auto: false,
                pools: vec![],
                pools_basis: None,
            },
        )
        .expect("opened");

    let transcript = proj.join("transcript.jsonl");
    let line = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-1",
            "usage": {"input_tokens": 500, "output_tokens": 200}
        }
    });
    std::fs::write(&transcript, format!("{line}\n{line}\n{line}\n")).expect("write");

    let hook_start = serde_json::json!({
        "hook_event_name": "SessionStart",
        "source": "resume",
        "session_id": "sess-resume-1",
        "transcript_path": transcript.display().to_string(),
        "cwd": proj.display().to_string(),
    });

    let res = log_usage_from(&hook_start, true).expect("log_usage_from");
    assert_eq!(res, None);

    // Verify journal has NO usage events appended
    let (events, _) = journal.read();
    assert!(!events
        .iter()
        .any(|e| matches!(&e.kind, crate::nirvana_run::Kind::Usage { .. })));

    // Verify offset file was created with offset == transcript length
    let offset_path = home_dir()
        .join(nirvana_seat::DIR)
        .join("usage-offsets")
        .join("sess-resume-1.json");
    assert!(offset_path.exists());
    let state: UsageOffsetState =
        serde_json::from_str(&std::fs::read_to_string(&offset_path).unwrap()).unwrap();
    assert_eq!(state.offset, std::fs::metadata(&transcript).unwrap().len());
}

#[test]
fn post_tool_use_output_has_exactly_keys_decision_and_reason() {
    let hook = serde_json::json!({
        "tool_name": "Edit",
    });
    let baseline = EditState::default();
    let mut now = EditState::default();
    now.files.insert(
        "src/main.rs".into(),
        FileDelta {
            is_tracked: true,
            added: 50,
            deleted: 0,
            line_count: 50,
            ..Default::default()
        },
    );
    let decision = check_edit_decision(&hook, None, &baseline, &now, EditLimit::default(), true)
        .expect("decision");
    let obj = decision.as_object().expect("object");
    let mut keys: Vec<String> = obj.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["decision", "reason"]);
}

#[test]
fn test_is_out_of_scope_memory_and_nirvana() {
    // Project's own memory folder is NOT exempt
    assert!(!is_out_of_scope("memory/notes.md"));
    assert!(!is_out_of_scope("src/memory/store.rs"));

    // User's Claude memory folder by absolute path IS exempt
    assert!(is_out_of_scope(
        "/home/user/.claude/projects/proj-123/memory/scratch.md"
    ));
    assert!(is_out_of_scope(
        "~/.claude/projects/proj-123/memory/scratch.md"
    ));
    assert!(is_out_of_scope(
        "C:/Users/fahim/.claude/projects/proj-123/memory/scratch.md"
    ));

    // .nirvana/ and .claude/ and NIRVANA.md are exempt
    assert!(is_out_of_scope(".nirvana/state.json"));
    assert!(is_out_of_scope(".nirvana"));
    assert!(is_out_of_scope(".claude/settings.json"));
    assert!(is_out_of_scope("NIRVANA.md"));
}

#[test]
fn guard_edit_decision_blocks_past_limit() {
    let proj = scratch("guard-limit-proj");
    let hook = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": "src/main.rs",
            "content": "line\n".repeat(31)
        }
    });
    let dec = guard_edit_decision(&hook, Some(&proj), Some(&proj), EditLimit::default(), true);
    assert!(dec.is_some());
}

#[test]
fn check_edit_decision_block_output() {
    let hook = serde_json::json!({
        "tool_name": "Edit",
    });
    let baseline = EditState::default();
    let mut now = EditState::default();
    now.files.insert(
        "src/main.rs".into(),
        FileDelta {
            is_tracked: true,
            added: 35,
            ..Default::default()
        },
    );
    let dec = check_edit_decision(&hook, None, &baseline, &now, EditLimit::default(), true);
    assert_eq!(dec.unwrap()["decision"], "block");
}

#[test]
fn log_usage_session_start_gating_and_write_offset() {
    let _guard = crate::nirvana::with_test_home("log-usage-write-offset");
    let proj = scratch("proj-usage-offset");
    let project = Project::at(&proj);
    let run = "r-off";
    let journal = project.journal(run);
    journal
        .append(
            1,
            crate::nirvana_run::Kind::Opened {
                project: "p".into(),
                orchestrator_pane: None,
                auto: false,
                pools: vec![],
                pools_basis: None,
            },
        )
        .expect("opened");

    let transcript = proj.join("t.jsonl");
    let line = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-1",
            "usage": {"input_tokens": 100, "output_tokens": 50}
        }
    });
    std::fs::write(&transcript, format!("{line}\n")).expect("write");

    let hook_stop = serde_json::json!({
        "hook_event_name": "Stop",
        "session_id": "sess-offset-test",
        "transcript_path": transcript.display().to_string(),
        "cwd": proj.display().to_string(),
    });

    let res = log_usage_from(&hook_stop, true).expect("log_usage_from");
    assert!(res.is_some());

    // Verify offset was written to file
    let offset_path = home_dir()
        .join(nirvana_seat::DIR)
        .join("usage-offsets")
        .join("sess-offset-test.json");
    assert!(offset_path.exists());
    let state: UsageOffsetState =
        serde_json::from_str(&std::fs::read_to_string(&offset_path).unwrap()).unwrap();
    assert_eq!(state.offset, std::fs::metadata(&transcript).unwrap().len());
}

#[test]
fn cost_cache_reuses_size_and_mtime_entry_with_sentinel_hash() {
    let dir = scratch("cache-sentinel");
    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&dir)
        .output();
    let file = dir.join("cached.txt");
    std::fs::write(&file, "initial text\n").expect("write");
    let initial_state = capture_edit_state(&dir);
    let orig_entry = initial_state.files.get("cached.txt").expect("entry");

    // Create a previous state with a sentinel hash
    let mut prev_state = EditState::default();
    let mut sentinel_entry = orig_entry.clone();
    sentinel_entry.hash = Some("sentinel-sha-12345".to_string());
    prev_state
        .files
        .insert("cached.txt".to_string(), sentinel_entry);

    // Capture with prev_state: keeps sentinel and does not re-read
    let cached_state = capture_edit_state_cached(&dir, Some(&prev_state));
    let entry = cached_state.files.get("cached.txt").expect("entry");
    assert_eq!(entry.hash.as_deref(), Some("sentinel-sha-12345"));
}

#[test]
fn large_file_over_256k_entry_is_size_colon_mtime() {
    let dir = scratch("large-file-size");
    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&dir)
        .output();
    let file = dir.join("large.bin");
    let size = 300 * 1024;
    // Write 300 KiB of non-zero bytes
    std::fs::write(&file, vec![b'a'; size]).expect("write");

    let state = capture_edit_state(&dir);
    let entry = state.files.get("large.bin").expect("entry");
    let expected_hash = format!("{}:{}", entry.size, entry.mtime);
    assert_eq!(entry.hash.as_deref(), Some(expected_hash.as_str()));
    assert_eq!(entry.content, None);
}

#[test]
fn check_edit_from_passes_baseline_cache() {
    let _guard = crate::nirvana::with_test_home("check-edit-baseline-cache");
    let proj = scratch("check-edit-cache-proj");
    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&proj)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Tester"])
        .current_dir(&proj)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "tester@test.com"])
        .current_dir(&proj)
        .output();
    let file1 = proj.join("cached1.txt");
    let file2 = proj.join("cached2.txt");
    std::fs::write(&file1, "hello 1\n").expect("write");
    std::fs::write(&file2, "hello 2\n").expect("write");

    let dir = home_dir().join(nirvana_seat::DIR).join("edit-baselines");
    std::fs::create_dir_all(&dir).expect("create dir");
    let session_path = dir.join("session-cache.json");

    let meta1 = std::fs::metadata(&file1).unwrap();
    let mtime1 = meta1
        .modified()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let meta2 = std::fs::metadata(&file2).unwrap();
    let mtime2 = meta2
        .modified()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut baseline = EditState::default();
    baseline.files.insert(
        "cached1.txt".to_string(),
        FileDelta {
            is_tracked: false,
            is_binary: false,
            added: 1,
            deleted: 0,
            line_count: 1,
            hash: Some("sentinel-from-baseline-1".to_string()),
            content: None,
            size: meta1.len(),
            mtime: mtime1,
        },
    );
    baseline.files.insert(
        "cached2.txt".to_string(),
        FileDelta {
            is_tracked: false,
            is_binary: false,
            added: 1,
            deleted: 0,
            line_count: 1,
            hash: Some("sentinel-from-baseline-2".to_string()),
            content: None,
            size: meta2.len(),
            mtime: mtime2,
        },
    );
    std::fs::write(&session_path, serde_json::to_string(&baseline).unwrap()).expect("write");

    let hook = serde_json::json!({
        "session_id": "session-cache",
        "cwd": proj.display().to_string(),
        "tool_name": "Edit",
    });

    let dec = check_edit_from(&hook, true).expect("check_edit_from");
    assert_eq!(dec, None);
}

#[test]
fn guard_edit_outside_project_ignored_and_mixed_case_windows_path() {
    let proj = scratch("guard-r4-proj");
    let outside = scratch("guard-r4-outside");
    let hook_outside = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": outside.join("scratch.txt").display().to_string(),
            "content": "line\n".repeat(100)
        }
    });

    // 1. no project, 100-line Write -> no deny
    let dec1 = guard_edit_decision(&hook_outside, None, None, EditLimit::default(), true);
    assert_eq!(dec1, None);

    // 2. a project, a 100-line Write to a path outside it -> no deny
    let dec2 = guard_edit_decision(
        &hook_outside,
        Some(&proj),
        Some(&proj),
        EditLimit::default(),
        true,
    );
    assert_eq!(dec2, None);

    // 3. same Write inside it with mixed-case drive letter and forward slashes -> deny
    let mut inside_str = proj
        .join("src/main.rs")
        .display()
        .to_string()
        .replace('\\', "/");
    if inside_str.len() >= 2 && inside_str.as_bytes()[1] == b':' {
        let first = (inside_str.as_bytes()[0] as char).to_ascii_uppercase();
        inside_str.replace_range(0..1, &first.to_string());
    }
    let hook_inside = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": inside_str,
            "content": "line\n".repeat(100)
        }
    });
    let dec3 = guard_edit_decision(
        &hook_inside,
        Some(&proj),
        Some(&proj),
        EditLimit::default(),
        true,
    );
    assert!(dec3.is_some());
}

#[test]
fn prompt_baseline_fail_open_removes_session_baseline() {
    let _guard = crate::nirvana::with_test_home("fail-open-removes-baseline");
    let proj = scratch("fail-open-proj");
    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&proj)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Tester"])
        .current_dir(&proj)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "tester@test.com"])
        .current_dir(&proj)
        .output();

    let dir = home_dir().join(nirvana_seat::DIR).join("edit-baselines");
    std::fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("sess-fail-open.json");
    std::fs::write(&path, "{\"dummy\": true}").expect("write");
    assert!(path.exists());

    // Create 2001 untracked files to trigger fail_open
    for i in 0..2001 {
        std::fs::write(proj.join(format!("file_{i}.txt")), "x").expect("write");
    }

    let hook = serde_json::json!({
        "session_id": "sess-fail-open",
        "cwd": proj.display().to_string(),
    });
    prompt_baseline_from(&hook, true).expect("prompt_baseline_from");
    // Baseline file must be removed on fail_open
    assert!(!path.exists());
}

#[test]
fn prune_older_baselines_keeps_fresh() {
    let _guard = crate::nirvana::with_test_home("prune-baselines");
    let dir = home_dir().join(nirvana_seat::DIR).join("edit-baselines");
    std::fs::create_dir_all(&dir).expect("create dir");
    let old_path = dir.join("old-sess.json");
    let fresh_path = dir.join("fresh-sess.json");

    std::fs::write(&old_path, "{}").expect("write old");
    std::fs::write(&fresh_path, "{}").expect("write fresh");

    // Set old_path modified time to 8 days ago
    let old_time = SystemTime::now() - std::time::Duration::from_secs(8 * 86400);
    let old_file = std::fs::OpenOptions::new()
        .write(true)
        .open(&old_path)
        .expect("open with write");
    old_file.set_modified(old_time).expect("set_modified");

    prune_files_older_than(&dir, 7 * 86400);

    assert!(!old_path.exists(), "8-day old baseline should be pruned");
    assert!(fresh_path.exists(), "fresh baseline should be kept");
}

#[test]
fn parse_numstat_z_handles_binary_and_rename() {
    // 1. Binary line: -\t-\tx.png\0
    let bin_raw = b"-\t-\tx.png\0";
    let entries = parse_numstat_z(bin_raw);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "x.png");
    assert!(entries[0].is_binary);
    assert_eq!(entries[0].added, 0);
    assert_eq!(entries[0].deleted, 0);

    // 2. Rename line: 1\t2\t\0old.txt\0new.txt\0
    let rename_raw = b"1\t2\t\0old.txt\0new.txt\0";
    let entries = parse_numstat_z(rename_raw);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "new.txt");
    assert!(!entries[0].is_binary);
    assert_eq!(entries[0].added, 1);
    assert_eq!(entries[0].deleted, 2);
}

#[test]
fn transcript_usage_synthetic_line_not_stored_in_seen_ids() {
    let dir = scratch("usage-no-id");
    let transcript = dir.join("transcript.jsonl");

    // Assistant line with NO message.id
    let line = serde_json::json!({
        "type": "assistant",
        "message": {
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        }
    });
    std::fs::write(&transcript, format!("{line}\n")).expect("write");

    let (tokens, _, combined_ids) = read_transcript_usage(&transcript, 0, &[]).expect("read");
    assert_eq!(tokens.input, 100);
    assert_eq!(tokens.output, 50);
    // Synthetic line-0 must NOT be in combined_ids
    assert!(!combined_ids.iter().any(|id| id.starts_with("line-")));
}

#[test]
fn capture_edit_state_over_2000_untracked_fails_open() {
    let repo = scratch("fail-open-untracked");
    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Tester"])
        .current_dir(&repo)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "tester@test.com"])
        .current_dir(&repo)
        .output();

    for i in 0..2001 {
        std::fs::write(repo.join(format!("f_{i}.txt")), "data").expect("write");
    }

    let state = capture_edit_state(&repo);
    assert!(state.fail_open);
    assert!(state.files.is_empty());
}
