//! Nirvana fork, feature 4: the usage windows of this machine's subscriptions, for the sidebar's "usage"
//! block. Plan 12's O1 grows it into readings: every pool and window with its reset, its freshness and its
//! pace, kept in a cache file that `herdr usage` prints for the plugin, the statusline and the seat engine.
//!
//! The owner, 2026-10-15: "Lets work on the next feature. This running server and usage from Nirvana lets
//! port it to the Nirvana herder" (nirvana/plans/2026-10-15-04-servers-and-usage.md). The owner, 2026-10-17:
//! "have a orchistrator agent that is aware of the 5h and 7s usage limits" (plan 12 L2, §3).
//!
//! A subscription window belongs to the machine, not to a space, so the window reads it itself, on one
//! worker thread per reader, and never through the server. Each provider is read the cheapest way that is
//! free:
//! - Claude: `claude -p /usage`, which Claude Code answers locally. The live check on 2026-10-15 returned
//!   model "<synthetic>", 0 tokens and a cost of 0 in 2.7 s. Old Nirvana paid a model turn for this.
//! - Codex: the newest `rate_limits` in its rollout files; Codex writes them after each turn.
//! - Antigravity: `agy -p /usage`, which is free; one run took 20.8 s on 2026-10-15, 2.0 s on 2026-10-17.
//! - Local: whether Ollama answers on 127.0.0.1:11434, and the GPU's memory.
//!
//! Both CLI readers run with a list argv, never through a shell: Git Bash rewrites `/usage` into a path, which
//! cost a paid model turn twice on 2026-10-17 (plan 12 §3).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

/// How often Claude is read while a Claude in this window is working, and while none is. Both sit under
/// `STALE_AFTER`, so an open window keeps the cache fresh and `herdr usage` need not read again. The idle
/// figure was 600 s before plan 12; a read is 0 tokens and took 2.8 s on 2026-10-17.
pub const CLAUDE_BUSY_EVERY: Duration = Duration::from_secs(120);
pub const CLAUDE_IDLE_EVERY: Duration = Duration::from_secs(240);
/// Codex is a file read; Antigravity's read is a CLI run (300 s before plan 12, now under `STALE_AFTER` as
/// Claude's); Local is a socket and `nvidia-smi`.
pub const CODEX_EVERY: Duration = Duration::from_secs(30);
pub const AGY_EVERY: Duration = Duration::from_secs(240);
pub const LOCAL_EVERY: Duration = Duration::from_secs(30);
/// A read that has not finished by then is killed and the last figure kept.
const CLAUDE_TIMEOUT: Duration = Duration::from_secs(60);
const AGY_TIMEOUT: Duration = Duration::from_secs(90);
const SMI_TIMEOUT: Duration = Duration::from_secs(10);
/// Ollama's default address, as Nirvana's local seat uses it.
const OLLAMA: &str = "127.0.0.1:11434";
const OLLAMA_TIMEOUT: Duration = Duration::from_millis(500);

/// Plan 12 §3: a reading older than this, in seconds, is stale. Old Nirvana's seating line.
pub const STALE_AFTER: u64 = 300;
pub const FIVE_HOURS: u64 = 5 * 3600;
pub const WEEK: u64 = 7 * 86_400;
/// Plan 12 §5.3: `pace = clamp((used + PACE_EASE) / (elapsed + PACE_EASE), PACE_MIN, PACE_MAX)`. The ease keeps
/// the first minutes after a reset from reading as 0 or as 3x ahead (Argon's review, §4.2). All three are
/// hypotheses, unmeasured.
pub const PACE_EASE: f32 = 0.10;
pub const PACE_MIN: f32 = 0.5;
pub const PACE_MAX: f32 = 2.0;
/// The foot marks a window ahead of pace above this: at that rate it runs out before its reset. A hypothesis.
pub const AHEAD_OF_PACE: f32 = 1.0;

/// The pools (plan 12 §3). A pool is one quota that several seats spend from.
pub const MAX: &str = "max";
pub const GEMINI: &str = "gemini";
pub const ARGON: &str = "argon";
pub const GOOGLE3P: &str = "google3p";
pub const CODEX: &str = "codex";
pub const LOCAL: &str = "local";
/// The foot's rows and the CLI's, in this order; a pool agy reports that is none of these follows them.
pub const POOLS: [&str; 6] = [MAX, GEMINI, ARGON, GOOGLE3P, CODEX, LOCAL];

/// The cache, `<home>/nirvana/usage.json`.
pub const CACHE_DIR: &str = "nirvana";
pub const CACHE_FILE: &str = "usage.json";

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

    /// The cache file's key and the JSON's `provider`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Antigravity => "antigravity",
            Self::Local => "local",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Claude => 0,
            Self::Codex => 1,
            Self::Antigravity => 2,
            Self::Local => 3,
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

/// The reader of a pool. Every pool none of `POOLS` names comes from an agy group.
pub fn provider_of(pool: &str) -> Provider {
    match pool {
        MAX => Provider::Claude,
        CODEX => Provider::Codex,
        LOCAL => Provider::Local,
        _ => Provider::Antigravity,
    }
}

/// A pool's name in the foot, which has 22 cells at herdr's default width beside a figure of 8. Google's
/// Claude and GPT pool is "G·Claude": 8 cells, and the G mirrors plan 12's `g-opus` and `g-sonnet` seats.
pub fn label(pool: &str) -> &str {
    match pool {
        MAX => "Claude",
        GEMINI => "Gemini",
        ARGON => "Argon",
        GOOGLE3P => "G\u{b7}Claude",
        CODEX => "Codex",
        LOCAL => "Local",
        other => other,
    }
}

/// The windows a known pool has. After a good read, one of these that the reader did not print is unknown,
/// never 0 (plan 12 §3); a window not listed, like Argon's 5 h, is absent.
fn expected_windows(pool: &str) -> &'static [&'static str] {
    match pool {
        MAX | GEMINI | GOOGLE3P | CODEX => &["5h", "week"],
        ARGON => &["week"],
        _ => &[],
    }
}

/// One window of one pool, as a reader saw it. Written to the cache file as is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub pool: String,
    /// `5h`, `week`, or a week scoped to some models, `week.fable`: a dotted window binds only those.
    pub window: String,
    /// The share used, 0 to 1. `None` is a window the pool has that the reader did not expose: unknown.
    pub used: Option<f32>,
    /// Unix seconds.
    pub resets_at: Option<u64>,
    /// The reset as the reader printed it, when that is not a timestamp: Claude's
    /// "Oct 17, 8:50pm (America/New_York)".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_text: Option<String>,
    /// Unix seconds.
    pub read_at: u64,
    /// The reader, and agy's bucket id: "claude /usage", "agy gemini-5h", "codex rollout".
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    /// Read more than `STALE_AFTER` ago.
    Old,
    /// Read before a reset that has since passed: the figure says nothing about the new window. Old
    /// Nirvana's "stuck at zero for claude" came from reading such a figure as live.
    ResetPassed,
}

impl Freshness {
    pub fn word(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Old => "old",
            Self::ResetPassed => "reset passed",
        }
    }
}

/// Plan 12 §3. A reading taken after its reset is fresh, so an idle account whose window has not started
/// again does not loop on re-reads (Argon's review, §4.6).
pub fn freshness(reading: &Reading, now: u64) -> Freshness {
    if let Some(at) = reading.resets_at {
        if at <= now && reading.read_at < at {
            return Freshness::ResetPassed;
        }
    }
    if now.saturating_sub(reading.read_at) > STALE_AFTER {
        Freshness::Old
    } else {
        Freshness::Fresh
    }
}

/// A window's length in seconds: `5h`, and every `week`, scoped or not.
pub fn window_length(window: &str) -> Option<u64> {
    match window.split('.').next() {
        Some("5h") => Some(FIVE_HOURS),
        Some("week") => Some(WEEK),
        _ => None,
    }
}

/// The share of the window gone when the reading was taken.
pub fn elapsed(reading: &Reading) -> Option<f32> {
    let length = window_length(&reading.window)?;
    let left = reading
        .resets_at?
        .saturating_sub(reading.read_at)
        .min(length);
    Some(1.0 - left as f32 / length as f32)
}

/// Plan 12 §5.3: above 1 the window is spent faster than it passes.
pub fn pace(reading: &Reading) -> Option<f32> {
    let used = reading.used?;
    let elapsed = elapsed(reading)?;
    Some(((used + PACE_EASE) / (elapsed + PACE_EASE)).clamp(PACE_MIN, PACE_MAX))
}

/// The window that binds the pool: of the windows every seat of the pool spends (no dot: `week.fable` binds
/// only Fable), the most used; an unknown one only when no window is known; a tie goes to the first listed,
/// which every reader makes 5 h before the week.
pub fn tightest(readings: &[Reading]) -> Option<&Reading> {
    readings
        .iter()
        .filter(|reading| !reading.window.contains('.'))
        .fold(None, |best: Option<&Reading>, reading| match best {
            Some(best) if reading.used.unwrap_or(-1.0) <= best.used.unwrap_or(-1.0) => Some(best),
            _ => Some(reading),
        })
}

/// "Gemini 4 Argon" to "gemini-4-argon": a pool named from an agy group that no prefix maps.
fn slug(text: &str) -> String {
    let mut slug = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').to_owned()
}

