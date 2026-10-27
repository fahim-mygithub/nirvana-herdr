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
    let on = |chain: &str, outcome: &str| -> nirvana_seat::RecordLine {
        serde_json::from_value(serde_json::json!({"order":"o","seat":"sonnet","effort":"medium","class":"build.bounded",
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
        on("subagent", "subagent-done"),
        on("subagent", "subagent-blocked"),
    ]);
    assert_eq!(tallies.len(), 1, "{tallies:?}");
    let tally = tallies[&(
        "sonnet".to_owned(),
        Effort::Medium,
        "build.bounded".to_owned(),
    )];
    assert_eq!((tally.passes, tally.orders), (1, 3));
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

#[test]
fn sources_axis_checks_citations_in_worker_stream() {
    // 1. Build class has no sources axis
    assert_eq!(sources_axis("build.bounded", None, ""), None);

    // 2. Empty sources array or missing report fails for research / media
    let empty_report = serde_json::json!({"sources": []});
    let axis = sources_axis("research.web", Some(&empty_report), "").expect("axis");
    assert_eq!(axis.mark, Mark::Fail);
    assert!(axis.why.contains("no sources"));

    let no_report = sources_axis("media.video", None, "").expect("axis");
    assert_eq!(no_report.mark, Mark::Fail);

    // 3. Cited URL never fetched fails
    let report_with_url = serde_json::json!({
        "sources": [
            {"url": "https://example.com/docs#intro", "used_for": "reading spec"}
        ]
    });
    let stream_without = "I researched the web using other means.";
    let axis = sources_axis("research.web", Some(&report_with_url), stream_without).expect("axis");
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
    let axis = sources_axis("research.web", Some(&report_with_url), &stream_with).expect("axis");
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

    let axis1 =
        sources_axis("research.web", Some(&report), &claude_assistant_text_stream).expect("axis");
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
        sources_axis("research.web", Some(&report), &claude_web_fetch_stream).expect("axis");
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
    let axis3 = sources_axis("research.web", Some(&report), &agy_stream).expect("axis");
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
    let axis = sources_axis("research.web", Some(&report), &agy_open_browser_stream).expect("axis");
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
    let axis =
        sources_axis("research.web", Some(&report), &claude_agent_browser_stream).expect("axis");
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
    let axis = sources_axis("research.web", Some(&report), &claude_echo_stream).expect("axis");
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
    let axis = sources_axis("research.web", Some(&report), &browser_subagent_stream).expect("axis");
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
        sources_axis("research.web", Some(&report), &claude_cache_tokens_stream).expect("axis");
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
    let mut seated_order = seated("build-1", None);
    seated_order.override_reason = Some("owner insisted on sonnet".into());
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
    assert_eq!(child.seated.seat, "sonnet");
    assert_eq!(child.seated.effort, Effort::Medium);
    assert_eq!(
        child.seated.override_reason,
        Some("owner insisted on sonnet".into())
    );

    // Run overrides in leases is still 3 because child was an inherited override and did not increment!
    let leases =
        crate::nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE))
            .expect("leases");
    assert_eq!(leases.overrides(run), 3);
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
        sources_axis("research.web", Some(&report), &stream)
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
