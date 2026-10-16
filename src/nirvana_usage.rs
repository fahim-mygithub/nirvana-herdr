//! Nirvana fork, feature 4: the usage windows of this machine's subscriptions, for the sidebar's "usage"
//! block.
//!
//! The owner, 2026-10-15: "Lets work on the next feature. This running server and usage from Nirvana lets
//! port it to the Nirvana herder" (nirvana/plans/2026-10-15-04-servers-and-usage.md).
//!
//! A subscription window belongs to the machine, not to a space, so the window reads it itself, on one
//! worker thread, and never through the server. Each provider is read the cheapest way that is free:
//! - Claude: `claude -p /usage`, which Claude Code answers locally. The live check on 2026-10-15 returned
//!   model "<synthetic>", 0 tokens and a cost of 0 in 2.7 s. Old Nirvana paid a model turn for this.
//! - Codex: the newest `rate_limits` in its rollout files; Codex writes them after each turn.
//! - Antigravity: `agy -p /usage`, which is free; one run took 20.8 s, so it is read every five minutes.
//! - Local: whether Ollama answers on 127.0.0.1:11434, and the GPU's memory.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

/// How often Claude is read while a Claude in this window is working, and while none is.
pub const CLAUDE_BUSY_EVERY: Duration = Duration::from_secs(120);
pub const CLAUDE_IDLE_EVERY: Duration = Duration::from_secs(600);
/// Codex is a file read; Antigravity's read is a 20 s run; Local is a socket and `nvidia-smi`.
pub const CODEX_EVERY: Duration = Duration::from_secs(30);
pub const AGY_EVERY: Duration = Duration::from_secs(300);
pub const LOCAL_EVERY: Duration = Duration::from_secs(30);
/// A read that has not finished by then is killed and the last figure kept.
const CLAUDE_TIMEOUT: Duration = Duration::from_secs(60);
const AGY_TIMEOUT: Duration = Duration::from_secs(90);
const SMI_TIMEOUT: Duration = Duration::from_secs(10);
/// Ollama's default address, as Nirvana's local seat uses it.
const OLLAMA: &str = "127.0.0.1:11434";
const OLLAMA_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Claude,
    Codex,
    Antigravity,
    Local,
}

impl Provider {
    pub const ALL: [Provider; 4] = [
        Provider::Claude,
        Provider::Codex,
        Provider::Antigravity,
        Provider::Local,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::Antigravity => "Antigravity",
            Self::Local => "Local",
        }
    }

    /// Old Nirvana's provider colours, its dark set (`ui/styles.css`). Local has no brand hue there and
    /// takes the theme's muted text instead (`None`).
    pub fn colour(self) -> Option<(u8, u8, u8)> {
        match self {
            Self::Claude => Some((0xD8, 0x88, 0x6A)),
            Self::Codex => Some((0x7B, 0xAE, 0xDD)),
            Self::Antigravity => Some((0xAB, 0x93, 0xD6)),
            Self::Local => None,
        }
    }
}

/// What a usage row shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Figure {
    /// Before the first read finished.
    Reading,
    /// Percent used in the five-hour and the seven-day window; `None` is a window the provider did not
    /// report, drawn "--", never 0%.
    Windows {
        five_hour: Option<u8>,
        seven_day: Option<u8>,
    },
    /// The GPU's memory while the local engine answers.
    Memory { used_mib: u64, total_mib: u64 },
    /// A state in a word: "not installed", "not exposed", "off", "on".
    Word(&'static str),
}

