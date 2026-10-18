//! Plan 12 O8b: Jev's roles 2 and 3. The live Jev is never reached: a fake client answers from the question it is
//! sent, so a recorded answer is exact and the declared options are the ones the contract checks.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::*;
use crate::nirvana_run::{replay, Seated};
use crate::nirvana_seat::{parse, Mode, DEFAULT};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nirvana-jev-roles-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

/// What the fake says to one question.
#[derive(Clone)]
enum Say {
    /// A choice with its confidence.
    Choice(String, f64),
    /// A yes/no probability.
    Noul(f64),
}

/// A reply in the wire's shape, built from the questions that were sent so the declared options always match.
fn reply_to(body: &str, say: &dyn Fn(&str) -> Say) -> String {
    let sent: Value = serde_json::from_str(body).expect("the body is JSON");
    let mut answers = Map::new();
    for (id, question) in sent["questions"].as_object().expect("questions") {
        let answer = match (question["type"].as_str(), say(id)) {
            (Some("noul"), Say::Noul(p)) => json!({"type": "noul", "noul": p}),
            (Some("choice"), Say::Choice(choice, confidence)) => {
                let keys: Vec<String> = question["criteria"]
                    .as_object()
                    .expect("criteria")
                    .keys()
                    .cloned()
                    .collect();
                assert!(keys.contains(&choice), "{choice} is not among {keys:?}");
                let rest = (1.0 - confidence) / (keys.len() as f64 - 1.0);
                let probabilities: Map<String, Value> = keys
                    .iter()
                    .map(|k| {
                        (
                            k.clone(),
                            json!(if *k == choice { confidence } else { rest }),
                        )
                    })
                    .collect();
                json!({"type": "choice", "choice": choice, "confidence": confidence, "probabilities": probabilities})
            }
            (kind, _) => panic!("the fake has no answer for {kind:?} {id}"),
        };
        answers.insert(id.clone(), answer);
    }
    json!({"model": "jev-test", "answers": answers,
           "usage": {"input_tokens": 900, "output_tokens": 12, "cost": 0.0003}})
    .to_string()
}

/// A client that answers by `say` and records every body it was sent.
fn fake(
    say: impl Fn(&str) -> Say + Send + Sync + 'static,
    seen: Arc<Mutex<Vec<String>>>,
) -> Client {
    Client::mock(Box::new(move |_url, body, _key, _left| {
        seen.lock().unwrap().push(body.to_owned());
        Ok((200, reply_to(body, &say), None))
    }))
}

fn failing(seen: Arc<Mutex<Vec<String>>>) -> Client {
    Client::mock(Box::new(move |_url, body, _key, _left| {
        seen.lock().unwrap().push(body.to_owned());
        Ok((500, String::new(), None))
    }))
}

fn table() -> Table {
    parse(DEFAULT).expect("the default table parses")
}

fn world() -> World {
    World {
        now: 1_800_000_000,
        ..World::default()
    }
}

/// A class that two seats clear at Balanced in the default table with no readings (`build.bounded` clears only
/// one at Balanced and four at Efficient: Jev has nothing to choose among there).
const CLASS: &str = "plan";

fn rule_pick(table: &Table, world: &World) -> Pick {
    crate::nirvana_seat::pick(table, world, &Request::new(CLASS)).expect("a seat")
}

#[test]
fn the_rule_leaves_more_than_one_seat_for_the_class_these_tests_use() {
    let pick = rule_pick(&table(), &world());
    assert!(
        pick.contenders.len() >= 2,
        "the tests need two contenders: {:?}",
        pick.contenders
    );
    assert_eq!(pick.contenders[0].seat, pick.seat, "the pick leads");
}

fn the_other(pick: &Pick) -> String {
    pick.contenders[1].seat.clone()
}

fn calls(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join(CALLS_FILE))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("a log line is JSON"))
        .collect()
}

