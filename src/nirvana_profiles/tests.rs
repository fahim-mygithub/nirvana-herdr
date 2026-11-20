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

/// Journals `order`'s end as a failed attempt does (`nirvana_worker::end_failed`).
fn end_order(project: &Project, order: &str) {
    project
        .journal("r1")
        .append(
            crate::nirvana_run::now(),
            Kind::Ended {
                order: Some(order.into()),
                outcome: "failed".into(),
                why: Some("a test ended it".into()),
                exit: Some(3),
                pools: Vec::new(),
                pools_basis: None,
            },
        )
        .unwrap();
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
             disallowed_tools, skills)"
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
    // Without a profile the class seats on hydrogen at low (plan 71: it took flash's place): the rule has something to
    // narrow.
    assert_eq!(
        (plain.seat.as_str(), plain.effort),
        ("hydrogen", Effort::Low)
    );
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
            .any(|loser| loser.seat == "hydrogen"
                && loser.why == "profile slicer asks for model claude-sonnet-5-5; this seat is gemini-4-hydrogen-{effort}"),
        "{:?}",
        sonnet.runners_up
    );
    let anthropic = with(None, Some("anthropic"), None).unwrap();
    assert_eq!(anthropic.family, "anthropic");
    // An agy model is named without its effort.
    assert_eq!(
        with(Some("gemini-4-hydrogen"), None, None).unwrap().seat,
        "hydrogen"
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
    assert_eq!((seat.seat.as_str(), seat.cli.as_str()), ("hydrogen", "agy"));
    std::fs::write(
        project.root.join("order.md"),
        "# Sweep the docs\nowns: docs/*.md\n",
    )
    .unwrap();
    let _live = nirvana_worker::take_live(&project, "r1", "a1")
        .unwrap()
        .unwrap();
    nirvana_worker::seat_order_with(
        &project,
        &request(&project, "a1", "hydrogen"),
        Some("sweeper"),
    )
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
    // Plan 47 P47b: no profile of the project's own, and the built-in scholar is still there.
    let scholar = format!(
        "- scholar (built into herdr): {}",
        summary(
            &parse(
                &builtin_shown("scholar"),
                "scholar",
                builtin("scholar").unwrap().profile
            )
            .unwrap()
        )
    );
    assert_eq!(
        digest_lines(&dir),
        [
            "Profiles here (an order that fits one is seated with `--profile <name>`):".to_owned(),
            scholar.clone()
        ],
        "no profiles of its own, the built-in only"
    );
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
            "Profiles here (an order that fits one is seated with `--profile <name>`):".to_owned(),
            "- broken: refused; `herdr profile check broken`".to_owned(),
            scholar,
            "- slicer: Slices and sends prints in the beta Bambu Studio through its server. (model claude-sonnet-5-5; \
             needs bambu-studio-beta)"
                .to_owned(),
            "- sweeper: Sweeps docs. (family google)".to_owned(),
        ]
    );
    for n in 0..9 {
        std::fs::write(path(&dir, &format!("p{n}")), "---\ndescription: d\n---\n").unwrap();
    }
    let lines = digest_lines(&dir);
    assert_eq!(lines.len(), 1 + DIGEST_PROFILES + 1, "{lines:?}");
    // Twelve of the project's own and the built-in: ten lines, then the three more.
    assert_eq!(lines.last().unwrap(), "- and 3 more: `herdr profile list`");
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
    // Plan 71: hydrogen, the seat that took flash's place (an override of flash is refused as off first).
    let refused = over("hydrogen").expect_err("the profile refuses hydrogen");
    assert_eq!(
        refused.reason,
        "with profile slicer: override hydrogen refused: profile slicer asks for model claude-sonnet-5-5; this seat is gemini-4-hydrogen-{effort}"
    );
    assert_eq!(over("sonnet").expect("sonnet").seat, "sonnet");
}

// ---- plan 35 P35b: a denying profile on agy ----

/// §3.4: a profile whose every tool maps to a measured agy rule seats on agy, denied through `--settings`; one with an
/// unmapped tool stays claude-only, and fails an agy order outright, naming the tool.
#[test]
fn p35b_a_profile_denying_mapped_tools_seats_on_agy_and_an_unmapped_one_does_not() {
    let (_home, project) = project("p35b-agy-deny");
    let text =
        "---\ndescription: Sweeps without writing.\nclass: sweep.mechanical\nfamily: google\n\
                disallowed_tools: [Write, Bash, mcp__bambu__print]\n---\nSweep only.\n";
    write_profile(&project, "reader", text);
    let profile = parsed(text);
    let mut ask = Request::new("sweep.mechanical");
    narrow(&mut ask, &profile);
    assert_eq!(ask.claude_only, None);
    let seat = rule_seat(&project, "sweep.mechanical", &profile);
    assert_eq!(seat.cli, "agy", "{}", seat.seat);
    std::fs::write(
        project.root.join("order.md"),
        "# Sweep the docs\nowns: docs/*.md\n",
    )
    .unwrap();
    let _live = nirvana_worker::take_live(&project, "r1", "a1")
        .unwrap()
        .unwrap();
    nirvana_worker::seat_order_with(
        &project,
        &request(&project, "a1", &seat.seat),
        Some("reader"),
    )
    .unwrap();
    let seated = seated_of(&project, "a1");
    let run_dir = project.run_dir("r1");
    assert_eq!(
        nirvana_worker::with_profile(vec!["agy".into()], &run_dir, &seated).unwrap(),
        ["agy"]
    );
    assert_eq!(
        nirvana_worker::agy_denials(&run_dir, &seated).unwrap(),
        ["write_file(*)", "command(*)", "mcp(bambu/print)"]
    );
    // An unmapped tool: claude-only at the seat, and an agy worker of it never starts.
    let unmapped = parsed(&text.replace("mcp__bambu__print", "WebSearch"));
    let mut ask = Request::new("sweep.mechanical");
    narrow(&mut ask, &unmapped);
    let why = ask.claude_only.unwrap_or_default();
    assert!(
        why.starts_with("profile slicer forbids tools (Write, Bash, WebSearch), and agy has no measured rule for WebSearch"),
        "{why}"
    );
    let copy = copy_path(&run_dir, "reader", &sha256_hex(text.as_bytes()));
    let changed = text.replace("mcp__bambu__print", "WebSearch");
    std::fs::write(&copy, &changed).unwrap();
    let mut moved = seated.clone();
    let order = std::fs::read_to_string(&seated.order_file)
        .unwrap()
        .replace(
            &sha256_hex(text.as_bytes()),
            &sha256_hex(changed.as_bytes()),
        );
    std::fs::write(&seated.order_file, order).unwrap();
    std::fs::rename(
        &copy,
        copy_path(&run_dir, "reader", &sha256_hex(changed.as_bytes())),
    )
    .unwrap();
    moved.cli = "agy".into();
    let why = nirvana_worker::with_profile(vec!["agy".into()], &run_dir, &moved).unwrap_err();
    assert_eq!(
        why,
        "a1 was seated with profile reader on agy, and agy cannot be denied WebSearch: no measured agy rule \
         matches (plan 35 §3.4)"
    );
    nirvana_seat::set_test_world(None);
}

/// Plan 37 §1.5: `herdr profile new <name> --from web-researcher` writes herdr's template, which parses as a
/// research.web profile whose body holds the brief analysis's rules 1-3 and 7; an unknown template is refused, and
/// so is a file already there.
#[test]
fn p37_profile_new_from_web_researcher_writes_the_template() {
    let (_home, project) = project("p37-template");
    let file = new_profile_from(&project.root, "web-researcher", "web-researcher").unwrap();
    assert_eq!(file, path(&project.root, "web-researcher"));
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(text, WEB_RESEARCHER);
    let profile = parse(&file, "web-researcher", &text).expect("parses");
    assert_eq!(profile.class.as_deref(), Some("research.web"));
    assert!(profile.body.chars().count() <= BODY_MAX);
    for rule in [
        "Never cite a URL from memory or from a search result.",
        "python -m yt_dlp --skip-download --print",
        "/.rss",
        "There is no YouTube connector",
        "Done means a command, and its output goes into `tests`",
        "run \nevery long step in the foreground",
    ] {
        let rule = rule.replace(" \n", " ");
        let flat = profile.body.replace('\n', " ");
        assert!(flat.contains(&rule), "no {rule:?} in {}", profile.body);
    }
    // Review N6: the body is paid on every call, so it holds rules only; the incidents are in the seat skill.
    for incident in ["reel-motion-gemini", "r2 ", "v12-smudge-fix", "MotoProject"] {
        assert!(
            !profile.body.contains(incident),
            "{incident:?} in {}",
            profile.body
        );
    }
    assert!(
        new_profile_from(&project.root, "web-researcher", "web-researcher")
            .unwrap_err()
            .ends_with("is already there")
    );
    let unknown = new_profile_from(&project.root, "other", "no-such").unwrap_err();
    assert!(unknown.contains("no template \"no-such\""), "{unknown}");
    assert!(!path(&project.root, "other").exists());
    assert_eq!(template_text("web-researcher"), Some(WEB_RESEARCHER));
}

