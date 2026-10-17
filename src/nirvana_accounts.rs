//! Nirvana fork, feature 11: Accounts. Who is signed in to each agent CLI, and a sign-in that runs the CLI's
//! own flow in a new tab. The window's panel is `client/shell/nirvana_accounts.rs`; the tab runs
//! `herdr account login <cli>`, the helper at the end of this file.
//! (nirvana/plans/2026-10-17-11-accounts.md)
//!
//! The owner, 2026-10-17: "In the bottom left let me manage my login accounts. Create a button so that this herder
//! instance let me login to claude code or antigravity or codex. I currently have a new antigravity account I want
//! to login to."
//!
//! herdr never reads, stores or moves a credential. It reads what each CLI prints about itself (`claude auth
//! status`, `codex login status`) and, for Antigravity, which has no status command, the line its own log writes on
//! every start. Codex's email is only in its `auth.json`, a credential file, so Codex shows its sign-in kind.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

/// The CLIs a panel row signs in to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cli {
    Claude,
    Codex,
    Antigravity,
}

impl Cli {
    pub const ALL: [Cli; 3] = [Cli::Claude, Cli::Codex, Cli::Antigravity];

    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Antigravity => "Antigravity",
        }
    }

    /// The word after `herdr account login` and after the command id.
    pub fn key(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Antigravity => "antigravity",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|cli| cli.key() == key)
    }

    fn program(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Antigravity => "agy",
        }
    }

    /// The usage foot's colour for the same provider, so the panel's dots match the foot's.
    pub fn colour(self) -> (u8, u8, u8) {
        let provider = match self {
            Self::Claude => crate::nirvana_usage::Provider::Claude,
            Self::Codex => crate::nirvana_usage::Provider::Codex,
            Self::Antigravity => crate::nirvana_usage::Provider::Antigravity,
        };
        provider.colour().unwrap_or((0x9A, 0x9A, 0x9A))
    }
}

/// Who is signed in, as the CLI says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signed {
    Reading,
    NotInstalled,
    Out,
    /// The account as shown: an email with its plan, or the kind of sign-in.
    In(String),
}

impl Signed {
    /// The row's button: a signed-in CLI switches, any other signs in.
    pub fn button(&self) -> &'static str {
        match self {
            Self::In(_) => "switch",
            _ => "log in",
        }
    }

    pub fn text(&self) -> String {
        match self {
            Self::Reading => "reading".into(),
            Self::NotInstalled => "not installed".into(),
            Self::Out => "not signed in".into(),
            Self::In(account) => account.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub cli: Cli,
    pub signed: Signed,
}

// ---- reading ----

/// `claude auth status`: JSON with `loggedIn`, `email` and `subscriptionType` (Claude Code 2.1.301).
pub fn claude_signed(json: &str) -> Signed {
    // The object only: `output` adds whatever the CLI wrote to stderr after it.
    let object = match (json.find('{'), json.rfind('}')) {
        (Some(start), Some(end)) if start < end => &json[start..=end],
        _ => json,
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(object) else {
        return Signed::Out;
    };
    if value.get("loggedIn").and_then(serde_json::Value::as_bool) != Some(true) {
        return Signed::Out;
    }
    let text = |key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.is_empty())
    };
    let who = text("email")
        .or_else(|| text("authMethod"))
        .unwrap_or("signed in");
    Signed::In(match text("subscriptionType") {
        Some(plan) => format!("{who} \u{b7} {plan}"),
        None => who.to_owned(),
    })
}

/// `codex login status`: "Logged in using ChatGPT", "Logged in using an API key - sk-...", or "Not logged in"
/// (Codex 0.42.0). The key's text is never shown.
pub fn codex_signed(text: &str) -> Signed {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty());
    match line {
        Some(line) if line.starts_with("Logged in using ChatGPT") => {
            Signed::In("ChatGPT sign-in".into())
        }
        Some(line) if line.starts_with("Logged in using an API key") => {
            Signed::In("API key".into())
        }
        Some(line) if line.starts_with("Logged in") => Signed::In("signed in".into()),
        _ => Signed::Out,
    }
}

