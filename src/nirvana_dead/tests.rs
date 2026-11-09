//! Dead panes: the close rules against a fake herdr (no server), real journals, job records and supervisor locks in
//! temp folders.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::*;

/// The order's end in the journals below: its `ended` line's second.
const ENDED: u64 = 1_000_100;
const LATER: u64 = ENDED + CLOSE_GRACE_S + 60;

struct Temp(PathBuf);

impl Temp {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nirvana_dead_{tag}_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project with one run whose orders are seated on `sonnet`, started in their panes, and ended (`delivered`) at
/// `ENDED`, except those listed in `open`, which are only started.
fn project_with(root: &Path, run: &str, orders: &[(&str, &str)], open: &[&str]) {
    let run_dir = root.join(".nirvana").join("runs").join(run);
    std::fs::create_dir_all(&run_dir).unwrap();
    let mut lines = vec![
        json!({"v":1,"seq":1,"at":1_000_000,"event":"opened","project":"p",
        "orchestrator_pane":"wP:p1","auto":true,"pools":[],"pools_basis":null}),
    ];
    let mut seq = 1;
    for (order, pane) in orders {
        seq += 1;
        lines.push(json!({"v":1,"seq":seq,"at":1_000_001,"event":"seated","order":order,"seat":"sonnet",
            "cli":"claude","model":"claude-sonnet-5-5","effort":"medium","pool":"max","class":"ops.terminal",
            "size":"medium","mode":"balanced","reason":"r","order_file":"f","cwd":"cwd"}));
        seq += 1;
        lines.push(
            json!({"v":1,"seq":seq,"at":1_000_002,"event":"started","order":order,"attempt":1,
            "pid":1,"pane":pane,"tab":null,"resume":null,"argv":[]}),
        );
        if !open.contains(order) {
            seq += 1;
            lines.push(
                json!({"v":1,"seq":seq,"at":ENDED,"event":"delivered","order":order,
                "report":{"status":"done"}}),
            );
        }
    }
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    std::fs::write(run_dir.join("journal.jsonl"), text).unwrap();
}

/// A pane as `pane list` lists it.
fn pane(id: &str, tab: &str, label: Option<&str>, cwd: &Path) -> Value {
    let mut pane = json!({"pane_id": id, "tab_id": tab, "workspace_id": "wP", "agent_status": "unknown",
        "cwd": cwd.display().to_string()});
    if let Some(label) = label {
        pane["label"] = json!(label);
    }
    pane
}

fn agent_pane(id: &str, tab: &str) -> Value {
    json!({"pane_id": id, "tab_id": tab, "workspace_id": "wP", "agent": "claude", "agent_status": "idle"})
}

/// A herdr that answers from `panes`, lists a tab per distinct `tab_id` (plus the space's agent tab), reads every
/// pane's foreground as `processes` unless `busy` names it, and records each `pane close`.
struct FakeHerdr {
    panes: Vec<Value>,
    extra_tabs: Vec<&'static str>,
    busy: Vec<&'static str>,
    relabelled: Vec<&'static str>,
    closed: RefCell<Vec<String>>,
}

impl FakeHerdr {
    fn new(panes: Vec<Value>) -> Self {
        Self {
            panes,
            extra_tabs: vec!["wP:t1"],
            busy: Vec::new(),
            relabelled: Vec::new(),
            closed: RefCell::new(Vec::new()),
        }
    }

