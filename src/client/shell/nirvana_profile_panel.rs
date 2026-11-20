//! Nirvana fork, plan 45 P45a: the `+` after the sidebar's `agents` heading, the space menu's `Profiles` line, and
//! the Profiles panel they open. (nirvana/plans/2026-11-18-45-agent-profiles.md §2 L1, L2, L8, §5 "The door", §7)
//!
//! The owner, 2026-11-18 (L1): "for each project we add a plus symbol next to agents and the agents + becocmes  abutton
//! to store profile. Theses would be besically where we store the agent proiles." And (L8): "Have lessons look for
//! moments where it can consider building an agent like we described for future runs."
//!
//! ```text
//!  agents [+1]       grouped
//! ```
//!
//! The panel is one project's: the folder of the space the `+` or the menu line belongs to, by the Preload group's
//! rule (`super::nirvana_profiles::root`, the nearest folder holding `.git`). Three parts:
//!
//! - **Profiles**, one row each: description, seat pins, servers, skills, cards, and the use `usage` counts, "used 0
//!   of 35". `e` or Enter opens the file in an edit tab, `K` its `knowledge/`.
//! - **Suggested**, the proposals waiting (plan 45 P45b). Enter opens one whole; Accept is only there, so the owner has
//!   the body on screen when they accept (R1: a proposal waits for the owner). `d` dismisses, in the list or the detail.
//! - **New**, `n`: a name, then a shape from `crate::nirvana_profiles::SHAPES`.
//!
//! **Nothing here reads a file while the window draws.** The data is read on a thread of its own, started from the
//! window's timer tick (`take_changed`) at most every `REREAD`, the Seats square's interval, and only after the last
//! read ended, so a slow `usage` (its scan is bounded at 3 s) never runs twice at once. The `+` asks only for the
//! waiting count; the full read (profiles, `usage`, proposals) runs while the panel is drawn.
//!
//! **No worker can press Accept.** The panel's keys and clicks come only from this window's own terminal input
//! (`handle_raw_events`); no API method, socket message or `herdr` verb reaches them, and Accept and Dismiss call
//! `crate::nirvana_profile_proposals` directly, not `herdr profile accept`. The edit tabs go through `command.invoke`,
//! as the Preload group's do; they open a file and accept nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::nirvana_mods::{draw_frame, folder_name};
use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crate::nirvana_profile_proposals::{self as proposals, Kind, Proposal};
use crate::protocol::ClientShellPane;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::Modifier;

/// The panel's widest; under it, the window's pane area less a cell each side.
pub(super) const PANEL_WIDTH: u16 = 120;
/// Under this inner width a profile's name takes a line of its own and its description wraps under it.
pub(super) const NARROW_PANEL: usize = 72;
/// The name column of the wide form: as wide as the longest name, at most this.
const NAME_COL_MAX: usize = 24;
/// Rows PageUp and PageDown move, as the Preload panel's do.
const PAGE: usize = 10;
/// Rows a wheel notch moves, as the Seats table's does.
const WHEEL_STEP: usize = 3;
/// The Seats square's re-read interval: the brief's rule, "the interval the Seats panel uses".
const REREAD: Duration = crate::nirvana_seats_square::REREAD;
/// A folder the window has not drawn for this long is no longer read, as the Seats square forgets a project.
const FORGET_AFTER: Duration = Duration::from_secs(30);

/// The empty states, in plain words.
pub(super) const NO_PROFILES: &str = "No profiles yet: n makes one, or accept a suggestion below";
pub(super) const NOTHING_SUGGESTED: &str =
    "Nothing suggested: herdr proposes a profile when it sees a role retyped across orders";
/// `a` in the list: Accept is only in the detail, where the body is on screen.
pub(super) const ACCEPT_IN_DETAIL: &str =
    "a accepts only in a proposal's detail, with its body on screen: enter opens it";
/// The use of a profile no order fits or carries.
pub(super) const NO_USE: &str = "no order fits or carries it";

// ---- where a space's project is ----

/// A space's folder, from the snapshot only (no file is read), and the tab its edit tabs open beside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SpaceFolder {
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    pub(super) folder: PathBuf,
}

fn pane_folder(pane: &ClientShellPane) -> Option<String> {
    pane.foreground_cwd
        .clone()
        .or_else(|| pane.cwd.clone())
        .filter(|cwd| !cwd.is_empty())
}

/// The folder of a space: its Nirvana Claude's pane, as the Seats panel finds the project; else its active tab's
/// focused pane, else that tab's first pane with a folder; else the folder herdr opens a new space's pane in.
pub(super) fn space_folder(
    snapshot: &ClientShellSnapshot,
    workspace_id: &str,
) -> Option<SpaceFolder> {
    let workspace = snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == workspace_id)?;
    let nirvana = snapshot
        .agents
        .iter()
        .filter(|agent| {
            agent.workspace_id == workspace_id
                && agent
                    .name
                    .as_deref()
                    .is_some_and(crate::nirvana_launch::is_nirvana_agent_name)
        })
        .find_map(|agent| {
            snapshot
                .panes
                .iter()
                .find(|pane| pane.pane_id == agent.pane_id)
                .and_then(pane_folder)
        });
    let in_tab = || {
        let panes: Vec<&ClientShellPane> = snapshot
            .panes
            .iter()
            .filter(|pane| {
                pane.workspace_id == workspace_id && pane.tab_id == workspace.active_tab_id
            })
            .collect();
        panes
            .iter()
            .find(|pane| pane.focused)
            .and_then(|pane| pane_folder(pane))
            .or_else(|| panes.iter().find_map(|pane| pane_folder(pane)))
    };
    let folder = nirvana
        .or_else(in_tab)
        .or_else(|| Some(workspace.new_workspace_cwd.clone()).filter(|cwd| !cwd.is_empty()))?;
    Some(SpaceFolder {
        workspace_id: workspace_id.to_owned(),
        tab_id: workspace.active_tab_id.clone(),
        folder: PathBuf::from(folder),
    })
}

/// The focused space's folder: the one the agents panel lists the agents of.
pub(super) fn focused_space_folder(snapshot: &ClientShellSnapshot) -> Option<SpaceFolder> {
    let workspace_id = snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.focused)
        .map(|workspace| workspace.workspace_id.clone())?;
    space_folder(snapshot, &workspace_id)
}

