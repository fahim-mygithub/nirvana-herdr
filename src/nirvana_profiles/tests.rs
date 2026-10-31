use super::*;

use std::collections::BTreeMap;

use crate::nirvana_run::{replay, Kind, Project, Seated};
use crate::nirvana_seat::Request;
use crate::nirvana_worker::{self, SeatRequest};

const SLICER: &str = "---\n\
description: Slices and sends prints in the beta Bambu Studio through its server.\n\
class: sweep.mechanical\n\
effort: medium\n\
needs: [bambu-studio-beta]\n\
disallowed_tools: []\n\
---\n\
You drive Bambu Studio only through the bambu-studio-beta tools.\n\
owns: src/never.rs\n";

fn file(name: &str) -> PathBuf {
    PathBuf::from(format!("C:/p/.nirvana/profiles/{name}.md"))
}

fn parsed(text: &str) -> Profile {
    parse(&file("slicer"), "slicer", text).expect("parses")
}

fn refusals(name: &str, text: &str) -> Vec<String> {
    parse(&file(name), name, text).expect_err("refused")
}

/// A project under a test home with the default seats and a `.mcp.json` defining bambu-studio-beta, over a stand-in
/// world where every CLI is signed in and no window is used. No herdr pane is named, so nothing wakes a real pane.
fn project(name: &str) -> (crate::nirvana::TestHomeGuard, Project) {
    std::env::remove_var("HERDR_PANE_ID");
    std::env::remove_var(crate::api::SOCKET_PATH_ENV_VAR);
    std::env::set_var("NIRVANA_NO_PANE", "1");
    let home = crate::nirvana::with_test_home(name);
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    let project = Project::at(home.path.join("project"));
    project.ensure().unwrap();
    std::fs::write(
        project.root.join(".mcp.json"),
        r#"{"mcpServers": {"bambu-studio-beta": {"command": "uv"}}}"#,
    )
    .unwrap();
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    (home, project)
}

fn write_profile(project: &Project, name: &str, text: &str) {
    std::fs::create_dir_all(folder(&project.root)).unwrap();
    std::fs::write(path(&project.root, name), text).unwrap();
}

fn request(project: &Project, order: &str, seat: &str) -> SeatRequest {
    SeatRequest {
        seat: seat.into(),
        order_file: project.root.join("order.md"),
        order: order.into(),
        run: "r1".into(),
        class: None,
        size: None,
        needs: Vec::new(),
        not_family: Vec::new(),
        mode: None,
        effort: None,
        reason: None,
        worktree: false,
        auto: false,
        send_ok: None,
    }
}

/// The seat the rule gives the class with this profile's narrowing and its needs.
fn rule_seat(project: &Project, class: &str, profile: &Profile) -> nirvana_seat::Pick {
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let mut pick = Request::new(class);
    let needs =
        crate::nirvana_gui_tools::seat_needs(&table, &project.root, &joined_needs(&[], profile))
            .unwrap();
    (pick.needs, pick.claude_only) = (needs.rest, needs.claude_only);
    narrow(&mut pick, profile);
    nirvana_seat::pick(&table, &nirvana_seat::live_world(&table), &pick).unwrap()
}

fn seated_of(project: &Project, order: &str) -> Seated {
    replay(&project.journal("r1").read().0)
        .order(order)
        .expect("seated")
        .seated
        .clone()
}

// ---- the file ----

#[test]
fn p33_a_profile_parses_every_key_and_its_body() {
    let profile = parsed(&SLICER.replace(
        "effort: medium\n",
        "effort: medium\nmodel: \"claude-sonnet-5-5\"\nfamily: anthropic\n# a comment\n\n",
    ));
    assert_eq!(
        profile.description,
        "Slices and sends prints in the beta Bambu Studio through its server."
    );
    assert_eq!(profile.class.as_deref(), Some("sweep.mechanical"));
    assert_eq!(profile.model.as_deref(), Some("claude-sonnet-5-5"));
    assert_eq!(profile.family.as_deref(), Some("anthropic"));
    assert_eq!(profile.effort, Some(Effort::Medium));
    assert_eq!(profile.needs, ["bambu-studio-beta"]);
    assert!(profile.disallowed_tools.is_empty());
    assert_eq!(
        profile.body,
        "You drive Bambu Studio only through the bambu-studio-beta tools.\nowns: src/never.rs"
    );
    // CRLF, as a Windows editor saves it, and a byte-order mark.
    let crlf = format!("\u{feff}{}", SLICER.replace('\n', "\r\n"));
    assert_eq!(parsed(&crlf).needs, ["bambu-studio-beta"]);
}

