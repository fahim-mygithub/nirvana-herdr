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

/// Two servers whose tool names flatten alike: `a.b` and `a_b` both give `mcp__a_b__*`.
const CLASHING_JSON: &str = r#"{"mcpServers": {"a.b": {"command": "one"}, "a_b": {"command": "two"}, "xtool": {"command": "x"}}}"#;

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
        .starts_with("not JSON"));
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
    let read = seat_needs(&table, &root, &needs).unwrap();
    assert_eq!(read.rest, words(&["code"]));
    assert_eq!(read.servers, words(&["bambu-studio-beta"]));
    assert!(read
        .claude_only
        .unwrap()
        .starts_with("bambu-studio-beta drives"));
    // No `.mcp.json`: no server, so the need stays and today's unknown-word check refuses it.
    std::fs::remove_file(root.join(MCP_FILE)).unwrap();
    assert_eq!(
        seat_needs(&table, &root, &needs).unwrap(),
        Needs {
            rest: needs.clone(),
            ..Needs::default()
        }
    );
}

#[test]
fn a_broken_mcp_json_refuses_a_server_need_naming_the_file() {
    let home = crate::nirvana::with_test_home("gui-broken-mcp");
    let table = table();
    let root = project(&home.path, Some("{"));
    let file = root.join(MCP_FILE).display().to_string();
    // Review F5: the refusal names the file and says it does not parse, not "no seat has xtool".
    let why = seat_needs(&table, &root, &words(&["code", "xtool"])).unwrap_err();
    assert!(
        why.starts_with(&format!("{file} does not parse (not JSON")),
        "{why}"
    );
    assert!(
        why.ends_with("so the need xtool names none of its servers"),
        "{why}"
    );
    // An order whose needs are all capabilities is not refused by a file it does not use.
    assert_eq!(
        seat_needs(&table, &root, &words(&["code"])).unwrap(),
        Needs {
            rest: words(&["code"]),
            ..Needs::default()
        }
    );
    // A file that cannot be read at all says so too.
    std::fs::remove_file(root.join(MCP_FILE)).unwrap();
    std::fs::create_dir(root.join(MCP_FILE)).unwrap();
    let why = seat_needs(&table, &root, &words(&["xtool"])).unwrap_err();
    assert!(why.starts_with(&format!("cannot read {file}")), "{why}");
}

#[test]
fn two_servers_whose_tools_flatten_alike_refuse_the_order() {
    let home = crate::nirvana::with_test_home("gui-key-clash");
    let table = table();
    let root = project(&home.path, Some(CLASHING_JSON));
    let all = server_names(CLASHING_JSON).unwrap();
    assert_eq!(key_clash(&words(&["xtool"]), &all), None);
    let why = seat_needs(&table, &root, &words(&["a.b"])).unwrap_err();
    assert_eq!(
        why,
        ".mcp.json servers a.b and a_b both name their tools mcp__a_b__*, so a grant for one is a grant for both; \
         rename one of them"
    );
    assert!(seat_needs(&table, &root, &words(&["xtool"])).is_ok());
    // At launch too: no config is written for it.
    let run_dir = root.join("run");
    let at_launch = write_order_config(&root, &run_dir, "o1", &words(&["a_b"])).unwrap_err();
    assert!(
        at_launch.starts_with(".mcp.json servers a_b and a.b"),
        "{at_launch}"
    );
    assert!(!order_config_path(&run_dir, "o1").exists());
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
fn a_worker_gets_its_orders_config_file_and_only_a_claude_server_order_does() {
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
    )
    .unwrap();
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
        )
        .unwrap(),
        today
    );
    let agy = vec!["agy".to_owned(), "--print=x".to_owned()];
    assert_eq!(
        order_argv(agy.clone(), &root, &run_dir, "o3", &needs, caps(), &mut out).unwrap(),
        agy
    );
    assert!(out.is_empty());
    assert!(!run_dir.join("mcp").join("o2.json").exists());
    // seats.toml refused at launch: the needs `.mcp.json` names are the servers, and one line says so.
    let mut out = Vec::new();
    let argv = order_argv(
        today.clone(),
        &root,
        &run_dir,
        "o5",
        &needs,
        Err("seats.toml is refused".into()),
        &mut out,
    )
    .unwrap();
    assert_eq!(argv.last().map(String::as_str), Some("mcp__xtool__*"));
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "seats.toml is refused (seats.toml is refused); the needs .mcp.json names are read as its servers\n\
         MCP servers for this order: xtool\n"
    );
}

