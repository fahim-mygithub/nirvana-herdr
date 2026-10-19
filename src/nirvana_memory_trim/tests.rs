//! Plan 13 P3: the memory trim. The live Jev is never reached: a fake client answers from the question it is sent. Every
//! test works in a scratch home and a scratch projects folder; nothing here reads or writes `~/.claude`, the encoding
//! test's folder names are pairs captured from it once and written down below.

use std::sync::{Arc, Mutex};

use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("nirvana-memory-trim-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn ctx(name: &str, limit: u64) -> Ctx {
    let root = scratch(name);
    Ctx {
        home: root.join("home"),
        projects: root.join("projects"),
        limit,
    }
}

const FOLDER: &str = "C:/work/proj";

/// An index of `n` entries: a short title and a hook of about thirty words each, in the shape of the real ones.
fn index_text(n: usize) -> String {
    let mut text = String::new();
    for i in 0..n {
        text.push_str(&format!(
            "- [Entry {i}](entry-{i}.md) — the hook for entry {i} says a good many words about what the file holds, when to open it, and what went wrong the last time it was ignored by anyone\n"
        ));
    }
    text
}

fn put_index(ctx: &Ctx, text: &str) -> PathBuf {
    let path = index_path(ctx, Path::new(FOLDER)).expect("a path");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

type Rewrite = dyn Fn(&str) -> Result<String, String>;

struct Fake(Box<Rewrite>);

impl Trimmer for Fake {
    fn rewrite(&self, original: &str, _limit: u64) -> Result<String, String> {
        (self.0)(original)
    }
}

/// A fake that keeps every link and drops every hook.
fn heads_only() -> Fake {
    Fake(Box::new(|text| {
        Ok(text
            .lines()
            .map(|l| format!("{}\n", l.split(" — ").next().unwrap()))
            .collect())
    }))
}

#[test]
fn the_folder_encoding_matches_the_names_claude_code_wrote() {
    // Pairs read from the real ~/.claude/projects on 2026-10-18.
    for (folder, name) in [
        (
            r"C:\Users\fahim\Desktop\Pojects\Nirvana",
            "C--Users-fahim-Desktop-Pojects-Nirvana",
        ),
        (
            r"C:\Users\fahim\.nirvana\agent-ask-20261013T160757-b500",
            "C--Users-fahim--nirvana-agent-ask-20261013T160757-b500",
        ),
        (
            r"\\wsl.localhost\Ubuntu-24.04\home\fahim\WSL\Projects\Sombra",
            "--wsl-localhost-Ubuntu-24-04-home-fahim-WSL-Projects-Sombra",
        ),
        (
            r"C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow",
            "C--Users-fahim-Desktop-Pojects-3D-Printer-Flow",
        ),
    ] {
        assert_eq!(encode(Path::new(folder)), name, "{folder}");
    }
    assert_eq!(
        encode(Path::new(r"C:\Users\fahim\Desktop\Pojects\Nirvana\")),
        "C--Users-fahim-Desktop-Pojects-Nirvana",
        "a trailing separator is not part of the name"
    );
}

#[test]
fn the_index_is_found_under_the_projects_root_and_nowhere_else() {
    let c = ctx("where", 4000);
    let path = index_path(&c, Path::new(r"C:\work\proj")).unwrap();
    assert_eq!(
        path,
        c.projects
            .join("C--work-proj")
            .join("memory")
            .join("MEMORY.md")
    );
    assert!(path.starts_with(&c.projects));
    assert!(index_path(&c, Path::new(&"x".repeat(250))).is_none());
}

#[test]
fn status_counts_tokens_flags_over_and_is_none_without_an_index() {
    let c = ctx("status", 4000);
    assert!(status_in(&c, Path::new(FOLDER)).is_none());
    put_index(&c, &index_text(10));
    let s = status_in(&c, Path::new(FOLDER)).expect("an index");
    assert_eq!(s.tokens, content_tokens(index_text(10).len()));
    assert!(!s.over && s.proposal.is_none() && s.backup.is_none());
    put_index(&c, &index_text(100));
    let s = status_in(&c, Path::new(FOLDER)).unwrap();
    assert!(s.over, "{} tokens over {}", s.tokens, s.limit);
    assert_eq!(s.limit, 4000);
}

#[test]
fn the_limit_comes_from_preload_toml_and_falls_back_to_the_default() {
    let home = scratch("limit");
    assert_eq!(limit_in(&home), DEFAULT_LIMIT_TOKENS);
    std::fs::create_dir_all(home.join("nirvana")).unwrap();
    let file = home.join("nirvana").join("preload.toml");
    std::fs::write(&file, "trim_over_tokens = 1234\n[preset.x]\nkeep = []\n").unwrap();
    assert_eq!(limit_in(&home), 1234);
    std::fs::write(&file, "trim_over_tokens = \"lots\"\n").unwrap();
    assert_eq!(limit_in(&home), DEFAULT_LIMIT_TOKENS);
    std::fs::write(&file, "not toml [[[").unwrap();
    assert_eq!(limit_in(&home), DEFAULT_LIMIT_TOKENS);
}

#[test]
fn the_pointer_check_refuses_a_proposal_that_drops_a_pointer_and_writes_nothing() {
    let c = ctx("refuse", 400);
    let original = index_text(40);
    let path = put_index(&c, &original);
    // A fake Jev that drops entry 7 whole: shorter, and wrong.
    let fake = Fake(Box::new(|text| {
        Ok(text
            .lines()
            .filter(|l| !l.contains("entry-7.md"))
            .map(|l| format!("{}\n", l.split(" — ").next().unwrap()))
            .collect())
    }));
    let err = propose_in(&c, Path::new(FOLDER), &fake).unwrap_err();
    assert!(err.contains("entry-7.md"), "{err}");
    assert!(
        !proposal_path(&path).exists(),
        "a refused proposal is not written"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        original,
        "the index is untouched"
    );
    // A link kept but a [[link]] lost is refused too.
    let linked = format!("{original}- [Linked](linked.md) — see [[other-memory]] for why\n");
    put_index(&c, &linked);
    let fake = Fake(Box::new(|text| {
        Ok(text
            .lines()
            .map(|l| format!("{}\n", l.split(" — ").next().unwrap()))
            .collect())
    }));
    assert!(propose_in(&c, Path::new(FOLDER), &fake)
        .unwrap_err()
        .contains("[[other-memory]]"));
    // An index with no pointers at all: the empty proposal passes the pointer check and must be refused for itself.
    put_index(
        &c,
        &"a plain line with no link in it at all, repeated
"
        .repeat(60),
    );
    assert!(propose_in(
        &c,
        Path::new(FOLDER),
        &Fake(Box::new(|_| Ok("  
"
        .into())))
    )
    .unwrap_err()
    .contains("empty"));
    put_index(&c, &linked);
    // Empty and not-smaller proposals are refused.
    assert!(propose_in(
        &c,
        Path::new(FOLDER),
        &Fake(Box::new(|_| Ok("  \n".into())))
    )
    .is_err());
    assert!(
        propose_in(&c, Path::new(FOLDER), &Fake(Box::new(|t| Ok(t.to_owned()))))
            .unwrap_err()
            .contains("not smaller")
    );
    assert!(!proposal_path(&path).exists());
}

#[test]
fn a_good_proposal_is_written_beside_the_index_and_the_index_is_not_touched() {
    let c = ctx("good", 400);
    let original = index_text(40);
    let path = put_index(&c, &original);
    let s = propose_in(&c, Path::new(FOLDER), &heads_only()).expect("proposed");
    let proposal = proposal_path(&path);
    assert_eq!(proposal.parent(), path.parent(), "beside the index");
    assert!(proposal.exists());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let p = s.proposal.expect("status shows the proposal");
    assert!(p.saved > 0 && p.tokens + p.saved == s.tokens);
}

#[test]
fn a_small_index_has_nothing_to_trim() {
    let c = ctx("small", 4000);
    put_index(&c, &index_text(5));
    let calls = Arc::new(Mutex::new(0));
    let seen = calls.clone();
    let fake = Fake(Box::new(move |t| {
        *seen.lock().unwrap() += 1;
        Ok(t.to_owned())
    }));
    assert!(propose_in(&c, Path::new(FOLDER), &fake)
        .unwrap_err()
        .contains("nothing to trim"));
    assert_eq!(*calls.lock().unwrap(), 0, "and Jev is not asked");
}

#[test]
fn accept_keeps_a_byte_identical_backup_then_replaces_the_index_and_restore_puts_it_back() {
    let c = ctx("accept", 400);
    let original = index_text(40);
    let path = put_index(&c, &original);
    propose_in(&c, Path::new(FOLDER), &heads_only()).unwrap();
    let proposal_text = std::fs::read_to_string(proposal_path(&path)).unwrap();
    let done = accept_in(&c, Path::new(FOLDER)).expect("accepted");
    assert_eq!(
        std::fs::read(&done.backup).unwrap(),
        original.as_bytes(),
        "the backup is the original, byte for byte"
    );
    assert!(done
        .backup
        .starts_with(c.home.join("nirvana").join("backup").join("memory")));
    assert!(done
        .backup
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with(&crate::nirvana_launch::folder_hash(Path::new(FOLDER))));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        proposal_text,
        "the index is the proposal"
    );
    assert!(!proposal_path(&path).exists());
    assert!(done.after < done.before);
    // The status now offers the backup.
    assert_eq!(
        status_in(&c, Path::new(FOLDER)).unwrap().backup,
        Some(done.backup.clone())
    );
    // Restore puts the original back, byte for byte.
    let used = restore_in(&c, Path::new(FOLDER)).expect("restored");
    assert_eq!(used, done.backup);
    assert_eq!(std::fs::read(&path).unwrap(), original.as_bytes());
}

#[test]
fn a_second_accept_makes_its_own_backup_and_restore_takes_the_newest() {
    let c = ctx("twice", 400);
    let path = put_index(&c, &index_text(40));
    propose_in(&c, Path::new(FOLDER), &heads_only()).unwrap();
    let first = accept_in(&c, Path::new(FOLDER)).unwrap();
    std::fs::write(&path, index_text(40)).unwrap();
    propose_in(&c, Path::new(FOLDER), &heads_only()).unwrap();
    let second = accept_in(&c, Path::new(FOLDER)).unwrap();
    assert_ne!(first.backup, second.backup);
    assert!(first.backup.exists() && second.backup.exists());
    // Restore takes the newest.
    assert_eq!(restore_in(&c, Path::new(FOLDER)).unwrap(), second.backup);
}

#[test]
fn accept_checks_again_against_the_index_as_it_is_now() {
    let c = ctx("stale", 400);
    let path = put_index(&c, &index_text(40));
    propose_in(&c, Path::new(FOLDER), &heads_only()).unwrap();
    // A session adds an entry after the proposal was made.
    let mut grown = std::fs::read_to_string(&path).unwrap();
    grown.push_str("- [Brand new](brand-new.md) — written after the proposal\n");
    std::fs::write(&path, &grown).unwrap();
    assert!(accept_in(&c, Path::new(FOLDER))
        .unwrap_err()
        .contains("brand-new.md"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        grown,
        "the index is untouched"
    );
    assert!(
        !c.home.join("nirvana").join("backup").exists(),
        "and no backup was made"
    );
    assert!(accept_in(&ctx("stale-none", 400), Path::new(FOLDER)).is_err());
}

#[test]
fn discard_deletes_the_proposal_and_only_the_proposal() {
    let c = ctx("discard", 400);
    let original = index_text(40);
    let path = put_index(&c, &original);
    assert_eq!(discard_in(&c, Path::new(FOLDER)), Ok(false));
    propose_in(&c, Path::new(FOLDER), &heads_only()).unwrap();
    assert_eq!(discard_in(&c, Path::new(FOLDER)), Ok(true));
    assert!(!proposal_path(&path).exists());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    assert!(
        restore_in(&c, Path::new(FOLDER)).is_err(),
        "no backup, nothing to restore"
    );
}

// ---- the assembly, and the fake Jev ------------------------------------------------------------------

#[test]
fn the_ladder_stops_at_the_limit_and_keeps_every_link() {
    let original = index_text(30);
    let needs: Vec<Need> = (0..30)
        .map(|i| match i % 3 {
            0 => Need::Redundant,
            1 => Need::Helpful,
            _ => Need::Needed,
        })
        .collect();
    let full = tokens_of(&original);
    // Under the limit already: nothing changes.
    assert_eq!(assemble(&original, &needs, full), original);
    // A limit just under: only the redundant hooks go.
    let mild = assemble(&original, &needs, full - 10);
    assert!(mild.lines().filter(|l| !l.contains(" — ")).count() >= 10);
    assert!(tokens_of(&mild) <= full - 10);
    assert!(
        mild.contains("the hook for entry 2 says a good many words about what the file holds"),
        "a needed hook is whole at the mild step"
    );
    // A hard limit: everything the ladder can do.
    let hard = assemble(&original, &needs, 100);
    assert!(tokens_of(&hard) < tokens_of(&mild));
    assert_eq!(
        pointers(&hard),
        pointers(&original),
        "every link survives the harshest cut"
    );
    assert_eq!(pointers(&mild), pointers(&original));
}

#[test]
fn a_hook_that_holds_a_pointer_is_never_cut_and_unknown_lines_pass_through() {
    let original = "# Memory\n\n- [A](a.md) — plain hook with words words words words words words words words words words words\n- [B](b.md) — see [[other]] and [c](c.md)\n  continued here\nnot an entry at all\n";
    let needs = [Need::Redundant, Need::Redundant];
    let out = assemble(original, &needs, 1);
    assert!(out.contains("- [A](a.md)\n"));
    assert!(out.contains("see [[other]] and [c](c.md)"), "{out}");
    assert!(out.contains("continued here"));
    assert!(out.starts_with("# Memory\n\n"));
    assert!(out.ends_with("not an entry at all\n"));
    assert_eq!(pointers(&out), pointers(original));
    // A continuation line follows its entry into the cut.
    let wrapped = "- [W](w.md) — first line\n  second line\n";
    assert_eq!(assemble(wrapped, &[Need::Redundant], 1), "- [W](w.md)\n");
}

#[test]
fn parse_and_render_without_cuts_give_the_text_back() {
    let text = "# T\n\n- [A](a.md) — x\n  y\n- plain bullet without a link\n- [B](b.md)\n\ntail\n";
    assert_eq!(render(&parse(text), &[]), text);
    let entries = parse(text)
        .iter()
        .filter(|b| matches!(b, Block::Entry(_)))
        .count();
    assert_eq!(entries, 2);
}

/// A fake Jev transport: every question is answered by `say`, from the entry number in the question's id.
fn jev_client(
    say: impl Fn(usize) -> (&'static str, f64) + Send + Sync + 'static,
    seen: Arc<Mutex<Vec<String>>>,
) -> Client {
    Client::mock(Box::new(move |_url, body, _key, _left| {
        seen.lock().unwrap().push(body.to_owned());
        let sent: Value = serde_json::from_str(body).unwrap();
        let mut answers = Map::new();
        for (id, question) in sent["questions"].as_object().unwrap() {
            let n: usize = id.trim_start_matches('e').parse().unwrap();
            let (choice, confidence) = say(n);
            let keys: Vec<String> = question["criteria"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            assert!(
                keys.contains(&choice.to_owned()),
                "{choice} not among {keys:?}"
            );
            let rest = (1.0 - confidence) / (keys.len() as f64 - 1.0);
            let probabilities: Map<String, Value> = keys
                .iter()
                .map(|k| {
                    (
                        k.clone(),
                        json!(if k == choice { confidence } else { rest }),
                    )
                })
                .collect();
            answers.insert(
                id.clone(),
                json!({"type": "choice", "choice": choice, "confidence": confidence, "probabilities": probabilities}),
            );
        }
        Ok((
            200,
            json!({"model": "jev-test", "answers": answers, "usage": {"input_tokens": 900, "output_tokens": 12, "cost": 0.0003}})
                .to_string(),
            None,
        ))
    }))
}

#[test]
fn jev_judges_each_entry_and_the_result_is_assembled_by_code() {
    let c = ctx("jev", 2000);
    let original = index_text(60);
    let path = put_index(&c, &original);
    let seen = Arc::new(Mutex::new(Vec::new()));
    // Entries 0..30 redundant, the rest needed; entry 31 is redundant but unsure.
    let client = jev_client(
        |n| {
            if n == 31 {
                ("redundant", 0.4)
            } else if n < 30 {
                ("redundant", 0.95)
            } else {
                ("needed", 0.9)
            }
        },
        seen.clone(),
    );
    let trimmer = JevTrimmer::new(client, Some("mock-key".into()), c.home.clone());
    let s = propose_in(&c, Path::new(FOLDER), &trimmer).expect("proposed");
    let proposed = std::fs::read_to_string(proposal_path(&path)).unwrap();
    assert!(s.proposal.unwrap().saved > 0);
    assert!(
        proposed.contains("- [Entry 0](entry-0.md)\n"),
        "a confident redundant hook is dropped"
    );
    assert!(
        proposed.contains("the hook for entry 40 says a good many words about what the file holds"),
        "a needed hook is kept"
    );
    assert!(
        proposed.contains("the hook for entry 31 says"),
        "an unsure answer never cuts"
    );
    assert_eq!(pointers(&proposed), pointers(&original));
    // One call for 60 entries; the log has one line, with no memory text in it.
    assert_eq!(seen.lock().unwrap().len(), 1);
    let log = std::fs::read_to_string(c.home.join("nirvana").join("jev-calls.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 1);
    let line: Value = serde_json::from_str(log.lines().next().unwrap()).unwrap();
    assert_eq!(line["role"], "memory-trim");
    assert_eq!(line["outcome"], "answered");
    assert!(
        !log.contains("hook for entry"),
        "the log carries no memory text"
    );
    // The text that was sent carries titles and hooks only.
    assert!(seen.lock().unwrap()[0].contains("hook for entry 5"));
}

#[test]
fn a_failed_jev_call_makes_no_proposal_and_is_logged_as_failed() {
    let c = ctx("jevfail", 700);
    let original = index_text(60);
    let path = put_index(&c, &original);
    let client = Client::mock(Box::new(|_, _, _, _| Ok((503, String::new(), None))));
    let trimmer = JevTrimmer::new(client, None, c.home.clone());
    let err = propose_in(&c, Path::new(FOLDER), &trimmer).unwrap_err();
    assert!(err.contains("Jev failed"), "{err}");
    assert!(!proposal_path(&path).exists());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let log = std::fs::read_to_string(c.home.join("nirvana").join("jev-calls.jsonl")).unwrap();
    assert!(log.contains("\"outcome\":\"failed\""), "{log}");
}

#[test]
fn a_secret_in_a_hook_is_redacted_before_it_is_sent() {
    let c = ctx("redact", 100);
    let secret = "sk-or-v1-0123456789abcdef0123456789abcdef0123456789abcdef";
    let text = format!(
        "{}- [Key](key.md) — the key is {secret} ok\n",
        index_text(20)
    );
    put_index(&c, &text);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let trimmer = JevTrimmer::new(
        jev_client(|_| ("needed", 0.9), seen.clone()),
        Some("mock-key".into()),
        c.home.clone(),
    );
    let _ = propose_in(&c, Path::new(FOLDER), &trimmer);
    assert!(!seen.lock().unwrap().is_empty());
    assert!(seen
        .lock()
        .unwrap()
        .iter()
        .all(|body| !body.contains(secret)));
}

#[test]
fn when_the_ladder_is_not_enough_needed_hooks_are_dropped_one_at_a_time_oldest_first() {
    // Hooks of five words, all needed: the word cuts change nothing, so only whole drops can reach the limit.
    let mut original = String::new();
    for i in 0..40 {
        original.push_str(&format!(
            "- [Entry {i}](entry-{i}.md) — five words of hook here
"
        ));
    }
    let needs = vec![Need::Needed; 40];
    let full = tokens_of(&original);
    let limit = full - 40;
    let out = assemble(&original, &needs, limit);
    assert!(tokens_of(&out) <= limit, "{} over {limit}", tokens_of(&out));
    let bare = |n: usize| {
        out.contains(&format!(
            "- [Entry {n}](entry-{n}.md)
"
        ))
    };
    assert!(bare(0), "the oldest hook goes first");
    assert!(!bare(39), "and the newest keeps its hook");
    assert_eq!(pointers(&out), pointers(&original));
    // One at a time: the cut stops as soon as the index fits, so not every hook is gone.
    let dropped = (0..40).filter(|n| bare(*n)).count();
    assert!((1..40).contains(&dropped), "{dropped} of 40 dropped");
    // Entries Jev called redundant or helpful went before any needed one was touched.
    let mixed: Vec<Need> = (0..40)
        .map(|i| {
            if i % 2 == 0 {
                Need::Helpful
            } else {
                Need::Needed
            }
        })
        .collect();
    let out = assemble(&original, &mixed, full - 20);
    assert!(
        out.contains(
            "- [Entry 0](entry-0.md)
"
        ) && out.contains("five words of hook here")
    );
    assert!(
        out.contains("- [Entry 1](entry-1.md) — five words of hook here"),
        "a needed hook stays while helpful ones can go"
    );
}

// ---- the fix round -----------------------------------------------------------------------------------

#[test]
fn no_test_of_this_package_reads_the_owners_claude_folder() {
    // A scan of the test sources, kept only because the live read it guards against came back once (the encoding test read
    // the owner's real projects folder). The needles are built in two pieces so this file does not contain them itself.
    let needles = [
        concat!("join(\".cla", "ude\")"),
        concat!("var_os(\"USERPRO", "FILE\")"),
        concat!("var_os(\"HO", "ME\")"),
    ];
    for (file, source) in [
        ("nirvana_memory_trim/tests.rs", include_str!("tests.rs")),
        (
            "nirvana_session_record/tests.rs",
            include_str!("../nirvana_session_record/tests.rs"),
        ),
    ] {
        for needle in needles {
            assert!(
                !source.contains(needle),
                "{file} reads the owner's home: {needle}"
            );
        }
    }
}

fn only(key: &str, count: usize) -> Pointers {
    Pointers::from([(key.to_owned(), count)])
}

#[test]
fn a_link_target_with_parentheses_parses_whole_and_survives_a_cut() {
    let original = "- [Spec](docs/spec_(v2).md) — the v2 hook words\n";
    let blocks = parse(original);
    match &blocks[0] {
        Block::Entry(entry) => {
            assert_eq!(entry.head, "- [Spec](docs/spec_(v2).md)");
            assert_eq!(entry.hook, " — the v2 hook words");
            assert_eq!(entry.title(), "Spec");
        }
        other => panic!("not an entry: {other:?}"),
    }
    assert_eq!(pointers(original), only("](docs/spec_(v2).md)", 1));
    assert_eq!(
        assemble(original, &[Need::Redundant], 1),
        "- [Spec](docs/spec_(v2).md)\n",
        "the hook goes and the whole target stays"
    );
    // Parentheses in the hook are not the link's.
    let plain = parse("- [A](a.md) — (see above)\n");
    match &plain[0] {
        Block::Entry(entry) => assert_eq!(entry.head, "- [A](a.md)"),
        other => panic!("not an entry: {other:?}"),
    }
    // A link that never closes still counts, to the end of its line.
    assert_eq!(pointers("- [A](a.md"), only("](a.md", 1));
}

#[test]
fn pointers_are_counted_so_dropping_one_of_two_links_to_a_file_is_refused() {
    let original = "- [A](x.md) — first hook words words words words\n- [B](x.md) — second hook words words words words\n";
    assert_eq!(pointers(original), only("](x.md)", 2));
    let err = check(original, "- [A](x.md)\n").unwrap_err();
    assert!(err.contains("x.md") && err.contains("drops"), "{err}");
    assert!(
        check(original, "- [A](x.md)\n- [B](x.md)\n").is_ok(),
        "both kept is fine"
    );
}

#[test]
fn a_hook_naming_a_file_in_another_form_is_never_cut_and_its_pointer_is_counted() {
    for (form, needle) in [
        ("see `notes/b.md` first", "notes/b.md"),
        ("see `b.md` first", "b.md"),
        ("see b.md first", "b.md"),
        ("see (b.md), then more", "b.md"),
        ("see <docs/b.md> first", "docs/b.md"),
        ("see [the doc][r] first", "[r]"),
        ("see [r][] first", "[r]"),
    ] {
        let original = format!("- [A](a.md) — {form}\n- [Z](z.md) — filler words filler words\n");
        let out = assemble(&original, &[Need::Redundant, Need::Redundant], 1);
        assert!(out.contains(form), "{form}: cut to {out:?}");
        assert!(
            out.contains("- [Z](z.md)\n"),
            "{form}: the plain neighbour is cut"
        );
        let err = check(&original, "- [A](a.md)\n- [Z](z.md)\n").unwrap_err();
        assert!(
            err.contains(needle) && err.contains("drops"),
            "{form}: {err}"
        );
    }
    // A reference definition on the line after a plain hook belongs to the entry and keeps it whole.
    let original = "- [A](a.md) — plain words\n[r]: docs/b.md\n- [Z](z.md) — filler words\n";
    let out = assemble(original, &[Need::Redundant, Need::Redundant], 1);
    assert!(out.contains("[r]: docs/b.md"), "{out:?}");
    assert!(check(original, "- [A](a.md)\n- [Z](z.md)\n")
        .unwrap_err()
        .contains("[r]: docs/b.md"));
    // Counts, here too.
    assert_eq!(pointers("see b.md and b.md"), only("b.md", 2));
    assert!(check(
        "- [A](a.md) — see b.md and b.md and more words here\n",
        "- [A](a.md) — see b.md\n"
    )
    .is_err());
}

#[test]
fn plain_hooks_stay_cuttable_and_a_title_or_code_or_a_tag_is_not_a_pointer() {
    let line = "- [A](a.md) — uses `cargo`, <b>bold</b> and e.g. more, x < y > z";
    assert_eq!(pointers(line), only("](a.md)", 1));
    let original = format!("{line}\n- [Z](z.md) — filler\n");
    assert_eq!(
        assemble(&original, &[Need::Redundant, Need::Redundant], 1),
        "- [A](a.md)\n- [Z](z.md)\n"
    );
    // A file name in a link's title is read once, as the link.
    assert_eq!(pointers("- [notes.md](notes.md)"), only("](notes.md)", 1));
}

#[test]
fn a_relative_folder_is_made_absolute_before_it_is_encoded() {
    let cwd = std::env::current_dir().unwrap();
    let got = folder_arg(Some("rel-dir")).unwrap();
    assert_eq!(got, cwd.join("rel-dir"));
    assert_eq!(encode(&got), encode(&cwd.join("rel-dir")));
    assert_ne!(
        encode(&got),
        encode(Path::new("rel-dir")),
        "the relative spelling is a name Claude Code never wrote"
    );
    assert_eq!(
        crate::nirvana_launch::folder_hash(&got),
        crate::nirvana_launch::folder_hash(&cwd.join("rel-dir"))
    );
    assert_eq!(folder_arg(None).unwrap(), cwd, "no argument is the cwd");
    // The index is looked for under the absolute folder's name.
    let c = ctx("relative", 400);
    let path = index_path(&c, &got).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, index_text(40)).unwrap();
    assert!(status_in(&c, &got).is_some());
    assert!(status_in(&c, Path::new("rel-dir")).is_none());
}

#[cfg(windows)]
#[test]
fn a_verbatim_prefix_is_dropped_from_the_folder_argument() {
    let plain = folder_arg(Some(r"C:\work\proj")).unwrap();
    let verbatim = folder_arg(Some(r"\\?\C:\work\proj")).unwrap();
    assert_eq!(verbatim, plain);
    assert_eq!(encode(&verbatim), "C--work-proj");
    assert_eq!(
        crate::nirvana_launch::folder_hash(&verbatim),
        crate::nirvana_launch::folder_hash(&plain)
    );
}

#[test]
fn write_atomic_leaves_a_neighbour_the_old_temp_name_would_have_taken() {
    let dir = scratch("atomic");
    // `with_extension("tmp.<pid>")` made `MEMORY.tmp.<pid>` out of `MEMORY.md`: this file was in its way, and the rename
    // took it.
    let neighbour = dir.join(format!("MEMORY.tmp.{}", std::process::id()));
    std::fs::write(&neighbour, "keep").unwrap();
    let target = dir.join("MEMORY.md");
    write_atomic(&target, b"new").unwrap();
    write_atomic(&target, b"newer").unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "newer");
    assert_eq!(std::fs::read_to_string(&neighbour).unwrap(), "keep");
    let left = std::fs::read_dir(&dir).unwrap().flatten().count();
    assert_eq!(left, 2, "the target and the neighbour, no temporary file");
}
