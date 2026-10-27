//! Nirvana fork, plan 25 §2: the server's half of a fresh session after a run ends.
//!
//! The rule lives in `nirvana_fresh::decide`; this file gathers its facts and applies the answer, with idle compact's
//! own machinery (plan 27, `app/nirvana_idle_compact.rs`): the strict input line read from the pane's screen, cursor
//! and scroll, the owner's last key, the paste, then the reads after it (`ENTER_TRIES`), Enter only at a read of
//! exactly `/clear`, and the erase of a paste whose Enter never went. Each pass does the work of at most one terminal.
//!
//! The two never race on a pane: the fresh start gets no paste while a compact's paste is in flight
//! (`Facts::compact_in_flight`), and the compact's pass leaves a pane whose `/clear` is in flight
//! (`FreshStretch::busy`). The server's loop runs the compact first, so a compact pasted in a pass holds the fresh
//! start off in the same pass.

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use bytes::Bytes;

use super::nirvana_idle_compact::{input_line_of, owner_key_of, write_keys};
use super::App;
use crate::nirvana_fresh::{after_clear_paste, decide, Decision, Facts, CLEAR, CLEAR_ERASE_KEYS};
use crate::nirvana_idle_compact::{
    read_outside_and_transcript, AfterPaste, InputLine, Outside, PaneQuery, Pending, Step,
};
use crate::terminal::{TerminalId, TerminalRuntime};

/// One Backspace per character of `/clear`, in the key encoding the pane's terminal uses (as idle compact's erase).
fn clear_erase_keys(runtime: &TerminalRuntime) -> Vec<u8> {
    let backspace = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Backspace,
        crossterm::event::KeyModifiers::NONE,
    );
    runtime
        .encode_terminal_key(backspace.into())
        .repeat(CLEAR_ERASE_KEYS)
}

/// Whether a Claude process's argv is Nirvana Claude's, for the wake's first rule.
fn is_nirvana(argv: &[String], home: &Path) -> bool {
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    crate::nirvana_run::is_nirvana_claude_argv(
        &argv,
        Some(&crate::nirvana_launch::plugin_dir(home)),
    )
}

impl App {
    /// The terminals a fresh start looks at: Claude terminals herdr started as Nirvana Claude (the agent name
    /// `nirvana` or `nirvana-<n>` that New Nirvana, the verb and Restart Claude give). The argv is read again before
    /// any paste (`decide`); the name keeps every other Claude pane off the deadline loop.
    fn fresh_terminals(
        &self,
    ) -> impl Iterator<Item = (&TerminalId, &crate::terminal::TerminalState)> {
        self.idle_compact_terminals().filter(|(_, terminal)| {
            terminal
                .agent_name
                .as_deref()
                .is_some_and(crate::nirvana_launch::is_nirvana_agent_name)
        })
    }

    /// When the next Nirvana Claude terminal is due a fresh-start check, for the server's deadline loop.
    pub(crate) fn fresh_deadline(&self) -> Option<Instant> {
        self.fresh_terminals()
            .filter_map(|(_, terminal)| {
                terminal
                    .idle_stretch
                    .fresh
                    .next_check_at(terminal.idle_stretch.since)
            })
            .min()
    }

    /// Checks the Claude terminal due first, and types `/clear` where `decide` says to. True when a key was typed.
    pub(crate) fn nirvana_fresh(&mut self, now: Instant) -> bool {
        if self.fresh_deadline().is_none_or(|due| now < due) {
            return false;
        }
        let Some(home) = crate::nirvana::home() else {
            return false;
        };
        // Review S1: the argv and the pane's own transcript, which a Stop's record must still name.
        let claude_dir = crate::nirvana_subagents::claude_config_dir();
        self.nirvana_fresh_with(now, &home, |query| {
            read_outside_and_transcript(query, claude_dir.as_deref(), SystemTime::now())
        })
    }

