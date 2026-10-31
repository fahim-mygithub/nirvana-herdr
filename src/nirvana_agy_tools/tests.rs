use super::*;
use crate::nirvana_seat::Effort;
use crate::nirvana_worker::{argv, LaunchSpec};

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

/// A project root under the test home, with a run folder.
fn project(name: &str) -> (crate::nirvana::TestHomeGuard, PathBuf, PathBuf) {
    let home = crate::nirvana::with_test_home(name);
    let root = home.path.join("project");
    std::fs::create_dir_all(&root).unwrap();
    let run_dir = root.join(".nirvana").join("runs").join("r1");
    (home, root, run_dir)
}

fn agy_argv(order_file: &str) -> Vec<String> {
    argv(&LaunchSpec {
        cli: "agy".into(),
        model: "gemini-3.8-flash-low".into(),
        effort: Effort::Low,
        prompt: crate::nirvana_worker::first_line(order_file),
        resume: None,
        print_timeout_s: 600,
    })
    .unwrap()
}

fn order<'a>(root: &'a Path, cwd: &'a Path, run_dir: &'a Path, file: &'a str) -> Order<'a> {
    Order {
        root,
        cwd,
        run_dir,
        order: "o1",
        order_file: file,
        deny: Vec::new(),
    }
}

#[test]
fn p35b_each_claude_tool_maps_to_its_measured_agy_rule_and_any_other_is_refused() {
    assert_eq!(
        deny_rules(&words(&[
            "Bash",
            "PowerShell",
            "Write",
            "Edit",
            "NotebookEdit",
            "Read"
        ]))
        .unwrap(),
        words(&["command(*)", "write_file(*)", "read_file(*)"])
    );
    assert_eq!(
        deny_rule("mcp__bambu__print").as_deref(),
        Some("mcp(bambu/print)")
    );
    assert_eq!(deny_rule("mcp__bambu__*").as_deref(), Some("mcp(bambu/*)"));
    assert_eq!(deny_rule("mcp__bambu").as_deref(), Some("mcp(bambu/*)"));
    // Unmeasured: a tool with no row, a pattern argument, an odd server name.
    for tool in [
        "WebSearch",
        "Task",
        "Grep",
        "Bash(git push:*)",
        "Edit(src/**)",
        "mcp__",
        "mcp__a b__x",
        "bash",
    ] {
        assert_eq!(deny_rule(tool), None, "{tool}");
    }
    let why = deny_rules(&words(&["Write", "WebSearch", "Grep"])).unwrap_err();
    assert_eq!(
        why,
        "agy cannot be denied WebSearch, Grep: no measured agy rule matches (plan 35 §3.4)"
    );
    assert_eq!(unmapped(&words(&["Bash", "Agent"])), words(&["Agent"]));
}

#[test]
fn p35b_a_server_is_translated_to_agys_shape_and_one_agy_cannot_take_is_refused() {
    let stdio: Value = serde_json::json!({"type": "stdio", "command": "node", "args": ["i.js"],
        "env": {"PRINTER_HOST": "192.0.2.10"}, "cwd": "C:/x"});
    assert_eq!(refusal("bambu", &stdio), None);
    assert_eq!(
        translate(&stdio),
        serde_json::json!({"command": "node", "args": ["i.js"], "env": {"PRINTER_HOST": "192.0.2.10"}, "cwd": "C:/x"})
    );
    let http: Value =
        serde_json::json!({"type": "http", "url": "http://127.0.0.1:9/mcp", "headers": {"X": "y"}});
    assert_eq!(refusal("h", &http), None);
    assert_eq!(
        translate(&http),
        serde_json::json!({"serverUrl": "http://127.0.0.1:9/mcp", "headers": {"X": "y"}})
    );
    // No type: a command is stdio, a url is http.
    assert_eq!(refusal("c", &serde_json::json!({"command": "x"})), None);
    assert_eq!(
        refusal("u", &serde_json::json!({"url": "http://h/mcp"})),
        None
    );
    let refused = |definition: Value| refusal("s", &definition).unwrap_or_default();
    assert!(
        refused(serde_json::json!({"command": "x", "args": ["--key=${KEY}"]}))
            .contains("uses ${...}")
    );
    assert!(
        refused(serde_json::json!({"command": "x", "env": {"A": {"deep": "${B}"}}}))
            .contains("uses ${...}")
    );
    assert!(
        refused(serde_json::json!({"type": "sse", "url": "http://h/sse"}))
            .contains("s is type sse, which no agy probe measured")
    );
    assert!(refused(serde_json::json!({"type": "ws", "url": "ws://h"})).contains("type ws"));
    assert!(
        refused(serde_json::json!({"command": "x", "timeout": 5})).contains("the key \"timeout\"")
    );
    assert!(
        refused(serde_json::json!({"type": "stdio", "url": "http://h"})).contains("do not say so")
    );
    assert!(refused(serde_json::json!({"env": {}})).contains("neither a command nor a url"));
    assert!(refused(serde_json::json!("node")).contains("not an object"));
}

