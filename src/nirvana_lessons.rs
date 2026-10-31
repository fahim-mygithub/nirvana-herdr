//! Nirvana fork, plan 12 O5: lessons a finished order leaves, once the gate has checked them, and the one line
//! the next order of their class gets in its brief. `herdr lessons for --class C`.
//! (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §7.4.3; old Nirvana decision 0023)
//!
//! The owner's rulings of 2026-10-16, verbatim: "Checked lessions should go live automatically, keep it per github
//! remote assuming that project has a github ... have the lesson stay in its own project. Keep a board per
//! workspace."
//!
//! - **Written only after the gate.** A Work Report's `lesson` (name, class, steps of at most 120 lines, an evidence
//!   quote) becomes `<project>/.nirvana/lessons/<slug>.md` only when its order passed the gate and the quote is in
//!   the worker's own stream, outside the Work Report itself. The Ledger-Lessons research of 2026-10-16 measured 21
//!   of 21 memory writes skipped where the old harness wrote into a folder the repositories did not have; here the
//!   folder is the project's own.
//! - **Any project, never pushed** (plan 33 §3.1, ledger L3). The GitHub-remote condition of 2026-10-16 is gone:
//!   `3D-Printer Flow` has no remote, its workers offered 41 lessons, and 0 were kept. herdr never pushes a lesson.
//! - **Profile scope** (§3.2): a lesson written from an order seated with a profile carries it; an order of class C
//!   and profile P is offered C's lessons with no profile, and P's.
//! - **Origin** (§3.5): `worker` (the gate kept a worker's lesson), `review` (a `lesson-review` order's op), `merge`
//!   (an umbrella of several). `herdr lessons stats` prints the counters per origin, so whether a review's lessons
//!   help is measured, not assumed: Hermes counts views and never outcomes (the 2026-10-30 source review §2h).
//! - **A ledger** (§3.4): every review and merge op appends `ledger.jsonl` with the bodies before and after, a
//!   merged-away file is kept under `.archive/`, and `herdr lessons rollback <id>` restores one op (Hermes'
//!   `skills/.curator_ledger.jsonl` and `hermes curator rollback`).
//! - **One line in a brief, never the body.** Decision 0023: "everything in the prompt is paid for on every call".
//!   The worker reads the file when it wants it.
//! - **Counters** in `.nirvana/lessons/index.json`: offered (a brief listed it), opened (the worker's stream read
//!   its file), passed after opened. A lesson offered often that does not help decays out of the briefs (`live`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::nirvana_run::{self, Project};

/// The folder in `.nirvana/`; tracked with the project (O3's `.gitignore` leaves it out on purpose).
pub const DIR: &str = "lessons";
pub const INDEX: &str = "index.json";
/// §7.4.3: "the steps (at most 120 lines)"; O3's Work Report check holds the same number.
pub const STEPS_LINES: usize = 120;
/// The decay rule, both numbers hypotheses: a lesson stays in the briefs for its first `DECAY_AFTER` offers
/// whatever happens, and after that only while at least one offer in `HELP_EVERY` led to an order that opened it
/// and passed. Six offers is two orders more than the send-back cap's three, so one bad week of a class does not
/// decay a lesson that a later order would have used.
pub const DECAY_AFTER: u32 = 6;
pub const HELP_EVERY: u32 = 3;
/// Plan 34 §2.3: the knowledge cards an order's brief lists, most helpful first, under the same decay rule; the rest
/// get one line naming `herdr kb search`. The number is the plan's, a hypothesis; on the staged reel-editor set of
/// 2026-10-30 the 15 lines and their head came to about 2,900 characters per order (a card line averages 177).
pub const CARDS_OFFERED: usize = 15;
/// Plan 34 §2.4: `herdr kb search` prints at most this many paths.
pub const SEARCH_RESULTS: usize = 20;
/// The orders an index entry remembers having counted, so a re-gated order is not counted twice.
const SEEN_KEPT: usize = 100;
/// The marker of the project's line in `<home>/nirvana/NIRVANA.md`.
const INDEX_MARK: &str = "<!-- herdr-lessons ";
/// §3.4: every review and merge op, one JSON line each, with the bodies before and after.
pub const LEDGER: &str = "ledger.jsonl";
/// §3.4: merged-away and rolled-over files, never deleted.
pub const ARCHIVE: &str = ".archive";
/// The verdict words a journal's `gated` event passes with. `accepted` is the word the gate wrote before O5's
/// outcome words: in `3D-Printer Flow`, 53 of the 72 `gated` events read `accepted` and none reads `pass`.
pub const PASS_WORDS: [&str; 2] = ["pass", "accepted"];
/// Plan 33 review S2: a review op's evidence is at least this long. A review's ops apply with no orchestrator turn, so
/// the quote is the one check left, and a quote of `e` is in every stream. A hypothesis: the shortest quote in the
/// review tests that proves a command's output ("TOTAL 12 passed, 0 failed") is 26.
pub const MIN_EVIDENCE: usize = 20;
/// Plan 33 review B3: every file herdr writes in `lessons/` (not `index.json`), one JSON line each with its sha after
/// the write. Under `runs/`, which O3's `.gitignore` ignores. A review whose folder changed in a way this log does not
/// explain is failed (`unexplained`).
const WRITES: &str = ".lesson-writes.jsonl";

/// §3.5: where a kept lesson came from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// The gate kept a worker's own lesson; every lesson written before plan 33.
    #[default]
    Worker,
    Review,
    Merge,
}

impl Origin {
    pub const ALL: [Self; 3] = [Self::Worker, Self::Review, Self::Merge];

    pub fn word(self) -> &'static str {
        match self {
            Self::Worker => "worker",
            Self::Review => "review",
            Self::Merge => "merge",
        }
    }
}

/// What a write records beside the lesson: the profile its order was seated with, and its origin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Meta<'a> {
    pub profile: Option<&'a str>,
    pub origin: Origin,
}

/// Plan 33 §2.1's profile name, `[a-z0-9-]{1,32}`: anything else in a `Profile:` line is not a profile.
pub fn valid_profile(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counters {
    #[serde(default)]
    pub offered: u32,
    #[serde(default)]
    pub opened: u32,
    #[serde(default)]
    pub passed_after_opened: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub class: String,
    /// The line a brief carries; the lesson's name, which the worker wrote as its title.
    pub description: String,
    pub order: String,
    pub run: String,
    pub written_at: u64,
    #[serde(flatten)]
    pub counters: Counters,
    /// Orders already counted as opening it (the last `SEEN_KEPT`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen: Vec<String>,
    /// Plan 33 §3.2: the profile of the order it was written from; offered only to orders of that profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Plan 33 §3.5. An entry written before it reads as `worker`, which it was.
    #[serde(default)]
    pub origin: Origin,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    #[serde(default = "one")]
    pub v: u32,
    #[serde(default)]
    pub lessons: BTreeMap<String, Entry>,
}

fn one() -> u32 {
    1
}

/// Whether a lesson still goes into briefs (the decay rule above).
pub fn live(counters: &Counters) -> bool {
    counters.offered < DECAY_AFTER || counters.passed_after_opened * HELP_EVERY >= counters.offered
}

pub fn dir(root: &Path) -> PathBuf {
    root.join(nirvana_run::DIR).join(DIR)
}

/// The lock for `index.json`: under `runs/`, which O3's `.gitignore` ignores, so the lock file never shows in
/// the project's `git status` (a lock inside `lessons/` would be tracked).
fn lock(root: &Path) -> std::io::Result<IndexLock> {
    let path = root
        .join(nirvana_run::DIR)
        .join("runs")
        .join(".lessons.lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    file.lock()?;
    Ok(IndexLock(file))
}

struct IndexLock(std::fs::File);

impl Drop for IndexLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub fn read_index(root: &Path) -> Index {
    std::fs::read_to_string(dir(root).join(INDEX))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_index(root: &Path, index: &Index) -> std::io::Result<()> {
    std::fs::create_dir_all(dir(root))?;
    let mut text = serde_json::to_string_pretty(index)?;
    text.push('\n');
    crate::nirvana_seat::write_atomic(&dir(root).join(INDEX), text.as_bytes())
}

/// A file name from a lesson's name: lowercase letters, digits and dashes, at most 48.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(48).collect();
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "lesson".into()
    } else {
        out
    }
}

/// Every string a worker's stream holds that the worker did not write as its Work Report: tool inputs and
/// results, its text. Left out: the run's `result` line, the `StructuredOutput` call that carries the report,
/// and any string holding the report's JSON (agy streams its answer as text): otherwise the lesson's own
/// evidence field would be found in the stream and every quote would pass.
pub fn stream_strings(stream: &str) -> Vec<String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                if !text.contains("\"evidence\"") {
                    out.push(text.replace("\r\n", "\n"));
                }
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            Value::Object(map) => {
                if map.get("type").and_then(Value::as_str) == Some("tool_use")
                    && map.get("name").and_then(Value::as_str) == Some("StructuredOutput")
                {
                    return;
                }
                map.values().for_each(|item| walk(item, out));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for line in stream.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) == Some("result")
            || value.get("event").and_then(Value::as_str) == Some("result")
        {
            continue;
        }
        walk(&value, &mut out);
    }
    out
}

/// §7.4.3: "the quote is in the worker's stream".
pub fn quote_in_stream(stream: &str, quote: &str) -> bool {
    let quote = quote.replace("\r\n", "\n");
    let quote = quote.trim();
    !quote.is_empty()
        && stream_strings(stream)
            .iter()
            .any(|text| text.contains(quote))
}

