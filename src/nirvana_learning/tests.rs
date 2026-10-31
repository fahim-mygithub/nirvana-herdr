use super::*;
use crate::nirvana_gate::axes_values;
use crate::nirvana_lessons::{Entry as LessonEntry, Index as LessonIndex, LedgerRow, Snap};
use crate::nirvana_run::Seated;
use crate::nirvana_seat::{Effort, Mode, SizeName};

pub(crate) fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "herdr-p36-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn entry(
    name: &str,
    class: &str,
    origin: Origin,
    profile: Option<&str>,
    c: [u32; 3],
) -> LessonEntry {
    LessonEntry {
        name: name.into(),
        class: class.into(),
        description: name.into(),
        order: "o1".into(),
        run: "r1".into(),
        written_at: 1,
        counters: Counters {
            offered: c[0],
            opened: c[1],
            passed_after_opened: c[2],
        },
        seen: Vec::new(),
        profile: profile.map(str::to_owned),
        origin,
    }
}

fn snap(slug: &str, file: Option<&str>) -> Snap {
    Snap {
        slug: slug.into(),
        file: file.map(str::to_owned),
        entry: None,
    }
}

fn lesson_row(
    id: &str,
    at: u64,
    op: &str,
    before: Vec<Snap>,
    after: Vec<Snap>,
    of: Option<&str>,
) -> LedgerRow {
    LedgerRow {
        v: 1,
        id: id.into(),
        at,
        op: op.into(),
        by: if op == "rollback" { "herdr" } else { "o1-lr" }.into(),
        run: "r1".into(),
        before,
        after,
        of: of.map(str::to_owned),
    }
}

pub(crate) fn card(name: &str, verdict: &str) -> String {
    format!("---\nname: {name}\nwhen: a reel opens\nverdict: {verdict}\nsource: notes/reel.md:1\n---\nThe body of {name}.\n")
}

fn card_row(
    id: &str,
    at: u64,
    op: &str,
    slug: &str,
    before: Option<&str>,
    after: Option<&str>,
    of: Option<&str>,
) -> knowledge::Row {
    knowledge::Row {
        v: 1,
        id: id.into(),
        at,
        op: op.into(),
        by: if op == "rollback" { "herdr" } else { "t1" }.into(),
        run: "r1".into(),
        slug: slug.into(),
        before: before.map(str::to_owned),
        after: after.map(str::to_owned),
        evidence: None,
        of: of.map(str::to_owned),
    }
}

fn jsonl<T: serde::Serialize>(rows: &[T]) -> String {
    rows.iter()
        .map(|row| serde_json::to_string(row).expect("row") + "\n")
        .collect()
}

fn seated(order: &str, class: &str) -> Seated {
    Seated {
        order: order.into(),
        seat: "flash".into(),
        cli: "agy".into(),
        model: "gemini-flash".into(),
        effort: Effort::Low,
        pool: "agy".into(),
        class: class.into(),
        chain: None,
        size: SizeName::Small,
        mode: Mode::Efficient,
        needs: Vec::new(),
        not_family: Vec::new(),
        reason: "r".into(),
        override_reason: None,
        order_file: "o.md".into(),
        cwd: "C:/p".into(),
        worktree: None,
        branch: None,
        parent: None,
        why: Some("lesson_review".into()),
        rule_pick: None,
        bar_dropped: None,
        step_down: None,
        use_basis: None,
        expected: BTreeMap::new(),
        jev_pick: None,
        send_ok: None,
    }
}

fn gated(order: &str, verdict: &str, axes: &[Axis]) -> Kind {
    Kind::Gated {
        order: order.into(),
        verdict: verdict.into(),
        reason: None,
        axes: axes_values(axes),
    }
}

/// The fixture's slugs and ids, for the tests that read them back.
pub(crate) const PATCHED: &str = "slice-with-brim";
pub(crate) const PROFILE: &str = "reel-editor";

