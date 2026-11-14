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
//! **A key goes only to the prompt the phone saw** (order item 4). The page gets `seen`, a hash of the whole prompt
//! (a menu's every row from the rule above it through its footer, though the sheet shows only the capped heading
//! lines; or the raw screen), and sends it back with the key. The server reads the screen again before it presses
//! anything: an agent no longer blocked is "answered on the PC"; a different menu is refused and sent back so the page
//! shows the new one. **One press per prompt**: a `seen` pressed once, and an Esc sent once, are spent for
//! `SPENT_FOR`, so a double tap or a second phone cannot put a second key into whatever Claude draws next (review S2,
//! S4). What it cannot close: the few milliseconds between that second read and `agent.send_keys`, and a prompt that
//! comes back word for word after `SPENT_FOR` (the same command asked twice), which reads as the one the phone saw.
//!
//! **Keys** come from `KEYS` only: the request names one of them and the name sent to herdr is the table's own
//! `&'static str`, never the request's text.
//!
//! **Pickers** (alpha 0.68.0). The owner, 2026-11-10, sent `/resume` from the phone and then "1": the PC opened Claude
//! Code's session picker, which herdr does not mark blocked (the pane read "done"), so no sheet opened, and the "1"
//! went in as a search with an Enter ("No sessions match "1""). A picker is any agent's screen, not working, whose
//! bottom rows close on a footer that names a way out (`PICKER_EXITS`: "Esc to cancel", "Esc to exit"); measured on
//! Claude Code 2.1.322's `/resume`, whose footer wraps over two rows and whose highlight is a `❯` in plain text. The
//! phone gets its screen from its top border down and the arrows, Enter, Esc and Backspace, and may type a search
//! (`/api/type`, pasted, without Enter) where the footer offers one. Its `seen` is the footer's hash, so moving the
//! highlight or typing a search is the same picker; the arrows and Backspace are not spent, Enter and Esc are.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
    // A picker's keys (alpha 0.68.0): taken only by a picker (`Shown::takes`).
    ("up", "up"),
    ("down", "down"),
    ("backspace", "backspace"),
];

/// The keys a picker takes. The arrows and Backspace only move or edit within it, so they are pressed as often as
/// the owner taps (`NAVIGATION`); Enter and Esc leave it, and are spent like a menu's digit.
const PICKER_KEYS: &[&str] = &["up", "down", "enter", "esc", "backspace"];
const NAVIGATION: &[&str] = &["up", "down", "backspace"];
/// A footer that ends a picker: the way out. "esc to interrupt" (a working Claude) is not one.
const PICKER_EXITS: &[&str] = &[
    "esc to cancel",
    "esc to exit",
    "esc to close",
    "esc to go back",
];
/// How many bottom rows a picker's footer may wrap over (`/resume`'s takes two at 120 columns).
const FOOTER_ROWS: usize = 3;
/// The most a typed search may be.
pub const SEARCH_MOST: usize = 200;

/// A key's body is `{"agent", "key", "seen"}`: a few dozen bytes.
pub const KEY_MOST: u64 = 4 * 1024;
/// The rows of a raw screen the phone is shown: the bottom of the pane, where a prompt is drawn.
pub const RAW_LINES: usize = 24;
/// What a menu's footer row says: Claude Code's "Enter to select · ↑/↓ to navigate · Esc to cancel",
/// "Esc to cancel · Tab to amend", "Enter to confirm · Esc to exit".
const FOOTER_WORDS: &[&str] = &["esc to", "enter to", "tab to", "to navigate"];
/// How long a pressed `seen`, or an Esc, stays spent for its pane. Claude redraws after a digit and herdr's status
/// follows an Esc with some delay (review S2, S4: neither measured); 3 s covers a double tap and a second phone and
/// still lets the owner press again on a prompt that did not take the key.
pub const SPENT_FOR: Duration = Duration::from_secs(3);

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
    /// A picker, not a blocked prompt (`picker`): `Some(true)` when its footer offers a typed search.
    pub picker: Option<bool>,
}

impl Shown {
    /// The keys this prompt takes: a menu's own digits and Esc, no Enter (the highlighted option can move on the
    /// PC without the menu's text changing); a picker its own (`PICKER_KEYS`); a raw screen every key but Backspace
    /// (an MCP form's Accept and Decline are chosen with the arrows).
    pub fn takes(&self, key: &str) -> bool {
        if self.picker.is_some() {
            return PICKER_KEYS.contains(&key);
        }
        if key == "backspace" {
            return false;
        }
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
            "picker": self.picker.is_some(),
            "search": self.picker == Some(true),
        })
    }
}