    /// `read` gives the Claude process's argv and the pane's transcript, so a test passes a stand-in.
    fn nirvana_fresh_with(
        &mut self,
        now: Instant,
        home: &Path,
        read: impl Fn(&PaneQuery) -> (Outside, Option<PathBuf>),
    ) -> bool {
        let Some(terminal_id) = self
            .fresh_terminals()
            .filter_map(|(id, terminal)| {
                let due = terminal
                    .idle_stretch
                    .fresh
                    .next_check_at(terminal.idle_stretch.since)?;
                Some((due, id))
            })
            .filter(|(due, _)| *due <= now)
            .min_by_key(|(due, _)| *due)
            .map(|(_, id)| id.clone())
        else {
            return false;
        };
        let Some(terminal) = self.state.terminals.get_mut(&terminal_id) else {
            return false;
        };
        if let Some(pending) = terminal.idle_stretch.fresh.pending.take() {
            return self.fresh_after_paste(&terminal_id, pending, home);
        }
        // Stamped first, pane or not, so a check that finds nothing waits for its next time.
        let fresh = &mut terminal.idle_stretch.fresh;
        fresh.checked_at = Some(now);
        fresh.checks += 1;
        let compact_in_flight = terminal.idle_stretch.pending.is_some();
        let compact_sent = terminal.idle_stretch.sent;
        let state = terminal.state;
        let panes = self.claude_panes();
        let Some(pane) = panes.iter().find(|pane| pane.terminal_id == terminal_id) else {
            return false;
        };
        let record = crate::nirvana_fresh::read_record(home, &pane.public);
        let runtime = self.terminal_runtimes.get(&terminal_id);
        // The process and the screen are read only when a record waits: a poll that finds none costs one file read.
        let waiting = matches!(record, Ok(Some(_)));
        let (nirvana, transcript) = if waiting && !compact_in_flight && !compact_sent {
            // The query idle compact builds, so `choose_transcript` tells this pane's conversation from another
            // Claude pane's in the same folder.
            let (outside, transcript) = read(&PaneQuery {
                pane_id: pane.public.clone(),
                shell_pid: runtime.and_then(TerminalRuntime::child_pid),
                folder: pane.folder.clone(),
                hook_session: pane.hook_session.clone(),
                others: panes
                    .iter()
                    .filter(|other| other.terminal_id != pane.terminal_id)
                    .filter_map(|other| {
                        Some((
                            other.public.clone(),
                            other.folder.clone()?,
                            other.hook_session.clone(),
                        ))
                    })
                    .collect(),
            });
            (outside.argv.map(|argv| is_nirvana(&argv, home)), transcript)
        } else {
            (None, None)
        };
        let input_line = runtime.map_or_else(
            || InputLine::Unsure("the pane has no runtime".into()),
            input_line_of,
        );
        let facts = Facts {
            compact_in_flight,
            compact_sent,
            record,
            now: crate::nirvana_run::now(),
            sent_for: crate::nirvana_fresh::read_sent(home, &pane.public),
            nirvana,
            transcript,
            state,
            owner_key: owner_key_of(&pane.public),
            input_line,
        };
        let public = pane.public.clone();
        // Review round 2, S-A: B1's rules again before a paste, over the record's journals and the pane's transcript
        // now, as the Stop hook that would take the record back may be late or killed.
        let recheck = |record: &crate::nirvana_fresh::Record| {
            crate::nirvana_fresh::recheck_in(
                record,
                &public,
                facts.transcript.as_deref(),
                facts.now,
            )
        };
        match decide(&facts, recheck) {
            Decision::Nothing => false,
            Decision::Skip(why) => {
                if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
                    terminal.idle_stretch.fresh.found = true;
                }
                tracing::debug!(pane = %public, why, "nirvana: the fresh start left the pane alone");
                false
            }
            Decision::Forget(why) => {
                crate::nirvana_fresh::forget(home, &public);
                tracing::info!(pane = %public, why, "nirvana: the fresh start forgot a stale record");
                false
            }
            Decision::Send(at) => self.fresh_paste(&terminal_id, &public, now, at),
        }
    }

    /// The paste of `/clear`. The reads after it are set only once the paste is queued.
    fn fresh_paste(
        &mut self,
        terminal_id: &TerminalId,
        public: &str,
        now: Instant,
        at: u64,
    ) -> bool {
        let Some(runtime) = self.terminal_runtimes.get(terminal_id) else {
            return false;
        };
        let (text, _) = super::api_helpers::encode_api_submission_parts(runtime, CLEAR);
        if let Err(err) = runtime.try_send_bytes(Bytes::from(text)) {
            tracing::warn!(pane = %public, err = %err, "nirvana: the fresh start could not queue /clear");
            return false;
        }
        if let Some(terminal) = self.state.terminals.get_mut(terminal_id) {
            let fresh = &mut terminal.idle_stretch.fresh;
            fresh.found = true;
            fresh.record_at = Some(at);
            fresh.pending = Some(Pending {
                pane: public.to_owned(),
                pasted_at: now,
                step: Step::Try(0),
            });
        }
        tracing::info!(pane = %public, "nirvana: a run ended; pasted /clear for a fresh session");
        true
    }

    /// One read after the paste, on the pane id the paste's pass found. True when a key was typed.
    fn fresh_after_paste(
        &mut self,
        terminal_id: &TerminalId,
        pending: Pending,
        home: &Path,
    ) -> bool {
        let public = pending.pane.clone();
        let Some(runtime) = self.terminal_runtimes.get(terminal_id) else {
            tracing::warn!(pane = %public, "nirvana: the fresh start lost the pane after the paste; /clear was left unsent");
            return false;
        };
        let line = input_line_of(runtime);
        match after_clear_paste(pending.step, &line, &owner_key_of(&public)) {
            AfterPaste::Enter => {
                let (_, enter) = super::api_helpers::encode_api_submission_parts(runtime, CLEAR);
                if let Err(err) = write_keys(runtime, enter) {
                    tracing::warn!(pane = %public, err = %err, "nirvana: the fresh start's Enter failed; /clear was left unsent");
                    return false;
                }
                let record_at = self
                    .state
                    .terminals
                    .get_mut(terminal_id)
                    .and_then(|terminal| {
                        terminal.idle_stretch.fresh.sent = true;
                        terminal.idle_stretch.fresh.record_at
                    });
                // The mark keeps one record from being cleared for twice, should the note's hook never take it.
                if let Some(at) = record_at {
                    if let Err(err) = crate::nirvana_fresh::write_sent(home, &public, at) {
                        tracing::warn!(pane = %public, err = %err, "nirvana: the fresh start's mark was not written");
                    }
                }
                tracing::info!(pane = %public, "nirvana: sent /clear for a fresh session");
                true
            }
            AfterPaste::Next(step) => {
                if let Some(terminal) = self.state.terminals.get_mut(terminal_id) {
                    terminal.idle_stretch.fresh.pending = Some(Pending { step, ..pending });
                }
                false
            }
            AfterPaste::Erase => match write_keys(runtime, clear_erase_keys(runtime)) {
                Ok(()) => {
                    if let Some(terminal) = self.state.terminals.get_mut(terminal_id) {
                        terminal.idle_stretch.fresh.pending = Some(Pending {
                            step: Step::Erased,
                            ..pending
                        });
                    }
                    true
                }
                Err(err) => {
                    tracing::warn!(pane = %public, err = %err, "nirvana: the fresh start could not erase the unsent /clear");
                    false
                }
            },
            AfterPaste::Erased => {
                tracing::info!(pane = %public, "nirvana: the fresh start erased the unsent /clear");
                false
            }
            AfterPaste::Leave(why) => {
                tracing::warn!(pane = %public, why, "nirvana: the fresh start left the input line as it was");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests;