/// A project with each thing the panel shows: three lessons (live, decayed, a profile's), a lessons ledger with a new,
/// a patch, a merge and a rollback, a folder profile whose cards are known by the ledger, known by adoption, not known,
/// a warning and a refused one, a one-file profile, a class over the merge line, and a run whose journal seats six
/// reviews today (done with one op kept and one refused, held back, failed, nothing to keep, every op refused, not
/// gated) and one yesterday. `now` is the fixture's today.
pub(crate) fn fixture(root: &Path, now: u64) {
    let lessons = nirvana_lessons::dir(root);
    std::fs::create_dir_all(&lessons).expect("lessons");
    let mut index = LessonIndex {
        v: 1,
        ..LessonIndex::default()
    };
    index.lessons.insert(
        PATCHED.into(),
        entry(
            "Slice with a brim",
            "print",
            Origin::Worker,
            None,
            [4, 2, 1],
        ),
    );
    index.lessons.insert(
        "level-the-bed".into(),
        entry(
            "Level the bed first",
            "print",
            Origin::Review,
            None,
            [9, 1, 0],
        ),
    );
    index.lessons.insert(
        "cut-on-the-beat".into(),
        entry(
            "Cut on the beat",
            "reel",
            Origin::Merge,
            Some(PROFILE),
            [2, 2, 2],
        ),
    );
    // Nine more live `print` lessons with no profile: the class is over `MERGE_ABOVE`, so a merge waits.
    for n in 0..9 {
        index.lessons.insert(
            format!("print-tip-{n}"),
            entry(
                &format!("Print tip {n}"),
                "print",
                Origin::Worker,
                None,
                [0, 0, 0],
            ),
        );
    }
    std::fs::write(
        lessons.join(nirvana_lessons::INDEX),
        serde_json::to_string_pretty(&index).expect("index"),
    )
    .expect("index");
    std::fs::write(lessons.join(format!("{PATCHED}.md")), "v2\n").expect("lesson");
    std::fs::write(lessons.join("level-the-bed.md"), "level\n").expect("lesson");
    std::fs::write(lessons.join("cut-on-the-beat.md"), "beat\n").expect("lesson");
    std::fs::write(
        lessons.join(nirvana_lessons::LEDGER),
        jsonl(&[
            lesson_row(
                "op-1",
                now - 400,
                "new",
                vec![snap("level-the-bed", None)],
                vec![snap("level-the-bed", Some("level\n"))],
                None,
            ),
            lesson_row(
                "op-2",
                now - 300,
                "patch",
                vec![snap(PATCHED, Some("v1\n"))],
                vec![snap(PATCHED, Some("v2\n"))],
                None,
            ),
            lesson_row(
                "op-3",
                now - 200,
                "merge",
                vec![snap("beat-a", Some("a\n")), snap("beat-b", Some("b\n"))],
                vec![
                    snap("cut-on-the-beat", Some("beat\n")),
                    snap("beat-a", None),
                    snap("beat-b", None),
                ],
                None,
            ),
            lesson_row(
                "op-4",
                now - 100,
                "new",
                vec![snap("gone", None)],
                vec![snap("gone", Some("g\n"))],
                None,
            ),
            lesson_row(
                "op-5",
                now - 50,
                "rollback",
                vec![snap("gone", Some("g\n"))],
                vec![snap("gone", None)],
                Some("op-4"),
            ),
        ]),
    )
    .expect("ledger");

    let profiles = profile_files::folder(root);
    let knowledge = profiles.join(PROFILE).join(profile_files::KNOWLEDGE);
    std::fs::create_dir_all(&knowledge).expect("knowledge");
    std::fs::write(
        profiles.join(PROFILE).join(profile_files::PROFILE_FILE),
        "---\ndescription: Edits reels.\n---\nYou edit reels.\n",
    )
    .expect("profile");
    std::fs::write(
        profiles.join("plain.md"),
        "---\ndescription: Plain.\n---\nPlain.\n",
    )
    .expect("plain");
    let (ledgered, adopted, stray, warning) = (
        card("Closer", "keep"),
        card("Opener", "keep"),
        card("Stray", "keep"),
        card("No jump cuts", "reject"),
    );
    std::fs::write(knowledge.join("closer.md"), &ledgered).expect("card");
    std::fs::write(knowledge.join("opener.md"), &adopted).expect("card");
    std::fs::write(knowledge.join("stray.md"), &stray).expect("card");
    std::fs::write(knowledge.join("no-jump-cuts.md"), &warning).expect("card");
    std::fs::write(knowledge.join("broken.md"), "no front matter\n").expect("card");
    let closer_v1 = card("Closer v1", "keep");
    std::fs::write(
        knowledge.join(knowledge::LEDGER),
        jsonl(&[
            card_row(
                "op-1",
                now - 250,
                "new",
                "closer",
                None,
                Some(&closer_v1),
                None,
            ),
            card_row(
                "op-2",
                now - 150,
                "patch",
                "closer",
                Some(&closer_v1),
                Some(&ledgered),
                None,
            ),
            card_row(
                "op-3",
                now - 120,
                "new",
                "no-jump-cuts",
                None,
                Some(&warning),
                None,
            ),
        ]),
    )
    .expect("card ledger");
    let mut kindex = knowledge::Index::default();
    kindex.cards.insert(
        "opener".into(),
        knowledge::Entry {
            adopted: knowledge::card_sha(&knowledge.join("opener.md")),
            ..knowledge::Entry::default()
        },
    );
    std::fs::write(
        knowledge.join(knowledge::INDEX),
        serde_json::to_string_pretty(&kindex).expect("index"),
    )
    .expect("card index");

    let project = Project::at(root);
    let journal = project.journal("r1");
    std::fs::create_dir_all(journal.dir()).expect("run");
    let seat = |order: &str, at: u64| {
        journal
            .append(at, Kind::Seated(Box::new(seated(order, review::CLASS))))
            .expect("seated");
    };
    seat("old-lr", now - 2 * 86_400);
    journal
        .append(
            now - 2 * 86_400 + 5,
            gated("old-lr", "failed", &[Axis::new("report", Mark::Fail, "x")]),
        )
        .expect("gated");
    journal
        .append(
            now - 60,
            Kind::Seated(Box::new(seated("build-1", "build.bounded"))),
        )
        .expect("seated");
    for order in ["a-lr", "b-lr", "c-lr", "d-lr", "e-lr", "f-lr"] {
        seat(order, now - 40);
    }
    let gate = |order: &str, at: u64, verdict: &str, axes: &[Axis]| {
        journal
            .append(at, gated(order, verdict, axes))
            .expect("gated");
    };
    gate(
        "a-lr",
        now - 30,
        "pass",
        &[
            Axis::new("report", Mark::Pass, "ok"),
            Axis::new("lesson_op", Mark::Pass, "op-2: patched slice-with-brim"),
            Axis::new("lesson_op", Mark::Note, "op 2 refused: the evidence is 5 characters; a quote of at least 20 proves something"),
        ],
    );
    gate(
        "b-lr",
        now - 25,
        "failed",
        &[
            Axis::new("report", Mark::Pass, "ok"),
            Axis::new(
                "lessons_folder",
                Mark::Fail,
                "x.md changed in .nirvana/lessons since the review was queued",
            ),
        ],
    );
    gate(
        "c-lr",
        now - 20,
        "failed",
        &[Axis::new(
            "report",
            Mark::Fail,
            "no Work Report was delivered",
        )],
    );
    gate(
        "f-lr",
        now - 15,
        "pass",
        &[
            Axis::new("report", Mark::Pass, "ok"),
            Axis::new("lesson_op", Mark::Pass, "nothing to keep"),
        ],
    );
    gate(
        "e-lr",
        now - 10,
        "pass",
        &[
            Axis::new("report", Mark::Pass, "ok"),
            Axis::new("lesson_op", Mark::Note, "op 1 refused: the evidence is 7 characters; a quote of at least 20 proves something"),
            Axis::new("card_op", Mark::Note, "card op 1 refused: the evidence is not in o1's stream"),
        ],
    );
}

