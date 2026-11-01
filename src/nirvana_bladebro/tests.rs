use super::*;

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

/// A folder with no `.agents/mcp_config.json` of its own.
fn none() -> PathBuf {
    std::env::temp_dir().join("p35c-blade-no-workspace")
}

/// R1: decision 0043's measured recipe, field by field, with the order's own home.
#[test]
fn p35c_the_definition_is_decision_0043s_recipe_with_the_orders_own_home() {
    let run_dir = Path::new(r"C:\p\.nirvana\runs\r1");
    let home = home(run_dir, "o1");
    assert_eq!(home, run_dir.join("blade").join("o1"));
    let given = definition(&home);
    if cfg!(windows) {
        assert_eq!(given["command"], "cmd");
        assert_eq!(given["args"], serde_json::json!(["/c", "bladebro", "mcp"]));
    } else {
        assert_eq!(given["command"], "bladebro");
        assert_eq!(given["args"], serde_json::json!(["mcp"]));
    }
    assert_eq!(
        given["env"],
        serde_json::json!({
            "BLADE_HOME": home.display().to_string(),
            "BLADE_CHROME_FLAGS": "--headless=new",
            "BLADE_LANE": "agent",
            "BLADE_IDLE_TIMEOUT": "60",
            "BLADE_NO_UPDATE_CHECK": "1",
            "BLADE_NO_WARMING": "1",
            "BLADE_PLAIN": "1",
        })
    );
    // Only the keys herdr carries: agy's translation refuses any other (plan 35 §3.2), so agy takes it as it is.
    assert_eq!(
        given.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["args", "command", "env"]
    );
    assert_eq!(crate::nirvana_agy_tools::refusal(SERVER, &given), None);
    // The home a verbatim path names is named one way (P35b's `plain_path`).
    let verbatim = definition(Path::new(r"\\?\C:\p\blade\o1"));
    assert_eq!(verbatim["env"]["BLADE_HOME"], r"C:\p\blade\o1");
}

/// R1: a need `browser` or `bladebro` grants it on claude and agy; any other need, or another CLI, does not; not on
/// PATH grants nothing.
#[test]
fn p35c_a_browser_need_grants_bladebro_on_either_cli_and_not_on_path_grants_nothing() {
    for cli in ["claude", "agy"] {
        for needs in [
            &["browser"][..],
            &["bladebro"],
            &["code", "BROWSER"],
            &["pdf, bladebro"],
        ] {
            assert_eq!(
                grant_with(cli, &words(needs), &[], false, true),
                Grant::Yes,
                "{cli} {needs:?}"
            );
            assert_eq!(
                grant_with(cli, &words(needs), &[], false, false),
                Grant::Missing,
                "{cli} {needs:?}"
            );
        }
        for needs in [
            &[][..],
            &["code"],
            &["agent-browser"],
            &["no-browser"],
            &["bladebro2"],
        ] {
            assert_eq!(
                grant_with(cli, &words(needs), &[], false, true),
                Grant::No,
                "{cli} {needs:?}"
            );
        }
    }
    assert_eq!(
        grant_with("codex", &words(&["browser"]), &[], false, true),
        Grant::No
    );
    TEST_INSTALLED.set(Some(false));
    assert_eq!(
        grant("agy", &words(&["browser"]), &[], &none()),
        Grant::Missing
    );
    TEST_INSTALLED.set(Some(true));
    assert_eq!(grant("agy", &words(&["browser"]), &[], &none()), Grant::Yes);
    TEST_INSTALLED.set(None);
}

