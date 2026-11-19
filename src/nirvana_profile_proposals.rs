//! Nirvana fork, plan 45 P45c: profile proposals, `herdr profile proposals|proposal show|accept|dismiss`.
//! (nirvana/plans/2026-11-18-45-agent-profiles.md §5, §6, §7)
//!
//! The owner, 2026-11-18 (plan 45 L8): "Have lessons look for moments where it can consider building an agent like we
//! described for future runs." Ruled 2026-11-19 (R1): "a proposed profile waits for Accept", and (R3) "one piece at a
//! time": a proposal is ONE piece, a new profile or one change to one profile's body, accepted or dismissed by itself.
//!
//! Measured before (§4 M7): 105 Megajam lesson reviews proposed no profile, skill or agent, and 96 of its 115 lessons
//! were keyed to one builder role. A profile changes the seat pick and what a worker is told on every later order,
//! and a worker's stream is not trusted text (§6), so a proposal is inert until the owner accepts it.
//!
//! **The file** is `<project>/.nirvana/profile-proposals/<id>.md`, `<id>` being `pp-<n>`. It is outside `profiles/`
//! because `nirvana_profiles::list` takes every folder there for a profile. A `---` front matter, one key a line:
//!
//! ```text
//! ---
//! kind: new | patch
//! profile: <profile name>
//! by: <order id> | detector
//! run: <run id>
//! date: YYYY-MM-DD                      (herdr's, at write)
//! key: <the writer's identity key>      (one line, e.g. `paragraph:<sha>` or `class:media.video needs:video`)
//! evidence: <counts and a "quoted" span of at least 20 characters, one line, at most 600>
//! base_sha: <64 hex>                    (patch only: `profile_sha` of the profile.md it read)
//! ---
//! <body>
//! ```
//!
//! For `new` the body is the whole profile file (its own `---` front matter and body), as `nirvana_profiles::parse`
//! reads it. For `patch` the body is the profile's whole new body: accept keeps the file's front matter as it stands
//! and replaces everything after its closing `---`, so a patch can never set a key, and the ledger row holds the file
//! before and after, so a rollback is exact.
//!
//! **What a proposal may not carry** (§6), checked when it is written and again when it is accepted (a file may be
//! edited by hand between): a server definition (a proposal is one text file and accept writes `profile.md` alone,
//! never an `mcp.json`); a `model` or `family` pin; a `skills` name the project does not have; a `needs` that is
//! neither a seat capability nor a server the project's `.mcp.json` names; a class the seat table does not have;
//! anything `nirvana_profiles::parse` refuses (a Send, herdr's footer mark, a body over `BODY_MAX`). A `new` proposal
//! for a name that exists is refused; a `patch` whose `base_sha` is not the profile's sha now is refused as stale.
//! The evidence and the body are redacted as a lesson's evidence is (`nirvana_lessons::redact`): the folder is
//! tracked with the project, and a worker's stream can hold a token.
//!
//! **The ledger** is `<project>/.nirvana/profile-proposals/ledger.jsonl`, one `Row` a line: `accept` (the profile
//! file before and after, and the proposal's text), `dismiss` (the proposal's identity and text, and the reason), and
//! `rollback` (`of` the op it undid). A dismissal's identity is the kind, the profile and the sha of the writer's key
//! normalised (`key_sha`), so `write` refuses the same proposal again, "dismissed on <date>", until that dismissal is
//! rolled back.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::nirvana_lessons;
use crate::nirvana_profiles;
use crate::nirvana_run;

/// The folder in `.nirvana/`, beside `profiles/`, never in it.
pub const DIR: &str = "profile-proposals";
pub const LEDGER: &str = "ledger.jsonl";
/// The front matter keys, in the order herdr writes them.
pub const KEYS: [&str; 8] = [
    "kind", "profile", "by", "run", "date", "key", "evidence", "base_sha",
];
/// The evidence is one line in the panel's Suggested list and in `herdr profile proposals`; past this it is a
/// paragraph, not a line. A hypothesis, like the lessons' caps.
pub const EVIDENCE_MAX: usize = 600;
/// The writer's key is an identity, not prose.
pub const KEY_MAX: usize = 200;
/// The writer that is not an order: plan 45 §5's detector, which reads the run with no model.
pub const DETECTOR: &str = "detector";

/// What a proposal is: a new profile, or one change to one profile's body (R3: one piece at a time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    New,
    Patch,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Patch => "patch",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "new" => Some(Self::New),
            "patch" => Some(Self::Patch),
            _ => None,
        }
    }
}

/// What a writer (P45b's detector or a lesson review's `profile_ops`) hands `write`. herdr sets the id and the date.
// P45b's proposer builds and passes it (plan 45 §8); until it lands only the tests do.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub kind: Kind,
    pub profile: String,
    /// The order whose Work Report carried it, or `DETECTOR`.
    pub by: String,
    pub run: String,
    /// What was measured: counts, and at least one quoted span of `MIN_EVIDENCE` characters or more.
    pub evidence: String,
    /// The writer's identity of this proposal, the same text each time the same thing is found (the repeated
    /// paragraph's sha, the class and needs): a dismissal is remembered by it.
    pub key: String,
    /// `new`: the whole profile file. `patch`: the profile's whole new body.
    pub body: String,
    /// `patch` only: `profile_sha` of the profile when the writer read it.
    pub base_sha: Option<String>,
}

/// A proposal waiting, as its file says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub id: String,
    pub kind: Kind,
    pub profile: String,
    pub by: String,
    pub run: String,
    pub date: String,
    pub evidence: String,
    pub key: String,
    pub base_sha: Option<String>,
    pub body: String,
    pub file: PathBuf,
}

