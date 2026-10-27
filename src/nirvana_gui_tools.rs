//! Nirvana fork, plan 29 P29c: workers can reach the project's MCP servers
//! (nirvana/plans/2026-10-26-29-gui-tools.md §5).
//!
//! Measured 2026-10-26 (§1): every one of 26 worker starts showed `mcp_servers: []`. A worker runs with
//! `--strict-mcp-config` and no `--mcp-config`, so workers drove Bambu Studio and xTool Studio through Bash, one
//! `uv run python -c` per action: 668 Studio GUI actions in 243 min, a median of about 9 s each, mostly process start.
//!
//! - An order whose `needs` names a server in the project's `.mcp.json` gets a per-order `--mcp-config` holding those
//!   servers alone, copied from the project's definition, and `--allowedTools mcp__<server>__*` for each.
//!   `--strict-mcp-config` stays, so nothing else loads. Names match exactly.
//! - A need that is a seat capability stays a capability, so a server named like one changes nothing that seats today.
//! - Only a Claude seat takes such an order. agy panes cannot see the Studio window (MainWindowHandle 0, §1), and only
//!   the claude argv carries `--mcp-config`. With no Claude seat free the order is refused, with the reason.
//! - The file is written beside the run's journal, `.nirvana/runs/<run>/mcp/<order>.json`, which `.nirvana/.gitignore`
//!   keeps out of git. The project's definition can hold a printer token in `env`, and the journal records the argv,
//!   so the definition never goes on the command line. The supervisor removes the file when the worker ends.
//!
//! The review round's rule, from the jig-print run (plan §2): a worker with a device server never starts a print or a
//! job, and every doubt grants nothing.
//! - An order seated for a server it cannot get at launch does not start: without it the worker falls back to Bash and
//!   pywinauto, which is how the jig print was sent.
//! - A `.mcp.json` that does not parse refuses the order that names a server, saying so, not as an unknown need.
//! - Two servers whose tool names flatten alike (`a.b`, `a_b`) refuse the order: the grant for one is a grant for both.
//! - Every order granted a server is told never to start a print, a laser job or a send.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::nirvana_seat::Table;

/// The project's MCP servers, as Claude Code reads them from the project root.
pub const MCP_FILE: &str = ".mcp.json";

/// The servers that drive a desktop window. §1: agy panes cannot see the Studio window.
pub const DESKTOP_SERVERS: [&str; 2] = ["bambu-studio-beta", "xtool"];

/// §5.3: the line a worker brief gets when its order is granted a desktop server, after `DEVICE_LINE`.
pub const BRIEF_LINE: &str = "Use the `*_run` batch and `*_state`; read crops, not full screenshots; never start a print or a job.";

/// Review round F4: the line every brief gets whose order is granted any `.mcp.json` server. `bambu` is printer
/// control and holds a token, and it is no desktop server, so §5.3's line alone never reached it.
pub const DEVICE_LINE: &str = "Never start a print, a laser job, or a send to a device. Stop and report when that is the next step.";

/// The server names a `.mcp.json` text defines, in its order.
pub fn server_names(text: &str) -> Result<Vec<String>, String> {
    Ok(defined(text)?.keys().cloned().collect())
}

fn defined(text: &str) -> Result<Map<String, Value>, String> {
    let value: Value = serde_json::from_str(text).map_err(|err| format!("not JSON: {err}"))?;
    match value.get("mcpServers") {
        Some(Value::Object(servers)) => Ok(servers.clone()),
        _ => Err("no mcpServers object".to_owned()),
    }
}

/// The needs that name one of `servers`, once each: matched exactly, and never a word some seat has as a capability.
pub fn servers_among(needs: &[String], capabilities: &[&str], servers: &[String]) -> Vec<String> {
    let mut named: Vec<String> = Vec::new();
    for need in needs {
        if !capabilities.contains(&need.as_str()) && servers.contains(need) && !named.contains(need)
        {
            named.push(need.clone());
        }
    }
    named
}

/// Why only a Claude seat may take an order that needs these servers; `None` when it needs none.
pub fn claude_only(servers: &[String]) -> Option<String> {
    if servers.is_empty() {
        return None;
    }
    Some(
        match servers
            .iter()
            .find(|server| DESKTOP_SERVERS.contains(&server.as_str()))
        {
            Some(desktop) => format!(
                "{desktop} drives a desktop window and agy panes cannot see it (plan 29 §5.2)"
            ),
            None => format!(
                "only a claude worker takes a per-order --mcp-config, for {} (plan 29 §5.1)",
                servers.join(", ")
            ),
        },
    )
}

