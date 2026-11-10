//! Who is let in, and what each route answers.
//!
//! The checks, all before any route: the connection must come from the Tailscale service (`peer.rs`); the
//! `Tailscale-User-Login` it set must be the login this PC is signed in to Tailscale as; the caller must be another
//! device, not this PC (`from_another_device`); the Host must be this PC's ts.net name; and a request that may change
//! something must come from this page (`same_site`). A tagged device, Funnel traffic and a user a device was shared
//! with all reach here without the owner's login, and are refused and logged, at most once a minute per caller
//! (plan 39 §1.2, §2 "a device that is not the owner's is refused"; review round S1, S4, S5).

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde_json::json;
use tracing::warn;

use super::herdr::Herdr;
use super::http::{Conn, Handler, Request, Response};
use super::pcs::Probe;
use super::peer::PeerCheck;
use super::tailscale::Tailscale;

/// The identity header `tailscale serve` sets on each request it proxies (Tailscale KB 1312).
pub const LOGIN_HEADER: &str = "Tailscale-User-Login";

/// The page, embedded in the binary: no build step, nothing to install (plan 39 §1.4). A screen is a file of its
/// own that registers itself with the shell in `app.js`; adding one is a row here and a `<script>` in index.html.
pub const ASSETS: &[(&str, &str, &str)] = &[
    (
        "/",
        "text/html; charset=utf-8",
        include_str!("page/index.html"),
    ),
    (
        "/app.css",
        "text/css; charset=utf-8",
        include_str!("page/app.css"),
    ),
    (
        "/app.js",
        "text/javascript; charset=utf-8",
        include_str!("page/app.js"),
    ),
    (
        "/pcs.js",
        "text/javascript; charset=utf-8",
        include_str!("page/pcs.js"),
    ),
    (
        "/spaces.js",
        "text/javascript; charset=utf-8",
        include_str!("page/spaces.js"),
    ),
    (
        "/chat.js",
        "text/javascript; charset=utf-8",
        include_str!("page/chat.js"),
    ),
    // P39c: screen 4, the prompt sheet and the Esc button.
    (
        "/prompt.js",
        "text/javascript; charset=utf-8",
        include_str!("page/prompt.js"),
    ),
    // P39d: the new-space and cards screens. Plan 40 review D3: these were served from pick.rs's own match and
    // listed again in shell.rs, by hand; one row here serves them and puts them in the offline shell and its tag.
    (
        "/newspace.js",
        "text/javascript; charset=utf-8",
        include_str!("page/newspace.js"),
    ),
    (
        "/cards.js",
        "text/javascript; charset=utf-8",
        include_str!("page/cards.js"),
    ),
];

/// The page loads its fonts from Google, as the prototype does, and everything else from this origin only.
pub const CSP: &str = "default-src 'self'; style-src 'self' https://fonts.googleapis.com; \
font-src https://fonts.gstatic.com; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; \
base-uri 'none'; form-action 'none'";

/// Whether the logins Tailscale named are the owner's: exactly one, and the owner's.
pub fn decide(logins: &[&str], owner: Option<&str>) -> Result<(), &'static str> {
    let Some(owner) = owner else {
        return Err("this PC's Tailscale login is not known yet");
    };
    match logins {
        [] => Err("no Tailscale user: a tagged device, Funnel, or not through Tailscale"),
        [login] if *login == owner => Ok(()),
        [_] => Err("another Tailscale user"),
        _ => Err("more than one Tailscale-User-Login"),
    }
}

/// The header `tailscale serve` sets to the caller's Tailscale address, replacing any the caller sent
/// (`addProxyForwardedHeaders` in Tailscale 1.102.2's `ipn/ipnlocal/serve.go`: `Header.Set("X-Forwarded-For",
/// SrcAddr.Addr())`).
pub const FROM_HEADER: &str = "X-Forwarded-For";

/// Whether the request came from another device, not from this PC (review round, S1). tailscaled also listens on
/// this PC's own Tailscale address for its serve ports (`localListener` in the same file), so any local process
/// that dials `https://<this pc>.<tailnet>.ts.net:8443/` reaches the mount, and Tailscale names this PC's user, the
/// owner, for it. Its source is this PC's own Tailscale address; a process cannot bind a source address that is
/// another device's. A source outside Tailscale's ranges gets no identity anyway, and is refused here too.
pub fn from_another_device(from: &[&str], own: &[IpAddr]) -> Result<(), &'static str> {
    let [from] = from else {
        return Err("not exactly one X-Forwarded-For");
    };
    let Ok(ip) = from.trim().parse::<IpAddr>() else {
        return Err("X-Forwarded-For is not an address");
    };
    let ip = match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        v4 => v4,
    };
    let tailnet = match ip {
        // 100.64.0.0/10, Tailscale's IPv4 range.
        IpAddr::V4(v4) => v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64,
        // fd7a:115c:a1e0::/48, its IPv6 range.
        IpAddr::V6(v6) => v6.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    };
    if !tailnet {
        return Err("not a Tailscale address");
    }
    if own.contains(&ip) {
        return Err("this PC's own Tailscale address: a local process, not another device");
    }
    Ok(())
}

