//! Nirvana fork, feature 5: the Preload mod. Everything a fresh Claude session in a folder preloads,
//! item by item with its tokens, and a switch that keeps an item out of every new Claude session there.
//!
//! The owner, 2026-10-16: "This mod square is called Preload and it will list all the tokens it will
//! preload for this fresh and every Claude session. Stuff like the modfied claude md, loaded memories,
//! preloaded skills and tools mcp connectors everything that is not the barebones. I want to be able to
//! see them and toggle them or modify them if I cant remove unselect them."
//! (nirvana/plans/2026-10-16-05-preload-mod.md)
//!
//! **The list** is Claude Code's own `/context`, run in the agent's folder: `claude -p /context
//! --no-session-persistence --verbose --output-format json`. Claude Code answers it locally: the check on
//! 2026-10-16 returned 0 tokens and a cost of 0, in 2.6 s. With `--verbose` the answer is a list of
//! events; the `init` event names the MCP servers, plugins and tools, and the local command's event
//! carries `context_usage`, every memory file, skill, agent and MCP tool with its tokens as integers.
//!
//! **The switches** are Claude Code's own settings, written to the folder's `.claude/settings.local.json`
//! so they hold for every Claude started there, inside herdr or not, and nothing is deleted. Each was
//! proven by `/context` before and after on 2026-10-16 ("Sonnet - measure Claude preload toggles", 96 runs,
//! all free):
//! - a CLAUDE.md file: its absolute path in `claudeMdExcludes` (forward slashes);
//! - auto memory: `autoMemoryEnabled: false`;
//! - a skill: `skillOverrides: {name: "off"}`; a plugin's own skill has no switch of its own, its plugin does;
//! - an MCP server or connector: `deniedMcpServers: [{serverName}]`, the name as `init` writes it
//!   ("claude.ai Gmail", "plugin:github:github"); `permissions.deny` hid the tools but left the server
//!   connected and its instructions loaded, so it is not used;
//! - a plugin: `enabledPlugins: {id: false}`, which takes its skills, agents, MCP servers and hooks;
//! - a built-in tool: `permissions.deny: [name]`;
//! - the session-start hooks' text: `disableAllHooks: true`.
//!
//! A custom agent has no setting that keeps it out (`Agent(name)` in `permissions.deny` is checked only
//! when the agent is spawned), so agents are listed with their file to edit, and no switch.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

/// A folder's list is read again after this long; a switch reads it again at once.
pub const STALE_AFTER: Duration = Duration::from_secs(600);
const CLAUDE_TIMEOUT: Duration = Duration::from_secs(60);
const CONTEXT_ARGS: [&str; 6] = [
    "-p",
    "/context",
    "--no-session-persistence",
    "--verbose",
    "--output-format",
    "json",
];

/// One named entry of the report, with its tokens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    /// Where it comes from: a memory file's type, a skill's or agent's source, an MCP tool's server, a
    /// server's source, a plugin's id.
    pub source: String,
    /// A skill's plugin, a server's status.
    pub extra: String,
    pub tokens: u32,
}

/// What `/context` says a fresh Claude session in a folder preloads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The tokens a session starts with. Claude Code's own total leaves out deferred tools, which load
    /// only when searched for.
    pub total: u32,
    pub window: u32,
    /// "System prompt", "System tools", "Skills", "Messages"... in the report's order.
    pub categories: Vec<(String, u32)>,
    pub memory: Vec<Named>,
    pub skills: Vec<Named>,
    pub agents: Vec<Named>,
    pub mcp_tools: Vec<Named>,
    pub servers: Vec<Named>,
    pub plugins: Vec<Named>,
    /// Built-in tools, by name.
    pub tools: Vec<String>,
}

impl Report {
    pub fn category(&self, name: &str) -> u32 {
        self.categories
            .iter()
            .find(|(category, _)| category == name)
            .map_or(0, |(_, tokens)| *tokens)
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn count(value: &Value, key: &str) -> u32 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX))
}

fn list<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// Reads the stdout of `claude -p /context --verbose --output-format json`. `None` when it holds no
/// `context_usage` (an older Claude Code, or one without `--verbose`).
pub fn parse(stdout: &str) -> Option<Report> {
    let events: Value = serde_json::from_str(stdout.trim()).ok()?;
    let events = events.as_array()?;
    let usage = events.iter().find_map(|event| event.get("context_usage"))?;
    let init = events
        .iter()
        .find(|event| event.get("subtype").and_then(Value::as_str) == Some("init"));
    let named = |value: &Value, name: &str, source: &str, extra: &str| Named {
        name: text(value, name),
        source: text(value, source),
        extra: text(value, extra),
        tokens: count(value, "tokens"),
    };
    let mut report = Report {
        total: count(usage, "total_tokens"),
        window: count(usage, "raw_max_tokens"),
        categories: list(usage, "categories")
            .map(|category| (text(category, "name"), count(category, "tokens")))
            .collect(),
        memory: list(usage, "memory_files")
            .map(|file| named(file, "path", "type", ""))
            .collect(),
        skills: list(usage, "skills")
            .map(|skill| named(skill, "name", "source", "plugin_name"))
            .collect(),
        agents: list(usage, "agents")
            .map(|agent| named(agent, "agent_type", "source", ""))
            .collect(),
        mcp_tools: list(usage, "mcp_tools")
            .map(|tool| named(tool, "name", "server_name", ""))
            .collect(),
        ..Report::default()
    };
    if let Some(init) = init {
        report.servers = list(init, "mcp_servers")
            .map(|server| named(server, "name", "source", "status"))
            .collect();
        report.plugins = list(init, "plugins")
            .map(|plugin| named(plugin, "name", "source", ""))
            .collect();
        report.tools = list(init, "tools")
            .filter_map(Value::as_str)
            .filter(|tool| !tool.starts_with("mcp__"))
            .map(str::to_owned)
            .collect();
    }
    Some(report)
}

/// The MCP tools' server name for a server as `init` names it: "claude.ai Gmail" is "claude_ai_Gmail",
/// "plugin:github:github" is "plugin_github_github".
pub fn tool_server_key(server: &str) -> String {
    server
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// What kind of thing an item is, its order inside its provider's group. A plugin's own row comes first,
/// above the parts it switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    /// Feature 9: herdr's own Jev switches and today's figures.
    Jev,
    Plugins,
    Memory,
    Skills,
    Mcp,
    Agents,
    Tools,
    Hooks,
}

