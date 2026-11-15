//! The Questions record: real files in temp folders, the CLI through `execute`, journals written as herdr writes them.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::*;

const NOW: u64 = 1_800_000_000;

struct Temp(PathBuf);

impl Temp {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nirvana_questions_{tag}_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn project(&self) -> Project {
        Project::at(&self.0)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}

fn run(project: &Project, words: &[&str]) -> Outcome {
    execute(project, &args(words), NOW)
}

fn seated(seq: u64, order: &str, class: &str, parent: Option<&str>) -> Value {
    let mut line = json!({"v":1,"seq":seq,"at":NOW - 1000 + seq,"event":"seated","order":order,"seat":"sonnet",
        "cli":"claude","model":"claude-sonnet-5-5","effort":"medium","pool":"max","class":class,
        "size":"medium","mode":"balanced","reason":"r","order_file":"f","cwd":"cwd"});
    if let Some(parent) = parent {
        line["parent"] = json!(parent);
        line["why"] = json!("send_back");
    }
    line
}

fn asked(seq: u64, order: &str, question: &str) -> Value {
    json!({"v":1,"seq":seq,"at":NOW - 1000 + seq,"event":"asked","order":order,"question":question})
}

fn write_journal(root: &Path, run: &str, lines: &[Value]) {
    let dir = root.join(".nirvana").join("runs").join(run);
    std::fs::create_dir_all(&dir).unwrap();
    let opened = json!({"v":1,"seq":0,"at":NOW - 1000,"event":"opened","project":"p","orchestrator_pane":"wP:p1",
        "auto":true,"pools":[],"pools_basis":null,"order":"o"});
    let text: String = std::iter::once(&opened)
        .chain(lines)
        .map(|line| format!("{line}\n"))
        .collect();
    std::fs::write(dir.join("journal.jsonl"), text).unwrap();
}

#[test]
fn the_latest_line_for_an_id_wins_in_its_first_place() {
    let a = json!({"id":"q1","source":"orchestrator","text":"a","state":"open","asked_at":1});
    let b = json!({"id":"q2","source":"orchestrator","text":"b","state":"open","asked_at":2});
    let a2 = json!({"id":"q1","source":"orchestrator","text":"a","state":"answered","answer":"yes",
        "answered_by":"orchestrator","asked_at":1});
    let questions = parse(&format!("{a}\n{b}\nnot json\n{a2}\n"));
    assert_eq!(questions.items.len(), 2);
    assert_eq!(questions.items[0].id, "q1");
    assert_eq!(questions.items[0].state, State::Answered);
    assert_eq!(questions.items[0].answer.as_deref(), Some("yes"));
    assert_eq!(questions.bad_lines, 1);
}

#[test]
fn add_files_numbered_questions_with_options_and_blocks() {
    let temp = Temp::new("add");
    let p = temp.project();
    let first = run(
        &p,
        &[
            "add",
            "Cut L06, L11 and L12 from scope?",
            "--option",
            "cut",
            "--option",
            "keep",
            "--recommended",
            "cut",
            "--blocks",
            "l05",
        ],
    );
    assert_eq!(
        (first.code, first.out.as_str()),
        (0, "q1 open\n"),
        "{first:?}"
    );
    let second = run(&p, &["add", "Download the assets?"]);
    assert_eq!(second.out, "q2 open\n");
    let q = load(&p);
    assert_eq!(q.items.len(), 2);
    let item = q.get("q1").unwrap();
    assert_eq!(item.options, ["cut", "keep"]);
    assert_eq!(item.recommended.as_deref(), Some("cut"));
    assert_eq!(item.blocks, ["l05"]);
    assert_eq!(
        (item.source.as_str(), item.state, item.asked_count),
        (ORCHESTRATOR, State::Open, 1)
    );
    assert_eq!(item.asked_at, NOW);
}

#[test]
fn the_same_text_asked_again_raises_its_count_and_makes_no_second_item() {
    let temp = Temp::new("same");
    let p = temp.project();
    run(&p, &["add", "Merge ia1 to main?"]);
    let again = run(&p, &["add", "  merge   IA1 to main  "]);
    assert_eq!(again.code, 0, "{again:?}");
    assert!(again.out.starts_with("q1 open, asked 2 times"), "{again:?}");
    let named = run(
        &p,
        &[
            "add",
            "Shall I merge the run branch now",
            "--same",
            "q1",
            "--blocks",
            "merge",
        ],
    );
    assert!(named.out.starts_with("q1 open, asked 3 times"), "{named:?}");
    let q = load(&p);
    assert_eq!(q.items.len(), 1, "{:?}", q.items);
    assert_eq!(q.items[0].asked_count, 3);
    assert_eq!(q.items[0].blocks, ["merge"]);
    assert_eq!(q.items[0].text, "Merge ia1 to main?");
}

#[test]
fn a_question_asked_again_after_its_answer_prints_the_answer() {
    let temp = Temp::new("answered_again");
    let p = temp.project();
    run(&p, &["add", "Merge ia1 to main?"]);
    run(&p, &["answer", "q1", "yes"]);
    let again = run(&p, &["add", "Merge ia1 to main?"]);
    assert_eq!(
        (again.code, again.out.as_str()),
        (0, "q1 was answered: yes\n"),
        "{again:?}"
    );
    assert_eq!(load(&p).get("q1").unwrap().state, State::Answered);
}

#[test]
fn bad_asks_are_refused_with_exit_2_and_nothing_written() {
    let temp = Temp::new("bad");
    let p = temp.project();
    let long = "x".repeat(TEXT_MAX + 1);
    for words in [
        vec![
            "add",
            "Pick one?",
            "--option",
            "a",
            "--option",
            "b",
            "--recommended",
            "c",
        ],
        vec!["add", "Pick one?", "--recommended", "a"],
        vec!["add", long.as_str()],
        vec!["add", "two\nlines"],
        vec!["add", "   "],
        vec!["add", "Pick?", "--option", "a", "--option", "a"],
        vec!["add", "Pick?", "--blocks", "not an id"],
        vec!["add", "Pick?", "--option"],
        vec!["add"],
        vec!["frobnicate"],
    ] {
        let out = run(&p, &words);
        assert_eq!(out.code, 2, "{words:?} {out:?}");
        assert!(out.err.starts_with("herdr ask: "), "{words:?} {out:?}");
    }
    let exactly = "y".repeat(TEXT_MAX);
    assert_eq!(run(&p, &["add", exactly.as_str()]).code, 0);
    assert_eq!(load(&p).items.len(), 1);
}

#[test]
fn the_cli_answer_is_the_orchestrators_and_no_flag_makes_it_the_owners() {
    let temp = Temp::new("by");
    let p = temp.project();
    run(&p, &["add", "Download the assets?"]);
    let forged = run(&p, &["answer", "q1", "yes", "--by", "owner"]);
    assert_eq!(forged.code, 2, "{forged:?}");
    // Refused as a flag, before any arity check: no `--by` reaches the record.
    assert!(forged.err.contains("answer takes no --by"), "{forged:?}");
    assert_eq!(load(&p).get("q1").unwrap().state, State::Open);
    let out = run(&p, &["answer", "q1", "Go ahead"]);
    assert_eq!(
        (out.code, out.out.as_str()),
        (0, "q1 answered\n"),
        "{out:?}"
    );
    let item = load(&p).get("q1").unwrap().clone();
    assert_eq!(item.state, State::Answered);
    assert_eq!(item.answer.as_deref(), Some("Go ahead"));
    assert_eq!(item.answered_by, Some(By::Orchestrator));
    assert_eq!(item.answered_at, Some(NOW));
    // Answered once, the orchestrator cannot answer it again.
    assert_eq!(run(&p, &["answer", "q1", "no"]).code, 1);
}

#[test]
fn the_owner_answers_through_the_library_and_may_correct_the_orchestrator() {
    let temp = Temp::new("owner");
    let p = temp.project();
    run(&p, &["add", "Scope cut?"]);
    run(&p, &["answer", "q1", "cut"]);
    let item = answer_as_owner(&p, "q1", "keep", NOW + 5).unwrap();
    assert_eq!(item.answered_by, Some(By::Owner));
    assert_eq!(item.answer.as_deref(), Some("keep"));
    // The owner's answer stands: nobody re-answers it, the owner included (a double tap answers once).
    assert!(matches!(
        answer_as_owner(&p, "q1", "cut", NOW + 6),
        Err(AskError::NotOpen { .. })
    ));
    assert!(matches!(
        answer(&p, "q1", "cut", NOW + 6),
        Err(AskError::NotOpen { .. })
    ));
    assert_eq!(load(&p).get("q1").unwrap().answer.as_deref(), Some("keep"));
}

#[test]
fn withdraw_takes_an_open_question_back_and_an_unknown_id_exits_1() {
    let temp = Temp::new("withdraw");
    let p = temp.project();
    run(&p, &["add", "Rename the module?"]);
    let out = run(&p, &["withdraw", "q1"]);
    assert_eq!(
        (out.code, out.out.as_str()),
        (0, "q1 withdrawn\n"),
        "{out:?}"
    );
    assert_eq!(load(&p).get("q1").unwrap().state, State::Withdrawn);
    assert_eq!(run(&p, &["withdraw", "q1"]).code, 1);
    assert_eq!(run(&p, &["answer", "q1", "x"]).code, 1);
    assert_eq!(run(&p, &["answer", "q9", "x"]).code, 1);
    // Asked again after it was taken back, it is a new question with a new id.
    assert_eq!(run(&p, &["add", "Rename the module?"]).out, "q2 open\n");
}

#[test]
fn list_shows_open_items_and_all_with_all() {
    let temp = Temp::new("list");
    let p = temp.project();
    run(
        &p,
        &[
            "add",
            "First?",
            "--option",
            "a",
            "--option",
            "b",
            "--recommended",
            "b",
        ],
    );
    run(&p, &["add", "Second?"]);
    run(&p, &["answer", "q2", "done"]);
    let open = run(&p, &["list"]);
    assert_eq!(open.code, 0);
    assert!(
        open.out
            .contains("q1  open  [orchestrator] First?  options: a | b (recommended)"),
        "{}",
        open.out
    );
    assert!(!open.out.contains("q2"), "{}", open.out);
    assert!(
        open.out.ends_with("1 open, the oldest 0s old\n"),
        "{}",
        open.out
    );
    let all = run(&p, &["list", "--all", "--json"]);
    let items: Vec<Item> = serde_json::from_str(&all.out).unwrap();
    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        ["q1", "q2"]
    );
    assert_eq!(run(&p, &["list", "--option", "x"]).code, 2);
}

