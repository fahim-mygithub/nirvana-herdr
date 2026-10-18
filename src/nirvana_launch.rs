//! Nirvana fork, plan 12 O4a: Nirvana Claude's launch, the sidebar's "new" menu, and the window the Explorer verb
//! "Open Nirvana" opens.
//!
//! The owner, 2026-10-17 (plan 12 L5): "Claude will be the orchistrator model alwys when you right click on a folder
//! or start a Nirvana herder session you will create a Nirvana version of Claude with all the tool / skills ready to
//! work task in the manner we are describing." And L6: "The reason we are building this as a seperate version which
//! tbh is proably just Claude Code with these Orchistrator level skills and memories and hooks is to differentiate
//! between the two." And L7: "Sometimes I want a Orchistrator style model that tries to utilize both Claude and
//! ANtigravity and sometimes I just want Claude Code or agy traditional to tackle the project."
//!
//! So Nirvana Claude is Claude Code, unpatched, with a plugin and a launch argv (plan 12 §6):
//!
//! ```text
//! claude --plugin-dir <home>/nirvana/claude --agent nirvana --model claude-opus-5-5 --effort medium
//!        --append-system-prompt-file <home>/nirvana/claude/ROLE.md --name "Nirvana" --settings <home>/nirvana/claude/settings.json
//! ```
//!
//! with `NIRVANA_ORCHESTRATOR=1` in its environment. O4a's `agents/nirvana.md` carried the role as its prompt, and its
//! `/context` showed the cost: an agent's prompt REPLACES Claude Code's own system prompt (the sub-agents page:
//! "replaces the default Claude Code system prompt entirely, the same way `--system-prompt` does"), 1.5k tokens
//! against plain Claude Code's 2.5k. The owner's bar is parity with Claude Code (memory
//! nirvana-parity-with-claude-code), so O4b empties the agent file's body ("an agent with an empty prompt and no
//! memory leaves the session's system prompt unchanged") and appends the role from `ROLE.md`. `/context` on
//! 2026-10-18: the empty agent alone 2.5k, with the role 4k. `--agent nirvana` stays because it is what tells Nirvana
//! Claude from plain Claude Code in a pane's argv (O3's wake guard, `nirvana_run::pane_program`). The model and
//! effort are R1's, Opus 5.5 at medium, in the agent file and as flags. Each flag was checked against `claude
//! --help` of Claude Code 2.1.302; `--append-system-prompt-file` is not in its list but is named under `--bare`, a
//! missing file exits 1, and the live `/context` took it.
//!
//! Plain Claude Code and agy carry none of this: the "new" menu's other two entries type `claude` and `agy` alone,
//! which is what a herdr agent start of those kinds types today.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Set for Nirvana Claude only. The plugin's hooks (O4b) read it; plain Claude never has it.
pub const ORCHESTRATOR_ENV: &str = "NIRVANA_ORCHESTRATOR";
/// Set by `herdr nirvana window` for the client it supervises: this Terminal window is already Nirvana's, with this
/// folder's button and badge, so the client's own menu does not claim it again.
pub const WINDOW_FOLDER_ENV: &str = "NIRVANA_WINDOW_FOLDER";
/// The herdr session the verb opens, as Nirvana-C's verb did, so the fork never meets a server on the default one.
pub const SESSION: &str = "nirvana";
/// `command.invoke` id from the "new" menu: a new space with this kind's agent; the kind's key after it.
pub const NEW_COMMAND: &str = "nirvana.new:";
/// `command.invoke` id from the verb's window: focus Nirvana Claude on a folder, or open one; `fresh:` when this
/// window's own click started the server, then the folder.
pub const OPEN_COMMAND: &str = "nirvana.open:";
pub const OPEN_FRESH: &str = "fresh:";
/// The fork's own toast app id, registered by `install.py` and by the toast code. One id for every toast, so the
/// header always reads "Nirvana" with the mark.
pub const TOAST_APP_ID: &str = "Nirvana.Herdr";
/// The taskbar id's prefix; each folder gets its own (P7: "Two windows with one id combine into one button showing
/// only the last badge").
pub const WINDOW_APP_ID_PREFIX: &str = "Nirvana.Herdr.";
/// What the taskbar button and the toast call the app.
pub const DISPLAY_NAME: &str = "Nirvana";