/// Where an item comes from: the panel's groups, in its order (feature 7). The owner, 2026-10-16:
/// "Organize the list by provider or built in." Read from Claude Code's own `source` fields in `/context`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Provider {
    /// Feature 9: herdr's Jev token savers, first, because they are herdr's own and save tokens rather
    /// than spend them.
    Jev,
    /// The folder's own: `projectSettings`, `localSettings`, a Project or Local memory file.
    Project,
    /// The user's own config, `~/.claude`: `userSettings`, a User memory file, auto memory.
    User,
    /// A plugin, by its name.
    Plugin(String),
    /// Synced from claude.ai: its connectors and skills.
    ClaudeAi,
    /// A source this reader does not know, as Claude Code wrote it.
    Other(String),
    /// Part of Claude Code itself: its tools, its skills, and the hooks' session-start text.
    BuiltIn,
    /// Managed policy.
    Managed,
}

impl Provider {
    pub fn title(&self) -> String {
        match self {
            Provider::Jev => "Jev token savers (herdr)".into(),
            Provider::Project => "This project".into(),
            Provider::User => "You (~/.claude)".into(),
            Provider::Plugin(name) => format!("Plugin {name}"),
            Provider::ClaudeAi => "claude.ai".into(),
            Provider::Other(source) => source.clone(),
            Provider::BuiltIn => "Built in (Claude Code)".into(),
            Provider::Managed => "Managed".into(),
        }
    }

    /// A memory file's type, a skill's, agent's or MCP server's source.
    fn of_source(source: &str) -> Provider {
        match source {
            "Project" | "Local" | "projectSettings" | "localSettings" | "project" | "local" => {
                Provider::Project
            }
            "User" | "AutoMem" | "userSettings" | "user" => Provider::User,
            "syncedSkills" | "claudeai" => Provider::ClaudeAi,
            "Managed" | "policySettings" | "managed" | "enterprise" => Provider::Managed,
            "built-in" | "builtin" | "bundled" => Provider::BuiltIn,
            other => Provider::Other(other.to_owned()),
        }
    }

    /// The plugin an MCP server comes with, from the name `init` gives it: "plugin:github:github".
    fn of_server(name: &str, source: &str) -> Provider {
        match name
            .strip_prefix("plugin:")
            .and_then(|rest| rest.split(':').next())
        {
            Some(plugin) => Provider::Plugin(plugin.to_owned()),
            None => Provider::of_source(source),
        }
    }

    /// A switched-off item no read remembers (herdr restarted since): where its switch says it comes from.
    fn of_switch(switch: &Switch, cwd: &Path, config_dir: Option<&Path>) -> Provider {
        match switch {
            Switch::ClaudeMd(path) => {
                let in_config = config_dir
                    .is_some_and(|dir| shown_path(path, cwd, Some(dir)).starts_with("~/.claude/"));
                if in_config {
                    Provider::User
                } else {
                    Provider::Project
                }
            }
            Switch::AutoMemory => Provider::User,
            Switch::Skill(name) => {
                if cwd.join(".claude").join("skills").join(name).is_dir() {
                    Provider::Project
                } else if config_dir.is_some_and(|dir| dir.join("skills").join(name).is_dir()) {
                    Provider::User
                } else {
                    Provider::BuiltIn
                }
            }
            Switch::McpServer(name) if name.starts_with("claude.ai ") => Provider::ClaudeAi,
            Switch::McpServer(name) => Provider::of_server(name, "user"),
            Switch::Plugin(id) => Provider::Plugin(id.split('@').next().unwrap_or(id).to_owned()),
            Switch::Tool(_) | Switch::Hooks => Provider::BuiltIn,
            Switch::JevGuide | Switch::JevCompact => Provider::Jev,
        }
    }
}

/// A group's tokens: the sum of its rows, a plugin's own row left out (it sums the rows under it). Built
/// in adds "System tools", which `/context` does not split per tool.
pub fn provider_tokens(items: &[Item], provider: &Provider, report: Option<&Report>) -> u32 {
    let rows: u32 = items
        .iter()
        .filter(|item| &item.provider == provider && item.group != Group::Plugins)
        .filter_map(|item| item.tokens)
        .sum();
    let tools = match (provider, report) {
        (Provider::BuiltIn, Some(report)) => report.category("System tools"),
        _ => 0,
    };
    rows + tools
}

/// What the window remembers of an item from the last read that listed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remembered {
    pub tokens: Option<u32>,
    pub provider: Provider,
}

/// The setting that keeps one item out of new sessions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Switch {
    /// A CLAUDE.md file, by its absolute path with forward slashes.
    ClaudeMd(String),
    AutoMemory,
    Skill(String),
    McpServer(String),
    /// A plugin by its id, "name@marketplace".
    Plugin(String),
    Tool(String),
    Hooks,
    /// Feature 9: herdr's Jev guide, on when its hook is in the folder's settings. Off is the default,
    /// so it is never one of the "N off".
    JevGuide,
    /// Feature 9: level 7's Stop hook.
    JevCompact,
}

/// One row of the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub provider: Provider,
    pub group: Group,
    pub name: String,
    pub detail: String,
    pub tokens: Option<u32>,
    pub on: bool,
    /// `None`: nothing switches it off by itself (a plugin's skill, an agent).
    pub switch: Option<Switch>,
    /// The file to open for editing.
    pub file: Option<PathBuf>,
}

fn forward(path: &str) -> String {
    path.replace('\\', "/")
}

/// A path shown short: inside the folder, relative to it; inside Claude's config folder, from "~/.claude".
fn shown_path(path: &str, cwd: &Path, config_dir: Option<&Path>) -> String {
    let path = forward(path);
    let under = |base: &Path| -> Option<String> {
        let base = forward(&base.to_string_lossy());
        let base = base.trim_end_matches('/');
        let head = path.get(..base.len())?;
        let rest = path.get(base.len()..)?.strip_prefix('/')?;
        head.eq_ignore_ascii_case(base).then(|| rest.to_owned())
    };
    if let Some(rest) = under(cwd) {
        return rest;
    }
    if let Some(rest) = config_dir.and_then(under) {
        return format!("~/.claude/{rest}");
    }
    path
}

