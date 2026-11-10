use super::*;
use crate::nirvana_run::Event;
use std::collections::BTreeMap;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "herdr-p33b-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn git(cwd: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A git project with no remote, `.nirvana/` made.
fn repo(name: &str) -> PathBuf {
    let root = temp(name);
    git(&root, &["-c", "init.defaultBranch=main", "init", "-q"]);
    git(&root, &["config", "user.name", "Review Test"]);
    git(&root, &["config", "user.email", "review@test"]);
    git(&root, &["config", "core.autocrlf", "false"]);
    std::fs::write(root.join("calc.py"), "def add(a, b):\n    return a + b\n").expect("calc");
    git(&root, &["add", "calc.py"]);
    git(&root, &["commit", "-q", "-m", "base"]);
    Project::at(&root).ensure().expect(".nirvana");
    root
}

/// A worker's claude stream: a command whose output is `output`, then its report.
fn stream(output: &str) -> String {
    [
        serde_json::json!({"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","name":"Bash","input":{"command":"git ls-files"}}]}}),
        serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","content":output}]}}),
        serde_json::json!({"type":"result","subtype":"success"}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n")
}

fn done(status: &str) -> Value {
    serde_json::json!({"status":status,"summary":"added mul","files":["calc.py"],"tests":[{"name":"test_mul","result":"pass"}],"gotchas":[]})
}

/// `done`, naming a gotcha: a pass the gate reviews (review-trim-agy-browser 2026-11-02: a clean first pass is not).
fn learned() -> Value {
    let mut report = done("done");
    report["gotchas"] = serde_json::json!(["calc.py: mul takes no floats"]);
    report
}

/// A profile that would show on a worker that took it: a persona, a model, an effort, a forbidden tool.
const SLICER: &str = "---\ndescription: Slices prints through the studio server.\nmodel: claude-opus-5-5\neffort: high\n\
disallowed_tools: [Bash]\n---\nYou slice prints only through the studio server.\n";

/// The profile `name` copied into the run as `seat run --profile` copies it, and the order copy's two lines.
fn seated_profile(root: &Path, run: &str, name: &str) -> crate::nirvana_profiles::Ref {
    let project = Project::at(root);
    std::fs::create_dir_all(crate::nirvana_profiles::folder(root)).expect("profiles");
    std::fs::write(crate::nirvana_profiles::path(root, name), SLICER).expect("profile");
    let loaded = crate::nirvana_profiles::load(root, name).expect("load");
    crate::nirvana_profiles::copy_into_run(&project.run_dir(run), &loaded).expect("copy");
    crate::nirvana_profiles::Ref {
        name: name.into(),
        sha: loaded.sha,
    }
}

/// Order `id` of class `build.bounded` in its own worktree, delivered with `report`, its stream holding `output`.
/// With a profile it is seated as `seat run --profile` seats it: the copy in the run, the order copy's two lines, the
/// `seated` reason naming the profile and its sha.
fn delivered(root: &Path, run: &str, id: &str, report: Value, output: &str, profile: Option<&str>) {
    let project = Project::at(root);
    let cwd = root.join(".nirvana").join("worktrees").join(id);
    git(
        root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            &format!("nirvana/{id}"),
            &cwd.display().to_string(),
            "HEAD",
        ],
    );
    std::fs::write(
        cwd.join("calc.py"),
        "def add(a, b):\n    return a + b\n\ndef mul(a, b):\n    return a * b\n",
    )
    .expect("mul");
    std::fs::write(cwd.join("test_mul.py"), "from calc import mul\n").expect("test");
    let profiled = profile.map(|name| seated_profile(root, run, name));
    let text = format!(
        "{}# Add mul\n\nowns: calc.py test_mul.py\ntest: git ls-files\ncheck: test_mul\n",
        profiled
            .as_ref()
            .map(crate::nirvana_profiles::order_lines)
            .unwrap_or_default()
    );
    let order_file = crate::nirvana_worker::write_order(&project, &cwd, id, &text).expect("order");
    let journal = project.journal(run);
    if !journal.path.is_file() {
        journal
            .append(
                1,
                Kind::Opened {
                    project: root.display().to_string(),
                    orchestrator_pane: None,
                    auto: false,
                    pools: Vec::new(),
                    pools_basis: None,
                },
            )
            .expect("opened");
    }
    let mut seated = seated(id, "build.bounded");
    if let Some(found) = &profiled {
        seated.reason = format!("r; profile {} sha256 {}", found.name, found.sha);
    }
    seated.cwd = cwd.display().to_string();
    seated.worktree = Some(cwd.display().to_string());
    seated.branch = Some(format!("nirvana/{id}"));
    seated.order_file = order_file;
    journal
        .append(2, Kind::Seated(Box::new(seated)))
        .expect("seated");
    write_stream(&project, run, id, &stream(output));
    journal
        .append(
            3,
            Kind::Delivered {
                order: id.into(),
                report,
            },
        )
        .expect("delivered");
}

fn write_stream(project: &Project, run: &str, id: &str, text: &str) {
    let dir = project.run_dir(run).join("streams");
    std::fs::create_dir_all(&dir).expect("streams");
    std::fs::write(dir.join(format!("{id}.jsonl")), text).expect("stream");
}

fn seated(id: &str, class: &str) -> Seated {
    Seated {
        order: id.into(),
        seat: "sonnet".into(),
        cli: "claude".into(),
        model: "claude-sonnet-5-5".into(),
        effort: crate::nirvana_seat::Effort::Medium,
        pool: "max".into(),
        class: class.into(),
        chain: Some("build.bounded".into()),
        size: SizeName::Small,
        mode: Mode::Balanced,
        needs: Vec::new(),
        not_family: Vec::new(),
        reason: "r".into(),
        override_reason: None,
        order_file: "o.md".into(),
        cwd: "C:/p".into(),
        worktree: None,
        branch: None,
        parent: None,
        why: None,
        rule_pick: None,
        bar_dropped: None,
        step_down: None,
        use_basis: None,
        expected: BTreeMap::new(),
        jev_pick: None,
        send_ok: None,
    }
}

fn state_of(root: &Path, run: &str) -> nirvana_run::RunState {
    replay(&Project::at(root).journal(run).read().0)
}

/// No test starts a worker or reads this machine's seats: the stand-in world, a test home, no herdr pane.
fn quiet(name: &str) -> crate::nirvana::TestHomeGuard {
    let guard = crate::nirvana::with_test_home(name);
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    // A send-back child starts through `launch_worker`; nextest gives each test its own process, so these hold
    // for this test alone, and the child is the test binary refusing `seat exec` as arguments.
    std::env::set_var("NIRVANA_NO_PANE", "1");
    std::env::remove_var("HERDR_PANE_ID");
    std::env::remove_var(crate::api::SOCKET_PATH_ENV_VAR);
    guard
}

