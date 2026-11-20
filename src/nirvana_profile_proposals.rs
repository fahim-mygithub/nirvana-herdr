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
//! The evidence and the body are redacted as a lesson's evidence is (`nirvana_lessons::redact`), and then refused when
//! still secret-shaped (`secret_shaped`): a worker's stream and an order can hold a token. A body or evidence holding
//! a character the Profiles panel would draw as nothing (`invisible`) is refused too, by its code point (P45a fix
//! round).
//!
//! **The panel's Accept** (P45a fix round) is `accept_seen`: the `Seen` the panel drew (the proposal file's sha and
//! the `target` file text `accept` would write, by the same `prepared` and `target`) must still hold inside the lock,
//! else nothing is written (`Refused::Changed`); it takes the lock with `try_lock` and says `Refused::Busy` rather than
//! wait. `try_dismiss` is `dismiss` the same way. `herdr profile accept` and `dismiss` are unchanged.
//!
//! **The folder is ignored by git** (P45b second fix round): herdr writes `profile-proposals/.gitignore`, `*`, when it
//! is missing (`ensure_ignore`). `secret_shaped` is a pattern list and is not complete (the second review got 20 of
//! 40 shapes past it into `pp-1.md`), so the ignore file is the guard and the list is the second layer. The one
//! crossing into a tracked file is `accept`, and `herdr profile accept` says so in one line.
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

/// P45b second fix round: the ignore file herdr writes in the proposals folder. The second review put 40 secret
/// shapes in a repeated order paragraph and 20 reached `pp-1.md` past `secret_shaped`, in a folder a project may
/// commit; a pattern list is never complete. A waiting proposal quotes order text, which lives in the ignored
/// `orders/`, so ignoring the folder makes a proposal no more exposed than what it quotes, and the one crossing into a
/// tracked file is the owner's Accept. Written as `nirvana_profiles::ensure_ignore` writes `profiles/.gitignore`: only
/// when absent, never rewritten, and `.nirvana/.gitignore` (the owner's) is never touched. The ledger is ignored with
/// it, so its history stays on this machine.
pub const IGNORE_TEXT: &str = "# Written by herdr (plan 45 P45b) only when this file is missing; it never rewrites it.\n\
# A proposal quotes past orders, which can hold a secret, so it stays out of git until the owner accepts it.\n\
*\n";

/// Writes `profile-proposals/.gitignore` when it is not there; `Ok(true)` when it wrote it. Never over a file that
/// exists, an empty one included (`create_new`, so two writers cannot both write it).
pub fn ensure_ignore(root: &Path) -> std::io::Result<bool> {
    use std::io::Write;
    std::fs::create_dir_all(folder(root))?;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(folder(root).join(nirvana_profiles::IGNORE_FILE))
    {
        Ok(mut file) => file.write_all(IGNORE_TEXT.as_bytes()).map(|()| true),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(err) => Err(err),
    }
}