/// Plan 40 M1: the new-profile template names no model id; its model line points at the seat table.
#[test]
fn the_template_points_at_the_seat_table_for_a_model() {
    let text = template(&[]);
    let model = text.lines().find(|line| line.starts_with("# model:")).expect("a model line");
    assert_eq!(model, "# model: a model id that herdr seat list shows");
    assert!(!text.contains("claude-"), "{text}");
}

// ---- plan 45 P45c: skills, a profile's own servers, shapes ----

/// A profile's own servers, made up: a fake command, never a definition from this machine.
const OWN_SERVERS: &str =
    r#"{"mcpServers": {"fake-tool": {"command": "fake-mcp-not-real", "args": ["--fake"]}}}"#;
const OPERATOR: &str = "---\ndescription: Drives the fake tool through its own server.\nclass: sweep.mechanical\n---\n\
You drive the fake tool only through its server.\n";

fn write_folder_profile(project: &Project, name: &str, text: &str, servers: Option<&str>) -> PathBuf {
    let dir = folder_of(&project.root, name);
    std::fs::create_dir_all(dir.join(KNOWLEDGE)).unwrap();
    std::fs::write(dir.join(PROFILE_FILE), text).unwrap();
    if let Some(servers) = servers {
        std::fs::write(dir.join(SERVERS_FILE), servers).unwrap();
    }
    dir.join(PROFILE_FILE)
}

fn skill(root: &Path, name: &str) {
    let dir = root.join(".claude").join("skills").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: the {name} skill.\n---\nbody\n"),
    )
    .unwrap();
}

#[test]
fn p92_skills_parse_and_check_refuses_a_skill_the_project_lacks() {
    let (_home, project) = project("p92-skills");
    let text = SLICER.replace("effort: medium\n", "effort: medium\nskills: [alpha, \"beta\"]\n");
    let profile = parsed(&text);
    assert_eq!(profile.skills, Some(vec!["alpha".to_owned(), "beta".to_owned()]));
    // P45c review F6: `skills: []` is none, the key absent is all, and a name twice is refused.
    assert_eq!(parsed(SLICER).skills, None);
    assert_eq!(
        parsed(&SLICER.replace("effort: medium\n", "effort: medium\nskills: []\n")).skills,
        Some(Vec::new())
    );
    let twice = refusals(
        "slicer",
        &SLICER.replace("effort: medium\n", "effort: medium\nskills: [alpha, beta, alpha]\n"),
    );
    assert!(
        twice.len() == 1 && twice[0].ends_with("skills: `alpha` is named twice"),
        "{twice:?}"
    );
    let found = refusals("slicer", &SLICER.replace("effort: medium\n", "effort: medium\nskills: [..]\n"));
    assert!(
        found.len() == 1 && found[0].ends_with("skills: `..` is not a skill's folder name"),
        "{found:?}"
    );
    skill(&project.root, "alpha");
    let file = path(&project.root, "slicer");
    let found = project_problems(&project.root, &file, &profile);
    assert_eq!(
        found,
        [format!(
            "{}: skills beta: no such skill in .claude/skills (the project's: alpha)",
            file.display()
        )]
    );
    nirvana_seat::set_test_world(None);
}

#[test]
fn p92_a_claude_worker_with_skills_gets_one_line_naming_them_and_none_without() {
    let (_home, project) = project("p92-skills-line");
    let run_dir = project.run_dir("r1");
    let found = Ref {
        name: "slicer".into(),
        sha: "a".repeat(64),
    };
    let with = parsed(&SLICER.replace("effort: medium\n", "effort: medium\nskills: [alpha, beta]\n"));
    let args = claude_args(&run_dir, &found, &with).unwrap();
    let body = std::fs::read_to_string(&args[1]).unwrap();
    assert!(
        body.ends_with(
            "owns: src/never.rs\n\nOf this project's skills, use only these: alpha, beta. Leave the others unused.\n"
        ),
        "{body}"
    );
    // An absent `skills` key changes nothing: the body alone, as before plan 45.
    let without = Ref {
        name: "plain".into(),
        sha: "b".repeat(64),
    };
    let args = claude_args(&run_dir, &without, &parsed(SLICER)).unwrap();
    assert_eq!(
        std::fs::read_to_string(&args[1]).unwrap(),
        "You drive Bambu Studio only through the bambu-studio-beta tools.\nowns: src/never.rs\n"
    );
    // P45c review F6: `skills: []` asks for none, in so many words.
    let none = Ref {
        name: "bare".into(),
        sha: "c".repeat(64),
    };
    let empty = parsed(&SLICER.replace("effort: medium\n", "effort: medium\nskills: []\n"));
    let args = claude_args(&run_dir, &none, &empty).unwrap();
    assert!(
        std::fs::read_to_string(&args[1])
            .unwrap()
            .ends_with("owns: src/never.rs\n\nUse none of this project's skills.\n"),
        "{args:?}"
    );
    nirvana_seat::set_test_world(None);
}

/// The plan's L3 verifier: "an order seated with the profile has the server, one seated without has neither". The
/// launch is `order_argv_seated` with what seating journaled, as `seat exec` calls it (`with_order_servers`).
#[test]
fn p92_a_profiles_own_servers_reach_its_order_pinned_by_the_run_copy() {
    let (_home, project) = project("p92-own-servers");
    // agy signed out: the claude path, whose config file is read here.
    nirvana_seat::set_test_world(Some(BTreeMap::from([(
        "agy".to_owned(),
        "signed out".to_owned(),
    )])));
    let profile_mcp = folder_of(&project.root, "operator").join(SERVERS_FILE);
    write_folder_profile(&project, "operator", OPERATOR, Some(OWN_SERVERS));
    let loaded = load(&project.root, "operator").unwrap();
    assert_eq!(loaded.profile.servers, ["fake-tool"]);
    // The sha covers the profile and the pin, and the pin holds the mcp.json's sha256 and names only.
    let pin = ServersPin {
        sha256: sha256_hex(OWN_SERVERS.as_bytes()),
        names: vec!["fake-tool".into()],
        claude_only: None,
    };
    assert_eq!(loaded.sha, seated_sha(OPERATOR.as_bytes(), Some(&pin.bytes())));
    let seat = rule_seat(&project, "sweep.mechanical", &loaded.profile);
    std::fs::write(project.root.join("order.md"), "# Run the fake tool\n").unwrap();
    let live = nirvana_worker::take_live(&project, "r1", "s1")
        .unwrap()
        .unwrap();
    nirvana_worker::seat_order_with(
        &project,
        &request(&project, "s1", &seat.seat),
        Some("operator"),
    )
    .unwrap();
    drop(live);
    let seated = seated_of(&project, "s1");
    let run_dir = project.run_dir("r1");
    assert!(seated.needs.is_empty(), "no needs line: {:?}", seated.needs);
    assert_eq!(
        std::fs::read(servers_copy_path(&run_dir, "operator", &loaded.sha)).unwrap(),
        OWN_SERVERS.as_bytes()
    );
    let brief = std::fs::read_to_string(&seated.order_file).unwrap();
    assert!(
        brief.contains(crate::nirvana_gui_tools::DEVICE_LINE),
        "{brief}"
    );
    let launch = |seated: &Seated| {
        crate::nirvana_gui_tools::order_argv_seated(
            vec!["claude".into(), "-p".into()],
            &project.root,
            Path::new(&seated.cwd),
            &run_dir,
            &seated.order,
            &seated.needs,
            &crate::nirvana_bladebro::Seat::default(),
            Ok(vec!["code"]),
            &mut Vec::new(),
        )
    };
    let command_of = |argv: &[String]| {
        let at = argv
            .iter()
            .position(|arg| arg == "--mcp-config")
            .unwrap_or_else(|| panic!("--mcp-config: {argv:?}"));
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&argv[at + 1]).unwrap()).unwrap();
        let servers: Vec<String> = config["mcpServers"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(servers, ["fake-tool"], "{config}");
        assert_eq!(argv[at + 2..], ["--allowedTools", "mcp__fake-tool__*"]);
        config["mcpServers"]["fake-tool"]["command"]
            .as_str()
            .unwrap_or("")
            .to_owned()
    };
    let argv = launch(&seated).unwrap();
    assert_eq!(command_of(&argv), "fake-mcp-not-real");
    // The owner edits the profile's mcp.json mid-run: the running order keeps the copy it was seated with.
    std::fs::write(&profile_mcp, OWN_SERVERS.replace("fake-mcp-not-real", "edited-fake")).unwrap();
    let argv = launch(&seated).unwrap();
    assert_eq!(command_of(&argv), "fake-mcp-not-real");
    // The run's servers copy changed since seating: the order does not start.
    std::fs::write(
        servers_copy_path(&run_dir, "operator", &loaded.sha),
        OWN_SERVERS.replace("fake-mcp-not-real", "tampered-fake"),
    )
    .unwrap();
    let why = launch(&seated).unwrap_err();
    assert!(
        why.starts_with("s1 was seated with a profile's own MCP servers and cannot get them: the profile's servers ")
            && why.contains(" have sha256 "),
        "{why}"
    );
    // The pin changed since seating: the sha fails, as the profile copy's always did.
    let pin_file = pin_path(&run_dir, "operator", &loaded.sha);
    let pinned = std::fs::read(&pin_file).unwrap();
    assert_eq!(pinned, pin.bytes());
    std::fs::write(
        &pin_file,
        String::from_utf8(pin.bytes()).unwrap().replace("fake-tool", "fake-tool2"),
    )
    .unwrap();
    let why = launch(&seated).unwrap_err();
    assert!(
        why.contains("the profile copy ") && why.contains(" has sha256 "),
        "{why}"
    );
    std::fs::write(&pin_file, &pinned).unwrap();
    std::fs::write(servers_copy_path(&run_dir, "operator", &loaded.sha), OWN_SERVERS).unwrap();
    // P45c review2 B1 (e): the start of an attempt (a restart, a crash re-launch) takes the order's config out, and
    // leaves the copy; (a) so the same open order launches with the seated definitions, the owner's edit of the live
    // file (above, still in place) notwithstanding.
    crate::nirvana_gui_tools::remove_order_config(&run_dir, "s1", &mut Vec::new());
    assert!(servers_copy_path(&run_dir, "operator", &loaded.sha).exists());
    assert_eq!(command_of(&launch(&seated).unwrap()), "fake-mcp-not-real");
    // While the order still wants a worker, a release keeps its copy.
    crate::nirvana_gui_tools::release_servers_copies(&run_dir, &mut Vec::new());
    assert!(servers_copy_path(&run_dir, "operator", &loaded.sha).exists());
    // (b) The order ends: the release takes the definitions copy out; the pin, with no definition, stays.
    end_order(&project, "s1");
    crate::nirvana_gui_tools::release_servers_copies(&run_dir, &mut Vec::new());
    assert!(!servers_copy_path(&run_dir, "operator", &loaded.sha).exists());
    assert!(!String::from_utf8(std::fs::read(&pin_file).unwrap())
        .unwrap()
        .contains("fake-mcp-not-real"));
    // A reader after the end needs names only, and has them.
    let after = from_run(
        &run_dir,
        &Ref {
            name: "operator".into(),
            sha: loaded.sha.clone(),
        },
    )
    .unwrap();
    assert_eq!(after.servers, ["fake-tool"]);
    // (d) A later start with the copy gone (a child seated with this reference after the order ended) takes the
    // profile's file, only at the pinned sha: the owner's edit above fails it, in words that say what to do; (c) the
    // file put back starts it.
    let why = launch(&seated).unwrap_err();
    assert!(
        why.contains("the profile's mcp.json changed since this order was seated")
            && why.contains("restore it, or seat the order again with `--profile operator`"),
        "{why}"
    );
    std::fs::write(&profile_mcp, OWN_SERVERS).unwrap();
    assert_eq!(command_of(&launch(&seated).unwrap()), "fake-mcp-not-real");
    assert!(
        !servers_copy_path(&run_dir, "operator", &loaded.sha).exists(),
        "the fallback writes no copy back"
    );
    // An order seated without the profile gets neither the server nor the line.
    let table = nirvana_seat::table_for(&project.root).unwrap();
    let plain_seat = nirvana_seat::pick(
        &table,
        &nirvana_seat::live_world(&table),
        &Request::new("sweep.mechanical"),
    )
    .unwrap();
    std::fs::write(project.root.join("order.md"), "# Run it plainly\n").unwrap();
    let mut plain_request = request(&project, "s2", &plain_seat.seat);
    plain_request.class = Some("sweep.mechanical".into());
    let live = nirvana_worker::take_live(&project, "r1", "s2")
        .unwrap()
        .unwrap();
    nirvana_worker::seat_order_with(&project, &plain_request, None).unwrap();
    drop(live);
    let plain = seated_of(&project, "s2");
    let brief = std::fs::read_to_string(&plain.order_file).unwrap();
    assert!(
        !brief.contains(crate::nirvana_gui_tools::DEVICE_LINE),
        "{brief}"
    );
    assert_eq!(launch(&plain).unwrap(), ["claude", "-p"]);
    nirvana_seat::set_test_world(None);
}

