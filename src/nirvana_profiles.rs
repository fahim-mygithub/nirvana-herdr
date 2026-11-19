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
//!   (`owns:`, `test:`), and, from plan 35 §3.4, its `disallowed_tools` as `--settings` deny rules.
//!
//! Plan 45 P45c (nirvana/plans/2026-11-18-45-agent-profiles.md §5), the owner's "specialized in holding tools, mcp and
//! most importantly context on how to process a task":
//! - **`skills: [a, b]`** names project skills (`.claude/skills/<name>/SKILL.md`); `herdr profile check` refuses one that
//!   is not there. An agy worker gets only those copied (`nirvana_agy_tools::with_tools`; without the key it gets all,
//!   as before). A claude worker loads every project skill through `--setting-sources project` and no flag herdr has
//!   measured limits them, so its system prompt gets one line naming the skills to use (`claude_args`): an instruction,
//!   not enforcement.
//! - **`<name>/mcp.json`**, a folder profile's own servers in a project `.mcp.json`'s shape, are granted to every order
//!   seated with the profile, merged into the per-order config as a `needs` server is (`nirvana_gui_tools`). A name the
//!   project's `.mcp.json` defines too is refused (no silent override), and agy's limits hold (`servers_claude_only`).
//!   The run pins it: its bytes are copied beside the profile copy, `<name>-<sha16>.mcp.json`, and the order's
//!   `Profile sha256:` is then `seated_sha` over both files, so the check before every start covers the servers too and
//!   an edit mid-run never reaches a running order. A profile with no `mcp.json` keeps the sha of its file alone.
//! - **Shapes**: `herdr profile new <name> --shape reviewer|operator|builder|researcher` writes the folder form with a
//!   starter body and starter cards, adopted, so the profile passes `profile check` as written.
//! - **Teaching** takes none of the taught profile's seat pins (`narrow`): Megajam, 2026-11-16, a teach was refused on
//!   the profile's `family` and `needs` and the orchestrator commented them out with `sed` to teach.

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
pub const KEYS: [&str; 8] = [
    "description",
    "class",
    "model",
    "family",
    "effort",
    "needs",
    "disallowed_tools",
    "skills",
];
/// Plan 45 §5: a folder profile's own servers, `<name>/mcp.json`, in a project `.mcp.json`'s shape.
pub const SERVERS_FILE: &str = "mcp.json";
/// Plan 45 §5: the shapes `herdr profile new <name> --shape <shape>` writes, from the profiles that worked.
pub const SHAPES: [&str; 4] = ["reviewer", "operator", "builder", "researcher"];
/// What `seated_sha` puts between the profile's bytes and its `mcp.json`'s, so no split of one text into two files
/// hashes like another split.
const SERVERS_SEP: &[u8] = b"\n\0herdr: the profile's mcp.json follows\0\n";
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
    /// Plan 45 §5: the project skills the worker is to use; empty for no selection (every skill, as before).
    pub skills: Vec<String>,
    pub body: String,
    /// Plan 45 §5: the server names of the profile's own `mcp.json`. The parser leaves it empty; `load` and `from_run`
    /// fill it from the file, never the definitions (a definition can hold a token, and a `Profile` is printed).
    pub servers: Vec<String>,
    /// Why only a claude worker can take the profile's own servers (a desktop server, `${...}`, an unmeasured type),
    /// as `nirvana_gui_tools::claude_only` says for a `needs` server; `None` when agy can take them or there are none.
    pub servers_claude_only: Option<String>,
}

/// A profile read from the project's folder: what it parsed to, its bytes and the sha it is seated with.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub profile: Profile,
    pub bytes: Vec<u8>,
    /// `seated_sha`: of `bytes` alone, or of `bytes` and the profile's `mcp.json` when it has one.
    pub sha: String,
    /// Plan 45 §5: the profile's own `mcp.json` as read with `bytes`; `None` when it has none.
    pub servers: Option<ServerFile>,
}

/// A profile's `mcp.json` bytes. Its `Debug` names the servers and the size only: the definitions can hold a token.
#[derive(Clone)]
pub struct ServerFile {
    pub bytes: Vec<u8>,
    pub names: Vec<String>,
    pub claude_only: Option<String>,
}

impl std::fmt::Debug for ServerFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ServerFile {{ names: {:?}, bytes: {} }}",
            self.names,
            self.bytes.len()
        )
    }
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

/// Plan 34 §2.1: a profile may be a folder, `<name>/profile.md` (the same format as `<name>.md`) beside `knowledge/`.
pub const PROFILE_FILE: &str = "profile.md";
pub const KNOWLEDGE: &str = "knowledge";

/// The folder a folder profile lives in, whether or not it exists.
pub fn folder_of(root: &Path, name: &str) -> PathBuf {
    folder(root).join(name)
}

/// The file a profile's name reads: `<name>/profile.md` when the folder is there, else `<name>.md`. A name that is both
/// is refused: which one the worker got would depend on herdr's order of looking.
pub fn resolve(root: &Path, name: &str) -> Result<PathBuf, String> {
    let (file, dir) = (path(root, name), folder_of(root, name));
    match (file.is_file(), dir.is_dir()) {
        (true, true) => Err(format!(
            "profile {name} is both {} and the folder {}: keep one",
            file.display(),
            dir.display()
        )),
        (_, true) => Ok(dir.join(PROFILE_FILE)),
        _ => Ok(file),
    }
}

