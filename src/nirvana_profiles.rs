//! Nirvana fork, plan 33 P33a: agent profiles per project, `herdr profile list|show|new|check`.
//! (nirvana/plans/2026-10-30-33-profiles-and-learning.md §2)
//!
//! The owner, 2026-10-30 (plan 33 L1): "For our next feature I want to implement the profile agent from Hermes.
//! Essentially give each project level aa designable agent profiles."
//!
//! Measured before (§1): every Claude worker got the same argv and no system prompt. Plan 31's audit found 22 of 22
//! Bambu GUI orders seated with no server, because nothing tied an order's kind to the server it needed.
//!
//! - **The file** is `<project>/.nirvana/profiles/<name>.md`, tracked with the project: a `---` front matter of
//!   `description` (required, at most 120 characters), `class`, `model`, `family`, `effort`, `needs` and
//!   `disallowed_tools`, then the persona body, at most 4,000 characters, because it is paid on every call of the
//!   worker (decision 0023). Any other key is refused with the file and the line, as `.nirvana/seats.toml` is. A key
//!   naming a Send is refused by name: plan 31's approval is per order, `seat run --send-ok`, never a profile's.
//! - **Seating** reads the file once: its bytes are parsed, hashed and copied into the run folder,
//!   `<run>/profiles/<name>-<sha16>.md`. The worker gets the copy, so an edit after seating never reaches a running
//!   order, and the order copy names it in two lines at line start, `Profile: <name>` (P33b's contract) and
//!   `Profile sha256: <hex>`. A send-back or a reseat quotes its parent's order text, so it keeps both lines and the
//!   same copy; the supervisor checks the copy's sha before every start.
//! - **The worker.** Claude gets `--append-system-prompt-file` with the body and `--disallowedTools`; agy gets the
//!   body at the top of its order copy, quoted, so a body line can never read as one of the gate's line-start lines
//!   (`owns:`, `test:`).

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::nirvana_seat::{self, Effort};

/// The folder in `.nirvana/`; tracked with the project, as `lessons/` is.
pub const DIR: &str = "profiles";
/// §2.1: `<name>` is `[a-z0-9-]{1,32}`.
pub const NAME_MAX: usize = 32;
/// §2.1: "at most 120 characters: the line Nirvana Claude picks by".
pub const DESCRIPTION_MAX: usize = 120;
/// §2.1: "The body is at most 4,000 characters. It is paid on every call of the worker (decision 0023)."
pub const BODY_MAX: usize = 4000;
/// The keys a profile may hold, `description` first because it is the one required.
pub const KEYS: [&str; 7] = [
    "description",
    "class",
    "model",
    "family",
    "effort",
    "needs",
    "disallowed_tools",
];
/// The contract with P33b (plan 33 §3.2): the order copy carries this at line start when it was seated with a profile.
pub const LINE: &str = "Profile:";
/// The copy the order was seated with. Starts `Profile ` and not `Profile:`, so P33b's reader never takes it for the name.
pub const SHA_LINE: &str = "Profile sha256:";
/// §2.5: "At most 10 lines; more prints a count and `herdr profile list`."
pub const DIGEST_PROFILES: usize = 10;
/// The first characters of the sha in a run copy's file name: two copies of one profile in one run differ there.
const SHA_IN_NAME: usize = 16;

/// A profile as its file says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub class: Option<String>,
    pub model: Option<String>,
    pub family: Option<String>,
    pub effort: Option<Effort>,
    pub needs: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub body: String,
}

/// A profile read from the project's folder: what it parsed to, its bytes and their sha256.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub profile: Profile,
    pub bytes: Vec<u8>,
    pub sha: String,
}

/// The profile an order copy names: its name and the sha of the copy it was seated with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    pub name: String,
    pub sha: String,
}