#[test]
fn p92_a_profile_server_the_project_defines_is_refused_and_agys_limits_hold() {
    let (_home, project) = project("p92-server-rules");
    let clash = r#"{"mcpServers": {"bambu-studio-beta": {"command": "fake-mcp-not-real"}}}"#;
    let file = write_folder_profile(&project, "clash", OPERATOR, Some(clash));
    let found = project_problems(
        &project.root,
        &file,
        &parse(&file, "clash", OPERATOR).unwrap(),
    );
    assert!(
        found.len() == 1
            && found[0].contains("server bambu-studio-beta is in the project's .mcp.json too"),
        "{found:?}"
    );
    let why = load(&project.root, "clash").unwrap_err();
    assert!(
        why.contains("server bambu-studio-beta is in the project's .mcp.json too"),
        "{why}"
    );
    // A definition agy cannot take keeps every order of the profile on claude, as a `needs` server's does.
    let remote = r#"{"mcpServers": {"fake-remote": {"command": "fake-mcp-not-real", "args": ["${HOME}"]}}}"#;
    write_folder_profile(&project, "remote", OPERATOR, Some(remote));
    let loaded = load(&project.root, "remote").unwrap();
    let mut pick = Request::new("sweep.mechanical");
    narrow(&mut pick, &loaded.profile);
    assert!(
        pick.claude_only.as_deref().is_some_and(|why| why
            .starts_with("profile remote's own server: only a claude worker can take fake-remote: fake-remote uses")),
        "{:?}",
        pick.claude_only
    );
    // Its Debug names the servers, never a definition: a definition can hold a token.
    let shown = format!("{loaded:?}");
    assert!(shown.contains("fake-remote") && !shown.contains("HOME"), "{shown}");
    // A `needs` naming the profile's own server is refused: it is granted with no needs line.
    let needing = OPERATOR.replace(
        "class: sweep.mechanical\n",
        "class: sweep.mechanical\nneeds: [fake-tool]\n",
    );
    let file = write_folder_profile(&project, "needing", &needing, Some(OWN_SERVERS));
    let found = project_problems(
        &project.root,
        &file,
        &parse(&file, "needing", &needing).unwrap(),
    );
    assert!(
        found.len() == 1 && found[0].ends_with("take it out of needs"),
        "{found:?}"
    );
    nirvana_seat::set_test_world(None);
}

#[test]
fn p92_every_shape_writes_the_folder_form_and_passes_check() {
    let (_home, project) = project("p92-shapes");
    let root = &project.root;
    let table = nirvana_seat::table_for(root);
    assert!(table.is_ok(), "the default seat table");
    let servers = crate::nirvana_gui_tools::project_servers(root).unwrap();
    for shape_name in SHAPES {
        let name = format!("my-{shape_name}");
        let file = new_profile_shape(root, &name, shape_name).unwrap();
        assert_eq!(file, folder_of(root, &name).join(PROFILE_FILE));
        let (_, listed, parsed) = list(root)
            .into_iter()
            .find(|(listed, _, _)| *listed == name)
            .expect("listed");
        let problems = check_one(root, &name, &listed, &parsed, &table, &servers);
        assert!(problems.is_empty(), "{shape_name}: {problems:?}");
        let offer = crate::nirvana_knowledge::offer_uncounted(root, root, &name);
        assert!(
            !offer.slugs.is_empty(),
            "{shape_name}: the starter cards are offered"
        );
        let again = new_profile_shape(root, &name, shape_name).unwrap_err();
        assert!(again.contains("is already there"), "{again}");
    }
    let reviewer = std::fs::read_to_string(folder_of(root, "my-reviewer").join(PROFILE_FILE)).unwrap();
    for line in [
        "TODO(owner): name what this reviewer judges",
        "1. Open every file you judge yourself. Never judge from a file name",
        "The worst defect sets the ceiling.",
        "your Work Report's `sources` lists every file you opened and every card you read",
        "7. Never edit, re-render, download or upload anything.",
    ] {
        assert!(reviewer.contains(line), "{line}: {reviewer}");
    }
    // The visual pins are suggestions, commented: they narrow the seats to the ones that read video natively.
    let parsed = parse(&file("my-reviewer"), "my-reviewer", &reviewer).unwrap();
    assert_eq!((parsed.class, parsed.needs.len()), (None, 0), "{reviewer}");
    assert_eq!(
        std::fs::read_to_string(folder_of(root, "my-researcher").join(PROFILE_FILE)).unwrap(),
        WEB_RESEARCHER
    );
    assert!(new_profile_shape(root, "x", "critic")
        .unwrap_err()
        .starts_with("no shape \"critic\""));
    nirvana_seat::set_test_world(None);
}

// ---- P45c fix round ----

