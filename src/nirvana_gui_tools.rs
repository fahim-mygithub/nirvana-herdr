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
//!   so the definition never goes on the command line.
//! - Any error building it keeps today's argv and says so in one line of the worker's view.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::nirvana_seat::Table;

/// The project's MCP servers, as Claude Code reads them from the project root.
pub const MCP_FILE: &str = ".mcp.json";

/// The servers that drive a desktop window. §1: agy panes cannot see the Studio window.
pub const DESKTOP_SERVERS: [&str; 2] = ["bambu-studio-beta", "xtool"];

/// §5.3: the one line a worker brief gets when its order needs a desktop server.
pub const BRIEF_LINE: &str = "Use the `*_run` batch and `*_state`; read crops, not full screenshots; never start a print or a job.";

/// The server names a `.mcp.json` text defines, in its order.
pub fn server_names(text: &str) -> Result<Vec<String>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|err| format!("{MCP_FILE} is not JSON: {err}"))?;
    let servers = value
        .get("mcpServers")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{MCP_FILE} has no mcpServers object"))?;
    Ok(servers.keys().cloned().collect())
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
        Ok(text) => server_names(&text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(format!("cannot read {}: {err}", path.display())),
    }
}

/// The needs the seat rule reads, and why only a Claude seat may take the order: the needs with every server need
/// taken out. A `.mcp.json` that cannot be read names no server, so its names stay needs that no seat has, and the
/// order is refused by today's unknown-word check rather than seated without its server.
pub fn seat_needs(table: &Table, root: &Path, needs: &[String]) -> (Vec<String>, Option<String>) {
    let servers = project_servers(root).unwrap_or_default();
    let named = servers_among(needs, &capabilities(table), &servers);
    let rest = needs
        .iter()
        .filter(|need| !named.contains(need))
        .cloned()
        .collect();
    (rest, claude_only(&named))
}

/// The per-order config: the named servers alone, each copied whole from the project's definition.
pub fn order_config(text: &str, servers: &[String]) -> Result<Value, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|err| format!("{MCP_FILE} is not JSON: {err}"))?;
    let defined = value
        .get("mcpServers")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{MCP_FILE} has no mcpServers object"))?;
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

/// Writes `<run_dir>/mcp/<order>.json` for the order's server needs and returns it with the servers; `None` when the
/// order needs no server.
pub fn write_order_config(
    root: &Path,
    run_dir: &Path,
    order: &str,
    needs: &[String],
    capabilities: &[&str],
) -> Result<Option<(PathBuf, Vec<String>)>, String> {
    let path = root.join(MCP_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("cannot read {}: {err}", path.display())),
    };
    let servers = servers_among(needs, capabilities, &server_names(&text)?);
    if servers.is_empty() {
        return Ok(None);
    }
    let config = order_config(&text, &servers)?;
    let dir = run_dir.join("mcp");
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let file = dir.join(format!("{order}.json"));
    let body = serde_json::to_string_pretty(&config).map_err(|err| err.to_string())?;
    std::fs::write(&file, format!("{body}\n"))
        .map_err(|err| format!("cannot write {}: {err}", file.display()))?;
    Ok(Some((file, servers)))
}

/// §5.1 and §5.4: the worker's argv with its order's servers, or today's argv and one line saying why not. Only a
/// claude argv takes them; `capabilities` is an error when seats.toml could not be read, which keeps today's argv too.
pub fn order_argv(
    argv: Vec<String>,
    root: &Path,
    run_dir: &Path,
    order: &str,
    needs: &[String],
    capabilities: Result<Vec<&str>, String>,
    out: &mut dyn Write,
) -> Vec<String> {
    if needs.is_empty() || argv.first().map(String::as_str) != Some("claude") {
        return argv;
    }
    let built =
        capabilities.and_then(|words| write_order_config(root, run_dir, order, needs, &words));
    match built {
        Ok(Some((file, servers))) => {
            let _ = writeln!(out, "MCP servers for this order: {}", servers.join(", "));
            with_servers(argv, &file, &servers)
        }
        Ok(None) => argv,
        Err(why) => {
            let _ = writeln!(out, "the worker starts without its MCP servers: {why}");
            argv
        }
    }
}

/// §5.3: the brief of an order that needs a desktop server ends with `BRIEF_LINE`.
pub fn with_desktop_line(mut text: String, needs: &[String]) -> String {
    if needs
        .iter()
        .any(|need| DESKTOP_SERVERS.contains(&need.as_str()))
    {
        text.push_str("\n\n");
        text.push_str(BRIEF_LINE);
    }
    text
}

#[cfg(test)]
mod tests;