fn with_cap(home: &Path, cap: u32) {
    let dir = home.join(seat::DIR);
    std::fs::create_dir_all(&dir).expect("seat dir");
    std::fs::write(
        dir.join(seat::SEATS_FILE),
        seat::DEFAULT.replace(
            "lesson_reviews_per_day = 20",
            &format!("lesson_reviews_per_day = {cap}"),
        ),
    )
    .expect("seats");
}

#[test]
fn p36_the_view_reads_lessons_ledgers_profiles_today_and_waiting_from_a_fixture_project() {
    let home = crate::nirvana::with_test_home("p36-view");
    with_cap(&home.path, 7);
    let root = temp("view");
    let now = crate::nirvana_run::now();
    fixture(&root, now);
    let view = load(&root, now);

    // Lessons: live first, each with its origin, class and profile scope; `live` as the offer reads it.
    assert_eq!(view.lessons.len(), 12);
    assert_eq!(
        view.live, 11,
        "level-the-bed is past six offers with no help: decayed"
    );
    assert!(
        view.lessons
            .last()
            .is_some_and(|l| l.slug == "level-the-bed" && !l.live),
        "{:?}",
        view.lessons.last()
    );
    let beat = view
        .lessons
        .iter()
        .find(|l| l.slug == "cut-on-the-beat")
        .expect("beat");
    assert_eq!(
        (beat.origin, beat.class.as_str(), beat.profile.as_deref()),
        (Origin::Merge, "reel", Some(PROFILE))
    );

    // Recent changes: both ledgers, newest first, every op kind, the undone op marked.
    let ids: Vec<String> = view
        .changes
        .iter()
        .map(|c| format!("{}:{}:{}", where_word(&c.source), c.id, c.op))
        .collect();
    assert_eq!(
        ids,
        [
            "lessons:op-5:rollback",
            "lessons:op-4:new",
            "cards:op-3:new",
            "cards:op-2:patch",
            "lessons:op-3:merge",
            "cards:op-1:new",
            "lessons:op-2:patch",
            "lessons:op-1:new",
        ],
    );
    let op4 = &view.changes[1];
    assert!(op4.undone && op4.refusal().is_some(), "{op4:?}");
    assert!(
        view.changes[0].refusal().is_some(),
        "a rollback is not undone"
    );
    let merge = &view.changes[4];
    assert!(merge.refusal().is_none());
    assert_eq!(merge.slugs, ["cut-on-the-beat", "beat-a", "beat-b"]);

    // Profiles: the folder one with its cards, the unknown one, the warning and the refused; the one-file one.
    assert_eq!(
        view.profiles,
        [
            ProfileRow {
                name: "plain".into(),
                ..ProfileRow::default()
            },
            ProfileRow {
                name: PROFILE.into(),
                knowledge: Some(
                    profile_files::folder_of(&root, PROFILE).join(profile_files::KNOWLEDGE)
                ),
                cards: 5,
                unknown: 1,
                warnings: 1,
                refused: 1,
            },
        ]
    );

    // Today: six reviews seated today against the table's cap, the yesterday one and the build order left out.
    let today = &view.today;
    assert_eq!((today.reviews.len(), today.cap), (6, 7));
    assert_eq!(
        (
            today.gated(),
            today.count(Ruled::Done),
            today.count(Ruled::HeldBack),
            today.count(Ruled::Failed),
            today.count(Ruled::Waiting)
        ),
        (5, 3, 1, 1, 1)
    );
    assert_eq!((today.applied(), today.refused()), (1, 3));
    assert_eq!(
        today.reasons,
        [
            (
                "the evidence is N characters; a quote of at least N proves something".to_owned(),
                2
            ),
            ("the evidence is not in oN's stream".to_owned(), 1),
        ]
    );
    // The last gated review today (e-lr) had every op refused.
    assert!(today.last_went_wrong());

    // Waiting: the print class is over the merge line.
    assert_eq!(view.waiting, [("print".to_owned(), None, 10)]);
    let _ = std::fs::remove_dir_all(&root);
}

