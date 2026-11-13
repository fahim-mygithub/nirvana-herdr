use super::*;
use crate::nirvana_run::{Event, RunState, V};
use crate::nirvana_seat::{Effort, Mode};
use crate::nirvana_servers::{Listener, Process};
use std::collections::BTreeMap;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "herdr-o5-gate-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn sh(cwd: &Path, args: &[&str]) -> String {
    git(cwd, args).unwrap_or_else(|why| panic!("{why}"))
}

/// A project with `calc.py` and `test_calc.py` on `main`, and `.nirvana/` as O3 writes it.
fn repo(name: &str) -> PathBuf {
    let root = temp(name);
    sh(&root, &["-c", "init.defaultBranch=main", "init", "-q"]);
    sh(&root, &["config", "user.name", "Gate Test"]);
    sh(&root, &["config", "user.email", "gate@test"]);
    sh(&root, &["config", "core.autocrlf", "false"]);
    std::fs::write(root.join("calc.py"), "def add(a, b):\n    return a + b\n").expect("calc");
    std::fs::write(root.join("test_calc.py"), "from calc import add\n").expect("test");
    std::fs::write(root.join("README.md"), "calc\n").expect("readme");
    sh(&root, &["add", "-A"]);
    sh(&root, &["commit", "-q", "-m", "base"]);
    Project::at(&root).ensure().expect(".nirvana");
    root
}

/// The order's worktree as O3's `seat run --worktree` makes it: `.nirvana/worktrees/<order>` on `nirvana/<order>`,
/// with the order's copy in its own `.nirvana/orders/`.
fn worktree(root: &Path, order: &str) -> PathBuf {
    let path = root.join(".nirvana").join("worktrees").join(order);
    sh(
        root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            &format!("nirvana/{order}"),
            &path.display().to_string(),
            "HEAD",
        ],
    );
    nirvana_run::ensure_dir(&path.join(".nirvana")).expect("dir");
    std::fs::create_dir_all(path.join(".nirvana").join("orders")).expect("orders");
    std::fs::write(
        path.join(".nirvana")
            .join("orders")
            .join(format!("{order}.md")),
        "the order\n",
    )
    .expect("copy");
    path
}

fn done() -> Value {
    serde_json::json!({"status":"done","summary":"added mul","files":["calc.py"],"tests":[{"name":"test_mul","result":"pass"}],"gotchas":[]})
}

fn no_ports() -> Result<Vec<u16>, String> {
    Ok(Vec::new())
}

fn spec_with(owns: &[&str], test: &str, checks: &[&str]) -> Spec {
    Spec {
        owns: owns.iter().map(|s| (*s).to_owned()).collect(),
        test: Some(test.to_owned()),
        checks: checks.iter().map(|s| (*s).to_owned()).collect(),
        serves: Vec::new(),
        profile: None,
        no_web: false,
    }
}

fn run_det(root: &Path, cwd: &Path, order: &str, spec: &Spec, report: Option<&Value>) -> Det {
    let branch = format!("nirvana/{order}");
    deterministic(
        &Subject {
            root,
            cwd,
            run: "r1",
            order,
            branch: Some(&branch),
            class: "build.bounded",
            report,
            spec,
            ports: &no_ports,
        },
        None,
    )
}

fn axis<'a>(det: &'a Det, name: &str) -> &'a Axis {
    det.axes
        .iter()
        .find(|axis| axis.axis == name)
        .unwrap_or_else(|| panic!("no {name} axis: {det:?}"))
}

/// The worker adds `mul` and its test file; `git ls-files` is the test command, so a check is "run" exactly when
/// its file is in the commit (no interpreter needed on the test machine).
fn work(cwd: &Path) {
    std::fs::write(
        cwd.join("calc.py"),
        "def add(a, b):\n    return a + b\n\ndef mul(a, b):\n    return a * b\n",
    )
    .expect("mul");
    std::fs::write(cwd.join("test_mul.py"), "from calc import mul\n").expect("test");
}

#[test]
fn a_clean_order_passes_every_axis_and_its_work_is_committed_on_its_branch() {
    let root = repo("clean");
    let cwd = worktree(&root, "mul");
    work(&cwd);
    let spec = spec_with(&["calc.py", "test_*.py"], "git ls-files", &["test_mul"]);
    let det = run_det(&root, &cwd, "mul", &spec, Some(&done()));
    assert!(failing(&det.axes).is_empty(), "{det:#?}");
    assert_eq!(axis(&det, "served").mark, Mark::Note);
    // The worker's work is one commit on its branch; herdr's own folder is not in it.
    let files = sh(&cwd, &["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(
        files.lines().collect::<Vec<_>>(),
        vec!["calc.py", "test_mul.py"]
    );
    assert_eq!(det.head, rev(&cwd, "HEAD"));
    assert_eq!(det.base, rev(&root, "refs/heads/nirvana-integrate/r1"));
}

#[test]
fn a_new_file_no_owns_pattern_names_stays_out_of_the_commit_and_is_said() {
    let root = repo("bytecode");
    let cwd = worktree(&root, "mul");
    work(&cwd);
    // The worker's own test run made bytecode in a project with no .gitignore (the live run of 2026-10-18).
    std::fs::create_dir_all(cwd.join("__pycache__")).expect("cache");
    std::fs::write(
        cwd.join("__pycache__").join("calc.cpython-313.pyc"),
        b"\x00",
    )
    .expect("pyc");
    let det = run_det(
        &root,
        &cwd,
        "mul",
        &spec_with(&["calc.py", "test_mul.py"], "git ls-files", &["test_mul"]),
        Some(&done()),
    );
    assert!(failing(&det.axes).is_empty(), "{det:#?}");
    let commit = axis(&det, "commit");
    assert_eq!(
        (commit.mark, commit.lines.clone()),
        (
            Mark::Note,
            vec!["__pycache__/calc.cpython-313.pyc".to_owned()]
        )
    );
    let files = sh(&cwd, &["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(
        files.lines().collect::<Vec<_>>(),
        vec!["calc.py", "test_mul.py"]
    );
    // With no `owns:` line every new file is the worker's; herdr's own folder never is.
    let other = worktree(&root, "any");
    std::fs::write(other.join("notes.md"), "n\n").expect("notes");
    assert!(commit_work(&other, "any", &[]).expect("commit").is_empty());
    let files = sh(&other, &["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(files.lines().collect::<Vec<_>>(), vec!["notes.md"]);
}

#[test]
fn the_branch_is_rebased_onto_the_integration_tip_before_the_tests() {
    let root = repo("rebase");
    let cwd = worktree(&root, "mul");
    work(&cwd);
    // Another order merged first: the integration branch moved past the worktree's start.
    ensure_integration(&root, "r1").expect("integration");
    let other = worktree(&root, "doc");
    std::fs::write(other.join("README.md"), "calc, with docs\n").expect("doc");
    commit_work(&other, "doc", &[]).expect("commit");
    let doc = rev(&other, "HEAD").expect("doc head");
    sh(
        &root,
        &["update-ref", "refs/heads/nirvana-integrate/r1", &doc],
    );
    let spec = spec_with(&["calc.py", "test_mul.py"], "git ls-files", &["test_mul"]);
    let det = run_det(&root, &cwd, "mul", &spec, Some(&done()));
    assert_eq!(axis(&det, "rebase").mark, Mark::Pass, "{det:#?}");
    assert_eq!(sh(&cwd, &["merge-base", "HEAD", &doc]).trim(), doc);
    // Only the order's own files count as changed: the other order's README is the base's now.
    assert_eq!(axis(&det, "owned").mark, Mark::Pass, "{det:#?}");
}

#[test]
fn a_conflict_with_the_integration_tip_is_a_failing_rebase_with_the_files_and_the_rebase_is_undone()
{
    let root = repo("conflict");
    let cwd = worktree(&root, "mul");
    work(&cwd);
    ensure_integration(&root, "r1").expect("integration");
    let other = worktree(&root, "sub");
    std::fs::write(
        other.join("calc.py"),
        "def add(a, b):\n    return a + b\n\ndef sub(a, b):\n    return a - b\n",
    )
    .expect("sub");
    commit_work(&other, "sub", &[]).expect("commit");
    sh(
        &root,
        &[
            "update-ref",
            "refs/heads/nirvana-integrate/r1",
            &rev(&other, "HEAD").expect("sub head"),
        ],
    );
    let det = run_det(
        &root,
        &cwd,
        "mul",
        &spec_with(&["*.py"], "git ls-files", &[]),
        Some(&done()),
    );
    let rebase = axis(&det, "rebase");
    assert_eq!(rebase.mark, Mark::Fail);
    assert_eq!(rebase.lines, vec!["calc.py".to_owned()]);
    assert!(!cwd.join(".git").is_dir());
    assert!(sh(
        &cwd,
        &["status", "--porcelain", "--", ".", ":(exclude).nirvana"]
    )
    .trim()
    .is_empty());
    let git_dir = PathBuf::from(sh(&cwd, &["rev-parse", "--git-dir"]).trim());
    assert!(!git_dir.join("rebase-merge").exists() && !git_dir.join("rebase-apply").exists());
    // Nothing after a failing rebase runs on a tree that is not the integrated one.
    assert_eq!(det.axes.len(), 1);
}

#[test]
fn the_named_check_absent_from_the_output_fails_the_tests_though_the_suite_passed() {
    let root = repo("absent");
    let cwd = worktree(&root, "mul");
    work(&cwd);
    let spec = spec_with(&["*.py"], "git ls-files", &["test_mul", "test_div"]);
    let det = run_det(&root, &cwd, "mul", &spec, Some(&done()));
    let tests = axis(&det, "tests");
    assert_eq!(tests.mark, Mark::Fail, "{det:#?}");
    assert!(tests.why.contains("test_div never ran"), "{}", tests.why);
    assert!(!tests.why.contains("test_mul"), "{}", tests.why);
    // The filtered output keeps the line that names the check that did run.
    assert!(
        tests.lines.iter().any(|line| line == "test_mul.py"),
        "{:?}",
        tests.lines
    );
}

#[test]
fn a_failing_test_command_fails_and_checks_without_a_command_fail_too() {
    let root = repo("exit");
    let cwd = worktree(&root, "mul");
    work(&cwd);
    let det = run_det(
        &root,
        &cwd,
        "mul",
        &spec_with(&["*.py"], "git rev-parse --verify no-such-ref", &[]),
        Some(&done()),
    );
    assert_eq!(axis(&det, "tests").mark, Mark::Fail, "{det:#?}");
    assert!(
        axis(&det, "tests").why.contains("exited 128")
            || axis(&det, "tests").why.contains("exited 1")
    );
    let none = tests_axis(None, None, &["test_mul".into()]);
    assert_eq!(none.mark, Mark::Fail);
    assert_eq!(tests_axis(None, None, &[]).mark, Mark::Note);
    let timed = CommandRun {
        exit: None,
        output: String::new(),
        timed_out: true,
        error: None,
    };
    let stopped = tests_axis(Some("x"), Some(&timed), &[]);
    assert_eq!(stopped.mark, Mark::Fail);
    assert!(stopped.why.contains("ran past"), "{}", stopped.why);
}

#[test]
fn a_missing_empty_or_blocked_report_fails_and_a_done_one_passes() {
    assert_eq!(report_axis(None).mark, Mark::Fail);
    let empty = report_axis(Some(&serde_json::json!({})));
    assert_eq!(
        (empty.mark, empty.why.as_str()),
        (Mark::Fail, "the Work Report is empty")
    );
    let mut blocked = done();
    blocked["status"] = "blocked".into();
    assert!(report_axis(Some(&blocked))
        .why
        .starts_with("the worker reported blocked"));
    let mut odd = done();
    odd["extra"] = 1.into();
    assert_eq!(report_axis(Some(&odd)).mark, Mark::Fail);
    assert_eq!(report_axis(Some(&done())).mark, Mark::Pass);
}

#[test]
fn only_the_files_the_order_owns_may_change() {
    let root = repo("owned");
    let cwd = worktree(&root, "mul");
    work(&cwd);
    std::fs::write(
        cwd.join("README.md"),
        "calc, edited by a worker that should not\n",
    )
    .expect("readme");
    let det = run_det(
        &root,
        &cwd,
        "mul",
        &spec_with(&["calc.py", "test_*.py"], "git ls-files", &[]),
        Some(&done()),
    );
    let owned = axis(&det, "owned");
    assert_eq!(owned.mark, Mark::Fail, "{det:#?}");
    assert_eq!(owned.lines, vec!["README.md".to_owned()]);
    // herdr's own folder in the worktree is never the worker's change.
    assert_eq!(
        owned_axis(
            "build.bounded",
            &[".nirvana/orders/mul.md".into(), "calc.py".into()],
            &["calc.py".into()]
        )
        .mark,
        Mark::Pass
    );
    // An order that names no owned file is not held to an empty scope; its files are listed (read-only classes).
    let none = owned_axis("research.web", &["calc.py".into()], &[]);
    assert_eq!(
        (none.mark, none.lines.clone()),
        (Mark::Note, vec!["calc.py".to_owned()])
    );
}

#[test]
fn owned_patterns_match_names_and_folders() {
    assert!(glob_match("calc.py", "calc.py"));
    assert!(!glob_match("calc.py", "src/calc.py"));
    assert!(glob_match("src/*.rs", "src/main.rs"));
    assert!(!glob_match("src/*.rs", "src/a/main.rs"));
    assert!(glob_match("src/**/*.rs", "src/main.rs"));
    assert!(glob_match("src/**/*.rs", "src/a/b/main.rs"));
    assert!(glob_match("src/**", "src/a/b"));
    assert!(glob_match("test_?.py", "test_a.py"));
    assert!(!glob_match("test_?.py", "test_ab.py"));
    assert!(glob_match("./docs/*", "docs/x.md"));
}

#[test]
fn the_order_file_names_its_gate_lines_at_the_start_of_a_line() {
    let text = "# Add mul\n\nowns: calc.py, test_mul.py\nowns: `docs/**`\ntest: python -m pytest -q -rA\ncheck: test_mul test_add\n\
serves: 8770\n  owns: indented.txt\nprose that says owns: nothing\n";
    let parsed = spec(text);
    assert_eq!(parsed.owns, vec!["calc.py", "test_mul.py", "docs/**"]);
    assert_eq!(parsed.test.as_deref(), Some("python -m pytest -q -rA"));
    assert_eq!(parsed.checks, vec!["test_mul", "test_add"]);
    assert_eq!(parsed.serves, vec![8770]);
    // herdr's footer and a send-back brief name no gate line of their own.
    let footer = spec(crate::nirvana_worker::ORDER_FOOTER);
    assert_eq!(footer, Spec::default());
}

#[test]
fn an_ops_order_must_serve_its_port_from_a_process_in_its_own_pane() {
    let processes = vec![
        Process {
            pid: 10,
            parent_pid: 1,
            name: "pwsh.exe".into(),
        },
        Process {
            pid: 11,
            parent_pid: 10,
            name: "herdr.exe".into(),
        },
        Process {
            pid: 12,
            parent_pid: 11,
            name: "python.exe".into(),
        },
        Process {
            pid: 20,
            parent_pid: 1,
            name: "node.exe".into(),
        },
    ];
    let listeners = vec![
        Listener {
            pid: 12,
            port: 8770,
        },
        Listener {
            pid: 20,
            port: 8771,
        },
    ];
    let ports = pane_ports(10, &processes, &listeners);
    assert_eq!(ports, vec![8770]);
    assert_eq!(
        served_axis("ops.terminal", &[8770], Ok(ports.clone())).mark,
        Mark::Pass
    );
    // A listener a parallel session started is not the worker's (old Nirvana's incident of 2026-10-08).
    let other = served_axis("ops.terminal", &[8771], Ok(ports));
    assert_eq!(other.mark, Mark::Fail);
    assert!(
        other.why.contains("8771") && other.why.contains("listens on 8770"),
        "{}",
        other.why
    );
    assert_eq!(
        served_axis("ops.terminal", &[], Ok(Vec::new())).mark,
        Mark::Fail
    );
    assert_eq!(
        served_axis("ops.terminal", &[1], Err("no pane".into())).mark,
        Mark::Fail
    );
    assert_eq!(
        served_axis("build.bounded", &[8770], Ok(Vec::new())).mark,
        Mark::Note
    );
}

fn seated(order: &str, parent: Option<&str>) -> Seated {
    Seated {
        order: order.into(),
        seat: "sonnet".into(),
        cli: "claude".into(),
        model: "claude-sonnet-5-5".into(),
        effort: Effort::Medium,
        pool: "max".into(),
        class: "build.bounded".into(),
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
        parent: parent.map(str::to_owned),
        why: parent.map(|_| "send_back".to_owned()),
        rule_pick: None,
        bar_dropped: None,
        step_down: None,
        use_basis: None,
        expected: BTreeMap::new(),
        jev_pick: None,
        send_ok: None,
    }
}

fn chain_state(orders: &[(&str, Option<&str>)]) -> RunState {
    let events: Vec<Event> = orders
        .iter()
        .enumerate()
        .map(|(index, (order, parent))| Event {
            v: V,
            seq: index as u64 + 1,
            at: 100 + index as u64,
            kind: Kind::Seated(Box::new(seated(order, *parent))),
        })
        .collect();
    replay(&events)
}

#[test]
fn send_backs_stop_at_three_per_order_then_the_owner_is_asked() {
    let state = chain_state(&[("a", None)]);
    assert_eq!(
        plan_send_back(&state, "a"),
        SendBack::Child {
            id: "a-s1".into(),
            n: 1
        }
    );
    let state = chain_state(&[("a", None), ("a-s1", Some("a")), ("a-s2", Some("a-s1"))]);
    assert_eq!(chain(&state, "a-s2"), ("a".into(), 2));
    assert_eq!(
        plan_send_back(&state, "a-s2"),
        SendBack::Child {
            id: "a-s3".into(),
            n: 3
        }
    );
    let state = chain_state(&[
        ("a", None),
        ("a-s1", Some("a")),
        ("a-s2", Some("a-s1")),
        ("a-s3", Some("a-s2")),
    ]);
    assert_eq!(chain(&state, "a-s3"), ("a".into(), 3));
    assert_eq!(plan_send_back(&state, "a-s3"), SendBack::Ask);
    // O3's replay counts each send-back on its parent, as the receipt shows it.
    assert_eq!(state.order("a").map(|a| a.send_backs), Some(1));
}

fn world_with(clis: &[(&str, &str)]) -> World {
    World {
        now: 1_792_289_318,
        clis: clis
            .iter()
            .map(|(cli, why)| ((*cli).to_owned(), (*why).to_owned()))
            .collect(),
        ..World::default()
    }
}

#[test]
fn the_reviewer_is_of_another_family_and_only_falls_back_to_the_builders_own_marked() {
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    let world = world_with(&[]);
    let (pick, same) = pick_other_family(
        &table,
        &world,
        "review.adversary",
        "anthropic",
        Mode::Balanced,
        "a-rev1",
        "SAME-FAMILY REVIEW",
    )
    .expect("reviewer");
    assert_eq!(
        (pick.family.as_str(), same),
        ("google", false),
        "{}",
        pick.reason
    );
    let (pick, same) = pick_other_family(
        &table,
        &world,
        "review.adversary",
        "google",
        Mode::Balanced,
        "a-rev1",
        "SAME-FAMILY REVIEW",
    )
    .expect("reviewer");
    assert_eq!(
        (pick.family.as_str(), same),
        ("anthropic", false),
        "{}",
        pick.reason
    );
    // agy signed out: no Google seat, so decision 0017's fallback (O2's `pick`), the gate's mark at the start of
    // the reason and the Pick's own words after it.
    let world = world_with(&[("agy", "agy not signed in")]);
    let (pick, same) = pick_other_family(
        &table,
        &world,
        "review.adversary",
        "anthropic",
        Mode::Balanced,
        "a-rev1",
        "SAME-FAMILY REVIEW",
    )
    .expect("fallback");
    assert_eq!((pick.family.as_str(), same), ("anthropic", true));
    assert!(
        pick.same_family_fallback
            && pick.reason.starts_with("SAME-FAMILY REVIEW: ")
            && pick.reason.contains(
                "; same family (anthropic), in a fresh session: no other family had a seat: "
            )
            && pick.reason.ends_with("(decision 0017)"),
        "{}",
        pick.reason
    );
    // The held-out author follows the same rule from the builder's family (decision 0019).
    let (author, _) = pick_other_family(
        &table,
        &world_with(&[]),
        "acceptance.author",
        "anthropic",
        Mode::Balanced,
        "a-acc1",
        "SAME-FAMILY AUTHOR",
    )
    .expect("author");
    assert_eq!(author.family, "google");
}

#[test]
fn a_review_verdict_comes_back_as_a_work_report() {
    let mut report = done();
    assert_eq!(review_axis("a-rev1", &report).mark, Mark::Pass);
    report["status"] = "blocked".into();
    report["gotchas"] = serde_json::json!(["calc.py: mul ignores negative numbers"]);
    let back = review_axis("a-rev1", &report);
    assert_eq!(
        (back.mark, back.lines.clone()),
        (
            Mark::Fail,
            vec!["calc.py: mul ignores negative numbers".to_owned()]
        )
    );
    report["status"] = "needs_answer".into();
    report["question"] = "is a negative product wanted?".into();
    assert_eq!(review_axis("a-rev1", &report).mark, Mark::Wait);
}

const PYTEST: &str = "test_acceptance.py::test_mul_small FAILED                       [ 33%]
=========================== short test summary info ============================
PASSED test_acceptance.py::test_add
FAILED test_acceptance.py::test_mul_small - AssertionError: assert None == 6
FAILED test_acceptance.py::test_mul_negative - AssertionError: assert None == -6
========================= 2 failed, 1 passed in 0.05s ==========================";

#[test]
fn held_out_cases_parse_from_pytest_cargo_and_unittest() {
    assert_eq!(
        cases(PYTEST),
        vec![
            ("test_acceptance.py::test_mul_small".to_owned(), false),
            ("test_acceptance.py::test_add".to_owned(), true),
            ("test_acceptance.py::test_mul_negative".to_owned(), false),
        ]
    );
    assert_eq!(
        cases("running 2 tests\ntest calc::adds ... ok\ntest calc::muls ... FAILED\ntest result: FAILED."),
        vec![("calc::adds".to_owned(), true), ("calc::muls".to_owned(), false)]
    );
    assert_eq!(
        cases(
            "test_mul (check.TestMul.test_mul) ... FAIL\ntest_add (check.TestMul.test_add) ... ok\n\
             ERROR: test_mul (check.TestMul.test_mul)\n----\nRan 2 tests in 0.001s\n\nFAILED (errors=1)"
        ),
        vec![
            ("check.TestMul.test_mul".to_owned(), false),
            ("check.TestMul.test_add".to_owned(), true)
        ]
    );
}

#[test]
fn a_held_out_check_is_valid_only_when_a_case_fails_at_the_base() {
    assert!(heldout_valid(&[]).expect_err("none").contains("no case"));
    assert!(heldout_valid(&[("a".into(), true)])
        .expect_err("passes")
        .contains("tests nothing"));
    assert!(heldout_valid(&[("a".into(), true), ("b".into(), false)]).is_ok());
}

#[test]
fn the_builder_sees_only_the_failing_held_out_case_ids() {
    let run = CommandRun {
        exit: Some(1),
        output: PYTEST.into(),
        timed_out: false,
        error: None,
    };
    let held = heldout_axis(&run);
    assert_eq!(held.mark, Mark::Fail);
    assert_eq!(
        held.lines,
        vec![
            "test_acceptance.py::test_mul_small".to_owned(),
            "test_acceptance.py::test_mul_negative".to_owned()
        ]
    );
    let axes = vec![Axis::new("tests", Mark::Pass, "ok"), held];
    let brief = send_back_brief(
        "mul",
        "mul",
        1,
        &axes,
        "# Add mul\n\nowns: calc.py\n",
        "nirvana-integrate/r1",
    );
    assert!(
        brief.contains("- `test_acceptance.py::test_mul_small`\n"),
        "{brief}"
    );
    // Nothing of the check's own output reaches the builder: no assertion, no expected value, no summary.
    for leak in ["AssertionError", "== 6", "-6", "passed in", "test_add"] {
        assert!(!brief.contains(leak), "{leak} leaked: {brief}");
    }
    assert!(brief.ends_with("# Add mul\n\nowns: calc.py\n"), "{brief}");
    assert!(brief.starts_with("# Send-back 1 of 3: mul\n"));
    let all_pass = CommandRun {
        exit: Some(0),
        output: "PASSED t::a\nPASSED t::b".into(),
        timed_out: false,
        error: None,
    };
    assert_eq!(heldout_axis(&all_pass).mark, Mark::Pass);
}

#[test]
fn feature_size_orders_get_a_held_out_check_and_load_bearing_ones_a_reviewer() {
    assert!(feature_size("build.bounded", SizeName::Medium));
    assert!(feature_size("fix.debug", SizeName::Large));
    assert!(!feature_size("build.bounded", SizeName::Small));
    assert!(!feature_size("write.docs", SizeName::Large));
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    assert!(load_bearing(Some(&table), "build.load_bearing"));
    assert!(load_bearing(Some(&table), "plan"));
    assert!(!load_bearing(Some(&table), "build.bounded"));
    assert!(load_bearing(None, "build.load_bearing"));
}

#[test]
fn the_outcome_words_and_what_the_tally_counts() {
    assert_eq!(
        Outcome::ALL.map(Outcome::word),
        ["pass", "sent_back", "failed", "asked", "reseated"]
    );
    assert_eq!(Outcome::tally("pass"), Some(true));
    for word in ["sent_back", "failed", "asked", "sent back", "crash_loop"] {
        assert_eq!(Outcome::tally(word), Some(false), "{word}");
    }
    assert_eq!(Outcome::tally("reseated"), None);
    // Plan 37b review S1: an order asked on its own block is the record's `blocked`, and not counted.
    assert_eq!(Outcome::tally(Outcome::BLOCKED), None);
    // Plan 40a's review round: one order and run is decided by its last line, so each line is its own order.
    let on = |chain: &str, outcome: &str| -> nirvana_seat::RecordLine {
        serde_json::from_value(serde_json::json!({"order":format!("o-{chain}-{outcome}"),"seat":"sonnet","effort":"medium","class":"build.bounded",
            "chain":chain,"outcome":outcome}))
        .expect("line")
    };
    let line = |outcome: &str| on("build.bounded", outcome);
    // O4b's SubagentStop lines (chain `subagent`, outcome `subagent-<status>`) are the orchestrator's judgement:
    // counted nowhere, neither on the class's chain nor as a row of their own.
    let tallies = nirvana_seat::tally(&[
        line("pass"),
        line("sent_back"),
        line("reseated"),
        line("asked"),
        line("blocked"),
        on("subagent", "subagent-done"),
        on("subagent", "subagent-blocked"),
    ]);
    assert_eq!(tallies.len(), 1, "{tallies:?}");
    let tally = tallies[&(
        "sonnet".to_owned(),
        Effort::Medium,
        "build.bounded".to_owned(),
    )];
    // `asked` is no longer a failure in the seat record (`nirvana_seat::tally_word`): pass and sent_back count.
    assert_eq!((tally.passes, tally.orders), (1, 2));
}

#[test]
fn a_merge_moves_only_the_integration_branch_and_refuses_one_that_moved() {
    let root = repo("merge");
    let cwd = worktree(&root, "mul");
    work(&cwd);
    let main_before = rev(&root, "HEAD").expect("main");
    let det = run_det(
        &root,
        &cwd,
        "mul",
        &spec_with(&["*.py"], "git ls-files", &["test_mul"]),
        Some(&done()),
    );
    assert!(failing(&det.axes).is_empty(), "{det:#?}");
    let (base, head) = (
        det.base.clone().expect("base"),
        det.head.clone().expect("head"),
    );
    let sha = merge(&root, "r1", "mul", &base, &head).expect("merge");
    assert_eq!(
        rev(&root, "refs/heads/nirvana-integrate/r1").as_deref(),
        Some(sha.as_str())
    );
    assert_eq!(
        sh(
            &root,
            &["rev-parse", &format!("{sha}^1"), &format!("{sha}^2")]
        ),
        format!("{base}\n{head}\n")
    );
    assert_eq!(
        sh(&root, &["rev-parse", &format!("{sha}^{{tree}}")]),
        sh(&root, &["rev-parse", &format!("{head}^{{tree}}")])
    );
    assert!(sh(&root, &["log", "-1", "--format=%B", &sha]).contains("Nirvana-Order: mul"));
    // The project's own checkout is never touched.
    assert_eq!(rev(&root, "HEAD"), Some(main_before));
    assert!(sh(
        &root,
        &["status", "--porcelain", "--", ".", ":(exclude).nirvana"]
    )
    .trim()
    .is_empty());
    let again = merge(&root, "r1", "mul", &base, &head).expect_err("moved");
    assert!(again.contains("moved since the gate"), "{again}");
}

#[test]
fn a_command_is_split_into_words_without_a_shell() {
    assert_eq!(
        split_command("python -m pytest -q"),
        vec!["python", "-m", "pytest", "-q"]
    );
    assert_eq!(
        split_command("cargo test \"a b\" 'c d' ''"),
        vec!["cargo", "test", "a b", "c d", ""]
    );
    assert!(split_command("   ").is_empty());
    let run = run_command(
        Path::new("."),
        "no-such-program-o5 --x",
        Duration::from_secs(5),
    );
    assert!(run.error.is_some() || run.exit != Some(0));
}

#[test]
fn the_reason_line_names_the_first_failing_axis_and_no_order() {
    let axes = vec![
        Axis::new("rebase", Mark::Pass, "onto x"),
        Axis::new("tests", Mark::Fail, "the named check(s) test_div never ran"),
        Axis::new(
            "owned",
            Mark::Fail,
            "1 file(s) changed that the order does not own",
        ),
    ];
    assert_eq!(
        reason_line(&axes).as_deref(),
        Some("tests: the named check(s) test_div never ran")
    );
    assert_eq!(reason_line(&axes[..1]), None);
}

#[test]
fn the_record_line_says_one_of_the_five_words() {
    let at = |seq: u64, kind: Kind| Event {
        v: V,
        seq,
        at: 100 + seq,
        kind,
    };
    let ended = |order: &str, outcome: &str| Kind::Ended {
        order: Some(order.into()),
        outcome: outcome.into(),
        why: Some("the worker could not start".into()),
        exit: None,
        pools: Vec::new(),
        pools_basis: None,
    };
    let gated = |order: &str, verdict: &str| Kind::Gated {
        order: order.into(),
        verdict: verdict.into(),
        reason: None,
        axes: Vec::new(),
    };
    let events = vec![
        at(1, Kind::Seated(Box::new(seated("a", None)))),
        at(2, ended("a", "failed")),
        at(3, Kind::Seated(Box::new(seated("b", None)))),
        at(4, gated("b", "sent_back")),
        at(5, Kind::Seated(Box::new(seated("c", None)))),
        at(6, gated("c", "looks fine to me")),
        at(7, Kind::Seated(Box::new(seated("d", None)))),
        at(
            8,
            Kind::Asked {
                order: Some("d".into()),
                question: "which?".into(),
            },
        ),
        at(9, Kind::Seated(Box::new(seated("e", None)))),
        at(10, ended("e", "stopped")),
    ];
    let state = replay(&events);
    let word = |order: &str| {
        nirvana_run::record_line("r1", state.order(order).expect(order), None).outcome
    };
    assert_eq!(
        ["a", "b", "c", "d", "e"].map(word),
        ["failed", "sent_back", "failed", "asked", "failed"].map(str::to_owned)
    );
}

#[test]
fn a_gated_event_keeps_every_axis_through_the_journal() {
    let root = temp("journal");
    let journal = Project::at(&root).journal("r1");
    let axes = vec![
        Axis::new("tests", Mark::Fail, "the named check(s) test_div never ran")
            .with(vec!["x".into()]),
    ];
    journal
        .append(
            5,
            Kind::Gated {
                order: "a".into(),
                verdict: "sent_back".into(),
                reason: reason_line(&axes),
                axes: axes_values(&axes),
            },
        )
        .expect("append");
    let (events, bad) = journal.read();
    assert_eq!(bad, 0);
    let Kind::Gated { axes: read, .. } = &events[0].kind else {
        panic!("gated")
    };
    let read: Vec<Axis> = read
        .iter()
        .map(|v| serde_json::from_value(v.clone()).expect("axis"))
        .collect();
    assert_eq!(read, axes);
}

// ---- Plan 24 §6: owned axis and requires_owns tests ----

#[test]
fn owned_axis_empty_owns_fails_for_build_fix_plan_and_notes_for_read_only() {
    let changed = vec!["src/main.rs".to_string()];
    let empty_owns: Vec<String> = vec![];

    // build.* fails
    let axis = owned_axis("build.bounded", &changed, &empty_owns);
    assert_eq!(axis.mark, Mark::Fail);
    assert!(axis
        .why
        .contains("unbounded: the order names no owned file"));

    let axis = owned_axis("build.load_bearing", &changed, &empty_owns);
    assert_eq!(axis.mark, Mark::Fail);

    // fix.* fails
    let axis = owned_axis("fix.debug", &changed, &empty_owns);
    assert_eq!(axis.mark, Mark::Fail);

    // plan fails
    let axis = owned_axis("plan", &changed, &empty_owns);
    assert_eq!(axis.mark, Mark::Fail);

    // read-only classes keep Note (Rule 7: never refuse a read-only class)
    let axis = owned_axis("research.web", &changed, &empty_owns);
    assert_eq!(axis.mark, Mark::Note);

    let axis = owned_axis("research.read", &changed, &empty_owns);
    assert_eq!(axis.mark, Mark::Note);

    let axis = owned_axis("chore.script", &changed, &empty_owns);
    assert_eq!(axis.mark, Mark::Note);

    // requires_owns helper
    assert!(requires_owns("build.bounded"));
    assert!(requires_owns("fix.debug"));
    assert!(requires_owns("plan"));
    assert!(!requires_owns("research.web"));
    assert!(!requires_owns("research.read"));
}

// ---- Plan 24 §5: sources axis tests ----

/// The sources axis of an order that asks for the web, as every research order did before live checks F2.
fn web_sources_axis(class: &str, report: Option<&Value>, stream: &str) -> Option<Axis> {
    sources_axis(class, Some("the test asks for the web"), report, stream)
}

#[test]
fn sources_axis_checks_citations_in_worker_stream() {
    // 1. Build class has no sources axis
    assert_eq!(web_sources_axis("build.bounded", None, ""), None);

    // 2. Empty sources array or missing report fails for research / media
    let empty_report = serde_json::json!({"sources": []});
    let axis = web_sources_axis("research.web", Some(&empty_report), "").expect("axis");
    assert_eq!(axis.mark, Mark::Fail);
    assert!(axis.why.contains("no sources"));

    let no_report = web_sources_axis("media.video", None, "").expect("axis");
    assert_eq!(no_report.mark, Mark::Fail);

    // 3. Cited URL never fetched fails
    let report_with_url = serde_json::json!({
        "sources": [
            {"url": "https://example.com/docs#intro", "used_for": "reading spec"}
        ]
    });
    let stream_without = "I researched the web using other means.";
    let axis =
        web_sources_axis("research.web", Some(&report_with_url), stream_without).expect("axis");
    assert_eq!(axis.mark, Mark::Fail);
    assert!(axis.why.contains("not fetched"));

    // 4. Fetched URL passes (with fragment and trailing slash trimmed) via JSON tool use
    let stream_with = serde_json::json!({
        "type": "assistant",
        "message": {
            "content": [{
                "type": "tool_use",
                "name": "WebFetch",
                "input": {"url": "https://example.com/docs/"}
            }]
        }
    })
    .to_string();
    let axis =
        web_sources_axis("research.web", Some(&report_with_url), &stream_with).expect("axis");
    assert_eq!(axis.mark, Mark::Pass);
}

#[test]
fn sources_axis_b2_real_fetch_calls_and_url_normalization() {
    let target_url = "https://example.com/a";
    let report = serde_json::json!({
        "sources": [
            {"url": target_url, "used_for": "spec verification"}
        ]
    });

    // 1. Claude stream whose assistant text cites the URL, with cache_read_input_tokens in usage, and no WebFetch: fails!
    let claude_assistant_text_stream = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-cite",
            "content": [
                {
                    "type": "text",
                    "text": format!("I read the docs at {target_url} and found the answer.")
                }
            ],
            "usage": {
                "input_tokens": 100,
                "cache_read_input_tokens": 500,
                "output_tokens": 50
            }
        }
    })
    .to_string();

    let axis1 = web_sources_axis("research.web", Some(&report), &claude_assistant_text_stream)
        .expect("axis");
    assert_eq!(axis1.mark, Mark::Fail);

    // 2. The same stream with a WebFetch tool_use of that URL: passes!
    let claude_web_fetch_stream = format!(
        "{}\n{}",
        claude_assistant_text_stream,
        serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "msg-fetch",
                "content": [
                    {
                        "type": "tool_use",
                        "name": "WebFetch",
                        "input": {
                            "url": "http://www.example.com/a/"
                        }
                    }
                ]
            }
        })
    );
    let axis2 =
        web_sources_axis("research.web", Some(&report), &claude_web_fetch_stream).expect("axis");
    assert_eq!(axis2.mark, Mark::Pass);

    // 3. An agy stream with read_url_content: passes!
    let agy_stream = serde_json::json!({
        "event": "step_update",
        "step_update": {
            "step_type": "tool",
            "tool_name": "read_url_content",
            "tool_info": {
                "parameters": {
                    "url": target_url
                }
            }
        }
    })
    .to_string();
    let axis3 = web_sources_axis("research.web", Some(&report), &agy_stream).expect("axis");
    assert_eq!(axis3.mark, Mark::Pass);

    // 4. http://www.x.com/a/ matches https://x.com/a
    assert_eq!(
        normalize_url("http://www.x.com/a/"),
        normalize_url("https://x.com/a")
    );
}