/// Why `write` refused a draft: every reason, one each.
// P45b's proposer reads it from `write` (plan 45 §8); until it lands only the tests do.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal(pub Vec<String>);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.join("; "))
    }
}

/// One line of `ledger.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub v: u32,
    /// `op-<n>`, the highest plus one (plan 33 review S5: never a count of rows).
    pub op: String,
    pub at: u64,
    /// `accept`, `dismiss` or `rollback`.
    pub action: String,
    pub proposal: String,
    pub kind: Kind,
    pub profile: String,
    /// `key_sha` of the proposal's key: a dismissal's identity with the kind and the profile.
    pub key: String,
    pub date: String,
    /// `accept`: the profile file written, under `.nirvana/`, for a person reading the ledger. Never read back as a
    /// path: a rollback finds the file by the profile's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `accept`: the profile file's text before (`None`: there was none) and after.
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    /// `accept` and `dismiss`: the proposal file's text, which a rollback puts back as waiting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// `rollback`: the op it undid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<String>,
}

pub fn folder(root: &Path) -> PathBuf {
    root.join(nirvana_run::DIR).join(DIR)
}

/// `pp-<n>`, `n` a number: an id names a file, so nothing else is one.
pub fn valid_id(id: &str) -> bool {
    id.strip_prefix("pp-")
        .is_some_and(|n| !n.is_empty() && n.len() <= 9 && n.bytes().all(|b| b.is_ascii_digit()))
}

pub fn path(root: &Path, id: &str) -> PathBuf {
    folder(root).join(format!("{id}.md"))
}