/// An agy bucket's pool. Argon's bucket id starts "gemini-" as well, so its prefix is checked first; an
/// unknown bucket goes to a pool named from its group, never dropped, so a new pool shows with no code change.
pub fn agy_pool(bucket: &str, group: &str) -> String {
    if bucket.starts_with("gemini-argon-") {
        ARGON.to_owned()
    } else if bucket.starts_with("gemini-") {
        GEMINI.to_owned()
    } else if bucket.starts_with("3p-") {
        GOOGLE3P.to_owned()
    } else {
        Some(slug(group))
            .filter(|slug| !slug.is_empty())
            .unwrap_or_else(|| "agy".to_owned())
    }
}

fn agy_window(window: Option<&str>, bucket: &str) -> String {
    match window {
        Some("5h") => "5h".to_owned(),
        Some("weekly") => "week".to_owned(),
        Some(other) if !other.is_empty() => other.to_owned(),
        _ if bucket.ends_with("-5h") => "5h".to_owned(),
        _ if bucket.ends_with("-weekly") => "week".to_owned(),
        _ => bucket.to_owned(),
    }
}

/// "2026-10-24T15:48:02Z", agy's `reset_time`, with an optional fraction and offset, in Unix seconds.
pub(crate) fn rfc3339(text: &str) -> Option<u64> {
    let text = text.trim();
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let date = time::Date::from_calendar_date(
        i32::try_from(number(0..4)?).ok()?,
        time::Month::try_from(u8::try_from(number(5..7)?).ok()?).ok()?,
        u8::try_from(number(8..10)?).ok()?,
    )
    .ok()?;
    let clock = time::Time::from_hms(
        u8::try_from(number(11..13)?).ok()?,
        u8::try_from(number(14..16)?).ok()?,
        u8::try_from(number(17..19)?).ok()?,
    )
    .ok()?;
    let rest = text.get(19..)?;
    let rest = rest.strip_prefix('.').map_or(rest, |fraction| {
        fraction.trim_start_matches(|c: char| c.is_ascii_digit())
    });
    let offset = match rest {
        "Z" | "z" | "" => 0,
        zone => {
            let sign = match zone.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let (hours, minutes) = zone[1..].split_once(':')?;
            sign * (hours.parse::<i64>().ok()? * 3600 + minutes.parse::<i64>().ok()? * 60)
        }
    };
    let at = time::PrimitiveDateTime::new(date, clock)
        .assume_utc()
        .unix_timestamp()
        - offset;
    u64::try_from(at).ok()
}

/// agy's `/usage` stream: the `command_result` event's groups and buckets, taken as they come (plan 12 §3).
/// used = 1 - `remaining_fraction`: agy prints what is left, Claude what is used, and reading one as the other
/// drew 1% left on a full account. Two buckets landing on one window keep the tighter, which binds.
pub fn agy_readings(stream: &str, read_at: u64) -> Option<Vec<Reading>> {
    let data = stream.lines().find_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        let command = value.get("command")?;
        (value.get("event")?.as_str()? == "command_result"
            && command.get("name")?.as_str()? == "usage")
            .then(|| command.get("data").cloned())
            .flatten()
    })?;
    let text = |value: &serde_json::Value, key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let mut readings: Vec<Reading> = Vec::new();
    for group in data.get("groups")?.as_array()? {
        let group_name = text(group, "name").unwrap_or_default();
        for bucket in group
            .get("buckets")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(id) = text(bucket, "id") else {
                continue;
            };
            let reading = Reading {
                pool: agy_pool(&id, &group_name),
                window: agy_window(text(bucket, "window").as_deref(), &id),
                used: bucket
                    .get("remaining_fraction")
                    .and_then(serde_json::Value::as_f64)
                    .map(|remaining| (1.0 - remaining.clamp(0.0, 1.0)) as f32),
                resets_at: text(bucket, "reset_time").and_then(|at| rfc3339(&at)),
                resets_text: None,
                read_at,
                source: format!("agy {id}"),
            };
            match readings
                .iter_mut()
                .find(|known| known.pool == reading.pool && known.window == reading.window)
            {
                Some(known) if known.used.unwrap_or(-1.0) >= reading.used.unwrap_or(-1.0) => {}
                Some(known) => *known = reading,
                None => readings.push(reading),
            }
        }
    }
    if readings.is_empty() {
        return None;
    }
    for pool in [GEMINI, GOOGLE3P, ARGON] {
        add_unknown(&mut readings, pool, read_at, "agy /usage");
    }
    // agy lists the week first; 5 h first, as Claude prints them, so a tie in `tightest` goes the same way.
    readings.sort_by_key(|reading| window_rank(&reading.window));
    Some(readings)
}

/// 5 h, the week, a scoped week, anything else; a stable sort keeps each pool's order within a rank.
fn window_rank(window: &str) -> u8 {
    match window {
        "5h" => 0,
        "week" => 1,
        scoped if scoped.starts_with("week.") => 2,
        _ => 3,
    }
}

/// A window the pool has that the read did not print: unknown, never 0.
fn add_unknown(readings: &mut Vec<Reading>, pool: &str, read_at: u64, source: &str) {
    for window in expected_windows(pool) {
        if !readings
            .iter()
            .any(|reading| reading.pool == pool && reading.window == *window)
        {
            readings.push(Reading {
                pool: pool.to_owned(),
                window: (*window).to_owned(),
                used: None,
                resets_at: None,
                resets_text: None,
                read_at,
                source: source.to_owned(),
            });
        }
    }
}

/// "35% used · resets ...": the share used. "<1%" reads as 0.
fn claude_used(rest: &str) -> Option<f32> {
    let rest = rest.trim_start();
    if rest.starts_with('<') {
        return Some(0.0);
    }
    let number = rest.split('%').next()?.trim().parse::<f32>().ok()?;
    Some((number / 100.0).clamp(0.0, 1.0))
}

/// Claude Code's `/usage` text. "Current session" is the 5 h window, "Current week (all models)" the week,
/// and any other "Current week (X)" a week scoped to X, `week.x`: today "(Fable)", which spends the all-models
/// week with a 50% allowance (P6). Text with neither of the first two (signed out, an error) is no reading.
pub fn claude_readings(text: &str, read_at: u64) -> Option<Vec<Reading>> {
    let mut readings: Vec<Reading> = Vec::new();
    for line in text.lines().map(str::trim) {
        let (window, rest) = if let Some(rest) = line.strip_prefix("Current session:") {
            ("5h".to_owned(), rest)
        } else if let Some((name, rest)) = line
            .strip_prefix("Current week (")
            .and_then(|rest| rest.split_once("):"))
        {
            if name == "all models" {
                ("week".to_owned(), rest)
            } else {
                (format!("week.{}", slug(name)), rest)
            }
        } else {
            continue;
        };
        if readings.iter().any(|reading| reading.window == window) {
            continue;
        }
        let Some(used) = claude_used(rest) else {
            continue;
        };
        let resets_text = rest
            .split_once("resets ")
            .map(|(_, reset)| reset.trim().to_owned());
        let resets_at = resets_text
            .as_deref()
            .and_then(wall_clock)
            .and_then(|clock| resolve(&clock, read_at));
        readings.push(Reading {
            pool: MAX.to_owned(),
            window,
            used: Some(used),
            resets_at,
            resets_text,
            read_at,
            source: "claude /usage".to_owned(),
        });
    }
    if !readings
        .iter()
        .any(|reading| reading.window == "5h" || reading.window == "week")
    {
        return None;
    }
    add_unknown(&mut readings, MAX, read_at, "claude /usage");
    Some(readings)
}

/// A reset as Claude Code prints it: a wall clock in an IANA zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WallClock {
    /// 1 to 12; `None` with `day` when only a time is printed, which is today or tomorrow.
    pub month: Option<u8>,
    pub day: Option<u8>,
    pub hour: u8,
    pub minute: u8,
    pub zone: String,
}

/// "Oct 17, 8:50pm (America/New_York)", "Oct 23, 12pm (America/New_York)", "3am (UTC)".
pub(crate) fn wall_clock(text: &str) -> Option<WallClock> {
    let (when, zone) = text.trim().rsplit_once('(')?;
    let zone = zone.trim().strip_suffix(')')?.trim().to_owned();
    let (date, clock) = match when.trim().split_once(',') {
        Some((date, clock)) => (Some(date.trim()), clock.trim()),
        None => (None, when.trim()),
    };
    let (month, day) = match date {
        Some(date) => {
            let (month, day) = date.split_once(' ')?;
            const MONTHS: [&str; 12] = [
                "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
            ];
            let month = month.get(..3)?.to_ascii_lowercase();
            let month = MONTHS.iter().position(|name| *name == month)? as u8 + 1;
            (Some(month), Some(day.trim().parse::<u8>().ok()?))
        }
        None => (None, None),
    };
    let lower = clock.to_ascii_lowercase();
    let (clock, afternoon) = if let Some(clock) = lower.strip_suffix("pm") {
        (clock, true)
    } else {
        (lower.strip_suffix("am")?, false)
    };
    let (hour, minute) = match clock.trim().split_once(':') {
        Some((hour, minute)) => (hour.parse::<u8>().ok()?, minute.parse::<u8>().ok()?),
        None => (clock.trim().parse::<u8>().ok()?, 0),
    };
    if !(1..=12).contains(&hour) || minute > 59 {
        return None;
    }
    Some(WallClock {
        month,
        day,
        hour: hour % 12 + if afternoon { 12 } else { 0 },
        minute,
        zone,
    })
}

