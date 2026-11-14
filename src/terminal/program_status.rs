//! Nirvana fork, alpha 0.72.0: the program status protocol, OSC 7501, spec revision 0.4
//! (https://www.superlogical.com/rex/docs/build/program-status), in a smaller form than upstream PR #5172.
//!
//! A program in a pane says what it is doing: idle, working, blocked, done or error. While a report is in place its
//! state wins over hooks and detection for that pane (`TerminalState::recompute_effective_state`). The owner,
//! 2026-11-14, on the proposal "It fixes the stale idle states at the source.": "Sure lets add the top 3". Measured
//! 2026-11-13: live worker panes read "idle" in `herdr agent list`, because a headless worker draws no screen the
//! detector knows.
//!
//! Only the root record is kept. A report with an `id` addresses a child record, which this form does not store, so
//! it is ignored; a `state=clear` with no `id` still clears the root, as the spec says every record goes.

use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine as _;

/// The reply to `OSC 7501 ; ?`. No `states` and no `kinds` means the spec's fixed lists, which this form takes whole.
pub(crate) const SUPPORT_REPLY: &[u8] = b"\x1b]7501;?\x1b\\";

/// The OSC number, as the body's command.
pub(crate) const OSC_COMMAND: &[u8] = b"7501";

// The spec's Limits table. A report that breaks one is discarded whole.
const MAX_KEY: usize = 16;
const MAX_MSG_ENCODED: usize = 2732;
const MAX_MSG_DECODED: usize = 2048;
const MAX_TITLE_ENCODED: usize = 256;
const MAX_TITLE_DECODED: usize = 192;
const MAX_APP: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramState {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
}

impl ProgramState {
    pub fn word(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Error => "error",
        }
    }

    /// herdr's own state for the pane. `done` and `error` are finished work: idle, which an unseen pane shows as done.
    pub fn agent_state(self) -> crate::detect::AgentState {
        match self {
            Self::Working => crate::detect::AgentState::Working,
            Self::Blocked => crate::detect::AgentState::Blocked,
            Self::Idle | Self::Done | Self::Error => crate::detect::AgentState::Idle,
        }
    }

    /// The spec's lifetime: `working` and `blocked` MUST go when the program exits or a shell prompt begins, `idle`
    /// MAY. herdr drops `idle` too, since a stale idle is the incident. `done` and `error` survive both.
    pub fn ends_with_its_program(self) -> bool {
        matches!(self, Self::Idle | Self::Working | Self::Blocked)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStatus {
    pub state: ProgramState,
    pub kind: Option<String>,
    pub app: Option<String>,
    pub msg: Option<String>,
}

/// What one report does to the root record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    Set(ProgramStatus),
    Clear,
}

/// The `pane list` and `agent list` tokens that carry the record: the state word and the decoded message. A reused
/// pane then names the order that runs in it now, not the one before (`herdr seat exec` puts the order id in `msg`).
pub const TOKEN: &str = "program_status";
pub const MSG_TOKEN: &str = "program_status_msg";

pub fn with_tokens(
    mut tokens: std::collections::HashMap<String, String>,
    status: Option<&ProgramStatus>,
) -> std::collections::HashMap<String, String> {
    if let Some(status) = status {
        tokens.insert(TOKEN.to_owned(), status.state.word().to_owned());
        if let Some(msg) = &status.msg {
            tokens.insert(MSG_TOKEN.to_owned(), msg.clone());
        }
    }
    tokens
}

/// Parses the payload after `7501;`. `None` is a report the spec says to ignore or discard: nothing is applied.
pub(crate) fn parse_report(payload: &[u8]) -> Option<Report> {
    let mut state = None;
    let mut kind = None;
    let mut app = None;
    let mut msg = None;
    let mut id = None;
    // Every pair is checked before the record is touched (spec, Limits).
    for pair in payload.split(|&byte| byte == b':') {
        let Some(eq) = pair.iter().position(|&byte| byte == b'=') else {
            continue;
        };
        let key = trim(&pair[..eq]);
        let value = trim(&pair[eq + 1..]);
        if key.len() > MAX_KEY {
            return None;
        }
        if key.is_empty() || !key.iter().all(u8::is_ascii_lowercase) {
            continue;
        }
        if !value.iter().all(|&byte| is_value_byte(byte)) {
            // Upstream #5172's one departure, kept: an `id` with a byte outside the set rejects the report. Skipping
            // the pair would turn `state=clear:id=a?b` into a clear of every record.
            if key == b"id" {
                return None;
            }
            continue;
        }
        // The last value of a repeated key wins.
        match key {
            b"state" => state = Some(value),
            b"kind" => kind = Some(value),
            b"app" => app = Some(value),
            b"msg" => msg = Some(value),
            b"id" => id = Some(value),
            b"title" => {
                // Not kept, but a title that breaks a limit still discards the report.
                decode_text(value, MAX_TITLE_ENCODED, MAX_TITLE_DECODED)?;
            }
            _ => {}
        }
    }
    let msg = match msg {
        Some(value) => Some(decode_text(value, MAX_MSG_ENCODED, MAX_MSG_DECODED)?),
        None => None,
    };
    let state = match state? {
        b"idle" => ProgramState::Idle,
        b"working" => ProgramState::Working,
        b"done" => ProgramState::Done,
        b"blocked" => ProgramState::Blocked,
        b"error" => ProgramState::Error,
        b"clear" => {
            return match id {
                None => Some(Report::Clear),
                // A clear of a child leaves the root, the only record kept, as it was.
                Some(_) => None,
            };
        }
        _ => return None,
    };
    if id.is_some() {
        // A valid id names a child this form does not keep; a malformed one is ignored rather than read as the root.
        return None;
    }
    let app = app
        .filter(|app| !app.is_empty() && app.len() <= MAX_APP && app.iter().all(|&b| is_id_byte(b)))
        .map(|app| String::from_utf8_lossy(app).into_owned());
    let kind = (state == ProgramState::Blocked)
        .then_some(kind)
        .flatten()
        .filter(|kind| matches!(*kind, b"permission" | b"question" | b"auth"))
        .map(|kind| String::from_utf8_lossy(kind).into_owned());
    Some(Report::Set(ProgramStatus {
        state,
        kind,
        app,
        msg: msg.filter(|msg| !msg.is_empty()),
    }))
}

