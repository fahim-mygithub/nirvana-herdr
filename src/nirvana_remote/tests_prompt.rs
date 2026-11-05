//! Plan 39 P39c's tests: a blocked agent's prompt as buttons, the raw fallback, a key only for the prompt the phone
//! saw, the key table, and Esc. Against a fake herdr whose screen and states the test sets, and the P39b wire helpers
//! (`tests_live`): the identity is the fake header tailscaled would set, the server binds 127.0.0.1 on a free port.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::herdr::Herdr;
use super::prompt;
use super::routes::{self, Site};
use super::tests_live::{
    fixed_now, from_page, get, post, proxied, serve, Admit, NoProbe, NoTailscale, DESK_IP, HOST,
    OWNER, REFUSED,
};

/// Claude Code's Bash permission dialog as drawn (the Running square's measurement case B,
/// `nirvana_running.rs`'s `a_permission_dialog_reads_with_its_command`).
const PERMISSION: &str = "\u{25CF} earlier output\n─────────────────────────────\n Bash command\n Tip: auto mode handles these prompts for you\n Write marker text to wait-check.txt and read it back\n\u{254C}\u{254C}\u{254C}\u{254C}\u{254C}\n echo nirvana-wait-check > wait-check.txt && cat wait-check.txt\n\u{254C}\u{254C}\u{254C}\u{254C}\u{254C}\n Do you want to proceed?\n \u{276F} 1. Yes\n   2. Yes, and always allow access to C:\\x from this project\n   3. Yes, and switch to auto mode\n   4. No\n Esc to cancel \u{b7} Tab to amend\n\n";

/// AskUserQuestion as drawn (case A).
const QUESTION: &str = "──────────────────────────────────────\n \u{2610} Button colour\nWhich colour should the button be?\n\u{276F} 1. Red (Recommended)\n     Red button colour.\n  2. Blue\n     Blue button colour.\n  3. Type something.\n───────────────────────────────────────\n  4. Chat about this\nEnter to select \u{b7} \u{2191}/\u{2193} to navigate \u{b7} Esc to cancel";

/// The same dialog asking about another command: what the PC shows after it answered the first and Claude asked
/// again.
const PERMISSION_NEXT: &str = "\u{25CF} earlier output\n─────────────────────────────\n Bash command\n git push origin main\n Do you want to proceed?\n \u{276F} 1. Yes\n   2. Yes, and don't ask again for: git push\n   3. No\n Esc to cancel \u{b7} Tab to amend\n";

/// An MCP form (claude.toml `mcp_elicitation_prompt`): blocked, with no numbers, under a numbered list Claude wrote.
const MCP_FORM: &str = "Here is the plan:\n  1. Read the file\n  2. Write the patch\n\nMCP server \"bladebro\" requests your input\n\u{276F} Accept\n  Decline\nEsc to cancel";

struct PromptHerdr {
    agents: Mutex<Vec<Value>>,
    screen: Mutex<String>,
    calls: Mutex<Vec<(String, Value)>>,
    /// `agent.send_keys` fails while set: herdr gone between the read and the key.
    keys_fail: AtomicBool,
}

impl PromptHerdr {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            agents: Mutex::new(vec![
                agent("p1", "nirvana", "working", "claude"),
                agent("p3", "printer", "blocked", "claude"),
                agent("p5", "codex", "blocked", "codex"),
                agent("p6", "flash", "idle", "claude"),
            ]),
            screen: Mutex::new(PERMISSION.into()),
            calls: Mutex::default(),
            keys_fail: AtomicBool::new(false),
        })
    }

    fn calls(&self, method: &str) -> Vec<Value> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(name, _)| name == method)
            .map(|(_, params)| params.clone())
            .collect()
    }

    fn set_status(&self, pane: &str, status: &str) {
        for agent in self.agents.lock().unwrap().iter_mut() {
            if agent["pane_id"] == pane {
                agent["agent_status"] = json!(status);
            }
        }
    }

    fn set_screen(&self, screen: &str) {
        *self.screen.lock().unwrap() = screen.into();
    }
}