/// The review round's nit: a named server whose tool name another server of the project's flattens to as well.
/// `--allowedTools mcp__a_b__*` approves the tools of `a.b` and of `a_b` alike, so granting one grants both.
pub fn key_clash(named: &[String], servers: &[String]) -> Option<String> {
    named.iter().find_map(|server| {
        let key = crate::nirvana_preload::tool_server_key(server);
        servers
            .iter()
            .find(|other| *other != server && crate::nirvana_preload::tool_server_key(other) == key)
            .map(|other| {
                format!(
                    "{MCP_FILE} servers {server} and {other} both name their tools mcp__{key}__*, so a grant for one \
                     is a grant for both; rename one of them"
                )
            })
    })
}

/// Every capability some seat in the table has.
pub fn capabilities(table: &Table) -> Vec<&str> {
    let mut words: Vec<&str> = table
        .seats
        .iter()
        .flat_map(|seat| seat.capabilities.iter().map(String::as_str))
        .collect();
    words.sort_unstable();
    words.dedup();
    words
}

/// The server names in `<root>/.mcp.json`; none when the project has no such file.
pub fn project_servers(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join(MCP_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            server_names(&text).map_err(|why| format!("{} does not parse ({why})", path.display()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(format!("cannot read {}: {err}", path.display())),
    }
}

/// An order's needs read against the project's servers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Needs {
    /// What the seat rule reads: the needs with every server need taken out.
    pub rest: Vec<String>,
    /// The needs granted as `.mcp.json` servers, once each.
    pub servers: Vec<String>,
    /// Why only a Claude seat may take the order; `None` when it is granted no server.
    pub claude_only: Option<String>,
}

/// The needs the seat rule reads, the servers granted, and why only a Claude seat may take the order. A need that no
/// seat has can only name a server, so a `.mcp.json` that cannot be read refuses such an order with the file's name
/// (review F5: it read as an unknown need); an order whose needs are all capabilities is not refused by it.
pub fn seat_needs(table: &Table, root: &Path, needs: &[String]) -> Result<Needs, String> {
    let words = capabilities(table);
    let servers = match project_servers(root) {
        Ok(servers) => servers,
        Err(why) => {
            return match needs.iter().find(|need| !words.contains(&need.as_str())) {
                Some(need) => Err(format!(
                    "{why}, so the need {need} names none of its servers"
                )),
                None => Ok(Needs {
                    rest: needs.to_vec(),
                    ..Needs::default()
                }),
            };
        }
    };
    let named = servers_among(needs, &words, &servers);
    if let Some(why) = key_clash(&named, &servers) {
        return Err(why);
    }
    let rest = needs
        .iter()
        .filter(|need| !named.contains(need))
        .cloned()
        .collect();
    Ok(Needs {
        rest,
        claude_only: claude_only(&named),
        servers: named,
    })
}

/// The servers an order was seated for: its needs that no seat has as a capability, once each. At seat time the
/// unknown-word check refused every such need that named no server (§5.1), so each one left names a server.
pub fn seated_servers(needs: &[String], capabilities: &[&str]) -> Vec<String> {
    let mut servers: Vec<String> = Vec::new();
    for need in needs {
        if !capabilities.contains(&need.as_str()) && !servers.contains(need) {
            servers.push(need.clone());
        }
    }
    servers
}

/// The per-order config: the named servers alone, each copied whole from the project's definition.
pub fn order_config(text: &str, servers: &[String]) -> Result<Value, String> {
    let defined = defined(text).map_err(|why| format!("{MCP_FILE} does not parse ({why})"))?;
    let mut chosen = Map::new();
    for server in servers {
        let definition = defined
            .get(server)
            .ok_or_else(|| format!("{MCP_FILE} has no server {server:?}"))?;
        chosen.insert(server.clone(), definition.clone());
    }
    Ok(serde_json::json!({ "mcpServers": chosen }))
}

/// A claude worker's argv with the order's config file and its servers' tools approved. `--allowedTools` takes one
/// comma-joined value: it and `--mcp-config` are variadic, and a flag follows each.
pub fn with_servers(mut argv: Vec<String>, file: &Path, servers: &[String]) -> Vec<String> {
    let tools: Vec<String> = servers
        .iter()
        .map(|server| {
            format!(
                "mcp__{}__*",
                crate::nirvana_preload::tool_server_key(server)
            )
        })
        .collect();
    argv.extend([
        "--mcp-config".to_owned(),
        file.display().to_string(),
        "--allowedTools".to_owned(),
        tools.join(","),
    ]);
    argv
}

/// Where an order's config is written: `<run_dir>/mcp/<order>.json`.
pub fn order_config_path(run_dir: &Path, order: &str) -> PathBuf {
    run_dir.join("mcp").join(format!("{order}.json"))
}

/// Writes the order's config for `servers`, read from `<root>/.mcp.json` now. Review F3: a server the order was seated
/// for that the file no longer defines, a file gone or not parsing, is an error, never a config without it.
pub fn write_order_config(
    root: &Path,
    run_dir: &Path,
    order: &str,
    servers: &[String],
) -> Result<PathBuf, String> {
    let path = root.join(MCP_FILE);
    let text = std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => format!("{} is gone", path.display()),
        _ => format!("cannot read {}: {err}", path.display()),
    })?;
    let names =
        server_names(&text).map_err(|why| format!("{} does not parse ({why})", path.display()))?;
    if let Some(gone) = servers.iter().find(|server| !names.contains(server)) {
        return Err(format!("{} no longer defines {gone}", path.display()));
    }
    if let Some(why) = key_clash(servers, &names) {
        return Err(why);
    }
    let config = order_config(&text, servers)?;
    let file = order_config_path(run_dir, order);
    let dir = file.parent().unwrap_or(run_dir);
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let body = serde_json::to_string_pretty(&config).map_err(|err| err.to_string())?;
    write_private(&file, format!("{body}\n").as_bytes())
        .map_err(|err| format!("cannot write {}: {err}", file.display()))?;
    Ok(file)
}

