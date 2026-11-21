//! Plan 47 P47c: `herdr scholar`. No test starts a model turn: the route turn is a stand-in, every seat is held live
//! so `seat_order_why` starts no worker, and the deliveries are journal lines a test writes.
use super::*;
use crate::nirvana_run::{Journal, Seated};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "p47c-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn git(root: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A git project under a test home with herdr's default seats, over a stand-in world where every CLI is signed in.
fn git_project(name: &str) -> (crate::nirvana::TestHomeGuard, Project) {
    let home = crate::nirvana::with_test_home(name);
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    let root = home.path.join("project");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["-c", "init.defaultBranch=main", "init", "-q"]);
    git(&root, &["config", "user.name", "Scholar Test"]);
    git(&root, &["config", "user.email", "scholar@test"]);
    git(&root, &["config", "core.autocrlf", "false"]);
    std::fs::write(root.join("README.md"), "a project\n").unwrap();
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-q", "-m", "base"]);
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    (home, Project::at(root))
}

fn seated(id: &str, class: &str, why: Option<&str>, parent: Option<&str>) -> Seated {
    Seated {
        order: id.into(),
        seat: "hydrogen".into(),
        cli: "agy".into(),
        model: "gemini-4-hydrogen-low".into(),
        effort: Effort::Low,
        pool: "gemini".into(),
        class: class.into(),
        chain: Some("research".into()),
        size: SizeName::Medium,
        mode: Mode::Balanced,
        needs: Vec::new(),
        not_family: Vec::new(),
        reason: "research.web → hydrogen low".into(),
        override_reason: None,
        order_file: "o.md".into(),
        cwd: "C:/p".into(),
        worktree: None,
        branch: None,
        parent: parent.map(str::to_owned),
        why: why.map(str::to_owned),
        rule_pick: None,
        bar_dropped: None,
        step_down: None,
        use_basis: None,
        expected: BTreeMap::new(),
        jev_pick: None,
        send_ok: None,
        profile: None,
        profile_fit: None,
    }
}

/// A run journal built with `Journal::append`, as the gate's tests build theirs.
struct J {
    journal: Journal,
    at: u64,
}

impl J {
    fn new(name: &str) -> Self {
        let journal = Journal::new(temp(name).join("journal.jsonl"));
        journal
            .append(
                1000,
                Kind::Opened {
                    project: "C:/p".into(),
                    orchestrator_pane: Some("wT:p1".into()),
                    auto: true,
                    pools: Vec::new(),
                    pools_basis: None,
                },
            )
            .unwrap();
        Self { journal, at: 1000 }
    }

    fn add(&mut self, kind: Kind) {
        self.at += 1;
        self.journal.append(self.at, kind).unwrap();
    }

    fn seat(&mut self, id: &str, class: &str) {
        self.add(Kind::Seated(Box::new(seated(
            id,
            class,
            Some(STAGE_WHY),
            None,
        ))));
    }

    /// The gate's send-back: the parent's verdict and the child, in one step.
    fn send_back(&mut self, parent: &str, child: &str) {
        self.gate(parent, "sent_back", "sources: a cited URL was never opened");
        let class = self.state().order(parent).unwrap().seated.class.clone();
        self.add(Kind::Seated(Box::new(seated(
            child,
            &class,
            Some("send_back"),
            Some(parent),
        ))));
    }

    fn deliver(&mut self, id: &str, summary: &str) {
        self.add(Kind::Delivered {
            order: id.into(),
            report: serde_json::json!({
                "status": "done",
                "summary": summary,
                "gotchas": [format!("{id}: the counter-argument")],
                "sources": [{"url": format!("https://example.org/{id}"), "used_for": "the claim"}],
                "files": [format!("{id}/findings.md")],
            }),
        });
    }

    fn gate(&mut self, id: &str, verdict: &str, reason: &str) {
        self.add(Kind::Gated {
            order: id.into(),
            verdict: verdict.into(),
            reason: Some(reason.into()),
            axes: Vec::new(),
        });
    }

    fn pass(&mut self, id: &str) {
        self.deliver(id, &format!("{id} answered"));
        self.gate(id, "pass", "every axis passed");
    }

    fn died(&mut self, id: &str) {
        self.add(Kind::Interrupted {
            order: id.into(),
            why: format!("{}: pid 4242 is gone", nirvana_run::WORKER_DIED),
        });
    }

    fn state(&self) -> RunState {
        nirvana_run::replay(&self.journal.read().0)
    }
}

fn chain(n: u32, style: &str) -> Chain {
    Chain {
        n,
        id: research_id("q", n),
        style: Some(style.into()),
        question: Some(format!("sub-question {n}")),
        why: Some("a signal".into()),
        reseat: None,
        seat: None,
        end: None,
    }
}

fn state(stage: Stage, styles: &[&str]) -> State {
    State {
        version: STATE_VERSION,
        run: "scholar-20261120-q".into(),
        question: "The owner's whole question?".into(),
        slug: "q".into(),
        date: "2026-11-20".into(),
        report_dir: "docs/research/2026-11-20-q".into(),
        mode: Mode::Balanced,
        pane: Some("wT:p1".into()),
        stage,
        started_at: 1000,
        stage_at: 1000,
        route: Some(RouteRecord {
            styles: styles.iter().map(|style| (*style).to_owned()).collect(),
            tries: 1,
            ..RouteRecord::default()
        }),
        orders: styles
            .iter()
            .enumerate()
            .map(|(at, style)| chain(at as u32 + 1, style))
            .collect(),
        writer: None,
        end: None,
        end_why: None,
        ended_at: None,
        check: Vec::new(),
        notes: Vec::new(),
        told: false,
        logged: false,
        end_tries: 0,
    }
}

fn passed(leaf: &str) -> Option<ChainEnd> {
    Some(ChainEnd {
        passed: true,
        leaf: leaf.into(),
        why: None,
        at: 1,
    })
}

/// Records a step's chain ends into the state, as `apply` does, and returns the next step.
fn settle(state: &mut State, run: &RunState, now: u64) -> Step {
    loop {
        match next(state, run, now, &Caps::default(), false) {
            Step::Ends(found) => {
                for (at, end) in found {
                    match at {
                        Some(at) => state.orders[at].end = Some(end),
                        None => state.writer.as_mut().unwrap().end = Some(end),
                    }
                }
            }
            other => return other,
        }
    }
}

const NOW: u64 = 1500;

fn ended_kind() -> Kind {
    Kind::Ended {
        order: None,
        outcome: "ended".into(),
        why: None,
        exit: None,
        pools: Vec::new(),
        pools_basis: None,
    }
}

// ---- the menu and the shipped texts ----

#[test]
fn p47c_every_style_has_a_card_in_the_built_in() {
    let found = crate::nirvana_profiles::builtin(PROFILE).expect("the built-in scholar");
    let slugs: Vec<&str> = found.cards.iter().map(|(slug, _)| *slug).collect();
    for style in STYLES {
        assert!(slugs.contains(&style), "no card {style} in {slugs:?}");
    }
    // The order text names the ladder too.
    assert!(slugs.contains(&"fetch-ladder"), "{slugs:?}");
    assert!(STYLES.contains(&FALLBACK_STYLE));
}

/// The six worked examples of the route text, each a question then its answer on one numbered line.
fn examples() -> Vec<(String, Value)> {
    ROUTE_TEXT
        .lines()
        .filter(|line| {
            line.chars().next().is_some_and(|c| c.is_ascii_digit())
                && line.contains("\" {\"orders\"")
        })
        .map(|line| {
            let at = line.find("\" {").expect("an answer") + 1;
            (
                line[..at].to_owned(),
                serde_json::from_str(&line[at + 1..]).unwrap_or_else(|err| panic!("{line}: {err}")),
            )
        })
        .collect()
}

#[test]
fn p47c_the_six_route_examples_parse_and_validate() {
    let found = examples();
    assert_eq!(found.len(), 6, "{found:?}");
    let counts: Vec<usize> = found
        .iter()
        .map(|(question, answer)| {
            validate(answer)
                .unwrap_or_else(|why| panic!("{question}: {why}"))
                .orders
                .len()
        })
        .collect();
    assert_eq!(counts, [1, 2, 1, 1, 1, 4]);
    // The card header is gone, and the text says the model reads nothing and gets the reports in its prompt.
    assert!(!ROUTE_TEXT.starts_with("---"), "{ROUTE_TEXT}");
    assert!(!WRITE_TEXT.starts_with("---"), "{WRITE_TEXT}");
    assert!(
        ROUTE_TEXT.contains("you have no tools: you open nothing"),
        "{ROUTE_TEXT}"
    );
    assert!(
        ROUTE_TEXT.contains("10. If a report listed after the question already answers it"),
        "{ROUTE_TEXT}"
    );
    assert!(!ROUTE_TEXT.contains("You may list the project's docs/research/ folder"));
    // The writer's summary opens with the page's path (the wake cuts a summary at 90 characters).
    assert!(
        WRITE_TEXT.contains("The summary starts with the page's path relative to the project root"),
        "{WRITE_TEXT}"
    );
}

