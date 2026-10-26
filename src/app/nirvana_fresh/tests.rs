//! Plan 25 §2, the server's half: tests in a pane runtime played from the recorded Claude Code screens, with a
//! temporary Nirvana home (`with_test_home`) for the pending record, the server's mark and the owner's keys.

use super::*;
use crate::detect::{Agent, AgentState};
use crate::nirvana_fresh::{FreshStretch, Record, LOOKUPS, POLL, RETRY};
use crate::nirvana_idle_compact::{IdleStretch, ENTER_TRIES, ERASED_AT, ERASE_AT};
use std::path::PathBuf;
use std::time::Duration;

const MIN: Duration = Duration::from_secs(60);
const RECORDED_EMPTY: &str = include_str!("../../nirvana_run/fixtures/claude-2.1.302-empty.ansi");
const RECORDED_TYPED: &str = include_str!("../../nirvana_run/fixtures/claude-2.1.302-typed.ansi");
/// Wide enough that no recorded line wraps, so screen rows are the recording's lines.
const COLS: u16 = 200;
/// The `❯` line's place in the recorded screens, as a cursor move: row 9, the column after the mark.
const AT_LINE_START: &str = "\x1b[9;3H";
/// The test pane's runtime takes Backspace as DEL, the encoding the terminal runtime gives it.
const BACKSPACE: &str = "\x7f";

fn test_app() -> App {
    let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(
        &crate::config::Config::default(),
        crate::app::AppPolicy::TEST,
        None,
        api_rx,
        crate::api::EventHub::default(),
    );
    app.state.workspaces = vec![crate::workspace::Workspace::test_new("work")];
    app.state.active = Some(0);
    app.state.selected = 0;
    app.state.ensure_test_terminals();
    app
}

fn root_terminal_id(app: &App) -> TerminalId {
    let tab = &app.state.workspaces[0].tabs[0];
    tab.panes[&tab.root_pane].attached_terminal_id.clone()
}

fn root_public(app: &App) -> String {
    app.public_pane_id(0, app.state.workspaces[0].tabs[0].root_pane)
        .unwrap()
}

fn screen_bytes(recorded: &str) -> Vec<u8> {
    format!("{}{AT_LINE_START}", recorded.replace('\n', "\r\n")).into_bytes()
}

/// The pane runs Claude, idle since `since`, with `recorded` on its screen; what is typed lands in the receiver.
fn idle_claude(
    app: &mut App,
    since: Instant,
    recorded: &str,
) -> tokio::sync::mpsc::Receiver<Bytes> {
    let terminal_id = root_terminal_id(app);
    let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
    terminal.detected_agent = Some(Agent::Claude);
    terminal.agent_name = Some("nirvana".into());
    terminal.state = AgentState::Idle;
    terminal.idle_stretch = IdleStretch {
        since: Some(since),
        ..IdleStretch::default()
    };
    let (runtime, receiver) = TerminalRuntime::test_with_channel_and_scrollback_bytes(
        COLS,
        24,
        0,
        &screen_bytes(recorded),
        8,
    );
    app.terminal_runtimes.insert(terminal_id, runtime);
    receiver
}

fn line_now_reads(app: &App, text: &str) {
    let runtime = app.terminal_runtimes.get(&root_terminal_id(app)).unwrap();
    runtime.test_process_pty_bytes(
        format!("\x1b[9;1H\x1b[0m❯\u{a0}{text}\x1b[K{AT_LINE_START}").as_bytes(),
    );
}

fn typed(receiver: &mut tokio::sync::mpsc::Receiver<Bytes>) -> String {
    let mut out = Vec::new();
    while let Ok(bytes) = receiver.try_recv() {
        out.extend_from_slice(&bytes);
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Nirvana Claude's process in the test home (`--agent nirvana` with this home's plugin), in the conversation the
/// records name.
fn nirvana_in(home: &Path) -> impl Fn(&PaneQuery) -> (Outside, Option<PathBuf>) + '_ {
    nirvana_with(home, Some(TRANSCRIPT))
}

