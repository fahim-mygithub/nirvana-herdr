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
        deny_rules(&words(&["Bash", "PowerShell", "Write", "Read"])).unwrap(),
        words(&["command(*)", "write_file(*)", "read_file(*)"])
    );
    // Review round S3: no probe saw `write_file(*)` stop agy's `edit_file`, so Edit and NotebookEdit have no rule.
    assert_eq!(
        unmapped(&words(&["Write", "Edit", "NotebookEdit"])),
        words(&["Edit", "NotebookEdit"])
    );
    assert_eq!(
        deny_rule("mcp__bambu__print").as_deref(),
        Some("mcp(bambu/print)")
    );
    assert_eq!(deny_rule("mcp__bambu__*").as_deref(), Some("mcp(bambu/*)"));
    assert_eq!(deny_rule("mcp__bambu").as_deref(), Some("mcp(bambu/*)"));
    assert_eq!(
        deny_rule("mcp__moto-reel__get_clip").as_deref(),
        Some("mcp(moto-reel/get_clip)")
    );
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
        // Review round S6: a server part with `_` may be Claude's flattening of `.`, a space or `_`, and `__` splits
        // two ways, so neither names one agy server for sure.
        "mcp__my_server__t",
        "mcp__cosplay_flow",
        "mcp__my.server__t",
        "mcp__a__b__c",
        "mcp__a___t",
        "mcp__a__t.x",
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
    let got = crate::nirvana_gui_tools::order_argv_seated(
        today.clone(),
        &root,
        &root,
        &run_dir,
        "o1",
        &words(&["code", "bambu", "reel"]),
        &crate::nirvana_bladebro::Seat::default(),
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
            "bambu": {"command": "node", "env": {"BAMBU_TOKEN": "fake-not-real"}, "cwd": plain_path(&root)},
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
    crate::nirvana_gui_tools::order_argv_seated(
        today.clone(),
        &root,
        &cwd,
        &run_dir,
        "o2",
        &words(&["bambu", "reel"]),
        &crate::nirvana_bladebro::Seat::default(),
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
        crate::nirvana_gui_tools::order_argv_seated(
            today.clone(),
            &root,
            &root,
            &run_dir,
            order,
            &words(needs),
            &crate::nirvana_bladebro::Seat::default(),
            caps(),
            &mut out,
        )
        .unwrap_err()
    };
    // Review round S1: a stdio server's own `cwd` is kept; one with none gets the worker's folder (the cwd given).
    std::fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers": {"bambu": {"command": "node", "args": ["tools/s.js"], "cwd": "D:/own"}, "reel": {"command": "uv", "args": ["run", "r.py"]}}}"#,
    )
    .unwrap();
    let wt = root.join("wt2");
    let mut out = Vec::new();
    crate::nirvana_gui_tools::order_argv_seated(
        today.clone(),
        &root,
        &wt,
        &run_dir,
        "o6",
        &words(&["bambu", "reel"]),
        &crate::nirvana_bladebro::Seat::default(),
        caps(),
        &mut out,
    )
    .unwrap();
    let written: Value = serde_json::from_str(
        &std::fs::read_to_string(tools_dir(&run_dir, "o6").join(MCP_CONFIG)).unwrap(),
    )
    .unwrap();
    assert_eq!(written["mcpServers"]["bambu"]["cwd"], "D:/own");
    assert_eq!(
        written["mcpServers"]["reel"]["cwd"],
        Value::from(plain_path(&wt))
    );
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
        serde_json::json!({"permissions": {"deny": [
            format!("write_file({}/)", rule_path(&tools)),
            format!("write_file({}/)", rule_path(&run_dir.join(DIR))),
        ]}})
    );
    // Review round S3: the deny file is under `<run>/agy/`, which the second rule covers.
    assert!(settings.starts_with(run_dir.join(DIR)));
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
        claude_skills(&root, &root, &mut Vec::new()),
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
        serde_json::json!({"permissions": {"deny": [
            "command(*)".to_owned(),
            "write_file(*)".to_owned(),
            format!("write_file({}/)", rule_path(&run_dir.join(DIR))),
        ]}})
    );
    // Review round nit: a rule given twice, not side by side, is written once.
    let mut twice = order(&root, &root, &run_dir, &file);
    twice.deny = words(&["command(*)", "write_file(*)", "command(*)"]);
    with_tools(today.clone(), &twice, &mut out).unwrap();
    let deny: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(deny["permissions"]["deny"].as_array().unwrap().len(), 3);
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