/// The three agents the sidebar's "new" menu offers (L7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewKind {
    Nirvana,
    ClaudeCode,
    Antigravity,
}

impl NewKind {
    pub const MENU: [NewKind; 3] = [NewKind::Nirvana, NewKind::ClaudeCode, NewKind::Antigravity];

    pub fn label(self) -> &'static str {
        match self {
            Self::Nirvana => "Nirvana",
            Self::ClaudeCode => "Claude Code",
            Self::Antigravity => "Antigravity",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Nirvana => "nirvana",
            Self::ClaudeCode => "claude",
            Self::Antigravity => "agy",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::MENU.into_iter().find(|kind| kind.key() == key)
    }

    /// The herdr agent kind label `agent start` takes; the program it types is herdr's own for that kind.
    pub fn agent_label(self) -> &'static str {
        match self {
            Self::Nirvana | Self::ClaudeCode => "claude",
            Self::Antigravity => "agy",
        }
    }

    /// The herdr agent name's stem; `nirvana` also marks the pane as Nirvana Claude's for the verb's second click.
    pub fn name_stem(self) -> &'static str {
        self.key()
    }

    /// What is typed after the program: Nirvana's flags, nothing for the other two.
    pub fn args(self, home: &Path) -> Vec<String> {
        match self {
            Self::Nirvana => nirvana_claude_args(home),
            Self::ClaudeCode | Self::Antigravity => Vec::new(),
        }
    }

    /// The pane's extra environment.
    pub fn env(self) -> Vec<(String, String)> {
        match self {
            Self::Nirvana => vec![(ORCHESTRATOR_ENV.to_owned(), "1".to_owned())],
            Self::ClaudeCode | Self::Antigravity => Vec::new(),
        }
    }

    pub fn command_id(self) -> String {
        format!("{NEW_COMMAND}{}", self.key())
    }
}

/// `<home>/nirvana/claude`, where `install.py` copies the plugin.
pub fn plugin_dir(home: &Path) -> PathBuf {
    home.join("nirvana").join("claude")
}

/// The orchestrator's role, appended to Claude Code's own system prompt.
pub const ROLE_FILE: &str = "ROLE.md";
/// §12 R1: "The orchestrator is Opus 5.5 at medium."
pub const MODEL: &str = "claude-opus-5-5";
pub const EFFORT: &str = "medium";

/// The arguments after `claude` (plan 12 §6, O4b), in its order.
pub fn nirvana_claude_args(home: &Path) -> Vec<String> {
    let plugin = plugin_dir(home);
    vec![
        "--plugin-dir".into(),
        plugin.display().to_string(),
        "--agent".into(),
        "nirvana".into(),
        "--model".into(),
        MODEL.into(),
        "--effort".into(),
        EFFORT.into(),
        "--append-system-prompt-file".into(),
        plugin.join(ROLE_FILE).display().to_string(),
        "--name".into(),
        DISPLAY_NAME.into(),
        "--settings".into(),
        plugin.join("settings.json").display().to_string(),
    ]
}