    fn call(&self, args: &[&str]) -> Result<Value, String> {
        match args {
            ["pane", "list"] => Ok(json!({"result": {"panes": self.panes}})),
            ["tab", "list"] => {
                let mut tabs: Vec<String> =
                    self.extra_tabs.iter().map(|t| (*t).to_owned()).collect();
                for pane in &self.panes {
                    let tab = pane["tab_id"].as_str().unwrap().to_owned();
                    if !tabs.contains(&tab) {
                        tabs.push(tab);
                    }
                }
                let tabs: Vec<Value> = tabs
                    .iter()
                    .map(|tab| json!({"tab_id": tab, "workspace_id": "wP"}))
                    .collect();
                Ok(json!({"result": {"tabs": tabs}}))
            }
            ["pane", "process-info", "--pane", id] => {
                let names: Vec<&str> = if self.busy.contains(id) {
                    vec!["pwsh.exe", "python.exe"]
                } else {
                    vec!["pwsh.exe"]
                };
                let list: Vec<Value> = names.iter().map(|name| json!({"name": name})).collect();
                Ok(json!({"result": {"process_info": {"foreground_processes": list}}}))
            }
            ["pane", "get", id] => {
                let pane = self
                    .panes
                    .iter()
                    .find(|pane| pane["pane_id"] == *id)
                    .ok_or("pane_not_found")?;
                let label = if self.relabelled.contains(id) {
                    json!("sonnet\u{b7}someone-else")
                } else {
                    pane.get("label").cloned().unwrap_or(Value::Null)
                };
                Ok(json!({"result": {"pane": {"pane_id": id, "label": label}}}))
            }
            ["pane", "close", id] => {
                self.closed.borrow_mut().push((*id).to_owned());
                Ok(json!({"result": {"type": "ok"}}))
            }
            other => Err(format!("the fake herdr has no answer for {other:?}")),
        }
    }

    fn sweep(&self, home: &Path, now: u64) -> Vec<String> {
        sweep(home, &|args| self.call(args), now, false).unwrap()
    }

    fn closed(&self) -> Vec<String> {
        self.closed.borrow().clone()
    }
}

fn worker(order: &str) -> String {
    format!("sonnet\u{b7}{order}")
}

#[test]
fn a_finished_workers_bare_shell_pane_closes_after_the_grace() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(&root.0, "quadxlslice", &[("quad-xl-slice", "wP:p38")], &[]);
    let herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:p38", "wP:t1G", Some(&worker("quad-xl-slice")), &root.0),
    ]);
    herdr.sweep(&home.0, ENDED + 30);
    assert!(herdr.closed().is_empty(), "inside the grace the pane stays");
    let lines = herdr.sweep(&home.0, LATER);
    assert_eq!(herdr.closed(), vec!["wP:p38".to_owned()]);
    assert_eq!(
        lines,
        vec![format!("closed wP:p38 ({})", worker("quad-xl-slice"))]
    );
}

#[test]
fn a_pane_whose_worker_still_runs_is_never_closed() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(&root.0, "r", &[("still-going", "wP:p40")], &["still-going"]);
    let project = Project::at(&root.0);
    let lock = crate::nirvana_worker::take_live(&project, "r", "still-going")
        .unwrap()
        .unwrap();
    let herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:p40", "wP:t2", Some(&worker("still-going")), &root.0),
    ]);
    herdr.sweep(&home.0, LATER + 1_000_000);
    assert!(
        herdr.closed().is_empty(),
        "a held supervisor lock keeps the pane"
    );
    drop(lock);
    // Its supervisor gone (a server restart), the same order's pane closes: the journal still says started.
    herdr.sweep(&home.0, LATER + 1_000_000);
    assert_eq!(herdr.closed(), vec!["wP:p40".to_owned()]);
}

#[test]
fn a_pane_running_a_program_is_never_closed() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(&root.0, "r", &[("ran", "wP:p41")], &[]);
    let mut herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:p41", "wP:t2", Some(&worker("ran")), &root.0),
    ]);
    herdr.busy = vec!["wP:p41"];
    herdr.sweep(&home.0, LATER);
    assert!(herdr.closed().is_empty());
}

#[test]
fn a_pane_the_owner_typed_in_after_its_worker_ended_is_kept() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(
        &root.0,
        "r",
        &[("typed", "wP:p5"), ("untouched", "wP:p6")],
        &[],
    );
    let keys = home.0.join("nirvana").join("keys");
    std::fs::create_dir_all(&keys).unwrap();
    std::fs::write(keys.join("wP_p5"), (ENDED + 10).to_string()).unwrap();
    // A key while the worker ran is not the owner's use of the pane afterwards.
    std::fs::write(keys.join("wP_p6"), (ENDED - 10).to_string()).unwrap();
    let herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:p5", "wP:t2", Some(&worker("typed")), &root.0),
        pane("wP:p6", "wP:t2", Some(&worker("untouched")), &root.0),
    ]);
    herdr.sweep(&home.0, LATER);
    assert_eq!(herdr.closed(), vec!["wP:p6".to_owned()]);
}

