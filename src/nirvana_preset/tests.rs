//! Plan 13 P1: the smart Preload's engine on fixture folders, with Jev answering from recordings.

use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A scratch folder of this test process, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nirvana-p13a-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn touch(folder: &Path, files: &[&str]) {
    for file in files {
        let path = folder.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{file}\n")).unwrap();
    }
}

/// §9's fixtures: `3d` holds a `.3mf`, two `.stl`, a `.gcode` and a README; `blank` is empty; `rust` holds a
/// `Cargo.toml` and `src/main.rs`; `mixed` has no clear type.
fn fixture(kind: &str) -> PathBuf {
    let folder = scratch(&format!("fx-{kind}"));
    match kind {
        "3d" => touch(
            &folder,
            &[
                "part.3mf",
                "left.stl",
                "right.stl",
                "plate.gcode",
                "README.md",
            ],
        ),
        "rust" => touch(&folder, &["Cargo.toml", "src/main.rs"]),
        "mixed" => touch(
            &folder,
            &[
                "notes.txt",
                "data.csv",
                "sketch.png",
                "run.sh",
                "config.json",
                "todo.md",
            ],
        ),
        _ => {}
    }
    folder
}

fn config() -> Config {
    parse_config(DEFAULT).expect("the default preload.toml parses")
}

fn markers() -> Vec<String> {
    config().markers()
}

/// Jev from recordings: each call takes the next answer map (or error), and counts.
struct Recorded {
    replies: Mutex<VecDeque<Result<Value, JevError>>>,
    calls: AtomicUsize,
    delay: Duration,
    sent: Mutex<Vec<Value>>,
}

impl Recorded {
    fn new(replies: Vec<Result<Value, JevError>>) -> Arc<Recorded> {
        Self::slow(replies, Duration::ZERO)
    }