/// The line Antigravity CLI 1.4.0 writes to its log on each start that finds a sign-in:
/// `server_oauth.go:229] OAuth: authenticated successfully as <email>`.
const AGY_SIGNED_IN: &str = "OAuth: authenticated successfully as ";

/// The email of the last sign-in in one agy log, if the log has one.
pub fn agy_log_account(log: &str) -> Option<String> {
    log.lines()
        .rev()
        .find_map(|line| line.split_once(AGY_SIGNED_IN))
        .map(|(_, rest)| rest.trim().to_owned())
        .filter(|email| !email.is_empty())
}

/// agy's log folder: `~/.gemini/antigravity-cli/log`, one `cli-<date>_<time>.log` per start.
fn agy_log_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()))?;
    Some(
        PathBuf::from(home)
            .join(".gemini")
            .join("antigravity-cli")
            .join("log"),
    )
}

/// The newest of the last few logs that names a sign-in. The logs are named by their start time, so the name
/// orders them. A start after a logout writes no such line; that log is skipped, so a stale account can show
/// until the next signed-in start (herdr's usage foot starts agy every five minutes).
pub fn agy_account_in(dir: &Path) -> Option<String> {
    let mut logs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("cli-") && name.ends_with(".log"))
        })
        .collect();
    logs.sort();
    logs.iter()
        .rev()
        .take(8)
        .find_map(|path| agy_log_account(&std::fs::read_to_string(path).ok()?))
}

/// Runs a CLI the way a shell finds it: an `.exe` straight, anything else (Codex is an npm `.cmd`) through
/// `cmd /C` on Windows, which `Command` alone cannot start.
fn cli_command(program: &str) -> Command {
    #[cfg(windows)]
    {
        let exe = std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(format!("{program}.exe")))
                .find(|candidate| candidate.is_file())
        });
        match exe {
            Some(exe) => Command::new(exe),
            None => {
                let mut command = Command::new("cmd");
                command.args(["/D", "/C", program]);
                command
            }
        }
    }
    #[cfg(not(windows))]
    Command::new(program)
}

/// What the CLI printed, stdout then stderr: Codex 0.42.0 prints `login status` on stderr.
fn output(cli: Cli, args: &[&str]) -> Option<String> {
    let mut command = cli_command(cli.program());
    command.args(args).stdin(std::process::Stdio::null());
    crate::platform::configure_background_command(&mut command);
    let out = command.output().ok()?;
    Some(format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    ))
}

pub fn read(cli: Cli) -> Signed {
    if !crate::nirvana_usage::on_path(cli.program()) {
        return Signed::NotInstalled;
    }
    match cli {
        Cli::Claude => output(cli, &["auth", "status"]).map_or(Signed::Out, |t| claude_signed(&t)),
        Cli::Codex => output(cli, &["login", "status"]).map_or(Signed::Out, |t| codex_signed(&t)),
        Cli::Antigravity => agy_log_dir()
            .and_then(|dir| agy_account_in(&dir))
            .map_or(Signed::Out, Signed::In),
    }
}

// ---- the worker: reads only while a panel asks ----

/// While the panel is open it asks at every draw; the accounts are read again this often.
pub const READ_EVERY: Duration = Duration::from_secs(10);
/// A panel that has not asked for this long is closed: the worker stops reading.
const ASKED_WITHIN: Duration = Duration::from_secs(3);

static CHANGED: AtomicBool = AtomicBool::new(false);
static ASKED_AT: AtomicU64 = AtomicU64::new(0);
static READ_NOW: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

fn state() -> &'static Mutex<Vec<Account>> {
    static ROWS: OnceLock<Mutex<Vec<Account>>> = OnceLock::new();
    ROWS.get_or_init(|| {
        Mutex::new(
            Cli::ALL
                .into_iter()
                .map(|cli| Account {
                    cli,
                    signed: Signed::Reading,
                })
                .collect(),
        )
    })
}

fn store(cli: Cli, signed: Signed) {
    let mut rows = state().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(row) = rows.iter_mut().find(|row| row.cli == cli) {
        if row.signed != signed {
            row.signed = signed;
            CHANGED.store(true, Ordering::Release);
        }
    }
}