#[test]
fn the_summary_counts_open_items_and_the_oldest_age() {
    let temp = Temp::new("summary");
    let p = temp.project();
    add(
        &p,
        &Ask {
            text: "Old?".into(),
            ..Ask::default()
        },
        NOW - 7200,
    )
    .unwrap();
    add(
        &p,
        &Ask {
            text: "New?".into(),
            ..Ask::default()
        },
        NOW - 60,
    )
    .unwrap();
    add(
        &p,
        &Ask {
            text: "Done?".into(),
            ..Ask::default()
        },
        NOW - 99_999,
    )
    .unwrap();
    answer(&p, "q3", "yes", NOW).unwrap();
    let summary = load(&p).summary(NOW);
    assert_eq!(
        summary,
        Summary {
            open: 2,
            oldest_age_s: Some(7200)
        }
    );
    assert_eq!(age_word(7200), "2h");
}

#[test]
fn a_workers_asked_event_becomes_one_item_however_often_it_is_read() {
    let temp = Temp::new("mirror");
    let p = temp.project();
    write_journal(
        &temp.0,
        "ia1",
        &[
            seated(1, "c6-candle-noise", "ops.terminal", None),
            asked(
                2,
                "c6-candle-noise",
                "Where exactly did you see the candle noise?",
            ),
        ],
    );
    let first = run(&p, &["list", "--json"]);
    let items: Vec<Item> = serde_json::from_str(&first.out).unwrap();
    assert_eq!(items.len(), 1, "{}", first.out);
    let item = &items[0];
    assert_eq!(item.source, "c6-candle-noise");
    assert_eq!(item.run.as_deref(), Some("ia1"));
    assert_eq!(item.text, "Where exactly did you see the candle noise?");
    assert_eq!(item.journal, ["ia1#2"]);
    assert_eq!(item.asked_at, NOW - 998);
    // Read again, with the cursor and without it: still one item asked once.
    run(&p, &["list"]);
    std::fs::remove_file(p.dir().join(CURSOR)).unwrap();
    run(&p, &["list"]);
    let q = load(&p);
    assert_eq!(q.items.len(), 1, "{:?}", q.items);
    assert_eq!(q.items[0].asked_count, 1);
    // The orchestrator's own questions number on from the mirrored one.
    assert_eq!(run(&p, &["add", "Mine?"]).out, "q2 open\n");
}

