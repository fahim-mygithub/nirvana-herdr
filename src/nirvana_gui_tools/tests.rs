use super::*;
use crate::nirvana_seat::{self, Effort, Request, World};
use crate::nirvana_worker::{argv, LaunchSpec};

/// Made-up servers in the shape of a project `.mcp.json` (names and commands only; the env values are fakes).
const MCP_JSON: &str = r#"{
  "mcpServers": {
    "bambu": {"type": "stdio", "command": "node", "args": ["C:\\fake\\bambu-printer-mcp\\dist\\index.js"],
              "env": {"PRINTER_HOST": "192.0.2.10", "BAMBU_TOKEN": "fake-token-not-real"}},
    "bambu-studio-beta": {"type": "stdio", "command": "uv",
                          "args": ["--directory", "C:\\fake\\studio-gui-mcp", "run", "python", "-m", "studio_gui_mcp"],
                          "env": {}},
    "xtool": {"type": "stdio", "command": "uv", "args": ["--directory", "C:\\fake\\xtool-mcp", "run", "xtool-mcp"],
              "env": {}},
    "cosplay_flow": {"command": "cosplay-flow-mcp", "env": {"COSPLAY_FLOW_MCP_BASE": "http://127.0.0.1:1"}}
  }
}"#;

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

fn table() -> Table {
    nirvana_seat::parse(nirvana_seat::DEFAULT).expect("the default table parses")
}

fn claude_argv() -> Vec<String> {
    argv(&LaunchSpec {
        cli: "claude".into(),
        model: "claude-sonnet-5-5".into(),
        effort: Effort::Medium,
        prompt: "Do the work order in o.md.".into(),
        resume: None,
        print_timeout_s: 0,
    })
    .unwrap()
}

/// A project root under the test home holding `text` as its `.mcp.json` (none when `None`).
fn project(home: &Path, text: Option<&str>) -> PathBuf {
    let root = home.join("project");
    std::fs::create_dir_all(&root).unwrap();
    if let Some(text) = text {
        std::fs::write(root.join(MCP_FILE), text).unwrap();
    }
    root
}

