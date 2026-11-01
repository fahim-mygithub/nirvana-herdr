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

fn defs() -> Map<String, Value> {
    defined(MCP_JSON).unwrap()
}

#[test]
fn only_a_claude_seat_takes_a_desktop_server_order_and_says_why() {
    assert_eq!(claude_only(&[], &defs()), None);
    let desktop = claude_only(&words(&["bambu", "bambu-studio-beta"]), &defs()).unwrap();
    assert!(
        desktop
            .starts_with("bambu-studio-beta drives a desktop window and agy panes cannot see it"),
        "{desktop}"
    );
    assert!(claude_only(&words(&["xtool"]), &defs())
        .unwrap()
        .starts_with("xtool drives a desktop window"));
}

/// Plan 35 P35b (R4): a non-desktop server no longer keeps an order on claude; a definition agy cannot take does.
#[test]
fn p35b_a_non_desktop_server_order_may_seat_on_agy_unless_agy_cannot_take_its_definition() {
    assert_eq!(claude_only(&words(&["bambu"]), &defs()), None);
    assert_eq!(
        claude_only(&words(&["cosplay_flow", "bambu"]), &defs()),
        None
    );
    let odd: Map<String, Value> = serde_json::from_str(
        r#"{"v": {"command": "x", "env": {"TOKEN": "${TOKEN}"}}, "s": {"type": "sse", "url": "http://h/sse"},
            "k": {"command": "x", "disabled": true}, "h": {"type": "http", "url": "http://h/mcp"}}"#,
    )
    .unwrap();
    let why = |name: &str| claude_only(&words(&[name]), &odd).unwrap_or_default();
    assert!(
        why("v").starts_with("only a claude worker can take v: v uses ${...}"),
        "{}",
        why("v")
    );
    assert!(
        why("s").contains("s is type sse, which no agy probe measured"),
        "{}",
        why("s")
    );
    assert!(why("k").contains("the key \"disabled\""), "{}", why("k"));
    assert_eq!(claude_only(&words(&["h"]), &odd), None);
    // Through `seat_needs`, the public path the seat rule reads.
    let home = crate::nirvana::with_test_home("p35b-seat-needs");
    let root = project(&home.path, Some(MCP_JSON));
    let read = seat_needs(&table(), &root, &words(&["code", "bambu"])).unwrap();
    assert_eq!((read.servers, read.claude_only), (words(&["bambu"]), None));
    let desktop = seat_needs(&table(), &root, &words(&["xtool"])).unwrap();
    assert!(desktop
        .claude_only
        .unwrap()
        .starts_with("xtool drives a desktop window"));
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
        request.claude_only = claude_only(&words(&["bambu-studio-beta"]), &defs());
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
    request.claude_only = claude_only(&words(&["bambu-studio-beta"]), &defs());
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
    // An agy worker with no server need: today's argv. With a desktop server (plan 35: the rest reach agy) it fails.
    let agy = vec!["agy".to_owned(), "--print=x".to_owned()];
    let code = words(&["code"]);
    assert_eq!(
        order_argv(
            agy.clone(),
            &root,
            &root,
            &run_dir,
            "o3",
            &code,
            caps(),
            &mut out
        )
        .unwrap(),
        agy
    );
    let why = order_argv(
        agy.clone(),
        &root,
        &root,
        &run_dir,
        "o4",
        &needs,
        caps(),
        &mut out,
    )
    .unwrap_err();
    assert!(
        why.starts_with(
            "o4 was seated for MCP server xtool and cannot get it: xtool drives a desktop window"
        ),
        "{why}"
    );
    assert!(!crate::nirvana_agy_tools::tools_dir(&run_dir, "o4").exists());
    assert!(out.is_empty());
    assert!(!run_dir.join("mcp").join("o2.json").exists());
    // seats.toml refused at launch: the needs `.mcp.json` names are the servers, and one line says so.
    let mut out = Vec::new();
    let argv = order_argv(
        today.clone(),
        &root,
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
    // Review N3: an approval with no server gets no Send line, as it gets no device line.
    assert_eq!(with_brief_lines("# o\n".into(), &[], true), "# o\n");
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

/// Review round: a bare "Studio" counts as the app when the project defines exactly one desktop server, and as
/// neither when it defines both; a word that only holds "studio" is not it.
#[test]
fn p31_a_bare_studio_names_the_one_desktop_server() {
    let bambu_only = vec!["bambu".to_owned(), "bambu-studio-beta".to_owned()];
    let hint =
        server_hint("Slice the hook in Studio, then save\n", &bambu_only, &[]).expect("hint");
    assert!(hint.contains("`--needs bambu-studio-beta`"), "{hint}");
    let xtool_only = vec!["xtool".to_owned()];
    let hint = server_hint("Open the dial in studio.", &xtool_only, &[]).expect("hint");
    assert!(hint.contains("`--needs xtool`"), "{hint}");
    let both = server_names(MCP_JSON).unwrap();
    assert_eq!(server_hint("Slice it in Studio", &both, &[]), None);
    assert_eq!(
        server_hint("Render it in a studiolike light", &bambu_only, &[]),
        None
    );
    assert_eq!(
        server_hint("Slice it in Studio", &["bambu".to_owned()], &[]),
        None
    );
    assert_eq!(
        server_hint(
            "Slice it in Studio",
            &bambu_only,
            &["bambu-studio-beta".to_owned()]
        ),
        None
    );
}

/// The time the B1 tests read the transcripts at: 2026-10-29T04:00:00Z, 2 h 28 min after `P31_AT`.
const P31_NOW: u64 = 1_793_246_400;
const P31_AT: &str = "2026-10-29T01:32:14.584Z";

/// The owner's approval in the B1 tests: 7 words, 36 characters.
const P31_WORDS: &str = "Print the nosepiece when it is ready";

/// One transcript line, as Claude Code 2.1.310 records it: a prompt the owner typed carries `origin: human`.
fn p31_line(kind: &str, content: Value, extra: &[(&str, Value)]) -> String {
    let mut line = serde_json::json!({
        "type": kind,
        "timestamp": P31_AT,
        "message": {"role": kind, "content": content},
    });
    for (key, value) in extra {
        line[*key] = value.clone();
    }
    line.to_string()
}

fn p31_human() -> (&'static str, Value) {
    ("origin", serde_json::json!({"kind": "human"}))
}

fn p31_owner(text: &str) -> String {
    p31_line("user", Value::String(text.into()), &[p31_human()])
}

#[test]
fn p31_a_send_ok_quote_is_found_only_in_the_owners_own_prompts() {
    let words = P31_WORDS;
    let human = p31_human();
    let not_owner = [
        // herdr's own wake digest is typed in the pane, so it is `human` too; its lead tells it.
        p31_line(
            "user",
            Value::String(format!("herdr run dune-v5: {words}")),
            std::slice::from_ref(&human),
        ),
        // Claude Code's news of a background task, a `!` command and its output.
        p31_line(
            "user",
            Value::String(format!("<task-notification>{words}</task-notification>")),
            &[("origin", serde_json::json!({"kind": "task-notification"}))],
        ),
        p31_line(
            "user",
            Value::String(format!("<bash-input>echo {words}</bash-input>")),
            std::slice::from_ref(&human),
        ),
        p31_line(
            "user",
            Value::String(format!("<bash-stdout>{words}</bash-stdout>")),
            std::slice::from_ref(&human),
        ),
        // A tool's result, a meta line, a subagent's prompt, and Nirvana Claude's own words.
        p31_line(
            "user",
            serde_json::json!([{"type": "tool_result", "content": words}]),
            std::slice::from_ref(&human),
        ),
        p31_line(
            "user",
            Value::String(words.into()),
            &[human.clone(), ("toolUseResult", serde_json::json!({}))],
        ),
        p31_line(
            "user",
            Value::String(words.into()),
            &[
                human.clone(),
                ("sourceToolUseID", Value::String("toolu_1".into())),
            ],
        ),
        p31_line(
            "user",
            Value::String(words.into()),
            &[human.clone(), ("isMeta", Value::Bool(true))],
        ),
        p31_line(
            "user",
            Value::String(words.into()),
            &[human.clone(), ("isSidechain", Value::Bool(true))],
        ),
        // Claude Code's compact summary is shown in the transcript only; `owner_prompt_in` reads no such flag.
        p31_line(
            "user",
            Value::String(words.into()),
            &[
                human.clone(),
                ("isVisibleInTranscriptOnly", Value::Bool(true)),
            ],
        ),
        p31_line(
            "assistant",
            serde_json::json!([{"type": "text", "text": words}]),
            std::slice::from_ref(&human),
        ),
        // A user line with no `origin`: what an agent-written prompt looks like, so it is not the owner's.
        p31_line("user", Value::String(words.into()), &[]),
    ]
    .join("\n");
    let transcript = Path::new("C:/t/s.jsonl");
    let why = send_ok_in(&not_owner, words, transcript, P31_NOW).unwrap_err();
    assert!(why.contains("none of the owner's own prompts"), "{why}");
    let owner = format!("{not_owner}\n{}", p31_owner(&format!("ok. {words}")));
    let found = send_ok_in(&owner, &format!("  {words} "), transcript, P31_NOW).expect("found");
    assert_eq!(found.quote, words);
    assert_eq!(found.prompt, format!("ok. {words}"));
    assert_eq!(found.at, P31_AT);
    assert_eq!(found.transcript, "C:/t/s.jsonl");
    assert!(
        found.check.contains(P31_AT) && found.check.contains("s.jsonl"),
        "{}",
        found.check
    );
    // Verbatim: another spelling is not the owner's words, and no words are no approval.
    assert!(send_ok_in(&owner, &words.to_lowercase(), transcript, P31_NOW).is_err());
    assert!(send_ok_in(&owner, "  ", transcript, P31_NOW).is_err());
}

/// Review B1.1: "ok" and a three-word quote refuse, though the owner typed them.
#[test]
fn p31_a_short_quote_refuses_though_the_owner_typed_it() {
    let transcript = Path::new("C:/t/s.jsonl");
    let text = [
        p31_owner("ok"),
        p31_owner("Send it now"),
        p31_owner("Send the print now"),
        p31_owner("Approved: sendthenosepiece"),
        p31_owner(P31_WORDS),
    ]
    .join("\n");
    // 2, 11 and 18 characters; and 26 characters in 2 words.
    for quote in [
        "ok",
        "Send it now",
        "Send the print now",
        "Approved: sendthenosepiece",
    ] {
        let why = send_ok_in(&text, quote, transcript, P31_NOW).unwrap_err();
        assert!(why.contains("too short"), "{quote}: {why}");
    }
    assert!(send_ok_in(&text, P31_WORDS, transcript, P31_NOW).is_ok());
}

/// Review B1.2: the matching prompt is at most 6 hours old.
#[test]
fn p31_a_seven_hour_old_prompt_refuses() {
    let transcript = Path::new("C:/t/s.jsonl");
    let at = crate::nirvana_usage::rfc3339(P31_AT).unwrap();
    let text = p31_owner(P31_WORDS);
    let why = send_ok_in(&text, P31_WORDS, transcript, at + 7 * 3600).unwrap_err();
    assert!(
        why.contains("7 h ago") && why.contains("older than 6 h"),
        "{why}"
    );
    assert!(send_ok_in(&text, P31_WORDS, transcript, at + 6 * 3600).is_ok());
    // A time that does not read is no prompt.
    let untimed = p31_line(
        "user",
        Value::String(P31_WORDS.into()),
        &[
            p31_human(),
            ("timestamp", Value::String("yesterday".into())),
        ],
    );
    assert!(send_ok_in(&untimed, P31_WORDS, transcript, P31_NOW).is_err());
}

/// Review B1.3: a teammate message, a cross-session message, an Artifact comment and a paste holding the quote refuse,
/// as do prompts that mix the owner's words with one. Each is written as the 2026-10-29 scan found it (a relay with no
/// `origin`), and again marked `human`, so the text alone must refuse it.
#[test]
fn p31_a_relayed_message_holding_the_quote_refuses() {
    let transcript = Path::new("C:/t/s.jsonl");
    let words = P31_WORDS;
    let relayed = [
        format!("<teammate-message teammate_id=\"team-lead\" summary=\"go\">\n{words}\n</teammate-message>"),
        format!(
            "Another Claude session sent a message: <teammate-message teammate_id=\"main\">{words}</teammate-message>"
        ),
        format!("<cross-session-message from=\"worker\">{words}</cross-session-message>"),
        format!("[Artifact comment sent to Claude] {words}"),
        format!("<pasted_content id=\"1\">{words}</pasted_content>"),
        // Mixed: the owner's words first, a relayed block after them.
        format!("{words}\n<teammate-message teammate_id=\"x\">done</teammate-message>"),
        format!("I approve. {words}\n\n<pasted_content id=\"2\">log</pasted_content>"),
    ];
    for text in &relayed {
        for extra in [vec![], vec![p31_human()]] {
            let line = p31_line("user", Value::String(text.clone()), &extra);
            let why = send_ok_in(&line, words, transcript, P31_NOW).unwrap_err();
            assert!(
                why.contains("none of the owner's own prompts"),
                "{text}: {why}"
            );
        }
        // A relay as one text block beside the owner's own block is mixed too.
        let blocks = serde_json::json!([
            {"type": "text", "text": format!("ok. {words}")},
            {"type": "text", "text": text},
        ]);
        let line = p31_line("user", blocks, &[p31_human()]);
        assert!(
            send_ok_in(&line, words, transcript, P31_NOW).is_err(),
            "{text}"
        );
    }
    // A real owner prompt, with an image beside its words, passes.
    let blocks = serde_json::json!([
        {"type": "text", "text": format!("Looks right. {words}")},
        {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AA=="}},
    ]);
    let line = p31_line("user", blocks, &[p31_human()]);
    let found = send_ok_in(&line, words, transcript, P31_NOW).expect("owner");
    assert_eq!(found.prompt, format!("Looks right. {words}"));
}

#[test]
fn p31_verify_send_ok_reads_the_recorded_session_and_refuses_every_doubt() {
    let home = crate::nirvana::with_test_home("p31-verify-send-ok");
    let folder = home.path.join("project");
    std::fs::create_dir_all(&folder).unwrap();
    let words = P31_WORDS;
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
    // A prompt typed a minute ago, read against the clock.
    let at = crate::nirvana_run::utc(crate::nirvana_run::now() - 60).replace(' ', "T");
    std::fs::write(
        &transcript,
        p31_line(
            "user",
            Value::String(words.into()),
            &[p31_human(), ("timestamp", Value::String(at.clone()))],
        ) + "\n",
    )
    .unwrap();
    let found = verify_send_ok(Some(&home.path), &folder, words).expect("found");
    assert_eq!(
        (found.quote.as_str(), found.at.as_str()),
        (words, at.as_str())
    );
}

/// Review S1: a child's brief puts the device line back in place of the Send line, and an approved parent's child
/// hears the parent may have sent. A child of a child, which quotes the root's text or its parent's brief, hears it too.
#[test]
fn p31_a_child_brief_never_carries_the_send_line() {
    let desktop = vec!["bambu-studio-beta".to_owned()];
    let approved = with_brief_lines("# Print it\n".into(), &desktop, true);
    let child = child_brief_lines(&approved, true);
    assert!(
        child.contains(&format!("{DEVICE_LINE}\n{BRIEF_LINE}")),
        "{child}"
    );
    assert!(
        !child.contains(SEND_LINE) && !child.contains(DESKTOP_SEND_LINE),
        "{child}"
    );
    assert!(child.trim_end().ends_with(SENT_BEFORE_LINE), "{child}");
    // A printer server's Send line alone.
    let printer = child_brief_lines(
        &with_brief_lines("# o\n".into(), &["bambu".into()], true),
        true,
    );
    assert!(
        printer.contains(DEVICE_LINE) && !printer.contains(SEND_LINE),
        "{printer}"
    );
    // The grandchild: its parent had no `send_ok`, but the text it quotes did.
    assert_eq!(child_brief_lines(&approved, false), child);
    assert_eq!(child_brief_lines(&child, false), child);
    // A brief that lost its lines still gets the device line when its parent was approved.
    let bare = child_brief_lines("# Send it\n", true);
    assert!(
        bare.contains(DEVICE_LINE) && bare.contains(SENT_BEFORE_LINE),
        "{bare}"
    );
    // No approval anywhere: the brief is unchanged.
    let plain = with_brief_lines("# Slice it\n".into(), &desktop, false);
    assert_eq!(child_brief_lines(&plain, false), plain);
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

// ---- plan 35 P35c: bladebro for browser orders, on both CLIs ----

/// The answer `nirvana_bladebro::installed` gives this thread's tests; restored when dropped.
struct Installed;
impl Installed {
    fn set(answer: bool) -> Self {
        crate::nirvana_bladebro::TEST_INSTALLED.set(Some(answer));
        Self
    }
}
impl Drop for Installed {
    fn drop(&mut self) {
        crate::nirvana_bladebro::TEST_INSTALLED.set(None);
    }
}

fn agy_argv() -> Vec<String> {
    argv(&LaunchSpec {
        cli: "agy".into(),
        model: "gemini-3.8-flash-low".into(),
        effort: Effort::Low,
        prompt: "Do the work order in o.md.".into(),
        resume: None,
        print_timeout_s: 600,
    })
    .unwrap()
}

/// A project `.mcp.json` that defines a `bladebro` of its own, beside a token-holding server.
const WITH_BLADEBRO: &str = r#"{"mcpServers": {"bladebro": {"command": "npx", "args": ["bladebro", "mcp"]}, "bambu": {"command": "node", "env": {"BAMBU_TOKEN": "fake-not-real"}}}}"#;

/// R1: `bladebro` is herdr's server, never a project's of that name, and no capability: the seat rule never reads it,
/// so `--needs bladebro` seats anywhere, and `browser` stays a capability for the pick.
#[test]
fn p35c_the_bladebro_need_is_herdrs_and_the_seat_rule_never_reads_it() {
    let home = crate::nirvana::with_test_home("p35c-seat-needs");
    let table = table();
    let root = project(&home.path, Some(WITH_BLADEBRO));
    let read = seat_needs(&table, &root, &words(&["code", "bladebro", "browser"])).unwrap();
    assert_eq!(read.rest, words(&["code", "browser"]));
    assert!(read.servers.is_empty());
    assert_eq!(read.claude_only, None);
    let caps = capabilities(&table);
    assert!(caps.contains(&"browser") && !caps.contains(&"bladebro"));
    assert_eq!(
        seated_servers(&words(&["bladebro", "bambu"]), &caps),
        words(&["bambu"])
    );
    assert!(servers_among(&words(&["bladebro"]), &caps, &words(&["bladebro"])).is_empty());
    // A `.mcp.json` that does not parse refuses a server need, never `bladebro`.
    std::fs::write(root.join(MCP_FILE), "{").unwrap();
    assert_eq!(
        seat_needs(&table, &root, &words(&["code", "bladebro"])).unwrap(),
        Needs {
            rest: words(&["code"]),
            ..Needs::default()
        }
    );
    // `herdr seat pick --needs bladebro` on a class agy wins: an agy seat, no unknown word.
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    std::fs::write(root.join(MCP_FILE), WITH_BLADEBRO).unwrap();
    let args = words(&["--class", "research.web", "--needs", "bladebro"]);
    let (table, picked) = nirvana_seat::pick_args_for(&root, &args).unwrap();
    assert!(
        picked.request.needs.is_empty(),
        "{:?}",
        picked.request.needs
    );
    assert!(nirvana_seat::unknown_words(&table, &picked.request.needs, &[]).is_empty());
    let chosen = nirvana_seat::pick(&table, &World::default(), &picked.request).unwrap();
    assert_eq!(chosen.cli, "agy", "{}", chosen.seat);
}

/// Item 1, claude: the order's `--mcp-config` holds herdr's bladebro (beside its servers, or alone), approved with
/// `mcp__bladebro__*`, its home made; the project's own `bladebro` entry is never the one given.
#[test]
fn p35c_a_claude_browser_order_gets_bladebro_in_its_mcp_config() {
    let _installed = Installed::set(true);
    let home = crate::nirvana::with_test_home("p35c-claude-argv");
    let root = project(&home.path, Some(WITH_BLADEBRO));
    let run_dir = root.join(".nirvana").join("runs").join("r1");
    let caps = || Ok(vec!["code", "browser"]);
    let today = claude_argv();
    let file = order_config_path(&run_dir, "o1");
    let blade = crate::nirvana_bladebro::home(&run_dir, "o1");
    // bladebro alone.
    let mut out = Vec::new();
    let got = order_argv(
        today.clone(),
        &root,
        &root,
        &run_dir,
        "o1",
        &words(&["browser"]),
        caps(),
        &mut out,
    )
    .unwrap();
    assert_eq!(&got[..today.len()], &today[..]);
    assert_eq!(
        &got[today.len()..],
        [
            "--mcp-config".to_owned(),
            file.display().to_string(),
            "--allowedTools".to_owned(),
            "mcp__bladebro__*".to_owned()
        ]
    );
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(
        written,
        serde_json::json!({"mcpServers": {"bladebro": crate::nirvana_bladebro::definition(&blade)}})
    );
    assert!(blade.is_dir());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "MCP servers for this order: bladebro\n"
    );
    // Beside a server, with `bladebro` named as a need: both, the project's bladebro entry left out.
    let mut out = Vec::new();
    let got = order_argv(
        today.clone(),
        &root,
        &root,
        &run_dir,
        "o2",
        &words(&["bladebro", "bambu"]),
        caps(),
        &mut out,
    )
    .unwrap();
    assert_eq!(
        got.last().map(String::as_str),
        Some("mcp__bambu__*,mcp__bladebro__*")
    );
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(order_config_path(&run_dir, "o2")).unwrap())
            .unwrap();
    assert_eq!(
        written["mcpServers"]["bambu"]["env"]["BAMBU_TOKEN"],
        "fake-not-real"
    );
    let blade2 = crate::nirvana_bladebro::home(&run_dir, "o2");
    assert_eq!(
        written["mcpServers"]["bladebro"],
        crate::nirvana_bladebro::definition(&blade2)
    );
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "MCP servers for this order: bambu, bladebro\n"
    );
    // The shell gets the same home the server has (item 2).
    let env = crate::nirvana_bladebro::worker_env(&got);
    assert!(
        env.contains(&("BLADE_HOME".to_owned(), blade2.display().to_string())),
        "{env:?}"
    );
    // No browser need: today's argv, no file, no home.
    let mut out = Vec::new();
    assert_eq!(
        order_argv(
            today.clone(),
            &root,
            &root,
            &run_dir,
            "o3",
            &words(&["code"]),
            caps(),
            &mut out
        )
        .unwrap(),
        today
    );
    assert!(!order_config_path(&run_dir, "o3").exists());
    assert!(!crate::nirvana_bladebro::home(&run_dir, "o3").exists());
    assert!(out.is_empty());
}