#[test]
fn shadow_records_both_picks_and_changes_nothing() {
    let (table, world) = (table(), world());
    let rule = rule_pick(&table, &world);
    let other = the_other(&rule);
    let dir = scratch("shadow");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let pick_other = other.clone();
    let client = fake(move |_| Say::Choice(pick_other.clone(), 0.9), seen.clone());
    let got = advise_seating(
        Some(&client),
        &dir,
        Setting::Shadow,
        &table,
        &world,
        &Request::new(CLASS),
        rule.clone(),
        true,
    );
    assert_eq!(got.seat, rule.seat, "shadow seats the rule's pick");
    assert_eq!(got.effort, rule.effort);
    let jev = got.jev_pick.expect("Jev's pick is recorded");
    assert_eq!(jev.seat.as_deref(), Some(other.as_str()));
    assert_eq!(jev.rule, rule.seat);
    assert!(!jev.agrees && !jev.acted);
    assert_eq!(jev.mode, "shadow");
    assert!(
        got.reason.contains("Jev (shadow) would seat")
            && got
                .reason
                .contains(&format!("the rule seated {}", rule.seat)),
        "{}",
        got.reason
    );
    assert_eq!(seen.lock().unwrap().len(), 1, "one call");
    let log = calls(&dir);
    assert_eq!(log.len(), 1);
    assert_eq!(log[0]["role"], "seating");
    assert_eq!(log[0]["answer"], other.as_str());
    assert_eq!(log[0]["rule"], rule.seat.as_str());
    assert_eq!(log[0]["outcome"], "answered");
    assert_eq!(log[0]["cost_usd"], 0.0003);
    assert!(log[0]["ms"].is_u64() && log[0]["score"] == 0.9);
    assert_eq!(log[0]["prompt"].as_str().map(str::len), Some(16));
    // The class's description and the seats' facts went; the log holds a hash, not the words.
    let sent = seen.lock().unwrap()[0].clone();
    assert!(sent.contains("score_for_this_class"));
    assert!(!std::fs::read_to_string(dir.join(CALLS_FILE))
        .unwrap()
        .contains("score_for_this_class"));
}

#[test]
fn jev_agreeing_with_the_rule_is_said_and_logged_as_the_rules_seat() {
    let (table, world) = (table(), world());
    let rule = rule_pick(&table, &world);
    let dir = scratch("agree");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let same = rule.seat.clone();
    let client = fake(move |_| Say::Choice(same.clone(), 0.95), seen);
    let got = advise_seating(
        Some(&client),
        &dir,
        Setting::Shadow,
        &table,
        &world,
        &Request::new(CLASS),
        rule.clone(),
        true,
    );
    let jev = got.jev_pick.expect("recorded");
    assert!(jev.agrees && jev.seat.as_deref() == Some(rule.seat.as_str()));
    assert!(got.reason.contains("Jev (shadow) agrees"), "{}", got.reason);
    let text = std::fs::read_to_string(dir.join(CALLS_FILE)).unwrap();
    let a = agreement_in(&text);
    assert_eq!(
        (a.seating_answered, a.seating_agreed, a.seating_differed),
        (1, 1, 0)
    );
}

#[test]
fn the_choice_floor_and_a_none_are_never_a_seat() {
    let (table, world) = (table(), world());
    let rule = rule_pick(&table, &world);
    let other = the_other(&rule);
    for (say, answer) in [
        // Under the 0.7 floor: unsure, though it names a seat.
        (Say::Choice(other.clone(), 0.6), "unsure"),
        // Exactly at the floor is a choice: 0.7.
        (Say::Choice(NONE.into(), 0.95), "none"),
    ] {
        let dir = scratch("floor");
        let client = fake(move |_| say.clone(), Arc::default());
        let got = advise_seating(
            Some(&client),
            &dir,
            Setting::On,
            &table,
            &world,
            &Request::new(CLASS),
            rule.clone(),
            true,
        );
        let jev = got.jev_pick.expect("recorded");
        assert_eq!(jev.answer, answer);
        assert!(jev.seat.is_none() && !jev.acted);
        assert_eq!(
            got.seat, rule.seat,
            "{answer} never moves the seat, on included"
        );
    }
    // At the floor exactly: 0.7 is a choice.
    let dir = scratch("floor-at");
    let at = other.clone();
    let client = fake(move |_| Say::Choice(at.clone(), 0.7), Arc::default());
    let got = advise_seating(
        Some(&client),
        &dir,
        Setting::Shadow,
        &table,
        &world,
        &Request::new(CLASS),
        rule,
        true,
    );
    assert_eq!(got.jev_pick.unwrap().seat.as_deref(), Some(other.as_str()));
}