/// Nirvana Claude's process in the test home, its transcript `transcript`.
fn nirvana_with<'a>(
    home: &'a Path,
    transcript: Option<&'a str>,
) -> impl Fn(&PaneQuery) -> (Outside, Option<PathBuf>) + 'a {
    move |_query| {
        let outside = Outside {
            argv: Some(vec![
                "claude".into(),
                "--plugin-dir".into(),
                crate::nirvana_launch::plugin_dir(home)
                    .display()
                    .to_string(),
                "--agent".into(),
                "nirvana".into(),
            ]),
            ..Outside::default()
        };
        (outside, transcript.map(PathBuf::from))
    }
}

fn plain_claude(_query: &PaneQuery) -> (Outside, Option<PathBuf>) {
    let outside = Outside {
        argv: Some(vec!["claude".into(), "--model".into(), "opus".into()]),
        ..Outside::default()
    };
    (outside, Some(PathBuf::from(TRANSCRIPT)))
}

/// A long-idle interactive Claude with a big context, as idle compact's `read_outside` would find it.
fn long_idle(_query: &PaneQuery) -> Outside {
    Outside {
        argv: Some(vec!["claude".into(), "--model".into(), "opus".into()]),
        transcript_age: Some(55 * MIN),
        context_tokens: Some(80_000),
    }
}

/// The pane's conversation, as the Stop hook names it in the record and the server reads it.
const TRANSCRIPT: &str = r"C:\Users\o\.claude\projects\C--work-print\abc-123.jsonl";

/// A Stop's record written now (review S1: an older one is stale), naming the pane's transcript.
fn record() -> Record {
    let at = crate::nirvana_run::now();
    Record {
        run: Some("r1".into()),
        ended_at: Some(at),
        context: Some(180_000),
        transcript: Some(TRANSCRIPT.into()),
        project: "C:\\work\\print".into(),
        at,
        by: "stop".into(),
    }
}

fn read_at(pasted: Instant, step: Step) -> Instant {
    Pending {
        pane: String::new(),
        pasted_at: pasted,
        step,
    }
    .due()
    .unwrap()
}