/// Whether the request was sent to this PC's ts.net name (review round, S5). tailscaled passes the caller's Host
/// through (`r.Out.Host = r.In.Host`), and a browser sets it from the URL, so a page on any other name (a DNS
/// rebinding to the tailnet address) fails here.
pub fn for_this_pc(hosts: &[&str], host: Option<&str>) -> Result<(), &'static str> {
    let Some(host) = host else {
        return Err("this PC's name is not known yet");
    };
    match hosts {
        [sent] if sent.eq_ignore_ascii_case(host) => Ok(()),
        _ => Err("Host is not this PC's ts.net name"),
    }
}

/// Whether a request that may change something came from this page (review round, S5). Every route is GET today;
/// P39b's POST routes inherit this. A cross-site form or fetch from any page open on the owner's phone carries the
/// owner's Tailscale identity, so the browser's own word on where it came from is required: `Sec-Fetch-Site`
/// same-origin (or none, a typed request), and an `Origin`, when sent, of exactly this PC's page. A request with
/// neither is refused: every browser that can run the page sends both.
///
/// A read of `/api/` is held to the same headers when they are sent (P39b review round, S7): another site's page on
/// the owner's phone could otherwise open `/api/chat/events` with an `<img>` or `EventSource`, carrying the owner's
/// identity, and each such view would take one of the six live slots from the owner's own (it could not read them:
/// no CORS). The page's own `fetch` and `EventSource` are same-origin, and iPhone Safari says so: WebKit sends
/// `Sec-Fetch-Site` since Safari 16.4 (iOS 16.4, March 2023), as Chrome and Firefox did before it. A read with
/// neither header is let in, unlike a write: a same-origin GET carries no `Origin` (the Fetch standard adds one to a
/// GET only when it is cross-origin in CORS mode), so a browser older than 16.4 sends nothing at all on the page's
/// own reads, and refusing that would lock such a phone out of a page it was let into; and another PC's Nirvana
/// Remote asking `/api/hello` (`pcs::Https`, not a browser) sends neither. A browser that sends neither cannot say
/// a request is cross-site, so this check cannot tell; a phone on iOS 16.4 or later always says. The page's own
/// files are not held to it: a link or a typed address opens `/`, and that navigation is `cross-site` or `none`.
pub fn same_site(request: &Request, host: &str) -> Result<(), &'static str> {
    let reads = request.method == "GET" || request.method == "HEAD";
    if reads && !request.path.starts_with("/api/") {
        return Ok(());
    }
    let site = request.header("Sec-Fetch-Site");
    let origin = request.header("Origin");
    if site.is_empty() && origin.is_empty() {
        if reads {
            return Ok(());
        }
        return Err("a request that may change something without Sec-Fetch-Site or Origin");
    }
    match site.as_slice() {
        [] => {}
        [value]
            if value.eq_ignore_ascii_case("same-origin") || value.eq_ignore_ascii_case("none") => {}
        _ => return Err("Sec-Fetch-Site is not same-origin"),
    }
    let page = format!("https://{host}");
    match origin.as_slice() {
        [] => Ok(()),
        [value] if value.eq_ignore_ascii_case(&page) => Ok(()),
        _ => Err("Origin is not this PC's page"),
    }
}

/// One log line per key per `every` (review round, S4): a connect loop to the port would otherwise write a line per
/// connection into herdr-server.log. The line that is let through says how many were held back.
pub struct Throttle {
    every: Duration,
    seen: Mutex<HashMap<String, (Instant, u64)>>,
}

/// Keys kept before the old ones are dropped, so a caller cycling keys cannot grow the map without end.
const THROTTLE_KEYS: usize = 1024;