fn agent(pane: &str, name: &str, status: &str, kind: &str) -> Value {
    json!({
        "pane_id": pane, "workspace_id": "w2", "name": name, "agent": kind,
        "title": "\u{2733} Printing the dial", "agent_status": status,
        "tokens": {"nirvana_kind": kind},
        "cwd": "C:\\Users\\owner\\Desktop\\Pojects\\3D-Printer Flow",
    })
}

impl Herdr for PromptHerdr {
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        self.calls
            .lock()
            .unwrap()
            .push((method.to_string(), params.clone()));
        match method {
            "workspace.list" => Ok(json!({"workspaces": [
                {"workspace_id": "w2", "label": "3D-Printer Flow", "number": 1},
            ]})),
            "agent.list" => Ok(json!({"agents": *self.agents.lock().unwrap()})),
            "agent.read" => Ok(json!({"read": {"text": *self.screen.lock().unwrap()}})),
            "agent.send_keys" if self.keys_fail.load(Ordering::SeqCst) => {
                Err("herdr is not answering".into())
            }
            "agent.send_keys" => Ok(json!({"type": "ok"})),
            other => Err(format!("unexpected {other}")),
        }
    }
}

fn site(herdr: Arc<PromptHerdr>) -> Arc<Site> {
    site_spent(herdr, prompt::SPENT_FOR)
}

