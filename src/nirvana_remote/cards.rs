//! Plan 39 P39d, §2.6: the Usage, Servers and Running cards on screen 2 (the prototype's three cards above the
//! spaces), each a read-only JSON route drawn from the same data the PC's own square draws.
//!
//! - **Usage** (`GET /api/cards/usage`). The sidebar foot's usage rows are `nirvana_usage::rows()`
//!   (`src/client/shell/sidebar.rs:558`): `foot_rows` over each reader's last `Source` (`src/nirvana_usage.rs:1320`),
//!   less `FOOT_HIDDEN` (`:1308-1315`). The window's readers write every `Source` they take into the shared cache
//!   file (`start_worker`, `:1286-1293`; `write_source`, `:1044`), `<Nirvana home>/nirvana/usage.json`
//!   (`cache_path`, `:1030`). This card reads that file and runs the same `foot_rows` and the same filter. It
//!   starts no reader: a reader runs `claude` and `agy`, which the herdr server must not start for a phone.
//! - **Servers** (`GET /api/cards/servers`). The foot's servers block is `server_rows`
//!   (`src/client/shell/nirvana_footer.rs:76-93`): each space's `nirvana_servers` token, in the spaces' order, which
//!   the server sets from `nirvana_servers::cached_ports` (`src/server/client_shell.rs:42-64`, `:113-120`). The
//!   cache lives in this same herdr server process, so the card reads it directly (`nirvana_servers::last_ports`),
//!   without recording shells the way `cached_ports` does. The block leads with Nirvana Remote's own row
//!   (`nirvana_footer::REMOTE`); a phone that reads this card reached it, so it is up on `https_port`.
//! - **Running** (`GET /api/cards/running`). The tab strip's Running square says, for its agent,
//!   "waits on you" when blocked, "working · <tool>" while a tool call is open in its transcript, else "working"
//!   (`running_value`, `src/client/shell/nirvana_mods.rs:119-130`), the tool from `nirvana_running::cached` over
//!   the agent's `nirvana_transcript` token (`:79-83`, `src/client/shell/composition.rs:244-247`). The square shows
//!   one agent; the card counts every agent at work (working or blocked, as the PC list's line counts them,
//!   `herdr::summary`) and gives each its square's words. The token's path is used only when it is a transcript
//!   under Claude Code's folder (`chat::is_transcript`).
//!
//! **What each field is, and why it may leave the PC.** Nothing here is a credential, a token, an account or an
//! e-mail; each field is one the PC's square already shows on its screen.
//! - Usage: `pool` and `label` (the pool's name, "Claude", "Gemini"), `provider` (the reader's key), per window
//!   `window` ("5h", "week"), `used` (a share, 0 to 1), `resets_in` (seconds), `stale` and `ahead` (the square's
//!   age and pace marks), `fill`, a `memory` reading in MiB for the local engine, or a `word` ("not exposed").
//!   Left on the PC: each reading's `source` (which command read it) and `resets_text` (Claude's reset as printed,
//!   with the owner's time zone); the cache has no account or login in it at all (`Reading`, `:169-187`).
//! - Servers: each space's name (already in `/api/spaces`) and its port numbers, and the remote row's port. No
//!   process, command line or URL.
//! - Running: each agent's pane id, name and space and its state (all in `/api/spaces` already), and the square's
//!   words, whose tool part is the tool's name only ("Bash"), never its command, path or input
//!   (`nirvana_running::Tool::detail` is not sent).

use std::path::Path;

use serde_json::{json, Value};

use super::routes::Site;
use crate::nirvana_running::Activity;
use crate::nirvana_usage::{self as usage, Figure, Provider, Shown, Source};

/// The cache's sources in `foot_rows`'s order. `Provider::ALL` is that order (`Provider::index`).
pub fn sources(cache: &usage::Cache) -> [Source; 4] {
    Provider::ALL.map(|provider| {
        cache
            .sources
            .get(provider.key())
            .cloned()
            .unwrap_or_default()
    })
}

fn window(shown: &Shown) -> Value {
    json!({
        "window": shown.window,
        "used": shown.used,
        "resets_in": shown.resets_in,
        "stale": shown.stale,
        "ahead": shown.ahead,
    })
}

/// One usage row as the card draws it.
fn usage_row(row: &usage::UsageRow) -> Value {
    let mut out = json!({
        "pool": row.pool,
        "label": row.label(),
        "provider": row.provider.key(),
        "fill": row.figure.fill(),
    });
    match &row.figure {
        Figure::Reading => out["word"] = json!("reading"),
        Figure::Word(word) => out["word"] = json!(word),
        Figure::Memory {
            used_mib,
            total_mib,
        } => out["memory"] = json!({ "used_mib": used_mib, "total_mib": total_mib }),
        Figure::Window(_) | Figure::Windows(_) => {
            let windows: Vec<Value> = row
                .figure
                .windows()
                .unwrap_or_default()
                .iter()
                .map(window)
                .collect();
            out["windows"] = json!(windows);
        }
    }
    out
}