/// Item 1, agy: herdr's bladebro goes into the order's tools folder, translated, with its home; the tools folder then
/// reaches the argv with `--add-dir`, and the shell gets the same home.
#[test]
fn p35c_an_agy_browser_order_gets_bladebro_in_its_tools_folder() {
    let _installed = Installed::set(true);
    let home = crate::nirvana::with_test_home("p35c-agy-argv");
    let root = project(&home.path, None);
    let run_dir = root.join(".nirvana").join("runs").join("r1");
    let today = agy_argv();
    let mut out = Vec::new();
    let got = order_argv(
        today.clone(),
        &root,
        &root,
        &run_dir,
        "a1",
        &words(&["bladebro"]),
        Ok(vec!["code"]),
        &mut out,
    )
    .unwrap();
    assert_eq!(got, today);
    let tools = crate::nirvana_agy_tools::tools_dir(&run_dir, "a1");
    let written: Value = serde_json::from_str(
        &std::fs::read_to_string(tools.join(crate::nirvana_agy_tools::MCP_CONFIG)).unwrap(),
    )
    .unwrap();
    let blade = crate::nirvana_bladebro::home(&run_dir, "a1");
    let mut expected = crate::nirvana_bladebro::definition(&blade);
    expected["cwd"] = Value::from(crate::nirvana_agy_tools::plain_path(&root));
    assert_eq!(
        written,
        serde_json::json!({"mcpServers": {"bladebro": expected}})
    );
    assert!(blade.is_dir());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "MCP servers for this order: bladebro\n"
    );
    let order_file = root.join("o.md").display().to_string();
    let order = crate::nirvana_agy_tools::Order {
        root: &root,
        cwd: &root,
        run_dir: &run_dir,
        order: "a1",
        order_file: &order_file,
        deny: Vec::new(),
    };
    let mut out = Vec::new();
    let argv = crate::nirvana_agy_tools::with_tools(got, &order, &mut out).unwrap();
    let at = argv
        .iter()
        .position(|arg| arg == "--add-dir")
        .expect("--add-dir");
    assert_eq!(argv[at + 1], crate::nirvana_agy_tools::plain_path(&tools));
    let env = crate::nirvana_bladebro::worker_env(&argv);
    assert!(
        env.contains(&("BLADE_HOME".to_owned(), blade.display().to_string())),
        "{env:?}"
    );
    assert!(
        env.contains(&("BLADE_LANE".to_owned(), "agent".to_owned())),
        "{env:?}"
    );
    // No browser need: nothing written, no home.
    let mut out = Vec::new();
    assert_eq!(
        order_argv(
            today.clone(),
            &root,
            &root,
            &run_dir,
            "a2",
            &words(&["code"]),
            Ok(vec!["code"]),
            &mut out
        )
        .unwrap(),
        today
    );
    assert!(!crate::nirvana_agy_tools::tools_dir(&run_dir, "a2").exists());
    assert!(!crate::nirvana_bladebro::home(&run_dir, "a2").exists());
}