/// A site whose pressed prompts stay spent for `window` (zero: a second press goes at once).
fn site_spent(herdr: Arc<PromptHerdr>, window: Duration) -> Arc<Site> {
    let mut site = Site::new(
        Arc::new(NoTailscale),
        herdr,
        Arc::new(Admit),
        Arc::new(NoProbe),
        super::HTTPS_PORT,
        fixed_now,
    );
    site.spent = prompt::Spent::new(window);
    *site.owner.write().unwrap() = Some(OWNER.into());
    *site.me.write().unwrap() = Some(routes::Me {
        host: HOST.into(),
        ips: vec![DESK_IP.parse().unwrap()],
    });
    Arc::new(site)
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

fn press(addr: std::net::SocketAddr, body: Value) -> (u16, Value) {
    let (status, _, text) = post(
        addr,
        "/api/key",
        "application/json",
        body.to_string().as_bytes(),
        OWNER,
    );
    (status, json_of(&text))
}

fn prompt_of(addr: std::net::SocketAddr, pane: &str) -> Value {
    let (status, _, text) = get(addr, &format!("/api/prompt?agent={pane}"), OWNER);
    assert_eq!(status, 200, "{text}");
    json_of(&text)
}

// --- Reading the menu ---------------------------------------------------------------------------------------------

#[test]
fn a_claude_menu_reads_as_its_heading_and_numbered_buttons() {
    let claude = agent("p3", "printer", "blocked", "claude");
    let shown = prompt::shown(&claude, PERMISSION);
    let menu = shown
        .menu
        .clone()
        .expect("Claude's permission dialog is a menu");
    assert_eq!(
        menu.lines,
        vec![
            "Bash command",
            "Write marker text to wait-check.txt and read it back",
            "echo nirvana-wait-check > wait-check.txt && cat wait-check.txt",
            "Do you want to proceed?",
        ]
    );
    assert_eq!(menu.options.len(), 4);
    assert_eq!(menu.options[0], (1, "Yes".to_string()));
    assert_eq!(menu.options[3], (4, "No".to_string()));
    // A menu takes its own digits and Esc: no Enter, no digit it does not offer.
    assert_eq!(shown.keys(), vec!["1", "2", "3", "4", "esc"]);
    assert_eq!(shown.seen.len(), 16);
    assert_eq!(
        prompt::shown(&claude, PERMISSION).seen,
        shown.seen,
        "the same screen, the same seen"
    );

    let question = prompt::shown(&claude, QUESTION);
    let menu = question.menu.expect("AskUserQuestion is a menu");
    assert_eq!(menu.options.len(), 4);
    assert_eq!(menu.options[3], (4, "Chat about this".to_string()));
    assert_ne!(question.seen, shown.seen);
}

/// A Bash permission dialog for a script of `lines`, drawn as `PERMISSION` is.
fn script_prompt(lines: &[&str]) -> String {
    let script: String = lines.iter().map(|line| format!(" {line}\n")).collect();
    format!(
        "\u{25CF} earlier output\n─────────────────────────────\n Bash command\n{script} Do you want to proceed?\n \u{276F} 1. Yes\n   2. No\n Esc to cancel \u{b7} Tab to amend\n"
    )
}

#[test]
fn menus_equal_in_their_last_rows_hash_apart_by_what_is_above() {
    let claude = agent("p3", "printer", "blocked", "claude");
    // Two scripts that differ only above the 6 rows the sheet shows (review S3).
    let tail = ["step 3", "step 4", "step 5", "step 6", "step 7"];
    let first = script_prompt(&[&["rm -rf build", "step 2"][..], &tail[..]].concat());
    let second = script_prompt(&[&["git push --force", "step 2"][..], &tail[..]].concat());
    let (one, two) = (
        prompt::shown(&claude, &first),
        prompt::shown(&claude, &second),
    );
    let (menu_one, menu_two) = (one.menu.clone().unwrap(), two.menu.clone().unwrap());
    assert_eq!(menu_one, menu_two, "the sheet shows the same capped lines");
    assert_eq!(menu_one.lines.len(), 6);
    assert!(!menu_one.lines.iter().any(|line| line.contains("rm -rf")));
    assert_ne!(one.seen, two.seen, "the whole prompt is hashed");
    // The highlight moving on the PC is not a new prompt (Enter is withheld from a menu for that reason).
    let moved = first
        .replace(" \u{276F} 1. Yes", "   1. Yes")
        .replace("   2. No", " \u{276F} 2. No");
    assert_eq!(prompt::shown(&claude, &moved).seen, one.seen);
    // What is above the dialog's rule is not the prompt.
    let earlier = first.replace("earlier output", "other output");
    assert_eq!(prompt::shown(&claude, &earlier).seen, one.seen);
}

#[test]
fn a_screen_that_is_not_a_claude_menu_is_shown_raw_with_every_key() {
    let claude = agent("p3", "printer", "blocked", "claude");
    // No numbers on the form: the list Claude wrote above it is not its menu.
    let form = prompt::shown(&claude, MCP_FORM);
    assert_eq!(form.menu, None);
    assert!(form.screen.contains("requests your input"));
    assert_eq!(
        form.keys(),
        vec!["1", "2", "3", "4", "5", "6", "7", "8", "9", "enter", "esc"]
    );
    // A numbered menu with no Esc footer under it is not read either.
    let no_footer = PERMISSION.replace(" Esc to cancel \u{b7} Tab to amend\n", "");
    assert_eq!(prompt::shown(&claude, &no_footer).menu, None);
    // Another agent's numbered menu: not measured, so raw (only Claude Code's digit-picks were).
    let codex = agent("p5", "codex", "blocked", "codex");
    let raw = prompt::shown(&codex, PERMISSION);
    assert_eq!(raw.menu, None);
    assert!(raw.screen.ends_with("Esc to cancel \u{b7} Tab to amend"));
    assert_ne!(raw.seen, prompt::shown(&claude, PERMISSION).seen);
    // The raw screen is the bottom of the pane.
    let tall = format!("{}{PERMISSION}", "line\n".repeat(80));
    assert_eq!(
        prompt::shown(&codex, &tall).screen.lines().count(),
        prompt::RAW_LINES
    );
}

#[test]
fn api_prompt_answers_a_blocked_agents_menu_read_off_its_screen() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let data = prompt_of(running.addr, "p3");
    assert_eq!(data["blocked"], true);
    assert_eq!(data["agent"]["status"], "needs");
    assert_eq!(data["agent"]["space"], "3D-Printer Flow");
    assert_eq!(data["menu"]["options"][1]["key"], "2");
    assert_eq!(
        data["menu"]["options"][1]["label"],
        "Yes, and always allow access to C:\\x from this project"
    );
    assert_eq!(data["screen"], "");
    assert_eq!(data["keys"], json!(["1", "2", "3", "4", "esc"]));
    // The visible rows, where the prompt is drawn.
    let reads = herdr.calls("agent.read");
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0]["source"], "visible");
    assert_eq!(reads[0]["target"], "p3");

    // Codex: the raw screen.
    let data = prompt_of(running.addr, "p5");
    assert_eq!(data["menu"], Value::Null);
    assert!(data["screen"]
        .as_str()
        .unwrap()
        .contains("Do you want to proceed?"));
    // Not blocked: no prompt, and the screen is not read.
    let data = prompt_of(running.addr, "p1");
    assert_eq!(data["blocked"], false);
    assert_eq!(herdr.calls("agent.read").len(), 2);
}

