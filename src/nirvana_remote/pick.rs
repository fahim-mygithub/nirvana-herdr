//! Plan 39 P39d, §2.5: the new-space picker (prototype screen 6, the "Open a new space" sheet on screen 2).
//!
//! The owner, 2026-11-04, to which folders the picker may open: "Desktop\Pojects only (Recommended)" (plan 39 L11,
//! §6.2). The root is `<the user's profile>\Desktop\Pojects` (`root`), read from `USERPROFILE` as
//! `nirvana_launch`'s own `user_home_dir` reads it, never a typed user name.
//!
//! **What the page may name.** A folder directly in the root, by its name exactly as `GET /api/folders` listed it.
//! The server does not take a path: it lists the root again and the name must be one of its entries
//! (`resolve`), so `..`, a `\\?\` or UNC path, another drive, a separator, an ADS `:` or a reserved device name
//! (`CON`, `COM1`, ...) is never a listed name and is refused before the disk is asked (`name_ok`). The entry must
//! be a plain folder: a junction or a symbolic link (`FILE_ATTRIBUTE_REPARSE_POINT`) is neither listed nor opened,
//! since it could point out of the root. Then both are canonicalised and the folder's parent must be the root.
//! The listing is folder names only: no file, no file's contents, no size or time.
//!
//! **How a space starts.** The same path as Explorer's "Open Nirvana here" (`herdr nirvana open <folder>`,
//! `nirvana_launch::open`, `src/nirvana_launch.rs:734`): its live-window route applies the folder's preset first
//! (`nirvana_preset::at_open`, `:754`), then sends `command.invoke` with `open_command_id(folder, fresh, true)`
//! (`:755`, `invoke_open` at `:939-959`; `nirvana window` does the same at `:858-860`). `fresh` is "this click
//! started the server" (`:753`); Nirvana Remote runs inside the
//! server, so it is false. herdr's app takes the id in `handle_command_invoke` (`src/app/custom_commands.rs:120-127`)
//! and opens the space in `nirvana_open_folder` (`src/app/nirvana_new.rs:70-101`): a new space on the folder with
//! Nirvana Claude started in its root pane by `nirvana_start_agent` (`:144-206`), resuming the folder's last session
//! as the PC would. The window's flash and toast (`:756-757`) are for someone at the PC and are not sent.
//!
//! **A folder already open.** When a pane of a space has the folder as its `cwd` (herdr's `pane.list`, compared as
//! `nirvana_launch::folder_key` compares folders once both are canonical, `canonical_key`), the answer names that
//! space and nothing is started: the phone opens it instead of a second one. herdr's own `nirvana_space_on` would
//! focus a space only when Nirvana Claude runs there, and would open a second space beside a plain shell on the
//! folder; this check covers both, and says which (`agent`): a space with only plain shells on the folder is "open,
//! no agent running" on the page (review S2, 2026-11-05), never a bare "open" with no Claude to be found. A space
//! with Nirvana Claude on the folder wins over one with only a shell. Opens are one at a time (`Local::opening`), so
//! two taps cannot both find the folder closed.
//!
//! **Between the check and the open.** The checks run again right before `command.invoke` (review S1): the preset
//! before it may take seconds (`OPEN_WALL`, and it may ask Jev), long enough for the folder to be swapped for a
//! junction. That narrows the window to the time between two syscalls and herdr's own read of the path; it does not
//! close it, since herdr opens the folder by its path and resolves it again itself.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{json, Value};

use super::http::{Conn, Framing, Request, Response};
use super::routes::Site;

/// The root under the user's profile, as the sheet names it.
pub const ROOT_UNDER_PROFILE: [&str; 2] = ["Desktop", "Pojects"];
/// A body of `{"name": ...}` is never longer than this.
pub const OPEN_MOST: u64 = 4 * 1024;
/// At most this many folders are listed.
pub const LIST_MOST: usize = 200;

/// A preset applier: `nirvana_preset::at_open` on the PC, a stand-in in tests (it reads Nirvana's home and may ask
/// Jev, which no test may do).
pub type Preset = Arc<dyn Fn(&Path) -> Option<String> + Send + Sync>;
/// A space's ports as the servers block last read them.
pub type Ports = Arc<dyn Fn(&str) -> Vec<u16> + Send + Sync>;

/// What P39d reads on this PC outside herdr's API: the picker's root, the preset the verb applies, and the ports
/// the servers block draws. The default reads nothing and applies nothing (a test's, and a `Site::new`'s).
pub struct Local {
    pub root: Option<PathBuf>,
    pub preset: Preset,
    pub ports: Ports,
    opening: Mutex<()>,
}