/// A wall clock in its zone as Unix seconds, near `now`: the year is the zone's current one, or the next
/// when that date passed more than a day ago (December reading a January reset); a time alone is today, or
/// tomorrow once passed. The fork has no time zone database (only `time`, with no zones), so the zone is
/// resolved by Windows' own ICU (`icuin.dll`, in Windows since 10 1703; herdr's ConPTY needs 1809). Elsewhere
/// the reset keeps its text and no timestamp.
#[cfg(windows)]
pub(crate) fn resolve(clock: &WallClock, now: u64) -> Option<u64> {
    use windows_sys::Win32::Globalization::{
        ucal_close, ucal_get, ucal_getCanonicalTimeZoneID, ucal_getMillis, ucal_open, ucal_set,
        ucal_setDateTime, ucal_setMillis, UCAL_DATE, UCAL_GREGORIAN, UCAL_MILLISECOND, UCAL_MONTH,
        UCAL_YEAR, U_ZERO_ERROR,
    };
    const DAY: f64 = 86_400.0;
    let zone: Vec<u16> = clock.zone.encode_utf16().collect();
    let length = i32::try_from(zone.len()).ok()?;
    let mut status = U_ZERO_ERROR;
    // An unknown zone opens as GMT with no error, so the name is checked first. ICU: above 0 is an error.
    let mut canonical = [0u16; 128];
    let mut system = 0i8;
    // SAFETY: every pointer is to a live buffer of the length passed.
    unsafe {
        ucal_getCanonicalTimeZoneID(
            zone.as_ptr(),
            length,
            canonical.as_mut_ptr(),
            canonical.len() as i32,
            &mut system,
            &mut status,
        )
    };
    if status > U_ZERO_ERROR {
        return None;
    }
    // SAFETY: as above; a null locale is ICU's default.
    let calendar = unsafe {
        ucal_open(
            zone.as_ptr(),
            length,
            std::ptr::null(),
            UCAL_GREGORIAN,
            &mut status,
        )
    };
    if calendar.is_null() || status > U_ZERO_ERROR {
        return None;
    }
    struct Close(*mut *mut core::ffi::c_void);
    impl Drop for Close {
        fn drop(&mut self) {
            // SAFETY: opened above, closed once.
            unsafe { ucal_close(self.0) }
        }
    }
    let _close = Close(calendar);
    let hour = i32::from(clock.hour);
    let minute = i32::from(clock.minute);
    // SAFETY: `calendar` is open until `_close` drops; ICU only reads and writes its own state.
    unsafe {
        let at = |year: i32, month: i32, day: i32, status: &mut i32| {
            ucal_setDateTime(calendar, year, month, day, hour, minute, 0, status);
            ucal_set(calendar, UCAL_MILLISECOND, 0);
            (ucal_getMillis(calendar as *const _, status) / 1000.0).floor()
        };
        ucal_setMillis(calendar, now as f64 * 1000.0, &mut status);
        let year = ucal_get(calendar as *const _, UCAL_YEAR, &mut status);
        let now = now as f64;
        let seconds = match (clock.month, clock.day) {
            (Some(month), Some(day)) => {
                let (month, day) = (i32::from(month) - 1, i32::from(day));
                let this_year = at(year, month, day, &mut status);
                if this_year + DAY < now {
                    at(year + 1, month, day, &mut status)
                } else {
                    this_year
                }
            }
            _ => {
                let month = ucal_get(calendar as *const _, UCAL_MONTH, &mut status);
                let day = ucal_get(calendar as *const _, UCAL_DATE, &mut status);
                let today = at(year, month, day, &mut status);
                if today < now {
                    at(year, month, day + 1, &mut status)
                } else {
                    today
                }
            }
        };
        (status <= U_ZERO_ERROR && seconds >= 0.0).then_some(seconds as u64)
    }
}

#[cfg(not(windows))]
pub(crate) fn resolve(_clock: &WallClock, _now: u64) -> Option<u64> {
    None
}

/// One Codex rollout line's `rate_limits`, from a `token_count` event. `written_at` is when the line
/// was written (the file's modified time) in Unix seconds, for windows that give `resets_in_seconds`.
/// A window whose reset has passed reads 0% with its next reset unknown: Codex writes after each turn, so a
/// newer line would exist had it run. When every window has reset, the reading is too old to say anything and
/// is no reading.
pub fn codex_readings(line: &str, written_at: u64, now: u64) -> Option<Vec<Reading>> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let limits = value
        .get("payload")
        .and_then(|payload| payload.get("rate_limits"))
        .or_else(|| value.get("rate_limits"))
        .filter(|limits| limits.is_object())?;
    let mut readings = Vec::new();
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
        // Codex names its windows by length: 299 or 300 minutes, and 10079 or 10080.
        let minutes = window
            .get("window_minutes")
            .and_then(serde_json::Value::as_u64);
        let name = match (key, minutes) {
            (_, Some(minutes)) if minutes <= 360 => "5h",
            (_, Some(minutes)) if minutes >= 1440 => "week",
            ("primary", None) => "5h",
            ("secondary", None) => "week",
            _ => continue,
        };
        readings.push(Reading {
            pool: CODEX.to_owned(),
            window: name.to_owned(),
            used: Some(if live {
                (used / 100.0).clamp(0.0, 1.0) as f32
            } else {
                0.0
            }),
            resets_at: resets_at.filter(|_| live),
            resets_text: None,
            read_at: now,
            source: "codex rollout".to_owned(),
        });
    }
    if !any_live || readings.is_empty() {
        return None;
    }
    add_unknown(&mut readings, CODEX, now, "codex rollout");
    Some(readings)
}

/// The GPU memory line of `nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader,nounits`
/// ("3539, 32607", in MiB), the first GPU: used and total.
pub fn gpu_memory(text: &str) -> Option<(u64, u64)> {
    let mut fields = text.lines().next()?.split(',').map(str::trim);
    let used_mib = fields.next()?.parse().ok()?;
    let total_mib = fields.next()?.parse().ok()?;
    Some((used_mib, total_mib))
}

/// Whether a program is on PATH, the way a shell would find it (`.exe`, `.cmd` on Windows).
pub(crate) fn on_path(program: &str) -> bool {
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
    run_in(program, args, None, timeout)
}