// --- Pressing ------------------------------------------------------------------------------------------------------

#[test]
fn a_tap_sends_that_options_key_to_the_prompt_the_phone_saw() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let seen = prompt_of(running.addr, "p3")["seen"].clone();
    let (status, answer) = press(
        running.addr,
        json!({"agent": "p3", "key": "2", "seen": seen}),
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        herdr.calls("agent.send_keys"),
        vec![json!({"target": "p3", "keys": ["2"]})]
    );
    // The raw fallback's Enter.
    let seen = prompt_of(running.addr, "p5")["seen"].clone();
    let (status, _) = press(
        running.addr,
        json!({"agent": "p5", "key": "enter", "seen": seen}),
    );
    assert_eq!(status, 200);
    assert_eq!(
        herdr.calls("agent.send_keys")[1],
        json!({"target": "p5", "keys": ["enter"]})
    );
}

#[test]
fn the_same_seen_twice_presses_once() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let seen = prompt_of(running.addr, "p3")["seen"].clone();
    let tap = json!({"agent": "p3", "key": "1", "seen": seen});
    let (status, answer) = press(running.addr, tap.clone());
    assert_eq!(status, 200, "{answer}");
    // A double tap, before Claude redraws: the same screen, the same seen, no second key (review S2).
    let (status, answer) = press(running.addr, tap.clone());
    assert_eq!(status, 409, "{answer}");
    assert_eq!(answer["error"], "already answered");
    // Esc on the same sheet is a second press of the same prompt too.
    let (status, _) = press(
        running.addr,
        json!({"agent": "p3", "key": "esc", "seen": seen}),
    );
    assert_eq!(status, 409);
    assert_eq!(herdr.calls("agent.send_keys").len(), 1);
    // Two phones at once: one key between them.
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let seen = prompt_of(running.addr, "p3")["seen"].clone();
    let addr = running.addr;
    let taps: Vec<_> = (0..4)
        .map(|_| {
            let tap = json!({"agent": "p3", "key": "1", "seen": seen});
            std::thread::spawn(move || press(addr, tap).0)
        })
        .collect();
    let mut statuses: Vec<u16> = taps.into_iter().map(|tap| tap.join().unwrap()).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, vec![200, 409, 409, 409]);
    assert_eq!(herdr.calls("agent.send_keys").len(), 1);
    // The next prompt is not spent by the last one.
    herdr.set_screen(PERMISSION_NEXT);
    let seen = prompt_of(running.addr, "p3")["seen"].clone();
    let (status, _) = press(
        running.addr,
        json!({"agent": "p3", "key": "1", "seen": seen}),
    );
    assert_eq!(status, 200);
    // A key that did not reach herdr spends nothing: the owner can press again at once.
    herdr.set_screen(PERMISSION);
    let tap = json!({"agent": "p3", "key": "1", "seen": prompt_of(running.addr, "p3")["seen"]});
    herdr.keys_fail.store(true, Ordering::SeqCst);
    assert_eq!(press(running.addr, tap.clone()).0, 502);
    herdr.keys_fail.store(false, Ordering::SeqCst);
    assert_eq!(press(running.addr, tap).0, 200);
    // Past the window the same seen goes again (a prompt asked again word for word).
    let herdr = PromptHerdr::new();
    let running = serve(site_spent(herdr.clone(), Duration::ZERO));
    let tap = json!({"agent": "p3", "key": "1", "seen": prompt_of(running.addr, "p3")["seen"]});
    assert_eq!(press(running.addr, tap.clone()).0, 200);
    assert_eq!(press(running.addr, tap).0, 200);
    assert_eq!(herdr.calls("agent.send_keys").len(), 2);
}