/// §3.2 through `order_argv`, the public path `seat exec` takes.
#[test]
fn p35b_an_agy_server_order_gets_its_servers_in_the_tools_folder_and_f3_holds() {
    let (_home, root, run_dir) = project("p35b-order-argv");
    std::fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers": {"bambu": {"type": "stdio", "command": "node", "env": {"BAMBU_TOKEN": "fake-not-real"}},
            "reel": {"type": "http", "url": "http://127.0.0.1:9/mcp"}, "xtool": {"command": "uv"}}}"#,
    )
    .unwrap();
    let file = root.join("o.md").display().to_string();
    let today = agy_argv(&file);
    let caps = || Ok(vec!["code"]);
    let mut out = Vec::new();
    let got = crate::nirvana_gui_tools::order_argv(
        today.clone(),
        &root,
        &root,
        &run_dir,
        "o1",
        &words(&["code", "bambu", "reel"]),
        caps(),
        &mut out,
    )
    .unwrap();
    assert_eq!(got, today, "the argv takes the folder later, in with_tools");
    let config = tools_dir(&run_dir, "o1").join(MCP_CONFIG);
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(
        written,
        serde_json::json!({"mcpServers": {
            "bambu": {"command": "node", "env": {"BAMBU_TOKEN": "fake-not-real"}},
            "reel": {"serverUrl": "http://127.0.0.1:9/mcp"}}})
    );
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "MCP servers for this order: bambu, reel\n"
    );
    // The workspace names bambu already: it is left out, with one line.
    let cwd = root.join("wt");
    std::fs::create_dir_all(cwd.join(".agents")).unwrap();
    std::fs::write(
        cwd.join(MCP_CONFIG),
        r#"{"mcpServers": {"bambu": {"command": "node"}}}"#,
    )
    .unwrap();
    let mut out = Vec::new();
    crate::nirvana_gui_tools::order_argv(
        today.clone(),
        &root,
        &cwd,
        &run_dir,
        "o2",
        &words(&["bambu", "reel"]),
        caps(),
        &mut out,
    )
    .unwrap();
    let written: Value = serde_json::from_str(
        &std::fs::read_to_string(tools_dir(&run_dir, "o2").join(MCP_CONFIG)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        written["mcpServers"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["reel"]
    );
    assert_eq!(
        String::from_utf8(out).unwrap(),
        format!(
            "bambu is in {} already, so herdr leaves it out\nMCP servers for this order: reel\n",
            cwd.join(MCP_CONFIG).display()
        )
    );
    // F3 on agy: a definition changed since the seat to one agy cannot take, a server gone, a desktop server.
    let launch = |order: &str, needs: &[&str]| {
        let mut out = Vec::new();
        crate::nirvana_gui_tools::order_argv(
            today.clone(),
            &root,
            &root,
            &run_dir,
            order,
            &words(needs),
            caps(),
            &mut out,
        )
        .unwrap_err()
    };
    std::fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers": {"bambu": {"command": "node", "env": {"T": "${TOKEN}"}}, "xtool": {"command": "uv"}}}"#,
    )
    .unwrap();
    let why = launch("o3", &["bambu"]);
    assert!(why.starts_with("o3 was seated for MCP server bambu and cannot get it: only a claude worker can take bambu: bambu uses ${...}"), "{why}");
    let why = launch("o4", &["reel"]);
    assert!(why.ends_with("no longer defines reel"), "{why}");
    let why = launch("o5", &["xtool"]);
    assert!(why.contains("xtool drives a desktop window"), "{why}");
    for order in ["o3", "o4", "o5"] {
        assert!(!tools_dir(&run_dir, order).exists(), "{order}");
    }
}