fn number(id: &str) -> u64 {
    id.strip_prefix("pp-")
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// A key as a dismissal remembers it: whitespace collapsed, lowercased, redacted (`redact_key`), hashed. Redacting
/// here as well as in the file means a raw key and the key the file holds have one identity.
pub fn key_sha(key: &str) -> String {
    nirvana_lessons::sha256(&redact_key(&nirvana_lessons::one_line(key).to_lowercase()))
}

/// P45c review F5: the key as it goes to disk. The folder is tracked, so a key is redacted as the evidence and the
/// body are; but the key is an identity, and `nirvana_lessons::redact` would turn two keys that differ only in a long
/// run (`paragraph:<sha>`, the writers' own form) into one `paragraph:[redacted]`, so one dismissal would refuse every
/// paragraph. Each run that redaction takes (33 or more base64 characters, letters and digits both) is replaced here by
/// `h:` and 16 hex of its lowercase sha256 instead: distinct runs stay distinct, the run itself never lands. The
/// lessons' named patterns (`Bearer ...`, `sk-...`, GitHub tokens) are then redacted as they are anywhere.
pub fn redact_key(key: &str) -> String {
    use std::sync::OnceLock;
    static RUN: OnceLock<regex::Regex> = OnceLock::new();
    let run = RUN.get_or_init(|| regex::Regex::new(r"[A-Za-z0-9+/]{33,}={0,2}").expect("run"));
    let hashed = run.replace_all(key, |found: &regex::Captures| {
        let found = &found[0];
        if found.bytes().any(|b| b.is_ascii_digit())
            && found.bytes().any(|b| b.is_ascii_alphabetic())
        {
            format!(
                "h:{}",
                &nirvana_lessons::sha256(&found.to_lowercase())[..16]
            )
        } else {
            found.to_owned()
        }
    });
    nirvana_lessons::redact(&hashed)
}

/// The sha a `patch` names in `base_sha`: of the profile's file (`<name>.md` or `<name>/profile.md`) as it stands,
/// CRLF read as LF so a checkout's line endings never stale a proposal. `None` when there is no such profile file.
pub fn profile_sha(root: &Path, name: &str) -> Option<String> {
    let file = nirvana_profiles::resolve(root, name).ok()?;
    read_lf(&file).map(|text| nirvana_lessons::sha256(&text))
}

fn read_lf(file: &Path) -> Option<String> {
    std::fs::read_to_string(file)
        .ok()
        .map(|text| text.replace("\r\n", "\n"))
}

// ---- the file ----

/// The proposal file herdr writes for `draft` on `date`, as it is: `write` redacts the draft first, once.
// Called by `write`, which P45b's proposer calls (plan 45 §8); until it lands only the tests reach it.
#[allow(dead_code)]
pub fn render(draft: &Draft, date: &str) -> String {
    let mut text = format!(
        "---\nkind: {}\nprofile: {}\nby: {}\nrun: {}\ndate: {date}\nkey: {}\nevidence: {}\n",
        draft.kind.word(),
        draft.profile,
        draft.by,
        draft.run,
        nirvana_lessons::one_line(&draft.key),
        nirvana_lessons::one_line(&draft.evidence)
    );
    if let Some(sha) = &draft.base_sha {
        text.push_str(&format!("base_sha: {sha}\n"));
    }
    text.push_str("---\n");
    text.push_str(draft.body.replace("\r\n", "\n").trim());
    text.push('\n');
    text
}

/// Parses a proposal file's text; `id` is its file's stem. Every problem is returned.
pub fn parse(file: &Path, id: &str, text: &str) -> Result<Proposal, Vec<String>> {
    let shown = file.display();
    let text = text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.first() != Some(&"---") {
        return Err(vec![format!(
            "{shown}: line 1: a proposal starts with a `---` line"
        )]);
    }
    let Some(close) = lines
        .iter()
        .skip(1)
        .position(|line| *line == "---")
        .map(|at| at + 1)
    else {
        return Err(vec![format!(
            "{shown}: no closing `---` line after the keys"
        )]);
    };
    let mut problems = Vec::new();
    if !valid_id(id) {
        problems.push(format!("{shown}: the file name `{id}` is not pp-<n>"));
    }
    let mut values: Vec<(&str, String)> = Vec::new();
    for (index, line) in lines.iter().enumerate().take(close).skip(1) {
        let Some((key, value)) = line.split_once(':') else {
            problems.push(format!(
                "{shown}: line {}: expected `key: value`",
                index + 1
            ));
            continue;
        };
        let key = key.trim();
        let Some(known) = KEYS.iter().find(|known| **known == key) else {
            problems.push(format!(
                "{shown}: line {}: unknown key `{key}` (the keys: {})",
                index + 1,
                KEYS.join(", ")
            ));
            continue;
        };
        if values.iter().any(|(seen, _)| seen == known) {
            problems.push(format!("{shown}: line {}: duplicate `{key}`", index + 1));
            continue;
        }
        values.push((known, value.trim().to_owned()));
    }
    let get = |key: &str| {
        values
            .iter()
            .find(|(seen, _)| *seen == key)
            .map(|(_, value)| value.clone())
    };
    let mut need = |key: &str| {
        get(key).unwrap_or_else(|| {
            problems.push(format!("{shown}: no `{key}`"));
            String::new()
        })
    };
    let (kind, profile, by, run, date, key, evidence) = (
        need("kind"),
        need("profile"),
        need("by"),
        need("run"),
        need("date"),
        need("key"),
        need("evidence"),
    );
    let kind = match Kind::parse(&kind) {
        Some(kind) => Some(kind),
        None => {
            problems.push(format!("{shown}: kind `{kind}` is neither new nor patch"));
            None
        }
    };
    let base_sha = get("base_sha");
    match (kind, &base_sha) {
        (Some(Kind::Patch), None) => problems.push(format!("{shown}: a patch names its base_sha")),
        (Some(Kind::New), Some(_)) => {
            problems.push(format!("{shown}: a new profile has no base_sha"))
        }
        _ => {}
    }
    let body = lines[close + 1..].join("\n").trim().to_owned();
    match kind {
        Some(kind) if problems.is_empty() => Ok(Proposal {
            id: id.to_owned(),
            kind,
            profile,
            by,
            run,
            date,
            evidence,
            key,
            base_sha,
            body,
            file: file.to_path_buf(),
        }),
        _ => Err(problems),
    }
}

/// `patch`'s result: the profile file's front matter as it stands, then `body`.
pub fn patched(current: &str, body: &str) -> Option<String> {
    let current = current.replace("\r\n", "\n");
    let lines: Vec<&str> = current.split('\n').collect();
    if lines.first().map(|line| line.trim_end()) != Some("---") {
        return None;
    }
    let close = lines
        .iter()
        .skip(1)
        .position(|line| line.trim_end() == "---")?
        + 1;
    Some(format!(
        "{}\n{}\n",
        lines[..=close].join("\n"),
        body.replace("\r\n", "\n").trim()
    ))
}

// ---- what a proposal may carry (§6) ----

/// The quoted spans of `text`: what lies between each pair of `"`.
fn quoted(text: &str) -> Vec<&str> {
    text.split('"').skip(1).step_by(2).collect()
}

/// The checks of a proposal's own fields: who wrote it, its key and its evidence.
fn field_problems(by: &str, run: &str, key: &str, evidence: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if by != DETECTOR && !nirvana_run::valid_id(by) {
        problems.push(format!("by `{by}` is neither an order id nor `{DETECTOR}`"));
    }
    if !nirvana_run::valid_id(run) {
        problems.push(format!("run `{run}` is not a run id"));
    }
    let key = nirvana_lessons::one_line(key);
    if key.is_empty() || key.chars().count() > KEY_MAX {
        problems.push(format!(
            "the key is {} characters; 1 to {KEY_MAX}, the same text each time the same thing is found",
            key.chars().count()
        ));
    }
    // §5: "Every op must quote its evidence": counts, and a quote long enough to prove something.
    let evidence = nirvana_lessons::one_line(evidence);
    let count = evidence.chars().count();
    if count == 0 || count > EVIDENCE_MAX {
        problems.push(format!(
            "the evidence is {count} characters; 1 to {EVIDENCE_MAX}, one line"
        ));
    }
    if !evidence.bytes().any(|b| b.is_ascii_digit()) {
        problems.push("the evidence holds no count: say how many orders, lessons or times".into());
    }
    if !quoted(&evidence)
        .iter()
        .any(|span| span.trim().chars().count() >= nirvana_lessons::MIN_EVIDENCE)
    {
        problems.push(format!(
            "the evidence quotes nothing: put at least {} characters of what was found between double quotes",
            nirvana_lessons::MIN_EVIDENCE
        ));
    }
    problems
}

/// §6 for a `new` proposal's profile text, read against the project as it stands.
fn new_problems(root: &Path, name: &str, body: &str) -> Vec<String> {
    let file = nirvana_profiles::folder_of(root, name).join(nirvana_profiles::PROFILE_FILE);
    let mut problems = Vec::new();
    if nirvana_profiles::path(root, name).exists()
        || nirvana_profiles::folder_of(root, name).exists()
    {
        problems.push(format!(
            "a profile {name} exists already: propose a patch of its body instead"
        ));
    }
    let profile = match nirvana_profiles::parse(&file, name, body) {
        Ok(profile) => profile,
        Err(found) => {
            problems.extend(found);
            return problems;
        }
    };
    if profile.model.is_some() || profile.family.is_some() {
        problems.push(
            "a proposal never pins a model or a family: the owner sets those by hand (plan 45 §6)"
                .into(),
        );
    }
    let skills = crate::nirvana_agy_tools::project_skills(root);
    for skill in profile
        .skills
        .iter()
        .flatten()
        .filter(|skill| !skills.contains(skill))
    {
        problems.push(format!(
            "skills {skill}: the project has no such skill in .claude/skills"
        ));
    }
    let table = crate::nirvana_seat::table_for(root);
    if let Some(class) = &profile.class {
        match &table {
            Ok(table) if table.class(class).is_some() => {}
            Ok(_) => problems.push(format!("class {class}: no such class in seats.toml")),
            Err(why) => problems.push(format!(
                "class {class}: seats.toml is refused ({why}), so the class cannot be checked"
            )),
        }
    }
    let servers = crate::nirvana_gui_tools::project_servers(root);
    for need in &profile.needs {
        let capability = table.as_ref().is_ok_and(|table| {
            crate::nirvana_gui_tools::capabilities(table).contains(&need.as_str())
        });
        let server = servers.as_ref().is_ok_and(|names| names.contains(need));
        if !capability && !server {
            problems.push(format!(
                "needs {need}: neither a seat capability nor a server the project's .mcp.json names (a proposal \
                 never brings a server of its own)"
            ));
        }
    }
    problems
}

/// §6 for a `patch`: a profile to patch, a well-formed `base_sha`, and the patched file passing the parser.
fn patch_problems(root: &Path, name: &str, body: &str, base_sha: Option<&str>) -> Vec<String> {
    let mut problems = Vec::new();
    if !base_sha.is_some_and(|sha| sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit())) {
        problems.push("a patch names the base_sha of the profile it read, 64 hex".into());
    }
    let file = match nirvana_profiles::resolve(root, name) {
        Ok(file) => file,
        Err(why) => {
            problems.push(why);
            return problems;
        }
    };
    let Some(current) = read_lf(&file) else {
        problems.push(format!(
            "no profile {name} to patch: {} is not there",
            file.display()
        ));
        return problems;
    };
    match patched(&current, body) {
        None => problems.push(format!("{} has no front matter to keep", file.display())),
        Some(text) => {
            if let Err(found) = nirvana_profiles::parse(&file, name, &text) {
                problems.extend(found);
            }
        }
    }
    problems
}