/// Whether the worker opened `<slug>.md`: a tool call whose input names it. The order's own text names every
/// offered lesson, so a tool result (the order file read back) does not count.
pub fn opened_in_stream(stream: &str, slug: &str) -> bool {
    let file = format!("{slug}.md");
    for line in stream.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        // claude: an assistant message's tool_use blocks.
        if let Some(blocks) = value.pointer("/message/content").and_then(Value::as_array) {
            if value.get("type").and_then(Value::as_str) == Some("assistant")
                && blocks.iter().any(|block| {
                    block.get("type").and_then(Value::as_str) == Some("tool_use")
                        && block["input"].to_string().contains(&file)
                })
            {
                return true;
            }
        }
        // agy: a tool step's parameters.
        if value
            .pointer("/step_update/step_type")
            .and_then(Value::as_str)
            == Some("tool")
            && value
                .pointer("/step_update/tool_info/parameters")
                .is_some_and(|params| params.to_string().contains(&file))
        {
            return true;
        }
    }
    false
}

/// What the gate decided about a Work Report's lesson.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Written(PathBuf),
    /// Checked and kept out, with why: the order did not pass, the quote is not in the stream, a field is missing.
    Refused(String),
}

/// A lesson's four fields, trimmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lesson {
    pub name: String,
    pub class: String,
    pub steps: String,
    pub evidence: String,
}

impl Lesson {
    pub fn from_value(lesson: &Value) -> Result<Self, String> {
        let text = |key: &str| {
            lesson
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_owned()
        };
        let lesson = Self {
            name: text("name"),
            class: text("class"),
            steps: text("steps"),
            evidence: text("evidence"),
        };
        if lesson.name.is_empty()
            || lesson.class.is_empty()
            || lesson.steps.is_empty()
            || lesson.evidence.is_empty()
        {
            return Err("the lesson lacks a name, class, steps or evidence".into());
        }
        if lesson.steps.lines().count() > STEPS_LINES {
            return Err(format!("the steps run over {STEPS_LINES} lines"));
        }
        Ok(lesson)
    }
}

/// The gate's checks, writing nothing: `herdr lessons import --dry-run` runs exactly these.
pub fn check(lesson: &Value, passed: bool, stream: &str) -> Result<Lesson, String> {
    let lesson = Lesson::from_value(lesson)?;
    if !passed {
        return Err("the order did not pass the gate".into());
    }
    if !quote_in_stream(stream, &lesson.evidence) {
        return Err("the evidence quote is not in the worker's stream".into());
    }
    Ok(lesson)
}

/// The gate's check and write. `passed` is the order's gate verdict; `stream` the worker's raw stream.
#[allow(clippy::too_many_arguments)]
pub fn verify_and_write(
    root: &Path,
    lesson: &Value,
    passed: bool,
    stream: &str,
    order: &str,
    run: &str,
    at: u64,
    meta: Meta,
) -> Verdict {
    let lesson = match check(lesson, passed, stream) {
        Ok(lesson) => lesson,
        Err(why) => return Verdict::Refused(why),
    };
    let written = (|| {
        let io = |err: std::io::Error| format!("could not write it: {err}");
        let _lock = lock(root).map_err(io)?;
        let mut index = read_index(root);
        let slug = slug(&lesson.name);
        // Plan 33 review S3: a worker's lesson never overwrites one a review or a merge made, nor one of another class
        // or profile; a merge's rollback would then refuse, and the umbrella would be lost silently.
        if let Some(entry) = index.lessons.get(&slug) {
            if entry.origin != Origin::Worker
                || entry.class != one_line(&lesson.class)
                || entry.profile.as_deref() != meta.profile
            {
                return Err(format!(
                    "a lesson {slug} of origin {}, class {}{} is kept: patch it through a review",
                    entry.origin.word(),
                    entry.class,
                    entry
                        .profile
                        .as_deref()
                        .map(|p| format!(", profile {p}"))
                        .unwrap_or_default()
                ));
            }
        }
        let path = write_locked(
            root,
            &mut index,
            &slug,
            &lesson,
            &Source { order, run, at },
            meta,
        )
        .map_err(io)?;
        write_index(root, &index).map_err(io)?;
        Ok::<_, String>(path)
    })();
    match written {
        Ok(path) => Verdict::Written(path),
        Err(why) => Verdict::Refused(why),
    }
}

pub(crate) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The order and run a write names, and when.
struct Source<'a> {
    order: &'a str,
    run: &'a str,
    at: u64,
}

fn lesson_path(root: &Path, slug: &str) -> PathBuf {
    dir(root).join(format!("{slug}.md"))
}

/// Plan 33 review nit: the evidence goes into a tracked file, and a worker's stream can hold a secret that the
/// owner's own `git push` would publish. Tokens shaped like `sk-...`, `ghp_...`, `github_pat_...`, `Bearer ...`, and
/// any run of over 32 letters and digits holding both (a hex or base64 secret; a sha quoted goes with it), are
/// replaced.
pub fn redact(text: &str) -> String {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<[regex::Regex; 4]> = OnceLock::new();
    let [bearer, sk, github, run] = PATTERNS.get_or_init(|| {
        [
            regex::Regex::new(r"(?i)\bbearer\s+\S{8,}").expect("bearer"),
            regex::Regex::new(r"\bsk-[A-Za-z0-9_-]{16,}").expect("sk"),
            regex::Regex::new(r"\b(?:gh[poursa]_[A-Za-z0-9]{16,}|github_pat_[A-Za-z0-9_]{16,})")
                .expect("github"),
            regex::Regex::new(r"[A-Za-z0-9+/]{33,}={0,2}").expect("run"),
        ]
    });
    let text = bearer.replace_all(text, "Bearer [redacted]");
    let text = sk.replace_all(&text, "[redacted]");
    let text = github.replace_all(&text, "[redacted]");
    run.replace_all(&text, |found: &regex::Captures| {
        let found = &found[0];
        if found.bytes().any(|b| b.is_ascii_digit())
            && found.bytes().any(|b| b.is_ascii_alphabetic())
        {
            "[redacted]".to_owned()
        } else {
            found.to_owned()
        }
    })
    .into_owned()
}

// ---- herdr's own writes in `lessons/` (plan 33 review B3) ----

fn writes_path(root: &Path) -> PathBuf {
    root.join(nirvana_run::DIR).join("runs").join(WRITES)
}

/// A file's path under `dir` (`lessons/`, or plan 34's `knowledge/`), `/`-separated, as the writes log and a snapshot
/// name it.
pub(crate) fn rel_in(dir: &Path, path: &Path) -> String {
    path.strip_prefix(dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn file_sha(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    std::fs::read(path).ok().map(|bytes| {
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    })
}

/// Records that herdr wrote or removed `path`: its sha now, `null` when it is gone.
fn noted(root: &Path, path: &Path) {
    noted_in(&dir(root), &writes_path(root), path);
}

/// `noted` for any folder herdr keeps a writes log of (plan 34: a profile's `knowledge/`).
pub(crate) fn noted_in(dir: &Path, log: &Path, path: &Path) {
    use std::io::Write;
    let line = serde_json::json!({"path": rel_in(dir, path), "sha": file_sha(path)});
    if let Some(parent) = log.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// What a review's gate compares: every file in `lessons/` but `index.json` (its counters move with every order), by
/// sha, and how many lines the writes log held then.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderSnap {
    pub files: BTreeMap<String, String>,
    pub writes: usize,
}

pub fn folder_snap(root: &Path) -> FolderSnap {
    folder_snap_in(&dir(root), &writes_path(root))
}

/// `folder_snap` of any folder and its writes log; the folder's own `index.json` (counters) is left out.
pub(crate) fn folder_snap_in(dir: &Path, log: &Path) -> FolderSnap {
    let mut files = BTreeMap::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(next) = todo.pop() {
        for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                todo.push(path);
            } else if path != dir.join(INDEX) {
                if let Some(sha) = file_sha(&path) {
                    files.insert(rel_in(dir, &path), sha);
                }
            }
        }
    }
    let writes = std::fs::read_to_string(log)
        .map(|text| text.lines().count())
        .unwrap_or(0);
    FolderSnap { files, writes }
}

/// The files of `lessons/` that changed since `before` with no herdr write after it that leaves each as it is now: a
/// reviewer that wrote a lesson itself, around the ops herdr checks.
pub fn unexplained(root: &Path, before: &FolderSnap) -> Vec<String> {
    unexplained_in(&dir(root), &writes_path(root), before)
}

/// `unexplained` for any folder and its writes log.
pub(crate) fn unexplained_in(dir: &Path, log: &Path, before: &FolderSnap) -> Vec<String> {
    let now = folder_snap_in(dir, log);
    let log = std::fs::read_to_string(log).unwrap_or_default();
    let herdr: Vec<(String, Option<String>)> = log
        .lines()
        .skip(before.writes)
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .map(|value| {
            (
                value["path"].as_str().unwrap_or("").to_owned(),
                value["sha"].as_str().map(str::to_owned),
            )
        })
        .collect();
    let mut paths: Vec<&String> = before.files.keys().chain(now.files.keys()).collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter(|path| before.files.get(*path) != now.files.get(*path))
        .filter(|path| {
            let sha = now.files.get(*path).cloned();
            !herdr.iter().any(|(p, s)| p == *path && *s == sha)
        })
        .cloned()
        .collect()
}