#[test]
fn sources_axis_r2_honest_browsing_and_shell_fetch() {
    let target_url = "https://example.com/target";
    let report = serde_json::json!({
        "sources": [
            {"url": target_url, "used_for": "spec research"}
        ]
    });

    // 1. an agy open_browser_url stream passes
    let agy_open_browser_stream = serde_json::json!({
        "event": "step_update",
        "step_update": {
            "step_type": "tool",
            "tool_name": "open_browser_url",
            "tool_info": {
                "parameters": {
                    "url": target_url
                }
            }
        }
    })
    .to_string();
    let axis =
        web_sources_axis("research.web", Some(&report), &agy_open_browser_stream).expect("axis");
    assert_eq!(axis.mark, Mark::Pass);

    // 2. a Claude Bash tool_use running `agent-browser open <url>` passes
    let claude_agent_browser_stream = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-bash-ab",
            "content": [
                {
                    "type": "tool_use",
                    "name": "Bash",
                    "input": {
                        "command": format!("agent-browser open \"{target_url}\" --wait")
                    }
                }
            ]
        }
    })
    .to_string();
    let axis = web_sources_axis("research.web", Some(&report), &claude_agent_browser_stream)
        .expect("axis");
    assert_eq!(axis.mark, Mark::Pass);

    // 3. a Claude Bash tool_use running `echo <url>` fails
    let claude_echo_stream = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-bash-echo",
            "content": [
                {
                    "type": "tool_use",
                    "name": "Bash",
                    "input": {
                        "command": format!("echo {target_url}")
                    }
                }
            ]
        }
    })
    .to_string();
    let axis = web_sources_axis("research.web", Some(&report), &claude_echo_stream).expect("axis");
    assert_eq!(axis.mark, Mark::Fail);

    // 4. a browser_subagent whose task names the URL passes
    let browser_subagent_stream = serde_json::json!({
        "event": "step_update",
        "step_update": {
            "step_type": "tool",
            "tool_name": "browser_subagent",
            "tool_info": {
                "parameters": {
                    "task": format!("Visit {target_url} and extract the documentation details.")
                }
            }
        }
    })
    .to_string();
    let axis =
        web_sources_axis("research.web", Some(&report), &browser_subagent_stream).expect("axis");
    assert_eq!(axis.mark, Mark::Pass);

    // 5. the earlier cache_read_input_tokens case still fails
    let claude_cache_tokens_stream = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-cite-only",
            "content": [
                {
                    "type": "text",
                    "text": format!("I saw {target_url} in my knowledge base.")
                }
            ],
            "usage": {
                "input_tokens": 100,
                "cache_read_input_tokens": 500,
                "output_tokens": 50
            }
        }
    })
    .to_string();
    let axis =
        web_sources_axis("research.web", Some(&report), &claude_cache_tokens_stream).expect("axis");
    assert_eq!(axis.mark, Mark::Fail);
}

// ---- Plan 24 §4: send-back override tests ----

#[test]
fn send_back_inherits_override_and_does_not_count_against_run_overrides() {
    let _guard = crate::nirvana::with_test_home("send-back-override");
    let root = repo("send-back-override");
    let project = Project::at(&root);
    let run = "r1";
    let journal = project.journal(run);

    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    // Plan 31 §2.5: the stand-in world, so the pick never reads this machine's live seats. With the live world the
    // rule picked sonnet without the override too, so its sabotage control could not go red (p29c record).
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let mut rule = nirvana_seat::Request::new("build.bounded");
    rule.size = SizeName::Small;
    let rule_pick = nirvana_seat::pick(&table, &nirvana_seat::live_world(&table), &rule)
        .expect("rule pick")
        .seat;
    // The parent sat on a seat the rule would not give it, by the owner's override.
    let overridden = if rule_pick == "opus" { "haiku" } else { "opus" };
    let mut seated_order = seated("build-1", None);
    seated_order.seat = overridden.into();
    seated_order.model = table
        .seat(overridden)
        .expect("seat")
        .model_for(Effort::Medium);
    seated_order.override_reason = Some(format!("owner insisted on {overridden}"));
    seated_order.cwd = worktree(&root, "build-1").display().to_string();

    // Initialize journal with Opened and Seated
    journal
        .append(
            1,
            Kind::Opened {
                project: "p".into(),
                orchestrator_pane: None,
                auto: false,
                pools: vec![],
                pools_basis: None,
            },
        )
        .expect("append");
    journal
        .append(2, Kind::Seated(Box::new(seated_order.clone())))
        .expect("append");

    // Pre-load 3 overrides used in run r1 to verify inherited override is exempt from the cap
    let mut init_leases = nirvana_seat::Leases::default();
    let now_ts = crate::nirvana_run::now();
    init_leases.runs.insert(
        run.to_string(),
        nirvana_seat::RunCount {
            overrides: 3,
            at: now_ts,
        },
    );
    nirvana_seat::write_leases(
        &nirvana_seat::dir().join(nirvana_seat::LEASES_FILE),
        &init_leases,
    )
    .expect("write leases");

    // Delivered order
    let mut state = replay(&journal.read().0);
    let known = state.order("build-1").expect("order").clone();

    let failing_axes = vec![Axis::new("tests", Mark::Fail, "test failed")];

    let ruling = send_back(
        &project,
        run,
        &journal,
        &state,
        &known,
        "build-1",
        "the order\n",
        failing_axes,
        Some(&table),
    )
    .expect("send_back");

    assert_eq!(ruling.verdict, "sent_back");
    let child_order = "build-1-s1";

    // Read replay state after send-back
    state = replay(&journal.read().0);
    let child = state.order(child_order).expect("child");
    assert_ne!(rule_pick, overridden, "the rule must pick another seat");
    assert_eq!(child.seated.seat, overridden);
    assert_eq!(child.seated.effort, Effort::Medium);
    assert_eq!(
        child.seated.override_reason,
        Some(format!("owner insisted on {overridden}"))
    );

    // Run overrides in leases is still 3 because child was an inherited override and did not increment!
    let leases =
        crate::nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE))
            .expect("leases");
    assert_eq!(leases.overrides(run), 3);
    nirvana_seat::set_test_world(None);
}