// ---- the `+` ----

/// The `+`'s words: brackets so it reads as a button without colour, and the waiting count inside when any wait.
pub(super) fn plus_text(waiting: Option<usize>) -> String {
    match waiting {
        Some(count) if count > 0 => format!("[+{count}]"),
        _ => "[+]".to_owned(),
    }
}

// ---- what the panel shows ----

/// One profile as the panel shows it: what its file parsed to (`None` when the parser refuses it) and its use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfileRow {
    pub(super) name: String,
    pub(super) file: PathBuf,
    pub(super) parsed: Option<Parsed>,
    /// `usage`'s carried and fit counts; `None` when `usage` has no row for it.
    pub(super) use_: Option<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub(super) description: String,
    pub(super) family: Option<String>,
    pub(super) model: Option<String>,
    pub(super) effort: Option<String>,
    pub(super) needs: Vec<String>,
    /// Whether the folder profile holds its own `mcp.json`.
    pub(super) own_servers: bool,
    pub(super) skills: Option<Vec<String>>,
    /// A folder profile's card count; `None` for a profile that is one file, which keeps none.
    pub(super) cards: Option<usize>,
}

/// One read of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct View {
    pub(super) root: PathBuf,
    pub(super) profiles: Vec<ProfileRow>,
    pub(super) proposals: Vec<Proposal>,
    /// A proposal file or a ledger line herdr cannot read, as `herdr profile proposals` names them.
    pub(super) unreadable: Vec<String>,
    /// Whether the use counts were read; the first read after an Accept or a New keeps the last ones.
    pub(super) usage_read: bool,
}

/// Reads the project at `root`: the profiles, the proposals waiting and, with `usage`, each profile's use.
pub(super) fn read_view(root: &Path, usage: bool) -> View {
    let counted = usage.then(|| crate::nirvana_profile_detect::usage(root));
    let profiles = crate::nirvana_profiles::list(root)
        .into_iter()
        .map(|(name, file, parsed)| {
            let use_ = counted.as_ref().and_then(|rows| {
                rows.iter()
                    .find(|row| row.profile == name)
                    .map(|row| (row.carried, row.fit))
            });
            let parsed = parsed.ok().map(|profile| Parsed {
                description: profile.description,
                family: profile.family,
                model: profile.model,
                effort: profile.effort.map(|effort| effort.word().to_owned()),
                needs: profile.needs,
                own_servers: crate::nirvana_profiles::servers_path(root, &name)
                    .is_some_and(|file| file.is_file()),
                skills: profile.skills,
                cards: crate::nirvana_profiles::knowledge_dir(root, &name)
                    .map(|dir| crate::nirvana_knowledge::cards(&dir).len()),
            });
            ProfileRow {
                name,
                file,
                parsed,
                use_,
            }
        })
        .collect();
    View {
        root: root.to_path_buf(),
        profiles,
        proposals: proposals::list(root),
        unreadable: proposals::unreadable(root)
            .into_iter()
            .map(|(file, why)| format!("{}: unreadable: {why}", file.display()))
            .collect(),
        usage_read: usage,
    }
}

// ---- the cache, read off the draw path ----

#[derive(Default)]
struct Entry {
    view: Option<View>,
    waiting: Option<usize>,
    reading: bool,
    read_at: Option<Instant>,
    /// The `+` or the panel drew this folder.
    asked_at: Option<Instant>,
    /// The panel drew this folder: the read is the full one.
    full_at: Option<Instant>,
}

static CHANGED: AtomicBool = AtomicBool::new(false);

fn entries() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    static ENTRIES: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    ENTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn recent(at: Option<Instant>) -> bool {
    at.is_some_and(|at| at.elapsed() < FORGET_AFTER)
}

/// Starts a read of `folder` when none runs and the last is older than `REREAD`; `force` skips the age, for a panel
/// that has no view yet after the `+` read only the count.
fn start_read(folder: &Path, force: bool) {
    let full = {
        let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
        let Some(entry) = entries.get_mut(folder) else {
            return;
        };
        let fresh = entry.read_at.is_some_and(|at| at.elapsed() < REREAD);
        if entry.reading || (fresh && !force) {
            return;
        }
        entry.reading = true;
        recent(entry.full_at)
    };
    let owned = folder.to_path_buf();
    let spawned = crate::thread_spawn::spawn_named("nirvana-profiles", move || {
        let root = super::nirvana_profiles::root(&owned);
        let view = full.then(|| read_view(&root, true));
        let waiting = view
            .as_ref()
            .map_or_else(|| proposals::list(&root).len(), |view| view.proposals.len());
        let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries.entry(owned).or_default();
        entry.reading = false;
        entry.read_at = Some(Instant::now());
        if let Some(view) = view {
            if entry.view.as_ref() != Some(&view) {
                entry.view = Some(view);
                CHANGED.store(true, Ordering::Release);
            }
        }
        if entry.waiting != Some(waiting) {
            entry.waiting = Some(waiting);
            CHANGED.store(true, Ordering::Release);
        }
    });
    if spawned.is_err() {
        if let Some(entry) = entries()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(folder)
        {
            entry.reading = false;
        }
    }
}

/// The waiting count of `folder`'s project as last read, for the `+` while the sidebar draws: a map read. The first
/// ask starts a read; later ones are the timer's. Under test the count is read at once.
pub(super) fn waiting_cached(folder: &Path) -> Option<usize> {
    #[cfg(test)]
    {
        Some(proposals::list(&super::nirvana_profiles::root(folder)).len())
    }
    #[cfg(not(test))]
    {
        let first = {
            let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
            let entry = entries.entry(folder.to_path_buf()).or_default();
            entry.asked_at = Some(Instant::now());
            if let Some(waiting) = entry.waiting {
                return Some(waiting);
            }
            !entry.reading
        };
        if first {
            start_read(folder, false);
        }
        None
    }
}

/// The panel's view of `folder` as last read: a map read while the window draws. Under test it is read at once.
pub(super) fn view_cached(folder: &Path) -> Option<View> {
    #[cfg(test)]
    {
        Some(read_view(&super::nirvana_profiles::root(folder), true))
    }
    #[cfg(not(test))]
    {
        let first = {
            let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
            let entry = entries.entry(folder.to_path_buf()).or_default();
            entry.asked_at = Some(Instant::now());
            entry.full_at = Some(Instant::now());
            if let Some(view) = &entry.view {
                return Some(view.clone());
            }
            !entry.reading
        };
        if first {
            start_read(folder, true);
        }
        None
    }
}