/// Writes `<slug>.md` and its index entry; the caller holds the lock and writes the index. A lesson already at the
/// slug keeps its counters: the newer text was checked more recently.
fn write_locked(
    root: &Path,
    index: &mut Index,
    slug: &str,
    lesson: &Lesson,
    source: &Source,
    meta: Meta,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir(root))?;
    let path = lesson_path(root, slug);
    let name_line = one_line(&lesson.name);
    let profile_line = meta
        .profile
        .map(|profile| format!("profile: {profile}\n"))
        .unwrap_or_default();
    let body = format!(
        "---\nname: {name_line}\ndescription: {name_line}\nclass: {}\n{profile_line}order: {}\nrun: {}\nevidence: {}\n---\n\n{}\n",
        one_line(&lesson.class),
        source.order,
        source.run,
        redact(&one_line(&lesson.evidence)),
        lesson.steps.replace("\r\n", "\n").trim_end()
    );
    std::fs::write(&path, body)?;
    noted(root, &path);
    let (counters, seen) = index
        .lessons
        .get(slug)
        .map(|entry| (entry.counters.clone(), entry.seen.clone()))
        .unwrap_or_default();
    index.lessons.insert(
        slug.to_owned(),
        Entry {
            name: name_line.clone(),
            class: one_line(&lesson.class),
            description: name_line,
            order: source.order.to_owned(),
            run: source.run.to_owned(),
            written_at: source.at,
            counters,
            seen,
            profile: meta.profile.map(str::to_owned),
            origin: meta.origin,
        },
    );
    Ok(path)
}

/// A lesson file's text with CRLF read as LF, so a checkout's line endings never change its sha.
fn read_body(root: &Path, slug: &str) -> Option<String> {
    std::fs::read_to_string(lesson_path(root, slug))
        .ok()
        .map(|text| text.replace("\r\n", "\n"))
}

pub fn sha256(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// §3.3: "a body sha", the sha256 of the lesson's file; a patch must name the one it read (read before write).
pub fn body_sha(root: &Path, slug: &str) -> Option<String> {
    read_body(root, slug).map(|text| sha256(&text))
}

/// The lesson's file as a review brief carries it.
pub fn body(root: &Path, slug: &str) -> Option<String> {
    read_body(root, slug)
}

// ---- the ledger: review and merge ops, with rollback (§3.4) ----

/// One lesson as it stood: its file and its index entry, `None` when it did not exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snap {
    pub slug: String,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub entry: Option<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerRow {
    pub v: u32,
    pub id: String,
    pub at: u64,
    /// new, patch, merge or rollback.
    pub op: String,
    /// The order whose Work Report carried the op, or `herdr` for a rollback.
    pub by: String,
    pub run: String,
    pub before: Vec<Snap>,
    pub after: Vec<Snap>,
    /// A rollback: the op it undid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<String>,
}

pub fn ledger(root: &Path) -> Vec<LedgerRow> {
    std::fs::read_to_string(dir(root).join(LEDGER))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line.trim()).ok())
        .collect()
}

fn snap(root: &Path, index: &Index, slug: &str) -> Snap {
    Snap {
        slug: slug.to_owned(),
        file: read_body(root, slug),
        entry: index.lessons.get(slug).cloned(),
    }
}

fn append_ledger(root: &Path, row: &LedgerRow) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(dir(root))?;
    let path = dir(root).join(LEDGER);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let mut line = serde_json::to_string(row)?;
    line.push('\n');
    file.write_all(line.as_bytes())?;
    drop(file);
    noted(root, &path);
    Ok(())
}