fn where_word(source: &Source) -> &'static str {
    match source {
        Source::Lessons => "lessons",
        Source::Profile(_) => "cards",
    }
}

#[test]
fn p36_the_mark_follows_the_last_review_gated_today() {
    let review = |order: &str, ruled: Ruled, at: u64, applied: usize, refused: usize| Review {
        order: order.into(),
        ruled,
        gated_at: Some(at),
        applied,
        refused,
    };
    let mut today = Today::default();
    assert!(!today.last_went_wrong(), "no review");
    today.reviews = vec![
        review("a", Ruled::Failed, 1, 0, 0),
        review("b", Ruled::Done, 2, 1, 1),
    ];
    assert!(!today.last_went_wrong(), "the last one kept an op");
    today.reviews.push(review("c", Ruled::HeldBack, 3, 0, 0));
    assert!(today.last_went_wrong(), "held back");
    today.reviews.push(review("d", Ruled::Failed, 4, 0, 0));
    assert!(today.last_went_wrong(), "failed");
    today.reviews.push(review("e", Ruled::Done, 5, 0, 2));
    assert!(today.last_went_wrong(), "every op refused");
    today.reviews.push(review("f", Ruled::Done, 6, 0, 0));
    assert!(!today.last_went_wrong(), "nothing to keep is fine");
    today.reviews.push(Review {
        order: "g".into(),
        ruled: Ruled::Waiting,
        gated_at: None,
        applied: 0,
        refused: 0,
    });
    assert!(
        !today.last_went_wrong(),
        "a review not gated yet is not the last gated"
    );
}