#[test]
fn p33_the_parser_refuses_each_bad_profile_naming_the_file_and_line() {
    let shown = file("slicer").display().to_string();
    // An unknown key, with the file and its line, as `.nirvana/seats.toml` is refused.
    let found = refusals(
        "slicer",
        &SLICER.replace("effort: medium\n", "effort: medium\ncolour: red\n"),
    );
    assert_eq!(
        found,
        [format!(
            "{shown}: line 5: unknown key `colour` (the keys: description, class, model, family, effort, needs, \
             disallowed_tools)"
        )]
    );
    // A Send key, by name: plan 31's approval is per order.
    for key in ["send_ok", "send-ok", "Send"] {
        let found = refusals(
            "slicer",
            &SLICER.replace("effort: medium\n", &format!("{key}: true\n")),
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with(&format!(
                "{shown}: line 4: `{key}`: a profile never grants a Send"
            )),
            "{found:?}"
        );
    }
    // The body may not grant a Send either.
    let found = refusals("slicer", &format!("{SLICER}Pass --send-ok yourself.\n"));
    assert!(found[0].contains("the body grants a Send"), "{found:?}");
    // The description: at most 120 characters, and required.
    let long = "d".repeat(DESCRIPTION_MAX + 1);
    let found = refusals(
        "slicer",
        &SLICER.replace(
            "description: Slices and sends prints in the beta Bambu Studio through its server.",
            &format!("description: {long}"),
        ),
    );
    assert_eq!(
        found,
        [format!(
            "{shown}: line 2: description: 121 characters; at most 120, the line Nirvana Claude picks by"
        )]
    );
    let exact = "d".repeat(DESCRIPTION_MAX);
    parsed(&SLICER.replace(
        "Slices and sends prints in the beta Bambu Studio through its server.",
        &exact,
    ));
    let found = refusals("slicer", "---\nclass: plan\n---\nbody\n");
    assert!(found[0].contains("no `description`"), "{found:?}");
    // The body: at most 4,000 characters.
    let at_most = format!("---\ndescription: d\n---\n{}\n", "b".repeat(BODY_MAX));
    assert_eq!(parsed(&at_most).body.chars().count(), BODY_MAX);
    let found = refusals(
        "slicer",
        &format!("---\ndescription: d\n---\n{}\n", "b".repeat(BODY_MAX + 1)),
    );
    assert_eq!(
        found,
        [format!(
            "{shown}: the body is 4001 characters; at most 4000, because it is paid on every call of the worker \
             (decision 0023)"
        )]
    );
    // The name: [a-z0-9-]{1,32}.
    for bad in ["Slicer", "slicer_2", "", &"s".repeat(NAME_MAX + 1)] {
        let found = refusals(bad, SLICER);
        assert!(
            found[0].contains(&format!("the name `{bad}` is not")),
            "{found:?}"
        );
    }
    assert!(valid_name(&"s".repeat(NAME_MAX)) && valid_name("bambu-2"));
    // A duplicate, a list over lines, a bad effort, no front matter: each refused, all of them printed.
    let found = refusals(
        "slicer",
        "---\ndescription: d\ndescription: e\nneeds:\n  - a\neffort: huge\n---\n",
    );
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(
        found[0].ends_with("line 3: duplicate `description`"),
        "{found:?}"
    );
    assert!(
        found[1].contains("line 4: needs: expected a list on one line"),
        "{found:?}"
    );
    assert!(
        found[2].contains("line 5: expected `key: value`"),
        "{found:?}"
    );
    assert!(
        found[3].contains("line 6: effort: `huge` is none of"),
        "{found:?}"
    );
    assert!(refusals("slicer", "description: d\n")[0].contains("line 1: a profile starts with"));
    assert!(refusals("slicer", "---\ndescription: d\n")[0].contains("no closing `---`"));
}

// ---- seating ----