#[test]
fn an_order_that_cannot_get_its_server_at_launch_does_not_start() {
    let home = crate::nirvana::with_test_home("gui-order-gone");
    // Seated for xtool; at launch `.mcp.json` defines only bambu.
    let root = project(
        &home.path,
        Some(r#"{"mcpServers": {"bambu": {"command": "node"}}}"#),
    );
    let file = root.join(MCP_FILE).display().to_string();
    let run_dir = root.join(".nirvana").join("runs").join("r1");
    let needs = words(&["code", "xtool"]);
    let launch = |order: &str| {
        let mut out = Vec::new();
        let got = order_argv(
            claude_argv(),
            &root,
            &run_dir,
            order,
            &needs,
            Ok(vec!["code"]),
            &mut out,
        );
        assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
        got.unwrap_err()
    };
    assert_eq!(
        launch("o1"),
        format!(
            "o1 was seated for MCP server xtool and cannot get it: {file} no longer defines xtool"
        )
    );
    // A file that does not parse, or is gone: the same refusal, with the cause.
    std::fs::write(root.join(MCP_FILE), "{").unwrap();
    let why = launch("o2");
    assert!(
        why.starts_with(&format!(
            "o2 was seated for MCP server xtool and cannot get it: {file} does not parse"
        )),
        "{why}"
    );
    std::fs::remove_file(root.join(MCP_FILE)).unwrap();
    assert_eq!(
        launch("o3"),
        format!("o3 was seated for MCP server xtool and cannot get it: {file} is gone")
    );
    assert!(!run_dir.join("mcp").exists());
}

#[test]
fn the_order_config_goes_when_the_worker_ends_and_a_failed_remove_says_so() {
    let home = crate::nirvana::with_test_home("gui-remove-config");
    let root = project(&home.path, Some(MCP_JSON));
    let run_dir = root.join(".nirvana").join("runs").join("r1");
    let file = write_order_config(&root, &run_dir, "o1", &words(&["bambu"])).unwrap();
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("fake-token-not-real"));
    let mut out = Vec::new();
    remove_order_config(&run_dir, "o1", &mut out);
    assert!(!file.exists());
    // Nothing there: nothing said.
    remove_order_config(&run_dir, "o1", &mut out);
    assert!(out.is_empty());
    // A remove that fails: one line naming the file.
    std::fs::create_dir_all(&file).unwrap();
    remove_order_config(&run_dir, "o1", &mut out);
    let line = String::from_utf8(out).unwrap();
    assert!(
        line.starts_with(&format!(
            "the order's MCP config stays at {}: ",
            file.display()
        )),
        "{line}"
    );
    assert_eq!(line.lines().count(), 1);
}

#[test]
fn every_server_order_brief_says_never_start_a_device_and_a_desktop_one_batches() {
    let brief = with_server_lines("Slice the jig.".into(), &words(&["bambu-studio-beta"]));
    assert_eq!(
        brief,
        "Slice the jig.\n\nNever start a print, a laser job, or a send to a device. Stop and report when that is the \
         next step.\nUse the `*_run` batch and `*_state`; read crops, not full screenshots; never start a print or a job."
    );
    assert!(with_server_lines("Engrave.".into(), &words(&["xtool"])).ends_with(BRIEF_LINE));
    // `bambu` is printer control, no desktop server: the device line alone.
    assert_eq!(
        with_server_lines("Print status.".into(), &words(&["bambu"])),
        format!("Print status.\n\n{DEVICE_LINE}")
    );
    assert_eq!(with_server_lines("No servers.".into(), &[]), "No servers.");
}