fn existing(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

/// Whether the folder's local settings switch this item off.
pub fn is_off(local: &Value, switch: &Switch) -> bool {
    match switch {
        Switch::ClaudeMd(path) => list(local, "claudeMdExcludes")
            .filter_map(Value::as_str)
            .any(|entry| entry.eq_ignore_ascii_case(path)),
        Switch::AutoMemory => local.get("autoMemoryEnabled") == Some(&Value::Bool(false)),
        Switch::Skill(name) => {
            local.pointer(&format!("/skillOverrides/{}", pointer_key(name)))
                == Some(&Value::String("off".into()))
        }
        Switch::McpServer(name) => list(local, "deniedMcpServers")
            .any(|entry| entry.get("serverName").and_then(Value::as_str) == Some(name)),
        Switch::Plugin(id) => {
            local.pointer(&format!("/enabledPlugins/{}", pointer_key(id)))
                == Some(&Value::Bool(false))
        }
        Switch::Tool(name) => local
            .pointer("/permissions/deny")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|entry| entry.as_str() == Some(name)),
        Switch::Hooks => local.get("disableAllHooks") == Some(&Value::Bool(true)),
        Switch::JevGuide => !crate::nirvana_jev_install::has_hook(
            local,
            crate::nirvana_jev_install::GUIDE_EVENT,
            crate::nirvana_jev_install::GUIDE,
        ),
        Switch::JevCompact => !crate::nirvana_jev_install::has_hook(
            local,
            crate::nirvana_jev_install::COMPACT_EVENT,
            crate::nirvana_jev_install::COMPACT,
        ),
    }
}

fn pointer_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// The items switched off in the folder's local settings, read back from them: an item switched off is
/// gone from `/context`, and the panel must still list it so it can be switched back on.
fn off_switches(local: &Value) -> Vec<Switch> {
    let mut off = Vec::new();
    off.extend(
        list(local, "claudeMdExcludes")
            .filter_map(Value::as_str)
            .map(|path| Switch::ClaudeMd(path.to_owned())),
    );
    if local.get("autoMemoryEnabled") == Some(&Value::Bool(false)) {
        off.push(Switch::AutoMemory);
    }
    if let Some(skills) = local.get("skillOverrides").and_then(Value::as_object) {
        off.extend(
            skills
                .iter()
                .filter(|(_, mode)| mode.as_str() == Some("off"))
                .map(|(name, _)| Switch::Skill(name.clone())),
        );
    }
    off.extend(
        list(local, "deniedMcpServers")
            .filter_map(|entry| entry.get("serverName").and_then(Value::as_str))
            .map(|name| Switch::McpServer(name.to_owned())),
    );
    if let Some(plugins) = local.get("enabledPlugins").and_then(Value::as_object) {
        off.extend(
            plugins
                .iter()
                .filter(|(_, enabled)| **enabled == Value::Bool(false))
                .map(|(id, _)| Switch::Plugin(id.clone())),
        );
    }
    // A bare tool name; a rule with a pattern, "Bash(git push:*)", is a permission, not a switch.
    off.extend(
        local
            .pointer("/permissions/deny")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|rule| !rule.is_empty() && rule.chars().all(|c| c.is_ascii_alphanumeric()))
            .map(|tool| Switch::Tool(tool.to_owned())),
    );
    if local.get("disableAllHooks") == Some(&Value::Bool(true)) {
        off.push(Switch::Hooks);
    }
    off
}

/// How many items the folder's local settings switch off: the strip's "N off".
pub fn off_count(local: &Value) -> usize {
    off_switches(local).len()
}