#[test]
fn p33_the_profile_narrows_the_seats_and_its_effort_replaces_the_class_effort() {
    let (_home, project) = project("p33-narrow");
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let world = nirvana_seat::live_world(&table);
    let plain = nirvana_seat::pick(&table, &world, &Request::new("sweep.mechanical")).unwrap();
    // Without a profile the class seats on flash at low: the rule has something to narrow.
    assert_eq!((plain.seat.as_str(), plain.effort), ("flash", Effort::Low));
    let with = |model: Option<&str>, family: Option<&str>, effort: Option<Effort>| {
        let mut request = Request::new("sweep.mechanical");
        request.profile = Some(nirvana_seat::Narrow {
            name: "slicer".into(),
            model: model.map(str::to_owned),
            family: family.map(str::to_owned),
            effort,
        });
        nirvana_seat::pick(&table, &world, &request)
    };
    let sonnet = with(Some("claude-sonnet-5-5"), None, None).unwrap();
    assert_eq!(sonnet.seat, "sonnet");
    assert_eq!(sonnet.profile.as_deref(), Some("slicer"));
    assert!(
        sonnet.reason.contains("; profile slicer"),
        "{}",
        sonnet.reason
    );
    assert!(
        sonnet
            .runners_up
            .iter()
            .any(|loser| loser.seat == "flash"
                && loser.why == "profile slicer asks for model claude-sonnet-5-5; this seat is gemini-3.8-flash-{effort}"),
        "{:?}",
        sonnet.runners_up
    );
    let anthropic = with(None, Some("anthropic"), None).unwrap();
    assert_eq!(anthropic.family, "anthropic");
    // An agy model is named without its effort.
    assert_eq!(
        with(Some("gemini-3.8-flash"), None, None).unwrap().seat,
        "flash"
    );
    // §2.2: "`effort` replaces the class effort" (sweep.mechanical's is low).
    let medium = with(Some("claude-sonnet-5-5"), None, Some(Effort::Medium)).unwrap();
    assert_eq!(medium.effort, Effort::Medium, "{}", medium.reason);
    nirvana_seat::set_test_world(None);
}

#[test]
fn p33_no_seat_fits_the_profile_and_the_pick_names_it() {
    let (_home, project) = project("p33-no-fit");
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let world = nirvana_seat::live_world(&table);
    let mut narrowed = Request::new("sweep.mechanical");
    narrowed.profile = Some(nirvana_seat::Narrow {
        name: "drafter".into(),
        model: Some("claude-opus-5-5".into()),
        family: None,
        effort: None,
    });
    let none = nirvana_seat::pick(&table, &world, &narrowed).unwrap_err();
    assert!(
        none.reason
            .starts_with("with profile drafter: no seat left for sweep.mechanical: "),
        "{}",
        none.reason
    );
    assert!(
        none.reason.contains(
            "haiku (profile drafter asks for model claude-opus-5-5; this seat is claude-haiku-5-5)"
        ),
        "{}",
        none.reason
    );
    // `seat run` with the profile waits as an unseated order does: the no-seat exit, nothing journaled.
    write_profile(
        &project,
        "drafter",
        "---\ndescription: Drafts on Opus.\nclass: sweep.mechanical\nmodel: claude-opus-5-5\n---\nYou draft.\n",
    );
    std::fs::write(project.root.join("order.md"), "# Draft it\n").unwrap();
    let (code, why) = nirvana_worker::seat_order_with(
        &project,
        &request(&project, "d1", "haiku"),
        Some("drafter"),
    )
    .unwrap_err();
    assert_eq!(code, nirvana_seat::NO_SEAT_EXIT, "{why}");
    assert!(why.starts_with("no seat: with profile drafter: "), "{why}");
    assert!(replay(&project.journal("r1").read().0)
        .order("d1")
        .is_none());
    nirvana_seat::set_test_world(None);
}

#[test]
fn p33_seat_pick_takes_the_profiles_class_and_joins_its_needs() {
    let (_home, project) = project("p33-pick");
    write_profile(&project, "slicer", SLICER);
    let args = |extra: &[&str]| -> Vec<String> {
        ["--profile", "slicer"]
            .iter()
            .chain(extra)
            .map(|arg| (*arg).to_owned())
            .collect()
    };
    let (_, picked) = nirvana_seat::pick_args_for(&project.root, &args(&[])).unwrap();
    assert_eq!(picked.request.class, "sweep.mechanical");
    // The profile's server need is routed as plan 29's: no seat capability, a claude seat only.
    assert!(
        picked.request.needs.is_empty(),
        "{:?}",
        picked.request.needs
    );
    assert!(picked.request.claude_only.is_some());
    assert_eq!(
        picked.request.profile.as_ref().map(|narrow| narrow.effort),
        Some(Some(Effort::Medium))
    );
    // A class the flags name stands.
    let (_, picked) =
        nirvana_seat::pick_args_for(&project.root, &args(&["--class", "chore.script"])).unwrap();
    assert_eq!(picked.request.class, "chore.script");
    let why = nirvana_seat::pick_args_for(&project.root, &["--profile".into(), "nope".into()])
        .unwrap_err();
    assert!(why.starts_with("no profile nope: "), "{why}");
    nirvana_seat::set_test_world(None);
}

