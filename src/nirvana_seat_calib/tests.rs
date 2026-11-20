//! Plan 46 P46a: the calibration's tests, named p97_ (controls in nirvana/controls.d/p97_seat_calibration.py). All on
//! invented orders: the fixture record was written from targets (no line of the owner's record), and every file is in a
//! temp folder or a test home. The fixture's journal paths (`c:/work/...`) name no folder on this machine.

use super::*;
use crate::nirvana_run::{self, Event, Kind, Seated};
use crate::nirvana_seat::{parse, Mode, Request, World};
use serde_json::{json, Value};

/// 2026-11-18 11:06:40Z: every fixture line is an hour apart before it.
const NOW: u64 = 1_795_000_000;
/// Invented orders whose tokens put each pair near a chosen ratio against v18 (37 lines: 33 medium orders of five pairs
/// in two projects, then a small order, a subagent, a seat the table lacks and a line with no tokens).
const FIXTURE: &str = include_str!("record_fixture.jsonl");
/// The table the fixture is measured against: v18 is a kept default, so it never changes under this test.
const V18: &str = include_str!("../nirvana_seats_v18.toml");

fn v18() -> Table {
    parse(V18).expect("v18 parses")
}

fn fixture() -> Vec<RecordLine> {
    FIXTURE
        .lines()
        .map(|line| serde_json::from_str(line).expect("a fixture line"))
        .collect()
}

fn pooled<'a>(cal: &'a Calibration, seat: &str, effort: Effort) -> &'a CostRow {
    cal.cost
        .iter()
        .find(|row| row.project.is_none() && row.seat == seat && row.effort == effort)
        .unwrap_or_else(|| panic!("no pooled row {seat} {effort:?}: {:?}", cal.cost))
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "herdr-p97-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// L3's verifier: "the Rust figures equal seat_fit.py's on a fixture record". The numbers are an actual run of the
/// script on this fixture, 2026-11-19:
///
///     python nirvana/seat_fit.py --record src/nirvana_seat_calib/record_fixture.jsonl --table src/nirvana_seats_v18.toml
///
/// which printed `cell argon high: median ratio 11.14, n 5, REFIT`, `carbon medium 2.65, n 4, keep`, `hydrogen low
/// 0.38, n 7, REFIT`, `opus high 2.10, n 11, REFIT` and `sonnet medium 0.88, n 6, keep`; and at full precision from the
/// script's own `fit` (in nirvana/: `seat_fit.fit(table, seat_fit.lines_of(record))`, median / size.medium.use):
/// 11.144207, 2.651352, 0.376863, 2.100817, 0.879076.
#[test]
fn p97_the_cost_fit_is_seat_fit_pys_on_the_fixture() {
    let cal = calibrate(&v18(), &fixture(), &Journals::default(), NOW);
    let printed = [
        ("argon", Effort::High, 5, 11.144207, "11.14", true),
        ("carbon", Effort::Medium, 4, 2.651352, "2.65", false),
        ("hydrogen", Effort::Low, 7, 0.376863, "0.38", true),
        ("opus", Effort::High, 11, 2.100817, "2.10", true),
        ("sonnet", Effort::Medium, 6, 0.879076, "0.88", false),
    ];
    let rows: Vec<&CostRow> = cal
        .cost
        .iter()
        .filter(|row| row.project.is_none())
        .collect();
    assert_eq!(
        rows.len(),
        printed.len(),
        "the script printed five cells: {rows:?}"
    );
    for (seat, effort, n, median, two, refit) in printed {
        let row = pooled(&cal, seat, effort);
        assert_eq!(row.n, n, "{seat} {effort:?}");
        assert!(
            (row.median - median).abs() < 1e-5,
            "{seat} {effort:?}: {} against {median}",
            row.median
        );
        assert_eq!(format!("{:.2}", row.median), two, "{seat} {effort:?}");
        assert_eq!(
            row.factor.is_some(),
            refit,
            "{seat} {effort:?}: REFIT is a factor"
        );
    }
}

/// §4's rule: a factor at 5 or more orders outside 0.5 to 2.0, the median itself, clamped to 0.25 to 4.0 and flagged.
#[test]
fn p97_a_factor_needs_five_orders_outside_the_band_and_is_clamped() {
    assert_eq!(factor(4, 0.3), (None, false), "four orders are too few");
    assert_eq!(factor(5, 0.3), (Some(0.3), false));
    assert_eq!(factor(5, 0.5), (None, false), "the band's edge is inside");
    assert_eq!(factor(5, 2.0), (None, false), "the band's edge is inside");
    assert_eq!(
        factor(5, 2.02),
        (Some(2.02), false),
        "Carbon medium's 2.02 at n 5 moves"
    );
    assert_eq!(
        factor(7, 11.13),
        (Some(4.0), true),
        "Argon high's 11.13 is a gap larger than one step"
    );
    assert_eq!(factor(5, 0.1), (Some(0.25), true));
    let cal = calibrate(&v18(), &fixture(), &Journals::default(), NOW);
    let argon = pooled(&cal, "argon", Effort::High);
    assert_eq!((argon.factor, argon.clamped), (Some(4.0), true));
    let hydrogen = pooled(&cal, "hydrogen", Effort::Low);
    assert_eq!(
        (hydrogen.factor, hydrogen.clamped),
        (Some(hydrogen.median), false)
    );
    assert_eq!(
        pooled(&cal, "carbon", Effort::Medium).factor,
        None,
        "n 4 does not move"
    );
    assert_eq!(
        pooled(&cal, "sonnet", Effort::Medium).factor,
        None,
        "inside the band"
    );
}