fn start_worker() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = crate::thread_spawn::spawn_named("nirvana-accounts", || {
            let mut last: Option<Instant> = None;
            loop {
                let asked = now_ms().saturating_sub(ASKED_AT.load(Ordering::Acquire))
                    < ASKED_WITHIN.as_millis() as u64;
                let due = last.is_none_or(|last| last.elapsed() >= READ_EVERY)
                    || READ_NOW.swap(false, Ordering::AcqRel);
                if asked && due {
                    for cli in Cli::ALL {
                        store(cli, read(cli));
                    }
                    last = Some(Instant::now());
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        });
    });
}

/// The three rows as last read. Each call says the panel is open; the first starts the reader. Under test the
/// reader never starts (it would run this machine's CLIs), and a test sets the rows.
pub fn accounts() -> Vec<Account> {
    #[cfg(test)]
    if let Some(rows) = TEST_ROWS.with_borrow(Clone::clone) {
        return rows;
    }
    if !cfg!(test) {
        ASKED_AT.store(now_ms(), Ordering::Release);
        start_worker();
    }
    state().lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Read again at once: the panel just opened, or a sign-in tab was just opened.
pub fn read_soon() {
    READ_NOW.store(true, Ordering::Release);
}

/// Whether a row changed since the last call: the window repaints on its next tick.
pub fn take_changed() -> bool {
    CHANGED.swap(false, Ordering::AcqRel)
}

#[cfg(test)]
thread_local! {
    static TEST_ROWS: std::cell::RefCell<Option<Vec<Account>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_accounts_for_test(rows: Option<Vec<Account>>) {
    TEST_ROWS.with_borrow_mut(|slot| *slot = rows);
}

// ---- the sign-in tab ----

/// The argv the server runs in the new tab.
pub fn login_argv(exe: &Path, cli: Cli) -> Vec<String> {
    vec![
        exe.display().to_string(),
        "account".into(),
        "login".into(),
        cli.key().into(),
    ]
}

/// What agy's switch types into its own pane once agy shows the old account. `NIRVANA_ACCOUNT_TYPE` replaces
/// it for a live check (`/help`), because a real `/logout` signs the owner out.
pub fn agy_switch_text() -> String {
    std::env::var("NIRVANA_ACCOUNT_TYPE")
        .ok()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| "/logout".into())
}

/// Whether agy's screen is its main prompt showing `account`: the banner's account line ("you@x (Google AI
/// Pro)") with "Antigravity CLI <version>", and no trust question. A new account's banner names the new
/// email, so a sign-in that has already happened is never logged out again. The email is matched as a whole
/// word: in a wide pane agy draws its logo on the same lines, left of the text (live, 2026-10-17).
pub fn agy_ready_for(screen: &str, account: &str) -> bool {
    screen.contains("Antigravity CLI ")
        && !screen.contains("Do you trust")
        && screen
            .lines()
            .any(|line| line.split_whitespace().any(|word| word == account))
}

/// How long the switch watches agy's screen for the old account before it gives up typing.
const AGY_WATCH: Duration = Duration::from_secs(90);

fn herdr_self(args: &[&str]) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::platform::configure_background_command(&mut command);
    let out = command.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Watches this tab's pane until agy shows `account`, then types the switch text and Enter, once.
fn type_logout_when_ready(account: String) {
    // herdr gives every pane its public id (`integration::env::HERDR_PANE_ID_ENV_VAR`, private there).
    let Ok(pane) = std::env::var("HERDR_PANE_ID") else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("nirvana-agy-switch".into())
        .spawn(move || {
            let started = Instant::now();
            while started.elapsed() < AGY_WATCH {
                std::thread::sleep(Duration::from_millis(500));
                let Some(screen) = herdr_self(&["pane", "read", &pane]) else {
                    continue;
                };
                if agy_ready_for(&screen, &account) {
                    // The prompt draws a moment after the banner; text typed before it is lost.
                    std::thread::sleep(Duration::from_millis(1500));
                    let _ = herdr_self(&["pane", "send-text", &pane, &agy_switch_text()]);
                    std::thread::sleep(Duration::from_millis(400));
                    let _ = herdr_self(&["pane", "send-keys", &pane, "enter"]);
                    return;
                }
            }
        });
}

fn wait_enter() -> String {
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_owned()
}

/// Runs the CLI with this tab's terminal, so its own sign-in screens and prompts are the owner's.
fn run_here(cli: Cli, args: &[&str]) -> std::io::Result<()> {
    let status = cli_command(cli.program()).args(args).status()?;
    if !status.success() {
        println!("\n{} exited with {status}.", cli.program());
    }
    Ok(())
}

/// The sentences the tab prints first: what will happen, and what it does to agents already running.
pub fn intro(cli: Cli, signed: &Signed) -> String {
    let now = match signed {
        Signed::In(account) => format!("Signed in now: {account}.\n"),
        _ => String::new(),
    };
    let how = match cli {
        Cli::Claude => "Your browser opens; sign in there with the account you want. If it does not open, \
                        use the link below and paste the code here.\nClaudes already running keep their \
                        account until they restart (the tab menu's Restart Claude)."
            .to_owned(),
        Cli::Codex => "Your browser opens; sign in there with the account you want.\nCodex sessions already \
                       running keep their account until they restart."
            .to_owned(),
        Cli::Antigravity => match signed {
            Signed::In(_) => "Antigravity has no sign-in command: herdr starts agy and types /logout when it \
                              shows this account. agy then asks you to sign in; pick the new account in the \
                              browser.\nOther agy sessions are signed out too and ask again on their next \
                              message."
                .to_owned(),
            _ => "agy starts and asks you to sign in; pick the account in the browser.".to_owned(),
        },
    };
    format!("herdr: signing in to {}.\n{now}{how}\n", cli.name())
}

/// `herdr account login <claude|codex|antigravity>`, the tab the panel opens; `herdr account status` prints the
/// three rows.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    match args.first().map(String::as_str) {
        Some("status") => {
            for cli in Cli::ALL {
                println!("{:<12} {}", cli.name(), read(cli).text());
            }
            Ok(())
        }
        Some("login") => {
            let Some(cli) = args.get(1).and_then(|key| Cli::from_key(key)) else {
                eprintln!("usage: herdr account login claude|codex|antigravity");
                std::process::exit(2);
            };
            login(cli)
        }
        _ => {
            eprintln!(
                "usage: herdr account status\n       herdr account login claude|codex|antigravity"
            );
            std::process::exit(2);
        }
    }
}