/// Review of the contract: the copy, its sha in the journal, the `Profile:` line P33b reads, and an edit made after
/// seating never reaching the worker, through `seat run` and `seat exec` with a stand-in for the worker's program.
#[cfg(windows)]
#[test]
fn p33_seat_run_copies_the_profile_with_its_sha_and_an_edit_after_seating_never_reaches_the_worker()
{
    let (_home, project) = project("p33-copy");
    write_profile(&project, "slicer", SLICER);
    let profile = parsed(SLICER);
    let seat = rule_seat(&project, "sweep.mechanical", &profile);
    assert_eq!(seat.cli, "claude", "the server need takes a claude seat");
    std::fs::write(project.root.join("order.md"), "# Slice the jig\n").unwrap();
    let live = nirvana_worker::take_live(&project, "r1", "s1")
        .unwrap()
        .unwrap();
    let outcome = nirvana_worker::seat_order_with(
        &project,
        &request(&project, "s1", &seat.seat),
        Some("slicer"),
    )
    .unwrap();
    drop(live);
    let sha = sha256_hex(SLICER.as_bytes());
    assert_eq!(outcome["profile"]["name"], "slicer", "{outcome}");
    assert_eq!(outcome["profile"]["sha256"], sha.as_str(), "{outcome}");
    let run_dir = project.run_dir("r1");
    let copy = copy_path(&run_dir, "slicer", &sha);
    assert_eq!(std::fs::read(&copy).unwrap(), SLICER.as_bytes());
    // The `seated` event: the class from the profile, its needs joined, the profile name and the sha.
    let seated = seated_of(&project, "s1");
    assert_eq!(seated.class, "sweep.mechanical");
    assert_eq!(seated.needs, ["bambu-studio-beta"]);
    assert!(
        seated
            .reason
            .ends_with(&format!("; profile slicer sha256 {sha}")),
        "{}",
        seated.reason
    );
    let line = std::fs::read_to_string(project.journal("r1").path)
        .unwrap()
        .lines()
        .find(|line| line.contains("\"seated\"") && line.contains("\"s1\""))
        .unwrap()
        .to_owned();
    assert!(
        line.contains(&format!("profile slicer sha256 {sha}")),
        "{line}"
    );
    // The order copy: P33b's contract line at line start, the sha line, and no body for a claude worker.
    let brief = std::fs::read_to_string(&seated.order_file).unwrap();
    assert!(
        brief.starts_with(&format!(
            "Profile: slicer\nProfile sha256: {sha}\n\n# Slice the jig"
        )),
        "{brief}"
    );
    assert!(!brief.contains("You drive Bambu Studio"), "{brief}");
    assert_eq!(crate::nirvana_gate::spec(&brief).owns, Vec::<String>::new());
    // The owner edits the profile after seating; the worker still gets the copy it was seated with.
    write_profile(
        &project,
        "slicer",
        &SLICER.replace("only through", "never through"),
    );
    crate::nirvana_worker::TEST_COMMAND.with_borrow_mut(|slot| {
        *slot = Some(vec!["cmd.exe".into(), "/c".into(), "exit /b 0".into()]);
    });
    let mut out = Vec::new();
    nirvana_worker::exec(&project, "r1", "s1", None, &mut out).unwrap();
    crate::nirvana_worker::TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
    let argv = project
        .journal("r1")
        .read()
        .0
        .into_iter()
        .find_map(|event| match event.kind {
            Kind::Started { order, argv, .. } if order == "s1" => Some(argv),
            _ => None,
        })
        .unwrap_or_else(|| panic!("started: {}", String::from_utf8_lossy(&out)));
    let at = argv
        .iter()
        .position(|arg| arg == "--append-system-prompt-file")
        .expect("--append-system-prompt-file");
    assert_eq!(
        argv[at + 1],
        body_path(&run_dir, "slicer", &sha).display().to_string()
    );
    assert_eq!(
        std::fs::read_to_string(&argv[at + 1]).unwrap(),
        "You drive Bambu Studio only through the bambu-studio-beta tools.\nowns: src/never.rs\n"
    );
    assert!(
        !argv.iter().any(|arg| arg == "--disallowedTools"),
        "{argv:?}"
    );
    nirvana_seat::set_test_world(None);
}

