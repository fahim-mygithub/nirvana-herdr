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
//! rule (`super::nirvana_profiles::root`, the nearest folder holding `.git`, else the folder itself). Its full path is
//! the line under the title, shortened in the middle when it does not fit. Three parts:
//!
//! - **Profiles**, one row each: description, seat pins, servers, skills, cards, and the use the detector counts, "used
//!   0 of 35", with "(partial)" when its scan was cut short. `e` or Enter opens the file in an edit tab, `K` its
//!   `knowledge/`. Plan 47 P47b: a built-in (`scholar`) the project does not override is a row marked "built into
//!   herdr"; it has no file, so `e` or Enter says so and names the copy, and the same key on it again writes the
//!   project's own copy (`nirvana_profiles::new_profile_from`) off the key path and opens it.
//! - **Suggested**, the proposals waiting (plan 45 P45b). Enter opens one whole; Accept is only there. `d` dismisses,
//!   in the list (the row highlighted when the list was last drawn) or the detail.
//! - **New**, `n`: a name, then a shape from `crate::nirvana_profiles::SHAPES`. It is made off the key path, and the
//!   panel then names the full path it wrote. Esc closes the panel while it is made; the next panel opened on that
//!   project says what came of it (made, with the profile selected, or refused), or that it is still making it.
//!
//! **What Accept requires** (P45a fix rounds; a proposal's body is drafted from order text and worker streams, which
//! are untrusted, and the owner's Accept is the only guard). Accept applies only text the owner had on screen, all of
//! it. `a` acts only in a proposal's detail, only as a bare key (no modifier but Shift), and only when all of these
//! hold:
//!
//! - the detail of that proposal has been drawn at least once since it was opened;
//! - the panel was drawn whole in the last frame (a window too small for it shows "window too small" and the panel
//!   takes no key but Esc);
//! - every line of the detail has been on screen since it was opened, with no gap from its top (`Detail::covered`).
//!   No scroll opens a gap: End, PageDown, Down and the wheel move at most to the first line not yet shown, so a long
//!   detail is paged through. The line above the keys counts the lines not yet shown, and `a` says "scroll to the end
//!   to accept (N more lines)" until there are none. A detail that fits needs no scroll. A resize that changes the
//!   panel's width, or a re-read that changes the detail's line count, starts the count again from the top; one that
//!   changes only its height keeps it, since the same width lays out the same lines.
//!
//! `d` in the detail acts once the detail has been drawn, by the same rule (it need not be read to its end). Every
//! name the panel draws from disk (profile names, proposal ids and target profiles, the project's folder and path) is
//! drawn through `visible`, as the text is.
//!
//! What the detail shows under "The file Accept writes" is `nirvana_profile_proposals::target` of the proposal
//! `prepared` (redacted), the very text `accept` writes. Accept sends what was drawn (`proposals::Seen`: the proposal
//! file's sha and that target) to `accept_seen`, which refuses inside the proposals' lock when the file on disk or the
//! profile it writes is no longer that; the panel then says so, reads again and opens the detail at the top. A
//! character the screen would drop is drawn as `<U+XXXX>`, and the store refuses a proposal holding one. Accept,
//! Dismiss and New never wait on the proposals' lock: Accept and Dismiss take it with `try_lock` and say "busy, press
//! again"; New runs on a thread of its own and the panel says "making" until it is done.
//!
//! **Nothing here reads a file while the window draws.** The data is read by a `Reader` on a thread of its own,
//! started from the window's timer tick (`take_changed`) at most every `REREAD`, the Seats square's interval, and only
//! after the last read ended (single flight). The `+` asks only for the waiting count; the full read (profiles, the use
//! scan, proposals) runs only while the panel is drawn. A read that panics leaves no folder "reading" for good, and a
//! read that started before an Accept or a Dismiss is dropped when it ends (a generation number), so the fresher view
//! the key path read stands.
//!
//! **No worker can press Accept.** The panel's keys and clicks come only from this window's own terminal input
//! (`handle_raw_events`); no API method, socket message or `herdr` verb reaches them, and Accept and Dismiss call
//! `crate::nirvana_profile_proposals` directly, not `herdr profile accept`. The edit tabs go through `command.invoke`,
//! as the Preload group's do; they open a file and accept nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use super::nirvana_mods::{draw_frame, folder_name};
use super::nirvana_spaces::fit;
use super::render::put_text;
use super::*;
use crate::nirvana_profile_proposals::{self as proposals, Kind, Proposal, Refused, Seen};
use crate::protocol::ClientShellPane;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::Modifier;

/// The panel's widest; under it, the window's pane area less a cell each side.
pub(super) const PANEL_WIDTH: u16 = 120;
/// The panel's narrowest and shortest: under these it is not drawn, says "window too small", and takes no key but Esc.
pub(super) const PANEL_MIN_WIDTH: u16 = 30;
pub(super) const PANEL_MIN_HEIGHT: u16 = 9;
/// Under this inner width a profile's name takes a line of its own and its description wraps under it.
pub(super) const NARROW_PANEL: usize = 72;
/// The name column of the wide form: as wide as the longest name, at most this.
const NAME_COL_MAX: usize = 24;
/// Rows PageUp and PageDown move, as the Preload panel's do.
const PAGE: usize = 10;
/// Rows a wheel notch moves, as the Seats table's does.
const WHEEL_STEP: usize = 3;
/// A tab in a proposal is drawn as this many spaces (ratatui drops it).
const TAB: &str = "    ";
/// The Seats square's re-read interval: the brief's rule, "the interval the Seats panel uses".
const REREAD: Duration = crate::nirvana_seats_square::REREAD;
/// A folder the window has not drawn for this long is no longer read, as the Seats square forgets a project.
const FORGET_AFTER: Duration = Duration::from_secs(30);
/// Review 2 F7: the use scan (`detect_with`) is most of a full read (225 of 240 ms on 60 runs of 34 orders), and the
/// counts move only when a run ends, so a drawn panel scans at most this often; profiles and proposals keep `REREAD`.
/// The panel opening, an Accept, a Dismiss and a New ask for a scan at the next read whatever the age.
pub(super) const USAGE_EVERY: Duration = Duration::from_secs(30);

/// The empty states, in plain words.
pub(super) const NO_PROFILES: &str = "No profiles yet: n makes one, or accept a suggestion below";
pub(super) const NOTHING_SUGGESTED: &str =
    "Nothing suggested: herdr proposes a profile when it sees a role retyped across orders";
/// `a` in the list: Accept is only in the detail, where the body is on screen.
pub(super) const ACCEPT_IN_DETAIL: &str =
    "a accepts only in a proposal's detail, with its body on screen: enter opens it";
/// `a` before the detail was drawn.
pub(super) const ACCEPT_NOT_DRAWN: &str =
    "a accepts once this proposal has been drawn on screen: press a again";
/// Any key but Esc while the panel was not drawn whole in the last frame.
pub(super) const TOO_SMALL: &str = "window too small for the Profiles panel: esc closes it";
/// Review 2 F2: `d` in a detail not drawn yet.
pub(super) const DISMISS_DETAIL_NOT_DRAWN: &str =
    "d dismisses once this proposal has been drawn on screen: press d again";
/// `d` in the list when no row of Suggested was highlighted on screen since the last move or dismiss.
pub(super) const DISMISS_NOT_DRAWN: &str =
    "d dismisses the highlighted row on screen: nothing was dismissed, look and press d again";
