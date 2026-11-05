//! Plan 39 P39c: the phone answers an agent's prompt, and stops it with Esc (§2.4, prototype screen 4).
//!
//! **Which agents are blocked** is herdr's word, not this file's: its detector marks a pane `blocked` from the agent's
//! own manifest (`src/detect/manifests/claude.toml`, rules `live_blocked_form`, `bash_permission_prompt`,
//! `generic_permission_prompt` and the rest), and the page's `needs` is that state (`spaces::status_word`).
//!
//! **What the menu says** is read off the agent's screen (`agent.read`, the visible rows) by the Running square's own
//! reader, `nirvana_running::prompt` (`src/nirvana_running.rs:213`), which the window uses for the same purpose
//! (`src/client/shell/nirvana_mods.rs:1294`): heading lines, then options numbered from 1. One digit picks an option,
//! no Enter: measured with Claude Code 2.1.301 on 2026-10-16 (`nirvana_running.rs:13-15`), and the window answers the
//! same way (`running_answer`, `nirvana_mods.rs:1301`). So a menu is read as buttons only for a Claude
//! (`chat::is_claude`), and only when its last option sits at the bottom of the screen above a footer that names Esc
//! (`at_bottom`): a numbered list Claude wrote further up is not its menu. Every other screen of a blocked agent
//! (Codex, Antigravity, an MCP form, a Claude dialog without numbers) is shown raw, with 1 to 9, Enter and Esc.
//!
//! **A key goes only to the prompt the phone saw** (order item 4). The page gets `seen`, a hash of what it was
//! shown (the menu's lines and options, or the raw screen), and sends it back with the key. The server reads the
//! screen again before it presses anything: an agent no longer blocked is "answered on the PC"; a different menu is
//! refused and sent back so the page shows the new one. What it cannot close: the few milliseconds between that
//! second read and `agent.send_keys`, and a prompt that comes back word for word (the same command asked twice),
//! which reads as the one the phone saw.
//!
//! **Keys** come from `KEYS` only: the request names one of them and the name sent to herdr is the table's own
//! `&'static str`, never the request's text.

use serde::Deserialize;
use serde_json::{json, Value};

use super::chat;
use super::http::{Conn, Framing, Request, Response};
use super::live::query_value;
use super::routes::Site;
use super::spaces;

/// The keys the phone may press, as the request names them, and the `agent.send_keys` name each is sent as
/// (`src/config/keybinds.rs:1315`: `enter`, `esc`, and a single character).
pub const KEYS: &[(&str, &str)] = &[
    ("1", "1"),
    ("2", "2"),
    ("3", "3"),
    ("4", "4"),
    ("5", "5"),
    ("6", "6"),
    ("7", "7"),
    ("8", "8"),
    ("9", "9"),
    ("enter", "enter"),
    ("esc", "esc"),
];

/// A key's body is `{"agent", "key", "seen"}`: a few dozen bytes.
pub const KEY_MOST: u64 = 4 * 1024;
/// The rows of a raw screen the phone is shown: the bottom of the pane, where a prompt is drawn.
pub const RAW_LINES: usize = 24;
/// What a menu's footer row says: Claude Code's "Enter to select · ↑/↓ to navigate · Esc to cancel",
/// "Esc to cancel · Tab to amend", "Enter to confirm · Esc to exit".
const FOOTER_WORDS: &[&str] = &["esc to", "enter to", "tab to", "to navigate"];

/// The key `asked` names, from `KEYS`; `None` for anything else.
pub fn key(asked: &str) -> Option<&'static str> {
    KEYS.iter()
        .find(|(name, _)| *name == asked)
        .map(|(_, key)| *key)
}

/// What the phone is shown for a blocked agent: its menu when it reads as one, else the raw bottom of its screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub menu: Option<crate::nirvana_running::Prompt>,
    pub screen: String,
    /// The hash the tap carries back (`seen`).
    pub seen: String,
}