#[test]
fn p33_the_claude_worker_gets_the_disallowed_tools_and_a_changed_copy_never_starts() {
    let (_home, project) = project("p33-claude-argv");
    let text = SLICER.replace(
        "disallowed_tools: []",
        "disallowed_tools: [Bash, \"Edit(src/**)\"]",
    );
    write_profile(&project, "careful", &text);
    let profile = parsed(&text);
    // A profile that forbids tools takes only a claude seat: only the claude argv carries --disallowedTools.
    let mut agy_only = Request::new("sweep.mechanical");
    narrow(&mut agy_only, &profile);
    assert!(
        agy_only
            .claude_only
            .as_deref()
            .is_some_and(|why| why.starts_with("profile slicer forbids tools (Bash, Edit(src/**))")),
        "{:?}",
        agy_only.claude_only
    );
    let seat = rule_seat(&project, "sweep.mechanical", &profile);
    std::fs::write(project.root.join("order.md"), "# Slice it\n").unwrap();
    let _live = nirvana_worker::take_live(&project, "r1", "c1")
        .unwrap()
        .unwrap();
    nirvana_worker::seat_order_with(
        &project,
        &request(&project, "c1", &seat.seat),
        Some("careful"),
    )
    .unwrap();
    let seated = seated_of(&project, "c1");
    let run_dir = project.run_dir("r1");
    let argv = nirvana_worker::with_profile(vec!["claude".into()], &run_dir, &seated).unwrap();
    let at = argv
        .iter()
        .position(|arg| arg == "--disallowedTools")
        .unwrap();
    assert_eq!(argv[at + 1], "Bash,Edit(src/**)");
    assert!(argv.iter().any(|arg| arg == "--append-system-prompt-file"));
    // An order with no profile is untouched.
    let mut plain = seated.clone();
    let plain_file = project.root.join("plain.md");
    std::fs::write(&plain_file, "# Plain\n").unwrap();
    plain.order_file = plain_file.display().to_string();
    assert_eq!(
        nirvana_worker::with_profile(vec!["claude".into()], &run_dir, &plain).unwrap(),
        ["claude"]
    );
    // The run's copy changed since seating: the worker never starts with it.
    let copy = copy_path(&run_dir, "careful", &sha256_hex(text.as_bytes()));
    std::fs::write(&copy, text.replace("Bash", "Read")).unwrap();
    let why = nirvana_worker::with_profile(vec!["claude".into()], &run_dir, &seated).unwrap_err();
    assert!(
        why.starts_with("c1 was seated with profile careful: the profile copy ")
            && why.contains(" has sha256 "),
        "{why}"
    );
    nirvana_seat::set_test_world(None);
}

#[test]
fn p33_the_agy_worker_gets_the_body_quoted_at_the_top_of_its_order() {
    let (_home, project) = project("p33-agy");
    let text = "---\ndescription: Sweeps on flash.\nclass: sweep.mechanical\nfamily: google\n---\nSweep only.\n\ntest: never\n";
    write_profile(&project, "sweeper", text);
    let profile = parsed(text);
    let seat = rule_seat(&project, "sweep.mechanical", &profile);
    assert_eq!((seat.seat.as_str(), seat.cli.as_str()), ("flash", "agy"));
    std::fs::write(
        project.root.join("order.md"),
        "# Sweep the docs\nowns: docs/*.md\n",
    )
    .unwrap();
    let _live = nirvana_worker::take_live(&project, "r1", "a1")
        .unwrap()
        .unwrap();
    nirvana_worker::seat_order_with(&project, &request(&project, "a1", "flash"), Some("sweeper"))
        .unwrap();
    let seated = seated_of(&project, "a1");
    let brief = std::fs::read_to_string(&seated.order_file).unwrap();
    let sha = sha256_hex(text.as_bytes());
    assert!(
        brief.starts_with(&format!(
            "## Profile: sweeper\n\n> Sweep only.\n>\n> test: never\n\n(end of profile sweeper)\n\nProfile: sweeper\n\
             Profile sha256: {sha}\n\n# Sweep the docs\nowns: docs/*.md\n"
        )),
        "{brief}"
    );
    // A body line never reads as a gate line: the order's own `owns:` stands, and no `test:` comes from the body.
    let spec = crate::nirvana_gate::spec(&brief);
    assert_eq!(spec.owns, ["docs/*.md"]);
    assert_eq!(spec.test, None);
    // agy takes no system prompt: its argv is unchanged, and the order copy is already right.
    let argv =
        nirvana_worker::with_profile(vec!["agy".into()], &project.run_dir("r1"), &seated).unwrap();
    assert_eq!(argv, ["agy"]);
    assert_eq!(std::fs::read_to_string(&seated.order_file).unwrap(), brief);
    nirvana_seat::set_test_world(None);
}

// ---- inheritance ----