#[test]
fn p36_the_counters_agree_with_herdr_lessons_stats() {
    let _home = crate::nirvana::with_test_home("p36-stats");
    let root = temp("stats");
    let now = crate::nirvana_run::now();
    fixture(&root, now);
    let view = load(&root, now);
    let index = nirvana_lessons::read_index(&root);
    assert_eq!(
        view.stats,
        nirvana_lessons::stats_lines(&index),
        "the panel prints `herdr lessons stats`"
    );
    // The rows the panel lists sum, per origin, to the same counters `stats` prints.
    for (origin, count, sum) in nirvana_lessons::stats(&index) {
        let rows: Vec<&LessonRow> = view.lessons.iter().filter(|l| l.origin == origin).collect();
        let mut mine = Counters::default();
        for row in &rows {
            mine.offered += row.counters.offered;
            mine.opened += row.counters.opened;
            mine.passed_after_opened += row.counters.passed_after_opened;
        }
        assert_eq!((rows.len(), mine), (count, sum), "{origin:?}");
    }
    for row in &view.lessons {
        assert_eq!(
            row.live,
            nirvana_lessons::live(&index.lessons[&row.slug].counters),
            "{}",
            row.slug
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn p36_undo_rolls_back_exactly_that_row_as_the_cli_does() {
    let _home = crate::nirvana::with_test_home("p36-undo");
    let root = temp("undo");
    let now = crate::nirvana_run::now();
    fixture(&root, now);
    let view = load(&root, now);
    let patch = view
        .changes
        .iter()
        .find(|c| c.source == Source::Lessons && c.id == "op-2")
        .expect("op-2")
        .clone();
    let said = undo(&root, &patch, now).expect("undone");
    assert!(said.contains("rolled back op-2"), "{said}");
    let rows = nirvana_lessons::ledger(&root);
    let last = rows.last().expect("row");
    assert_eq!(
        (last.op.as_str(), last.of.as_deref()),
        ("rollback", Some("op-2"))
    );
    assert_eq!(
        rows.iter().filter(|r| r.op == "rollback").count(),
        2,
        "one new rollback"
    );
    assert_eq!(
        nirvana_lessons::body(&root, PATCHED).as_deref(),
        Some("v1\n")
    );
    // The card ledger's op-2 (the same id, another ledger) is untouched, and undone through `herdr kb`'s rollback.
    let dir = profile_files::knowledge_dir(&root, PROFILE).expect("dir");
    assert_eq!(knowledge::ledger(&dir).len(), 3);
    let card_op = view
        .changes
        .iter()
        .find(|c| c.source == Source::Profile(PROFILE.into()) && c.id == "op-2")
        .expect("card op-2")
        .clone();
    undo(&root, &card_op, now).expect("card undone");
    let rows = knowledge::ledger(&dir);
    assert_eq!((rows.len(), rows[3].of.as_deref()), (4, Some("op-2")));
    assert_eq!(
        std::fs::read_to_string(dir.join("closer.md")).expect("closer"),
        card("Closer v1", "keep")
    );
    // A second undo of the same row is refused, as the CLI refuses it.
    assert!(undo(&root, &patch, now).is_err_and(|why| why.contains("rolled back already")));
    let _ = std::fs::remove_dir_all(&root);
}

/// Every file under `dir` with its bytes.
pub(crate) fn tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in listed(&next) {
            if entry.is_dir() {
                stack.push(entry);
            } else if let Ok(bytes) = std::fs::read(&entry) {
                out.insert(entry, bytes);
            }
        }
    }
    out
}

#[test]
fn p36_reading_the_view_writes_nothing() {
    let home = crate::nirvana::with_test_home("p36-nowrite");
    let root = temp("nowrite");
    let now = crate::nirvana_run::now();
    fixture(&root, now);
    let (before, home_before) = (tree(&root), tree(&home.path));
    let view = load(&root, now);
    let _ = stamp(&root, now);
    assert_eq!(
        view.today.cap,
        review::PER_DAY,
        "no seats.toml: herdr's default"
    );
    assert_eq!(tree(&root), before, "the project");
    assert_eq!(
        tree(&home.path),
        home_before,
        "herdr's home: no seats.toml written"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn p36_the_stamp_changes_with_the_files_the_view_is_made_from() {
    let _home = crate::nirvana::with_test_home("p36-stamp");
    let root = temp("stamp");
    let now = crate::nirvana_run::now();
    fixture(&root, now);
    let first = stamp(&root, now);
    assert_eq!(stamp(&root, now), first, "nothing changed");
    assert_ne!(stamp(&root, now + 86_400), first, "a new day");
    let grow = |path: PathBuf| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open");
        file.write_all(b"\n").expect("grow");
    };
    grow(nirvana_lessons::dir(&root).join(nirvana_lessons::INDEX));
    let second = stamp(&root, now);
    assert_ne!(second, first, "the index");
    grow(
        profile_files::knowledge_dir(&root, PROFILE)
            .expect("dir")
            .join("opener.md"),
    );
    let third = stamp(&root, now);
    assert_ne!(third, second, "a card");
    grow(Project::at(&root).journal("r1").path);
    assert_ne!(stamp(&root, now), third, "a journal");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn p36_the_cache_reads_on_a_thread_of_its_own() {
    let _home = crate::nirvana::with_test_home("p36-cache");
    set_live_for_test(true);
    let root = temp("cache");
    let now = crate::nirvana_run::now();
    fixture(&root, now);
    assert_eq!(
        cached(&root),
        None,
        "the first call starts a read and returns"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let view = loop {
        if let Some(view) = cached(&root) {
            break view;
        }
        assert!(Instant::now() < deadline, "no view in 10 s");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(view.live, 11);
    assert_eq!(
        reads_on_this_thread(),
        0,
        "the caller's thread read nothing"
    );
    // A lesson added later shows after the next check.
    let mut index = nirvana_lessons::read_index(&root);
    index.lessons.insert(
        "new-one".into(),
        entry("New one", "print", Origin::Worker, None, [0, 0, 0]),
    );
    std::fs::write(
        nirvana_lessons::dir(&root).join(nirvana_lessons::INDEX),
        serde_json::to_string_pretty(&index).expect("index"),
    )
    .expect("index");
    let deadline = Instant::now() + REFRESH + Duration::from_secs(10);
    loop {
        if cached(&root).is_some_and(|v| v.live == 12) {
            break;
        }
        assert!(Instant::now() < deadline, "the change was not read");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(take_changed());
    assert_eq!(reads_on_this_thread(), 0);
    set_live_for_test(false);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn p36_refusal_reasons_count_as_one_whatever_their_numbers() {
    assert_eq!(
        reason_key("the evidence is 5 characters"),
        "the evidence is N characters"
    );
    assert_eq!(reason_key("op 12 of 340"), "op N of N");
    assert_eq!(reason_key("no digits"), "no digits");
}

#[test]
fn p36_recent_changes_stop_at_the_newest_twelve() {
    let _home = crate::nirvana::with_test_home("p36-cap");
    let root = temp("cap");
    let dir = nirvana_lessons::dir(&root);
    std::fs::create_dir_all(&dir).expect("lessons");
    let rows: Vec<LedgerRow> = (1..=15)
        .map(|n| {
            lesson_row(
                &format!("op-{n}"),
                1_000 + n,
                "new",
                vec![snap("a", None)],
                vec![snap("a", Some("a"))],
                None,
            )
        })
        .collect();
    std::fs::write(dir.join(nirvana_lessons::LEDGER), jsonl(&rows)).expect("ledger");
    let view = load(&root, 2_000);
    assert_eq!(view.changes.len(), CHANGES_SHOWN);
    assert_eq!(
        (view.changes[0].id.as_str(), view.changes[11].id.as_str()),
        ("op-15", "op-4")
    );
    let _ = std::fs::remove_dir_all(&root);
}