#[tokio::test]
async fn fresh_server_types_clear_then_enter_into_an_idle_nirvana_claude() {
    let guard = crate::nirvana::with_test_home("fresh-send");
    let home = guard.path.clone();
    let mut app = test_app();
    let t0 = Instant::now();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    let public = root_public(&app);
    let written = record();
    crate::nirvana_fresh::write_record(&home, &public, &written).unwrap();
    assert_eq!(app.fresh_deadline(), Some(t0 + LOOKUPS[0]));
    // Nothing is due before the first lookup.
    assert!(!app.nirvana_fresh_with(t0 + Duration::from_secs(1), &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "");
    let pasted = t0 + LOOKUPS[0];
    assert!(app.nirvana_fresh_with(pasted, &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "/clear");
    assert_eq!(app.fresh_deadline(), Some(pasted + ENTER_TRIES[0]));
    line_now_reads(&app, "/clear");
    assert!(app.nirvana_fresh_with(pasted + ENTER_TRIES[0], &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "\r");
    // Sent: the mark names the record, and nothing more is due in the stretch.
    assert_eq!(
        crate::nirvana_fresh::read_sent(&home, &public),
        Some(written.at)
    );
    assert!(
        app.state.terminals[&root_terminal_id(&app)]
            .idle_stretch
            .fresh
            .sent
    );
    assert_eq!(app.fresh_deadline(), None);
}

#[tokio::test]
async fn fresh_server_with_no_record_types_nothing_and_looks_again_later() {
    let guard = crate::nirvana::with_test_home("fresh-none");
    let home = guard.path.clone();
    let mut app = test_app();
    let t0 = Instant::now();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    let asked = std::cell::Cell::new(0);
    let counting = |query: &PaneQuery| {
        asked.set(asked.get() + 1);
        nirvana_in(&home)(query)
    };
    for (n, after) in LOOKUPS.iter().enumerate() {
        assert!(!app.nirvana_fresh_with(t0 + *after, &home, counting));
        let next = LOOKUPS
            .get(n + 1)
            .map_or(t0 + *after + POLL, |next| t0 + *next);
        assert_eq!(app.fresh_deadline(), Some(next));
    }
    assert_eq!(typed(&mut receiver), "");
    // No record, no process read.
    assert_eq!(asked.get(), 0);
}

#[tokio::test]
async fn fresh_server_types_only_into_nirvana_claude() {
    let guard = crate::nirvana::with_test_home("fresh-plain");
    let home = guard.path.clone();
    let mut app = test_app();
    let t0 = Instant::now();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    crate::nirvana_fresh::write_record(&home, &root_public(&app), &record()).unwrap();
    for after in LOOKUPS {
        assert!(!app.nirvana_fresh_with(t0 + after, &home, plain_claude));
    }
    assert_eq!(typed(&mut receiver), "");
    // Left for now: tried again after the owner's quiet window, not at the minute's poll.
    assert_eq!(app.fresh_deadline(), Some(t0 + LOOKUPS[2] + RETRY));
    assert!(
        !app.nirvana_fresh_with(t0 + LOOKUPS[2] + RETRY, &home, |_| {
            (Outside::default(), None)
        })
    );
    assert_eq!(typed(&mut receiver), "");
}

#[tokio::test]
async fn fresh_server_types_nothing_while_the_owner_types_or_the_line_holds_text() {
    let guard = crate::nirvana::with_test_home("fresh-owner");
    let home = guard.path.clone();
    // The typed line first: the owner's key, once noted, stays in the home for the rest of the test.
    for owner_typed in [false, true] {
        let mut app = test_app();
        let t0 = Instant::now();
        let recorded = if owner_typed {
            RECORDED_EMPTY
        } else {
            RECORDED_TYPED
        };
        let mut receiver = idle_claude(&mut app, t0, recorded);
        let public = root_public(&app);
        crate::nirvana_fresh::write_record(&home, &public, &record()).unwrap();
        if owner_typed {
            crate::nirvana_run::note_owner_key(&public);
        }
        assert!(!app.nirvana_fresh_with(t0 + LOOKUPS[0], &home, nirvana_in(&home)));
        assert_eq!(typed(&mut receiver), "", "owner typed: {owner_typed}");
    }
}

#[tokio::test]
async fn fresh_server_never_clears_twice_for_one_record() {
    let guard = crate::nirvana::with_test_home("fresh-mark");
    let home = guard.path.clone();
    let mut app = test_app();
    let t0 = Instant::now();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    let public = root_public(&app);
    let written = record();
    crate::nirvana_fresh::write_record(&home, &public, &written).unwrap();
    crate::nirvana_fresh::write_sent(&home, &public, written.at).unwrap();
    assert!(!app.nirvana_fresh_with(t0 + LOOKUPS[0], &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "");
}

/// The paste echoed after the last try: erased with one Backspace per character of `/clear`, then read once more.
#[tokio::test]
async fn fresh_server_erases_a_clear_whose_enter_never_went() {
    let guard = crate::nirvana::with_test_home("fresh-erase");
    let home = guard.path.clone();
    let mut app = test_app();
    let t0 = Instant::now();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    crate::nirvana_fresh::write_record(&home, &root_public(&app), &record()).unwrap();
    let pasted = t0 + LOOKUPS[0];
    assert!(app.nirvana_fresh_with(pasted, &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "/clear");
    line_now_reads(&app, "");
    for n in 0..ENTER_TRIES.len() {
        assert!(!app.nirvana_fresh_with(read_at(pasted, Step::Try(n)), &home, nirvana_in(&home)));
    }
    assert_eq!(app.fresh_deadline(), Some(pasted + ERASE_AT));
    line_now_reads(&app, "/clear");
    assert!(app.nirvana_fresh_with(pasted + ERASE_AT, &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), BACKSPACE.repeat(6));
    assert_eq!(app.fresh_deadline(), Some(pasted + ERASED_AT));
    line_now_reads(&app, "");
    assert!(!app.nirvana_fresh_with(pasted + ERASED_AT, &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "");
    // Never sent: no mark, and the record is tried again later in the stretch.
    assert_eq!(
        crate::nirvana_fresh::read_sent(&home, &root_public(&app)),
        None
    );
    assert!(app.fresh_deadline().is_some());
}

/// The work order: "a pane with a pending idle-compact paste gets no fresh start in the same pass, and the reverse."
#[tokio::test]
async fn fresh_start_and_idle_compact_never_share_a_pane() {
    let guard = crate::nirvana::with_test_home("fresh-race");
    let home = guard.path.clone();
    // A compact pasted in a pass holds the fresh start off in the same pass.
    let mut app = test_app();
    let t0 = Instant::now();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    let public = root_public(&app);
    crate::nirvana_fresh::write_record(&home, &public, &record()).unwrap();
    let pass = t0 + 51 * MIN;
    assert!(app.nirvana_idle_compact_with(pass, long_idle));
    assert_eq!(typed(&mut receiver), "/compact");
    assert!(!app.nirvana_fresh_with(pass, &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "");
    // The paste in flight is a fact of its own, not only the stretch's `sent` (review S3's guard, which the paste
    // also sets, would hide it): a pending paste with `sent` cleared still holds the fresh start off.
    let terminal_id = root_terminal_id(&app);
    let stretch = &mut app
        .state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .idle_stretch;
    assert!(stretch.pending.is_some());
    stretch.sent = false;
    stretch.fresh = FreshStretch::default();
    assert!(!app.nirvana_fresh_with(pass, &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "");
    // And the reverse: a `/clear` in flight holds the compact off.
    let mut app = test_app();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    let terminal_id = root_terminal_id(&app);
    app.state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .idle_stretch
        .fresh = FreshStretch {
        pending: Some(Pending {
            pane: public.clone(),
            pasted_at: pass,
            step: Step::Try(0),
        }),
        ..FreshStretch::default()
    };
    assert!(!app.nirvana_idle_compact_with(pass, long_idle));
    assert_eq!(typed(&mut receiver), "");
    // Stamped, so the compact is due again a RECHECK on, not at once.
    assert_eq!(
        app.idle_compact_deadline(),
        Some(pass + crate::nirvana_idle_compact::RECHECK)
    );
    // Without the `/clear` in flight the same pass compacts: the guard is what held it.
    app.state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .idle_stretch = IdleStretch {
        since: Some(t0),
        ..IdleStretch::default()
    };
    assert!(app.nirvana_idle_compact_with(pass, long_idle));
    assert_eq!(typed(&mut receiver), "/compact");
}

/// Review S3: once the `/clear` Enter went, idle compact stays off the pane for the stretch (it could read the old
/// transcript and type `/compact` into the new session); and once idle compact went, the fresh start does.
#[tokio::test]
async fn a_sent_clear_holds_idle_compact_off_and_a_sent_compact_holds_the_fresh_start_off() {
    let guard = crate::nirvana::with_test_home("fresh-sent-race");
    let home = guard.path.clone();
    let t0 = Instant::now();
    let pass = t0 + 51 * MIN;
    // The fresh start sent: the compact's pass types nothing, and is stamped.
    let mut app = test_app();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    let terminal_id = root_terminal_id(&app);
    app.state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .idle_stretch
        .fresh = FreshStretch {
        sent: true,
        ..FreshStretch::default()
    };
    assert!(!app.nirvana_idle_compact_with(pass, long_idle));
    assert_eq!(typed(&mut receiver), "");
    assert_eq!(
        app.idle_compact_deadline(),
        Some(pass + crate::nirvana_idle_compact::RECHECK)
    );
    // The compact went and its reads are done: a record now waits, and the fresh start types nothing.
    let mut app = test_app();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    let public = root_public(&app);
    let terminal_id = root_terminal_id(&app);
    app.state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .idle_stretch
        .sent = true;
    crate::nirvana_fresh::write_record(&home, &public, &record()).unwrap();
    for after in LOOKUPS {
        assert!(!app.nirvana_fresh_with(t0 + after, &home, nirvana_in(&home)));
    }
    assert_eq!(typed(&mut receiver), "");
    // In a new stretch, with nothing sent, the same record is typed for: the stretch's mark is what held it.
    app.state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .idle_stretch = IdleStretch {
        since: Some(t0),
        ..IdleStretch::default()
    };
    assert!(app.nirvana_fresh_with(t0 + LOOKUPS[0], &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "/clear");
}

/// Review S1: a Stop's record older than `FRESH_WITHIN` is removed with its mark, and nothing is typed.
#[tokio::test]
async fn fresh_server_forgets_a_stale_stop_record() {
    let guard = crate::nirvana::with_test_home("fresh-stale");
    let home = guard.path.clone();
    let mut app = test_app();
    let t0 = Instant::now();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    let public = root_public(&app);
    let stale = Record {
        at: crate::nirvana_run::now() - crate::nirvana_fresh::FRESH_WITHIN.as_secs() - 60,
        ..record()
    };
    crate::nirvana_fresh::write_record(&home, &public, &stale).unwrap();
    crate::nirvana_fresh::write_sent(&home, &public, stale.at - 1).unwrap();
    assert!(!app.nirvana_fresh_with(t0 + LOOKUPS[0], &home, nirvana_in(&home)));
    assert_eq!(typed(&mut receiver), "");
    assert_eq!(crate::nirvana_fresh::read_record(&home, &public), Ok(None));
    assert_eq!(crate::nirvana_fresh::read_sent(&home, &public), None);
}

/// Review S1: a Stop's record is typed for only while the pane is still in the conversation the record names.
#[tokio::test]
async fn fresh_server_types_only_into_the_records_conversation() {
    let guard = crate::nirvana::with_test_home("fresh-transcript");
    let home = guard.path.clone();
    let other = r"C:\Users\o\.claude\projects\C--work-print\def-456.jsonl";
    for (transcript, sends) in [
        (Some(other), false),
        (None, false),
        (Some(TRANSCRIPT), true),
    ] {
        let mut app = test_app();
        let t0 = Instant::now();
        let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
        crate::nirvana_fresh::write_record(&home, &root_public(&app), &record()).unwrap();
        assert_eq!(
            app.nirvana_fresh_with(t0 + LOOKUPS[0], &home, nirvana_with(&home, transcript)),
            sends,
            "{transcript:?}"
        );
        let expected = if sends { "/clear" } else { "" };
        assert_eq!(typed(&mut receiver), expected, "{transcript:?}");
    }
}

#[tokio::test]
async fn fresh_deadline_reaches_the_server_loop() {
    let guard = crate::nirvana::with_test_home("fresh-loop");
    let _home = guard.path.clone();
    let mut app = test_app();
    let t0 = Instant::now();
    let _receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    // Idle compact's own check is 50 minutes off; the fresh start's first lookup is the loop's next deadline.
    assert_eq!(
        app.next_headless_loop_deadline_with_git_refresh(t0, false, false),
        Some(t0 + LOOKUPS[0])
    );
}

/// Only a pane herdr started as Nirvana Claude is looked at: another Claude pane is never on the loop.
#[tokio::test]
async fn fresh_deadline_counts_only_panes_started_as_nirvana_claude() {
    let guard = crate::nirvana::with_test_home("fresh-name");
    let home = guard.path.clone();
    let mut app = test_app();
    let t0 = Instant::now();
    let mut receiver = idle_claude(&mut app, t0, RECORDED_EMPTY);
    crate::nirvana_fresh::write_record(&home, &root_public(&app), &record()).unwrap();
    for name in [None, Some("claude"), Some("nirvana-x")] {
        app.state
            .terminals
            .get_mut(&root_terminal_id(&app))
            .unwrap()
            .agent_name = name.map(str::to_owned);
        assert_eq!(app.fresh_deadline(), None, "{name:?}");
        assert!(!app.nirvana_fresh_with(t0 + LOOKUPS[0], &home, nirvana_in(&home)));
        assert_eq!(typed(&mut receiver), "", "{name:?}");
    }
    app.state
        .terminals
        .get_mut(&root_terminal_id(&app))
        .unwrap()
        .agent_name = Some("nirvana-2".into());
    assert_eq!(app.fresh_deadline(), Some(t0 + LOOKUPS[0]));
}