pub fn valid_name(name: &str) -> bool {
    (1..=NAME_MAX).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub fn folder(root: &Path) -> PathBuf {
    root.join(crate::nirvana_run::DIR).join(DIR)
}

pub fn path(root: &Path, name: &str) -> PathBuf {
    folder(root).join(format!("{name}.md"))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn word(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'))
}

/// `[a, b]` on one line, items trimmed and unquoted.
fn one_line(value: &str) -> Result<Vec<String>, String> {
    let value = value.trim();
    let inner = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| format!("expected a list on one line, `[a, b]`, not `{value}`"))?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|item| {
            let item = unquote(item);
            if item.is_empty() {
                Err("an empty item in the list".to_owned())
            } else {
                Ok(item.to_owned())
            }
        })
        .collect()
}

/// Parses a profile's text. `path` names the file in every refusal and `name` is its file's stem. Every problem is
/// returned, each on its own line with the file and, where it has one, the line, so `herdr profile check` prints all.
pub fn parse(path: &Path, name: &str, text: &str) -> Result<Profile, Vec<String>> {
    let shown = path.display();
    let mut problems = Vec::new();
    if !valid_name(name) {
        problems.push(format!(
            "{shown}: the name `{name}` is not 1 to {NAME_MAX} of [a-z0-9-]"
        ));
    }
    // An editor's byte-order mark would read as part of the first line (plan 23's parser does the same).
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text.lines().collect();
    if lines.first().map(|line| line.trim_end()) != Some("---") {
        problems.push(format!(
            "{shown}: line 1: a profile starts with a `---` line, then its keys, then `---`, then the body"
        ));
        return Err(problems);
    }
    let Some(close) = lines
        .iter()
        .skip(1)
        .position(|line| line.trim_end() == "---")
        .map(|at| at + 1)
    else {
        problems.push(format!("{shown}: no closing `---` line after the keys"));
        return Err(problems);
    };
    let mut profile = Profile {
        name: name.to_owned(),
        description: String::new(),
        class: None,
        model: None,
        family: None,
        effort: None,
        needs: Vec::new(),
        disallowed_tools: Vec::new(),
        body: String::new(),
    };
    let mut seen: Vec<&str> = Vec::new();
    let mut described = false;
    for (index, line) in lines.iter().enumerate().take(close).skip(1) {
        let line_no = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            problems.push(format!(
                "{shown}: line {line_no}: expected `key: value` (a list goes on one line, `[a, b]`)"
            ));
            continue;
        };
        let key = key.trim();
        // Plan 31 §2.2 stands: a Send is approved per order, from the owner's own words, never by a profile.
        if key.to_ascii_lowercase().contains("send") {
            problems.push(format!(
                "{shown}: line {line_no}: `{key}`: a profile never grants a Send; the owner's approval is per order, \
                 `herdr seat run --send-ok` (plan 31)"
            ));
            continue;
        }
        let Some(known) = KEYS.iter().find(|known| **known == key) else {
            problems.push(format!(
                "{shown}: line {line_no}: unknown key `{key}` (the keys: {})",
                KEYS.join(", ")
            ));
            continue;
        };
        if seen.contains(known) {
            problems.push(format!("{shown}: line {line_no}: duplicate `{key}`"));
            continue;
        }
        seen.push(known);
        let scalar = unquote(value);
        let mut bad = |why: String| problems.push(format!("{shown}: line {line_no}: {key}: {why}"));
        match *known {
            "description" => {
                described = true;
                let count = scalar.chars().count();
                if scalar.is_empty() {
                    bad("empty; it is the line Nirvana Claude picks the profile by".into());
                } else if count > DESCRIPTION_MAX {
                    bad(format!(
                        "{count} characters; at most {DESCRIPTION_MAX}, the line Nirvana Claude picks by"
                    ));
                } else {
                    profile.description = scalar.to_owned();
                }
            }
            "class" | "model" | "family" if !word(scalar) => {
                bad(format!("`{scalar}` is not one word"));
            }
            "class" => profile.class = Some(scalar.to_owned()),
            "model" => profile.model = Some(scalar.to_owned()),
            "family" => profile.family = Some(scalar.to_owned()),
            "effort" => match Effort::parse(scalar) {
                Some(effort) => profile.effort = Some(effort),
                None => bad(format!(
                    "`{scalar}` is none of low, medium, high, xhigh, max"
                )),
            },
            "needs" => match one_line(value) {
                Ok(items) => match items.iter().find(|item| !word(item)) {
                    Some(item) => bad(format!("`{item}` is not one word")),
                    None => profile.needs = items,
                },
                Err(why) => bad(why),
            },
            // `--disallowedTools` takes the list as one comma-joined value, as `--allowedTools` does (plan 29).
            "disallowed_tools" => match one_line(value) {
                Ok(items) => match items.iter().find(|item| item.contains(',')) {
                    Some(item) => bad(format!("`{item}` holds a comma")),
                    None => profile.disallowed_tools = items,
                },
                Err(why) => bad(why),
            },
            _ => unreachable!("every key is matched above"),
        }
    }
    if !described {
        problems.push(format!(
            "{shown}: no `description` (required: the line Nirvana Claude picks the profile by)"
        ));
    }
    let body = lines[close + 1..].join("\n");
    let body = body.trim();
    let count = body.chars().count();
    if count > BODY_MAX {
        problems.push(format!(
            "{shown}: the body is {count} characters; at most {BODY_MAX}, because it is paid on every call of the \
             worker (decision 0023)"
        ));
    }
    if body.contains("--send-ok") || body.contains(crate::nirvana_gui_tools::SEND_LINE) {
        problems.push(format!(
            "{shown}: the body grants a Send; a profile never does, the owner's approval is per order (plan 31)"
        ));
    }
    // herdr's own footer mark: an order holding it is never given the footer (`write_order`).
    if body.contains(crate::nirvana_worker::FOOTER_MARK) {
        problems.push(format!(
            "{shown}: the body holds herdr's footer mark `{}`",
            crate::nirvana_worker::FOOTER_MARK
        ));
    }
    profile.body = body.to_owned();
    if problems.is_empty() {
        Ok(profile)
    } else {
        Err(problems)
    }
}