#[test]
fn a_failed_call_is_never_a_choice_and_the_pick_stands() {
    let (table, world) = (table(), world());
    let rule = rule_pick(&table, &world);
    let dir = scratch("failed");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let client = failing(seen.clone());
    let got = advise_seating(
        Some(&client),
        &dir,
        Setting::On,
        &table,
        &world,
        &Request::new(CLASS),
        rule.clone(),
        true,
    );
    assert_eq!(got.seat, rule.seat);
    let jev = got.jev_pick.expect("a failure is recorded");
    assert_eq!(jev.answer, "failed");
    assert!(jev.seat.is_none() && !jev.acted && !jev.agrees);
    assert!(got.reason.contains("Jev (on) failed"), "{}", got.reason);
    let log = calls(&dir);
    assert_eq!(log[0]["outcome"], "failed");
    assert!(log[0]["answer"].is_null() && log[0]["score"].is_null());
    let a = agreement_in(&std::fs::read_to_string(dir.join(CALLS_FILE)).unwrap());
    assert_eq!((a.seating_failed, a.seating_answered), (1, 0));
}

#[test]
fn no_key_off_one_contender_or_an_override_asks_nothing() {
    let (table, world) = (table(), world());
    let rule = rule_pick(&table, &world);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let client = fake(|_| Say::Choice(NONE.into(), 0.9), seen.clone());
    let dir = scratch("quiet");
    // Off.
    let got = advise_seating(
        Some(&client),
        &dir,
        Setting::Off,
        &table,
        &world,
        &Request::new(CLASS),
        rule.clone(),
        true,
    );
    assert!(got.jev_pick.is_none());
    // One contender is no question.
    let mut single = rule.clone();
    single.contenders.truncate(1);
    let got = advise_seating(
        Some(&client),
        &dir,
        Setting::Shadow,
        &table,
        &world,
        &Request::new(CLASS),
        single,
        true,
    );
    assert!(got.jev_pick.is_none());
    // The orchestrator's override stands without Jev.
    let mut over = rule.clone();
    over.override_reason = Some("because".into());
    let got = advise_seating(
        Some(&client),
        &dir,
        Setting::Shadow,
        &table,
        &world,
        &Request::new(CLASS),
        over,
        true,
    );
    assert!(got.jev_pick.is_none());
    // No client: no key.
    let got = advise_seating(
        None,
        &dir,
        Setting::Shadow,
        &table,
        &world,
        &Request::new(CLASS),
        rule,
        true,
    );
    assert!(got.jev_pick.is_none());
    assert!(seen.lock().unwrap().is_empty(), "no call was made");
    assert!(!dir.join(CALLS_FILE).exists(), "and nothing was logged");
}

#[test]
fn on_takes_jevs_seat_through_the_override_guard_and_the_run_cap_holds() {
    let (table, mut world) = (table(), world());
    let rule = rule_pick(&table, &world);
    let other = the_other(&rule);
    let want = other.clone();
    let say = move |_: &str| Say::Choice(want.clone(), 0.92);
    // On, and the caller lets it act: Jev's seat, as an override, with the contenders kept.
    let dir = scratch("on");
    let got = advise_seating(
        Some(&fake(say.clone(), Arc::default())),
        &dir,
        Setting::On,
        &table,
        &world,
        &Request::new(CLASS),
        rule.clone(),
        true,
    );
    assert_eq!(got.seat, other, "Jev's seat is seated");
    assert!(got
        .override_reason
        .as_deref()
        .unwrap_or("")
        .contains("Jev advised"));
    assert_eq!(
        got.rule_pick.as_deref().map(|p| p.starts_with(&rule.seat)),
        Some(true)
    );
    let jev = got.jev_pick.as_ref().expect("recorded");
    assert!(jev.acted && jev.mode == "on");
    assert!(got.reason.contains("Jev (on) took"), "{}", got.reason);
    assert!(got.contenders.len() >= 2);
    // A caller that named its seat already (`seat run <seat>`) keeps it: on records and does not act.
    let got = advise_seating(
        Some(&fake(say.clone(), Arc::default())),
        &scratch("on-named"),
        Setting::On,
        &table,
        &world,
        &Request::new(CLASS),
        rule.clone(),
        false,
    );
    assert_eq!(got.seat, rule.seat);
    assert!(!got.jev_pick.unwrap().acted);
    // The run has taken its overrides: the guard refuses, and the rule's pick stands with the reason.
    world.overrides_used = table.constants.overrides_per_run;
    let got = advise_seating(
        Some(&fake(say, Arc::default())),
        &scratch("on-cap"),
        Setting::On,
        &table,
        &world,
        &Request::new(CLASS),
        rule.clone(),
        true,
    );
    assert_eq!(got.seat, rule.seat);
    let jev = got.jev_pick.expect("recorded");
    assert!(!jev.acted);
    assert!(
        jev.note
            .as_deref()
            .unwrap_or("")
            .contains("overrides already taken"),
        "{jev:?}"
    );
    assert!(
        got.reason.contains("Jev (on) was refused"),
        "{}",
        got.reason
    );
}