/// Whether an agent name is one `NewKind::Nirvana` gives: `nirvana` or `nirvana-<n>`.
pub fn is_nirvana_agent_name(name: &str) -> bool {
    name == NewKind::Nirvana.name_stem()
        || name
            .strip_prefix("nirvana-")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The first of `stem`, `stem-2`, `stem-3`, ... that `taken` does not hold.
pub fn free_agent_name(stem: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(stem) {
        return stem.to_owned();
    }
    (2u32..)
        .map(|n| format!("{stem}-{n}"))
        .find(|name| !taken(name))
        .expect("an unbounded range always has a free name")
}

/// The folder as the taskbar id hashes it: separators as `/`, no trailing separator, lower case. Windows paths are
/// case-insensitive, and Explorer hands `%V` in whatever case the folder was typed, so `C:\Work` and `c:/work/` must
/// be one button.
pub fn folder_key(folder: &Path) -> String {
    let text = folder.display().to_string().replace('\\', "/");
    let trimmed = text.trim_end_matches('/');
    let trimmed = if trimmed.is_empty() { "/" } else { trimmed };
    trimmed.to_lowercase()
}

/// 16 hex digits of the folder key's sha256: short enough for an AppUserModelID (at most 128 characters, no
/// spaces), long enough that two of the owner's folders never meet.
pub fn folder_hash(folder: &Path) -> String {
    let digest = Sha256::digest(folder_key(folder).as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

pub fn window_app_id(folder: &Path) -> String {
    format!("{WINDOW_APP_ID_PREFIX}{}", folder_hash(folder))
}

/// The folder's name as the toast shows it: its last part, or the whole path for a drive root.
pub fn folder_name(folder: &Path) -> String {
    folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| folder.display().to_string())
}

/// The badge's letter: the first letter or digit of the folder's name, upper-cased when that is one character
/// (`ß` stays `ß`, not `SS`). `.config` gives `C`; a name with neither, or none, gives `N` for Nirvana.
pub fn folder_letter(folder: &Path) -> char {
    letter_of(&folder_name(folder))
}

pub fn letter_of(name: &str) -> char {
    let Some(first) = name.chars().find(|c| c.is_alphanumeric()) else {
        return 'N';
    };
    let mut upper = first.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(single), None) => single,
        _ => first,
    }
}

/// "Nirvana · <folder>", the toast's title (plan 12 §8).
pub fn toast_title(folder: &Path) -> String {
    format!("{DISPLAY_NAME} \u{b7} {}", folder_name(folder))
}

/// The folder Explorer hands the verb. A drive root arrives as `C:"` because `"%V"` with `%V = C:\` puts a
/// backslash before the closing quote, which the argument rules read as an escaped quote; the quote is dropped and
/// the root's backslash put back.
pub fn verb_folder(arg: &str) -> PathBuf {
    let trimmed = arg.trim().trim_end_matches('"').trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return PathBuf::from(format!("{trimmed}\\"));
    }
    PathBuf::from(trimmed)
}

/// The `command.invoke` id the verb's window sends.
pub fn open_command_id(folder: &Path, fresh: bool) -> String {
    format!(
        "{OPEN_COMMAND}{}{}",
        if fresh { OPEN_FRESH } else { "" },
        folder.display()
    )
}

/// The folder and the fresh flag back from an `OPEN_COMMAND` id's tail.
pub fn parse_open_command(rest: &str) -> (PathBuf, bool) {
    match rest.strip_prefix(OPEN_FRESH) {
        Some(folder) => (PathBuf::from(folder), true),
        None => (PathBuf::from(rest), false),
    }
}

const USAGE: &str = "usage: herdr --session nirvana nirvana window <folder>
  Opens Nirvana Claude on <folder> in this terminal: starts the session's server if it is not running, focuses the
  Nirvana Claude already on the folder or opens a space with one, marks this Windows Terminal window with the
  Nirvana button and the folder's letter, and runs the herdr client until it exits. Explorer's \"Open Nirvana\"
  runs it.
       herdr nirvana digest|guard-spawn|subagent-stop|stop|statusline
  The Nirvana Claude plugin's hooks and statusline (plan 12 O4b); each reads Claude Code's JSON on stdin.";