/// A project's own figure when it has 5 or more orders for the pair (its verdict, a factor or none), else the pooled.
#[test]
fn p97_a_projects_own_figure_wins_at_five_orders() {
    let cal = calibrate(&v18(), &fixture(), &Journals::default(), NOW);
    let own = |project: &str, seat: &str, effort: Effort| {
        cal.cost
            .iter()
            .find(|row| {
                row.project.as_deref() == Some(project) && row.seat == seat && row.effort == effort
            })
            .cloned()
            .unwrap_or_else(|| panic!("no {project} row for {seat}"))
    };
    let (alpha, beta) = (
        root_key(Path::new("c:/work/alpha")),
        root_key(Path::new("c:/work/beta")),
    );
    let opus_alpha = own(&alpha, "opus", Effort::High);
    assert_eq!(opus_alpha.n, 6);
    assert!(opus_alpha.factor.is_some_and(|f| f > 2.0), "{opus_alpha:?}");
    let opus_beta = own(&beta, "opus", Effort::High);
    assert_eq!((opus_beta.n, opus_beta.factor), (5, None), "{opus_beta:?}");
    assert_eq!(own(&alpha, "hydrogen", Effort::Low).n, 4);
    let pooled_opus = pooled(&cal, "opus", Effort::High)
        .factor
        .expect("pooled opus high");
    let pooled_hydrogen = pooled(&cal, "hydrogen", Effort::Low)
        .factor
        .expect("pooled hydrogen low");
    let at = |project: Option<&str>, seat: &str, effort: Effort| {
        Factors::from_calibration(&cal, project.map(str::to_owned))
            .for_cell(seat, effort)
            .map(|applied| (applied.factor, applied.project.clone()))
    };
    // Beta's own five orders sit inside the band: its opus high books as the table says, the pooled 2.10 aside.
    assert_eq!(at(Some(&beta), "opus", Effort::High), None);
    assert_eq!(
        at(Some(&alpha), "opus", Effort::High),
        Some((opus_alpha.factor.unwrap(), Some(alpha.clone())))
    );
    // Four and three orders of hydrogen low each: the pooled figure for both, and for a project with none.
    assert_eq!(
        at(Some(&alpha), "hydrogen", Effort::Low),
        Some((pooled_hydrogen, None))
    );
    assert_eq!(
        at(Some("c:/work/gamma"), "opus", Effort::High),
        Some((pooled_opus, None))
    );
    assert_eq!(at(None, "opus", Effort::High), Some((pooled_opus, None)));
    // The journal's path names the project, a worktree's included, either slash.
    assert_eq!(
        project_of("C:\\Work\\Alpha\\.nirvana\\runs\\r1\\journal.jsonl#12"),
        Some(root_key(Path::new("C:\\Work\\Alpha")))
    );
    assert_eq!(
        root_key(Path::new("C:/Work/Alpha/.nirvana/worktrees/o1")),
        root_key(Path::new("C:/Work/Alpha"))
    );
    assert_eq!(project_of("no journal"), None);
}

/// Only record lines of the last 60 days: models and prices move.
#[test]
fn p97_only_the_last_sixty_days_count() {
    let shift = |days: u64| -> Vec<RecordLine> {
        fixture()
            .into_iter()
            .map(|mut line| {
                line.at = line.at.map(|at| at - days * 86_400);
                line
            })
            .collect()
    };
    let recent = calibrate(&v18(), &shift(58), &Journals::default(), NOW);
    assert_eq!(
        recent.cost,
        calibrate(&v18(), &fixture(), &Journals::default(), NOW).cost
    );
    let old = calibrate(&v18(), &shift(61), &Journals::default(), NOW);
    assert!(old.cost.is_empty(), "{:?}", old.cost);
    assert_eq!(old.lines_used, 0);
    let mut timeless = fixture();
    for line in &mut timeless {
        (line.at, line.ended_at, line.seated_at) = (None, None, None);
    }
    assert!(
        calibrate(&v18(), &timeless, &Journals::default(), NOW)
            .cost
            .is_empty(),
        "no time, no age"
    );
}

fn gated_line(
    order: &str,
    class: &str,
    seat: &str,
    first: &str,
    not_worker: Option<bool>,
    ago: u64,
) -> RecordLine {
    serde_json::from_value(json!({
        "order": order, "run": "rs", "seat": seat, "effort": "high", "class": class, "outcome": "pass",
        "first_verdict": first, "first_no_worker_fault": not_worker, "size": "medium", "at": NOW - ago,
        "seated_at": NOW - ago - 600, "ended_at": NOW - ago,
    }))
    .expect("a line")
}

/// Plan 46 D1: the score rule is a finding, both ways, never a change. §3's case: Opus in build.load_bearing (score 95,
/// Efficient bar 70) at 8 first passes of 21 would go to 68; without the 11 refusals no worker could fix, 8 of 10 keeps
/// 95.
#[test]
fn p97_scores_are_findings_both_ways_and_never_applied() {
    let table = v18();
    let mut lines = Vec::new();
    for n in 0..8 {
        lines.push(gated_line(
            &format!("p{n}"),
            "build.load_bearing",
            "opus",
            "pass",
            None,
            3_600 * n,
        ));
    }
    for n in 0..2 {
        lines.push(gated_line(
            &format!("w{n}"),
            "build.load_bearing",
            "opus",
            "sent_back",
            None,
            3_600 * n,
        ));
    }
    for n in 0..11 {
        let (word, flag) = if n % 2 == 0 {
            ("sent_back", Some(true))
        } else {
            ("amend", None)
        };
        lines.push(gated_line(
            &format!("b{n}"),
            "build.load_bearing",
            "opus",
            word,
            flag,
            3_600 * n,
        ));
    }
    let cal = calibrate(&table, &lines, &Journals::default(), NOW);
    let finding = cal
        .findings
        .iter()
        .find(|f| (f.class.as_str(), f.seat.as_str()) == ("build.load_bearing", "opus"))
        .expect("a finding");
    assert_eq!(
        (finding.gated, finding.first_pass, finding.not_worker),
        (21, 8, 11)
    );
    assert_eq!(finding.rule, Rule::Lower { to: 68.0 }, "{}", finding.text);
    assert_eq!(finding.rule_worker, Rule::Keep, "{}", finding.text);
    assert!((finding.share.unwrap() - 8.0 / 21.0).abs() < 1e-9);
    assert!((finding.share_worker.unwrap() - 0.8).abs() < 1e-9);
    assert!(
        finding.text.contains("would go 95 to 68"),
        "{}",
        finding.text
    );
    assert!(
        finding.text.contains("8 of 10 (0.80), keeps 95"),
        "{}",
        finding.text
    );
    // Nothing a pick reads came from it: no booking moved and the score stays 95.
    assert!(cal.cost.is_empty());
    let world = World {
        now: NOW,
        calibration: Factors::from_calibration(&cal, None),
        ..World::default()
    };
    let mut request = Request::new("build.load_bearing");
    request.mode = Mode::Sota;
    request.override_seat = Some(("opus".into(), "the score's own".into()));
    let got = crate::nirvana_seat::pick(&table, &world, &request).expect("opus");
    assert_eq!(got.score, 95.0);
    // Under 10 gated is too few.
    let few = calibrate(&table, &lines[..9], &Journals::default(), NOW);
    assert_eq!(few.findings[0].rule, Rule::TooFew);
    // The raise: 10 of 10 first passes, merged 8 of the 10 in a run that merges, under the Balanced bar (85) at 80.
    let mut journals = Journals::default();
    journals.merging.insert("rs".into());
    let mut raise = Vec::new();
    for n in 0..10 {
        raise.push(gated_line(
            &format!("h{n}"),
            "build.bounded",
            "hydrogen",
            "pass",
            None,
            60 * n,
        ));
        if n < 8 {
            journals.merged.insert(("rs".into(), format!("h{n}")));
        }
    }
    let cal = calibrate(&table, &raise, &journals, NOW);
    assert_eq!(
        cal.findings[0].rule,
        Rule::Raise { to: 86.0 },
        "{}",
        cal.findings[0].text
    );
    assert_eq!(cal.findings[0].merged, Some(0.8));
    // Merged not known (no run that merges) keeps it.
    let cal = calibrate(&table, &raise, &Journals::default(), NOW);
    assert_eq!(cal.findings[0].rule, Rule::Keep);
}