#[test]
fn p33_a_send_back_child_keeps_the_parents_profile_copy_on_either_cli() {
    let (_home, project) = project("p33-send-back");
    write_profile(&project, "slicer", SLICER);
    let seat = rule_seat(&project, "sweep.mechanical", &parsed(SLICER));
    std::fs::write(project.root.join("order.md"), "# Slice the jig\n").unwrap();
    let _live = nirvana_worker::take_live(&project, "r1", "p1")
        .unwrap()
        .unwrap();
    nirvana_worker::seat_order_with(
        &project,
        &request(&project, "p1", &seat.seat),
        Some("slicer"),
    )
    .unwrap();
    let parent = seated_of(&project, "p1");
    let sha = sha256_hex(SLICER.as_bytes());
    // The owner edits the profile before the send-back: the child keeps the parent's copy.
    write_profile(
        &project,
        "slicer",
        &SLICER.replace("only through", "never through"),
    );
    // O5's own brief, from the parent's order copy, as `herdr gate run` writes it.
    let root_text = std::fs::read_to_string(&parent.order_file).unwrap();
    let brief = crate::nirvana_gate::send_back_brief(
        "p1",
        "p1",
        1,
        &[],
        &root_text,
        "nirvana-integrate/r1",
    );
    assert_eq!(
        in_order(&brief),
        Some(Ok(Ref {
            name: "slicer".into(),
            sha: sha.clone()
        }))
    );
    let run_dir = project.run_dir("r1");
    let mut child = parent.clone();
    let child_file = project.root.join("child.md");
    std::fs::write(&child_file, &brief).unwrap();
    child.order_file = child_file.display().to_string();
    // On claude: the parent's body as its system prompt.
    let argv = nirvana_worker::with_profile(vec!["claude".into()], &run_dir, &child).unwrap();
    assert_eq!(
        argv,
        [
            "claude".to_owned(),
            "--append-system-prompt-file".to_owned(),
            body_path(&run_dir, "slicer", &sha).display().to_string()
        ]
    );
    assert!(std::fs::read_to_string(&argv[2])
        .unwrap()
        .contains("only through"));
    // On agy: the parent's body at the top of the child's order, once, however often it starts.
    child.cli = "agy".into();
    nirvana_worker::with_profile(vec!["agy".into()], &run_dir, &child).unwrap();
    nirvana_worker::with_profile(vec!["agy".into()], &run_dir, &child).unwrap();
    let agy_brief = std::fs::read_to_string(&child_file).unwrap();
    assert!(
        agy_brief.starts_with("## Profile: slicer\n\n> You drive Bambu Studio only through"),
        "{agy_brief}"
    );
    assert_eq!(
        agy_brief.matches("## Profile: slicer").count(),
        1,
        "{agy_brief}"
    );
    assert!(agy_brief.ends_with(&brief), "{agy_brief}");
    // Back on claude, the agy section goes again.
    child.cli = "claude".into();
    nirvana_worker::with_profile(vec!["claude".into()], &run_dir, &child).unwrap();
    assert_eq!(std::fs::read_to_string(&child_file).unwrap(), brief);
    nirvana_seat::set_test_world(None);
}