/// After the panel wrote (an Accept, a Dismiss): the profiles and the proposals read again now, on the key's path, so
/// the list never shows a proposal that is gone; the use counts are kept from the last read until the next tick's.
fn refresh_now(folder: &Path, root: &Path) {
    let mut fresh = read_view(root, false);
    let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
    let entry = entries.entry(folder.to_path_buf()).or_default();
    if let Some(old) = &entry.view {
        fresh.usage_read = old.usage_read;
        for row in &mut fresh.profiles {
            row.use_ = old
                .profiles
                .iter()
                .find(|known| known.name == row.name)
                .and_then(|known| known.use_);
        }
    }
    entry.waiting = Some(fresh.proposals.len());
    entry.view = Some(fresh);
    entry.read_at = None;
    CHANGED.store(true, Ordering::Release);
}

/// Starts the reads that are due: every folder drawn in the last `FORGET_AFTER`.
fn poll() {
    let due: Vec<PathBuf> = {
        let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, entry| recent(entry.asked_at));
        entries
            .iter()
            .filter(|(_, entry)| {
                !entry.reading && entry.read_at.is_none_or(|at| at.elapsed() >= REREAD)
            })
            .map(|(folder, _)| folder.clone())
            .collect()
    };
    for folder in due {
        start_read(&folder, false);
    }
}

/// Whether a read changed what the `+` or the panel shows since the last call: the window repaints. Called on the
/// window's timer tick, which is where the due reads start, so the draw path never reads.
pub(crate) fn take_changed() -> bool {
    poll();
    CHANGED.swap(false, Ordering::AcqRel)
}

// ---- the panel ----

/// The panel's two lists; Tab moves between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Profiles,
    Suggested,
}

impl Part {
    fn other(self) -> Self {
        match self {
            Self::Profiles => Self::Suggested,
            Self::Suggested => Self::Profiles,
        }
    }
}

/// The open proposal, by its id, and how far its text is scrolled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Detail {
    pub(super) id: String,
    pub(super) scroll: usize,
}

/// The open Profiles panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfilePanel {
    /// The space's folder, which the cache is keyed by; `root` is its project.
    pub(super) folder: PathBuf,
    pub(super) root: PathBuf,
    pub(super) workspace_id: String,
    pub(super) tab_id: String,
    pub(super) focus: Part,
    /// An index into the profiles, and the name it was on: a re-read keeps the name selected.
    pub(super) profile_selected: usize,
    pub(super) profile_key: Option<String>,
    /// An index into the proposals, and the id it was on.
    pub(super) proposal_selected: usize,
    pub(super) proposal_key: Option<String>,
    /// The first body line shown.
    pub(super) scroll: usize,
    pub(super) detail: Option<Detail>,
    /// `n`: the name being typed.
    pub(super) naming: Option<String>,
    /// After the name: the name and the shape chosen, 0 for none, else `SHAPES[n - 1]`.
    pub(super) shaping: Option<(String, usize)>,
    /// What an Accept, a Dismiss or a New said, kept on screen until the next key; a refusal word for word.
    pub(super) notice: Vec<(String, Tone)>,
    /// A one-line answer in the footer.
    pub(super) message: Option<String>,
}

impl ProfilePanel {
    pub(super) fn new(space: SpaceFolder) -> Self {
        let root = super::nirvana_profiles::root(&space.folder);
        Self {
            folder: space.folder,
            root,
            workspace_id: space.workspace_id,
            tab_id: space.tab_id,
            focus: Part::Profiles,
            profile_selected: 0,
            profile_key: None,
            proposal_selected: 0,
            proposal_key: None,
            scroll: 0,
            detail: None,
            naming: None,
            shaping: None,
            notice: Vec::new(),
            message: None,
        }
    }

    /// Puts each selection back on its name or id in `view`, else holds its index to the rows there are.
    pub(super) fn follow(&mut self, view: &View) {
        let names: Vec<&str> = view.profiles.iter().map(|row| row.name.as_str()).collect();
        let ids: Vec<&str> = view.proposals.iter().map(|row| row.id.as_str()).collect();
        follow_one(&mut self.profile_selected, &mut self.profile_key, &names);
        follow_one(&mut self.proposal_selected, &mut self.proposal_key, &ids);
    }

    /// Selects row `index` of the focused list in `view` and remembers its name or id.
    fn select(&mut self, view: &View, index: usize) {
        match self.focus {
            Part::Profiles => {
                self.profile_selected = index;
                self.profile_key = view.profiles.get(index).map(|row| row.name.clone());
            }
            Part::Suggested => {
                self.proposal_selected = index;
                self.proposal_key = view.proposals.get(index).map(|row| row.id.clone());
            }
        }
    }

    fn selected(&self) -> usize {
        match self.focus {
            Part::Profiles => self.profile_selected,
            Part::Suggested => self.proposal_selected,
        }
    }
}

fn follow_one(selected: &mut usize, key: &mut Option<String>, keys: &[&str]) {
    if let Some(at) = key
        .as_deref()
        .and_then(|known| keys.iter().position(|row| *row == known))
    {
        *selected = at;
    }
    *selected = (*selected).min(keys.len().saturating_sub(1));
    *key = keys.get(*selected).map(|row| (*row).to_owned());
}

/// Where the panel drew its parts, for the mouse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ProfileHits {
    pub(super) area: Rect,
    pub(super) close: Rect,
    /// Each line of each profile, with its index.
    pub(super) profiles: Vec<(Rect, usize)>,
    /// Each line of each proposal, with its index.
    pub(super) proposals: Vec<(Rect, usize)>,
    /// The two headings; a click moves the focus there.
    pub(super) headings: Vec<(Rect, Part)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Plain,
    Muted,
    Heading,
    Attention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Text(String, Tone),
    Heading(Part, String),
    /// A line of a profile or a proposal, by its index; `first` marks its first line, where `▸` goes.
    Row(Part, usize, bool, String, Tone),
}