/// The panel's rows: what the report lists, each on unless the folder's settings switch it off, then
/// each item switched off there that the report no longer lists. `remembered` holds what an item was
/// when it was last listed. Rows go by provider, then kind, then name, so a row keeps its place when
/// switched; each row's detail names its kind, its provider being the header above it.
pub fn items(
    report: Option<&Report>,
    local: &Value,
    cwd: &Path,
    config_dir: Option<&Path>,
    remembered: &HashMap<Switch, Remembered>,
) -> Vec<Item> {
    let mut items = Vec::new();
    let item =
        |provider, group, name: String, detail: String, tokens, switch: Option<Switch>, file| {
            Item {
                provider,
                group,
                on: switch.as_ref().is_none_or(|switch| !is_off(local, switch)),
                name,
                detail,
                tokens,
                switch,
                file,
            }
        };
    if let Some(report) = report {
        for file in &report.memory {
            let auto = file.source == "AutoMem";
            items.push(item(
                Provider::of_source(&file.source),
                Group::Memory,
                if auto {
                    "MEMORY.md".into()
                } else {
                    shown_path(&file.name, cwd, config_dir)
                },
                if auto { "auto memory" } else { "memory file" }.into(),
                Some(file.tokens),
                Some(if auto {
                    Switch::AutoMemory
                } else {
                    Switch::ClaudeMd(forward(&file.name))
                }),
                Some(PathBuf::from(&file.name)),
            ));
        }
        for skill in &report.skills {
            let plugin = skill.source == "plugin";
            let file = match skill.source.as_str() {
                "userSettings" => config_dir.and_then(|dir| {
                    existing(dir.join("skills").join(&skill.name).join("SKILL.md"))
                }),
                "projectSettings" => existing(
                    cwd.join(".claude")
                        .join("skills")
                        .join(&skill.name)
                        .join("SKILL.md"),
                ),
                _ => None,
            };
            items.push(item(
                if plugin {
                    Provider::Plugin(skill.extra.clone())
                } else {
                    Provider::of_source(&skill.source)
                },
                Group::Skills,
                skill.name.clone(),
                "skill".into(),
                Some(skill.tokens),
                (!plugin).then(|| Switch::Skill(skill.name.clone())),
                file,
            ));
        }
        for server in &report.servers {
            let key = tool_server_key(&server.name);
            let tools: Vec<_> = report
                .mcp_tools
                .iter()
                .filter(|tool| tool.source == key)
                .collect();
            let provider = Provider::of_server(&server.name, &server.source);
            let mut detail = format!(
                "{}, {} tool{}",
                if provider == Provider::ClaudeAi {
                    "connector"
                } else {
                    "MCP"
                },
                tools.len(),
                if tools.len() == 1 { "" } else { "s" }
            );
            if server.extra != "connected" && !server.extra.is_empty() {
                detail = format!("{detail}, {}", server.extra);
            }
            items.push(item(
                provider,
                Group::Mcp,
                server.name.clone(),
                detail,
                Some(tools.iter().map(|tool| tool.tokens).sum()),
                Some(Switch::McpServer(server.name.clone())),
                None,
            ));
        }
        for plugin in &report.plugins {
            // Claude Code's own built-in plugins have no marketplace to switch them in.
            if plugin.source.ends_with("@builtin") {
                continue;
            }
            let prefix = format!("{}:", plugin.name);
            let server_prefix = format!("plugin_{}_", tool_server_key(&plugin.name));
            let tokens = report
                .skills
                .iter()
                .filter(|skill| skill.source == "plugin" && skill.extra == plugin.name)
                .map(|skill| skill.tokens)
                .chain(
                    report
                        .agents
                        .iter()
                        .filter(|agent| agent.name.starts_with(&prefix))
                        .map(|agent| agent.tokens),
                )
                .chain(
                    report
                        .mcp_tools
                        .iter()
                        .filter(|tool| tool.source.starts_with(&server_prefix))
                        .map(|tool| tool.tokens),
                )
                .sum();
            items.push(item(
                Provider::Plugin(plugin.name.clone()),
                Group::Plugins,
                plugin.name.clone(),
                "whole plugin".into(),
                Some(tokens),
                Some(Switch::Plugin(plugin.source.clone())),
                None,
            ));
        }
        for agent in &report.agents {
            let file = match agent.source.as_str() {
                "userSettings" => config_dir.and_then(|dir| {
                    existing(dir.join("agents").join(format!("{}.md", agent.name)))
                }),
                "projectSettings" => existing(
                    cwd.join(".claude")
                        .join("agents")
                        .join(format!("{}.md", agent.name)),
                ),
                _ => None,
            };
            let provider = match (agent.source.as_str(), agent.name.split_once(':')) {
                ("plugin", Some((plugin, _))) => Provider::Plugin(plugin.to_owned()),
                (source, _) => Provider::of_source(source),
            };
            items.push(item(
                provider,
                Group::Agents,
                agent.name.clone(),
                "agent".into(),
                Some(agent.tokens),
                None,
                file,
            ));
        }
        for tool in &report.tools {
            items.push(item(
                Provider::BuiltIn,
                Group::Tools,
                tool.clone(),
                "tool".into(),
                None,
                Some(Switch::Tool(tool.clone())),
                None,
            ));
        }
        items.push(item(
            Provider::BuiltIn,
            Group::Hooks,
            "session start text".into(),
            "all hooks".into(),
            Some(report.category("Messages")),
            Some(Switch::Hooks),
            None,
        ));
    }
    items.extend(jev_items(local));
    for switch in off_switches(local) {
        if items
            .iter()
            .any(|item| item.switch.as_ref() == Some(&switch))
        {
            continue;
        }
        let seen = remembered.get(&switch);
        let tokens = seen.and_then(|seen| seen.tokens);
        let provider = seen.map_or_else(
            || Provider::of_switch(&switch, cwd, config_dir),
            |seen| seen.provider.clone(),
        );
        let (group, name, detail, file) = match &switch {
            Switch::ClaudeMd(path) => (
                Group::Memory,
                shown_path(path, cwd, config_dir),
                "memory file".into(),
                Some(PathBuf::from(path)),
            ),
            Switch::AutoMemory => (
                Group::Memory,
                "MEMORY.md".into(),
                "auto memory".into(),
                None,
            ),
            Switch::Skill(name) => (Group::Skills, name.clone(), "skill".into(), None),
            Switch::McpServer(name) => (Group::Mcp, name.clone(), "MCP".into(), None),
            Switch::Plugin(id) => (
                Group::Plugins,
                id.split('@').next().unwrap_or(id).to_owned(),
                "whole plugin".into(),
                None,
            ),
            Switch::Tool(name) => (Group::Tools, name.clone(), "tool".into(), None),
            Switch::Hooks => (
                Group::Hooks,
                "session start text".into(),
                "all hooks".into(),
                None,
            ),
            // Listed by `jev_items` always; never one of `off_switches`.
            Switch::JevGuide | Switch::JevCompact => continue,
        };
        items.push(Item {
            provider,
            group,
            name,
            detail,
            tokens,
            on: false,
            switch: Some(switch),
            file,
        });
    }
    items.sort_by(|a, b| {
        a.provider
            .cmp(&b.provider)
            .then_with(|| a.group.cmp(&b.group))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    items
}

/// Feature 9: the Jev group. The guide and compact advice switch herdr's own hooks; the last row is
/// what Jev did today, from herdr's own log, or why it cannot run.
fn jev_items(local: &Value) -> Vec<Item> {
    let row = |name: &str, detail: String, tokens, switch: Option<Switch>| Item {
        provider: Provider::Jev,
        group: Group::Jev,
        on: switch.as_ref().is_none_or(|s| !is_off(local, s)),
        name: name.into(),
        detail,
        tokens,
        switch,
        file: None,
    };
    let (today, _) = crate::nirvana_jev::today_line();
    vec![
        row(
            "Jev guide",
            "Claude, Codex, agy: ask Jev, not read".into(),
            Some(crate::nirvana_jev_install::guide_tokens()),
            Some(Switch::JevGuide),
        ),
        row(
            "Compact advice",
            "Claude: says when to /compact".into(),
            Some(0),
            Some(Switch::JevCompact),
        ),
        row("today", today, None, None),
    ]
}

fn array<'a>(object: &'a mut Map<String, Value>, key: &str) -> &'a mut Vec<Value> {
    let slot = object
        .entry(key)
        .or_insert_with(|| Value::Array(Vec::new()));
    if !slot.is_array() {
        *slot = Value::Array(Vec::new());
    }
    slot.as_array_mut().expect("an array")
}

fn object<'a>(object: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let slot = object
        .entry(key)
        .or_insert_with(|| Value::Object(Map::new()));
    if !slot.is_object() {
        *slot = Value::Object(Map::new());
    }
    slot.as_object_mut().expect("an object")
}