/// Reads `<root>/.nirvana/profiles/<name>.md` once: parsed, and its bytes with their sha.
pub fn load(root: &Path, name: &str) -> Result<Loaded, String> {
    if !valid_name(name) {
        return Err(format!(
            "--profile {name:?}: a profile's name is 1 to {NAME_MAX} of [a-z0-9-]"
        ));
    }
    let file = path(root, name);
    let bytes = std::fs::read(&file).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => format!(
            "no profile {name}: {} is not there; `herdr profile list` names this project's",
            file.display()
        ),
        _ => format!("cannot read {}: {err}", file.display()),
    })?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| format!("{} is not UTF-8", file.display()))?;
    let profile = parse(&file, name, text).map_err(|problems| {
        format!(
            "profile {name} is refused; `herdr profile check {name}`:\n{}",
            problems.join("\n")
        )
    })?;
    Ok(Loaded {
        profile,
        sha: sha256_hex(&bytes),
        bytes,
    })
}

/// Where the run keeps the copy an order was seated with.
pub fn copy_path(run_dir: &Path, name: &str, sha: &str) -> PathBuf {
    let short: String = sha.chars().take(SHA_IN_NAME).collect();
    run_dir.join(DIR).join(format!("{name}-{short}.md"))
}

/// The file a claude worker's `--append-system-prompt-file` names: the copy's body alone.
pub fn body_path(run_dir: &Path, name: &str, sha: &str) -> PathBuf {
    copy_path(run_dir, name, sha).with_extension("body.md")
}

