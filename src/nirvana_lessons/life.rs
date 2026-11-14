//! Plan 73 (nirvana/plans/2026-11-14-lesson-lifecycle.md): a lesson's life by time, after Hermes' curator
//! (`agent/curator.py`) and skill ledger (`tools/skill_ledger.py`).
//!
//! The owner, 2026-11-14: "Sure lets add the top 3", for "Hermes's idle queue and lesson lifecycle, added to the lesson
//! reviews." The proposal: "a lesson unused for 14 days is marked stale and archived at 30, never deleted; pinned
//! lessons are exempt; every lesson change is logged (append-only JSONL with before/after content) so one edit can be
//! undone".
//!
//! - **A use** is a lesson delivered into an order's prompt: `count_offered_at` moves `Entry::used_at`, and so does
//!   herdr writing it. An entry from before plan 73 has no `used_at`; the first sweep dates it to that sweep, since
//!   counting from `written_at` would archive a lesson a brief offered yesterday.
//! - **Stale** is read, not stored (`life`): a stored mark would need clearing on every offer, a second index write on
//!   the seat path. A stale lesson is still offered; an offer makes it active again, as a use does in Hermes.
//! - **Archived** is a ledger op, `archive`, by the idle tick (`nirvana_idle_review::tick_with`) or `herdr lessons
//!   sweep`: the file goes to `.archive/<op>-<slug>.md` and out of the index, as `retire` moves it. Never deleted.
//!   `herdr lessons undo` puts it back and counts that as a use, so the next sweep keeps it.
//! - **Pinned** (`herdr lessons pin`) is never stale, archived or retired; `pin` and `unpin` are ledger ops too.

use std::path::{Path, PathBuf};

use super::{
    dir, ledger, ledgered, lock, read_index, rollback, slug, to_archive, write_index, Entry,
    ARCHIVE,
};

/// Plan 73 L3, the owner's numbers in the proposal; hypotheses. Unused this long, a lesson is stale.
pub const STALE_AFTER_S: u64 = 14 * 86_400;
/// Unused this long, a lesson is archived.
pub const ARCHIVE_AFTER_S: u64 = 30 * 86_400;
/// The ledger's `by` and `run` for the ops this file makes.
const BY: &str = "herdr";
const SWEEP_RUN: &str = "lifecycle";
const OWNER_RUN: &str = "owner";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Life {
    Active,
    Stale,
    /// Unused `ARCHIVE_AFTER_S`: the next sweep archives it.
    Due,
    Pinned,
    /// No use on record yet: the next sweep dates it.
    Undated,
}

pub fn life(entry: &Entry, now: u64) -> Life {
    if entry.pinned {
        return Life::Pinned;
    }
    let Some(used) = entry.used_at else {
        return Life::Undated;
    };
    let unused = now.saturating_sub(used);
    if unused >= ARCHIVE_AFTER_S {
        Life::Due
    } else if unused >= STALE_AFTER_S {
        Life::Stale
    } else {
        Life::Active
    }
}

fn days(entry: &Entry, now: u64) -> u64 {
    entry
        .used_at
        .map_or(0, |used| now.saturating_sub(used) / 86_400)
}

/// The word `herdr lessons list` adds; `None` for an active or undated lesson.
pub fn word(entry: &Entry, now: u64) -> Option<String> {
    match life(entry, now) {
        Life::Pinned => Some("pinned".into()),
        Life::Stale => Some(format!("stale, unused {} days", days(entry, now))),
        Life::Due => Some(format!("archive due, unused {} days", days(entry, now))),
        Life::Active | Life::Undated => None,
    }
}

/// What a sweep did, or with `dry` would do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Swept {
    /// Lessons with no use on record, dated to the sweep.
    pub dated: Vec<String>,
    /// `(slug, days unused)`.
    pub stale: Vec<(String, u64)>,
    /// `(slug, op id)`; the op id is empty on a dry run.
    pub archived: Vec<(String, String)>,
    /// `(slug, why)`.
    pub failed: Vec<(String, String)>,
}