fn width(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

fn pad(text: &str, cells: usize) -> String {
    let text = fit(text, cells);
    let room = cells.saturating_sub(width(&text));
    format!("{text}{}", " ".repeat(room))
}

/// `text` wrapped to `cells`: each of its lines by words, a word wider than `cells` split by characters, a blank line
/// kept. Never empty, never panics, whatever the text.
pub(super) fn wrap(text: &str, cells: usize) -> Vec<String> {
    let cells = cells.max(1);
    let mut out = Vec::new();
    for source in text.replace("\r\n", "\n").split('\n') {
        let mut line = String::new();
        let mut used = 0;
        for word in source.split(' ') {
            let word_width = width(word);
            if used > 0 && used + 1 + word_width > cells {
                out.push(std::mem::take(&mut line));
                used = 0;
            }
            if used > 0 {
                line.push(' ');
                used += 1;
            }
            if word_width <= cells.saturating_sub(used) {
                line.push_str(word);
                used += word_width;
                continue;
            }
            for character in word.chars() {
                let cell = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
                if used + cell > cells && used > 0 {
                    out.push(std::mem::take(&mut line));
                    used = 0;
                }
                line.push(character);
                used += cell;
            }
        }
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Parts joined by ` · ` and packed into lines of `cells`; a part wider than a line is cut.
pub(super) fn pack(parts: &[String], cells: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for part in parts {
        let part = fit(part, cells);
        if !line.is_empty() && width(&line) + 3 + width(&part) > cells {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push_str(" \u{b7} ");
        }
        line.push_str(&part);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// A profile's use: "used 3 of 35", carried of fit; in words when nothing fits or carries it.
pub(super) fn use_text(row: &ProfileRow, usage_read: bool) -> String {
    if !usage_read {
        return "use: counting".to_owned();
    }
    match row.use_ {
        None => "use: not counted".to_owned(),
        Some((0, 0)) => NO_USE.to_owned(),
        Some((carried, fit)) => format!("used {carried} of {fit}"),
    }
}

/// A parsed profile's facts after its description: the seat pins that are set, its servers, its skills, its cards.
pub(super) fn profile_facts(parsed: &Parsed) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(family) = &parsed.family {
        parts.push(format!("family {family}"));
    }
    if let Some(model) = &parsed.model {
        parts.push(format!("model {model}"));
    }
    if let Some(effort) = &parsed.effort {
        parts.push(format!("effort {effort}"));
    }
    match (parsed.needs.is_empty(), parsed.own_servers) {
        (false, true) => parts.push(format!(
            "servers {} + own mcp.json",
            parsed.needs.join(", ")
        )),
        (false, false) => parts.push(format!("servers {}", parsed.needs.join(", "))),
        (true, true) => parts.push("servers: own mcp.json".to_owned()),
        (true, false) => {}
    }
    match &parsed.skills {
        Some(skills) if skills.is_empty() => parts.push("skills none".to_owned()),
        Some(skills) => parts.push(format!("skills {}", skills.join(", "))),
        None => {}
    }
    if let Some(cards) = parsed.cards {
        parts.push(format!("{cards} card{}", if cards == 1 { "" } else { "s" }));
    }
    parts
}

/// A proposal's evidence on one line.
pub(super) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The list view's lines at `inner` cells (the `▸` gutter of two included).
fn list_lines(view: &View, panel: &ProfilePanel, inner: usize) -> Vec<Line> {
    let text = inner.saturating_sub(2);
    let mut lines = Vec::new();
    for (said, tone) in &panel.notice {
        for line in wrap(said, text) {
            lines.push(Line::Text(line, *tone));
        }
    }
    if let Some((name, at)) = &panel.shaping {
        let mut words = vec![format!("New profile {name}, its shape:")];
        for (place, shape) in std::iter::once("none")
            .chain(crate::nirvana_profiles::SHAPES)
            .enumerate()
        {
            words.push(if place == *at {
                format!("[{place} {shape}]")
            } else {
                format!(" {place} {shape} ")
            });
        }
        for line in wrap(&words.join(" "), text) {
            lines.push(Line::Text(line, Tone::Attention));
        }
    }
    if let Some(name) = &panel.naming {
        lines.push(Line::Text(format!("New profile: {name}_"), Tone::Attention));
    }
    if !lines.is_empty() {
        lines.push(Line::Text(String::new(), Tone::Plain));
    }

    let heading = |part: Part, word: &str, count: usize| {
        let words = format!("{word} \u{b7} {count}");
        if panel.focus == part {
            format!("[{words}]")
        } else {
            format!(" {words} ")
        }
    };
    lines.push(Line::Heading(
        Part::Profiles,
        heading(Part::Profiles, "Profiles", view.profiles.len()),
    ));
    if view.profiles.is_empty() {
        lines.push(Line::Text(format!("  {NO_PROFILES}"), Tone::Muted));
    }
    let wide = inner >= NARROW_PANEL;
    let name_col = view
        .profiles
        .iter()
        .map(|row| width(&row.name))
        .max()
        .unwrap_or(0)
        .min(NAME_COL_MAX);
    for (index, row) in view.profiles.iter().enumerate() {
        let push = |lines: &mut Vec<Line>, first: bool, line: String, tone: Tone| {
            lines.push(Line::Row(Part::Profiles, index, first, line, tone));
        };
        let Some(parsed) = &row.parsed else {
            let refused = format!("refused: herdr profile check {}", row.name);
            if wide {
                push(
                    &mut lines,
                    true,
                    format!("{}  {refused}", pad(&row.name, name_col)),
                    Tone::Attention,
                );
            } else {
                push(&mut lines, true, row.name.clone(), Tone::Attention);
                push(&mut lines, false, format!("  {refused}"), Tone::Attention);
            }
            continue;
        };
        let lead = if wide { name_col + 2 } else { 2 };
        let room = text.saturating_sub(lead);
        let description = if parsed.description.is_empty() {
            "no description".to_owned()
        } else {
            parsed.description.clone()
        };
        let mut said = wrap(&description, room).into_iter();
        if wide {
            let first = said.next().unwrap_or_default();
            push(
                &mut lines,
                true,
                format!("{}  {first}", pad(&row.name, name_col)),
                Tone::Plain,
            );
        } else {
            push(&mut lines, true, row.name.clone(), Tone::Plain);
        }
        for more in said {
            push(
                &mut lines,
                false,
                format!("{}{more}", " ".repeat(lead)),
                Tone::Plain,
            );
        }
        let mut facts = profile_facts(parsed);
        facts.push(use_text(row, view.usage_read));
        for packed in pack(&facts, room) {
            push(
                &mut lines,
                false,
                format!("{}{packed}", " ".repeat(lead)),
                Tone::Muted,
            );
        }
    }

    lines.push(Line::Text(String::new(), Tone::Plain));
    lines.push(Line::Heading(
        Part::Suggested,
        heading(Part::Suggested, "Suggested", view.proposals.len()),
    ));
    if view.proposals.is_empty() {
        lines.push(Line::Text(format!("  {NOTHING_SUGGESTED}"), Tone::Muted));
    }
    for (index, proposal) in view.proposals.iter().enumerate() {
        let head = format!(
            "{} \u{b7} {} \u{b7} {}",
            proposal.id,
            proposal.kind.word(),
            proposal.profile
        );
        let evidence = one_line(&proposal.evidence);
        if wide {
            lines.push(Line::Row(
                Part::Suggested,
                index,
                true,
                fit(&format!("{head} \u{b7} {evidence}"), text),
                Tone::Plain,
            ));
        } else {
            lines.push(Line::Row(Part::Suggested, index, true, head, Tone::Plain));
            lines.push(Line::Row(
                Part::Suggested,
                index,
                false,
                fit(&format!("  {evidence}"), text),
                Tone::Muted,
            ));
        }
    }
    for line in &view.unreadable {
        for wrapped in wrap(line, text) {
            lines.push(Line::Text(wrapped, Tone::Attention));
        }
    }
    lines
}

/// The detail view's lines for `proposal` at `cells`: the whole evidence and the whole proposed text, wrapped.
pub(super) fn detail_lines(proposal: &Proposal, cells: usize) -> Vec<(String, Tone)> {
    let mut lines = vec![(
        match proposal.kind {
            Kind::New => format!("{} \u{b7} new profile {}", proposal.id, proposal.profile),
            Kind::Patch => format!("{} \u{b7} a patch of {}", proposal.id, proposal.profile),
        },
        Tone::Heading,
    )];
    for line in wrap(
        &format!(
            "by {} \u{b7} run {} \u{b7} {}",
            proposal.by, proposal.run, proposal.date
        ),
        cells,
    ) {
        lines.push((line, Tone::Muted));
    }
    lines.push((String::new(), Tone::Plain));
    lines.push(("Evidence".to_owned(), Tone::Heading));
    for line in wrap(&proposal.evidence, cells.saturating_sub(2)) {
        lines.push((format!("  {line}"), Tone::Plain));
    }
    lines.push((String::new(), Tone::Plain));
    let heading = match proposal.kind {
        Kind::New => format!(
            "The file Accept writes, .nirvana/profiles/{}/profile.md",
            proposal.profile
        ),
        Kind::Patch => format!(
            "The new body of {}; Accept replaces its body and keeps its front matter",
            proposal.profile
        ),
    };
    for line in wrap(&heading, cells) {
        lines.push((line, Tone::Heading));
    }
    for line in wrap(&proposal.body, cells.saturating_sub(2)) {
        lines.push((format!("  {line}"), Tone::Plain));
    }
    lines
}

/// The footer's keys for the part in focus, in `cells`: the long form, else a short one that keeps `esc` (at 80
/// columns the panel's inner width is 48 and the long form lost it).
pub(super) fn footer_keys(panel: &ProfilePanel, cells: usize) -> &'static str {
    let long = footer_long(panel);
    if width(long) <= cells {
        return long;
    }
    if panel.naming.is_some() || panel.shaping.is_some() {
        "enter \u{b7} esc cancels"
    } else if panel.detail.is_some() {
        "a accept \u{b7} d dismiss \u{b7} \u{2191}/\u{2193} scroll \u{b7} esc back"
    } else if panel.focus == Part::Profiles {
        "enter/e edit \u{b7} K cards \u{b7} n new \u{b7} tab \u{b7} esc close"
    } else {
        "enter open \u{b7} d dismiss \u{b7} n new \u{b7} tab \u{b7} esc close"
    }
}

fn footer_long(panel: &ProfilePanel) -> &'static str {
    if panel.naming.is_some() {
        "a-z 0-9 - \u{b7} enter next \u{b7} esc cancels"
    } else if panel.shaping.is_some() {
        "\u{2190}/\u{2192} or 0-4 shape \u{b7} enter makes it \u{b7} esc cancels"
    } else if panel.detail.is_some() {
        "a accept \u{b7} d dismiss \u{b7} \u{2191}/\u{2193} pgup/pgdn scroll \u{b7} esc back"
    } else if panel.focus == Part::Profiles {
        "\u{2191}/\u{2193} choose \u{b7} enter/e edit \u{b7} K cards \u{b7} n new \u{b7} tab suggested \u{b7} esc close"
    } else {
        "\u{2191}/\u{2193} choose \u{b7} enter open \u{b7} d dismiss \u{b7} n new \u{b7} tab profiles \u{b7} esc close"
    }
}

/// Draws the panel over `area`, as tall as its content, as the Seats panel is drawn.
pub(super) fn render_profile_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &mut ProfilePanel,
    view: Option<&View>,
    palette: &Palette,
) -> ProfileHits {
    let mut hits = ProfileHits::default();
    let panel_width = area.width.saturating_sub(2).min(PANEL_WIDTH);
    if panel_width < 30 || area.height < 8 {
        return hits;
    }
    let inner = usize::from(panel_width.saturating_sub(4));
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    let tone_style = |tone: Tone| match tone {
        Tone::Plain => base,
        Tone::Muted => base.fg(palette.overlay1),
        Tone::Heading => base.fg(palette.accent).add_modifier(Modifier::BOLD),
        Tone::Attention => base.fg(palette.yellow),
    };
    if let Some(view) = view {
        panel.follow(view);
    }
    let proposal = panel.detail.as_ref().and_then(|detail| {
        view.and_then(|view| view.proposals.iter().find(|known| known.id == detail.id))
    });
    let folder = folder_name(&panel.root);
    let (title, body): (String, Vec<Line>) = match (view, &panel.detail) {
        (None, _) => (
            format!(" Profiles \u{b7} {folder} "),
            vec![Line::Text(
                "reading this project's profiles".into(),
                Tone::Muted,
            )],
        ),
        (Some(_), Some(detail)) => {
            let mut body: Vec<Line> = panel
                .notice
                .iter()
                .flat_map(|(said, tone)| {
                    wrap(said, inner)
                        .into_iter()
                        .map(|line| Line::Text(line, *tone))
                })
                .collect();
            if !body.is_empty() {
                body.push(Line::Text(String::new(), Tone::Plain));
            }
            match proposal {
                Some(proposal) => body.extend(
                    detail_lines(proposal, inner)
                        .into_iter()
                        .map(|(line, tone)| Line::Text(line, tone)),
                ),
                None => body.push(Line::Text(
                    format!(
                        "{} is no longer waiting: esc goes back to the list",
                        detail.id
                    ),
                    Tone::Attention,
                )),
            }
            (
                format!(" Profiles \u{b7} {folder} \u{b7} {} ", detail.id),
                body,
            )
        }
        (Some(view), None) => (
            format!(" Profiles \u{b7} {folder} "),
            list_lines(view, panel, inner),
        ),
    };

    // The frame holds the body and two more lines: a blank and the keys.
    let height = (body.len() as u16 + 4).min(area.height.saturating_sub(1));
    let rect = Rect::new(area.x + 1, area.y + 1, panel_width, height);
    hits.area = rect;
    hits.close = draw_frame(buffer, rect, &title, palette);
    let x = rect.x + 2;
    let body_height = usize::from(rect.height.saturating_sub(4));
    let most = body.len().saturating_sub(body_height);
    let scroll = if let Some(detail) = panel.detail.as_mut() {
        detail.scroll = detail.scroll.min(most);
        detail.scroll
    } else {
        // Keep the selected row's first line in view, and as much of the row as fits under it.
        let selected = panel.selected();
        let rows: Vec<usize> = body
            .iter()
            .enumerate()
            .filter(|(_, line)| matches!(line, Line::Row(part, index, ..) if *part == panel.focus && *index == selected))
            .map(|(at, _)| at)
            .collect();
        if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
            if *first < panel.scroll {
                panel.scroll = *first;
            } else if body_height > 0 && *last >= panel.scroll + body_height {
                panel.scroll = (*last + 1 - body_height).min(*first);
            }
        }
        panel.scroll = panel.scroll.min(most);
        panel.scroll
    };
    let width_cells = inner as u16;
    for (row, line) in body.iter().skip(scroll).take(body_height).enumerate() {
        let y = rect.y + 1 + row as u16;
        match line {
            Line::Text(text, tone) => {
                put_text(
                    buffer,
                    x,
                    y,
                    width_cells,
                    &fit(text, inner),
                    tone_style(*tone),
                );
            }
            Line::Heading(part, text) => {
                let style = if *part == panel.focus {
                    tone_style(Tone::Heading)
                } else {
                    tone_style(Tone::Muted).add_modifier(Modifier::BOLD)
                };
                let shown = fit(text, inner);
                put_text(buffer, x, y, width_cells, &shown, style);
                hits.headings
                    .push((Rect::new(x, y, width(&shown) as u16, 1), *part));
            }
            Line::Row(part, index, first, text, tone) => {
                let selected = *part == panel.focus
                    && *index
                        == match part {
                            Part::Profiles => panel.profile_selected,
                            Part::Suggested => panel.proposal_selected,
                        };
                let row_rect = Rect::new(x, y, width_cells, 1);
                let mut style = tone_style(*tone);
                if selected {
                    style = style.bg(palette.selection_bg);
                    buffer.set_style(row_rect, Style::default().bg(palette.selection_bg));
                }
                // The selected row reads without colour too: `▸` in the gutter of its first line.
                let gutter = if selected && *first {
                    "\u{25b8} "
                } else {
                    "  "
                };
                put_text(buffer, x, y, 2, gutter, tone_style(Tone::Heading));
                put_text(
                    buffer,
                    x + 2,
                    y,
                    width_cells.saturating_sub(2),
                    &fit(text, inner.saturating_sub(2)),
                    style,
                );
                match part {
                    Part::Profiles => hits.profiles.push((row_rect, *index)),
                    Part::Suggested => hits.proposals.push((row_rect, *index)),
                }
            }
        }
    }
    let foot_y = rect.bottom().saturating_sub(2);
    let (foot, style) = match &panel.message {
        Some(message) => (message.as_str(), base.fg(palette.yellow)),
        None => (footer_keys(panel, inner), base.fg(palette.overlay1)),
    };
    put_text(buffer, x, foot_y, width_cells, &fit(foot, inner), style);
    hits
}