/// §2.2: "The profile file is copied, with its sha256, into the run folder at seating." The bytes `load` read, so the
/// copy is what was parsed; a copy already there with the same sha is kept.
pub fn copy_into_run(run_dir: &Path, loaded: &Loaded) -> Result<PathBuf, String> {
    let file = copy_path(run_dir, &loaded.profile.name, &loaded.sha);
    if std::fs::read(&file).is_ok_and(|bytes| sha256_hex(&bytes) == loaded.sha) {
        return Ok(file);
    }
    let dir = file.parent().unwrap_or(run_dir);
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    nirvana_seat::write_atomic(&file, &loaded.bytes)
        .map_err(|err| format!("cannot write {}: {err}", file.display()))?;
    Ok(file)
}

/// The profile an order copy names, from its two lines at line start; `None` when it names none, an error when the
/// lines are not herdr's (no sha, a bad name).
pub fn in_order(text: &str) -> Option<Result<Ref, String>> {
    let name = text
        .lines()
        .find_map(|line| line.strip_prefix(LINE))?
        .trim()
        .to_owned();
    let sha = text
        .lines()
        .find_map(|line| line.strip_prefix(SHA_LINE))
        .map(|sha| sha.trim().to_owned());
    Some(match sha {
        _ if !valid_name(&name) => Err(format!(
            "the order's `{LINE} {name}` names no profile: 1 to {NAME_MAX} of [a-z0-9-]"
        )),
        Some(sha) if sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Ok(Ref { name, sha })
        }
        _ => Err(format!(
            "the order says `{LINE} {name}` with no `{SHA_LINE}` herdr wrote: seat it with `--profile {name}`"
        )),
    })
}

/// The copy an order was seated with, read from the run folder. Its sha is checked first: a copy that changed since
/// is never given to a worker.
pub fn from_run(run_dir: &Path, profile: &Ref) -> Result<Profile, String> {
    let file = copy_path(run_dir, &profile.name, &profile.sha);
    let bytes = std::fs::read(&file).map_err(|err| {
        format!(
            "the profile copy {} the order was seated with cannot be read: {err}",
            file.display()
        )
    })?;
    let sha = sha256_hex(&bytes);
    if sha != profile.sha {
        return Err(format!(
            "the profile copy {} has sha256 {sha}, not the {} the order was seated with",
            file.display(),
            profile.sha
        ));
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|_| format!("{} is not UTF-8", file.display()))?;
    parse(&file, &profile.name, text).map_err(|problems| problems.join("\n"))
}

pub fn order_lines(profile: &Ref) -> String {
    format!("{LINE} {}\n{SHA_LINE} {}\n", profile.name, profile.sha)
}

fn section_head(name: &str) -> String {
    format!("## Profile: {name}")
}

fn section_end(name: &str) -> String {
    format!("(end of profile {name})")
}

/// §2.3, agy: the body under `## Profile: <name>`, each line quoted. A quoted line starts `> `, so none of the gate's
/// line-start lines (`owns:`, `test:`, `Profile:`) can come from a profile's body.
pub fn agy_section(name: &str, body: &str) -> String {
    let quoted: Vec<String> = body
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                ">".to_owned()
            } else {
                format!("> {line}")
            }
        })
        .collect();
    format!(
        "{}\n\n{}\n\n{}\n\n",
        section_head(name),
        quoted.join("\n"),
        section_end(name)
    )
}

/// The text with every agy section of the profile taken out: a claude child of an agy order gets the body as its
/// system prompt instead, and an agy child gets one section, at its top.
pub fn strip_sections(text: &str, name: &str) -> String {
    let (head, end) = (section_head(name), section_end(name));
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < lines.len() {
        let line = lines[at].trim_end_matches(['\r', '\n']);
        if line == head {
            if let Some(close) = (at + 1..lines.len())
                .find(|&next| lines[next].trim_end_matches(['\r', '\n']) == end)
            {
                at = close + 1;
                if lines
                    .get(at)
                    .is_some_and(|next| next.trim_end_matches(['\r', '\n']).is_empty())
                {
                    at += 1;
                }
                continue;
            }
        }
        out.push_str(lines[at]);
        at += 1;
    }
    out
}