impl Shown {
    /// The keys this prompt takes: a menu's own digits and Esc, no Enter (the highlighted option can move on the
    /// PC without the menu's text changing); a raw screen takes every key.
    pub fn takes(&self, key: &str) -> bool {
        match &self.menu {
            Some(menu) => {
                key == "esc"
                    || menu
                        .options
                        .iter()
                        .any(|(digit, _)| key == digit.to_string())
            }
            None => true,
        }
    }

    pub fn keys(&self) -> Vec<&'static str> {
        KEYS.iter()
            .map(|(name, _)| *name)
            .filter(|name| self.takes(name))
            .collect()
    }

    fn json(&self) -> Value {
        let menu = self.menu.as_ref().map(|menu| {
            let options: Vec<Value> = menu
                .options
                .iter()
                .map(|(digit, label)| json!({"key": digit.to_string(), "label": label}))
                .collect();
            json!({ "lines": menu.lines, "options": options })
        });
        json!({
            "menu": menu,
            "screen": if self.menu.is_some() { "" } else { self.screen.as_str() },
            "seen": self.seen,
            "keys": self.keys(),
        })
    }
}

/// FNV-1a, 64 bits: stable across builds and restarts, so a `seen` from before a restart still compares. Not a
/// secret: the text it hashes is what the phone was shown.
fn hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Whether the menu's last option, option `last`, is the bottom of the screen: after it come only rules and footer
/// rows (`FOOTER_WORDS`), one of them naming Esc. Anything else there (an MCP form's "Accept", a question's own
/// lines) means the numbers are a list Claude wrote above the real prompt, and the screen is shown raw.
fn at_bottom(rows: &[String], last: u8) -> bool {
    let mark = format!("{last}. ");
    let Some(at) = rows.iter().rposition(|row| {
        row.trim_start()
            .trim_start_matches('\u{276F}')
            .trim_start()
            .starts_with(&mark)
    }) else {
        return false;
    };
    let after: Vec<String> = rows[at + 1..]
        .iter()
        .map(|row| row.trim().to_lowercase())
        .filter(|row| !row.is_empty())
        .collect();
    let rule = |row: &str| {
        row.chars()
            .all(|c| matches!(c, '\u{2500}' | '\u{254C}' | '-'))
    };
    let footer = |row: &str| FOOTER_WORDS.iter().any(|words| row.contains(words));
    after.iter().all(|row| rule(row) || footer(row))
        && after.iter().any(|row| row.contains("esc to"))
}

/// The prompt on `screen` (`agent.read`'s text) as the phone is shown it.
pub fn shown(agent: &Value, screen: &str) -> Shown {
    let rows: Vec<String> = screen
        .lines()
        .map(|row| row.trim_end().to_string())
        .collect();
    let menu = chat::is_claude(agent)
        .then(|| crate::nirvana_running::prompt(&rows))
        .flatten()
        .filter(|menu| {
            menu.options
                .last()
                .is_some_and(|(last, _)| at_bottom(&rows, *last))
        });
    let mut bottom: Vec<&str> = rows.iter().map(String::as_str).collect();
    while bottom.last().is_some_and(|row| row.trim().is_empty()) {
        bottom.pop();
    }
    let skip = bottom.len().saturating_sub(RAW_LINES);
    let screen = bottom[skip..].join("\n");
    let seen = match &menu {
        Some(menu) => {
            let mut text = String::from("menu\n");
            for line in &menu.lines {
                text.push_str(line);
                text.push('\n');
            }
            for (digit, label) in &menu.options {
                text.push_str(&format!("{digit}. {label}\n"));
            }
            hash(&text)
        }
        None => hash(&format!("raw\n{screen}")),
    };
    Shown { menu, screen, seen }
}

fn error(status: u16, why: impl Into<String>) -> Response {
    Response::json(status, &json!({ "error": why.into() }))
}

#[derive(Deserialize)]
struct KeyBody {
    agent: String,
    key: String,
    #[serde(default)]
    seen: Option<String>,
}

/// What a page that holds a stale prompt is told: the agent's prompt now, or that it has none.
const ANSWERED: &str = "answered on the PC";