    fn slow(replies: Vec<Result<Value, JevError>>, delay: Duration) -> Arc<Recorded> {
        Arc::new(Recorded {
            replies: Mutex::new(replies.into()),
            calls: AtomicUsize::new(0),
            delay,
            sent: Mutex::new(Vec::new()),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Judge for Recorded {
    fn ask(&self, state: &Value, questions: &Map<String, Value>) -> Result<Reply, JevError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.sent
            .lock()
            .unwrap()
            .push(json!({"state": state, "questions": questions}));
        std::thread::sleep(self.delay);
        let next = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("a recorded answer for every call");
        next.map(|answers| Reply {
            model: "recorded".into(),
            answers: answers.as_object().cloned().unwrap_or_default(),
            input_tokens: 10,
            output_tokens: 2,
            cost_usd: Some(0.0001),
            ms: 7,
        })
    }
}

fn choice(choice: &str, confidence: f64) -> Value {
    json!({"type": "choice", "choice": choice, "confidence": confidence, "probabilities": {}})
}

fn judge(recorded: &Arc<Recorded>) -> Option<Arc<dyn Judge>> {
    Some(Arc::clone(recorded) as Arc<dyn Judge>)
}

// ---- the configuration ----------------------------------------------------------------------------------

#[test]
fn the_default_presets_parse_in_the_files_order_with_their_core_tags_and_rules() {
    let config = config();
    let names: Vec<&str> = config.presets.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        ["3d-print", "photo", "rust", "web", "python", "research", "blank", "general"]
    );
    assert_eq!(
        config.core,
        ["nirvana", "bladebro", "superpowers", "context7"]
    );
    assert_eq!(config.resume_within_hours, 24.0);
    assert_eq!(config.trim_over_tokens, 4_000);
    // §6.3: auto memory is on for every seeded preset except blank.
    for preset in &config.presets {
        assert_eq!(
            preset.auto_memory,
            preset.name != "blank",
            "{}",
            preset.name
        );
    }
    assert!(config.preset(GENERAL).unwrap().keeps_all());
    for tag in ["3d-print", "photo", "research", "personal", "code"] {
        assert!(config.tags.iter().any(|(t, _)| t == tag), "{tag}");
    }
    // A typo in the owner's file is said, never half-read.
    assert!(parse_config("core = []\n[preset.x]\nkeeps = [\"code\"]\n").is_err());
}

#[test]
fn preload_toml_is_written_from_the_default_once_and_the_owners_from_then_on() {
    let dir = scratch("config");
    assert_eq!(load_config(&dir).unwrap(), config());
    assert_eq!(
        std::fs::read_to_string(dir.join(CONFIG_FILE)).unwrap(),
        DEFAULT
    );
    std::fs::write(
        dir.join(CONFIG_FILE),
        "core = [\"x\"]\nresume_within_hours = 2\n[preset.only]\nkeep = [\"*\"]\n",
    )
    .unwrap();
    let mine = load_config(&dir).unwrap();
    assert_eq!(mine.core, ["x"]);
    assert_eq!(resume_within_hours(&dir), 2.0);
    assert_eq!(mine.presets.len(), 1);
}

// ---- the fingerprint and the rules --------------------------------------------------------------------------

#[test]
fn a_folder_of_3mf_and_stl_reads_3d_print_and_an_empty_one_blank() {
    // L4's verifier: "a folder of .3mf/.stl reads 3d-print, an empty one blank".
    let config = config();
    let ruled = |folder: &Path| by_rules(&config, &fingerprint(folder, &markers()));
    let three_d = fixture("3d");
    let fp = fingerprint(&three_d, &markers());
    assert_eq!(fp.files, 5);
    assert_eq!(fp.ext.get("stl"), Some(&2));
    assert!(fp.markers.contains("*.3mf") && fp.markers.contains("*.gcode"));
    assert_eq!(fp.readme.as_deref(), Some("README.md\n"));
    match ruled(&three_d) {
        Ruled::Chosen(choice) => {
            assert_eq!(choice.preset, "3d-print");
            assert_eq!(choice.by, "rule");
            assert!(choice.reason.contains("80%"), "{}", choice.reason);
        }
        other => panic!("{other:?}"),
    }
    let blank = fixture("blank");
    assert!(fingerprint(&blank, &markers()).blank);
    assert!(matches!(ruled(&blank), Ruled::Chosen(c) if c.preset == "blank"));
    // Nothing but .claude and .git is still blank (§3.1).
    touch(&blank, &[".claude/settings.local.json", ".git/HEAD"]);
    let fp = fingerprint(&blank, &markers());
    assert!(fp.blank, "{fp:?}");
    assert_eq!(fp.files, 0, ".claude and .git are never walked");
    assert!(matches!(ruled(&blank), Ruled::Chosen(c) if c.preset == "blank"));
    let rust = fixture("rust");
    assert!(
        matches!(ruled(&rust), Ruled::Chosen(c) if c.preset == "rust" && c.reason == "rule: marker Cargo.toml")
    );
    assert!(matches!(ruled(&fixture("mixed")), Ruled::Ask(why) if why == "no rule matches"));
}

#[test]
fn a_marker_beats_a_share_and_two_close_shares_go_to_jev() {
    let config = config();
    let folder = fixture("3d");
    touch(&folder, &["package.json"]);
    assert!(
        matches!(by_rules(&config, &fingerprint(&folder, &markers())), Ruled::Chosen(c) if c.preset == "web"),
        "§3.4: a marker match beats an extension share"
    );
    let close = parse_config(
        "[preset.a]\nwhen = { ext = [\"stl\"], share = 0.2 }\nkeep = [\"x\"]\n\
         [preset.b]\nwhen = { ext = [\"md\"], share = 0.2 }\nkeep = [\"y\"]\n",
    )
    .unwrap();
    let even = scratch("close");
    touch(&even, &["a.stl", "b.stl", "c.md", "d.md", "e.md"]);
    // 40% and 60% are 20 points apart: no tie, and the first in file order wins, whatever its share.
    assert!(matches!(
        by_rules(&close, &fingerprint(&even, &close.markers())),
        Ruled::Chosen(c) if c.preset == "a"
    ));
    let tie = scratch("tie");
    touch(&tie, &["a.stl", "b.stl", "c.md", "d.md"]);
    assert!(matches!(
        by_rules(&close, &fingerprint(&tie, &close.markers())),
        Ruled::Ask(why) if why == "a 50% and b 50% are within 10%"
    ));
}

#[test]
fn the_walk_skips_build_and_dependency_folders_and_the_hash_moves_with_a_doubling() {
    let folder = fixture("rust");
    let before = fingerprint(&folder, &markers());
    touch(
        &folder,
        &[
            "target/debug/a.o",
            "node_modules/x/index.js",
            ".venv/lib/x.py",
            ".claude/x.json",
        ],
    );
    let after = fingerprint(&folder, &markers());
    assert_eq!(after.files, before.files, "{after:?}");
    assert_eq!(after.hash(), before.hash());
    // `.git` is never walked, but it is a marker: a folder that became a repository reads again.
    touch(&folder, &[".git/config"]);
    let git = fingerprint(&folder, &markers());
    assert_eq!(git.files, before.files);
    assert!(git.markers.contains(".git"));
    assert_ne!(git.hash(), before.hash());
    let before = git;
    // One more .rs file is the same bucket (1 -> 2 is a new bucket, 2 -> 3 is not).
    touch(&folder, &["src/lib.rs"]);
    let two = fingerprint(&folder, &markers()).hash();
    assert_ne!(two, before.hash());
    touch(&folder, &["src/more.rs"]);
    assert_eq!(fingerprint(&folder, &markers()).hash(), two);
    assert_eq!(bucket(1), 1);
    assert_eq!(bucket(3), 2);
    assert_eq!(bucket(4), 3);
}

// ---- Jev for the folder ---------------------------------------------------------------------------------

#[test]
fn jev_chooses_only_at_the_floor_and_none_unsure_or_a_failure_is_general() {
    let config = config();
    let reply = |answer: Value| Reply {
        model: "recorded".into(),
        answers: json!({ "preset": answer }).as_object().cloned().unwrap(),
        input_tokens: 1,
        output_tokens: 1,
        cost_usd: None,
        ms: 1,
    };
    let (sure, _, _) = read_folder_reply(&config, &reply(choice("web", 0.91)));
    assert_eq!((sure.preset.as_str(), sure.by.as_str()), ("web", "jev"));
    let (unsure, logged, _) = read_folder_reply(&config, &reply(choice("web", 0.55)));
    assert_eq!(
        (unsure.preset.as_str(), logged.as_str()),
        (GENERAL, "unsure")
    );
    let (none, logged, _) = read_folder_reply(&config, &reply(choice(NONE, 0.95)));
    assert_eq!((none.preset.as_str(), logged.as_str()), (GENERAL, NONE));
    let (unknown, _, _) = read_folder_reply(&config, &reply(choice("cooking", 0.99)));
    assert_eq!(unknown.preset, GENERAL);
}

#[test]
fn the_folder_question_carries_counts_markers_and_the_readme_never_a_files_text() {
    let config = config();
    let folder = fixture("mixed");
    std::fs::write(folder.join("notes.txt"), "PRIVATE-NOTE-TEXT").unwrap();
    std::fs::write(folder.join("README.md"), "A tool for sorting receipts.").unwrap();
    let (state, questions) = folder_question(&config, &fingerprint(&folder, &markers()));
    let text = json!({"s": state, "q": questions}).to_string();
    assert!(!text.contains("PRIVATE-NOTE-TEXT"), "{text}");
    assert!(text.contains("A tool for sorting receipts."));
    assert_eq!(state["folder"]["extension_counts"]["txt"], 1);
    let criteria = questions["preset"]["criteria"].as_object().unwrap();
    let offered: Vec<&str> = criteria.keys().map(String::as_str).collect();
    // Blank and general are never offered; none always is.
    for name in [
        "3d-print", "photo", "rust", "web", "python", "research", NONE,
    ] {
        assert!(offered.contains(&name), "{offered:?}");
    }
    assert!(!offered.contains(&"blank") && !offered.contains(&GENERAL));
}

#[test]
fn a_cache_miss_asks_jev_once_and_the_same_fingerprint_spends_nothing() {
    let dir = scratch("cache");
    let folder = fixture("mixed");
    let jev = Recorded::new(vec![Ok(json!({"preset": choice("research", 0.88)}))]);
    let record = choose(&dir, &folder, judge(&jev), OPEN_WALL).unwrap();
    assert_eq!(
        (record.choice.as_str(), record.by.as_str()),
        ("research", "jev")
    );
    assert!(
        record
            .reason
            .starts_with("no rule matches; Jev: research 0.88"),
        "{}",
        record.reason
    );
    assert_eq!(jev.calls(), 1);
    // §3.4 step 3: "An open with the same hash spends nothing."
    let again = choose(&dir, &folder, judge(&jev), OPEN_WALL).unwrap();
    assert_eq!(again, record);
    assert_eq!(jev.calls(), 1);
    // The folder changes: chosen again.
    touch(&folder, &["Cargo.toml"]);
    let changed = choose(&dir, &folder, judge(&jev), OPEN_WALL).unwrap();
    assert_eq!(
        (changed.choice.as_str(), changed.by.as_str()),
        ("rust", "rule")
    );
    assert_eq!(jev.calls(), 1, "the rules decided; Jev was not asked");
    // Every call is a line of jev-calls.jsonl with its role and a hash, never the key or the text.
    let log = std::fs::read_to_string(dir.join(crate::nirvana_jev_roles::CALLS_FILE)).unwrap();
    let lines: Vec<Value> = log
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    // The plan's own word (§3.4), not the constant: a renamed role would hide from `herdr jev agree`'s readers.
    assert_eq!(lines[0]["role"], "preload-folder");
    assert_eq!(lines[0]["answer"], "research");
    assert_eq!(lines[0]["prompt"].as_str().map(str::len), Some(16));
    assert!(!log.contains("notes.txt") && !log.contains("mock-key"));
}

#[test]
fn a_failure_or_no_key_is_general_and_asked_again_at_the_next_open() {
    let dir = scratch("failed");
    let folder = fixture("mixed");
    let jev = Recorded::new(vec![
        Err(JevError::Http(503)),
        Ok(json!({"preset": choice(NONE, 0.9)})),
    ]);
    let first = choose(&dir, &folder, judge(&jev), OPEN_WALL).unwrap();
    assert_eq!(
        (first.choice.as_str(), first.by.as_str(), first.retry),
        (GENERAL, "failed", true)
    );
    let second = choose(&dir, &folder, judge(&jev), OPEN_WALL).unwrap();
    assert_eq!(
        (second.choice.as_str(), second.by.as_str(), second.retry),
        (GENERAL, "none", false)
    );
    assert_eq!(jev.calls(), 2);
    let log = std::fs::read_to_string(dir.join(crate::nirvana_jev_roles::CALLS_FILE)).unwrap();
    assert!(
        log.lines()
            .next()
            .unwrap()
            .contains("\"outcome\":\"failed\""),
        "{log}"
    );
    let no_key = scratch("nokey");
    let record = choose(&no_key, &folder, None, OPEN_WALL).unwrap();
    assert_eq!((record.choice.as_str(), record.retry), (GENERAL, true));
    assert!(record.reason.ends_with("no Jev key"));
}

#[test]
fn past_the_wall_the_open_goes_ahead_on_general_and_the_answer_is_used_next_time() {
    let dir = scratch("wall");
    let folder = fixture("mixed");
    let jev = Recorded::slow(
        vec![Ok(json!({"preset": choice("python", 0.93)}))],
        Duration::from_millis(400),
    );
    let started = Instant::now();
    let record = choose(&dir, &folder, judge(&jev), Duration::from_millis(50)).unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(350),
        "the open waited past its wall"
    );
    assert_eq!(
        (record.choice.as_str(), record.by.as_str(), record.retry),
        (GENERAL, "timeout", true)
    );
    // The asking thread records Jev's answer when it comes.
    let deadline = Instant::now() + Duration::from_secs(5);
    let late = loop {
        let now = read_record(&dir, &folder).unwrap();
        if now.by == "jev" || Instant::now() > deadline {
            break now;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(
        (late.choice.as_str(), late.by.as_str(), late.retry),
        ("python", "jev", false)
    );
    // ...so the next open spends nothing.
    assert_eq!(
        choose(&dir, &folder, judge(&jev), OPEN_WALL)
            .unwrap()
            .choice,
        "python"
    );
    assert_eq!(jev.calls(), 1);
}

// ---- the catalog --------------------------------------------------------------------------------------------

fn reading(items: &[(&str, u32)]) -> Reading {
    Reading {
        at: 1,
        total: 23_502,
        items: items
            .iter()
            .map(|(key, tokens)| {
                (
                    (*key).to_owned(),
                    ReadItem {
                        tokens: *tokens,
                        about: format!("about {key}"),
                    },
                )
            })
            .collect(),
    }
}

/// The owner's catalog as the 2026-10-18 `/context` named it, cut down.
fn owners_reading() -> Reading {
    reading(&[
        ("skill:bambu-h2c-cosplay-print", 141),
        ("skill:blender-stl-manipulation", 287),
        ("skill:cosplay-color-painting", 135),
        ("skill:portrait-color-correction", 302),
        ("skill:batch-photo-processing", 266),
        ("skill:scholar-core", 277),
        ("skill:benchmark", 112),
        ("skill:dataviz", 482),
        ("mcp:bladebro", 315),
        ("mcp:claude.ai Gmail", 315),
        ("mcp:claude.ai Felt Maps", 315),
        ("mcp:plugin:context7:context7", 0),
        ("mcp:plugin:github:github", 315),
        ("plugin:superpowers@superpowers-marketplace", 1_000),
        ("plugin:kotlin-lsp@claude-plugins-official", 0),
        ("plugin:rust-analyzer-lsp@claude-plugins-official", 0),
    ])
}

#[test]
fn the_name_rules_seed_the_catalog_for_free_and_leave_core_and_the_rest_to_jev() {
    let dir = scratch("seed");
    let config = config();
    let reading = owners_reading();
    assert_eq!(seed_by_rules(&dir, &config, &reading).unwrap(), 10);
    let catalog = read_catalog(&dir);
    let tags = |key: &str| catalog.entries.get(key).map(|e| e.tags.join(","));
    assert_eq!(
        tags("skill:bambu-h2c-cosplay-print").as_deref(),
        Some("3d-print")
    );
    assert_eq!(
        tags("skill:blender-stl-manipulation").as_deref(),
        Some("3d-print")
    );
    assert_eq!(
        tags("skill:cosplay-color-painting").as_deref(),
        Some("3d-print")
    );
    assert_eq!(
        tags("skill:portrait-color-correction").as_deref(),
        Some("photo")
    );
    assert_eq!(
        tags("skill:batch-photo-processing").as_deref(),
        Some("photo")
    );
    assert_eq!(tags("skill:scholar-core").as_deref(), Some("research"));
    assert_eq!(tags("mcp:claude.ai Gmail").as_deref(), Some("personal"));
    assert_eq!(
        tags("plugin:rust-analyzer-lsp@claude-plugins-official").as_deref(),
        Some("rust")
    );
    assert_eq!(
        tags("plugin:kotlin-lsp@claude-plugins-official").as_deref(),
        Some("kotlin")
    );
    assert_eq!(catalog.entries["skill:scholar-core"].source, "rule");
    // Core is never tagged; benchmark, dataviz and github have no rule: Jev's.
    assert_eq!(tags("mcp:bladebro"), None);
    assert_eq!(tags("plugin:superpowers@superpowers-marketplace"), None);
    let left: Vec<String> = untagged(&config, &catalog, &reading)
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(
        left,
        [
            "mcp:plugin:github:github",
            "skill:benchmark",
            "skill:dataviz"
        ]
    );
    // Seeding again adds nothing.
    assert_eq!(seed_by_rules(&dir, &config, &reading).unwrap(), 0);
}

#[test]
fn herdr_only_appends_so_a_line_the_owner_edited_is_never_retagged() {
    let dir = scratch("owner");
    let config = config();
    std::fs::write(
        dir.join(CATALOG_FILE),
        "# mine\n\"skill:bambu-h2c-cosplay-print\" = [\"code\"]  # owner\n\"skill:scholar-core\" = []\n",
    )
    .unwrap();
    seed_by_rules(&dir, &config, &owners_reading()).unwrap();
    let text = std::fs::read_to_string(dir.join(CATALOG_FILE)).unwrap();
    assert!(text.starts_with("# mine\n\"skill:bambu-h2c-cosplay-print\" = [\"code\"]  # owner\n\"skill:scholar-core\" = []\n"), "{text}");
    let catalog = parse_catalog(&text);
    assert_eq!(
        catalog.entries["skill:bambu-h2c-cosplay-print"].tags,
        ["code"]
    );
    assert_eq!(
        catalog.entries["skill:bambu-h2c-cosplay-print"].source,
        "owner"
    );
    // A line with no comment is the owner's too.
    assert_eq!(catalog.entries["skill:scholar-core"].source, "owner");
    assert_eq!(
        catalog.entries["skill:portrait-color-correction"].source,
        "rule"
    );
    // A key with a space and a dot round-trips.
    let line = catalog_line("mcp:claude.ai Booking.com", &["personal".into()], "rule");
    assert_eq!(
        line,
        "\"mcp:claude.ai Booking.com\" = [\"personal\"]  # rule"
    );
    assert_eq!(
        parse_catalog(&line).entries["mcp:claude.ai Booking.com"].tags,
        ["personal"]
    );
}

#[test]
fn jev_tags_at_the_floor_none_and_unsure_stay_on_and_a_failed_call_writes_nothing() {
    let dir = scratch("tag");
    let config = config();
    let items: Vec<(String, String)> = [
        "skill:benchmark",
        "skill:dataviz",
        "mcp:plugin:github:github",
        "skill:docx",
    ]
    .iter()
    .map(|k| ((*k).to_owned(), format!("about {k}")))
    .collect();
    let jev = Recorded::new(vec![Ok(json!({
        "i1": choice("research", 0.82),
        "i2": choice(NONE, 0.9),
        "i3": choice("code", 0.55),
        "i4": choice("cooking", 0.99),
    }))]);
    let tagged = tag_with(jev.as_ref(), &dir, &config, &items, 5);
    assert_eq!(
        tagged,
        Tagged {
            calls: 1,
            tagged: 1,
            left_on: 3,
            ..Tagged::default()
        }
    );
    let catalog = read_catalog(&dir);
    assert_eq!(catalog.entries["skill:benchmark"].tags, ["research"]);
    assert_eq!(
        catalog.entries["skill:benchmark"].source,
        "jev research 0.82"
    );
    assert!(catalog.entries["skill:dataviz"].tags.is_empty());
    assert_eq!(
        catalog.entries["mcp:plugin:github:github"].source,
        "jev unsure code 0.55"
    );
    assert!(
        catalog.entries["skill:docx"].tags.is_empty(),
        "a tag Jev was not offered is no tag"
    );
    // One question per item, each over the known tags and none; the state holds what each item says of itself.
    let sent = jev.sent.lock().unwrap()[0].clone();
    let criteria = sent["questions"]["i1"]["criteria"].as_object().unwrap();
    assert!(criteria.contains_key(NONE) && criteria.contains_key("3d-print"));
    assert_eq!(sent["state"]["items"]["i3"]["name"], "github");

    let dir = scratch("tag-failed");
    let jev = Recorded::new(vec![Err(JevError::Timeout)]);
    let tagged = tag_with(jev.as_ref(), &dir, &config, &items, 5);
    assert_eq!((tagged.failed_calls, tagged.not_asked), (1, 4));
    assert!(
        read_catalog(&dir).entries.is_empty(),
        "a failure is never a tag"
    );
    let log = std::fs::read_to_string(dir.join(crate::nirvana_jev_roles::CALLS_FILE)).unwrap();
    assert!(log.contains("\"role\":\"preload-tag\"") && log.contains("\"outcome\":\"failed\""));
    // The call cap: no call past it.
    let many: Vec<(String, String)> = (0..40)
        .map(|i| (format!("skill:s{i}"), String::new()))
        .collect();
    let jev = Recorded::new(vec![Ok(json!({}))]);
    let capped = tag_with(jev.as_ref(), &scratch("tag-cap"), &config, &many, 1);
    assert_eq!((capped.calls, jev.calls(), capped.not_asked), (1, 1, 24));
}

// ---- what goes off ------------------------------------------------------------------------------------------

fn seeded(name: &str) -> (PathBuf, Config, Catalog, Reading) {
    let dir = scratch(name);
    let config = config();
    let reading = owners_reading();
    seed_by_rules(&dir, &config, &reading).unwrap();
    append_catalog(
        &dir,
        &[
            (
                "skill:benchmark".into(),
                vec!["research".into()],
                "jev research 0.82".into(),
            ),
            ("skill:dataviz".into(), Vec::new(), "jev none 0.90".into()),
            // Installed once, gone from the last reading: not counted off.
            (
                "skill:uninstalled".into(),
                vec!["photo".into()],
                "rule".into(),
            ),
            // The owner tagged a core item: core still wins.
            (
                "mcp:bladebro".into(),
                vec!["personal".into()],
                "owner".into(),
            ),
        ],
    )
    .unwrap();
    let catalog = read_catalog(&dir);
    (dir, config, catalog, reading)
}

fn keys(off: &[(String, String)]) -> Vec<&str> {
    off.iter().map(|(key, _)| key.as_str()).collect()
}

#[test]
fn each_fixture_preset_switches_off_what_section_9_expects() {
    let (_dir, config, catalog, reading) = seeded("off");
    // 3d: "the photo and scholar skills and the personal connectors are off; bambu and blender stay on."
    let three_d = off_items(&config, "3d-print", &catalog, Some(&reading), &[]);
    let three_d_keys = keys(&three_d);
    for off in [
        "skill:portrait-color-correction",
        "skill:batch-photo-processing",
        "skill:scholar-core",
        "skill:benchmark",
        "mcp:claude.ai Gmail",
        "mcp:claude.ai Felt Maps",
    ] {
        assert!(
            three_d_keys.contains(&off),
            "{off} not off: {three_d_keys:?}"
        );
    }
    for on in [
        "skill:bambu-h2c-cosplay-print",
        "skill:blender-stl-manipulation",
        "skill:dataviz",
        "mcp:bladebro",
        "mcp:plugin:context7:context7",
        "plugin:superpowers@superpowers-marketplace",
        "skill:uninstalled",
        AUTO_MEMORY,
    ] {
        assert!(!three_d_keys.contains(&on), "{on} off: {three_d_keys:?}");
    }
    // blank: "the 3D, photo and personal items are off, and auto memory is off."
    let blank = off_items(&config, "blank", &catalog, Some(&reading), &[]);
    let blank_keys = keys(&blank);
    for off in [
        "skill:bambu-h2c-cosplay-print",
        "skill:portrait-color-correction",
        "mcp:claude.ai Gmail",
        AUTO_MEMORY,
    ] {
        assert!(blank_keys.contains(&off), "{off}: {blank_keys:?}");
    }
    assert!(
        !blank_keys.contains(&"skill:scholar-core"),
        "blank keeps research"
    );
    // rust: "the 3D, photo and personal items are off."
    let rust = off_items(&config, "rust", &catalog, Some(&reading), &[]);
    let rust_keys = keys(&rust);
    for off in [
        "skill:cosplay-color-painting",
        "skill:batch-photo-processing",
        "mcp:claude.ai Felt Maps",
        "plugin:kotlin-lsp@claude-plugins-official",
    ] {
        assert!(rust_keys.contains(&off), "{off}: {rust_keys:?}");
    }
    assert!(!rust_keys.contains(&"plugin:rust-analyzer-lsp@claude-plugins-official"));
    assert!(!rust_keys.contains(&AUTO_MEMORY));
    // general and an unknown preset switch nothing.
    assert!(off_items(&config, GENERAL, &catalog, Some(&reading), &[]).is_empty());
    assert!(off_items(&config, "nope", &catalog, Some(&reading), &[]).is_empty());
    // A pin keeps an item on.
    let pinned = off_items(
        &config,
        "3d-print",
        &catalog,
        Some(&reading),
        &["skill:scholar-core".into()],
    );
    assert!(!keys(&pinned).contains(&"skill:scholar-core"));
    assert_eq!(pinned.len(), three_d.len() - 1);
    // The estimate is the off items' tokens from the reading.
    assert_eq!(
        saved_tokens(&three_d, Some(&reading)),
        three_d
            .iter()
            .map(|(k, _)| reading.items[k].tokens)
            .sum::<u32>()
    );
}

#[test]
fn the_generated_file_is_the_plugins_settings_plus_the_switches_and_the_workers_has_no_env() {
    let base = json!({"theme": "custom:nirvana-dark", "env": {"NIRVANA_ORCHESTRATOR": "1"}, "statusLine": {"type": "command"}});
    let off = vec![
        ("skill:scholar-core".to_owned(), "research".to_owned()),
        ("mcp:claude.ai Gmail".to_owned(), "personal".to_owned()),
        (
            "plugin:kotlin-lsp@claude-plugins-official".to_owned(),
            "kotlin".to_owned(),
        ),
        (AUTO_MEMORY.to_owned(), "auto_memory = false".to_owned()),
    ];
    let value = generated(&base, &off);
    assert_eq!(value["theme"], "custom:nirvana-dark");
    assert_eq!(value["env"]["NIRVANA_ORCHESTRATOR"], "1");
    assert_eq!(value["statusLine"]["type"], "command");
    assert_eq!(value["skillOverrides"], json!({"scholar-core": "off"}));
    assert_eq!(
        value["deniedMcpServers"],
        json!([{"serverName": "claude.ai Gmail"}])
    );
    assert_eq!(
        value["enabledPlugins"],
        json!({"kotlin-lsp@claude-plugins-official": false})
    );
    assert_eq!(value["autoMemoryEnabled"], json!(false));
    let worker = generated(&Value::Object(Map::new()), &off);
    assert!(worker.get("env").is_none() && worker.get("theme").is_none());
    assert_eq!(worker["skillOverrides"], value["skillOverrides"]);
    // No switch, no change.
    assert_eq!(generated(&base, &[]), base);
}

/// A home with the plugin's settings, the seeded catalog and its reading, and a 3d folder chosen by the rules.
fn opened(name: &str) -> (PathBuf, PathBuf, Outcome) {
    let (dir, _config, _catalog, reading) = seeded(name);
    std::fs::create_dir_all(dir.join("claude")).unwrap();
    std::fs::write(
        dir.join("claude").join("settings.json"),
        "{\"theme\": \"custom:nirvana-dark\", \"env\": {\"NIRVANA_ORCHESTRATOR\": \"1\"}}",
    )
    .unwrap();
    write_json(&preload_dir(&dir).join(READING_FILE), &reading).unwrap();
    let folder = fixture(&format!("{name}-3d"));
    touch(
        &folder,
        &[
            "part.3mf",
            "left.stl",
            "right.stl",
            "plate.gcode",
            "README.md",
        ],
    );
    let outcome = apply(&dir, &folder, None, OPEN_WALL).unwrap();
    (dir, folder, outcome)
}

fn file(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn apply_writes_both_files_and_nothing_in_the_folder() {
    let (dir, folder, outcome) = opened("apply");
    assert_eq!(outcome.name(), "3d-print");
    assert_eq!(outcome.settings, settings_path(&dir, &folder));
    let settings = file(&outcome.settings);
    assert_eq!(settings["theme"], "custom:nirvana-dark");
    assert_eq!(settings["skillOverrides"]["scholar-core"], "off");
    assert!(settings["skillOverrides"]
        .get("bambu-h2c-cosplay-print")
        .is_none());
    let worker = file(&outcome.worker_settings);
    assert!(worker.get("env").is_none());
    assert_eq!(worker["skillOverrides"], settings["skillOverrides"]);
    // §2: "The preset touches nothing in the owner's folder."
    assert!(!folder.join(".claude").exists());
    let line = outcome.toast_line().unwrap();
    assert!(
        line.starts_with(&format!(
            "3d-print preset \u{b7} {} off \u{b7} \u{2212}",
            outcome.off.len()
        )),
        "{line}"
    );
    assert!(line.ends_with(" a turn \u{b7} Preload p to undo"), "{line}");
}

#[test]
fn undo_regenerates_the_file_without_the_preset_and_p_again_puts_it_back() {
    let (dir, folder, outcome) = opened("undo");
    let with = file(&outcome.settings);
    let off = set_preset_in(&dir, &folder, false).unwrap();
    assert_eq!(off.name(), OFF);
    assert!(off.off.is_empty());
    assert_eq!(off.toast_line(), None);
    // §9: "`p` regenerates the file without the preset": the plugin's settings alone.
    assert_eq!(
        file(&off.settings),
        json!({"theme": "custom:nirvana-dark", "env": {"NIRVANA_ORCHESTRATOR": "1"}})
    );
    assert_eq!(file(&off.worker_settings), json!({}));
    let view = view_in(&dir, &folder);
    assert_eq!(view.name, OFF);
    assert!(view.off.is_empty());
    // The next open keeps everything on: the cached choice is reused and the record's undo kept.
    let reopened = apply(&dir, &folder, None, OPEN_WALL).unwrap();
    assert_eq!(reopened.name(), OFF);
    let back = set_preset_in(&dir, &folder, true).unwrap();
    assert_eq!(file(&back.settings), with);
}

#[test]
fn a_pinned_item_is_left_out_of_the_file_and_unpinning_puts_it_back_off() {
    let (dir, folder, outcome) = opened("pin");
    let switch = Switch::Skill("scholar-core".into());
    assert!(outcome.off.iter().any(|(k, _)| k == "skill:scholar-core"));
    let pinned = pin_on_in(&dir, &folder, &switch, true).unwrap();
    assert!(file(&pinned.settings)["skillOverrides"]
        .get("scholar-core")
        .is_none());
    assert_eq!(pinned.off.len(), outcome.off.len() - 1);
    assert_eq!(
        read_record(&dir, &folder).unwrap().pinned_on,
        ["skill:scholar-core"]
    );
    // Pinned twice is one pin.
    pin_on_in(&dir, &folder, &switch, true).unwrap();
    assert_eq!(read_record(&dir, &folder).unwrap().pinned_on.len(), 1);
    let unpinned = pin_on_in(&dir, &folder, &switch, false).unwrap();
    assert_eq!(file(&unpinned.settings), file(&outcome.settings));
    // A switch a preset never moves cannot be pinned.
    assert!(pin_on_in(&dir, &folder, &Switch::Hooks, true).is_err());
    // A pin survives a new fingerprint: it is the owner's.
    touch(&folder, &["more.stl", "more2.stl", "more3.stl"]);
    pin_on_in(&dir, &folder, &switch, true).unwrap();
    let rechosen = apply(&dir, &folder, None, OPEN_WALL).unwrap();
    assert_eq!(rechosen.record.pinned_on, ["skill:scholar-core"]);
}

#[test]
fn the_view_names_the_preset_its_off_switches_and_the_estimate() {
    let (dir, folder, outcome) = opened("view");
    let view = view_in(&dir, &folder);
    assert_eq!(view.name, "3d-print");
    assert!(view.reason.starts_with("rule: "), "{}", view.reason);
    assert_eq!(view.off.len(), outcome.off.len());
    assert!(view.off.iter().any(|(switch, tag)| *switch
        == Switch::McpServer("claude.ai Gmail".into())
        && tag == "personal"));
    assert_eq!(view.saved_tokens, outcome.saved);
    assert!(view.saved_tokens > 0);
    let never = view_in(&dir, &scratch("never-opened"));
    assert_eq!(never.name, "");
    assert!(never.off.is_empty());
}

// ---- the reading ------------------------------------------------------------------------------------------

#[test]
fn the_reading_lists_switchable_items_with_their_turn_cost() {
    let reading = reading_from(crate::nirvana_preload::FIXTURE, None, 9).expect("a reading");
    let keys: Vec<&str> = reading.items.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "mcp:bladebro",
            "mcp:claude.ai Gmail",
            "mcp:claude.ai Tavily",
            "mcp:plugin:github:github",
            "plugin:github@claude-plugins-official",
            "plugin:superpowers@superpowers-marketplace",
            "skill:benchmark",
            "skill:dataviz",
            "skill:docx",
        ]
    );
    assert_eq!(reading.items["skill:benchmark"].tokens, 112);
    // Deferred tool schemas are not a turn's cost: a connected server's share of instructions and the tool list.
    let share = (2_685 + 618) / 3;
    assert_eq!(reading.items["mcp:bladebro"].tokens, share);
    assert_eq!(
        reading.items["mcp:claude.ai Tavily"].tokens, 0,
        "needs-auth"
    );
    assert!(reading.items["mcp:bladebro"].about.contains("act"));
    assert_eq!(
        reading.items["plugin:superpowers@superpowers-marketplace"].tokens,
        76 + 350
    );
    assert_eq!(reading.total, 23_607);
}

#[test]
fn a_skills_description_comes_from_its_front_matter() {
    assert_eq!(
        skill_description("---\nname: x\ndescription: Slice and print on the H2C.\n---\nbody"),
        "Slice and print on the H2C."
    );
    assert_eq!(
        skill_description("---\nname: x\ndescription: >\n  Folded\n  lines.\nother: y\n---\n"),
        "Folded lines."
    );
    assert_eq!(skill_description("no front matter"), "");
}

#[test]
fn names_rules_and_core_match_as_written() {
    assert!(glob_match("skill:bambu*", "skill:bambu-h2c-cosplay-print"));
    assert!(glob_match(
        "skill:*-color-correction",
        "skill:Landscape-Color-Correction"
    ));
    assert!(glob_match(
        "plugin:*-lsp@*",
        "plugin:clangd-lsp@claude-plugins-official"
    ));
    assert!(!glob_match("skill:scholar-*", "skill:benchmark"));
    assert!(!glob_match(
        "skill:*-color-correction",
        "skill:batch-photo-processing"
    ));
    assert!(glob_match("mcp:claude.ai Gmail", "mcp:claude.ai gmail"));
    assert!(!glob_match("mcp:claude.ai Gmail", "mcp:claude.ai Gmail2"));
    let config = config();
    for core in [
        "mcp:bladebro",
        "mcp:plugin:context7:context7",
        "plugin:superpowers@claude-plugins-official",
        "plugin:context7@claude-plugins-official",
    ] {
        assert!(is_core(&config, core), "{core}");
    }
    assert!(!is_core(&config, "plugin:nirvana-cc@nirvana-c"));
    for switch in [
        Switch::Skill("x".into()),
        Switch::McpServer("claude.ai Gmail".into()),
        Switch::Plugin("a@b".into()),
        Switch::AutoMemory,
    ] {
        assert_eq!(switch_of(&key_of(&switch).unwrap()), Some(switch));
    }
}