/// Review F3: a written-out value under a secret-looking key of a profile's own servers is noted, naming the server and
/// the key, never the value; a `${VAR}` value, one holding one, and an ordinary key are not; the note refuses nothing.
#[test]
fn p92_a_literal_token_in_a_profiles_servers_is_noted_by_key_never_by_value() {
    let (_home, project) = project("p92-secret-notes");
    let servers = r#"{"mcpServers": {"fake-tool": {"command": "fake-mcp-not-real",
        "env": {"API_TOKEN": "FAKE-TOKEN-123", "MODE": "fast", "Secret_Ref": "${FAKE_SECRET}", "PASSWORD": ""},
        "headers": {"Authorization": "Bearer ${FAKE_BEARER}", "X-Cookie": "FAKE-COOKIE-456"}}}}"#;
    let file = write_folder_profile(&project, "operator", OPERATOR, Some(servers));
    let mcp = folder_of(&project.root, "operator").join(SERVERS_FILE);
    // No `profiles/.gitignore` yet: the notes end with one saying no rule covers the file.
    let uncovered = notes(&project.root, "operator");
    assert_eq!(
        uncovered.last().map(String::as_str),
        Some(
            format!(
                "note: {}: no ignore rule covers it ({} is missing or has no `*/mcp.json` line), so a `git add` of \
                 the project would track the values above",
                mcp.display(),
                folder(&project.root).join(IGNORE_FILE).display()
            )
            .as_str()
        ),
        "{uncovered:?}"
    );
    assert_eq!(uncovered.len(), 3, "{uncovered:?}");
    assert!(ensure_ignore(&project.root).unwrap());
    let found = notes(&project.root, "operator");
    assert_eq!(
        found,
        [
            format!(
                "note: {}: server fake-tool: env API_TOKEN is written out, not `${{VAR}}`; put the value in the \
                 environment and write `${{API_TOKEN}}` (a literal is copied into each run that seats the profile)",
                mcp.display()
            ),
            format!(
                "note: {}: server fake-tool: headers X-Cookie is written out, not `${{VAR}}`; put the value in the \
                 environment and write `${{X-Cookie}}` (a literal is copied into each run that seats the profile)",
                mcp.display()
            ),
        ]
    );
    assert!(found.iter().all(|note| !note.contains("FAKE-TOKEN-123") && !note.contains("FAKE-COOKIE-456")));
    // A note, not a refusal: check passes and the profile loads.
    let table = nirvana_seat::table_for(&project.root);
    let names = crate::nirvana_gui_tools::project_servers(&project.root).unwrap();
    let parsed = parse(&file, "operator", OPERATOR);
    assert_eq!(check_one(&project.root, "operator", &file, &parsed, &table, &names), Vec::<String>::new());
    assert!(load(&project.root, "operator").is_ok());
    nirvana_seat::set_test_world(None);
}

/// Review F7: a new profile never takes a Windows device name, in any case, though such a name still parses and loads
/// (Windows itself will not let a test write `con.md`, so the load side is the name check `load` uses).
#[test]
fn p92_a_new_profile_never_takes_a_windows_device_name() {
    let (_home, project) = project("p92-device-names");
    for name in ["con", "nul", "aux", "prn", "com1", "lpt9"] {
        assert!(!valid_new_name(name), "{name}");
        assert!(valid_name(name), "{name} still names a profile on disk");
        let why = new_profile(&project.root, name).unwrap_err();
        assert!(why.contains("is a Windows device name"), "{name}: {why}");
        let why = new_profile_shape(&project.root, name, "builder").unwrap_err();
        assert!(why.contains("is a Windows device name"), "{name}: {why}");
    }
    assert!(crate::nirvana_knowledge::windows_device("CON.md"));
    assert!(crate::nirvana_knowledge::windows_device("Com1.txt"));
    for name in ["com10", "console", "con-x", "nully"] {
        assert!(valid_new_name(name), "{name}");
    }
    assert!(parse(&file("con"), "con", SLICER).is_ok());
    nirvana_seat::set_test_world(None);
}

/// Review F3, the lead's ruling: `.nirvana/.gitignore` is the owner's byte for byte; herdr writes
/// `profiles/.gitignore` (`*/mcp.json`) when it is missing, at the load that seats a profile with its own servers
/// (and `profile check`), never over a file that is there, an emptied one included. git itself is asked what it
/// ignores, and `list` never takes the file for a profile.
#[test]
fn p92_profiles_gitignore_keeps_a_profiles_servers_out_of_git_and_is_never_rewritten() {
    let (_home, project) = project("p92-ignore");
    let owners = std::fs::read_to_string(project.dir().join(".gitignore")).unwrap();
    let ignore = folder(&project.root).join(IGNORE_FILE);
    write_folder_profile(&project, "operator", OPERATOR, Some(OWN_SERVERS));
    write_profile(&project, "plain", SLICER);
    // A profile with no mcp.json writes nothing.
    load(&project.root, "plain").unwrap();
    assert!(!ignore.exists());
    assert!(!ignore_covers(&project.root));
    // Seating loads the profile: the file is written, as herdr's text.
    load(&project.root, "operator").unwrap();
    assert_eq!(std::fs::read_to_string(&ignore).unwrap(), IGNORE_TEXT);
    assert!(ignore_covers(&project.root));
    assert_eq!(
        std::fs::read_to_string(project.dir().join(".gitignore")).unwrap(),
        owners,
        "the project's .nirvana/.gitignore is untouched"
    );
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&project.root)
            .output()
            .unwrap()
    };
    assert!(git(&["init", "-q"]).status.success());
    let checked = git(&["check-ignore", "-q", ".nirvana/profiles/operator/mcp.json"]);
    assert_eq!(checked.status.code(), Some(0), "mcp.json is ignored");
    for tracked in [
        ".nirvana/profiles/operator/profile.md",
        ".nirvana/profiles/plain.md",
        ".nirvana/profiles/.gitignore",
    ] {
        let checked = git(&["check-ignore", "-q", tracked]);
        assert_eq!(checked.status.code(), Some(1), "{tracked} is tracked");
    }
    // `list` (and so the panel, check and the digest) never reads the file as a profile.
    let names: Vec<String> = list(&project.root).into_iter().map(|(name, _, _)| name).collect();
    assert_eq!(names, ["operator", "plain"]);
    // The owner empties it to track the servers: no later load or ensure writes it again.
    std::fs::write(&ignore, "").unwrap();
    load(&project.root, "operator").unwrap();
    assert!(!ensure_ignore(&project.root).unwrap());
    assert_eq!(std::fs::read_to_string(&ignore).unwrap(), "");
    assert!(!ignore_covers(&project.root));
    nirvana_seat::set_test_world(None);
}

/// Seats `order` with the operator profile on the rule's seat, as `herdr seat run --profile operator` does.
fn seat_operator(project: &Project, order: &str) -> Seated {
    let loaded = load(&project.root, "operator").unwrap();
    let seat = rule_seat(project, "sweep.mechanical", &loaded.profile);
    std::fs::write(project.root.join("order.md"), "# Run the fake tool\n").unwrap();
    let live = nirvana_worker::take_live(project, "r1", order)
        .unwrap()
        .unwrap();
    nirvana_worker::seat_order_with(project, &request(project, order, &seat.seat), Some("operator"))
        .unwrap();
    drop(live);
    seated_of(project, order)
}

fn launch_of(project: &Project, seated: &Seated) -> Result<Vec<String>, String> {
    crate::nirvana_gui_tools::order_argv_seated(
        vec!["claude".into(), "-p".into()],
        &project.root,
        Path::new(&seated.cwd),
        &project.run_dir("r1"),
        &seated.order,
        &seated.needs,
        &crate::nirvana_bladebro::Seat::default(),
        Ok(vec!["code"]),
        &mut Vec::new(),
    )
}

/// The `command` of the profile's server in the per-order config the argv names.
fn own_command(argv: &[String]) -> String {
    let at = argv
        .iter()
        .position(|arg| arg == "--mcp-config")
        .unwrap_or_else(|| panic!("--mcp-config: {argv:?}"));
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&argv[at + 1]).unwrap()).unwrap();
    config["mcpServers"]["fake-tool"]["command"]
        .as_str()
        .unwrap_or("")
        .to_owned()
}

/// P45c review2 B1, the sibling: two open orders seated with the same profile version share one copy. The first one's
/// end leaves it while the second may still start, and the second launches with the seated definitions after the
/// owner edits the live file; once both have ended, a supervisor's start (`sweep_leftovers`) takes it out.
#[test]
fn p92_the_servers_copy_lives_while_any_order_seated_with_it_may_start() {
    let (_home, project) = project("p92-copy-sibling");
    nirvana_seat::set_test_world(Some(BTreeMap::from([(
        "agy".to_owned(),
        "signed out".to_owned(),
    )])));
    let profile_mcp = folder_of(&project.root, "operator").join(SERVERS_FILE);
    write_folder_profile(&project, "operator", OPERATOR, Some(OWN_SERVERS));
    let first = seat_operator(&project, "s1");
    let run_dir = project.run_dir("r1");
    let found = in_order(&std::fs::read_to_string(&first.order_file).unwrap())
        .unwrap()
        .unwrap();
    let copy = servers_copy_path(&run_dir, &found.name, &found.sha);
    // The sibling: the same brief, so the same reference and the same copy (as a send-back child keeps its parent's).
    let mut second = first.clone();
    second.order = "s2".into();
    let brief = crate::nirvana_gui_tools::order_copy_path(Path::new(&first.cwd), "s2");
    std::fs::copy(&first.order_file, &brief).unwrap();
    second.order_file = brief.display().to_string();
    project
        .journal("r1")
        .append(crate::nirvana_run::now(), Kind::Seated(Box::new(second.clone())))
        .unwrap();
    end_order(&project, "s1");
    crate::nirvana_gui_tools::release_servers_copies(&run_dir, &mut Vec::new());
    assert!(copy.exists(), "s2 still wants a worker and holds the copy");
    std::fs::write(&profile_mcp, OWN_SERVERS.replace("fake-mcp-not-real", "edited-fake")).unwrap();
    assert_eq!(own_command(&launch_of(&project, &second).unwrap()), "fake-mcp-not-real");
    end_order(&project, "s2");
    let mut out = Vec::new();
    nirvana_worker::sweep_leftovers(&project, "r1", "none", &mut out);
    assert!(!copy.exists(), "{}", String::from_utf8_lossy(&out));
    assert!(pin_path(&run_dir, &found.name, &found.sha).exists());
    nirvana_seat::set_test_world(None);
}