#[test]
fn a_pane_with_no_worker_or_job_label_is_never_closed() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(&root.0, "r", &[("ran", "wP:p7")], &[]);
    let herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:p7", "wP:t2", None, &root.0),
        pane("wP:p8", "wP:t2", Some("notes"), &root.0),
        pane("wP:p9", "wP:t2", Some("my shell\u{b7}x y"), &root.0),
    ]);
    herdr.sweep(&home.0, LATER);
    assert!(herdr.closed().is_empty());
}

#[test]
fn the_spaces_agent_tab_and_an_agent_pane_are_never_closed() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(
        &root.0,
        "r",
        &[("beside", "wP:p2"), ("agented", "wP:p3")],
        &[],
    );
    let mut agented = pane("wP:p3", "wP:t3", Some(&worker("agented")), &root.0);
    agented["agent"] = json!("claude");
    let herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:p2", "wP:t1", Some(&worker("beside")), &root.0),
        agented,
    ]);
    herdr.sweep(&home.0, LATER);
    assert!(herdr.closed().is_empty());
}

#[test]
fn a_run_tabs_panes_all_close_but_a_spaces_last_pane_stays() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(&root.0, "r", &[("one", "wP:pA"), ("two", "wP:pB")], &[]);
    let herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:pA", "wP:t9", Some(&worker("one")), &root.0),
        pane("wP:pB", "wP:t9", Some(&worker("two")), &root.0),
    ]);
    herdr.sweep(&home.0, LATER);
    // Both panes of the run tab close; herdr closes a tab with its last pane (`Workspace::close_pane`).
    assert_eq!(herdr.closed(), vec!["wP:pA".to_owned(), "wP:pB".to_owned()]);

    // A space whose only tab holds only a finished worker's pane keeps it: closing it would close the space.
    let mut alone = FakeHerdr::new(vec![pane("wP:pA", "wP:t9", Some(&worker("one")), &root.0)]);
    alone.extra_tabs = Vec::new();
    alone.sweep(&home.0, LATER);
    assert!(alone.closed().is_empty());
    // Two such panes: the first closes, and the pass counts it gone, so the second, now the last, stays.
    let mut pair = FakeHerdr::new(vec![
        pane("wP:pA", "wP:t9", Some(&worker("one")), &root.0),
        pane("wP:pB", "wP:t9", Some(&worker("two")), &root.0),
    ]);
    pair.extra_tabs = Vec::new();
    pair.sweep(&home.0, LATER);
    assert_eq!(pair.closed(), vec!["wP:pA".to_owned()]);
}

fn job(home: &Path, id: &str, name: &str, pane: &str, ended: Option<u64>, pid: Option<u32>) {
    let record = crate::nirvana_job::JobRecord {
        id: id.to_owned(),
        name: name.to_owned(),
        argv: vec!["cargo".into()],
        cwd: String::new(),
        space: Some("wP".into()),
        pane: Some(pane.to_owned()),
        started: ENDED - 50,
        ended,
        exit: ended.map(|_| 0),
        pid,
    };
    crate::nirvana_job::write_job(home, &record).unwrap();
}

#[test]
fn an_ended_jobs_pane_closes_and_a_running_ones_stays() {
    let home = Temp::new("home");
    job(&home.0, "job-1", "bs209-build", "wP:pJ1", Some(ENDED), None);
    // This test's own process stands for the job's: alive, no end recorded.
    job(
        &home.0,
        "job-2",
        "long-test",
        "wP:pJ2",
        None,
        Some(std::process::id()),
    );
    // The pane's latest job is another than its label names: no record names this one.
    job(&home.0, "job-3", "other", "wP:pJ3", Some(ENDED), None);
    let mut herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:pJ1", "wP:tZ", Some("job\u{b7}bs209-build"), &home.0),
        pane("wP:pJ2", "wP:tZ", Some("job\u{b7}long-test"), &home.0),
        pane("wP:pJ3", "wP:tZ", Some("job\u{b7}renamed"), &home.0),
    ]);
    herdr.extra_tabs = vec!["wP:t1"];
    herdr.sweep(&home.0, LATER);
    assert_eq!(herdr.closed(), vec!["wP:pJ1".to_owned()]);
}