/// The footer of a picker at the bottom of `rows`, lowercased, its wrapped rows joined; `None` when the bottom is not
/// one (`PICKER_EXITS`).
pub fn picker_footer(rows: &[String]) -> Option<String> {
    let bottom: Vec<&str> = rows
        .iter()
        .map(|row| row.trim())
        .filter(|row| !row.is_empty())
        .collect();
    let from = bottom.len().saturating_sub(FOOTER_ROWS);
    let footer = bottom[from..].join(" ").to_lowercase();
    // The way out closes the footer: on its last row, so an "Esc to cancel" further up is a prompt Claude drew
    // earlier, not one open now.
    let last = bottom.last()?.to_lowercase();
    PICKER_EXITS
        .iter()
        .any(|exit| last.contains(exit) && footer.contains(exit))
        .then_some(footer)
}

/// The picker on `screen`, as the phone is shown it: the rows from its top border (`▔`) down, or the bottom
/// `RAW_LINES` when it has none.
pub fn picker(screen: &str) -> Option<Shown> {
    let rows: Vec<String> = screen
        .lines()
        .map(|row| row.trim_end().to_string())
        .collect();
    let footer = picker_footer(&rows)?;
    let mut bottom: Vec<&str> = rows.iter().map(String::as_str).collect();
    while bottom.last().is_some_and(|row| row.trim().is_empty()) {
        bottom.pop();
    }
    let border = |row: &&str| {
        let row = row.trim();
        row.chars().count() >= 3 && row.chars().all(|c| c == '\u{2594}')
    };
    let top = bottom
        .iter()
        .rposition(border)
        .map_or(bottom.len().saturating_sub(RAW_LINES), |at| at + 1);
    let shown: Vec<&str> = bottom[top..]
        .iter()
        .copied()
        .filter(|row| !row.trim().is_empty())
        .collect();
    Some(Shown {
        menu: None,
        screen: shown.join("\n"),
        seen: hash(&format!("picker\n{footer}")),
        picker: Some(footer.contains("type to search")),
    })
}

/// What each pane was last pressed with (`seen <hash>`, or `esc`), and when: one press per prompt (review S2, S4).
pub struct Spent {
    window: Duration,
    last: Mutex<HashMap<String, (String, Instant)>>,
}

impl Spent {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            last: Mutex::default(),
        }
    }

    /// Takes `what` for `pane`, under the lock so two requests at once cannot both have it; false when the pane was
    /// pressed with the same `what` within the window.
    fn claim(&self, pane: &str, what: &str) -> bool {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((was, at)) = last.get(pane) {
            if was == what && at.elapsed() < self.window {
                return false;
            }
        }
        last.insert(pane.to_string(), (what.to_string(), Instant::now()));
        true
    }

    /// Gives `what` back when its key did not reach herdr, so the owner can press again at once.
    fn release(&self, pane: &str, what: &str) {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        if last.get(pane).is_some_and(|(was, _)| was == what) {
            last.remove(pane);
        }
    }
}

impl Default for Spent {
    fn default() -> Self {
        Self::new(SPENT_FOR)
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

/// A menu's whole prompt as `seen` hashes it: every row from the nearest full rule (`─`) above option 1 (the top of
/// the screen when there is none) to the bottom, with the `❯` highlight stripped and blank rows dropped. The sheet
/// shows `nirvana_running::prompt`'s lines, capped at 6 above option 1 and without dashed rules and "Tip:" rows; a
/// hash of only those let two prompts equal in their last 6 rows (a long script, a longer diff) read as one (review
/// S3).
fn whole_prompt(rows: &[String], menu: &crate::nirvana_running::Prompt) -> String {
    let bare = |row: &str| row.trim().trim_start_matches('\u{276F}').trim().to_string();
    let first = menu
        .options
        .first()
        .map(|(digit, label)| format!("{digit}. {label}"))
        .unwrap_or_default();
    let last = menu
        .options
        .last()
        .map(|(digit, _)| format!("{digit}. "))
        .unwrap_or_default();
    // Option 1 is the nearest row above the last option that reads as it (the walk `nirvana_running::prompt` did).
    let at_last = rows
        .iter()
        .rposition(|row| bare(row).starts_with(&last))
        .unwrap_or(rows.len());
    let one = rows[..at_last]
        .iter()
        .rposition(|row| bare(row).starts_with(&first))
        .unwrap_or(at_last);
    let top = rows[..one]
        .iter()
        .rposition(|row| {
            let row = row.trim();
            row.chars().count() >= 3 && row.chars().all(|c| c == '\u{2500}')
        })
        .unwrap_or(0);
    let mut text = String::new();
    for row in &rows[top..] {
        let row = bare(row);
        if !row.is_empty() {
            text.push_str(&row);
            text.push('\n');
        }
    }
    text
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
        Some(menu) => hash(&format!("menu\n{}", whole_prompt(&rows, menu))),
        None => hash(&format!("raw\n{screen}")),
    };
    Shown {
        menu,
        screen,
        seen,
        picker: None,
    }
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
/// What a second press of a prompt already pressed is told (review S2).
const SPENT: &str = "already answered";

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

    /// `{"blocked": false}`, or the prompt the phone is shown for a blocked agent, or the picker open on an agent
    /// that is not working (`picker`).
    pub(super) fn prompt_now(
        &self,
        found: &super::live::Found,
    ) -> Result<(Value, Option<Shown>), Response> {
        let mut row = spaces::agent_row(&found.agent);
        row["space"] = json!(found.space);
        // Review fix round, alpha 0.72.0: keys are pressed on this, so it is the status detection reads
        // (`program_status::decision_status`), not a program status report.
        let status = crate::terminal::program_status::decision_status(&found.agent).unwrap_or("");
        if status == "working" {
            return Ok((json!({"blocked": false, "agent": row}), None));
        }
        if status != "blocked" {
            let Some(shown) = picker(&self.screen(found.id())?) else {
                return Ok((json!({"blocked": false, "agent": row}), None));
            };
            let mut payload = shown.json();
            payload["blocked"] = json!(false);
            payload["agent"] = row;
            return Ok((payload, Some(shown)));
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
                if crate::terminal::program_status::decision_status(&found.agent) != Some("working") {
                    return Response::json(
                        409,
                        &json!({"error": "the agent is not working: nothing to stop", "prompt": now}),
                    );
                }
            }
        }
        // One press per prompt, one Esc per stop: past every check above, so only a key that would go spends it. A
        // picker's arrows and Backspace only move within it, and go as often as they are tapped.
        let what = match body.seen.as_deref() {
            Some(seen) => format!("seen {seen}"),
            None => "esc".to_string(),
        };
        let moves = now["picker"] == true && NAVIGATION.contains(&key);
        if !moves && !self.spent.claim(found.id(), &what) {
            let why = if body.seen.is_some() {
                SPENT
            } else {
                "Esc already sent: the agent is stopping"
            };
            return error(409, why);
        }
        match self.herdr.call(
            "agent.send_keys",
            json!({"target": found.id(), "keys": [key]}),
        ) {
            Ok(_) => Response::json(200, &json!({"sent": key})),
            Err(why) => {
                if !moves {
                    self.spent.release(found.id(), &what);
                }
                error(502, why)
            }
        }
    }