fn login(cli: Cli) -> std::io::Result<()> {
    let before = read(cli);
    println!("{}", intro(cli, &before));
    if before == Signed::NotInstalled {
        println!("{} is not on PATH.", cli.program());
    } else {
        match cli {
            Cli::Claude => run_here(cli, &["auth", "login"])?,
            Cli::Codex => run_here(cli, &["login"])?,
            Cli::Antigravity => {
                if let Signed::In(account) = &before {
                    type_logout_when_ready(account.clone());
                }
                loop {
                    run_here(cli, &[])?;
                    println!(
                        "\nagy closed. Press Enter to start it again (to sign in), or type q and Enter to finish."
                    );
                    if wait_enter().eq_ignore_ascii_case("q") {
                        break;
                    }
                }
            }
        }
        println!("\nNow: {}", read(cli).text());
    }
    println!("Press Enter to close this tab.");
    wait_enter();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_shows_the_email_and_plan_from_its_status() {
        // Claude Code 2.1.301's `claude auth status`, 2026-10-17 (the email and ids replaced).
        let json = r#"{"loggedIn": true, "authMethod": "claude.ai", "apiProvider": "firstParty",
            "email": "me@example.com", "orgId": "x", "subscriptionType": "max"}"#;
        assert_eq!(
            claude_signed(json),
            Signed::In("me@example.com \u{b7} max".into())
        );
        assert_eq!(
            claude_signed(&format!("{json}\nsome warning on stderr\n")),
            Signed::In("me@example.com \u{b7} max".into())
        );
        assert_eq!(claude_signed(r#"{"loggedIn": false}"#), Signed::Out);
        assert_eq!(claude_signed("not json"), Signed::Out);
        assert_eq!(
            claude_signed(r#"{"loggedIn": true, "authMethod": "console"}"#),
            Signed::In("console".into())
        );
    }

    #[test]
    fn codex_shows_the_kind_of_sign_in_and_never_the_key() {
        assert_eq!(
            codex_signed("Logged in using ChatGPT\n"),
            Signed::In("ChatGPT sign-in".into())
        );
        let key = codex_signed("Logged in using an API key - sk-proj-***abcd\n");
        assert_eq!(key, Signed::In("API key".into()));
        assert!(!key.text().contains("sk-"));
        assert_eq!(codex_signed("Not logged in\n"), Signed::Out);
        assert_eq!(codex_signed(""), Signed::Out);
    }

    #[test]
    fn antigravity_is_read_from_the_newest_log_that_names_a_sign_in() {
        let dir = std::env::temp_dir().join(format!("nirvana-agy-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let line = |email: &str| {
            format!("I1017 12:00:12.211749       1 server_oauth.go:229] {AGY_SIGNED_IN}{email}\n")
        };
        std::fs::write(dir.join("cli-20261017_110000.log"), line("old@example.com")).unwrap();
        std::fs::write(dir.join("cli-20261017_120000.log"), line("new@example.com")).unwrap();
        // A start that found no sign-in (after a logout) names nobody and is skipped.
        std::fs::write(dir.join("cli-20261017_130000.log"), "I1017 nothing here\n").unwrap();
        std::fs::write(dir.join("other.log"), line("not-a-cli-log@example.com")).unwrap();
        assert_eq!(agy_account_in(&dir).as_deref(), Some("new@example.com"));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(agy_account_in(&dir), None);
    }

    #[test]
    fn agy_is_logged_out_only_while_it_shows_the_old_account() {
        // agy 1.4.0's screen, read from a herdr pane on 2026-10-17 (the email replaced).
        let main = "  Antigravity CLI 1.4.0\n  old@example.com (Google AI Pro)\n  Gemini 4 Argon (Medium)\n>\n";
        assert!(agy_ready_for(main, "old@example.com"));
        assert!(
            !agy_ready_for(main, "new@example.com"),
            "a new account is kept"
        );
        // A wide pane: the logo shares the banner's lines (herdr pane, 120 columns, 2026-10-17).
        let wide = "      \u{2584}\u{2580}\u{2580}\u{2584}        Antigravity CLI 1.4.0\n     \u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}       old@example.com (Google AI Pro)\n>\n";
        assert!(agy_ready_for(wide, "old@example.com"));
        assert!(
            !agy_ready_for(wide, "d@example.com"),
            "a word, not a part of one"
        );
        let trust = "Accessing workspace:\n Do you trust the contents of this project?\n Antigravity CLI 1.4.0\n old@example.com (Google AI Pro)\n";
        assert!(!agy_ready_for(trust, "old@example.com"));
        assert!(!agy_ready_for("PS C:\\> agy\n", "old@example.com"));
    }

    #[test]
    fn the_tab_runs_this_binary_with_the_cli_named() {
        let argv = login_argv(Path::new("C:/herdr/herdr.exe"), Cli::Antigravity);
        assert_eq!(argv[1..], ["account", "login", "antigravity"]);
        for cli in Cli::ALL {
            assert_eq!(Cli::from_key(cli.key()), Some(cli));
        }
        assert_eq!(Cli::from_key("gemini"), None);
    }

    #[test]
    fn the_button_switches_a_signed_in_cli_and_logs_in_any_other() {
        assert_eq!(Signed::In("a".into()).button(), "switch");
        assert_eq!(Signed::Out.button(), "log in");
        assert_eq!(Signed::NotInstalled.button(), "log in");
        let text = intro(Cli::Antigravity, &Signed::In("old@example.com".into()));
        assert!(text.contains("/logout") && text.contains("old@example.com"));
        assert!(!intro(Cli::Antigravity, &Signed::Out).contains("/logout"));
    }
}