impl Default for Local {
    fn default() -> Self {
        Self::with(None, Arc::new(|_| None), Arc::new(|_| Vec::new()))
    }
}

impl Local {
    pub fn with(root: Option<PathBuf>, preset: Preset, ports: Ports) -> Self {
        Self {
            root,
            preset,
            ports,
            opening: Mutex::new(()),
        }
    }

    /// The PC's own: the profile's root, the verb's preset, the server's ports cache.
    pub fn live() -> Self {
        Self::with(
            root(),
            Arc::new(crate::nirvana_preset::at_open),
            Arc::new(crate::nirvana_servers::last_ports),
        )
    }
}

/// `<the user's profile>\Desktop\Pojects`.
pub fn root() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .filter(|home| !home.is_empty())?;
    let mut root = PathBuf::from(home);
    for part in ROOT_UNDER_PROFILE {
        root.push(part);
    }
    root.is_absolute().then_some(root)
}

/// Windows' device names: `CON` or `con.txt` names the console, not a folder, in every folder.
fn reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end();
    let upper = stem.to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let Some(tail) = upper
        .strip_prefix("COM")
        .or_else(|| upper.strip_prefix("LPT"))
    else {
        return false;
    };
    matches!(
        tail,
        "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "\u{b9}" | "\u{b2}" | "\u{b3}"
    )
}

/// Whether `name` can be one folder's name in the root at all: one plain part, nothing a path would read as more.
pub fn name_ok(name: &str) -> Result<(), &'static str> {
    if name.is_empty() || name.len() > 255 {
        return Err("a folder's name");
    }
    // `\` and `/` are separators (and `\\?\` and UNC start with them), `:` is a drive or an ADS stream, and the rest
    // Windows does not allow in a name.
    if name.chars().any(|c| {
        c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }) {
        return Err("a folder's name has no path in it");
    }
    // Windows drops a trailing dot or space, so `groove-coach.` would open `groove-coach`; and `.` and `..` end in one.
    if name.ends_with('.') || name.ends_with(' ') {
        return Err("a folder's name does not end in a dot or a space");
    }
    if reserved(name) {
        return Err("a Windows device name");
    }
    Ok(())
}

/// A folder, not a file, a link or a junction. The link itself is read (`symlink_metadata`), never its target:
/// `metadata` would follow a junction and read its target's folder.
pub fn plain_dir(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT: any reparse point, a cloud placeholder included. For a junction this and
        // `is_symlink` below are a second and third word: Rust's `FileType` already reads a junction's own entry as
        // a link and not a folder (measured: the junction test stayed green with both taken away; it goes red only
        // when the target is read instead, control `p39d_plain_dir_no_follow`).
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    meta.is_dir() && !meta.file_type().is_symlink()
}

/// The root's folders the picker offers: plain folders whose name `name_ok` takes, not hidden ones (a name starting
/// with `.`, such as `.nirvana`), the most recently changed first.
pub fn list(root: &Path) -> Result<Vec<String>, String> {
    let entries = std::fs::read_dir(root).map_err(|err| format!("{}: {err}", root.display()))?;
    let mut folders: Vec<(std::time::SystemTime, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if name.starts_with('.') || name_ok(&name).is_err() || !plain_dir(&entry.path()) {
                return None;
            }
            let changed = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            Some((changed, name))
        })
        .collect();
    folders.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.to_lowercase().cmp(&right.1.to_lowercase()))
    });
    folders.truncate(LIST_MOST);
    Ok(folders.into_iter().map(|(_, name)| name).collect())
}

/// The folder the page named, after every check: one of `list`'s names, a plain folder, and inside the root once
/// both are canonical. The path is the root joined with the name, as Explorer would hand the verb it.
pub fn resolve(root: &Path, name: &str) -> Result<PathBuf, (u16, &'static str)> {
    name_ok(name).map_err(|why| (400, why))?;
    let listed = list(root).map_err(|_| (409, "the root folder cannot be read"))?;
    if !listed.iter().any(|known| known == name) {
        return Err((404, "no such folder in Desktop\\Pojects"));
    }
    let folder = root.join(name);
    if !plain_dir(&folder) {
        return Err((400, "not a plain folder"));
    }
    if !inside(root, &folder) {
        return Err((400, "outside Desktop\\Pojects"));
    }
    Ok(folder)
}