/// Every reason a proposal of these parts may not be written or accepted now. Stale and dismissed are not here: the
/// first is `accept`'s and the second `write`'s.
fn content_problems(
    root: &Path,
    kind: Kind,
    name: &str,
    body: &str,
    base_sha: Option<&str>,
) -> Vec<String> {
    if !nirvana_profiles::valid_name(name) {
        return vec![format!(
            "profile `{name}` is not 1 to {} of [a-z0-9-]",
            nirvana_profiles::NAME_MAX
        )];
    }
    // P45c review F7: a new profile never takes a Windows device name; a patch names one on disk, which may.
    if kind == Kind::New {
        if let Some(why) = nirvana_profiles::new_name_problem(name) {
            return vec![why];
        }
    }
    match kind {
        Kind::New => new_problems(root, name, body),
        Kind::Patch => patch_problems(root, name, body, base_sha),
    }
}

// ---- the ledger and the lock ----

/// The rows of the ledger that parse, in order. A line that does not is never dropped in silence: `broken` names it,
/// `unreadable` (and so `herdr profile proposals`) prints it, and no op or proposal number it may hold is used again.
pub fn ledger(root: &Path) -> Vec<Row> {
    read_ledger(root).0
}

/// P45c review F4: the ledger's lines that do not parse, each with its line number (from 1), its text and why. A torn
/// line, as a writer killed mid-append leaves, used to swallow the next row and be dropped with it.
pub fn broken(root: &Path) -> Vec<(usize, String, String)> {
    read_ledger(root).1
}

type Ledger = (Vec<Row>, Vec<(usize, String, String)>);

fn read_ledger(root: &Path) -> Ledger {
    let text = std::fs::read_to_string(folder(root).join(LEDGER)).unwrap_or_default();
    let mut rows = Vec::new();
    let mut bad = Vec::new();
    for (at, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Row>(line) {
            Ok(row) => rows.push(row),
            Err(err) => bad.push((at + 1, line.to_owned(), err.to_string())),
        }
    }
    (rows, bad)
}

/// Every `<prefix><digits>` in `text`, as numbers: what a line that does not parse may hold.
fn numbers_in(text: &str, prefix: &str) -> Vec<u64> {
    text.match_indices(prefix)
        .filter_map(|(at, _)| {
            let digits: String = text[at + prefix.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse().ok()
        })
        .collect()
}

/// `op-<n>`, past every op a row holds and every one a broken line may hold (P45c review F4: an op id a torn line
/// held was handed out again). A line is one op, so the count of lines is a floor too: a torn line whose op id was
/// cut is never reused.
fn next_op(root: &Path, rows: &[Row]) -> String {
    let bad = broken(root);
    let top = rows
        .iter()
        .filter_map(|row| row.op.strip_prefix("op-")?.parse::<u64>().ok())
        .chain(
            bad.iter()
                .flat_map(|(_, line, _)| numbers_in(line, "\"op-")),
        )
        .chain(std::iter::once((rows.len() + bad.len()) as u64))
        .max()
        .unwrap_or(0);
    format!("op-{}", top + 1)
}

/// One row on a line of its own. P45c review F4: a ledger that does not end in a newline (a writer killed mid-line)
/// gets one first, so the row is never glued onto the torn line and lost with it.
fn append(root: &Path, row: &Row) -> std::io::Result<()> {
    use std::io::{Read, Seek, SeekFrom, Write};
    std::fs::create_dir_all(folder(root))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(folder(root).join(LEDGER))?;
    let mut line = String::new();
    if file.seek(SeekFrom::End(0))? > 0 {
        file.seek(SeekFrom::End(-1))?;
        let mut last = [0u8; 1];
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            line.push('\n');
        }
    }
    line.push_str(&serde_json::to_string(row)?);
    line.push('\n');
    file.write_all(line.as_bytes())
}