/// P45c review2 B1, end to end through `seat exec`: the owner edits the live `mcp.json` after seating; the order's
/// start (whose start-of-attempt clean-up used to take the copy, so it read the edited file and refused) launches with
/// the seated definitions, and when the attempt fails and the order ends, its copy goes and its pin stays.
#[cfg(windows)]
#[test]
fn p92_seat_exec_starts_with_the_seated_servers_and_its_end_takes_the_copy() {
    let (_home, project) = project("p92-copy-exec");
    nirvana_seat::set_test_world(Some(BTreeMap::from([(
        "agy".to_owned(),
        "signed out".to_owned(),
    )])));
    let profile_mcp = folder_of(&project.root, "operator").join(SERVERS_FILE);
    write_folder_profile(&project, "operator", OPERATOR, Some(OWN_SERVERS));
    let seated = seat_operator(&project, "s1");
    let run_dir = project.run_dir("r1");
    let found = in_order(&std::fs::read_to_string(&seated.order_file).unwrap())
        .unwrap()
        .unwrap();
    let copy = servers_copy_path(&run_dir, &found.name, &found.sha);
    assert!(copy.exists());
    std::fs::write(&profile_mcp, OWN_SERVERS.replace("fake-mcp-not-real", "edited-fake")).unwrap();
    // The stand-in worker keeps the config it was started with, then fails with no report.
    let config = crate::nirvana_gui_tools::order_config_path(&run_dir, "s1");
    let kept = project.root.join("kept-config.json");
    // One argument each: `Command` would escape quotes inside one as `\"`, which cmd does not read.
    let windows = |path: &Path| path.display().to_string().replace('/', "\\");
    crate::nirvana_worker::TEST_COMMAND.with_borrow_mut(|slot| {
        *slot = Some(
            ["cmd.exe", "/c", "copy", "/y"]
                .into_iter()
                .map(str::to_owned)
                .chain([windows(&config), windows(&kept)])
                .chain([">nul", "&", "exit", "3"].into_iter().map(str::to_owned))
                .collect(),
        );
    });
    let mut out = Vec::new();
    let code = nirvana_worker::exec(&project, "r1", "s1", None, &mut out);
    crate::nirvana_worker::TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
    let view = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(code.unwrap(), 1, "{view}");
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&kept).unwrap_or_default()).unwrap_or_default();
    assert_eq!(
        config["mcpServers"]["fake-tool"]["command"], "fake-mcp-not-real",
        "the worker got the seated definitions: {view}"
    );
    let phase = replay(&project.journal("r1").read().0)
        .order("s1")
        .map(|known| known.phase);
    assert_eq!(phase, Some(crate::nirvana_run::Phase::Failed));
    assert!(!copy.exists(), "the order ended, so its copy went: {view}");
    assert!(pin_path(&run_dir, &found.name, &found.sha).exists());
    nirvana_seat::set_test_world(None);
}

/// P45c review2 C2: a profile whose own `class:` is a review class (here the gate's `review.adversary`) joins none of
/// its `needs`, though the caller names no class: the class default is applied before the guard.
#[test]
fn p92_a_profile_whose_own_class_reviews_joins_no_need() {
    let (_home, project) = project("p92-review-class-needs");
    let text = "---\ndescription: Reviews prints in the beta Bambu Studio.\nclass: review.adversary\n\
needs: [bambu-studio-beta]\n---\nYou review.\n";
    write_profile(&project, "odd", text);
    let mut plain = parsed(text);
    plain.needs.clear();
    let seat = rule_seat(&project, "review.adversary", &plain);
    std::fs::write(project.root.join("order.md"), "# Review it\n").unwrap();
    let live = nirvana_worker::take_live(&project, "r1", "q1")
        .unwrap()
        .unwrap();
    let seated =
        nirvana_worker::seat_order_with(&project, &request(&project, "q1", &seat.seat), Some("odd"));
    drop(live);
    seated.unwrap_or_else(|(code, why)| panic!("refused ({code}): {why}"));
    let seated = seated_of(&project, "q1");
    assert_eq!(seated.class, "review.adversary");
    assert!(seated.needs.is_empty(), "no profile need is joined: {:?}", seated.needs);
    nirvana_seat::set_test_world(None);
}

/// P45c review2 C1: the rule, by class family, so a review class not built yet is covered.
#[test]
fn p92_no_review_teach_or_acceptance_class_takes_a_profiles_servers() {
    for class in [
        crate::nirvana_knowledge::TEACH_CLASS,
        crate::nirvana_review::CLASS,
        "review.adversary",
        "review.some-later-kind",
        "acceptance.author",
    ] {
        assert!(takes_no_profile_servers(class), "{class}");
    }
    for class in [
        "build.bounded",
        "sweep.mechanical",
        "judge.visual",
        "research.web",
        "review",
        "reviewer.x",
    ] {
        assert!(!takes_no_profile_servers(class), "{class}");
    }
}

/// P45c review2 D1: `profile check` writes `profiles/.gitignore` only when some profile has an `mcp.json`, and a later
/// negation line takes the cover back, as git reads it.
#[test]
fn p92_the_ignore_is_written_only_for_servers_and_a_negation_uncovers() {
    let (_home, project) = project("p92-ignore-negation");
    let ignore = folder(&project.root).join(IGNORE_FILE);
    write_profile(&project, "plain", SLICER);
    assert!(!ensure_ignore_for_servers(&project.root).unwrap());
    assert!(!ignore.exists(), "no profile has servers: nothing is written");
    write_folder_profile(&project, "operator", OPERATOR, Some(OWN_SERVERS));
    assert!(ensure_ignore_for_servers(&project.root).unwrap());
    assert!(ignore_covers(&project.root));
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&project.root)
            .output()
            .unwrap()
    };
    assert!(git(&["init", "-q"]).status.success());
    let ignored = || {
        git(&["check-ignore", "-q", ".nirvana/profiles/operator/mcp.json"])
            .status
            .code()
            == Some(0)
    };
    for (text, covered) in [
        (format!("{IGNORE_TEXT}!*/mcp.json\n"), false),
        (format!("{IGNORE_TEXT}!operator/mcp.json\n"), false),
        (format!("{IGNORE_TEXT}!/*/mcp.json\n*/mcp.json\n"), true),
        (format!("{IGNORE_TEXT}!**/mcp.json\n"), false),
    ] {
        std::fs::write(&ignore, &text).unwrap();
        assert_eq!(ignore_covers(&project.root), covered, "{text}");
        assert_eq!(ignored(), covered, "git reads it so too: {text}");
    }
    // The deeper file wins over `.nirvana/.gitignore` when it speaks; when it says nothing, the outer rule covers.
    let outer = project.dir().join(".gitignore");
    let owners = std::fs::read_to_string(&outer).unwrap();
    std::fs::write(&outer, format!("{owners}profiles/*/mcp.json\n")).unwrap();
    std::fs::write(&ignore, "!*/mcp.json\n").unwrap();
    assert!(!ignore_covers(&project.root));
    assert!(!ignored());
    std::fs::write(&ignore, "").unwrap();
    assert!(ignore_covers(&project.root));
    assert!(ignored());
    nirvana_seat::set_test_world(None);
}

// ---- plan 47 P47b: the built-in scholar ----

fn scholar() -> &'static Builtin {
    builtin("scholar").expect("scholar is built in")
}

fn scholar_body() -> String {
    parse(&builtin_shown("scholar"), "scholar", scholar().profile)
        .expect("parses")
        .body
}