/// Dates the undated, then archives each lesson `Due`. An index that does not read is no lesson, so nothing is
/// written over it.
pub fn sweep(root: &Path, now: u64, dry: bool) -> Swept {
    let mut out = Swept::default();
    // The lock makes `.nirvana/runs/`; a project the tick knows that keeps no lesson gets no new folder from a sweep.
    if !dir(root).join(super::INDEX).is_file() {
        return out;
    }
    {
        let Ok(_lock) = lock(root) else {
            out.failed
                .push(("index".into(), "the lessons lock cannot be taken".into()));
            return out;
        };
        let mut index = read_index(root);
        for (slug, entry) in index.lessons.iter_mut() {
            if entry.used_at.is_none() {
                out.dated.push(slug.clone());
                if !dry {
                    entry.used_at = Some(now);
                }
            }
        }
        if !dry && !out.dated.is_empty() {
            if let Err(err) = write_index(root, &index) {
                out.failed.push(("index".into(), err.to_string()));
                return out;
            }
        }
    }
    let index = read_index(root);
    for (slug, entry) in &index.lessons {
        match life(entry, now) {
            Life::Stale => out.stale.push((slug.clone(), days(entry, now))),
            Life::Due if dry => out.archived.push((slug.clone(), String::new())),
            Life::Due => match archive(root, slug, now) {
                Ok(id) => out.archived.push((slug.clone(), id)),
                Err(why) => out.failed.push((slug.clone(), why)),
            },
            _ => {}
        }
    }
    out
}

/// The `archive` op. Read again under the lock: an offer since the sweep read the index keeps the lesson.
pub fn archive(root: &Path, slug: &str, at: u64) -> Result<String, String> {
    let slugs = [slug.to_owned()];
    ledgered(root, "archive", BY, SWEEP_RUN, at, &slugs, |index, id| {
        let Some(entry) = index.lessons.get(slug) else {
            return Err(format!("no lesson {slug} to archive"));
        };
        if life(entry, at) != Life::Due {
            return Err(format!(
                "{slug} is not unused {} days",
                ARCHIVE_AFTER_S / 86_400
            ));
        }
        to_archive(root, index, slug, id)
    })
    .map(|(_, id)| id)
}

/// `pin` or `unpin` as a ledger op. An unpin starts the lesson's clock again: with its old `used_at`, a lesson pinned
/// for months would be archived by the next sweep.
pub fn pin(root: &Path, slug_in: &str, on: bool, at: u64) -> Result<String, String> {
    if slug(slug_in) != slug_in {
        return Err(format!("{slug_in:?} is not a lesson slug"));
    }
    let op = if on { "pin" } else { "unpin" };
    let slugs = [slug_in.to_owned()];
    ledgered(root, op, BY, OWNER_RUN, at, &slugs, |index, _| {
        let Some(entry) = index.lessons.get_mut(slug_in) else {
            return Err(format!("no lesson {slug_in}"));
        };
        if entry.pinned == on {
            return Err(format!("{slug_in} is {op}ned already"));
        }
        entry.pinned = on;
        if !on {
            entry.used_at = Some(at);
        }
        Ok(slug_in.to_owned())
    })
    .map(|(_, id)| id)
}

/// The op `undo` takes: `target` itself when it is an op id, else the newest op on that lesson that is not a rollback
/// and not rolled back.
pub fn undo_target(root: &Path, target: &str) -> Result<String, String> {
    if is_op_id(target) {
        return Ok(target.to_owned());
    }
    if slug(target) != target {
        return Err(format!("{target:?} is not an op id or a lesson slug"));
    }
    let rows = ledger(root);
    rows.iter()
        .rev()
        .filter(|row| row.op != "rollback")
        .filter(|row| {
            !rows
                .iter()
                .any(|r| r.of.as_deref() == Some(row.id.as_str()))
        })
        .find(|row| {
            row.before
                .iter()
                .chain(&row.after)
                .any(|s| s.slug == target)
        })
        .map(|row| row.id.clone())
        .ok_or_else(|| format!("no op on {target} to undo"))
}