pub(crate) struct Lock(std::fs::File);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Under `runs/`, which `.nirvana/.gitignore` ignores, as the knowledge cards' lock is. P45c review F7: `herdr profile
/// new` takes it too, so it and an `accept` of a new profile of the same name cannot interleave.
pub(crate) fn lock(root: &Path) -> Result<Lock, String> {
    let path = root
        .join(nirvana_run::DIR)
        .join("runs")
        .join(".profile-proposals.lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|err| format!("cannot open {}: {err}", path.display()))?;
    file.lock()
        .map_err(|err| format!("the proposals' lock: {err}"))?;
    Ok(Lock(file))
}

/// The ops a rollback has undone.
fn rolled_back(rows: &[Row]) -> Vec<&str> {
    rows.iter()
        .filter(|row| row.action == "rollback")
        .filter_map(|row| row.of.as_deref())
        .collect()
}

fn today() -> String {
    nirvana_run::utc(nirvana_run::now())
        .chars()
        .take(10)
        .collect()
}

// ---- the API the panel (P45a) and the proposer (P45b) call ----

/// Every proposal waiting, oldest first (by its number). A file that does not parse is left out here and named by
/// `unreadable`.
pub fn list(root: &Path) -> Vec<Proposal> {
    let mut found: Vec<Proposal> = files(root)
        .into_iter()
        .filter_map(|(id, file)| {
            let text = std::fs::read_to_string(&file).ok()?;
            parse(&file, &id, &text).ok()
        })
        .collect();
    found.sort_by_key(|proposal| number(&proposal.id));
    found
}

/// The `*.md` files in the folder that are not proposals herdr can read, each with why, then each ledger line that does
/// not parse (P45c review F4), as `line <n>: <why>` against the ledger's path. The line's text is not repeated: it can
/// hold a proposal's text.
pub fn unreadable(root: &Path) -> Vec<(PathBuf, String)> {
    let mut found: Vec<(PathBuf, String)> = files(root)
        .into_iter()
        .filter_map(|(id, file)| {
            let why = match std::fs::read_to_string(&file) {
                Err(err) => err.to_string(),
                Ok(text) => parse(&file, &id, &text).err()?.join("; "),
            };
            Some((file, why))
        })
        .collect();
    let ledger = folder(root).join(LEDGER);
    for (at, _, why) in broken(root) {
        found.push((
            ledger.clone(),
            format!(
                "line {at} does not parse ({why}); its op and proposal numbers are not used again"
            ),
        ));
    }
    found
}

fn files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, PathBuf)> = std::fs::read_dir(folder(root))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|file| file.is_file() && file.extension().is_some_and(|ext| ext == "md"))
        .filter_map(|file| {
            let id = file.file_stem()?.to_string_lossy().into_owned();
            Some((id, file))
        })
        .collect();
    found.sort();
    found
}

/// One waiting proposal by its id.
pub fn get(root: &Path, id: &str) -> Result<Proposal, String> {
    if !valid_id(id) {
        return Err(format!("{id:?} is not a proposal id (pp-<n>)"));
    }
    let file = path(root, id);
    let text = std::fs::read_to_string(&file).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => format!(
            "no proposal {id} waiting in {}: `herdr profile proposals` lists them",
            folder(root).display()
        ),
        _ => format!("cannot read {}: {err}", file.display()),
    })?;
    parse(&file, id, &text).map_err(|problems| problems.join("\n"))
}

/// When a dismissal of this proposal's identity (kind, profile, `key_sha` of `key`) stands, the date it was made;
/// `None` when none does (or it was rolled back).
// Called by `write`, and by P45b's proposer before it drafts (plan 45 §8); until it lands only the tests reach it.
#[allow(dead_code)]
pub fn was_dismissed(root: &Path, kind: Kind, profile: &str, key: &str) -> Option<String> {
    let rows = ledger(root);
    let undone = rolled_back(&rows);
    let sha = key_sha(key);
    rows.iter()
        .rev()
        .find(|row| {
            row.action == "dismiss"
                && row.kind == kind
                && row.profile == profile
                && row.key == sha
                && !undone.contains(&row.op.as_str())
        })
        .map(|row| row.date.clone())
}