// ---- keys, clicks and what they do ----

impl ClientShellState {
    /// Opens the Profiles panel on `space`'s project. One panel is open at a time, so the strip's close.
    pub(super) fn open_profile_panel(&mut self, space: SpaceFolder) {
        self.nirvana_preload = None;
        self.nirvana_running = None;
        self.nirvana_memory = None;
        self.nirvana_accounts = None;
        self.nirvana_seats = None;
        self.nirvana_jobs = None;
        self.nirvana_git = None;
        self.nirvana_learning = None;
        self.nirvana_profile_panel = Some(ProfilePanel::new(space));
    }

    /// The `+`: opens the panel on the focused space's project, or closes it when it is open on that folder.
    pub(super) fn toggle_profile_panel(&mut self) {
        let Some(space) = self.snapshot.as_deref().and_then(focused_space_folder) else {
            return;
        };
        if self
            .nirvana_profile_panel
            .as_ref()
            .is_some_and(|panel| panel.folder == space.folder)
        {
            self.nirvana_profile_panel = None;
            return;
        }
        self.open_profile_panel(space);
    }

    /// The space menu's `Profiles`: the panel on that space's project.
    pub(super) fn open_space_profile_panel(&mut self, workspace_id: &str) {
        if let Some(space) = self
            .snapshot
            .as_deref()
            .and_then(|snapshot| space_folder(snapshot, workspace_id))
        {
            self.open_profile_panel(space);
        }
    }

