//! Nirvana fork, plan 27: an idle Claude compacts before its prompt cache goes stale.
//!
//! The owner, 2026-10-23: "Ensusre idle agents compact when cache is about to get stale. This should be a setting
//! for claude code." Claude Code writes its prompt cache with a one-hour life: every non-zero cache write in the five
//! newest transcripts on this machine was `ephemeral_1h_input_tokens` (2026-10-23). A Claude left idle past the hour
//! writes its whole context again on the owner's next turn; `/compact` sent at 50 minutes still reads that context
//! from cache and leaves a small one.
//!
//! The rule is one pure function, `decide`, over the facts in `Facts`. The server (`app/nirvana_idle_compact.rs`)
//! gathers the facts it holds itself (state, idle time, the screen) and the ones outside herdr (`read_outside`: the
//! Claude process, its transcript), then types `/compact` only on `Decision::Send`. Any fact it cannot read stays
//! `None`, and every `None` is a reason not to send: this types into the owner's own panes.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::config::IdleCompactRule;
use crate::detect::{Agent, AgentState};

/// The life of Claude Code's prompt cache (measured above). Past it a compact no longer reads from cache, so the
/// stretch is not checked again.
pub const CACHE_LIFE: Duration = Duration::from_secs(60 * 60);
/// A pane checked and left alone is checked again this much later in the same stretch: the owner may clear a
/// half-typed line, or a background write may age past the limit.
pub const RECHECK: Duration = Duration::from_secs(60);
/// What is typed into the pane.
pub const COMMAND: &str = "/compact";

/// One unbroken idle stretch of a pane: when it began, when it was last checked, and whether `/compact` went in it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdleStretch {
    pub since: Option<Instant>,
    pub checked_at: Option<Instant>,
    pub sent: bool,
}

impl IdleStretch {
    /// The stretch after the pane's state is recomputed (plan 27 §2 rules 3 and 7). `Idle` keeps the stretch it is
    /// in, or starts one now; any other state ends it, so the next `Idle` starts afresh with nothing sent.
    pub fn after(self, state: AgentState, now: Instant) -> Self {
        if state != AgentState::Idle {
            return Self::default();
        }
        if self.since.is_some() {
            return self;
        }
        Self {
            since: Some(now),
            ..Self::default()
        }
    }

    /// When the stretch is next due a check, or `None` once `/compact` went or the cache has expired. A check that
    /// left the pane alone waits `RECHECK`, so a skipped pane never makes the server loop spin.
    pub fn next_check_at(self, minutes: Duration) -> Option<Instant> {
        let since = self.since?;
        if self.sent {
            return None;
        }
        let first = since + minutes;
        let at = self
            .checked_at
            .map_or(first, |checked| first.max(checked + RECHECK));
        (at < since + CACHE_LIFE).then_some(at)
    }
}

/// Everything `decide` reads about one pane. `None` means "could not be read".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The setting, `None` when it is off or its numbers are out of range.
    pub rule: Option<IdleCompactRule>,
    /// The agent herdr sees in the pane.
    pub agent: Option<Agent>,
    /// The pane is a `herdr seat exec` worker's.
    pub seat_worker: bool,
    /// The Claude process's argv.
    pub argv: Option<Vec<String>>,
    pub state: AgentState,
    /// How long the pane has been `Idle` without a break.
    pub idle_for: Option<Duration>,
    /// `/compact` already went in this idle stretch.
    pub sent_in_stretch: bool,
    /// How long ago the pane's one transcript was last written; `None` when it did not resolve to exactly one file.
    pub transcript_age: Option<Duration>,
    /// The last assistant turn's context (`context_tokens`).
    pub context_tokens: Option<u64>,
    /// The text on Claude Code's input line (`nirvana_run::prompt_line_text`): `Some("")` when it is empty.
    pub prompt_line: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Send,
    Skip(String),
}