impl Figure {
    /// The bar's fill: the five-hour window, as old Nirvana's bar shows the shortest window, or the GPU
    /// memory in use.
    pub fn fill(&self) -> Option<f32> {
        match self {
            Self::Windows {
                five_hour: Some(used),
                ..
            } => Some(f32::from(*used) / 100.0),
            Self::Memory {
                used_mib,
                total_mib,
            } if *total_mib > 0 => Some((*used_mib as f32 / *total_mib as f32).clamp(0.0, 1.0)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRow {
    pub provider: Provider,
    pub figure: Figure,
}

fn percent(value: f64) -> u8 {
    value.round().clamp(0.0, 100.0) as u8
}

/// The percent after a label in Claude Code's `/usage` text: "Current session: 2% used · resets ...".
/// "<1%" reads as 0.
fn percent_after(text: &str, label: &str) -> Option<u8> {
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with(label))?;
    let rest = line.trim_start()[label.len()..].trim_start();
    let number = rest
        .trim_start_matches('<')
        .split('%')
        .next()?
        .trim()
        .parse::<f64>()
        .ok()?;
    Some(if rest.starts_with('<') {
        0
    } else {
        percent(number)
    })
}

/// Claude Code's `/usage` text: "Current session" is the five-hour window, "Current week (all models)"
/// the seven-day one. Text with neither (signed out, an error) is no reading.
pub fn claude_usage(text: &str) -> Option<Figure> {
    let five_hour = percent_after(text, "Current session:");
    let seven_day = percent_after(text, "Current week (all models):");
    (five_hour.is_some() || seven_day.is_some()).then_some(Figure::Windows {
        five_hour,
        seven_day,
    })
}

/// agy's `/usage` stream: the `command_result` event's groups and buckets. The Gemini group's
/// `gemini-5h` and `gemini-weekly` buckets are the windows, used = 1 - `remaining_fraction`. Another
/// `gemini-` bucket with that window stands in when the named one is missing.
pub fn agy_usage(stream: &str) -> Option<Figure> {
    let data = stream.lines().find_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        let command = value.get("command")?;
        (value.get("event")?.as_str()? == "command_result"
            && command.get("name")?.as_str()? == "usage")
            .then(|| command.get("data").cloned())
            .flatten()
    })?;
    let buckets = data
        .get("groups")?
        .as_array()?
        .iter()
        .filter_map(|group| group.get("buckets")?.as_array())
        .flatten()
        .collect::<Vec<_>>();
    let used = |exact: &str, window: &str| {
        let field = |bucket: &serde_json::Value, key: &str| {
            bucket
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let bucket = buckets
            .iter()
            .find(|bucket| field(bucket, "id").as_deref() == Some(exact))
            .or_else(|| {
                buckets.iter().find(|bucket| {
                    field(bucket, "id").is_some_and(|id| id.starts_with("gemini-"))
                        && field(bucket, "window").as_deref() == Some(window)
                })
            })?;
        let remaining = bucket.get("remaining_fraction")?.as_f64()?;
        Some(percent((1.0 - remaining.clamp(0.0, 1.0)) * 100.0))
    };
    let five_hour = used("gemini-5h", "5h");
    let seven_day = used("gemini-weekly", "weekly");
    (five_hour.is_some() || seven_day.is_some()).then_some(Figure::Windows {
        five_hour,
        seven_day,
    })
}

/// One Codex rollout line's `rate_limits`, from a `token_count` event. `written_at` is when the line
/// was written (the file's modified time) in Unix seconds, for windows that give `resets_in_seconds`.
/// A window whose reset has passed reads 0%; when every window has reset, the reading is too old to say
/// anything and is no reading.
pub fn codex_rate_limits(line: &str, written_at: u64, now: u64) -> Option<Figure> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let limits = value
        .get("payload")
        .and_then(|payload| payload.get("rate_limits"))
        .or_else(|| value.get("rate_limits"))
        .filter(|limits| limits.is_object())?;
    let mut five_hour = None;
    let mut seven_day = None;
    let mut any_live = false;
    for key in ["primary", "secondary"] {
        let Some(window) = limits.get(key).filter(|window| window.is_object()) else {
            continue;
        };
        let Some(used) = window
            .get("used_percent")
            .and_then(serde_json::Value::as_f64)
        else {
            continue;
        };
        let resets_at = window
            .get("resets_at")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| {
                window
                    .get("resets_in_seconds")
                    .and_then(serde_json::Value::as_u64)
                    .map(|seconds| written_at.saturating_add(seconds))
            });
        let live = resets_at.is_none_or(|at| at > now);
        any_live |= live;
        let figure = if live { percent(used) } else { 0 };
        // Codex names its windows by length: 299 or 300 minutes, and 10079 or 10080.
        let minutes = window
            .get("window_minutes")
            .and_then(serde_json::Value::as_u64);
        match (key, minutes) {
            (_, Some(minutes)) if minutes <= 360 => five_hour = Some(figure),
            (_, Some(minutes)) if minutes >= 1440 => seven_day = Some(figure),
            ("primary", None) => five_hour = Some(figure),
            ("secondary", None) => seven_day = Some(figure),
            _ => {}
        }
    }
    (any_live && (five_hour.is_some() || seven_day.is_some())).then_some(Figure::Windows {
        five_hour,
        seven_day,
    })
}

