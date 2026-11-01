//! Nirvana fork, plan 35 P35b: what an agy worker starts with (nirvana/plans/2026-10-31-35-agy-tool-parity.md §3).
//!
//! Measured before (§1.2): a seated agy worker got no project MCP server, no rules when the project had only
//! `CLAUDE.md` (agy reads `AGENTS.md`, `GEMINI.md` and `.agents/rules/`, never `CLAUDE.md`), no project skill, and a
//! profile that denied any tool failed the order outright.
//!
//! - **The tools folder** is `<run>/agy/<order>/`, under `runs/`, which `.nirvana/.gitignore` ignores. The worker gets
//!   it with `--add-dir`: plan probe 2 measured that an add-dir folder's `.agents/mcp_config.json` and `AGENTS.md` load,
//!   and P35b probe 1 that its `.agents/skills/<name>/SKILL.md` is listed with its description and read on use. No
//!   file goes into the project, the worktree or their `.agents/`.
//! - **The deny file** is `<run>/agy/<order>.settings.json`, outside the tools folder, passed with `--settings`. Every
//!   row of `DENY_TABLE` was seen to deny under `--dangerously-skip-permissions` (P35b probes 2 and 3, plan probe 2),
//!   each answering "Matches user-configured deny rule". A tool with no row is refused on agy, never guessed.
//! - An order with none of these gets no `--add-dir`, no `--settings` and today's prompt: its argv is byte for byte
//!   what it was before plan 35.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The folder in the run's folder that holds every agy order's tools folder and deny file.
pub const DIR: &str = "agy";

/// agy's workspace MCP file, as the tools folder and a workspace both hold it (agy docs, `/docs/mcp?app=cli`).
pub const MCP_CONFIG: &str = ".agents/mcp_config.json";

/// §3.4: Claude Code's tool names and the agy rule each becomes. Measured, every row, against agy 1.4.12 with
/// `--dangerously-skip-permissions`:
/// - `command(*)`: P35b probe 2, "Permission denied for unsandboxed(echo P35B-CMD)". The terminal sandbox is off on
///   this machine, so agy checks a command as `unsandboxed(...)`, and `command(*)` denies it.
/// - `write_file(*)`: plan probe 2, against agy's `write_file` tool.
/// - `read_file(*)`: P35b probe 2. In that probe a write in the worker's own folder was refused too, with
///   `read_file(*)` and a path rule in the same file; which one refused it was not separated, so a `Read` denial on agy
///   may also deny writes. It never denies less than Claude's.
///
/// `Edit` and `NotebookEdit` have no row (review round S3). The model's file tools are `write_file` and `edit_file`
/// (review probe 5: it named exactly these two and had no notebook tool), and no probe saw `write_file(*)` deny an
/// `edit_file` call. A rule that may leave edits open is no `Edit` denial, so such a profile stays on claude.
pub const DENY_TABLE: [(&str, &str); 4] = [
    ("Bash", "command(*)"),
    ("PowerShell", "command(*)"),
    ("Write", "write_file(*)"),
    ("Read", "read_file(*)"),
];

/// Plan 33 review B3 on agy (§3.4): a lesson reviewer reads and answers, so it may neither run nor write. Whether
/// `write_file(*)` also stops `edit_file` is unmeasured (`DENY_TABLE`); the gate's check of the lessons folder stays
/// the backstop, as it is for a tool Claude Code adds later.
pub const REVIEW_DENIED: [&str; 2] = ["command(*)", "write_file(*)"];

/// The keys of a `.mcp.json` server herdr carries to agy. Any other key refuses the server on agy: dropping it could
/// change what the server is (`disabled`, a timeout), and agy's own `--settings` refused an unknown key at parse
/// (plan probe 1), so its MCP file may too.
pub const SERVER_KEYS: [&str; 7] = ["type", "command", "args", "env", "cwd", "url", "headers"];