/// The page's reference rows: orders, gated, first passes, merged where the run merges, median minutes, cost against
/// booking, the last 14 days; under 5 orders `thin`.
#[test]
fn p97_reference_rows_count_each_cell() {
    let table = v18();
    let mut lines = Vec::new();
    for n in 0..6u64 {
        let first = if n < 4 { "pass" } else { "sent_back" };
        // Two of the six are 20 days old.
        let ago = if n < 2 { 20 * 86_400 } else { 60 };
        lines.push(gated_line(
            &format!("o{n}"),
            "build.bounded",
            "hydrogen",
            first,
            None,
            ago,
        ));
    }
    lines.push(gated_line(
        "lone",
        "build.bounded",
        "sonnet",
        "pass",
        None,
        60,
    ));
    let mut journals = Journals::default();
    journals.merging.insert("rs".into());
    for order in ["o0", "o1", "o2"] {
        journals.merged.insert(("rs".into(), order.into()));
    }
    let cal = calibrate(&table, &lines, &journals, NOW);
    let row = cal
        .cells
        .iter()
        .find(|row| row.seat == "hydrogen")
        .expect("hydrogen's row");
    assert_eq!(
        (row.class.as_str(), row.effort),
        ("build.bounded", Some(Effort::High))
    );
    assert_eq!((row.orders, row.gated, row.first_pass), (6, 6, 4));
    assert_eq!((row.merged, row.merge_orders), (3, 6));
    assert_eq!(row.recent, 4);
    assert_eq!(row.median_minutes, Some(10.0));
    assert_eq!(row.status(), "measured");
    let lone = cal
        .cells
        .iter()
        .find(|row| row.seat == "sonnet")
        .expect("sonnet's row");
    assert_eq!((lone.orders, lone.status()), (1, "thin"));
    // The fixture's orders carry tokens: each cell's cost against its booking, at the order's own size.
    let cal = calibrate(&v18(), &fixture(), &Journals::default(), NOW);
    let hydrogen = cal.cells.iter().find(|row| row.seat == "hydrogen").unwrap();
    assert_eq!(hydrogen.costed, 7);
    assert!(
        (hydrogen.median_cost.unwrap() - pooled(&cal, "hydrogen", Effort::Low).median).abs() < 1e-9
    );
    // A subagent's line is no order of any cell; every other line is one order of one cell.
    assert_eq!(cal.cells.iter().map(|row| row.orders).sum::<usize>(), 36);
}

/// The ledger: a row when a factor appears, moves by more than 10%, or goes.
#[test]
fn p97_the_ledger_writes_a_row_when_a_factor_appears_moves_or_goes() {
    let cal = calibrate(&v18(), &fixture(), &Journals::default(), NOW);
    let first = ledger(None, &cal, NOW);
    let factors = cal.cost.iter().filter(|row| row.factor.is_some()).count();
    assert_eq!(first.len(), factors, "every factor appears: {first:?}");
    assert!(first
        .iter()
        .all(|row| row.before.is_none() && row.after.is_some()));
    assert!(
        ledger(Some(&cal), &cal, NOW).is_empty(),
        "no change, no row"
    );
    let with = |f: f64| {
        let mut next = cal.clone();
        let row = next
            .cost
            .iter_mut()
            .find(|row| row.project.is_none() && row.seat == "hydrogen")
            .unwrap();
        row.factor = Some(f);
        next
    };
    let was = pooled(&cal, "hydrogen", Effort::Low).factor.unwrap();
    assert!(
        ledger(Some(&cal), &with(was * 1.09), NOW).is_empty(),
        "9% is no move"
    );
    let moved = ledger(Some(&cal), &with(was * 1.2), NOW);
    assert_eq!(moved.len(), 1);
    assert_eq!((moved[0].before, moved[0].n), (Some(was), 7));
    let mut gone = cal.clone();
    gone.cost.retain(|row| row.seat != "hydrogen");
    let rows = ledger(Some(&cal), &gone, NOW);
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].before, rows[0].after), (Some(was), None));
}

