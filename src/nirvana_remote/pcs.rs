//! The PC list, the page's first screen (plan 39 §1.3, the prototype's "Your PCs").
//!
//! Every machine signed in as the owner's Tailscale user, this PC first: online and answering as Nirvana, online
//! without Nirvana, or asleep with when it was last seen. Phones and tablets are on the tailnet too but are not
//! PCs, so they are left out.

use std::time::Duration;

use serde::Serialize;

use super::herdr::Summary;
use super::tailscale::{Node, Status};

/// A peer that does not answer `/api/hello` in this long reads as "Nirvana not running".
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
/// Tailscale's names for systems that are not PCs.
const NOT_PCS: &[&str] = &["iOS", "android", "tvOS"];

/// What `/api/hello` answers: how one Nirvana Remote knows another, and the counts for its line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Hello {
    pub app: String,
    pub version: String,
    pub spaces: usize,
    pub working: usize,
}

/// The `app` a Nirvana Remote names itself with.
pub const APP: &str = "nirvana-remote";

pub trait Probe: Send + Sync {
    /// Ask the Nirvana Remote at `https://<dns>:<port>/api/hello`; `None` when nothing answers as one.
    fn hello(&self, dns: &str, port: u16) -> Option<Hello>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Nirvana,
    Online,
    Asleep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pc {
    pub name: String,
    #[serde(rename = "self")]
    pub is_self: bool,
    pub state: State,
    pub line: String,
    /// Where a tap goes: this page for this PC, the peer's own page for another Nirvana.
    pub url: Option<String>,
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// "3 spaces, 2 agents at work", as the prototype words it.
pub fn nirvana_line(spaces: usize, working: usize) -> String {
    let at_work = if working == 0 {
        "no agents at work".to_string()
    } else {
        format!("{} at work", plural(working, "agent", "agents"))
    };
    format!("{}, {at_work}", plural(spaces, "space", "spaces"))
}

/// "Asleep, last seen 2 h ago".
pub fn asleep_line(last_seen: Option<time::OffsetDateTime>, now: time::OffsetDateTime) -> String {
    let Some(seen) = last_seen else {
        return "Asleep".into();
    };
    let minutes = (now - seen).whole_minutes().max(0);
    let ago = match minutes {
        0 => "just now".to_string(),
        1..=59 => format!("{minutes} min ago"),
        60..=2879 => format!("{} h ago", minutes / 60),
        _ => format!("{} d ago", minutes / 1440),
    };
    format!("Asleep, last seen {ago}")
}

fn is_pc(node: &Node) -> bool {
    !NOT_PCS.iter().any(|os| node.os.eq_ignore_ascii_case(os))
}

/// The list: this PC, then the owner's other PCs, those answering as Nirvana first, then online, then asleep.
/// A tagged machine belongs to the tailnet, not to a user, and Nirvana Remote lets in users only.
pub fn list(
    status: &Status,
    me: Summary,
    https_port: u16,
    probe: &dyn Probe,
    now: time::OffsetDateTime,
) -> Vec<Pc> {
    let mine = status
        .peers
        .iter()
        .filter(|peer| peer.user_id == status.me.user_id && !peer.tagged && is_pc(peer))
        .collect::<Vec<_>>();
    // The probes run at once, each bounded by PROBE_TIMEOUT, so the list waits for the slowest, not the sum.
    let hellos = std::thread::scope(|scope| {
        let asks = mine
            .iter()
            .map(|peer| {
                scope.spawn(move || {
                    (peer.online && !peer.dns.is_empty())
                        .then(|| probe.hello(&peer.dns, https_port))
                        .flatten()
                        .filter(|hello| hello.app == APP)
                })
            })
            .collect::<Vec<_>>();
        asks.into_iter()
            .map(|ask| ask.join().ok().flatten())
            .collect::<Vec<_>>()
    });
    let mut others = mine
        .iter()
        .zip(hellos)
        .map(|(peer, hello)| match hello {
            Some(hello) => Pc {
                name: peer.host.clone(),
                is_self: false,
                state: State::Nirvana,
                line: nirvana_line(hello.spaces, hello.working),
                url: Some(format!("https://{}:{https_port}/", peer.dns)),
            },
            None if peer.online => Pc {
                name: peer.host.clone(),
                is_self: false,
                state: State::Online,
                line: "Online, Nirvana not running".into(),
                url: None,
            },
            None => Pc {
                name: peer.host.clone(),
                is_self: false,
                state: State::Asleep,
                line: asleep_line(peer.last_seen, now),
                url: None,
            },
        })
        .collect::<Vec<_>>();
    let rank = |state: &State| match state {
        State::Nirvana => 0,
        State::Online => 1,
        State::Asleep => 2,
    };
    others.sort_by_key(|pc| rank(&pc.state));
    let mut pcs = vec![Pc {
        name: status.me.host.clone(),
        is_self: true,
        state: State::Nirvana,
        line: nirvana_line(me.spaces, me.working),
        url: Some("/".into()),
    }];
    pcs.extend(others);
    pcs
}

/// The real probe: one HTTPS GET through this PC's Tailscale. The peer's Tailscale names this PC's user in the
/// identity header, and that user is the owner, so a Nirvana Remote there lets the probe in.
pub struct Https;

impl Probe for Https {
    fn hello(&self, dns: &str, port: u16) -> Option<Hello> {
        let agent = ureq::AgentBuilder::new()
            .timeout(PROBE_TIMEOUT)
            .redirects(0)
            .build();
        let response = agent
            .get(&format!("https://{dns}:{port}/api/hello"))
            .call()
            .ok()?;
        // ureq is built without its `json` feature (Cargo.toml), so the body is read as text.
        serde_json::from_str(&response.into_string().ok()?).ok()
    }
}