/// Writes `draft` as a waiting proposal and returns its id, or every reason it may not be written: its fields, what it
/// carries (§6), a dismissal of the same identity, the same proposal already waiting, and for a patch a `base_sha`
/// that is not the profile's now (the writer read something else). The key, the evidence and the body are redacted
/// first, and every check reads them redacted, as they land on disk.
// P45b's proposer calls it (plan 45 §8); until it lands only the tests do.
#[allow(dead_code)]
pub fn write(root: &Path, draft: &Draft) -> Result<String, Refusal> {
    let _lock = lock(root).map_err(|why| Refusal(vec![why]))?;
    // P45c review F5: the key too, and every check below reads what lands on disk. Review2 E1: lowercased first, as
    // `key_sha` does, so the named patterns (`sk-` is case-sensitive in `nirvana_lessons::redact`) see an `SK-...`
    // run as `sk-...` and it never lands in the clear, and the key on disk and its `key_sha` are one identity.
    let draft = Draft {
        key: redact_key(&draft.key.to_lowercase()),
        evidence: nirvana_lessons::redact(&draft.evidence),
        body: nirvana_lessons::redact(&draft.body),
        ..draft.clone()
    };
    let mut problems = field_problems(&draft.by, &draft.run, &draft.key, &draft.evidence);
    problems.extend(content_problems(
        root,
        draft.kind,
        &draft.profile,
        &draft.body,
        draft.base_sha.as_deref(),
    ));
    if draft.kind == Kind::New && draft.base_sha.is_some() {
        problems.push("a new profile has no base_sha".into());
    }
    if draft.kind == Kind::Patch {
        let now = profile_sha(root, &draft.profile);
        if draft.base_sha.is_some() && now.is_some() && draft.base_sha != now {
            problems.push(format!(
                "base_sha {} is not {}'s sha now, {}: read the profile again",
                draft.base_sha.as_deref().unwrap_or(""),
                draft.profile,
                now.as_deref().unwrap_or("")
            ));
        }
    }
    if let Some(date) = was_dismissed(root, draft.kind, &draft.profile, &draft.key) {
        problems.push(format!(
            "the owner dismissed this proposal ({} of {}) on {date}",
            draft.kind.word(),
            draft.profile
        ));
    }
    let sha = key_sha(&draft.key);
    if let Some(waiting) = list(root).iter().find(|waiting| {
        waiting.kind == draft.kind
            && waiting.profile == draft.profile
            && key_sha(&waiting.key) == sha
    }) {
        problems.push(format!(
            "the same proposal is waiting already as {}",
            waiting.id
        ));
    }
    if !problems.is_empty() {
        return Err(Refusal(problems));
    }
    let (rows, bad) = read_ledger(root);
    let top = files(root)
        .iter()
        .map(|(id, _)| number(id))
        .chain(rows.iter().map(|row| number(&row.proposal)))
        .chain(
            bad.iter()
                .flat_map(|(_, line, _)| numbers_in(line, "\"pp-")),
        )
        .max()
        .unwrap_or(0);
    let id = format!("pp-{}", top + 1);
    let file = path(root, &id);
    std::fs::create_dir_all(folder(root)).map_err(|err| {
        Refusal(vec![format!(
            "cannot create {}: {err}",
            folder(root).display()
        )])
    })?;
    crate::nirvana_seat::write_atomic(&file, render(&draft, &today()).as_bytes())
        .map_err(|err| Refusal(vec![format!("cannot write {}: {err}", file.display())]))?;
    Ok(id)
}

/// The proposal's checks again, now: what it carries (a file can be edited between write and accept), and for a
/// patch that the profile still stands at `base_sha`.
fn accept_problems(root: &Path, proposal: &Proposal) -> Vec<String> {
    let mut problems = field_problems(
        &proposal.by,
        &proposal.run,
        &proposal.key,
        &proposal.evidence,
    );
    problems.extend(content_problems(
        root,
        proposal.kind,
        &proposal.profile,
        &proposal.body,
        proposal.base_sha.as_deref(),
    ));
    if proposal.kind == Kind::Patch && problems.is_empty() {
        let now = profile_sha(root, &proposal.profile).unwrap_or_default();
        if proposal.base_sha.as_deref() != Some(now.as_str()) {
            problems.push(format!(
                "{} is stale: profile {} changed since it was proposed (base_sha {}, now {now}); dismiss it, and the \
                 next review proposes from the profile as it stands",
                proposal.id,
                proposal.profile,
                proposal.base_sha.as_deref().unwrap_or("")
            ));
        }
    }
    problems
}