/// The file: written whole with its version, date and record size, a ledger beside it; a dry run and a failed read
/// write nothing; the idle tick's `due` after 20 more lines or a day.
#[test]
fn p97_a_check_writes_the_file_and_a_failed_read_writes_nothing() {
    let dir = temp("check");
    let table = v18();
    assert!(
        check(&dir, &table, NOW, true).is_err(),
        "no record, nothing to check"
    );
    assert!(!path(&dir).exists());
    assert!(!due(&dir, &table, NOW), "no record is never due");
    assert_eq!(tick(&dir, &table, NOW), None);
    std::fs::write(dir.join(nirvana_seat::RECORD_FILE), FIXTURE).unwrap();
    assert!(due(&dir, &table, NOW), "no calibration yet");
    let dry = check(&dir, &table, NOW, false).expect("a dry run");
    assert!(!dry.written && !path(&dir).exists() && !ledger_path(&dir).exists());
    assert!(!dry.ledger.is_empty());
    let said = text(&dry, Path::new("record.jsonl"));
    assert!(said.contains("; dry run, nothing written\n"), "{said}");
    assert!(
        said.contains("\n  hydrogen low: median 0.38, n 7, booked x0.38\n"),
        "{said}"
    );
    assert!(
        said.contains("\n  argon high: median 11.14, n 5, booked x4.00, clamped"),
        "{said}"
    );
    assert!(
        said.contains("\nwould change: hydrogen low: the table's to x0.38 (n 7)\n"),
        "{said}"
    );
    let summary = tick(&dir, &table, NOW).expect("due, so checked");
    let cal = read(&dir).expect("the file");
    assert_eq!(
        (cal.version, cal.computed_at, cal.record_lines),
        (VERSION, NOW, 37)
    );
    assert_eq!(cal.record_bytes, FIXTURE.len() as u64);
    assert_eq!(cal.date, "2026-11-18");
    assert_eq!(cal.summary, summary);
    assert!(
        summary.starts_with(
            "3 costs booked as measured, 2 more by a project's own orders, 2 gaps more than one step off"
        ),
        "{summary}"
    );
    let ledger_rows = std::fs::read_to_string(ledger_path(&dir)).unwrap();
    // Pooled argon, hydrogen and opus; alpha's opus and beta's argon.
    assert_eq!(ledger_rows.lines().count(), 5, "{ledger_rows}");
    // Not due again until the record grows by 20 lines or a day passes.
    assert!(!due(&dir, &table, NOW + 60));
    assert_eq!(tick(&dir, &table, NOW + 60), None);
    let one = FIXTURE.lines().next().unwrap();
    let grown = |n: usize| format!("{FIXTURE}{}", format!("{one}\n").repeat(n));
    std::fs::write(dir.join(nirvana_seat::RECORD_FILE), grown(19)).unwrap();
    assert!(!due(&dir, &table, NOW + 60), "19 lines more");
    std::fs::write(dir.join(nirvana_seat::RECORD_FILE), grown(20)).unwrap();
    assert!(due(&dir, &table, NOW + 60), "20 lines more");
    std::fs::write(dir.join(nirvana_seat::RECORD_FILE), FIXTURE).unwrap();
    assert!(due(&dir, &table, NOW + DUE_S), "a day on");
    // A second check finds nothing changed: no new ledger rows.
    tick(&dir, &table, NOW + DUE_S).expect("checked again");
    assert_eq!(
        std::fs::read_to_string(ledger_path(&dir)).unwrap(),
        ledger_rows
    );
    // A record that cannot be read writes nothing: the file stays the last check's.
    let before = std::fs::read(path(&dir)).unwrap();
    std::fs::remove_file(dir.join(nirvana_seat::RECORD_FILE)).unwrap();
    std::fs::create_dir(dir.join(nirvana_seat::RECORD_FILE)).unwrap();
    assert!(check(&dir, &table, NOW + 3 * DUE_S, true).is_err());
    assert_eq!(tick(&dir, &table, NOW + 3 * DUE_S), None);
    assert_eq!(std::fs::read(path(&dir)).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

fn seated(order: &str) -> Kind {
    Kind::Seated(Box::new(
        serde_json::from_value::<Seated>(json!({
            "order": order, "seat": "opus", "cli": "claude", "model": "claude-opus-5-5", "effort": "high",
            "pool": "max", "class": "build.load_bearing", "size": "medium", "mode": "balanced", "reason": "r",
            "order_file": "o.md", "cwd": "c:/p",
        }))
        .expect("a seated row"),
    ))
}

fn gated(order: &str, verdict: &str, axes: Vec<Value>) -> Kind {
    Kind::Gated {
        order: order.into(),
        verdict: verdict.into(),
        reason: None,
        axes,
    }
}

/// The record gains the first gate verdict and its fault (§4 "The record"), fed by the journal's replay; old lines
/// parse; and the journals fill a line written before plan 46.
#[test]
fn p97_the_record_line_and_the_journals_carry_the_first_verdict() {
    let owned_brief =
        json!({"axis": "owned", "mark": "fail", "why": "outside owns", "fault": "brief"});
    let tests_failed = json!({"axis": "tests", "mark": "fail", "why": "1 failed"});
    let tests_worker =
        json!({"axis": "tests", "mark": "fail", "why": "1 failed", "fault": "worker"});
    let kinds = vec![
        seated("o1"),
        gated("o1", "sent_back", vec![owned_brief.clone()]),
        gated("o1", "pass", vec![]),
        Kind::Merged {
            order: "o1".into(),
            sha: Some("abc".into()),
            by_hand: None,
        },
        seated("o2"),
        gated("o2", "pass", vec![]),
        seated("o3"),
        gated("o3", "amend", vec![]),
        seated("o4"),
        gated("o4", "sent_back", vec![tests_failed]),
        seated("o5"),
        gated("o5", "sent_back", vec![owned_brief, tests_worker]),
    ];
    let events: Vec<Event> = kinds
        .into_iter()
        .enumerate()
        .map(|(seq, kind)| Event {
            v: nirvana_run::V,
            seq: seq as u64 + 1,
            at: NOW - 600 + seq as u64,
            kind,
        })
        .collect();
    let state = nirvana_run::replay(&events);
    let first = |order: &str| {
        let known = state.order(order).unwrap();
        (known.first_verdict.clone(), known.first_no_worker_fault)
    };
    assert_eq!(
        first("o1"),
        (Some("sent_back".into()), Some(true)),
        "the brief's fault, first"
    );
    assert_eq!(first("o2"), (Some("pass".into()), None));
    assert_eq!(
        first("o3"),
        (Some("amend".into()), Some(true)),
        "an amend is no worker's"
    );
    assert_eq!(
        first("o4"),
        (Some("sent_back".into()), None),
        "no fault said: the worker's"
    );
    assert_eq!(
        first("o5"),
        (Some("sent_back".into()), Some(false)),
        "one axis the worker's"
    );
    let line = nirvana_run::record_line("r1", state.order("o1").unwrap(), Some("j#1".into()));
    assert_eq!(line.outcome, "pass");
    assert_eq!(
        (line.first_verdict.as_deref(), line.first_no_worker_fault),
        (Some("sent_back"), Some(true))
    );
    let text = serde_json::to_string(&line).unwrap();
    assert!(
        text.contains(r#""first_verdict":"sent_back","first_no_worker_fault":true"#),
        "{text}"
    );
    let back: RecordLine = serde_json::from_str(&text).unwrap();
    assert_eq!(back, line);
    assert!(back.extra.is_empty(), "known fields, not extra keys");
    let old: RecordLine =
        serde_json::from_value(json!({"order": "x", "seat": "opus", "effort": "high",
        "class": "plan", "outcome": "pass"}))
        .expect("an old line parses");
    assert_eq!((old.first_verdict, old.first_no_worker_fault), (None, None));
    // A line written before plan 46 names its journal: the replay gives its first verdict and its merge.
    let root = temp("journals");
    let journal = nirvana_run::Project::at(&root).journal("r1");
    std::fs::create_dir_all(journal.path.parent().unwrap()).unwrap();
    let text: String = events
        .iter()
        .map(|event| format!("{}\n", serde_json::to_string(event).unwrap()))
        .collect();
    std::fs::write(&journal.path, text).unwrap();
    let mut bare = line.clone();
    (bare.first_verdict, bare.first_no_worker_fault) = (None, None);
    bare.journal = Some(format!("{}#12", journal.path.display()));
    bare.at = Some(NOW);
    let journals = read_journals(
        std::slice::from_ref(&bare),
        NOW,
        &Limits::default(),
        std::time::Instant::now(),
    );
    assert_eq!(
        journals.first.get(&("r1".into(), "o1".into())),
        Some(&("sent_back".to_owned(), Some(true)))
    );
    assert!(journals.merged.contains(&("r1".into(), "o1".into())));
    assert!(journals.merging.contains("r1"));
    assert_eq!(journals.files, 1, "one journal file read");
    assert_eq!(journals.first.len(), 5);
    let cal = calibrate(&v18(), std::slice::from_ref(&bare), &journals, NOW);
    let row = &cal.cells[0];
    assert_eq!(
        (row.gated, row.first_pass, row.merged, row.merge_orders),
        (1, 0, 1, 1)
    );
    assert_eq!(cal.findings[0].not_worker, 1);
    // The record's own field wins over the journal's.
    let mut own = bare;
    own.first_verdict = Some("pass".into());
    assert_eq!(
        calibrate(&v18(), &[own], &journals, NOW).cells[0].first_pass,
        1
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// §4 "When": the idle tick checks the table with no model, no seat and no spare usage, counts no review, keeps the
/// summary machine-wide for the Learning panel's line, and writes nothing when a read fails.
#[test]
fn p97_the_idle_tick_checks_the_table_with_no_seat_and_no_review() {
    use crate::nirvana_idle_review as idle;
    let home = crate::nirvana::with_test_home("p97-idle");
    let dir = nirvana_seat::dir();
    std::fs::create_dir_all(&dir).unwrap();
    let now = nirvana_run::now();
    // The fixture moved to now, so its lines are recent.
    let record: String = fixture()
        .into_iter()
        .map(|mut line| {
            line.at = line.at.map(|at| at + (now - NOW));
            format!("{}\n", serde_json::to_string(&line).unwrap())
        })
        .collect();
    std::fs::write(dir.join(nirvana_seat::RECORD_FILE), &record).unwrap();
    let table = parse(nirvana_seat::DEFAULT).unwrap();
    // No readings at all: no seat is spare, so no review can run.
    let world = World {
        now,
        ..World::default()
    };
    let lines = idle::tick_with(&[], &table, &world, None);
    assert!(lines.is_empty(), "{lines:?}");
    let cal = read(&dir).expect("checked in the tick");
    assert_eq!(cal.computed_at, now);
    let global = idle::read_global();
    assert_eq!(global.calibration, Some((now, cal.summary.clone())));
    assert!(global.windows.is_empty(), "no review counted");
    // The Learning panel's view of any project carries the machine's line.
    let view = idle::view(&dir.join("some-project"));
    assert_eq!(view.calibration, global.calibration);
    let shown = idle::lines(&view, now);
    assert!(
        shown.contains(&format!("seat table checked just now: {}", cal.summary)),
        "{shown:?}"
    );
    // Not due a minute on: the file and the line stay as they were.
    let world = World {
        now: now + 60,
        ..World::default()
    };
    idle::tick_with(&[], &table, &world, None);
    assert_eq!(read(&dir).unwrap().computed_at, now);
    assert_eq!(
        idle::read_global().calibration,
        Some((now, cal.summary.clone()))
    );
    // A record that cannot be read: nothing is written, and the tick goes on.
    std::fs::remove_file(path(&dir)).unwrap();
    std::fs::remove_file(dir.join(nirvana_seat::RECORD_FILE)).unwrap();
    std::fs::create_dir(dir.join(nirvana_seat::RECORD_FILE)).unwrap();
    let lines = idle::tick_with(&[], &table, &world, None);
    assert!(lines.is_empty(), "{lines:?}");
    assert!(!path(&dir).exists());
    assert_eq!(idle::read_global().calibration, Some((now, cal.summary)));
    drop(home);
}

/// The owner's hold (2026-11-18): a held pair's figure is recorded and marked, counted apart in the summary, and
/// said so by `seat calibrate`; `--picks` puts no move on Balanced plan as held.
#[test]
fn p97_held_pairs_are_measured_and_marked() {
    let table = parse(nirvana_seat::DEFAULT).unwrap();
    let cal = calibrate(&table, &fixture(), &Journals::default(), NOW);
    let opus = pooled(&cal, "opus", Effort::High);
    assert!(opus.held && opus.factor.is_some(), "{opus:?}");
    assert!(!pooled(&cal, "hydrogen", Effort::Low).held);
    // Against the default, hydrogen low's refit puts its fixture orders near 1: argon high is the one pooled booking.
    assert!(cal.summary.starts_with("1 cost booked as measured, 1 more by a project's own orders, 2 held by calibration_hold"),
        "{}", cal.summary);
    let checked = Checked {
        calibration: cal.clone(),
        ledger: Vec::new(),
        written: true,
        journals: 0,
        millis: 0,
        skipped: Skipped::default(),
    };
    let said = text(&checked, Path::new("record.jsonl"));
    assert!(
        said.contains(
            "\n  opus high: median 2.10, n 11, held at the table's booking (measured x2.10)\n"
        ),
        "{said}"
    );
    let held = moved_picks(&table, &cal, None, NOW);
    assert!(
        held.iter().all(|m| !m.starts_with("balanced plan:")),
        "{held:?}"
    );
    // A booking four times dearer on every effort of hydrogen moves picks off it, and `--picks` names them.
    let mut dear = Calibration::default();
    for effort in [Effort::Low, Effort::Medium, Effort::High] {
        dear.cost.push(CostRow {
            seat: "hydrogen".into(),
            effort,
            project: None,
            n: 9,
            median: 4.0,
            factor: Some(4.0),
            clamped: false,
            held: false,
        });
    }
    let moved = moved_picks(&table, &dear, None, NOW);
    assert!(
        !moved.is_empty() && moved.iter().all(|m| m.contains(": hydrogen ")),
        "{moved:?}"
    );
    let shown = picks_text(&table, &dear, NOW);
    assert!(
        shown.starts_with(&format!(
            "picks that move, as held, pooled: {}
",
            moved.len()
        )),
        "{shown}"
    );
}

/// The P46a review, finding 7: the cached world (the prompt hook, the launch, the media doctor) reads no factors, so
/// it never parses the file; the idle review's pick reads the pooled ones (`pooled_factors`), only when the table says
/// on and the file fits the table, so it books as `seat pick` does.
#[test]
fn p97_the_cached_world_reads_no_factors_and_the_idle_pick_reads_the_pooled() {
    let dir = temp("cached");
    let table = v18_on();
    let cal = calibrate(&table, &fixture(), &Journals::default(), NOW);
    std::fs::write(path(&dir), serde_json::to_string(&cal).unwrap()).unwrap();
    assert_eq!(
        nirvana_seat::cached_world(&dir, NOW).calibration,
        Factors::default(),
        "the cached world reads none"
    );
    let factors = nirvana_seat::pooled_factors(&table, &dir);
    let applied = factors
        .for_cell("hydrogen", Effort::Low)
        .expect("hydrogen low");
    assert_eq!(applied.n, 7);
    assert_eq!(factors.project, None, "no project: the pooled factors");
    // Off in the table: none, and the file is not read (a file that does not parse changes nothing).
    assert_eq!(
        nirvana_seat::pooled_factors(&v18(), &dir),
        Factors::default()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// v18 with `calibration = true`: v18's cells, so the fixture's factors are what the script printed.
fn v18_on() -> Table {
    let mut table = v18();
    table.calibration = true;
    table
}

/// The table v18 with one line's text replaced, parsed.
fn v18_edited(find: &str, replace: &str) -> Table {
    assert_eq!(V18.matches(find).count(), 1, "{find}");
    let mut table = parse(&V18.replacen(find, replace, 1)).expect("the edit parses");
    table.calibration = true;
    table
}

/// The P46a review, finding 1: a factor belongs to the cells it was measured against. A file computed on v18's
/// Hydrogen low cell is ignored once the table carries today's refit cell, and the same for a hand edit of one cell, a
/// price, a pool's per_usd, a class weight or the medium size's use: every reader books by the table and the check is
/// due at once. An edit nothing in the ratio reads (`overrides_per_run`, a comment, the hold, a score, the mode) keeps
/// the factors.
#[test]
fn p97_a_factor_belongs_to_the_cells_it_was_measured_on() {
    let dir = temp("fingerprint");
    std::fs::write(dir.join(nirvana_seat::RECORD_FILE), FIXTURE).unwrap();
    let measured = v18_on();
    let checked = check(&dir, &measured, NOW, true).expect("checked");
    assert_eq!(
        checked.calibration.table_fingerprint,
        fingerprint(&measured)
    );
    let cal = read(&dir).expect("the file");
    assert!(cal.fits(&measured));
    assert!(!due(&dir, &measured, NOW + 60), "just checked");
    let booked = |table: &Table| {
        nirvana_seat::pooled_factors(table, &dir)
            .for_cell("hydrogen", Effort::Low)
            .is_some()
    };
    assert!(booked(&measured), "measured on this table: booked");
    let low = "{ effort = \"low\", windows = { \"5h\" = 0.0002235, week = 0.0001118 }";
    let changed = [
        ("today's refit hydrogen low cell", {
            let mut today = parse(nirvana_seat::DEFAULT).unwrap();
            today.calibration_hold.clear();
            today
        }),
        (
            "a hand edit of one cell",
            v18_edited(
                low,
                "{ effort = \"low\", windows = { \"5h\" = 0.0002235, week = 0.0001119 }",
            ),
        ),
        (
            "the cell's date",
            v18_edited(
                "windows = { \"5h\" = 0.0002235, week = 0.0001118 }, at = \"2026-11-12\"",
                "windows = { \"5h\" = 0.0002235, week = 0.0001118 }, at = \"2026-11-13\"",
            ),
        ),
        ("the medium size's use", {
            let mut table = v18_on();
            table.size.medium.use_factor *= 1.5;
            table
        }),
        ("a class weight", {
            let mut table = v18_on();
            table.classes[0].weight *= 2.0;
            table
        }),
        ("a pool's per_usd", {
            let mut table = v18_on();
            for per in table.pools[0].per_usd.values_mut() {
                *per *= 2.0;
            }
            table
        }),
        ("a price", {
            let mut table = v18_on();
            let seat = table
                .seats
                .iter_mut()
                .find(|seat| !seat.prices.is_empty())
                .unwrap();
            seat.prices[0].input *= 2.0;
            table
        }),
    ];
    for (what, table) in &changed {
        assert!(!cal.fits(table), "{what}: fits");
        assert!(!booked(table), "{what}: the file's factor is still booked");
        assert_eq!(
            nirvana_seat::world_factors(table, &dir),
            Factors::default(),
            "{what}: live"
        );
        assert!(due(&dir, table, NOW + 60), "{what}: not due at once");
    }
    let mut unrelated = vec![
        (
            "overrides_per_run",
            v18_edited("\noverrides_per_run = 3\n", "\noverrides_per_run = 5\n"),
        ),
        (
            "a comment",
            v18_edited(
                "# Overrides the orchestrator",
                "# Overrides that the orchestrator",
            ),
        ),
        (
            "a basis",
            v18_edited(
                "carries flash's fit x8.97\" }",
                "carries flash's fit x8.97, noted\" }",
            ),
        ),
    ];
    let mut held = v18_on();
    held.calibration_hold = vec!["opus:high".into()];
    unrelated.push(("the hold line", held));
    let mut scored = v18_on();
    scored.classes[0].seats[0].score -= 1.0;
    unrelated.push(("a score", scored));
    let mut mode = v18_on();
    mode.mode = Mode::Sota;
    unrelated.push(("the mode", mode));
    for (what, table) in &unrelated {
        assert!(cal.fits(table), "{what}: does not fit");
        assert!(booked(table), "{what}: the factor was dropped");
        assert!(!due(&dir, table, NOW + 60), "{what}: due");
    }
    // The idle tick checks again against the new table at once, and books by it from then on.
    let (_, today) = &changed[0];
    let summary = tick(&dir, today, NOW + 120).expect("due, so checked");
    assert!(read(&dir).unwrap().fits(today), "{summary}");
    assert!(!due(&dir, today, NOW + 180));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The P46a review, finding 3: the file is checked where it is read. A factor that is not finite and positive drops
/// its row, a good one is kept; an unknown file version is ignored whole. The P46a second review, F6: the check clamps
/// every factor it writes, so one outside `CLAMP` (1e9, 0.1) is corruption and drops its row as well, where it was
/// booked at the clamp's edge; a clamped row the check wrote (4.0, flagged) is booked and stays flagged; `dropped_rows`
/// counts the dropped ones.
#[test]
fn p97_factors_out_of_bounds_are_dropped_when_read() {
    let dir = temp("clamp-read");
    let table = v18_on();
    let row = |seat: &str, effort: &str, factor: Value| json!({"seat": seat, "effort": effort, "n": 7, "median": 1.0, "factor": factor});
    let file = |version: u32| {
        json!({
            "version": version, "computed_at": NOW, "date": "2026-11-18", "record_lines": 37, "record_bytes": 1,
            "lines_used": 37, "on": true, "table_fingerprint": fingerprint(&table), "findings": [], "cells": [],
            "summary": "s",
            "cost": [
                row("hydrogen", "low", json!(0.0)),
                row("hydrogen", "medium", json!(-2.0)),
                row("hydrogen", "high", json!("NaN")),
                row("opus", "high", json!(1e9)),
                row("sonnet", "medium", json!(0.37)),
                {"seat": "sonnet", "effort": "high", "n": 7, "median": null, "factor": "0.41"},
                {"seat": "opus", "effort": "medium", "n": 7, "median": 9.0, "factor": 4.0, "clamped": true},
                row("opus", "low", json!(0.1)),
            ],
        })
    };
    std::fs::write(path(&dir), serde_json::to_string(&file(VERSION)).unwrap()).unwrap();
    let cal = read(&dir).expect("one bad row does not lose the file");
    let factors = nirvana_seat::pooled_factors(&table, &dir);
    for effort in [Effort::Low, Effort::Medium, Effort::High] {
        assert_eq!(
            factors.for_cell("hydrogen", effort),
            None,
            "{effort:?}: dropped"
        );
    }
    assert_eq!(factors.for_cell("opus", Effort::High), None, "1e9: dropped");
    assert_eq!(factors.for_cell("opus", Effort::Low), None, "0.1: dropped");
    let edge = factors
        .for_cell("opus", Effort::Medium)
        .expect("the check's clamped row");
    assert_eq!((edge.factor, edge.clamped), (CLAMP.1, true));
    assert!(
        edge.words().contains("the record reads x9.00"),
        "{}",
        edge.words()
    );
    assert_eq!(dropped_rows(&cal), 5, "0, -2, NaN, 1e9 and 0.1");
    let sonnet = factors
        .for_cell("sonnet", Effort::Medium)
        .expect("a good row");
    assert_eq!((sonnet.factor, sonnet.clamped), (0.37, false));
    assert_eq!(
        factors.for_cell("sonnet", Effort::High).map(|a| a.factor),
        Some(0.41),
        "a number as text"
    );
    assert!(
        cal.cost[5].median.is_nan(),
        "null reads as NaN, not as a lost file"
    );
    // A row under five orders books nothing, whatever it says.
    let mut thin = cal.clone();
    thin.cost[4].n = 4;
    assert_eq!(
        Factors::from_calibration(&thin, None).for_cell("sonnet", Effort::Medium),
        None
    );
    // A project's dropped row falls back to the pooled figure.
    let mut own = cal.clone();
    own.cost.push(CostRow {
        project: Some("c:/p".into()),
        factor: Some(f64::NAN),
        ..cal.cost[4].clone()
    });
    assert_eq!(
        Factors::from_calibration(&own, Some("c:/p".into()))
            .for_cell("sonnet", Effort::Medium)
            .map(|a| a.factor),
        Some(0.37)
    );
    // Another version: ignored whole, and due.
    std::fs::write(
        path(&dir),
        serde_json::to_string(&file(VERSION + 1)).unwrap(),
    )
    .unwrap();
    assert_eq!(read(&dir), None);
    assert_eq!(
        nirvana_seat::pooled_factors(&table, &dir),
        Factors::default()
    );
    std::fs::write(dir.join(nirvana_seat::RECORD_FILE), FIXTURE).unwrap();
    assert!(due(&dir, &table, NOW));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The P46a second review: what the fingerprint must not see, and what it must. Moving blocks (seats, classes, pools,
/// cells, price rows), turning a seat off, a class's bars and `0.5` written as `5e-1` keep it; a change to any one
/// window of any pool's `per_usd` changes it.
#[test]
fn p97_the_fingerprint_sees_only_what_the_ratio_reads() {
    let table = v18_on();
    let mark = fingerprint(&table);
    let mut same: Vec<(String, Table)> = Vec::new();
    let mut moved = table.clone();
    moved.seats.reverse();
    moved.classes.reverse();
    moved.pools.reverse();
    for seat in &mut moved.seats {
        seat.uses.reverse();
        seat.prices.reverse();
    }
    same.push(("every block moved".into(), moved));
    let mut off = table.clone();
    off.seats[0].enabled = !off.seats[0].enabled;
    same.push(("a seat turned off or on".into(), off));
    let mut bars = table.clone();
    let with_bars = bars.classes.iter().position(|class| class.bars.is_some()).expect("a class with bars");
    bars.classes[with_bars].bars = None;
    same.push(("a class's bars".into(), bars));
    assert_eq!(V18.matches("\nweight = 0.5\n").count(), 3, "three class weights of 0.5");
    let mut written = parse(&V18.replace("\nweight = 0.5\n", "\nweight = 5e-1\n")).expect("5e-1 parses");
    written.calibration = true;
    same.push(("0.5 written as 5e-1".into(), written));
    for (what, edited) in &same {
        assert_eq!(fingerprint(edited), mark, "{what}");
    }
    let mut windows = 0;
    for (p, pool) in table.pools.iter().enumerate() {
        for window in pool.per_usd.keys() {
            let mut edited = table.clone();
            *edited.pools[p].per_usd.get_mut(window).unwrap() *= 2.0;
            assert_ne!(fingerprint(&edited), mark, "pool {} per_usd {window}", pool.id);
            windows += 1;
        }
    }
    assert!(windows > table.pools.len(), "more than the week window: {windows}");
}

/// The P46a second review: a back-off note dated after now (a clock moved back) is ignored, where it held every check
/// off until that time.
#[test]
fn p97_a_back_off_note_from_the_future_is_ignored() {
    let dir = temp("future-backoff");
    let table = v18_on();
    std::fs::write(dir.join(nirvana_seat::RECORD_FILE), FIXTURE).unwrap();
    assert!(due(&dir, &table, NOW), "no file: due");
    std::fs::write(failed_path(&dir), format!("{}\n", NOW - 60)).unwrap();
    assert!(!due(&dir, &table, NOW), "a minute ago: backing off");
    std::fs::write(failed_path(&dir), format!("{}\n", NOW + 30 * 86_400)).unwrap();
    assert!(due(&dir, &table, NOW), "a month ahead: ignored");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The P46a review, finding 4: the idle check is bounded. A missing journal and one over the byte cap are skipped by
/// their metadata; past the file cap the oldest runs are skipped; past the budget every journal left is, and the
/// check still completes and says how many it skipped. A write that fails backs off instead of replaying at every
/// tick, and says so; a write that works clears it.
#[test]
fn p97_the_idle_check_is_bounded() {
    let root = temp("bounded");
    let line = |run: &str, ago: u64| -> RecordLine {
        serde_json::from_value(json!({
            "order": format!("o-{run}"), "run": run, "seat": "opus", "effort": "high", "class": "plan",
            "outcome": "pass", "at": NOW - ago,
            "journal": format!("{}/.nirvana/runs/{run}/journal.jsonl#1", root.display()),
        }))
        .expect("a line")
    };
    let journal = |run: &str, bytes: usize| {
        let path = root
            .join(".nirvana")
            .join("runs")
            .join(run)
            .join("journal.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, " ".repeat(bytes)).unwrap();
    };
    journal("r-new", 10);
    journal("r-mid", 10);
    journal("r-big", 5_000);
    let lines = vec![
        line("r-new", 60),
        line("r-mid", 600),
        line("r-big", 900),
        line("r-gone", 1_200),
    ];
    let limits = Limits {
        max_bytes: 1_000,
        max_files: 5,
        budget: std::time::Duration::from_secs(60),
    };
    let got = read_journals(&lines, NOW, &limits, std::time::Instant::now());
    assert_eq!(
        (got.files, got.skipped),
        (
            2,
            Skipped {
                missing: 1,
                large: 1,
                many: 0,
                late: 0
            }
        )
    );
    let capped = read_journals(
        &lines,
        NOW,
        &Limits {
            max_files: 1,
            ..limits
        },
        std::time::Instant::now(),
    );
    assert_eq!(capped.files, 1);
    assert_eq!(
        capped.skipped.many, 3,
        "the three older runs: {:?}",
        capped.skipped
    );
    let late = read_journals(
        &lines,
        NOW,
        &Limits {
            budget: std::time::Duration::ZERO,
            ..limits
        },
        std::time::Instant::now(),
    );
    assert_eq!((late.files, late.skipped.late), (0, 4));
    assert_eq!(
        late.skipped.words(&limits).as_deref(),
        Some("4 journal files skipped (4 past the 60 s budget)")
    );
    assert_eq!(
        got.skipped.words(&limits).as_deref(),
        Some("2 journal files skipped (1 missing or unreadable, 1 over 0 MB)")
    );
    // The whole check completes past its budget and says so in its summary.
    let dir = temp("bounded-home");
    let record: String = lines
        .iter()
        .map(|l| format!("{}\n", serde_json::to_string(l).unwrap()))
        .collect();
    std::fs::write(dir.join(nirvana_seat::RECORD_FILE), &record).unwrap();
    let table = v18_on();
    let zero = Limits {
        budget: std::time::Duration::ZERO,
        ..Limits::default()
    };
    let checked = compute(&dir, &table, NOW, &zero).expect("completes");
    assert_eq!((checked.journals, checked.skipped.late), (0, 4));
    assert!(
        checked
            .calibration
            .summary
            .ends_with("; 4 journal files skipped (4 past the 0 s budget)"),
        "{}",
        checked.calibration.summary
    );
    assert!(text(&checked, Path::new("r")).contains("0 journal files read, 4 skipped\n"));
    // A write that fails: said, and backed off for an hour; the record is not read again meanwhile.
    std::fs::create_dir(path(&dir)).unwrap();
    let said = tick(&dir, &table, NOW).expect("said");
    assert!(
        said.starts_with("the check could not be written ("),
        "{said}"
    );
    assert!(said.ends_with("; the next try is in 60 min"), "{said}");
    assert_eq!(
        std::fs::read_to_string(failed_path(&dir)).unwrap(),
        format!("{NOW}\n")
    );
    assert!(!due(&dir, &table, NOW + BACKOFF_S - 1), "backing off");
    assert_eq!(tick(&dir, &table, NOW + 60), None);
    assert!(due(&dir, &table, NOW + BACKOFF_S), "an hour on");
    std::fs::remove_dir(path(&dir)).unwrap();
    tick(&dir, &table, NOW + BACKOFF_S).expect("checked");
    assert!(
        read(&dir).is_some() && !failed_path(&dir).exists(),
        "written, and the note cleared"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&root);
}

/// The P46a review, finding 6: cost reads one line per order and run, the last, as the scores do. A corrected line
/// repeating an order's tokens no longer counts it twice toward the n of 5.
#[test]
fn p97_cost_counts_one_line_per_order() {
    let lines = fixture();
    let base = calibrate(&v18(), &lines, &Journals::default(), NOW);
    let hydrogen = pooled(&base, "hydrogen", Effort::Low).clone();
    let mut doubled = lines.clone();
    doubled.extend(lines.iter().filter(|l| l.seat == "hydrogen").cloned());
    let again = calibrate(&v18(), &doubled, &Journals::default(), NOW);
    assert_eq!(
        pooled(&again, "hydrogen", Effort::Low),
        &hydrogen,
        "a repeated line counts once"
    );
    // Four orders written twice stay four: no factor.
    let four: Vec<RecordLine> = lines
        .iter()
        .filter(|l| l.seat == "carbon")
        .cloned()
        .collect();
    assert_eq!(four.len(), 4);
    let twice: Vec<RecordLine> = four.iter().chain(&four).cloned().collect();
    let row = calibrate(&v18(), &twice, &Journals::default(), NOW);
    assert_eq!((row.cost[0].n, row.cost[0].factor), (4, None));
    // The last line of an order is the one read.
    let mut corrected = four[0].clone();
    corrected.tokens = corrected.tokens.map(|mut t| {
        t.output *= 10;
        t
    });
    let last = calibrate(
        &v18(),
        &[four[0].clone(), corrected.clone()],
        &Journals::default(),
        NOW,
    );
    let only = calibrate(&v18(), &[corrected], &Journals::default(), NOW);
    assert_eq!(last.cost[0].median, only.cost[0].median);
}