#[test]
fn a_long_worker_question_is_cut_to_one_line_and_kept_in_full() {
    let temp = Temp::new("long");
    let p = temp.project();
    let long = format!(
        "c5-zones-s1 is blocked:\n{}",
        "the gate's owned finding looks false. ".repeat(10)
    );
    write_journal(
        &temp.0,
        "ia1",
        &[
            seated(1, "c5-zones-s1", "ops.terminal", None),
            asked(2, "c5-zones-s1", &long),
        ],
    );
    let q = refresh(&p, NOW).unwrap();
    let item = &q.items[0];
    assert_eq!(item.text.chars().count(), TEXT_MAX);
    assert!(!item.text.contains('\n'));
    assert_eq!(item.detail.as_deref(), Some(long.trim()));
}

#[test]
fn a_worker_question_the_run_answers_is_settled_by_the_journal() {
    let temp = Temp::new("settle");
    let p = temp.project();
    let lines = vec![
        seated(1, "w1", "ops.terminal", None),
        asked(2, "w1", "Which palette?"),
        seated(3, "review", crate::nirvana_review::CLASS, None),
        asked(4, "review", "Is w1 right?"),
    ];
    write_journal(&temp.0, "r1", &lines);
    let q = refresh(&p, NOW).unwrap();
    assert_eq!(
        q.items.len(),
        1,
        "a review's question is not the owner's: {:?}",
        q.items
    );
    assert_eq!(q.items[0].state, State::Open);
    // A later order seated with no parent answers the oldest open question (`nirvana_run::replay`).
    let mut more = lines.clone();
    more.push(seated(5, "w2", "ops.terminal", None));
    write_journal(&temp.0, "r1", &more);
    let q = refresh(&p, NOW + 1).unwrap();
    let item = q.get("q1").unwrap();
    assert_eq!(item.state, State::Answered, "{item:?}");
    assert_eq!(item.answered_by, Some(By::Journal));
    assert_eq!(
        item.answer.as_deref(),
        Some("answered in the run: order w2 took it up")
    );
}