/// Accepts a waiting proposal: a `new` one becomes `profiles/<name>/profile.md` (the folder form, with an empty
/// `knowledge/`, so it can be taught), a `patch` replaces its profile's body. One ledger row holds the file before and
/// after and the proposal's text; the proposal file goes. The line says which op did it, for `rollback`.
pub fn accept(root: &Path, id: &str) -> Result<String, String> {
    let _lock = lock(root)?;
    let mut proposal = get(root, id)?;
    // P45c review F5: the file may have been edited by hand since `write`, so what goes into the profile is redacted
    // again here, and checked as it will be written.
    proposal.body = nirvana_lessons::redact(&proposal.body);
    let problems = accept_problems(root, &proposal);
    if !problems.is_empty() {
        return Err(format!("{id} is refused: {}", problems.join("; ")));
    }
    let name = &proposal.profile;
    let (file, before, after) = match proposal.kind {
        Kind::New => {
            let dir = nirvana_profiles::folder_of(root, name);
            std::fs::create_dir_all(dir.join(nirvana_profiles::KNOWLEDGE)).map_err(|err| {
                remove_empty(&dir);
                format!("cannot create {}: {err}", dir.display())
            })?;
            (
                dir.join(nirvana_profiles::PROFILE_FILE),
                None,
                format!("{}\n", proposal.body.trim()),
            )
        }
        Kind::Patch => {
            let file = nirvana_profiles::resolve(root, name)?;
            let before = std::fs::read_to_string(&file)
                .map_err(|err| format!("cannot read {}: {err}", file.display()))?;
            let after = patched(&before, &proposal.body)
                .ok_or_else(|| format!("{} has no front matter to keep", file.display()))?;
            (file, Some(before), after)
        }
    };
    let text = std::fs::read_to_string(&proposal.file)
        .map_err(|err| format!("cannot read {}: {err}", proposal.file.display()))?;
    crate::nirvana_seat::write_atomic(&file, after.as_bytes()).map_err(|err| {
        // P45c review F7: a new profile's folder made above goes again, so a failed accept leaves nothing behind.
        if proposal.kind == Kind::New {
            remove_empty(&nirvana_profiles::folder_of(root, name));
        }
        format!("cannot write {}: {err}", file.display())
    })?;
    let rows = ledger(root);
    let op = next_op(root, &rows);
    let row = Row {
        v: 1,
        op: op.clone(),
        at: nirvana_run::now(),
        action: "accept".into(),
        proposal: id.to_owned(),
        kind: proposal.kind,
        profile: name.clone(),
        key: key_sha(&proposal.key),
        date: today(),
        path: Some(nirvana_lessons::rel_in(&root.join(nirvana_run::DIR), &file)),
        before: before.clone(),
        after: Some(after),
        text: Some(text),
        reason: None,
        of: None,
    };
    if let Err(err) = append(root, &row) {
        // In the ledger and the folder, or in neither: the profile goes back as it stood.
        let undone = match &before {
            Some(before) => crate::nirvana_seat::write_atomic(&file, before.as_bytes()),
            None => remove_new(&nirvana_profiles::folder_of(root, name)),
        };
        return Err(format!(
            "the ledger: {err}{}",
            undone
                .err()
                .map(|e| format!("; and the profile could not be put back: {e}"))
                .unwrap_or_default()
        ));
    }
    let _ = std::fs::remove_file(&proposal.file);
    Ok(format!(
        "{op}: accepted {id}, {} {name} ({})",
        match proposal.kind {
            Kind::New => "new profile",
            Kind::Patch => "a patch of",
        },
        file.display()
    ))
}

/// A new profile's folder taken out whole, when it holds nothing but its `profile.md` and an empty `knowledge/`.
/// `knowledge/` goes first, and a card in it stops the removal before `profile.md` is touched (P45c review F7: a teach
/// writes cards under the knowledge lock, not this one, so one may land after `holds_more` looked).
fn remove_new(dir: &Path) -> std::io::Result<()> {
    let knowledge = dir.join(nirvana_profiles::KNOWLEDGE);
    match std::fs::remove_dir(&knowledge) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err),
        _ => {}
    }
    std::fs::remove_file(dir.join(nirvana_profiles::PROFILE_FILE))?;
    std::fs::remove_dir(dir)
}

/// A new profile's folder `accept` made, taken out when the accept fails before it wrote: only empty folders go.
fn remove_empty(dir: &Path) {
    let _ = std::fs::remove_dir(dir.join(nirvana_profiles::KNOWLEDGE));
    let _ = std::fs::remove_dir(dir);
}

/// Whether a new profile's folder holds anything but `profile.md` and an empty `knowledge/` (cards taught since).
fn holds_more(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            let name = entry.file_name();
            if name == nirvana_profiles::PROFILE_FILE {
                return false;
            }
            name != nirvana_profiles::KNOWLEDGE
                || std::fs::read_dir(entry.path()).is_ok_and(|mut inside| inside.next().is_some())
        })
}

/// Dismisses a waiting proposal: the ledger keeps its identity, its text and the reason, so `write` refuses the same
/// proposal again; the proposal file goes.
pub fn dismiss(root: &Path, id: &str, reason: Option<&str>) -> Result<String, String> {
    let _lock = lock(root)?;
    let proposal = get(root, id)?;
    let text = std::fs::read_to_string(&proposal.file)
        .map_err(|err| format!("cannot read {}: {err}", proposal.file.display()))?;
    let rows = ledger(root);
    let op = next_op(root, &rows);
    let row = Row {
        v: 1,
        op: op.clone(),
        at: nirvana_run::now(),
        action: "dismiss".into(),
        proposal: id.to_owned(),
        kind: proposal.kind,
        profile: proposal.profile.clone(),
        key: key_sha(&proposal.key),
        date: today(),
        path: None,
        before: None,
        after: None,
        text: Some(text),
        reason: reason
            .map(nirvana_lessons::one_line)
            .filter(|reason| !reason.is_empty()),
        of: None,
    };
    append(root, &row).map_err(|err| format!("the ledger: {err}"))?;
    let _ = std::fs::remove_file(&proposal.file);
    Ok(format!(
        "{op}: dismissed {id} ({} of {}); the same proposal is not made again",
        proposal.kind.word(),
        proposal.profile
    ))
}