/// An op id as `next_id` makes it, `op-` and digits. Plan 73 review 5: a bare `op-` prefix took a slug such as
/// `op-amp-wiring` for an op id, so its undo failed with "no op".
fn is_op_id(target: &str) -> bool {
    target
        .strip_prefix("op-")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// `herdr lessons undo <op-id|slug>`: `rollback` of `undo_target`.
pub fn undo(root: &Path, target: &str, at: u64) -> Result<String, String> {
    rollback(root, &undo_target(root, target)?, at)
}

/// One archived lesson that is still archived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archived {
    pub id: String,
    pub op: String,
    pub slug: String,
    pub at: u64,
    pub file: PathBuf,
}

/// Every lesson an `archive`, `retire` or `merge` op moved to `.archive/` whose op is not rolled back, oldest first.
pub fn archived(root: &Path) -> Vec<Archived> {
    let rows = ledger(root);
    let mut out = Vec::new();
    for row in &rows {
        if !matches!(row.op.as_str(), "archive" | "retire" | "merge")
            || rows
                .iter()
                .any(|r| r.of.as_deref() == Some(row.id.as_str()))
        {
            continue;
        }
        for was in row.before.iter().filter(|s| s.file.is_some()) {
            let gone = row
                .after
                .iter()
                .any(|s| s.slug == was.slug && s.file.is_none());
            if gone && slug(&was.slug) == was.slug {
                out.push(Archived {
                    id: row.id.clone(),
                    op: row.op.clone(),
                    slug: was.slug.clone(),
                    at: row.at,
                    file: dir(root)
                        .join(ARCHIVE)
                        .join(format!("{}-{}.md", row.id, was.slug)),
                });
            }
        }
    }
    out
}

pub fn sweep_report(done: &Swept, dry: bool) -> String {
    let mut text = String::new();
    if !done.dated.is_empty() {
        text.push_str(&format!(
            "{} {} no use on record. Their clock starts now.\n",
            done.dated.len(),
            if done.dated.len() == 1 {
                "lesson had"
            } else {
                "lessons had"
            }
        ));
    }
    for (slug, days) in &done.stale {
        text.push_str(&format!("stale: {slug}, unused {days} days.\n"));
    }
    for (slug, id) in &done.archived {
        if dry {
            text.push_str(&format!("would archive {slug}.\n"));
        } else {
            text.push_str(&format!(
                "archived {slug} as {id}. Undo with herdr lessons undo {id}.\n"
            ));
        }
    }
    for (slug, why) in &done.failed {
        text.push_str(&format!("could not archive {slug}: {why}.\n"));
    }
    if text.is_empty() {
        text.push_str("no lesson is stale.\n");
    }
    text
}