/// Plan 33 review S5: the highest `op-<n>` plus one. `ledger()` drops a line it cannot read, so a count of rows would
/// hand a damaged ledger's next op an id it already holds.
fn next_id(rows: &[LedgerRow]) -> String {
    let top = rows
        .iter()
        .filter_map(|row| row.id.strip_prefix("op-")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    format!("op-{}", top + 1)
}

/// Puts each lesson file back as `snaps` hold it; what failed is said, never hidden.
fn restore_files(root: &Path, snaps: &[Snap]) -> Vec<String> {
    let mut failed = Vec::new();
    for was in snaps {
        // A file the failed op never reached is left byte for byte: the snapshot reads CRLF as LF.
        if read_body(root, &was.slug) == was.file {
            continue;
        }
        let path = lesson_path(root, &was.slug);
        let done = match &was.file {
            Some(text) => std::fs::write(&path, text),
            None if path.exists() => std::fs::remove_file(&path),
            None => Ok(()),
        };
        match done {
            Ok(()) => noted(root, &path),
            Err(err) => failed.push(format!("{}: {err}", was.slug)),
        }
    }
    failed
}

/// Runs `change` under the lock and records it: every touched slug before and after, in one ledger row. `slugs` names
/// every lesson `change` may write or remove. When it refuses, or the index or the ledger cannot be written, every
/// one of them is put back as it stood (plan 33 review S5): an op is in the folder and the ledger, or in neither.
/// `change` gets the op's id and returns the slug it made.
fn ledgered(
    root: &Path,
    op: &str,
    by: &str,
    run: &str,
    at: u64,
    slugs: &[String],
    change: impl FnOnce(&mut Index, &str) -> Result<String, String>,
) -> Result<(String, String), String> {
    let _lock = lock(root).map_err(|err| err.to_string())?;
    let was = read_index(root);
    let mut index = was.clone();
    let id = next_id(&ledger(root));
    let mut before: Vec<Snap> = Vec::new();
    for slug in slugs {
        if !before.iter().any(|snap| snap.slug == *slug) {
            before.push(snap(root, &index, slug));
        }
    }
    let undo = |why: String, index_written: bool, before: &[Snap]| -> String {
        let mut failed = restore_files(root, before);
        if index_written {
            if let Err(err) = write_index(root, &was) {
                failed.push(format!("{INDEX}: {err}"));
            }
        }
        if failed.is_empty() {
            why
        } else {
            format!("{why}; and could not put back {}", failed.join(", "))
        }
    };
    let made = match change(&mut index, &id) {
        Ok(made) => made,
        Err(why) => return Err(undo(why, false, &before)),
    };
    if let Err(err) = write_index(root, &index) {
        return Err(undo(err.to_string(), true, &before));
    }
    if !before.iter().any(|snap| snap.slug == made) {
        before.push(Snap {
            slug: made.clone(),
            file: None,
            entry: None,
        });
    }
    let after = before
        .iter()
        .map(|was| snap(root, &index, &was.slug))
        .collect();
    let row = LedgerRow {
        v: 1,
        id: id.clone(),
        at,
        op: op.to_owned(),
        by: by.to_owned(),
        run: run.to_owned(),
        before: before.clone(),
        after,
        of: None,
    };
    if let Err(err) = append_ledger(root, &row) {
        return Err(undo(format!("the ledger: {err}"), true, &before));
    }
    Ok((made, id))
}

/// A review's `new` op: the lesson must not exist yet, since a lesson already kept is patched after it is read
/// (Hermes' read-before-write: `skill_manage` refuses a patch to a skill not viewed in the same review).
pub fn review_new(
    root: &Path,
    lesson: &Lesson,
    by: &str,
    run: &str,
    at: u64,
    profile: Option<&str>,
) -> Result<(String, String), String> {
    let slug = slug(&lesson.name);
    ledgered(
        root,
        "new",
        by,
        run,
        at,
        std::slice::from_ref(&slug),
        |index, _| {
            if index.lessons.contains_key(&slug) || lesson_path(root, &slug).exists() {
                return Err(format!(
                    "a lesson {slug} exists: patch it with its base_sha"
                ));
            }
            write_locked(
                root,
                index,
                &slug,
                lesson,
                &Source { order: by, run, at },
                Meta {
                    profile,
                    origin: Origin::Review,
                },
            )
            .map_err(|err| err.to_string())?;
            Ok(slug.clone())
        },
    )
}

/// A review's `patch` op: `base_sha` must be the lesson's body sha now, and the lesson one the reviewed order is
/// offered (`fits` its class and profile; plan 33 review S1). The slug stays; class, profile and counters carry.
#[allow(clippy::too_many_arguments)]
pub fn review_patch(
    root: &Path,
    slug: &str,
    base_sha: &str,
    lesson: &Lesson,
    scope: (&str, Option<&str>),
    by: &str,
    run: &str,
    at: u64,
) -> Result<String, String> {
    let slugs = [slug.to_owned()];
    ledgered(root, "patch", by, run, at, &slugs, |index, _| {
        let Some(entry) = index.lessons.get(slug).cloned() else {
            return Err(format!("no lesson {slug} to patch"));
        };
        if !fits(&entry, scope.0, scope.1) {
            return Err(format!(
                "{slug} is not a lesson the reviewed order is offered (class {}{})",
                entry.class,
                entry
                    .profile
                    .as_deref()
                    .map(|p| format!(", profile {p}"))
                    .unwrap_or_default()
            ));
        }
        let now_sha = body_sha(root, slug).unwrap_or_default();
        if now_sha != base_sha {
            return Err(format!(
                "base_sha {base_sha} is not {slug}'s body sha {now_sha}: it changed since it was read"
            ));
        }
        let lesson = Lesson {
            class: entry.class.clone(),
            ..lesson.clone()
        };
        write_locked(
            root,
            index,
            slug,
            &lesson,
            &Source { order: by, run, at },
            Meta {
                profile: entry.profile.as_deref(),
                origin: Origin::Review,
            },
        )
        .map_err(|err| err.to_string())?;
        Ok(slug.to_owned())
    })
    .map(|(_, id)| id)
}

/// §3.4: an umbrella of `slugs`, of one class and one profile. Each part's file goes to `.archive/<id>-<slug>.md`
/// and out of the index; the umbrella's counters are the sum of its parts'. `shas` holds the parts' body shas as
/// the merge order's brief showed them: a part changed since is refused. Plan 33 review S5: the umbrella is written
/// before any part is removed, and `ledgered` puts every file back when a step fails.
#[allow(clippy::too_many_arguments)]
pub fn merge(
    root: &Path,
    slugs: &[String],
    name: &str,
    steps: &str,
    shas: &BTreeMap<String, String>,
    by: &str,
    run: &str,
    at: u64,
) -> Result<(String, String), String> {
    let mut parts: Vec<String> = Vec::new();
    for slug in slugs {
        if !parts.contains(slug) {
            parts.push(slug.clone());
        }
    }
    if parts.len() < 2 {
        return Err("a merge names at least two lessons".into());
    }
    let (name, steps) = (name.trim(), steps.trim());
    if name.is_empty() || steps.is_empty() {
        return Err("a merge lacks a name or steps".into());
    }
    if steps.lines().count() > STEPS_LINES {
        return Err(format!("the steps run over {STEPS_LINES} lines"));
    }
    let target = slug(name);
    let mut touched = parts.clone();
    touched.push(target.clone());
    ledgered(root, "merge", by, run, at, &touched, |index, id| {
        let mut entries = Vec::new();
        for part in &parts {
            let Some(entry) = index.lessons.get(part).cloned() else {
                return Err(format!("no lesson {part} to merge"));
            };
            if let Some(sha) = shas.get(part) {
                if body_sha(root, part).as_deref() != Some(sha.as_str()) {
                    return Err(format!("{part} changed since the merge was queued"));
                }
            }
            entries.push(entry);
        }
        let first = entries[0].clone();
        if entries
            .iter()
            .any(|entry| entry.class != first.class || entry.profile != first.profile)
        {
            return Err("a merge's lessons must share one class and one profile".into());
        }
        if !parts.contains(&target) && index.lessons.contains_key(&target) {
            return Err(format!(
                "the umbrella's name {target} is another lesson's: name it apart"
            ));
        }
        // The archive copies first: a copy is never lost, and a failed merge leaves only a spare copy behind.
        let archive = dir(root).join(ARCHIVE);
        std::fs::create_dir_all(&archive).map_err(|err| err.to_string())?;
        let mut counters = Counters::default();
        let mut seen: Vec<String> = Vec::new();
        for (part, entry) in parts.iter().zip(&entries) {
            let copy = archive.join(format!("{id}-{part}.md"));
            std::fs::copy(lesson_path(root, part), &copy)
                .map_err(|err| format!("archive {part}: {err}"))?;
            noted(root, &copy);
            counters.offered += entry.counters.offered;
            counters.opened += entry.counters.opened;
            counters.passed_after_opened += entry.counters.passed_after_opened;
            for order in &entry.seen {
                if !seen.contains(order) {
                    seen.push(order.clone());
                }
            }
        }
        while seen.len() > SEEN_KEPT {
            seen.remove(0);
        }
        let lesson = Lesson {
            name: name.to_owned(),
            class: first.class.clone(),
            steps: steps.to_owned(),
            evidence: format!("merged from {}", parts.join(", ")),
        };
        for part in &parts {
            index.lessons.remove(part);
        }
        write_locked(
            root,
            index,
            &target,
            &lesson,
            &Source { order: by, run, at },
            Meta {
                profile: first.profile.as_deref(),
                origin: Origin::Merge,
            },
        )
        .map_err(|err| format!("the umbrella {target}: {err}"))?;
        if let Some(entry) = index.lessons.get_mut(&target) {
            entry.counters = counters;
            entry.seen = seen;
        }
        for part in parts.iter().filter(|part| **part != target) {
            let from = lesson_path(root, part);
            std::fs::remove_file(&from).map_err(|err| format!("remove {part}: {err}"))?;
            noted(root, &from);
        }
        Ok(target.clone())
    })
}

/// `herdr lessons rollback <id>`: the lessons op `id` touched go back to how they stood before it, when each still
/// stands as the op left it. A lesson changed since (a later op, a later gate) is refused: roll that back first.
/// Counters an umbrella earned after its merge go with it; its parts get back their own.
pub fn rollback(root: &Path, id: &str, at: u64) -> Result<String, String> {
    let _lock = lock(root).map_err(|err| err.to_string())?;
    let rows = ledger(root);
    let Some(row) = rows.iter().find(|row| row.id == id && row.op != "rollback") else {
        return Err(format!(
            "no op {id} in {}",
            dir(root).join(LEDGER).display()
        ));
    };
    if rows.iter().any(|r| r.of.as_deref() == Some(id)) {
        return Err(format!("{id} is rolled back already"));
    }
    // Plan 33 review S6: `ledger.jsonl` is tracked with the project, so a slug read from it names a file only when it
    // is one `slug()` could have made; `../x` would write outside `lessons/`. Checked before any write.
    if let Some(bad) = row
        .before
        .iter()
        .chain(&row.after)
        .find(|snap| slug(&snap.slug) != snap.slug)
    {
        return Err(format!(
            "{id} names {:?}, which is not a lesson slug: the ledger was edited by hand",
            bad.slug
        ));
    }
    let mut index = read_index(root);
    for after in &row.after {
        if read_body(root, &after.slug) != after.file {
            return Err(format!(
                "{} changed since {id}: roll back the later op first",
                after.slug
            ));
        }
    }
    let now: Vec<Snap> = row
        .after
        .iter()
        .map(|after| snap(root, &index, &after.slug))
        .collect();
    for before in &row.before {
        let path = lesson_path(root, &before.slug);
        match &before.file {
            Some(text) => std::fs::write(&path, text).map_err(|err| err.to_string())?,
            None if path.exists() => std::fs::remove_file(&path).map_err(|err| err.to_string())?,
            None => {}
        }
        noted(root, &path);
        match &before.entry {
            Some(entry) => {
                index.lessons.insert(before.slug.clone(), entry.clone());
            }
            None => {
                index.lessons.remove(&before.slug);
            }
        }
    }
    write_index(root, &index).map_err(|err| err.to_string())?;
    let back = next_id(&rows);
    append_ledger(
        root,
        &LedgerRow {
            v: 1,
            id: back.clone(),
            at,
            op: "rollback".into(),
            by: "herdr".into(),
            run: row.run.clone(),
            before: now,
            after: row.before.clone(),
            of: Some(id.to_owned()),
        },
    )
    .map_err(|err| err.to_string())?;
    Ok(format!(
        "{back}: rolled back {id} ({} of {})",
        row.op,
        row.before
            .iter()
            .map(|s| s.slug.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// One brief line per live lesson an order of `class` and `profile` is offered (`fits`): the path from the
/// worker's folder, and the description.
pub fn lines_for(index: &Index, class: &str, profile: Option<&str>) -> Vec<(String, String)> {
    index
        .lessons
        .iter()
        .filter(|(_, entry)| live(&entry.counters) && fits(entry, class, profile))
        .map(|(slug, entry)| {
            (
                slug.clone(),
                format!(
                    "- {}/{DIR}/{slug}.md: {}",
                    nirvana_run::DIR,
                    entry.description
                ),
            )
        })
        .collect()
}

/// §3.2: "lessons of class C with no profile, plus lessons of profile P".
pub fn fits(entry: &Entry, class: &str, profile: Option<&str>) -> bool {
    match entry.profile.as_deref() {
        None => entry.class == class,
        Some(own) => profile == Some(own),
    }
}

/// An order seated with no profile; see `offer_for`.
#[cfg(test)]
pub fn offer(root: &Path, cwd: &Path, class: &str) -> String {
    offer_for(root, cwd, class, None)
}

/// The section to add to a new order's brief, and each offered lesson's `offered` counted. Empty when nothing is
/// live for the class and profile. The files are copied into the worker's folder when it is a worktree, since
/// claude's print-mode file tools stay inside the working folder.
pub fn offer_for(root: &Path, cwd: &Path, class: &str, profile: Option<&str>) -> String {
    let Ok(_lock) = lock(root) else {
        return String::new();
    };
    let mut index = read_index(root);
    let lines = lines_for(&index, class, profile);
    if lines.is_empty() {
        return String::new();
    }
    let of = profile.map_or_else(
        || format!("{class} orders"),
        |profile| format!("{class} and {profile} orders"),
    );
    let mut section = format!(
        "\n\n## Lessons from earlier {of}\n\nEach was checked by herdr's gate. Open one only if it fits this order.\n\n"
    );
    for (slug, line) in &lines {
        if cwd != root {
            let from = lesson_path(root, slug);
            let to = lesson_path(cwd, slug);
            if let Some(parent) = to.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(from, to);
        }
        section.push_str(line);
        section.push('\n');
        if let Some(entry) = index.lessons.get_mut(slug) {
            entry.counters.offered += 1;
        }
    }
    let _ = write_index(root, &index);
    section
}

// ---- measured, not assumed (§3.5) ----

/// Per origin: how many lessons, and their offered, opened and passed after opened summed.
pub fn stats(index: &Index) -> Vec<(Origin, usize, Counters)> {
    Origin::ALL
        .iter()
        .map(|origin| {
            let mut sum = Counters::default();
            let mut count = 0;
            for entry in index.lessons.values().filter(|e| e.origin == *origin) {
                count += 1;
                sum.offered += entry.counters.offered;
                sum.opened += entry.counters.opened;
                sum.passed_after_opened += entry.counters.passed_after_opened;
            }
            (*origin, count, sum)
        })
        .collect()
}

pub fn stats_lines(index: &Index) -> Vec<String> {
    stats(index)
        .into_iter()
        .map(|(origin, count, sum)| {
            format!(
                "{}: {count} lesson(s) · offered {} · opened {} · passed after opened {}",
                origin.word(),
                sum.offered,
                sum.opened,
                sum.passed_after_opened
            )
        })
        .collect()
}

/// The receipt's section; empty when the project keeps no lesson.
pub fn stats_section(root: &Path) -> String {
    let index = read_index(root);
    if index.lessons.is_empty() {
        return String::new();
    }
    let mut text = String::from("\n## Lessons by origin\n\n");
    for line in stats_lines(&index) {
        text.push_str(&format!("- {line}\n"));
    }
    text
}

// ---- recovering the lost ones (§3.6) ----

/// What `herdr lessons import` did, or with `--dry-run` would do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Imported {
    /// `(run, order, slug)` for each lesson written, or that would be.
    pub written: Vec<(String, String, String)>,
    /// `(run, order, name, why)` for each refused.
    pub refused: Vec<(String, String, String, String)>,
}

impl Imported {
    /// The lessons kept: two orders offering one lesson write one file.
    pub fn distinct(&self) -> usize {
        self.written
            .iter()
            .map(|(_, _, slug)| slug.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }
}

/// What one order of a journal says, read as raw JSON lines: a journal written before one of today's fields
/// existed may not replay through today's `Event`, and the import must read every run the project kept.
#[derive(Default)]
struct Told {
    lesson: Option<Value>,
    verdict: Option<String>,
    order_file: Option<String>,
    class: Option<String>,
}

fn told(journal: &str) -> BTreeMap<String, Told> {
    let mut orders: BTreeMap<String, Told> = BTreeMap::new();
    for line in journal.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let Some(order) = value.get("order").and_then(Value::as_str) else {
            continue;
        };
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let entry = orders.entry(order.to_owned()).or_default();
        match value.get("event").and_then(Value::as_str) {
            Some("seated") => {
                entry.order_file = text("order_file");
                entry.class = text("class");
            }
            Some("delivered") => entry.lesson = value.pointer("/report/lesson").cloned(),
            Some("gated") => entry.verdict = text("verdict"),
            _ => {}
        }
    }
    orders
}

/// The `Profile:` line of an order's copy (the gate's line list), when it names a valid profile.
pub fn order_profile(text: &str) -> Option<String> {
    crate::nirvana_gate::spec(text).profile
}

/// §3.6: every lesson a worker offered on an order its journal shows gated pass, through the gate's own checks.
/// `dry` checks and writes nothing: no folder, no lock file, no index.
pub fn import(project: &Project, run: Option<&str>, dry: bool, at: u64) -> Imported {
    let runs = match run {
        Some(run) => vec![run.to_owned()],
        None => project.runs(),
    };
    let mut out = Imported::default();
    // Plan 33 review S3: a slug already kept, or kept earlier in this pass, is skipped, so a second import changes
    // nothing and never overwrites a lesson a review patched or a merge made with the worker's first text.
    let mut kept: std::collections::BTreeSet<String> =
        read_index(&project.root).lessons.into_keys().collect();
    for run in runs {
        let journal = std::fs::read_to_string(project.run_dir(&run).join("journal.jsonl"))
            .unwrap_or_default();
        for (order, said) in told(&journal) {
            let Some(lesson) = said.lesson else {
                continue;
            };
            // Plan 33 §3.3 and review S4: "The reviewer's own lesson, if any, is ignored."
            if said.class.as_deref() == Some(crate::nirvana_review::CLASS) {
                continue;
            }
            let name = lesson
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let Some(verdict) = said.verdict else {
                out.refused
                    .push((run.clone(), order, name, "the order was never gated".into()));
                continue;
            };
            let passed = PASS_WORDS.contains(&verdict.as_str());
            let stream = std::fs::read_to_string(
                project
                    .run_dir(&run)
                    .join("streams")
                    .join(format!("{order}.jsonl")),
            )
            .unwrap_or_default();
            let profile = said
                .order_file
                .and_then(|file| std::fs::read_to_string(file).ok())
                .and_then(|text| order_profile(&text));
            let checked = check(&lesson, passed, &stream).map(|lesson| slug(&lesson.name));
            let result =
                if let Some(slug) = checked.as_ref().ok().filter(|slug| kept.contains(*slug)) {
                    Err(format!("a lesson {slug} is kept already"))
                } else if dry {
                    checked
                } else {
                    match verify_and_write(
                        &project.root,
                        &lesson,
                        passed,
                        &stream,
                        &order,
                        &run,
                        at,
                        Meta {
                            profile: profile.as_deref(),
                            origin: Origin::Worker,
                        },
                    ) {
                        Verdict::Written(_) => Ok(slug(&name)),
                        Verdict::Refused(why) => Err(why),
                    }
                };
            match result {
                Ok(slug) => {
                    kept.insert(slug.clone());
                    out.written.push((run.clone(), order, slug));
                }
                Err(why) if !passed && why == "the order did not pass the gate" => {
                    out.refused.push((
                        run.clone(),
                        order,
                        name,
                        format!("the order was gated {verdict}"),
                    ))
                }
                Err(why) => out.refused.push((run.clone(), order, name, why)),
            }
        }
    }
    out
}

/// The gate's count at an order's verdict: each lesson the worker opened gains `opened`, and
/// `passed_after_opened` when the order passed. Once per order.
pub fn note_gated(
    root: &Path,
    stream: &str,
    order: &str,
    passed: bool,
) -> std::io::Result<Vec<String>> {
    let _lock = lock(root)?;
    let mut index = read_index(root);
    let mut counted = Vec::new();
    for (slug, entry) in index.lessons.iter_mut() {
        if entry.seen.iter().any(|seen| seen == order) || !opened_in_stream(stream, slug) {
            continue;
        }
        entry.counters.opened += 1;
        if passed {
            entry.counters.passed_after_opened += 1;
        }
        entry.seen.push(order.to_owned());
        if entry.seen.len() > SEEN_KEPT {
            entry.seen.remove(0);
        }
        counted.push(slug.clone());
    }
    if !counted.is_empty() {
        write_index(root, &index)?;
    }
    Ok(counted)
}

/// The project's line in `<home>/nirvana/NIRVANA.md`, which the orchestrator's SessionStart digest reads (O4b).
/// One line per project, replaced in place; nothing else in the file is touched.
pub fn index_line_text(root: &Path, live_count: usize) -> String {
    let root = root.display();
    format!(
        "- lessons for {root}: {live_count} live in {root}{sep}{}{sep}{DIR}; `herdr lessons for --class <class>` lists a class's {INDEX_MARK}{root} -->",
        nirvana_run::DIR,
        sep = std::path::MAIN_SEPARATOR
    )
}

pub fn write_index_line(home: &Path, root: &Path) -> std::io::Result<PathBuf> {
    let index = read_index(root);
    let count = index
        .lessons
        .values()
        .filter(|entry| live(&entry.counters))
        .count();
    let path = home.join(crate::nirvana_seat::DIR).join("NIRVANA.md");
    // A new file gets O4b's head, as its board line's writer gives it: either may write first.
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| crate::nirvana_claude::MEMORY_HEAD.to_owned());
    let mark = format!("{INDEX_MARK}{} -->", root.display());
    let line = index_line_text(root, count);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    match lines.iter().position(|existing| existing.ends_with(&mark)) {
        Some(at) => lines[at] = line,
        None => lines.push(line),
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = lines.join("\n");
    out.push('\n');
    crate::nirvana_seat::write_atomic(&path, out.as_bytes())?;
    Ok(path)
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr lessons for --class <class> [--all]   one line per lesson: its path and description
       herdr lessons list [--json]                  every lesson with its counters
       herdr lessons stats                          per origin (worker, review, merge): offered, opened, passed after
       herdr lessons import [--run <run>] [--dry-run]  keep the lessons of gated-pass orders, through the gate's checks
       herdr lessons merge [--class <class>] [--run <run>]  queue an umbrella review of a class over 8 live lessons
       herdr lessons rollback <op-id>               undo one review or merge op from ledger.jsonl";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let project = Project::from_env();
    let root = project.root.clone();
    match args.first().map(String::as_str) {
        Some("for") => {
            let mut class = None;
            let mut all = false;
            let mut iter = args[1..].iter();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--class" => class = iter.next().cloned(),
                    "--all" => all = true,
                    _ => usage_exit(),
                }
            }
            let Some(class) = class else { usage_exit() };
            let index = read_index(&root);
            for (slug, entry) in &index.lessons {
                if entry.class != class {
                    continue;
                }
                let is_live = live(&entry.counters);
                if !is_live && !all {
                    continue;
                }
                println!(
                    "{}: {}{}",
                    dir(&root).join(format!("{slug}.md")).display(),
                    entry.description,
                    if is_live { "" } else { " (decayed)" }
                );
            }
            Ok(())
        }
        Some("list") => {
            let index = read_index(&root);
            if args.get(1).map(String::as_str) == Some("--json") {
                println!("{}", serde_json::to_string_pretty(&index)?);
                return Ok(());
            }
            if index.lessons.is_empty() {
                println!("no lesson in {}", dir(&root).display());
            }
            for (slug, entry) in &index.lessons {
                println!(
                    "{slug} · {} · offered {} · opened {} · passed after opened {}{}",
                    entry.class,
                    entry.counters.offered,
                    entry.counters.opened,
                    entry.counters.passed_after_opened,
                    if live(&entry.counters) {
                        ""
                    } else {
                        " · decayed"
                    }
                );
            }
            Ok(())
        }
        Some("stats") => {
            let index = read_index(&root);
            for line in stats_lines(&index) {
                println!("{line}");
            }
            Ok(())
        }
        Some("import") => {
            let mut run = None;
            let mut dry = false;
            let mut iter = args[1..].iter();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--run" => run = iter.next().cloned(),
                    "--dry-run" => dry = true,
                    _ => usage_exit(),
                }
            }
            if run
                .as_deref()
                .is_some_and(|run| !nirvana_run::valid_id(run))
            {
                usage_exit();
            }
            let done = import(&project, run.as_deref(), dry, nirvana_run::now());
            print!("{}", import_report(&done, dry));
            if !dry && !done.written.is_empty() {
                if let Some(home) = crate::nirvana::home() {
                    let _ = write_index_line(&home, &root);
                }
            }
            Ok(())
        }
        Some("merge") => crate::nirvana_review::merge_cli(&project, &args[1..]),
        Some("rollback") => {
            let Some(id) = args.get(1) else { usage_exit() };
            match rollback(&root, id, nirvana_run::now()) {
                Ok(line) => {
                    println!("{line}");
                    Ok(())
                }
                Err(why) => {
                    eprintln!("herdr lessons rollback: {why}");
                    std::process::exit(1);
                }
            }
        }
        _ => usage_exit(),
    }
}