/// `herdr nirvana <subcommand>`.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    match args.first().map(String::as_str) {
        Some("window") if args.len() == 2 => {
            let code = window(&verb_folder(&args[1]))?;
            std::process::exit(code);
        }
        // Plan 12 O4b: the plugin's hooks and statusline.
        Some(name) if args.len() == 1 && crate::nirvana_claude::SUBCOMMANDS.contains(&name) => {
            crate::nirvana_claude::cli(name).expect("a listed subcommand")
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

/// The verb's window: the server, the space, the button and badge, the toast, then the client until it exits.
fn window(folder: &Path) -> std::io::Result<i32> {
    if !folder.is_dir() {
        eprintln!("nirvana: {} is not a folder", folder.display());
        hold_window();
        return Ok(2);
    }
    let folder = std::path::absolute(folder).unwrap_or_else(|_| folder.to_path_buf());
    let fresh = !crate::server::autodetect::is_server_listening().unwrap_or(false);
    if fresh {
        // The server seeds its first space from its start folder; the open below then takes that space.
        std::env::set_current_dir(&folder)?;
        crate::server::autodetect::spawn_server_daemon()?;
        crate::server::autodetect::wait_for_server_socket(
            &crate::server::socket_paths::client_socket_path(),
            std::time::Duration::from_secs(15),
        )?;
    }
    if let Err(err) = invoke_open(&folder, fresh) {
        // The window still opens: the space and its shell are there to see why.
        eprintln!("nirvana: Nirvana Claude did not start: {err}");
    }
    let claimed = crate::nirvana_taskbar::claim_window(&folder);
    if let Err(err) = &claimed {
        // This command runs before herdr's logging starts, so the window itself says it.
        eprintln!("nirvana: the taskbar button was not set: {err}");
    }
    crate::nirvana_taskbar::toast_open(&folder);
    std::env::set_var(WINDOW_FOLDER_ENV, &folder);
    let code = crate::nirvana::supervise_code(&[]);
    // Badge first, then the properties (P7): the Terminal window outlives this process.
    if claimed.is_ok() {
        crate::nirvana_taskbar::release_window();
    }
    code
}

fn invoke_open(folder: &Path, fresh: bool) -> std::io::Result<()> {
    let request = crate::api::schema::Request {
        id: "nirvana:open".into(),
        method: crate::api::schema::Method::CommandInvoke(
            crate::api::schema::CommandInvokeParams {
                command_id: open_command_id(folder, fresh),
                workspace_id: None,
                tab_id: None,
                pane_id: None,
                selection: None,
            },
        ),
    };
    let response = crate::api::client::ApiClient::local()
        .request_value(&request)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    match response.get("error") {
        Some(error) => Err(std::io::Error::other(error.to_string())),
        None => Ok(()),
    }
}

/// A window that closes at once never shows why; wait for Enter when there is someone to press it.
fn hold_window() {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        eprintln!("Press Enter to close.");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAGS: [&str; 7] = [
        "--plugin-dir",
        "--model",
        "--effort",
        "--append-system-prompt-file",
        "--agent",
        "--name",
        "--settings",
    ];

    #[test]
    fn nirvana_claude_takes_the_plugin_agent_name_and_settings_from_home() {
        let home = Path::new("C:/Users/o/AppData/Local/Programs/Nirvana-Herdr");
        let plugin = home.join("nirvana").join("claude");
        assert_eq!(
            NewKind::Nirvana.args(home),
            vec![
                "--plugin-dir".to_string(),
                plugin.display().to_string(),
                "--agent".into(),
                "nirvana".into(),
                "--model".into(),
                "claude-opus-5-5".into(),
                "--effort".into(),
                "medium".into(),
                "--append-system-prompt-file".into(),
                plugin.join("ROLE.md").display().to_string(),
                "--name".into(),
                "Nirvana".into(),
                "--settings".into(),
                plugin.join("settings.json").display().to_string(),
            ]
        );
        assert_eq!(
            NewKind::Nirvana.env(),
            vec![("NIRVANA_ORCHESTRATOR".to_string(), "1".to_string())]
        );
        assert_eq!(NewKind::Nirvana.agent_label(), "claude");
        // O4b: the role is appended to Claude Code's own prompt; the agent's own file has no prompt to replace it.
        let agent = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("nirvana/claude/agents/nirvana.md"),
        )
        .expect("the agent file");
        let body = agent.splitn(3, "---").nth(2).expect("front matter");
        assert!(
            body.trim().is_empty(),
            "the agent's prompt replaces Claude Code's: {body:?}"
        );
    }

    #[test]
    fn plain_claude_code_and_agy_carry_no_nirvana_flag_and_no_nirvana_env() {
        // L6: "a test that plain Claude's argv carries no Nirvana flag".
        let home = Path::new("/home/o/.nirvana-herdr");
        for kind in [NewKind::ClaudeCode, NewKind::Antigravity] {
            let args = kind.args(home);
            for flag in FLAGS {
                assert!(!args.iter().any(|a| a == flag), "{kind:?} carries {flag}");
            }
            assert!(
                !args.iter().any(|a| a.contains("nirvana")),
                "{kind:?}: {args:?}"
            );
            assert!(kind.env().is_empty(), "{kind:?} sets an env");
            assert!(!is_nirvana_agent_name(kind.name_stem()));
        }
        assert_eq!(NewKind::ClaudeCode.agent_label(), "claude");
        assert_eq!(NewKind::Antigravity.agent_label(), "agy");
        assert_eq!(
            crate::detect::parse_agent_label(NewKind::Antigravity.agent_label()),
            Some(crate::detect::Agent::Antigravity)
        );
    }

    #[test]
    fn the_menu_offers_nirvana_claude_code_and_antigravity_in_that_order() {
        let labels: Vec<_> = NewKind::MENU.iter().map(|kind| kind.label()).collect();
        assert_eq!(labels, ["Nirvana", "Claude Code", "Antigravity"]);
        for kind in NewKind::MENU {
            assert_eq!(NewKind::from_key(kind.key()), Some(kind));
            assert_eq!(kind.command_id(), format!("nirvana.new:{}", kind.key()));
        }
        assert_eq!(NewKind::from_key("codex"), None);
    }

    #[test]
    fn nirvana_agent_names_are_recognised_and_numbered() {
        assert!(is_nirvana_agent_name("nirvana"));
        assert!(is_nirvana_agent_name("nirvana-12"));
        assert!(!is_nirvana_agent_name("nirvana-"));
        assert!(!is_nirvana_agent_name("nirvana-x"));
        assert!(!is_nirvana_agent_name("claude"));
        let taken = ["nirvana", "nirvana-2"];
        assert_eq!(
            free_agent_name("nirvana", |n| taken.contains(&n)),
            "nirvana-3"
        );
        assert_eq!(free_agent_name("agy", |_| false), "agy");
    }

    #[test]
    fn one_folder_one_id_whatever_its_case_or_separators() {
        let a = Path::new(r"C:\Users\o\Desktop\Pojects\MotoProject");
        let b = Path::new("c:/users/o/desktop/pojects/motoproject/");
        assert_eq!(folder_hash(a), folder_hash(b));
        assert_eq!(folder_hash(a).len(), 16);
        assert!(folder_hash(a).bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(
            folder_hash(a),
            folder_hash(Path::new(r"C:\Users\o\Desktop\Pojects\Nirvana"))
        );
        let id = window_app_id(a);
        assert!(id.starts_with("Nirvana.Herdr.") && id.len() <= 128 && !id.contains(' '));
        // The toast's id is the fork's own and never a folder's.
        assert_ne!(TOAST_APP_ID, id);
        // A known value pins the hash, so a change of rule is a visible change of every button.
        assert_eq!(folder_key(a), "c:/users/o/desktop/pojects/motoproject");
    }

    #[test]
    fn the_letter_is_the_first_letter_or_digit_of_the_folder() {
        assert_eq!(folder_letter(Path::new("C:/work/pojects")), 'P');
        assert_eq!(folder_letter(Path::new("C:/work/2026-plans")), '2');
        assert_eq!(folder_letter(Path::new("C:/work/.config")), 'C');
        assert_eq!(folder_letter(Path::new("C:/work/über")), 'Ü');
        assert_eq!(folder_letter(Path::new("C:/work/日本")), '日');
        assert_eq!(letter_of("ßeta"), 'ß');
        assert_eq!(letter_of(""), 'N');
        assert_eq!(letter_of("---"), 'N');
        assert_eq!(folder_letter(Path::new("")), 'N');
        assert_eq!(toast_title(Path::new("C:/x/Pojects")), "Nirvana · Pojects");
    }

    #[test]
    fn the_verb_folder_survives_explorers_quoting_of_a_drive_root() {
        assert_eq!(verb_folder("C:\""), PathBuf::from("C:\\"));
        assert_eq!(verb_folder("D:"), PathBuf::from("D:\\"));
        assert_eq!(
            verb_folder(r"C:\Users\o\My Work"),
            PathBuf::from(r"C:\Users\o\My Work")
        );
    }

    #[test]
    fn the_open_command_round_trips_folder_and_fresh() {
        let folder = Path::new(r"C:\Users\o\My Work");
        for fresh in [false, true] {
            let id = open_command_id(folder, fresh);
            let rest = id.strip_prefix(OPEN_COMMAND).unwrap();
            assert_eq!(parse_open_command(rest), (folder.to_path_buf(), fresh));
        }
    }
}