/// A folder profile's `knowledge/`; `None` for a profile that is one file, which keeps no cards.
pub fn knowledge_dir(root: &Path, name: &str) -> Option<PathBuf> {
    valid_name(name)
        .then(|| folder_of(root, name))
        .filter(|dir| dir.is_dir() && !path(root, name).is_file())
        .map(|dir| dir.join(KNOWLEDGE))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The sha an order is seated with (`Profile sha256:`): of the profile file's bytes alone, as since plan 33, or, for a
/// profile with its own `mcp.json` (plan 45), of both, so the check before every start pins its servers too.
pub fn seated_sha(profile: &[u8], servers: Option<&[u8]>) -> String {
    match servers {
        None => sha256_hex(profile),
        Some(servers) => {
            let mut hash = Sha256::new();
            hash.update(profile);
            hash.update(SERVERS_SEP);
            hash.update(servers);
            format!("{:x}", hash.finalize())
        }
    }
}

/// A folder profile's `mcp.json`, whether or not it is there; `None` for a profile that is one file, which holds no
/// servers.
pub fn servers_path(root: &Path, name: &str) -> Option<PathBuf> {
    knowledge_dir(root, name).map(|_| folder_of(root, name).join(SERVERS_FILE))
}

/// A skill's folder name: letters, digits, `.`, `_` and `-`, never starting with `.` (so never `..`).
fn skill_word(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('.')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A profile's own servers as `servers_problems` reads them: the names, then the definitions by name.
pub type Servers = (Vec<String>, serde_json::Map<String, serde_json::Value>);

/// The problems of a profile's `mcp.json` text, else its server names and definitions. `project` is the project's
/// `.mcp.json` names: a server defined in both is refused (a profile never overrides a project server in silence), and
/// a project file that cannot be read refuses the profile's servers, since a clash cannot be ruled out.
pub fn servers_problems(
    file: &Path,
    text: &str,
    project: &Result<Vec<String>, String>,
) -> Result<Servers, Vec<String>> {
    let shown = file.display();
    let defined = crate::nirvana_gui_tools::definitions(text).map_err(|why| {
        vec![format!(
            "{shown}: {why}; a profile's servers are `{{\"mcpServers\": {{...}}}}`, as in .mcp.json"
        )]
    })?;
    let mut problems = Vec::new();
    let project = match project {
        Ok(names) => names.clone(),
        Err(why) => {
            problems.push(format!(
                "{shown}: {why}, so a clash with the project's servers cannot be ruled out"
            ));
            Vec::new()
        }
    };
    let names: Vec<String> = defined.keys().cloned().collect();
    for (name, definition) in &defined {
        if !definition.is_object() {
            problems.push(format!("{shown}: server {name:?} is not an object"));
        }
        if name.trim().is_empty() {
            problems.push(format!("{shown}: a server with an empty name"));
        }
        if name == crate::nirvana_bladebro::SERVER {
            problems.push(format!(
                "{shown}: server {name}: the name is herdr's own browser server; rename it"
            ));
        }
        if project.contains(name) {
            problems.push(format!(
                "{shown}: server {name} is in the project's {} too; a profile never overrides a project server, so \
                 rename one",
                crate::nirvana_gui_tools::MCP_FILE
            ));
        }
    }
    let mut all = names.clone();
    all.extend(project.iter().filter(|name| !names.contains(name)).cloned());
    if let Some(why) = crate::nirvana_gui_tools::key_clash(&names, &all) {
        problems.push(format!("{shown}: {why}"));
    }
    if problems.is_empty() {
        Ok((names, defined))
    } else {
        Err(problems)
    }
}

/// The profile's own `mcp.json`, read and checked against the project's `.mcp.json`; `None` when it has none.
pub fn read_servers(root: &Path, name: &str) -> Result<Option<ServerFile>, Vec<String>> {
    let Some(file) = servers_path(root, name) else {
        return Ok(None);
    };
    let bytes = match std::fs::read(&file) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(vec![format!("cannot read {}: {err}", file.display())]),
    };
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| vec![format!("{} is not UTF-8", file.display())])?;
    let project = crate::nirvana_gui_tools::project_servers(root);
    let (names, defined) = servers_problems(&file, text, &project)?;
    let claude_only = crate::nirvana_gui_tools::claude_only(&names, &defined);
    Ok(Some(ServerFile {
        bytes,
        names,
        claude_only,
    }))
}