#[test]
fn p33_a_reseat_keeps_the_profile_copy_and_its_narrowing() {
    let (_home, project) = project("p33-reseat");
    let limit = nirvana_worker::RateLimit {
        signal: "rate_limit_event".into(),
        pool: Some("max".into()),
        window: None,
        resets_at: None,
        detail: String::new(),
    };
    // The parent sits on haiku, by an override where the rule would seat it elsewhere; the reseat drops an override
    // on the spent pool, so the rule and the profile pick the child's seat.
    let reseat = |order: &str, profile: &str, text: &str| -> (Seated, String) {
        write_profile(&project, profile, text);
        std::fs::write(project.root.join("order.md"), "# Sweep it\n").unwrap();
        let _live = nirvana_worker::take_live(&project, "r1", order)
            .unwrap()
            .unwrap();
        let mut req = request(&project, order, "haiku");
        req.class = Some("sweep.mechanical".into());
        req.reason = Some("the parent is on claude for this test".into());
        nirvana_worker::seat_order_with(&project, &req, Some(profile)).unwrap();
        let seated = seated_of(&project, order);
        let journal = project.journal("r1");
        let child = nirvana_worker::child_id(&replay(&journal.read().0), order);
        let _child_live = nirvana_worker::take_live(&project, "r1", &child)
            .unwrap()
            .unwrap();
        let mut out = Vec::new();
        nirvana_worker::rate_limited(
            &project,
            &journal,
            "r1",
            &seated,
            &limit,
            &nirvana_worker::Stream::new("claude"),
            &mut out,
        )
        .unwrap();
        (seated, child)
    };
    // Narrowed to anthropic, a spent max pool leaves no seat: without the profile the reseat would take flash.
    let narrowed = "---\ndescription: Sweeps on Claude.\nfamily: anthropic\n---\nSweep.\n";
    let (seated, child) = reseat("n1", "claude-only", narrowed);
    assert_eq!(seated.cli, "claude");
    let state = replay(&project.journal("r1").read().0);
    assert!(
        state.order(&child).is_none(),
        "re-seated off the profile's family"
    );
    assert!(project
        .journal("r1")
        .read()
        .0
        .iter()
        .any(|event| matches!(&event.kind, Kind::Asked { order: Some(o), .. } if o == "n1")));
    // Not narrowed: the reseat takes flash, and the child keeps the parent's copy, at the top of its agy order.
    let open = "---\ndescription: Sweeps anywhere.\neffort: low\n---\nSweep carefully.\n";
    let (seated, child) = reseat("o1", "anywhere", &open.replace("Sweeps", "Sweeps (claude)"));
    assert_eq!(seated.seat, "haiku");
    write_profile(&project, "anywhere", &open.replace("carefully", "fast"));
    let child_seated = seated_of(&project, &child);
    assert_eq!(child_seated.cli, "agy", "{}", child_seated.seat);
    let sha = sha256_hex(open.replace("Sweeps", "Sweeps (claude)").as_bytes());
    let child_text = std::fs::read_to_string(&child_seated.order_file).unwrap();
    assert_eq!(
        in_order(&child_text),
        Some(Ok(Ref {
            name: "anywhere".into(),
            sha
        }))
    );
    nirvana_worker::with_profile(vec!["agy".into()], &project.run_dir("r1"), &child_seated)
        .unwrap();
    let child_text = std::fs::read_to_string(&child_seated.order_file).unwrap();
    assert!(
        child_text.starts_with("## Profile: anywhere\n\n> Sweep carefully.\n"),
        "{child_text}"
    );
    nirvana_seat::set_test_world(None);
}

#[test]
fn p33_an_order_naming_a_profile_is_seated_only_with_that_profile() {
    let (_home, project) = project("p33-named");
    write_profile(&project, "slicer", SLICER);
    let seat = rule_seat(&project, "sweep.mechanical", &parsed(SLICER)).seat;
    let _live: Vec<_> = ["w1", "w2", "w3", "w4"]
        .iter()
        .map(|order| {
            nirvana_worker::take_live(&project, "r1", order)
                .unwrap()
                .unwrap()
        })
        .collect();
    // The order says `Profile:` itself: seated without `--profile`, herdr would never apply what P33b reads.
    std::fs::write(project.root.join("order.md"), "# Slice\nProfile: slicer\n").unwrap();
    let mut req = request(&project, "w1", &seat);
    req.class = Some("sweep.mechanical".into());
    let (code, why) = nirvana_worker::seat_order_with(&project, &req, None).unwrap_err();
    assert_eq!(code, 2);
    assert!(why.contains("seat it with `--profile slicer`"), "{why}");
    // With that profile the line is herdr's own, once.
    nirvana_worker::seat_order_with(&project, &request(&project, "w2", &seat), Some("slicer"))
        .unwrap();
    let brief = std::fs::read_to_string(seated_of(&project, "w2").order_file).unwrap();
    assert_eq!(brief.matches("Profile: slicer\n").count(), 1, "{brief}");
    // A booking `seat pick --book` made without the profile is not seated with it, nor one made with it without.
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let mut leases = nirvana_seat::Leases::default();
    let world = nirvana_seat::live_world(&table);
    let mut plain = Request::new("sweep.mechanical");
    plain.claude_only = Some("a server".into());
    let plain_pick = nirvana_seat::pick(&table, &world, &plain).unwrap();
    nirvana_seat::book_pick(
        &mut leases,
        &table,
        &plain_pick,
        Some("w3"),
        Some("r1"),
        world.now,
    );
    let mut narrowed = plain.clone();
    narrow(&mut narrowed, &parsed(SLICER));
    let narrowed_pick = nirvana_seat::pick(&table, &world, &narrowed).unwrap();
    nirvana_seat::book_pick(
        &mut leases,
        &table,
        &narrowed_pick,
        Some("w4"),
        Some("r1"),
        world.now,
    );
    nirvana_seat::write_leases(
        &nirvana_seat::dir().join(nirvana_seat::LEASES_FILE),
        &leases,
    )
    .unwrap();
    std::fs::write(project.root.join("order.md"), "# Slice\n").unwrap();
    let (_, why) = nirvana_worker::seat_order_with(
        &project,
        &request(&project, "w3", &plain_pick.seat),
        Some("slicer"),
    )
    .unwrap_err();
    assert!(why.contains("was booked without a profile"), "{why}");
    let (_, why) = nirvana_worker::seat_order_with(
        &project,
        &request(&project, "w4", &narrowed_pick.seat),
        None,
    )
    .unwrap_err();
    assert!(why.contains("was booked with profile slicer"), "{why}");
    nirvana_seat::set_test_world(None);
}