/// The use of a profile no order fits or carries.
pub(super) const NO_USE: &str = "no order fits or carries it";
/// Plan 47 P47b: the mark on a built-in's row.
pub(super) const BUILT_IN: &str = "built into herdr";
/// The same, when the scan was cut short.
pub(super) const NO_USE_PARTIAL: &str = "none seen (partial)";

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
    /// The detector's carried and fit counts; `None` when it has no row for it.
    pub(super) use_: Option<(usize, usize)>,
    /// Plan 47 P47a (L1): how many Running orders of this project carry it now (`nirvana_subagents::live_profiles`).
    pub(super) live: usize,
    /// Plan 47 P47b: herdr's built-in, which the project has no file for; `file` is then no path on disk.
    pub(super) builtin: bool,
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
    /// What the detail shows of each proposal, by id, and what its Accept is held to: read with it, from one text.
    pub(super) seen: HashMap<String, Seen>,
    /// A proposal file or a ledger line herdr cannot read, as `herdr profile proposals` names them.
    pub(super) unreadable: Vec<String>,
    /// Whether the use counts were read; the first read after an Accept or a New keeps the last ones.
    pub(super) usage_read: bool,
    /// The scan behind the use counts was cut short by one of its limits (`partial`): the counts are a floor.
    pub(super) usage_partial: bool,
}

/// Reads the project at `root`: the profiles, the proposals waiting and, with `usage`, each profile's use.
pub(super) fn read_view(root: &Path, usage: bool) -> View {
    // Review finding 9: `detect_with` says what its scan skipped, which `usage` drops.
    let counted = usage.then(|| {
        crate::nirvana_profile_detect::detect_with(
            root,
            &crate::nirvana_profile_detect::Limits::default(),
        )
    });
    // Plan 47 P47a (L1): the Running orders of this project that carry each profile, from the rows the agents panel's
    // poller already holds (an orchestrator pane counts when its folder's project is this one, by the panel's own
    // rule). Only what the agents panel or the spaces view asked for in the last minute is there: see `live_profiles`.
    let live = crate::nirvana_subagents::live_profiles(|folder| {
        super::nirvana_profiles::root(folder) == root
    });
    // Plan 47 P47b: the built-ins the project does not override are rows too.
    let profiles = crate::nirvana_profiles::listed(root)
        .into_iter()
        .map(|found| {
            let (name, file, parsed, builtin) = (found.name, found.file, found.parsed, found.builtin);
            let use_ = counted.as_ref().and_then(|detection| {
                detection
                    .usage
                    .iter()
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
                cards: match crate::nirvana_profiles::builtin(&name).filter(|_| builtin) {
                    Some(found) => Some(found.cards.len()),
                    None => crate::nirvana_profiles::knowledge_dir(root, &name)
                        .map(|dir| crate::nirvana_knowledge::cards(&dir).len()),
                },
            });
            ProfileRow {
                live: live.get(&name).copied().unwrap_or(0),
                name,
                file,
                parsed,
                use_,
                builtin,
            }
        })
        .collect();
    let (proposals, seen): (Vec<Proposal>, HashMap<String, Seen>) = proposals::list_seen(root)
        .into_iter()
        .map(|(proposal, seen)| {
            let id = proposal.id.clone();
            (proposal, (id, seen))
        })
        .unzip();
    View {
        root: root.to_path_buf(),
        profiles,
        proposals,
        seen,
        unreadable: proposals::unreadable(root)
            .into_iter()
            .map(|(file, why)| format!("{}: unreadable: {why}", file.display()))
            .collect(),
        usage_read: usage,
        usage_partial: counted
            .as_ref()
            .is_some_and(|detection| partial(&detection.skipped)),
    }
}

/// Review 2 F8: whether a limit cut the scan the use counts come from, so they are a floor. The counts come from the
/// journals' `seated` lines, so only what left a `seated` line uncounted makes them partial: runs past the run cap
/// (`runs`), a journal over its byte cap or unread (`journals`: its orders are not counted at all), orders past the
/// order cap or left when the budget ran out (`orders`), and the time budget (`budget`). An order copy that is missing,
/// over its byte cap or elsewhere (`unreadable`, `files`, `elsewhere`) leaves its order counted from the journal.
pub(super) fn partial(skipped: &crate::nirvana_profile_detect::Skipped) -> bool {
    skipped.runs > 0 || skipped.journals > 0 || skipped.orders > 0 || skipped.budget
}

// ---- the reader, off the draw path ----

/// One read's result: the full view when the panel is drawn, else only the waiting count.
pub(super) struct Read {
    pub(super) view: Option<View>,
    pub(super) waiting: usize,
}

/// What one read is asked for: the panel's whole view (`full`) or the waiting count only, and with the view the use
/// scan (`usage`) or not, in which case the last counts are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Want {
    pub(super) full: bool,
    pub(super) usage: bool,
}

type ReadFn = dyn Fn(&Path, Want) -> Read + Send + Sync;

#[derive(Default)]
struct Entry {
    view: Option<View>,
    waiting: Option<usize>,
    reading: bool,
    read_at: Option<Instant>,
    /// The `+` or the panel asked for this folder.
    asked_at: Option<Instant>,
    /// Bumped by `refresh`: a read started before it is dropped when it ends.
    generation: u64,
    /// When the last use scan landed, and whether one is asked for at the next full read whatever its age.
    usage_at: Option<Instant>,
    usage_due: bool,
}

/// The folders' reads: one at a time each, on a thread of its own. The window has one (`reader`); a test builds its
/// own with a read of its choosing (review finding 10: the seam).
pub(super) struct Reader {
    entries: Mutex<HashMap<PathBuf, Entry>>,
    changed: AtomicBool,
    /// The folder of the panel drawn in the last frame, set from the window's tick: only its reads are full.
    showing: Mutex<Option<PathBuf>>,
    read: Box<ReadFn>,
    usage_every: Duration,
}

impl Reader {
    pub(super) fn new(read: impl Fn(&Path, Want) -> Read + Send + Sync + 'static) -> Arc<Self> {
        Self::scanning_every(USAGE_EVERY, read)
    }