#[test]
fn send_back_retries_with_rule_pick_when_inherited_model_is_refused() {
    let _guard = crate::nirvana::with_test_home("send-back-refused-fallback");
    let root = repo("send-back-refused-fallback");
    let project = Project::at(&root);
    let run = "r-fallback";
    let journal = project.journal(run);

    // Turn sonnet off in table
    let mut table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    if let Some(seat) = table.seats.iter_mut().find(|s| s.id == "sonnet") {
        seat.enabled = false;
    }

    let mut seated_order = seated("build-fb", None);
    seated_order.seat = "sonnet".into();
    seated_order.override_reason = Some("owner insisted on sonnet".into());
    seated_order.cwd = worktree(&root, "build-fb").display().to_string();

    journal
        .append(
            1,
            Kind::Opened {
                project: "p".into(),
                orchestrator_pane: None,
                auto: false,
                pools: vec![],
                pools_basis: None,
            },
        )
        .expect("append");
    journal
        .append(2, Kind::Seated(Box::new(seated_order.clone())))
        .expect("append");

    let mut state = replay(&journal.read().0);
    let known = state.order("build-fb").expect("order").clone();

    let failing_axes = vec![Axis::new("tests", Mark::Fail, "test failed")];

    // Ruling should be sent_back (fallback to rule's pick), NOT asked!
    let ruling = send_back(
        &project,
        run,
        &journal,
        &state,
        &known,
        "build-fb",
        "the order\n",
        failing_axes,
        Some(&table),
    )
    .expect("send_back");

    assert_eq!(ruling.verdict, "sent_back");
    let child_order = "build-fb-s1";

    state = replay(&journal.read().0);
    let child = state.order(child_order).expect("child");
    // Child picked by normal rule, not sonnet
    assert_ne!(child.seated.seat, "sonnet");
    assert_eq!(child.seated.override_reason, None);
    assert!(child
        .seated
        .reason
        .contains("inherited override sonnet refused"));
}

#[test]
fn deterministic_includes_sources_axis_for_research_and_media() {
    let root = repo("det-sources");
    let spec = Spec::default();
    let report = serde_json::json!({
        "sources": [{"url": "https://example.com/test", "used_for": "spec"}]
    });
    let stream = serde_json::json!({
        "type": "assistant",
        "message": {
            "content": [{
                "type": "tool_use",
                "name": "WebFetch",
                "input": {"url": "https://example.com/test"}
            }]
        }
    })
    .to_string();
    let stream_file = root.join(".nirvana/runs/r1/worker-res.stream.jsonl");
    std::fs::create_dir_all(stream_file.parent().unwrap()).unwrap();
    std::fs::write(&stream_file, &stream).unwrap();

    let det = deterministic(
        &Subject {
            root: &root,
            cwd: &root,
            run: "r1",
            order: "res",
            branch: None,
            class: "research.web",
            report: Some(&report),
            spec: &spec,
            ports: &no_ports,
        },
        None,
    );
    assert!(det.axes.iter().any(|a| a.axis == "sources"));
}

fn claude_bash(command: &str, description: &str) -> String {
    serde_json::json!({
        "type": "assistant",
        "message": {"id": "msg-r4", "content": [{
            "type": "tool_use",
            "name": "Bash",
            "input": {"command": command, "description": description}
        }]}
    })
    .to_string()
}

fn agy_run(command: &str) -> String {
    serde_json::json!({
        "event": "step_update",
        "step_update": {
            "step_type": "tool",
            "tool_name": "run_command",
            "tool_info": {"parameters": {"CommandLine": command, "Cwd": "https://example.com/target"}}
        }
    })
    .to_string()
}

#[test]
fn shell_fetch_counts_only_the_segment_that_fetches_and_only_the_command() {
    let url = "https://example.com/target";
    let report = serde_json::json!({"sources": [{"url": url, "used_for": "spec"}]});
    let mark = |stream: String| {
        web_sources_axis("research.web", Some(&report), &stream)
            .unwrap()
            .mark
    };

    // A fetch in one segment does not vouch for a URL another segment only prints.
    assert_eq!(
        mark(claude_bash(
            &format!("curl https://other.org/a; echo {url}"),
            ""
        )),
        Mark::Fail
    );
    assert_eq!(
        mark(claude_bash(
            &format!("curl https://other.org/a && echo {url}"),
            ""
        )),
        Mark::Fail
    );
    // A fetch word inside a URL is not a fetch.
    assert_eq!(
        mark(claude_bash(&format!("echo https://curl.se/ {url}"), "")),
        Mark::Fail
    );
    // The description is not the command.
    assert_eq!(mark(claude_bash("ls", &format!("curl {url}"))), Mark::Fail);
    // agy: only CommandLine is the command.
    assert_eq!(mark(agy_run("dir")), Mark::Fail);

    // Honest fetches still pass, with shell punctuation after the URL.
    assert_eq!(
        mark(claude_bash(&format!("curl -s {url}; echo done"), "")),
        Mark::Pass
    );
    assert_eq!(mark(claude_bash(&format!("(curl {url})"), "")), Mark::Pass);
    assert_eq!(
        mark(claude_bash(
            &format!("echo start && agent-browser open {url}, then"),
            ""
        )),
        Mark::Pass
    );
    assert_eq!(mark(agy_run(&format!("curl.exe -L {url}"))), Mark::Pass);
}

#[test]
fn url_tail_keeps_a_paren_the_url_opened() {
    assert_eq!(
        crate::nirvana_gate::extract_url_tokens("see (https://en.wikipedia.org/wiki/Foo_(bar))."),
        vec!["https://en.wikipedia.org/wiki/Foo_(bar)".to_owned()]
    );
}

// ---- Plan 29 review round: a server order's send-back goes on a claude seat or not at all ----

#[test]
fn p29_a_send_back_seats_a_server_order_on_claude_and_a_broken_mcp_json_asks() {
    let _guard = crate::nirvana::with_test_home("p29-send-back");
    let root = repo("p29-send-back");
    let project = Project::at(&root);
    std::fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers": {"xtool": {"command": "uv"}}}"#,
    )
    .expect("mcp");
    // Every CLI signed in, nothing used: no `claude auth status`, no `claude -p /usage`.
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    // Without the server the class seats on agy: the rule has something to move.
    let plain = nirvana_seat::pick(
        &table,
        &nirvana_seat::live_world(&table),
        &Request::new("sweep.mechanical"),
    )
    .expect("pick");
    assert_eq!(plain.cli, "agy", "{}", plain.seat);
    let send = |order: &str| {
        let run = "r1";
        let journal = project.journal(run);
        let mut seated_order = seated(order, None);
        seated_order.seat = "haiku".into();
        seated_order.model = "claude-haiku-5-5".into();
        seated_order.class = "sweep.mechanical".into();
        seated_order.chain = Some("chore".into());
        seated_order.needs = vec!["xtool".into()];
        seated_order.cwd = worktree(&root, order).display().to_string();
        journal
            .append(
                1,
                Kind::Opened {
                    project: "p".into(),
                    orchestrator_pane: None,
                    auto: false,
                    pools: vec![],
                    pools_basis: None,
                },
            )
            .expect("append");
        journal
            .append(2, Kind::Seated(Box::new(seated_order)))
            .expect("append");
        let state = replay(&journal.read().0);
        let known = state.order(order).expect("order").clone();
        // Held live, so the send-back starts no worker.
        let child = format!("{order}-s1");
        let live = crate::nirvana_worker::take_live(&project, run, &child)
            .expect("live")
            .expect("lock");
        let ruling = send_back(
            &project,
            run,
            &journal,
            &state,
            &known,
            order,
            "the order\n",
            vec![Axis::new("tests", Mark::Fail, "test failed")],
            Some(&table),
        )
        .expect("send_back");
        drop(live);
        let state = replay(&journal.read().0);
        (
            ruling,
            state.order(&child).map(|child| child.seated.clone()),
        )
    };
    let (ruling, child) = send("jig-1");
    assert_eq!(ruling.verdict, "sent_back", "{:?}", ruling.next);
    assert_eq!(child.expect("child").cli, "claude");
    // A `.mcp.json` that no longer parses: the owner is asked, naming the file, and nothing is seated.
    std::fs::write(root.join(".mcp.json"), "{").expect("mcp");
    let (ruling, child) = send("jig-2");
    assert_eq!(ruling.verdict, "asked");
    assert!(
        ruling.next[0].starts_with("jig-2 must go back, but ")
            && ruling.next[0].contains("does not parse"),
        "{:?}",
        ruling.next
    );
    assert!(child.is_none());
    nirvana_seat::set_test_world(None);
}

// ---- plan 31 §2.5 and review S1: a send-back keeps the device line and never inherits the Send ----

#[test]
fn p31_a_send_back_child_never_inherits_the_send_approval() {
    use crate::nirvana_gui_tools::{
        with_brief_lines, SendOk, BRIEF_LINE, DESKTOP_SEND_LINE, DEVICE_LINE, SEND_LINE,
        SENT_BEFORE_LINE,
    };
    let _guard = crate::nirvana::with_test_home("p31-send-back-lines");
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let root = repo("p31-send-back-lines");
    let project = Project::at(&root);
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    let approval = SendOk {
        quote: "Print the nosepiece when it is ready".into(),
        prompt: "ok. Print the nosepiece when it is ready".into(),
        at: "2026-10-29T01:32:14.584Z".into(),
        transcript: "t.jsonl".into(),
        check: "found verbatim in the owner's prompt at 2026-10-29T01:32:14.584Z".into(),
    };
    let desktop = vec!["bambu-studio-beta".to_owned()];
    for (order, send_ok) in [("slice-1", None), ("print-1", Some(approval.clone()))] {
        let run = format!("r-{order}");
        let journal = project.journal(&run);
        let mut parent = seated(order, None);
        parent.cwd = worktree(&root, order).display().to_string();
        parent.send_ok = send_ok.clone();
        // The parent's brief as `seat run` wrote it: the order, its lines, herdr's footer.
        let text = with_brief_lines(
            "# Slice the nosepiece\n".into(),
            &desktop,
            send_ok.is_some(),
        );
        parent.order_file =
            crate::nirvana_worker::write_order(&project, &root, order, &text).expect("order");
        journal
            .append(
                1,
                Kind::Opened {
                    project: "p".into(),
                    orchestrator_pane: None,
                    auto: false,
                    pools: vec![],
                    pools_basis: None,
                },
            )
            .expect("append");
        journal
            .append(2, Kind::Seated(Box::new(parent.clone())))
            .expect("append");
        let state = replay(&journal.read().0);
        let known = state.order(order).expect("order").clone();
        let root_text = order_text(&parent);
        let ruling = send_back(
            &project,
            &run,
            &journal,
            &state,
            &known,
            order,
            &root_text,
            vec![Axis::new("tests", Mark::Fail, "the slice warns")],
            Some(&table),
        )
        .expect("send_back");
        assert_eq!(ruling.verdict, "sent_back");
        let state = replay(&journal.read().0);
        let child = state
            .orders
            .iter()
            .find(|known| known.seated.parent.as_deref() == Some(order))
            .expect("child");
        let brief = std::fs::read_to_string(&child.seated.order_file).expect("brief");
        // Review S1: a new Send needs a new quote. The child keeps the device line, and an approved parent's child
        // hears it may already have sent.
        assert_eq!(child.seated.send_ok, None, "{order}");
        assert!(
            brief.contains(&format!("{DEVICE_LINE}\n{BRIEF_LINE}")),
            "{brief}"
        );
        assert!(
            !brief.contains(SEND_LINE) && !brief.contains(DESKTOP_SEND_LINE),
            "{brief}"
        );
        assert_eq!(
            brief.contains(SENT_BEFORE_LINE),
            send_ok.is_some(),
            "{brief}"
        );
    }
    nirvana_seat::set_test_world(None);
}

/// Plan 33 §3.2: P33a's contract, `Profile: <name>` at a line's start; a bad name or an indented line is no profile.
#[test]
fn p33_the_order_copy_names_its_profile_at_a_lines_start() {
    assert_eq!(
        spec("# Slice\nProfile: slicer\n").profile.as_deref(),
        Some("slicer")
    );
    assert_eq!(
        spec("# Slice\nProfile: `slicer-2`\n").profile.as_deref(),
        Some("slicer-2")
    );
    assert_eq!(spec("# Slice\n  Profile: slicer\n").profile, None);
    assert_eq!(spec("# Slice\nProfile: Slicer Two\n").profile, None);
    assert_eq!(spec("## Profile: slicer\n").profile, None);
}

/// Plan 33 review B1: the gate reads no profile for a lesson review, whose brief quotes the order it reviews.
#[test]
fn p33_the_gate_reads_no_profile_for_a_lesson_review() {
    let dir = temp("p33-review-profile");
    let file = dir.join("review.md");
    std::fs::write(&file, "Profile: slicer\n# Lesson review of b1\n").unwrap();
    let mut order = seated("b1-lr", None);
    order.order_file = file.display().to_string();
    assert_eq!(
        order_profile(&order, &Spec::default()).as_deref(),
        Some("slicer")
    );
    order.class = crate::nirvana_review::CLASS.into();
    assert_eq!(order_profile(&order, &Spec::default()), None);
}

// ---- Plan 35 P35a: the sources check sees agy's reads ----

/// The sources axis over one cited URL and a stream of one agy `run_command` (or a Claude Bash call).
fn p35a_mark(url: &str, stream: &str) -> Mark {
    let report = serde_json::json!({"sources": [{"url": url, "used_for": "spec"}]});
    web_sources_axis("research.web", Some(&report), stream)
        .expect("axis")
        .mark
}

/// Plan 35 P35a item 1: each Python fetch word counts the URLs of its own segment, from agy and from Claude. Each
/// case holds only its own word, so dropping any one of them fails here.
#[test]
fn p35a_python_fetch_words_count_their_segment() {
    let url = "https://example.com/target";
    let cases = [
        format!("python -c \"from urllib import request; print(request.urlopen('{url}').read())\""),
        format!("python -c \"import urllib.request; r = urllib.request.Request('{url}')\""),
        format!("python -c \"import requests; print(requests.get('{url}').text)\""),
        format!("python -c \"import requests; requests.post('{url}', data={{}})\""),
        format!("python -c \"import requests; print(requests.head('{url}').status_code)\""),
        format!("python -c \"import requests; print(requests.Session().get('{url}').text)\""),
        format!("python -c \"import httpx; print(httpx.get('{url}').text)\""),
        format!("python -c \"import httpx; print(httpx.Client().get('{url}').text)\""),
    ];
    for case in &cases {
        assert_eq!(p35a_mark(url, &agy_run(case)), Mark::Pass, "agy: {case}");
        assert_eq!(
            p35a_mark(url, &claude_bash(case, "")),
            Mark::Pass,
            "claude: {case}"
        );
    }
}

/// Plan 35 P35a item 1: PowerShell's `Invoke-RestMethod` and `irm` fetch; `iwr` and `Invoke-WebRequest` still do.
#[test]
fn p35a_powershell_fetch_words_count_and_irm_as_a_whole_word() {
    let url = "https://example.com/target";
    for case in [
        format!("pwsh -NoProfile -Command \"$c = (Invoke-RestMethod -Uri '{url}'); $c\""),
        format!("$r = irm {url} -Headers @{{a=1}}"),
        format!("(irm '{url}').name"),
        format!("iwr -UseBasicParsing {url}"),
        format!("Invoke-WebRequest -Uri {url}"),
    ] {
        assert_eq!(p35a_mark(url, &agy_run(&case)), Mark::Pass, "{case}");
    }
}

/// Plan 35 P35a item 2: `gh api repos/<o>/<n>[/...]` reads the repository's two URLs and its own endpoint, flags
/// `-X GET` before the endpoint included.
#[test]
fn p35a_gh_api_counts_the_repository_and_its_endpoint() {
    let cmd = "gh api repos/Ai-Rook/dance-scoring/commits/master -q '.commit.committer.date'";
    for url in [
        "https://github.com/Ai-Rook/dance-scoring",
        "https://api.github.com/repos/Ai-Rook/dance-scoring",
        "https://api.github.com/repos/Ai-Rook/dance-scoring/commits/master",
    ] {
        assert_eq!(p35a_mark(url, &agy_run(cmd)), Mark::Pass, "{url}");
    }
    let flagged = "gh api -X GET /repos/o/n/license 2>$null || echo none";
    assert_eq!(
        p35a_mark(
            "https://api.github.com/repos/o/n/license",
            &agy_run(flagged)
        ),
        Mark::Pass
    );
    assert_eq!(
        p35a_mark("https://github.com/o/n", &claude_bash(flagged, "")),
        Mark::Pass
    );
}

/// Plan 35 P35a item 2: `gh repo view <o>/<n>` (or its github.com URL) reads the repository's two URLs.
#[test]
fn p35a_gh_repo_view_counts_the_repository() {
    let cmd = "gh repo view MindDock/motion-tracker --json url,stargazerCount; gh repo view https://github.com/lguibr/Mimeflow.git";
    for url in [
        "https://github.com/MindDock/motion-tracker",
        "https://api.github.com/repos/MindDock/motion-tracker",
        "https://github.com/lguibr/Mimeflow",
    ] {
        assert_eq!(p35a_mark(url, &agy_run(cmd)), Mark::Pass, "{url}");
    }
    // The stable id would count these URLs anyway (the repository named in a gh read), so the mapping itself is
    // checked where the per-segment rule uses it.
    let urls = shell_fetch_urls(cmd);
    for url in [
        "https://github.com/MindDock/motion-tracker",
        "https://api.github.com/repos/MindDock/motion-tracker",
        "https://github.com/lguibr/Mimeflow",
        "https://api.github.com/repos/lguibr/Mimeflow",
    ] {
        assert!(urls.contains(&url.to_owned()), "{url}: {urls:?}");
    }
    // A search names repositories it does not read.
    assert_eq!(
        p35a_mark(
            "https://github.com/MindDock/motion-tracker",
            &agy_run("gh search repos MindDock/motion-tracker")
        ),
        Mark::Fail
    );
}

/// Plan 35 P35a items 1 and 4: still per segment. A URL that a segment only prints (`python` alone fetches nothing),
/// lists on a line of its own, or puts beside `confirm` or `$irm` (both holding `irm`) does not count. The f-string
/// case is yt-video-pick's, in the real send-backs below.
#[test]
fn p35a_a_url_counts_only_from_a_segment_that_fetches() {
    let url = "https://example.com/target";
    assert_eq!(
        p35a_mark(
            url,
            &agy_run(&format!("echo {url}; python -c \"print('{url}')\""))
        ),
        Mark::Fail
    );
    let listed = format!(
        "python -c \"\nimport urllib.request\nurls = [\n    '{url}',\n]\nfor u in urls:\n    print(len(urllib.request.urlopen(u).read()))\n\""
    );
    assert_eq!(p35a_mark(url, &agy_run(&listed)), Mark::Fail);
    assert_eq!(
        p35a_mark(url, &agy_run(&format!("confirm {url}"))),
        Mark::Fail
    );
    assert_eq!(
        p35a_mark(url, &agy_run(&format!("$irm = '{url}'; Write-Output $irm"))),
        Mark::Fail
    );
}

/// Plan 35 P35a item 3: the probe-2 stream (agy calling bladebro's `act navigate` through `call_mcp_tool`) passes
/// `https://example.com`.
#[test]
fn p35a_the_probe_2_bladebro_stream_passes_example_com() {
    const PROBE_2: &str = include_str!("fixtures/probe2-bladebro.jsonl");
    assert_eq!(p35a_mark("https://example.com", PROBE_2), Mark::Pass);
}

/// Each fixture's cited sources, and how many of them the sources check counts.
fn p35a_counts(fixture: &str) -> (usize, usize) {
    let (head, stream) = fixture.split_once('\n').expect("head line");
    let head: Value = serde_json::from_str(head).expect("head json");
    let urls: Vec<&str> = head["sources"]
        .as_array()
        .expect("sources")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let counted = urls
        .iter()
        .filter(|url| p35a_mark(url, stream) == Mark::Pass)
        .count();
    (urls.len(), counted)
}

/// Plan 35 P35a items 4 and 5, and the MotoProject round: the 11 sources send-backs in groove-coach (research §4's 9,
/// and yt-breakdown and yt-breakdown-illit after it), cut to their tool calls with each output's lines that hold a URL
/// or a cited id (record `p35a.md`). Review round: the counts are the real gate's on the full streams too (the same
/// for all 18); body-extrapolation's 19 lost were plain URLs counted only from outputs (B1). master counts 23 of agy's 85.
#[test]
fn p35a_real_sources_send_backs_count_what_the_twin_counts() {
    let fixtures: [(&str, &str, usize, usize); 11] = [
        (
            "asl-inspiration",
            include_str!("fixtures/asl-inspiration.jsonl"),
            12,
            10,
        ),
        (
            "asl-report",
            include_str!("fixtures/asl-report.jsonl"),
            14,
            14,
        ),
        (
            "body-extrapolation",
            include_str!("fixtures/body-extrapolation.jsonl"),
            20,
            1,
        ),
        (
            "ref-authoring",
            include_str!("fixtures/ref-authoring.jsonl"),
            10,
            10,
        ),
        (
            "ref-authoring-s1",
            include_str!("fixtures/ref-authoring-s1.jsonl"),
            10,
            10,
        ),
        (
            "ref-dance-apps",
            include_str!("fixtures/ref-dance-apps.jsonl"),
            8,
            8,
        ),
        (
            "repo-breakdown",
            include_str!("fixtures/repo-breakdown.jsonl"),
            6,
            0,
        ),
        (
            "video-breakdown",
            include_str!("fixtures/video-breakdown.jsonl"),
            2,
            2,
        ),
        (
            "yt-breakdown",
            include_str!("fixtures/yt-breakdown.jsonl"),
            1,
            0,
        ),
        (
            "yt-breakdown-illit",
            include_str!("fixtures/yt-breakdown-illit.jsonl"),
            3,
            0,
        ),
        (
            "yt-video-pick",
            include_str!("fixtures/yt-video-pick.jsonl"),
            5,
            5,
        ),
    ];
    for (order, text, cited, counted) in fixtures {
        assert_eq!(p35a_counts(text), (cited, counted), "{order}");
    }
}