#[test]
fn p35b_an_order_with_nothing_to_add_keeps_todays_argv_byte_for_byte() {
    let (_home, root, run_dir) = project("p35b-nothing");
    let file = root.join("o.md").display().to_string();
    let today = agy_argv(&file);
    let mut out = Vec::new();
    let got = with_tools(
        today.clone(),
        &order(&root, &root, &run_dir, &file),
        &mut out,
    )
    .unwrap();
    assert_eq!(got, today);
    assert!(!tools_dir(&run_dir, "o1").exists());
    assert!(!settings_path(&run_dir, "o1").exists());
    assert!(out.is_empty());
    // A claude argv is never touched, whatever the folder holds.
    std::fs::write(root.join("CLAUDE.md"), "rule\n").unwrap();
    let claude = vec!["claude".to_owned(), "-p".to_owned(), "go".to_owned()];
    assert_eq!(
        with_tools(
            claude.clone(),
            &order(&root, &root, &run_dir, &file),
            &mut out
        )
        .unwrap(),
        claude
    );
    assert!(!tools_dir(&run_dir, "o1").exists());
}

#[test]
fn p35b_a_project_with_only_claude_md_gives_agy_its_rules_and_the_prompt_names_both_folders() {
    let (_home, root, run_dir) = project("p35b-rules");
    let body = "# Rules\n\nNever push. See @docs/more.md for the rest.\n";
    std::fs::write(root.join("CLAUDE.md"), body).unwrap();
    let cwd = root.join(".nirvana").join("worktrees").join("o1");
    std::fs::create_dir_all(&cwd).unwrap();
    let file = cwd.join("o.md").display().to_string();
    let today = agy_argv(&file);
    let mut out = Vec::new();
    let got = with_tools(
        today.clone(),
        &order(&root, &cwd, &run_dir, &file),
        &mut out,
    )
    .unwrap();
    let tools = tools_dir(&run_dir, "o1");
    let rules = std::fs::read_to_string(tools.join("AGENTS.md")).unwrap();
    assert_eq!(
        rules,
        format!(
            "This is {}, the project's CLAUDE.md, given to an agy worker as rules by herdr (plan 35). Its @path imports \
             are not followed.\n\n{body}",
            root.join("CLAUDE.md").display()
        )
    );
    // The flags go before the one `--print=` token, which names both folders.
    let settings = settings_path(&run_dir, "o1");
    let n = today.len();
    assert_eq!(&got[..n - 1], &today[..n - 1]);
    assert_eq!(
        &got[n - 1..],
        [
            "--add-dir".to_owned(),
            tools.display().to_string(),
            "--settings".to_owned(),
            settings.display().to_string(),
            format!("--print={}", first_line(&file, &cwd, &tools)),
        ]
    );
    assert!(got[n + 3].contains(&format!(
        "Your folder is {}; work only there.",
        cwd.display()
    )));
    assert!(got[n + 3].contains("holds herdr's tool config: never write there."));
    // Writes into the tools folder are denied, in the path form probe 1 measured.
    let deny: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(
        deny,
        serde_json::json!({"permissions": {"deny": [format!("write_file({}/)", rule_path(&tools))]}})
    );
    assert!(!rule_path(&tools).contains('\\'));
    // A continuation keeps its own line.
    let resumed = argv(&LaunchSpec {
        cli: "agy".into(),
        model: "gemini-3.8-flash-low".into(),
        effort: Effort::Low,
        prompt: crate::nirvana_worker::continue_line(&file, "it stalled"),
        resume: Some("c-1".into()),
        print_timeout_s: 600,
    })
    .unwrap();
    let got = with_tools(
        resumed.clone(),
        &order(&root, &cwd, &run_dir, &file),
        &mut out,
    )
    .unwrap();
    assert_eq!(got.last(), resumed.last());
    assert!(out.is_empty());
}