/// A folder with no `.nirvana` at all and no test home: the built-in needs no file, and nothing here may write one.
fn bare_folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "p47b-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every file under `root`, `/`-separated and sorted, but those under `.nirvana/runs/` and `.nirvana/orders/`: herdr's
/// run state for this machine, which the project's `.nirvana/.gitignore` ignores.
fn project_files(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .display()
                .to_string()
                .replace('\\', "/");
            if rel == ".nirvana/runs" || rel == ".nirvana/orders" {
                continue;
            }
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                out.push(rel);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// The card paths a scholar order's brief names, from the worker's folder.
fn card_paths(sha: &str) -> Vec<String> {
    scholar()
        .cards
        .iter()
        .map(|(slug, _)| {
            format!(
                ".nirvana/runs/r1/profiles/scholar-{}/knowledge/{slug}.md",
                &sha[..16]
            )
        })
        .collect()
}

/// Item 1: the shipped text passes herdr's own parsers and `profile check`'s functions, as a project profile would.
#[test]
fn p47b_the_built_in_text_passes_profile_check() {
    let (_home, project) = project("p47b-check");
    let found = scholar();
    let entry = listed(&project.root)
        .into_iter()
        .find(|found| found.name == "scholar")
        .expect("listed");
    assert!(entry.builtin);
    let table = nirvana_seat::table_for(&project.root);
    assert!(table.is_ok(), "{table:?}");
    let servers = crate::nirvana_gui_tools::project_servers(&project.root).unwrap_or_default();
    assert_eq!(
        check_one(
            &project.root,
            "scholar",
            &entry.file,
            &entry.parsed,
            &table,
            &servers
        ),
        Vec::<String>::new()
    );
    assert_eq!(builtin_card_problems(found), Vec::<String>::new());
    assert_eq!(
        check_note(&project.root, "scholar"),
        format!(" ({}, 9 cards)", builtin_word())
    );
    let profile = entry.parsed.expect("the profile parser takes it");
    assert_eq!(profile.class.as_deref(), Some("research.web"));
    assert!(
        profile.model.is_none()
            && profile.family.is_none()
            && profile.effort.is_none()
            && profile.needs.is_empty()
            && profile.skills.is_none()
            && profile.disallowed_tools.is_empty(),
        "no seat pin and no tool rule: {profile:?}"
    );
    assert!(profile.body.chars().count() <= BODY_MAX);
    let slugs: Vec<&str> = found.cards.iter().map(|(slug, _)| *slug).collect();
    assert_eq!(
        slugs,
        [
            "benchmark",
            "breadth-then-cluster",
            "competitive-analysis",
            "decision-matrix",
            "fetch-ladder",
            "hypothesis-elimination",
            "source-of-truth",
            "synthesis",
            "trajectory"
        ]
    );
    assert!(slugs.len() <= crate::nirvana_lessons::CARDS_OFFERED);
    // What the binary carries is the shipped folder, byte for byte.
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("nirvana/profiles/scholar");
    assert_eq!(
        std::fs::read(shipped.join(PROFILE_FILE)).unwrap(),
        found.profile.as_bytes()
    );
    for (slug, text) in found.cards {
        assert_eq!(
            std::fs::read(shipped.join(KNOWLEDGE).join(format!("{slug}.md"))).unwrap(),
            text.as_bytes(),
            "{slug}"
        );
    }
    nirvana_seat::set_test_world(None);
}

/// Item 2, ruling R5: a worker may download a clip or take screenshots to see frames, and still says which it saw.
#[test]
fn p47b_the_text_lets_a_worker_see_frames_and_still_say_which() {
    let flat = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let body = flat(&scholar_body());
    for said in [
        "you may download the clip with the download tool your session has, such as yt-dlp, or take browser screenshots",
        "Name each frame you opened and what is in it.",
        "A video read only by transcript: say you saw no frames.",
    ] {
        assert!(body.contains(said), "{said:?} in the body:\n{body}");
    }
    let ladder = flat(
        scholar()
            .cards
            .iter()
            .find(|(slug, _)| *slug == "fetch-ladder")
            .expect("the ladder")
            .1,
    );
    for said in [
        "A transcript comes from the browser's \"Show transcript\" panel",
        "with the download tool your session has, such as yt-dlp; agy reads a local clip's frames natively with its \
         file read.",
        "Or take browser screenshots of the moments you need.",
        "Name each frame you opened and what is in it; if you read only the transcript, say you saw no frames.",
    ] {
        assert!(ladder.contains(said), "{said:?} in the ladder:\n{ladder}");
    }
    assert!(
        !ladder.contains("do not retry it"),
        "the clip download is allowed now"
    );
}

/// Item 3: a project with no `.nirvana` folder at all has the scholar in every reader, and nothing is written.
#[test]
fn p47b_a_project_with_no_nirvana_folder_has_the_scholar_everywhere() {
    let dir = bare_folder("none");
    let sha = sha256_hex(scholar().profile.as_bytes());
    // `listed`: the CLI's list, show and check, the Preload group, the Profiles panel and the detector read it.
    let found: Vec<(String, bool, bool)> = listed(&dir)
        .into_iter()
        .map(|found| (found.name, found.builtin, found.parsed.is_ok()))
        .collect();
    assert_eq!(found, [("scholar".to_owned(), true, true)]);
    assert!(list(&dir).is_empty(), "no file of the project's own");
    // `load`: seat run and seat pick.
    let loaded = load(&dir, "scholar").expect("loads with no file");
    assert!(loaded.builtin && loaded.servers.is_none());
    assert_eq!(loaded.sha, sha);
    assert_eq!(loaded.bytes, scholar().profile.as_bytes());
    // The session digest.
    let digest = digest_lines(&dir);
    assert_eq!(digest.len(), 2, "{digest:?}");
    assert!(
        digest[1].starts_with("- scholar (built into herdr): Researches a question"),
        "{digest:?}"
    );
    // `fitting()` and the 0.90.0 fit line.
    let fit = crate::nirvana_profile_detect::fitting(&dir, "research.web", &[]);
    assert_eq!(fit, ["scholar"]);
    assert_eq!(
        nirvana_worker::fit_line(&fit, "research.web").as_deref(),
        Some(
            "herdr: profile scholar fits this order (class research.web); pass --profile scholar to seat with it"
        )
    );
    assert!(crate::nirvana_profile_detect::fitting(&dir, "build.bounded", &[]).is_empty());
    // `profile list` and `profile show`, as the CLI prints them.
    let mut out = Vec::new();
    assert_eq!(list_to(&dir, false, &mut out), 0);
    let listed_text = String::from_utf8(out).unwrap();
    println!("herdr profile list:\n{listed_text}");
    assert!(
        listed_text
            .lines()
            .any(|line| line.starts_with("scholar: Researches a question")),
        "{listed_text}"
    );
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(
        show_to(&dir, "scholar", &mut out, &mut err),
        0,
        "{}",
        String::from_utf8_lossy(&err)
    );
    let shown = String::from_utf8(out).unwrap();
    println!("herdr profile show scholar:\n{shown}");
    assert!(shown.ends_with(scholar().profile), "{shown}");
    // Nothing was written: the folder is as empty as it was.
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "no .nirvana folder"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Item 5: `profile list` (a word, and `--json` a field) and `profile show` (a first line) mark the built-in.
#[test]
fn p47b_list_and_show_mark_the_built_in_and_name_the_copy() {
    let dir = bare_folder("marks");
    let sha = sha256_hex(scholar().profile.as_bytes());
    let mut out = Vec::new();
    list_to(&dir, false, &mut out);
    let text = String::from_utf8(out).unwrap();
    let line = text
        .lines()
        .find(|line| line.starts_with("scholar:"))
        .expect("a scholar line");
    assert!(
        line.ends_with(&format!(
            " [built into herdr {}]",
            crate::nirvana::VERSION
        )),
        "{line}"
    );
    let mut out = Vec::new();
    list_to(&dir, true, &mut out);
    let rows: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(rows[0]["name"], "scholar", "{rows}");
    assert_eq!(rows[0]["builtin"], true, "{rows}");
    assert!(rows[0]["file"].is_null(), "{rows}");
    assert_eq!(rows[0]["class"], "research.web", "{rows}");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    show_to(&dir, "scholar", &mut out, &mut err);
    let shown = String::from_utf8(out).unwrap();
    let mut lines = shown.lines();
    let first = lines.next().unwrap();
    assert_eq!(first, builtin_head("scholar", &sha));
    assert!(
        first.starts_with(&format!(
            "scholar is built into herdr (Nirvana-Herdr {}) · sha256 {sha}",
            crate::nirvana::VERSION
        )),
        "{first}"
    );
    assert!(
        first.contains("`herdr profile new scholar --from scholar` writes the project's own copy"),
        "{first}"
    );
    assert_eq!(
        lines.next().unwrap(),
        "cards (9, offered to an order seated with it, from its run folder): benchmark, breadth-then-cluster, \
         competitive-analysis, decision-matrix, fetch-ladder, hypothesis-elimination, source-of-truth, synthesis, \
         trajectory"
    );
    // A project profile's row carries no mark.
    std::fs::create_dir_all(folder(&dir)).unwrap();
    std::fs::write(path(&dir, "slicer"), SLICER).unwrap();
    let mut out = Vec::new();
    list_to(&dir, true, &mut out);
    let rows: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(rows[1]["name"], "slicer", "{rows}");
    assert_eq!(rows[1]["builtin"], false, "{rows}");
    let mut out = Vec::new();
    list_to(&dir, false, &mut out);
    let text = String::from_utf8(out).unwrap();
    assert!(!text.contains("no profiles of this project's own"), "{text}");
    assert!(
        !text
            .lines()
            .find(|line| line.starts_with("slicer:"))
            .unwrap()
            .contains("built into"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Item 4: a project's own `scholar`, a file or a folder, is the only one; a broken one shows broken.
#[test]
fn p47b_a_projects_own_scholar_is_the_only_one_and_a_broken_one_shows_broken() {
    let dir = bare_folder("own");
    std::fs::create_dir_all(folder(&dir)).unwrap();
    let own = "---\ndescription: This project's own scholar.\nclass: research.web\n---\nYou research for this project.\n";
    std::fs::write(path(&dir, "scholar"), own).unwrap();
    let found = listed(&dir);
    assert_eq!(found.len(), 1, "one scholar, not two");
    assert!(!found[0].builtin);
    assert_eq!(found[0].file, path(&dir, "scholar"));
    assert_eq!(
        found[0].parsed.as_ref().unwrap().description,
        "This project's own scholar."
    );
    let loaded = load(&dir, "scholar").unwrap();
    assert!(!loaded.builtin);
    assert_eq!(loaded.bytes, own.as_bytes());
    assert!(builtin_here(&dir, "scholar").is_none());
    // Its cards are its own: a one-file profile keeps none, and none of the built-in's is offered.
    let offer = crate::nirvana_knowledge::offer_seated(
        &dir,
        &dir,
        &dir.join(".nirvana/runs/r1"),
        "scholar",
    );
    assert!(offer.section.is_empty(), "{}", offer.section);
    assert_eq!(check_note(&dir, "scholar"), "");
    // A broken one is shown broken; the built-in does not take over.
    std::fs::write(path(&dir, "scholar"), "---\ncolour: red\n---\n").unwrap();
    let found = listed(&dir);
    assert_eq!(found.len(), 1);
    assert!(!found[0].builtin && found[0].parsed.is_err());
    let why = load(&dir, "scholar").unwrap_err();
    assert!(why.starts_with("profile scholar is refused"), "{why}");
    // So is a folder of the name with no profile.md.
    std::fs::remove_file(path(&dir, "scholar")).unwrap();
    std::fs::create_dir_all(folder_of(&dir, "scholar")).unwrap();
    let found = listed(&dir);
    assert_eq!(found.len(), 1);
    assert!(!found[0].builtin && found[0].parsed.is_err());
    assert!(load(&dir, "scholar").is_err());
    assert!(crate::nirvana_profile_detect::fitting(&dir, "research.web", &[]).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Item 6: `herdr profile new scholar --from scholar` writes the built-in byte for byte; the copy then wins.
#[test]
fn p47b_new_from_scholar_writes_the_copy_byte_equal_and_it_then_wins() {
    let (_home, project) = project("p47b-copy");
    let root = project.root.clone();
    let found = scholar();
    let made = new_profile_from(&root, "scholar", "scholar").expect("the copy is written");
    let dir = folder_of(&root, "scholar");
    assert_eq!(made, dir.join(PROFILE_FILE));
    assert_eq!(std::fs::read(&made).unwrap(), found.profile.as_bytes());
    for (slug, text) in found.cards {
        assert_eq!(
            std::fs::read(dir.join(KNOWLEDGE).join(format!("{slug}.md"))).unwrap(),
            text.as_bytes(),
            "{slug}"
        );
    }
    // The project's copy is the only scholar now, of the same sha, and its cards are the project's, adopted.
    let entries = listed(&root);
    let scholars: Vec<&Listed> = entries.iter().filter(|found| found.name == "scholar").collect();
    assert_eq!(scholars.len(), 1);
    assert!(!scholars[0].builtin && scholars[0].parsed.is_ok());
    let loaded = load(&root, "scholar").unwrap();
    assert!(!loaded.builtin);
    assert_eq!(loaded.sha, sha256_hex(found.profile.as_bytes()));
    let offer = crate::nirvana_knowledge::offer_uncounted(&root, &root, "scholar");
    assert_eq!(offer.slugs.len(), 9, "{}", offer.section);
    assert!(
        offer
            .section
            .contains("(.nirvana/profiles/scholar/knowledge/fetch-ladder.md)"),
        "{}",
        offer.section
    );
    let table = nirvana_seat::table_for(&root);
    let servers = crate::nirvana_gui_tools::project_servers(&root).unwrap_or_default();
    assert_eq!(
        check_one(
            &root,
            "scholar",
            &scholars[0].file,
            &scholars[0].parsed,
            &table,
            &servers
        ),
        Vec::<String>::new()
    );
    assert_eq!(check_note(&root, "scholar"), " (9 cards)");
    // Never over the copy; another name takes the same text.
    assert!(new_profile_from(&root, "scholar", "scholar")
        .unwrap_err()
        .contains("already there"));
    assert_eq!(std::fs::read(&made).unwrap(), found.profile.as_bytes());
    let other = new_profile_from(&root, "scholar-two", "scholar").unwrap();
    assert_eq!(std::fs::read(other).unwrap(), found.profile.as_bytes());
    nirvana_seat::set_test_world(None);
}

/// Item 7, claude: the body by the path a project profile's takes, the nine cards in the run, nothing in the project.
#[test]
fn p47b_a_claude_worker_seated_with_the_built_in_gets_its_body_and_cards_from_the_run() {
    let (_home, project) = project("p47b-claude");
    let found = scholar();
    let sha = sha256_hex(found.profile.as_bytes());
    std::fs::write(
        project.root.join("order.md"),
        "# Find the current surface API\n",
    )
    .unwrap();
    let before = project_files(&project.root);
    let live = nirvana_worker::take_live(&project, "r1", "s1")
        .unwrap()
        .unwrap();
    let mut req = request(&project, "s1", "sonnet");
    req.reason = Some("p47b: the claude seat's argv".into());
    let outcome = nirvana_worker::seat_order_with(&project, &req, Some("scholar")).unwrap();
    drop(live);
    assert_eq!(outcome["profile"]["name"], "scholar", "{outcome}");
    assert_eq!(outcome["profile"]["sha256"], sha.as_str(), "{outcome}");
    let seated = seated_of(&project, "s1");
    assert_eq!(
        (seated.cli.as_str(), seated.class.as_str()),
        ("claude", "research.web")
    );
    assert!(
        seated
            .reason
            .ends_with(&format!("; profile scholar sha256 {sha}")),
        "{}",
        seated.reason
    );
    assert_eq!(seated.profile, Some(Some("scholar".to_owned())));
    let run_dir = project.run_dir("r1");
    assert_eq!(
        std::fs::read(copy_path(&run_dir, "scholar", &sha)).unwrap(),
        found.profile.as_bytes()
    );
    // The nine cards, in the run beside the copy, byte for byte.
    let cards = builtin_cards_dir(&run_dir, "scholar", &sha);
    let mut held: Vec<String> = std::fs::read_dir(&cards)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    held.sort();
    let wanted: Vec<String> = found
        .cards
        .iter()
        .map(|(slug, _)| format!("{slug}.md"))
        .collect();
    assert_eq!(held, wanted);
    for (slug, text) in found.cards {
        assert_eq!(
            std::fs::read(cards.join(format!("{slug}.md"))).unwrap(),
            text.as_bytes()
        );
    }
    // The order offers each by a path that is on disk from the worker's folder.
    let brief = std::fs::read_to_string(&seated.order_file).unwrap();
    assert!(
        brief.starts_with(&format!("Profile: scholar\nProfile sha256: {sha}\n")),
        "{brief}"
    );
    assert!(
        brief.contains("\n## Knowledge cards of the profile scholar\n"),
        "{brief}"
    );
    for rel in card_paths(&sha) {
        assert!(brief.contains(&format!("({rel})")), "{rel} in {brief}");
        assert!(Path::new(&seated.cwd).join(&rel).is_file(), "{rel}");
    }
    // The claude argv and its system-prompt file: the body, by the path a project profile's takes.
    let argv = nirvana_worker::with_profile(vec!["claude".into()], &run_dir, &seated).unwrap();
    assert_eq!(
        argv,
        [
            "claude".to_owned(),
            "--append-system-prompt-file".to_owned(),
            body_path(&run_dir, "scholar", &sha).display().to_string()
        ]
    );
    assert_eq!(
        std::fs::read_to_string(&argv[2]).unwrap(),
        format!("{}\n", scholar_body())
    );
    // Nothing new in the project but herdr's run state, and no knowledge store of the built-in.
    assert_eq!(project_files(&project.root), before);
    assert!(!folder(&project.root).exists());
    assert!(!crate::nirvana_knowledge::writes_log(&project.root, "scholar").exists());
    nirvana_seat::set_test_world(None);
}

/// Item 7, agy: the body quoted at the top of the order copy, the cards by path, the tools folder as for a project
/// profile with no `skills` key, and nothing in the project.
#[test]
fn p47b_an_agy_worker_seated_with_the_built_in_gets_the_quoted_body_and_its_cards() {
    let (_home, project) = project("p47b-agy");
    let found = scholar();
    let sha = sha256_hex(found.profile.as_bytes());
    skill(&project.root, "alpha");
    std::fs::write(
        project.root.join("order.md"),
        "# Find the current surface API\n",
    )
    .unwrap();
    let before = project_files(&project.root);
    let profile = parse(&builtin_shown("scholar"), "scholar", found.profile).unwrap();
    let seat = rule_seat(&project, "research.web", &profile);
    assert_eq!(seat.cli, "agy", "{}", seat.seat);
    let live = nirvana_worker::take_live(&project, "r1", "a1")
        .unwrap()
        .unwrap();
    nirvana_worker::seat_order_with(
        &project,
        &request(&project, "a1", &seat.seat),
        Some("scholar"),
    )
    .unwrap();
    drop(live);
    let seated = seated_of(&project, "a1");
    let run_dir = project.run_dir("r1");
    let brief = std::fs::read_to_string(&seated.order_file).unwrap();
    assert!(
        brief.starts_with(&agy_section("scholar", &profile.body)),
        "{brief}"
    );
    assert!(
        brief.contains(&format!(
            "(end of profile scholar)\n\nProfile: scholar\nProfile sha256: {sha}\n"
        )),
        "{brief}"
    );
    for rel in card_paths(&sha) {
        assert!(brief.contains(&format!("({rel})")), "{rel} in {brief}");
        assert!(Path::new(&seated.cwd).join(&rel).is_file(), "{rel}");
    }
    // agy takes no system prompt: its argv is unchanged, and the order copy is already right.
    let argv = nirvana_worker::with_profile(vec!["agy".into()], &run_dir, &seated).unwrap();
    assert_eq!(argv, ["agy"]);
    assert_eq!(std::fs::read_to_string(&seated.order_file).unwrap(), brief);
    // The tools folder: every project skill, as for a project profile with no `skills` key; nothing of the built-in's.
    let order = crate::nirvana_agy_tools::Order {
        root: &project.root,
        cwd: Path::new(&seated.cwd),
        run_dir: &run_dir,
        order: "a1",
        order_file: &seated.order_file,
        deny: nirvana_worker::agy_denials(&run_dir, &seated).unwrap(),
    };
    let print = format!("--print={}", nirvana_worker::first_line(&seated.order_file));
    let argv =
        crate::nirvana_agy_tools::with_tools(vec!["agy".into(), print], &order, &mut Vec::new())
            .unwrap();
    let tools = crate::nirvana_agy_tools::tools_dir(&run_dir, "a1");
    assert!(
        argv.contains(&crate::nirvana_agy_tools::plain_path(&tools)),
        "{argv:?}"
    );
    assert_eq!(project_files(&tools), [".agents/skills/alpha/SKILL.md"]);
    // Nothing new in the project but herdr's run state.
    assert_eq!(project_files(&project.root), before);
    assert!(!folder(&project.root).exists());
    nirvana_seat::set_test_world(None);
}

/// Item 8: the built-in's cards are offered with no adoption; no card op, adoption, teacher or `kb` touches them.
#[test]
fn p47b_the_built_ins_cards_are_offered_unadopted_and_no_card_op_rewrites_them() {
    let (_home, project) = project("p47b-cards");
    let root = project.root.clone();
    let copy = copy_command("scholar");
    let offer = crate::nirvana_knowledge::offer_seated(&root, &root, &project.run_dir("r1"), "scholar");
    let offered: Vec<&str> = offer
        .section
        .lines()
        .filter(|line| line.starts_with("- "))
        .collect();
    assert_eq!(offered.len(), 9, "{}", offer.section);
    assert!(offer.slugs.is_empty(), "nothing is counted in a knowledge store");
    // A lesson review's or a teacher's card ops, new and patch, are refused with the copy step.
    let quote = "this quote is long enough to be the evidence of a card op";
    let stream = format!("the worker wrote: {quote}");
    let ladder = scholar()
        .cards
        .iter()
        .find(|(slug, _)| *slug == "fetch-ladder")
        .unwrap()
        .1;
    let report = serde_json::json!({"card_ops": [
        {"op": "new", "name": "Read X posts in the browser", "when": "An X post", "verdict": "keep",
         "source": "a test", "body": "Use the browser.", "evidence": quote},
        {"op": "patch", "slug": "fetch-ladder", "base_sha": crate::nirvana_lessons::sha256(ladder),
         "name": "Fetch ladder", "when": "Always", "verdict": "keep", "source": "a test", "body": "Changed.",
         "evidence": quote}
    ]});
    let done = crate::nirvana_knowledge::review_ops(
        &root,
        &report,
        Some("scholar"),
        &stream,
        &[],
        "o1",
        "r1",
    );
    assert_eq!(done.len(), 2, "{done:?}");
    for (kept, line) in &done {
        assert!(
            !kept && line.contains("built into herdr") && line.contains(&copy),
            "{line}"
        );
    }
    let ops = crate::nirvana_knowledge::parse_card_ops(&report);
    let Some(Ok(crate::nirvana_knowledge::CardOp::New(draft))) = ops.first() else {
        panic!("a new op: {ops:?}");
    };
    let why = crate::nirvana_knowledge::apply_new(&root, "scholar", draft, "o1", "r1", 1).unwrap_err();
    assert!(why.contains(&copy), "{why}");
    // Adoption, teaching and `herdr kb` say the same.
    assert!(crate::nirvana_knowledge::adopt(&root, "scholar")
        .unwrap_err()
        .contains(&copy));
    let teach = crate::nirvana_knowledge::Teach {
        profile: "scholar",
        from: "https://example.com/a",
        note: None,
        asked: "the owner".into(),
    };
    let (code, why) = crate::nirvana_knowledge::teach(&project, "r1", &teach).unwrap_err();
    assert_eq!(code, 2);
    assert!(why.contains(&copy), "{why}");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(
        crate::nirvana_knowledge::kb_run(
            &root,
            &["list".into(), "--profile".into(), "scholar".into()],
            &mut out,
            &mut err
        ),
        1
    );
    assert!(String::from_utf8_lossy(&err).contains(&copy));
    // Nothing went into the project's knowledge store.
    assert!(!folder(&root).exists());
    assert!(!crate::nirvana_knowledge::writes_log(&root, "scholar").exists());
    nirvana_seat::set_test_world(None);
}

/// Item 11: an order seated under an older text keeps its copy and sha; nothing compares it with today's.
#[test]
fn p47b_an_order_seated_under_an_older_text_keeps_its_copy() {
    let (_home, project) = project("p47b-older");
    let found = scholar();
    std::fs::write(project.root.join("order.md"), "# Research it\n").unwrap();
    let live = nirvana_worker::take_live(&project, "r1", "s1")
        .unwrap()
        .unwrap();
    let mut req = request(&project, "s1", "sonnet");
    req.reason = Some("p47b: an upgrade".into());
    nirvana_worker::seat_order_with(&project, &req, Some("scholar")).unwrap();
    drop(live);
    let seated = seated_of(&project, "s1");
    let run_dir = project.run_dir("r1");
    // An older herdr seated another order of this run under another text: its copy and its sha stay its own.
    let older = found.profile.replace(
        "You research one question",
        "You researched one question, as an older herdr said",
    );
    assert_ne!(older, found.profile);
    let older_sha = sha256_hex(older.as_bytes());
    std::fs::write(copy_path(&run_dir, "scholar", &older_sha), &older).unwrap();
    let mut child = seated.clone();
    let file = project.root.join("older.md");
    std::fs::write(
        &file,
        format!(
            "{}\n# Research it\n",
            order_lines(&Ref {
                name: "scholar".into(),
                sha: older_sha.clone()
            })
        ),
    )
    .unwrap();
    child.order_file = file.display().to_string();
    child.reason = format!("r; profile scholar sha256 {older_sha}");
    assert_eq!(from_reason(&child.reason).as_deref(), Some("scholar"));
    let argv = nirvana_worker::with_profile(vec!["claude".into()], &run_dir, &child).unwrap();
    assert_eq!(
        argv[2],
        body_path(&run_dir, "scholar", &older_sha)
            .display()
            .to_string()
    );
    assert!(std::fs::read_to_string(&argv[2])
        .unwrap()
        .starts_with("You researched one question, as an older herdr said"));
    // Today's order beside it keeps today's.
    let argv = nirvana_worker::with_profile(vec!["claude".into()], &run_dir, &seated).unwrap();
    assert!(std::fs::read_to_string(&argv[2])
        .unwrap()
        .starts_with("You research one question"));
    nirvana_seat::set_test_world(None);
}

/// A teach order, a lesson review and a `review.*` or `acceptance.*` order take no profile text and no server they did
/// not take before: none is fitted, a review's argv takes nothing, and the built-in holds no server or need.
#[test]
fn p47b_teach_and_review_orders_take_no_text_or_server_of_the_built_in() {
    let (_home, project) = project("p47b-reviews");
    let root = project.root.clone();
    assert_eq!(
        nirvana_worker::fitting_profiles(&root, None, "research.web", &[]),
        ["scholar"]
    );
    for class in [
        crate::nirvana_knowledge::TEACH_CLASS,
        crate::nirvana_review::CLASS,
        "review.adversary",
        "acceptance.author",
    ] {
        assert!(
            nirvana_worker::fitting_profiles(&root, None, class, &[]).is_empty(),
            "{class}"
        );
        assert!(takes_no_profile_servers(class), "{class}");
    }
    std::fs::write(root.join("order.md"), "# Research it\n").unwrap();
    let live = nirvana_worker::take_live(&project, "r1", "s1")
        .unwrap()
        .unwrap();
    let mut req = request(&project, "s1", "sonnet");
    req.reason = Some("p47b: a review of it".into());
    nirvana_worker::seat_order_with(&project, &req, Some("scholar")).unwrap();
    drop(live);
    let mut review = seated_of(&project, "s1");
    review.class = crate::nirvana_review::CLASS.into();
    assert_eq!(
        nirvana_worker::with_profile(vec!["claude".into()], &project.run_dir("r1"), &review)
            .unwrap(),
        ["claude"]
    );
    let loaded = load(&root, "scholar").unwrap();
    assert!(
        loaded.servers.is_none()
            && loaded.profile.servers.is_empty()
            && loaded.profile.needs.is_empty()
            && loaded.profile.disallowed_tools.is_empty()
    );
    nirvana_seat::set_test_world(None);
}