/// The proposals folder, made when missing, with its ignore file. Every writer into the folder comes through here,
/// so a folder made by an older build gets the file at the next write. A failure to write the ignore file is said in
/// one line on stderr and the write goes on: the proposal is still the owner's to read, and a stopped write loses it.
fn make_folder(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(folder(root))?;
    if let Err(err) = ensure_ignore(root) {
        eprintln!(
            "herdr: cannot write {}: {err}; the proposals there are not ignored by git",
            folder(root).join(nirvana_profiles::IGNORE_FILE).display()
        );
    }
    Ok(())
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

/// P45b review F1: the shape a secret in `text` has, by name, or `None`. The name is what a refusal says; the value is
/// never returned. The proposals folder was tracked with the project then (`.nirvana/.gitignore` ignores only
/// `runs/`, `orders/` and `worktrees/`; the second fix round gave the folder its own ignore file), and the review
/// reproduced an AWS key id, `password=...`, a 32-hex key, a Slack token
/// and `sk_live_...` landing in `pp-1.md` twice through `nirvana_lessons::redact`, which knows only `Bearer`, `sk-`,
/// GitHub tokens and long mixed runs. CLAUDE.md rule 12: no credential in any tracked file.
///
/// Here, not in the detector, so the detector (which calls this module) and the store share one list without a cycle.
/// It is never run over herdr's own sha lines (`base_sha`, `Profile sha256:`), which are 64 hex by design: only over a
/// proposal's body, description and evidence, and over order text.
///
/// **This list is not complete, and no pattern list is.** The second review put 40 secret shapes through it and 20
/// reached `pp-1.md`; the second fix round added a rule for each (`second_layer`), and a shape nobody wrote down still
/// passes. The guard is structural: the proposals folder holds a `.gitignore` of `*` (`ensure_ignore`), so a waiting
/// proposal is no more exposed than the ignored order file it quotes, and the one crossing into a tracked file is the
/// owner's Accept, which says so.
pub fn secret_shaped(text: &str) -> Option<&'static str> {
    use std::sync::OnceLock;
    static SHAPES: OnceLock<Vec<(&'static str, regex::Regex)>> = OnceLock::new();
    let shapes = SHAPES.get_or_init(|| {
        [
            ("PEM header", r"-----BEGIN [A-Z]"),
            ("AWS access key id", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
            ("Slack token", r"\bxox[a-z]-[A-Za-z0-9-]+"),
            ("Stripe key", r"\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]+"),
            ("Google API key", r"\bAIza[0-9A-Za-z_-]{16,}"),
            ("URL credentials", r"[A-Za-z][A-Za-z0-9+.-]*://[^/\s:@]+:[^@\s/]+@"),
            (
                "UUID after a key word",
                r"(?i)\b[a-z0-9_.-]*(?:key|token|secret|password|auth)[\s=:'\x22]{0,4}[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b",
            ),
            ("run of 32 or more hex characters", r"\b[0-9a-fA-F]{32,}\b"),
        ]
        .into_iter()
        .map(|(name, pattern)| (name, regex::Regex::new(pattern).expect(name)))
        .collect()
    });
    if let Some((name, _)) = shapes.iter().find(|(_, shape)| shape.is_match(text)) {
        return Some(name);
    }
    // `Basic <base64>`: the run must look encoded (a digit, `+`, `/`, `=`, or a capital after its first letter), so
    // "basic understanding" in prose is not one.
    static BASIC: OnceLock<regex::Regex> = OnceLock::new();
    let basic = BASIC.get_or_init(|| {
        regex::Regex::new(r"(?i)\bbasic\s+([A-Za-z0-9+/]{8,}={0,2})").expect("basic")
    });
    if basic.captures_iter(text).any(|found| {
        let run = &found[1];
        run.bytes().any(|b| b.is_ascii_digit() || matches!(b, b'+' | b'/' | b'='))
            || run.bytes().skip(1).any(|b| b.is_ascii_uppercase())
    }) {
        return Some("HTTP Basic credentials");
    }
    // A secret word (`key`, `secret`, `password`, `token`, `pwd`, and since the second fix round `passphrase`, `sig`,
    // `signature`, `auth`, `authorization`, `credential(s)`, `api key`, `access key`, `client secret`), then `=` or
    // `:`, then a value. After `=` any value of 3 or more: that is an assignment (review2: `key=ab1` landed). After `:`
    // a value of 5 or more that is quoted or holds a digit, so "Key: read the log" in prose is not one and `token:
    // "abcdef"` or `pwd: abc12` is; after `passphrase:` any value of 3 or more, since a passphrase is words. A quoted
    // value is the whole quote, spaces and all (review2: `password="my pass phrase here"` landed, read as `my`).
    static ASSIGNED: OnceLock<regex::Regex> = OnceLock::new();
    let assigned = ASSIGNED.get_or_init(|| {
        regex::Regex::new(
            r#"(?i)\b[a-z0-9_.-]*(passphrase|signature|sig|authorization|auth|credentials?|api[-_ ]?key|access[-_ ]?key|client[-_ ]?secret|key|secret|password|passwd|token|pwd)["']?\s*([=:])\s*(?:"([^"\n]+)"|'([^'\n]+)'|([^\s"',;)\]}]+))"#,
        )
        .expect("assigned")
    });
    if assigned.captures_iter(text).any(|found| {
        let quoted = found.get(3).or_else(|| found.get(4));
        let value = quoted.or_else(|| found.get(5)).map_or("", |value| value.as_str());
        let count = value.chars().count();
        match &found[2] {
            "=" => count >= 3,
            _ if found[1].eq_ignore_ascii_case("passphrase") => count >= 3,
            _ => count >= 5 && (quoted.is_some() || value.bytes().any(|b| b.is_ascii_digit())),
        }
    }) {
        return Some("credential assignment (key, secret, password or token)");
    }
    if nirvana_lessons::redact(text) != text {
        return Some("token nirvana_lessons::redact knows (Bearer, sk-, GitHub, a long mixed run)");
    }
    second_layer(text)
}

/// P45b second fix round: the shapes the second review reproduced past the list above, 20 of its 40 landing in
/// `pp-1.md`. Each rule here names one of them. They run after the first list, so a shape that list names keeps its
/// name. This is still a pattern list and it is still not complete: the guard is the `.gitignore` in the proposals
/// folder (`ensure_ignore`), and this list only keeps a secret out of the text the owner may later accept.
fn second_layer(text: &str) -> Option<&'static str> {
    use std::sync::OnceLock;
    // A value after a flag, `is` or a scheme is a secret only when it looks like one (a digit, or long), so "pass
    // --token with the id" and "the secret is patience" in prose stay. A value starting with `$`, `%` or `<` is a
    // variable or a placeholder, never the secret itself.
    fn looks_secret(value: &str, long: usize) -> bool {
        let count = value.chars().count();
        !value.starts_with(['$', '%', '<'])
            && (count >= long || (count >= 4 && value.bytes().any(|b| b.is_ascii_digit())))
    }
    static RULES: OnceLock<Vec<(&'static str, regex::Regex, usize)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            // review2 `authorization: token abcdef1234567890xyz`.
            (
                "authorization header value",
                r#"(?i)\bauthorization["']?\s*[:=]\s*["']?[A-Za-z][A-Za-z0-9_-]*\s+([A-Za-z0-9._~+/=-]+)"#,
                20,
            ),
            // review2 `my password is hunter2`.
            (
                "secret word, then is and a value",
                r#"(?i)\b(?:password|passphrase|passwd|pwd|secret|token|api[-_ ]?key|access[-_ ]?key|credentials?)\s+is\s+["']?([^\s"',;)]+)"#,
                12,
            ),
            // review2 `--token <v>`.
            (
                "secret flag and its value",
                r#"(?i)(?:^|\s)--(?:token|password|secret|api-key|passphrase)(?:=|\s+)["']?([^\s"']+)"#,
                12,
            ),
            // review2 `sshpass -p <v>`: any value.
            ("sshpass password", r#"(?i)\bsshpass\s+-p\s*["']?([^\s"']+)"#, 1),
            // review2 `curl -u admin:<v>`: any value.
            ("curl user and password", r#"(?i)\bcurl\b[^\n]*?\s-u\s*["']?[^\s:"']+:([^\s"']+)"#, 1),
            // review2: a JWT of any length (the short one landed), and a Google OAuth access token.
            ("JWT-shaped text (eyJ...)", r"\beyJ([A-Za-z0-9_-]{10,})", 1),
            ("Google OAuth token (ya29.)", r"\bya29\.([A-Za-z0-9_-]{10,})", 1),
        ]
        .into_iter()
        .map(|(name, pattern, long)| (name, regex::Regex::new(pattern).expect(name), long))
        .collect()
    });
    for (name, rule, long) in rules {
        if rule.captures_iter(text).any(|found| looks_secret(&found[1], *long)) {
            return Some(name);
        }
    }
    // review2: a 24-hex and a 25-alnum key, a 31-hex digest, a deploy token of 20, and base64 lines of 28 and 32.
    // 20 or more letters and digits with no separator, at least one of each. It withholds some long identifiers
    // (`Profile2LayoutBuilderTest`), which the lead accepted 2026-11-19.
    static RUN: OnceLock<regex::Regex> = OnceLock::new();
    let run = RUN.get_or_init(|| regex::Regex::new(r"[A-Za-z0-9]{20,}").expect("run"));
    if run.find_iter(text).any(|found| {
        let found = found.as_str();
        found.bytes().any(|b| b.is_ascii_digit()) && found.bytes().any(|b| b.is_ascii_alphabetic())
    }) {
        return Some("run of 20 or more letters and digits");
    }
    // review2: an AWS id split by a line break and a 40-hex key wrapped across lines. The fixed shapes again, over
    // the text with every whitespace taken out. No `\b` here, since the joined words around a value touch it; the hex
    // run must hold a digit and a letter, so a joined column of numbers is not one.
    let joined: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if joined.len() == text.len() {
        return None;
    }
    static JOINED: OnceLock<Vec<(&'static str, regex::Regex)>> = OnceLock::new();
    let shapes = JOINED.get_or_init(|| {
        [
            ("across whitespace: PEM header", r"-----BEGIN[A-Z]"),
            ("across whitespace: AWS access key id", r"(?:AKIA|ASIA)[0-9A-Z]{16}"),
            ("across whitespace: Slack token", r"xox[a-z]-[A-Za-z0-9-]{10,}"),
            ("across whitespace: Stripe key", r"(?:^|[^A-Za-z])(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{8,}"),
            ("across whitespace: Google API key", r"AIza[0-9A-Za-z_-]{16,}"),
            ("across whitespace: GitHub token", r"github_pat_[A-Za-z0-9_]{16,}"),
            // Two parts, each `eyJ`: a joined "they Just" is `eyJust`, never `eyJ...\.eyJ`.
            ("across whitespace: JWT-shaped text", r"eyJ[A-Za-z0-9_-]{5,}\.eyJ[A-Za-z0-9_-]{2,}"),
        ]
        .into_iter()
        .map(|(name, pattern)| (name, regex::Regex::new(pattern).expect(name)))
        .collect()
    });
    if let Some((name, _)) = shapes.iter().find(|(_, shape)| shape.is_match(&joined)) {
        return Some(name);
    }
    static HEX: OnceLock<regex::Regex> = OnceLock::new();
    let hex = HEX.get_or_init(|| regex::Regex::new(r"[0-9a-fA-F]{32,}").expect("hex"));
    if hex.find_iter(&joined).any(|found| {
        let found = found.as_str();
        found.bytes().any(|b| b.is_ascii_alphabetic()) && found.bytes().any(|b| b.is_ascii_digit())
    }) {
        return Some("across whitespace: run of 32 or more hex characters");
    }
    None
}

/// The refusal for a field that is secret-shaped: the shape's name, never the value.
fn secret_problems(body: &str, evidence: &str) -> Vec<String> {
    [("body", body), ("evidence", evidence)]
        .into_iter()
        .filter_map(|(field, text)| {
            secret_shaped(text).map(|shape| {
                format!(
                    "the {field} holds a secret-shaped text ({shape}): herdr never writes one into a proposal, \
                     which an Accept makes a tracked profile"
                )
            })
        })
        .collect()
}

/// P45a fix round (review finding 4): a character the Profiles panel's screen would not show. ratatui-core 0.1.0's
/// `Buffer::set_stringn` drops every grapheme that holds a control character and every grapheme unicode-width 0.2
/// measures 0 wide, and a format character riding on a visible grapheme is kept in its cell but draws as nothing. So a
/// body could carry text the owner never saw and Accept would write it. Refused here: every control character but
/// newline and tab (a CRLF's CR is read as LF first), every format character of Unicode 15.1 (general category Cf, and
/// the line and paragraph separators), and the default-ignorable fillers and selectors the brief lists (U+034F, the
/// Hangul fillers, U+17B4/5, the Mongolian selectors, U+FE00 to U+FE0F, U+FFA0, the tag and selector block U+E0000 to
/// U+E0FFF). Combining marks are not here: they draw on their base. Also refused, found by the sweep
/// `p94_fix_nothing_the_store_takes_is_dropped_by_the_screen`, which draws every code point: the conjoining Hangul vowels
/// and finals, U+1160 to U+11FF and U+D7B0 to U+D7FF, which unicode-width measures 0 wide, so one after anything but a
/// leading consonant is dropped, and U+A8FA, the Devanagari caret, which it measures 0 wide too. Korean written in
/// decomposed jamo is refused with them; precomposed syllables are text.
pub fn invisible(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t')
        || matches!(
            u32::from(c),
            0x00AD
                | 0x034F
                | 0x0600..=0x0605
                | 0x061C
                | 0x06DD
                | 0x070F
                | 0x0890..=0x0891
                | 0x08E2
                | 0x115F..=0x11FF
                | 0x17B4..=0x17B5
                | 0x180B..=0x180F
                | 0x200B..=0x200F
                | 0x2028..=0x202E
                | 0x2060..=0x206F
                | 0x3164
                | 0xA8FA
                | 0xD7B0..=0xD7FF
                | 0xFE00..=0xFE0F
                | 0xFEFF
                | 0xFFA0
                | 0xFFF0..=0xFFFB
                | 0x110BD
                | 0x110CD
                | 0x13430..=0x1343F
                | 0x1BCA0..=0x1BCA3
                | 0x1D173..=0x1D17A
                | 0xE0000..=0xE0FFF
        )
}

/// The first character of `text` the panel would not show (`invisible`), a CRLF read as LF.
pub fn first_invisible(text: &str) -> Option<char> {
    text.replace("\r\n", "\n").chars().find(|c| invisible(*c))
}

/// The refusal for a field holding a character the panel would not show: its code point, never the text.
fn invisible_problems(body: &str, evidence: &str) -> Vec<String> {
    [("body", body), ("evidence", evidence)]
        .into_iter()
        .filter_map(|(field, text)| {
            first_invisible(text).map(|c| {
                format!(
                    "the {field} holds U+{:04X}, a character the Profiles panel cannot show: herdr never writes one \
                     into a proposal, so nothing Accept writes is hidden from the owner",
                    u32::from(c)
                )
            })
        })
        .collect()
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
    make_folder(root)?;
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
    let file = lock_file(root)?;
    file.lock()
        .map_err(|err| format!("the proposals' lock: {err}"))?;
    Ok(Lock(file))
}

/// P45a fix round (review finding 6): the lock without the wait, for the window's key path, which must never block on
/// another herdr's write. `Ok(None)`: another holder has it now.
pub(crate) fn try_lock(root: &Path) -> Result<Option<Lock>, String> {
    let file = lock_file(root)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(Lock(file))),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(err)) => Err(format!("the proposals' lock: {err}")),
    }
}

