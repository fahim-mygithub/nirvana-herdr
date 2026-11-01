//! Nirvana fork, plan 35 P35c: bladebro for browser orders, on both CLIs
//! (nirvana/plans/2026-10-31-35-agy-tool-parity.md §4 R1 and §5 P35c).
//!
//! The owner, 2026-10-31, to R1 (should a browser order get bladebro on either CLI, with its own browser home per
//! order): "Yes, both CLIs (Recommended)". Measured before (§1.1): none of 123 claude worker streams named bladebro,
//! while `nirvana_lean::BROWSER_LINE` told browser orders on claude seats to use it. Decision 0043's per-order config
//! was old Nirvana's and was never ported.
//!
//! - **The grant.** A need `browser` or `bladebro` grants bladebro on a claude or an agy seat. `browser` stays a
//!   capability for the pick; `bladebro` is no capability and seats anywhere. The definition is herdr's own, never a
//!   project's `.mcp.json` entry of the same name.
//! - **The definition** is decision 0043's measured recipe: `cmd /c bladebro mcp` (the form the owner's user-scope
//!   registration measured), `BLADE_HOME` the order's own folder `<run>/blade/<order>` (never `~/.blade`, never the
//!   owner's Chrome profile), `BLADE_CHROME_FLAGS=--headless=new` (0043: with no flag its Chrome had a window on the
//!   owner's desktop), `BLADE_LANE=agent` (0043: the real-browser lane drives the owner's browser),
//!   `BLADE_IDLE_TIMEOUT=60` (0040 ruling 3's backstop; bladebro's default is 600) and `BLADE_NO_UPDATE_CHECK=1`
//!   (0043: a hypothesis read off the binary). A claude worker gets it in its per-order `--mcp-config` with
//!   `mcp__bladebro__*` approved; an agy worker in its tools folder (`nirvana_agy_tools`), which plan probe 2 measured.
//! - **The shell.** The worker's own environment carries the same `BLADE_*` (0043's fix round: the variables reached
//!   the server only, and a shell `bladebro` used `~/.blade`). `worker_env` reads them back from the config the argv
//!   names, so the shell and the server can never be given two homes.
//! - **Not installed grants nothing** (0043 item 6), and the order says so in one line.
//! - **What wins over the grant** (review round): a profile that denies the whole server (`mcp__bladebro` or
//!   `mcp__bladebro__*`) and, on agy, a workspace `.agents/mcp_config.json` that names a `bladebro` of its own (S2:
//!   agy loads it anyway, with the workspace's home, so a herdr home and line would be a grant herdr cannot keep).
//!   Neither makes a home; each says why in one line.
//! - **One reading** (review round S3): the brief's line is decided at seating and the grant at each launch, by the
//!   same `grant`. The launch reads back what the brief was told (`Seat::read`) and grants only what both readings
//!   say; a launch that disagrees with its brief says so in one line of the worker's view.
//! - **The home goes when the worker ends**: 3 tries a second apart, then one line. The checks once leaked 144 GB of
//!   temp profiles. Nothing is killed by name: on Windows the server owns its Chrome (0043: "daemon mode is
//!   Unix-only"), and the worker's kill-on-close job ends it with the worker.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

/// The server's name in every config herdr writes, and the program on PATH.
pub const SERVER: &str = "bladebro";

/// R1: the needs that grant bladebro, as whole tokens in any case.
pub const GRANTING: [&str; 2] = ["browser", SERVER];

/// The folder in the run's folder that holds each granted order's `BLADE_HOME`.
pub const DIR: &str = "blade";

/// Decision 0043 (and 0040 ruling 3): seconds before bladebro's Chrome shuts down when idle. Its own default is 600.
pub const IDLE_TIMEOUT_S: u64 = 60;

/// 0043 item 6: what the order is told when bladebro is not on PATH.
pub const MISSING_LINE: &str =
    "bladebro is not on PATH, so this order has no browser tool (`npm install -g bladebro` gives it).";

/// Item 4: a home that will not go is tried this many times, `REMOVE_PAUSE` apart, then said in one line. Chrome's
/// files can stay open for a moment after the worker's job closes.
pub const REMOVE_TRIES: u32 = 3;
pub const REMOVE_PAUSE: Duration = Duration::from_secs(1);