/// Plan 27 §2: send `/compact` only when every rule holds. The first rule that fails is the reason.
pub fn decide(facts: &Facts) -> Decision {
    let skip = |reason: String| Decision::Skip(reason);
    // 1. The setting is on.
    let Some(rule) = facts.rule else {
        return skip("idle compact is off".into());
    };
    // 2. Claude, and not a worker.
    if facts.agent != Some(Agent::Claude) {
        return skip("the pane is not Claude".into());
    }
    if facts.seat_worker {
        return skip("the pane is a seat worker's".into());
    }
    let Some(argv) = facts.argv.as_deref() else {
        return skip("Claude's process could not be read".into());
    };
    if is_headless(argv) {
        return skip("Claude runs headless (-p)".into());
    }
    // 3. Idle without a break for at least `minutes`.
    if facts.state != AgentState::Idle {
        return skip(format!("the pane is {:?}, not idle", facts.state));
    }
    let minutes = rule.minutes.as_secs() / 60;
    if facts.idle_for.is_none_or(|idle| idle < rule.minutes) {
        return skip(format!("idle under {minutes} min"));
    }
    // 7. Once per idle stretch.
    if facts.sent_in_stretch {
        return skip("already compacted in this idle stretch".into());
    }
    // 4. One transcript, last written at least `minutes` ago.
    let Some(age) = facts.transcript_age else {
        return skip("the transcript did not resolve to one file".into());
    };
    if age < rule.minutes {
        return skip(format!(
            "the transcript was written under {minutes} min ago"
        ));
    }
    // 5. A context worth compacting.
    let Some(tokens) = facts.context_tokens else {
        return skip("the transcript holds no context reading".into());
    };
    if tokens < rule.min_tokens {
        return skip(format!("context {tokens} is under {}", rule.min_tokens));
    }
    // 6. Nothing on the input line.
    match facts.prompt_line.as_deref() {
        None => skip("no input line on the screen".into()),
        Some(text) if !text.is_empty() => skip("the input line holds text".into()),
        Some(_) => Decision::Send,
    }
}

/// A worker's Claude runs `-p`/`--print` and ends when done; it is never compacted.
pub fn is_headless(argv: &[String]) -> bool {
    argv.iter().any(|arg| arg == "-p" || arg == "--print")
}

/// The context a turn read: `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`. Unlike
/// `nirvana_subagents::parse_claude_usage` (a turn's whole traffic, for the agents panel), `output_tokens` is left
/// out: it is what the turn wrote, not what the next one re-reads. `None` when none of the three is there.
pub fn context_tokens(usage: &Value) -> Option<u64> {
    let fields = [
        "input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ];
    let read: Vec<u64> = fields
        .iter()
        .filter_map(|field| usage.get(field).and_then(Value::as_u64))
        .collect();
    (!read.is_empty()).then(|| read.iter().sum())
}

/// The context of the last assistant line in a transcript's text.
pub fn last_context_tokens(text: &str) -> Option<u64> {
    text.lines().rev().find_map(|line| {
        let value: Value = serde_json::from_str(line.trim()).ok()?;
        if value.get("type").and_then(Value::as_str) != Some("assistant") {
            return None;
        }
        Some(context_tokens(value.pointer("/message/usage")?))
    })?
}

/// The last assistant turn's context, read from the file's tail and widened up to 8 MB the way the agents panel
/// reads it (`nirvana_subagents::read_transcript_tail_with_expansion`).
pub fn read_context_tokens(path: &Path) -> Option<u64> {
    use crate::nirvana_subagents::{read_file_tail, INITIAL_TAIL_WINDOW, MAX_TAIL_WINDOW};
    let len = std::fs::metadata(path).ok()?.len();
    let mut window = INITIAL_TAIL_WINDOW;
    loop {
        let text = read_file_tail(path, window).ok()?;
        let found = last_context_tokens(&text);
        if found.is_some() || window >= MAX_TAIL_WINDOW || len <= window {
            return found;
        }
        window = (window * 4).min(MAX_TAIL_WINDOW);
    }
}

/// How long ago the file was last written. `None` when it cannot be read or its time is ahead of the clock.
pub fn file_age(path: &Path, now: SystemTime) -> Option<Duration> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    now.duration_since(modified).ok()
}