/// §3.5: agy gives rules their own 20,000-token budget (changelog line 369). At about 4 bytes a token, a copy past
/// this many bytes may be cut; the worker's view says so. The 4 is a hypothesis, not a measurement.
pub const RULES_BUDGET_BYTES: usize = 80_000;

/// The most of one skill folder herdr copies. A skill past it is left out with one line: a copy runs before every
/// attempt. A hypothesis; no project here has a skill near it.
pub const SKILL_MAX_BYTES: u64 = 4 * 1024 * 1024;

/// The most herdr copies of all the skills together (review round nit: a per-skill cap alone lets many skills copy
/// before every attempt). Skills are copied by name; one that would pass the total is left out with one line. A
/// hypothesis, like `SKILL_MAX_BYTES`.
pub const SKILLS_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// §5 P35b item 3: the line an agy research or media order gets after its `sources` line. P35a teaches the gate to see
/// shell fetches; a URL built inside a script (`watch?v={vid}`) it still cannot see. The MotoProject analysis of
/// 2026-10-31 (F1): four agy research orders whose brief said to open every page cited 47 URLs, all real (26 of 26
/// YouTube ids exist); the one order without that line invented 12 of 12. Four against one is too few to size the
/// effect, so this is an instruction with one measurement behind it, not enforcement.
pub const SOURCES_LINE: &str = "Never cite a URL you did not open in this session. Open each one in a command of its own (curl, yt_dlp, or bladebro when you have it) before citing it; a URL built inside a script is not seen as read.";

/// `<run>/agy/<order>/`.
pub fn tools_dir(run_dir: &Path, order: &str) -> PathBuf {
    run_dir.join(DIR).join(order)
}

/// `<run>/agy/<order>.settings.json`: outside the tools folder, so it is no workspace file.
pub fn settings_path(run_dir: &Path, order: &str) -> PathBuf {
    run_dir.join(DIR).join(format!("{order}.settings.json"))
}

/// A path as herdr hands it to agy, without the `\\?\` a canonical Windows path carries. The argv's `--add-dir` and
/// the rule that denies writes there come from this one text, so they never name the folder two ways.
pub fn plain_path(path: &Path) -> String {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => rest.to_owned(),
        _ => text,
    }
}

/// A path as an agy rule names it: forward slashes, the drive letter kept. P35b probe 1 measured
/// `write_file(C:/.../tools/)` denying a write there; the docs' drive-stripped form was not separated (probe 2).
pub fn rule_path(path: &Path) -> String {
    plain_path(path).replace('\\', "/")
}