/// The MotoProject round (orchestrator, 2026-10-31): agy research orders in MotoProject, cut the same way. r2 read
/// through `yt_dlp`, Python URL lists and Reddit `.rss`; master and the per-segment rule counted 0 of its 22. The
/// fabricated reel-motion-gemini stays at 0 of 12, and so does its copy with one added step that prints the notes
/// file holding every cited URL (the analysis's sabotage, `control_fab_cat.jsonl`): no fetch, so no read. Review
/// round: reveal-gemini's 8 lost were plain URLs counted only from outputs (B1); r2 keeps 22 through its
/// `subprocess` curl or its cut commands' imports (either alone keeps 22); imp-gemini-aspect needs the cut rule for 5.
#[test]
fn p35a_motoproject_streams_count_what_the_twin_counts() {
    let fixtures: [(&str, &str, usize, usize); 7] = [
        (
            "reel-motion-gemini-r2",
            include_str!("fixtures/moto-reel-motion-gemini-r2.jsonl"),
            22,
            22,
        ),
        (
            "reel-motion-gemini",
            include_str!("fixtures/moto-reel-motion-gemini.jsonl"),
            12,
            0,
        ),
        (
            "fab-cat",
            include_str!("fixtures/moto-fab-cat.jsonl"),
            12,
            0,
        ),
        (
            "reveal-gemini",
            include_str!("fixtures/moto-reveal-gemini.jsonl"),
            18,
            9,
        ),
        (
            "imp-gemini-aspect",
            include_str!("fixtures/moto-imp-gemini-aspect.jsonl"),
            17,
            17,
        ),
        (
            "v13-drone-research",
            include_str!("fixtures/moto-v13-drone-research.jsonl"),
            12,
            2,
        ),
        (
            "v12-smudge-map",
            include_str!("fixtures/moto-v12-smudge-map.jsonl"),
            12,
            12,
        ),
    ];
    for (order, text, cited, counted) in fixtures {
        assert_eq!(p35a_counts(text), (cited, counted), "{order}");
    }
}

/// An agy `run_command` step with its output.
fn agy_run_out(command: &str, output: &str) -> String {
    serde_json::json!({
        "step_update": {
            "step_type": "tool",
            "state": "DONE",
            "tool_name": "run_command",
            "tool_info": {"parameters": {"CommandLine": command}, "output": output}
        }
    })
    .to_string()
}

/// The MotoProject round: what has a stable id, and what does not.
#[test]
fn p35a_stable_ids_are_video_post_repository_and_path() {
    let video = |id: &str| Some(StableId::Video(id.to_owned()));
    assert_eq!(
        stable_id("https://www.youtube.com/watch?v=U24jDyYnU3A&t=3"),
        video("U24jDyYnU3A")
    );
    assert_eq!(
        stable_id("https://youtube.com/shorts/v9SaLex6pn0"),
        video("v9SaLex6pn0")
    );
    assert_eq!(
        stable_id("https://youtu.be/5-HTPj-tS54"),
        video("5-HTPj-tS54")
    );
    assert_eq!(
        stable_id("https://www.reddit.com/r/Insta360/comments/1qdd4qr/keyframe_ramping/"),
        Some(StableId::Post("1qdd4qr".into()))
    );
    assert_eq!(
        stable_id("https://github.com/MindDock/motion-tracker"),
        Some(StableId::Repo("minddock/motion-tracker".into()))
    );
    assert_eq!(
        stable_id("https://api.github.com/repos/o/n"),
        Some(StableId::Repo("o/n".into()))
    );
    assert_eq!(
        stable_id("file:///D:/MotoProject/scripts/post.py"),
        Some(StableId::Path("d:/motoproject/scripts/post.py".into()))
    );
    // Review nit: a Unix path keeps its `/`, and any `%XX` escape is decoded.
    assert_eq!(
        stable_id("file:///home/x/notes.md"),
        Some(StableId::Path("/home/x/notes.md".into()))
    );
    assert_eq!(
        stable_id("file:///C:/My%20Notes/caf%C3%A9.md"),
        Some(StableId::Path("c:/my notes/caf\u{e9}.md".into()))
    );
    // A page inside a repository, a search, a short id and any other site keep the per-segment rule alone.
    for url in [
        "https://github.com/o/n/blob/main/x.py",
        "https://www.youtube.com/results?search_query=hiking",
        "https://www.youtube.com/watch?v=short",
        "https://example.com/watch?v=U24jDyYnU3A",
    ] {
        assert_eq!(stable_id(url), None, "{url}");
    }
}

/// The MotoProject round: a stable id counts anywhere in a command that runs a fetch (`yt_dlp` included) or in its
/// output. A command that fetches nothing vouches for nothing, whatever it prints, and a longer id is not the cited
/// one. Review round (B1): a plain URL never counts from an output.
#[test]
fn p35a_a_stable_id_counts_in_a_fetching_command_or_its_output() {
    let video = "https://www.youtube.com/watch?v=U24jDyYnU3A";
    let listed = "python -c \"import yt_dlp\nids = [\n  'U24jDyYnU3A',\n]\nfor i in ids:\n  yt_dlp.YoutubeDL().extract_info(i, download=False)\"";
    assert_eq!(p35a_mark(video, &agy_run(listed)), Mark::Pass);
    let post = "https://www.reddit.com/r/Insta360/comments/1qdd4qr/keyframe_ramping/";
    let rss = "python -c \"import urllib.request\nu = 'https://www.reddit.com/r/Insta360/comments/1qdd4qr/keyframe_ramping/.rss'\nprint(urllib.request.urlopen(u).read()[:99])\"";
    assert_eq!(p35a_mark(post, &agy_run(rss)), Mark::Pass);
    // A resolved redirect's output names the video it led to (reveal-gemini's `LOC |` lines).
    let redirect =
        "curl -sI https://vertexaisearch.cloud.google.com/grounding-api-redirect/AUZIYQF";
    let location = format!("location: {video}\n");
    assert_eq!(
        p35a_mark(video, &agy_run_out(redirect, &location)),
        Mark::Pass
    );
    // Claude: the same through a Bash call and its result.
    let claude = |command: &str, result: &str| {
        format!(
            "{}\n{}",
            serde_json::json!({"type": "assistant", "message": {"content": [{
                "type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": command}
            }]}}),
            serde_json::json!({"type": "user", "message": {"content": [{
                "type": "tool_result", "tool_use_id": "t1", "content": result
            }]}})
        )
    };
    assert_eq!(p35a_mark(video, &claude(redirect, &location)), Mark::Pass);
    // A Bash call that fetches nothing vouches for nothing in its result.
    assert_eq!(
        p35a_mark(video, &claude("ls downloads", &location)),
        Mark::Fail
    );

    // Printing the notes is not a read: no fetch word in the command.
    let plain = "https://example.com/blog/hooks";
    let notes = agy_run_out("type notes.md", &format!("{video}\n{post}\n{plain}\n"));
    for url in [video, post, plain] {
        assert_eq!(p35a_mark(url, &notes), Mark::Fail, "{url}");
    }
    assert_eq!(p35a_mark(video, &agy_run("echo U24jDyYnU3A")), Mark::Fail);
    // A plain URL in a fetching command's text, outside the segment that fetches, still does not count.
    assert_eq!(
        p35a_mark(
            plain,
            &agy_run(&format!("curl -s https://other.org/a; echo {plain}"))
        ),
        Mark::Fail
    );
    // A longer id or post id is another source.
    assert_eq!(
        p35a_mark(video, &agy_run("yt-dlp --print title xU24jDyYnU3A")),
        Mark::Fail
    );
    assert_eq!(
        p35a_mark(
            post,
            &agy_run("curl -s https://www.reddit.com/comments/1qdd4qrx/.rss")
        ),
        Mark::Fail
    );
}

/// Review round (B1): a plain URL never counts from a fetch's output, which is the owner's "plain URLs keep the
/// per-segment rule". Each case is the review's: an echo after a fetch, the notes printed beside a fetch, the notes
/// fetched as `file://`, and a real resolved redirect (the URL resolved is not the URL fetched).
#[test]
fn p35a_a_plain_url_never_counts_from_an_output() {
    let (a, b) = ("https://a.org/x", "https://b.org/y");
    let echoed = agy_run_out(
        &format!("curl -sI https://example.com; echo {a} {b}"),
        &format!("HTTP/1.1 200 OK\n{a} {b}\n"),
    );
    let cat = agy_run_out(
        "cat notes.md; curl -sI https://example.com",
        &format!("# notes\n{a}\n{b}\nHTTP/1.1 200 OK\n"),
    );
    let file = agy_run_out(
        "curl -s file:///C:/proj/notes.md",
        &format!("# notes\n{a}\n{b}\n"),
    );
    let resolved = agy_run_out(
        "curl -sI https://vertexaisearch.cloud.google.com/grounding-api-redirect/AUZIYQF",
        &format!("location: {a}\n"),
    );
    for (case, stream) in [
        ("echo", &echoed),
        ("cat", &cat),
        ("file", &file),
        ("resolved", &resolved),
    ] {
        assert_eq!(p35a_mark(a, stream), Mark::Fail, "{case}");
    }
}

/// Review round (S2): a fetch word counts only as a command word, never inside a comment, a string or an argument,
/// so a command that fetches nothing cannot vouch for the ids typed in it. A real fetch still does, in each of the
/// shapes agy wrote: an assignment, a subexpression, a `subprocess` argv (reel-motion-gemini-r2's `curl.exe`) and
/// `os.system`.
#[test]
fn p35a_a_fetch_word_counts_only_as_a_command_word() {
    let video = "https://www.youtube.com/watch?v=U24jDyYnU3A";
    for case in [
        "echo U24jDyYnU3A # yt-dlp",
        "echo \"curl\" U24jDyYnU3A",
        "Write-Output curl U24jDyYnU3A",
        "type curl.txt; echo U24jDyYnU3A",
        "$curl_notes = 'U24jDyYnU3A'",
        "$curl = 'U24jDyYnU3A'",
        "python -c \"curl_count = 0\nprint('U24jDyYnU3A')\"",
        "python -c \"import requests  # requests.get(u) comes next\nids = ['U24jDyYnU3A']\nprint(*ids)\"",
        "python -c \"x = 'requests.get(u)'\nprint('U24jDyYnU3A')\"",
        "echo urlopen(U24jDyYnU3A)",
        "python -c \"import subprocess\nsubprocess.run(['echo', 'curl', 'U24jDyYnU3A'])\"",
        "python -c \"import yt_dlp\nprint('U24jDyYnU3A')\"",
    ] {
        assert_eq!(p35a_mark(video, &agy_run(case)), Mark::Fail, "{case}");
    }
    for case in [
        "$u = 'https://www.youtube.com/watch?v=' + 'U24jDyYnU3A'; $r = Invoke-WebRequest -Uri $u",
        "(irm \"https://noembed.com/embed?id=U24jDyYnU3A\").title",
        "python -c \"import subprocess\nout = subprocess.check_output(['curl.exe', '-s', 'https://noembed.com/embed?v=U24jDyYnU3A'])\"",
        "python -c \"import os\nos.system('yt-dlp -j U24jDyYnU3A')\"",
        "pwsh -NoProfile -Command \"Invoke-RestMethod https://noembed.com/embed?v=U24jDyYnU3A\"",
    ] {
        assert_eq!(p35a_mark(video, &agy_run(case)), Mark::Pass, "{case}");
    }
}

/// Review round (S2): agy cuts a long `CommandLine` at 512 characters and an ellipsis, so the call can lie past the
/// cut (66 of reel-motion-gemini-r2's 112 commands). In a cut command an import of a fetch module counts; in a whole
/// command it does not.
#[test]
fn p35a_a_cut_command_counts_its_fetch_import() {
    let video = "https://www.youtube.com/watch?v=U24jDyYnU3A";
    let head = "@'\nimport yt_dlp, json, sys\nyt_urls = [\n    'https://www.youtube.com/watch?v=U24jDyYnU3A',\n    'https://www.youtube.com/watch?v=";
    assert_eq!(
        p35a_mark(video, &agy_run(&format!("{head}\u{2026}"))),
        Mark::Pass
    );
    assert_eq!(p35a_mark(video, &agy_run(head)), Mark::Fail);
    assert_eq!(
        p35a_mark(
            video,
            &agy_run("@'\nimport json\nids = ['U24jDyYnU3A',\u{2026}")
        ),
        Mark::Fail
    );
}

/// Review round (S2): a fetch whose output lists ids it never opened vouches only for the ids typed in it: a YouTube
/// search or `--flat-playlist`, a Reddit `search.json`, the GitHub search API, and a watch page's related videos. A
/// local reader's or a `file://` fetch's output vouches for nothing.
#[test]
fn p35a_a_listing_or_a_local_read_does_not_vouch_for_its_output() {
    let video = "https://www.youtube.com/watch?v=U24jDyYnU3A";
    let post = "https://www.reddit.com/r/hiking/comments/1dffk22/action_cams/";
    let repo = "https://github.com/MindDock/motion-tracker";
    let listings = [
        ("python -m yt_dlp -J \"ytsearch5:hiking\"", video),
        ("yt-dlp --flat-playlist -J https://vimeo.com/channels/staffpicks", video),
        ("yt-dlp -J https://www.youtube.com/@bestthree60/videos", video),
        (
            "curl -s \"https://www.reddit.com/r/hiking/search.json?q=action+cam&restrict_sr=1\"",
            post,
        ),
        (
            "curl -s \"https://api.github.com/search/repositories?q=motion+tracker\"",
            repo,
        ),
        ("curl -sL https://www.youtube.com/watch?v=AAAAAAAAAAA", video),
        ("cat notes.md; curl -sI https://example.com", video),
        ("curl -s file:///C:/proj/notes.md", video),
        (
            "python -c \"import urllib.request\nprint(open('notes.md').read())\nurllib.request.urlopen('https://example.com')\"",
            video,
        ),
    ];
    let output = format!(
        "{{\"id\": \"U24jDyYnU3A\", \"permalink\": \"https://www.reddit.com/r/hiking/comments/1dffk22/action_cams/\", \
\"full_name\": \"MindDock/motion-tracker\"}}\n{video}\n"
    );
    for (command, url) in listings {
        assert_eq!(
            p35a_mark(url, &agy_run_out(command, &output)),
            Mark::Fail,
            "{command}"
        );
    }
    // A reader later in a pipeline reads the fetch, not a file.
    assert_eq!(
        p35a_mark(
            video,
            &agy_run_out(
                "curl -sI https://vertexaisearch.cloud.google.com/grounding-api-redirect/AUZ | grep -i location",
                &format!("location: {video}\n")
            )
        ),
        Mark::Pass
    );
    // The watch page fetched vouches for its own id, typed in the command; yt-dlp's read of one video for its output.
    assert_eq!(
        p35a_mark(
            "https://youtu.be/AAAAAAAAAAA",
            &agy_run_out(
                "curl -sL https://www.youtube.com/watch?v=AAAAAAAAAAA",
                &output
            )
        ),
        Mark::Pass
    );
    assert_eq!(
        p35a_mark(
            video,
            &agy_run_out(
                "yt-dlp -j https://youtu.be/BBBBBBBBBBB",
                &format!("{{\"webpage_url\": \"{video}\"}}")
            )
        ),
        Mark::Pass
    );
}

/// Review round (S2): Claude's WebFetch counts only its `url`, not the ids typed in its `prompt`; a browser MCP call
/// (bladebro) counts its input.
#[test]
fn p35a_claude_webfetch_counts_its_url_and_a_browser_mcp_call_its_input() {
    let video = "https://www.youtube.com/watch?v=U24jDyYnU3A";
    let call = |name: &str, input: Value| {
        serde_json::json!({"type": "assistant", "message": {"content": [{
            "type": "tool_use", "id": "w1", "name": name, "input": input
        }]}})
        .to_string()
    };
    let prompted = call(
        "WebFetch",
        serde_json::json!({"url": "https://example.com/x", "prompt": "summarise U24jDyYnU3A and 1dffk22"}),
    );
    assert_eq!(p35a_mark(video, &prompted), Mark::Fail);
    let fetched = call(
        "WebFetch",
        serde_json::json!({"url": "https://youtu.be/U24jDyYnU3A", "prompt": "summarise"}),
    );
    assert_eq!(p35a_mark(video, &fetched), Mark::Pass);
    let bladebro = call(
        "mcp__bladebro__act",
        serde_json::json!({"action": "navigate", "url": "https://m.youtube.com/watch?v=U24jDyYnU3A"}),
    );
    assert_eq!(p35a_mark(video, &bladebro), Mark::Pass);
}

/// Review round (S1): a `gh` read counts only when `gh` is the segment's command word.
#[test]
fn p35a_gh_counts_only_as_a_command_word() {
    let url = "https://github.com/o/n";
    for case in [
        "echo gh api repos/o/n",
        "python -c \"x = 1\n# gh repo view o/n\nprint(x)\"",
        "python -c \"cmd = 'gh api repos/o/n'\nprint(cmd)\"",
        "Write-Output 'gh repo view o/n'",
    ] {
        assert_eq!(p35a_mark(url, &agy_run(case)), Mark::Fail, "{case}");
    }
    for case in [
        "$meta = gh api repos/o/n --jq .description",
        "Set-Location C:\\proj\ngh repo view o/n --json url",
        "(gh api \"repos/o/n/readme\").content",
    ] {
        assert_eq!(p35a_mark(url, &agy_run(case)), Mark::Pass, "{case}");
    }
}

/// Review (tests): a repository id is not part of a longer name, `.git` after it is the same repository, and a
/// longer suffix is not; the host prefixes a cited URL may carry still give its id; a post id stands in a reddit.com
/// address, so a GitHub `comments/<id>` is not it.
#[test]
fn p35a_repo_and_post_ids_keep_their_bounds() {
    let fetch = |rest: &str| agy_run(&format!("curl -sI https://example.com; echo {rest}"));
    let repo = "https://github.com/foo/bar";
    assert_eq!(p35a_mark(repo, &fetch("foo/bar.git")), Mark::Pass);
    for longer in ["foo/bar-baz", "myfoo/bar", "foo/bar.github", "foo/bar_2"] {
        assert_eq!(p35a_mark(repo, &fetch(longer)), Mark::Fail, "{longer}");
    }
    let post = "https://www.reddit.com/r/x/comments/abcd12/slug/";
    assert_eq!(
        p35a_mark(
            post,
            &agy_run("curl -s https://api.github.com/repos/o/n/issues/1/comments/abcd12")
        ),
        Mark::Fail
    );
    assert_eq!(
        p35a_mark(
            post,
            &agy_run("curl -s https://old.reddit.com/r/x/comments/abcd12/.rss")
        ),
        Mark::Pass
    );
    for (url, id) in [
        (
            "https://m.youtube.com/watch?v=U24jDyYnU3A",
            StableId::Video("U24jDyYnU3A".into()),
        ),
        (
            "https://music.youtube.com/watch?v=U24jDyYnU3A",
            StableId::Video("U24jDyYnU3A".into()),
        ),
        (
            "https://old.reddit.com/r/x/comments/abcd12/",
            StableId::Post("abcd12".into()),
        ),
        (
            "https://embed.reddit.com/r/x/comments/abcd12/",
            StableId::Post("abcd12".into()),
        ),
        (
            "https://www.github.com/Foo/Bar",
            StableId::Repo("foo/bar".into()),
        ),
    ] {
        assert_eq!(stable_id(url), Some(id), "{url}");
    }
}

/// The MotoProject round: `yt_dlp`, `yt-dlp` and `YoutubeDL` are fetch words, per segment as well.
#[test]
fn p35a_yt_dlp_is_a_fetch_word() {
    let url = "https://vimeo.com/76979871";
    for case in [
        format!("python -m yt_dlp --skip-download {url}"),
        format!("yt-dlp -j {url}"),
        format!("python -c \"print(YoutubeDL().extract_info('{url}'))\""),
    ] {
        assert_eq!(p35a_mark(url, &agy_run(&case)), Mark::Pass, "{case}");
    }
}

/// The MotoProject round: a `file:///` source counts when a file tool opened that path, the case and separators
/// aside; a shell `type` of it does not.
#[test]
fn p35a_a_file_source_counts_against_read_file() {
    let url = "file:///D:/MotoProject/scripts/post.py";
    let agy = serde_json::json!({"step_update": {"step_type": "tool", "state": "DONE", "tool_name": "read_file",
        "tool_info": {"parameters": {"Path": "D:\\MotoProject\\scripts\\post.py"}}}})
    .to_string();
    assert_eq!(p35a_mark(url, &agy), Mark::Pass);
    let claude = serde_json::json!({"type": "assistant", "message": {"content": [{
        "type": "tool_use", "id": "r1", "name": "Read", "input": {"file_path": "d:/motoproject/scripts/post.py"}
    }]}})
    .to_string();
    assert_eq!(p35a_mark(url, &claude), Mark::Pass);
    assert_eq!(
        p35a_mark(url, &agy_run("type D:\\MotoProject\\scripts\\post.py")),
        Mark::Fail
    );
    assert_eq!(
        p35a_mark("file:///D:/MotoProject/scripts/other.py", &agy),
        Mark::Fail
    );
}

/// The MotoProject round: reel-motion-gemini's `Owns:` line was read as "the order names no owned file".
#[test]
fn p35a_the_owns_key_is_read_in_any_case() {
    for line in ["owns: a.py", "Owns: a.py", "OWNS: a.py"] {
        assert_eq!(spec(line).owns, vec!["a.py".to_owned()], "{line}");
    }
    assert!(spec("ownsx: a.py").owns.is_empty());
}