/// The orchestrator's role text and its seat skill name the verb, the resume, and the single lookup's profile.
#[test]
fn p47c_the_role_and_the_seat_skill_send_research_to_herdr_scholar() {
    const ROLE: &str = include_str!("../../nirvana/claude/ROLE.md");
    const SKILL: &str = include_str!("../../nirvana/claude/skills/seat/SKILL.md");
    for found in [
        "goes to `herdr scholar \"<question>\"`",
        "then end your turn",
        "`herdr scholar resume <run>`",
        "seated with\n  `--profile scholar`",
        "publish the page only when this session\n  can and the owner wants it",
    ] {
        assert!(ROLE.contains(found), "{found:?} not in ROLE.md");
    }
    // The agy plugin's build fails above 24 KB of role text (`nirvana/agy_plugin.py` BODY_CAP).
    assert!(ROLE.len() < 24 * 1024, "{}", ROLE.len());
    for found in [
        "### A research question: `herdr scholar`",
        "its summary starts with the page's path",
        "`herdr scholar tag <run> hit|partial|miss",
    ] {
        assert!(SKILL.contains(found), "{found:?} not in the seat skill");
    }
}

// ---- route ----

fn answer(styles: &[&str]) -> Value {
    serde_json::json!({
        "orders": styles.iter().map(|style| serde_json::json!({
            "style": style, "question": format!("{style} sub-question"), "why": "a signal"
        })).collect::<Vec<_>>(),
        "note": "",
    })
}

#[test]
fn p47c_one_two_and_five_orders_validate_and_seven_are_cut_to_five() {
    for styles in [&STYLES[..1], &STYLES[..2], &STYLES[..5]] {
        let got = validate(&answer(styles)).unwrap();
        assert_eq!(
            got.orders
                .iter()
                .map(|order| order.style.as_str())
                .collect::<Vec<_>>(),
            styles
        );
        assert_eq!(got.note, "");
        // Each order is seated, one per step, back to back.
        let mut fresh = state(Stage::Research, styles);
        fresh.stage_at = NOW;
        assert_eq!(
            next(&fresh, &RunState::default(), NOW, &Caps::default(), false),
            Step::Seat(0)
        );
    }
    let got = validate(&answer(&STYLES[..7])).unwrap();
    assert_eq!(got.orders.len(), MAX_ORDERS);
    assert_eq!(
        got.note,
        format!(
            "2 order(s) past five were dropped: {}, {}",
            STYLES[5], STYLES[6]
        )
    );
}

#[test]
fn p47c_an_unknown_style_a_repeated_style_and_an_empty_question_are_refused() {
    let unknown = answer(&["benchmark", "vibes"]);
    assert_eq!(
        validate(&unknown).unwrap_err(),
        "order 2 names the unknown style \"vibes\""
    );
    let twice = answer(&["trajectory", "trajectory"]);
    assert_eq!(
        validate(&twice).unwrap_err(),
        "the style trajectory is used twice"
    );
    let mut empty = answer(&["benchmark"]);
    empty["orders"][0]["question"] = "  ".into();
    assert_eq!(validate(&empty).unwrap_err(), "order 1 has no question");
    assert!(
        validate(&serde_json::json!({"orders": []})).is_err(),
        "no note"
    );
    // The schema the turn is held to names the eight and nothing else.
    let schema: Value = serde_json::from_str(&route_schema()).unwrap();
    assert_eq!(
        schema["properties"]["orders"]["items"]["properties"]["style"]["enum"],
        serde_json::json!(STYLES)
    );
}

fn result_line(structured: Value) -> String {
    serde_json::json!([{
        "type": "result", "subtype": "success", "is_error": false, "result": "",
        "structured_output": structured,
        "usage": {"input_tokens": 900, "output_tokens": 120},
    }])
    .to_string()
}

fn exited(stdout: String) -> Result<End, String> {
    Ok(End::Exited {
        stdout,
        stderr: String::new(),
        code: Some(0),
    })
}

fn world_with<'a>(
    route: &'a dyn Fn(&[String], &Path) -> Result<End, String>,
    sleep: &'a dyn Fn(u64),
    end_run: &'a dyn Fn(&Project, &str) -> Result<(), String>,
    wake: &'a dyn Fn(&Project, &str),
    now: &'a dyn Fn() -> u64,
) -> World<'a> {
    World {
        now,
        sleep,
        route,
        seat: &seat_real,
        stop: &|_, _, order| format!("{order}: stopped"),
        wake,
        end_run,
        wakeable: &|_| true,
    }
}

#[test]
fn p47c_a_route_that_fails_twice_runs_one_fallback_order_with_the_owners_words() {
    let (_home, project) = git_project("p47c-fallback");
    let started = start(
        &project,
        "Is wgpu still on 0.30?",
        None,
        &|_| None,
        false,
        1_763_600_000,
    )
    .unwrap();
    let calls = Cell::new(0);
    let failing = |argv: &[String], _cwd: &Path| {
        calls.set(calls.get() + 1);
        // No tool at all, and the schema.
        let tools = argv
            .iter()
            .position(|arg| arg == "--tools")
            .expect("--tools");
        assert_eq!(argv[tools + 1], "");
        assert!(argv.iter().any(|arg| arg == "--json-schema"));
        if calls.get() == 1 {
            Err("claude did not start".to_owned())
        } else {
            exited(result_line(answer(&["vibes"])))
        }
    };
    let now = || 1_763_600_000;
    let world = world_with(&failing, &|_| {}, &|_, _| Ok(()), &|_, _| {}, &now);
    let (got, record) = route(&project, &started.state, &world);
    assert_eq!(calls.get(), ROUTE_TRIES as i32);
    assert_eq!(got.orders.len(), 1);
    assert_eq!(got.orders[0].style, FALLBACK_STYLE);
    assert_eq!(got.orders[0].question, "Is wgpu still on 0.30?");
    assert!(record.fell_back);
    let why = record.why.clone().unwrap();
    assert!(
        why.contains("claude did not start") && why.contains("unknown style"),
        "{why}"
    );
    // The Method section is told.
    let mut fell = started.state.clone();
    fell.route = Some(record);
    let notes = method_notes(&fell).join("\n");
    assert!(notes.contains("the routing failed"), "{notes}");
    nirvana_seat::set_test_world(None);
}

#[test]
fn p47c_the_route_prompt_lists_the_projects_reports_and_the_answer_is_read() {
    let root = temp("titles");
    for (folder, title) in [
        ("2026-11-01-a", "# Wgpu surfaces"),
        ("2026-11-02-b", "Egui or Bevy"),
    ] {
        std::fs::create_dir_all(root.join(REPORTS).join(folder)).unwrap();
        std::fs::write(
            root.join(REPORTS).join(folder).join("report.md"),
            format!("{title}\n\nbody\n"),
        )
        .unwrap();
    }
    let reports = existing_reports(&root);
    assert_eq!(
        reports,
        [
            (
                "docs/research/2026-11-02-b/report.md".to_owned(),
                "Egui or Bevy".to_owned()
            ),
            (
                "docs/research/2026-11-01-a/report.md".to_owned(),
                "Wgpu surfaces".to_owned()
            ),
        ]
    );
    let prompt = route_prompt("Should I?", &reports);
    assert!(prompt.starts_with(ROUTE_TEXT.trim_end()), "{prompt}");
    assert!(
        prompt.contains("<question>\nShould I?\n</question>"),
        "{prompt}"
    );
    assert!(
        prompt.contains("- docs/research/2026-11-01-a/report.md: Wgpu surfaces"),
        "{prompt}"
    );
    for n in 0..25 {
        let folder = root.join(REPORTS).join(format!("x{n:02}"));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("report.md"), "t\n").unwrap();
    }
    assert_eq!(existing_reports(&root).len(), TITLES_MAX);
    // structured_output first; a result text in a fence; an error.
    let (value, tokens) = answer_value(&result_line(answer(&["benchmark"]))).unwrap();
    assert_eq!(value["orders"][0]["style"], "benchmark");
    assert_eq!(tokens.output, 120);
    let fenced = serde_json::json!([{"type":"result","subtype":"success","is_error":false,
        "result":"```json\n{\"orders\":[],\"note\":\"hi\"}\n```"}])
    .to_string();
    assert_eq!(answer_value(&fenced).unwrap().0["note"], "hi");
    let failed = serde_json::json!([{"type":"result","subtype":"error_max_turns","is_error":true}])
        .to_string();
    assert!(answer_value(&failed).is_err());
}