/// Whether `folder`, once every link on its way is followed (`canonicalize`), sits directly in `root`, followed the
/// same way. A junction whose target is elsewhere, or a path that climbs out, is not.
pub fn inside(root: &Path, folder: &Path) -> bool {
    match (std::fs::canonicalize(root), std::fs::canonicalize(folder)) {
        (Ok(root), Ok(folder)) => folder.parent() == Some(root.as_path()),
        _ => false,
    }
}

/// `folder_key` of the folder as the disk names it: a `\\?\` prefix taken off, and on a local drive every short
/// (8.3) name and link read away (`canonicalize`, itself without its `\\?\`). A UNC path is left as written (a server
/// that is away could hold the read for seconds), and so is a path that cannot be read (review N1, 2026-11-05).
pub fn canonical_key(path: &Path) -> String {
    let text = path.display().to_string();
    let plain = text
        .strip_prefix(r"\\?\")
        .filter(|rest| !rest.starts_with(r"UNC\"))
        .unwrap_or(&text);
    let bytes = plain.as_bytes();
    let local = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let canonical = local
        .then(|| std::fs::canonicalize(plain).ok())
        .flatten()
        .map(|found| found.display().to_string());
    let named = canonical
        .as_deref()
        .map(|found| found.strip_prefix(r"\\?\").unwrap_or(found))
        .unwrap_or(plain);
    crate::nirvana_launch::folder_key(Path::new(named))
}

/// One pane from a `pane.list` answer: its id, its space and its folder's `canonical_key`.
pub struct PaneOn {
    pub pane: String,
    pub space: String,
    pub key: String,
}

/// Each pane's folder, read once for every folder a listing asks about.
pub fn pane_folders(panes: &Value) -> Vec<PaneOn> {
    let Some(panes) = panes["panes"].as_array() else {
        return Vec::new();
    };
    panes
        .iter()
        .filter_map(|pane| {
            Some(PaneOn {
                pane: pane["pane_id"].as_str().unwrap_or("").to_string(),
                space: pane["workspace_id"].as_str()?.to_string(),
                key: canonical_key(Path::new(pane["cwd"].as_str()?)),
            })
        })
        .collect()
}

/// The panes Nirvana Claude runs in, from an `agent.list` answer: an agent named as herdr's `nirvana_space_on` reads
/// a Nirvana agent (`is_nirvana_agent_name`: `nirvana`, `nirvana-2`, ...).
pub fn nirvana_panes(agents: &Value) -> Vec<String> {
    let Some(agents) = agents["agents"].as_array() else {
        return Vec::new();
    };
    agents
        .iter()
        .filter(|agent| {
            agent["name"]
                .as_str()
                .is_some_and(crate::nirvana_launch::is_nirvana_agent_name)
        })
        .filter_map(|agent| agent["pane_id"].as_str().map(str::to_string))
        .collect()
}

/// The space with a pane on `folder`, and whether Nirvana Claude runs in one of its panes there: a space with the
/// agent first, else the first with any pane there (a plain shell).
pub fn space_on(panes: &[PaneOn], nirvana: &[String], folder: &Path) -> Option<(String, bool)> {
    let key = canonical_key(folder);
    let on: Vec<&PaneOn> = panes.iter().filter(|pane| pane.key == key).collect();
    if let Some(pane) = on.iter().find(|pane| nirvana.contains(&pane.pane)) {
        return Some((pane.space.clone(), true));
    }
    on.first().map(|pane| (pane.space.clone(), false))
}

/// The root as the sheet's subtitle shows it.
pub fn root_label() -> String {
    ROOT_UNDER_PROFILE.join("\\")
}

fn error(status: u16, why: impl Into<String>) -> Response {
    Response::json(status, &json!({ "error": why.into() }))
}

#[derive(Deserialize)]
struct OpenBody {
    name: String,
}

impl Site {
    /// P39d's routes, after every check `route` makes: `GET /api/folders`, `POST /api/folders/open`, the cards'
    /// three `GET`s (`cards.rs`). The two screens' scripts are rows of `routes::ASSETS`. `None` for any other path.
    pub fn new_space_and_cards(&self, request: &Request, conn: &mut Conn) -> Option<Response> {
        let reads = request.method == "GET" || request.method == "HEAD";
        let answer = match (request.path.as_str(), reads) {
            ("/api/folders", true) => self.folders(),
            ("/api/folders/open", false) if request.method == "POST" => {
                self.open_folder(request, conn)
            }
            ("/api/cards/usage", true) => self.usage_card(),
            ("/api/cards/servers", true) => self.servers_card(),
            ("/api/cards/running", true) => self.running_card(),
            (
                "/api/folders" | "/api/cards/usage"
                | "/api/cards/servers" | "/api/cards/running",
                false,
            ) => Response::text(405, "GET only").with("Allow", "GET, HEAD"),
            ("/api/folders/open", _) => Response::text(405, "POST only").with("Allow", "POST"),
            _ => return None,
        };
        Some(answer)
    }

    /// `GET /api/folders`: the root's folders, each marked when a space is already open on it.
    fn folders(&self) -> Response {
        let Some(root) = self.local.root.as_deref() else {
            return error(409, "no Desktop\\Pojects folder on this PC");
        };
        let names = match list(root) {
            Ok(names) => names,
            Err(_) => return error(409, "Desktop\\Pojects cannot be read on this PC"),
        };
        let (panes, nirvana) = match self.panes_and_agents() {
            Ok(both) => both,
            Err(why) => return error(502, why),
        };
        let folders: Vec<Value> = names
            .into_iter()
            .map(|name| {
                let found = space_on(&panes, &nirvana, &root.join(&name));
                json!({
                    "name": name,
                    "open": found.is_some(),
                    "space": found.as_ref().map(|found| found.0.clone()),
                    "agent": found.is_some_and(|found| found.1),
                })
            })
            .collect();
        Response::json(200, &json!({ "root": root_label(), "folders": folders }))
    }

    /// herdr's panes (`pane_folders`) and the panes Nirvana Claude runs in (`nirvana_panes`).
    fn panes_and_agents(&self) -> Result<(Vec<PaneOn>, Vec<String>), String> {
        let panes = self.herdr.call("pane.list", json!({}))?;
        let agents = self.herdr.call("agent.list", json!({}))?;
        Ok((pane_folders(&panes), nirvana_panes(&agents)))
    }

    /// `POST /api/folders/open` `{"name"}`: the space on the folder, opened as "Open Nirvana here" opens it, or the
    /// one already open on it.
    fn open_folder(&self, request: &Request, conn: &mut Conn) -> Response {
        let json_body = request.header("Content-Type").first().is_some_and(|kind| {
            kind.trim()
                .to_ascii_lowercase()
                .starts_with("application/json")
        });
        if !json_body {
            return error(415, "an open is JSON");
        }
        if conn.framing() == Framing::None {
            return error(411, "an open has a length");
        }
        let mut body = Vec::new();
        if let Err(why) = conn.read_body(OPEN_MOST, &mut body) {
            return error(why.status(), "the open could not be read");
        }
        let Ok(body) = serde_json::from_slice::<OpenBody>(&body) else {
            return error(400, "an open is {name}");
        };
        let Some(root) = self.local.root.as_deref() else {
            return error(409, "no Desktop\\Pojects folder on this PC");
        };
        let folder = match resolve(root, &body.name) {
            Ok(folder) => folder,
            Err((status, why)) => return error(status, why),
        };
        let Ok(_one) = self.local.opening.lock() else {
            return error(500, "the picker is broken");
        };
        let (panes, nirvana) = match self.panes_and_agents() {
            Ok(both) => both,
            Err(why) => return error(502, why),
        };
        if let Some((space, agent)) = space_on(&panes, &nirvana, &folder) {
            return Response::json(
                200,
                &json!({ "space": space, "existing": true, "agent": agent }),
            );
        }
        let preset = (self.local.preset)(&folder);
        // Again, right before herdr is asked: the preset may have taken seconds (review S1).
        if !plain_dir(&folder) || !inside(root, &folder) {
            return error(409, "the folder changed while it was opening");
        }
        let started = match self.herdr.call(
            "command.invoke",
            json!({ "command_id": crate::nirvana_launch::open_command_id(&folder, false, true) }),
        ) {
            Ok(started) => started,
            Err(why) => return error(502, why),
        };
        // herdr's answer names the agent it started, and the agent its space; the argv is left on the PC.
        let space = started["agent"]["workspace_id"]
            .as_str()
            .map(str::to_string)
            .or_else(|| {
                let panes = pane_folders(&self.herdr.call("pane.list", json!({})).ok()?);
                space_on(&panes, &[], &folder).map(|found| found.0)
            });
        Response::json(
            200,
            &json!({ "space": space, "existing": false, "preset": preset }),
        )
    }
}