/// The GPU memory line of `nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader,nounits`
/// ("3539, 32607", in MiB), the first GPU.
pub fn gpu_memory(text: &str) -> Option<Figure> {
    let mut fields = text.lines().next()?.split(',').map(str::trim);
    let used_mib = fields.next()?.parse().ok()?;
    let total_mib = fields.next()?.parse().ok()?;
    Some(Figure::Memory {
        used_mib,
        total_mib,
    })
}

/// Whether a program is on PATH, the way a shell would find it (`.exe`, `.cmd` on Windows).
fn on_path(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let names = if cfg!(windows) {
        vec![
            format!("{program}.exe"),
            format!("{program}.cmd"),
            program.to_owned(),
        ]
    } else {
        vec![program.to_owned()]
    };
    std::env::split_paths(&path).any(|dir| names.iter().any(|name| dir.join(name).is_file()))
}

/// Runs a program without a window and returns its stdout, or `None` when it is missing, fails or
/// outlives `timeout` (then it is killed).
fn run(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::platform::configure_background_command(&mut command);
    let mut child = command.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    reader.join().ok()
}

fn read_claude() -> Option<Figure> {
    if !on_path("claude") {
        return Some(Figure::Word("not installed"));
    }
    let text = run(
        "claude",
        &[
            "-p",
            "/usage",
            "--no-session-persistence",
            "--output-format",
            "text",
        ],
        CLAUDE_TIMEOUT,
    )?;
    claude_usage(&text)
}

fn read_codex() -> Option<Figure> {
    let installed = on_path("codex");
    let now = unix(SystemTime::now());
    let figure = crate::nirvana_spaces::codex_home()
        .map(|home| crate::nirvana_spaces::recent_codex_rollouts(&home.join("sessions")))
        .into_iter()
        .flatten()
        .find_map(|path| codex_rollout_figure(&path, now));
    Some(match figure {
        Some(figure) => figure,
        None if installed => Figure::Word("not exposed"),
        None => Figure::Word("not installed"),
    })
}

fn codex_rollout_figure(path: &Path, now: u64) -> Option<Figure> {
    let written_at = unix(path.metadata().ok()?.modified().ok()?);
    let text = crate::nirvana_spaces::read_tail(path)?;
    text.lines()
        .rev()
        .filter(|line| line.contains("\"rate_limits\""))
        .find_map(|line| codex_rate_limits(line, written_at, now))
}

fn read_agy() -> Option<Figure> {
    if !on_path("agy") {
        return Some(Figure::Word("not installed"));
    }
    let stream = run(
        "agy",
        &["-p", "/usage", "--output-format", "stream-json"],
        AGY_TIMEOUT,
    )?;
    agy_usage(&stream)
}

fn ollama_answers() -> bool {
    let Ok(address) = OLLAMA.parse() else {
        return false;
    };
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&address, OLLAMA_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(OLLAMA_TIMEOUT));
    let _ = stream.set_write_timeout(Some(OLLAMA_TIMEOUT));
    if stream
        .write_all(b"GET /api/version HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut head = [0u8; 16];
    let read = stream.read(&mut head).unwrap_or(0);
    head[..read].starts_with(b"HTTP/1.") && head[..read].windows(3).any(|w| w == b"200")
}

fn read_local() -> Option<Figure> {
    if !ollama_answers() {
        return Some(Figure::Word("off"));
    }
    Some(
        run(
            "nvidia-smi",
            &[
                "--query-gpu=memory.used,memory.total",
                "--format=csv,noheader,nounits",
            ],
            SMI_TIMEOUT,
        )
        .and_then(|text| gpu_memory(&text))
        .unwrap_or(Figure::Word("on")),
    )
}

fn unix(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

static CHANGED: AtomicBool = AtomicBool::new(false);
static CLAUDE_WORKING: AtomicBool = AtomicBool::new(false);

fn rows_state() -> &'static Mutex<Vec<UsageRow>> {
    static ROWS: OnceLock<Mutex<Vec<UsageRow>>> = OnceLock::new();
    ROWS.get_or_init(|| {
        Mutex::new(
            Provider::ALL
                .into_iter()
                .map(|provider| UsageRow {
                    provider,
                    figure: Figure::Reading,
                })
                .collect(),
        )
    })
}