    fn profile_view(&self) -> Option<View> {
        self.nirvana_profile_panel
            .as_ref()
            .and_then(|panel| view_cached(&panel.folder))
    }

    fn profile_say(&mut self, message: impl Into<String>) {
        if let Some(panel) = self.nirvana_profile_panel.as_mut() {
            panel.message = Some(message.into());
        }
    }

    fn profile_notice(&mut self, notice: Vec<(String, Tone)>) {
        if let Some(panel) = self.nirvana_profile_panel.as_mut() {
            panel.notice = notice;
        }
    }

    /// Opens `file` or a folder in an edit tab beside the space's tab, through `command.invoke` as the Preload
    /// group's `e` and `K` do, and closes the panel: the editor is what the owner sees.
    fn profile_edit_tab(&mut self, prefix: &str, path: &Path, outcome: &mut ClientShellInput) {
        let Some(panel) = self.nirvana_profile_panel.take() else {
            return;
        };
        self.push_endpoint_method(
            crate::api::schema::Method::CommandInvoke(crate::api::schema::CommandInvokeParams {
                command_id: format!("{prefix}{}", path.display()),
                workspace_id: Some(panel.workspace_id),
                tab_id: Some(panel.tab_id),
                pane_id: None,
                selection: None,
            }),
            outcome,
        );
    }