/// The bytes a program writes for one root report. Used by `herdr seat exec`; `msg` is plain text.
pub fn encode_report(state: ProgramState, app: &str, msg: &str) -> String {
    let msg: String = msg.chars().filter(|ch| !ch.is_control()).collect();
    let msg = base64::engine::general_purpose::STANDARD.encode(msg.as_bytes());
    format!("\x1b]7501;state={}:app={app}:msg={msg}\x1b\\", state.word())
}

fn trim(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |end| end + 1);
    &bytes[start..end]
}

fn is_value_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b',' | b'+' | b'/' | b'=' | b'-')
}

fn is_id_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'+' | b'-')
}

/// Standard base64 of UTF-8, padding optional, no control characters once decoded. The encoded size is checked
/// before decoding (spec, Limits).
fn decode_text(value: &[u8], max_encoded: usize, max_decoded: usize) -> Option<String> {
    if value.len() > max_encoded {
        return None;
    }
    let engine = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );
    let bytes = engine.decode(value).ok()?;
    if bytes.len() > max_decoded {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    (!text.chars().any(char::is_control)).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(payload: &str) -> ProgramStatus {
        match parse_report(payload.as_bytes()) {
            Some(Report::Set(status)) => status,
            other => panic!("{payload}: {other:?}"),
        }
    }

    #[test]
    fn program_status_parses_the_spec_examples() {
        let status = set("state=working:app=brew:msg=SW5zdGFsbGluZyB1cGRhdGVz");
        assert_eq!(status.state, ProgramState::Working);
        assert_eq!(status.app.as_deref(), Some("brew"));
        assert_eq!(status.msg.as_deref(), Some("Installing updates"));
        let status = set("state=blocked:kind=permission:app=terraform:msg=QXBwbHkgMyB0byBhZGQsIDEgdG8gY2hhbmdlLCAwIHRvIGRlc3Ryb3k/");
        assert_eq!(status.state, ProgramState::Blocked);
        assert_eq!(status.kind.as_deref(), Some("permission"));
        assert_eq!(
            status.msg.as_deref(),
            Some("Apply 3 to add, 1 to change, 0 to destroy?")
        );
        assert_eq!(
            set("state=done:msg=VXBncmFkZWQgMTIgcGFja2FnZXM")
                .msg
                .as_deref(),
            Some("Upgraded 12 packages")
        );
        assert_eq!(parse_report(b"state=clear"), Some(Report::Clear));
    }

    #[test]
    fn program_status_ignores_what_the_spec_says_to_ignore() {
        // No state, an unknown state, a child id, a malformed id, a child clear.
        for payload in [
            "app=x",
            "state=paused",
            "state=working:id=build",
            "state=working:id=a?b",
            "state=clear:id=x",
        ] {
            assert_eq!(parse_report(payload.as_bytes()), None, "{payload}");
        }
        // A malformed pair is skipped and the rest applies; unknown keys are ignored; the last value wins.
        let status = set("junk:=x:state=idle:future=1:state = working ");
        assert_eq!(status.state, ProgramState::Working);
        // kind only with blocked; an app outside its set is absent.
        assert_eq!(set("state=working:kind=auth").kind, None);
        assert_eq!(set("state=blocked:kind=other").kind, None);
        assert_eq!(set("state=idle:app=a,b").app, None);
    }

    #[test]
    fn program_status_discards_a_report_that_breaks_a_limit() {
        // A control character in msg (a newline), bad base64 twice, an oversized msg, an oversized key. A byte
        // outside the value set is not here: the spec skips that pair and applies the rest.
        let newline = base64::engine::general_purpose::STANDARD.encode("a\nb");
        let long = base64::engine::general_purpose::STANDARD.encode("x".repeat(2049));
        for payload in [
            format!("state=working:msg={newline}"),
            "state=working:msg=abcde".to_owned(),
            "state=working:msg=a".to_owned(),
            format!("state=working:msg={long}"),
            format!("state=working:{}=1", "k".repeat(17)),
        ] {
            assert_eq!(parse_report(payload.as_bytes()), None, "{payload}");
        }
    }

    #[test]
    fn program_status_encode_round_trips() {
        let bytes = encode_report(ProgramState::Working, "nirvana", "c6-candle-noise");
        let body = bytes
            .strip_prefix("\x1b]7501;")
            .and_then(|rest| rest.strip_suffix("\x1b\\"))
            .unwrap();
        let status = set(body);
        assert_eq!(status.state, ProgramState::Working);
        assert_eq!(status.app.as_deref(), Some("nirvana"));
        assert_eq!(status.msg.as_deref(), Some("c6-candle-noise"));
    }
}