fn store(provider: Provider, figure: Option<Figure>) {
    let mut rows = rows_state().lock().unwrap_or_else(|e| e.into_inner());
    let Some(row) = rows.iter_mut().find(|row| row.provider == provider) else {
        return;
    };
    // A failed read keeps the last figure; a first read that fails says the figure is not exposed.
    let figure = match (figure, &row.figure) {
        (Some(figure), _) => figure,
        (None, Figure::Reading) => Figure::Word("not exposed"),
        (None, _) => return,
    };
    if row.figure != figure {
        row.figure = figure;
        CHANGED.store(true, Ordering::Release);
    }
}

fn start_worker() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        // Each provider on its own thread: Antigravity's 20 s read must not hold up Claude's.
        let every = |provider: Provider| match provider {
            Provider::Claude => {
                if CLAUDE_WORKING.load(Ordering::Acquire) {
                    CLAUDE_BUSY_EVERY
                } else {
                    CLAUDE_IDLE_EVERY
                }
            }
            Provider::Codex => CODEX_EVERY,
            Provider::Antigravity => AGY_EVERY,
            Provider::Local => LOCAL_EVERY,
        };
        for provider in Provider::ALL {
            let _ = crate::thread_spawn::spawn_named("nirvana-usage", move || loop {
                let started = Instant::now();
                let figure = match provider {
                    Provider::Claude => read_claude(),
                    Provider::Codex => read_codex(),
                    Provider::Antigravity => read_agy(),
                    Provider::Local => read_local(),
                };
                store(provider, figure);
                // Sleep in short steps so a Claude that starts working shortens the wait.
                while started.elapsed() < every(provider) {
                    std::thread::sleep(Duration::from_secs(1));
                }
            });
        }
    });
}