#[test]
fn the_seating_question_offers_each_contender_and_none_and_carries_no_order_text() {
    let (table, world) = (table(), world());
    let rule = rule_pick(&table, &world);
    let mut request = Request::new(CLASS);
    request.mode = Mode::Sota;
    let (state, questions) = seating_question(&table, &request, &rule).expect("a question");
    let options: Vec<String> = questions["seat"]["criteria"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    for contender in &rule.contenders {
        assert!(options.contains(&contender.seat), "{options:?}");
    }
    assert!(options.contains(&NONE.to_string()));
    assert_eq!(options.len(), rule.contenders.len() + 1);
    assert_eq!(state["order"]["mode"], "sota");
    assert_eq!(state["order"]["class"], CLASS);
    assert!(state["order"]["what_the_class_is"].as_str().is_some());
    let mut single = rule;
    single.contenders.truncate(1);
    assert!(seating_question(&table, &request, &single).is_none());
}

#[test]
fn a_jev_pick_rides_the_seated_event_and_an_old_line_without_it_still_reads() {
    let (table, world) = (table(), world());
    let rule = rule_pick(&table, &world);
    let jev = JevPick {
        seat: Some(the_other(&rule)),
        answer: "seat".into(),
        score: 0.9,
        rule: rule.seat.clone(),
        agrees: false,
        mode: "shadow".into(),
        acted: false,
        note: None,
    };
    let mut seated: Value = json!({
        "order": "a", "seat": "sonnet", "cli": "claude", "model": "m", "effort": "high", "pool": "max",
        "class": CLASS, "size": "small", "mode": "balanced", "reason": "r", "order_file": "o", "cwd": "c"
    });
    let old: Seated = serde_json::from_value(seated.clone()).expect("a line before O8b reads");
    assert!(old.jev_pick.is_none());
    seated["jev_pick"] = serde_json::to_value(&jev).unwrap();
    let read: Seated = serde_json::from_value(seated).unwrap();
    assert_eq!(read.jev_pick.as_ref(), Some(&jev));
    let line = serde_json::to_string(&read).unwrap();
    assert!(line.contains("\"jev_pick\""));
    assert!(
        !serde_json::to_string(&old).unwrap().contains("jev_pick"),
        "none is not written"
    );
}

#[test]
fn the_pick_prints_its_contenders_and_jevs_advice_in_json() {
    let (table, world) = (table(), world());
    let rule = rule_pick(&table, &world);
    let value = serde_json::to_value(&rule).unwrap();
    assert!(value["contenders"].as_array().map_or(0, Vec::len) >= 2);
    assert!(value.get("jev_pick").is_none(), "no advice, no field");
    // One seat left is no question, and the empty list is not printed.
    let mut single = rule;
    single.contenders.clear();
    assert!(serde_json::to_value(&single)
        .unwrap()
        .get("contenders")
        .is_none());
}

// ---- the setting and the log -----------------------------------------------------------------------

#[test]
fn the_setting_is_shadow_by_default_off_for_a_word_it_does_not_know_and_set_by_the_cli() {
    let dir = scratch("settings");
    assert_eq!(
        settings_in(&dir),
        Settings {
            watch: Setting::Shadow,
            seating: Setting::Shadow
        }
    );
    std::fs::write(
        dir.join(SETTINGS_FILE),
        "watch = \"on\"\nseating = \"onn\"\n",
    )
    .unwrap();
    assert_eq!(
        settings_in(&dir),
        Settings {
            watch: Setting::On,
            seating: Setting::Off
        }
    );
    std::fs::write(dir.join(SETTINGS_FILE), "watch = 3\n").unwrap();
    assert_eq!(
        settings_in(&dir),
        Settings {
            watch: Setting::Off,
            seating: Setting::Shadow
        }
    );
    std::fs::write(dir.join(SETTINGS_FILE), "this is not toml [").unwrap();
    assert_eq!(
        settings_in(&dir).watch,
        Setting::Shadow,
        "a broken file is the default, not on"
    );
    write_setting(&dir, Role::Seating, Setting::On).unwrap();
    write_setting(&dir, Role::Watch, Setting::Off).unwrap();
    assert_eq!(
        settings_in(&dir),
        Settings {
            watch: Setting::Off,
            seating: Setting::On
        }
    );
}

#[test]
fn agree_counts_agreement_difference_declines_failures_and_the_watch() {
    let text = [
        r#"{"role":"seating","outcome":"answered","answer":"sonnet","rule":"sonnet","cost_usd":0.001}"#,
        r#"{"role":"seating","outcome":"answered","answer":"opus","rule":"sonnet","cost_usd":0.001}"#,
        r#"{"role":"seating","outcome":"answered","answer":"none","rule":"sonnet","cost_usd":0.001}"#,
        r#"{"role":"seating","outcome":"answered","answer":"unsure","rule":"sonnet","cost_usd":0.001}"#,
        r#"{"role":"seating","outcome":"failed","answer":null}"#,
        r#"{"role":"watch","outcome":"answered","answer":"yes","cost_usd":0.002}"#,
        r#"{"role":"watch","outcome":"answered","answer":"no","cost_usd":0.002}"#,
        r#"{"role":"watch","outcome":"failed"}"#,
        "not json",
    ]
    .join("\n");
    let a = agreement_in(&text);
    assert_eq!(
        (
            a.seating_answered,
            a.seating_agreed,
            a.seating_differed,
            a.seating_declined,
            a.seating_failed
        ),
        (4, 1, 1, 2, 1)
    );
    assert_eq!((a.watch_answered, a.watch_yes, a.watch_failed), (2, 1, 1));
    assert!((a.cost_usd - 0.008).abs() < 1e-9, "{}", a.cost_usd);
    let said = agreement_text(&a);
    assert!(said.contains("50% of 2 choices matched the rule"), "{said}");
    assert!(agreement_text(&Agreement::default()).contains("no choice yet"));
}

// ---- role 2 ----------------------------------------------------------------------------------------

fn reply_of(say: Vec<(&str, Say)>) -> Reply {
    let mut answers = Map::new();
    for (id, say) in say {
        answers.insert(
            id.into(),
            match say {
                Say::Noul(p) => json!({"type": "noul", "noul": p}),
                Say::Choice(c, p) => json!({"type": "choice", "choice": c, "confidence": p}),
            },
        );
    }
    Reply {
        model: "jev-test".into(),
        answers,
        input_tokens: 1,
        output_tokens: 1,
        cost_usd: Some(0.0),
        ms: 1,
    }
}

#[test]
fn the_floors_make_a_yes_a_no_or_an_unsure() {
    let watch = |flag: f64, kind: &str, confidence: f64| {
        read_watch(&reply_of(vec![
            ("flag", Say::Noul(flag)),
            ("kind", Say::Choice(kind.into(), confidence)),
        ]))
    };
    // The 0.7 floor is a yes, with the kind Jev is sure of.
    let yes = watch(0.7, "looping", 0.9);
    assert_eq!((yes.answer, yes.kind.as_deref()), ("yes", Some("looping")));
    // Just under it is unsure.
    assert_eq!(watch(0.69, "looping", 0.9).answer, "unsure");
    // The 0.3 ceiling is a no; just over it is unsure.
    assert_eq!(watch(0.3, "working", 0.9).answer, "no");
    assert_eq!(watch(0.31, "working", 0.9).answer, "unsure");
    // A kind Jev is unsure of still yields a yes, named stuck.
    let vague = watch(0.95, "looping", 0.5);
    assert_eq!(
        (vague.answer, vague.kind.as_deref()),
        ("yes", Some("stuck"))
    );
    // A yes that Jev's own kind answer calls plain work is a contradiction, so it is not a yes.
    assert_eq!(watch(0.95, "working", 0.9).answer, "unsure");
}

#[test]
fn a_failed_watch_is_never_a_yes_and_the_log_says_failed() {
    let dir = scratch("watch-failed");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let verdict = judge(
        &failing(seen),
        &dir,
        "the agent is asking: continue?",
        None,
        json!({"order": "a"}),
    );
    assert!(verdict.is_err());
    let log = calls(&dir);
    assert_eq!(
        (log[0]["role"].as_str(), log[0]["outcome"].as_str()),
        (Some("watch"), Some("failed"))
    );
    assert_eq!(log[0]["order"], "a");
}

#[test]
fn a_screen_is_cut_to_its_tail_and_redacted_before_it_is_sent() {
    let secret = "sk-abcdefghijklmnopqrstuvwxyz0123456789";
    let mut screen: String = (0..200).map(|i| format!("old line {i}\n")).collect();
    screen.push_str(&format!("export KEY={secret}\nthe last line\n"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let client = fake(
        |id| {
            if id == "flag" {
                Say::Noul(0.1)
            } else {
                Say::Choice("working".into(), 0.9)
            }
        },
        seen.clone(),
    );
    let dir = scratch("redact");
    judge(&client, &dir, &screen, None, json!({})).expect("answered");
    let sent = seen.lock().unwrap()[0].clone();
    assert!(!sent.contains(secret), "the secret did not leave");
    assert!(sent.contains("the last line"));
    assert!(!sent.contains("old line 0\\n"), "the old head was cut");
    assert!(sent.contains("old line 199"));
    assert!(sent.contains("untrusted_screen"));
}

fn journal_with_worker(dir: &Path, pane: Option<&str>, started: u64) -> Journal {
    let journal = Journal::new(dir.join("journal.jsonl"));
    journal
        .append(
            1000,
            Kind::Opened {
                project: "C:/p".into(),
                orchestrator_pane: Some("orch".into()),
                auto: true,
                pools: Vec::new(),
                pools_basis: None,
            },
        )
        .unwrap();
    let seated: Seated = serde_json::from_value(json!({
        "order": "a", "seat": "sonnet", "cli": "claude", "model": "m", "effort": "high", "pool": "max",
        "class": CLASS, "size": "small", "mode": "balanced", "reason": "r", "order_file": "o", "cwd": "c"
    }))
    .unwrap();
    journal
        .append(1000, Kind::Seated(Box::new(seated)))
        .unwrap();
    journal
        .append(
            started,
            Kind::Started {
                order: "a".into(),
                attempt: 1,
                pid: None,
                pane: pane.map(str::to_owned),
                tab: None,
                resume: None,
                argv: Vec::new(),
            },
        )
        .unwrap();
    journal
}

fn say_looping() -> impl Fn(&str) -> Say + Send + Sync + 'static {
    |id| {
        if id == "flag" {
            Say::Noul(0.93)
        } else {
            Say::Choice("looping".into(), 0.9)
        }
    }
}

#[test]
fn a_worker_is_due_after_ten_minutes_with_a_pane_and_not_twice_in_five() {
    let dir = scratch("due");
    let journal = journal_with_worker(&dir, Some("p1"), 2000);
    let (events, _) = journal.read();
    let state = replay(&events);
    let live = |_: &str| true;
    assert!(
        due(&events, &state, 2000 + WATCH_AFTER_S, &live).is_empty(),
        "ten minutes exactly is not over"
    );
    assert_eq!(
        due(&events, &state, 2000 + WATCH_AFTER_S + 1, &live).len(),
        1
    );
    assert!(
        due(&events, &state, 5000, &|_: &str| false).is_empty(),
        "a worker with no supervisor is not watched"
    );
    // Watched at 3000: not again until five minutes later.
    journal
        .append(
            3000,
            Kind::JevWatch {
                order: "a".into(),
                answer: "no".into(),
                kind: None,
                score: 0.1,
                acted: false,
            },
        )
        .unwrap();
    let (events, _) = journal.read();
    let state = replay(&events);
    assert!(due(&events, &state, 3000 + WATCH_EVERY_S - 1, &live).is_empty());
    assert_eq!(due(&events, &state, 3000 + WATCH_EVERY_S, &live).len(), 1);
    // A restart moves the clock: ten minutes run from the last start, not the first.
    let restarted = journal_with_worker(&scratch("due-restart"), Some("p1"), 2000);
    restarted
        .append(
            2900,
            Kind::Started {
                order: "a".into(),
                attempt: 2,
                pid: None,
                pane: Some("p1".into()),
                tab: None,
                resume: None,
                argv: Vec::new(),
            },
        )
        .unwrap();
    let (events, _) = restarted.read();
    let state = replay(&events);
    assert!(due(&events, &state, 2000 + WATCH_AFTER_S + 1, &live).is_empty());
    assert_eq!(
        due(&events, &state, 2900 + WATCH_AFTER_S + 1, &live).len(),
        1
    );
    // An order that is over is not watched.
    let over = journal_with_worker(&scratch("due-over"), Some("p1"), 2000);
    over.append(
        3000,
        Kind::Ended {
            order: Some("a".into()),
            outcome: "delivered".into(),
            why: None,
            exit: Some(0),
            pools: Vec::new(),
            pools_basis: None,
        },
    )
    .unwrap();
    let (events, _) = over.read();
    assert!(due(&events, &replay(&events), 9000, &live).is_empty());
    // A headless worker has no pane to read.
    let headless = journal_with_worker(&scratch("due-headless"), None, 2000);
    let (events, _) = headless.read();
    assert!(due(&events, &replay(&events), 9000, &live).is_empty());
}

#[test]
fn shadow_writes_the_watch_event_and_wakes_nobody() {
    let dir = scratch("watch-shadow");
    let journal = journal_with_worker(&dir, Some("p1"), 2000);
    let client = fake(say_looping(), Arc::default());
    let screens = Mutex::new(Vec::new());
    let read = |pane: &str| {
        screens.lock().unwrap().push(pane.to_owned());
        Some("same error again\nsame error again\n".to_owned())
    };
    let lines = watch_tick(
        &journal,
        &dir,
        Setting::Shadow,
        Some(&client),
        None,
        4000,
        &|_| true,
        &read,
    )
    .unwrap();
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("Jev (shadow) reads its screen: yes (looping) 0.93"),
        "{}",
        lines[0]
    );
    assert_eq!(*screens.lock().unwrap(), vec!["p1".to_owned()]);
    let (events, _) = journal.read();
    let state = replay(&events);
    let watch = events.iter().find_map(|e| match &e.kind {
        Kind::JevWatch {
            order,
            answer,
            kind,
            score,
            acted,
        } => Some((order.clone(), answer.clone(), kind.clone(), *score, *acted)),
        _ => None,
    });
    assert_eq!(
        watch,
        Some((
            "a".into(),
            "yes".into(),
            Some("looping".into()),
            0.93,
            false
        ))
    );
    assert!(
        state.queue.is_empty(),
        "shadow queues nothing: {:?}",
        state.queue
    );
    let order = state.order("a").unwrap();
    assert_eq!(order.phase, Phase::Running, "and moves no order");
    let log = calls(&dir);
    assert_eq!(
        (log[0]["role"].as_str(), log[0]["answer"].as_str()),
        (Some("watch"), Some("yes"))
    );
    // The next tick, a minute later, does not look again.
    let again = watch_tick(
        &journal,
        &dir,
        Setting::Shadow,
        Some(&client),
        None,
        4060,
        &|_| true,
        &read,
    )
    .unwrap();
    assert!(again.is_empty());
}

#[test]
fn on_queues_a_yes_for_the_wake_and_a_no_or_a_failure_queues_nothing() {
    let read = |_: &str| Some("screen\n".to_owned());
    // On, a yes: a line in the queue the wake path carries.
    let dir = scratch("watch-on");
    let journal = journal_with_worker(&dir, Some("p1"), 2000);
    let client = fake(say_looping(), Arc::default());
    watch_tick(
        &journal,
        &dir,
        Setting::On,
        Some(&client),
        None,
        4000,
        &|_| true,
        &read,
    )
    .unwrap();
    let state = replay(&journal.read().0);
    assert_eq!(state.queue.len(), 1, "{:?}", state.queue);
    assert!(
        state.queue[0]
            .line
            .contains("a: Jev reads its screen as looping (0.93)"),
        "{}",
        state.queue[0].line
    );
    assert_eq!(state.queue[0].order.as_deref(), Some("a"));
    // The wake decision on that queue is O3's own: an idle orchestrator is typed to, a typing owner is not.
    let view = |last_key_at| crate::nirvana_run::PaneView {
        status: Some("idle".into()),
        prompt: Some(String::new()),
        last_key_at,
        program: crate::nirvana_run::PaneProgram::NirvanaClaude,
    };
    assert!(matches!(
        crate::nirvana_run::wake_decision("r", &state, Some(&view(None)), 4001, false),
        crate::nirvana_run::Wake::Type(_)
    ));
    assert!(matches!(
        crate::nirvana_run::wake_decision("r", &state, Some(&view(Some(3990))), 4001, false),
        crate::nirvana_run::Wake::Queue(_)
    ));
    // A no: an event, no queue line.
    let dir = scratch("watch-on-no");
    let journal = journal_with_worker(&dir, Some("p1"), 2000);
    let no = fake(
        |id| {
            if id == "flag" {
                Say::Noul(0.05)
            } else {
                Say::Choice("working".into(), 0.9)
            }
        },
        Arc::default(),
    );
    watch_tick(
        &journal,
        &dir,
        Setting::On,
        Some(&no),
        None,
        4000,
        &|_| true,
        &read,
    )
    .unwrap();
    assert!(replay(&journal.read().0).queue.is_empty());
    // A failure: written as failed, never a yes, nothing queued.
    let dir = scratch("watch-on-failed");
    let journal = journal_with_worker(&dir, Some("p1"), 2000);
    let lines = watch_tick(
        &journal,
        &dir,
        Setting::On,
        Some(&failing(Arc::default())),
        None,
        4000,
        &|_| true,
        &read,
    )
    .unwrap();
    assert!(lines[0].contains("failed"), "{}", lines[0]);
    let (events, _) = journal.read();
    assert!(events
        .iter()
        .any(|e| matches!(&e.kind, Kind::JevWatch { answer, .. } if answer == "failed")));
    assert!(replay(&events).queue.is_empty());
}

#[test]
fn off_and_no_key_read_no_screen_and_ask_nothing() {
    let dir = scratch("watch-off");
    let journal = journal_with_worker(&dir, Some("p1"), 2000);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let client = fake(say_looping(), seen.clone());
    let read = |_: &str| -> Option<String> { panic!("no screen is read") };
    assert!(watch_tick(
        &journal,
        &dir,
        Setting::Off,
        Some(&client),
        None,
        4000,
        &|_| true,
        &read
    )
    .unwrap()
    .is_empty());
    assert!(watch_tick(
        &journal,
        &dir,
        Setting::Shadow,
        None,
        None,
        4000,
        &|_| true,
        &read
    )
    .unwrap()
    .is_empty());
    assert!(seen.lock().unwrap().is_empty());
    assert!(!journal
        .read()
        .0
        .iter()
        .any(|e| matches!(e.kind, Kind::JevWatch { .. })));
}

#[test]
fn an_ended_run_is_not_watched() {
    let dir = scratch("watch-ended");
    let journal = journal_with_worker(&dir, Some("p1"), 2000);
    journal
        .append(
            3000,
            Kind::Ended {
                order: None,
                outcome: "done".into(),
                why: None,
                exit: None,
                pools: Vec::new(),
                pools_basis: None,
            },
        )
        .unwrap();
    let read = |_: &str| -> Option<String> { panic!("no screen is read") };
    let client = fake(say_looping(), Arc::default());
    assert!(watch_tick(
        &journal,
        &dir,
        Setting::On,
        Some(&client),
        None,
        9000,
        &|_| true,
        &read
    )
    .unwrap()
    .is_empty());
}
