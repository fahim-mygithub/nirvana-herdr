//! Nirvana fork, plan 13 P3 (§6.1): the record of Nirvana Claude's last session in a folder.
//!
//! `herdr memory record-session` runs from Nirvana Claude's `SessionStart` and `Stop` hooks and reads the hook's JSON on
//! stdin. It writes `<home>/nirvana/sessions/<folder_hash>.json`, `{"session_id", "at", "transcript"}`, which "Open
//! Nirvana" (P1) reads to add `--resume <id>` while the record is younger than `resume_within_hours`.
//!
//! **It never fails the hook.** A hook that exits non-zero shows the owner an error on every session start, for a
//! convenience. Any error, a bad stdin, a folder it cannot write to, exits 0 with nothing written; the one thing lost is
//! the resume, and the launch then starts fresh.
//!
//! **Which folder.** The record is keyed by the folder `nirvana window` opened, because that is the folder P1's launch
//! hashes. A hook's `cwd` is the session's *current* directory: measured 2026-10-18 on Claude Code 2.1.302, a session
//! started in `proj` whose Bash call ran `cd sub` got `cwd = proj` at `SessionStart` and `cwd = proj\sub` at `Stop`, so a
//! `Stop` hook keyed by `cwd` would write under the subfolder's hash and the next open would never find the record.
//! `CLAUDE_PROJECT_DIR` (measured in the same run: forward slashes, `proj` at both events, which is also the folder the
//! transcript is filed under) is read first, and `cwd` is only the fallback.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `<home>/nirvana/sessions/`, one file per folder.
pub const DIR: &str = "sessions";
/// The environment variable Claude Code sets for hooks: the directory the session started in.
pub const PROJECT_DIR_ENV: &str = "CLAUDE_PROJECT_DIR";

/// The record's file format, fixed by plan 13 §8 so P1 can read it without this module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub session_id: String,
    /// RFC 3339, UTC.
    pub at: String,
    /// The absolute path of the session's transcript.
    pub transcript: String,
}

/// `<home>/nirvana/sessions/<folder_hash>.json`.
pub fn path_in(home: &Path, folder: &Path) -> PathBuf {
    home.join(crate::nirvana_seat::DIR).join(DIR).join(format!(
        "{}.json",
        crate::nirvana_launch::folder_hash(folder)
    ))
}

/// The folder a hook belongs to: the session's start directory when Claude Code names it, else the hook's `cwd`.
pub fn folder_of(hook: &Value, project_dir: Option<&str>) -> Option<PathBuf> {
    project_dir
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
        .or_else(|| {
            hook.get("cwd")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|cwd| !cwd.is_empty())
        })
        .map(PathBuf::from)
}

/// The record a hook describes, or why it describes none. A subagent's hook is never a record: its session is not the
/// conversation the owner resumes.
pub fn record_of(hook: &Value, now: &str) -> Result<Record, &'static str> {
    if hook.get("agent_id").is_some_and(|a| !a.is_null()) {
        return Err("a subagent's hook");
    }
    let text = |key: &str| {
        hook.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let session_id = text("session_id").ok_or("no session_id")?;
    // The id becomes `--resume <id>` on a command line; only the characters of a UUID are ever written.
    if !session_id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("a session_id with an odd character");
    }
    let transcript = text("transcript_path").ok_or("no transcript_path")?;
    if !Path::new(transcript).is_absolute() {
        return Err("a relative transcript_path");
    }
    Ok(Record {
        session_id: session_id.to_owned(),
        at: now.to_owned(),
        transcript: transcript.to_owned(),
    })
}

/// Writes the record for `folder`, through a temporary file and a rename so a reader never sees half of one.
pub fn write(home: &Path, folder: &Path, record: &Record) -> std::io::Result<()> {
    let path = path_in(home, folder);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let mut text = serde_json::to_string_pretty(record)?;
    text.push('\n');
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The record for `folder`, if there is a readable one. P1's launch reads the file by its fixed format and this is the
/// same reading for any other caller; the tests use it, the command does not.
#[allow(dead_code)]
pub fn read(home: &Path, folder: &Path) -> Option<Record> {
    let text = std::fs::read_to_string(path_in(home, folder)).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// One hook, all of it: the whole of `record-session` apart from reading stdin and the clock. `Ok(path)` when a record
/// was written.
pub fn handle(
    home: &Path,
    hook: &Value,
    project_dir: Option<&str>,
    now: &str,
) -> Result<PathBuf, &'static str> {
    let record = record_of(hook, now)?;
    let folder = folder_of(hook, project_dir).ok_or("no folder")?;
    write(home, &folder, &record).map_err(|_| "the record could not be written")?;
    Ok(path_in(home, &folder))
}

fn time_now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// The command's body, apart from stdin and the environment. Always `Ok`: see the module's doc.
pub fn run(input: &str, home: Option<&Path>, project_dir: Option<&str>) -> std::io::Result<()> {
    let hook: Value =
        serde_json::from_str(input.trim_start_matches('\u{feff}')).unwrap_or(Value::Null);
    if let Some(home) = home {
        let _ = handle(home, &hook, project_dir, &time_now());
    }
    Ok(())
}

/// `herdr memory record-session`.
pub fn cli() -> std::io::Result<()> {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let project_dir = std::env::var(PROJECT_DIR_ENV).ok();
    run(
        &input,
        crate::nirvana::home().as_deref(),
        project_dir.as_deref(),
    )
}

#[cfg(test)]
mod tests;