#[test]
fn a_second_esc_within_the_window_is_refused() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let stop = json!({"agent": "p1", "key": "esc", "seen": null});
    assert_eq!(press(running.addr, stop.clone()).0, 200);
    // herdr still says working, but Esc went: a second one could reach an idle Claude and clear the PC's input
    // (review S4).
    let (status, answer) = press(running.addr, stop.clone());
    assert_eq!(status, 409, "{answer}");
    assert_eq!(answer["error"], "Esc already sent: the agent is stopping");
    assert_eq!(herdr.calls("agent.send_keys").len(), 1);
    // Per pane: another working agent can still be stopped.
    herdr.set_status("p6", "working");
    let other = json!({"agent": "p6", "key": "esc", "seen": null});
    assert_eq!(press(running.addr, other).0, 200);
    // Past the window, Esc goes again.
    let herdr = PromptHerdr::new();
    let running = serve(site_spent(herdr.clone(), Duration::ZERO));
    assert_eq!(press(running.addr, stop.clone()).0, 200);
    assert_eq!(press(running.addr, stop).0, 200);
    assert_eq!(herdr.calls("agent.send_keys").len(), 2);
}

#[test]
fn a_tap_on_a_prompt_that_changed_is_refused_with_the_new_one() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let seen = prompt_of(running.addr, "p3")["seen"].clone();
    // The PC answered and Claude asked again: another menu under the phone's finger.
    herdr.set_screen(PERMISSION_NEXT);
    let (status, answer) = press(
        running.addr,
        json!({"agent": "p3", "key": "1", "seen": seen}),
    );
    assert_eq!(status, 409);
    assert_eq!(answer["error"], "the prompt changed on the PC");
    assert_eq!(answer["prompt"]["blocked"], true);
    assert_eq!(
        answer["prompt"]["menu"]["options"][1]["label"],
        "Yes, and don't ask again for: git push"
    );
    assert!(herdr.calls("agent.send_keys").is_empty(), "nothing pressed");
    // With the new menu's seen, the key goes.
    let (status, _) = press(
        running.addr,
        json!({"agent": "p3", "key": "1", "seen": answer["prompt"]["seen"]}),
    );
    assert_eq!(status, 200);
    // A raw screen that moved is refused the same way.
    let seen = prompt_of(running.addr, "p5")["seen"].clone();
    herdr.set_screen(QUESTION);
    let (status, _) = press(
        running.addr,
        json!({"agent": "p5", "key": "1", "seen": seen}),
    );
    assert_eq!(status, 409);
    assert_eq!(herdr.calls("agent.send_keys").len(), 1);
}

#[test]
fn a_tap_after_the_pc_answered_presses_nothing() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let seen = prompt_of(running.addr, "p3")["seen"].clone();
    herdr.set_status("p3", "working");
    let (status, answer) = press(
        running.addr,
        json!({"agent": "p3", "key": "1", "seen": seen}),
    );
    assert_eq!(status, 409);
    assert_eq!(answer["error"], "answered on the PC");
    assert_eq!(answer["prompt"]["blocked"], false);
    // Not even Esc: the phone's sheet was for a prompt that is gone.
    let (status, _) = press(
        running.addr,
        json!({"agent": "p3", "key": "esc", "seen": seen}),
    );
    assert_eq!(status, 409);
    assert!(herdr.calls("agent.send_keys").is_empty());
}