fn lock_file(root: &Path) -> Result<std::fs::File, String> {
    let path = root
        .join(nirvana_run::DIR)
        .join("runs")
        .join(".profile-proposals.lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|err| format!("cannot open {}: {err}", path.display()))
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
    read_one(root, id).map(|(proposal, _)| proposal)
}

/// `get`, and the file's text as read, which `accept` keeps in the ledger and a `Seen` names by its sha.
fn read_one(root: &Path, id: &str) -> Result<(Proposal, String), String> {
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
    let proposal = parse(&file, id, &text).map_err(|problems| problems.join("\n"))?;
    Ok((proposal, text))
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
    // P45b review F1, the last guard: after the redaction above, whatever is still secret-shaped is refused, not
    // written. The writers refuse it first; this holds for any writer, today's or a later one.
    problems.extend(secret_problems(&draft.body, &draft.evidence));
    problems.extend(invisible_problems(&draft.body, &draft.evidence));
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
    } else if let Some(waiting) = list(root)
        .iter()
        .find(|waiting| waiting.kind == draft.kind && waiting.profile == draft.profile)
    {
        // P45b, R3 ("one piece at a time"): a second proposal for the same profile under another key would wait beside
        // the first, and accepting one would stale the other. The writer proposes again once the owner has acted.
        problems.push(format!(
            "a {} proposal of {} is waiting already as {}: one piece at a time; accept or dismiss {} first",
            draft.kind.word(),
            draft.profile,
            waiting.id,
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
    make_folder(root).map_err(|err| {
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
    // P45b review F1: a file edited by hand since `write` is checked again before it becomes a profile.
    problems.extend(secret_problems(&proposal.body, &proposal.evidence));
    // P45a fix round: and nothing the panel could not have shown, a hand-written file included.
    problems.extend(invisible_problems(&proposal.body, &proposal.evidence));
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
    accept_locked(root, id, None).map_err(|refused| refused.to_string())
}

/// Why `accept_seen` or `try_dismiss` did nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Another herdr holds the proposals' lock now; nothing was waited for.
    Busy,
    /// The proposal or the profile it writes is not what the panel drew; nothing was accepted.
    Changed(String),
    /// The store's refusal, word for word, as `accept` and `dismiss` say it.
    No(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => f.write_str(BUSY),
            Self::Changed(why) | Self::No(why) => f.write_str(why),
        }
    }
}

/// What the panel says when the lock is held.
pub const BUSY: &str = "busy: another herdr is writing the proposals; press again";

/// The profile file Accept writes and its whole text after, built by `target` from a proposal `prepared` gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub file: PathBuf,
    /// A patch's profile file as it stands; `None` for a new profile.
    pub before: Option<String>,
    pub after: String,
}

/// P45a fix round (review findings 1 and 3): what the Profiles panel drew of a waiting proposal, and what its Accept is
/// held to. `sha` is of the proposal file's text as read; `target` is what `accept` would write, by the same
/// `prepared` and `target` it writes through, the redaction included. For a patch `target.before` is the profile file
/// as it stood, so a profile changed since the draw is a change too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub id: String,
    pub sha: String,
    pub target: Result<Target, String>,
}

/// A proposal as Accept writes it: its body redacted (P45c review F5: the file may have been edited by hand since
/// `write`, so what goes into the profile is redacted again, and checked as it will be written).
pub fn prepared(mut proposal: Proposal) -> Proposal {
    proposal.body = nirvana_lessons::redact(&proposal.body);
    proposal
}

/// The file Accept writes for a `prepared` proposal and its text after: a `new` one's `profiles/<name>/profile.md`,
/// its body trimmed; a `patch`'s profile file with its front matter kept and its body replaced. Reads, writes nothing.
pub fn target(root: &Path, proposal: &Proposal) -> Result<Target, String> {
    let name = &proposal.profile;
    match proposal.kind {
        Kind::New => Ok(Target {
            file: nirvana_profiles::folder_of(root, name).join(nirvana_profiles::PROFILE_FILE),
            before: None,
            after: format!("{}\n", proposal.body.trim()),
        }),
        Kind::Patch => {
            let file = nirvana_profiles::resolve(root, name)?;
            let before = std::fs::read_to_string(&file)
                .map_err(|err| format!("cannot read {}: {err}", file.display()))?;
            let after = patched(&before, &proposal.body)
                .ok_or_else(|| format!("{} has no front matter to keep", file.display()))?;
            Ok(Target {
                file,
                before: Some(before),
                after,
            })
        }
    }
}

/// Every proposal waiting, as `list` reads them, each with what the panel shows of it: one read of each file, so the
/// proposal and its `Seen` are of the same text.
pub fn list_seen(root: &Path) -> Vec<(Proposal, Seen)> {
    let mut found: Vec<(Proposal, Seen)> = files(root)
        .into_iter()
        .filter_map(|(id, file)| {
            let text = std::fs::read_to_string(&file).ok()?;
            let proposal = parse(&file, &id, &text).ok()?;
            let seen = Seen {
                id: id.clone(),
                sha: nirvana_lessons::sha256(&text),
                target: target(root, &prepared(proposal.clone())),
            };
            Some((proposal, seen))
        })
        .collect();
    found.sort_by_key(|(proposal, _)| number(&proposal.id));
    found
}

/// The panel's Accept (P45a fix round): `accept`, refused with `Refused::Changed` inside the lock when the proposal
/// file or the profile file it writes is not what `seen` says was drawn, and with `Refused::Busy` at once when the lock
/// is held, never waiting on it. The CLI's `accept` is unchanged.
pub fn accept_seen(root: &Path, seen: &Seen) -> Result<String, Refused> {
    let _lock = try_lock(root).map_err(Refused::No)?.ok_or(Refused::Busy)?;
    accept_locked(root, &seen.id, Some(seen))
}

fn accept_locked(root: &Path, id: &str, seen: Option<&Seen>) -> Result<String, Refused> {
    let (proposal, text) = match read_one(root, id) {
        Ok(read) => read,
        Err(why) if seen.is_some() => return Err(Refused::Changed(why)),
        Err(why) => return Err(Refused::No(why)),
    };
    let proposal = prepared(proposal);
    let target = target(root, &proposal);
    if let Some(seen) = seen {
        if seen.sha != nirvana_lessons::sha256(&text) || seen.target != target {
            return Err(Refused::Changed(format!(
                "{id} changed since the panel drew it, so nothing was accepted: it is open again from the top; \
                 read it to its end and press a"
            )));
        }
    }
    let problems = accept_problems(root, &proposal);
    if !problems.is_empty() {
        return Err(Refused::No(format!("{id} is refused: {}", problems.join("; "))));
    }
    let name = &proposal.profile;
    let Target {
        file,
        before,
        after,
    } = target.map_err(Refused::No)?;
    if proposal.kind == Kind::New {
        let dir = nirvana_profiles::folder_of(root, name);
        std::fs::create_dir_all(dir.join(nirvana_profiles::KNOWLEDGE)).map_err(|err| {
            remove_empty(&dir);
            Refused::No(format!("cannot create {}: {err}", dir.display()))
        })?;
    }
    crate::nirvana_seat::write_atomic(&file, after.as_bytes()).map_err(|err| {
        // P45c review F7: a new profile's folder made above goes again, so a failed accept leaves nothing behind.
        if proposal.kind == Kind::New {
            remove_empty(&nirvana_profiles::folder_of(root, name));
        }
        Refused::No(format!("cannot write {}: {err}", file.display()))
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
        return Err(Refused::No(format!(
            "the ledger: {err}{}",
            undone
                .err()
                .map(|e| format!("; and the profile could not be put back: {e}"))
                .unwrap_or_default()
        )));
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
    dismiss_locked(root, id, reason)
}

/// The panel's Dismiss (P45a fix round, review finding 6): `dismiss`, or `Refused::Busy` at once when the lock is held.
pub fn try_dismiss(root: &Path, id: &str, reason: Option<&str>) -> Result<String, Refused> {
    let _lock = try_lock(root).map_err(Refused::No)?.ok_or(Refused::Busy)?;
    dismiss_locked(root, id, reason).map_err(Refused::No)
}

fn dismiss_locked(root: &Path, id: &str, reason: Option<&str>) -> Result<String, String> {
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
        make_folder(root).map_err(|err| err.to_string())?;
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

/// P45b second fix round: the line after an accept's success line. The proposal sat in an ignored folder; the profile
/// it became is not ignored, and the check that kept a secret out of it is a pattern list the second review got 20 of
/// 40 shapes past. One line, so the owner reads it.
pub const ACCEPT_NOTE: &str = "note: the profile file is not ignored by git, and herdr's secret check is a pattern \
list: read its body before you commit it";

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
        ["accept", id] => {
            let code = said(accept(root, id), "accept", out, err);
            if code == 0 {
                let _ = writeln!(out, "{ACCEPT_NOTE}");
            }
            code
        }
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