#[test]
fn seat_pick_reads_the_server_needs_and_takes_a_claude_seat() {
    let home = crate::nirvana::with_test_home("gui-seat-pick");
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    let root = project(&home.path, Some(MCP_JSON));
    let args = |needs: &str| words(&["--class", "sweep.mechanical", "--needs", needs]);
    let world = World::default();
    // Without a server need the class seats on agy: the rule has something to move.
    let (table, plain) = nirvana_seat::pick_args_for(&root, &args("code")).unwrap();
    assert_eq!(
        nirvana_seat::pick(&table, &world, &plain.request)
            .unwrap()
            .cli,
        "agy"
    );
    // `herdr seat pick --needs xtool`: the need is no capability, and only a claude seat takes it.
    let (table, picked) = nirvana_seat::pick_args_for(&root, &args("code,xtool")).unwrap();
    assert_eq!(picked.request.needs, words(&["code"]));
    let chosen = nirvana_seat::pick(&table, &world, &picked.request).unwrap();
    assert_eq!(chosen.cli, "claude", "{}", chosen.seat);
    // A `.mcp.json` that does not parse: `seat pick` says so.
    std::fs::write(root.join(MCP_FILE), "{").unwrap();
    let why = nirvana_seat::pick_args_for(&root, &args("xtool")).unwrap_err();
    assert!(why.contains("does not parse"), "{why}");
}

// ---- plan 31 P31a ----

#[test]
fn p31_a_send_ok_order_gets_the_send_line_in_place_of_the_device_line() {
    let desktop = vec!["bambu-studio-beta".to_owned()];
    let sent = with_brief_lines("# o\n".into(), &desktop, true);
    assert!(
        sent.ends_with(&format!("\n\n{SEND_LINE}\n{DESKTOP_SEND_LINE}")),
        "{sent}"
    );
    assert!(!sent.contains(DEVICE_LINE), "{sent}");
    assert!(!sent.contains("never start a print"), "{sent}");
    // A printer server is no desktop: the Send line alone.
    let printer = with_brief_lines("# o\n".into(), &["bambu".to_owned()], true);
    assert!(printer.ends_with(&format!("\n\n{SEND_LINE}")), "{printer}");
    // An approval with no server still carries the rule.
    let bare = with_brief_lines("# o\n".into(), &[], true);
    assert!(bare.ends_with(&format!("\n\n{SEND_LINE}")), "{bare}");
    // Without an approval every server order keeps the device line, and no order gets the Send line.
    let kept = with_brief_lines("# o\n".into(), &desktop, false);
    assert_eq!(kept, with_server_lines("# o\n".into(), &desktop));
    assert!(!kept.contains(SEND_LINE), "{kept}");
    assert_eq!(with_brief_lines("# o\n".into(), &[], false), "# o\n");
}

#[test]
fn p31_the_hint_names_the_server_when_the_order_names_its_app_and_needs_none() {
    let defined = server_names(MCP_JSON).unwrap();
    let hint = server_hint("# Slice in bambu studio, then save\n", &defined, &[]).expect("hint");
    assert!(
        hint.contains("Bambu Studio") && hint.contains("`--needs bambu-studio-beta`"),
        "{hint}"
    );
    let hint = server_hint("Open the job in xTool Studio\n", &defined, &[]).expect("hint");
    assert!(hint.contains("`--needs xtool`"), "{hint}");
    // A need naming a server, no app named, or a project with no such server: nothing.
    assert_eq!(
        server_hint("Bambu Studio", &defined, &["bambu".to_owned()]),
        None
    );
    assert_eq!(
        server_hint("Fit the nosepiece in Blender", &defined, &[]),
        None
    );
    assert_eq!(
        server_hint("Bambu Studio", &["bambu".to_owned()], &[]),
        None
    );
}

/// One transcript line, as Claude Code 2.1.311 records it.
fn p31_line(kind: &str, content: Value, extra: &[(&str, Value)]) -> String {
    let mut line = serde_json::json!({
        "type": kind,
        "timestamp": "2026-10-29T01:32:14.584Z",
        "message": {"role": kind, "content": content},
    });
    for (key, value) in extra {
        line[*key] = value.clone();
    }
    line.to_string()
}