// ---- the planner over journals ----

#[test]
fn p47c_all_pass_seats_the_writer() {
    let mut j = J::new("all-pass");
    let mut st = state(Stage::Research, &["benchmark", "trajectory"]);
    j.seat("q-r1", RESEARCH_CLASS);
    j.seat("q-r2", RESEARCH_CLASS);
    assert_eq!(
        next(&st, &j.state(), NOW, &Caps::default(), false),
        Step::Wait
    );
    j.pass("q-r1");
    j.pass("q-r2");
    assert_eq!(settle(&mut st, &j.state(), NOW), Step::SeatWriter);
    assert!(st
        .orders
        .iter()
        .all(|chain| chain.end.as_ref().unwrap().passed));
}

#[test]
fn p47c_one_passes_and_one_asks_the_writer_gets_the_one_and_hears_of_the_other() {
    let mut j = J::new("one-asks");
    let mut st = state(Stage::Research, &["benchmark", "trajectory"]);
    j.seat("q-r1", RESEARCH_CLASS);
    j.seat("q-r2", RESEARCH_CLASS);
    j.pass("q-r1");
    j.deliver("q-r2", "needs the owner's login");
    j.add(Kind::Asked {
        order: Some("q-r2".into()),
        question: "q-r2 is blocked: a login wall".into(),
    });
    j.gate("q-r2", "asked", "the worker reported blocked");
    let run = j.state();
    assert_eq!(settle(&mut st, &run, NOW), Step::SeatWriter);
    let r2 = st.orders[1].end.clone().unwrap();
    assert!(!r2.passed);
    assert_eq!(
        r2.why.as_deref(),
        Some("gated asked: the worker reported blocked")
    );
    let text = write_text(&st, &run);
    assert!(
        text.contains(&format!("Findings file: `{}`", findings_rel(&st.run, 1))),
        "{text}"
    );
    assert!(!text.contains(&findings_rel(&st.run, 2)), "{text}");
    assert!(
        text.contains(
            "- r2 · trajectory: sub-question 2 (why: gated asked: the worker reported blocked)"
        ),
        "{text}"
    );
    assert!(text.contains("> q-r1 answered"), "{text}");
    assert!(
        text.contains("- https://example.org/q-r1 · the claim"),
        "{text}"
    );
    assert!(text.contains("- q-r1: the counter-argument"), "{text}");
}

#[test]
fn p47c_a_chain_sent_back_twice_then_passing_waits_on_its_leaf() {
    let mut j = J::new("send-backs");
    let mut st = state(Stage::Research, &["benchmark"]);
    j.seat("q-r1", RESEARCH_CLASS);
    j.deliver("q-r1", "first try");
    j.send_back("q-r1", "q-r1-s1");
    assert_eq!(settle(&mut st, &j.state(), NOW), Step::Wait);
    j.deliver("q-r1-s1", "second try");
    j.send_back("q-r1-s1", "q-r1-s2");
    assert_eq!(settle(&mut st, &j.state(), NOW), Step::Wait);
    assert!(st.orders[0].end.is_none());
    j.pass("q-r1-s2");
    assert_eq!(settle(&mut st, &j.state(), NOW), Step::SeatWriter);
    assert_eq!(st.orders[0].end.as_ref().unwrap().leaf, "q-r1-s2");
    // The writer reads the leaf's report, not the root's.
    let text = write_text(&st, &j.state());
    assert!(text.contains("> q-r1-s2 answered"), "{text}");
    // A sent-back verdict whose child the journal does not show yet: the chain still runs.
    let mut lone = J::new("send-back-alone");
    let mut st = state(Stage::Research, &["benchmark"]);
    lone.seat("q-r1", RESEARCH_CLASS);
    lone.deliver("q-r1", "first try");
    lone.gate("q-r1", "sent_back", "sources: a cited URL was never opened");
    assert_eq!(settle(&mut st, &lone.state(), NOW), Step::Wait);
}