#[test]
fn p35b_agy_rules_of_the_projects_own_mean_no_copy() {
    let (_home, root, run_dir) = project("p35b-own-rules");
    std::fs::write(root.join("CLAUDE.md"), "claude rule\n").unwrap();
    let cwd = root.join("sub");
    std::fs::create_dir_all(&cwd).unwrap();
    for own in ["AGENTS.md", "GEMINI.md", ".agents/rules/r.md"] {
        let at = root.join(own);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "own\n").unwrap();
        assert_eq!(claude_rules(&cwd, &root), None, "{own}");
        std::fs::remove_file(&at).unwrap();
    }
    // A rules folder with no .md file is no rules.
    std::fs::write(root.join(".agents/rules/r.txt"), "x").unwrap();
    assert_eq!(claude_rules(&cwd, &root), Some(root.join("CLAUDE.md")));
    // The cwd's own CLAUDE.md first.
    std::fs::write(cwd.join("CLAUDE.md"), "sub rule\n").unwrap();
    assert_eq!(claude_rules(&cwd, &root), Some(cwd.join("CLAUDE.md")));
    let file = cwd.join("o.md").display().to_string();
    let mut out = Vec::new();
    with_tools(
        agy_argv(&file),
        &order(&root, &cwd, &run_dir, &file),
        &mut out,
    )
    .unwrap();
    let rules = std::fs::read_to_string(tools_dir(&run_dir, "o1").join("AGENTS.md")).unwrap();
    assert!(rules.ends_with("(plan 35).\n\nsub rule\n"), "{rules}");
    assert!(!has_imports("mail me@x.org; a @ b"));
    assert!(has_imports("@AGENTS.md"));
}

#[test]
fn p35b_a_long_claude_md_still_goes_and_the_view_says_agy_may_cut_it() {
    let (_home, root, run_dir) = project("p35b-long-rules");
    std::fs::write(root.join("CLAUDE.md"), "x".repeat(RULES_BUDGET_BYTES)).unwrap();
    let file = root.join("o.md").display().to_string();
    let mut out = Vec::new();
    with_tools(
        agy_argv(&file),
        &order(&root, &root, &run_dir, &file),
        &mut out,
    )
    .unwrap();
    let line = String::from_utf8(out).unwrap();
    assert!(
        line.contains("past agy's rules budget of about 80000: agy may cut it"),
        "{line}"
    );
    assert_eq!(line.lines().count(), 1);
    assert!(tools_dir(&run_dir, "o1").join("AGENTS.md").is_file());
}