// ---- the review round (records/p35b.md "Review round") ----

/// S5: a skill over `SKILL_MAX_BYTES` is left out of a real `with_tools` with one line; the total cap holds too.
#[test]
fn p35b_a_skill_over_the_cap_is_left_out_and_the_skills_together_have_a_cap() {
    let (_home, root, run_dir) = project("p35b-skill-cap");
    let skills = root.join(".claude").join("skills");
    for name in ["big", "small"] {
        std::fs::create_dir_all(skills.join(name)).unwrap();
        std::fs::write(
            skills.join(name).join("SKILL.md"),
            format!("---\nname: {name}\n---\n"),
        )
        .unwrap();
    }
    // A sparse file: the cap reads the length, so no 4 MiB is written.
    std::fs::File::create(skills.join("big").join("blob.bin"))
        .unwrap()
        .set_len(SKILL_MAX_BYTES)
        .unwrap();
    let file = root.join("o.md").display().to_string();
    let mut out = Vec::new();
    with_tools(
        agy_argv(&file),
        &order(&root, &root, &run_dir, &file),
        &mut out,
    )
    .unwrap();
    let copied = tools_dir(&run_dir, "o1").join(".agents").join("skills");
    assert!(copied.join("small").join("SKILL.md").is_file());
    assert!(!copied.join("big").exists());
    let line = String::from_utf8(out).unwrap();
    assert!(
        line.starts_with("the skill big is left out: its folder holds ")
            && line.ends_with(&format!(" bytes, over {SKILL_MAX_BYTES}\n")),
        "{line}"
    );
    assert_eq!(line.lines().count(), 1);
    // The total: with a 10-byte budget, the first 6-byte skill goes and the second does not.
    let from = root.join("caps");
    let list: Vec<(String, PathBuf)> = ["a", "b"]
        .iter()
        .map(|name| {
            std::fs::create_dir_all(from.join(name)).unwrap();
            std::fs::write(from.join(name).join("SKILL.md"), "six b\n").unwrap();
            ((*name).to_owned(), from.join(name))
        })
        .collect();
    let to = root.join("caps-out");
    let mut out = Vec::new();
    copy_skills(&list, &to, 100, 10, &mut out).unwrap();
    assert!(to.join("a").join("SKILL.md").is_file());
    assert!(!to.join("b").exists());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "the skill b is left out: with it the skills would hold 12 bytes, over 10\n"
    );
}

/// Nit: a linked skill folder, or a linked `.claude/skills`, is not followed, each with one line.
#[cfg(windows)]
#[test]
fn p35b_a_linked_skill_is_left_out_with_a_line() {
    let (_home, root, _run_dir) = project("p35b-skill-link");
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("SKILL.md"), "---\nname: away\n---\n").unwrap();
    let skills = root.join(".claude").join("skills");
    std::fs::create_dir_all(&skills).unwrap();
    let junction = |link: &Path, target: &Path| {
        let status = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    };
    junction(&skills.join("away"), &outside);
    let mut out = Vec::new();
    assert!(claude_skills(&root, &root, &mut out).is_empty());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "the skill away is left out: it is a link, which herdr does not follow\n"
    );
    let cwd = root.join("sub");
    std::fs::create_dir_all(cwd.join(".claude")).unwrap();
    junction(&cwd.join(".claude").join("skills"), &skills);
    let mut out = Vec::new();
    assert!(claude_skills(&cwd, &root, &mut out).is_empty());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        format!(
            "{} is a link, so herdr copies no skill from it\n",
            cwd.join(".claude").join("skills").display()
        )
    );
}