#[test]
fn p47c_no_researcher_passing_ends_no_delivery_and_seats_no_writer() {
    let mut j = J::new("none-pass");
    let mut st = state(Stage::Research, &["benchmark", "trajectory"]);
    j.seat("q-r1", RESEARCH_CLASS);
    j.seat("q-r2", RESEARCH_CLASS);
    j.deliver("q-r1", "x");
    j.gate("q-r1", "asked", "3 send-backs spent");
    j.add(Kind::Ended {
        order: Some("q-r2".into()),
        outcome: "failed".into(),
        why: Some("no Work Report".into()),
        exit: Some(1),
        pools: Vec::new(),
        pools_basis: None,
    });
    match settle(&mut st, &j.state(), NOW) {
        Step::Finish(end, why) => {
            assert_eq!(end, ends::NO_DELIVERY);
            assert!(why.contains("r1 gated asked: 3 send-backs spent"), "{why}");
            assert!(why.contains("r2 failed (failed): no Work Report"), "{why}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn p47c_an_interrupted_chain_is_reseated_once_and_twice_is_not_delivered() {
    let mut j = J::new("interrupted");
    let mut st = state(Stage::Research, &["benchmark"]);
    j.seat("q-r1", RESEARCH_CLASS);
    j.died("q-r1");
    assert_eq!(
        settle(&mut st, &j.state(), NOW),
        Step::Reseat(0, "q-r1-r1".into())
    );
    // The driver writes the reseat's id before it seats; until the journal shows it, the step is to seat it.
    st.orders[0].reseat = Some("q-r1-r1".into());
    assert_eq!(
        settle(&mut st, &j.state(), NOW),
        Step::Reseat(0, "q-r1-r1".into())
    );
    j.seat("q-r1-r1", RESEARCH_CLASS);
    assert_eq!(settle(&mut st, &j.state(), NOW), Step::Wait);
    j.died("q-r1-r1");
    match settle(&mut st, &j.state(), NOW) {
        Step::Finish(end, _) => assert_eq!(end, ends::NO_DELIVERY),
        other => panic!("{other:?}"),
    }
    let end = st.orders[0].end.clone().unwrap();
    assert_eq!(end.leaf, "q-r1-r1");
    assert!(
        end.why.unwrap().starts_with("interrupted twice: "),
        "{:?}",
        st.orders[0]
    );
    // A reseat that passes delivers the chain.
    let mut j = J::new("reseat-passes");
    let mut st = state(Stage::Research, &["benchmark"]);
    j.seat("q-r1", RESEARCH_CLASS);
    j.died("q-r1");
    st.orders[0].reseat = Some("q-r1-r1".into());
    j.seat("q-r1-r1", RESEARCH_CLASS);
    j.pass("q-r1-r1");
    assert_eq!(settle(&mut st, &j.state(), NOW), Step::SeatWriter);
}

#[test]
fn p47c_the_cap_stops_what_still_runs_and_counts_it_not_delivered() {
    let mut j = J::new("cap");
    let mut st = state(Stage::Research, &["benchmark", "trajectory"]);
    j.seat("q-r1", RESEARCH_CLASS);
    j.seat("q-r2", RESEARCH_CLASS);
    j.pass("q-r1");
    let caps = Caps::default();
    let run = j.state();
    let just_under = st.stage_at + caps.research_s - 1;
    assert_eq!(settle(&mut st, &run, just_under), Step::Wait);
    let at_cap = st.stage_at + caps.research_s;
    assert_eq!(
        next(&st, &run, at_cap, &caps, false),
        Step::Stop(
            vec![(Some(1), "q-r2".to_owned())],
            "over the time cap".to_owned()
        )
    );
    assert_eq!(RESEARCH_CAP_S, 90 * 60);
    assert_eq!(WRITE_CAP_S, 45 * 60);
    // The writer's cap runs from the write stage's start.
    let mut w = J::new("write-cap");
    let mut ws = state(Stage::Write, &["benchmark"]);
    ws.orders[0].end = passed("q-r1");
    ws.writer = Some(writer_chain(&ws));
    w.seat("q-write", WRITE_CLASS);
    assert_eq!(
        next(&ws, &w.state(), ws.stage_at + WRITE_CAP_S, &caps, false),
        Step::Stop(
            vec![(None, "q-write".to_owned())],
            "over the time cap".to_owned()
        )
    );
    // The variables override the caps.
    std::env::set_var("P47C_TEST_ONLY_CAP_S", "7");
    assert_eq!(env_secs("P47C_TEST_ONLY_CAP_S", 99), 7);
    assert_eq!(env_secs("P47C_TEST_ONLY_CAP_UNSET", 99), 99);
}

#[test]
fn p47c_the_writer_passing_is_checked_and_failing_ends_no_report() {
    let mut j = J::new("writer");
    let mut st = state(Stage::Write, &["benchmark"]);
    st.orders[0].end = passed("q-r1");
    assert_eq!(
        next(&st, &j.state(), NOW, &Caps::default(), false),
        Step::SeatWriter
    );
    st.writer = Some(writer_chain(&st));
    j.seat("q-write", WRITE_CLASS);
    assert_eq!(settle(&mut st, &j.state(), NOW), Step::Wait);
    j.pass("q-write");
    assert_eq!(settle(&mut st, &j.state(), NOW), Step::Check);
    let mut j = J::new("writer-fails");
    let mut st = state(Stage::Write, &["benchmark"]);
    st.orders[0].end = passed("q-r1");
    st.writer = Some(writer_chain(&st));
    j.seat("q-write", WRITE_CLASS);
    j.deliver("q-write", "x");
    j.gate("q-write", "asked", "owned: a tracked file outside owns");
    match settle(&mut st, &j.state(), NOW) {
        Step::Finish(end, why) => {
            assert_eq!(end, ends::NO_REPORT);
            assert!(why.contains("owned: a tracked file outside owns"), "{why}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn p47c_a_stop_in_each_stage_stops_what_runs_then_ends_stopped() {
    let j = J::new("stop-route");
    let st = state(Stage::Route, &[]);
    match next(&st, &j.state(), NOW, &Caps::default(), true) {
        Step::Finish(end, _) => assert_eq!(end, ends::STOPPED),
        other => panic!("{other:?}"),
    }
    let mut j = J::new("stop-research");
    let mut st = state(Stage::Research, &["benchmark", "trajectory"]);
    j.seat("q-r1", RESEARCH_CLASS);
    j.seat("q-r2", RESEARCH_CLASS);
    j.pass("q-r2");
    st.orders[1].end = passed("q-r2");
    assert_eq!(
        next(&st, &j.state(), NOW, &Caps::default(), true),
        Step::Stop(
            vec![(Some(0), "q-r1".to_owned())],
            "stopped by `herdr scholar stop`".to_owned()
        )
    );
    st.orders[0].end = Some(ChainEnd {
        passed: false,
        leaf: "q-r1".into(),
        why: None,
        at: 1,
    });
    match next(&st, &j.state(), NOW, &Caps::default(), true) {
        Step::Finish(end, _) => assert_eq!(end, ends::STOPPED),
        other => panic!("{other:?}"),
    }
    let mut j = J::new("stop-write");
    let mut st = state(Stage::Write, &["benchmark"]);
    st.orders[0].end = passed("q-r1");
    st.writer = Some(writer_chain(&st));
    j.seat("q-write", WRITE_CLASS);
    assert_eq!(
        next(&st, &j.state(), NOW, &Caps::default(), true),
        Step::Stop(
            vec![(None, "q-write".to_owned())],
            "stopped by `herdr scholar stop`".to_owned()
        )
    );
    // A stopped run ends at once: nobody waits for its wake.
    let mut done = st.clone();
    done.end = Some(ends::STOPPED.into());
    done.told = true;
    done.logged = true;
    done.ended_at = Some(NOW);
    let mut queued = J::new("stop-queued");
    queued.add(Kind::Asked {
        order: None,
        question: "scholar stopped".into(),
    });
    assert!(!queued.state().queue.is_empty());
    let caps = Caps {
        wakeable: Some(true),
        ..Caps::default()
    };
    assert_eq!(next(&done, &queued.state(), NOW, &caps, true), Step::EndRun);
}

#[test]
fn p47c_a_resume_in_each_stage_goes_on_from_the_state_file() {
    // Route: no answer recorded, so the route turn runs again.
    let st = state(Stage::Route, &[]);
    assert_eq!(
        next(
            &st,
            &J::new("resume-route").state(),
            NOW,
            &Caps::default(),
            false
        ),
        Step::Route
    );
    // Research: r1's worker died with the PC; r2 was never seated. r1 is reseated once, then r2 seated.
    let mut j = J::new("resume-research");
    let st = state(Stage::Research, &["benchmark", "trajectory"]);
    j.seat("q-r1", RESEARCH_CLASS);
    j.died("q-r1");
    assert_eq!(
        next(&st, &j.state(), NOW, &Caps::default(), false),
        Step::Reseat(0, "q-r1-r1".into())
    );
    let mut st2 = st.clone();
    st2.orders[0].reseat = Some("q-r1-r1".into());
    j.seat("q-r1-r1", RESEARCH_CLASS);
    assert_eq!(
        next(&st2, &j.state(), NOW, &Caps::default(), false),
        Step::Seat(1)
    );
    // Write: a writer never seated is seated; one seated and running waits.
    let mut j = J::new("resume-write");
    let mut st = state(Stage::Write, &["benchmark"]);
    st.orders[0].end = passed("q-r1");
    st.writer = Some(writer_chain(&st));
    assert_eq!(
        next(&st, &j.state(), NOW, &Caps::default(), false),
        Step::SeatWriter
    );
    j.seat("q-write", WRITE_CLASS);
    assert_eq!(
        next(&st, &j.state(), NOW, &Caps::default(), false),
        Step::Wait
    );
    // Ending: told, logged, then the queue empty: the run ends.
    let mut st = state(Stage::Ending, &["benchmark"]);
    st.end = Some(ends::NO_DELIVERY.into());
    assert_eq!(
        next(&st, &j.state(), NOW, &Caps::default(), false),
        Step::Tell
    );
    st.told = true;
    assert_eq!(
        next(&st, &j.state(), NOW, &Caps::default(), false),
        Step::Log
    );
    st.logged = true;
    let quiet = J::new("resume-ending");
    assert_eq!(
        next(&st, &quiet.state(), NOW, &Caps::default(), false),
        Step::EndRun
    );
}

#[test]
fn p47c_the_run_ends_only_after_its_wake_is_typed_within_a_bound() {
    let mut st = state(Stage::Ending, &["benchmark"]);
    st.end = Some(ends::REPORT.into());
    st.ended_at = Some(NOW);
    st.logged = true;
    // A report tells nothing of its own: the writer's delivery is the wake.
    assert!(!tells(ends::REPORT) && tells(ends::NO_DELIVERY));
    let mut j = J::new("end-wait");
    j.seat("q-write", WRITE_CLASS);
    j.pass("q-write");
    let run = j.state();
    assert_eq!(run.queue.len(), 2, "{:?}", run.queue);
    let can = Caps {
        wakeable: Some(true),
        ..Caps::default()
    };
    assert_eq!(next(&st, &run, NOW + 60, &can, false), Step::Wait);
    // The bound.
    assert_eq!(next(&st, &run, NOW + END_WAIT_S, &can, false), Step::EndRun);
    // Nobody to wake: at once.
    let cannot = Caps {
        wakeable: Some(false),
        ..Caps::default()
    };
    assert_eq!(next(&st, &run, NOW + 60, &cannot, false), Step::EndRun);
    // Typed: at once.
    let through = j.state().last_seq;
    j.add(Kind::Woken {
        via: "prompt".into(),
        through,
        digest: "d".into(),
    });
    assert_eq!(next(&st, &j.state(), NOW + 60, &can, false), Step::EndRun);
    // Ended by someone else: done.
    j.add(ended_kind());
    assert_eq!(next(&st, &j.state(), NOW + 60, &can, false), Step::Done);
    assert_eq!(END_WAIT_S, 6 * 3600);
}

// ---- the replay hunk ----

#[test]
fn p47c_replay_queues_nothing_for_a_scholar_researcher_and_every_line_of_the_writer() {
    let mut j = J::new("quiet");
    // A researcher that passes first time.
    j.seat("q-r1", RESEARCH_CLASS);
    j.pass("q-r1");
    // One sent back, whose child (why send_back) passes.
    j.seat("q-r2", RESEARCH_CLASS);
    j.deliver("q-r2", "x");
    j.send_back("q-r2", "q-r2-s1");
    j.pass("q-r2-s1");
    assert_eq!(j.state().queue, Vec::new(), "researchers queue nothing");
    // A question, a stall and a failure still queue.
    j.seat("q-r3", RESEARCH_CLASS);
    j.deliver("q-r3", "blocked");
    j.add(Kind::Asked {
        order: Some("q-r3".into()),
        question: "q-r3 asks for a login".into(),
    });
    j.gate("q-r3", "asked", "blocked");
    j.seat("q-r4", RESEARCH_CLASS);
    j.add(Kind::Stalled {
        order: "q-r4".into(),
        silent_s: 600,
        stalls: 1,
        next: "restart in 30 s".into(),
    });
    // The writer's every line.
    j.seat("q-write", WRITE_CLASS);
    j.pass("q-write");
    // An ordinary research.web order, not a scholar's.
    j.add(Kind::Seated(Box::new(seated(
        "plain",
        RESEARCH_CLASS,
        None,
        None,
    ))));
    j.pass("plain");
    let lines: Vec<String> = j
        .state()
        .queue
        .iter()
        .map(|item| item.line.clone())
        .collect();
    let starts: Vec<&str> = lines
        .iter()
        .map(|line| line.split(' ').next().unwrap())
        .collect();
    assert_eq!(
        starts,
        ["q-r3", "q-r3", "q-r4", "q-write", "q-write", "plain", "plain"],
        "{lines:#?}"
    );
    assert!(lines[0].starts_with("q-r3 asks:"), "{lines:#?}");
    assert!(lines[1].starts_with("q-r3 gated asked"), "{lines:#?}");
    assert!(
        lines[3].starts_with("q-write delivered done:"),
        "{lines:#?}"
    );
}

#[test]
fn p47c_a_quiet_researcher_keeps_its_row_on_the_agents_panel() {
    let root = temp("rows");
    let project = Project::at(&root);
    let run = "scholar-20261120-rows";
    let journal = project.journal(run);
    let now = nirvana_run::now();
    journal
        .append(
            now,
            Kind::Opened {
                project: root.display().to_string(),
                orchestrator_pane: Some("p47c-rows-pane".into()),
                auto: true,
                pools: Vec::new(),
                pools_basis: None,
            },
        )
        .unwrap();
    journal
        .append(
            now,
            Kind::Seated(Box::new(seated(
                "rows-r1",
                RESEARCH_CLASS,
                Some(STAGE_WHY),
                None,
            ))),
        )
        .unwrap();
    journal
        .append(
            now,
            Kind::Delivered {
                order: "rows-r1".into(),
                report: serde_json::json!({"status": "done", "summary": "s", "gotchas": []}),
            },
        )
        .unwrap();
    let replayed = nirvana_run::replay(&journal.read().0);
    assert!(replayed.queue.is_empty(), "{:?}", replayed.queue);
    let rows = crate::nirvana_subagents::read_seat_orders(&root, "p47c-rows-pane", now);
    assert!(rows.iter().any(|row| row.name == "rows-r1"), "{rows:?}");
}

#[test]
fn p47c_the_writers_summary_path_fits_the_wake_with_a_40_character_slug() {
    let slug40 = "a".repeat(40);
    let mut st = state(Stage::Write, &[]);
    st.slug = slug40.clone();
    st.report_dir = format!("{REPORTS}/2026-11-20-{slug40}");
    let path = report_html_rel(&st);
    let mut j = J::new("wake-path");
    let id = writer_id(&slug40);
    j.seat(&id, WRITE_CLASS);
    j.deliver(
        &id,
        &format!("{path} The answer: X is slow because of Y, measured on three machines."),
    );
    let line = j.state().queue[0].line.clone();
    assert!(
        line.starts_with(&format!("{id} delivered done: {path}")),
        "{line}"
    );
    let digest = nirvana_run::digest(&st.run, &j.state().queue);
    assert!(digest.contains(&path), "{digest}");
    // The writer's order asks for the path first, and owns its folder.
    let text = write_text(&st, &j.state());
    assert!(
        text.contains(&format!("`summary` starts with `{path}`")),
        "{text}"
    );
    assert!(
        text.contains(&format!("\nowns: {}/\n", st.report_dir)),
        "{text}"
    );
    assert_eq!(
        crate::nirvana_gate::spec(&text).owns,
        [format!("{}/", st.report_dir)]
    );
}

// ---- the verb ----

#[test]
fn p47c_each_refusal_of_the_verb_says_one_line_and_writes_nothing() {
    let none = |_: &str| None;
    // Inside a seated worker.
    let plain = Project::at(temp("not-git"));
    let seated_env = |key: &str| (key == "NIRVANA_ORDER").then(|| "x-r1".to_owned());
    let refused = start(&plain, "Q?", None, &seated_env, false, 1).unwrap_err();
    assert_eq!(refused.code, 2);
    assert!(
        refused.line.starts_with("refused inside order x-r1"),
        "{refused:?}"
    );
    // An empty question.
    let refused = start(&plain, "   ", None, &none, false, 1).unwrap_err();
    assert!(
        refused.line.starts_with("the question is empty"),
        "{refused:?}"
    );
    // Not a git repository.
    let refused = start(&plain, "Q?", None, &none, false, 1).unwrap_err();
    assert!(
        refused.line.contains("is not a git repository"),
        "{refused:?}"
    );
    assert!(
        !plain.dir().exists(),
        "a refusal wrote {}",
        plain.dir().display()
    );
    // Another run here with a live driver: named; --force starts another.
    let (_home, project) = git_project("p47c-busy");
    let first = start(
        &project,
        "First question?",
        None,
        &none,
        false,
        1_763_600_000,
    )
    .unwrap();
    let held = take_driver(&project, &first.state.run).unwrap();
    let refused = start(
        &project,
        "Second question?",
        None,
        &none,
        false,
        1_763_600_001,
    )
    .unwrap_err();
    assert!(refused.line.contains(&first.state.run), "{refused:?}");
    assert_eq!(scholar_runs(&project).len(), 1);
    let forced = start(
        &project,
        "Second question?",
        None,
        &none,
        true,
        1_763_600_001,
    )
    .unwrap();
    assert_ne!(forced.state.run, first.state.run);
    drop(held);
    // With no live driver an unfinished run is no bar.
    assert!(start(
        &project,
        "Third question?",
        None,
        &none,
        false,
        1_763_600_002
    )
    .is_ok());
    nirvana_seat::set_test_world(None);
}

#[test]
fn p47c_the_opened_line_names_the_verbs_pane_and_the_driver_inherits_its_env() {
    let (_home, project) = git_project("p47c-opened");
    let started = start(
        &project,
        "Why?",
        Some("wT:p7"),
        &|_| None,
        false,
        1_763_600_000,
    )
    .unwrap();
    let replayed = nirvana_run::replay(&project.journal(&started.state.run).read().0);
    assert_eq!(
        replayed.opened.unwrap().orchestrator_pane.as_deref(),
        Some("wT:p7")
    );
    assert_eq!(replayed.orchestrator_pane.as_deref(), Some("wT:p7"));
    let saved = load_state(&project, &started.state.run).unwrap();
    assert_eq!(saved.pane.as_deref(), Some("wT:p7"));
    assert_eq!(saved.stage, Stage::Route);
    // The driver's command keeps the caller's env (HERDR_PANE_ID among it) and names the project.
    let log = std::fs::File::create(temp("cmd").join("driver.log")).unwrap();
    let command =
        driver_command(Path::new("herdr.exe"), &project, &started.state.run, log).unwrap();
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args, ["scholar", "drive", started.state.run.as_str()]);
    let envs: Vec<(String, Option<String>)> = command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    assert_eq!(
        envs,
        [(
            nirvana_run::PROJECT_ENV.to_owned(),
            Some(project.root.display().to_string())
        )]
    );
    nirvana_seat::set_test_world(None);
}

#[test]
fn p47c_slugs_and_ids_stay_plain_and_short() {
    let long: String = (0..60).map(|n| format!("word{n} ")).collect();
    assert!(long.len() >= 300, "{}", long.len());
    let cut = slug(&long);
    assert!(cut.len() <= SLUG_MAX, "{cut}");
    assert!(
        !cut.ends_with('-') && cut.split('-').all(|word| word.starts_with("word")),
        "{cut}"
    );
    assert!(
        cut.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "{cut}"
    );
    let (ymd, date) = dates(1_763_600_000);
    assert_eq!((ymd.as_str(), date.as_str()), ("20251120", "2025-11-20"));
    let run = format!("scholar-{ymd}-{cut}-9");
    assert!(nirvana_run::valid_id(&run), "{run}");
    for id in [
        format!("{}-s3", research_id(&cut, 5)),
        format!("{}-r1-s3", writer_id(&cut)),
    ] {
        assert!(nirvana_run::valid_id(&id), "{id}");
    }
    // A slug of 64 still leaves room for every suffix.
    let wide = "z".repeat(64);
    assert!(nirvana_run::valid_id(&format!(
        "{}-r1-s3",
        writer_id(&wide)
    )));
    // No ASCII letter or digit.
    let other = slug("日本語の質問ですか？");
    assert!(
        other.starts_with("question-") && other.len() == 17,
        "{other}"
    );
    assert_eq!(other, slug("日本語の質問ですか？"));
    assert_ne!(other, slug("別の質問"));
    // A path: no dot, slash or `..` reaches a folder name.
    let path = slug("../../Windows/System32/config/SAM.txt");
    assert_eq!(path, "windows-system32-config-sam-txt");
    let root = temp("slug-root");
    let dir = report_dir(&root, &date, &path);
    assert!(
        Path::new(&dir)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_))),
        "{dir}"
    );
    // The -2 rule, for the run and the folder.
    let project = Project::at(&root);
    let first = run_id(&project, &ymd, &path);
    std::fs::create_dir_all(project.run_dir(&first)).unwrap();
    assert_eq!(run_id(&project, &ymd, &path), format!("{first}-2"));
    std::fs::create_dir_all(root.join(&dir)).unwrap();
    assert_eq!(report_dir(&root, &date, &path), format!("{dir}-2"));
    // A word that ends at the cap is kept whole; one cut by the cap goes.
    assert_eq!(
        slug("aaaaaaaaaa bbbbbbbbbb cccccccccc ddddddd tail"),
        "aaaaaaaaaa-bbbbbbbbbb-cccccccccc-ddddddd"
    );
    assert_eq!(
        slug("aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddd tail"),
        "aaaaaaaaaa-bbbbbbbbbb-cccccccccc"
    );
    assert_eq!(slug(&format!("{} tail", "b".repeat(40))), "b".repeat(40));
}

#[test]
fn p47c_stop_order_stops_an_order_with_no_supervisor_as_seat_stop_does() {
    let (_home, project) = git_project("p47c-stop-order");
    let run = "scholar-20261120-stop";
    let journal = project.journal(run);
    journal
        .append(
            1,
            Kind::Opened {
                project: project.root.display().to_string(),
                orchestrator_pane: None,
                auto: true,
                pools: Vec::new(),
                pools_basis: None,
            },
        )
        .unwrap();
    let mut order = seated("stop-r1", RESEARCH_CLASS, Some(STAGE_WHY), None);
    order.cwd = project.root.display().to_string();
    journal.append(2, Kind::Seated(Box::new(order))).unwrap();
    let said = crate::nirvana_worker::stop_order(&project, run, "stop-r1").unwrap();
    assert!(said.starts_with("stopped"), "{said}");
    let replayed = nirvana_run::replay(&journal.read().0);
    assert_eq!(
        status(&replayed, "stop-r1"),
        ChainStatus::NotDelivered("stop-r1".into(), "stopped".into())
    );
    // Stopped already: nothing runs.
    let again = crate::nirvana_worker::stop_order(&project, run, "stop-r1").unwrap();
    assert_eq!(again, "stop-r1 is interrupted; nothing runs");
    nirvana_seat::set_test_world(None);
}

#[test]
fn p47c_a_driver_at_the_cap_stops_the_running_chain_and_tells_the_orchestrator() {
    let (_home, project) = git_project("p47c-cap-drive");
    let pane = std::env::var("HERDR_PANE_ID").unwrap_or_else(|_| "wT:p9".to_owned());
    let started = start(&project, "How slow?", Some(&pane), &|_| None, false, 1_000).unwrap();
    let run = started.state.run.clone();
    let mut st = started.state.clone();
    st.stage = Stage::Research;
    st.route = Some(RouteRecord {
        styles: vec!["benchmark".into()],
        tries: 1,
        ..RouteRecord::default()
    });
    st.orders = vec![Chain {
        id: research_id(&st.slug, 1),
        ..chain(1, "benchmark")
    }];
    save_state(&project, &st).unwrap();
    let leaf = research_id(&st.slug, 1);
    let journal = project.journal(&run);
    journal
        .append(
            2,
            Kind::Seated(Box::new(seated(
                &leaf,
                RESEARCH_CLASS,
                Some(STAGE_WHY),
                None,
            ))),
        )
        .unwrap();
    let stops = RefCell::new(Vec::<String>::new());
    let stop = |_: &Project, _: &str, order: &str| {
        stops.borrow_mut().push(order.to_owned());
        format!("{order}: stopped")
    };
    let wakes = Cell::new(0);
    let sleep = |_: u64| {
        let through = nirvana_run::replay(&journal.read().0).last_seq;
        journal
            .append(
                3,
                Kind::Woken {
                    via: "prompt".into(),
                    through,
                    digest: "d".into(),
                },
            )
            .unwrap();
    };
    let route = |_: &[String], _: &Path| Err("no route turn in this test".to_owned());
    let end_run = |project: &Project, run: &str| {
        project
            .journal(run)
            .append(4, ended_kind())
            .map(|_| ())
            .map_err(|err| err.to_string())
    };
    let wake = |_: &Project, _: &str| wakes.set(wakes.get() + 1);
    let now = || 1_000 + RESEARCH_CAP_S;
    let world = World {
        stop: &stop,
        ..world_with(&route, &sleep, &end_run, &wake, &now)
    };
    let done = drive(&project, &run, &world, Caps::default()).unwrap();
    assert_eq!(*stops.borrow(), [leaf.clone()]);
    let end = done.orders[0].end.clone().unwrap();
    assert!(!end.passed);
    assert_eq!(end.why.as_deref(), Some("over the time cap"));
    assert_eq!(done.end.as_deref(), Some(ends::NO_DELIVERY));
    assert!(done.told && done.logged);
    assert_eq!(wakes.get(), 1);
    assert_eq!(done.stage, Stage::Done);
    nirvana_seat::set_test_world(None);
}

#[test]
fn p47c_the_html_check_fails_every_outside_load_and_passes_a_link() {
    let bad = [
        ("<script src=\"app.js\"></script>", "a <script> with a src"),
        (
            "<script type=\"module\" src='https://cdn.x/a.js'></script>",
            "a <script> with a src",
        ),
        (
            "<link rel=\"stylesheet\" href=\"https://fonts.x/css\">",
            "a <link> href on another host",
        ),
        (
            "<img src=\"http://x.org/a.png\">",
            "a <img> src on another host",
        ),
        (
            "<img srcset=\"//x.org/a.png 2x\">",
            "a <img> srcset on another host",
        ),
        (
            "<iframe src=\"https://x.org\"></iframe>",
            "a <iframe> src on another host",
        ),
        ("<style>@import 'x.css';</style>", "an @import"),
        (
            "<div style=\"background:url(https://x.org/a.png)\"></div>",
            "a url() on another host",
        ),
        (
            "<style>body{background:url( 'http://x.org/a.png')}</style>",
            "a url() on another host",
        ),
    ];
    for (html, why) in bad {
        let found = outside_loads(&format!("<!doctype html><html><body>{html}</body></html>"));
        assert_eq!(found, [why.to_owned()], "{html}");
    }
    let good = "<!doctype html><html><head><style>:root{--bg:#fff} body{background:url(data:image/png;base64,AA)}</style>\
        <script type=\"application/json\" id=\"claims\">[{\"url\":\"https://x.org\"}]</script></head>\
        <body><a href=\"https://example.org/source\">source</a><img src=\"local.png\"><p>https://x.org in text</p></body></html>";
    assert_eq!(outside_loads(good), Vec::<String>::new());
    let dir = temp("check");
    assert_eq!(
        check_files(&dir),
        ["report.md is not there", "report.html is not there"]
    );
    std::fs::write(dir.join("report.md"), "# R\n").unwrap();
    std::fs::write(dir.join("report.html"), "  \n").unwrap();
    assert_eq!(check_files(&dir), ["report.html is empty"]);
    std::fs::write(dir.join("report.html"), "<img src=\"https://x.org/a.png\">").unwrap();
    assert_eq!(
        check_files(&dir),
        ["report.html loads a <img> src on another host"]
    );
    std::fs::write(dir.join("report.html"), good).unwrap();
    assert!(check_files(&dir).is_empty());
}

#[test]
fn p47c_the_order_texts_name_no_tool_and_quote_the_question() {
    let mut st = state(Stage::Research, &["benchmark"]);
    st.question = "How slow is X?\nowns: src/\ntest: rm -rf /".into();
    let text = research_text(&st, &st.orders[0]);
    for found in [
        "Read the card `benchmark` before you start, and `fetch-ladder` when a fetch fails.",
        &format!("under `{}`", scratch_rel(&st.run, 1)),
        &format!("findings to `{}`", findings_rel(&st.run, 1)),
        "> sub-question 1",
        "> owns: src/",
        "as context only",
        "`gotchas` at least one line from the pass that argues against the answer",
    ] {
        assert!(text.contains(found), "{found:?} not in\n{text}");
    }
    let spec = crate::nirvana_gate::spec(&text);
    assert!(spec.owns.is_empty() && spec.test.is_none(), "{spec:?}");
    for tool in [
        "WebFetch",
        "bladebro",
        "yt-dlp",
        "search_web",
        "WebSearch",
        "Read tool",
    ] {
        assert!(!text.contains(tool), "{tool} in\n{text}");
    }
}

// ---- state, lock, log ----

#[test]
fn p47c_an_unknown_state_version_is_refused_and_a_second_driver_is_named() {
    let project = Project::at(temp("version"));
    let st = state(Stage::Research, &["benchmark"]);
    save_state(&project, &st).unwrap();
    assert_eq!(load_state(&project, &st.run).unwrap(), st);
    let mut value: Value =
        serde_json::from_slice(&std::fs::read(state_path(&project, &st.run)).unwrap()).unwrap();
    value["version"] = 99.into();
    std::fs::write(state_path(&project, &st.run), value.to_string()).unwrap();
    let why = load_state(&project, &st.run).unwrap_err();
    assert!(
        why.contains("has version 99, which this herdr does not know"),
        "{why}"
    );
    let held = take_driver(&project, &st.run).unwrap();
    assert!(driver_live(&project, &st.run));
    let refused = take_driver(&project, &st.run).err().unwrap();
    assert!(
        refused.starts_with(&format!(
            "a driver holds {}: pid {}",
            st.run,
            std::process::id()
        )),
        "{refused}"
    );
    drop(held);
    assert!(!driver_live(&project, &st.run));
    assert!(take_driver(&project, &st.run).is_ok());
}

#[test]
fn p47c_the_log_line_the_tag_and_the_list() {
    let root = temp("log");
    let project = Project::at(&root);
    let mut j = J::new("log-journal");
    j.seat("q-r1", RESEARCH_CLASS);
    j.add(Kind::Usage {
        order: Some("q-r1".into()),
        attempt: 1,
        session: None,
        tokens: Tokens {
            input: 10,
            cache_write: 0,
            cache_read: 5,
            output: 3,
        },
    });
    j.deliver("q-r1", "x");
    j.send_back("q-r1", "q-r1-s1");
    j.add(Kind::Usage {
        order: Some("q-r1-s1".into()),
        attempt: 1,
        session: None,
        tokens: Tokens {
            input: 1,
            cache_write: 1,
            cache_read: 1,
            output: 1,
        },
    });
    j.pass("q-r1-s1");
    let mut st = state(Stage::Ending, &["benchmark"]);
    st.orders[0].end = passed("q-r1-s1");
    st.end = Some(ends::REPORT.into());
    let line = log_line(&st, &j.state(), 1600);
    assert_eq!(line["v"], LOG_VERSION);
    assert_eq!(line["end"], "report");
    assert_eq!(line["report"], "docs/research/2026-11-20-q/report.html");
    assert_eq!(line["outcome"], Value::Null);
    assert_eq!(line["wall_s"], 600);
    assert_eq!(line["orders"][0]["send_backs"], 1);
    assert_eq!(line["orders"][0]["tokens"]["input"], 11);
    assert_eq!(line["orders"][0]["end"], "passed");
    assert_eq!(line["routing"]["styles"][0], "benchmark");
    append_line(&log_path(&root), &line).unwrap();
    // A tag of a run the log does not hold is refused; the last tag wins; nothing is rewritten.
    assert!(tag_line(&root, "scholar-nope", "hit", None, 1)
        .unwrap_err()
        .contains("no scholar run scholar-nope"));
    assert!(tag_line(&root, &st.run, "great", None, 1).is_err());
    append_line(
        &log_path(&root),
        &tag_line(&root, &st.run, "miss", None, 1).unwrap(),
    )
    .unwrap();
    append_line(
        &log_path(&root),
        &tag_line(&root, &st.run, "hit", Some("good"), 2).unwrap(),
    )
    .unwrap();
    let lines = read_log(&root);
    assert_eq!(lines.len(), 3);
    assert_eq!(outcome_of(&lines, &st.run).as_deref(), Some("hit"));
    assert_eq!(
        lines[2],
        serde_json::json!({"run": st.run, "outcome": "hit", "note": "good", "at": 2})
    );
    // The list: newest first, with the outcome, and an unfinished run from its state file.
    let mut older = st.clone();
    older.run = "scholar-20261119-old".into();
    older.started_at = 10;
    older.stage = Stage::Research;
    older.end = None;
    save_state(&project, &older).unwrap();
    let mut newer = st.clone();
    newer.started_at = 2000;
    save_state(&project, &newer).unwrap();
    let list = list_text(&project);
    let rows: Vec<&str> = list.lines().collect();
    assert_eq!(rows.len(), 2, "{list}");
    assert!(
        rows[0].starts_with(&format!("{} · 2026-11-20 · report · outcome hit", st.run)),
        "{list}"
    );
    assert!(
        rows[1].starts_with("scholar-20261119-old · 2026-11-20 · research (no driver"),
        "{list}"
    );
}

// ---- a whole run ----

fn listing(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if rel == ".git" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn p47c_a_whole_scripted_run_seats_researchers_then_the_writer_logs_and_ends() {
    let (_home, project) = git_project("p47c-whole");
    let before = listing(&project.root);
    // The pane the verb runs in. A test run inside herdr has one: using it keeps `seat_order_why` from asking herdr
    // about a pane move.
    let pane = std::env::var("HERDR_PANE_ID").unwrap_or_else(|_| "wT:p9".to_owned());
    let t0 = 1_763_600_000;
    let started = start(
        &project,
        "Why is the build slow, and by how much?",
        Some(&pane),
        &|_| None,
        false,
        t0,
    )
    .unwrap();
    let run = started.state.run.clone();
    let slug = started.state.slug.clone();
    let (r1, r2, writer) = (
        research_id(&slug, 1),
        research_id(&slug, 2),
        writer_id(&slug),
    );
    // Held live, so the seats start no worker.
    let _live: Vec<_> = [&r1, &r2, &writer]
        .iter()
        .map(|id| {
            crate::nirvana_worker::take_live(&project, &run, id)
                .unwrap()
                .unwrap()
        })
        .collect();
    let routes = Cell::new(0);
    let route = |_argv: &[String], _cwd: &Path| {
        routes.set(routes.get() + 1);
        exited(result_line(serde_json::json!({
            "orders": [
                {"style": "hypothesis-elimination", "question": "Why is the build slow?", "why": "why is"},
                {"style": "benchmark", "question": "How slow is the build, with what conditions?", "why": "by how much"},
            ],
            "note": "",
        })))
    };
    let clock = Cell::new(t0);
    let now = || clock.get();
    let stages = RefCell::new(Vec::<Stage>::new());
    let queued = RefCell::new(Vec::<String>::new());
    let wakes = Cell::new(0);
    let ended = Cell::new(false);
    let journal = project.journal(&run);
    let report_dir = project.root.join(&started.state.report_dir);
    let html_rel = report_html_rel(&started.state);
    // Each sleep is a poll: the scripted workers and the gate act between two.
    let sleep = |_secs: u64| {
        clock.set(clock.get() + POLL_S);
        let st = load_state(&project, &run).unwrap();
        if stages.borrow().last() != Some(&st.stage) {
            stages.borrow_mut().push(st.stage);
        }
        let at = clock.get();
        let replayed = nirvana_run::replay(&journal.read().0);
        let delivered = |id: &str| {
            replayed
                .order(id)
                .is_some_and(|known| known.report.is_some())
        };
        let seated = |id: &str| replayed.order(id).is_some();
        let report = |id: &str, summary: &str| Kind::Delivered {
            order: id.to_owned(),
            report: serde_json::json!({"status": "done", "summary": summary, "gotchas": ["the other side"],
                "sources": [{"url": format!("https://example.org/{id}"), "used_for": "a claim"}]}),
        };
        let pass = |id: &str| Kind::Gated {
            order: id.to_owned(),
            verdict: "pass".into(),
            reason: None,
            axes: Vec::new(),
        };
        if seated(&r1) && seated(&r2) && !delivered(&r1) {
            for (n, id) in [(1, &r1), (2, &r2)] {
                let findings = project.root.join(findings_rel(&run, n));
                std::fs::write(
                    &findings,
                    format!("- claim {n} · https://example.org/{id}\n"),
                )
                .unwrap();
                journal
                    .append(at, report(id, &format!("answer {n}")))
                    .unwrap();
                journal.append(at, pass(id)).unwrap();
            }
        } else if seated(&writer) && !delivered(&writer) {
            std::fs::write(report_dir.join("report.md"), "# The build\n").unwrap();
            std::fs::write(
                report_dir.join("report.html"),
                "<!doctype html><title>r</title><p>ok</p>",
            )
            .unwrap();
            journal
                .append(
                    at,
                    report(
                        &writer,
                        &format!("{html_rel} The build is slow because of X."),
                    ),
                )
                .unwrap();
            journal.append(at, pass(&writer)).unwrap();
        } else if replayed
            .order(&writer)
            .is_some_and(|known| known.verdict.is_some())
            && !replayed.queue.is_empty()
        {
            // The orchestrator's wake: what it was told, then the journal says it was typed.
            *queued.borrow_mut() = replayed
                .queue
                .iter()
                .map(|item| item.line.clone())
                .collect();
            journal
                .append(
                    at,
                    Kind::Woken {
                        via: "prompt".into(),
                        through: replayed.last_seq,
                        digest: "d".into(),
                    },
                )
                .unwrap();
        }
    };
    let end_run = |project: &Project, run: &str| {
        ended.set(true);
        project
            .journal(run)
            .append(clock.get(), ended_kind())
            .map(|_| ())
            .map_err(|err| err.to_string())
    };
    let wake = |_: &Project, _: &str| wakes.set(wakes.get() + 1);
    let world = world_with(&route, &sleep, &end_run, &wake, &now);
    let done = drive(&project, &run, &world, Caps::default()).unwrap();

    assert_eq!(done.stage, Stage::Done);
    assert_eq!(done.end.as_deref(), Some(ends::REPORT), "{done:#?}");
    assert_eq!(routes.get(), 1);
    assert!(ended.get());
    assert_eq!(
        wakes.get(),
        0,
        "a report wakes through the writer's own delivery"
    );
    // The seated lines.
    let replayed = nirvana_run::replay(&journal.read().0);
    assert_eq!(
        replayed
            .opened
            .as_ref()
            .unwrap()
            .orchestrator_pane
            .as_deref(),
        Some(pane.as_str())
    );
    for (id, class, profile) in [
        (&r1, RESEARCH_CLASS, Some("scholar")),
        (&r2, RESEARCH_CLASS, Some("scholar")),
        (&writer, WRITE_CLASS, None),
    ] {
        let known = replayed
            .order(id)
            .unwrap_or_else(|| panic!("{id} not seated"));
        assert_eq!(known.seated.class, class, "{id}");
        assert_eq!(
            known.seated.profile,
            Some(profile.map(str::to_owned)),
            "{id}"
        );
        assert_eq!(known.seated.why.as_deref(), Some(STAGE_WHY), "{id}");
        assert!(known.seated.worktree.is_none(), "{id}");
    }
    // The state file at each stage the polls saw.
    let seen = stages.borrow().clone();
    assert!(
        seen.contains(&Stage::Research)
            && seen.contains(&Stage::Write)
            && seen.contains(&Stage::Ending),
        "{seen:?}"
    );
    // The order texts.
    let text_of =
        |id: &str| std::fs::read_to_string(&replayed.order(id).unwrap().seated.order_file).unwrap();
    for (n, id, style) in [(1, &r1, "hypothesis-elimination"), (2, &r2, "benchmark")] {
        let text = text_of(id);
        assert!(text.contains(&format!("Read the card `{style}`")), "{text}");
        assert!(text.contains(&scratch_rel(&run, n)), "{text}");
        assert!(text.contains(&findings_rel(&run, n)), "{text}");
    }
    let wtext = text_of(&writer);
    assert!(
        wtext.contains(&findings_rel(&run, 1)) && wtext.contains(&findings_rel(&run, 2)),
        "{wtext}"
    );
    assert!(
        wtext.contains(&format!("\nowns: {}/\n", started.state.report_dir)),
        "{wtext}"
    );
    // The wake: nothing of the researchers, the writer's lines, its delivery opening with the path.
    let lines = queued.borrow().clone();
    assert!(!lines.is_empty());
    assert!(
        lines
            .iter()
            .all(|line| line.starts_with(&format!("{writer} "))),
        "{lines:#?}"
    );
    assert!(
        lines[0].starts_with(&format!("{writer} delivered done: {html_rel}")),
        "{lines:#?}"
    );
    // The log line, and the run's end.
    let log = read_log(&project.root);
    assert_eq!(log.len(), 1);
    assert_eq!(log[0]["run"], run.as_str());
    assert_eq!(log[0]["end"], "report");
    assert_eq!(log[0]["orders"].as_array().unwrap().len(), 2);
    assert_eq!(log[0]["outcome"], Value::Null);
    assert!(replayed.ended.is_some());
    // New files only under .nirvana/ and docs/research/.
    let after = listing(&project.root);
    let new: Vec<&String> = after.iter().filter(|path| !before.contains(path)).collect();
    assert!(!new.is_empty());
    assert!(
        new.iter()
            .all(|path| path.starts_with(".nirvana/") || path.starts_with("docs/research/")),
        "{new:#?}"
    );
    assert!(after.contains(&format!("{}/report.html", started.state.report_dir)));
    nirvana_seat::set_test_world(None);
}

#[test]
fn p47c_nothing_to_research_ends_told_logged_and_with_no_order() {
    let (_home, project) = git_project("p47c-nothing");
    let pane = std::env::var("HERDR_PANE_ID").unwrap_or_else(|_| "wT:p9".to_owned());
    let started = start(
        &project,
        "hello there",
        Some(&pane),
        &|_| None,
        false,
        1_763_600_000,
    )
    .unwrap();
    let run = started.state.run.clone();
    let route = |_: &[String], _: &Path| {
        exited(result_line(
            serde_json::json!({"orders": [], "note": "a greeting"}),
        ))
    };
    let wakes = Cell::new(0);
    let wake = |_: &Project, _: &str| wakes.set(wakes.get() + 1);
    let end_run = |project: &Project, run: &str| {
        project
            .journal(run)
            .append(1, ended_kind())
            .map(|_| ())
            .map_err(|err| err.to_string())
    };
    let now = || 1_763_600_000;
    let journal = project.journal(&run);
    let sleep = |_: u64| {
        let through = nirvana_run::replay(&journal.read().0).last_seq;
        journal
            .append(
                1,
                Kind::Woken {
                    via: "prompt".into(),
                    through,
                    digest: "d".into(),
                },
            )
            .unwrap();
    };
    let world = world_with(&route, &sleep, &end_run, &wake, &now);
    let done = drive(&project, &run, &world, Caps::default()).unwrap();
    assert_eq!(done.end.as_deref(), Some(ends::NOTHING));
    assert!(done.end_why.as_deref().unwrap().contains("a greeting"));
    assert!(done.orders.is_empty());
    assert_eq!(wakes.get(), 1);
    let events = journal.read().0;
    assert!(nirvana_run::replay(&events).orders.is_empty());
    let asked: Vec<&str> = events
        .iter()
        .filter_map(|event| match &event.kind {
            Kind::Asked {
                order: None,
                question,
            } => Some(question.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(
        asked[0].starts_with(&format!(
            "scholar nothing_to_research: `herdr scholar status {run}`"
        )),
        "{asked:?}"
    );
    assert_eq!(read_log(&project.root)[0]["end"], ends::NOTHING);
    assert!(!project.root.join(&started.state.report_dir).exists());
    nirvana_seat::set_test_world(None);
}