/// The agy rule for one Claude Code tool name, or `None` when no measured rule matches it. `mcp__<s>__<t>` becomes
/// `mcp(<s>/<t>)`, and `mcp__<s>__*` or `mcp__<s>` becomes `mcp(<s>/*)` (P35b probe 3 measured both forms).
///
/// Review round S6: Claude names a server's tools by the server's name with every character but a letter, a digit
/// and `-` made `_` (`nirvana_preload::tool_server_key`), while agy names it by its `.mcp.json` key. A server part of
/// only letters, digits and `-` is its key exactly, whatever the project defines. One with `_` may come from `.`, a
/// space or `_`, and `a__b__t` splits two ways, so either is refused, as is a tool part that starts with `_` or holds
/// `__` (`mcp__a___t` is server `a` and tool `_t`, or `a_` and `t`). Resolving against the project's keys instead
/// would need the root in `nirvana_profiles::narrow`, whose callers in `nirvana_seat` and `nirvana_gate` do not pass
/// it; a seat rule and an exec that read the name two ways would seat an order the exec then fails.
pub fn deny_rule(tool: &str) -> Option<String> {
    if let Some((_, rule)) = DENY_TABLE.iter().find(|(name, _)| *name == tool) {
        return Some((*rule).to_owned());
    }
    let rest = tool.strip_prefix("mcp__")?;
    let (server, name) = rest.split_once("__").unwrap_or((rest, "*"));
    let key = !server.is_empty()
        && server
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    let tool_part = name == "*"
        || (!name.is_empty()
            && !name.starts_with('_')
            && !name.contains("__")
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')));
    (key && tool_part).then(|| format!("mcp({server}/{name})"))
}

/// The tools of `tools` no measured agy rule matches.
pub fn unmapped(tools: &[String]) -> Vec<String> {
    tools
        .iter()
        .filter(|tool| deny_rule(tool).is_none())
        .cloned()
        .collect()
}

/// The agy rules for a Claude Code deny list, once each; an error naming every tool without one.
pub fn deny_rules(tools: &[String]) -> Result<Vec<String>, String> {
    let missing = unmapped(tools);
    if !missing.is_empty() {
        return Err(format!(
            "agy cannot be denied {}: no measured agy rule matches (plan 35 §3.4)",
            missing.join(", ")
        ));
    }
    let mut rules: Vec<String> = Vec::new();
    for rule in tools.iter().filter_map(|tool| deny_rule(tool)) {
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    }
    Ok(rules)
}

/// Why agy cannot take this `.mcp.json` server definition; `None` when it can (§3.2). Every doubt refuses:
/// - a key herdr does not carry (`SERVER_KEYS`);
/// - `${...}` anywhere: Claude Code expands it, agy documents no expansion;
/// - a `type` other than `stdio` or `http`. `sse` is refused too: no probe measured it (P35b probe 3 measured an
///   `http` server through `serverUrl` and `headers`).
pub fn refusal(server: &str, definition: &Value) -> Option<String> {
    let Some(object) = definition.as_object() else {
        return Some(format!("{server} is not an object in .mcp.json"));
    };
    if let Some(key) = object
        .keys()
        .find(|key| !SERVER_KEYS.contains(&key.as_str()))
    {
        return Some(format!(
            "{server} has the key {key:?}, which herdr does not carry to agy (plan 35 §3.2)"
        ));
    }
    if names_variable(definition) {
        return Some(format!(
            "{server} uses ${{...}}, which Claude Code expands and agy does not (plan 35 §2)"
        ));
    }
    let command = object.get("command");
    let url = object.get("url");
    let kind = match object.get("type") {
        None if command.is_some() => "stdio",
        None if url.is_some() => "http",
        None => return Some(format!("{server} names neither a command nor a url")),
        Some(Value::String(kind)) => kind.as_str(),
        Some(_) => return Some(format!("{server} has a type that is not a string")),
    };
    match kind {
        "stdio" if command.is_some_and(Value::is_string) && url.is_none() => None,
        "http" if url.is_some_and(Value::is_string) && command.is_none() => None,
        "stdio" | "http" => Some(format!(
            "{server} is {kind} but its command and url do not say so"
        )),
        other => Some(format!(
            "{server} is type {other}, which no agy probe measured (plan 35 §3.2)"
        )),
    }
}

fn names_variable(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains("${"),
        Value::Array(items) => items.iter().any(names_variable),
        Value::Object(map) => map
            .iter()
            .any(|(key, value)| key.contains("${") || names_variable(value)),
        _ => false,
    }
}

/// §3.2: a definition agy can take, in agy's shape. `command`, `args`, `env` and `cwd` pass; `url` becomes
/// `serverUrl`; `headers` passes; `type` is dropped. Call it only where `refusal` said `None`.
pub fn translate(definition: &Value) -> Value {
    let mut out = Map::new();
    if let Some(object) = definition.as_object() {
        for (key, value) in object {
            match key.as_str() {
                "type" => {}
                "url" => {
                    out.insert("serverUrl".to_owned(), value.clone());
                }
                _ => {
                    out.insert(key.clone(), value.clone());
                }
            }
        }
    }
    Value::Object(out)
}