/// Item 2: the worker's shell gets the `BLADE_*` of the bladebro its argv's config names, on either CLI, and nothing
/// when the config has none.
#[test]
fn p35c_the_worker_env_is_read_from_the_config_its_argv_names() {
    let home_guard = crate::nirvana::with_test_home("p35c-worker-env");
    let run_dir = home_guard.path.join("runs").join("r1");
    let blade = home(&run_dir, "o1");
    let expected: Vec<(String, String)> = {
        let mut env: Vec<(String, String)> = env(&blade)
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect();
        env.sort();
        env
    };
    // Review round S1: a token-like key another hand put in the bladebro entry stays out of the shell too.
    let mut ours = definition(&blade);
    ours["env"]["GITHUB_TOKEN"] = Value::from("fake-not-real");
    let config = serde_json::json!({"mcpServers": {
        "bambu": {"command": "node", "env": {"BAMBU_TOKEN": "fake-not-real"}},
        "bladebro": ours,
    }});
    let mcp = run_dir.join("mcp").join("o1.json");
    std::fs::create_dir_all(mcp.parent().unwrap()).unwrap();
    std::fs::write(&mcp, config.to_string()).unwrap();
    let claude = words(&[
        "claude",
        "-p",
        "x",
        "--mcp-config",
        &mcp.display().to_string(),
        "--allowedTools",
        "y",
    ]);
    let mut got = worker_env(&claude);
    got.sort();
    assert_eq!(got, expected);
    // Another server's env never reaches the shell, nor a key of this one that is not `BLADE_*`.
    assert!(!got.iter().any(|(key, _)| key == "BAMBU_TOKEN"));
    assert!(!got.iter().any(|(key, _)| key == "GITHUB_TOKEN"));
    let tools = run_dir.join("agy").join("o1");
    let agy_file = tools.join(crate::nirvana_agy_tools::MCP_CONFIG);
    std::fs::create_dir_all(agy_file.parent().unwrap()).unwrap();
    std::fs::write(&agy_file, config.to_string()).unwrap();
    let agy = words(&[
        "agy",
        "--add-dir",
        &tools.display().to_string(),
        "--print=x",
    ]);
    let mut got = worker_env(&agy);
    got.sort();
    assert_eq!(got, expected);
    // No bladebro in the config, no config, no flag, another CLI: nothing.
    std::fs::write(&mcp, r#"{"mcpServers": {"bambu": {"command": "node"}}}"#).unwrap();
    assert!(worker_env(&claude).is_empty());
    std::fs::remove_file(&agy_file).unwrap();
    assert!(worker_env(&agy).is_empty());
    assert!(worker_env(&words(&["claude", "-p", "x"])).is_empty());
    assert!(worker_env(&words(&[
        "codex",
        "--mcp-config",
        &mcp.display().to_string()
    ]))
    .is_empty());
}

/// Item 4: the home goes; one that will not go is tried `REMOVE_TRIES` times and said in one line; none is nothing.
#[test]
fn p35c_the_home_is_removed_and_a_home_that_stays_is_said_once() {
    let home_guard = crate::nirvana::with_test_home("p35c-remove-home");
    let run_dir = home_guard.path.join("runs").join("r1");
    let blade = home(&run_dir, "o1");
    std::fs::create_dir_all(blade.join("profile").join("Default")).unwrap();
    std::fs::write(blade.join("profile").join("Default").join("Cookies"), "x").unwrap();
    let mut out = Vec::new();
    remove_home(&run_dir, "o1", &mut out);
    assert!(!blade.exists());
    assert!(out.is_empty());
    remove_home(&run_dir, "o1", &mut out);
    assert!(out.is_empty());
    // Review round N3: a home that is not there is never waited on, whatever the pause, so the removal before every
    // attempt and in the sweep costs nothing for an order with no home.
    let started = std::time::Instant::now();
    remove_home_with(&blade, 3, Duration::from_secs(5), &mut out);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    assert!(out.is_empty());
    // Review round N2: the tries and the pause of `remove_home` itself, counted by its line and the time it takes, on
    // a file where the folder was (`remove_dir_all` fails each time).
    std::fs::create_dir_all(blade.parent().unwrap()).unwrap();
    std::fs::write(&blade, "not a folder").unwrap();
    let started = std::time::Instant::now();
    remove_home(&run_dir, "o1", &mut out);
    assert!(
        started.elapsed() >= Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    let line = String::from_utf8(std::mem::take(&mut out)).unwrap();
    assert!(line.contains(" after 3 tries: "), "{line}");
    std::fs::remove_file(&blade).unwrap();
    // A file where the folder was: `remove_dir_all` fails each time. The pause is short here; the tries are counted
    // by the time they take.
    std::fs::create_dir_all(blade.parent().unwrap()).unwrap();
    std::fs::write(&blade, "not a folder").unwrap();
    let started = std::time::Instant::now();
    remove_home_with(&blade, 3, Duration::from_millis(150), &mut out);
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "{:?}",
        started.elapsed()
    );
    let line = String::from_utf8(out).unwrap();
    assert!(
        line.starts_with(&format!(
            "the order's bladebro home stays at {} after 3 tries: ",
            blade.display()
        )),
        "{line}"
    );
    assert_eq!(line.lines().count(), 1);
}

/// Review round: a profile that denies bladebro whole wins over the need, on both CLIs; one tool denied leaves the
/// grant (the CLI's own deny holds that tool back).
#[test]
fn p35c_a_profile_denying_bladebro_wins_over_the_grant() {
    for cli in ["claude", "agy"] {
        for denied in [
            &["mcp__bladebro__*"][..],
            &["mcp__bladebro"],
            &["Write", " MCP__Bladebro__* "],
            &["Write,mcp__bladebro"],
        ] {
            assert_eq!(
                grant_with(cli, &words(&["browser"]), &words(denied), false, true),
                Grant::Denied,
                "{cli} {denied:?}"
            );
        }
        for denied in [
            &["mcp__bladebro__eval"][..],
            &["mcp__bladebro2__*"],
            &["Write"],
        ] {
            assert_eq!(
                grant_with(cli, &words(&["browser"]), &words(denied), false, true),
                Grant::Yes,
                "{cli} {denied:?}"
            );
        }
    }
    // No browser need: a denial is nothing to say.
    assert_eq!(
        grant_with(
            "claude",
            &words(&["code"]),
            &words(&["mcp__bladebro"]),
            false,
            true
        ),
        Grant::No
    );
}

/// Review round S2: an agy workspace whose own `.agents/mcp_config.json` names bladebro is not granted herdr's; a
/// claude worker never loads that file (`--strict-mcp-config`), so it is granted.
#[test]
fn p35c_an_agy_workspace_naming_bladebro_is_not_granted() {
    let home_guard = crate::nirvana::with_test_home("p35c-workspace");
    let cwd = home_guard.path.join("project");
    let file = cwd.join(crate::nirvana_agy_tools::MCP_CONFIG);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        &file,
        r#"{"mcpServers": {"bladebro": {"command": "bladebro"}}}"#,
    )
    .unwrap();
    let browser = words(&["browser"]);
    TEST_INSTALLED.set(Some(true));
    assert!(workspace_names(&cwd));
    assert_eq!(grant("agy", &browser, &[], &cwd), Grant::Native);
    assert_eq!(grant("claude", &browser, &[], &cwd), Grant::Yes);
    assert_eq!(grant_with("agy", &browser, &[], true, true), Grant::Native);
    assert_eq!(grant_with("claude", &browser, &[], true, true), Grant::Yes);
    // Another server only, or a file that does not parse: granted.
    std::fs::write(&file, r#"{"mcpServers": {"bambu": {"command": "node"}}}"#).unwrap();
    assert!(!workspace_names(&cwd));
    assert_eq!(grant("agy", &browser, &[], &cwd), Grant::Yes);
    std::fs::write(&file, "{").unwrap();
    assert!(!workspace_names(&cwd));
    TEST_INSTALLED.set(None);
}

/// Review round S3: the launch grants only what the brief was told and what is true now, and says each mismatch.
#[test]
fn p35c_the_launch_grants_only_what_the_brief_was_told() {
    let cwd = none();
    let said = |grant: Grant, told: Option<bool>| {
        let mut out = Vec::new();
        let given = at_launch(grant, told, &cwd, &mut out);
        (given, String::from_utf8(out).unwrap())
    };
    assert_eq!(said(Grant::Yes, Some(true)), (true, String::new()));
    assert_eq!(said(Grant::Yes, None), (true, String::new()));
    assert_eq!(
        said(Grant::Yes, Some(false)),
        (false, format!("{UNTOLD_LINE}\n"))
    );
    assert_eq!(
        said(Grant::Missing, Some(true)),
        (false, format!("{MISSING_LINE}\n{TOLD_LINE}\n"))
    );
    assert_eq!(
        said(Grant::Missing, Some(false)),
        (false, format!("{MISSING_LINE}\n"))
    );
    assert_eq!(
        said(Grant::Denied, None),
        (false, format!("{DENIED_LINE}\n"))
    );
    assert_eq!(
        said(Grant::No, Some(true)),
        (false, format!("{TOLD_LINE}\n"))
    );
    assert_eq!(said(Grant::No, Some(false)), (false, String::new()));
    let (given, line) = said(Grant::Native, None);
    assert!(!given);
    assert!(line.contains("names a bladebro of its own"), "{line}");
    assert_eq!(line.lines().count(), 1);
}

/// Review round: the launch reads the brief's line and the profile copy's deny list back from seating; a lesson
/// review never takes the profile its quoted order names.
#[test]
fn p35c_the_seat_is_read_back_from_the_brief_and_the_profile_copy() {
    let home_guard = crate::nirvana::with_test_home("p35c-seat-read");
    let run_dir = home_guard.path.join("runs").join("r1");
    let profile = "---\ndescription: Reads pages without a browser.\nclass: research.web\n\
                   disallowed_tools: [Write, mcp__bladebro__*]\n---\nRead only.\n";
    let sha = crate::nirvana_profiles::sha256_hex(profile.as_bytes());
    let copy = crate::nirvana_profiles::copy_path(&run_dir, "reader", &sha);
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    std::fs::write(&copy, profile).unwrap();
    let brief = home_guard.path.join("o1.md");
    std::fs::write(
        &brief,
        format!(
            "{} reader\n{} {sha}\n\nFind the price.\n\n{}",
            crate::nirvana_profiles::LINE,
            crate::nirvana_profiles::SHA_LINE,
            crate::nirvana_lean::BROWSER_LINE
        ),
    )
    .unwrap();
    let seat = Seat::read(&run_dir, "research.web", &brief);
    assert_eq!(seat.denied, words(&["Write", "mcp__bladebro__*"]));
    assert_eq!(seat.told, Some(true));
    let review = Seat::read(&run_dir, crate::nirvana_review::CLASS, &brief);
    assert!(review.denied.is_empty());
    std::fs::write(&brief, "Find the price.\n").unwrap();
    assert_eq!(
        Seat::read(&run_dir, "research.web", &brief),
        Seat {
            denied: Vec::new(),
            told: Some(false)
        }
    );
    assert_eq!(
        Seat::read(&run_dir, "research.web", &home_guard.path.join("gone.md")),
        Seat::default()
    );
}