/// Drops `key` when it holds an empty list or object, so switching an item back on leaves the file as
/// it was.
fn drop_empty(object: &mut Map<String, Value>, key: &str) {
    let empty = match object.get(key) {
        Some(Value::Array(list)) => list.is_empty(),
        Some(Value::Object(map)) => map.is_empty(),
        _ => false,
    };
    if empty {
        object.remove(key);
    }
}

/// Switches one item on or off in a folder's local settings. Only the item's own entry is added or
/// removed; every other setting in the file stays.
pub fn set(local: &mut Value, switch: &Switch, on: bool) {
    if !local.is_object() {
        *local = Value::Object(Map::new());
    }
    let root = local.as_object_mut().expect("an object");
    match switch {
        Switch::ClaudeMd(path) => {
            let excludes = array(root, "claudeMdExcludes");
            excludes.retain(|entry| {
                !entry
                    .as_str()
                    .is_some_and(|entry| entry.eq_ignore_ascii_case(path))
            });
            if !on {
                excludes.push(Value::String(path.clone()));
            }
            drop_empty(root, "claudeMdExcludes");
        }
        Switch::AutoMemory => {
            if on {
                root.remove("autoMemoryEnabled");
            } else {
                root.insert("autoMemoryEnabled".into(), Value::Bool(false));
            }
        }
        Switch::Skill(name) => {
            let skills = object(root, "skillOverrides");
            if on {
                skills.remove(name);
            } else {
                skills.insert(name.clone(), Value::String("off".into()));
            }
            drop_empty(root, "skillOverrides");
        }
        Switch::McpServer(name) => {
            let denied = array(root, "deniedMcpServers");
            denied.retain(|entry| entry.get("serverName").and_then(Value::as_str) != Some(name));
            if !on {
                denied.push(serde_json::json!({ "serverName": name }));
            }
            drop_empty(root, "deniedMcpServers");
        }
        Switch::Plugin(id) => {
            let plugins = object(root, "enabledPlugins");
            if on {
                plugins.remove(id);
            } else {
                plugins.insert(id.clone(), Value::Bool(false));
            }
            drop_empty(root, "enabledPlugins");
        }
        Switch::Tool(name) => {
            let permissions = object(root, "permissions");
            let deny = array(permissions, "deny");
            deny.retain(|entry| entry.as_str() != Some(name));
            if !on {
                deny.push(Value::String(name.clone()));
            }
            drop_empty(permissions, "deny");
            drop_empty(root, "permissions");
        }
        Switch::Hooks => {
            if on {
                root.remove("disableAllHooks");
            } else {
                root.insert("disableAllHooks".into(), Value::Bool(true));
            }
        }
        Switch::JevGuide => crate::nirvana_jev_install::set_hook(
            local,
            crate::nirvana_jev_install::GUIDE_EVENT,
            crate::nirvana_jev_install::GUIDE,
            &crate::nirvana_board::exe(),
            on,
        ),
        Switch::JevCompact => crate::nirvana_jev_install::set_hook(
            local,
            crate::nirvana_jev_install::COMPACT_EVENT,
            crate::nirvana_jev_install::COMPACT,
            &crate::nirvana_board::exe(),
            on,
        ),
    }
}

/// The folder's local settings file, which Claude Code reads for every session started there.
pub fn local_settings_path(cwd: &Path) -> PathBuf {
    cwd.join(".claude").join("settings.local.json")
}

/// The folder's local settings; an empty object when there is no file. A file that is not JSON is an
/// error, never replaced: it is the owner's.
pub fn read_local(cwd: &Path) -> Result<Value, String> {
    let path = local_settings_path(cwd);
    match std::fs::read_to_string(&path) {
        Ok(text) if text.trim().is_empty() => Ok(Value::Object(Map::new())),
        Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|err| format!("{} is not JSON: {err}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Value::Object(Map::new())),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

/// Writes the folder's local settings through a temporary file, so Claude never reads half a file.
pub fn write_local(cwd: &Path, local: &Value) -> Result<(), String> {
    let path = local_settings_path(cwd);
    let dir = path.parent().expect("a parent");
    std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let mut text = serde_json::to_string_pretty(local).map_err(|err| err.to_string())?;
    text.push('\n');
    let temp = dir.join(".settings.local.json.nirvana-tmp");
    std::fs::write(&temp, text).map_err(|err| format!("{}: {err}", temp.display()))?;
    std::fs::rename(&temp, &path).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        format!("{}: {err}", path.display())
    })
}

/// Switches one item in the folder's settings, then reads the folder's list again.
pub fn switch_item(cwd: &Path, switch: &Switch, on: bool) -> Result<Value, String> {
    let mut local = read_local(cwd)?;
    set(&mut local, switch, on);
    write_local(cwd, &local)?;
    // The guide reaches the agents that are not Claude Code through their own files.
    if *switch == Switch::JevGuide {
        crate::nirvana_jev_install::others(cwd, on)?;
    }
    folders()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(cwd.to_path_buf())
        .or_default()
        .view
        .local = local.clone();
    CHANGED.store(true, Ordering::Release);
    refresh(cwd);
    Ok(local)
}

/// A token figure, short: "618", "2.5k", "29.1k", "1m".
pub fn short(tokens: u32) -> String {
    if tokens >= 1_000_000 {
        let m = f64::from(tokens) / 1_000_000.0;
        return format!("{}m", trim_tenths(m));
    }
    if tokens >= 1_000 {
        let k = f64::from(tokens) / 1_000.0;
        return format!("{}k", trim_tenths(k));
    }
    tokens.to_string()
}

fn trim_tenths(value: f64) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").map_or(text.clone(), str::to_owned)
}

/// What the window knows of a folder's list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub report: Option<Report>,
    pub reading: bool,
    /// The last read gave no list (Claude Code missing, too old, or it failed).
    pub failed: bool,
    pub remembered: HashMap<Switch, Remembered>,
    /// The folder's local settings, read with the list and after each switch.
    pub local: Value,
}

#[derive(Default)]
struct Folder {
    view: View,
    read_at: Option<Instant>,
}

static CHANGED: AtomicBool = AtomicBool::new(false);