/// The four rows as last read. The first call starts the readers; under test they never start (they
/// would run this machine's `claude` and `agy`), and a test sets the rows it draws.
pub fn rows() -> Vec<UsageRow> {
    #[cfg(test)]
    if let Some(rows) = TEST_ROWS.with_borrow(Clone::clone) {
        return rows;
    }
    if !cfg!(test) {
        start_worker();
    }
    rows_state()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

#[cfg(test)]
thread_local! {
    static TEST_ROWS: std::cell::RefCell<Option<Vec<UsageRow>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_rows_for_test(rows: Option<Vec<UsageRow>>) {
    TEST_ROWS.with_borrow_mut(|slot| *slot = rows);
}

/// Whether a figure changed since the last call: the window repaints on its next timer tick.
pub fn take_changed() -> bool {
    CHANGED.swap(false, Ordering::AcqRel)
}

/// Told by the window as it draws: a Claude in it is working, so Claude's window moves and is read
/// more often.
pub fn note_claude_working(working: bool) {
    CLAUDE_WORKING.store(working, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Claude Code 2.1.301's `/usage` answer, captured 2026-10-15 (the contributions block shortened).
    const CLAUDE_TEXT: &str = "You are currently using your subscription to power your Claude Code usage\n\nCurrent session: 2% used \u{b7} resets Oct 16, 2:30am (America/New_York)\nCurrent week (all models): 70% used \u{b7} resets Oct 16, 12pm (America/New_York)\nCurrent week (Fable): 83% used \u{b7} resets Oct 16, 12pm (America/New_York)\n\nWhat's contributing to your limits usage?\n  98% of your usage came from subagent-heavy sessions\n";

    #[test]
    fn claude_reads_the_session_and_the_all_models_week() {
        assert_eq!(
            claude_usage(CLAUDE_TEXT),
            Some(Figure::Windows {
                five_hour: Some(2),
                seven_day: Some(70)
            })
        );
        assert_eq!(
            claude_usage("Current session: <1% used\n"),
            Some(Figure::Windows {
                five_hour: Some(0),
                seven_day: None
            })
        );
        assert_eq!(
            claude_usage("Invalid API key \u{b7} Please run /login"),
            None
        );
    }

    /// agy's `/usage` event, captured 2026-10-15 (descriptions cut).
    const AGY_STREAM: &str = r#"{"event":"init"}
{"event":"command_result","command":{"name":"usage","data":{"groups":[{"name":"Gemini Models","buckets":[{"id":"gemini-weekly","window":"weekly","remaining_fraction":0.75,"reset_time":"2026-10-23T01:50:52Z"},{"id":"gemini-5h","window":"5h","remaining_fraction":1,"reset_time":"2026-10-16T06:50:52Z"}]},{"name":"Claude and GPT models","buckets":[{"id":"3p-weekly","window":"weekly","remaining_fraction":0.1},{"id":"3p-5h","window":"5h","remaining_fraction":0.2}]},{"name":"Gemini 4 Argon","buckets":[{"id":"gemini-argon-wtu-weekly","window":"weekly","remaining_fraction":0.5}]}]}}}
{"event":"result","result":{"status":"SUCCESS"}}"#;

    #[test]
    fn antigravity_reads_the_gemini_group() {
        assert_eq!(
            agy_usage(AGY_STREAM),
            Some(Figure::Windows {
                five_hour: Some(0),
                seven_day: Some(25)
            })
        );
        let without_named = AGY_STREAM.replace("\"gemini-weekly\"", "\"other\"");
        assert_eq!(
            agy_usage(&without_named),
            Some(Figure::Windows {
                five_hour: Some(0),
                seven_day: Some(50)
            })
        );
        assert_eq!(agy_usage("{\"event\":\"result\"}"), None);
    }

    /// Codex 0.42's `token_count` event, from a rollout on this machine (2025-09-27).
    const CODEX_LINE: &str = r#"{"timestamp":"2025-09-27T09:20:00Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"primary":{"used_percent":5.0,"window_minutes":299,"resets_in_seconds":7549},"secondary":{"used_percent":2.0,"window_minutes":10079,"resets_in_seconds":594349}}}}"#;

    #[test]
    fn codex_reads_its_windows_and_drops_reset_ones() {
        let written = 1_000_000;
        assert_eq!(
            codex_rate_limits(CODEX_LINE, written, written + 60),
            Some(Figure::Windows {
                five_hour: Some(5),
                seven_day: Some(2)
            })
        );
        // Past the five-hour reset, inside the week.
        assert_eq!(
            codex_rate_limits(CODEX_LINE, written, written + 7549 + 1),
            Some(Figure::Windows {
                five_hour: Some(0),
                seven_day: Some(2)
            })
        );
        // Both reset: a reading that old says nothing.
        assert_eq!(
            codex_rate_limits(CODEX_LINE, written, written + 594349 + 1),
            None
        );
        let absolute = r#"{"payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":40.4,"window_minutes":300,"resets_at":2000},"secondary":null}}}"#;
        assert_eq!(
            codex_rate_limits(absolute, 0, 1000),
            Some(Figure::Windows {
                five_hour: Some(40),
                seven_day: None
            })
        );
    }

    #[test]
    fn the_gpu_line_is_used_and_total_memory() {
        let figure = gpu_memory("3539, 32607\n").unwrap();
        assert_eq!(
            figure,
            Figure::Memory {
                used_mib: 3539,
                total_mib: 32607
            }
        );
        assert!((figure.fill().unwrap() - 0.1085).abs() < 0.001);
        assert_eq!(gpu_memory("NVIDIA-SMI has failed"), None);
    }

    #[test]
    fn a_failed_read_keeps_the_last_figure() {
        let read = Figure::Windows {
            five_hour: Some(3),
            seven_day: Some(40),
        };
        store(Provider::Claude, Some(read.clone()));
        assert!(take_changed());
        store(Provider::Claude, None);
        assert!(!take_changed());
        // A first read that fails says so instead of reading forever.
        store(Provider::Codex, None);
        let rows = rows();
        assert_eq!(rows[0].figure, read);
        assert_eq!(rows[1].figure, Figure::Word("not exposed"));
    }

    #[test]
    fn the_bar_follows_the_five_hour_window() {
        let figure = Figure::Windows {
            five_hour: Some(40),
            seven_day: Some(90),
        };
        assert_eq!(figure.fill(), Some(0.4));
        assert_eq!(Figure::Word("off").fill(), None);
        assert_eq!(Figure::Reading.fill(), None);
    }
}