#[test]
fn a_labelled_pane_no_record_names_is_kept() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(&root.0, "r", &[("known", "wP:pK")], &[]);
    let herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:pG", "wP:t2", Some(&worker("ghost")), &root.0),
        // The journal's order, on another seat than the label names.
        pane("wP:pK", "wP:t2", Some("opus\u{b7}known"), &root.0),
        pane("wP:pJ", "wP:t2", Some("job\u{b7}gone"), &root.0),
    ]);
    herdr.sweep(&home.0, LATER);
    assert!(herdr.closed().is_empty());
}

#[test]
fn a_pane_relabelled_since_the_list_is_kept() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(&root.0, "r", &[("reused", "wP:pR")], &[]);
    let mut herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:pR", "wP:t2", Some(&worker("reused")), &root.0),
    ]);
    herdr.relabelled = vec!["wP:pR"];
    herdr.sweep(&home.0, LATER);
    assert!(herdr.closed().is_empty());
}

#[test]
fn a_dry_run_names_each_verdict_and_closes_nothing() {
    let (home, root) = (Temp::new("home"), Temp::new("proj"));
    project_with(&root.0, "r", &[("done-one", "wP:pD")], &[]);
    let herdr = FakeHerdr::new(vec![
        agent_pane("wP:p1", "wP:t1"),
        pane("wP:pD", "wP:t2", Some(&worker("done-one")), &root.0),
        pane("wP:pG", "wP:t2", Some(&worker("ghost")), &root.0),
    ]);
    let lines = sweep(&home.0, &|args| herdr.call(args), LATER, true).unwrap();
    assert!(herdr.closed().is_empty());
    assert_eq!(
        lines,
        vec![
            format!("would close wP:pD ({})", worker("done-one")),
            format!(
                "keeps wP:pG ({}): no record names its worker or job",
                worker("ghost")
            ),
        ]
    );
}

#[test]
fn order_live_reads_the_supervisor_not_the_phase() {
    let now = 10_000;
    for phase in [Phase::Running, Phase::Deferred, Phase::Seated] {
        assert!(order_live(phase, true, 0, now), "{phase:?} supervised");
        assert!(!order_live(phase, false, 0, now), "{phase:?} unsupervised");
    }
    assert!(order_live(
        Phase::Seated,
        false,
        now - SEAT_GRACE_S + 1,
        now
    ));
    assert!(!order_live(Phase::Running, false, now - 1, now));
    for phase in [
        Phase::Asked,
        Phase::Delivered,
        Phase::Interrupted,
        Phase::Failed,
    ] {
        assert!(!order_live(phase, true, now, now), "{phase:?} never live");
    }
    assert_eq!(phase_word(Phase::Running, false), "stopped");
    assert_eq!(phase_word(Phase::Running, true), "running");
    assert_eq!(phase_word(Phase::Asked, false), "asked");
}

#[test]
fn labels_name_a_worker_or_a_job() {
    assert_eq!(
        parse_label("flash\u{b7}bs-2-09-mixed-nozzle-lr"),
        Some(Owner::Worker {
            seat: "flash".into(),
            order: "bs-2-09-mixed-nozzle-lr".into()
        })
    );
    assert_eq!(
        parse_label("job\u{b7}bs209-build"),
        Some(Owner::Job {
            name: "bs209-build".into()
        })
    );
    assert_eq!(parse_label("1"), None);
    assert_eq!(parse_label("Jobs"), None);
    assert_eq!(parse_label("my shell\u{b7}x y"), None);
}