#[test]
fn p31_a_send_ok_quote_is_found_only_in_the_owners_own_prompts() {
    let words = "Print it when ready";
    let not_owner = [
        // herdr's own wake digest, Claude Code's news of a background task, a `!` command and its output.
        p31_line(
            "user",
            Value::String(format!("herdr run dune-v5: {words}")),
            &[],
        ),
        p31_line(
            "user",
            Value::String(format!("<task-notification>{words}</task-notification>")),
            &[],
        ),
        p31_line(
            "user",
            Value::String(format!("<bash-input>echo {words}</bash-input>")),
            &[],
        ),
        p31_line(
            "user",
            Value::String(format!("<bash-stdout>{words}</bash-stdout>")),
            &[],
        ),
        // A tool's result, a meta line, a subagent's prompt, and Nirvana Claude's own words.
        p31_line(
            "user",
            serde_json::json!([{"type": "tool_result", "content": words}]),
            &[],
        ),
        p31_line(
            "user",
            Value::String(words.into()),
            &[("isMeta", Value::Bool(true))],
        ),
        p31_line(
            "user",
            Value::String(words.into()),
            &[("isSidechain", Value::Bool(true))],
        ),
        p31_line(
            "assistant",
            serde_json::json!([{"type": "text", "text": words}]),
            &[],
        ),
    ]
    .join("\n");
    let transcript = Path::new("C:/t/s.jsonl");
    let why = send_ok_in(&not_owner, words, transcript).unwrap_err();
    assert!(why.contains("none of the owner's prompts"), "{why}");
    let owner = format!(
        "{not_owner}\n{}",
        p31_line("user", Value::String(format!("ok. {words}")), &[])
    );
    let found = send_ok_in(&owner, &format!("  {words} "), transcript).expect("found");
    assert_eq!(found.quote, words);
    assert!(
        found.check.contains("2026-10-29T01:32:14.584Z") && found.check.contains("s.jsonl"),
        "{}",
        found.check
    );
    // Verbatim: another spelling is not the owner's words, and no words are no approval.
    assert!(send_ok_in(&owner, "print it when ready", transcript).is_err());
    assert!(send_ok_in(&owner, "  ", transcript).is_err());
}

#[test]
fn p31_verify_send_ok_reads_the_recorded_session_and_refuses_every_doubt() {
    let home = crate::nirvana::with_test_home("p31-verify-send-ok");
    let folder = home.path.join("project");
    std::fs::create_dir_all(&folder).unwrap();
    let words = "Print it when ready";
    assert!(verify_send_ok(None, &folder, words).is_err());
    let why = verify_send_ok(Some(&home.path), &folder, words).unwrap_err();
    assert!(
        why.contains("no Nirvana Claude session is recorded"),
        "{why}"
    );
    let transcript = home.path.join("t.jsonl");
    let record = crate::nirvana_session_record::Record {
        session_id: "s".into(),
        at: "2026-10-29T01:32:15Z".into(),
        transcript: transcript.display().to_string(),
    };
    crate::nirvana_session_record::write(&home.path, &folder, &record).unwrap();
    let why = verify_send_ok(Some(&home.path), &folder, words).unwrap_err();
    assert!(why.contains("cannot be read"), "{why}");
    std::fs::write(
        &transcript,
        p31_line("user", Value::String(words.into()), &[]) + "\n",
    )
    .unwrap();
    let found = verify_send_ok(Some(&home.path), &folder, words).expect("found");
    assert_eq!(found.quote, words);
}

/// Plan 31 §2.1's measurement, run by hand: `P31_ORDERS` names a TSV of `run, order, cli, class, gui|-, file` for
/// recorded orders, and the hint is read on each file's order text (herdr's footer cut off) against
/// `bambu-studio-beta`, the server those orders' project defines. It prints each flagged order and the totals.
#[test]
#[ignore]
fn p31_measure_the_hint_on_recorded_orders() {
    let list = std::env::var("P31_ORDERS").expect("P31_ORDERS");
    let defined = vec!["bambu-studio-beta".to_owned()];
    let (mut orders, mut gui, mut flagged, mut flagged_gui) = (0, 0, 0, 0);
    for line in std::fs::read_to_string(list).unwrap().lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        let text = std::fs::read_to_string(cols[5]).unwrap();
        let text = text.split("\n\n---\nFrom herdr").next().unwrap_or("");
        let is_gui = cols[4] == "gui";
        let hint = server_hint(text, &defined, &[]);
        orders += 1;
        gui += usize::from(is_gui);
        if hint.is_some() {
            flagged += 1;
            flagged_gui += usize::from(is_gui);
        }
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            cols[0],
            cols[1],
            cols[2],
            cols[3],
            cols[4],
            if hint.is_some() { "FLAGGED" } else { "-" }
        );
    }
    println!("orders {orders}, gui {gui}, flagged {flagged}, flagged gui {flagged_gui}");
}