/// Item 1 and decision 0043 item 6: bladebro not on PATH grants nothing on either CLI, and the view says so once.
#[test]
fn p35c_bladebro_not_on_path_grants_nothing_and_says_so() {
    let _installed = Installed::set(false);
    let home = crate::nirvana::with_test_home("p35c-missing");
    let root = project(&home.path, Some(MCP_JSON));
    let run_dir = root.join(".nirvana").join("runs").join("r1");
    for (order, today) in [("c1", claude_argv()), ("a1", agy_argv())] {
        let mut out = Vec::new();
        let got = order_argv(
            today.clone(),
            &root,
            &root,
            &run_dir,
            order,
            &words(&["browser"]),
            Ok(vec!["browser"]),
            &mut out,
        )
        .unwrap();
        assert_eq!(got, today, "{order}");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!("{}\n", crate::nirvana_bladebro::MISSING_LINE)
        );
        assert!(!crate::nirvana_bladebro::home(&run_dir, order).exists());
        assert!(!order_config_path(&run_dir, order).exists());
        assert!(!crate::nirvana_agy_tools::tools_dir(&run_dir, order).exists());
    }
    // With a server beside it, the server is still given.
    let mut out = Vec::new();
    let got = order_argv(
        claude_argv(),
        &root,
        &root,
        &run_dir,
        "c2",
        &words(&["browser", "bambu"]),
        Ok(vec!["browser"]),
        &mut out,
    )
    .unwrap();
    assert_eq!(got.last().map(String::as_str), Some("mcp__bambu__*"));
}