/// `run`, in a folder: the Preload mod asks Claude Code about the folder an agent works in.
pub(crate) fn run_in(
    program: &str,
    args: &[&str],
    dir: Option<&Path>,
    timeout: Duration,
) -> Option<String> {
    let mut command = Command::new(program);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
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

/// What one read gave.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Readings(Vec<Reading>),
    /// A state in a word: "not installed", "not exposed", "off", "on".
    Word(&'static str),
    /// The GPU's memory in MiB while the local engine answers.
    Memory {
        used_mib: u64,
        total_mib: u64,
    },
}

fn read_claude(now: u64) -> Option<Answer> {
    if !on_path("claude") {
        return Some(Answer::Word("not installed"));
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
    claude_readings(&text, now).map(Answer::Readings)
}

fn read_codex(now: u64) -> Option<Answer> {
    let installed = on_path("codex");
    let readings = crate::nirvana_spaces::codex_home()
        .map(|home| crate::nirvana_spaces::recent_codex_rollouts(&home.join("sessions")))
        .into_iter()
        .flatten()
        .find_map(|path| codex_rollout_readings(&path, now));
    Some(match readings {
        Some(readings) => Answer::Readings(readings),
        None if installed => Answer::Word("not exposed"),
        None => Answer::Word("not installed"),
    })
}

fn codex_rollout_readings(path: &Path, now: u64) -> Option<Vec<Reading>> {
    let written_at = unix(path.metadata().ok()?.modified().ok()?);
    let text = crate::nirvana_spaces::read_tail(path)?;
    text.lines()
        .rev()
        .filter(|line| line.contains("\"rate_limits\""))
        .find_map(|line| codex_readings(line, written_at, now))
}

fn read_agy(now: u64) -> Option<Answer> {
    if !on_path("agy") {
        return Some(Answer::Word("not installed"));
    }
    let stream = run(
        "agy",
        &["-p", "/usage", "--output-format", "stream-json"],
        AGY_TIMEOUT,
    )?;
    agy_readings(&stream, now).map(Answer::Readings)
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

fn read_local() -> Option<Answer> {
    if !ollama_answers() {
        return Some(Answer::Word("off"));
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
        .map_or(Answer::Word("on"), |(used_mib, total_mib)| Answer::Memory {
            used_mib,
            total_mib,
        }),
    )
}

fn read(provider: Provider, now: u64) -> Option<Answer> {
    match provider {
        Provider::Claude => read_claude(now),
        Provider::Codex => read_codex(now),
        Provider::Antigravity => read_agy(now),
        Provider::Local => read_local(),
    }
}

pub(crate) fn unix(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn now() -> u64 {
    unix(SystemTime::now())
}

/// What one reader last gave, as the window keeps it and the cache file holds it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Source {
    /// When the reader last answered, Unix seconds.
    #[serde(default)]
    pub read_at: Option<u64>,
    /// When it last failed after that: what it gave before is kept and shown with its age.
    #[serde(default)]
    pub failed_at: Option<u64>,
    /// A state in a word in place of readings: "not installed", "not exposed", "off", "on".
    #[serde(default)]
    pub word: Option<String>,
    /// Local only: the GPU's memory in MiB, used and total.
    #[serde(default)]
    pub memory_mib: Option<(u64, u64)>,
    #[serde(default)]
    pub readings: Vec<Reading>,
}

impl Source {
    /// The last time the reader was tried, answered or not.
    fn tried_at(&self) -> Option<u64> {
        self.read_at.max(self.failed_at)
    }

    /// Takes a read. A failed one keeps the last readings; a first read that fails says the figure is not
    /// exposed instead of reading forever.
    pub fn apply(&mut self, answer: Option<Answer>, now: u64) {
        let Some(answer) = answer else {
            if self.read_at.is_none() {
                self.word = Some("not exposed".to_owned());
            }
            self.failed_at = Some(now);
            return;
        };
        self.read_at = Some(now);
        self.failed_at = None;
        self.word = None;
        self.memory_mib = None;
        self.readings = match answer {
            Answer::Readings(readings) => readings,
            Answer::Word(word) => {
                self.word = Some(word.to_owned());
                Vec::new()
            }
            Answer::Memory {
                used_mib,
                total_mib,
            } => {
                self.memory_mib = Some((used_mib, total_mib));
                vec![Reading {
                    pool: LOCAL.to_owned(),
                    window: "vram".to_owned(),
                    used: (total_mib > 0)
                        .then(|| (used_mib as f32 / total_mib as f32).clamp(0.0, 1.0)),
                    resets_at: None,
                    resets_text: None,
                    read_at: now,
                    source: "local nvidia-smi".to_owned(),
                }]
            }
        };
    }

    /// Whether `herdr usage` should read this again: never tried, tried more than `STALE_AFTER` ago, or a
    /// reading's reset has passed since the last try. A try after the reset counts, so a reader whose window
    /// has not started again is not read on every call.
    pub fn needs_read(&self, now: u64) -> bool {
        let Some(tried) = self.tried_at() else {
            return true;
        };
        now.saturating_sub(tried) > STALE_AFTER
            || self
                .readings
                .iter()
                .any(|reading| reading.resets_at.is_some_and(|at| at <= now && tried < at))
    }
}

/// The cache file: each reader's last `Source`, keyed by `Provider::key`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cache {
    pub version: u32,
    #[serde(default)]
    pub sources: BTreeMap<String, Source>,
}

pub fn cache_path() -> Option<PathBuf> {
    crate::nirvana::home().map(|home| home.join(CACHE_DIR).join(CACHE_FILE))
}

pub fn read_cache(path: &Path) -> Cache {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes one reader's source into the cache file, keeping the other readers' entries, atomically: a temp
/// file, then a rename. Several windows and `herdr usage` share the file, so an entry is replaced only by one
/// tried as late or later.
pub fn write_source(path: &Path, provider: Provider, source: &Source) -> std::io::Result<()> {
    static WRITING: Mutex<()> = Mutex::new(());
    let _writing = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut cache = read_cache(path);
    cache.version = 1;
    let newer = cache
        .sources
        .get(provider.key())
        .is_none_or(|known| source.tried_at() >= known.tried_at());
    if !newer {
        return Ok(());
    }
    cache
        .sources
        .insert(provider.key().to_owned(), source.clone());
    let dir = path.parent().expect("a parent");
    std::fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".{CACHE_FILE}.{}.tmp", std::process::id()));
    std::fs::write(&temp, serde_json::to_vec_pretty(&cache)?)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// The foot's figure for one pool: its tightest window.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub window: String,
    pub used: Option<f32>,
    pub pace: Option<f32>,
    /// Seconds to the reset.
    pub resets_in: Option<u64>,
    pub stale: bool,
    /// Any window every seat of the pool spends (no dot: `week.fable` binds only Fable) is fresh and ahead
    /// of pace, not only the shown one. Plan 12 O2: live on 2026-10-17 Claude's week read pace 1.67 while
    /// its 5 h window, the tightest at 0.98, was shown, and the row carried no mark.
    pub ahead: bool,
}

/// What a usage row shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Figure {
    /// Before the first read finished.
    Reading,
    /// The pool's tightest window.
    Window(Shown),
    /// The GPU's memory while the local engine answers.
    Memory { used_mib: u64, total_mib: u64 },
    /// A state in a word: "not installed", "not exposed", "off", "on".
    Word(String),
}

impl Figure {
    /// The bar's fill: the tightest window, or the GPU memory in use.
    pub fn fill(&self) -> Option<f32> {
        match self {
            Self::Window(Shown {
                used: Some(used), ..
            }) => Some(used.clamp(0.0, 1.0)),
            Self::Memory {
                used_mib,
                total_mib,
            } if *total_mib > 0 => Some((*used_mib as f32 / *total_mib as f32).clamp(0.0, 1.0)),
            _ => None,
        }
    }

    /// A window of the pool ahead of pace, and fresh enough to say so (`Shown::ahead`).
    pub fn ahead(&self) -> bool {
        matches!(self, Self::Window(Shown { ahead: true, .. }))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageRow {
    pub pool: String,
    pub provider: Provider,
    pub figure: Figure,
}

impl UsageRow {
    pub fn label(&self) -> &str {
        label(&self.pool)
    }
}

/// The pools to show: `POOLS`, then any other pool a reader gave, in the order it came.
fn pool_names<'a>(sources: impl Iterator<Item = &'a Source>) -> Vec<String> {
    let mut pools: Vec<String> = POOLS.iter().map(|pool| (*pool).to_owned()).collect();
    for source in sources {
        for reading in &source.readings {
            if !pools.contains(&reading.pool) {
                pools.push(reading.pool.clone());
            }
        }
    }
    pools
}

/// Whether any window every seat of the pool spends is fresh and ahead of pace. A scoped week
/// (`week.fable`) binds one model only, so it marks nothing, as it never decides the tightest window.
fn any_ahead(readings: &[Reading], now: u64) -> bool {
    readings.iter().any(|reading| {
        !reading.window.contains('.')
            && freshness(reading, now) == Freshness::Fresh
            && pace(reading).is_some_and(|pace| pace > AHEAD_OF_PACE)
    })
}

/// One row per pool from the readers' sources, at `now`.
pub fn pool_rows(sources: &[Source; 4], now: u64) -> Vec<UsageRow> {
    pool_names(sources.iter())
        .into_iter()
        .map(|pool| {
            let provider = provider_of(&pool);
            let source = &sources[provider.index()];
            let readings: Vec<Reading> = source
                .readings
                .iter()
                .filter(|reading| reading.pool == pool)
                .cloned()
                .collect();
            let figure = if let Some(word) = &source.word {
                Figure::Word(word.clone())
            } else if source.read_at.is_none() {
                Figure::Reading
            } else if let Some((used_mib, total_mib)) = source.memory_mib.filter(|_| pool == LOCAL)
            {
                Figure::Memory {
                    used_mib,
                    total_mib,
                }
            } else if let Some(reading) = tightest(&readings) {
                let freshness = freshness(reading, now);
                Figure::Window(Shown {
                    window: reading.window.clone(),
                    used: reading.used,
                    pace: pace(reading).filter(|_| freshness != Freshness::ResetPassed),
                    resets_in: reading
                        .resets_at
                        .filter(|_| freshness != Freshness::ResetPassed)
                        .map(|at| at.saturating_sub(now)),
                    stale: freshness != Freshness::Fresh,
                    ahead: any_ahead(&readings, now),
                })
            } else {
                Figure::Word("not exposed".to_owned())
            };
            UsageRow {
                pool,
                provider,
                figure,
            }
        })
        .collect()
}

static CHANGED: AtomicBool = AtomicBool::new(false);
static CLAUDE_WORKING: AtomicBool = AtomicBool::new(false);

fn sources_state() -> &'static Mutex<[Source; 4]> {
    static SOURCES: OnceLock<Mutex<[Source; 4]>> = OnceLock::new();
    SOURCES.get_or_init(|| Mutex::new(Default::default()))
}

fn store(provider: Provider, answer: Option<Answer>, now: u64) -> Source {
    let mut sources = sources_state().lock().unwrap_or_else(|e| e.into_inner());
    let source = &mut sources[provider.index()];
    let before = source.clone();
    source.apply(answer, now);
    // A failure only moves `failed_at`; the figure the foot draws is the same.
    if source.readings != before.readings
        || source.word != before.word
        || source.memory_mib != before.memory_mib
    {
        CHANGED.store(true, Ordering::Release);
    }
    source.clone()
}

fn start_worker() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let path = cache_path();
        // What any window or `herdr usage` read last shows at once, with its age, until this window's own read.
        if let Some(path) = &path {
            let cache = read_cache(path);
            let mut sources = sources_state().lock().unwrap_or_else(|e| e.into_inner());
            for provider in Provider::ALL {
                if let Some(source) = cache.sources.get(provider.key()) {
                    sources[provider.index()] = source.clone();
                }
            }
        }
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
            let path = path.clone();
            let _ = crate::thread_spawn::spawn_named("nirvana-usage", move || loop {
                let started = Instant::now();
                let read_at = now();
                let answer = read(provider, read_at);
                let source = store(provider, answer, read_at);
                if let Some(path) = &path {
                    let _ = write_source(path, provider, &source);
                }
                // Sleep in short steps so a Claude that starts working shortens the wait.
                while started.elapsed() < every(provider) {
                    std::thread::sleep(Duration::from_secs(1));
                }
            });
        }
    });
}