pub(crate) fn unquote(value: &str) -> &str {
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
pub(crate) fn one_line(value: &str) -> Result<Vec<String>, String> {
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
        skills: Vec::new(),
        body: String::new(),
        servers: Vec::new(),
        servers_claude_only: None,
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
            // Plan 45 §5: a skill is a folder name; `check` reads that it is there (`skill_problems`).
            "skills" => match one_line(value) {
                Ok(items) => match items.iter().find(|item| !skill_word(item)) {
                    Some(item) => bad(format!("`{item}` is not a skill's folder name")),
                    None => profile.skills = items,
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
    // Plan 34 §2.1: the run copy is `profile.md`'s bytes alone; the cards are read live, like lessons.
    let file = resolve(root, name)?;
    let bytes = std::fs::read(&file).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => format!(
            "no profile {name}: {} is not there; `herdr profile list` names this project's",
            file.display()
        ),
        _ => format!("cannot read {}: {err}", file.display()),
    })?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| format!("{} is not UTF-8", file.display()))?;
    let refused = |problems: Vec<String>| {
        format!(
            "profile {name} is refused; `herdr profile check {name}`:\n{}",
            problems.join("\n")
        )
    };
    let mut profile = parse(&file, name, text).map_err(refused)?;
    // Plan 45 §5: read with the profile file, once, so the copy and its sha are of what was checked.
    let servers = read_servers(root, name).map_err(refused)?;
    if let Some(found) = &servers {
        profile.servers = found.names.clone();
        profile.servers_claude_only = found.claude_only.clone();
    }
    Ok(Loaded {
        profile,
        sha: seated_sha(&bytes, servers.as_ref().map(|found| found.bytes.as_slice())),
        bytes,
        servers,
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

/// Plan 45 §5: where the run keeps the copy of a profile's own `mcp.json`, beside the profile copy of the same sha.
pub fn servers_copy_path(run_dir: &Path, name: &str, sha: &str) -> PathBuf {
    copy_path(run_dir, name, sha).with_extension("mcp.json")
}

/// The run's copies of `name` at `sha`, the profile copy's bytes and the servers copy's when there is one, with their
/// seated sha. `None` when the profile copy cannot be read.
fn copies(run_dir: &Path, name: &str, sha: &str) -> Option<(Vec<u8>, Option<Vec<u8>>, String)> {
    let bytes = std::fs::read(copy_path(run_dir, name, sha)).ok()?;
    let servers = std::fs::read(servers_copy_path(run_dir, name, sha)).ok();
    let seated = seated_sha(&bytes, servers.as_deref());
    Some((bytes, servers, seated))
}

/// §2.2: "The profile file is copied, with its sha256, into the run folder at seating." The bytes `load` read, so the
/// copy is what was parsed; a copy already there with the same sha is kept. Plan 45: a profile's own `mcp.json` is
/// copied beside it, the owner's alone where `std` can say so (`write_private`), since a definition can hold a token.
pub fn copy_into_run(run_dir: &Path, loaded: &Loaded) -> Result<PathBuf, String> {
    let name = &loaded.profile.name;
    let file = copy_path(run_dir, name, &loaded.sha);
    if copies(run_dir, name, &loaded.sha).is_some_and(|(_, _, sha)| sha == loaded.sha) {
        return Ok(file);
    }
    let dir = file.parent().unwrap_or(run_dir);
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    if let Some(servers) = &loaded.servers {
        let to = servers_copy_path(run_dir, name, &loaded.sha);
        crate::nirvana_gui_tools::write_private(&to, &servers.bytes)
            .map_err(|err| format!("cannot write {}: {err}", to.display()))?;
    }
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
/// is never given to a worker. Plan 45: the sha covers the servers copy too, so one added, removed or edited since
/// seating fails the same way.
pub fn from_run(run_dir: &Path, profile: &Ref) -> Result<Profile, String> {
    read_copy(run_dir, profile).map(|(parsed, _)| parsed)
}

/// Plan 45 §5: the definitions of the profile's own servers an order was seated with, from the run's copy, checked as
/// `from_run` checks it; empty for a profile with no `mcp.json`. The launch merges them into the order's config.
pub fn run_servers(
    run_dir: &Path,
    profile: &Ref,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    read_copy(run_dir, profile).map(|(_, servers)| servers.unwrap_or_default())
}

type Copy = (Profile, Option<serde_json::Map<String, serde_json::Value>>);

fn read_copy(run_dir: &Path, profile: &Ref) -> Result<Copy, String> {
    let file = copy_path(run_dir, &profile.name, &profile.sha);
    let (bytes, servers, sha) = copies(run_dir, &profile.name, &profile.sha).ok_or_else(|| {
        let err = std::fs::read(&file)
            .err()
            .map(|err| err.to_string())
            .unwrap_or_default();
        format!(
            "the profile copy {} the order was seated with cannot be read: {err}",
            file.display()
        )
    })?;
    if sha != profile.sha {
        return Err(format!(
            "the profile copy {} has sha256 {sha}, not the {} the order was seated with",
            file.display(),
            profile.sha
        ));
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|_| format!("{} is not UTF-8", file.display()))?;
    let mut parsed = parse(&file, &profile.name, text).map_err(|problems| problems.join("\n"))?;
    let Some(servers) = servers else {
        return Ok((parsed, None));
    };
    let shown = servers_copy_path(run_dir, &profile.name, &profile.sha);
    let text = std::str::from_utf8(&servers)
        .map_err(|_| format!("{} is not UTF-8", shown.display()))?;
    let defined = crate::nirvana_gui_tools::definitions(text)
        .map_err(|why| format!("{}: {why}", shown.display()))?;
    parsed.servers = defined.keys().cloned().collect();
    parsed.servers_claude_only = crate::nirvana_gui_tools::claude_only(&parsed.servers, &defined);
    Ok((parsed, Some(defined)))
}

/// The profile an order was seated with, as its `seated` event journals it: `seat run --profile` ends the reason with
/// `; profile <name> sha256 <hex>`. The journal, not the order's text: a lesson review's brief quotes another order's
/// text (plan 33 review B1).
pub fn from_reason(reason: &str) -> Option<String> {
    let (_, tail) = reason.rsplit_once("; profile ")?;
    let (name, sha) = tail.split_once(" sha256 ")?;
    (valid_name(name) && sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| name.to_owned())
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

pub fn strip_profile_lines(text: &str) -> String {
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
    // Plan 45 §5: the profile's own servers are granted as a `needs` server is, so the brief gets the same lines
    // (plan 29 review F4: every order granted a server is told never to start a device). Once: a reseat keeps them.
    crate::nirvana_gui_tools::with_granted_lines(out, &parsed.servers)
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

/// §2.2: the profile's model and family narrow the seats and its effort replaces the class's. Plan 35 §3.4: a profile
/// that forbids a tool with no measured agy rule takes only a claude seat, since a rule a worker never got is no rule;
/// one whose every tool maps (`nirvana_agy_tools::DENY_TABLE`, `mcp__*`) may seat on agy, denied through `--settings`.
///
/// Plan 45 §5 (M6), Megajam 2026-11-16: `herdr profile teach aaa-anim-reviewer` was refused on the profile's
/// `family: google` and `needs: [video]`, and the orchestrator commented them out with `sed` to teach. A teacher reads
/// a source and writes cards; it is not the profile's own work, so a `profile-teach` request keeps the profile's name
/// (its copy, body and cards still go with the order) and takes none of its model, family, effort or needs. Both
/// pickers that seat a teacher (`seat pick --profile` and `seat run`'s booking) pass through here.
pub fn narrow(request: &mut nirvana_seat::Request, profile: &Profile) {
    let teach = request.class == crate::nirvana_knowledge::TEACH_CLASS;
    request.profile = Some(nirvana_seat::Narrow {
        name: profile.name.clone(),
        model: profile.model.clone().filter(|_| !teach),
        family: profile.family.clone().filter(|_| !teach),
        effort: profile.effort.filter(|_| !teach),
    });
    if teach {
        // The teach order names no need of its own (`nirvana_knowledge::teach`), so each one here is the profile's.
        request.needs.retain(|need| !profile.needs.contains(need));
    }
    // Plan 45 §5: the profile's own servers hold agy's limits as a `needs` server does (plan 35 §3.2).
    if let Some(why) = &profile.servers_claude_only {
        if request.claude_only.is_none() {
            request.claude_only = Some(format!("profile {}'s own server: {why}", profile.name));
        }
    }
    let unmapped = crate::nirvana_agy_tools::unmapped(&profile.disallowed_tools);
    if !unmapped.is_empty() && request.claude_only.is_none() {
        request.claude_only = Some(format!(
            "profile {} forbids tools ({}), and agy has no measured rule for {}, so only a claude worker's \
             --disallowedTools enforces that (plan 35 §3.4)",
            profile.name,
            profile.disallowed_tools.join(", "),
            unmapped.join(", ")
        ));
    }
}

/// The narrowing for a child of an order: from the run's copy its parent was seated with, never the project's file,
/// which may have changed since. Nothing when the text names no profile, and nothing for a lesson review, whose brief
/// quotes the order it reviews (plan 33 review B1: a reseat would put the review on the profile's model or effort,
/// off the cheap seat).
pub fn narrow_for_order(
    request: &mut nirvana_seat::Request,
    run_dir: &Path,
    text: &str,
) -> Result<(), String> {
    if request.class == crate::nirvana_review::CLASS {
        return Ok(());
    }
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
    let mut prompt = parsed.body.clone();
    if !parsed.skills.is_empty() {
        if !prompt.is_empty() {
            prompt.push_str("\n\n");
        }
        prompt.push_str(&skills_line(&parsed.skills));
    }
    if !prompt.is_empty() {
        let file = body_path(run_dir, &profile.name, &profile.sha);
        let body = format!("{prompt}\n");
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

/// Plan 45 §5: the line a claude worker's system prompt gets for a profile's `skills`. Claude Code loads every project
/// skill through `--setting-sources project`, and no flag herdr has measured selects among them. The docs describe a
/// `Skill(name)` permission rule, which no probe here has run, so it is not used: a rule a worker may not get is no
/// rule. This line asks; it does not enforce (an agy worker is given only the named skills).
pub fn skills_line(skills: &[String]) -> String {
    format!(
        "Of this project's skills, use only these: {}. Leave the others unused.",
        skills.join(", ")
    )
}

/// Every file in the project's profiles folder, by name, with what it parsed to.
pub fn list(root: &Path) -> Vec<(String, PathBuf, Result<Profile, Vec<String>>)> {
    let Ok(entries) = std::fs::read_dir(folder(root)) else {
        return Vec::new();
    };
    // Plan 34 §2.1: a `<name>.md` file and a `<name>/` folder are both profiles; a name that is both is refused.
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter_map(|entry| {
            let file_name = entry.file_name()?.to_string_lossy().into_owned();
            if entry.is_dir() {
                Some(file_name)
            } else if entry.is_file() && entry.extension().is_some_and(|ext| ext == "md") {
                entry
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            let (file, parsed) = match resolve(root, &name) {
                Err(why) => (path(root, &name), Err(vec![why])),
                Ok(file) => {
                    let parsed = match std::fs::read(&file) {
                        Ok(bytes) => match std::str::from_utf8(&bytes) {
                            Ok(text) => parse(&file, &name, text),
                            Err(_) => Err(vec![format!("{} is not UTF-8", file.display())]),
                        },
                        Err(err) => Err(vec![format!("cannot read {}: {err}", file.display())]),
                    };
                    (file, parsed)
                }
            };
            (name, file, parsed)
        })
        .collect()
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
    // Plan 40 M1: the model line points at the seat table rather than naming an id. It was the only model id this
    // area wrote into shipped text, and an id rots when the seats move on.
    format!(
        "---\n\
description: One line, at most 120 characters, that Nirvana Claude picks this profile by.\n\
# The other keys are optional; take the # off to use one.\n\
# class: build.bounded\n\
# model: a model id that herdr seat list shows\n\
# family: anthropic\n\
# effort: medium\n\
# needs: [a-server]\n\
# disallowed_tools: []\n\
# skills: [a-skill-in-.claude/skills]\n\
# This project's MCP servers (.mcp.json): {servers}\n\
# A profile never grants a Send: that is per order, `herdr seat run --send-ok`.\n\
---\n\
You are the worker for one kind of order in this project. Say what you do, what you never do, and how you check \
your work. At most {BODY_MAX} characters: this is paid on every call of the worker.\n"
    )
}

/// Plan 37 §1.5: the templates `herdr profile new <name> --from <template>` writes, so a method learned once is
/// carried by `--profile` instead of retyped into every brief (MotoProject: 0 of 35 orders carried a profile, and the
/// Oct 23 "REAL URLs ONLY" method never became a standing rule).
pub const TEMPLATES: [&str; 1] = ["web-researcher"];

/// The `web-researcher` body: rules 1-3 and 7 of the MotoProject brief analysis
/// (`nirvana/plans/2026-10-31-35-records/moto-gemini-briefs.md` §3). Rule 4 is not here: P35a fixed the sources rule
/// it worked around. Untested as profile text (the analysis's risk): check it on the next research order. The body
/// holds the rules only (review N6: a body is paid on every call); the MotoProject failure behind each rule is in the
/// seat skill's agy subsection.
pub const WEB_RESEARCHER: &str = "---\n\
description: Web research on agy or Claude: open every source in this session, cite only what was opened, check it.\n\
class: research.web\n\
# A profile never grants a Send: that is per order, `herdr seat run --send-ok`.\n\
---\n\
You research the web for this project. Your Work Report's sources are only pages and videos you opened in this \
session.\n\
\n\
1. Never cite a URL from memory or from a search result. Cite only what you opened in this session; drop what you \
could not open.\n\
2. Read each kind of source with a tool you really have:\n\
   - a search result link that is a vertexaisearch redirect: resolve it with `curl -sI <link>` and use its \
`Location`;\n\
   - YouTube: `python -m yt_dlp --skip-download --print \"%(id)s|%(title)s|%(channel)s\" <url>`, and \
`--write-auto-subs` when you need the transcript;\n\
   - Reddit: `curl -s -A \"<a browser user agent>\" <thread>/.rss`; on a 429, wait for the reset;\n\
   - any other page: one `curl` per page, or the browser tool when your order gives you one.\n\
   There is no YouTube connector: say so and use yt_dlp.\n\
3. Done means a command, and its output goes into `tests`: last, re-open every cited id or URL in one command \
(`python -m yt_dlp --skip-download --print id <url> <url>...`, `curl -sI` for pages) and paste what it printed. A \
check written as prose is not a check.\n\
4. Put downloads and working files where the order's project rules say, never in your own scratch folder, and run \
every long step in the foreground; never wait on a background task.\n";

/// The text of a template by its name.
pub fn template_text(name: &str) -> Option<&'static str> {
    match name {
        "web-researcher" => Some(WEB_RESEARCHER),
        _ => None,
    }
}

/// `herdr profile new <name> --from <template>`: a template herdr ships, never over a file already there.
pub fn new_profile_from(root: &Path, name: &str, from: &str) -> Result<PathBuf, String> {
    let text = template_text(from).ok_or_else(|| {
        format!(
            "no template {from:?}; the templates: {}",
            TEMPLATES.join(", ")
        )
    })?;
    write_new(root, name, text)
}

/// Plan 45 §5: a shape's starter, the profile text and its starter cards `(slug, text)`.
pub struct Shape {
    pub profile: String,
    pub cards: Vec<(&'static str, String)>,
}

/// The front matter lines every shape ends its keys with: where its servers and skills go, and the Send rule.
const SHAPE_TAIL: &str = "# skills: [a-skill-in-.claude/skills]\n\
# Its own servers go in mcp.json beside this file, in .mcp.json's shape; every order seated with it gets them.\n\
# A profile never grants a Send: that is per order, `herdr seat run --send-ok`.\n";

fn shape_card(name: &str, when: &str, source: &str, tags: &str, body: &str) -> String {
    format!(
        "---\nname: {name}\nwhen: {when}\nverdict: keep\nsource: {source}\ntags: [{tags}]\n---\n{body}"
    )
}

/// Plan 45 §5: the shapes, each from a profile that worked in the record (§4). Every placeholder the owner fills is a
/// line starting `TODO(owner):`, which the parser takes as body text, so the profile passes `check` as written.
pub fn shape(name: &str) -> Option<Shape> {
    match name {
        // From Megajam's aaa-anim-reviewer (§4 M4, M6): the judging block, `sources` for local files, and the pins a
        // visual reviewer wants, commented, because they narrow the seats to the ones that read video natively and a
        // project may judge stills or pages instead.
        "reviewer" => Some(Shape {
            profile: format!(
                "---\n\
description: Judges finished work against its rubric card; opens every file itself and never edits.\n\
# For a visual reviewer, take the # off these: they seat it where video is read natively.\n\
# class: media.video\n\
# effort: high\n\
# needs: [video]\n\
{SHAPE_TAIL}---\n\
You review work for this project. You judge; you never make.\n\
\n\
TODO(owner): name what this reviewer judges (renders, assets, reels, pages) and the bar it holds them to.\n\
\n\
1. Open every file you judge yourself. Never judge from a file name, a summary or another worker's report.\n\
2. For each criterion, say meets or breaks, with the evidence (the file, the frame, the line), and name the card \
you judged it by.\n\
3. Score on the anchors of the rubric card. The worst defect sets the ceiling.\n\
4. Name the top three defects, and the one fix that would lift the score most.\n\
5. Say what you could not check, and why.\n\
6. For a review of local files, your Work Report's `sources` lists every file you opened and every card you read.\n\
7. Never edit, re-render, download or upload anything. A fix is a finding in your report, not a change.\n"
            ),
            cards: vec![
                (
                    "score-on-the-rubric-bands",
                    shape_card(
                        "Score on bands from 1 to 10; the worst defect sets the ceiling",
                        "Every score this reviewer gives.",
                        "herdr profile shape reviewer, from Megajam's aaa-anim-reviewer rubric (plan 45 §4 M4)",
                        "rubric, score",
                        "TODO(owner): say what each band means for this project's work.\n\
\n\
- 1-2: broken. It does not do the job at all.\n\
- 3-4: one breaking defect makes it unusable, whatever else is good.\n\
- 5-6: usable, with defects a viewer sees at once.\n\
- 7-8: good; defects only on a close look.\n\
- 9-10: no defect you can name with evidence.\n\
\n\
The worst defect sets the ceiling: a breaking defect caps the score at 4. Say which band, and which defect set it.\n",
                    ),
                ),
                (
                    "capture-and-judge-the-before-first",
                    shape_card(
                        "Judge the rendered output, and score the before frame first with the same prompt",
                        "A review of anything that renders: a frame, a page, a model, a reel.",
                        "herdr profile shape reviewer, from Megajam's judging step in 121 orders (plan 45 §4 M3)",
                        "capture, before, after",
                        "1. Judge what renders, never the plan or the code that makes it: capture it and open the \
capture.\n\
2. Score the before frame first, with the same prompt and the same rubric, so the after has something to beat.\n\
3. Compare the after against the before, criterion by criterion. A change the capture does not show is not a \
change.\n\
4. Name the capture files in your Work Report's `sources`.\n",
                    ),
                ),
            ],
        }),
        // From MotoProject's reel-editor and the Bambu orders (§5): plan 31 found 22 of 22 Bambu GUI orders seated
        // with no server, and a worker without its tool falls back to scripts, which is how the jig print was sent.
        "operator" => Some(Shape {
            profile: format!(
                "---\n\
description: Works only through the project's named servers; drafts before any costly or irreversible step.\n\
# Name the servers it drives; a need naming a .mcp.json server grants that server.\n\
# needs: [a-server-in-.mcp.json]\n\
{SHAPE_TAIL}---\n\
You operate this project's tools through their servers.\n\
\n\
TODO(owner): name the servers and the tool calls this operator uses, and what each costly step costs.\n\
\n\
1. Work only through the named servers' tools. Never drive the app by hand, by a script or by keystrokes instead.\n\
2. Before a costly or irreversible step (a render, an export, a print, a post: anything that spends, publishes or \
cannot be undone), make the draft and report it. Take the step only when your order says to.\n\
3. When a tool you need is missing or fails, report it with its error and stop. Never work around it by hand.\n\
4. Your Work Report names each tool call that changed something, and what it changed.\n"
            ),
            cards: vec![(
                "draft-before-the-costly-step",
                shape_card(
                    "Draft before the costly step; a missing tool is reported, never worked around",
                    "A step that spends, publishes or cannot be undone, or a tool that is missing or failing.",
                    "herdr profile shape operator, from plan 31's audit of the Bambu orders (plan 45 §5)",
                    "operator, draft, tools",
                    "TODO(owner): name this project's costly steps and the draft each one has (a preview, a slice, \
a dry run).\n\
\n\
Plan 31's audit found 22 of 22 Bambu GUI orders seated with no server. Without its tool a worker falls back to \
scripts, and the jig print was sent that way. So: stop and report instead.\n",
                ),
            )],
        }),
        // From Megajam's ue-builder (§5): 96 of its 115 lessons were keyed to it (§4 M7), and the engine sharing block
        // was retyped into 119 orders in 23 wordings (§4 M3); a builder profile says such a rule once.
        "builder" => Some(Shape {
            profile: format!(
                "---\n\
description: Builds in this project under its standing rules and shared-resource rules, each said once.\n\
# class: build.bounded\n\
{SHAPE_TAIL}---\n\
You build in this project. These rules stand for every order; your order adds its own.\n\
\n\
TODO(owner): the standing rules, one line each, with the reason (the incident or the measurement behind it).\n\
TODO(owner): the shared resources (an engine, a server, a port, a device) and the rule for each, said once.\n\
\n\
1. Measure before you claim. Each claim in your Work Report names the command that shows it and what it printed.\n\
2. Change only the files your order names as yours. A change anywhere else goes in your report as an exact hunk.\n\
3. Run long steps in the foreground and read their output; never wait on a background task.\n"
            ),
            cards: vec![(
                "measure-before-you-claim",
                shape_card(
                    "Measure before you claim; a check written as prose is not a check",
                    "Every claim of done, fixed, faster or passing in a Work Report.",
                    "herdr profile shape builder (plan 45 §5)",
                    "measure, tests",
                    "Paste the command and what it printed into `tests`. A claim with no command behind it reads \
as unchecked, and the gate treats it so.\n",
                ),
            )],
        }),
        // Today's web-researcher (plan 37 §1.5), in the folder form so it can be taught.
        "researcher" => Some(Shape {
            profile: WEB_RESEARCHER.to_owned(),
            cards: vec![(
                "cite-only-what-you-opened",
                shape_card(
                    "Cite only what you opened in this session",
                    "Every source in a research Work Report.",
                    "herdr profile shape researcher, from the MotoProject brief analysis of 2026-10-31 (plan 35)",
                    "sources, research",
                    "Four agy research orders whose brief said to open every page cited 47 URLs, all real; the one \
order without that line invented 12 of 12. Open each source in a command of its own before citing it.\n",
                ),
            )],
        }),
        _ => None,
    }
}

/// `herdr profile new <name> --shape <shape>`: the folder form, `<name>/profile.md` and `knowledge/` with the shape's
/// starter cards, adopted so they are offered. Never over a profile already there; every file is parsed before any is
/// written.
pub fn new_profile_shape(root: &Path, name: &str, shape_name: &str) -> Result<PathBuf, String> {
    let found = shape(shape_name).ok_or_else(|| {
        format!(
            "no shape {shape_name:?}; the shapes: {}",
            SHAPES.join(", ")
        )
    })?;
    if !valid_name(name) {
        return Err(format!(
            "{name:?}: a profile's name is 1 to {NAME_MAX} of [a-z0-9-]"
        ));
    }
    let (file, dir) = (path(root, name), folder_of(root, name));
    if file.exists() {
        return Err(format!("{} is already there", file.display()));
    }
    if dir.exists() {
        return Err(format!("{} is already there (a folder profile)", dir.display()));
    }
    let profile_file = dir.join(PROFILE_FILE);
    parse(&profile_file, name, &found.profile).map_err(|problems| problems.join("\n"))?;
    let knowledge = dir.join(KNOWLEDGE);
    for (slug, text) in &found.cards {
        crate::nirvana_knowledge::parse(&knowledge.join(format!("{slug}.md")), slug, text)
            .map_err(|problems| problems.join("\n"))?;
    }
    std::fs::create_dir_all(&knowledge)
        .map_err(|err| format!("cannot create {}: {err}", knowledge.display()))?;
    std::fs::write(&profile_file, &found.profile)
        .map_err(|err| format!("cannot write {}: {err}", profile_file.display()))?;
    for (slug, text) in &found.cards {
        let card = knowledge.join(format!("{slug}.md"));
        std::fs::write(&card, text).map_err(|err| format!("cannot write {}: {err}", card.display()))?;
    }
    // Plan 34 review B1: only a card herdr knows is offered; these are herdr's own, so they are adopted as written.
    crate::nirvana_knowledge::adopt(root, name)?;
    Ok(profile_file)
}

/// `herdr profile new <name>`: the template, never over a file already there.
pub fn new_profile(root: &Path, name: &str) -> Result<PathBuf, String> {
    let servers = crate::nirvana_gui_tools::project_servers(root).unwrap_or_default();
    write_new(root, name, &template(&servers))
}

fn write_new(root: &Path, name: &str, text: &str) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err(format!(
            "{name:?}: a profile's name is 1 to {NAME_MAX} of [a-z0-9-]"
        ));
    }
    let file = path(root, name);
    if file.exists() {
        return Err(format!("{} is already there", file.display()));
    }
    if folder_of(root, name).exists() {
        return Err(format!(
            "{} is already there (a folder profile)",
            folder_of(root, name).display()
        ));
    }
    parse(&file, name, text).map_err(|problems| problems.join("\n"))?;
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

/// Plan 45 §5: what `check` reads against the project beyond the seat table: each `skills` name that is not a project
/// skill (`nirvana_agy_tools::project_skills`), the profile's own `mcp.json` (`servers_problems`), and a `needs` that
/// names one of those servers, which is granted with no `needs` line and which the seat rule would read as an unknown
/// word.
pub fn project_problems(root: &Path, file: &Path, profile: &Profile) -> Vec<String> {
    let shown = file.display();
    let mut problems = Vec::new();
    let skills = crate::nirvana_agy_tools::project_skills(root);
    for skill in &profile.skills {
        if !skills.contains(skill) {
            problems.push(format!(
                "{shown}: skills {skill}: no such skill in .claude/skills (the project's: {})",
                if skills.is_empty() {
                    "none".to_owned()
                } else {
                    skills.join(", ")
                }
            ));
        }
    }
    match read_servers(root, &profile.name) {
        Err(found) => problems.extend(found),
        Ok(Some(servers)) => {
            for need in profile.needs.iter().filter(|need| servers.names.contains(need)) {
                problems.push(format!(
                    "{shown}: needs {need}: a server of the profile's own mcp.json is granted with no needs line; \
                     take it out of needs"
                ));
            }
        }
        Ok(None) => {}
    }
    problems
}

/// Every problem `herdr profile check` prints for one profile: the parser's, the table's, the project's (plan 45) and
/// the cards'. Empty when it passes.
pub fn check_one(
    root: &Path,
    name: &str,
    file: &Path,
    parsed: &Result<Profile, Vec<String>>,
    table: &Result<nirvana_seat::Table, String>,
    servers: &[String],
) -> Vec<String> {
    let mut problems = match (parsed, table) {
        (Ok(profile), Ok(table)) => table_problems(file, profile, table, servers),
        (Ok(_), Err(_)) => Vec::new(),
        (Err(problems), _) => problems.clone(),
    };
    if let Ok(profile) = parsed {
        problems.extend(project_problems(root, file, profile));
    }
    // Plan 34 §2.2: every card of a folder profile, one line per problem with its file and line.
    problems.extend(crate::nirvana_knowledge::check_problems(root, name));
    problems
}

const USAGE: &str = "usage: herdr profile list
       herdr profile show <name>
       herdr profile new <name>          writes .nirvana/profiles/<name>.md from the template
       herdr profile new <name> --from <template>
                                         writes it from a template herdr ships: web-researcher
       herdr profile new <name> --shape <reviewer|operator|builder|researcher>
                                         writes the folder form, profile.md and starter cards, adopted
       herdr profile check [<name>]      prints every refusal, a folder profile's cards too; exit 1 when there is one
       herdr profile check <name> --adopt
                                         records the cards you wrote by hand as they stand, so they are offered
       herdr profile teach <name> --from <url|path> [--note \"what to learn\"] [--quote \"the owner's words\"] [--run <run>]
                                         seats a profile-teach order; its Work Report's card_ops become cards;
                                         the teacher takes none of the profile's model, family, effort or needs
       herdr profile proposals           the proposals waiting: id, kind, profile, one line of evidence
       herdr profile proposal show <id>  one proposal whole
       herdr profile accept <id>         a new profile goes into profiles/<name>/profile.md, a patch into its body
       herdr profile dismiss <id> [--reason \"...\"]
                                         the same proposal is never made again
       herdr profile proposals rollback <op>
                                         undoes one accept or dismiss from profile-proposals/ledger.jsonl
A profile is used by `herdr seat run <seat> --profile <name> ...` and `herdr seat pick --profile <name> ...`.
In profile.md: `skills: [a, b]` names the project skills (.claude/skills) its worker uses. A folder profile's
mcp.json, in .mcp.json's shape, holds its own servers: every order seated with it gets them, no needs line.";

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
                let shown = resolve(&root, name).unwrap_or_else(|_| path(&root, name));
                println!("{} · sha256 {}", shown.display(), loaded.sha);
                // Plan 45: the names only; a definition can hold a token.
                if let Some(servers) = &loaded.servers {
                    println!(
                        "own servers (mcp.json, in the sha above): {}",
                        servers.names.join(", ")
                    );
                }
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
        (Some("new"), [name, flag, shape]) if flag == "--shape" => {
            match new_profile_shape(&root, name, shape) {
                Ok(file) => {
                    println!("{}", file.display());
                    0
                }
                Err(why) => {
                    eprintln!("herdr profile new: {why}");
                    1
                }
            }
        }
        // Plan 45 §5: the proposals a lesson review or the detector made, each inert until accepted.
        (Some("proposals" | "proposal" | "accept" | "dismiss"), _) => {
            crate::nirvana_profile_proposals::cli_to(
                &root,
                args,
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            )
        }
        (Some("new"), [name, flag, from]) if flag == "--from" => {
            match new_profile_from(&root, name, from) {
                Ok(file) => {
                    println!("{}", file.display());
                    0
                }
                Err(why) => {
                    eprintln!("herdr profile new: {why}");
                    1
                }
            }
        }
        (Some("check"), [name, flag]) if flag == "--adopt" => {
            // Plan 34 P34a review B1: the owner's one step for cards written by hand; then the check, as without it.
            match crate::nirvana_knowledge::adopt(&root, name) {
                Ok(adopted) => println!(
                    "{name}: adopted {} card{}{}",
                    adopted.len(),
                    if adopted.len() == 1 { "" } else { "s" },
                    if adopted.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", adopted.join(", "))
                    }
                ),
                Err(why) => {
                    eprintln!("herdr profile check --adopt: {why}");
                    std::process::exit(1);
                }
            }
            return cli(&["check".to_owned(), name.clone()]);
        }
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
                let problems = check_one(&root, name, file, parsed, &table, &servers);
                if problems.is_empty() {
                    println!(
                        "{name}: ok{}",
                        crate::nirvana_knowledge::count_note(&root, name)
                    );
                } else {
                    refused += 1;
                    for problem in problems {
                        println!("{problem}");
                    }
                }
            }
            i32::from(refused > 0 || (chosen.is_empty() && !names.is_empty()))
        }
        (Some("teach"), rest) => crate::nirvana_knowledge::teach_cli(&root, rest),
        _ => usage_exit(),
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests;