fn folders() -> &'static Mutex<HashMap<PathBuf, Folder>> {
    static FOLDERS: OnceLock<Mutex<HashMap<PathBuf, Folder>>> = OnceLock::new();
    FOLDERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn store(cwd: &Path, report: Option<Report>) {
    let local = read_local(cwd).ok();
    let mut folders = folders().lock().unwrap_or_else(|e| e.into_inner());
    let folder = folders.entry(cwd.to_path_buf()).or_default();
    folder.view.reading = false;
    if let Some(local) = local {
        folder.view.local = local;
    }
    folder.read_at = Some(Instant::now());
    match report {
        Some(report) => {
            let config_dir = crate::nirvana::claude_config_dir();
            for item in items(
                Some(&report),
                &Value::Null,
                cwd,
                config_dir.as_deref(),
                &HashMap::new(),
            ) {
                if let Some(switch) = item.switch {
                    folder.view.remembered.insert(
                        switch,
                        Remembered {
                            tokens: item.tokens,
                            provider: item.provider,
                        },
                    );
                }
            }
            folder.view.report = Some(report);
            folder.view.failed = false;
        }
        // A failed read keeps the last list.
        None => folder.view.failed = folder.view.report.is_none(),
    }
    CHANGED.store(true, Ordering::Release);
}

fn read(cwd: &Path) -> Option<Report> {
    if !crate::nirvana_usage::on_path("claude") {
        return None;
    }
    let stdout = crate::nirvana_usage::run_in("claude", &CONTEXT_ARGS, Some(cwd), CLAUDE_TIMEOUT)?;
    parse(&stdout)
}

fn start_read(cwd: &Path, force: bool) {
    {
        let mut folders = folders().lock().unwrap_or_else(|e| e.into_inner());
        let folder = folders.entry(cwd.to_path_buf()).or_default();
        let fresh = folder.read_at.is_some_and(|at| at.elapsed() < STALE_AFTER);
        if folder.view.reading || (fresh && !force) {
            return;
        }
        folder.view.reading = true;
    }
    CHANGED.store(true, Ordering::Release);
    let folder = cwd.to_path_buf();
    let spawned = crate::thread_spawn::spawn_named("nirvana-preload", move || {
        let report = read(&folder);
        store(&folder, report);
    });
    if spawned.is_err() {
        store(cwd, None);
    }
}

/// The folder's list as last read. The first ask for a folder, and an ask after `STALE_AFTER`, starts a
/// read on its own thread; under test none starts (it would run this machine's `claude`).
pub fn view(cwd: &Path) -> View {
    #[cfg(test)]
    if let Some(view) = TEST_VIEW.with_borrow(Clone::clone) {
        return view;
    }
    if !cfg!(test) {
        start_read(cwd, false);
    }
    folders()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(cwd)
        .map(|folder| folder.view.clone())
        .unwrap_or_default()
}

/// Reads the folder's list again now, after a switch.
pub fn refresh(cwd: &Path) {
    if !cfg!(test) {
        start_read(cwd, true);
    }
}