/// The import's counts, then each refusal reason with its count, then each refused order.
pub fn import_report(done: &Imported, dry: bool) -> String {
    let verb = if dry { "would write" } else { "wrote" };
    let mut text = format!(
        "{verb} {} lesson(s) from {} order(s); refused {}\n",
        done.distinct(),
        done.written.len(),
        done.refused.len()
    );
    let mut reasons: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, _, _, why) in &done.refused {
        *reasons.entry(why.as_str()).or_default() += 1;
    }
    for (why, count) in reasons {
        text.push_str(&format!("  {count} refused: {why}\n"));
    }
    for (run, order, name, why) in &done.refused {
        text.push_str(&format!("  - {run}/{order} {name:?}: {why}\n"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "herdr-o5-lessons-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn git(root: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A claude stream: the worker ran a command whose output holds the quote, then delivered a report whose
    /// lesson quotes it (the report repeats the quote in StructuredOutput and in the result line).
    fn stream(output: &str, quote: &str) -> String {
        let lesson = serde_json::json!({"name":"Run the suite headless","class":"build.bounded","steps":"1. run it","evidence":quote});
        [
            serde_json::json!({"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","name":"Bash","input":{"command":"godot --headless -s run_tests.gd"}}]}}),
            serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","content":output}]}}),
            serde_json::json!({"type":"assistant","message":{"id":"m2","content":[{"type":"tool_use","name":"StructuredOutput","input":{"status":"done","lesson":lesson}}]}}),
            serde_json::json!({"type":"result","subtype":"success","structured_output":{"status":"done","lesson":lesson}}),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
    }

    fn lesson(quote: &str) -> Value {
        serde_json::json!({"name":"Run the suite headless","class":"build.bounded","steps":"1. godot --headless -s run_tests.gd\n2. read TOTAL","evidence":quote})
    }

    #[test]
    fn a_quote_counts_only_from_the_stream_outside_the_work_report() {
        let text = stream("=== TOTAL 12 passed, 0 failed", "TOTAL 12 passed");
        assert!(quote_in_stream(&text, "TOTAL 12 passed"));
        // The report's own lesson carries this quote, in StructuredOutput and in the result: not evidence.
        let text = stream("nothing useful", "TOTAL 99 passed");
        assert!(!quote_in_stream(&text, "TOTAL 99 passed"));
        assert!(!quote_in_stream(&text, "   "));
    }

    /// Plan 33 §3.1 (L3): a folder with no git remote, and no git at all, keeps a checked lesson.
    #[test]
    fn p33_a_lesson_is_written_after_a_pass_in_a_project_with_no_remote() {
        let root = temp("write");
        git(&root, &["init", "-q"]);
        let remotes = std::process::Command::new("git")
            .args(["remote", "-v"])
            .current_dir(&root)
            .output()
            .expect("git remote");
        assert!(remotes.stdout.is_empty(), "the fixture has no remote");
        let text = stream("=== TOTAL 12 passed", "TOTAL 12 passed");
        let l = lesson("TOTAL 12 passed");
        assert_eq!(
            verify_and_write(&root, &l, false, &text, "o1", "r1", 5, Meta::default()),
            Verdict::Refused("the order did not pass the gate".into())
        );
        assert!(matches!(
            verify_and_write(&root, &lesson("not in it"), true, &text, "o1", "r1", 5, Meta::default()),
            Verdict::Refused(why) if why.contains("not in the worker's stream")
        ));
        assert!(!dir(&root).join("run-the-suite-headless.md").exists());
        let Verdict::Written(path) =
            verify_and_write(&root, &l, true, &text, "o1", "r1", 5, Meta::default())
        else {
            panic!("written")
        };
        assert_eq!(path, dir(&root).join("run-the-suite-headless.md"));
        let body = std::fs::read_to_string(&path).expect("body");
        assert!(
            body.starts_with("---\nname: Run the suite headless\n"),
            "{body}"
        );
        assert!(body.contains("\nclass: build.bounded\n") && body.ends_with("2. read TOTAL\n"));
        let index = read_index(&root);
        assert_eq!(index.lessons["run-the-suite-headless"].order, "o1");
    }

    #[test]
    fn steps_over_120_lines_are_refused() {
        let root = temp("long");
        let mut l = lesson("TOTAL 12 passed");
        l["steps"] = Value::String("step\n".repeat(121));
        let text = stream("TOTAL 12 passed", "TOTAL 12 passed");
        assert!(matches!(
            verify_and_write(&root, &l, true, &text, "o1", "r1", 5, Meta::default()),
            Verdict::Refused(why) if why.contains("120")
        ));
    }

    #[test]
    fn the_brief_gets_one_line_per_live_lesson_and_counts_the_offer() {
        let root = temp("brief");
        let text = stream("TOTAL 12 passed", "TOTAL 12 passed");
        verify_and_write(
            &root,
            &lesson("TOTAL 12 passed"),
            true,
            &text,
            "o1",
            "r1",
            5,
            Meta::default(),
        );
        assert_eq!(offer(&root, &root, "chore.script"), "");
        let section = offer(&root, &root, "build.bounded");
        assert!(
            section.ends_with(
                "\n- .nirvana/lessons/run-the-suite-headless.md: Run the suite headless\n"
            ),
            "{section:?}"
        );
        // The line names the path and the description, never the body.
        assert!(!section.contains("godot --headless"));
        assert_eq!(
            read_index(&root).lessons["run-the-suite-headless"]
                .counters
                .offered,
            1
        );
        // A worktree gets its own copy, where the worker's file tools reach.
        let worktree = root.join("wt");
        offer(&root, &worktree, "build.bounded");
        assert!(dir(&worktree).join("run-the-suite-headless.md").is_file());
    }

    #[test]
    fn opened_and_passed_after_opened_count_once_per_order() {
        let root = temp("count");
        let text = stream("TOTAL 12 passed", "TOTAL 12 passed");
        verify_and_write(
            &root,
            &lesson("TOTAL 12 passed"),
            true,
            &text,
            "o1",
            "r1",
            5,
            Meta::default(),
        );
        let read = serde_json::json!({"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","name":"Read","input":{"file_path":".nirvana/lessons/run-the-suite-headless.md"}}]}}).to_string();
        // The order file read back names the lesson in a tool result: not an open.
        let named = serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","content":"- .nirvana/lessons/run-the-suite-headless.md: Run"}]}}).to_string();
        assert_eq!(
            note_gated(&root, &named, "o2", true).expect("note"),
            Vec::<String>::new()
        );
        assert_eq!(
            note_gated(&root, &read, "o2", true).expect("note"),
            vec!["run-the-suite-headless".to_owned()]
        );
        assert!(note_gated(&root, &read, "o2", true)
            .expect("again")
            .is_empty());
        note_gated(&root, &read, "o3", false).expect("o3");
        let c = &read_index(&root).lessons["run-the-suite-headless"].counters;
        assert_eq!((c.opened, c.passed_after_opened), (2, 1));
    }

    #[test]
    fn a_lesson_offered_often_that_does_not_help_decays_out_of_the_briefs() {
        let c = |offered, passed| Counters {
            offered,
            opened: passed,
            passed_after_opened: passed,
        };
        assert!(live(&c(5, 0)));
        assert!(!live(&c(6, 0)));
        assert!(live(&c(6, 2)));
        assert!(!live(&c(9, 2)));
        assert!(live(&c(9, 3)));
        let mut index = Index::default();
        index.lessons.insert(
            "x".into(),
            Entry {
                name: "X".into(),
                class: "build.bounded".into(),
                description: "X".into(),
                order: "o".into(),
                run: "r".into(),
                written_at: 0,
                counters: c(6, 0),
                seen: Vec::new(),
                profile: None,
                origin: Origin::Worker,
            },
        );
        assert!(lines_for(&index, "build.bounded", None).is_empty());
    }

    #[test]
    fn nirvana_md_gets_one_line_per_project_replaced_in_place() {
        let home = temp("home");
        let root = temp("proj");
        let path = home.join("nirvana").join("NIRVANA.md");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(&path, "# Nirvana\n\nthe owner's line\n").expect("seed");
        write_index_line(&home, &root).expect("first");
        write_index_line(&home, &root).expect("second");
        let text = std::fs::read_to_string(&path).expect("text");
        assert!(
            text.starts_with("# Nirvana\n\nthe owner's line\n- lessons for "),
            "{text}"
        );
        assert_eq!(text.matches(INDEX_MARK).count(), 1);
        assert!(text.contains(": 0 live in "));
    }

    /// Plan 33 §3.2: class C with no profile, plus profile P's own; a profile's lesson never reaches another order.
    #[test]
    fn p33_offer_gives_the_class_lessons_with_no_profile_and_the_profiles_own() {
        let root = temp("profile");
        let write = |name: &str, class: &str, profile: Option<&str>| {
            let quote = format!("{name} proof");
            let text = stream(&quote, &quote);
            let mut l = lesson(&quote);
            l["name"] = Value::String(name.into());
            l["class"] = Value::String(class.into());
            let meta = Meta {
                profile,
                origin: Origin::Worker,
            };
            assert!(matches!(
                verify_and_write(&root, &l, true, &text, "o", "r", 5, meta),
                Verdict::Written(_)
            ));
        };
        write("Plain build", "build.bounded", None);
        write("Slicer gui", "ops.terminal", Some("slicer"));
        write("Other ops", "ops.terminal", None);
        let section = offer_for(&root, &root, "build.bounded", Some("slicer"));
        assert!(
            section.contains("plain-build.md") && section.contains("slicer-gui.md"),
            "{section}"
        );
        assert!(
            !section.contains("other-ops.md"),
            "another class: {section}"
        );
        assert!(
            section.contains("build.bounded and slicer orders"),
            "{section}"
        );
        let plain = offer(&root, &root, "ops.terminal");
        assert!(
            plain.contains("other-ops.md") && !plain.contains("slicer-gui.md"),
            "{plain}"
        );
        let index = read_index(&root);
        assert_eq!(
            index.lessons["slicer-gui"].profile.as_deref(),
            Some("slicer")
        );
        let body = std::fs::read_to_string(dir(&root).join("slicer-gui.md")).expect("body");
        assert!(body.contains("\nprofile: slicer\n"), "{body}");
        assert_eq!(index.lessons["slicer-gui"].counters.offered, 1);
    }

    #[test]
    fn p33_stats_count_per_origin() {
        let mut index = Index::default();
        let entry = |origin, offered, passed| Entry {
            name: "x".into(),
            class: "c".into(),
            description: "x".into(),
            order: "o".into(),
            run: "r".into(),
            written_at: 0,
            counters: Counters {
                offered,
                opened: passed,
                passed_after_opened: passed,
            },
            seen: Vec::new(),
            profile: None,
            origin,
        };
        index
            .lessons
            .insert("a".into(), entry(Origin::Worker, 4, 1));
        index
            .lessons
            .insert("b".into(), entry(Origin::Review, 2, 2));
        index
            .lessons
            .insert("c".into(), entry(Origin::Review, 1, 0));
        assert_eq!(
            stats_lines(&index),
            [
                "worker: 1 lesson(s) · offered 4 · opened 1 · passed after opened 1",
                "review: 2 lesson(s) · offered 3 · opened 2 · passed after opened 2",
                "merge: 0 lesson(s) · offered 0 · opened 0 · passed after opened 0",
            ]
        );
        // An index written before plan 33 reads every lesson as a worker's.
        let old: Entry = serde_json::from_str(
            r#"{"name":"x","class":"c","description":"x","order":"o","run":"r","written_at":0,"offered":1}"#,
        )
        .expect("old entry");
        assert_eq!((old.origin, old.profile), (Origin::Worker, None));
    }

    /// §3.6 on a fixture: a run with an order gated `accepted` (the pre-O5 word), one `sent_back`, one never gated,
    /// and one whose quote is only in its own report. The dry pass writes nothing at all.
    #[test]
    fn p33_import_keeps_only_gated_pass_lessons_and_the_dry_run_writes_nothing() {
        let root = temp("import");
        let project = Project::at(&root);
        let run_dir = project.run_dir("old");
        std::fs::create_dir_all(run_dir.join("streams")).expect("streams");
        let order_copy = root.join("passed.md");
        std::fs::write(&order_copy, "# do it\nProfile: slicer\n").expect("copy");
        let delivered = |order: &str, quote: &str| {
            serde_json::json!({"v":1,"seq":2,"at":2,"event":"delivered","order":order,
                "report":{"status":"done","summary":"s","files":[],"tests":[],"gotchas":[],
                "lesson":{"name":format!("Lesson of {order}"),"class":"build.bounded","steps":"1. x","evidence":quote}}})
        };
        let lines = [
            serde_json::json!({"v":1,"seq":1,"at":1,"event":"seated","order":"passed","order_file":order_copy.display().to_string()}),
            delivered("passed", "TOTAL 12 passed"),
            serde_json::json!({"v":1,"seq":3,"at":3,"event":"gated","order":"passed","verdict":"accepted","reason":"ok"}),
            delivered("failed", "TOTAL 0 passed"),
            serde_json::json!({"v":1,"seq":5,"at":5,"event":"gated","order":"failed","verdict":"sent_back"}),
            delivered("ungated", "TOTAL 1 passed"),
            delivered("unproved", "TOTAL 7 passed"),
            serde_json::json!({"v":1,"seq":8,"at":8,"event":"gated","order":"unproved","verdict":"pass"}),
        ];
        let journal: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(run_dir.join("journal.jsonl"), journal.join("\n")).expect("journal");
        for (order, output) in [
            ("passed", "TOTAL 12 passed"),
            ("failed", "TOTAL 0 passed"),
            ("ungated", "TOTAL 1 passed"),
            ("unproved", "nothing"),
        ] {
            std::fs::write(
                run_dir.join("streams").join(format!("{order}.jsonl")),
                stream(output, "unused"),
            )
            .expect("stream");
        }
        let listing = |dir: &Path| -> Vec<PathBuf> {
            let mut out = Vec::new();
            let mut todo = vec![dir.to_path_buf()];
            while let Some(next) = todo.pop() {
                for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
                    if entry.path().is_dir() {
                        todo.push(entry.path());
                    }
                    out.push(entry.path());
                }
            }
            out.sort();
            out
        };
        let before = listing(&root);
        let dry = import(&project, None, true, 9);
        assert_eq!(listing(&root), before, "the dry run wrote nothing");
        assert_eq!(
            dry.written,
            vec![(
                "old".to_owned(),
                "passed".to_owned(),
                "lesson-of-passed".to_owned()
            )]
        );
        let why: BTreeMap<&str, &str> = dry
            .refused
            .iter()
            .map(|(_, order, _, why)| (order.as_str(), why.as_str()))
            .collect();
        assert_eq!(why["failed"], "the order was gated sent_back");
        assert_eq!(why["ungated"], "the order was never gated");
        assert_eq!(
            why["unproved"],
            "the evidence quote is not in the worker's stream"
        );
        let report = import_report(&dry, true);
        assert!(
            report.starts_with("would write 1 lesson(s) from 1 order(s); refused 3\n"),
            "{report}"
        );
        let real = import(&project, Some("old"), false, 9);
        assert_eq!(real.written, dry.written);
        let index = read_index(&root);
        assert_eq!(index.lessons.len(), 1);
        let entry = &index.lessons["lesson-of-passed"];
        assert_eq!(
            (entry.origin, entry.profile.as_deref()),
            (Origin::Worker, Some("slicer"))
        );
    }

    /// Review S3: a worker's lesson never overwrites a review's or a merge's, nor one of another class or profile.
    #[test]
    fn p33_a_worker_lesson_never_overwrites_a_review_or_another_scope() {
        let root = temp("clobber");
        let text = stream("=== TOTAL 12 passed", "TOTAL 12 passed");
        let l = lesson("TOTAL 12 passed");
        assert!(matches!(
            verify_and_write(&root, &l, true, &text, "o1", "r1", 5, Meta::default()),
            Verdict::Written(_)
        ));
        // The same slug again from a worker of the same class and no profile: written, as before plan 33.
        assert!(matches!(
            verify_and_write(&root, &l, true, &text, "o2", "r1", 6, Meta::default()),
            Verdict::Written(_)
        ));
        let profiled = Meta {
            profile: Some("slicer"),
            origin: Origin::Worker,
        };
        assert_eq!(
            verify_and_write(&root, &l, true, &text, "o3", "r1", 7, profiled),
            Verdict::Refused(
                "a lesson run-the-suite-headless of origin worker, class build.bounded is kept: patch it through a review"
                    .into()
            )
        );
        let mut other = l.clone();
        other["class"] = "ops.terminal".into();
        assert!(matches!(
            verify_and_write(&root, &other, true, &text, "o4", "r1", 8, Meta::default()),
            Verdict::Refused(why) if why.ends_with("patch it through a review")
        ));
        let mut index = read_index(&root);
        index
            .lessons
            .get_mut("run-the-suite-headless")
            .expect("entry")
            .origin = Origin::Merge;
        write_index(&root, &index).expect("index");
        let before = body(&root, "run-the-suite-headless");
        assert!(matches!(
            verify_and_write(&root, &l, true, &text, "o5", "r1", 9, Meta::default()),
            Verdict::Refused(why) if why.contains("of origin merge")
        ));
        assert_eq!(body(&root, "run-the-suite-headless"), before);
        assert_eq!(
            read_index(&root).lessons["run-the-suite-headless"].order,
            "o2"
        );
    }

    /// Review S3 and S4: a second import changes nothing, and a lesson-review order's own lesson is never imported.
    #[test]
    fn p33_a_second_import_changes_nothing_and_skips_the_reviewers_lesson() {
        let root = temp("import-twice");
        let project = Project::at(&root);
        let run_dir = project.run_dir("old");
        std::fs::create_dir_all(run_dir.join("streams")).expect("streams");
        let delivered = |order: &str| {
            serde_json::json!({"v":1,"seq":2,"at":2,"event":"delivered","order":order,
                "report":{"status":"done","summary":"s","files":[],"tests":[],"gotchas":[],
                "lesson":{"name":format!("Lesson of {order}"),"class":"build.bounded","steps":"1. x","evidence":"TOTAL 12 passed"}}})
        };
        let gated = |order: &str| serde_json::json!({"v":1,"seq":3,"at":3,"event":"gated","order":order,"verdict":"pass"});
        let lines = [
            serde_json::json!({"v":1,"seq":1,"at":1,"event":"seated","order":"w1","class":"build.bounded"}),
            delivered("w1"),
            gated("w1"),
            serde_json::json!({"v":1,"seq":1,"at":1,"event":"seated","order":"w1-lr","class":"lesson-review"}),
            delivered("w1-lr"),
            gated("w1-lr"),
        ];
        let journal: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(run_dir.join("journal.jsonl"), journal.join("\n")).expect("journal");
        for order in ["w1", "w1-lr"] {
            std::fs::write(
                run_dir.join("streams").join(format!("{order}.jsonl")),
                stream("TOTAL 12 passed", "unused"),
            )
            .expect("stream");
        }
        let first = import(&project, None, false, 9);
        assert_eq!(
            first.written,
            vec![("old".to_owned(), "w1".to_owned(), "lesson-of-w1".to_owned())]
        );
        assert!(first.refused.is_empty(), "{:?}", first.refused);
        assert!(!read_index(&root).lessons.contains_key("lesson-of-w1-lr"));
        // A review patches it; a second import keeps the patch.
        std::fs::write(dir(&root).join("lesson-of-w1.md"), "patched\n").expect("patch");
        let second = import(&project, None, false, 10);
        assert!(second.written.is_empty(), "{:?}", second.written);
        assert_eq!(second.refused[0].3, "a lesson lesson-of-w1 is kept already");
        assert_eq!(body(&root, "lesson-of-w1").as_deref(), Some("patched\n"));
        assert_eq!(import(&project, None, true, 11).written, Vec::new());
    }

    fn kept_lesson(root: &Path, name: &str) -> String {
        let text = stream(&format!("{name} proof"), "unused");
        let mut l = lesson(&format!("{name} proof"));
        l["name"] = name.into();
        assert!(matches!(
            verify_and_write(root, &l, true, &text, "o", "r", 5, Meta::default()),
            Verdict::Written(_)
        ));
        slug(name)
    }

    /// Review S5: a merge whose umbrella cannot be written leaves every part where it was, the index as it was and no
    /// ledger row; ids are the highest plus one, past a damaged line.
    #[test]
    fn p33_a_failed_merge_leaves_the_folder_as_it_was_and_ids_never_collide() {
        let root = temp("merge-fail");
        let parts = vec![kept_lesson(&root, "Tip one"), kept_lesson(&root, "Tip two")];
        let index_before = read_index(&root);
        // The umbrella's path is a folder: its write fails.
        std::fs::create_dir_all(dir(&root).join("tips.md")).expect("folder");
        let why = merge(
            &root,
            &parts,
            "Tips",
            "1. both",
            &BTreeMap::new(),
            "m",
            "r",
            9,
        )
        .expect_err("the umbrella cannot be written");
        assert!(why.starts_with("the umbrella tips: "), "{why}");
        for part in &parts {
            assert!(lesson_path(&root, part).is_file(), "{part} is kept");
        }
        assert_eq!(read_index(&root), index_before);
        assert!(ledger(&root).is_empty());
        std::fs::remove_dir(dir(&root).join("tips.md")).expect("folder");
        // A ledger holding op-5 and a damaged line: the next op is op-6.
        std::fs::write(
            dir(&root).join(LEDGER),
            format!(
                "{}\nnot json\n",
                serde_json::to_string(&LedgerRow {
                    v: 1,
                    id: "op-5".into(),
                    at: 1,
                    op: "new".into(),
                    by: "x".into(),
                    run: "r".into(),
                    before: Vec::new(),
                    after: Vec::new(),
                    of: None,
                })
                .expect("row")
            ),
        )
        .expect("ledger");
        let (made, id) = merge(
            &root,
            &parts,
            "Tips",
            "1. both",
            &BTreeMap::new(),
            "m",
            "r",
            10,
        )
        .expect("merge");
        assert_eq!((made.as_str(), id.as_str()), ("tips", "op-6"));
        assert!(!lesson_path(&root, &parts[0]).exists());
        assert!(dir(&root).join(ARCHIVE).join("op-6-tip-one.md").is_file());
    }

    /// Review S5: a merge that fails after it removed a part (the second part's file is held open, shared for reading only)
    /// puts the removed part back and takes the umbrella out.
    #[cfg(windows)]
    #[test]
    fn p33_a_merge_that_fails_midway_puts_every_file_back() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = temp("merge-midway");
        let parts = vec![kept_lesson(&root, "Tip one"), kept_lesson(&root, "Tip two")];
        let index_before = read_index(&root);
        let first = body(&root, &parts[0]);
        let held = std::fs::OpenOptions::new()
            .read(true)
            // FILE_SHARE_READ alone: the archive copy reads it, the removal is refused.
            .share_mode(1)
            .open(lesson_path(&root, &parts[1]))
            .expect("hold");
        let why = merge(
            &root,
            &parts,
            "Tips",
            "1. both",
            &BTreeMap::new(),
            "m",
            "r",
            9,
        )
        .expect_err("the second part cannot be removed");
        drop(held);
        assert!(why.starts_with("remove tip-two: "), "{why}");
        assert_eq!(body(&root, &parts[0]), first, "the removed part is back");
        assert!(!lesson_path(&root, "tips").exists(), "the umbrella is out");
        assert_eq!(read_index(&root), index_before);
        assert!(ledger(&root).is_empty());
    }

    /// Review S6: a ledger row naming a slug `slug()` could not make is refused before any file is written.
    #[test]
    fn p33_rollback_refuses_a_ledger_slug_that_leaves_the_folder() {
        let root = temp("rollback-slug");
        let outside = root.join(nirvana_run::DIR).join("escaped.md");
        let row = LedgerRow {
            v: 1,
            id: "op-1".into(),
            at: 1,
            op: "new".into(),
            by: "x".into(),
            run: "r".into(),
            before: vec![Snap {
                slug: "../escaped".into(),
                file: Some("owned\n".into()),
                entry: None,
            }],
            after: vec![Snap {
                slug: "../escaped".into(),
                file: None,
                entry: None,
            }],
            of: None,
        };
        std::fs::create_dir_all(dir(&root)).expect("dir");
        std::fs::write(
            dir(&root).join(LEDGER),
            serde_json::to_string(&row).expect("row") + "\n",
        )
        .expect("ledger");
        let why = rollback(&root, "op-1", 2).expect_err("refused");
        assert!(why.contains("not a lesson slug"), "{why}");
        assert!(!outside.exists());
        assert_eq!(ledger(&root).len(), 1);
    }

    /// Review nit: the evidence written into a tracked lesson file carries no token.
    #[test]
    fn p33_evidence_is_redacted_before_it_is_written() {
        let hex = "a".repeat(20) + &"1".repeat(20);
        let said = format!(
            "key sk-proj-AbCdEfGhIjKlMnOpQr gh ghp_AbCdEfGhIjKlMnOpQrSt auth Bearer eyJhbGciOiJIUzI1 sha {hex} kept TOTAL 12 passed"
        );
        assert_eq!(
            redact(&said),
            "key [redacted] gh [redacted] auth Bearer [redacted] sha [redacted] kept TOTAL 12 passed"
        );
        assert_eq!(
            redact("run-the-godot-suite-headless-and-read-it"),
            "run-the-godot-suite-headless-and-read-it"
        );
        let root = temp("redact");
        let text = stream(&format!("=== {said}"), "unused");
        let l = lesson(&said);
        let Verdict::Written(path) =
            verify_and_write(&root, &l, true, &text, "o1", "r1", 5, Meta::default())
        else {
            panic!("written")
        };
        let body = std::fs::read_to_string(path).expect("body");
        assert!(
            !body.contains("sk-proj") && !body.contains("ghp_") && !body.contains(&hex),
            "{body}"
        );
        assert!(body.contains("kept TOTAL 12 passed"), "{body}");
    }

    #[test]
    fn slugs_are_safe_file_names() {
        assert_eq!(
            slug("Run the Godot suite --headless!"),
            "run-the-godot-suite-headless"
        );
        assert_eq!(slug("../../etc"), "etc");
        assert_eq!(slug("…"), "lesson");
    }
}