/// The servers the workspace's own `.agents/mcp_config.json` names; they load anyway.
pub fn workspace_servers(cwd: &Path) -> Result<Vec<String>, String> {
    let path = cwd.join(MCP_CONFIG);
    match std::fs::read_to_string(&path) {
        Ok(text) => crate::nirvana_gui_tools::server_names(&text)
            .map_err(|why| format!("{} does not parse ({why})", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(format!("cannot read {}: {err}", path.display())),
    }
}

/// §3.2: writes `<tools>/.agents/mcp_config.json` with the order's servers, translated, and returns the names it
/// wrote. `defined` is the project's definitions of exactly these servers. A server the workspace names already is
/// left out with one line, because it loads anyway and which copy wins is not documented. Review F3: a server agy
/// cannot take is an error, never a config without it.
pub fn write_servers(
    defined: &Map<String, Value>,
    cwd: &Path,
    run_dir: &Path,
    order: &str,
    out: &mut dyn Write,
) -> Result<Vec<String>, String> {
    let native = workspace_servers(cwd).unwrap_or_else(|why| {
        let _ = writeln!(out, "{why}; herdr gives the order's servers anyway");
        Vec::new()
    });
    let mut chosen = Map::new();
    for (server, definition) in defined {
        if let Some(why) = refusal(server, definition) {
            return Err(why);
        }
        if native.contains(server) {
            let _ = writeln!(
                out,
                "{server} is in {} already, so herdr leaves it out",
                cwd.join(MCP_CONFIG).display()
            );
            continue;
        }
        let mut agy = translate(definition);
        // Review round S1: a stdio server with no `cwd` gets the worker's folder written in, the folder a claude
        // worker starts it from, so a project-relative script runs the same on both. Review probe 4 measured that an
        // add-dir server's `cwd` is honoured (a relative script found only there answered); one without starts in
        // agy's own cwd, which is the worker's folder today, so this pins what is now true by accident.
        if let Some(object) = agy.as_object_mut() {
            if object.contains_key("command") && !object.contains_key("cwd") {
                object.insert("cwd".to_owned(), Value::from(plain_path(cwd)));
            }
        }
        chosen.insert(server.clone(), agy);
    }
    if chosen.is_empty() {
        return Ok(Vec::new());
    }
    let file = tools_dir(run_dir, order).join(MCP_CONFIG);
    let body = serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": chosen }))
        .map_err(|err| err.to_string())?;
    write_file(&file, format!("{body}\n").as_bytes(), true)?;
    Ok(chosen.keys().cloned().collect())
}

fn write_file(file: &Path, body: &[u8], private: bool) -> Result<(), String> {
    let dir = file.parent().unwrap_or(file);
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let written = if private {
        crate::nirvana_gui_tools::write_private(file, body)
    } else {
        std::fs::write(file, body)
    };
    written.map_err(|err| format!("cannot write {}: {err}", file.display()))
}

/// Whether `dir` holds rules agy reads by itself: `AGENTS.md`, `GEMINI.md` or `.agents/rules/*.md`.
pub fn has_agy_rules(dir: &Path) -> bool {
    ["AGENTS.md", "GEMINI.md"]
        .iter()
        .any(|name| dir.join(name).is_file())
        || std::fs::read_dir(dir.join(".agents").join("rules")).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                entry.path().extension().is_some_and(|ext| ext == "md") && entry.path().is_file()
            })
        })
}

/// The folders whose agy rules and skills the worker's workspace is taken to hold: the cwd, then each parent up to
/// the project root. The walk stops at a folder holding `.git` (review round nit): a worktree is its own checkout, so
/// an untracked `AGENTS.md` in the root is not in it, and finding it there left the worker with no rules at all. When
/// the cwd is outside the root (none is today), the root is checked too.
pub fn workspace_dirs(cwd: &Path, root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut dir = Some(cwd);
    while let Some(at) = dir {
        dirs.push(at.to_path_buf());
        if at == root || !at.starts_with(root) || at.join(".git").exists() {
            break;
        }
        dir = at.parent();
    }
    if !cwd.starts_with(root) {
        dirs.push(root.to_path_buf());
    }
    dirs
}