#[test]
fn a_worker_question_a_child_order_continues_is_settled_by_the_journal() {
    let temp = Temp::new("child");
    let p = temp.project();
    let lines = vec![
        seated(1, "w1", "ops.terminal", None),
        asked(2, "w1", "Which palette?"),
    ];
    write_journal(&temp.0, "r1", &lines);
    refresh(&p, NOW).unwrap();
    // A continuation is a child order (`parent`), and it answers none by `answers`: the parent link settles it.
    let mut more = lines.clone();
    more.push(seated(3, "w1-s1", "ops.terminal", Some("w1")));
    write_journal(&temp.0, "r1", &more);
    let item = refresh(&p, NOW + 1).unwrap().get("q1").unwrap().clone();
    assert_eq!(item.state, State::Answered, "{item:?}");
    assert_eq!(
        item.answer.as_deref(),
        Some("answered in the run: order w1-s1 took it up")
    );
}

#[test]
fn a_worker_question_settles_when_its_order_goes_on() {
    let temp = Temp::new("goes_on");
    let p = temp.project();
    let lines = vec![
        seated(1, "w1", "ops.terminal", None),
        asked(2, "w1", "Which palette?"),
    ];
    write_journal(&temp.0, "r1", &lines);
    refresh(&p, NOW).unwrap();
    // A worker that asked mid-order and then delivered answered its own (`nirvana_run::replay`).
    let mut more = lines.clone();
    more.push(
        json!({"v":1,"seq":3,"at":NOW,"event":"delivered","order":"w1","report":{"status":"done"}}),
    );
    write_journal(&temp.0, "r1", &more);
    let item = refresh(&p, NOW + 1).unwrap().get("q1").unwrap().clone();
    assert_eq!(item.state, State::Answered, "{item:?}");
    assert_eq!(
        item.answer.as_deref(),
        Some("answered in the run: the order went on (delivered)")
    );
}

#[test]
fn a_worker_question_stays_open_while_its_order_waits() {
    let temp = Temp::new("waits");
    let p = temp.project();
    write_journal(
        &temp.0,
        "r1",
        &[
            seated(1, "w1", "ops.terminal", None),
            asked(2, "w1", "Which palette?"),
        ],
    );
    refresh(&p, NOW).unwrap();
    // The journal grows with an unrelated event; the question is still open.
    write_journal(
        &temp.0,
        "r1",
        &[
            seated(1, "w1", "ops.terminal", None),
            asked(2, "w1", "Which palette?"),
            json!({"v":1,"seq":3,"at":NOW,"event":"woken","via":"notice","through":2,"digest":"d"}),
        ],
    );
    let q = refresh(&p, NOW + 1).unwrap();
    assert_eq!(q.items[0].state, State::Open, "{:?}", q.items);
}
