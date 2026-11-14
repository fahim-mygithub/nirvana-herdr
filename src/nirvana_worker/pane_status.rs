//! Nirvana fork, alpha 0.72.0: `herdr seat exec` reports its order's status to its own pane, with the program status
//! protocol (OSC 7501, `crate::terminal::program_status`). The workers it runs, Claude Code and agy, report nothing,
//! and a headless worker draws no screen herdr's detector reads. Measured 2026-11-13 in the owner's session: live
//! worker panes read "idle" in `herdr agent list`, with the folder of the order that used the pane before.
//!
//! The folder came from the pane's last PowerShell prompt. The launch line runs `Set-Location` and `seat exec` on one
//! line, so no prompt shows the new folder until the worker ends. `seat exec` now also writes OSC 7 with its folder.
//!
//! A detached `seat exec` writes neither: its stdout is `streams/<order>.log`, and escapes there would be noise.

use std::io::{IsTerminal as _, Write};
use std::path::Path;

use crate::nirvana_run::Phase;
use crate::terminal::program_status::{encode_report, ProgramState};

/// The record's `app`: one name for every Nirvana worker, whatever CLI runs under it.
const APP: &str = "nirvana";

pub(super) struct PaneStatus {
    on: bool,
}

impl PaneStatus {
    /// On only for a pane launch (`--pane`) whose stdout is the pane's terminal.
    pub(super) fn new(pane: Option<&str>) -> Self {
        let in_pane = std::env::var(crate::integration::HERDR_PANE_ID_ENV_VAR).ok();
        Self {
            on: on_for(pane, in_pane.as_deref(), std::io::stdout().is_terminal()),
        }
    }

    /// Writes `working` and the folder, runs the order, then writes how it ended, also when `exec` failed: an error
    /// return must not leave the pane reading working.
    pub(super) fn around(
        &self,
        out: &mut dyn Write,
        order: &str,
        cwd: Option<&Path>,
        exec: impl FnOnce(&mut dyn Write) -> std::io::Result<i32>,
        phase: impl FnOnce() -> Option<Phase>,
    ) -> std::io::Result<i32> {
        self.begin(out, order, cwd);
        let code = exec(out);
        self.end(out, order, phase());
        code
    }

    pub(super) fn begin(&self, out: &mut dyn Write, order: &str, cwd: Option<&Path>) {
        if self.on {
            let _ = out.write_all(begin_bytes(order, cwd).as_bytes());
            let _ = out.flush();
        }
    }

    pub(super) fn end(&self, out: &mut dyn Write, order: &str, phase: Option<Phase>) {
        if self.on {
            let _ = out.write_all(end_bytes(order, phase).as_bytes());
            let _ = out.flush();
        }
    }
}

/// Review fix round: a terminal on stdout is not proof it is the pane's. herdr gives each pane's shell its public id
/// in `HERDR_PANE_ID` (`pane.rs`, `PaneLaunchIdentity::Managed`), the same id `--pane` names. A `seat exec --pane w1:p2`
/// typed in another pane, or in a terminal outside herdr, has another id or none, so it writes nothing.
fn on_for(pane: Option<&str>, in_pane: Option<&str>, stdout_is_terminal: bool) -> bool {
    stdout_is_terminal && pane.is_some_and(|pane| in_pane == Some(pane))
}

/// OSC 7 with the order's folder, then `working` with the order id.
pub(super) fn begin_bytes(order: &str, cwd: Option<&Path>) -> String {
    let mut bytes = cwd.map(osc7).unwrap_or_default();
    bytes.push_str(&encode_report(ProgramState::Working, APP, order));
    bytes
}

/// What the order's phase says once `exec` returns. An order that asks waits on the owner: blocked. One a worker
/// delivered for is done, one that failed or was cut is an error, one stopped is idle. A phase that says this pane
/// ran nothing, such as another pane's live worker, clears the record.
pub(super) fn end_bytes(order: &str, phase: Option<Phase>) -> String {
    let state = match phase {
        Some(Phase::Asked) => ProgramState::Blocked,
        Some(Phase::Delivered | Phase::Gated | Phase::Merged | Phase::Reseated) => {
            ProgramState::Done
        }
        Some(Phase::Failed | Phase::Interrupted) => ProgramState::Error,
        Some(Phase::Deferred) => ProgramState::Idle,
        Some(Phase::Seated | Phase::Running) | None => {
            return "\x1b]7501;state=clear\x1b\\".to_owned()
        }
    };
    let report = encode_report(state, APP, order);
    if state == ProgramState::Blocked {
        // The spec's `kind` for a question the owner must answer.
        return report.replacen(":app=", ":kind=question:app=", 1);
    }
    report
}