/// Undoes one `accept` or `dismiss` exactly, when what it left still stands: an accepted profile file goes back to its
/// text before (a new profile's folder is taken out, refused when cards were taught into it since), and the proposal
/// is waiting again, as it was. A dismissal undone no longer refuses the same proposal.
pub fn rollback(root: &Path, op: &str) -> Result<String, String> {
    let _lock = lock(root)?;
    let rows = ledger(root);
    let Some(row) = rows
        .iter()
        .find(|row| row.op == op && matches!(row.action.as_str(), "accept" | "dismiss"))
        .cloned()
    else {
        return Err(format!(
            "no accept or dismiss {op} in {}",
            folder(root).join(LEDGER).display()
        ));
    };
    if rolled_back(&rows).contains(&op) {
        return Err(format!("{op} is rolled back already"));
    }
    // The ledger is tracked, so its names are checked before they name a file.
    if !nirvana_profiles::valid_name(&row.profile) || !valid_id(&row.proposal) {
        return Err(format!(
            "{op} names {:?} and {:?}: the ledger was edited by hand",
            row.profile, row.proposal
        ));
    }
    let waiting = path(root, &row.proposal);
    if waiting.exists() {
        return Err(format!("{} is there already", waiting.display()));
    }
    if row.action == "accept" {
        let name = &row.profile;
        let file = match row.kind {
            Kind::New => {
                nirvana_profiles::folder_of(root, name).join(nirvana_profiles::PROFILE_FILE)
            }
            Kind::Patch => nirvana_profiles::resolve(root, name)?,
        };
        let now = std::fs::read_to_string(&file).ok();
        if now.as_deref().map(|text| text.replace("\r\n", "\n"))
            != row.after.as_deref().map(|text| text.replace("\r\n", "\n"))
        {
            return Err(format!(
                "{} changed since {op}: roll back the later change first",
                file.display()
            ));
        }
        match (&row.kind, &row.before) {
            (Kind::New, _) => {
                let dir = nirvana_profiles::folder_of(root, name);
                if holds_more(&dir) {
                    return Err(format!(
                        "{} holds more than the profile {op} accepted (cards taught since?): roll those back first",
                        dir.display()
                    ));
                }
                remove_new(&dir)
                    .map_err(|err| format!("cannot remove {}: {err}", dir.display()))?;
            }
            (Kind::Patch, Some(before)) => {
                crate::nirvana_seat::write_atomic(&file, before.as_bytes())
                    .map_err(|err| format!("cannot write {}: {err}", file.display()))?;
            }
            (Kind::Patch, None) => {
                return Err(format!(
                    "{op} keeps no text before: the ledger was edited by hand"
                ));
            }
        }
    }
    if let Some(text) = &row.text {
        std::fs::create_dir_all(folder(root)).map_err(|err| err.to_string())?;
        crate::nirvana_seat::write_atomic(&waiting, text.as_bytes())
            .map_err(|err| format!("cannot write {}: {err}", waiting.display()))?;
    }
    let back = next_op(root, &rows);
    append(
        root,
        &Row {
            v: 1,
            op: back.clone(),
            at: nirvana_run::now(),
            action: "rollback".into(),
            proposal: row.proposal.clone(),
            kind: row.kind,
            profile: row.profile.clone(),
            key: row.key.clone(),
            date: today(),
            path: None,
            before: None,
            after: None,
            text: None,
            reason: None,
            of: Some(op.to_owned()),
        },
    )
    .map_err(|err| format!("the ledger: {err}"))?;
    Ok(format!(
        "{back}: rolled back {op} ({} of {}); {} is waiting again",
        row.action, row.proposal, row.proposal
    ))
}

// ---- `herdr profile proposals|proposal|accept|dismiss` ----

const USAGE: &str = "usage: herdr profile proposals
       herdr profile proposal show <id>
       herdr profile accept <id>
       herdr profile dismiss <id> [--reason \"...\"]
       herdr profile proposals rollback <op>";

/// The evidence as one line of a listing.
fn short(text: &str) -> String {
    const SHOWN: usize = 120;
    let line = nirvana_lessons::one_line(text);
    if line.chars().count() <= SHOWN {
        line
    } else {
        format!("{}...", line.chars().take(SHOWN).collect::<String>())
    }
}

/// The proposal verbs of `herdr profile`, `args` starting at the verb; the exit code.
pub fn cli_to(
    root: &Path,
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> i32 {
    fn said(
        result: Result<String, String>,
        verb: &str,
        out: &mut dyn std::io::Write,
        err: &mut dyn std::io::Write,
    ) -> i32 {
        match result {
            Ok(line) => {
                let _ = writeln!(out, "{line}");
                0
            }
            Err(why) => {
                let _ = writeln!(err, "herdr profile {verb}: {why}");
                1
            }
        }
    }
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["proposals"] => {
            let waiting = list(root);
            if waiting.is_empty() {
                let _ = writeln!(out, "no proposal waiting in {}", folder(root).display());
            }
            for proposal in &waiting {
                let _ = writeln!(
                    out,
                    "{} · {} · {} · {}",
                    proposal.id,
                    proposal.kind.word(),
                    proposal.profile,
                    short(&proposal.evidence)
                );
            }
            for (file, why) in unreadable(root) {
                let _ = writeln!(out, "{}: unreadable: {why}", file.display());
            }
            0
        }
        ["proposals", "rollback", op] => said(rollback(root, op), "proposals rollback", out, err),
        ["proposal", "show", id] => match get(root, id) {
            Ok(proposal) => {
                let text = std::fs::read_to_string(&proposal.file).unwrap_or_default();
                let _ = writeln!(out, "{}", proposal.file.display());
                let _ = write!(out, "{text}");
                0
            }
            Err(why) => {
                let _ = writeln!(err, "herdr profile proposal show: {why}");
                1
            }
        },
        ["accept", id] => said(accept(root, id), "accept", out, err),
        ["dismiss", id] => said(dismiss(root, id, None), "dismiss", out, err),
        ["dismiss", id, "--reason", reason] => {
            said(dismiss(root, id, Some(reason)), "dismiss", out, err)
        }
        _ => {
            let _ = writeln!(err, "{USAGE}");
            2
        }
    }
}

#[cfg(test)]
mod tests;