impl Throttle {
    pub fn new(every: Duration) -> Self {
        Self {
            every,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// `Some(held back since the last line)` when `key` may log at `now`.
    pub fn allow(&self, key: &str, now: Instant) -> Option<u64> {
        let Ok(mut seen) = self.seen.lock() else {
            return None;
        };
        if let Some((last, held)) = seen.get_mut(key) {
            if now.saturating_duration_since(*last) < self.every {
                *held += 1;
                return None;
            }
            let count = *held;
            *last = now;
            *held = 0;
            return Some(count);
        }
        if seen.len() >= THROTTLE_KEYS {
            let every = self.every;
            seen.retain(|_, (last, _)| now.saturating_duration_since(*last) < every);
            if seen.len() >= THROTTLE_KEYS {
                return None;
            }
        }
        seen.insert(key.to_string(), (now, 0));
        Some(0)
    }
}

/// How long `/api/pcs` reuses one `tailscale status` (review nit N2): a phone refreshing the list does not start a
/// process per hit.
pub const STATUS_FOR: Duration = Duration::from_secs(5);
/// A refusal is logged at most once per caller in this long.
pub const LOG_EVERY: Duration = Duration::from_secs(60);

/// This PC as the checks need it, set by the supervisor from `status --json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Me {
    /// `<pc>.<tailnet>.ts.net:8443`, the only Host let in.
    pub host: String,
    /// Its own Tailscale addresses: a request from one of them is a local process.
    pub ips: Vec<IpAddr>,
}

type Cached = Option<(Instant, Result<String, String>)>;

pub struct Site {
    pub tailscale: Arc<dyn Tailscale>,
    pub herdr: Arc<dyn Herdr>,
    pub peer: Arc<dyn PeerCheck>,
    pub probe: Arc<dyn Probe>,
    /// Set by the supervisor from `status --json`; `None` refuses everyone.
    pub owner: RwLock<Option<String>>,
    /// Set and cleared with `owner`; `None` refuses everyone.
    pub me: RwLock<Option<Me>>,
    pub https_port: u16,
    pub now: fn() -> time::OffsetDateTime,
    pub throttle: Throttle,
    status: Mutex<Cached>,
    /// The one `events.subscribe` every live page shares (P39b).
    pub feed: Arc<super::spaces::Feed>,
    /// The live views open now, ended when Nirvana Remote stops.
    pub streams: Arc<super::stream::Streams>,
    /// Claude Code's config folder: transcripts are read from under it only. `None` reads none (a test's default).
    pub claude_dir: Option<PathBuf>,
    /// The fork's install root, for the folder session records "Open Nirvana here" resumes from.
    pub nirvana_home: Option<PathBuf>,
    /// P39c: what each pane was last pressed with, so one prompt takes one press (prompt.rs, review S2).
    pub spent: super::prompt::Spent,
    /// P39d: the picker's root, the preset and the ports (pick.rs). The default reads and starts nothing.
    pub local: super::pick::Local,
    /// P39e: the phones with alerts on and this PC's push key (alerts.rs). `None` answers that alerts are not set up.
    pub alerts: Option<Arc<super::alerts::Alerts>>,
    /// P39u: the uploads being read now, at most `live::UPLOADS_AT_ONCE`.
    pub uploads: super::live::Uploads,
}

impl Site {
    pub fn new(
        tailscale: Arc<dyn Tailscale>,
        herdr: Arc<dyn Herdr>,
        peer: Arc<dyn PeerCheck>,
        probe: Arc<dyn Probe>,
        https_port: u16,
        now: fn() -> time::OffsetDateTime,
    ) -> Self {
        Self {
            tailscale,
            peer,
            probe,
            owner: RwLock::new(None),
            me: RwLock::new(None),
            https_port,
            now,
            throttle: Throttle::new(LOG_EVERY),
            status: Mutex::new(None),
            feed: super::spaces::Feed::new(herdr.clone()),
            streams: super::stream::Streams::new(),
            claude_dir: None,
            nirvana_home: None,
            spent: super::prompt::Spent::default(),
            local: super::pick::Local::default(),
            alerts: None,
            uploads: super::live::Uploads::default(),
            herdr,
        }
    }

    /// `tailscale status --json`, read again only when the last read is older than `STATUS_FOR`.
    fn status_text(&self) -> Result<String, String> {
        if let Ok(cached) = self.status.lock() {
            if let Some((at, text)) = cached.as_ref() {
                if at.elapsed() < STATUS_FOR {
                    return text.clone();
                }
            }
        }
        let text = self.tailscale.status();
        if let Ok(mut cached) = self.status.lock() {
            *cached = Some((Instant::now(), text.clone()));
        }
        text
    }
}

const REFUSED: &str = "Only the Tailscale account this PC is signed in with can connect.";

impl Handler for Site {
    fn admit(&self, peer: SocketAddr, local: SocketAddr) -> Result<(), String> {
        self.peer.check(peer, local).map_err(|refusal| {
            let key = refusal
                .pid
                .map_or_else(|| "no row".to_string(), |pid| format!("pid {pid}"));
            if let Some(held_back) = self.throttle.allow(&key, Instant::now()) {
                warn!(%peer, why = %refusal.why, held_back, "nirvana remote refused a connection");
            }
            refusal.why
        })
    }

