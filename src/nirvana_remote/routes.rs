//! Who is let in, and what each route answers.
//!
//! Two checks, both before any route: the connection must come from the Tailscale service (`peer.rs`), and the
//! `Tailscale-User-Login` it set must be the login this PC is signed in to Tailscale as. A tagged device, Funnel
//! traffic and a user a device was shared with all reach here without the owner's login, and are refused and
//! logged (plan 39 §1.2, §2 "a device that is not the owner's is refused").

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use serde_json::json;
use tracing::warn;

use super::herdr::Herdr;
use super::http::{Handler, Request, Response};
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

pub struct Site {
    pub tailscale: Arc<dyn Tailscale>,
    pub herdr: Arc<dyn Herdr>,
    pub peer: Arc<dyn PeerCheck>,
    pub probe: Arc<dyn Probe>,
    /// Set by the supervisor from `status --json`; `None` refuses everyone.
    pub owner: RwLock<Option<String>>,
    pub https_port: u16,
    pub now: fn() -> time::OffsetDateTime,
}

const REFUSED: &str = "Only the Tailscale account this PC is signed in with can connect.";

impl Handler for Site {
    fn admit(&self, peer: SocketAddr, local: SocketAddr) -> Result<(), String> {
        self.peer.check(peer, local)
    }

    fn refused(&self, peer: SocketAddr, why: &str) -> Response {
        warn!(%peer, why, "nirvana remote refused a connection");
        Response::text(403, REFUSED)
    }

    fn handle(&self, peer: SocketAddr, request: Request) -> Response {
        let logins = request.header(LOGIN_HEADER);
        let owner = self.owner.read().ok().and_then(|owner| owner.clone());
        if let Err(why) = decide(&logins, owner.as_deref()) {
            warn!(
                %peer,
                login = logins.first().copied().unwrap_or(""),
                path = %request.path,
                why,
                "nirvana remote refused a request"
            );
            return Response::text(403, REFUSED);
        }
        if request.method != "GET" && request.method != "HEAD" {
            return Response::text(405, "GET only").with("Allow", "GET, HEAD");
        }
        if let Some((_, kind, body)) = ASSETS.iter().find(|(path, _, _)| *path == request.path) {
            let response = Response::new(200, kind, *body);
            return if request.path == "/" {
                response.with("Content-Security-Policy", CSP)
            } else {
                response
            };
        }
        match request.path.as_str() {
            "/api/hello" => self.hello(),
            "/api/pcs" => self.pcs(),
            _ => Response::text(404, "not found"),
        }
    }
}

impl Site {
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
            .tailscale
            .status()
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