/// `ESC ] 7 ; file:///C:/dir ESC \`, percent-encoded. herdr's own reader (`parse_reported_cwd`) takes this form.
fn osc7(cwd: &Path) -> String {
    let path = cwd.to_string_lossy().replace('\\', "/");
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    let mut encoded = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'.' | b'_' | b'-' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    format!("\x1b]7;file://{encoded}\x1b\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::program_status::{parse_report, Report};

    fn report_of(bytes: &str) -> Option<Report> {
        let start = bytes.find("\x1b]7501;")? + "\x1b]7501;".len();
        let body = &bytes[start..];
        parse_report(body[..body.find("\x1b\\")?].as_bytes())
    }

    fn state_of(bytes: &str) -> Option<ProgramState> {
        match report_of(bytes)? {
            Report::Set(status) => Some(status.state),
            Report::Clear => None,
        }
    }

    #[test]
    fn a_wake_reads_detection_not_a_report_in_the_orchestrator_pane() {
        // `pane get` for an orchestrator pane where a `cat` wrote a done report over a working Claude.
        let info = serde_json::json!({"result": {"pane": {
            "agent_status": "done",
            "tokens": {"program_status": "done", "detected_status": "working"},
        }}});
        assert_eq!(
            crate::nirvana_worker::pane_decision_status(&info).as_deref(),
            Some("working")
        );
        let plain = serde_json::json!({"pane": {"agent_status": "idle", "tokens": {}}});
        assert_eq!(
            crate::nirvana_worker::pane_decision_status(&plain).as_deref(),
            Some("idle")
        );
    }

    #[test]
    fn seat_exec_reports_working_with_the_order_and_its_folder() {
        let bytes = begin_bytes("c6-candle-noise", Some(Path::new(r"C:\p\my proj\.nirvana")));
        assert!(
            bytes.starts_with("\x1b]7;file:///C:/p/my%20proj/.nirvana\x1b\\"),
            "{bytes:?}"
        );
        let Some(Report::Set(status)) = report_of(&bytes) else {
            panic!("{bytes:?}");
        };
        assert_eq!(status.state, ProgramState::Working);
        assert_eq!(status.msg.as_deref(), Some("c6-candle-noise"));
        assert_eq!(status.app.as_deref(), Some("nirvana"));
    }

    #[test]
    fn seat_exec_end_follows_the_orders_phase() {
        assert_eq!(
            state_of(&end_bytes("o", Some(Phase::Asked))),
            Some(ProgramState::Blocked)
        );
        assert_eq!(
            state_of(&end_bytes("o", Some(Phase::Delivered))),
            Some(ProgramState::Done)
        );
        assert_eq!(
            state_of(&end_bytes("o", Some(Phase::Gated))),
            Some(ProgramState::Done)
        );
        assert_eq!(
            state_of(&end_bytes("o", Some(Phase::Failed))),
            Some(ProgramState::Error)
        );
        assert_eq!(
            state_of(&end_bytes("o", Some(Phase::Interrupted))),
            Some(ProgramState::Error)
        );
        assert_eq!(
            report_of(&end_bytes("o", Some(Phase::Running))),
            Some(Report::Clear)
        );
        assert_eq!(report_of(&end_bytes("o", None)), Some(Report::Clear));
        let Some(Report::Set(asked)) = report_of(&end_bytes("o", Some(Phase::Asked))) else {
            panic!("asked");
        };
        assert_eq!(asked.kind.as_deref(), Some("question"));
        assert_eq!(asked.msg.as_deref(), Some("o"));
    }

    #[test]
    fn seat_exec_writes_only_to_the_pane_it_names() {
        assert!(on_for(Some("w1:p2"), Some("w1:p2"), true));
        assert!(!on_for(Some("w1:p2"), Some("w1:p3"), true), "another pane's terminal");
        assert!(!on_for(Some("w1:p2"), None, true), "a terminal outside herdr");
        assert!(!on_for(Some("w1:p2"), Some("w1:p2"), false), "stdout is a log");
        assert!(!on_for(None, Some("w1:p2"), true), "no --pane");
    }

    #[test]
    fn seat_exec_reports_around_the_order_and_after_an_error() {
        let on = PaneStatus { on: true };
        let mut out = Vec::new();
        let code = on.around(
            &mut out,
            "o1",
            None,
            |out| {
                out.write_all(b"<worker>")?;
                Ok(3)
            },
            || Some(Phase::Delivered),
        );
        assert_eq!(code.unwrap(), 3);
        let text = String::from_utf8(out).unwrap();
        let worker = text.find("<worker>").unwrap();
        assert_eq!(state_of(&text[..worker]), Some(ProgramState::Working));
        assert_eq!(state_of(&text[worker..]), Some(ProgramState::Done));

        let mut out = Vec::new();
        let code = on.around(
            &mut out,
            "o1",
            None,
            |_| Err(std::io::Error::other("journal")),
            || Some(Phase::Failed),
        );
        assert!(code.is_err());
        let text = String::from_utf8(out).unwrap();
        let last = text.rfind("\x1b]7501;").unwrap();
        assert_eq!(state_of(&text[last..]), Some(ProgramState::Error));
    }

    #[test]
    fn a_detached_seat_exec_writes_no_escape() {
        // No `--pane`: a detached start, whose stdout is a log file.
        let status = PaneStatus::new(None);
        let mut out = Vec::new();
        status.begin(&mut out, "o", Some(Path::new(r"C:\p")));
        status.end(&mut out, "o", Some(Phase::Delivered));
        assert!(out.is_empty());
        // `cargo test` captures stdout, so a pane launch under test is also off: stdout is no terminal there.
        let status = PaneStatus::new(Some("w1:p1"));
        status.begin(&mut out, "o", None);
        assert_eq!(out.is_empty(), !std::io::stdout().is_terminal());
    }
}