    fn refused(&self, _peer: SocketAddr, _why: &str) -> Response {
        Response::text(403, REFUSED).with("Content-Security-Policy", CSP)
    }

    fn handle(&self, peer: SocketAddr, request: Request, conn: &mut Conn) -> Response {
        // Every answer, a refusal included, carries the CSP (review nit N7).
        self.route(peer, &request, conn)
            .with("Content-Security-Policy", CSP)
    }
}

impl Site {
    fn route(&self, peer: SocketAddr, request: &Request, conn: &mut Conn) -> Response {
        let logins = request.header(LOGIN_HEADER);
        let owner = self.owner.read().ok().and_then(|owner| owner.clone());
        let me = self.me.read().ok().and_then(|me| me.clone());
        let ips = me.as_ref().map_or(&[][..], |me| me.ips.as_slice());
        let host = me.as_ref().map(|me| me.host.as_str());
        let checked = decide(&logins, owner.as_deref())
            .and_then(|()| from_another_device(&request.header(FROM_HEADER), ips))
            .map_err(|why| (403, why))
            .and_then(|()| for_this_pc(&request.header("Host"), host).map_err(|why| (421, why)))
            .and_then(|()| same_site(request, host.unwrap_or("")).map_err(|why| (403, why)));
        if let Err((status, why)) = checked {
            let login = logins.first().copied().unwrap_or("");
            if let Some(held_back) = self
                .throttle
                .allow(&format!("{login} {why}"), Instant::now())
            {
                warn!(
                    %peer,
                    login,
                    path = %request.path,
                    why,
                    held_back,
                    "nirvana remote refused a request"
                );
            }
            return Response::text(status, REFUSED);
        }
        // P39d's routes, after every check above: the picker, the cards, and their two scripts (pick.rs).
        if let Some(answer) = self.new_space_and_cards(request, conn) {
            return answer;
        }
        // P39e's routes, after every check above too: the alerts toggle, and the home-screen app's files (alerts.rs).
        if let Some(answer) = self.alerts_routes(request, conn) {
            return answer;
        }
        // P39b's routes that change something: every check above has passed, and only now is a body read.
        if request.method == "POST" {
            match request.path.as_str() {
                "/api/send" => return self.send(request, conn),
                "/api/upload" => return self.upload(request, conn),
                // P39c: one key to the prompt the phone saw, or Esc (prompt.rs).
                "/api/key" => return self.key(request, conn),
                _ => {}
            }
        }
        if request.method != "GET" && request.method != "HEAD" {
            return Response::text(405, "GET only").with("Allow", "GET, HEAD");
        }
        if let Some((path, kind, body)) = ASSETS.iter().find(|(path, _, _)| *path == request.path) {
            // P39o: the page carries this build's tag, which the worker's cache is named for (shell.rs).
            return Response::new(200, kind, super::shell::served(path, body));
        }
        match request.path.as_str() {
            // P39o: the page's reach check; the build tag says whether its shell is this build's (shell.rs).
            "/api/hello" => self
                .hello()
                .with(super::shell::BUILD_HEADER, super::shell::build()),
            "/api/pcs" => self.pcs(),
            "/api/spaces" => self.spaces(),
            "/api/events" => self.events(request, conn),
            "/api/chat" => self.chat(request),
            "/api/chat/events" => self.chat_events(request, conn),
            // P39c: a blocked agent's prompt as the phone is shown it (prompt.rs).
            "/api/prompt" => self.prompt(request),
            _ => Response::text(404, "not found"),
        }
    }

    fn hello(&self) -> Response {
        let summary = super::herdr::summary(self.herdr.as_ref()).unwrap_or_default();
        Response::json(
            200,
            &json!(super::pcs::Hello {
                app: super::pcs::APP.into(),
                version: crate::nirvana::VERSION.into(),
                spaces: summary.spaces,
                working: summary.working,
            }),
        )
    }

    fn pcs(&self) -> Response {
        let status = match self
            .status_text()
            .map_err(super::tailscale::Off::Unreadable)
            .and_then(|text| super::tailscale::parse_status(&text))
        {
            Ok(status) => status,
            Err(off) => return Response::json(502, &json!({ "error": off.long() })),
        };
        let me = match super::herdr::summary(self.herdr.as_ref()) {
            Ok(me) => me,
            Err(error) => return Response::json(502, &json!({ "error": error })),
        };
        let pcs = super::pcs::list(
            &status,
            me,
            self.https_port,
            self.probe.as_ref(),
            (self.now)(),
        );
        Response::json(
            200,
            &json!({ "version": crate::nirvana::VERSION, "pcs": pcs }),
        )
    }
}