#[cfg(test)]
thread_local! {
    /// A test's answer to "is bladebro on PATH", so no test depends on this machine's install.
    pub(crate) static TEST_INSTALLED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Whether `bladebro` is on PATH, the way a shell finds it (`bladebro.cmd` on Windows).
pub fn installed() -> bool {
    #[cfg(test)]
    if let Some(answer) = TEST_INSTALLED.get() {
        return answer;
    }
    crate::nirvana_usage::on_path(SERVER)
}

/// Review round: what the order's view is told when its profile denies the whole server.
pub const DENIED_LINE: &str =
    "the order's profile denies bladebro, so herdr grants it no browser and makes no browser home.";

/// Review round S3: what the view is told when this launch grants less than the brief, written at seating, names.
pub const TOLD_LINE: &str =
    "the brief, written at seating, tells the worker to use bladebro, and this launch grants none.";

/// Review round S3: what the view is told when bladebro would be granted now but the brief does not name it.
pub const UNTOLD_LINE: &str =
    "bladebro would be granted now, but the brief, written at seating, does not name it, so none is granted.";

/// What an order gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grant {
    /// No need asks for a browser, or the CLI is neither claude nor agy.
    No,
    /// A need asks for it and `bladebro` is not on PATH: nothing is granted, and the order says so.
    Missing,
    /// Review round: the order's profile denies the whole server; the denial wins, nothing is granted.
    Denied,
    /// Review round S2: an agy worker's workspace names a `bladebro` of its own, which agy loads anyway.
    Native,
    Yes,
}

/// Review round: whether a Claude Code deny list denies bladebro whole, `mcp__bladebro` or `mcp__bladebro__*`, in
/// any case. A deny of one of its tools (`mcp__bladebro__eval`) leaves the grant: each CLI's own deny rule holds that
/// tool back (claude reads its deny before its allow; on agy it maps to `mcp(bladebro/eval)`). Pure.
pub fn denies(denied: &[String]) -> bool {
    denied
        .iter()
        .flat_map(|tools| tools.split(','))
        .map(|tool| tool.trim().to_ascii_lowercase())
        .any(|tool| tool == format!("mcp__{SERVER}") || tool == format!("mcp__{SERVER}__*"))
}

/// Review round S2: whether `cwd`'s own `.agents/mcp_config.json` names `bladebro`. A file that does not parse names
/// nothing, as `nirvana_agy_tools::write_servers` reads it.
pub fn workspace_names(cwd: &Path) -> bool {
    crate::nirvana_agy_tools::workspace_servers(cwd)
        .is_ok_and(|names| names.iter().any(|name| name == SERVER))
}

/// Whether the needs ask for bladebro: a need, split on commas and whitespace and lower-cased, holds `browser` or
/// `bladebro` as a whole token (plan 25's review nit: `no-browser` is not a browser). Pure.
pub fn wanted(needs: &[String]) -> bool {
    needs.iter().any(|need| {
        need.split(|c: char| c == ',' || c.is_whitespace())
            .map(str::to_ascii_lowercase)
            .any(|token| GRANTING.contains(&token.as_str()))
    })
}

/// R1, pure: the grant for an order on `cli` with `needs`, denied `denied` by its profile, given whether its agy
/// workspace names a `bladebro` of its own (`native`) and whether bladebro is on PATH. A denial wins over the need, and
/// on agy so does the workspace's own `bladebro`.
pub fn grant_with(
    cli: &str,
    needs: &[String],
    denied: &[String],
    native: bool,
    on_path: bool,
) -> Grant {
    if !matches!(cli, "claude" | "agy") || !wanted(needs) {
        Grant::No
    } else if denies(denied) {
        Grant::Denied
    } else if cli == "agy" && native {
        Grant::Native
    } else if on_path {
        Grant::Yes
    } else {
        Grant::Missing
    }
}

/// R1: `grant_with`, reading `cwd`'s workspace file and PATH now. The brief (`nirvana_lean::browser_line`) and the
/// launch (`nirvana_gui_tools::order_argv_seated`) both decide through this one function.
pub fn grant(cli: &str, needs: &[String], denied: &[String], cwd: &Path) -> Grant {
    if !matches!(cli, "claude" | "agy") || !wanted(needs) {
        return Grant::No;
    }
    grant_with(
        cli,
        needs,
        denied,
        cli == "agy" && workspace_names(cwd),
        installed(),
    )
}

/// What the launch reads back from seating: the profile's deny list, and whether the brief names bladebro.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seat {
    /// The Claude Code tool names the order's profile denies (`disallowed_tools`).
    pub denied: Vec<String>,
    /// Whether the brief written at seating holds `nirvana_lean::BROWSER_LINE`; `None` when no brief was read.
    pub told: Option<bool>,
}

impl Seat {
    /// The order's brief and, when it names one, its profile copy in `run_dir`. A lesson review quotes the order it
    /// reviews and never takes its profile (plan 33 review B1), so its denials are none. A brief or a profile copy
    /// that cannot be read gives what could be read: `with_profile` fails such an order next, before it starts.
    // Until P35c's merge hunk H7 calls it, only the tests do; H8 removes this allowance.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn read(run_dir: &Path, class: &str, order_file: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(order_file) else {
            return Self::default();
        };
        let denied = match crate::nirvana_profiles::in_order(&text) {
            Some(Ok(found)) if class != crate::nirvana_review::CLASS => {
                crate::nirvana_profiles::from_run(run_dir, &found)
                    .map(|profile| profile.disallowed_tools)
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        };
        Self {
            denied,
            told: Some(text.contains(crate::nirvana_lean::BROWSER_LINE)),
        }
    }
}