/// `herdr lessons undo|pin|unpin|sweep|archived`, as text to print.
pub fn cli(root: &Path, verb: &str, args: &[String], now: u64) -> Result<String, String> {
    let one = || match args {
        [one] => Ok(one.as_str()),
        _ => Err(format!("usage: herdr lessons {verb} <id>")),
    };
    match verb {
        "undo" => undo(root, one()?, now).map(|line| format!("{line}\n")),
        "pin" | "unpin" => {
            let slug = one()?;
            let id = pin(root, slug, verb == "pin", now)?;
            Ok(format!(
                "{id}: {verb}ned {slug}. Undo with herdr lessons undo {id}.\n"
            ))
        }
        "sweep" => {
            let dry = match args {
                [] => false,
                [flag] if flag == "--dry-run" => true,
                _ => return Err("usage: herdr lessons sweep [--dry-run]".into()),
            };
            Ok(sweep_report(&sweep(root, now, dry), dry))
        }
        "archived" => {
            let all = archived(root);
            if all.is_empty() {
                return Ok("no lesson is archived.\n".into());
            }
            Ok(all
                .iter()
                .map(|a| {
                    format!(
                        "{} {} {}, {} days ago: {}. Undo with herdr lessons undo {}.\n",
                        a.id,
                        a.op,
                        a.slug,
                        now.saturating_sub(a.at) / 86_400,
                        a.file.display(),
                        a.id
                    )
                })
                .collect())
        }
        _ => Err(format!("no verb {verb}")),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        body, count_offered_at, merge, retire, verify_and_write, verify_and_write_noted, Counters,
        Index, Meta, Offer, Origin, Verdict, INDEX, LEDGER,
    };
    use super::*;
    use std::collections::BTreeMap;

    const DAY: u64 = 86_400;
    const T0: u64 = 1_000 * DAY;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "herdr-p73-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// A lesson file and its entry, last used at `used_at`.
    fn plant(root: &Path, slug: &str, used_at: Option<u64>, offered: u32) {
        std::fs::create_dir_all(dir(root)).expect("lessons");
        std::fs::write(
            dir(root).join(format!("{slug}.md")),
            format!("---\nname: {slug}\nclass: build.bounded\n---\n\n1. {slug}\n"),
        )
        .expect("file");
        let mut index: Index = read_index(root);
        index.lessons.insert(
            slug.to_owned(),
            Entry {
                name: slug.to_owned(),
                class: "build.bounded".into(),
                description: slug.to_owned(),
                order: "o".into(),
                run: "r".into(),
                written_at: 0,
                counters: Counters {
                    offered,
                    opened: 0,
                    passed_after_opened: 0,
                },
                seen: Vec::new(),
                profile: None,
                origin: Origin::Worker,
                used_at,
                pinned: false,
            },
        );
        std::fs::write(
            dir(root).join(INDEX),
            serde_json::to_string_pretty(&index).unwrap(),
        )
        .expect("index");
    }

    fn entry(root: &Path, slug: &str) -> Option<Entry> {
        read_index(root).lessons.get(slug).cloned()
    }

    #[test]
    fn p73_a_lesson_unused_14_days_is_stale_and_archived_at_30_never_deleted() {
        let root = temp("life");
        plant(&root, "read-the-total", Some(T0), 2);
        let text = body(&root, "read-the-total").expect("body");
        assert_eq!(
            life(&entry(&root, "read-the-total").unwrap(), T0 + 13 * DAY),
            Life::Active
        );
        let day14 = sweep(&root, T0 + 14 * DAY, false);
        assert_eq!(day14.stale, vec![("read-the-total".to_owned(), 14)]);
        assert!(day14.archived.is_empty(), "{day14:?}");
        assert!(
            entry(&root, "read-the-total").is_some(),
            "a stale lesson is kept"
        );
        let dry = sweep(&root, T0 + 30 * DAY, true);
        assert_eq!(
            dry.archived,
            vec![("read-the-total".to_owned(), String::new())]
        );
        assert!(
            entry(&root, "read-the-total").is_some(),
            "a dry run moves nothing"
        );
        let day30 = sweep(&root, T0 + 30 * DAY, false);
        let [(slug, id)] = day30.archived.as_slice() else {
            panic!("{day30:?}")
        };
        assert_eq!(slug, "read-the-total");
        assert!(entry(&root, slug).is_none(), "out of the index");
        assert!(
            !dir(&root).join(format!("{slug}.md")).exists(),
            "out of lessons/"
        );
        let kept = dir(&root).join(ARCHIVE).join(format!("{id}-{slug}.md"));
        assert_eq!(
            std::fs::read_to_string(&kept)
                .expect("archived copy")
                .replace("\r\n", "\n"),
            text,
            "never deleted"
        );
        let row = ledger(&root)
            .into_iter()
            .find(|r| &r.id == id)
            .expect("row");
        assert_eq!(row.op, "archive");
        assert_eq!(row.before[0].file.as_deref(), Some(text.as_str()));
        assert_eq!(row.after[0].file, None);
        assert_eq!(archived(&root).len(), 1);
        // Undo by the lesson's name, and the next sweep keeps it: the undo is a use.
        undo(&root, slug, T0 + 31 * DAY).expect("undo");
        assert_eq!(body(&root, slug).as_deref(), Some(text.as_str()));
        assert_eq!(entry(&root, slug).unwrap().counters.offered, 2);
        assert!(sweep(&root, T0 + 31 * DAY, false).archived.is_empty());
        assert!(
            archived(&root).is_empty(),
            "an undone archive is not listed"
        );
    }

    #[test]
    fn p73_a_pinned_lesson_is_never_stale_archived_or_retired() {
        let root = temp("pin");
        plant(&root, "keep-me", Some(T0), 6);
        let id = pin(&root, "keep-me", true, T0 + DAY).expect("pin");
        assert!(
            pin(&root, "keep-me", true, T0 + DAY).is_err(),
            "pinned already"
        );
        let late = T0 + 60 * DAY;
        assert_eq!(life(&entry(&root, "keep-me").unwrap(), late), Life::Pinned);
        let swept = sweep(&root, late, false);
        assert!(
            swept.archived.is_empty() && swept.stale.is_empty(),
            "{swept:?}"
        );
        assert!(
            retire(&root, "keep-me", "o", "r", late).is_err(),
            "never retired"
        );
        assert!(entry(&root, "keep-me").is_some());
        // Undo of the pin keeps the offers counted since.
        let mut index = read_index(&root);
        index.lessons.get_mut("keep-me").unwrap().counters.offered = 9;
        std::fs::write(
            dir(&root).join(INDEX),
            serde_json::to_string(&index).unwrap(),
        )
        .unwrap();
        undo(&root, &id, late).expect("undo pin");
        let back = entry(&root, "keep-me").unwrap();
        assert!(!back.pinned);
        assert_eq!(back.counters.offered, 9);
        // An unpin starts the clock again. The unpin comes 40 days after the undo above, since an undo is a use too
        // (review 1): unpinned at `late`, the clock would read `late` with or without the restart.
        pin(&root, "keep-me", true, late).expect("pin again");
        let unpinned = late + 40 * DAY;
        pin(&root, "keep-me", false, unpinned).expect("unpin");
        assert_eq!(entry(&root, "keep-me").unwrap().used_at, Some(unpinned));
        assert!(sweep(&root, unpinned + DAY, false).archived.is_empty());
        let ops: Vec<String> = ledger(&root).into_iter().map(|r| r.op).collect();
        assert!(
            ops.contains(&"pin".to_owned()) && ops.contains(&"unpin".to_owned()),
            "{ops:?}"
        );
    }

    #[test]
    fn p73_an_offer_is_a_use_and_an_undated_lesson_is_dated_not_archived() {
        let root = temp("use");
        plant(&root, "old-one", None, 0);
        let first = sweep(&root, T0 + 100 * DAY, false);
        assert_eq!(first.dated, vec!["old-one".to_owned()]);
        assert!(first.archived.is_empty(), "{first:?}");
        assert_eq!(
            entry(&root, "old-one").unwrap().used_at,
            Some(T0 + 100 * DAY)
        );
        let offer = Offer {
            section: String::new(),
            slugs: vec!["old-one".into()],
        };
        count_offered_at(&root, &offer, T0 + 125 * DAY);
        let used = entry(&root, "old-one").unwrap();
        assert_eq!(
            (used.used_at, used.counters.offered),
            (Some(T0 + 125 * DAY), 1)
        );
        assert!(
            sweep(&root, T0 + 140 * DAY, false).archived.is_empty(),
            "offered 15 days ago"
        );
    }

    /// A claude stream whose one command printed `output`.
    fn stream(output: &str) -> String {
        [
            serde_json::json!({"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}}]}}),
            serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","content":output}]}}),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n")
    }

    #[test]
    fn p73_a_workers_write_is_a_ledger_op_that_undo_takes_back() {
        let root = temp("write");
        let quote = "TOTAL 12 passed, 0 failed";
        let write = |steps: &str, order: &str, at: u64| {
            let lesson = serde_json::json!({"name":"Read the total","class":"build.bounded","steps":steps,"evidence":quote});
            verify_and_write(
                &root,
                &lesson,
                true,
                &stream(quote),
                order,
                "r1",
                at,
                Meta::default(),
            )
        };
        assert!(matches!(
            write("1. read TOTAL", "o1", T0),
            Verdict::Written(_)
        ));
        let first = body(&root, "read-the-total").expect("first");
        assert_eq!(
            entry(&root, "read-the-total").unwrap().used_at,
            Some(T0),
            "a write is a use"
        );
        assert!(matches!(
            write("1. read TOTAL last", "o2", T0 + 1),
            Verdict::Written(_)
        ));
        let rows = ledger(&root);
        assert_eq!(
            rows.iter()
                .map(|r| (r.op.as_str(), r.by.as_str()))
                .collect::<Vec<_>>(),
            vec![("write", "o1"), ("write", "o2")]
        );
        assert_eq!(rows[1].before[0].file.as_deref(), Some(first.as_str()));
        undo(&root, "read-the-total", T0 + 2).expect("undo the overwrite");
        assert_eq!(
            body(&root, "read-the-total").as_deref(),
            Some(first.as_str())
        );
        undo(&root, "read-the-total", T0 + 3).expect("undo the first write");
        assert!(body(&root, "read-the-total").is_none());
        assert!(
            undo(&root, "read-the-total", T0 + 4).is_err(),
            "nothing left to undo"
        );
    }

    /// The gate's write of `steps` as "Read the total", by order `order` at `at`.
    fn gate_write(root: &Path, steps: &str, order: &str, at: u64) -> (Verdict, Option<String>) {
        let quote = "TOTAL 12 passed, 0 failed";
        let lesson = serde_json::json!({"name":"Read the total","class":"build.bounded","steps":steps,"evidence":quote});
        verify_and_write_noted(
            root,
            &lesson,
            true,
            &stream(quote),
            order,
            "r1",
            at,
            Meta::default(),
        )
    }

    /// Review 1: a rollback restores the entry from before the op, and that `used_at` can be weeks old. The rollback
    /// keeps the newest use, so the next sweep does not archive what was just taken back.
    #[test]
    fn p73_a_rollback_keeps_the_newest_use_so_the_sweep_keeps_it() {
        let root = temp("rb-use");
        plant(&root, "tip-one", Some(T0), 0);
        plant(&root, "tip-two", Some(T0), 0);
        let parts = vec!["tip-one".to_owned(), "tip-two".to_owned()];
        let (_, id) = merge(
            &root,
            &parts,
            "Tips",
            "1. both",
            &BTreeMap::new(),
            "o",
            "r",
            T0 + 29 * DAY,
        )
        .expect("merge");
        undo(&root, &id, T0 + 31 * DAY).expect("undo the merge");
        for part in &parts {
            assert_eq!(
                entry(&root, part).unwrap().used_at,
                Some(T0 + 31 * DAY),
                "{part}"
            );
        }
        let swept = sweep(&root, T0 + 31 * DAY + 600, false);
        assert!(swept.archived.is_empty(), "merge parts kept: {swept:?}");
        // A write over a lesson last used 20 days before it, undone a day later.
        plant(&root, "read-the-total", Some(T0 - 20 * DAY), 0);
        assert!(matches!(
            gate_write(&root, "1. read TOTAL", "o1", T0).0,
            Verdict::Written(_)
        ));
        undo(&root, "read-the-total", T0 + DAY).expect("undo the write");
        assert_eq!(
            entry(&root, "read-the-total").unwrap().used_at,
            Some(T0 + DAY)
        );
        let swept = sweep(&root, T0 + 11 * DAY, false);
        assert!(
            swept.archived.is_empty(),
            "the older lesson kept: {swept:?}"
        );
    }

    /// Review 2: a merge archives its parts, so a pinned part is refused, as retire refuses it, and nothing moves.
    #[test]
    fn p73_a_merge_naming_a_pinned_lesson_is_refused() {
        let root = temp("merge-pin");
        plant(&root, "tips", Some(T0), 0);
        plant(&root, "tip-two", Some(T0), 0);
        pin(&root, "tips", true, T0).expect("pin");
        let rows_before = ledger(&root).len();
        let index_before = read_index(&root);
        let why = merge(
            &root,
            &["tips".to_owned(), "tip-two".to_owned()],
            "Tips",
            "1. both",
            &BTreeMap::new(),
            "o",
            "r",
            T0 + DAY,
        )
        .expect_err("a pinned part");
        assert!(why.contains("tips is pinned"), "{why}");
        assert_eq!(read_index(&root), index_before);
        assert_eq!(ledger(&root).len(), rows_before);
        assert!(body(&root, "tips").is_some() && body(&root, "tip-two").is_some());
    }

    /// Review 3: a pin changes the entry, not the file. Rolling back the write under it would remove a pinned lesson.
    #[test]
    fn p73_a_rollback_under_a_later_pin_is_refused() {
        let root = temp("rb-pin");
        assert!(matches!(
            gate_write(&root, "1. read TOTAL", "o1", T0).0,
            Verdict::Written(_)
        ));
        let write = ledger(&root)[0].id.clone();
        let pinned = pin(&root, "read-the-total", true, T0 + 1).expect("pin");
        let why = rollback(&root, &write, T0 + 2).expect_err("pinned since");
        assert!(why.contains("pinned or unpinned since"), "{why}");
        let kept = entry(&root, "read-the-total").expect("kept");
        assert!(kept.pinned);
        // Undo the pin first, and the write rolls back.
        rollback(&root, &pinned, T0 + 3).expect("undo the pin");
        rollback(&root, &write, T0 + 4).expect("then the write");
        assert!(entry(&root, "read-the-total").is_none());
    }

    /// Review 4: an unwritable ledger keeps a lesson that passed the gate, with a note for the gate's axis.
    #[test]
    fn p73_an_unwritable_ledger_keeps_the_gates_lesson_with_a_note() {
        let root = temp("ledger-locked");
        // A folder where the ledger file goes: an append to it fails, as an antivirus or git lock makes it fail.
        std::fs::create_dir_all(dir(&root).join(LEDGER)).expect("folder");
        let (verdict, note) = gate_write(&root, "1. read TOTAL", "o1", T0);
        assert!(matches!(verdict, Verdict::Written(_)), "{verdict:?}");
        assert!(
            note.as_deref()
                .is_some_and(|n| n.starts_with("not in the ledger")),
            "{note:?}"
        );
        assert!(body(&root, "read-the-total").is_some(), "the file is kept");
        assert!(
            entry(&root, "read-the-total").is_some(),
            "the entry is kept"
        );
        // A review's op still refuses on the same ledger, as before.
        assert!(pin(&root, "read-the-total", true, T0 + 1).is_err());
        assert!(!entry(&root, "read-the-total").unwrap().pinned);
    }

    /// Review 5: a slug that starts with `op-` is a slug, not an op id.
    #[test]
    fn p73_undo_takes_a_slug_that_starts_with_op() {
        let root = temp("op-slug");
        plant(&root, "op-amp-wiring", Some(T0), 0);
        let id = pin(&root, "op-amp-wiring", true, T0).expect("pin");
        assert_eq!(
            undo_target(&root, "op-amp-wiring").as_deref(),
            Ok(id.as_str())
        );
        assert_eq!(undo_target(&root, "op-12").as_deref(), Ok("op-12"));
        undo(&root, "op-amp-wiring", T0 + 1).expect("undo by slug");
        assert!(!entry(&root, "op-amp-wiring").unwrap().pinned);
    }

    #[test]
    fn p73_the_cli_says_what_it_did_in_plain_sentences() {
        let root = temp("cli");
        plant(&root, "gone-quiet", Some(T0), 1);
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let now = T0 + 30 * DAY;
        let swept = cli(&root, "sweep", &args(&[]), now).expect("sweep");
        assert!(
            swept.starts_with("archived gone-quiet as op-1. Undo with herdr lessons undo op-1."),
            "{swept:?}"
        );
        let listed = cli(&root, "archived", &args(&[]), now).expect("archived");
        assert!(
            listed.starts_with("op-1 archive gone-quiet, 0 days ago: "),
            "{listed:?}"
        );
        assert!(cli(&root, "pin", &args(&["../x"]), now).is_err());
        for text in [&swept, &listed] {
            assert!(
                !text.contains('\u{2014}') && !text.contains('('),
                "{text:?}"
            );
        }
    }
}