/// The MotoProject round: a sources send-back trusts nothing the worker cited; any other keeps "do not start over".
/// Review round (S4): a sources fail with nothing cited to open (no Work Report) keeps it too, and a mixed fail keeps
/// it and adds the sources text.
#[test]
fn p35a_a_sources_send_back_says_no_cited_source_is_trusted() {
    let cited = Axis::new(
        "sources",
        Mark::Fail,
        "2 source(s) cited that were not fetched, read or browsed",
    )
    .with(vec!["https://a.org/x".into(), "https://b.org/y".into()]);
    let tests = Axis::new("tests", Mark::Fail, "1 failing");
    let brief = |axes: &[Axis]| send_back_brief("o", "o", 1, axes, "# o\n", "nirvana-integrate/r1");
    let only = brief(std::slice::from_ref(&cited));
    assert!(only.contains(SOURCES_REDO), "{only}");
    assert!(!only.contains("do not start over"), "{only}");
    let mixed = brief(&[cited, tests.clone()]);
    assert!(mixed.contains(SOURCES_REDO), "{mixed}");
    assert!(mixed.contains("do not start over"), "{mixed}");
    let alone = brief(&[tests]);
    assert!(alone.contains("do not start over"), "{alone}");
    assert!(!alone.contains(SOURCES_REDO), "{alone}");
    for why in [
        "no sources: no Work Report was delivered",
        "no sources: the Work Report names no sources",
    ] {
        let none = brief(&[Axis::new("sources", Mark::Fail, why)]);
        assert!(none.contains("do not start over"), "{none}");
        assert!(!none.contains(SOURCES_REDO), "{none}");
    }
}

/// The owned axis of an order in the project folder, gated with no branch.
fn p35a_root_owned(root: &Path, order: &str, owns: &[&str]) -> Axis {
    let spec = spec_with(owns, "git ls-files", &[]);
    let det = deterministic(
        &Subject {
            root,
            cwd: root,
            run: "r1",
            order,
            branch: None,
            class: "build.bounded",
            report: Some(&done()),
            spec: &spec,
            ports: &no_ports,
        },
        None,
    );
    axis(&det, "owned").clone()
}

fn p35a_seat_in_root(root: &Path, order: &str, at: u64) {
    let mut order_seated = seated(order, None);
    order_seated.cwd = root.display().to_string();
    Project::at(root)
        .journal("r1")
        .append(at, Kind::Seated(Box::new(order_seated)))
        .expect("seated");
}

/// The MotoProject round: reel-motion-gemini-r2 ran in the project folder and was sent back for "3958 file(s) changed
/// that the order does not own", the whole dirty tree. With the seat snapshot, only what the order changed counts: a
/// file dirty before seating counts only when the order changes it again, a new unowned file still fails, and so
/// does a clean tracked file the order deletes.
#[test]
fn p35a_the_owned_axis_in_the_project_folder_counts_only_the_orders_changes() {
    let root = repo("p35a-owned-snap");
    std::fs::write(root.join("README.md"), "dirty before seating\n").unwrap();
    std::fs::write(root.join("stray.txt"), "untracked before seating\n").unwrap();
    std::fs::remove_file(root.join("test_calc.py")).unwrap();
    let project = Project::at(&root);
    p35a_seat_in_root(&root, "root-order", now());
    assert_eq!(
        write_seat_snapshot(&project, "r1", "root-order", &root),
        Ok(3)
    );
    std::fs::write(root.join("calc.py"), "def add(a, b):\n    return b + a\n").unwrap();
    let owned = p35a_root_owned(&root, "root-order", &["calc.py"]);
    assert_eq!(owned.mark, Mark::Pass, "{owned:?}");
    assert!(owned.why.starts_with("1 changed file"), "{owned:?}");
    assert!(!owned.why.contains(NO_SNAPSHOT), "{owned:?}");

    // The order changes a file that was already dirty, and makes a new one: both are its own, unowned.
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(
        root.join("README.md"),
        "dirty before seating, and changed by the order\n",
    )
    .unwrap();
    std::fs::write(root.join("new.txt"), "the order's\n").unwrap();
    let owned = p35a_root_owned(&root, "root-order", &["calc.py"]);
    assert_eq!(owned.mark, Mark::Fail, "{owned:?}");
    assert_eq!(
        owned.lines,
        vec!["README.md".to_owned(), "new.txt".to_owned()]
    );
}

/// The MotoProject round: an order seated before the snapshot existed (r2 itself) counts a dirty file only when it was
/// created or modified at or after its seating. Review round (S3): a deletion counts again, as on master; a file the
/// order wrote with an old modification time (yt-dlp's upload date) counts by its creation time; a non-ASCII name
/// counts; and the axis says the snapshot was missing.
#[test]
fn p35a_with_no_snapshot_the_owned_axis_counts_files_modified_since_seating() {
    let root = repo("p35a-owned-mtime");
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    let age = |path: &Path| {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(old)
            .unwrap();
    };
    for name in ["README.md", "stray.txt"] {
        std::fs::write(root.join(name), "dirty an hour before seating\n").unwrap();
        age(&root.join(name));
    }
    // Seating is in whole seconds: a second later, nothing made before it reads as made at or after it.
    std::thread::sleep(Duration::from_millis(1100));
    p35a_seat_in_root(&root, "old-order", now());
    std::fs::write(root.join("calc.py"), "def add(a, b):\n    return b + a\n").unwrap();
    let owned = p35a_root_owned(&root, "old-order", &["calc.py"]);
    assert_eq!(owned.mark, Mark::Pass, "{owned:?}");
    assert!(owned.why.starts_with("1 changed file"), "{owned:?}");
    assert!(owned.why.contains(NO_SNAPSHOT), "{owned:?}");

    std::fs::remove_file(root.join("test_calc.py")).unwrap();
    std::fs::create_dir_all(root.join("notes")).unwrap();
    std::fs::write(root.join("notes").join("caf\u{e9}.md"), "the order's\n").unwrap();
    std::fs::write(root.join("clip.mp4"), "downloaded with the upload date\n").unwrap();
    age(&root.join("clip.mp4"));
    let owned = p35a_root_owned(&root, "old-order", &["calc.py"]);
    assert_eq!(owned.mark, Mark::Fail, "{owned:?}");
    let mut expected = vec!["notes/caf\u{e9}.md", "test_calc.py"];
    // A platform with no creation time sees only the modification time.
    if std::fs::metadata(root.join("clip.mp4"))
        .and_then(|meta| meta.created())
        .is_ok()
    {
        expected.insert(0, "clip.mp4");
    }
    let mut lines = owned.lines.clone();
    lines.sort();
    assert_eq!(lines, expected, "{owned:?}");
}

// ---- plan 37: cited videos are checked, and invented ones step the redo up ----

const P37_REAL: &str = "real0000000";
const P37_FAKE: &str = "fake0000000";
const P37_SLOW: &str = "slow0000000";
const P37_GONE: &str = "gone0000000";

/// A stand-in oEmbed on 127.0.0.1, never the network: 200 for `P37_REAL`, 404 for `P37_FAKE`, 401 for `P37_GONE`,
/// and `P37_SLOW` answered only after 3 s. Each request is answered on its own thread; the server lives with the test.
fn p37_oembed() -> String {
    use std::io::Write as _;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}/oembed", listener.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                    head.push(byte[0]);
                }
                let line = String::from_utf8_lossy(&head)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_owned();
                let status = if line.contains(P37_REAL) {
                    "200 OK"
                } else if line.contains(P37_FAKE) {
                    "404 Not Found"
                } else if line.contains(P37_GONE) {
                    "401 Unauthorized"
                } else if line.contains(P37_SLOW) {
                    std::thread::sleep(Duration::from_secs(3));
                    "200 OK"
                } else {
                    "400 Bad Request"
                };
                let body = "{}";
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    base
}

fn p37_report(ids: &[&str]) -> Value {
    let sources: Vec<Value> = ids
        .iter()
        .map(|id| {
            serde_json::json!({"url": format!("https://www.youtube.com/watch?v={id}"), "used_for": "x"})
        })
        .collect();
    serde_json::json!({"status": "done", "summary": "s", "files": [], "tests": [], "gotchas": [], "sources": sources})
}

const BUDGET: Duration = Duration::from_secs(OEMBED_BUDGET_S);

/// L4: "a 404 fails, an unreachable check is a note". A 200 passes, a 404 fails naming the id, another status and a
/// timeout only add a note, and after a timeout the calls left are not made.
#[test]
fn p37_the_existence_check_fails_a_404_passes_a_200_and_only_notes_a_timeout() {
    // A gate outside the tests asks YouTube itself, over https; these calls go to the stand-in only.
    assert_eq!(OEMBED_URL, "https://www.youtube.com/oembed");
    let base = p37_oembed();
    let short = Duration::from_millis(800);
    assert_eq!(oembed(&base, P37_REAL, short), Video::Exists);
    assert_eq!(oembed(&base, P37_FAKE, short), Video::Missing);
    assert_eq!(oembed(&base, P37_GONE, short), Video::Other(401));
    assert!(matches!(
        oembed(&base, P37_SLOW, short),
        Video::Unreachable(_)
    ));
    let pass = || {
        Axis::new(
            "sources",
            Mark::Pass,
            "2 source(s) cited, each verified in stream",
        )
    };
    let mut ask = |id: &str, _: Duration| oembed(&base, id, short);
    // A 200 and a 404: the axis fails, naming the missing id only.
    let report = p37_report(&[P37_REAL, P37_FAKE]);
    let (axis, note) = check_videos(
        pass(),
        Some(&report),
        &mut ask,
        OEMBED_CALLS_PER_GATE,
        BUDGET,
    );
    assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
    assert_eq!(axis.lines, vec![format!("{NO_SUCH_VIDEO}{P37_FAKE}")]);
    assert!(note.is_none(), "{note:?}");
    assert_eq!(
        invented_videos(std::slice::from_ref(&axis)),
        vec![P37_FAKE.to_owned()]
    );
    // Only 200s: the axis passes and says what was checked.
    let report = p37_report(&[P37_REAL]);
    let (axis, note) = check_videos(
        pass(),
        Some(&report),
        &mut ask,
        OEMBED_CALLS_PER_GATE,
        BUDGET,
    );
    assert_eq!(axis.mark, Mark::Pass, "{axis:?}");
    assert!(
        axis.why.ends_with("1 cited video(s) exist on YouTube"),
        "{}",
        axis.why
    );
    assert!(note.is_none());
    // A 401, then a timeout, then a 404 never asked: no fail, one note naming all three.
    let report = p37_report(&[P37_GONE, P37_SLOW, P37_FAKE]);
    let (axis, note) = check_videos(
        pass(),
        Some(&report),
        &mut ask,
        OEMBED_CALLS_PER_GATE,
        BUDGET,
    );
    assert_eq!(axis.mark, Mark::Pass, "{axis:?}");
    let note = note.expect("a note");
    assert_eq!((note.axis.as_str(), note.mark), ("videos", Mark::Note));
    assert_eq!(note.lines.len(), 3, "{note:?}");
    assert!(
        note.lines[1].starts_with(&format!("{P37_SLOW}: oEmbed unreachable")),
        "{note:?}"
    );
    assert!(note.lines[2].contains("not checked"), "{note:?}");
    // The cap: calls past it are not made, and the note says how many.
    let mut calls = 0;
    let report = p37_report(&[P37_REAL, P37_FAKE]);
    let (axis, note) = check_videos(
        pass(),
        Some(&report),
        &mut |_, _| {
            calls += 1;
            Video::Exists
        },
        1,
        BUDGET,
    );
    assert_eq!((calls, axis.mark), (1, Mark::Pass));
    assert!(note.expect("note").lines[0].contains("over the cap of 1"));
    // Review N3: the calls of one gate share a budget; each is told what is left, and none is made once it is spent.
    let mut lefts = Vec::new();
    let (_, note) = check_videos(
        pass(),
        Some(&report),
        &mut |_, left| {
            lefts.push(left);
            Video::Exists
        },
        OEMBED_CALLS_PER_GATE,
        BUDGET,
    );
    assert!(note.is_none(), "{note:?}");
    assert!(
        lefts.len() == 2 && lefts.iter().all(|left| *left <= BUDGET),
        "{lefts:?}"
    );
    let (axis, note) = check_videos(
        pass(),
        Some(&report),
        &mut |_, _| unreachable!("the budget is spent"),
        OEMBED_CALLS_PER_GATE,
        Duration::ZERO,
    );
    assert_eq!(axis.mark, Mark::Pass, "{axis:?}");
    let note = note.expect("a note");
    assert_eq!(note.lines.len(), 2, "{note:?}");
    assert!(
        note.lines[0].ends_with("s for oEmbed were spent"),
        "{note:?}"
    );
}

/// Through `deterministic`, as a gate runs it: the stand-in is asked and a 404 fails the sources axis. With no
/// stand-in a test build asks nothing.
#[test]
fn p37_the_gate_asks_oembed_for_a_research_orders_videos() {
    let root = repo("p37-det");
    let spec = Spec::default();
    let report = p37_report(&[P37_REAL, P37_FAKE]);
    let streams = root.join(".nirvana/runs/r1/streams");
    std::fs::create_dir_all(&streams).unwrap();
    std::fs::write(
        streams.join("res.jsonl"),
        agy_run(&format!(
            "curl -s https://www.youtube.com/watch?v={P37_REAL} https://www.youtube.com/watch?v={P37_FAKE}"
        )),
    )
    .unwrap();
    let det = || {
        deterministic(
            &Subject {
                root: &root,
                cwd: &root,
                run: "r1",
                order: "res",
                branch: None,
                class: "research.web",
                report: Some(&report),
                spec: &spec,
                ports: &no_ports,
            },
            None,
        )
    };
    let unasked = det();
    assert!(
        !unasked.axes.iter().any(|axis| axis.axis == "videos")
            && !axis(&unasked, "sources").why.contains("on YouTube"),
        "{unasked:?}"
    );
    set_test_oembed(Some(p37_oembed()));
    let asked = det();
    set_test_oembed(None);
    let sources = axis(&asked, "sources");
    assert_eq!(sources.mark, Mark::Fail, "{sources:?}");
    assert!(
        sources
            .lines
            .contains(&format!("{NO_SUCH_VIDEO}{P37_FAKE}")),
        "{sources:?}"
    );
    assert!(
        !sources.lines.iter().any(|line| line.contains(P37_REAL)),
        "{sources:?}"
    );
}

/// The send-back of a research order whose sources failed, on `seat`, with `lines` on the sources axis: the child's
/// seat, its override reason, its brief, and the run's override count after.
fn p37_send_back(
    name: &str,
    seat: &str,
    lines: Vec<String>,
) -> (String, Option<String>, String, u32) {
    let axes = vec![Axis::new("sources", Mark::Fail, "the sources").with(lines)];
    let sent = p37_gate_fail(name, seat, axes, BTreeMap::new());
    assert_eq!(sent.ruling.verdict, "sent_back", "{:?}", sent.ruling);
    let child = sent.child.expect("child");
    let brief = std::fs::read_to_string(&child.seated.order_file).expect("brief");
    (
        child.seated.seat,
        child.seated.override_reason,
        brief,
        sent.overrides,
    )
}

/// What one failing gate of a research order made: its ruling, the child if one was seated, a second gate of the
/// order by hand, and the run's override count after.
struct P37Gated {
    ruling: Ruling,
    child: Option<OrderState>,
    again: Result<Ruling, String>,
    overrides: u32,
    /// The orchestrator's queue lines after both gates (plan 37b).
    queue: Vec<String>,
    /// Each `gated` event's reason, in journal order (plan 37b review N3).
    reasons: Vec<Option<String>>,
    /// The order's record word after the gate (plan 37b review S1).
    outcome: String,
}

/// The send-back of a research order seated on `seat`, with `axes`, in a world whose `clis` are refused.
fn p37_gate_fail(
    name: &str,
    seat: &str,
    axes: Vec<Axis>,
    clis: BTreeMap<String, String>,
) -> P37Gated {
    p37_gate_fail_with(name, seat, axes, clis, None)
}