fn strip_profile_lines(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| !line.starts_with(LINE) && !line.starts_with(SHA_LINE))
        .collect()
}

/// The order copy a worker on `cli` reads for an order seated with `profile`: the two lines at line start once, and
/// for agy the quoted body at the top. Pure; `seat run` writes it and `seat exec` rewrites a child's to its own CLI.
pub fn order_text(text: &str, profile: &Ref, parsed: &Profile, cli: &str) -> String {
    let mut out = strip_sections(text, &profile.name);
    if !matches!(in_order(&out), Some(Ok(known)) if known == *profile) {
        out = format!("{}\n{}", order_lines(profile), strip_profile_lines(&out));
    }
    if cli == "agy" && !parsed.body.is_empty() {
        out = format!("{}{out}", agy_section(&profile.name, &parsed.body));
    }
    out
}

/// §2.2: the order's needs, then the profile's not already among them, before plan 29's server routing.
pub fn joined_needs(order: &[String], profile: &Profile) -> Vec<String> {
    let mut needs = order.to_vec();
    for need in &profile.needs {
        if !needs.contains(need) {
            needs.push(need.clone());
        }
    }
    needs
}

/// §2.2: the profile's model and family narrow the seats and its effort replaces the class's. A profile that forbids
/// tools takes only a claude seat: only the claude argv carries `--disallowedTools`, and a rule a worker never got is
/// no rule.
pub fn narrow(request: &mut nirvana_seat::Request, profile: &Profile) {
    request.profile = Some(nirvana_seat::Narrow {
        name: profile.name.clone(),
        model: profile.model.clone(),
        family: profile.family.clone(),
        effort: profile.effort,
    });
    if !profile.disallowed_tools.is_empty() && request.claude_only.is_none() {
        request.claude_only = Some(format!(
            "profile {} forbids tools ({}), and only a claude worker's --disallowedTools enforces that (plan 33 §2.3)",
            profile.name,
            profile.disallowed_tools.join(", ")
        ));
    }
}

/// The narrowing for a child of an order: from the run's copy its parent was seated with, never the project's file,
/// which may have changed since. Nothing when the text names no profile.
pub fn narrow_for_order(
    request: &mut nirvana_seat::Request,
    run_dir: &Path,
    text: &str,
) -> Result<(), String> {
    match in_order(text) {
        None => Ok(()),
        Some(found) => {
            let parsed = from_run(run_dir, &found?)?;
            narrow(request, &parsed);
            Ok(())
        }
    }
}

/// §2.3, claude: the flags a worker of this profile starts with, `--append-system-prompt-file` naming the copy's body
/// (written beside the copy) and `--disallowedTools` with the list joined.
pub fn claude_args(run_dir: &Path, profile: &Ref, parsed: &Profile) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    if !parsed.body.is_empty() {
        let file = body_path(run_dir, &profile.name, &profile.sha);
        let body = format!("{}\n", parsed.body);
        if std::fs::read_to_string(&file).ok().as_deref() != Some(body.as_str()) {
            nirvana_seat::write_atomic(&file, body.as_bytes())
                .map_err(|err| format!("cannot write {}: {err}", file.display()))?;
        }
        args.extend([
            "--append-system-prompt-file".to_owned(),
            file.display().to_string(),
        ]);
    }
    if !parsed.disallowed_tools.is_empty() {
        args.extend([
            "--disallowedTools".to_owned(),
            parsed.disallowed_tools.join(","),
        ]);
    }
    Ok(args)
}