/// What the server knows about a Claude pane for the reads outside herdr.
#[derive(Debug, Clone, Default)]
pub struct PaneQuery {
    pub pane_id: String,
    pub shell_pid: Option<u32>,
    pub folder: Option<PathBuf>,
    /// The conversation id Claude's hooks reported to herdr.
    pub hook_session: Option<String>,
    /// The other Claude panes: id, folder, hook session. Two in one folder that nothing tells apart resolve to no
    /// transcript (`resolve_claude_session`), so neither is sent anything.
    pub others: Vec<(String, PathBuf, Option<String>)>,
}

/// The facts that live outside herdr.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outside {
    pub argv: Option<Vec<String>>,
    pub transcript_age: Option<Duration>,
    pub context_tokens: Option<u64>,
}

/// Reads the pane's Claude process, then its one transcript. Stops at the first read that fails, leaving the rest
/// `None`.
pub fn read_outside(
    query: &PaneQuery,
    claude_dir: Option<&Path>,
    home: Option<&Path>,
    now: SystemTime,
) -> Outside {
    let Some(job) = query.shell_pid.and_then(crate::detect::foreground_job) else {
        return Outside::default();
    };
    let Some((Agent::Claude, _, claude_pid)) = crate::detect::identify_agent_process_in_job(&job)
    else {
        return Outside::default();
    };
    let argv = job
        .processes
        .iter()
        .find(|process| process.pid == claude_pid)
        .and_then(|process| process.argv.clone());
    let mut outside = Outside {
        argv,
        ..Outside::default()
    };
    let (Some(claude_dir), Some(home), Some(folder)) = (claude_dir, home, query.folder.as_deref())
    else {
        return outside;
    };
    // The hook's id first; else the one Claude wrote for its own pid (`<config>/sessions/<pid>.json`).
    let session = query.hook_session.clone().or_else(|| {
        crate::nirvana::read_claude_session(claude_dir, claude_pid).map(|s| s.session_id)
    });
    let others: Vec<(&str, &Path, Option<&str>)> = query
        .others
        .iter()
        .map(|(id, folder, session)| (id.as_str(), folder.as_path(), session.as_deref()))
        .collect();
    let now_secs = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let Some(transcript) = crate::nirvana_subagents::resolve_claude_session(
        &query.pane_id,
        folder,
        session.as_deref(),
        crate::nirvana_subagents::process_creation_secs(claude_pid),
        &others,
        claude_dir,
        home,
        now_secs,
    ) else {
        return outside;
    };
    outside.transcript_age = file_age(&transcript, now);
    outside.context_tokens = read_context_tokens(&transcript);
    outside
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    fn rule() -> IdleCompactRule {
        IdleCompactRule {
            minutes: 50 * MIN,
            min_tokens: 50_000,
        }
    }

    /// Every rule holds: the one case that sends. Each test below breaks one fact of it.
    fn sending() -> Facts {
        Facts {
            rule: Some(rule()),
            agent: Some(Agent::Claude),
            seat_worker: false,
            argv: Some(vec!["claude".into(), "--model".into(), "opus".into()]),
            state: AgentState::Idle,
            idle_for: Some(51 * MIN),
            sent_in_stretch: false,
            transcript_age: Some(51 * MIN),
            context_tokens: Some(80_000),
            prompt_line: Some(String::new()),
        }
    }

    fn skipped(facts: &Facts, why: &str) {
        match decide(facts) {
            Decision::Skip(reason) => assert!(reason.contains(why), "{reason:?} lacks {why:?}"),
            Decision::Send => panic!("sent, expected a skip for {why:?}: {facts:?}"),
        }
    }

    #[test]
    fn idle_compact_sends_when_every_rule_holds() {
        assert_eq!(decide(&sending()), Decision::Send);
        // At exactly the limits it still sends: "at least" in §2 rules 3 to 5.
        let facts = Facts {
            idle_for: Some(50 * MIN),
            transcript_age: Some(50 * MIN),
            context_tokens: Some(50_000),
            ..sending()
        };
        assert_eq!(decide(&facts), Decision::Send);
    }

    #[test]
    fn idle_compact_rule1_setting_off_sends_nothing() {
        skipped(
            &Facts {
                rule: None,
                ..sending()
            },
            "off",
        );
    }

    #[test]
    fn idle_compact_rule2_only_claude() {
        for agent in [None, Some(Agent::Codex), Some(Agent::Antigravity)] {
            skipped(&Facts { agent, ..sending() }, "not Claude");
        }
    }

    #[test]
    fn idle_compact_rule2_seat_worker_pane_left_alone() {
        skipped(
            &Facts {
                seat_worker: true,
                ..sending()
            },
            "seat worker",
        );
    }

    #[test]
    fn idle_compact_rule2_unread_process_sends_nothing() {
        skipped(
            &Facts {
                argv: None,
                ..sending()
            },
            "could not be read",
        );
    }

    #[test]
    fn idle_compact_rule2_headless_claude_left_alone() {
        for flag in ["-p", "--print"] {
            let argv = vec!["claude".into(), flag.into(), "do it".into()];
            skipped(
                &Facts {
                    argv: Some(argv),
                    ..sending()
                },
                "headless",
            );
        }
    }

    #[test]
    fn idle_compact_rule3_only_an_idle_pane() {
        for state in [
            AgentState::Working,
            AgentState::Blocked,
            AgentState::Unknown,
        ] {
            skipped(&Facts { state, ..sending() }, "not idle");
        }
    }

    #[test]
    fn idle_compact_rule3_idle_under_minutes_waits() {
        for idle_for in [None, Some(49 * MIN)] {
            skipped(
                &Facts {
                    idle_for,
                    ..sending()
                },
                "idle under 50 min",
            );
        }
    }

    #[test]
    fn idle_compact_rule4_transcript_must_resolve() {
        skipped(
            &Facts {
                transcript_age: None,
                ..sending()
            },
            "one file",
        );
    }

    #[test]
    fn idle_compact_rule4_recent_transcript_waits() {
        skipped(
            &Facts {
                transcript_age: Some(10 * MIN),
                ..sending()
            },
            "written under 50 min",
        );
    }

    #[test]
    fn idle_compact_rule5_context_must_be_read() {
        skipped(
            &Facts {
                context_tokens: None,
                ..sending()
            },
            "no context",
        );
    }

    #[test]
    fn idle_compact_rule5_small_context_left_alone() {
        skipped(
            &Facts {
                context_tokens: Some(49_999),
                ..sending()
            },
            "under 50000",
        );
    }

    #[test]
    fn idle_compact_rule6_no_input_line_sends_nothing() {
        skipped(
            &Facts {
                prompt_line: None,
                ..sending()
            },
            "no input line",
        );
    }

    #[test]
    fn idle_compact_rule6_typed_text_left_alone() {
        skipped(
            &Facts {
                prompt_line: Some("half a thought".into()),
                ..sending()
            },
            "holds text",
        );
    }

    #[test]
    fn idle_compact_rule7_once_per_stretch() {
        skipped(
            &Facts {
                sent_in_stretch: true,
                ..sending()
            },
            "already compacted",
        );
    }

    #[test]
    fn idle_stretch_starts_on_idle_and_keeps_its_start() {
        let t0 = Instant::now();
        let stretch = IdleStretch::default().after(AgentState::Idle, t0);
        assert_eq!(stretch.since, Some(t0));
        // A later recompute that is still idle keeps the first instant, so the idle time is unbroken.
        let later = stretch.after(AgentState::Idle, t0 + 10 * MIN);
        assert_eq!(later.since, Some(t0));
    }

    #[test]
    fn idle_stretch_ends_on_any_other_state() {
        let t0 = Instant::now();
        let sent = IdleStretch {
            since: Some(t0),
            checked_at: Some(t0 + 50 * MIN),
            sent: true,
        };
        for state in [
            AgentState::Working,
            AgentState::Blocked,
            AgentState::Unknown,
        ] {
            assert_eq!(sent.after(state, t0 + 51 * MIN), IdleStretch::default());
        }
        // The next idle is a fresh stretch with nothing sent.
        let fresh = sent
            .after(AgentState::Working, t0 + 51 * MIN)
            .after(AgentState::Idle, t0 + 52 * MIN);
        assert_eq!(fresh.since, Some(t0 + 52 * MIN));
        assert!(!fresh.sent);
    }

    #[test]
    fn idle_stretch_is_due_at_minutes_then_rechecks_until_the_cache_expires() {
        let t0 = Instant::now();
        let stretch = IdleStretch {
            since: Some(t0),
            ..IdleStretch::default()
        };
        assert_eq!(stretch.next_check_at(50 * MIN), Some(t0 + 50 * MIN));
        // Checked and left alone: a minute later, not at once (no spinning).
        let checked = IdleStretch {
            checked_at: Some(t0 + 50 * MIN),
            ..stretch
        };
        assert_eq!(checked.next_check_at(50 * MIN), Some(t0 + 51 * MIN));
        // A check due at or past the cache's hour never comes.
        let late = IdleStretch {
            checked_at: Some(t0 + 59 * MIN),
            ..stretch
        };
        assert_eq!(late.next_check_at(50 * MIN), None);
        // Once sent, never again in the stretch.
        let sent = IdleStretch {
            sent: true,
            ..stretch
        };
        assert_eq!(sent.next_check_at(50 * MIN), None);
        // Not idle: never due.
        assert_eq!(IdleStretch::default().next_check_at(50 * MIN), None);
    }

    #[test]
    fn context_tokens_leave_out_output() {
        let usage = serde_json::json!({
            "input_tokens": 10,
            "cache_read_input_tokens": 60_000,
            "cache_creation_input_tokens": 500,
            "output_tokens": 9_000,
        });
        assert_eq!(context_tokens(&usage), Some(60_510));
        assert_eq!(
            context_tokens(&serde_json::json!({"output_tokens": 5})),
            None
        );
    }

    #[test]
    fn last_context_tokens_reads_the_last_assistant_line() {
        let text = concat!(
            r#"{"type":"assistant","message":{"usage":{"input_tokens":1,"cache_read_input_tokens":100}}}"#,
            "\n",
            r#"{"type":"assistant","message":{"usage":{"input_tokens":2,"cache_read_input_tokens":70000}}}"#,
            "\n",
            r#"{"type":"user","message":{"content":"next"}}"#,
            "\n",
            // A later line of another type with a usage is not a turn's context.
            r#"{"type":"summary","message":{"usage":{"input_tokens":5}}}"#,
            "\n",
        );
        assert_eq!(last_context_tokens(text), Some(70_002));
        assert_eq!(last_context_tokens(r#"{"type":"user","message":{}}"#), None);
    }

    #[test]
    fn transcript_facts_come_from_a_temporary_file() {
        let dir = std::env::temp_dir().join(format!(
            "nirvana-idle-compact-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.jsonl");
        std::fs::write(
            &path,
            "{\"type\":\"assistant\",\"message\":{\"usage\":{\"input_tokens\":3,\"cache_creation_input_tokens\":51000,\"output_tokens\":7}}}\n",
        )
        .unwrap();
        assert_eq!(read_context_tokens(&path), Some(51_003));
        let written = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(file_age(&path, written + 55 * MIN), Some(55 * MIN));
        // A file time ahead of the clock is not an age.
        assert_eq!(file_age(&path, written - MIN), None);
        assert_eq!(read_context_tokens(&dir.join("missing.jsonl")), None);
        // The last assistant line sits before 100 KB of other lines: the read widens past its first 64 KB.
        let long = dir.join("long.jsonl");
        let mut text = String::from(
            "{\"type\":\"assistant\",\"message\":{\"usage\":{\"input_tokens\":60000}}}\n",
        );
        let filler =
            "{\"type\":\"user\",\"message\":{\"content\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"}}\n";
        while text.len() < 100 * 1024 {
            text.push_str(filler);
        }
        std::fs::write(&long, text).unwrap();
        assert_eq!(read_context_tokens(&long), Some(60_000));
        let _ = std::fs::remove_dir_all(dir);
    }
}