#[cfg(test)]
thread_local! {
    static TEST_VIEW: std::cell::RefCell<Option<View>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_view_for_test(view: Option<View>) {
    TEST_VIEW.with_borrow_mut(|slot| *slot = view);
}

/// Whether a list changed since the last call: the window repaints on its next timer tick.
pub fn take_changed() -> bool {
    CHANGED.swap(false, Ordering::AcqRel)
}

/// A report shaped as `claude -p /context --verbose --output-format json` writes it, cut down from the
/// run on 2026-10-16 in a scratch folder (the names and figures are that run's).
#[cfg(test)]
pub(crate) const FIXTURE: &str = r###"[
{"type":"system","subtype":"init","cwd":"C:\\work\\proj","tools":["Task","Bash","WebFetch","mcp__bladebro__act","mcp__claude_ai_Gmail__get_thread","mcp__plugin_github_github__get_me"],
 "mcp_servers":[{"name":"plugin:github:github","status":"connected","source":"plugin"},{"name":"bladebro","status":"connected","source":"user"},{"name":"claude.ai Gmail","status":"connected","source":"claudeai"},{"name":"claude.ai Tavily","status":"needs-auth","source":"claudeai"}],
 "plugins":[{"name":"superpowers","path":"C:\\x","source":"superpowers@superpowers-marketplace"},{"name":"github","path":"C:\\y","source":"github@claude-plugins-official"},{"name":"cc-plugin-telemetry","path":"C:\\z","source":"cc-plugin-telemetry@builtin"}]},
{"type":"assistant","message":{"content":[]},"context_usage":{"model":"claude-opus-5-5","total_tokens":23607,"raw_max_tokens":1000000,"percentage":2,
 "categories":[{"name":"System prompt","tokens":2561,"kind":"used"},{"name":"System tools","tokens":5381,"kind":"used"},{"name":"MCP tools","tokens":618,"kind":"used"},{"name":"MCP server instructions","tokens":2685,"kind":"used"},{"name":"MCP tools (deferred)","tokens":129919,"kind":"deferred"},{"name":"Custom agents","tokens":450,"kind":"used"},{"name":"Memory files","tokens":155,"kind":"used"},{"name":"Skills","tokens":9971,"kind":"used"},{"name":"Messages","tokens":1620,"kind":"used"},{"name":"Free space","tokens":943600,"kind":"free"}],
 "mcp_tools":[{"name":"mcp__bladebro__act","server_name":"bladebro","tokens":3224},{"name":"mcp__bladebro__run","server_name":"bladebro","tokens":864},{"name":"mcp__claude_ai_Gmail__get_thread","server_name":"claude_ai_Gmail","tokens":1200},{"name":"mcp__plugin_github_github__get_me","server_name":"plugin_github_github","tokens":108}],
 "memory_files":[{"path":"C:\\work\\proj\\CLAUDE.md","type":"Project","tokens":57},{"path":"C:\\work\\proj\\CLAUDE.local.md","type":"Local","tokens":44},{"path":"C:\\home\\.claude\\projects\\C--work-proj\\memory\\MEMORY.md","type":"AutoMem","tokens":3902}],
 "agents":[{"agent_type":"demo","source":"projectSettings","tokens":27},{"agent_type":"superpowers:code-reviewer","source":"plugin","tokens":350}],
 "skills":[{"name":"benchmark","source":"userSettings","tokens":112},{"name":"superpowers:brainstorming","source":"plugin","plugin_name":"superpowers","tokens":76},{"name":"dataviz","source":"built-in","tokens":480},{"name":"docx","source":"syncedSkills","tokens":12}]}},
{"type":"result","subtype":"success","total_cost_usd":0,"result":"## Context Usage"}
]"###;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn report() -> Report {
        parse(FIXTURE).expect("a /context report")
    }

    #[test]
    fn the_report_reads_into_its_total_and_items() {
        let report = report();
        assert_eq!(report.total, 23_607);
        assert_eq!(report.window, 1_000_000);
        assert_eq!(report.category("Memory files"), 155);
        assert_eq!(report.memory.len(), 3);
        assert_eq!(report.memory[2].source, "AutoMem");
        assert_eq!(report.skills[1].extra, "superpowers");
        assert_eq!(report.mcp_tools[0].source, "bladebro");
        assert_eq!(report.servers[2].name, "claude.ai Gmail");
        assert_eq!(
            report.plugins[0].source,
            "superpowers@superpowers-marketplace"
        );
        assert_eq!(report.tools, vec!["Task", "Bash", "WebFetch"]);
        // Without --verbose the answer is one object with the markdown alone.
        assert_eq!(
            parse(r###"{"type":"result","result":"## Context Usage"}"###),
            None
        );
    }

    #[test]
    fn each_kind_of_item_gets_its_proven_switch() {
        let cwd = Path::new(r"C:\work\proj");
        let items = items(
            Some(&report()),
            &json!({}),
            cwd,
            Some(Path::new(r"C:\home\.claude")),
            &HashMap::new(),
        );
        let find = |name: &str| {
            items
                .iter()
                .find(|item| item.name == name)
                .unwrap_or_else(|| panic!("{name}"))
        };
        assert_eq!(
            find("CLAUDE.md").switch,
            Some(Switch::ClaudeMd("C:/work/proj/CLAUDE.md".into()))
        );
        assert_eq!(find("CLAUDE.local.md").detail, "memory file");
        assert_eq!(find("MEMORY.md").switch, Some(Switch::AutoMemory));
        assert_eq!(
            find("benchmark").switch,
            Some(Switch::Skill("benchmark".into()))
        );
        assert_eq!(
            find("dataviz").switch,
            Some(Switch::Skill("dataviz".into()))
        );
        // A plugin's skill is switched with its plugin.
        assert_eq!(find("superpowers:brainstorming").switch, None);
        assert_eq!(find("superpowers:brainstorming").detail, "skill");
        assert_eq!(
            find("bladebro").switch,
            Some(Switch::McpServer("bladebro".into()))
        );
        assert_eq!(find("bladebro").tokens, Some(3_224 + 864));
        assert_eq!(find("bladebro").detail, "MCP, 2 tools");
        assert_eq!(find("claude.ai Gmail").tokens, Some(1_200));
        assert_eq!(
            find("claude.ai Tavily").detail,
            "connector, 0 tools, needs-auth"
        );
        assert_eq!(
            find("github").switch,
            Some(Switch::Plugin("github@claude-plugins-official".into()))
        );
        assert_eq!(find("github").tokens, Some(108));
        assert_eq!(find("superpowers").tokens, Some(76 + 350));
        assert!(items.iter().all(|item| item.name != "cc-plugin-telemetry"));
        assert_eq!(find("demo").switch, None);
        assert_eq!(find("Bash").switch, Some(Switch::Tool("Bash".into())));
        assert_eq!(find("session start text").tokens, Some(1_620));
        // Every item is on with no local settings, but herdr's own Jev switches, which are off until
        // switched on (feature 9).
        assert!(items
            .iter()
            .all(|item| item.on || item.provider == Provider::Jev));
        assert!(!find("Jev guide").on && !find("Compact advice").on);
        // Grouped by provider, then by kind inside a provider.
        assert!(items.windows(2).all(|pair| {
            (&pair[0].provider, pair[0].group) <= (&pair[1].provider, pair[1].group)
        }));
    }

    #[test]
    fn a_switched_off_item_stays_listed_off_with_its_last_tokens() {
        let cwd = Path::new(r"C:\work\proj");
        let mut local = json!({"permissions": {"deny": ["Bash(rm:*)"]}});
        set(&mut local, &Switch::Skill("benchmark".into()), false);
        // Still listed by the report (this Claude has not restarted), but switched off.
        set(
            &mut local,
            &Switch::ClaudeMd("C:/work/proj/CLAUDE.md".into()),
            false,
        );
        set(&mut local, &Switch::McpServer("bladebro".into()), false);
        let mut remembered = HashMap::new();
        remembered.insert(
            Switch::McpServer("bladebro".into()),
            Remembered {
                tokens: Some(4_088),
                provider: Provider::User,
            },
        );
        // The next report no longer lists them.
        let mut next = report();
        next.skills.retain(|skill| skill.name != "benchmark");
        next.servers.retain(|server| server.name != "bladebro");
        let items = items(Some(&next), &local, cwd, None, &remembered);
        let bladebro = items
            .iter()
            .find(|item| item.name == "bladebro")
            .expect("bladebro listed");
        assert!(!bladebro.on);
        assert_eq!(bladebro.tokens, Some(4_088));
        assert_eq!(bladebro.provider, Provider::User);
        let benchmark = items
            .iter()
            .find(|item| item.name == "benchmark")
            .expect("benchmark listed");
        assert!(!benchmark.on);
        assert_eq!(benchmark.group, Group::Skills);
        let claude_md: Vec<_> = items
            .iter()
            .filter(|item| item.name == "CLAUDE.md")
            .collect();
        assert_eq!(claude_md.len(), 1, "listed once");
        assert!(!claude_md[0].on);
        assert_eq!(claude_md[0].tokens, Some(57));
        // A permission rule with a pattern is not a switched-off tool.
        assert!(items.iter().all(|item| item.name != "Bash(rm:*)"));
    }

    #[test]
    fn each_item_is_grouped_under_the_provider_it_comes_from() {
        let cwd = Path::new(r"C:\work\proj");
        let items = items(
            Some(&report()),
            &json!({}),
            cwd,
            Some(Path::new(r"C:\home\.claude")),
            &HashMap::new(),
        );
        let provider = |name: &str| {
            items
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.provider.clone())
                .unwrap_or_else(|| panic!("{name}"))
        };
        let plugin = |name: &str| Provider::Plugin(name.into());
        assert_eq!(provider("CLAUDE.md"), Provider::Project);
        assert_eq!(provider("CLAUDE.local.md"), Provider::Project);
        assert_eq!(provider("demo"), Provider::Project);
        assert_eq!(provider("MEMORY.md"), Provider::User);
        assert_eq!(provider("benchmark"), Provider::User);
        assert_eq!(provider("bladebro"), Provider::User);
        assert_eq!(provider("superpowers"), plugin("superpowers"));
        assert_eq!(provider("superpowers:brainstorming"), plugin("superpowers"));
        assert_eq!(provider("superpowers:code-reviewer"), plugin("superpowers"));
        assert_eq!(provider("github"), plugin("github"));
        assert_eq!(provider("plugin:github:github"), plugin("github"));
        assert_eq!(provider("claude.ai Gmail"), Provider::ClaudeAi);
        assert_eq!(provider("docx"), Provider::ClaudeAi);
        assert_eq!(provider("dataviz"), Provider::BuiltIn);
        assert_eq!(provider("Bash"), Provider::BuiltIn);
        assert_eq!(provider("session start text"), Provider::BuiltIn);
        // A plugin's own row heads its group; its header leaves that row out, which sums the rest.
        let superpowers: Vec<_> = items
            .iter()
            .filter(|item| item.provider == plugin("superpowers"))
            .collect();
        assert_eq!(superpowers[0].name, "superpowers");
        assert_eq!(
            provider_tokens(&items, &plugin("superpowers"), None),
            76 + 350
        );
        // Built in adds the built-in tools' category to its rows.
        assert_eq!(
            provider_tokens(&items, &Provider::BuiltIn, Some(&report())),
            480 + 1_620 + 5_381
        );
    }

    #[test]
    fn a_switched_off_item_nothing_remembers_is_grouped_by_its_switch() {
        let cwd = Path::new(r"C:\work\proj");
        let config = Path::new(r"C:\home\.claude");
        let of = |switch: Switch| Provider::of_switch(&switch, cwd, Some(config));
        assert_eq!(
            of(Switch::ClaudeMd("C:/work/proj/CLAUDE.md".into())),
            Provider::Project
        );
        assert_eq!(
            of(Switch::ClaudeMd("C:/home/.claude/CLAUDE.md".into())),
            Provider::User
        );
        assert_eq!(of(Switch::AutoMemory), Provider::User);
        assert_eq!(
            of(Switch::McpServer("claude.ai Gmail".into())),
            Provider::ClaudeAi
        );
        assert_eq!(
            of(Switch::McpServer("plugin:github:github".into())),
            Provider::Plugin("github".into())
        );
        assert_eq!(of(Switch::McpServer("bladebro".into())), Provider::User);
        assert_eq!(
            of(Switch::Plugin("superpowers@superpowers-marketplace".into())),
            Provider::Plugin("superpowers".into())
        );
        assert_eq!(of(Switch::Tool("WebFetch".into())), Provider::BuiltIn);
        assert_eq!(of(Switch::Skill("no-such-skill".into())), Provider::BuiltIn);
    }

    #[test]
    fn switching_writes_only_the_items_entry_and_back_leaves_the_file_as_it_was() {
        let before = json!({"permissions": {"allow": ["Bash(ls:*)"], "deny": ["Bash(rm:*)"]}, "model": "opus"});
        let switches = [
            Switch::ClaudeMd("C:/work/proj/CLAUDE.md".into()),
            Switch::AutoMemory,
            Switch::Skill("benchmark".into()),
            Switch::McpServer("claude.ai Gmail".into()),
            Switch::Plugin("github@claude-plugins-official".into()),
            Switch::Tool("WebFetch".into()),
            Switch::Hooks,
        ];
        let mut local = before.clone();
        for switch in &switches {
            set(&mut local, switch, false);
            assert!(is_off(&local, switch), "{switch:?}");
        }
        assert_eq!(local["claudeMdExcludes"], json!(["C:/work/proj/CLAUDE.md"]));
        assert_eq!(local["autoMemoryEnabled"], json!(false));
        assert_eq!(local["skillOverrides"], json!({"benchmark": "off"}));
        assert_eq!(
            local["deniedMcpServers"],
            json!([{"serverName": "claude.ai Gmail"}])
        );
        assert_eq!(
            local["enabledPlugins"],
            json!({"github@claude-plugins-official": false})
        );
        assert_eq!(
            local["permissions"]["deny"],
            json!(["Bash(rm:*)", "WebFetch"])
        );
        assert_eq!(local["disableAllHooks"], json!(true));
        // Switching off twice adds nothing twice.
        set(&mut local, &switches[0], false);
        assert_eq!(local["claudeMdExcludes"].as_array().map(Vec::len), Some(1));
        for switch in &switches {
            set(&mut local, switch, true);
            assert!(!is_off(&local, switch), "{switch:?}");
        }
        assert_eq!(local, before);
    }

    #[test]
    fn the_local_settings_file_round_trips_and_a_broken_one_is_left_alone() {
        let dir = std::env::temp_dir().join(format!("nirvana-preload-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        assert_eq!(read_local(&dir), Ok(json!({})));
        let local = switch_item(&dir, &Switch::Hooks, false).expect("written");
        assert_eq!(read_local(&dir), Ok(local));
        std::fs::write(local_settings_path(&dir), "{ not json").expect("broken file");
        assert!(switch_item(&dir, &Switch::Hooks, true).is_err());
        assert_eq!(
            std::fs::read_to_string(local_settings_path(&dir))
                .ok()
                .as_deref(),
            Some("{ not json")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn token_figures_are_short() {
        assert_eq!(short(618), "618");
        assert_eq!(short(2_000), "2k");
        assert_eq!(short(23_607), "23.6k");
        assert_eq!(short(1_000_000), "1m");
        assert_eq!(
            tool_server_key("claude.ai Booking.com"),
            "claude_ai_Booking_com"
        );
        assert_eq!(
            tool_server_key("plugin:github:github"),
            "plugin_github_github"
        );
    }
}