/// The rows as last read, one per pool. The first call starts the readers; under test they never start
/// (they would run this machine's `claude` and `agy`), and a test sets the rows it draws.
pub fn rows() -> Vec<UsageRow> {
    #[cfg(test)]
    if let Some(rows) = TEST_ROWS.with_borrow(Clone::clone) {
        return rows;
    }
    if !cfg!(test) {
        start_worker();
    }
    let sources = sources_state()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    pool_rows(&sources, now())
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

/// "45m", "2h59m", "6d17h": the time to a reset in at most five cells.
pub fn short_duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    if minutes < 60 {
        format!("{minutes}m")
    } else if minutes < 24 * 60 {
        format!("{}h{:02}m", minutes / 60, minutes % 60)
            .trim_end_matches("00m")
            .to_owned()
    } else {
        let hours = minutes / 60;
        format!("{}d{}h", hours / 24, hours % 24)
    }
}

/// `herdr usage --json`, for the plugin, the statusline and the seat engine:
///
/// ```json
/// {"now": 1760737872,
///  "pools": [{"pool": "max", "label": "Claude", "provider": "claude",
///             "state": "ok",            // ok | failed | reading | not installed | not exposed | off | on
///             "read_at": 1760737860, "failed_at": null,
///             "tightest": "5h",         // the window that binds the pool; null when none is known
///             "windows": [{"window": "5h", "used": 0.41,   // 0..1; null = not exposed, never 0
///                          "resets_at": 1760748600, "resets_in_s": 10728,
///                          "resets_text": "Oct 17, 8:50pm (America/New_York)",
///                          "read_at": 1760737860, "age_s": 12,
///                          "fresh": true, "stale": null,   // or "old" (over 300 s), "reset passed"
///                          "elapsed": 0.40, "pace": 1.02,
///                          "source": "claude /usage"}]}]}
/// ```
///
/// A window a pool does not have is absent. `local` has one window, `vram`, with no reset.
pub fn view(sources: &[Source; 4], now: u64) -> serde_json::Value {
    use serde_json::json;
    let pools = pool_names(sources.iter())
        .into_iter()
        .map(|pool| {
            let provider = provider_of(&pool);
            let source = &sources[provider.index()];
            let readings: Vec<Reading> = source
                .readings
                .iter()
                .filter(|reading| reading.pool == pool)
                .cloned()
                .collect();
            let state = match (&source.word, source.read_at, source.failed_at) {
                (Some(word), _, _) => word.clone(),
                (None, None, _) => "reading".to_owned(),
                (None, Some(_), Some(_)) => "failed".to_owned(),
                (None, Some(_), None) => "ok".to_owned(),
            };
            let windows = readings
                .iter()
                .map(|reading| {
                    let freshness = freshness(reading, now);
                    json!({
                        "window": reading.window,
                        "used": reading.used.map(round),
                        "resets_at": reading.resets_at,
                        "resets_in_s": reading.resets_at.map(|at| at.saturating_sub(now)),
                        "resets_text": reading.resets_text,
                        "read_at": reading.read_at,
                        "age_s": now.saturating_sub(reading.read_at),
                        "fresh": freshness == Freshness::Fresh,
                        "stale": (freshness != Freshness::Fresh).then(|| freshness.word()),
                        "elapsed": elapsed(reading).map(round),
                        "pace": pace(reading).map(round),
                        "source": reading.source,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "pool": pool,
                "label": label(&pool),
                "provider": provider.key(),
                "state": state,
                "read_at": source.read_at,
                "failed_at": source.failed_at,
                "tightest": tightest(&readings).filter(|r| r.used.is_some()).map(|r| r.window.clone()),
                "windows": windows,
            })
        })
        .collect::<Vec<_>>();
    json!({ "now": now, "pools": pools })
}

/// An f32 share as JSON prints it: six places, not the float's widening noise (0.44 printed 0.4399999976).
fn round(share: f32) -> f64 {
    (f64::from(share) * 1e6).round() / 1e6
}

/// The human table of `herdr usage`.
pub fn table(sources: &[Source; 4], now: u64) -> String {
    let mut out = format!(
        "{:<10} {:<11} {:>5}  {:<9} {:>5}  {}\n",
        "pool", "window", "used", "resets in", "pace", "read"
    );
    for row in pool_rows(sources, now) {
        let source = &sources[row.provider.index()];
        let readings: Vec<&Reading> = source
            .readings
            .iter()
            .filter(|reading| reading.pool == row.pool)
            .collect();
        if readings.is_empty() || source.word.is_some() {
            let word = match &row.figure {
                Figure::Word(word) => word.clone(),
                _ => "reading".to_owned(),
            };
            out.push_str(&format!("{:<10} {word}\n", row.label()));
            continue;
        }
        for reading in readings {
            let freshness = freshness(reading, now);
            let used = reading
                .used
                .map_or_else(|| "--".to_owned(), |used| format!("{:.0}%", used * 100.0));
            let resets = reading.resets_at.map_or_else(
                || "-".to_owned(),
                |at| short_duration(at.saturating_sub(now)),
            );
            let pace = pace(reading).map_or_else(|| "-".to_owned(), |pace| format!("{pace:.2}"));
            let mut read = format!(
                "{} {}",
                freshness.word(),
                short_age(now.saturating_sub(reading.read_at))
            );
            if source.failed_at.is_some() {
                read.push_str(", last read failed");
            }
            out.push_str(&format!(
                "{:<10} {:<11} {used:>5}  {resets:<9} {pace:>5}  {read}\n",
                row.label(),
                reading.window,
            ));
        }
    }
    out
}

/// "12s", "4m", "2h": how old a reading is.
fn short_age(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86_400)
    }
}