#[test]
fn p35b_the_projects_claude_skills_are_copied_whole_unless_the_workspace_has_them() {
    let (_home, root, run_dir) = project("p35b-skills");
    let skills = root.join(".claude").join("skills");
    for name in ["seat", "probe"] {
        std::fs::create_dir_all(skills.join(name).join("refs")).unwrap();
        std::fs::write(
            skills.join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: the {name} skill.\n---\nbody\n"),
        )
        .unwrap();
        std::fs::write(skills.join(name).join("refs").join("a.md"), "ref\n").unwrap();
    }
    // A folder with no SKILL.md is no skill; the workspace's own `probe` wins.
    std::fs::create_dir_all(skills.join("notes")).unwrap();
    std::fs::create_dir_all(root.join(".agents").join("skills").join("probe")).unwrap();
    assert_eq!(
        claude_skills(&root, &root),
        vec![("seat".to_owned(), skills.join("seat"))]
    );
    let file = root.join("o.md").display().to_string();
    let mut out = Vec::new();
    let got = with_tools(
        agy_argv(&file),
        &order(&root, &root, &run_dir, &file),
        &mut out,
    )
    .unwrap();
    let copied = tools_dir(&run_dir, "o1").join(".agents").join("skills");
    assert_eq!(
        std::fs::read_to_string(copied.join("seat").join("SKILL.md")).unwrap(),
        "---\nname: seat\ndescription: the seat skill.\n---\nbody\n"
    );
    assert!(copied.join("seat").join("refs").join("a.md").is_file());
    assert!(!copied.join("probe").exists());
    assert!(!copied.join("notes").exists());
    assert!(got.contains(&"--add-dir".to_owned()));
    // The project's own skill folder is never written.
    assert_eq!(
        std::fs::read_dir(root.join(".agents").join("skills").join("probe"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn p35b_a_denied_order_gets_its_rules_in_the_settings_file_and_no_add_dir() {
    let (_home, root, run_dir) = project("p35b-deny");
    let file = root.join("o.md").display().to_string();
    let today = agy_argv(&file);
    let mut denied = order(&root, &root, &run_dir, &file);
    denied.deny = REVIEW_DENIED.map(str::to_owned).to_vec();
    let mut out = Vec::new();
    let got = with_tools(today.clone(), &denied, &mut out).unwrap();
    let settings = settings_path(&run_dir, "o1");
    let n = today.len();
    assert_eq!(
        &got[n - 1..],
        [
            "--settings".to_owned(),
            settings.display().to_string(),
            today[n - 1].clone()
        ]
    );
    assert!(!got.contains(&"--add-dir".to_owned()));
    let deny: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(
        deny,
        serde_json::json!({"permissions": {"deny": ["command(*)", "write_file(*)"]}})
    );
}

#[test]
fn p35b_the_tools_folder_and_the_deny_file_go_and_a_failed_remove_says_so() {
    let (_home, root, run_dir) = project("p35b-remove");
    std::fs::write(root.join("CLAUDE.md"), "rule\n").unwrap();
    let file = root.join("o.md").display().to_string();
    let mut out = Vec::new();
    with_tools(
        agy_argv(&file),
        &order(&root, &root, &run_dir, &file),
        &mut out,
    )
    .unwrap();
    assert!(tools_dir(&run_dir, "o1").is_dir() && settings_path(&run_dir, "o1").is_file());
    // Through plan 29's remove, which `seat exec` calls when the worker ends.
    crate::nirvana_gui_tools::remove_order_config(&run_dir, "o1", &mut out);
    assert!(!tools_dir(&run_dir, "o1").exists());
    assert!(!settings_path(&run_dir, "o1").exists());
    remove(&run_dir, "o1", &mut out);
    assert!(out.is_empty());
    // A deny file that cannot be removed: one line naming it.
    std::fs::create_dir_all(settings_path(&run_dir, "o1")).unwrap();
    remove(&run_dir, "o1", &mut out);
    let line = String::from_utf8(out).unwrap();
    assert!(
        line.starts_with(&format!(
            "the order's agy deny file stays at {}: ",
            settings_path(&run_dir, "o1").display()
        )),
        "{line}"
    );
    assert_eq!(line.lines().count(), 1);
}

#[test]
fn p35b_only_an_agy_research_or_media_order_gets_the_sources_line() {
    assert_eq!(sources_line("agy", "research.web"), Some(SOURCES_LINE));
    assert_eq!(sources_line("agy", "media.video"), Some(SOURCES_LINE));
    assert_eq!(sources_line("claude", "research.web"), None);
    assert_eq!(sources_line("agy", "build.bounded"), None);
}