#[test]
fn only_keys_from_the_table_reach_send_keys() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let seen = prompt_of(running.addr, "p5")["seen"].clone();
    let before = herdr.calls.lock().unwrap().len();
    for key in [
        "ctrl+c",
        "C-c",
        "a",
        "10",
        "0",
        "1\r",
        "enter enter",
        "ESC",
        "esc ",
        "\u{1b}",
        "",
        "tab",
    ] {
        let (status, answer) = press(
            running.addr,
            json!({"agent": "p5", "key": key, "seen": seen}),
        );
        assert_eq!(status, 400, "{key:?}: {answer}");
        assert_eq!(answer["error"], "not a key the phone may press", "{key:?}");
    }
    // A key that is not one is refused before herdr is asked anything.
    assert_eq!(herdr.calls.lock().unwrap().len(), before);
    // A key in the table but not this menu's: no Enter, no digit it does not offer.
    let seen = prompt_of(running.addr, "p3")["seen"].clone();
    for key in ["enter", "7"] {
        let (status, answer) = press(
            running.addr,
            json!({"agent": "p3", "key": key, "seen": seen}),
        );
        assert_eq!(status, 400, "{key}: {answer}");
        assert_eq!(answer["error"], "not one of this prompt's keys");
    }
    let (status, _, _) = post(
        running.addr,
        "/api/key",
        "text/plain",
        json!({"agent": "p3", "key": "1", "seen": seen})
            .to_string()
            .as_bytes(),
        OWNER,
    );
    assert_eq!(status, 415, "a key is JSON");
    assert!(herdr.calls("agent.send_keys").is_empty());
    // Every name the table sends is one herdr's key parser takes.
    for (_, sent) in prompt::KEYS {
        assert!(
            crate::config::parse_key_combo(sent).is_some(),
            "{sent} is a herdr key"
        );
    }
}

#[test]
fn esc_stops_a_working_agent_and_goes_nowhere_else() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let (status, answer) = press(
        running.addr,
        json!({"agent": "p1", "key": "esc", "seen": null}),
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        herdr.calls("agent.send_keys"),
        vec![json!({"target": "p1", "keys": ["esc"]})]
    );
    // An idle agent has nothing to stop.
    let (status, answer) = press(
        running.addr,
        json!({"agent": "p6", "key": "esc", "seen": null}),
    );
    assert_eq!(status, 409);
    assert_eq!(answer["error"], "the agent is not working: nothing to stop");
    // A digit to a working agent is not a stop.
    let (status, _) = press(
        running.addr,
        json!({"agent": "p1", "key": "1", "seen": null}),
    );
    assert_eq!(status, 409);
    // The phone saw it working and it is asking now: no key, and the page gets the prompt.
    let (status, answer) = press(
        running.addr,
        json!({"agent": "p3", "key": "esc", "seen": null}),
    );
    assert_eq!(status, 409);
    assert_eq!(answer["prompt"]["blocked"], true);
    assert_eq!(herdr.calls("agent.send_keys").len(), 1);
}