    /// `POST /api/type` `{"agent", "text", "seen"}`: a search typed into the picker the phone saw, with no Enter, so
    /// the list narrows and the owner picks from it. Only a picker whose footer offers a search takes it.
    pub(super) fn type_search(&self, request: &Request, conn: &mut Conn) -> Response {
        let json_body = request.header("Content-Type").first().is_some_and(|kind| {
            kind.trim()
                .to_ascii_lowercase()
                .starts_with("application/json")
        });
        if !json_body {
            return error(415, "a search is JSON");
        }
        if conn.framing() == Framing::None {
            return error(411, "a search has a length");
        }
        let mut body = Vec::new();
        if let Err(why) = conn.read_body(KEY_MOST, &mut body) {
            return error(why.status(), "the search could not be read");
        }
        let Ok(body) = serde_json::from_slice::<TypeBody>(&body) else {
            return error(400, "a search is {agent, text, seen}");
        };
        let text = search_text(&body.text);
        if text.is_empty() {
            return error(400, "nothing to type");
        }
        let found = match self.found(&body.agent) {
            Ok(found) => found,
            Err(refused) => return refused,
        };
        let (now, shown) = match self.prompt_now(&found) {
            Ok(read) => read,
            Err(refused) => return refused,
        };
        match shown {
            Some(shown) if shown.picker == Some(true) && shown.seen == body.seen => {}
            Some(shown) if shown.picker.is_some() && shown.seen == body.seen => {
                return error(400, "this picker has no search")
            }
            _ => {
                return Response::json(
                    409,
                    &json!({"error": "the picker changed on the PC", "prompt": now}),
                )
            }
        }
        // A paste with no key after it: `pane.send_input` wraps the text in bracketed paste when the pane asked for
        // it. Measured 2026-11-10 on `/resume`: the search took "ses" pasted this way, and ignored the same letters
        // sent as plain bytes (`pane.send_text`) or as keys.
        match self.herdr.call(
            "pane.send_input",
            json!({"pane_id": found.id(), "text": text, "keys": []}),
        ) {
            Ok(_) => Response::json(200, &json!({"typed": text.chars().count()})),
            Err(why) => error(502, why),
        }
    }
}

#[derive(Deserialize)]
struct TypeBody {
    agent: String,
    text: String,
    seen: String,
}

/// A search as typed into a picker: printable characters only, no newline or tab (either would be a key: Enter picks,
/// Tab may move), no control or escape (`send::typed`), at most `SEARCH_MOST`.
pub fn search_text(text: &str) -> String {
    text.chars()
        .filter(|&c| c != '\n' && c != '\t' && super::send::typed(c))
        .take(SEARCH_MOST)
        .collect::<String>()
        .trim()
        .to_string()
}