/// Every file in the project's profiles folder, by name, with what it parsed to.
pub fn list(root: &Path) -> Vec<(String, PathBuf, Result<Profile, Vec<String>>)> {
    let Ok(entries) = std::fs::read_dir(folder(root)) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf, Result<Profile, Vec<String>>)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|file| file.is_file() && file.extension().is_some_and(|ext| ext == "md"))
        .map(|file| {
            let name = file
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let parsed = match std::fs::read(&file) {
                Ok(bytes) => match std::str::from_utf8(&bytes) {
                    Ok(text) => parse(&file, &name, text),
                    Err(_) => Err(vec![format!("{} is not UTF-8", file.display())]),
                },
                Err(err) => Err(vec![format!("cannot read {}: {err}", file.display())]),
            };
            (name, file, parsed)
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// The one-line summary of a profile: its description, then model or family and needs.
pub fn summary(profile: &Profile) -> String {
    let mut parts = Vec::new();
    match (&profile.model, &profile.family) {
        (Some(model), _) => parts.push(format!("model {model}")),
        (None, Some(family)) => parts.push(format!("family {family}")),
        (None, None) => {}
    }
    if !profile.needs.is_empty() {
        parts.push(format!("needs {}", profile.needs.join(", ")));
    }
    if parts.is_empty() {
        profile.description.clone()
    } else {
        format!("{} ({})", profile.description, parts.join("; "))
    }
}

/// §2.5: the session digest's profile lines. At most `DIGEST_PROFILES` profile lines; more ends with a count.
pub fn digest_lines(root: &Path) -> Vec<String> {
    let profiles = list(root);
    if profiles.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![
        "Profiles here (an order that fits one is seated with `--profile <name>`):".to_owned(),
    ];
    for (name, _, parsed) in profiles.iter().take(DIGEST_PROFILES) {
        lines.push(match parsed {
            Ok(profile) => format!("- {name}: {}", summary(profile)),
            Err(_) => format!("- {name}: refused; `herdr profile check {name}`"),
        });
    }
    if profiles.len() > DIGEST_PROFILES {
        lines.push(format!(
            "- and {} more: `herdr profile list`",
            profiles.len() - DIGEST_PROFILES
        ));
    }
    lines
}

/// §2.4: the file `herdr profile new` writes, with the project's `.mcp.json` server names as a comment.
pub fn template(servers: &[String]) -> String {
    let servers = if servers.is_empty() {
        "none (no .mcp.json here)".to_owned()
    } else {
        servers.join(", ")
    };
    format!(
        "---\n\
description: One line, at most 120 characters, that Nirvana Claude picks this profile by.\n\
# The other keys are optional; take the # off to use one.\n\
# class: build.bounded\n\
# model: claude-sonnet-5-5\n\
# family: anthropic\n\
# effort: medium\n\
# needs: [a-server]\n\
# disallowed_tools: []\n\
# This project's MCP servers (.mcp.json): {servers}\n\
# A profile never grants a Send: that is per order, `herdr seat run --send-ok`.\n\
---\n\
You are the worker for one kind of order in this project. Say what you do, what you never do, and how you check \
your work. At most {BODY_MAX} characters: this is paid on every call of the worker.\n"
    )
}

/// `herdr profile new <name>`: the template, never over a file already there.
pub fn new_profile(root: &Path, name: &str) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err(format!(
            "{name:?}: a profile's name is 1 to {NAME_MAX} of [a-z0-9-]"
        ));
    }
    let file = path(root, name);
    if file.exists() {
        return Err(format!("{} is already there", file.display()));
    }
    let servers = crate::nirvana_gui_tools::project_servers(root).unwrap_or_default();
    let text = template(&servers);
    parse(&file, name, &text).map_err(|problems| problems.join("\n"))?;
    let dir = folder(root);
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    std::fs::write(&file, text).map_err(|err| format!("cannot write {}: {err}", file.display()))?;
    Ok(file)
}