/// `herdr usage [--json] [--refresh]`: prints the cache, after reading again every reader whose entry is
/// missing or stale, or all of them with `--refresh`, and writing what they gave. Exits 0 when any window has
/// a figure, 1 when none could be produced.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    let mut json = false;
    let mut refresh = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            "--refresh" => refresh = true,
            _ => {
                eprintln!("usage: herdr usage [--json] [--refresh]");
                std::process::exit(2);
            }
        }
    }
    let path = cache_path();
    let cache = path.as_deref().map(read_cache).unwrap_or_default();
    let mut sources: [Source; 4] = Default::default();
    for provider in Provider::ALL {
        if let Some(source) = cache.sources.get(provider.key()) {
            sources[provider.index()] = source.clone();
        }
    }
    let read_at = now();
    let stale: Vec<Provider> = Provider::ALL
        .into_iter()
        .filter(|provider| refresh || sources[provider.index()].needs_read(read_at))
        .collect();
    let answers: Vec<(Provider, Option<Answer>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = stale
            .iter()
            .map(|&provider| (provider, scope.spawn(move || read(provider, read_at))))
            .collect();
        handles
            .into_iter()
            .map(|(provider, handle)| (provider, handle.join().ok().flatten()))
            .collect()
    });
    for (provider, answer) in answers {
        let source = &mut sources[provider.index()];
        source.apply(answer, read_at);
        if let Some(path) = &path {
            if let Err(err) = write_source(path, provider, source) {
                eprintln!("herdr usage: could not write {}: {err}", path.display());
            }
        }
    }
    let now = now();
    if json {
        println!("{}", serde_json::to_string_pretty(&view(&sources, now))?);
    } else {
        print!("{}", table(&sources, now));
    }
    std::io::stdout().flush()?;
    let any = sources
        .iter()
        .any(|source| source.readings.iter().any(|reading| reading.used.is_some()));
    if !any {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Claude Code 2.1.301's `/usage` answer, captured 2026-10-15 (the contributions block shortened).
    const CLAUDE_TEXT: &str = "You are currently using your subscription to power your Claude Code usage\n\nCurrent session: 2% used \u{b7} resets Oct 16, 2:30am (America/New_York)\nCurrent week (all models): 70% used \u{b7} resets Oct 16, 12pm (America/New_York)\nCurrent week (Fable): 83% used \u{b7} resets Oct 16, 12pm (America/New_York)\n\nWhat's contributing to your limits usage?\n  98% of your usage came from subagent-heavy sessions\n";

    /// The same answer at 2026-10-17T21:51Z (the contributions block shortened).
    const CLAUDE_TEXT_1017: &str = "You are currently using your subscription to power your Claude Code usage\n\nCurrent session: 41% used \u{b7} resets Oct 17, 8:50pm (America/New_York)\nCurrent week (all models): 37% used \u{b7} resets Oct 23, 12pm (America/New_York)\nCurrent week (Fable): 0% used \u{b7} resets Oct 23, 12pm (America/New_York)\n\nWhat's contributing to your limits usage?\nLast 24h \u{b7} 1880 requests \u{b7} 11 sessions\n  82% of your usage was at >150k context\n";

    /// 2026-10-17T21:51:12Z, when `CLAUDE_TEXT_1017` and `AGY_STREAM_1017` were read.
    const READ_1017: u64 = 1_792_273_872;

    fn window<'a>(readings: &'a [Reading], pool: &str, window: &str) -> Option<&'a Reading> {
        readings
            .iter()
            .find(|reading| reading.pool == pool && reading.window == window)
    }

    fn used(readings: &[Reading], pool: &str, name: &str) -> Option<Option<f32>> {
        window(readings, pool, name).map(|reading| reading.used)
    }

    fn close(a: Option<Option<f32>>, b: f32) -> bool {
        a.flatten().is_some_and(|a| (a - b).abs() < 1e-4)
    }

    #[test]
    fn claude_reads_the_session_the_week_and_each_scoped_week() {
        let readings = claude_readings(CLAUDE_TEXT, 0).unwrap();
        assert!(close(used(&readings, MAX, "5h"), 0.02), "{readings:?}");
        assert!(close(used(&readings, MAX, "week"), 0.70), "{readings:?}");
        assert!(
            close(used(&readings, MAX, "week.fable"), 0.83),
            "{readings:?}"
        );
        let session = window(&readings, MAX, "5h").unwrap();
        assert_eq!(
            session.resets_text.as_deref(),
            Some("Oct 16, 2:30am (America/New_York)")
        );
        assert_eq!(session.source, "claude /usage");
        // "<1%" reads as 0; a missing week is unknown, never 0.
        let readings = claude_readings("Current session: <1% used\n", 0).unwrap();
        assert_eq!(used(&readings, MAX, "5h"), Some(Some(0.0)));
        assert_eq!(used(&readings, MAX, "week"), Some(None));
        assert_eq!(used(&readings, MAX, "week.fable"), None);
        assert_eq!(
            claude_readings("Invalid API key \u{b7} Please run /login", 0),
            None
        );
    }

    #[test]
    fn a_claude_reset_is_a_wall_clock_in_its_zone() {
        assert_eq!(
            wall_clock("Oct 17, 8:50pm (America/New_York)"),
            Some(WallClock {
                month: Some(10),
                day: Some(17),
                hour: 20,
                minute: 50,
                zone: "America/New_York".into()
            })
        );
        assert_eq!(
            wall_clock("Oct 23, 12pm (America/New_York)").map(|c| (c.hour, c.minute)),
            Some((12, 0))
        );
        assert_eq!(
            wall_clock("Jan 2, 12am (UTC)").map(|c| (c.month, c.day, c.hour)),
            Some((Some(1), Some(2), 0))
        );
        assert_eq!(
            wall_clock("3:05am (Europe/London)").map(|c| (c.month, c.hour, c.minute)),
            Some((None, 3, 5))
        );
        assert_eq!(wall_clock("soon"), None);
        assert_eq!(wall_clock("Oct 17, 13pm (UTC)"), None);
    }

    /// The zone is resolved by Windows' ICU; elsewhere the reset keeps only its text.
    #[cfg(windows)]
    #[test]
    fn a_claude_reset_resolves_in_its_zone_and_year() {
        let readings = claude_readings(CLAUDE_TEXT_1017, READ_1017).unwrap();
        // 8:50pm EDT on 2026-10-17 is 00:50Z on the 18th; 12pm on the 23rd is 16:00Z.
        assert_eq!(
            window(&readings, MAX, "5h").unwrap().resets_at,
            rfc3339("2026-10-18T00:50:00Z")
        );
        assert_eq!(
            window(&readings, MAX, "week").unwrap().resets_at,
            rfc3339("2026-10-23T16:00:00Z")
        );
        // In winter New York is 5 h behind.
        let dec = rfc3339("2026-12-01T00:00:00Z").unwrap();
        let clock = wall_clock("Dec 1, 9am (America/New_York)").unwrap();
        assert_eq!(resolve(&clock, dec), rfc3339("2026-12-01T14:00:00Z"));
        // A December reading of a January reset is next year's.
        let late = rfc3339("2026-12-31T12:00:00Z").unwrap();
        let clock = wall_clock("Jan 2, 12pm (UTC)").unwrap();
        assert_eq!(resolve(&clock, late), rfc3339("2027-01-02T12:00:00Z"));
        // A time alone is today, or tomorrow once it has passed.
        let clock = wall_clock("11pm (UTC)").unwrap();
        assert_eq!(resolve(&clock, late), rfc3339("2026-12-31T23:00:00Z"));
        let clock = wall_clock("3am (UTC)").unwrap();
        assert_eq!(resolve(&clock, late), rfc3339("2027-01-01T03:00:00Z"));
        // An unknown zone is no timestamp, not GMT.
        let clock = wall_clock("Oct 17, 8:50pm (Mars/Olympus)").unwrap();
        assert_eq!(resolve(&clock, READ_1017), None);
    }

    #[test]
    fn rfc3339_reads_agys_reset_times() {
        assert_eq!(rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339("2026-10-24T15:48:02Z"), Some(1_792_856_882));
        assert_eq!(rfc3339("2026-10-24T15:48:02.250Z"), Some(1_792_856_882));
        assert_eq!(rfc3339("2026-10-24T11:48:02-04:00"), Some(1_792_856_882));
        assert_eq!(rfc3339("tomorrow"), None);
    }

    /// agy's `/usage` event, captured 2026-10-15 (descriptions cut).
    const AGY_STREAM: &str = r#"{"event":"init"}
{"event":"command_result","command":{"name":"usage","data":{"groups":[{"name":"Gemini Models","buckets":[{"id":"gemini-weekly","window":"weekly","remaining_fraction":0.75,"reset_time":"2026-10-23T01:50:52Z"},{"id":"gemini-5h","window":"5h","remaining_fraction":1,"reset_time":"2026-10-16T06:50:52Z"}]},{"name":"Claude and GPT models","buckets":[{"id":"3p-weekly","window":"weekly","remaining_fraction":0.1},{"id":"3p-5h","window":"5h","remaining_fraction":0.2}]},{"name":"Gemini 4 Argon","buckets":[{"id":"gemini-argon-wtu-weekly","window":"weekly","remaining_fraction":0.5}]}]}}}
{"event":"result","result":{"status":"SUCCESS"}}"#;

    /// agy 1.4.1's `/usage` event at 2026-10-17T21:51Z (descriptions and the result's response cut).
    const AGY_STREAM_1017: &str = r#"{"event":"command_result","command":{"name":"usage","data":{"description":"Within each group, models share a weekly limit and a 5-hour limit.","groups":[{"name":"Gemini Models","description":"Models within this group: Gemini Flash, Gemini Pro","buckets":[{"id":"gemini-weekly","name":"Weekly Limit Remaining","window":"weekly","remaining_fraction":0.9831965565681458,"reset_time":"2026-10-24T15:48:02Z"},{"id":"gemini-5h","name":"Five Hour Limit Remaining","window":"5h","remaining_fraction":0.9803827404975891,"reset_time":"2026-10-18T02:35:36Z"}]},{"name":"Claude and GPT models","description":"Models within this group: Claude Opus, Claude Sonnet, GPT-OSS","buckets":[{"id":"3p-weekly","name":"Weekly Limit Remaining","window":"weekly","remaining_fraction":0.9597715735435486,"reset_time":"2026-10-19T10:54:24Z"},{"id":"3p-5h","name":"Five Hour Limit Remaining","window":"5h","remaining_fraction":0.9668887853622437,"reset_time":"2026-10-18T02:40:32Z"}]},{"name":"Gemini 4 Argon","description":"For paid Pro plans, Argon quota is separate.","buckets":[{"id":"gemini-argon-wtu-weekly","name":"Gemini 4 Argon Weekly Limit","window":"weekly","remaining_fraction":0.978826642036438,"reset_time":"2026-10-24T16:03:32Z"}]}]}}}
{"event":"result","result":{"conversation_id":"","status":"SUCCESS"}}"#;

    #[test]
    fn antigravity_reads_every_group_into_its_pool() {
        let readings = agy_readings(AGY_STREAM_1017, READ_1017).unwrap();
        assert!(close(used(&readings, GEMINI, "week"), 1.0 - 0.983_196_6));
        assert!(close(used(&readings, GEMINI, "5h"), 1.0 - 0.980_382_7));
        assert!(close(used(&readings, GOOGLE3P, "week"), 1.0 - 0.959_771_6));
        assert!(close(used(&readings, GOOGLE3P, "5h"), 1.0 - 0.966_888_8));
        assert!(close(used(&readings, ARGON, "week"), 1.0 - 0.978_826_6));
        // Argon has no 5 h window: absent, not unknown.
        assert_eq!(window(&readings, ARGON, "5h"), None);
        assert_eq!(readings.len(), 5, "{readings:?}");
        // agy lists each week first; the readings put 5 h first.
        let gemini: Vec<&str> = readings
            .iter()
            .filter(|reading| reading.pool == GEMINI)
            .map(|reading| reading.window.as_str())
            .collect();
        assert_eq!(gemini, ["5h", "week"]);
        let five = window(&readings, GEMINI, "5h").unwrap();
        assert_eq!(five.resets_at, rfc3339("2026-10-18T02:35:36Z"));
        assert_eq!(five.source, "agy gemini-5h");
        assert_eq!(five.read_at, READ_1017);
        // The 2026-10-15 shape, a 3p bucket with no reset.
        let readings = agy_readings(AGY_STREAM, 0).unwrap();
        assert!(close(used(&readings, GEMINI, "week"), 0.25));
        assert!(close(used(&readings, GOOGLE3P, "5h"), 0.8));
        assert_eq!(window(&readings, GOOGLE3P, "5h").unwrap().resets_at, None);
        assert_eq!(agy_readings("{\"event\":\"result\"}", 0), None);
    }

    #[test]
    fn agy_buckets_map_to_pools_by_prefix_argon_first() {
        assert_eq!(agy_pool("gemini-argon-wtu-weekly", "Gemini 4 Argon"), ARGON);
        assert_eq!(agy_pool("gemini-argon-5h", "Gemini Models"), ARGON);
        assert_eq!(agy_pool("gemini-5h", "Gemini Models"), GEMINI);
        assert_eq!(agy_pool("3p-weekly", "Claude and GPT models"), GOOGLE3P);
        // An unknown bucket is never dropped: its group names its pool.
        assert_eq!(agy_pool("neon-daily", "Gemini 5 Neon"), "gemini-5-neon");
        assert_eq!(agy_pool("neon-daily", ""), "agy");
        let stream = AGY_STREAM_1017.replace(
            r#"{"name":"Gemini 4 Argon""#,
            r#"{"name":"Gemini 5 Neon","buckets":[{"id":"neon-weekly","window":"weekly","remaining_fraction":0.4}]},{"name":"Gemini 4 Argon""#,
        );
        let readings = agy_readings(&stream, READ_1017).unwrap();
        assert!(
            close(used(&readings, "gemini-5-neon", "week"), 0.6),
            "{readings:?}"
        );
        let sources = sources_with(Some(readings), None);
        let pools: Vec<String> = pool_rows(&sources, READ_1017)
            .into_iter()
            .map(|row| row.pool)
            .collect();
        assert_eq!(
            pools,
            [MAX, GEMINI, ARGON, GOOGLE3P, CODEX, LOCAL, "gemini-5-neon"]
        );
    }

    #[test]
    fn a_pool_agy_did_not_print_is_unknown_and_never_zero() {
        // The Argon group's one bucket renamed into Google's pool: agy printed no Argon bucket.
        let without_argon = AGY_STREAM_1017.replace("gemini-argon-wtu-weekly", "3p-weekly");
        let readings = agy_readings(&without_argon, READ_1017).unwrap();
        assert_eq!(used(&readings, ARGON, "week"), Some(None), "{readings:?}");
        // Two buckets on Google's week keep the tighter, 4.0% over the renamed Argon's 2.1%.
        assert!(close(used(&readings, GOOGLE3P, "week"), 1.0 - 0.959_771_6));
        // No remaining fraction: unknown, not 0.
        let blank = AGY_STREAM_1017.replace(r#""remaining_fraction":0.9803827404975891,"#, "");
        let readings = agy_readings(&blank, READ_1017).unwrap();
        assert_eq!(used(&readings, GEMINI, "5h"), Some(None));
    }

    /// Codex 0.42's `token_count` event, from a rollout on this machine (2025-09-27).
    const CODEX_LINE: &str = r#"{"timestamp":"2025-09-27T09:20:00Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"primary":{"used_percent":5.0,"window_minutes":299,"resets_in_seconds":7549},"secondary":{"used_percent":2.0,"window_minutes":10079,"resets_in_seconds":594349}}}}"#;

    #[test]
    fn codex_reads_its_windows_and_drops_reset_ones() {
        let written = 1_000_000;
        let readings = codex_readings(CODEX_LINE, written, written + 60).unwrap();
        assert!(close(used(&readings, CODEX, "5h"), 0.05));
        assert!(close(used(&readings, CODEX, "week"), 0.02));
        assert_eq!(
            window(&readings, CODEX, "5h").unwrap().resets_at,
            Some(written + 7549)
        );
        // Past the five-hour reset, inside the week.
        let readings = codex_readings(CODEX_LINE, written, written + 7549 + 1).unwrap();
        assert_eq!(used(&readings, CODEX, "5h"), Some(Some(0.0)));
        assert_eq!(window(&readings, CODEX, "5h").unwrap().resets_at, None);
        assert!(close(used(&readings, CODEX, "week"), 0.02));
        // Both reset: a reading that old says nothing.
        assert_eq!(
            codex_readings(CODEX_LINE, written, written + 594349 + 1),
            None
        );
        let absolute = r#"{"payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":40.4,"window_minutes":300,"resets_at":2000},"secondary":null}}}"#;
        let readings = codex_readings(absolute, 0, 1000).unwrap();
        assert!(close(used(&readings, CODEX, "5h"), 0.404));
        assert_eq!(used(&readings, CODEX, "week"), Some(None));
    }

    #[test]
    fn the_gpu_line_is_used_and_total_memory() {
        assert_eq!(gpu_memory("3539, 32607\n"), Some((3539, 32607)));
        let figure = Figure::Memory {
            used_mib: 3539,
            total_mib: 32607,
        };
        assert!((figure.fill().unwrap() - 0.1085).abs() < 0.001);
        assert_eq!(gpu_memory("NVIDIA-SMI has failed"), None);
    }

    fn reading(window: &str, used: Option<f32>, resets_at: Option<u64>, read_at: u64) -> Reading {
        Reading {
            pool: MAX.into(),
            window: window.into(),
            used,
            resets_at,
            resets_text: None,
            read_at,
            source: "test".into(),
        }
    }

    #[test]
    fn a_reading_is_stale_when_old_or_read_before_a_passed_reset() {
        let now = 1_000_000;
        let fresh = reading("5h", Some(0.3), Some(now + 3600), now - 60);
        assert_eq!(freshness(&fresh, now), Freshness::Fresh);
        let old = reading("5h", Some(0.3), Some(now + 3600), now - STALE_AFTER - 1);
        assert_eq!(freshness(&old, now), Freshness::Old);
        assert_eq!(
            freshness(&reading("5h", Some(0.3), None, now - STALE_AFTER), now),
            Freshness::Fresh
        );
        // Read 30 s before a reset 10 s ago.
        let passed = reading("5h", Some(0.9), Some(now - 10), now - 40);
        assert_eq!(freshness(&passed, now), Freshness::ResetPassed);
        // Read after its reset (an idle account whose window has not started again): fresh, no loop.
        let after = reading("5h", Some(0.0), Some(now - 100), now - 50);
        assert_eq!(freshness(&after, now), Freshness::Fresh);
    }

    #[test]
    fn a_failed_reader_keeps_its_readings_and_is_read_again_only_when_due() {
        let now = 1_000_000;
        let mut source = Source::default();
        assert!(source.needs_read(now));
        // A first read that fails says so instead of reading forever.
        source.apply(None, now);
        assert_eq!(source.word.as_deref(), Some("not exposed"));
        let readings = vec![reading("5h", Some(0.4), Some(now + 3600), now)];
        source.apply(Some(Answer::Readings(readings.clone())), now);
        assert_eq!((source.word.as_deref(), source.failed_at), (None, None));
        source.apply(None, now + 100);
        assert_eq!(source.readings, readings);
        assert_eq!(source.failed_at, Some(now + 100));
        assert!(!source.needs_read(now + 100 + STALE_AFTER));
        assert!(source.needs_read(now + 100 + STALE_AFTER + 1));
        // A reset that passed after the last try asks for a read; one before it does not.
        let mut source = Source::default();
        let readings = vec![reading("5h", Some(0.9), Some(now + 60), now)];
        source.apply(Some(Answer::Readings(readings)), now);
        assert!(!source.needs_read(now + 59));
        assert!(source.needs_read(now + 61));
        source.apply(None, now + 61);
        assert!(!source.needs_read(now + 120));
    }

    #[test]
    fn pace_is_the_used_share_against_the_share_of_the_window_gone() {
        let now = 1_000_000;
        // 41% used with 3 h of 5 h left: 40% gone, (0.41 + 0.1) / (0.4 + 0.1).
        let five = reading("5h", Some(0.41), Some(now + 3 * 3600), now);
        assert!((elapsed(&five).unwrap() - 0.4).abs() < 1e-4);
        assert!((pace(&five).unwrap() - 1.02).abs() < 1e-4);
        // A window just reset, nothing used: on pace, not 0 and not infinite, which the ease is for.
        let start = reading("5h", Some(0.0), Some(now + FIVE_HOURS), now);
        assert_eq!(pace(&start), Some(1.0));
        // Nothing used with an hour left: clamped to the floor.
        let idle = reading("5h", Some(0.0), Some(now + 1800), now);
        assert_eq!(pace(&idle), Some(PACE_MIN));
        // 90% of the week in its first day: clamped to the ceiling.
        let week = reading("week", Some(0.9), Some(now + 6 * 86_400), now);
        assert_eq!(pace(&week), Some(PACE_MAX));
        // The Fable week is a week.
        let fable = reading(
            "week.fable",
            Some(0.5),
            Some(now + 3 * 86_400 + 43_200),
            now,
        );
        assert!((pace(&fable).unwrap() - 1.0).abs() < 1e-4);
        assert_eq!(pace(&reading("5h", None, Some(now + 60), now)), None);
        assert_eq!(pace(&reading("5h", Some(0.2), None, now)), None);
    }

    #[test]
    fn the_tightest_window_binds_every_seat_of_the_pool() {
        let readings = vec![
            reading("5h", Some(0.41), None, 0),
            reading("week", Some(0.37), None, 0),
            reading("week.fable", Some(0.99), None, 0),
        ];
        assert_eq!(tightest(&readings).unwrap().window, "5h");
        let readings = vec![
            reading("5h", Some(0.1), None, 0),
            reading("week", Some(0.6), None, 0),
        ];
        assert_eq!(tightest(&readings).unwrap().window, "week");
        let readings = vec![
            reading("5h", None, None, 0),
            reading("week", Some(0.0), None, 0),
        ];
        assert_eq!(tightest(&readings).unwrap().window, "week");
        let readings = vec![
            reading("5h", Some(0.2), None, 0),
            reading("week", Some(0.2), None, 0),
        ];
        assert_eq!(tightest(&readings).unwrap().window, "5h");
    }

    /// Plan 12 O2: live on 2026-10-17 Claude's 5 h window (shown, the tightest) read pace 0.98 and its
    /// week 1.67, and the row had no mark. The mark is the pool's: any window every seat spends.
    #[test]
    fn the_pace_mark_shows_when_any_window_of_the_pool_is_ahead() {
        let now = 1_000_000;
        // 44% used with 2 h 44 m of 5 h left: pace 0.98, behind. 38% of the week with 5 d 17 h left: 1.67.
        let five = reading("5h", Some(0.44), Some(now + 2 * 3600 + 44 * 60), now);
        let week = reading("week", Some(0.38), Some(now + 5 * 86_400 + 17 * 3600), now);
        assert!(pace(&five).unwrap() < AHEAD_OF_PACE, "{:?}", pace(&five));
        assert!(pace(&week).unwrap() > AHEAD_OF_PACE, "{:?}", pace(&week));
        let mut sources: [Source; 4] = Default::default();
        sources[0].apply(
            Some(Answer::Readings(vec![five.clone(), week.clone()])),
            now,
        );
        let rows = pool_rows(&sources, now + 10);
        let Figure::Window(shown) = &rows[0].figure else {
            panic!("{:?}", rows[0]);
        };
        assert_eq!(shown.window, "5h", "the 5 h window is still the one shown");
        assert!(rows[0].figure.ahead(), "the week is ahead: {shown:?}");
        // A scoped week binds Fable only: ahead alone, it marks nothing.
        let fable = reading("week.fable", Some(0.9), Some(now + 5 * 86_400), now);
        let behind = reading("week", Some(0.1), Some(now + 5 * 86_400 + 17 * 3600), now);
        sources[0].apply(
            Some(Answer::Readings(vec![five.clone(), behind, fable])),
            now,
        );
        assert!(!pool_rows(&sources, now + 10)[0].figure.ahead());
        // A stale week says nothing of pace.
        sources[0].apply(Some(Answer::Readings(vec![five, week])), now);
        assert!(!pool_rows(&sources, now + STALE_AFTER + 1)[0].figure.ahead());
    }

    fn sources_with(agy: Option<Vec<Reading>>, claude: Option<Vec<Reading>>) -> [Source; 4] {
        let mut sources: [Source; 4] = Default::default();
        if let Some(claude) = claude {
            sources[0].apply(Some(Answer::Readings(claude)), READ_1017);
        }
        sources[1].apply(Some(Answer::Word("not exposed")), READ_1017);
        if let Some(agy) = agy {
            sources[2].apply(Some(Answer::Readings(agy)), READ_1017);
        }
        sources[3].apply(
            Some(Answer::Memory {
                used_mib: 3539,
                total_mib: 32607,
            }),
            READ_1017,
        );
        sources
    }

    #[test]
    fn each_pool_row_shows_its_tightest_window_and_its_pace() {
        let claude = claude_readings(CLAUDE_TEXT_1017, READ_1017).unwrap();
        let agy = agy_readings(AGY_STREAM_1017, READ_1017).unwrap();
        let sources = sources_with(Some(agy), Some(claude));
        let rows = pool_rows(&sources, READ_1017 + 30);
        let labels: Vec<&str> = rows.iter().map(UsageRow::label).collect();
        assert_eq!(
            labels,
            [
                "Claude",
                "Gemini",
                "Argon",
                "G\u{b7}Claude",
                "Codex",
                "Local"
            ]
        );
        let Figure::Window(claude) = &rows[0].figure else {
            panic!("{:?}", rows[0]);
        };
        assert_eq!(claude.window, "5h");
        assert!((claude.used.unwrap() - 0.41).abs() < 1e-4);
        assert!(!claude.stale);
        if cfg!(windows) {
            assert_eq!(
                claude.resets_in,
                rfc3339("2026-10-18T00:50:00Z").map(|at| at - READ_1017 - 30)
            );
            assert!(rows[0].figure.ahead(), "{claude:?}");
        }
        let Figure::Window(argon) = &rows[2].figure else {
            panic!("{:?}", rows[2]);
        };
        assert_eq!(argon.window, "week");
        assert!(!rows[2].figure.ahead());
        assert_eq!(rows[4].figure, Figure::Word("not exposed".into()));
        assert_eq!(
            rows[5].figure,
            Figure::Memory {
                used_mib: 3539,
                total_mib: 32607
            }
        );
        // Past `STALE_AFTER` every reading is stale and says nothing of pace.
        let later = pool_rows(&sources, READ_1017 + STALE_AFTER + 1);
        assert!(matches!(&later[0].figure, Figure::Window(shown) if shown.stale));
        assert!(!later[0].figure.ahead());
        // Before any read, "reading"; agy not installed reaches all its pools.
        let mut sources: [Source; 4] = Default::default();
        sources[2].apply(Some(Answer::Word("not installed")), 0);
        let rows = pool_rows(&sources, 0);
        assert_eq!(rows[0].figure, Figure::Reading);
        for row in &rows[1..4] {
            assert_eq!(row.figure, Figure::Word("not installed".into()), "{row:?}");
        }
    }

    #[test]
    fn the_cache_round_trips_and_keeps_the_newer_entry() {
        let dir = std::env::temp_dir().join(format!("nirvana-usage-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(CACHE_DIR).join(CACHE_FILE);
        let claude = claude_readings(CLAUDE_TEXT_1017, READ_1017).unwrap();
        let agy = agy_readings(AGY_STREAM_1017, READ_1017).unwrap();
        let sources = sources_with(Some(agy), Some(claude));
        for provider in Provider::ALL {
            write_source(&path, provider, &sources[provider.index()]).unwrap();
        }
        let cache = read_cache(&path);
        assert_eq!(cache.version, 1);
        for provider in Provider::ALL {
            assert_eq!(
                cache.sources.get(provider.key()),
                Some(&sources[provider.index()]),
                "{}",
                provider.key()
            );
        }
        // An older entry, from a window that read before, does not replace a newer one.
        let mut older = sources[0].clone();
        older.read_at = Some(READ_1017 - 600);
        older.readings.clear();
        write_source(&path, Provider::Claude, &older).unwrap();
        assert_eq!(read_cache(&path).sources["claude"], sources[0]);
        // No temp file is left behind.
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from(CACHE_FILE)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_json_view_names_each_window_its_freshness_and_its_pace() {
        let claude = claude_readings(CLAUDE_TEXT_1017, READ_1017).unwrap();
        let agy = agy_readings(AGY_STREAM_1017, READ_1017).unwrap();
        let mut sources = sources_with(Some(agy), Some(claude));
        sources[2].apply(None, READ_1017 + 400);
        let json = view(&sources, READ_1017 + 30);
        let pools = json["pools"].as_array().unwrap();
        let pool = |name: &str| pools.iter().find(|pool| pool["pool"] == name).unwrap();
        assert_eq!(pool("max")["state"], "ok");
        assert_eq!(pool("max")["tightest"], "5h");
        let week = &pool("max")["windows"][1];
        assert_eq!(week["window"], "week");
        assert_eq!(week["used"], 0.37);
        assert_eq!(week["fresh"], true);
        assert_eq!(week["stale"], serde_json::Value::Null);
        assert_eq!(week["age_s"], 30);
        assert!(week["pace"].as_f64().is_some() == cfg!(windows));
        assert_eq!(pool("gemini")["state"], "failed");
        assert_eq!(pool("argon")["windows"].as_array().unwrap().len(), 1);
        assert_eq!(pool("codex")["state"], "not exposed");
        assert_eq!(pool("local")["windows"][0]["window"], "vram");
        // A window not exposed is null, never 0.
        let mut session_only = sources.clone();
        session_only[0].apply(
            claude_readings("Current session: 5% used\n", READ_1017).map(Answer::Readings),
            READ_1017,
        );
        let json = view(&session_only, READ_1017);
        assert_eq!(json["pools"][0]["windows"][1]["window"], "week");
        assert_eq!(
            json["pools"][0]["windows"][1]["used"],
            serde_json::Value::Null
        );
        let stale = view(&sources, READ_1017 + STALE_AFTER + 1);
        assert_eq!(stale["pools"][0]["windows"][0]["stale"], "old");
        // The table holds a line per window.
        let table = table(&sources, READ_1017 + 30);
        assert!(
            table
                .lines()
                .any(|line| line.starts_with("Claude") && line.contains("week.fable")),
            "{table}"
        );
        assert!(
            table
                .lines()
                .any(|line| line.starts_with("Codex") && line.contains("not exposed")),
            "{table}"
        );
        assert!(table.contains("last read failed"), "{table}");
    }

    #[test]
    fn a_reset_reads_in_five_cells() {
        assert_eq!(short_duration(45 * 60), "45m");
        assert_eq!(short_duration(2 * 3600 + 59 * 60), "2h59m");
        assert_eq!(short_duration(3 * 3600), "3h");
        assert_eq!(short_duration(6 * 86_400 + 17 * 3600 + 5), "6d17h");
    }

    #[test]
    fn the_bar_follows_the_tightest_window() {
        let figure = Figure::Window(Shown {
            window: "5h".into(),
            used: Some(0.4),
            pace: None,
            resets_in: None,
            stale: false,
            ahead: false,
        });
        assert_eq!(figure.fill(), Some(0.4));
        assert_eq!(Figure::Word("off".into()).fill(), None);
        assert_eq!(Figure::Reading.fill(), None);
    }
}