// ---- designing them, and the digest ----

#[test]
fn p33_the_digest_lists_each_profile_in_at_most_ten_lines() {
    let dir = std::env::temp_dir().join(format!("p33-digest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(digest_lines(&dir).is_empty(), "no profiles, no lines");
    std::fs::create_dir_all(folder(&dir)).unwrap();
    std::fs::write(
        path(&dir, "slicer"),
        SLICER.replace("effort: medium", "model: claude-sonnet-5-5"),
    )
    .unwrap();
    std::fs::write(path(&dir, "broken"), "---\ncolour: red\n---\n").unwrap();
    std::fs::write(
        path(&dir, "sweeper"),
        "---\ndescription: Sweeps docs.\nfamily: google\n---\n",
    )
    .unwrap();
    assert_eq!(
        digest_lines(&dir),
        [
            "Profiles here (an order that fits one is seated with `--profile <name>`):",
            "- broken: refused; `herdr profile check broken`",
            "- slicer: Slices and sends prints in the beta Bambu Studio through its server. (model claude-sonnet-5-5; \
             needs bambu-studio-beta)",
            "- sweeper: Sweeps docs. (family google)",
        ]
    );
    for n in 0..9 {
        std::fs::write(path(&dir, &format!("p{n}")), "---\ndescription: d\n---\n").unwrap();
    }
    let lines = digest_lines(&dir);
    assert_eq!(lines.len(), 1 + DIGEST_PROFILES + 1, "{lines:?}");
    assert_eq!(lines.last().unwrap(), "- and 2 more: `herdr profile list`");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn p33_new_writes_a_template_naming_the_projects_servers_and_check_reads_the_table() {
    let (_home, project) = project("p33-new");
    let file = new_profile(&project.root, "slicer").unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("# This project's MCP servers (.mcp.json): bambu-studio-beta\n"),
        "{text}"
    );
    assert!(parse(&file, "slicer", &text).is_ok());
    assert!(new_profile(&project.root, "slicer")
        .unwrap_err()
        .ends_with("is already there"));
    assert!(new_profile(&project.root, "Bad").is_err());
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let servers = vec!["bambu-studio-beta".to_owned()];
    assert!(table_problems(&file, &parsed(SLICER), &table, &servers).is_empty());
    let typo = parsed(
        &SLICER
            .replace("class: sweep.mechanical", "class: gui")
            .replace("effort: medium", "model: claude-sonet-5-5")
            .replace("[bambu-studio-beta]", "[bambu-studio]"),
    );
    let found = table_problems(&file, &typo, &table, &servers);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(
        found[0].ends_with("class gui: no such class in seats.toml"),
        "{found:?}"
    );
    assert!(
        found[1].ends_with("no seat in seats.toml has model claude-sonet-5-5"),
        "{found:?}"
    );
    assert!(
        found[2]
            .ends_with("needs bambu-studio: neither a seat capability nor a server in .mcp.json"),
        "{found:?}"
    );
    nirvana_seat::set_test_world(None);
}

/// Plan 33 review nit: `--override` onto a seat the profile refuses is refused, naming the profile's reason; an
/// override onto a seat it takes still stands. §2.2's "`--override` keeps its meaning": an override never takes a seat
/// a filter skipped, and a profile is such a filter.
#[test]
fn p33_an_override_onto_a_seat_the_profile_refuses_is_refused() {
    let (_home, project) = project("p33-override");
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let world = nirvana_seat::live_world(&table);
    let over = |seat: &str| {
        let mut request = Request::new("sweep.mechanical");
        request.profile = Some(nirvana_seat::Narrow {
            name: "slicer".into(),
            model: Some("claude-sonnet-5-5".into()),
            family: None,
            effort: None,
        });
        request.override_seat = Some((seat.into(), "the owner asked".into()));
        nirvana_seat::pick(&table, &world, &request)
    };
    let refused = over("flash").expect_err("the profile refuses flash");
    assert_eq!(
        refused.reason,
        "with profile slicer: override flash refused: profile slicer asks for model claude-sonnet-5-5; this seat is gemini-3.8-flash-{effort}"
    );
    assert_eq!(over("sonnet").expect("sonnet").seat, "sonnet");
}