    /// A reader whose use scan runs at most every `usage_every`: the window's is `USAGE_EVERY`, a test's is short.
    pub(super) fn scanning_every(
        usage_every: Duration,
        read: impl Fn(&Path, Want) -> Read + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            entries: Mutex::new(HashMap::new()),
            changed: AtomicBool::new(false),
            showing: Mutex::new(None),
            read: Box::new(read),
            usage_every,
        })
    }

    fn entries(&self) -> MutexGuard<'_, HashMap<PathBuf, Entry>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The folder whose panel is drawn now, or none: the full read (with the use scan) is only that folder's.
    pub(super) fn show(&self, folder: Option<PathBuf>) {
        *self.showing.lock().unwrap_or_else(|e| e.into_inner()) = folder;
    }

    fn showing(&self, folder: &Path) -> bool {
        self.showing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref()
            == Some(folder)
    }

    /// Starts a read of `folder` when none runs and the last is older than `REREAD`; `force` skips the age and makes
    /// the read full, for a panel that has no view yet.
    fn start(self: &Arc<Self>, folder: &Path, force: bool) {
        let showing = self.showing(folder);
        let (want, generation) = {
            let mut entries = self.entries();
            let Some(entry) = entries.get_mut(folder) else {
                return;
            };
            let fresh = entry.read_at.is_some_and(|at| at.elapsed() < REREAD);
            if entry.reading || (fresh && !force) {
                return;
            }
            entry.reading = true;
            let full = force || showing;
            let usage = full
                && (entry.usage_due
                    || entry
                        .usage_at
                        .is_none_or(|at| at.elapsed() >= self.usage_every));
            if usage {
                entry.usage_due = false;
            }
            (Want { full, usage }, entry.generation)
        };
        let reader = Arc::clone(self);
        let owned = folder.to_path_buf();
        let spawned = crate::thread_spawn::spawn_named("nirvana-profiles", move || {
            // Review finding 10: a read that panics must not leave the folder "reading" for good.
            let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (reader.read)(&owned, want)
            }));
            reader.finish(&owned, generation, want, read.ok());
        });
        if spawned.is_err() {
            if let Some(entry) = self.entries().get_mut(folder) {
                entry.reading = false;
                entry.usage_due |= want.usage;
            }
        }
    }

    /// A read ended: kept when no `refresh` came since it started, dropped when one did (the key path's view is
    /// fresher), and a panicked one (`None`) is tried again after `REREAD`. A use scan that did not land is asked for
    /// again; a view read without one keeps the last counts.
    fn finish(&self, folder: &Path, generation: u64, want: Want, read: Option<Read>) {
        let mut entries = self.entries();
        let entry = entries.entry(folder.to_path_buf()).or_default();
        entry.reading = false;
        if entry.generation != generation {
            entry.read_at = None;
            entry.usage_due |= want.usage;
            return;
        }
        entry.read_at = Some(Instant::now());
        let Some(read) = read else {
            entry.usage_due |= want.usage;
            return;
        };
        if let Some(mut view) = read.view {
            if view.usage_read {
                entry.usage_at = Some(Instant::now());
            } else if let Some(old) = &entry.view {
                keep_usage(&mut view, old);
            }
            if entry.view.as_ref() != Some(&view) {
                entry.view = Some(view);
                self.changed.store(true, Ordering::Release);
            }
        }
        if entry.waiting != Some(read.waiting) {
            entry.waiting = Some(read.waiting);
            self.changed.store(true, Ordering::Release);
        }
    }

    /// The waiting count of `folder` as last read, for the `+`: a map read. With none yet a read starts, unless one
    /// runs: `start` alone keeps the reads single, so one test and one control prove it.
    pub(super) fn waiting(self: &Arc<Self>, folder: &Path) -> Option<usize> {
        {
            let mut entries = self.entries();
            let entry = entries.entry(folder.to_path_buf()).or_default();
            entry.asked_at = Some(Instant::now());
            if let Some(waiting) = entry.waiting {
                return Some(waiting);
            }
        }
        self.start(folder, false);
        None
    }

    /// The panel's view of `folder` as last read: a map read. With none yet a full read starts, unless one runs.
    pub(super) fn view(self: &Arc<Self>, folder: &Path) -> Option<View> {
        {
            let mut entries = self.entries();
            let entry = entries.entry(folder.to_path_buf()).or_default();
            entry.asked_at = Some(Instant::now());
            if let Some(view) = &entry.view {
                return Some(view.clone());
            }
        }
        self.start(folder, true);
        None
    }

    /// After the panel wrote: `fresh`, read on the key's path, stands; the use counts are kept from the last read until
    /// the next read scans them again (asked for here), and a read in flight is dropped when it ends.
    pub(super) fn refresh(&self, folder: &Path, mut fresh: View) {
        let mut entries = self.entries();
        let entry = entries.entry(folder.to_path_buf()).or_default();
        if let Some(old) = &entry.view {
            keep_usage(&mut fresh, old);
        }
        entry.usage_due = true;
        entry.generation = entry.generation.wrapping_add(1);
        entry.waiting = Some(fresh.proposals.len());
        entry.view = Some(fresh);
        entry.read_at = None;
        self.changed.store(true, Ordering::Release);
    }

    /// The panel opened on `folder`: its next full read scans the use whatever the last scan's age.
    pub(super) fn opened(&self, folder: &Path) {
        let mut entries = self.entries();
        let entry = entries.entry(folder.to_path_buf()).or_default();
        entry.asked_at = Some(Instant::now());
        entry.usage_due = true;
    }

    /// Whether `folder`'s next full read is asked to scan the use: the reader tests' window on it.
    #[cfg(test)]
    pub(super) fn usage_due(&self, folder: &Path) -> bool {
        self.entries()
            .get(folder)
            .is_some_and(|entry| entry.usage_due)
    }

    /// Whether a read of `folder` runs now: the reader tests' window on it.
    #[cfg(test)]
    pub(super) fn reading(&self, folder: &Path) -> bool {
        self.entries()
            .get(folder)
            .is_some_and(|entry| entry.reading)
    }

    /// Starts the reads that are due: every folder asked for in the last `FORGET_AFTER`.
    pub(super) fn poll(self: &Arc<Self>) {
        let due: Vec<PathBuf> = {
            let mut entries = self.entries();
            entries.retain(|_, entry| entry.asked_at.is_some_and(|at| at.elapsed() < FORGET_AFTER));
            entries
                .iter()
                .filter(|(_, entry)| entry.read_at.is_none_or(|at| at.elapsed() >= REREAD))
                .map(|(folder, _)| folder.clone())
                .collect()
        };
        for folder in due {
            self.start(&folder, false);
        }
    }

    fn mark_changed(&self) {
        self.changed.store(true, Ordering::Release);
    }

    /// `poll`, then whether a read changed what the `+` or the panel shows since the last call.
    pub(super) fn take_changed(self: &Arc<Self>) -> bool {
        self.poll();
        self.changed.swap(false, Ordering::AcqRel)
    }
}

/// The use counts of `old` carried into `fresh`, a view read without the use scan.
fn keep_usage(fresh: &mut View, old: &View) {
    fresh.usage_read = old.usage_read;
    fresh.usage_partial = old.usage_partial;
    for row in &mut fresh.profiles {
        row.use_ = old
            .profiles
            .iter()
            .find(|known| known.name == row.name)
            .and_then(|known| known.use_);
    }
}

/// A read as the window's reader does it.
fn read_folder(folder: &Path, want: Want) -> Read {
    let root = super::nirvana_profiles::root(folder);
    if want.full {
        let view = read_view(&root, want.usage);
        Read {
            waiting: view.proposals.len(),
            view: Some(view),
        }
    } else {
        Read {
            view: None,
            waiting: proposals::list(&root).len(),
        }
    }
}

fn reader() -> &'static Arc<Reader> {
    static READER: OnceLock<Arc<Reader>> = OnceLock::new();
    READER.get_or_init(|| Reader::new(read_folder))
}

/// Whether the window's reader is asked to scan `folder`'s use at its next full read: the tests' window on `opened`.
#[cfg(test)]
pub(super) fn usage_asked(folder: &Path) -> bool {
    reader().usage_due(folder)
}

/// The waiting count of `folder`'s project as last read, for the `+` while the sidebar draws: a map read. Under test
/// the count is read at once.
pub(super) fn waiting_cached(folder: &Path) -> Option<usize> {
    #[cfg(test)]
    {
        Some(proposals::list(&super::nirvana_profiles::root(folder)).len())
    }
    #[cfg(not(test))]
    {
        reader().waiting(folder)
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
        reader().view(folder)
    }
}

/// After the panel wrote (an Accept, a Dismiss, a New): the profiles and the proposals read again now, on the key's
/// path, so the list never shows a proposal that is gone; the use counts are kept from the last read until the next.
fn refresh_now(folder: &Path, root: &Path) {
    reader().refresh(folder, read_view(root, false));
}

