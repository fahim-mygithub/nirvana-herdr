use super::*;

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
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
                grant_with(cli, &words(needs), true),
                Grant::Yes,
                "{cli} {needs:?}"
            );
            assert_eq!(
                grant_with(cli, &words(needs), false),
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
                grant_with(cli, &words(needs), true),
                Grant::No,
                "{cli} {needs:?}"
            );
        }
    }
    assert_eq!(grant_with("codex", &words(&["browser"]), true), Grant::No);
    TEST_INSTALLED.set(Some(false));
    assert_eq!(grant("agy", &words(&["browser"])), Grant::Missing);
    TEST_INSTALLED.set(Some(true));
    assert_eq!(grant("agy", &words(&["browser"])), Grant::Yes);
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
    let config = serde_json::json!({"mcpServers": {
        "bambu": {"command": "node", "env": {"BAMBU_TOKEN": "fake-not-real"}},
        "bladebro": definition(&blade),
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
    // Another server's env never reaches the shell.
    assert!(!got.iter().any(|(key, _)| key == "BAMBU_TOKEN"));
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
    assert_eq!(REMOVE_TRIES, 3);
    assert_eq!(REMOVE_PAUSE, Duration::from_secs(1));
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