/// The usage card from the cache file, at `now`: the foot's rows, in the foot's order, the hidden pools left out.
pub fn usage_card(cache: &usage::Cache, now: u64) -> Value {
    let rows: Vec<Value> = usage::foot_rows(&sources(cache), now)
        .iter()
        .filter(|row| !usage::FOOT_HIDDEN.contains(&row.pool.as_str()))
        .map(usage_row)
        .collect();
    json!({ "rows": rows })
}

/// The servers card from a `workspace.list` answer: Nirvana Remote first, then each space's ports in the spaces'
/// order, as the foot's block lists them.
pub fn servers_card(
    workspaces: &Value,
    ports: &dyn Fn(&str) -> Vec<u16>,
    https_port: u16,
) -> Value {
    let rows: Vec<Value> = workspaces["workspaces"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .flat_map(|space| {
            let name = space["label"].as_str().unwrap_or("").to_string();
            ports(space["workspace_id"].as_str().unwrap_or(""))
                .into_iter()
                .map(move |port| json!({ "space": name, "port": port }))
        })
        .collect();
    json!({
        "remote": { "name": "remote", "port": https_port },
        "up": rows.len(),
        "servers": rows,
    })
}

/// The Running square's words for an agent at work (`running_value`): `None` for one that is not.
pub fn running_words(status: &str, activity: Option<&Activity>) -> Option<String> {
    match status {
        "blocked" => Some("waits on you".into()),
        "working" => Some(
            match activity
                .and_then(|activity| activity.now.as_ref())
                .filter(|tool| !tool.name.is_empty())
            {
                Some(tool) => format!("working \u{b7} {}", tool.name),
                None => "working".into(),
            },
        ),
        _ => None,
    }
}

/// The running card from `workspace.list` and `agent.list` answers; `activity` reads an agent's open tool call.
/// An agent in no space of this herdr is left out, as the spaces list leaves it out.
pub fn running_card(
    workspaces: &Value,
    agents: &Value,
    activity: &dyn Fn(&Value) -> Option<Activity>,
) -> Value {
    let spaces = workspaces["workspaces"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut working = 0;
    let mut needs = 0;
    let rows: Vec<Value> = agents["agents"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|agent| {
            let space = spaces
                .iter()
                .find(|space| space["workspace_id"] == agent["workspace_id"])?;
            let status = agent["agent_status"].as_str().unwrap_or("");
            let words = running_words(status, activity(agent).as_ref())?;
            if status == "blocked" {
                needs += 1;
            } else {
                working += 1;
            }
            let row = super::spaces::agent_row(agent);
            Some(json!({
                "id": row["id"],
                "name": row["name"],
                "space": space["label"],
                "status": row["status"],
                "words": words,
            }))
        })
        .collect();
    json!({ "working": working, "needs": needs, "agents": rows })
}

/// The agent's open tool call, read by `read` (`nirvana_running::cached`) from its `nirvana_transcript` token, as
/// the Running square reads it; only a token that is a transcript under Claude Code's folder is read.
pub fn activity_of(
    agent: &Value,
    claude: Option<&Path>,
    read: &dyn Fn(&Path) -> Option<Activity>,
) -> Option<Activity> {
    let claude = claude?;
    let token = agent["tokens"][crate::nirvana_spaces::TRANSCRIPT_TOKEN].as_str()?;
    let path = Path::new(token);
    super::chat::is_transcript(path, claude)
        .then(|| read(path))
        .flatten()
}

fn error(why: impl Into<String>) -> super::http::Response {
    super::http::Response::json(502, &json!({ "error": why.into() }))
}

impl Site {
    pub(super) fn usage_card(&self) -> super::http::Response {
        let cache = self
            .nirvana_home
            .as_deref()
            .map(|home| usage::read_cache(&home.join(usage::CACHE_DIR).join(usage::CACHE_FILE)))
            .unwrap_or_default();
        let now = (self.now)().unix_timestamp().max(0) as u64;
        super::http::Response::json(200, &usage_card(&cache, now))
    }

    pub(super) fn servers_card(&self) -> super::http::Response {
        match self.herdr.call("workspace.list", json!({})) {
            Ok(workspaces) => super::http::Response::json(
                200,
                &servers_card(&workspaces, self.local.ports.as_ref(), self.https_port),
            ),
            Err(why) => error(why),
        }
    }

    pub(super) fn running_card(&self) -> super::http::Response {
        let lists = self
            .herdr
            .call("workspace.list", json!({}))
            .and_then(|workspaces| Ok((workspaces, self.herdr.call("agent.list", json!({}))?)));
        let (workspaces, agents) = match lists {
            Ok(lists) => lists,
            Err(why) => return error(why),
        };
        let claude = self.claude_dir.as_deref();
        let activity = |agent: &Value| activity_of(agent, claude, &crate::nirvana_running::cached);
        super::http::Response::json(200, &running_card(&workspaces, &agents, &activity))
    }
}