/// What `check` adds to the parser's refusals: the class, model, family and needs read against the seat table and
/// the project's `.mcp.json`, so a typo is found before an order waits on it.
pub fn table_problems(
    file: &Path,
    profile: &Profile,
    table: &nirvana_seat::Table,
    servers: &[String],
) -> Vec<String> {
    let shown = file.display();
    let mut problems = Vec::new();
    if let Some(class) = &profile.class {
        if table.class(class).is_none() {
            problems.push(format!(
                "{shown}: class {class}: no such class in seats.toml"
            ));
        }
    }
    let narrow = nirvana_seat::Narrow {
        name: profile.name.clone(),
        model: profile.model.clone(),
        family: profile.family.clone(),
        effort: profile.effort,
    };
    if table
        .seats
        .iter()
        .all(|seat| narrow.refuses(seat).is_some())
    {
        problems.push(format!(
            "{shown}: no seat in seats.toml has {}",
            [
                profile.model.as_ref().map(|model| format!("model {model}")),
                profile
                    .family
                    .as_ref()
                    .map(|family| format!("family {family}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" and ")
        ));
    }
    let capabilities = crate::nirvana_gui_tools::capabilities(table);
    for need in &profile.needs {
        if !capabilities.contains(&need.as_str()) && !servers.contains(need) {
            problems.push(format!(
                "{shown}: needs {need}: neither a seat capability nor a server in .mcp.json"
            ));
        }
    }
    problems
}

const USAGE: &str = "usage: herdr profile list
       herdr profile show <name>
       herdr profile new <name>          writes .nirvana/profiles/<name>.md from the template
       herdr profile check [<name>]      prints every refusal; exit 1 when there is one
A profile is used by `herdr seat run <seat> --profile <name> ...` and `herdr seat pick --profile <name> ...`.";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let root = crate::nirvana_run::Project::from_env().root;
    let code = match (
        args.first().map(String::as_str),
        args.get(1..).unwrap_or(&[]),
    ) {
        (Some("list"), []) => {
            let profiles = list(&root);
            if profiles.is_empty() {
                println!(
                    "no profiles in {}; `herdr profile new <name>` makes one",
                    folder(&root).display()
                );
            }
            for (name, _, parsed) in &profiles {
                match parsed {
                    Ok(profile) => println!("{name}: {}", summary(profile)),
                    Err(_) => println!("{name}: refused; `herdr profile check {name}`"),
                }
            }
            0
        }
        (Some("show"), [name]) => match load(&root, name) {
            Ok(loaded) => {
                println!("{} · sha256 {}", path(&root, name).display(), loaded.sha);
                print!("{}", String::from_utf8_lossy(&loaded.bytes));
                0
            }
            Err(why) => {
                eprintln!("herdr profile show: {why}");
                1
            }
        },
        (Some("new"), [name]) => match new_profile(&root, name) {
            Ok(file) => {
                println!("{}", file.display());
                0
            }
            Err(why) => {
                eprintln!("herdr profile new: {why}");
                1
            }
        },
        (Some("check"), names) if names.len() <= 1 => {
            let table = nirvana_seat::table_for(&root);
            if let Err(why) = &table {
                println!("seats.toml is refused, so only the files are checked: {why}");
            }
            let servers = crate::nirvana_gui_tools::project_servers(&root).unwrap_or_default();
            let chosen: Vec<_> = list(&root)
                .into_iter()
                .filter(|(name, _, _)| names.first().is_none_or(|wanted| wanted == name))
                .collect();
            if chosen.is_empty() {
                println!(
                    "no profile {}in {}",
                    names
                        .first()
                        .map(|name| format!("{name} "))
                        .unwrap_or_default(),
                    folder(&root).display()
                );
            }
            let mut refused = 0;
            for (name, file, parsed) in &chosen {
                let problems = match (parsed, &table) {
                    (Ok(profile), Ok(table)) => table_problems(file, profile, table, &servers),
                    (Ok(_), Err(_)) => Vec::new(),
                    (Err(problems), _) => problems.clone(),
                };
                if problems.is_empty() {
                    println!("{name}: ok");
                } else {
                    refused += 1;
                    for problem in problems {
                        println!("{problem}");
                    }
                }
            }
            i32::from(refused > 0 || (chosen.is_empty() && !names.is_empty()))
        }
        _ => usage_exit(),
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests;