/// Item 4: the order's bladebro home goes with its other tool files when the worker ends.
#[test]
fn p35c_the_bladebro_home_goes_with_the_orders_config() {
    let _installed = Installed::set(true);
    let home = crate::nirvana::with_test_home("p35c-remove");
    let root = project(&home.path, None);
    let run_dir = root.join(".nirvana").join("runs").join("r1");
    let mut out = Vec::new();
    order_argv(
        claude_argv(),
        &root,
        &root,
        &run_dir,
        "o1",
        &words(&["browser"]),
        Ok(vec!["browser"]),
        &mut out,
    )
    .unwrap();
    let blade = crate::nirvana_bladebro::home(&run_dir, "o1");
    std::fs::create_dir_all(blade.join("profile")).unwrap();
    std::fs::write(blade.join("profile").join("Cookies"), "x").unwrap();
    let mut out = Vec::new();
    remove_order_config(&run_dir, "o1", &mut out);
    assert!(!blade.exists());
    assert!(!order_config_path(&run_dir, "o1").exists());
    assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
}

/// Owed from the plan 35 merge (P35a): `seat run` of an order in the project folder writes the seat snapshot the
/// owned axis reads, naming the files dirty at seating. Without the call the gate counts the whole dirty tree.
#[test]
fn p35c_seat_run_in_the_project_folder_writes_the_seat_snapshot() {
    use crate::nirvana_run::Project;
    let home = crate::nirvana::with_test_home("p35c-seat-snapshot");
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    let project = Project::at(home.path.join("project"));
    project.ensure().unwrap();
    let git = |args: &[&str]| {
        let done = std::process::Command::new("git")
            .args(args)
            .current_dir(&project.root)
            .output()
            .unwrap();
        assert!(
            done.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&done.stderr)
        );
    };
    git(&["init", "-q"]);
    std::fs::write(
        project.root.join("dirty.txt"),
        "the owner's work in progress\n",
    )
    .unwrap();
    let order_file = project.root.join("order.md");
    std::fs::write(&order_file, "# Find the dial specs\n").unwrap();
    nirvana_seat::set_test_world(Some(std::collections::BTreeMap::new()));
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let seat = nirvana_seat::pick(
        &table,
        &nirvana_seat::live_world(&table),
        &nirvana_seat::Request::new("research.web"),
    )
    .unwrap()
    .seat;
    let request = crate::nirvana_worker::SeatRequest {
        seat,
        order_file: order_file.clone(),
        order: "s1".into(),
        run: "r1".into(),
        class: Some("research.web".into()),
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
    // A live lock: the seat is journaled and no worker is started.
    let _live = crate::nirvana_worker::take_live(&project, "r1", "s1")
        .unwrap()
        .unwrap();
    let seated = crate::nirvana_worker::seat_order(&project, &request);
    nirvana_seat::set_test_world(None);
    seated.unwrap();
    let snapshot = project
        .run_dir("r1")
        .join("gate")
        .join("s1.seat-dirty.json");
    let text = std::fs::read_to_string(&snapshot)
        .unwrap_or_else(|err| panic!("{}: {err}", snapshot.display()));
    let read: Map<String, Value> = serde_json::from_str(&text).unwrap();
    assert!(read.contains_key("dirty.txt"), "{text}");
    assert!(read.contains_key("order.md"), "{text}");
}