    fn profile_edit(&mut self, view: &View, index: usize, outcome: &mut ClientShellInput) {
        let Some(row) = view.profiles.get(index) else {
            self.profile_say("no profile to edit: n makes one");
            return;
        };
        let file = row.file.clone();
        self.profile_edit_tab(crate::nirvana::EDIT_FILE_COMMAND, &file, outcome);
    }

    /// `K`: a folder profile's `knowledge/`, made when it is not there yet so the editor has a folder to start in.
    fn profile_cards(&mut self, view: &View, index: usize, outcome: &mut ClientShellInput) {
        let Some(row) = view.profiles.get(index) else {
            self.profile_say("no profile selected");
            return;
        };
        let name = row.name.clone();
        match crate::nirvana_profiles::knowledge_dir(&view.root, &name) {
            Some(dir) => match std::fs::create_dir_all(&dir) {
                Ok(()) => self.profile_edit_tab(crate::nirvana::EDIT_DIR_COMMAND, &dir, outcome),
                Err(err) => self.profile_say(format!("cannot make {}: {err}", dir.display())),
            },
            None => self.profile_say(format!(
                "{name} keeps no knowledge cards: only a folder profile does, .nirvana/profiles/{name}/knowledge/"
            )),
        }
    }

    /// Accept, from the detail only: the library's line and `ACCEPT_NOTE` on screen, or its refusal word for word.
    fn profile_accept(&mut self, id: &str) {
        let Some((folder, root)) = self
            .nirvana_profile_panel
            .as_ref()
            .map(|panel| (panel.folder.clone(), panel.root.clone()))
        else {
            return;
        };
        let result = proposals::accept(&root, id);
        refresh_now(&folder, &root);
        match result {
            Ok(line) => {
                if let Some(panel) = self.nirvana_profile_panel.as_mut() {
                    panel.detail = None;
                }
                self.profile_notice(vec![
                    (line, Tone::Plain),
                    (proposals::ACCEPT_NOTE.to_owned(), Tone::Attention),
                ]);
            }
            Err(why) => self.profile_notice(vec![(why, Tone::Attention)]),
        }
    }

    fn profile_dismiss(&mut self, id: &str) {
        let Some((folder, root)) = self
            .nirvana_profile_panel
            .as_ref()
            .map(|panel| (panel.folder.clone(), panel.root.clone()))
        else {
            return;
        };
        let result = proposals::dismiss(&root, id, None);
        refresh_now(&folder, &root);
        match result {
            Ok(line) => {
                if let Some(panel) = self.nirvana_profile_panel.as_mut() {
                    panel.detail = None;
                }
                self.profile_notice(vec![(line, Tone::Plain)]);
            }
            Err(why) => self.profile_notice(vec![(why, Tone::Attention)]),
        }
    }

    /// `n`'s last step: the profile written through `new_profile` or `new_profile_shape`, and opened.
    fn profile_make(&mut self, name: &str, shape: usize, outcome: &mut ClientShellInput) {
        let Some(root) = self
            .nirvana_profile_panel
            .as_ref()
            .map(|panel| panel.root.clone())
        else {
            return;
        };
        let made = match shape.checked_sub(1) {
            None => crate::nirvana_profiles::new_profile(&root, name),
            Some(at) => match crate::nirvana_profiles::SHAPES.get(at) {
                Some(shape) => crate::nirvana_profiles::new_profile_shape(&root, name, shape),
                None => Err(format!("no shape {shape}")),
            },
        };
        match made {
            Ok(file) => self.profile_edit_tab(crate::nirvana::EDIT_FILE_COMMAND, &file, outcome),
            Err(why) => self.profile_notice(vec![(format!("not made: {why}"), Tone::Attention)]),
        }
    }