/// Whether a read changed what the `+` or the panel shows since the last call: the window repaints. Called on the
/// window's timer tick, which is where the due reads start, so the draw path never reads.
pub(crate) fn take_changed() -> bool {
    reader().take_changed()
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

/// The open proposal, by its id, how far its text is scrolled, and what the owner has seen of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Detail {
    pub(super) id: String,
    pub(super) scroll: usize,
    /// Drawn at least once since it was opened.
    pub(super) drawn: bool,
    /// Every line of it has been on screen since it was opened: `covered` reached its last line.
    pub(super) read_to_end: bool,
    /// Review 2 F1: its lines `0..covered` (the notice above them not counted) have each been on screen, with no gap
    /// from its top. A scroll never goes past the first line not yet shown, so this grows a screen at a time.
    pub(super) covered: usize,
    /// Its lines not yet on screen, counted from `covered`, at the last draw.
    pub(super) below: usize,
    /// The width it was laid out at and its line count, at the last draw: a change starts `covered` again from the top.
    pub(super) laid: Option<(usize, usize)>,
    /// What was drawn: Accept is held to it, and a re-read that differs opens the detail again from the top.
    pub(super) shown: Option<Seen>,
}

impl Detail {
    pub(super) fn new(id: String) -> Self {
        Self {
            id,
            scroll: 0,
            drawn: false,
            read_to_end: false,
            covered: 0,
            below: 0,
            laid: None,
            shown: None,
        }
    }
}

/// A New running on its own thread (review finding 6: the window never waits on the proposals' lock `new_profile`
/// takes). The slot is filled when it is done.
#[derive(Clone)]
pub(crate) struct Making {
    pub(super) name: String,
    slot: Arc<Mutex<Option<Result<PathBuf, String>>>>,
}

impl Making {
    fn take(&self) -> Option<Result<PathBuf, String>> {
        self.slot.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

/// Review 2 F4: each New on its thread, by project, until a panel lands it. The owner may close the panel while a New
/// waits on the proposals' lock, which another herdr can hold for long, and is never kept in the panel for it; the
/// next panel opened on that project lands the result as its notice, or says it is still making it. One per project:
/// a project's panel makes one New at a time, and a panel opened on it takes the one there is.
fn unlanded() -> MutexGuard<'static, HashMap<PathBuf, Making>> {
    static UNLANDED: OnceLock<Mutex<HashMap<PathBuf, Making>>> = OnceLock::new();
    UNLANDED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

impl std::fmt::Debug for Making {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Making").field("name", &self.name).finish()
    }
}

impl PartialEq for Making {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && Arc::ptr_eq(&self.slot, &other.slot)
    }
}

impl Eq for Making {}

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
    /// A New on its thread.
    pub(super) making: Option<Making>,
    /// What an Accept, a Dismiss or a New said, kept on screen until the next key; a refusal word for word.
    pub(super) notice: Vec<(String, Tone)>,
    /// A one-line answer in the footer.
    pub(super) message: Option<String>,
    /// The last frame drew the panel whole (review finding 11): keys act only then.
    pub(super) drawn: bool,
    /// The proposal highlighted when Suggested was last drawn with the focus; `d` in the list dismisses that one, and a
    /// move or a dismiss clears it (review finding 7).
    pub(super) drawn_proposal: Option<String>,
    /// Plan 47 P47b: `e` or Enter on a built-in's row said it has no file here; the same key on the same row next
    /// makes the project's copy. Any other key clears it.
    pub(super) copy_armed: Option<String>,
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
            making: None,
            notice: Vec::new(),
            message: None,
            drawn: false,
            drawn_proposal: None,
            copy_armed: None,
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
        self.drawn_proposal = None;
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

/// Review finding 4, the panel's layer: `text` as the screen can show all of it. A tab is drawn as spaces; every
/// character the store refuses as invisible (`proposals::invisible`) and every grapheme the buffer would drop (one
/// holding a control character, or 0 cells wide) is drawn as `<U+XXXX>` per character. A newline is kept for `wrap`.
pub(super) fn visible(text: &str) -> String {
    let named = |out: &mut String, c: char| out.push_str(&format!("<U+{:04X}>", u32::from(c)));
    let mut first = String::new();
    for c in text.replace("\r\n", "\n").chars() {
        match c {
            '\n' => first.push('\n'),
            '\t' => first.push_str(TAB),
            c if proposals::invisible(c) => named(&mut first, c),
            c => first.push(c),
        }
    }
    let mut out = String::new();
    for grapheme in unicode_segmentation::UnicodeSegmentation::graphemes(first.as_str(), true) {
        if grapheme == "\n" || (!grapheme.contains(char::is_control) && width(grapheme) > 0) {
            out.push_str(grapheme);
        } else {
            for c in grapheme.chars() {
                if c == '\n' {
                    out.push(c);
                } else {
                    named(&mut out, c);
                }
            }
        }
    }
    out
}

/// `text` in `cells`, cut in the middle when it does not fit, so a path keeps its root and its last folder.
pub(super) fn middle(text: &str, cells: usize) -> String {
    if width(text) <= cells {
        return text.to_owned();
    }
    let room = cells.saturating_sub(1);
    let head_room = room / 2;
    let tail_room = room - head_room;
    let mut head = String::new();
    let mut used = 0;
    for c in text.chars() {
        let cell = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + cell > head_room {
            break;
        }
        head.push(c);
        used += cell;
    }
    let mut tail: Vec<char> = Vec::new();
    let mut used = 0;
    for c in text.chars().rev() {
        let cell = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + cell > tail_room {
            break;
        }
        tail.push(c);
        used += cell;
    }
    tail.reverse();
    format!("{head}\u{2026}{}", tail.into_iter().collect::<String>())
}