#[test]
fn the_server_names_are_the_keys_of_mcp_servers() {
    assert_eq!(
        server_names(MCP_JSON).unwrap(),
        words(&["bambu", "bambu-studio-beta", "cosplay_flow", "xtool"])
    );
    assert!(server_names("{not json")
        .unwrap_err()
        .contains("is not JSON"));
    assert!(server_names(r#"{"servers": {}}"#)
        .unwrap_err()
        .contains("no mcpServers object"));
}

#[test]
fn a_need_names_a_server_exactly_and_never_a_capability() {
    let servers = words(&["bambu-studio-beta", "xtool", "browser"]);
    let caps = ["code", "browser"];
    // Exact: neither case nor a prefix names a server.
    assert_eq!(
        servers_among(
            &words(&["Bambu-Studio-Beta", "bambu-studio", "xtool ", "code"]),
            &caps,
            &servers
        ),
        Vec::<String>::new()
    );
    // A capability stays a capability even when a server has its name; a repeat is named once.
    assert_eq!(
        servers_among(
            &words(&["browser", "xtool", "bambu-studio-beta", "xtool"]),
            &caps,
            &servers
        ),
        words(&["xtool", "bambu-studio-beta"])
    );
}

#[test]
fn only_a_claude_seat_takes_a_server_order_and_a_desktop_server_says_why() {
    assert_eq!(claude_only(&[]), None);
    let desktop = claude_only(&words(&["bambu", "bambu-studio-beta"])).unwrap();
    assert!(
        desktop
            .starts_with("bambu-studio-beta drives a desktop window and agy panes cannot see it"),
        "{desktop}"
    );
    assert!(claude_only(&words(&["xtool"]))
        .unwrap()
        .starts_with("xtool drives a desktop window"));
    let other = claude_only(&words(&["bambu"])).unwrap();
    assert!(
        other.starts_with("only a claude worker takes a per-order --mcp-config, for bambu"),
        "{other}"
    );
}

#[test]
fn a_desktop_server_order_skips_every_agy_seat_and_is_refused_without_a_claude_seat() {
    let table = table();
    let world = World::default();
    let mut agy_won = false;
    for class in table.classes.iter().map(|class| class.id.clone()) {
        let plain = nirvana_seat::pick(&table, &world, &Request::new(&class));
        agy_won |= plain.as_ref().is_ok_and(|pick| pick.cli == "agy");
        let mut request = Request::new(&class);
        request.claude_only = claude_only(&words(&["bambu-studio-beta"]));
        if let Ok(pick) = nirvana_seat::pick(&table, &world, &request) {
            assert_eq!(pick.cli, "claude", "{class} seated {} on agy", pick.seat);
        }
    }
    // The rule had something to move: some class seats on agy without it.
    assert!(agy_won, "no class seats on agy in the default table");
    // No Claude seat free: the order is refused, and the reason names the window.
    let mut out = World::default();
    out.clis
        .insert("claude".into(), "claude not signed in".into());
    let mut request = Request::new("build.bounded");
    request.claude_only = claude_only(&words(&["bambu-studio-beta"]));
    let no = nirvana_seat::pick(&table, &out, &request).unwrap_err();
    assert!(
        no.reason
            .contains("bambu-studio-beta drives a desktop window and agy panes cannot see it"),
        "{}",
        no.reason
    );
}

#[test]
fn seat_needs_takes_the_server_needs_out_and_reads_the_projects_mcp_json() {
    let home = crate::nirvana::with_test_home("gui-seat-needs");
    let table = table();
    let needs = words(&["code", "bambu-studio-beta"]);
    let root = project(&home.path, Some(MCP_JSON));
    let (rest, why) = seat_needs(&table, &root, &needs);
    assert_eq!(rest, words(&["code"]));
    assert!(why.unwrap().starts_with("bambu-studio-beta drives"));
    // No `.mcp.json`, or one that is not JSON: no server, so the need stays and today's check refuses it.
    std::fs::remove_file(root.join(MCP_FILE)).unwrap();
    assert_eq!(seat_needs(&table, &root, &needs), (needs.clone(), None));
    std::fs::write(root.join(MCP_FILE), "{").unwrap();
    assert_eq!(seat_needs(&table, &root, &needs), (needs.clone(), None));
}

#[test]
fn the_order_config_holds_only_the_named_servers_copied_whole() {
    let config = order_config(MCP_JSON, &words(&["bambu-studio-beta", "bambu"])).unwrap();
    let servers = config["mcpServers"].as_object().unwrap();
    assert_eq!(servers.len(), 2);
    let full: Value = serde_json::from_str(MCP_JSON).unwrap();
    assert_eq!(servers["bambu"], full["mcpServers"]["bambu"]);
    assert_eq!(
        servers["bambu-studio-beta"],
        full["mcpServers"]["bambu-studio-beta"]
    );
    assert!(order_config(MCP_JSON, &words(&["blender"]))
        .unwrap_err()
        .contains("no server \"blender\""));
}

#[test]
fn the_argv_keeps_strict_mcp_and_adds_the_config_and_the_servers_tools() {
    let file = Path::new("C:\\p\\.nirvana\\runs\\r1\\mcp\\o1.json");
    let argv = with_servers(
        claude_argv(),
        file,
        &words(&["bambu-studio-beta", "claude.ai Gmail"]),
    );
    assert!(argv.contains(&"--strict-mcp-config".to_owned()));
    let at = argv.iter().position(|arg| arg == "--mcp-config").unwrap();
    assert_eq!(argv[at + 1], file.display().to_string());
    let tools = argv.iter().position(|arg| arg == "--allowedTools").unwrap();
    assert_eq!(
        argv[tools + 1],
        "mcp__bambu-studio-beta__*,mcp__claude_ai_Gmail__*"
    );
    assert_eq!(
        argv.iter().filter(|arg| *arg == "--allowedTools").count(),
        1
    );
}

#[test]
fn a_worker_gets_its_orders_config_file_and_any_error_keeps_todays_argv() {
    let home = crate::nirvana::with_test_home("gui-order-argv");
    let root = project(&home.path, Some(MCP_JSON));
    let run_dir = root.join(".nirvana").join("runs").join("r1");
    let caps = || Ok(vec!["code", "tools"]);
    let today = claude_argv();
    let needs = words(&["code", "xtool"]);
    let mut out = Vec::new();
    let argv = order_argv(
        today.clone(),
        &root,
        &run_dir,
        "o1",
        &needs,
        caps(),
        &mut out,
    );
    let file = run_dir.join("mcp").join("o1.json");
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(
        written["mcpServers"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["xtool"]
    );
    assert_eq!(&argv[..today.len()], &today[..]);
    assert_eq!(
        &argv[today.len()..],
        [
            "--mcp-config".to_owned(),
            file.display().to_string(),
            "--allowedTools".to_owned(),
            "mcp__xtool__*".to_owned()
        ]
    );
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "MCP servers for this order: xtool\n"
    );
    // No server need, or an agy worker: today's argv, no file, no line.
    let mut out = Vec::new();
    assert_eq!(
        order_argv(
            today.clone(),
            &root,
            &run_dir,
            "o2",
            &words(&["code"]),
            caps(),
            &mut out
        ),
        today
    );
    let agy = vec!["agy".to_owned(), "--print=x".to_owned()];
    assert_eq!(
        order_argv(agy.clone(), &root, &run_dir, "o3", &needs, caps(), &mut out),
        agy
    );
    assert!(out.is_empty());
    assert!(!run_dir.join("mcp").join("o2.json").exists());
    // A `.mcp.json` that is not JSON, or seats.toml refused: today's argv and one line.
    std::fs::write(root.join(MCP_FILE), "{").unwrap();
    let mut out = Vec::new();
    assert_eq!(
        order_argv(
            today.clone(),
            &root,
            &run_dir,
            "o4",
            &needs,
            caps(),
            &mut out
        ),
        today
    );
    let line = String::from_utf8(out).unwrap();
    assert!(
        line.starts_with("the worker starts without its MCP servers: .mcp.json is not JSON"),
        "{line}"
    );
    assert_eq!(line.lines().count(), 1);
    let mut out = Vec::new();
    assert_eq!(
        order_argv(
            today.clone(),
            &root,
            &run_dir,
            "o5",
            &needs,
            Err("seats.toml is refused".into()),
            &mut out
        ),
        today
    );
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "the worker starts without its MCP servers: seats.toml is refused\n"
    );
}

#[test]
fn a_desktop_order_brief_gets_the_one_line() {
    let brief = with_desktop_line(
        "Slice the jig.".into(),
        &words(&["code", "bambu-studio-beta"]),
    );
    assert_eq!(
        brief,
        "Slice the jig.\n\nUse the `*_run` batch and `*_state`; read crops, not full screenshots; never start a print or a job."
    );
    assert!(with_desktop_line("Engrave.".into(), &words(&["xtool"])).ends_with(BRIEF_LINE));
    assert_eq!(
        with_desktop_line("Print status.".into(), &words(&["bambu", "code"])),
        "Print status."
    );
}