/// The launch's answer, given this launch's `grant` and what the brief was told: granted only when both say so. Each
/// grant that is not given says why in one line of `out`, and one that disagrees with the brief says that too.
pub fn at_launch(grant: Grant, told: Option<bool>, cwd: &Path, out: &mut dyn Write) -> bool {
    match grant {
        Grant::Yes if told == Some(false) => {
            let _ = writeln!(out, "{UNTOLD_LINE}");
            return false;
        }
        Grant::Yes => return true,
        Grant::Missing => {
            let _ = writeln!(out, "{MISSING_LINE}");
        }
        Grant::Denied => {
            let _ = writeln!(out, "{DENIED_LINE}");
        }
        Grant::Native => {
            let _ = writeln!(
                out,
                "{} names a bladebro of its own, which agy loads anyway with that server's home, so herdr grants \
                 none and makes no browser home.",
                cwd.join(crate::nirvana_agy_tools::MCP_CONFIG).display()
            );
        }
        Grant::No => {}
    }
    if told == Some(true) {
        let _ = writeln!(out, "{TOLD_LINE}");
    }
    false
}

/// `<run>/blade/<order>`: the order's own `BLADE_HOME`, under `runs/`, which `.nirvana/.gitignore` ignores.
pub fn home(run_dir: &Path, order: &str) -> PathBuf {
    run_dir.join(DIR).join(order)
}

/// The server's variables, the same in its definition and in the worker's shell (item 2).
pub fn env(home: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("BLADE_HOME", crate::nirvana_agy_tools::plain_path(home)),
        ("BLADE_CHROME_FLAGS", "--headless=new".to_owned()),
        ("BLADE_LANE", "agent".to_owned()),
        ("BLADE_IDLE_TIMEOUT", IDLE_TIMEOUT_S.to_string()),
        ("BLADE_NO_UPDATE_CHECK", "1".to_owned()),
    ]
}

/// The one server entry, in `.mcp.json`'s shape (agy's translation passes `command`, `args` and `env`). `cmd /c
/// bladebro mcp` on Windows, as the owner's user-scope registration measured; the shim itself elsewhere.
pub fn definition(home: &Path) -> Value {
    let env: serde_json::Map<String, Value> = env(home)
        .into_iter()
        .map(|(key, value)| (key.to_owned(), Value::from(value)))
        .collect();
    let (command, args) = if cfg!(windows) {
        ("cmd", vec!["/c", SERVER, "mcp"])
    } else {
        (SERVER, vec!["mcp"])
    };
    serde_json::json!({ "command": command, "args": args, "env": env })
}

/// Item 2: the `BLADE_*` variables of the bladebro server the worker's argv was given, read from the config file the
/// argv names (claude's `--mcp-config`, agy's `--add-dir` tools folder); none when it has no bladebro. Reading them
/// back from the file means the shell and the server are handed one home. `nirvana_worker::run_attempt` lays them over
/// the worker's environment (P35c's hunk, applied at the merge; until then only the tests call this). Review round S1:
/// only the `BLADE_*` keys cross, so no token another hand put in that entry reaches the shell.
#[cfg_attr(not(test), allow(dead_code))]
pub fn worker_env(argv: &[String]) -> Vec<(String, String)> {
    let after = |flag: &str| {
        argv.iter()
            .position(|arg| arg == flag)
            .and_then(|at| argv.get(at + 1))
            .map(PathBuf::from)
    };
    let file = match argv.first().map(String::as_str) {
        Some("claude") => after("--mcp-config"),
        Some("agy") => {
            after("--add-dir").map(|tools| tools.join(crate::nirvana_agy_tools::MCP_CONFIG))
        }
        _ => None,
    };
    let Some(text) = file.and_then(|file| std::fs::read_to_string(file).ok()) else {
        return Vec::new();
    };
    let Ok(config) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    config["mcpServers"][SERVER]["env"]
        .as_object()
        .map(|env| {
            env.iter()
                .filter(|(key, _)| key.starts_with("BLADE_"))
                .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// Item 4: the order's `BLADE_HOME` goes when the worker ends, whatever the outcome.
pub fn remove_home(run_dir: &Path, order: &str, out: &mut dyn Write) {
    remove_home_with(&home(run_dir, order), REMOVE_TRIES, REMOVE_PAUSE, out);
}

/// `remove_home` for one folder: `tries` removes `pause` apart, then one line naming the folder and the last error. A
/// folder that is not there is nothing to say.
pub fn remove_home_with(home: &Path, tries: u32, pause: Duration, out: &mut dyn Write) {
    let mut last = None;
    for attempt in 0..tries.max(1) {
        if attempt > 0 {
            std::thread::sleep(pause);
        }
        match std::fs::remove_dir_all(home) {
            Ok(()) => return,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(err) => last = Some(err),
        }
    }
    if let Some(err) = last {
        let _ = writeln!(
            out,
            "the order's bladebro home stays at {} after {} tries: {err}",
            home.display(),
            tries.max(1)
        );
    }
}

#[cfg(test)]
mod tests;