/// §3.5: the `CLAUDE.md` to give an agy worker as rules, or `None`. None when a workspace folder
/// (`workspace_dirs`) has agy rules of its own, or when neither the cwd nor the root has a `CLAUDE.md`.
pub fn claude_rules(cwd: &Path, root: &Path) -> Option<PathBuf> {
    if workspace_dirs(cwd, root)
        .iter()
        .any(|dir| has_agy_rules(dir))
    {
        return None;
    }
    [cwd, root]
        .iter()
        .map(|dir| dir.join("CLAUDE.md"))
        .find(|file| file.is_file())
}

/// Whether a rules text holds a Claude Code `@path` import, which agy does not follow. The word names a file: its
/// last part has an extension (`@AGENTS.md`, `@docs/more.md`, `@~/.claude/x.md`), so `@types/node` is no import.
pub fn has_imports(text: &str) -> bool {
    text.split_whitespace().any(|word| {
        word.strip_prefix('@').is_some_and(|rest| {
            let rest = rest.trim_end_matches([',', ';', ':', ')', '.']);
            rest.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '~' | '_'))
                && rest
                    .rsplit('/')
                    .next()
                    .is_some_and(|last| last.trim_start_matches('.').contains('.'))
        })
    })
}

/// §3.5: the tools folder's `AGENTS.md`: one header line, then the file's bytes.
pub fn rules_text(source: &Path, body: &str) -> String {
    let imports = if has_imports(body) {
        " Its @path imports are not followed."
    } else {
        ""
    };
    format!(
        "This is {}, the project's CLAUDE.md, given to an agy worker as rules by herdr (plan 35).{imports}\n\n{body}",
        source.display()
    )
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// The project's Claude skills, `<cwd>/.claude/skills/<name>/SKILL.md` (the root's when the cwd has none), each with
/// its folder, by name. A skill a workspace folder's `.agents/skills/` has already (`workspace_dirs`) is left out:
/// agy loads that one. Links are not followed, each with one line: a linked skill folder, or a `.claude/skills` that
/// is itself a link, could copy from outside the project.
pub fn claude_skills(cwd: &Path, root: &Path, out: &mut dyn Write) -> Vec<(String, PathBuf)> {
    let from = [cwd, root]
        .iter()
        .map(|dir| dir.join(".claude").join("skills"))
        .find(|dir| dir.is_dir());
    let Some(from) = from else {
        return Vec::new();
    };
    if is_link(&from) {
        let _ = writeln!(
            out,
            "{} is a link, so herdr copies no skill from it",
            from.display()
        );
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&from) else {
        return Vec::new();
    };
    let workspace = workspace_dirs(cwd, root);
    let mut skills: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() && entry.path().join("SKILL.md").is_file() {
            let _ = writeln!(
                out,
                "the skill {} is left out: it is a link, which herdr does not follow",
                entry.file_name().to_string_lossy()
            );
            continue;
        }
        if !kind.is_dir() || !entry.path().join("SKILL.md").is_file() {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if workspace
            .iter()
            .any(|dir| dir.join(".agents").join("skills").join(&name).exists())
        {
            continue;
        }
        skills.push((name, entry.path()));
    }
    skills.sort();
    skills
}

/// Copies `skills` into `<to>/<name>/`, in order, leaving out with one line each a skill past `per_skill` bytes or one
/// that would take the copy past `total`.
pub fn copy_skills(
    skills: &[(String, PathBuf)],
    to: &Path,
    per_skill: u64,
    total: u64,
    out: &mut dyn Write,
) -> Result<(), String> {
    let mut copied = 0u64;
    for (name, from) in skills {
        let bytes = folder_bytes(from);
        if bytes > per_skill {
            let _ = writeln!(
                out,
                "the skill {name} is left out: its folder holds {bytes} bytes, over {per_skill}"
            );
            continue;
        }
        if copied + bytes > total {
            let _ = writeln!(
                out,
                "the skill {name} is left out: with it the skills would hold {} bytes, over {total}",
                copied + bytes
            );
            continue;
        }
        let target = to.join(name);
        copy_folder(from, &target).map_err(|err| {
            format!(
                "cannot copy the skill {name} to {}: {err}",
                target.display()
            )
        })?;
        copied += bytes;
    }
    Ok(())
}

/// The bytes under `dir`, files only, links not followed.
fn folder_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => folder_bytes(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map(|meta| meta.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

fn copy_folder(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_folder(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// What an agy order is started with, beyond its servers (which `nirvana_gui_tools::order_argv_seated` writes first).
pub struct Order<'a> {
    pub root: &'a Path,
    pub cwd: &'a Path,
    pub run_dir: &'a Path,
    pub order: &'a str,
    pub order_file: &'a str,
    /// The agy rules the order is denied: its profile's, or a lesson review's.
    pub deny: Vec<String>,
}

/// §3.3: the prompt of an agy worker with a tools folder. With two workspace folders "this folder" is ambiguous, and in
/// plan probe 2 the model wrote into the added one.
pub fn first_line(order_file: &str, cwd: &Path, tools: &Path) -> String {
    format!(
        "Do the work order in {order_file}. Your folder is {}; work only there. {} holds herdr's tool config: never \
         write there. End with the Work Report your output schema asks for.",
        plain_path(cwd),
        plain_path(tools)
    )
}

/// §3.1: an agy worker's argv with its tools folder and deny file. Writes the rules (§3.5) and the skills into the
/// tools folder, beside the servers file `order_argv_seated` wrote. When the folder holds anything the argv gets
/// `--add-dir <tools>`, the deny file gets `write_file(<tools>/)`, and a first start's prompt names both folders
/// (§3.3). Any deny file also denies `write_file(<run>/agy/)`, which holds it. When there is nothing to add the argv
/// comes back as it went in. Any other CLI's argv is untouched.
pub fn with_tools(
    mut argv: Vec<String>,
    order: &Order,
    out: &mut dyn Write,
) -> Result<Vec<String>, String> {
    if argv.first().map(String::as_str) != Some("agy") {
        return Ok(argv);
    }
    let tools = tools_dir(order.run_dir, order.order);
    if let Some(source) = claude_rules(order.cwd, order.root) {
        let body = std::fs::read_to_string(&source)
            .map_err(|err| format!("cannot read {}: {err}", source.display()))?;
        let text = rules_text(&source, &body);
        if text.len() > RULES_BUDGET_BYTES {
            let _ = writeln!(
                out,
                "{} runs {} bytes, past agy's rules budget of about {RULES_BUDGET_BYTES}: agy may cut it",
                source.display(),
                text.len()
            );
        }
        write_file(&tools.join("AGENTS.md"), text.as_bytes(), false)?;
    }
    copy_skills(
        &claude_skills(order.cwd, order.root, out),
        &tools.join(".agents").join("skills"),
        SKILL_MAX_BYTES,
        SKILLS_MAX_BYTES,
        out,
    )?;
    let used = std::fs::read_dir(&tools).is_ok_and(|mut entries| entries.next().is_some());
    // Once each, wherever a rule repeats (a plain `dedup` kept a repeat that was not next to its first).
    let mut deny: Vec<String> = Vec::new();
    let add = |deny: &mut Vec<String>, rule: String| {
        if !deny.contains(&rule) {
            deny.push(rule);
        }
    };
    for rule in &order.deny {
        add(&mut deny, rule.clone());
    }
    if used {
        add(&mut deny, format!("write_file({}/)", rule_path(&tools)));
    }
    if !used && deny.is_empty() {
        return Ok(argv);
    }
    // Review round S3: the deny file sits beside the tools folder, so `write_file(<tools>/)` does not cover it. The
    // folder form of `<run>/agy/` covers it and every other order's tools folder; probe 1 measured that form with the
    // file written directly inside, as the deny file is.
    add(
        &mut deny,
        format!("write_file({}/)", rule_path(&order.run_dir.join(DIR))),
    );
    let mut flags: Vec<String> = Vec::new();
    if used {
        flags.extend(["--add-dir".to_owned(), plain_path(&tools)]);
        let today = format!(
            "--print={}",
            crate::nirvana_worker::first_line(order.order_file)
        );
        if let Some(print) = argv.iter_mut().find(|arg| **arg == today) {
            *print = format!(
                "--print={}",
                first_line(order.order_file, order.cwd, &tools)
            );
        }
    }
    let file = settings_path(order.run_dir, order.order);
    let body = serde_json::to_string_pretty(&serde_json::json!({"permissions": {"deny": deny}}))
        .map_err(|err| err.to_string())?;
    write_file(&file, format!("{body}\n").as_bytes(), false)?;
    flags.extend(["--settings".to_owned(), plain_path(&file)]);
    // agy takes its prompt as the one token `--print=<line>`, last; the flags go before it.
    let at = argv
        .iter()
        .rposition(|arg| arg.starts_with("--print="))
        .unwrap_or(argv.len());
    argv.splice(at..at, flags);
    Ok(argv)
}

/// §3.1 and review F2: the tools folder and the deny file go when the worker ends, whatever the outcome. A remove that
/// fails says so in one line.
pub fn remove(run_dir: &Path, order: &str, out: &mut dyn Write) {
    let tools = tools_dir(run_dir, order);
    if let Err(err) = std::fs::remove_dir_all(&tools) {
        if err.kind() != std::io::ErrorKind::NotFound {
            let _ = writeln!(
                out,
                "the order's agy tools folder stays at {}: {err}",
                tools.display()
            );
        }
    }
    let file = settings_path(run_dir, order);
    if let Err(err) = std::fs::remove_file(&file) {
        if err.kind() != std::io::ErrorKind::NotFound {
            let _ = writeln!(
                out,
                "the order's agy deny file stays at {}: {err}",
                file.display()
            );
        }
    }
}

/// Review round S4: the orders that have a tools folder, a deny file, a plan 29 MCP file or (P35c) a bladebro home in
/// `run_dir`, by name, once each. Each can hold a server's token until it is removed.
pub fn leftover_orders(run_dir: &Path) -> Vec<String> {
    let names = |dir: PathBuf, suffix: &'static str, dirs: bool| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(move |entry| {
                let name = entry.file_name().into_string().ok()?;
                match name.strip_suffix(suffix) {
                    Some(order) if !suffix.is_empty() => Some(order.to_owned()),
                    None if dirs && entry.file_type().is_ok_and(|kind| kind.is_dir()) => Some(name),
                    _ => None,
                }
            })
    };
    // Plan 35 P35c item 4: a bladebro home is a browser profile, cookies and all; the checks once leaked 144 GB of them.
    let homes = std::fs::read_dir(run_dir.join(crate::nirvana_bladebro::DIR))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok());
    let mut orders: Vec<String> = names(run_dir.join(DIR), ".settings.json", true)
        .chain(names(run_dir.join("mcp"), ".json", false))
        .chain(homes)
        .collect();
    orders.sort();
    orders.dedup();
    orders
}

/// §5 P35b item 3: `SOURCES_LINE` for an agy worker's research or media order.
pub fn sources_line(cli: &str, class: &str) -> Option<&'static str> {
    (cli == "agy" && (class.starts_with("research.") || class.starts_with("media.")))
        .then_some(SOURCES_LINE)
}

#[cfg(test)]
mod tests;