#[test]
fn a_refused_caller_gets_nothing_from_the_prompt_routes() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr.clone()));
    let body = json!({"agent": "p1", "key": "esc", "seen": null}).to_string();
    let key_from = |login: &str, from: &str, page: &str| {
        let head = proxied("POST", "/api/key", login).replace(super::tests_live::PHONE, from);
        let request = format!(
            "{head}{page}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        super::tests_live::raw(running.addr, request.as_bytes())
    };
    let page = from_page();
    for (login, from, headers, status) in [
        (
            "someone@else.com",
            super::tests_live::PHONE,
            page.as_str(),
            403,
        ),
        ("", super::tests_live::PHONE, page.as_str(), 403),
        // The owner's login from this PC's own Tailscale address: a local process, not the phone.
        (OWNER, DESK_IP, page.as_str(), 403),
        // A cross-site page on the phone, and a POST with neither header.
        (
            OWNER,
            super::tests_live::PHONE,
            "Sec-Fetch-Site: cross-site\r\n",
            403,
        ),
        (OWNER, super::tests_live::PHONE, "", 403),
    ] {
        let (got, _, text) = key_from(login, from, headers);
        assert_eq!(got, status, "{login} from {from} with {headers:?}");
        assert_eq!(text, REFUSED);
    }
    let (got, _, text) = get(running.addr, "/api/prompt?agent=p3", "someone@else.com");
    assert_eq!((got, text.as_str()), (403, REFUSED));
    // A cross-site read of the prompt.
    let request = format!(
        "{}Sec-Fetch-Site: cross-site\r\n\r\n",
        proxied("GET", "/api/prompt?agent=p3", OWNER)
    );
    let (got, _, _) = super::tests_live::raw(running.addr, request.as_bytes());
    assert_eq!(got, 403);
    // Another Host: a rebinding to the tailnet address.
    let request = format!(
        "{}\r\n",
        proxied("GET", "/api/prompt?agent=p3", OWNER).replace(HOST, "evil.example:8443")
    );
    let (got, _, _) = super::tests_live::raw(running.addr, request.as_bytes());
    assert_eq!(got, 421);
    assert!(
        herdr.calls.lock().unwrap().is_empty(),
        "herdr is asked nothing for a refused caller"
    );
}

// --- The page -------------------------------------------------------------------------------------------------------

#[test]
fn the_page_has_the_prompt_sheet_and_the_esc_button() {
    let herdr = PromptHerdr::new();
    let running = serve(site(herdr));
    let (status, _, index) = get(running.addr, "/", OWNER);
    assert_eq!(status, 200);
    assert!(index.contains("<script src=\"/prompt.js\"></script>"));
    let (status, head, js) = get(running.addr, "/prompt.js", OWNER);
    assert_eq!(status, 200);
    assert!(head.contains("text/javascript"));
    // The sheet opens on the way into "needs you", and a tap carries what the sheet showed.
    assert!(js.contains("if (now === 'needs' && was !== 'needs') { pressed = false; read(); }"));
    assert!(js.contains("onclick: () => press(o.key, data.seen, note, true)"));
    // One press per prompt: the sheet's keys are off until the answer (review S2); after a menu's digit the sheet
    // reads again and redraws (review N1); Esc rests 3 s after it went (review S4).
    assert!(js.contains("      hold(true);\n"));
    assert!(js.contains("if (menu && key !== 'esc') after(key, seen); else sheetClose();"));
    assert!(js.contains("if (data.seen !== seen) { show(data,"));
    assert!(js.contains("      esc.disabled = true;\n"));
    assert!(js.contains("setTimeout(() => { esc.disabled = false; }, ESC_REST)"));
    // A prompt that changed is shown again, one that went away says so.
    assert!(js.contains("show(now, 'The prompt changed on the PC: this is the one there now.')"));
    assert!(js.contains("say('Answered on the PC.')"));
    // The raw screen's keypad is the server's list of keys.
    assert!(js.contains("...data.keys.map((k) =>"));
    // Esc is the PC's key, sent with no prompt seen.
    assert!(js.contains("{ agent: id, key: 'esc', seen: null }"));
    assert!(!js.contains("innerHTML"));
    let (_, _, chat) = get(running.addr, "/chat.js", OWNER);
    assert!(chat.contains("prompts.answer, prompts.esc, sendButton"));
    assert!(chat.contains("prompts.status(agent.status);"));
    assert!(chat.contains("return () => { prompts.close(); ended(); };"));
    let (_, _, css) = get(running.addr, "/app.css", OWNER);
    assert!(css.contains(".esc-button {"));
}