/// Nits: the walk stops at a checkout's own `.git`, so an untracked root `AGENTS.md` does not leave a worktree worker
/// with no rules; a root `.agents/skills/<name>` counts for a cwd below it; `@types/node` is no import.
#[test]
fn p35b_the_workspace_walk_stops_at_a_checkout_and_reaches_the_root_below_one() {
    let (_home, root, _run_dir) = project("p35b-walk");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("AGENTS.md"), "untracked\n").unwrap();
    std::fs::write(root.join("CLAUDE.md"), "root rule\n").unwrap();
    let worktree = root.join(".nirvana").join("worktrees").join("o1");
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(worktree.join(".git"), "gitdir: elsewhere\n").unwrap();
    std::fs::write(worktree.join("CLAUDE.md"), "tracked rule\n").unwrap();
    assert_eq!(workspace_dirs(&worktree, &root), vec![worktree.clone()]);
    assert_eq!(
        claude_rules(&worktree, &root),
        Some(worktree.join("CLAUDE.md"))
    );
    // A plain folder below the root walks up to it, and the root's own rules count.
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    assert_eq!(workspace_dirs(&sub, &root), vec![sub.clone(), root.clone()]);
    assert_eq!(claude_rules(&sub, &root), None);
    let skills = root.join(".claude").join("skills").join("s");
    std::fs::create_dir_all(&skills).unwrap();
    std::fs::write(skills.join("SKILL.md"), "---\nname: s\n---\n").unwrap();
    assert_eq!(claude_skills(&sub, &root, &mut Vec::new()).len(), 1);
    std::fs::create_dir_all(root.join(".agents").join("skills").join("s")).unwrap();
    assert!(claude_skills(&sub, &root, &mut Vec::new()).is_empty());
    assert!(!has_imports("npm i @types/node, then run"));
    assert!(has_imports("See @docs/more.md."));
    assert!(has_imports("@~/.claude/mine.md"));
}

/// Nit: the `--add-dir` text and the rule's path come from one text, without a `\\?\` prefix.
#[test]
fn p35b_a_verbatim_path_is_named_one_way() {
    assert_eq!(plain_path(Path::new(r"\\?\C:\p\runs")), r"C:\p\runs");
    assert_eq!(rule_path(Path::new(r"\\?\C:\p\runs")), "C:/p/runs");
    assert_eq!(
        plain_path(Path::new(r"\\?\UNC\host\share")),
        r"\\?\UNC\host\share"
    );
}

/// S4: the orders a run's `agy/` and `mcp/` folders hold files for, once each.
#[test]
fn p35b_the_leftover_orders_are_read_from_both_folders() {
    let (_home, _root, run_dir) = project("p35b-leftovers");
    assert!(leftover_orders(&run_dir).is_empty());
    std::fs::create_dir_all(tools_dir(&run_dir, "a1")).unwrap();
    std::fs::write(settings_path(&run_dir, "a1"), "{}").unwrap();
    std::fs::write(settings_path(&run_dir, "a2"), "{}").unwrap();
    std::fs::create_dir_all(run_dir.join("mcp")).unwrap();
    std::fs::write(run_dir.join("mcp").join("c1.json"), "{}").unwrap();
    std::fs::write(run_dir.join("mcp").join("notes.txt"), "x").unwrap();
    assert_eq!(leftover_orders(&run_dir), words(&["a1", "a2", "c1"]));
    // Plan 35 P35c item 4: a bladebro home left alone is a leftover too; a stray file there is not an order.
    std::fs::create_dir_all(crate::nirvana_bladebro::home(&run_dir, "b1")).unwrap();
    std::fs::create_dir_all(crate::nirvana_bladebro::home(&run_dir, "c1")).unwrap();
    std::fs::write(run_dir.join("blade").join("notes.txt"), "x").unwrap();
    assert_eq!(leftover_orders(&run_dir), words(&["a1", "a2", "b1", "c1"]));
}