/// `text` wrapped to `cells`: each of its lines by words, a word wider than `cells` split by characters, a blank line
/// kept. Never empty, never panics, whatever the text.
///
/// Review 2 F3: a line's leading spaces are drawn as written, so a code block and a nested list keep their shape (an
/// indent of a line or more is whole lines of spaces, then the rest). A wrapped line's next lines keep that indent
/// while it is at most half the line and leaves room for a wide character, else start at the margin. A line of spaces
/// only is an empty line.
pub(super) fn wrap(text: &str, cells: usize) -> Vec<String> {
    let cells = cells.max(1);
    let mut out = Vec::new();
    for source in text.replace("\r\n", "\n").split('\n') {
        let words = source.trim_start_matches(' ');
        if words.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut indent = source.len() - words.len();
        while indent >= cells {
            out.push(" ".repeat(cells));
            indent -= cells;
        }
        let hang = if indent * 2 <= cells && indent + 2 <= cells {
            indent
        } else {
            0
        };
        let mut line = " ".repeat(indent);
        let mut used = indent;
        let mut first = true;
        for word in words.split(' ') {
            let word_width = width(word);
            if !first && used + 1 + word_width > cells {
                out.push(std::mem::replace(&mut line, " ".repeat(hang)));
                used = hang;
                first = true;
            }
            if !first {
                line.push(' ');
                used += 1;
            }
            first = false;
            if word_width <= cells.saturating_sub(used) {
                line.push_str(word);
                used += word_width;
                continue;
            }
            for character in word.chars() {
                let cell = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
                if used + cell > cells && used > 0 {
                    out.push(std::mem::replace(&mut line, " ".repeat(hang)));
                    used = hang;
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

/// A profile's use: "used 3 of 35", carried of fit; in words when nothing fits or carries it; "(partial)" when the
/// scan behind it was cut short, so the counts are a floor.
pub(super) fn use_text(row: &ProfileRow, usage_read: bool, partial: bool) -> String {
    if !usage_read {
        return "use: counting".to_owned();
    }
    match (row.use_, partial) {
        (None, _) => "use: not counted".to_owned(),
        (Some((0, 0)), false) => NO_USE.to_owned(),
        (Some((0, 0)), true) => NO_USE_PARTIAL.to_owned(),
        (Some((carried, fit)), false) => format!("used {carried} of {fit}"),
        (Some((carried, fit)), true) => format!("used {carried} of {fit} (partial)"),
    }
}

/// Plan 47 P47a (L1): `in use: N now` beside the use when Running orders of this project carry the profile; nothing
/// when none do.
pub(super) fn live_text(row: &ProfileRow) -> Option<String> {
    (row.live > 0).then(|| format!("in use: {} now", row.live))
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
        for line in wrap(&visible(said), text) {
            lines.push(Line::Text(line, *tone));
        }
    }
    if let Some(making) = &panel.making {
        lines.push(Line::Text(
            format!("making {}: working", making.name),
            Tone::Attention,
        ));
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
    // Review 2 F6: a name is a folder or file name on disk, drawn through `visible` as the text is.
    let names: Vec<String> = view.profiles.iter().map(|row| visible(&row.name)).collect();
    let name_col = names
        .iter()
        .map(|name| width(name))
        .max()
        .unwrap_or(0)
        .min(NAME_COL_MAX);
    for (index, row) in view.profiles.iter().enumerate() {
        let push = |lines: &mut Vec<Line>, first: bool, line: String, tone: Tone| {
            lines.push(Line::Row(Part::Profiles, index, first, line, tone));
        };
        let name = &names[index];
        let Some(parsed) = &row.parsed else {
            let refused = format!("refused: herdr profile check {name}");
            if wide {
                push(
                    &mut lines,
                    true,
                    format!("{}  {refused}", pad(name, name_col)),
                    Tone::Attention,
                );
            } else {
                push(&mut lines, true, name.clone(), Tone::Attention);
                for said in wrap(&format!("  {refused}"), text) {
                    push(&mut lines, false, said, Tone::Attention);
                }
            }
            continue;
        };
        let lead = if wide { name_col + 2 } else { 2 };
        let room = text.saturating_sub(lead);
        let description = if parsed.description.is_empty() {
            "no description".to_owned()
        } else {
            visible(&parsed.description)
        };
        let mut said = wrap(&description, room).into_iter();
        if wide {
            let first = said.next().unwrap_or_default();
            push(
                &mut lines,
                true,
                format!("{}  {first}", pad(name, name_col)),
                Tone::Plain,
            );
        } else {
            push(&mut lines, true, name.clone(), Tone::Plain);
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
        // Plan 47 P47b: the mark, first, so it is on the row's first line of facts at any width.
        if row.builtin {
            facts.insert(0, BUILT_IN.to_owned());
        }
        facts.push(use_text(row, view.usage_read, view.usage_partial));
        if let Some(live) = live_text(row) {
            facts.push(live);
        }
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
            visible(&proposal.id),
            proposal.kind.word(),
            visible(&proposal.profile)
        );
        let evidence = visible(&one_line(&proposal.evidence));
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
        for wrapped in wrap(&visible(line), text) {
            lines.push(Line::Text(wrapped, Tone::Attention));
        }
    }
    lines
}

/// The detail view's lines for `proposal` at `cells`: the whole evidence, then the whole file Accept writes, which is
/// `seen.target`, built by the store's own `prepared` and `target`, the text `accept` writes.
pub(super) fn detail_lines(
    proposal: &Proposal,
    seen: Option<&Seen>,
    root: &Path,
    cells: usize,
) -> Vec<(String, Tone)> {
    let (id, profile) = (visible(&proposal.id), visible(&proposal.profile));
    let mut lines = vec![(
        match proposal.kind {
            Kind::New => format!("{id} \u{b7} new profile {profile}"),
            Kind::Patch => format!("{id} \u{b7} a patch of {profile}"),
        },
        Tone::Heading,
    )];
    for line in wrap(
        &visible(&format!(
            "by {} \u{b7} run {} \u{b7} {}",
            proposal.by, proposal.run, proposal.date
        )),
        cells,
    ) {
        lines.push((line, Tone::Muted));
    }
    lines.push((String::new(), Tone::Plain));
    lines.push(("Evidence".to_owned(), Tone::Heading));
    for line in wrap(&visible(&proposal.evidence), cells.saturating_sub(2)) {
        lines.push((format!("  {line}"), Tone::Plain));
    }
    lines.push((String::new(), Tone::Plain));
    match seen.map(|seen| &seen.target) {
        Some(Ok(target)) => {
            let shown = visible(
                &target
                    .file
                    .strip_prefix(root)
                    .unwrap_or(&target.file)
                    .display()
                    .to_string()
                    .replace('\\', "/"),
            );
            let heading = match proposal.kind {
                Kind::New => format!("The file Accept writes, {shown}"),
                Kind::Patch => format!(
                    "The file Accept writes, {shown}: its front matter as it stands, the new body"
                ),
            };
            for line in wrap(&heading, cells) {
                lines.push((line, Tone::Heading));
            }
            for line in wrap(&visible(&target.after), cells.saturating_sub(2)) {
                lines.push((format!("  {line}"), Tone::Plain));
            }
        }
        Some(Err(why)) => {
            for line in wrap(
                &visible(&format!("Accept cannot write this proposal: {why}")),
                cells,
            ) {
                lines.push((line, Tone::Attention));
            }
        }
        None => lines.push((
            "The file Accept writes: not read yet".to_owned(),
            Tone::Attention,
        )),
    }
    lines
}

/// The footer's keys for the panel's state, in `cells`: the long form, else a short one that keeps `esc` (at 80
/// columns the panel's inner width is 48 and the long form lost it). In the detail, `a` is named only when it acts.
pub(super) fn footer_keys(panel: &ProfilePanel, cells: usize) -> String {
    let long = footer_long(panel);
    if width(&long) <= cells {
        return long;
    }
    let ready = panel.detail.as_ref().is_some_and(accept_ready);
    if panel.making.is_some() {
        "working \u{b7} esc closes".to_owned()
    } else if panel.naming.is_some() || panel.shaping.is_some() {
        "enter \u{b7} esc cancels".to_owned()
    } else if panel.detail.is_some() && ready {
        "a accept \u{b7} d dismiss \u{b7} \u{2191}/\u{2193} scroll \u{b7} esc back".to_owned()
    } else if panel.detail.is_some() {
        "\u{2193} read to the end \u{b7} d dismiss \u{b7} esc back".to_owned()
    } else if panel.focus == Part::Profiles {
        "enter/e edit \u{b7} K cards \u{b7} n new \u{b7} tab \u{b7} esc close".to_owned()
    } else {
        "enter open \u{b7} d dismiss \u{b7} n new \u{b7} tab \u{b7} esc close".to_owned()
    }
}

fn footer_long(panel: &ProfilePanel) -> String {
    if panel.making.is_some() {
        "working: keys wait until it is made \u{b7} esc closes".to_owned()
    } else if panel.naming.is_some() {
        "a-z 0-9 - \u{b7} enter next \u{b7} esc cancels".to_owned()
    } else if panel.shaping.is_some() {
        // Review finding 13: the range is the shapes there are, never a typed count.
        format!(
            "\u{2190}/\u{2192} or 0-{} shape \u{b7} enter makes it \u{b7} esc cancels",
            crate::nirvana_profiles::SHAPES.len()
        )
    } else if let Some(detail) = &panel.detail {
        if accept_ready(detail) {
            "a accept \u{b7} d dismiss \u{b7} \u{2191}/\u{2193} pgup/pgdn scroll \u{b7} esc back"
                .to_owned()
        } else {
            "\u{2193}/pgdn/end read to the end, then a accepts \u{b7} d dismiss \u{b7} esc back"
                .to_owned()
        }
    } else if panel.focus == Part::Profiles {
        "\u{2191}/\u{2193} choose \u{b7} enter/e edit \u{b7} K cards \u{b7} n new \u{b7} tab suggested \u{b7} esc close"
            .to_owned()
    } else {
        "\u{2191}/\u{2193} choose \u{b7} enter open \u{b7} d dismiss \u{b7} n new \u{b7} tab profiles \u{b7} esc close"
            .to_owned()
    }
}

/// Whether this detail has been drawn, with what was drawn kept: the rule `a` and `d` both hold to.
fn detail_drawn(detail: &Detail) -> bool {
    detail.drawn && detail.shown.is_some()
}

/// Whether `a` acts in this detail: drawn, and read to its last line, with what was drawn kept to send.
fn accept_ready(detail: &Detail) -> bool {
    detail_drawn(detail) && detail.read_to_end
}

/// Why `a` does not act in this detail yet, in the panel's words.
fn accept_missing(detail: &Detail) -> Option<String> {
    if !detail_drawn(detail) {
        Some(ACCEPT_NOT_DRAWN.to_owned())
    } else if !detail.read_to_end {
        Some(format!(
            "scroll to the end to accept ({} more line{})",
            detail.below,
            if detail.below == 1 { "" } else { "s" }
        ))
    } else {
        None
    }
}

/// Draws the panel over `area`, as tall as its content, as the Seats panel is drawn. Sets `panel.drawn`, and in the
/// detail what was drawn and how far it was read.
pub(super) fn render_profile_panel(
    buffer: &mut Buffer,
    area: Rect,
    panel: &mut ProfilePanel,
    view: Option<&View>,
    palette: &Palette,
) -> ProfileHits {
    let mut hits = ProfileHits::default();
    panel.drawn = false;
    let panel_width = area.width.saturating_sub(2).min(PANEL_WIDTH);
    let base = Style::default().fg(palette.text).bg(palette.panel_bg);
    if panel_width < PANEL_MIN_WIDTH || area.height < PANEL_MIN_HEIGHT {
        // Review finding 11: too small to draw, so it takes no key but Esc, and says so where it can.
        if area.width > 0 && area.height > 0 {
            let said = fit(TOO_SMALL, usize::from(area.width));
            let rect = Rect::new(area.x, area.y, width(&said) as u16, 1);
            buffer.set_style(rect, base.fg(palette.yellow));
            put_text(
                buffer,
                area.x,
                area.y,
                area.width,
                &said,
                base.fg(palette.yellow),
            );
            hits.area = rect;
        }
        return hits;
    }
    let inner = usize::from(panel_width.saturating_sub(4));
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
    let seen = panel
        .detail
        .as_ref()
        .and_then(|detail| view.and_then(|view| view.seen.get(&detail.id)));
    // A re-read that changed what the detail shows: it is read again from the top, and Accept is held to the new one.
    if let Some(detail) = panel.detail.as_mut() {
        if detail.shown.as_ref() != seen {
            if detail.shown.is_some() {
                panel.notice = vec![(
                    format!(
                        "{} changed while it was open: read it again from the top",
                        detail.id
                    ),
                    Tone::Attention,
                )];
                *detail = Detail::new(detail.id.clone());
            }
            detail.shown = seen.cloned();
        }
    }
    let folder = visible(&folder_name(&panel.root));
    // The detail's notice lines above its own: `covered` counts its own lines only.
    let mut prefix = 0;
    let (title, body): (String, Vec<Line>) = match (view, &panel.detail) {
        (None, _) => (
            format!(" Profiles \u{b7} {folder} "),
            vec![Line::Text(
                "reading this project's profiles".into(),
                Tone::Muted,
            )],
        ),
        (Some(view), Some(detail)) => {
            let mut body: Vec<Line> = panel
                .notice
                .iter()
                .flat_map(|(said, tone)| {
                    wrap(&visible(said), inner)
                        .into_iter()
                        .map(|line| Line::Text(line, *tone))
                })
                .collect();
            if !body.is_empty() {
                body.push(Line::Text(String::new(), Tone::Plain));
            }
            prefix = body.len();
            match proposal {
                Some(proposal) => body.extend(
                    detail_lines(proposal, seen, &view.root, inner)
                        .into_iter()
                        .map(|(line, tone)| Line::Text(line, tone)),
                ),
                None => body.push(Line::Text(
                    format!(
                        "{} is no longer waiting: esc goes back to the list",
                        visible(&detail.id)
                    ),
                    Tone::Attention,
                )),
            }
            (
                format!(" Profiles \u{b7} {folder} \u{b7} {} ", visible(&detail.id)),
                body,
            )
        }
        (Some(view), None) => (
            format!(" Profiles \u{b7} {folder} "),
            list_lines(view, panel, inner),
        ),
    };

    // The frame holds the project's path, the body and two more lines: the place (or a blank) and the keys.
    let height = (body.len() as u16 + 5).min(area.height.saturating_sub(1));
    let rect = Rect::new(area.x + 1, area.y + 1, panel_width, height);
    hits.area = rect;
    hits.close = draw_frame(buffer, rect, &title, palette);
    let x = rect.x + 2;
    let width_cells = inner as u16;
    // Say where: the project's full path on its own line, cut in the middle, never at the end.
    put_text(
        buffer,
        x,
        rect.y + 1,
        width_cells,
        &middle(&visible(&panel.root.display().to_string()), inner),
        tone_style(Tone::Muted),
    );
    let body_height = usize::from(rect.height.saturating_sub(5));
    let most = body.len().saturating_sub(body_height);
    let detail_len = body.len() - prefix;
    let scroll = if let Some(detail) = panel.detail.as_mut() {
        if proposal.is_some() {
            // Review 2 F1: the same width lays out the same lines, so the count stands; another width, or another line
            // count, maps no line exactly, so the count starts again from the top.
            let laid = (inner, detail_len);
            if detail.laid.is_some_and(|before| before != laid) {
                detail.covered = 0;
                detail.read_to_end = false;
            }
            detail.laid = Some(laid);
        }
        detail.scroll = detail.scroll.min(most);
        // Review 2 F1: never past the first line not yet shown, so no scroll (End, PageDown, Down, the wheel) opens a
        // gap in what the owner has seen.
        detail.scroll = detail.scroll.min(prefix + detail.covered);
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
    let mut drawn_proposal = None;
    for (row, line) in body.iter().skip(scroll).take(body_height).enumerate() {
        let y = rect.y + 2 + row as u16;
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
                    if *part == Part::Suggested && *first {
                        drawn_proposal = view
                            .and_then(|view| view.proposals.get(*index))
                            .map(|proposal| proposal.id.clone());
                    }
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
    let shown_to = (scroll + body_height).min(body.len());
    let place_y = rect.bottom().saturating_sub(3);
    if let Some(detail) = panel.detail.as_mut() {
        // What the owner has seen of it: drawn, and its lines on screen from its top with no gap, now this screen
        // further (the scroll is at most the first line not yet shown, so this screen joins on).
        detail.drawn = proposal.is_some();
        if proposal.is_some() {
            detail.covered = detail
                .covered
                .max(shown_to.saturating_sub(prefix))
                .min(detail_len);
            detail.read_to_end = detail.covered >= detail_len;
        }
        detail.below = detail_len.saturating_sub(detail.covered);
        let mut place = format!(
            "lines {}-{} of {}",
            (scroll + 1).min(shown_to),
            shown_to,
            body.len()
        );
        // Counted from the mark, not from the scroll: the lines not yet on screen.
        if detail.below > 0 {
            place.push_str(&format!(" \u{b7} \u{25bc} {} unread", detail.below));
        } else if shown_to < body.len() {
            place.push_str(&format!(
                " \u{b7} \u{25bc} {} more below",
                body.len() - shown_to
            ));
        }
        put_text(
            buffer,
            x,
            place_y,
            width_cells,
            &fit(&place, inner),
            tone_style(Tone::Muted),
        );
    }
    panel.drawn_proposal = drawn_proposal;
    let foot_y = rect.bottom().saturating_sub(2);
    let keys;
    let said;
    let (foot, style) = match &panel.message {
        Some(message) => {
            said = visible(message);
            (said.as_str(), base.fg(palette.yellow))
        }
        None => {
            keys = footer_keys(panel, inner);
            (keys.as_str(), base.fg(palette.overlay1))
        }
    };
    put_text(buffer, x, foot_y, width_cells, &fit(foot, inner), style);
    panel.drawn = true;
    hits
}

// ---- keys, clicks and what they do ----

/// Review 2 F4: a New that finished after its panel closed, as the next panel's notice. No edit tab opens by itself
/// (the owner was not looking); the profile is selected, so `e` opens it.
fn landed_late(panel: &mut ProfilePanel, name: &str, made: Result<PathBuf, String>) {
    match made {
        Ok(file) => {
            panel.notice = vec![
                (
                    format!("made {} after the panel closed", file.display()),
                    Tone::Plain,
                ),
                (
                    "it is selected under Profiles: e opens it in an edit tab".to_owned(),
                    Tone::Muted,
                ),
            ];
            panel.focus = Part::Profiles;
            panel.profile_key = Some(name.to_owned());
        }
        Err(why) => {
            panel.notice = vec![(
                format!("New profile {name}, after the panel closed: not made: {why}"),
                Tone::Attention,
            )]
        }
    }
}

/// Review finding 2: a key that writes or opens acts only with no modifier but Shift held; Ctrl+A, Alt+A and Ctrl+D do
/// nothing. This panel's rule; the strip's panels are left as they are.
fn bare(key: &crate::input::TerminalKey) -> bool {
    key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
}

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
        let mut panel = ProfilePanel::new(space);
        // Review 2 F7: the use is scanned when the panel opens, whatever the last scan's age.
        reader().opened(&panel.folder);
        // Review 2 F4: a New this project's last panel closed on.
        let pending = unlanded().get(&panel.root).cloned();
        if let Some(making) = pending {
            match making.take() {
                Some(made) => {
                    unlanded().remove(&panel.root);
                    landed_late(&mut panel, &making.name, made);
                }
                None => panel.making = Some(making),
            }
        }
        self.nirvana_profile_panel = Some(panel);
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

    /// The window's timer tick: tells the reader which panel is drawn (only its reads are full), and lands a New that
    /// finished on its thread.
    pub(crate) fn tick_profile_panel(&mut self, outcome: &mut ClientShellInput) {
        reader().show(
            self.nirvana_profile_panel
                .as_ref()
                .filter(|panel| panel.drawn)
                .map(|panel| panel.folder.clone()),
        );
        let made = self
            .nirvana_profile_panel
            .as_ref()
            .and_then(|panel| panel.making.as_ref())
            .and_then(Making::take);
        if let Some(made) = made {
            self.profile_made(made, outcome);
            outcome.repaint = true;
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

    /// Opens `path` in an edit tab beside the space's tab, through `command.invoke` as the Preload group's `e` and `K`
    /// do.
    fn profile_open_tab(&mut self, prefix: &str, path: &Path, outcome: &mut ClientShellInput) {
        let Some((workspace_id, tab_id)) = self
            .nirvana_profile_panel
            .as_ref()
            .map(|panel| (panel.workspace_id.clone(), panel.tab_id.clone()))
        else {
            return;
        };
        self.push_endpoint_method(
            crate::api::schema::Method::CommandInvoke(crate::api::schema::CommandInvokeParams {
                command_id: format!("{prefix}{}", path.display()),
                workspace_id: Some(workspace_id),
                tab_id: Some(tab_id),
                pane_id: None,
                selection: None,
            }),
            outcome,
        );
    }

    /// `e`, Enter and `K`: the edit tab, and the panel closes, since the editor is what the owner sees.
    fn profile_edit_tab(&mut self, prefix: &str, path: &Path, outcome: &mut ClientShellInput) {
        self.profile_open_tab(prefix, path, outcome);
        self.nirvana_profile_panel = None;
    }

    /// `e` and Enter. Plan 47 P47b: a built-in's row has no file to open. The first press says so and names the copy
    /// (`armed` is what the last key left, `copy_armed`); the same key on the same row next writes the project's copy
    /// off the key path, which `profile_made` then opens. Nothing is written before that second press.
    fn profile_edit(
        &mut self,
        view: &View,
        index: usize,
        armed: Option<String>,
        outcome: &mut ClientShellInput,
    ) {
        let Some(row) = view.profiles.get(index) else {
            self.profile_say("no profile to edit: n makes one");
            return;
        };
        if row.builtin {
            let name = row.name.clone();
            if armed.as_deref() == Some(name.as_str()) {
                self.profile_copy(&name);
                return;
            }
            let shown = visible(&name);
            self.profile_notice(vec![
                (
                    format!(
                        "{shown} is built into herdr: this project has no file for it, so there is nothing to open"
                    ),
                    Tone::Attention,
                ),
                (
                    format!(
                        "e again writes the project's own copy into .nirvana/profiles/{shown}/ (its text and cards, \
                         which then win over the built-in) and opens it; or run `{}`",
                        crate::nirvana_profiles::copy_command(&shown)
                    ),
                    Tone::Muted,
                ),
            ]);
            if let Some(panel) = self.nirvana_profile_panel.as_mut() {
                panel.copy_armed = Some(name);
            }
            return;
        }
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
        if row.builtin {
            self.profile_say(crate::nirvana_profiles::builtin_refusal(
                &visible(&name),
                "its cards ship with herdr and have no folder here: e makes the copy",
            ));
            return;
        }
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

    fn profile_places(&self) -> Option<(PathBuf, PathBuf)> {
        self.nirvana_profile_panel
            .as_ref()
            .map(|panel| (panel.folder.clone(), panel.root.clone()))
    }

    /// Accept, from the detail only, of what was drawn (`seen`): the library's line and `ACCEPT_NOTE` on screen; its
    /// refusal word for word; "busy" without a wait; or, when the proposal changed since the draw, the detail open
    /// again from the top.
    fn profile_accept(&mut self, seen: &Seen) {
        let Some((folder, root)) = self.profile_places() else {
            return;
        };
        let result = proposals::accept_seen(&root, seen);
        if result != Err(Refused::Busy) {
            refresh_now(&folder, &root);
        }
        let Some(panel) = self.nirvana_profile_panel.as_mut() else {
            return;
        };
        match result {
            Ok(line) => {
                panel.detail = None;
                panel.notice = vec![
                    (line, Tone::Plain),
                    (proposals::ACCEPT_NOTE.to_owned(), Tone::Attention),
                ];
            }
            Err(Refused::Busy) => panel.message = Some(proposals::BUSY.to_owned()),
            Err(Refused::Changed(why)) => {
                panel.detail = Some(Detail::new(seen.id.clone()));
                panel.notice = vec![(why, Tone::Attention)];
            }
            Err(Refused::No(why)) => panel.notice = vec![(why, Tone::Attention)],
        }
    }

    /// Dismiss of one proposal id: from the detail its id, from the list the row drawn highlighted.
    fn profile_dismiss(&mut self, id: &str) -> Result<(), Refused> {
        let Some((folder, root)) = self.profile_places() else {
            return Ok(());
        };
        let result = proposals::try_dismiss(&root, id, None);
        if result != Err(Refused::Busy) {
            refresh_now(&folder, &root);
        }
        let Some(panel) = self.nirvana_profile_panel.as_mut() else {
            return Ok(());
        };
        match result {
            Ok(line) => {
                panel.detail = None;
                panel.notice = vec![(line, Tone::Plain)];
                Ok(())
            }
            Err(Refused::Busy) => {
                panel.message = Some(proposals::BUSY.to_owned());
                Err(Refused::Busy)
            }
            Err(refused) => {
                panel.notice = vec![(refused.to_string(), Tone::Attention)];
                Ok(())
            }
        }
    }

    /// `n`'s last step: the profile written through `new_profile` or `new_profile_shape` on a thread of its own, since
    /// both take the proposals' lock and the window never waits on it. `tick_profile_panel` (or the next key) lands it.
    fn profile_make(&mut self, name: &str, shape: usize) {
        self.profile_make_with(name, move |root, owned| match shape.checked_sub(1) {
            None => crate::nirvana_profiles::new_profile(root, owned),
            Some(at) => match crate::nirvana_profiles::SHAPES.get(at) {
                Some(shape) => crate::nirvana_profiles::new_profile_shape(root, owned, shape),
                None => Err(format!("no shape {shape}")),
            },
        });
    }

    /// Plan 47 P47b: the second `e` on a built-in's row, the project's own copy, made as a New is.
    fn profile_copy(&mut self, name: &str) {
        self.profile_make_with(name, |root, owned| {
            crate::nirvana_profiles::new_profile_from(root, owned, owned)
        });
    }

    /// A New's job on a thread of its own, landed by `tick_profile_panel` or the next key.
    fn profile_make_with(
        &mut self,
        name: &str,
        job: impl FnOnce(&Path, &str) -> Result<PathBuf, String> + Send + 'static,
    ) {
        let Some(root) = self
            .nirvana_profile_panel
            .as_ref()
            .map(|panel| panel.root.clone())
        else {
            return;
        };
        let slot = Arc::new(Mutex::new(None));
        let filled = Arc::clone(&slot);
        let owned = name.to_owned();
        let project = root.clone();
        let spawned = crate::thread_spawn::spawn_named("nirvana-profile-new", move || {
            let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&root, &owned)))
                .unwrap_or_else(|_| Err("herdr failed while making it".to_owned()));
            *filled.lock().unwrap_or_else(|e| e.into_inner()) = Some(made);
            reader().mark_changed();
        });
        match spawned {
            Ok(_) => {
                let making = Making {
                    name: name.to_owned(),
                    slot,
                };
                unlanded().insert(project, making.clone());
                if let Some(panel) = self.nirvana_profile_panel.as_mut() {
                    panel.making = Some(making);
                }
            }
            Err(err) => self.profile_notice(vec![(
                format!("not made: herdr could not start making it: {err}"),
                Tone::Attention,
            )]),
        }
    }

    /// A New is done: the full path it wrote, said in the panel, and the file opened in an edit tab beside the space's.
    fn profile_made(&mut self, made: Result<PathBuf, String>, outcome: &mut ClientShellInput) {
        let Some((folder, root)) = self.profile_places() else {
            return;
        };
        unlanded().remove(&root);
        if let Some(panel) = self.nirvana_profile_panel.as_mut() {
            panel.making = None;
        }
        match made {
            Ok(file) => {
                refresh_now(&folder, &root);
                self.profile_open_tab(crate::nirvana::EDIT_FILE_COMMAND, &file, outcome);
                self.profile_notice(vec![
                    (format!("made {}", file.display()), Tone::Plain),
                    (
                        "it is open in an edit tab beside this space's tab: esc closes this panel"
                            .to_owned(),
                        Tone::Muted,
                    ),
                ]);
            }
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
        // A New that finished lands before the key is read.
        let made = self
            .nirvana_profile_panel
            .as_ref()
            .and_then(|panel| panel.making.as_ref())
            .and_then(Making::take);
        if let Some(made) = made {
            self.profile_made(made, outcome);
        }
        let code = key.code;
        let bare = bare(key);
        let view = self.profile_view();
        let Some(panel) = self.nirvana_profile_panel.as_mut() else {
            return;
        };
        panel.message = None;
        // While a New runs, and while the panel was not drawn whole in the last frame: Esc only.
        if let Some(making) = &panel.making {
            if code == KeyCode::Esc {
                self.nirvana_profile_panel = None;
            } else {
                panel.message = Some(format!("making {}: working, keys wait", making.name));
            }
            return;
        }
        if !panel.drawn {
            if code == KeyCode::Esc {
                self.nirvana_profile_panel = None;
            } else {
                panel.message = Some(TOO_SMALL.to_owned());
            }
            return;
        }
        // What an Accept, a Dismiss or a New said stays until this key.
        panel.notice.clear();
        // Plan 47 P47b: a built-in's first `e` arms the copy for this one key only.
        let armed = panel.copy_armed.take();
        if !bare && matches!(code, KeyCode::Char(_) | KeyCode::Enter) {
            return;
        }
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
                    self.profile_make(&name, shape);
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
                KeyCode::Char('a') => match (accept_missing(detail), detail.shown.clone()) {
                    (None, Some(seen)) => self.profile_accept(&seen),
                    (missing, _) => {
                        panel.message = Some(missing.unwrap_or_else(|| ACCEPT_NOT_DRAWN.to_owned()))
                    }
                },
                KeyCode::Char('d') if !detail_drawn(detail) => {
                    panel.message = Some(DISMISS_DETAIL_NOT_DRAWN.to_owned())
                }
                KeyCode::Char('d') => {
                    let id = detail.id.clone();
                    let _ = self.profile_dismiss(&id);
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
                self.profile_edit(&view, selected, armed, outcome);
            }
            (KeyCode::Char('K'), Part::Profiles) => self.profile_cards(&view, selected, outcome),
            (KeyCode::Enter, Part::Suggested) => match view.proposals.get(selected) {
                Some(proposal) => {
                    panel.detail = Some(Detail::new(proposal.id.clone()));
                }
                None => panel.message = Some(NOTHING_SUGGESTED.to_owned()),
            },
            // Review finding 7: the row highlighted when the list was last drawn, not where `follow` puts the index;
            // once per draw, so a held `d` never walks down the list.
            (KeyCode::Char('d'), Part::Suggested) => match panel.drawn_proposal.take() {
                None if view.proposals.is_empty() => {
                    panel.message = Some(NOTHING_SUGGESTED.to_owned())
                }
                None => panel.message = Some(DISMISS_NOT_DRAWN.to_owned()),
                Some(id) if !view.proposals.iter().any(|known| known.id == id) => {
                    panel.message =
                        Some(format!("{id} is no longer waiting: nothing was dismissed"));
                }
                Some(id) => {
                    if self.profile_dismiss(&id) == Err(Refused::Busy) {
                        if let Some(panel) = self.nirvana_profile_panel.as_mut() {
                            panel.drawn_proposal = Some(id);
                        }
                    }
                }
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