/// `p37_gate_fail`, the order first delivering `report` when one is given (plan 37b).
fn p37_gate_fail_with(
    name: &str,
    seat: &str,
    axes: Vec<Axis>,
    clis: BTreeMap<String, String>,
    report: Option<Value>,
) -> P37Gated {
    let _guard = crate::nirvana::with_test_home(name);
    let root = repo(name);
    let project = Project::at(&root);
    let run = "r1";
    let journal = project.journal(run);
    nirvana_seat::set_test_world(Some(clis));
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    let mut parent = seated("res", None);
    parent.class = "research.web".into();
    parent.chain = Some("research".into());
    parent.seat = seat.into();
    parent.cli = "agy".into();
    parent.model = table.seat(seat).expect("seat").model_for(Effort::Low);
    parent.effort = Effort::Low;
    parent.cwd = root.display().to_string();
    journal
        .append(1, Kind::Seated(Box::new(parent)))
        .expect("seated");
    if let Some(report) = report {
        journal
            .append(
                2,
                Kind::Delivered {
                    order: "res".into(),
                    report,
                },
            )
            .expect("delivered");
    }
    // The child is held live, so the send-back starts no worker.
    let _live = crate::nirvana_worker::take_live(&project, run, "res-s1").unwrap();
    let state = replay(&journal.read().0);
    let known = state.order("res").expect("order").clone();
    let ruling = send_back(
        &project,
        run,
        &journal,
        &state,
        &known,
        "res",
        "# res\n",
        axes,
        Some(&table),
    )
    .expect("send_back");
    let child = replay(&journal.read().0).order("res-s1").cloned();
    let again = gate(&project, run, "res");
    let overrides = nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE))
        .map_or(0, |leases| leases.overrides(run));
    let events = journal.read().0;
    let after = replay(&events);
    let outcome = nirvana_run::outcome_word(after.order("res").expect("res"));
    let queue = after.queue.into_iter().map(|item| item.line).collect();
    let reasons = events
        .iter()
        .filter_map(|event| match &event.kind {
            Kind::Gated { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect();
    nirvana_seat::set_test_world(None);
    P37Gated {
        ruling,
        child,
        again,
        overrides,
        queue,
        reasons,
        outcome,
    }
}

/// Plan 71: the parent is on hydrogen, the seat that took flash's place (flash is off, and an off seat steps up to
/// nothing).
/// L5: "Step up one seat". reel-motion-gemini's redo went back to the flash-low seat that invented its sources; a
/// missing video now sends the redo to the next stronger seat of research.web, carbon, as herdr's own step (no run
/// override spent), and its brief trusts nothing from the first try.
#[test]
fn p37_a_send_back_for_invented_videos_steps_up_one_seat_and_trusts_nothing() {
    let (seat, reason, brief, overrides) = p37_send_back(
        "p37-step-up",
        "hydrogen",
        vec![format!("{NO_SUCH_VIDEO}{P37_FAKE}")],
    );
    assert_eq!(seat, "carbon");
    assert!(
        reason
            .as_deref()
            .is_some_and(|why| why.starts_with("step up from hydrogen to carbon")),
        "{reason:?}"
    );
    assert_eq!(overrides, 0);
    assert!(brief.contains(INVENTED_REDO), "{brief}");
    assert!(!brief.contains("do not start over"), "{brief}");
}

/// Any other sources send-back keeps today's re-pick: the rule's seat, no override, and the sources brief.
#[test]
fn p37_an_ordinary_send_back_keeps_the_rules_seat() {
    let (seat, reason, brief, _) = p37_send_back(
        "p37-keep",
        "flash",
        vec!["https://example.com/not-read".into()],
    );
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    let mut request = nirvana_seat::Request::new("research.web");
    request.size = SizeName::Small;
    let rule = nirvana_seat::pick(&table, &nirvana_seat::live_world(&table), &request)
        .expect("pick")
        .seat;
    nirvana_seat::set_test_world(None);
    assert_eq!(seat, rule);
    assert_eq!(reason, None);
    assert!(brief.contains(SOURCES_REDO), "{brief}");
    assert!(!brief.contains(INVENTED_REDO), "{brief}");
}

fn p37_invented() -> Vec<Axis> {
    vec![Axis::new("sources", Mark::Fail, "the sources")
        .with(vec![format!("{NO_SUCH_VIDEO}{P37_FAKE}")])]
}

/// Review S2 (a): a gate by hand of an order already sent back gives the stored verdict and its child, not an error,
/// and seats nothing more.
#[test]
fn p37_a_gate_by_hand_of_a_sent_back_order_gives_its_child() {
    let sent = p37_gate_fail(
        "p37-again-sent",
        "hydrogen",
        p37_invented(),
        BTreeMap::new(),
    );
    assert_eq!(sent.ruling.verdict, "sent_back");
    let again = sent.again.expect("the stored verdict");
    assert_eq!((again.verdict.as_str(), again.exit()), ("sent_back", 1));
    assert!(
        again.next[0].starts_with("res-s1 seated on carbon"),
        "{:?}",
        again.next
    );
}

/// Review S1: invented sources on the strongest seat of the class (carbon has none above it) are not redone on the
/// rule's pick, the cheap seat that invented reel-motion-gemini's: the owner is asked, and a gate by hand gives
/// the same question.
#[test]
fn p37_invented_sources_with_no_seat_above_ask() {
    let sent = p37_gate_fail("p37-no-above", "carbon", p37_invented(), BTreeMap::new());
    assert_eq!(sent.ruling.verdict, "asked", "{:?}", sent.ruling);
    assert!(sent.child.is_none());
    let question = &sent.ruling.next[0];
    assert!(
        question.contains("no seat of research.web stands above carbon"),
        "{question}"
    );
    let again = sent.again.expect("the stored verdict");
    assert_eq!(again.verdict, "asked");
    assert_eq!(&again.next[0], question);
}

/// Review S1: the stepped seat refused (agy signed out) never falls back to the rule's pick: the owner is asked.
#[test]
fn p37_invented_sources_whose_step_is_refused_ask() {
    let clis = BTreeMap::from([("agy".to_owned(), "agy is signed out".to_owned())]);
    let sent = p37_gate_fail("p37-step-refused", "hydrogen", p37_invented(), clis);
    assert_eq!(sent.ruling.verdict, "asked", "{:?}", sent.ruling);
    assert!(sent.child.is_none());
    assert!(
        sent.ruling.next[0].contains("the seat above it, carbon, is refused"),
        "{:?}",
        sent.ruling.next
    );
}

/// Review N4: on a mixed fail plan 35's rule holds: the other axes' instruction stays, the invented-sources text is
/// added, and nothing of the first try's sources is trusted; alone, the order starts over.
#[test]
fn p37_a_mixed_fail_with_invented_videos_keeps_the_fix_and_distrusts_the_sources() {
    let mut axes = p37_invented();
    axes.push(Axis::new("rebase", Mark::Fail, "conflicts").with(vec!["src/a.rs".into()]));
    let mixed = send_back_brief(
        "o1",
        "o1",
        1,
        &axes,
        "# o1
",
        "nirvana/r1",
    );
    assert!(
        mixed.contains(&format!(
            "fix what the gate found, do not start over. For the sources: {INVENTED_SOURCES}"
        )),
        "{mixed}"
    );
    assert!(!mixed.contains(INVENTED_REDO), "{mixed}");
    assert!(mixed.contains("git rebase nirvana/r1"), "{mixed}");
    let alone = send_back_brief(
        "o1",
        "o1",
        1,
        &p37_invented(),
        "# o1
",
        "nirvana/r1",
    );
    assert!(alone.contains(INVENTED_REDO), "{alone}");
    assert!(!alone.contains("do not start over"), "{alone}");
}

/// Review N7: a reviewer re-seated when its pool ran out, and a send-back of a held-out author, still help their
/// subject: `seat exec` does not gate them as plain orders, and `helps_of` follows the chain to the sidecar.
#[test]
fn p37_a_reseated_or_sent_back_helper_keeps_helps() {
    let root = repo("p37-helps");
    let project = Project::at(&root);
    let journal = project.journal("r1");
    let mut reviewer = seated("o1-rev1", None);
    reviewer.class = "review.adversary".into();
    let mut reseat = seated("o1-rev1-r1", Some("o1-rev1"));
    reseat.class = "review.adversary".into();
    reseat.why = Some("reseat".into());
    let mut author = seated("o1-auth1", None);
    author.class = "acceptance.author".into();
    let mut sent = seated("o1-auth1-s1", Some("o1-auth1"));
    sent.class = "acceptance.author".into();
    for (n, s) in [reviewer, reseat, author, sent].into_iter().enumerate() {
        journal
            .append(n as u64 + 1, Kind::Seated(Box::new(s)))
            .unwrap();
    }
    for (helper, kind) in [("o1-rev1", "review"), ("o1-auth1", "author")] {
        write_sidecar(
            &project,
            "r1",
            &Sidecar {
                order: helper.into(),
                helps: Some((kind.into(), "o1".into())),
                ..Sidecar::default()
            },
        )
        .unwrap();
    }
    let state = replay(&journal.read().0);
    for (child, kind) in [("o1-rev1-r1", "review"), ("o1-auth1-s1", "author")] {
        assert_eq!(
            helps_of(&project, "r1", &state, child),
            Some((kind.to_owned(), "o1".to_owned()))
        );
        assert!(!gates_on_delivery(
            &project,
            "r1",
            child,
            "review.adversary",
            "done"
        ));
    }
    assert_eq!(helps_of(&project, "r1", &state, "o1"), None);
}

/// Plan 37 §1.1: `seat exec` gates every done delivery but a helper's; a lesson review whatever it reported. A
/// blocked one is plan 37b's (`p37b_herdr_gates_a_blocked_delivery_but_never_a_helpers`).
#[test]
fn p37_herdr_gates_each_done_delivery_but_a_helpers() {
    let root = repo("p37-gates");
    let project = Project::at(&root);
    assert!(gates_on_delivery(
        &project,
        "r1",
        "o1",
        "research.web",
        "done"
    ));
    assert!(gates_on_delivery(
        &project,
        "r1",
        "o1",
        "build.bounded",
        "done"
    ));
    assert!(gates_on_delivery(
        &project,
        "r1",
        "o1",
        crate::nirvana_review::CLASS,
        "blocked"
    ));
    write_sidecar(
        &project,
        "r1",
        &Sidecar {
            order: "o1-rev1".into(),
            helps: Some(("review".into(), "o1".into())),
            ..Sidecar::default()
        },
    )
    .unwrap();
    assert!(!gates_on_delivery(
        &project,
        "r1",
        "o1-rev1",
        "review.adversary",
        "done"
    ));
}

/// Plan 37b (the owner, 2026-11-01: "send it blocked as well for validationg"): `seat exec` gates a blocked delivery
/// too; a helper's, blocked or done, stays the orchestrator's (not ruled), and a `needs_answer` one is asked already.
#[test]
fn p37b_herdr_gates_a_blocked_delivery_but_never_a_helpers() {
    let root = repo("p37b-gates");
    let project = Project::at(&root);
    for class in ["research.web", "build.bounded"] {
        assert!(gates_on_delivery(&project, "r1", "o1", class, "blocked"));
        assert!(gates_on_delivery(&project, "r1", "o1", class, "done"));
        assert!(!gates_on_delivery(
            &project,
            "r1",
            "o1",
            class,
            "needs_answer"
        ));
    }
    let journal = project.journal("r1");
    let mut reviewer = seated("o1-rev1", None);
    reviewer.class = "review.adversary".into();
    let mut reseat = seated("o1-rev1-r1", Some("o1-rev1"));
    reseat.class = "review.adversary".into();
    reseat.why = Some("reseat".into());
    let mut author = seated("o1-auth1", None);
    author.class = "acceptance.author".into();
    for (n, s) in [reviewer, reseat, author].into_iter().enumerate() {
        journal
            .append(n as u64 + 1, Kind::Seated(Box::new(s)))
            .unwrap();
    }
    for (helper, kind) in [("o1-rev1", "review"), ("o1-auth1", "author")] {
        write_sidecar(
            &project,
            "r1",
            &Sidecar {
                order: helper.into(),
                helps: Some((kind.into(), "o1".into())),
                ..Sidecar::default()
            },
        )
        .unwrap();
    }
    for (helper, class) in [
        ("o1-rev1", "review.adversary"),
        ("o1-rev1-r1", "review.adversary"),
        ("o1-auth1", "acceptance.author"),
    ] {
        for status in ["blocked", "done"] {
            assert!(
                !gates_on_delivery(&project, "r1", helper, class, status),
                "{helper} {status} was gated"
            );
        }
    }
}

const P37B_BLOCK: &str = "the dial maker's spec sheet is behind a login";

fn p37b_blocked() -> Value {
    serde_json::json!({
        "status": "blocked",
        "summary": P37B_BLOCK,
        "files": [],
        "tests": [],
        "gotchas": ["https://example.com/specs answers 401"],
    })
}

/// Plan 37b: a blocked delivery whose gate finds nothing else is asked, never passed and never sent back. The block
/// opens the question and is the gated line's reason; the verdict queues once, and a gate by hand queues no more.
#[test]
fn p37b_a_blocked_delivery_is_asked_with_its_block_and_queued_once() {
    let report = p37b_blocked();
    let axes = vec![report_axis(Some(&report))];
    assert_eq!(axes[0].mark, Mark::Fail, "{axes:?}");
    let sent = p37_gate_fail_with("p37b-blocked", "flash", axes, BTreeMap::new(), Some(report));
    assert_eq!(sent.ruling.verdict, "asked", "{:?}", sent.ruling);
    assert!(sent.child.is_none());
    let question = &sent.ruling.next[0];
    assert!(
        question.starts_with(&format!(
            "res is blocked: {P37B_BLOCK}. the gate finds nothing else"
        )),
        "{question}"
    );
    let again = sent.again.expect("the stored verdict");
    assert_eq!(again.verdict, "asked");
    assert_eq!(&again.next[0], question);
    // Review N3: the gated line's reason is the block behind a 9-character prefix, so it survives the 90-char cut.
    assert_eq!(sent.reasons, [Some(format!("blocked: {P37B_BLOCK}"))]);
    // Review S4: one wake entry carries the block, the gate's question; the delivered and gated lines are not queued.
    let res: Vec<&String> = sent
        .queue
        .iter()
        .filter(|line| line.starts_with("res "))
        .collect();
    assert_eq!(res.len(), 1, "{:?}", sent.queue);
    assert!(
        res[0].starts_with(&format!("res asks: res is blocked: {P37B_BLOCK}")),
        "{:?}",
        sent.queue
    );
    // Review S1: the record's word is `blocked`, which the seat's tally does not count.
    assert_eq!(sent.outcome, Outcome::BLOCKED);
}

/// Plan 37b: a blocked delivery with invented sources (and a rebase conflict ahead of its report axis) is still
/// asked: the block wins over the send-back and its step-up, the gated line names the block, not the first failing
/// axis, and the question names the invented videos beside it.
#[test]
fn p37b_a_blocked_delivery_with_invented_sources_is_asked_not_stepped_up() {
    let report = p37b_blocked();
    let mut axes = vec![Axis::new("rebase", Mark::Fail, "conflicts").with(vec!["src/a.rs".into()])];
    axes.push(report_axis(Some(&report)));
    axes.extend(p37_invented());
    let sent = p37_gate_fail_with(
        "p37b-blocked-invented",
        "flash",
        axes,
        BTreeMap::new(),
        Some(report),
    );
    assert_eq!(sent.ruling.verdict, "asked", "{:?}", sent.ruling);
    assert!(sent.child.is_none(), "{:?}", sent.child);
    assert_eq!(sent.overrides, 0);
    let question = &sent.ruling.next[0];
    assert!(
        question.starts_with(&format!(
            "res is blocked: {P37B_BLOCK}. the gate also finds "
        )),
        "{question}"
    );
    for found in [
        "rebase: conflicts",
        "sources: the sources",
        "1 cited video(s) do not exist",
    ] {
        assert!(question.contains(found), "no {found:?} in {question}");
    }
    assert_eq!(sent.reasons, [Some(format!("blocked: {P37B_BLOCK}"))]);
}

/// Plan 37b: the block is the report's summary, with its question when it asks one; only a blocked report has one.
#[test]
fn p37b_the_block_is_the_summary_and_its_question() {
    let mut known = chain_state(&[("o1", None)])
        .order("o1")
        .cloned()
        .expect("o1");
    assert_eq!(block_reason(&known), None);
    let mut report = p37b_blocked();
    known.report = Some(report.clone());
    assert_eq!(block_reason(&known).as_deref(), Some(P37B_BLOCK));
    report["question"] = "may I sign in as the owner?".into();
    known.report = Some(report.clone());
    assert_eq!(
        block_reason(&known),
        Some(format!("{P37B_BLOCK} (may I sign in as the owner?)"))
    );
    // Review N2: an empty summary never makes "blocked: ."; the question stands alone, or "no reason given".
    report["summary"] = "".into();
    known.report = Some(report.clone());
    assert_eq!(
        block_reason(&known).as_deref(),
        Some("may I sign in as the owner?")
    );
    report["question"] = " ".into();
    known.report = Some(report.clone());
    assert_eq!(block_reason(&known).as_deref(), Some("no reason given"));
    report["status"] = "done".into();
    known.report = Some(report);
    assert_eq!(block_reason(&known), None);
}

/// Plan 37b review N4: a blocked report's gate does not run the order's `test:` (its tree is unfinished, and a long
/// test only delays the wake); the tests axis says so. A done report's gate runs it as before.
#[test]
fn p37b_a_blocked_gate_notes_the_test_command_and_does_not_run_it() {
    let root = repo("p37b-no-test");
    let cwd = worktree(&root, "mul");
    work(&cwd);
    let spec = spec_with(&["calc.py", "test_*.py"], "git ls-files", &["test_mul"]);
    // The gate writes the test's output to its log only when it ran the test.
    let gate = |report: &Value, log: &Path| {
        deterministic(
            &Subject {
                root: &root,
                cwd: &cwd,
                run: "r1",
                order: "mul",
                branch: Some("nirvana/mul"),
                class: "build.bounded",
                report: Some(report),
                spec: &spec,
                ports: &no_ports,
            },
            Some(log),
        )
    };
    let blocked_log = root.join("blocked-g1.log");
    let det = gate(&p37b_blocked(), &blocked_log);
    let tests = axis(&det, "tests");
    assert_eq!(tests.mark, Mark::Note, "{tests:?}");
    assert_eq!(
        tests.why,
        "`git ls-files` not run: the worker reported blocked"
    );
    assert!(!blocked_log.exists(), "the test ran");
    let done_log = root.join("done-g1.log");
    let det = gate(&done(), &done_log);
    assert_eq!(axis(&det, "tests").mark, Mark::Pass, "{det:?}");
    assert!(done_log.exists());
}

/// Plan 37b review S4: a blocked delivery gated `asked` wakes the orchestrator with one entry, the gate's question,
/// where it queued three (delivered, gated, asks). A blocked delivery whose gate stopped keeps its delivered line, a
/// regate still queues nothing, and a done order's cap ask is as it was (its gated line and its question).
#[test]
fn p37b_one_blocked_delivery_wakes_with_one_entry() {
    let event = |seq: u64, kind: Kind| Event {
        v: V,
        seq,
        at: seq,
        kind,
    };
    let delivered = |order: &str, status: &str| Kind::Delivered {
        order: order.into(),
        report: serde_json::json!({"status": status, "summary": P37B_BLOCK}),
    };
    let gated = |order: &str, reason: &str| Kind::Gated {
        order: order.into(),
        verdict: "asked".into(),
        reason: Some(reason.into()),
        axes: Vec::new(),
    };
    let asked = |order: &str, question: &str| Kind::Asked {
        order: Some(order.into()),
        question: question.into(),
    };
    let events = vec![
        event(1, Kind::Seated(Box::new(seated("b1", None)))),
        event(2, Kind::Seated(Box::new(seated("b2", None)))),
        event(3, Kind::Seated(Box::new(seated("d1", None)))),
        event(4, delivered("b1", "blocked")),
        event(5, gated("b1", &format!("blocked: {P37B_BLOCK}"))),
        event(
            6,
            asked(
                "b1",
                &format!("b1 is blocked: {P37B_BLOCK}. the gate finds nothing else"),
            ),
        ),
        event(7, gated("b1", "a later gate")),
        event(8, delivered("b2", "blocked")),
        event(9, delivered("d1", "done")),
        event(10, gated("d1", "sources: invented")),
        event(11, asked("d1", "d1: 3 send-backs are spent")),
    ];
    let lines: Vec<String> = replay(&events)
        .queue
        .into_iter()
        .map(|item| item.line)
        .collect();
    assert_eq!(
        lines,
        [
            format!("b1 asks: b1 is blocked: {P37B_BLOCK}. the gate finds nothing else"),
            format!("b2 delivered blocked: {P37B_BLOCK}"),
            format!("d1 delivered done: {P37B_BLOCK}"),
            "d1 gated asked: sources: invented".to_owned(),
            "d1 asks: d1: 3 send-backs are spent".to_owned(),
        ]
    );
}

/// Plan 37b: ROLE.md and both skills say a blocked delivery is gated and asked, never sent back.
#[test]
fn p37b_the_docs_say_a_blocked_delivery_is_gated_and_asked() {
    let role = include_str!("../../nirvana/claude/ROLE.md");
    let seat = include_str!("../../nirvana/claude/skills/seat/SKILL.md");
    let gate = include_str!("../../nirvana/claude/skills/gate/SKILL.md");
    for text in [role, seat] {
        assert!(text.contains("herdr gates each `done` or `blocked` delivery itself"));
        assert!(text.contains("then asked, never sent"), "{text}");
        assert!(!text.contains("a `blocked` one"), "{text}");
    }
    assert!(gate.contains("A `blocked` delivery is never sent back"));
    // Review S3: a blocked-asked order is the orchestrator's to resolve first, the owner's only when it cannot, and
    // the gate skill's cap paragraph does not say otherwise.
    assert!(role.contains("A blocked-asked order is yours to resolve first"));
    assert!(role.contains("ask the\n   owner only when you cannot"));
    assert!(gate.contains("a blocked order's question is yours to resolve first"));
    assert!(gate.contains("take it to the owner only when you cannot"));
    // Review S4 and N1: ROLE names the one wake line, and item 5 keeps the file's 120 columns.
    assert!(role.contains("wakes you once as `<order> asks: <order> is blocked: ...`"));
    let at = role.find("5. herdr gates each").expect("item 5");
    assert!(
        p37_item(role, at)
            .lines()
            .all(|line| line.chars().count() <= 120),
        "{}",
        p37_item(role, at)
    );
}

/// The item of a rules list holding byte `at`: from its first line (one that is not indented) through its indented
/// continuation lines.
fn p37_item(text: &str, at: usize) -> &str {
    let start = text[..at]
        .rmatch_indices('\n')
        .map(|(n, _)| n + 1)
        .find(|n| !text[*n..].starts_with(' '))
        .unwrap_or(0);
    let mut end = text[at..].find('\n').map_or(text.len(), |n| at + n);
    while let Some(next) = text.get(end + 1..) {
        if next.starts_with(' ') {
            end = next.find('\n').map_or(text.len(), |n| end + 1 + n);
        } else {
            break;
        }
    }
    &text[start..end]
}

/// The plan 37 §1.4 rules: each in ROLE.md or the seat skill, each naming the MotoProject failure it answers.
#[test]
fn p37_the_agy_brief_rules_are_in_role_and_the_seat_skill_each_naming_its_failure() {
    let role = include_str!("../../nirvana/claude/ROLE.md");
    let skill = include_str!("../../nirvana/claude/skills/seat/SKILL.md");
    for (text, rule, failure) in [
        (
            role,
            "herdr gates each `done` or `blocked` delivery itself",
            "1 of 26 deliveries",
        ),
        (
            role,
            "says how to read each source and how done is checked",
            "invented 12 of 12",
        ),
        (role, "Never promise", "YouTube connector"),
        (role, "--profile web-researcher", "--from web-researcher"),
        (
            skill,
            "Never cite a URL from memory or from a search result.",
            "invented",
        ),
        (skill, "%(id)s|%(title)s|%(channel)s", "9 steps"),
        (skill, "/.rss", "10 failed Reddit reads"),
        (
            skill,
            "bladebro when the order was seated with `--needs bladebro`",
            "no YouTube connector",
        ),
        (
            skill,
            "ends with a command whose output goes into `tests`",
            "prose check",
        ),
        (skill, "`owns:` in lowercase", "`Owns:`"),
        (
            skill,
            "The project rules the order touches, quoted",
            "6 mp4s on C:",
        ),
        (skill, "long steps run in the foreground", "stalled 600 s"),
        (
            skill,
            "The current state as absolute paths",
            "10-slide post",
        ),
        (skill, "Explore, read, write, verify", "opened nothing"),
    ] {
        let at = text
            .find(rule)
            .unwrap_or_else(|| panic!("no rule {rule:?}"));
        let item = p37_item(text, at);
        assert!(
            item.contains(failure),
            "{rule:?} names no {failure:?}: {item}"
        );
    }
    let gate = skill
        .split(
            "

",
        )
        .find(|part| part.contains("herdr gates each `done` or `blocked` delivery itself"))
        .expect("the seat skill's gate paragraph");
    assert!(
        gate.contains("one seat up") && gate.contains("trusts nothing"),
        "{gate}"
    );
    // Review S3: both say what the wake's line is, as `nirvana_run`'s queue writes it
    // (`p37_the_first_verdict_after_a_delivery_queues_whatever_was_woken` pins that side).
    for text in [role, skill] {
        assert!(
            text.contains("`<order> gated <verdict>: <reason>`"),
            "{text}"
        );
    }
    // Review N5: bladebro is promised to an agy worker only when its order was seated with `--needs bladebro`.
    assert!(!skill.contains("when the order needs a browser"));
    for row in [
        "- [ ] the verify line",
        "- [ ] lowercase `owns:`",
        "- [ ] a read method per source type, only real tools",
        "- [ ] a \"done means\" that ends in a command whose output goes in `tests`",
        "`--profile web-researcher` when the project has it",
    ] {
        assert!(skill.contains(row), "no checklist row {row:?}");
    }
}

/// Plan 37 §1.1: a gate that books a reviewer waits, and its `herdr seat run` line went to `seat exec`'s view, read by
/// no one; a gate by hand gives the line again while the reviewer is not seated, and says how when its booking is gone.
#[test]
fn p37_a_gate_by_hand_gives_a_booked_reviewers_line_again() {
    let _home = crate::nirvana::with_test_home("p37-review-line");
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let root = repo("p37-review-line");
    let project = Project::at(&root);
    let cwd = worktree(&root, "lb");
    work(&cwd);
    let text = "# Add mul

owns: calc.py test_mul.py
test: git ls-files
check: test_mul
";
    let order_file = crate::nirvana_worker::write_order(&project, &cwd, "lb", text).expect("order");
    let journal = project.journal("r1");
    let mut order = seated("lb", None);
    order.class = "build.load_bearing".into();
    order.cwd = cwd.display().to_string();
    order.worktree = Some(cwd.display().to_string());
    order.branch = Some("nirvana/lb".into());
    order.order_file = order_file;
    journal
        .append(1, Kind::Seated(Box::new(order)))
        .expect("seated");
    journal
        .append(
            2,
            Kind::Delivered {
                order: "lb".into(),
                report: done(),
            },
        )
        .expect("delivered");
    let first = gate(&project, "r1", "lb").expect("gate");
    assert_eq!(first.verdict, "wait", "{first:?}");
    let line = first.next.first().expect("the reviewer's line").clone();
    assert!(line.starts_with("herdr seat run "), "{line}");
    let again = gate(&project, "r1", "lb").expect("gate again");
    assert_eq!(again.verdict, "wait", "{again:?}");
    assert_eq!(again.next, vec![line.clone()]);
    std::fs::remove_file(nirvana_seat::dir().join(nirvana_seat::LEASES_FILE)).expect("leases");
    let gone = gate(&project, "r1", "lb").expect("gate with no booking");
    assert!(
        gone.next[0].starts_with("lb-rev1's booking is gone"),
        "{gone:?}"
    );
    nirvana_seat::set_test_world(None);
}

// ---- live checks 2026-11-01 F2 and F3: sources only when the order asks for the web; a child keeps the effort ----

/// lc4's order as the orchestrator wrote it, and lc3's: neither asks for the web.
const GSRC_LC4: &str = "Without opening, reading or listing any file, quote verbatim every numbered rule this \
project's rules give agents, and name the file the rules say they came from. If you were given no project rules, say \
exactly that.\n";
const GSRC_LC3: &str = "List every MCP server and MCP tool you have, by name. Then call the livetools server's lc_ping \
tool with word \"hi\" and report its exact answer.\n";

fn gsrc_needs(needs: &[&str]) -> Vec<String> {
    needs.iter().map(|need| (*need).to_owned()).collect()
}

/// F2: what asks for the web is the class research.web, a need of browser, bladebro or web, or a URL or the web named
/// in the order as written; lc3 and lc4 ask for none of it, and only research and media orders get a sources line.
#[test]
fn gsrc_the_web_is_asked_by_the_class_a_need_or_the_order_text() {
    let none: Vec<String> = Vec::new();
    for (class, needs, text, asked) in [
        ("research.web", &[][..], GSRC_LC4, true),
        ("research.read", &["browser"][..], GSRC_LC4, true),
        ("research.read", &["bladebro"][..], GSRC_LC4, true),
        ("media.video", &["web"][..], GSRC_LC4, true),
        (
            "research.read",
            &[][..],
            "Summarise https://example.com/spec.\n",
            true,
        ),
        (
            "research.read",
            &[][..],
            "Search the web for the H2C's nozzle sizes.\n",
            true,
        ),
        (
            "media.video",
            &[][..],
            "Describe the YouTube video the owner names.\n",
            true,
        ),
        ("research.read", &[][..], GSRC_LC4, false),
        ("research.read", &[][..], GSRC_LC3, false),
        ("research.read", &["video", "xtool"][..], GSRC_LC3, false),
        (
            "media.video",
            &[][..],
            "Cut E:/clips/a.mp4 at 00:12.\n",
            false,
        ),
    ] {
        let got = web_asked(class, &gsrc_needs(needs), text);
        assert_eq!(got.is_some(), asked, "{class} {needs:?} {text:?}: {got:?}");
        let line = sources_brief_line(class, &gsrc_needs(needs), text);
        assert_eq!(line, Some(if asked { WEB_SOURCES } else { NO_WEB_SOURCES }));
    }
    assert_eq!(
        web_asked("research.read", &gsrc_needs(&["browser"]), GSRC_LC4).as_deref(),
        Some("the order needs browser")
    );
    assert_eq!(
        sources_brief_line("build.bounded", &none, "https://x"),
        None
    );
    assert_eq!(sources_brief_line("chore.script", &none, GSRC_LC4), None);
}

/// Review S3 and S5: each word, an order it catches and one it must not (the record's table). Only research and media
/// orders get a line, so the orders that must not be caught are research orders about the project or this machine.
#[test]
fn gsrc_the_words_catch_lookups_and_skip_local_hosts_and_code() {
    let none: Vec<String> = Vec::new();
    for (word, catches, skips) in [
        (
            "url",
            "Summarise https://example.com/spec.",
            "Call http://localhost:8770/api/runs and list the run ids.",
        ),
        (
            "url 127",
            "Read http://docs.example.org/a.",
            "Quote what http://127.0.0.1:8770/health answers.",
        ),
        (
            "url ::1",
            "Read https://[2001:db8::2]/notes.",
            "Quote what http://[::1]:8770/health answers.",
        ),
        (
            "domain",
            "Read github.com/fahim-mygithub/nirvana-herdr/issues and list the open bugs.",
            "List what nirvana/controls.d/gsrc.py controls.",
        ),
        (
            "domain docs.rs",
            "Check docs.rs/serde/latest for the derive attributes.",
            "Read ./src/nirvana_gate.rs and list its consts.",
        ),
        (
            "the web",
            "Search the web for the H2C's nozzle sizes.",
            "Summarise the webhook handler's retry rule.",
        ),
        (
            "youtube",
            "Describe the YouTube video the owner names.",
            "Describe the clip at E:/clips/a.mp4.",
        ),
        (
            "tiktok",
            "Summarise the owner's last three TikTok posts.",
            "Summarise the owner's last three commits.",
        ),
        (
            "look up",
            "Look up the Bambu H2C's build volume.",
            "Look up the commit rule in CLAUDE.md and quote it.",
        ),
        (
            "search for",
            "Search for the xTool F2 Ultra's laser power.",
            "Search for every caller of seat_once in src/ and list them.",
        ),
        (
            "latest price",
            "Give the latest price of a 1 kg PLA spool from Bambu.",
            "Give the latest price row in nirvana/plans/costs.md.",
        ),
        (
            "reviews of",
            "Collect reviews of the Prusa Core One.",
            "Collect the reviews of P37 in the plans folder.",
        ),
        (
            "podcast",
            "Summarise this week's Lex Fridman podcast episode.",
            "Transcribe the podcast at E:/pod.mp3.",
        ),
        (
            "'s channel",
            "List the five newest uploads on Veritasium's channel.",
            "Name the channel the worker sends its rows on in the code.",
        ),
        (
            "transcript of",
            "Give the transcript of the Veritasium video on the Monty Hall problem.",
            "Give the transcript of meeting.m4a.",
        ),
        (
            "website",
            "Read the vendor's website for the warranty terms.",
            "Explain how the website folder's build script bundles its pages.",
        ),
        (
            "internet",
            "Find the answer on the internet.",
            "Explain how the internet-facing port is chosen in the code.",
        ),
    ] {
        let caught = web_asked("research.read", &none, catches);
        assert!(caught.is_some(), "{word} must catch {catches:?}");
        let skipped = web_asked("research.read", &none, skips);
        assert!(
            skipped.is_none(),
            "{word} must not catch {skips:?}: {skipped:?}"
        );
    }
    assert_eq!(
        web_asked(
            "research.read",
            &none,
            "Call http://localhost:8770/a and https://x.org/b."
        )
        .as_deref(),
        Some("the order names https://x.org/b")
    );
    // A word is a word: `the web` is not in `the webhook`, and a plural still counts.
    assert_eq!(
        web_asked("research.read", &none, "Summarise two podcasts on the H2C."),
        Some("the order says `podcast`".into())
    );
    // A local mark spares only its own sentence: the lookup in the first one still asks for the web.
    assert_eq!(
        web_asked(
            "research.read",
            &none,
            "Look up the H2C's build volume. Write it to notes.md."
        ),
        Some("the order says `look up`".into())
    );
}

/// Review S4: what the worker did decides first. On an order herdr found no web in, a worker that opened pages and
/// cites none fails; one that opened only this machine, or nothing, gets a note. The remaining gap: a current-fact
/// order the words miss, answered from memory with no read, passes with a note.
#[test]
fn gsrc_a_no_web_order_that_opened_pages_must_cite_them() {
    let empty = serde_json::json!({"status": "done", "sources": []});
    let curl = |url: &str| {
        serde_json::json!({"message": {"content": [{"type": "tool_use", "id": "t1", "name": "Bash",
            "input": {"command": format!("curl -s {url}")}}]}})
        .to_string()
    };
    let fetched = curl("https://www.bambulab.com/en/h2c");
    let axis = sources_axis("research.read", None, Some(&empty), &fetched).expect("axis");
    assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
    assert!(axis.why.contains("opened 1 page(s)"), "{axis:?}");
    assert!(axis.lines[0].contains("bambulab.com"), "{axis:?}");
    let axis = sources_axis("research.read", None, None, &fetched).expect("axis");
    assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
    let browsed = serde_json::json!({"message": {"content": [{"type": "tool_use", "id": "t2",
        "name": "mcp__bladebro__act", "input": {"action": "click", "text": "Specs"}}]}})
    .to_string();
    let axis = sources_axis("research.read", None, Some(&empty), &browsed).expect("axis");
    assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
    let local = curl("http://localhost:8770/api/runs");
    let axis = sources_axis("research.read", None, Some(&empty), &local).expect("axis");
    assert_eq!(axis.mark, Mark::Note, "{axis:?}");
    // The gap, written down: the words miss "cost today", and a memory answer opens nothing.
    let memory = "What does the Bambu H2C cost today?\n";
    assert_eq!(web_asked("research.read", &[], memory), None);
    let answered =
        serde_json::json!({"status": "done", "summary": "about 2,399 USD", "sources": []});
    let axis = sources_axis("research.read", None, Some(&answered), "").expect("axis");
    assert_eq!(axis.mark, Mark::Note, "{axis:?}");
    // A no-web order that cites the page it opened passes, as any order does.
    let cited = serde_json::json!({"status": "done",
        "sources": [{"url": "https://www.bambulab.com/en/h2c", "used_for": "price"}]});
    let axis = sources_axis("research.read", None, Some(&cited), &fetched).expect("axis");
    assert_eq!(axis.mark, Mark::Pass, "{axis:?}");
}

/// F2: an order herdr seated with `NO_WEB_SOURCES` that cites nothing gets a note; one that cites a page it never
/// opened still fails, and a web order or one seated before the rule still fails on nothing cited.
#[test]
fn gsrc_an_order_that_asks_no_web_and_cites_nothing_is_a_note() {
    let no_web = spec(&format!("{GSRC_LC4}\n\n{NO_WEB_SOURCES} The agy line.\n"));
    assert!(no_web.no_web);
    // Review N3: a copy re-wrapped after seating keeps the short mark on its first line.
    let wrapped = NO_WEB_SOURCES.replacen(" so the gate", "\nso the gate", 1);
    assert!(spec(&format!("{GSRC_LC4}\n\n{wrapped}\n")).no_web);
    assert!(!spec(&format!("{GSRC_LC4}\n\n{WEB_SOURCES}\n")).no_web);
    assert_eq!(web_of("research.read", &no_web), None);
    // The class wins over the line, and an order with neither line asks for the web, as before the rule.
    assert!(web_of("research.web", &no_web).is_some());
    assert!(web_of("research.read", &Spec::default()).is_some());
    let empty = serde_json::json!({"status": "done", "sources": []});
    let silent = serde_json::json!({"status": "done"});
    for report in [Some(&empty), Some(&silent), None] {
        let axis = sources_axis("research.read", None, report, "").expect("axis");
        assert_eq!(axis.mark, Mark::Note, "{axis:?}");
        assert!(axis.why.starts_with("no web asked; none cited"), "{axis:?}");
        let axis = sources_axis("research.read", Some("asked"), report, "").expect("axis");
        assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
    }
    let unread = serde_json::json!({"status": "done", "sources": [{"url": "https://example.com", "used_for": "x"}]});
    let axis = sources_axis("research.read", None, Some(&unread), "").expect("axis");
    assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
    assert_eq!(axis.lines, vec!["https://example.com".to_owned()]);
    // Through the gate's own axes: the note never fails the order.
    let root = repo("gsrc-det");
    let det = deterministic(
        &Subject {
            root: &root,
            cwd: &root,
            run: "r1",
            order: "lc4",
            branch: None,
            class: "research.read",
            report: Some(&empty),
            spec: &no_web,
            ports: &no_ports,
        },
        None,
    );
    let sources = det
        .axes
        .iter()
        .find(|found| found.axis == "sources")
        .expect("sources");
    assert_eq!(sources.mark, Mark::Note, "{sources:?}");
    assert!(!failing(&det.axes).iter().any(|axis| axis.axis == "sources"));
}

/// F2: a send-back of an order that asks for no web, on any axis, tells the worker no source is wanted; a web order's
/// brief does not say it.
#[test]
fn gsrc_a_send_back_of_a_no_web_order_wants_no_fetch() {
    let axes = vec![Axis::new("report", Mark::Fail, "status blocked")];
    let no_web = format!("{GSRC_LC4}\n\n{NO_WEB_SOURCES}\n");
    let brief = send_back_brief("lc4", "lc4", 1, &axes, &no_web, "nirvana-integrate/r1");
    assert!(brief.contains(NO_WEB_REDO), "{brief}");
    // Review S2: neither line talks a worker out of a fetch the answer needs, nor into one it does not.
    for line in [NO_WEB_SOURCES, NO_WEB_REDO] {
        assert!(
            line.contains("needs a page you do not have, open it and cite it"),
            "{line}"
        );
        assert!(
            line.contains("never open one only to have a source"),
            "{line}"
        );
        assert!(!line.contains("asks for no web page"), "{line}");
    }
    let web = format!("{GSRC_LC4}\n\n{WEB_SOURCES}\n");
    let brief = send_back_brief("lc4", "lc4", 1, &axes, &web, "nirvana-integrate/r1");
    assert!(!brief.contains(NO_WEB_REDO), "{brief}");
}

/// The child a failing gate of `class` on `seat` at `effort` seats, with `axes`.
fn gsrc_send_back(
    name: &str,
    class: &str,
    seat: &str,
    effort: Effort,
    axes: Vec<Axis>,
) -> (Ruling, Seated) {
    let _guard = crate::nirvana::with_test_home(name);
    let root = repo(name);
    let project = Project::at(&root);
    let run = "r1";
    let journal = project.journal(run);
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    let mut parent = seated("res", None);
    parent.class = class.into();
    parent.chain = Some("research".into());
    parent.seat = seat.into();
    parent.cli = "agy".into();
    parent.model = table.seat(seat).expect("seat").model_for(effort);
    parent.effort = effort;
    parent.cwd = root.display().to_string();
    journal
        .append(1, Kind::Seated(Box::new(parent)))
        .expect("seated");
    let _live = crate::nirvana_worker::take_live(&project, run, "res-s1").unwrap();
    let state = replay(&journal.read().0);
    let known = state.order("res").expect("order").clone();
    let ruling = send_back(
        &project,
        run,
        &journal,
        &state,
        &known,
        "res",
        "# res\n",
        axes,
        Some(&table),
    )
    .expect("send_back");
    let child = replay(&journal.read().0)
        .order("res-s1")
        .map(|child| child.seated.clone())
        .expect("child");
    nirvana_seat::set_test_world(None);
    (ruling, child)
}

/// F3: lc4 ran research.read at `--effort low` and its send-back was seated at medium, the class's effort. The child
/// now runs at the parent's.
#[test]
fn gsrc_a_send_back_keeps_the_parents_effort() {
    // The rule alone gives research.read medium: the test has an effort to keep.
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    let mut request = nirvana_seat::Request::new("research.read");
    request.size = SizeName::Small;
    let rule =
        nirvana_seat::pick(&table, &nirvana_seat::live_world(&table), &request).expect("pick");
    nirvana_seat::set_test_world(None);
    assert_eq!(rule.effort, Effort::Medium, "{}", rule.reason);
    let axes = vec![Axis::new("report", Mark::Fail, "status blocked")];
    let (ruling, child) = gsrc_send_back("gsrc-keep", "research.read", "flash", Effort::Low, axes);
    assert_eq!(child.effort, Effort::Low, "{}", child.reason);
    let seat = table.seat(&child.seat).expect("seat");
    assert_eq!(child.model, seat.model_for(Effort::Low));
    assert!(
        child.reason.contains("effort low kept from the parent"),
        "{}",
        child.reason
    );
    assert!(ruling.next[0].contains(" low ("), "{:?}", ruling.next);
}

/// F3 and plan 37 §1.3: the step-up names a seat, not an effort, so the stronger seat runs at the parent's effort.
#[test]
fn gsrc_a_stepped_up_redo_keeps_the_parents_effort() {
    let axes = vec![Axis::new("sources", Mark::Fail, "the sources")
        .with(vec![format!("{NO_SUCH_VIDEO}{P37_FAKE}")])];
    let (_, child) = gsrc_send_back("gsrc-step", "research.web", "hydrogen", Effort::High, axes);
    assert_eq!(child.seat, "carbon", "{}", child.reason);
    assert_eq!(child.effort, Effort::High, "{}", child.reason);
}

/// A child whose seat does not offer the parent's effort runs at the nearest it offers below, and the reason says so.
#[test]
fn gsrc_a_seat_without_the_parents_effort_keeps_the_picks() {
    nirvana_seat::set_test_world(Some(BTreeMap::new()));
    let table = nirvana_seat::parse(nirvana_seat::DEFAULT).expect("table");
    let mut request = nirvana_seat::Request::new("research.read");
    request.override_seat = Some(("haiku".into(), "test".into()));
    request.effort = Some(Effort::High);
    let pick =
        nirvana_seat::pick(&table, &nirvana_seat::live_world(&table), &request).expect("pick");
    nirvana_seat::set_test_world(None);
    assert_eq!(pick.seat, "haiku");
    assert_eq!(pick.effort, Effort::Medium);
    assert!(
        pick.reason
            .contains("the parent's effort high; haiku has no high; medium"),
        "{}",
        pick.reason
    );
}

/// The docs state the sources rule as herdr applies it.
#[test]
fn gsrc_the_docs_say_sources_only_when_the_order_asks_for_the_web() {
    let role = include_str!("../../nirvana/claude/ROLE.md");
    let gate = include_str!("../../nirvana/claude/skills/gate/SKILL.md");
    assert!(role.contains("herdr asks an order for web sources only when it asks for the web"));
    assert!(role.contains("opened no page gets a note, never"));
    assert!(role.contains(
        "one that opened pages must cite them, and a source it does cite is still checked"
    ));
    assert!(gate.contains("must cite its sources in the Work"));
    assert!(gate.contains("citing nothing with no page\n  opened is a note"));
    assert!(gate
        .contains("one that asks for the web, or whose worker opened a page, cites at least one"));
    assert!(!gate.contains("their Work Report must carry a `sources` array"));
}

// ---- plan 40 (c40b): the review's B findings, T1 and the addenda ----

fn seated_why(order: &str, parent: Option<&str>, why: &str) -> Seated {
    let mut seated = seated(order, parent);
    seated.why = parent.map(|_| why.to_owned());
    seated
}

fn chain_state_why(orders: &[(&str, Option<&str>, &str)]) -> RunState {
    let events: Vec<Event> = orders
        .iter()
        .enumerate()
        .map(|(index, (order, parent, why))| Event {
            v: V,
            seq: index as u64 + 1,
            at: 100 + index as u64,
            kind: Kind::Seated(Box::new(seated_why(order, *parent, why))),
        })
        .collect();
    replay(&events)
}

/// Review B1: a reseat child belongs to its parent's chain (the root's held-out check and snapshot are found), and a
/// reseat is no send-back (the cap counts send-backs only, across the reseat).
#[test]
fn c40b_a_reseat_child_is_part_of_its_parents_chain() {
    let state = chain_state_why(&[("o1", None, ""), ("o1-r1", Some("o1"), "reseat")]);
    assert_eq!(chain(&state, "o1-r1"), ("o1".into(), 0));
    let state = chain_state_why(&[
        ("o1", None, ""),
        ("o1-s1", Some("o1"), "send_back"),
        ("o1-s1-r1", Some("o1-s1"), "reseat"),
        ("o1-s2", Some("o1-s1-r1"), "send_back"),
    ]);
    assert_eq!(chain(&state, "o1-s2"), ("o1".into(), 2));
    // The next send-back is the third, named from the root.
    assert_eq!(
        plan_send_back(&state, "o1-s2"),
        SendBack::Child {
            id: "o1-s3".into(),
            n: 3
        }
    );
    // Any other why still ends the chain.
    let state = chain_state_why(&[("o1", None, ""), ("o1-x", Some("o1"), "teach")]);
    assert_eq!(chain(&state, "o1-x"), ("o1-x".into(), 0));
}

/// Review B2: a test that times out ends with its whole tree. `cmd` starts `ping`, which holds the pipes; with `cmd`
/// alone killed, or the test outside the job, the gate waited the full 29 s (and, for a hung binary, for ever).
#[cfg(windows)]
#[test]
fn c40b_a_timed_out_test_is_killed_with_its_whole_tree() {
    let cwd = temp("tree");
    let started = std::time::Instant::now();
    let run = run_command(&cwd, "cmd /C ping -n 30 127.0.0.1", Duration::from_secs(1));
    let took = started.elapsed();
    assert!(run.timed_out, "{run:?}");
    assert!(took < Duration::from_secs(15), "took {took:?}: {run:?}");
}

/// Review B4: a named check that passed early survives 60 later `PASSED` lines, and is judged on the whole output.
#[test]
fn c40b_a_check_that_ran_early_is_not_cut_by_later_passed_lines() {
    let checks = vec!["test_gate_lock".to_owned()];
    let mut output = "test_gate_lock ... ok\n".to_owned();
    for n in 0..60 {
        output.push_str(&format!("tests/test_{n}.py::case PASSED\n"));
    }
    let run = CommandRun {
        exit: Some(0),
        output: output.clone(),
        ..CommandRun::default()
    };
    let axis = tests_axis(Some("pytest -v"), Some(&run), &checks);
    assert_eq!(axis.mark, Mark::Pass, "{axis:?}");
    let kept = filter(&output, &checks);
    assert_eq!(kept.len(), KEPT_LINES);
    assert_eq!(kept[0], "test_gate_lock ... ok");
    assert_eq!(kept[KEPT_LINES - 1], "tests/test_59.py::case PASSED");
    // A check absent from the whole output still fails (Nirvana CLAUDE.md rule 9).
    let absent = CommandRun {
        exit: Some(0),
        output: output.replace("test_gate_lock", "test_other"),
        ..CommandRun::default()
    };
    assert_eq!(
        tests_axis(Some("pytest -v"), Some(&absent), &checks).mark,
        Mark::Fail
    );
    // The shown lines are clipped at 300 characters; a check named past the clip still ran.
    let long = CommandRun {
        exit: Some(0),
        output: format!("{} test_gate_lock ok\n", "x".repeat(320)),
        ..CommandRun::default()
    };
    assert_eq!(
        tests_axis(Some("pytest -v"), Some(&long), &checks).mark,
        Mark::Pass
    );
}

/// Review round (N2): the pinned line is the check's result, not a compile line that names it first.
#[test]
fn c40b_the_pinned_check_line_is_its_result() {
    let checks = vec!["c40b_probe".to_owned()];
    let mut output = "   Compiling c40b_probe v0.1.0\ntest c40b_probe ... ok\n".to_owned();
    for n in 0..60 {
        output.push_str(&format!("tests/test_{n}.py::case PASSED\n"));
    }
    let kept = filter(&output, &checks);
    assert_eq!(kept.len(), KEPT_LINES);
    assert_eq!(kept[0], "test c40b_probe ... ok", "{kept:?}");
    assert!(!kept.iter().any(|line| line.contains("Compiling")), "{kept:?}");
    // A name holding a result word (`fail`) is not its own result.
    let checks = vec!["test_fail_case".to_owned()];
    let output = format!(
        "collecting test_fail_case\ntest_fail_case PASSED\n{}",
        "tests/x.py::y PASSED\n".repeat(60)
    );
    assert_eq!(filter(&output, &checks)[0], "test_fail_case PASSED");
}

/// Review B5: an untracked file with a non-ASCII name is committed (git quoted it without `-z`).
#[test]
fn c40b_a_non_ascii_new_file_is_committed() {
    let root = repo("cafe");
    let cwd = worktree(&root, "cafe");
    std::fs::write(cwd.join("caf\u{e9}.md"), "menu\n").expect("cafe");
    let left = commit_work(&cwd, "cafe", &["*.md".to_owned()]).expect("commit");
    assert!(left.is_empty(), "{left:?}");
    let files = sh(
        &cwd,
        &["-c", "core.quotepath=false", "show", "--name-only", "--format=", "HEAD"],
    );
    assert_eq!(files.lines().collect::<Vec<_>>(), vec!["caf\u{e9}.md"]);
}

/// Review B6: `Test:`, `Check:` and `Serves:` are read as `owns:` is, the case ignored.
#[test]
fn c40b_test_check_and_serves_ignore_case() {
    let spec = spec("Test: cargo nextest run\nCHECK: gate_lock\nServes: 8770\nOwns: a.rs");
    assert_eq!(spec.test.as_deref(), Some("cargo nextest run"));
    assert_eq!(spec.checks, vec!["gate_lock".to_owned()]);
    assert_eq!(spec.serves, vec![8770]);
    assert_eq!(spec.owns, vec!["a.rs".to_owned()]);
}

/// Review round (N1): a `check:` line in any case is names only when it looks like names; prose stays prose. `test:`
/// and `serves:` keep the simple rule.
#[test]
fn c40b_a_check_line_of_prose_names_no_check() {
    for prose in [
        "Check: the page loads",
        "check: the form submits and the toast shows",
        "Check: everything.",
        "CHECK: test_mul and test_add",
    ] {
        assert!(spec(prose).checks.is_empty(), "{prose}");
    }
    for (names, want) in [
        ("check: test_mul test_add", &["test_mul", "test_add"][..]),
        ("Check: gate::lock, c40b-a", &["gate::lock", "c40b-a"][..]),
        ("CHECK: smoke", &["smoke"][..]),
        ("Check: `alpha` `beta`", &["alpha", "beta"][..]),
    ] {
        assert_eq!(spec(names).checks, want, "{names}");
    }
    // `test:` and `serves:` are read in any case, as B6 made them.
    let spec = spec("TEST: cargo test the thing\nSERVES: 8770 8771");
    assert_eq!(spec.test.as_deref(), Some("cargo test the thing"));
    assert_eq!(spec.serves, vec![8770, 8771]);
}

/// Plan 40 T1: a file list is capped at 40 lines and one `+N more`.
#[test]
fn c40b_file_lists_are_capped_with_a_count_of_the_rest() {
    let changed: Vec<String> = (0..100).map(|n| format!("f{n}.rs")).collect();
    let axis = owned_axis("build.bounded", &changed, &["a.rs".to_owned()]);
    assert_eq!(axis.mark, Mark::Fail);
    assert_eq!(axis.lines.len(), FILE_LINES_MOST + 1);
    assert_eq!(axis.lines.last().map(String::as_str), Some("+60 more"));
    assert!(axis.why.contains("100"), "{}", axis.why);
    assert_eq!(capped(vec!["a".into()]), vec!["a".to_owned()]);
}

/// Plan 40 addendum A: an accepted order counts as a pass in the seat's tally.
#[test]
fn c40b_an_accepted_order_counts_as_a_pass() {
    assert_eq!(Outcome::tally(Outcome::ACCEPTED), Some(true));
    assert_eq!(Outcome::tally("failed"), Some(false));
    assert_eq!(Outcome::tally(Outcome::BLOCKED), None);
}

/// An answered agy step, as the real fixtures hold them (`state: DONE`).
fn agy_tool(index: u64, tool: &str, parameters: Value) -> String {
    agy_step(index, "DONE", tool, parameters, "")
}

fn agy_step(index: u64, state: &str, tool: &str, parameters: Value, output: &str) -> String {
    serde_json::json!({"event": "step_update", "step_update": {"step_index": index, "state": state, "step_type": "tool",
        "tool_name": tool, "tool_info": {"name": tool, "parameters": parameters, "output": output}}})
    .to_string()
}

/// A Claude tool call and its answer.
fn claude_call(id: &str, tool: &str, input: Value, is_error: bool) -> String {
    [
        serde_json::json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": id, "name": tool, "input": input}]}}),
        serde_json::json!({"type": "user", "message": {"content": [
            {"type": "tool_result", "tool_use_id": id, "content": "x", "is_error": is_error}]}}),
    ]
    .map(|line| line.to_string())
    .join("\n")
}

fn cites(urls: &[&str]) -> Value {
    serde_json::json!({"status": "done", "summary": "s", "gotchas": [],
        "sources": urls.iter().map(|url| serde_json::json!({"url": url, "used_for": "the video"})).collect::<Vec<_>>()})
}

/// Plan 40 addendum B, from the three real shapes of 2026-10-31 (paths shortened, no personal content). yt-breakdown:
/// the cited video's downloaded copy was opened and probed by path, and no fetch named its URL.
#[test]
fn c40b_a_media_order_cites_the_video_it_opened_by_path() {
    let kit = r"C:\Users\me\AppData\Local\groove-authoring\youtube\8aQ8SoQl5EY";
    let probe = format!(r#"python -c "video_path = r'{kit}\proxy.mp4'""#);
    let finish = agy_tool(
        150,
        "finish",
        serde_json::json!({"sources": [{"url": "https://www.youtube.com/watch?v=8aQ8SoQl5EY"}]}),
    );
    let notes = [
        agy_tool(6, "read_file", serde_json::json!({"Path": format!(r"{kit}\ingest.json")})),
        agy_tool(35, "run_command", serde_json::json!({ "CommandLine": probe })),
        finish.clone(),
    ]
    .join("\n");
    let report = cites(&["https://www.youtube.com/watch?v=8aQ8SoQl5EY"]);
    // Review round (S1): its notes file and a script that names the path are no sign it watched the video.
    let axis = sources_axis("media.video", None, Some(&report), &notes).expect("an axis");
    assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
    // Its `read_file` of the downloaded proxy, answered: the video was opened.
    let stream = format!(
        "{notes}\n{}",
        agy_tool(9, "read_file", serde_json::json!({"Path": format!(r"{kit}\proxy.mp4")}))
    );
    let axis = sources_axis("media.video", None, Some(&report), &stream).expect("an axis");
    assert_eq!(axis.mark, Mark::Pass, "{axis:?}");
    // The real streams, whole: each cited video passes under media.video.
    for (order, fixture, video) in [
        ("yt-breakdown", include_str!("fixtures/yt-breakdown.jsonl"), "8aQ8SoQl5EY"),
        ("yt-breakdown-illit", include_str!("fixtures/yt-breakdown-illit.jsonl"), "KuKVOfwBPQI"),
    ] {
        let (_, real) = fixture.split_once('\n').expect("head line");
        let report = cites(&[&format!("https://www.youtube.com/watch?v={video}")]);
        let axis = sources_axis("media.video", None, Some(&report), real).expect("an axis");
        assert_eq!(axis.mark, Mark::Pass, "{order}: {axis:?}");
    }
    // research.* keeps its rule: the same stream does not vouch for the page.
    let axis = sources_axis("research.web", None, Some(&report), &stream).expect("an axis");
    assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
    // The report's own `finish` call never vouches.
    let axis = sources_axis("media.video", None, Some(&report), &finish).expect("an axis");
    assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
}

/// yt-breakdown-illit: the video passes by its kit; the two tempo pages it never fetched still fail, until a command
/// names one.
#[test]
fn c40b_a_media_order_still_fails_pages_it_never_fetched() {
    let kit = r"C:\Users\me\AppData\Local\groove-authoring\youtube\KuKVOfwBPQI";
    let stream = agy_tool(
        10,
        "read_file",
        serde_json::json!({"Path": format!(r"{kit}\contact_sheet.jpg")}),
    );
    let tunebat = "https://tunebat.com/Info/Do-the-Dance/6A1s2x4N6C9w7Y6tH7xX7P";
    let songbpm = "https://songbpm.com/@illit/do-the-dance";
    let report = cites(&["https://www.youtube.com/watch?v=KuKVOfwBPQI", tunebat, songbpm]);
    let axis = sources_axis("media.video", None, Some(&report), &stream).expect("an axis");
    assert_eq!(axis.mark, Mark::Fail, "{axis:?}");
    assert_eq!(axis.lines, vec![tunebat.to_owned(), songbpm.to_owned()]);
    let fetched = format!(
        "{stream}\n{}",
        agy_tool(
            11,
            "run_command",
            serde_json::json!({ "CommandLine": format!("ffprobe {songbpm}") })
        )
    );
    let axis = sources_axis("media.video", None, Some(&report), &fetched).expect("an axis");
    assert_eq!(axis.lines, vec![tunebat.to_owned()]);
}

const S1_VIDEO: &str = "https://www.youtube.com/watch?v=8aQ8SoQl5EY";

fn s1_mark(stream: &str) -> Mark {
    sources_axis("media.video", None, Some(&cites(&[S1_VIDEO])), stream)
        .expect("an axis")
        .mark
}

/// Review round (S1): a call that only names the cited video, or one that tried to open it and failed, is no read.
/// Each refusal has its own control in `c40b.py`.
#[test]
fn c40b_a_media_cite_needs_a_call_that_opened_it() {
    let bash = |command: &str, is_error: bool| {
        claude_call("t1", "Bash", serde_json::json!({ "command": command }), is_error)
    };
    let proxy = r"C:\x\8aQ8SoQl5EY\proxy.mp4";
    // echo: names the URL, fetches nothing.
    assert_eq!(s1_mark(&bash(&format!("echo {S1_VIDEO}"), false)), Mark::Fail, "echo");
    // A heredoc that writes the URL into a notes file.
    let heredoc = format!("cat > notes.md <<'EOF'\n{S1_VIDEO}\nEOF");
    assert_eq!(s1_mark(&bash(&heredoc, false)), Mark::Fail, "heredoc");
    // ls of the id's folder and of its media file: it lists, it does not open.
    assert_eq!(s1_mark(&bash(r"ls C:\x\8aQ8SoQl5EY", false)), Mark::Fail, "ls folder");
    assert_eq!(s1_mark(&bash(&format!("ls {proxy}"), false)), Mark::Fail, "ls file");
    // A Read of a missing path: the answer is an error.
    let read = |path: &str, is_error: bool| {
        claude_call("t2", "Read", serde_json::json!({ "file_path": path }), is_error)
    };
    assert_eq!(s1_mark(&read(proxy, true)), Mark::Fail, "missing Read");
    assert_eq!(
        s1_mark(&agy_step(3, "ERROR", "read_file", serde_json::json!({ "Path": proxy }), "")),
        Mark::Fail,
        "missing read_file"
    );
    // An unanswered agy call is the call alone.
    assert_eq!(
        s1_mark(&agy_step(3, "ACTIVE", "read_file", serde_json::json!({ "Path": proxy }), "")),
        Mark::Fail,
        "unanswered read_file"
    );
    // A file in the id's folder that holds no media.
    assert_eq!(s1_mark(&read(r"C:\x\8aQ8SoQl5EY\notes.md", false)), Mark::Fail, "notes Read");
    // A failed ffprobe, in both streams.
    assert_eq!(s1_mark(&bash(&format!("ffprobe {proxy}"), true)), Mark::Fail, "ffprobe Bash");
    let failed = agy_step(
        4,
        "DONE",
        "run_command",
        serde_json::json!({ "CommandLine": format!("ffprobe {proxy}") }),
        "Command exited with code 1.\n\nCommand output:\nNo such file",
    );
    assert_eq!(s1_mark(&failed), Mark::Fail, "ffprobe agy");
    // ffmpeg's output file is not something it opened.
    let writes = format!(r"ffmpeg -i C:\x\other.mp4 {proxy}");
    assert_eq!(s1_mark(&bash(&writes, false)), Mark::Fail, "ffmpeg output");
}

/// Review round (S1): the calls that do open the cited video count: a fetch or a probe naming its URL, ffmpeg with the
/// URL as input, a media file of its id read or probed without an error.
#[test]
fn c40b_a_media_cite_counts_a_call_that_opened_it() {
    let bash = |command: &str| claude_call("t1", "Bash", serde_json::json!({ "command": command }), false);
    let proxy = r"C:\x\8aQ8SoQl5EY\proxy.mp4";
    assert_eq!(s1_mark(&bash(&format!("ffprobe -v error {S1_VIDEO}"))), Mark::Pass, "ffprobe url");
    assert_eq!(s1_mark(&bash(&format!("ffmpeg -i \"{S1_VIDEO}\" out.mp4"))), Mark::Pass, "ffmpeg url");
    assert_eq!(s1_mark(&bash(&format!("ffprobe \"{proxy}\""))), Mark::Pass, "ffprobe file");
    assert_eq!(s1_mark(&bash(&format!("ffmpeg -ss 0 -i {proxy} -frames:v 1 f.jpg"))), Mark::Pass, "ffmpeg file");
    let ok = agy_step(
        4,
        "DONE",
        "run_command",
        serde_json::json!({ "CommandLine": format!("ffprobe {proxy}") }),
        "Command output:\nDuration: 00:03:10",
    );
    assert_eq!(s1_mark(&ok), Mark::Pass, "ffprobe agy");
    let frame = claude_call(
        "t2",
        "Read",
        serde_json::json!({ "file_path": r"C:\x\8aQ8SoQl5EY\frame_001.jpg" }),
        false,
    );
    assert_eq!(s1_mark(&frame), Mark::Pass, "frame Read");
    // The fetch filter research.* uses: a cut python command that imports yt_dlp.
    let cut = format!("python -c \"\nimport yt_dlp\nurl = '{S1_VIDEO}'\n\u{2026}");
    assert_eq!(s1_mark(&bash(&cut)), Mark::Pass, "cut yt_dlp");
}

/// video-breakdown: a local file cited as `file:///` and opened by `read_file`. It passes since plan 35a round 2,
/// which landed after this order was gated; kept as the third shape.
#[test]
fn c40b_a_media_order_cites_the_local_file_it_opened() {
    let stream = agy_tool(
        4,
        "read_file",
        serde_json::json!({"Path": r"C:\Users\me\Downloads\IMG_4941.mov"}),
    );
    let report = cites(&["file:///C:/Users/me/Downloads/IMG_4941.mov"]);
    let axis = sources_axis("media.video", None, Some(&report), &stream).expect("an axis");
    assert_eq!(axis.mark, Mark::Pass, "{axis:?}");
}

// ---- 0.71.1: a folder owned by its trailing slash; the owner's commits a worktree began with ----

#[test]
fn p72_an_owns_line_ending_in_a_slash_owns_the_whole_folder() {
    assert!(glob_match("Config/", "Config/DefaultEngine.ini"));
    assert!(glob_match(
        "Source/InkAether/",
        "Source/InkAether/Private/Deep/A.cpp"
    ));
    assert!(!glob_match("Config/", "ConfigX/a.ini"));
    assert!(!glob_match("Config/", "Other/Config/a.ini"));
    // A bare slash owns nothing, as an empty line does.
    assert!(!glob_match("/", "a.txt"));
    // A Windows separator is read as `/` first.
    let read = spec("owns: Config\\ Source/InkAether/\n");
    let changed = vec![
        "Config/DefaultEngine.ini".to_owned(),
        "Source/InkAether/Private/A.cpp".to_owned(),
    ];
    let axis = owned_axis("build.bounded", &changed, &read.owns);
    assert_eq!(axis.mark, Mark::Pass, "{axis:?}");
}

/// As the owner's project on 2026-11-13: the integration branch starts at `base`, then the owner commits on main,
/// then the order's worktree is made from main through herdr's own `add_worktree`, which records where it began.
fn begun_after_an_owner_commit(name: &str, order: &str) -> (PathBuf, PathBuf) {
    let root = repo(name);
    ensure_integration(&root, "r1").expect("integration");
    std::fs::create_dir_all(root.join("Docs").join("Handoff")).expect("docs");
    std::fs::write(
        root.join("Docs").join("Handoff").join("TABLE.md"),
        "the table\n",
    )
    .expect("doc");
    std::fs::write(root.join("README.md"), "calc, with the handoff\n").expect("readme");
    sh(&root, &["add", "Docs/Handoff/TABLE.md", "README.md"]);
    sh(&root, &["commit", "-q", "-m", "Handoff (owner)"]);
    let path = root.join(".nirvana").join("worktrees").join(order);
    crate::nirvana_worker::add_worktree(&root, &path, &format!("nirvana/{order}"))
        .expect("worktree");
    nirvana_run::ensure_dir(&path.join(".nirvana")).expect("dir");
    std::fs::create_dir_all(path.join(".nirvana").join("orders")).expect("orders");
    std::fs::write(
        path.join(".nirvana")
            .join("orders")
            .join(format!("{order}.md")),
        "the order\n",
    )
    .expect("copy");
    (root, path)
}

#[test]
fn p72_the_owners_commits_a_worktree_began_with_are_not_the_orders() {
    let (_root, cwd) = begun_after_an_owner_commit("p72-begun", "am1");
    work(&cwd);
    let spec = spec_with(&["calc.py", "test_*.py"], "git ls-files", &["test_mul"]);
    let det = run_det(&_root, &cwd, "am1", &spec, Some(&done()));
    let owned = axis(&det, "owned");
    assert_eq!(owned.mark, Mark::Pass, "{det:#?}");
    assert!(
        owned.why.contains("2 file(s) the worktree began with"),
        "{}",
        owned.why
    );
}

#[test]
fn p72_without_the_start_record_the_owners_files_still_count() {
    // A worktree made before 0.71.1 has no record: the gate counts as it did, which is the sent-back am1.
    let (root, cwd) = begun_after_an_owner_commit("p72-norecord", "am1");
    std::fs::remove_file(start_file(&cwd)).expect("record");
    work(&cwd);
    let spec = spec_with(&["calc.py", "test_*.py"], "git ls-files", &["test_mul"]);
    let det = run_det(&root, &cwd, "am1", &spec, Some(&done()));
    let owned = axis(&det, "owned");
    assert_eq!(owned.mark, Mark::Fail, "{det:#?}");
    assert!(
        owned.lines.iter().any(|f| f == "Docs/Handoff/TABLE.md"),
        "{owned:?}"
    );
}

#[test]
fn p72_a_worker_edit_to_a_file_the_worktree_began_with_still_counts() {
    let (root, cwd) = begun_after_an_owner_commit("p72-sneak", "am1");
    work(&cwd);
    std::fs::write(
        cwd.join("README.md"),
        "calc, with the handoff, and the worker's line\n",
    )
    .expect("edit");
    let spec = spec_with(&["calc.py", "test_*.py"], "git ls-files", &["test_mul"]);
    let det = run_det(&root, &cwd, "am1", &spec, Some(&done()));
    let owned = axis(&det, "owned");
    assert_eq!(owned.mark, Mark::Fail, "{det:#?}");
    assert_eq!(owned.lines, vec!["README.md".to_owned()], "{owned:?}");
}

#[test]
fn p72_a_forged_start_record_passes_nothing() {
    // Review of 0.71.1, finding 1: the worker commits an unowned edit and writes its own HEAD as the start record.
    let (root, cwd) = begun_after_an_owner_commit("p72-forged", "am1");
    work(&cwd);
    std::fs::write(cwd.join("secret.txt"), "not the order's\n").expect("unowned");
    sh(&cwd, &["add", "calc.py", "test_mul.py", "secret.txt"]);
    sh(&cwd, &["commit", "-q", "-m", "worker"]);
    let head = sh(&cwd, &["rev-parse", "HEAD"]).trim().to_owned();
    std::fs::write(start_file(&cwd), format!("{head}\n")).expect("forge");
    let spec = spec_with(&["calc.py", "test_*.py"], "git ls-files", &["test_mul"]);
    let det = run_det(&root, &cwd, "am1", &spec, Some(&done()));
    let owned = axis(&det, "owned");
    assert_eq!(owned.mark, Mark::Fail, "{det:#?}");
    assert!(owned.lines.iter().any(|f| f == "secret.txt"), "{owned:?}");
}

#[test]
fn p72_a_branch_that_undoes_merged_work_keeps_the_file_as_the_orders() {
    // The integration branch changed README.md (another order's merged work) and the owner's main changed it too:
    // a branch that lands main's version over it is not passed as the owner's.
    let root = repo("p72-revert");
    ensure_integration(&root, "r1").expect("integration");
    sh(&root, &["checkout", "-q", "nirvana-integrate/r1"]);
    std::fs::write(root.join("README.md"), "calc, as another order merged it\n").expect("merged");
    sh(&root, &["commit", "-q", "-am", "another order"]);
    sh(&root, &["checkout", "-q", "main"]);
    std::fs::write(root.join("README.md"), "calc, as the owner wrote it\n").expect("owner");
    sh(&root, &["commit", "-q", "-am", "owner"]);
    let start = rev(&root, "HEAD").expect("start");
    let base = rev(&root, "refs/heads/nirvana-integrate/r1").expect("base");
    let path = root.join(".nirvana").join("worktrees").join("o1");
    crate::nirvana_worker::add_worktree(&root, &path, "nirvana/o1").expect("worktree");
    let changed = vec!["README.md".to_owned()];
    assert_eq!(begun_with(&path, &base, &start, &changed), Ok(Vec::new()));
    // And without the integration branch's change the same file is the owner's.
    let fork = sh(&root, &["merge-base", &start, &base]).trim().to_owned();
    assert_eq!(
        begun_with(&path, &fork, &start, &changed),
        Ok(vec!["README.md".to_owned()])
    );
}