impl Site {
    /// The visible rows of the agent's pane: where a prompt is drawn.
    fn screen(&self, id: &str) -> Result<String, Response> {
        self.herdr
            .call(
                "agent.read",
                json!({"target": id, "source": "visible", "format": "text", "strip_ansi": true}),
            )
            .map(|read| read["read"]["text"].as_str().unwrap_or("").to_string())
            .map_err(|why| error(502, why))
    }

    /// `{"blocked": false}`, or the prompt the phone is shown for a blocked agent.
    fn prompt_now(&self, found: &super::live::Found) -> Result<(Value, Option<Shown>), Response> {
        let mut row = spaces::agent_row(&found.agent);
        row["space"] = json!(found.space);
        if found.agent["agent_status"].as_str() != Some("blocked") {
            return Ok((json!({"blocked": false, "agent": row}), None));
        }
        let shown = shown(&found.agent, &self.screen(found.id())?);
        let mut payload = shown.json();
        payload["blocked"] = json!(true);
        payload["agent"] = row;
        Ok((payload, Some(shown)))
    }

    /// `GET /api/prompt?agent=<id>`.
    pub(super) fn prompt(&self, request: &Request) -> Response {
        let Some(id) = query_value(&request.query, "agent") else {
            return error(400, "which agent?");
        };
        let found = match self.found(&id) {
            Ok(found) => found,
            Err(refused) => return refused,
        };
        match self.prompt_now(&found) {
            Ok((payload, _)) => Response::json(200, &payload),
            Err(refused) => refused,
        }
    }

    /// `POST /api/key` `{"agent", "key", "seen"}`: one key, to the prompt the phone saw (`seen`), or Esc to a
    /// working agent (`seen` null).
    pub(super) fn key(&self, request: &Request, conn: &mut Conn) -> Response {
        let json_body = request.header("Content-Type").first().is_some_and(|kind| {
            kind.trim()
                .to_ascii_lowercase()
                .starts_with("application/json")
        });
        if !json_body {
            return error(415, "a key is JSON");
        }
        if conn.framing() == Framing::None {
            return error(411, "a key has a length");
        }
        let mut body = Vec::new();
        if let Err(why) = conn.read_body(KEY_MOST, &mut body) {
            return error(why.status(), "the key could not be read");
        }
        let Ok(body) = serde_json::from_slice::<KeyBody>(&body) else {
            return error(400, "a key is {agent, key, seen}");
        };
        // Checked before herdr is asked anything: a key not in the table goes nowhere.
        let Some(key) = key(&body.key) else {
            return error(400, "not a key the phone may press");
        };
        let found = match self.found(&body.agent) {
            Ok(found) => found,
            Err(refused) => return refused,
        };
        let (now, shown) = match self.prompt_now(&found) {
            Ok(read) => read,
            Err(refused) => return refused,
        };
        match (shown, body.seen.as_deref()) {
            // The phone saw this prompt: the key goes, if the prompt takes it.
            (Some(shown), Some(seen)) if shown.seen == seen => {
                if !shown.takes(key) {
                    return error(400, "not one of this prompt's keys");
                }
            }
            // Another prompt than the one the phone saw, or one where the phone saw none: nothing is pressed, and
            // the page is sent the prompt as it is now.
            (Some(_), _) => {
                return Response::json(
                    409,
                    &json!({"error": "the prompt changed on the PC", "prompt": now}),
                )
            }
            // The phone saw a prompt and there is none now: the PC answered it.
            (None, Some(_)) => {
                return Response::json(409, &json!({"error": ANSWERED, "prompt": now}))
            }
            // No prompt either way: Esc stops a working agent, as on the PC; nothing else is pressed.
            (None, None) => {
                if key != "esc" {
                    return error(409, "only Esc goes to an agent with no prompt");
                }
                if found.agent["agent_status"].as_str() != Some("working") {
                    return Response::json(
                        409,
                        &json!({"error": "the agent is not working: nothing to stop", "prompt": now}),
                    );
                }
            }
        }
        match self.herdr.call(
            "agent.send_keys",
            json!({"target": found.id(), "keys": [key]}),
        ) {
            Ok(_) => Response::json(200, &json!({"sent": key})),
            Err(why) => error(502, why),
        }
    }
}