fn gated_axes(root: &Path, run: &str, id: &str) -> Vec<Value> {
    Project::at(root)
        .journal(run)
        .read()
        .0
        .into_iter()
        .rev()
        .find_map(|event: Event| match event.kind {
            Kind::Gated { order, axes, .. } if order == id => Some(axes),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn p33_a_review_is_queued_after_a_pass_and_its_new_op_keeps_a_lesson() {
    let _home = quiet("review-pass");
    let root = repo("review-pass");
    let project = Project::at(&root);
    let mut report = done("done");
    report["lesson"] = serde_json::json!({"name":"Count the files","class":"build.bounded","steps":"1. git ls-files","evidence":"TOTAL 12 passed"});
    delivered(
        &root,
        "r1",
        "b1",
        report,
        "TOTAL 12 passed, stream only tail",
        Some("slicer"),
    );
    let ruling = nirvana_gate::gate(&project, "r1", "b1").expect("gate");
    assert_eq!(ruling.verdict, "pass", "{ruling:?}");
    // §3.1 and §3.2: the worker's own lesson is kept with no remote, carrying the order copy's profile.
    let worker = &nirvana_lessons::read_index(&root).lessons["count-the-files"];
    assert_eq!(
        (worker.origin, worker.profile.as_deref()),
        (nirvana_lessons::Origin::Worker, Some("slicer"))
    );
    assert!(
        ruling
            .next
            .iter()
            .any(|line| line.starts_with("b1-lr seated on flash low")),
        "{:?}",
        ruling.next
    );
    let state = state_of(&root, "r1");
    let review = state.order("b1-lr").expect("the review is seated");
    assert_eq!(review.seated.class, CLASS);
    assert_eq!(review.seated.mode, Mode::Efficient);
    // Review B3: its own folder under the run, never the project's root.
    let cwd = review_cwd(&project, "r1", "b1-lr");
    assert_eq!(review.seated.cwd, cwd.display().to_string());
    assert!(cwd.starts_with(project.run_dir("r1")));
    let brief = std::fs::read_to_string(&review.seated.order_file).expect("brief");
    assert!(
        brief.contains(&cwd.join("stream.jsonl").display().to_string()),
        "the brief names the stream's copy in the review's folder: {brief}"
    );
    assert_eq!(
        std::fs::read_to_string(cwd.join("stream.jsonl")).expect("copy"),
        stream("TOTAL 12 passed, stream only tail")
    );
    assert!(
        brief.contains("## The rules") && brief.contains("pass\n"),
        "{brief}"
    );
    assert!(brief.contains("profile slicer"), "{brief}");
    assert!(
        !brief.contains("stream only tail"),
        "the brief never holds the stream"
    );
    // The reviewer delivers one op whose evidence is in b1's stream, and a lesson of its own.
    let mut report = done("done");
    report["lesson_ops"] = serde_json::json!([{"op":"new","name":"Read the suite total","class":"build.bounded","steps":"1. read TOTAL","evidence":"TOTAL 12 passed, stream only tail"}]);
    report["lesson"] = serde_json::json!({"name":"Mine","class":"lesson-review","steps":"x","evidence":"TOTAL 12 passed"});
    project
        .journal("r1")
        .append(
            9,
            Kind::Delivered {
                order: "b1-lr".into(),
                report,
            },
        )
        .expect("delivered");
    let ruling = nirvana_gate::gate(&project, "r1", "b1-lr").expect("gate the review");
    assert_eq!(ruling.verdict, "pass", "{ruling:?}");
    let index = nirvana_lessons::read_index(&root);
    let kept = &index.lessons["read-the-suite-total"];
    assert_eq!(kept.origin, nirvana_lessons::Origin::Review);
    assert_eq!(kept.profile.as_deref(), Some("slicer"));
    assert!(
        !index.lessons.contains_key("mine"),
        "the reviewer's own lesson is ignored"
    );
    // A review never reviews a review.
    assert!(state_of(&root, "r1").order("b1-lr-lr").is_none());
    assert!(ruling.next.is_empty());
}

#[test]
fn p33_a_review_is_queued_after_a_send_back_with_its_reasons() {
    let _home = quiet("review-send-back");
    let root = repo("review-send-back");
    let project = Project::at(&root);
    // Plan 37b: a blocked delivery is asked, never sent back, so the send-back comes from a committed file the order
    // does not own (an uncommitted one is left out of the gate's commit, a note).
    delivered(&root, "r1", "b2", done("done"), "nothing", None);
    let cwd = root.join(".nirvana").join("worktrees").join("b2");
    std::fs::write(cwd.join("unowned.py"), "x = 1\n").expect("unowned");
    git(&cwd, &["add", "unowned.py"]);
    git(&cwd, &["commit", "-q", "-m", "unowned"]);
    let ruling = nirvana_gate::gate(&project, "r1", "b2").expect("gate");
    assert_eq!(ruling.verdict, "sent_back", "{ruling:?}");
    assert!(
        ruling
            .next
            .iter()
            .any(|line| line.starts_with("b2-lr seated on")),
        "{:?}",
        ruling.next
    );
    let state = state_of(&root, "r1");
    let brief = std::fs::read_to_string(&state.order("b2-lr").expect("review").seated.order_file)
        .expect("brief");
    assert!(brief.contains("sent back, for:\n\n- owned:"), "{brief}");
}

#[test]
fn p33_over_the_daily_cap_the_review_is_skipped_and_journaled() {
    let _home = quiet("review-cap");
    let root = repo("review-cap");
    let project = Project::at(&root);
    let today = now() / 86_400;
    change_state(&root, |state| {
        state.day = today;
        state.queued = PER_DAY;
    })
    .expect("state");
    delivered(&root, "r1", "b3", learned(), "TOTAL 1 passed", None);
    let ruling = nirvana_gate::gate(&project, "r1", "b3").expect("gate");
    assert_eq!(ruling.verdict, "pass");
    assert!(state_of(&root, "r1").order("b3-lr").is_none());
    let axes = gated_axes(&root, "r1", "b3");
    let skipped = axes
        .iter()
        .find(|axis| axis["axis"] == SKIPPED)
        .unwrap_or_else(|| panic!("no review_skipped axis: {axes:?}"));
    assert!(
        skipped["why"]
            .as_str()
            .unwrap_or("")
            .contains("daily cap of 20"),
        "{skipped}"
    );
    // A new day has room again; a cap of 0 turns reviews off.
    assert_eq!(cap_refusal(&root, PER_DAY, now() + 86_400), None);
    assert!(cap_refusal(&root, 0, now()).is_some());
    let mut review = state_of(&root, "r1").order("b3").expect("b3").clone();
    review.seated.class = CLASS.into();
    assert_eq!(
        wanted_with(
            &project,
            &state_of(&root, "r1"),
            &review,
            &Verdict::Pass,
            PER_DAY,
            now() + 86_400
        ),
        Err(Some("a review never reviews a review".into()))
    );
}

#[test]
fn p33_the_cap_reads_lesson_reviews_per_day_from_constants() {
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    assert!(nirvana_seat::DEFAULT.contains("\nlesson_reviews_per_day = 20\n"));
    assert_eq!(per_day(Some(&table)), PER_DAY);
    // A table written before the line reads the default; a refused table, too.
    let old = nirvana_seat::parse(nirvana_seat::HISTORICAL_DEFAULTS[4]).expect("v5");
    assert_eq!(per_day(Some(&old)), PER_DAY);
    let three =
        nirvana_seat::DEFAULT.replace("lesson_reviews_per_day = 20", "lesson_reviews_per_day = 3");
    assert_eq!(
        per_day(Some(&nirvana_seat::parse(&three).expect("three"))),
        3
    );
    assert_eq!(per_day(None), PER_DAY);
}

#[test]
fn p33_the_review_seats_flash_first_then_sonnet_low_at_efficient() {
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    let world = nirvana_seat::World {
        now: 1_792_289_318,
        ..nirvana_seat::World::default()
    };
    let first = pick(&table, &world, "x-lr").expect("pick");
    assert_eq!((first.seat.as_str(), first.effort.word()), ("flash", "low"));
    let mut no_agy = world.clone();
    no_agy.clis.insert("agy".into(), "signed out".into());
    let second = pick(&table, &no_agy, "x-lr").expect("pick");
    assert_eq!(
        (second.seat.as_str(), second.effort.word()),
        ("sonnet", "low")
    );
}

fn kept(root: &Path, name: &str, class: &str) -> String {
    let text = stream(&format!("{name} proof"));
    let lesson = serde_json::json!({"name":name,"class":class,"steps":"1. do it","evidence":format!("{name} proof")});
    let verdict = nirvana_lessons::verify_and_write(
        root,
        &lesson,
        true,
        &text,
        "o",
        "r",
        5,
        nirvana_lessons::Meta::default(),
    );
    assert!(
        matches!(verdict, nirvana_lessons::Verdict::Written(_)),
        "{verdict:?}"
    );
    nirvana_lessons::slug(name)
}

#[test]
fn p33_each_lesson_op_refusal_says_why() {
    let root = temp("ops");
    let slug = kept(&root, "Read the total", "build.bounded");
    let sha = nirvana_lessons::body_sha(&root, &slug).expect("sha");
    let text = stream("TOTAL 3 passed, 0 failed in 1.2 s");
    let proof = "TOTAL 3 passed, 0 failed";
    let report = serde_json::json!({"lesson_ops":[
        {"op":"new","name":"Not proved","class":"build.bounded","steps":"1","evidence":"never printed by any command"},
        {"op":"patch","slug":slug,"base_sha":"0000","name":"Read the total","class":"build.bounded","steps":"2","evidence":proof},
        {"op":"rename","name":"x"},
        {"op":"merge","slugs":["a","b"],"name":"x","steps":"y"},
        {"op":"patch","slug":slug,"name":"Read the total","class":"build.bounded","steps":"2","evidence":proof},
        {"op":"new","name":"Read the total","class":"build.bounded","steps":"3","evidence":proof},
        {"op":"patch","slug":slug,"base_sha":sha,"name":"Read the total","class":"build.bounded","steps":"1. read TOTAL last","evidence":proof},
    ]});
    let applied = apply_review(
        &root,
        parse_ops(&report),
        &text,
        &Scope {
            order: "s1",
            class: "build.bounded",
            profile: None,
            herdr_wrote: Vec::new(),
        },
        "s1-lr",
        "r",
        9,
    );
    let lines: Vec<(bool, &str)> = applied
        .lines
        .iter()
        .map(|(k, l)| (*k, l.as_str()))
        .collect();
    assert_eq!(
        lines[0],
        (false, "op 1 refused: the evidence is not in s1's stream")
    );
    assert!(
        !lines[1].0 && lines[1].1.contains("base_sha 0000 is not"),
        "{lines:?}"
    );
    assert_eq!(lines[2], (false, "op 3 refused: unknown op \"rename\""));
    assert_eq!(
        lines[3],
        (
            false,
            "op 4 refused: merge is a merge order's op, not a review's"
        )
    );
    assert_eq!(
        lines[4],
        (
            false,
            "op 5 refused: a patch names the slug and the base_sha it read"
        )
    );
    assert_eq!(
        lines[5],
        (
            false,
            "op 6 refused: a lesson read-the-total exists: patch it with its base_sha"
        )
    );
    assert!(
        lines[6].0 && lines[6].1.ends_with("patched read-the-total"),
        "{lines:?}"
    );
    let body = nirvana_lessons::body(&root, &slug).expect("body");
    assert!(body.ends_with("1. read TOTAL last\n"), "{body}");
    // Only the applied op is in the ledger.
    assert_eq!(nirvana_lessons::ledger(&root).len(), 1);
}

/// Review S1 and S2: an op keeps a lesson only for the reviewed order's class and profile, and only on a quote of at
/// least 20 characters that the worker's stream holds and herdr did not write.
#[test]
fn p33_a_review_op_stays_in_its_scope_and_its_evidence_is_the_workers_own() {
    let root = temp("scope");
    let gui = kept(&root, "Click the field first", "gui.studio");
    let gui_sha = nirvana_lessons::body_sha(&root, &gui).expect("sha");
    let own = kept(&root, "Read the total", "build.bounded");
    let own_sha = nirvana_lessons::body_sha(&root, &own).expect("sha");
    let order_copy = "# Add mul\n\nRun the suite and read TOTAL from its last line.\n";
    let text = stream(&format!(
        "{order_copy}\nTOTAL 3 passed, 0 failed in 1.2 s\nRead the total proof"
    ));
    let proof = "TOTAL 3 passed, 0 failed";
    let report = serde_json::json!({"lesson_ops":[
        {"op":"new","name":"Plant a gui lesson","class":"gui.studio","steps":"1","evidence":proof},
        {"op":"patch","slug":gui,"base_sha":gui_sha,"name":"Click","class":"build.bounded","steps":"2","evidence":proof},
        {"op":"new","name":"Short","class":"build.bounded","steps":"1","evidence":"TOTAL 3"},
        {"op":"new","name":"From the order","class":"build.bounded","steps":"1","evidence":"Run the suite and read TOTAL"},
        {"op":"new","name":"From a lesson","class":"build.bounded","steps":"1","evidence":"Read the total proof"},
        {"op":"patch","slug":own,"base_sha":own_sha,"name":"Read the total","class":"gui.studio","steps":"1. read TOTAL last","evidence":proof},
    ]});
    let applied = apply_review(
        &root,
        parse_ops(&report),
        &text,
        &Scope {
            order: "s1",
            class: "build.bounded",
            profile: None,
            herdr_wrote: vec![order_copy.to_owned()],
        },
        "s1-lr",
        "r",
        9,
    );
    let lines: Vec<(bool, &str)> = applied
        .lines
        .iter()
        .map(|(k, l)| (*k, l.as_str()))
        .collect();
    assert_eq!(
        lines[0],
        (
            false,
            "op 1 refused: a review of a build.bounded order keeps build.bounded lessons, not gui.studio"
        )
    );
    assert_eq!(
        lines[1],
        (
            false,
            "op 2 refused: click-the-field-first is not a lesson the reviewed order is offered (class gui.studio)"
        )
    );
    assert_eq!(
        lines[2],
        (
            false,
            "op 3 refused: the evidence is 7 characters; a quote of at least 20 proves something"
        )
    );
    assert_eq!(
        lines[3],
        (
            false,
            "op 4 refused: the evidence is text herdr wrote (the order or this brief), not the worker's"
        )
    );
    assert_eq!(
        lines[4],
        (
            false,
            "op 5 refused: the evidence is lesson read-the-total's own text"
        )
    );
    // A patch keeps the entry's own class, whatever the op says.
    assert!(lines[5].0, "{lines:?}");
    let index = nirvana_lessons::read_index(&root);
    assert_eq!(index.lessons[&own].class, "build.bounded");
    assert_eq!(index.lessons[&gui].class, "gui.studio");
    assert!(!index.lessons.contains_key("plant-a-gui-lesson"));
    let body = nirvana_lessons::body(&root, &own).expect("body");
    assert!(body.contains("\nclass: build.bounded\n"), "{body}");
}

/// Review B1: a review of an order seated with a profile takes none of it. Its brief carries no profile line, a
/// worker started on it gets no persona and no forbidden tools, a reseat is not narrowed, and its ops are scoped to
/// the profile the subject's `seated` event journals.
#[test]
fn p33_a_review_of_a_profiled_order_takes_no_profile() {
    let _home = quiet("review-profile");
    let root = repo("review-profile");
    let project = Project::at(&root);
    delivered(
        &root,
        "r1",
        "b1",
        learned(),
        // The worker read its order file back: the stream holds herdr's own text too.
        "TOTAL 12 passed, 0 failed in 3 s\nowns: calc.py test_mul.py\n",
        Some("slicer"),
    );
    nirvana_gate::gate(&project, "r1", "b1").expect("gate");
    let state = state_of(&root, "r1");
    let subject = state.order("b1").expect("b1").seated.clone();
    let review = state.order("b1-lr").expect("review").seated.clone();
    let brief = std::fs::read_to_string(&review.order_file).expect("brief");
    assert!(
        crate::nirvana_profiles::in_order(&brief).is_none()
            && !brief.lines().any(|line| line.starts_with("Profile")),
        "{brief}"
    );
    assert!(brief.contains("profile slicer"), "{brief}");
    // A review whose order file did say the subject's lines, as a reseat's continuation brief quotes its parent's.
    let mut quoting = review.clone();
    quoting.cli = "claude".into();
    quoting.order_file = subject.order_file.clone();
    let run_dir = project.run_dir("r1");
    assert_eq!(
        crate::nirvana_worker::with_profile(vec!["claude".to_owned()], &run_dir, &quoting),
        Ok(vec!["claude".to_owned()])
    );
    let mut request = nirvana_seat::Request::new(CLASS);
    crate::nirvana_profiles::narrow_for_order(
        &mut request,
        &run_dir,
        &std::fs::read_to_string(&subject.order_file).expect("copy"),
    )
    .expect("narrow");
    assert_eq!((request.profile, request.claude_only), (None, None));
    // Its op keeps a lesson of the subject's journaled profile; an op quoting the order copy herdr wrote keeps none.
    let mut report = done("done");
    report["lesson_ops"] = serde_json::json!([
        {"op":"new","name":"Read the suite total","class":"build.bounded","steps":"1. read TOTAL","evidence":"TOTAL 12 passed, 0 failed"},
        {"op":"new","name":"Own the files","class":"build.bounded","steps":"1. own them","evidence":"owns: calc.py test_mul.py"},
    ]);
    project
        .journal("r1")
        .append(
            9,
            Kind::Delivered {
                order: "b1-lr".into(),
                report,
            },
        )
        .expect("delivered");
    let ruling = nirvana_gate::gate(&project, "r1", "b1-lr").expect("gate the review");
    assert_eq!(ruling.verdict, "pass", "{ruling:?}");
    let index = nirvana_lessons::read_index(&root);
    assert_eq!(
        index.lessons["read-the-suite-total"].profile.as_deref(),
        Some("slicer")
    );
    assert!(!index.lessons.contains_key("own-the-files"), "{ruling:?}");
}

/// Review B3: a lesson written into `.nirvana/lessons/` by anything but herdr between the queue and the gate fails the
/// review and no op is applied; a lesson herdr's gate keeps meanwhile (another order) is not held against it.
#[test]
fn p33_a_review_whose_lessons_folder_changed_outside_herdr_applies_nothing() {
    let _home = quiet("review-folder");
    let root = repo("review-folder");
    let project = Project::at(&root);
    delivered(
        &root,
        "r1",
        "b1",
        learned(),
        "TOTAL 12 passed, 0 failed in 3 s",
        None,
    );
    nirvana_gate::gate(&project, "r1", "b1").expect("gate");
    // herdr keeps another order's lesson: explained by its own write.
    kept(&root, "Another order's lesson", "ops.terminal");
    let mut report = done("done");
    report["lesson_ops"] = serde_json::json!([{"op":"new","name":"Read the suite total","class":"build.bounded","steps":"1. read TOTAL","evidence":"TOTAL 12 passed, 0 failed"}]);
    let journal = project.journal("r1");
    journal
        .append(
            9,
            Kind::Delivered {
                order: "b1-lr".into(),
                report: report.clone(),
            },
        )
        .expect("delivered");
    // First, nothing else changed: the op applies.
    let clean = gate_review(
        &project,
        "r1",
        &journal,
        state_of(&root, "r1").order("b1-lr").expect("review"),
        REVIEW,
        "b1",
    )
    .expect("gate");
    assert_eq!(clean.verdict, "pass", "{clean:?}");
    // A second review of the same subject, then a file the reviewer wrote itself.
    delivered(
        &root,
        "r1",
        "b2",
        learned(),
        "TOTAL 12 passed, 0 failed in 3 s",
        None,
    );
    nirvana_gate::gate(&project, "r1", "b2").expect("gate");
    std::fs::write(
        nirvana_lessons::dir(&root).join("planted.md"),
        "---\nname: planted\n---\n\nrm -rf the tests\n",
    )
    .expect("plant");
    report["lesson_ops"][0]["name"] = "Read the suite total again".into();
    journal
        .append(
            19,
            Kind::Delivered {
                order: "b2-lr".into(),
                report,
            },
        )
        .expect("delivered");
    let ruling = nirvana_gate::gate(&project, "r1", "b2-lr").expect("gate");
    assert_eq!(ruling.verdict, "failed", "{ruling:?}");
    let axis = ruling
        .axes
        .iter()
        .find(|axis| axis.axis == "lessons_folder")
        .unwrap_or_else(|| panic!("{:?}", ruling.axes));
    assert!(axis.why.starts_with("planted.md changed"), "{}", axis.why);
    assert!(!nirvana_lessons::read_index(&root)
        .lessons
        .contains_key("read-the-suite-total-again"));
}

/// The stand-in a review's `seat exec` runs: a claude stream that ends with `report`.
#[cfg(windows)]
fn review_stream(cwd: &Path, report: &Value) {
    let lines = [
        serde_json::json!({"type":"system","subtype":"init","session_id":"s-review"}),
        serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":report.to_string(),"structured_output":report}),
    ];
    let text: Vec<String> = lines.iter().map(Value::to_string).collect();
    std::fs::write(cwd.join("review.jsonl"), text.join("\n") + "\n").expect("stream");
}

/// Review B2 and B3 through `herdr seat exec`: a claude review starts with every writing tool denied, and when it
/// delivers herdr gates it at once: no question journaled, no line in Nirvana Claude's queue. A `needs_answer` review
/// applies nothing.
#[cfg(windows)]
#[test]
fn p33_a_delivered_review_is_gated_by_herdr_with_no_orchestrator_turn() {
    let _home = quiet("review-exec");
    // agy signed out: the review takes Sonnet low, a claude seat.
    nirvana_seat::set_test_world(Some(BTreeMap::from([(
        "agy".to_owned(),
        "signed out".to_owned(),
    )])));
    let root = repo("review-exec");
    let project = Project::at(&root);
    let run = "r1";
    let journal = project.journal(run);
    let exec_review = |subject: &str, report: Value| -> (Vec<String>, String) {
        delivered(
            &root,
            run,
            subject,
            learned(),
            "TOTAL 12 passed, 0 failed in 3 s",
            Some("slicer"),
        );
        nirvana_gate::gate(&project, run, subject).expect("gate");
        // Plan 37 review N8: a line already waits in the queue, so a wake would act on it; the review's delivery
        // still wakes no one.
        journal
            .append(
                crate::nirvana_run::now(),
                Kind::Asked {
                    order: None,
                    question: format!("queued before {subject}'s review"),
                },
            )
            .unwrap();
        let id = review_id(subject);
        let review = state_of(&root, run)
            .order(&id)
            .expect("review")
            .seated
            .clone();
        assert_eq!(review.cli, "claude", "{review:?}");
        review_stream(Path::new(&review.cwd), &report);
        crate::nirvana_worker::TEST_COMMAND.with_borrow_mut(|slot| {
            *slot = Some(vec![
                "cmd.exe".into(),
                "/c".into(),
                "type review.jsonl".into(),
            ]);
        });
        let mut out = Vec::new();
        crate::nirvana_worker::exec(&project, run, &id, None, &mut out).expect("exec");
        crate::nirvana_worker::TEST_COMMAND.with_borrow_mut(|slot| *slot = None);
        let events = journal.read().0;
        let delivered_at = events
            .iter()
            .position(|event| matches!(&event.kind, Kind::Delivered { order, .. } if *order == id))
            .expect("delivered");
        assert!(
            !events[delivered_at..]
                .iter()
                .any(|event| matches!(event.kind, Kind::WakeQueued { .. } | Kind::Woken { .. })),
            "a lesson review woke the orchestrator: {}",
            String::from_utf8_lossy(&out)
        );
        let argv = journal
            .read()
            .0
            .into_iter()
            .find_map(|event| match event.kind {
                Kind::Started { order, argv, .. } if order == id => Some(argv),
                _ => None,
            })
            .unwrap_or_else(|| panic!("started: {}", String::from_utf8_lossy(&out)));
        (argv, String::from_utf8_lossy(&out).into_owned())
    };
    let mut report = done("done");
    report["lesson_ops"] = serde_json::json!([{"op":"new","name":"Read the suite total","class":"build.bounded","steps":"1. read TOTAL","evidence":"TOTAL 12 passed, 0 failed"}]);
    let (argv, out) = exec_review("b1", report.clone());
    let at = argv
        .iter()
        .position(|arg| arg == "--disallowedTools")
        .unwrap_or_else(|| panic!("{argv:?}"));
    assert_eq!(argv[at + 1], DENIED);
    let denied: Vec<&str> = DENIED.split(',').collect();
    for tool in [
        "Bash",
        "PowerShell",
        "Edit",
        "Write",
        "NotebookEdit",
        "Agent",
    ] {
        assert!(denied.contains(&tool), "{tool} is not denied: {DENIED}");
    }
    assert!(
        !argv.iter().any(|arg| arg == "--append-system-prompt-file"),
        "the subject's persona: {argv:?}"
    );
    let state = state_of(&root, run);
    assert_eq!(
        state.order("b1-lr").and_then(|o| o.verdict.as_deref()),
        Some("pass"),
        "{out}"
    );
    assert!(nirvana_lessons::read_index(&root)
        .lessons
        .contains_key("read-the-suite-total"));
    assert!(
        !state
            .queue
            .iter()
            .any(|item| item.order.as_deref() == Some("b1-lr")),
        "{:?}",
        state.queue
    );
    // needs_answer: no question for the orchestrator, and nothing applied.
    report["status"] = "needs_answer".into();
    report["question"] = "keep it?".into();
    report["lesson_ops"][0]["name"] = "Read the suite total twice".into();
    let (_, out) = exec_review("b2", report);
    let state = state_of(&root, run);
    assert_eq!(
        state.order("b2-lr").and_then(|o| o.verdict.as_deref()),
        Some("failed"),
        "{out}"
    );
    assert!(!journal
        .read()
        .0
        .iter()
        .any(|event| matches!(&event.kind, Kind::Asked { order: Some(o), .. } if o == "b2-lr")));
    assert!(!state
        .queue
        .iter()
        .any(|item| item.order.as_deref() == Some("b2-lr")));
    assert!(!nirvana_lessons::read_index(&root)
        .lessons
        .contains_key("read-the-suite-total-twice"));
    nirvana_seat::set_test_world(None);
}

#[test]
fn p33_a_crowded_class_is_merged_archived_ledgered_and_rolled_back() {
    let _home = quiet("merge");
    let root = repo("merge");
    let project = Project::at(&root);
    delivered(&root, "r1", "seed", done("done"), "x", None);
    let mut slugs = Vec::new();
    for n in 0..9 {
        slugs.push(kept(&root, &format!("Studio tip {n}"), "build.bounded"));
    }
    // Counters to sum.
    {
        let mut index = nirvana_lessons::read_index(&root);
        for (n, slug) in slugs.iter().enumerate() {
            let c = &mut index.lessons.get_mut(slug).expect("entry").counters;
            (c.offered, c.opened, c.passed_after_opened) = (n as u32, n as u32, n as u32);
        }
        std::fs::write(
            nirvana_lessons::dir(&root).join(nirvana_lessons::INDEX),
            serde_json::to_string(&index).expect("json"),
        )
        .expect("index");
    }
    assert_eq!(crowded(&root), vec![("build.bounded".into(), None, 9)]);
    let lines = merge_crowded(&project, "r1");
    assert!(
        lines[0].contains("lm-build-bounded-1 seated on"),
        "{lines:?}"
    );
    let state = state_of(&root, "r1");
    let brief = std::fs::read_to_string(
        &state
            .order("lm-build-bounded-1")
            .expect("merge")
            .seated
            .order_file,
    )
    .expect("brief");
    assert!(
        brief.contains("### studio-tip-8") && brief.contains("\"op\": \"merge\""),
        "{brief}"
    );
    // A second run end does not queue a second merge of the same class.
    assert!(merge_crowded(&project, "r1")[0].contains("pending already"));
    let mut report = done("done");
    report["lesson_ops"] = serde_json::json!([
        {"op":"merge","slugs":[slugs[0], slugs[1], slugs[2]],"name":"Studio tips","steps":"1. click the field first"},
        {"op":"new","name":"x","class":"build.bounded","steps":"1","evidence":"x"},
        {"op":"merge","slugs":[slugs[3], "not-given"],"name":"y","steps":"z"},
    ]);
    project
        .journal("r1")
        .append(
            9,
            Kind::Delivered {
                order: "lm-build-bounded-1".into(),
                report,
            },
        )
        .expect("delivered");
    // `herdr run end`'s settle gates the delivered merge.
    let settled = settle(&project);
    assert_eq!(settled, vec!["lm-build-bounded-1: pass".to_owned()]);
    let index = nirvana_lessons::read_index(&root);
    let umbrella = &index.lessons["studio-tips"];
    assert_eq!(umbrella.origin, nirvana_lessons::Origin::Merge);
    assert_eq!(
        (
            umbrella.counters.offered,
            umbrella.counters.opened,
            umbrella.counters.passed_after_opened
        ),
        (3, 3, 3),
        "the merged counters are the sum of the parts'"
    );
    for part in &slugs[..3] {
        assert!(!index.lessons.contains_key(part));
        assert!(!nirvana_lessons::dir(&root)
            .join(format!("{part}.md"))
            .exists());
        assert!(nirvana_lessons::dir(&root)
            .join(nirvana_lessons::ARCHIVE)
            .join(format!("op-1-{part}.md"))
            .is_file());
    }
    let axes = gated_axes(&root, "r1", "lm-build-bounded-1");
    let whys: Vec<&str> = axes.iter().filter_map(|a| a["why"].as_str()).collect();
    assert!(
        whys.iter().any(|w| w.contains("new is a review's op")),
        "{whys:?}"
    );
    assert!(
        whys.iter()
            .any(|w| w.contains("not-given is not one of the lessons")),
        "{whys:?}"
    );
    let ledger = nirvana_lessons::ledger(&root);
    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger[0].op, "merge");
    assert!(ledger[0].before.iter().filter(|s| s.file.is_some()).count() == 3);
    assert!(ledger[0]
        .after
        .iter()
        .any(|s| s.slug == "studio-tips" && s.file.is_some()));
    // Rollback restores the parts with their counters and takes the umbrella out.
    let said = nirvana_lessons::rollback(&root, "op-1", 20).expect("rollback");
    assert!(said.starts_with("op-2: rolled back op-1 (merge"), "{said}");
    let index = nirvana_lessons::read_index(&root);
    assert!(!index.lessons.contains_key("studio-tips"));
    assert_eq!(index.lessons[&slugs[2]].counters.offered, 2);
    assert!(nirvana_lessons::dir(&root)
        .join(format!("{}.md", slugs[0]))
        .is_file());
    assert!(nirvana_lessons::rollback(&root, "op-1", 21)
        .expect_err("twice")
        .contains("rolled back already"));
    assert!(
        settle(&project).is_empty(),
        "a settled merge leaves the pending list"
    );
}

#[test]
fn p33_run_end_queues_the_merge_of_a_crowded_class_and_the_receipt_prints_origins() {
    let _home = quiet("run-end");
    let root = repo("run-end");
    let project = Project::at(&root);
    delivered(&root, "r1", "seed", done("done"), "x", None);
    assert!(!crate::nirvana_retro::open_section(&project).contains("Lessons by origin"));
    for n in 0..9 {
        kept(&root, &format!("Tip {n}"), "ops.terminal");
    }
    let line = crate::nirvana_retro::retro(&project, "r1").expect("retro");
    assert!(
        line.contains("lessons merge (9 live): lm-ops-terminal-1 seated on"),
        "{line}"
    );
    let receipt = crate::nirvana_retro::open_section(&project);
    assert!(
        receipt.contains("## Lessons by origin\n\n- worker: 9 lesson(s) · offered 0"),
        "{receipt}"
    );
}

// ---- review-trim-agy-browser 2026-11-02 (live checks F8): a review only when there is something to learn ----

/// One order of `chain_state`: `(id, parent, why, verdict)`.
type Link<'a> = (&'a str, Option<&'a str>, Option<&'a str>, Option<&'a str>);

/// A journal of `orders`, every one delivered `report`, replayed.
fn chain_state(root: &Path, run: &str, orders: &[Link], report: &Value) -> nirvana_run::RunState {
    let journal = Project::at(root).journal(run);
    for (n, (id, parent, why, verdict)) in orders.iter().enumerate() {
        let at = 10 + 3 * n as u64;
        let mut seated = seated(id, "build.bounded");
        seated.parent = parent.map(str::to_owned);
        seated.why = why.map(str::to_owned);
        journal
            .append(at, Kind::Seated(Box::new(seated)))
            .expect("seated");
        journal
            .append(
                at + 1,
                Kind::Delivered {
                    order: (*id).into(),
                    report: report.clone(),
                },
            )
            .expect("delivered");
        if let Some(verdict) = verdict {
            journal
                .append(
                    at + 2,
                    Kind::Gated {
                        order: (*id).into(),
                        verdict: (*verdict).into(),
                        reason: None,
                        axes: Vec::new(),
                    },
                )
                .expect("gated");
        }
    }
    state_of(root, run)
}

#[test]
fn lrs_a_clean_first_pass_is_skipped_before_the_cap_counts_it() {
    let _home = quiet("lrs-skip");
    let root = repo("lrs-skip");
    let project = Project::at(&root);
    let state = chain_state(&root, "r1", &[("c1", None, None, None)], &done("done"));
    let known = state.order("c1").expect("c1");
    let why = wanted_with(&project, &state, known, &Verdict::Pass, PER_DAY, now())
        .expect_err("a clean first pass is skipped")
        .expect("with a reason");
    assert!(
        why.starts_with("nothing to learn: a first-pass delivery"),
        "{why}"
    );
    // The skip comes before the cap: a spent cap still reads as nothing to learn, and nothing is counted.
    set_used_for_test(&root, now(), PER_DAY);
    assert_eq!(
        wanted_with(&project, &state, known, &Verdict::Pass, PER_DAY, now()),
        Err(Some(why))
    );
    assert_eq!(used_today(&root, now()), PER_DAY);
    set_used_for_test(&root, now(), 0);
    // Reviews turned off say so, not "nothing to learn" (review nit).
    assert_eq!(
        wanted_with(&project, &state, known, &Verdict::Pass, 0, now()),
        Err(Some(
            "lesson reviews are off (lesson_reviews_per_day = 0)".into()
        ))
    );
    // A send-back verdict always has something to learn.
    assert_eq!(
        wanted_with(
            &project,
            &state,
            known,
            &Verdict::SentBack(vec!["owned: x".into()]),
            PER_DAY,
            now()
        ),
        Ok(())
    );
}

#[test]
fn lrs_each_report_signal_keeps_the_review() {
    let _home = quiet("lrs-signals");
    let root = repo("lrs-signals");
    for (n, (key, value)) in [
        (
            "gotchas",
            serde_json::json!(["calc.py: mul ignores negatives"]),
        ),
        (
            "lesson",
            serde_json::json!({"name":"n","class":"build.bounded","steps":"s","evidence":"e"}),
        ),
        ("lesson_ops", serde_json::json!([{"op":"new"}])),
        ("card_ops", serde_json::json!([{"op":"new"}])),
        // A lesson written as one line of text is a lesson too.
        ("lesson", serde_json::json!("calc.py: mul takes no floats")),
    ]
    .into_iter()
    .enumerate()
    {
        let mut report = done("done");
        report[key] = value;
        let run = format!("r{n}");
        let state = chain_state(&root, &run, &[("s1", None, None, None)], &report);
        assert_eq!(
            idle(&state, state.order("s1").expect("s1"), &Verdict::Pass),
            None,
            "a report naming {key} has something to learn"
        );
    }
    // Empty lists and an empty lesson are the clean report's own shape.
    let mut report = done("done");
    report["lesson_ops"] = serde_json::json!([]);
    report["card_ops"] = serde_json::json!([]);
    report["lesson"] = Value::Null;
    let state = chain_state(&root, "r9", &[("s1", None, None, None)], &report);
    assert!(idle(&state, state.order("s1").expect("s1"), &Verdict::Pass).is_some());
    // A blank says nothing either (review nit): a blank gotcha, a blank lesson line, an empty lesson object.
    for (n, (key, value)) in [
        ("gotchas", serde_json::json!([""])),
        ("gotchas", serde_json::json!(["  ", null])),
        ("lesson", serde_json::json!("  ")),
        ("lesson", serde_json::json!({})),
    ]
    .into_iter()
    .enumerate()
    {
        let mut report = done("done");
        report[key] = value.clone();
        let run = format!("b{n}");
        let state = chain_state(&root, &run, &[("s1", None, None, None)], &report);
        assert!(
            idle(&state, state.order("s1").expect("s1"), &Verdict::Pass).is_some(),
            "{key} = {value} says nothing"
        );
    }
}

#[test]
fn lrs_a_pass_after_a_send_back_keeps_the_review_through_a_reseat() {
    let _home = quiet("lrs-chain");
    let root = repo("lrs-chain");
    let clean = done("done");
    // A redo that passes: its chain holds the correction.
    let state = chain_state(
        &root,
        "r1",
        &[
            ("o", None, None, Some("sent_back")),
            ("o-s1", Some("o"), Some("send_back"), None),
        ],
        &clean,
    );
    assert_eq!(
        idle(&state, state.order("o-s1").expect("redo"), &Verdict::Pass),
        None
    );
    // The redo's pool ran out and a reseat child delivered: `nirvana_gate::chain` stops at the reseat; this walks on.
    let state = chain_state(
        &root,
        "r2",
        &[
            ("o", None, None, Some("sent_back")),
            ("o-s1", Some("o"), Some("send_back"), None),
            ("o-s1-r1", Some("o-s1"), Some("reseat"), None),
        ],
        &clean,
    );
    assert_eq!(
        idle(
            &state,
            state.order("o-s1-r1").expect("reseat"),
            &Verdict::Pass
        ),
        None
    );
    // A reseat of a first order is still the first pass.
    let state = chain_state(
        &root,
        "r3",
        &[
            ("p", None, None, None),
            ("p-r1", Some("p"), Some("reseat"), None),
        ],
        &clean,
    );
    assert!(idle(&state, state.order("p-r1").expect("reseat"), &Verdict::Pass).is_some());
    // A reseat whose parent was gated other than pass (asked) has something to learn.
    let state = chain_state(
        &root,
        "r4",
        &[
            ("q", None, None, Some("asked")),
            ("q-r1", Some("q"), Some("reseat"), None),
        ],
        &clean,
    );
    assert_eq!(
        idle(&state, state.order("q-r1").expect("reseat"), &Verdict::Pass),
        None
    );
}

#[test]
fn lrs_the_gate_seats_no_review_of_a_clean_first_pass_and_counts_none() {
    let _home = quiet("lrs-gate");
    let root = repo("lrs-gate");
    let project = Project::at(&root);
    delivered(&root, "r1", "c1", done("done"), "TOTAL 1 passed", None);
    let ruling = nirvana_gate::gate(&project, "r1", "c1").expect("gate");
    assert_eq!(ruling.verdict, "pass", "{ruling:?}");
    assert!(state_of(&root, "r1").order("c1-lr").is_none());
    assert_eq!(
        used_today(&root, now()),
        0,
        "a skipped review is not counted"
    );
    // Said in the `gated` event, as the `review_skipped` axis: the gate asks `wanted_for` with its verdict.
    let axes = gated_axes(&root, "r1", "c1");
    let journaled = axes.iter().any(|axis| {
        axis["axis"] == SKIPPED
            && axis["why"]
                .as_str()
                .is_some_and(|why| why.starts_with("nothing to learn"))
    });
    assert!(journaled, "{axes:?} {:?}", ruling.next);
    // The same delivery naming a gotcha is reviewed, and counted.
    let mut report = done("done");
    report["gotchas"] = serde_json::json!(["calc.py: mul ignores negatives"]);
    delivered(&root, "r1", "c2", report, "TOTAL 1 passed", None);
    let ruling = nirvana_gate::gate(&project, "r1", "c2").expect("gate");
    assert!(
        ruling
            .next
            .iter()
            .any(|line| line.starts_with("c2-lr seated on")),
        "{:?}",
        ruling.next
    );
    assert_eq!(used_today(&root, now()), 1);
}

/// `Kind::Asked` of `order`, as the gate's `ask` and a worker's question journal it.
fn asked(root: &Path, run: &str, at: u64, order: &str) {
    Project::at(root)
        .journal(run)
        .append(
            at,
            Kind::Asked {
                order: Some(order.into()),
                question: format!("{order}: which file?"),
            },
        )
        .expect("asked");
}

/// One order seated and delivered `report`, with `parent` and `why`, and no gate.
fn seat_and_deliver(root: &Path, run: &str, at: u64, link: Link, report: &Value) {
    let (id, parent, why, _) = link;
    let journal = Project::at(root).journal(run);
    let mut seated = seated(id, "build.bounded");
    seated.parent = parent.map(str::to_owned);
    seated.why = why.map(str::to_owned);
    journal
        .append(at, Kind::Seated(Box::new(seated)))
        .expect("seated");
    journal
        .append(
            at + 1,
            Kind::Delivered {
                order: id.into(),
                report: report.clone(),
            },
        )
        .expect("delivered");
}

/// Review S3: a question in the chain, or the run's question that started it, keeps the review. On 0.52.0 a blocked
/// delivery is asked and its answer is a fresh order with no `parent`: before this, its clean redo read as a first pass.
#[test]
fn lrs_a_question_in_the_chain_or_the_one_that_started_it_keeps_the_review() {
    let _home = quiet("lrs-asked");
    let root = repo("lrs-asked");
    let project = Project::at(&root);
    // The 0.52.0 path, end to end: a blocked delivery is gated asked; the orchestrator seats a fresh redo; it passes
    // with a clean report and is reviewed.
    let mut blocked = done("blocked");
    blocked["summary"] = "calc.py is not in the worktree".into();
    delivered(&root, "r1", "b1", blocked, "TOTAL 0 passed", None);
    let ruling = nirvana_gate::gate(&project, "r1", "b1").expect("gate the block");
    assert_eq!(ruling.verdict, "asked", "{ruling:?}");
    delivered(&root, "r1", "b2", done("done"), "TOTAL 1 passed", None);
    let state = state_of(&root, "r1");
    assert_eq!(state.order("b1").map(|order| order.asked), Some(1));
    assert_eq!(
        state.order("b2").and_then(|order| order.answers.as_deref()),
        Some("b1")
    );
    let ruling = nirvana_gate::gate(&project, "r1", "b2").expect("gate the redo");
    assert_eq!(ruling.verdict, "pass", "{ruling:?}");
    assert!(
        ruling
            .next
            .iter()
            .any(|line| line.starts_with("b2-lr seated on")),
        "{:?}",
        ruling.next
    );
    let clean = done("done");
    // A worker that asked mid-order and then delivered: its own question, which no later order answers.
    let journal = project.journal("r2");
    journal
        .append(10, Kind::Seated(Box::new(seated("m", "build.bounded"))))
        .expect("seated");
    asked(&root, "r2", 11, "m");
    journal
        .append(
            12,
            Kind::Delivered {
                order: "m".into(),
                report: clean.clone(),
            },
        )
        .expect("delivered");
    seat_and_deliver(&root, "r2", 20, ("n", None, None, None), &clean);
    let state = state_of(&root, "r2");
    assert_eq!(
        idle(&state, state.order("m").expect("m"), &Verdict::Pass),
        None
    );
    assert_eq!(
        state.order("n").and_then(|order| order.answers.clone()),
        None
    );
    assert!(idle(&state, state.order("n").expect("n"), &Verdict::Pass).is_some());
    // A gated question: a lesson review seated after it answers nothing; the next fresh order answers it, and so does
    // its reseat child (the same chain root); an order after that, with the question taken, is a first pass again.
    seat_and_deliver(&root, "r3", 10, ("q", None, None, None), &clean);
    asked(&root, "r3", 12, "q");
    let journal = project.journal("r3");
    let mut review = seated("q-lr", CLASS);
    review.why = Some("lesson_review".into());
    journal
        .append(13, Kind::Seated(Box::new(review)))
        .expect("review");
    seat_and_deliver(&root, "r3", 20, ("z", None, None, None), &clean);
    seat_and_deliver(
        &root,
        "r3",
        30,
        ("z-r1", Some("z"), Some("reseat"), None),
        &clean,
    );
    seat_and_deliver(&root, "r3", 40, ("y", None, None, None), &clean);
    let state = state_of(&root, "r3");
    assert_eq!(
        state.order("q-lr").and_then(|order| order.answers.clone()),
        None
    );
    assert_eq!(
        state.order("z").and_then(|order| order.answers.as_deref()),
        Some("q")
    );
    for order in ["z", "z-r1"] {
        assert_eq!(
            idle(&state, state.order(order).expect("answer"), &Verdict::Pass),
            None,
            "{order} answers q"
        );
    }
    assert!(
        idle(&state, state.order("y").expect("y"), &Verdict::Pass).is_some(),
        "another chain's question is not this order's"
    );
}

/// Plan 40: rule 6 keeps what it refused and names its one exception, a measured dead end that was resolved.
#[test]
fn rule_six_keeps_a_measured_and_resolved_dead_end() {
    let six = RULES
        .lines()
        .skip_while(|line| !line.starts_with("6. "))
        .take_while(|line| !line.starts_with("7. "))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(six.contains("a claim that a tool is broken or cannot be used"), "{six}");
    assert!(six.contains("keep nothing"), "{six}");
    assert!(
        six.contains(
            "One exception: a dead end the stream shows the worker measured and then worked around in the project's \
own code or method"
        ),
        "{six}"
    );
    // Plan 40d N11: the exception is scoped, so "python was missing, the worker used py" stays a setup failure.
    assert!(
        six.contains("The exception never covers a missing program, a sign-in or a path on this machine."),
        "{six}"
    );
}