/// Review F2: the definition can hold a printer token, so the file is the owner's alone where `std` says so in one
/// flag (0600 on Unix). On Windows it takes the ACL of the project folder it sits in: narrowing it needs the security
/// API, which this round does not build.
fn write_private(file: &Path, body: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(file)?.write_all(body)
}

/// Review F2: the order's config goes when its worker process ends, whatever the outcome; a restart or a reseat
/// writes it again. A remove that fails says so in one line of the worker's view.
pub fn remove_order_config(run_dir: &Path, order: &str, out: &mut dyn Write) {
    let file = order_config_path(run_dir, order);
    match std::fs::remove_file(&file) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            let _ = writeln!(
                out,
                "the order's MCP config stays at {}: {err}",
                file.display()
            );
        }
        _ => {}
    }
}

/// §5.1: the worker's argv with its order's servers. Only a claude argv takes them, and only for an order with a
/// server need. Review F3: an order that cannot get a server it was seated for gets an error, and the supervisor
/// fails it rather than start it without. With seats.toml refused, the needs `.mcp.json` names are read as servers.
pub fn order_argv(
    argv: Vec<String>,
    root: &Path,
    run_dir: &Path,
    order: &str,
    needs: &[String],
    capabilities: Result<Vec<&str>, String>,
    out: &mut dyn Write,
) -> Result<Vec<String>, String> {
    if needs.is_empty() || argv.first().map(String::as_str) != Some("claude") {
        return Ok(argv);
    }
    let servers = match capabilities {
        Ok(words) => seated_servers(needs, &words),
        Err(why) => {
            let _ = writeln!(
                out,
                "seats.toml is refused ({why}); the needs {MCP_FILE} names are read as its servers"
            );
            let defined = project_servers(root)?;
            seated_servers(needs, &[])
                .into_iter()
                .filter(|need| defined.contains(need))
                .collect()
        }
    };
    if servers.is_empty() {
        return Ok(argv);
    }
    let file = write_order_config(root, run_dir, order, &servers).map_err(|why| {
        format!(
            "{order} was seated for MCP server {} and cannot get it: {why}",
            servers.join(", ")
        )
    })?;
    let _ = writeln!(out, "MCP servers for this order: {}", servers.join(", "));
    Ok(with_servers(argv, &file, &servers))
}

/// §5.3 and review F4: the brief of an order granted any server ends with `DEVICE_LINE`, and one granted a desktop
/// server with `BRIEF_LINE` after it. `servers` are the needs granted as servers, never a raw need that shares a name.
pub fn with_server_lines(mut text: String, servers: &[String]) -> String {
    if servers.is_empty() {
        return text;
    }
    text.push_str("\n\n");
    text.push_str(DEVICE_LINE);
    if servers
        .iter()
        .any(|server| DESKTOP_SERVERS.contains(&server.as_str()))
    {
        text.push('\n');
        text.push_str(BRIEF_LINE);
    }
    text
}

#[cfg(test)]
mod tests;