    /// The panel's keys. Every key goes to the panel while it is open.
    pub(super) fn route_profile_panel_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        outcome.repaint = true;
        let view = self.profile_view();
        let Some(panel) = self.nirvana_profile_panel.as_mut() else {
            return;
        };
        panel.message = None;
        // What an Accept, a Dismiss or a New said stays until this key.
        panel.notice.clear();
        let code = key.code;
        // `n`: every key is the name's until enter or esc, then the shape's.
        if let Some(name) = panel.naming.as_mut() {
            match super::nirvana_profiles::naming_key(name, code) {
                super::nirvana_profiles::Naming::Typing => {}
                super::nirvana_profiles::Naming::Cancel => panel.naming = None,
                super::nirvana_profiles::Naming::Make(name) => {
                    panel.naming = None;
                    match crate::nirvana_profiles::new_name_problem(&name) {
                        Some(why) => {
                            panel.notice = vec![(format!("not made: {why}"), Tone::Attention)]
                        }
                        None => panel.shaping = Some((name, 0)),
                    }
                }
            }
            return;
        }
        if let Some((name, at)) = panel.shaping.as_mut() {
            let count = crate::nirvana_profiles::SHAPES.len() + 1;
            match code {
                KeyCode::Esc => panel.shaping = None,
                KeyCode::Left
                | KeyCode::Up
                | KeyCode::Char('h')
                | KeyCode::Char('k')
                | KeyCode::BackTab => {
                    *at = (*at + count - 1) % count;
                }
                KeyCode::Right
                | KeyCode::Down
                | KeyCode::Char('l')
                | KeyCode::Char('j')
                | KeyCode::Tab => {
                    *at = (*at + 1) % count;
                }
                KeyCode::Char(digit) if digit.is_ascii_digit() => {
                    let chosen = usize::from(digit as u8 - b'0');
                    if chosen < count {
                        *at = chosen;
                    }
                }
                KeyCode::Enter => {
                    let (name, shape) = (name.clone(), *at);
                    panel.shaping = None;
                    self.profile_make(&name, shape, outcome);
                }
                _ => {}
            }
            return;
        }
        let Some(view) = view else {
            if matches!(code, KeyCode::Esc | KeyCode::Char('q')) {
                self.nirvana_profile_panel = None;
            }
            return;
        };
        panel.follow(&view);
        if let Some(detail) = panel.detail.as_mut() {
            match code {
                KeyCode::Esc | KeyCode::Char('q') => panel.detail = None,
                KeyCode::Up | KeyCode::Char('k') => detail.scroll = detail.scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    detail.scroll = detail.scroll.saturating_add(1)
                }
                KeyCode::PageUp => detail.scroll = detail.scroll.saturating_sub(PAGE),
                KeyCode::PageDown => detail.scroll = detail.scroll.saturating_add(PAGE),
                KeyCode::Home => detail.scroll = 0,
                // Held to the text at the next draw.
                KeyCode::End => detail.scroll = usize::MAX,
                KeyCode::Char('a') => {
                    let id = detail.id.clone();
                    self.profile_accept(&id);
                }
                KeyCode::Char('d') => {
                    let id = detail.id.clone();
                    self.profile_dismiss(&id);
                }
                _ => {}
            }
            return;
        }
        let count = match panel.focus {
            Part::Profiles => view.profiles.len(),
            Part::Suggested => view.proposals.len(),
        };
        let last = count.saturating_sub(1);
        let selected = panel.selected();
        let moved = match code {
            KeyCode::Up | KeyCode::Char('k') => Some(selected.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => Some((selected + 1).min(last)),
            KeyCode::PageUp => Some(selected.saturating_sub(PAGE)),
            KeyCode::PageDown => Some((selected + PAGE).min(last)),
            KeyCode::Home => Some(0),
            KeyCode::End => Some(last),
            _ => None,
        };
        if let Some(next) = moved {
            panel.select(&view, next);
            return;
        }
        match (code, panel.focus) {
            (KeyCode::Esc | KeyCode::Char('q'), _) => self.nirvana_profile_panel = None,
            (KeyCode::Tab | KeyCode::BackTab, _) => panel.focus = panel.focus.other(),
            (KeyCode::Char('n'), _) => panel.naming = Some(String::new()),
            (KeyCode::Enter | KeyCode::Char('e'), Part::Profiles) => {
                self.profile_edit(&view, selected, outcome);
            }
            (KeyCode::Char('K'), Part::Profiles) => self.profile_cards(&view, selected, outcome),
            (KeyCode::Enter, Part::Suggested) => match view.proposals.get(selected) {
                Some(proposal) => {
                    panel.detail = Some(Detail {
                        id: proposal.id.clone(),
                        scroll: 0,
                    });
                }
                None => panel.message = Some(NOTHING_SUGGESTED.to_owned()),
            },
            (KeyCode::Char('d'), Part::Suggested) => match view.proposals.get(selected) {
                Some(proposal) => {
                    let id = proposal.id.clone();
                    self.profile_dismiss(&id);
                }
                None => panel.message = Some(NOTHING_SUGGESTED.to_owned()),
            },
            (KeyCode::Char('a'), _) => panel.message = Some(ACCEPT_IN_DETAIL.to_owned()),
            (KeyCode::Char('e' | 'K'), Part::Suggested) => {
                panel.message = Some("e and K act on a profile: tab moves to Profiles".to_owned());
            }
            _ => {}
        }
    }

    /// The panel's mouse. While it is open every event is the panel's.
    pub(super) fn handle_profile_panel_mouse(
        &mut self,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if self.nirvana_profile_panel.is_none() {
            return false;
        }
        let point = (mouse.column, mouse.row);
        let hits = self.hits.nirvana_profile_panel.clone();
        let wheel = match mouse.kind {
            MouseEventKind::ScrollUp => Some(false),
            MouseEventKind::ScrollDown => Some(true),
            _ => None,
        };
        if let Some(down) = wheel {
            let view = self.profile_view();
            let Some(panel) = self.nirvana_profile_panel.as_mut() else {
                return true;
            };
            if let Some(detail) = panel.detail.as_mut() {
                detail.scroll = if down {
                    detail.scroll.saturating_add(WHEEL_STEP)
                } else {
                    detail.scroll.saturating_sub(WHEEL_STEP)
                };
            } else if let Some(view) = view {
                panel.follow(&view);
                let count = match panel.focus {
                    Part::Profiles => view.profiles.len(),
                    Part::Suggested => view.proposals.len(),
                };
                let selected = panel.selected();
                let next = if down {
                    (selected + WHEEL_STEP).min(count.saturating_sub(1))
                } else {
                    selected.saturating_sub(WHEEL_STEP)
                };
                panel.select(&view, next);
            }
            outcome.repaint = true;
            return true;
        }
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return true;
        }
        outcome.repaint = true;
        if !super::contains(hits.area, point) || super::contains(hits.close, point) {
            self.nirvana_profile_panel = None;
            return true;
        }
        let view = self.profile_view();
        let Some(panel) = self.nirvana_profile_panel.as_mut() else {
            return true;
        };
        panel.message = None;
        if let Some((_, part)) = hits
            .headings
            .iter()
            .find(|(rect, _)| super::contains(*rect, point))
        {
            panel.focus = *part;
            return true;
        }
        let row = hits
            .profiles
            .iter()
            .find(|(rect, _)| super::contains(*rect, point))
            .map(|(_, index)| (Part::Profiles, *index))
            .or_else(|| {
                hits.proposals
                    .iter()
                    .find(|(rect, _)| super::contains(*rect, point))
                    .map(|(_, index)| (Part::Suggested, *index))
            });
        if let (Some((part, index)), Some(view)) = (row, view) {
            panel.focus = part;
            panel.select(&view, index);
        }
        true
    }
}
