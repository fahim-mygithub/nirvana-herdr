//! Plan 48's decisions as pure functions over stated facts, and the worker's loop over a scripted world. Nothing here
//! starts a server, a window or a process, or stops a real one: the world's sockets, server start and clock are
//! stand-ins, and a made-up run id names the result file's lines.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::time::{Duration, Instant};

use super::*;

#[test]
fn the_shutdown_says_restart_only_when_the_stop_was_one() {
    assert_eq!(shutdown_message(true), SHUTDOWN_REASON);
    assert_eq!(
        shutdown_message(false),
        "server is shutting down",
        "upstream's own text otherwise"
    );
}

#[test]
fn only_a_restarts_reason_brings_the_client_back() {
    assert!(is_restart(Some(SHUTDOWN_REASON)));
    assert!(
        !is_restart(Some(PLAIN_SHUTDOWN)),
        "a plain stop ends the window"
    );
    assert!(!is_restart(Some("detached")));
    assert!(!is_restart(None));
}

/// The client never starts the new server while the old one holds its sockets: it comes back (exit 75) only once
/// they are gone.
#[test]
fn the_client_comes_back_only_once_the_old_sockets_are_gone() {
    assert_eq!(client_exit(true), crate::nirvana::RELOAD_EXIT_CODE);
    assert_eq!(
        client_exit(false),
        1,
        "sockets still held: the window leaves"
    );
    let looks = Cell::new(0);
    let slept = Cell::new(Duration::ZERO);
    let gone = wait_until(
        STOP_WAIT,
        POLL,
        || {
            looks.set(looks.get() + 1);
            looks.get() >= 3
        },
        |step| slept.set(slept.get() + step),
    );
    assert!(gone);
    assert_eq!(slept.get(), POLL * 2, "it looked again each step");
    let slept = Cell::new(Duration::ZERO);
    assert!(
        !wait_until(
            STOP_WAIT,
            POLL,
            || false,
            |step| slept.set(slept.get() + step)
        ),
        "a server that keeps its sockets is waited on for STOP_WAIT, then given up"
    );
    assert_eq!(slept.get(), STOP_WAIT);
}

/// Item 3: with no window the server comes back with no window. The window mark is the Nirvana window's, so it counts
/// for the session that window runs only.
#[test]
fn no_window_brings_the_server_back_alone() {
    assert_eq!(comeback(true, Some("nirvana")), Comeback::Window);
    assert_eq!(
        comeback(false, Some("nirvana")),
        Comeback::ServerAlone,
        "the window was closed"
    );
    assert_eq!(
        comeback(true, Some("work")),
        Comeback::ServerAlone,
        "another session's restart: the Nirvana window is not its"
    );
    assert_eq!(comeback(true, None), Comeback::ServerAlone);
}

fn facts() -> Facts {
    Facts::default()
}

#[test]
fn the_worker_waits_for_the_old_server_then_gives_up() {
    let waiting = Facts {
        since_stop: Duration::from_secs(1),
        ..facts()
    };
    assert_eq!(next_step(&waiting), Step::Wait);
    let stuck = Facts {
        since_stop: STOP_WAIT,
        ..facts()
    };
    assert_eq!(next_step(&stuck), Step::Fail(NOT_STOPPED));
    // A socket that answers before the old one went is the old server's, never the new one.
    let old_answers = Facts {
        listening: true,
        since_stop: Duration::from_secs(1),
        ..facts()
    };
    assert_eq!(next_step(&old_answers), Step::Wait);
}

#[test]
fn with_a_window_the_worker_waits_for_its_client_and_starts_the_server_only_if_none_came() {
    let gone = Facts {
        window: true,
        old_gone: true,
        since_gone: Duration::from_secs(5),
        ..facts()
    };
    assert_eq!(
        next_step(&gone),
        Step::Wait,
        "the window's client starts it"
    );
    let late = Facts {
        since_gone: WINDOW_WAIT,
        ..gone
    };
    assert_eq!(next_step(&late), Step::StartServer);
    let back = Facts {
        listening: true,
        ..gone
    };
    assert_eq!(next_step(&back), Step::Done);
}

#[test]
fn with_no_window_the_worker_starts_the_server_at_once_and_waits_for_it_to_listen() {
    let gone = Facts {
        old_gone: true,
        ..facts()
    };
    assert_eq!(next_step(&gone), Step::StartServer);
    let starting = Facts {
        since_start: Some(Duration::from_secs(10)),
        ..gone
    };
    assert_eq!(next_step(&starting), Step::Wait, "started once, not again");
    let never = Facts {
        since_start: Some(READY_WAIT),
        ..gone
    };
    assert_eq!(next_step(&never), Step::Fail(NOT_LISTENING));
    let up = Facts {
        listening: true,
        since_start: Some(Duration::from_secs(3)),
        ..gone
    };
    assert_eq!(next_step(&up), Step::Done);
}

/// A scripted world: the old server goes after `gone_after` looks, a server listens `listen_after` looks after it went
/// (or after the worker started one, with `only_if_started`), and each sleep moves the clock.
struct Script {
    base: Instant,
    clock: Cell<Duration>,
    looks: Cell<usize>,
    gone_after: Option<usize>,
    gone_at: Cell<Option<usize>>,
    listen_after: Option<usize>,
    only_if_started: bool,
    started: Cell<usize>,
    refuse: Option<&'static str>,
    reports: RefCell<Vec<String>>,
}

impl Script {
    fn new(gone_after: Option<usize>, listen_after: Option<usize>) -> Self {
        Self {
            base: Instant::now(),
            clock: Cell::new(Duration::ZERO),
            looks: Cell::new(0),
            gone_after,
            gone_at: Cell::new(None),
            listen_after,
            only_if_started: false,
            started: Cell::new(0),
            refuse: None,
            reports: RefCell::default(),
        }
    }

    fn states(&self) -> Vec<String> {
        self.reports.borrow().clone()
    }
}

impl World for Script {
    fn ask_restart(&self) -> Result<(), String> {
        self.refuse.map_or(Ok(()), |why| Err(why.to_owned()))
    }

    fn old_gone(&self) -> bool {
        self.looks.set(self.looks.get() + 1);
        let gone = self
            .gone_after
            .is_some_and(|after| self.looks.get() >= after);
        if gone && self.gone_at.get().is_none() {
            self.gone_at.set(Some(self.looks.get()));
        }
        gone
    }

    fn listening(&self) -> bool {
        if self.only_if_started && self.started.get() == 0 {
            return false;
        }
        let (Some(after), Some(at)) = (self.listen_after, self.gone_at.get()) else {
            return false;
        };
        self.looks.set(self.looks.get() + 1);
        self.looks.get() >= at + after
    }

    fn start_server(&self) -> Result<(), String> {
        self.started.set(self.started.get() + 1);
        Ok(())
    }

    fn now(&self) -> Instant {
        self.base + self.clock.get()
    }

    fn sleep(&self, step: Duration) {
        self.clock.set(self.clock.get() + step);
    }

    fn report(&self, state: &str, _why: &str) {
        self.reports.borrow_mut().push(state.to_owned());
    }
}

/// The command or the phone with a window attached: the window's client brings the server back, the worker only
/// waits.
#[test]
fn a_restart_with_a_window_waits_for_its_client_and_starts_nothing() {
    let world = Script::new(Some(3), Some(5));
    assert!(run(&world, Comeback::Window).is_ok());
    assert_eq!(world.started.get(), 0, "the window's client started it");
    assert_eq!(world.states(), ["stopping", "stopped", "done"]);
}

#[test]
fn a_restart_with_no_window_starts_the_server_once_the_old_one_is_gone() {
    let mut world = Script::new(Some(2), Some(3));
    world.only_if_started = true;
    assert!(run(&world, Comeback::ServerAlone).is_ok());
    assert_eq!(world.started.get(), 1);
    assert_eq!(world.states(), ["stopping", "stopped", "starting", "done"]);
}

#[test]
fn a_window_whose_client_never_comes_gets_the_server_started_for_it() {
    let mut world = Script::new(Some(1), Some(2));
    world.only_if_started = true;
    let took = run(&world, Comeback::Window).expect("it ends well");
    assert_eq!(world.started.get(), 1);
    assert!(took >= WINDOW_WAIT, "after the window's wait: {took:?}");
}

#[test]
fn a_restart_says_what_did_not_come_back() {
    let world = Script::new(None, None);
    assert_eq!(
        run(&world, Comeback::ServerAlone),
        Err(NOT_STOPPED.to_owned())
    );
    assert_eq!(world.started.get(), 0, "nothing started over a live server");
    assert_eq!(world.states().last().map(String::as_str), Some("failed"));

    let mut world = Script::new(Some(1), None);
    world.only_if_started = true;
    assert_eq!(
        run(&world, Comeback::ServerAlone),
        Err(NOT_LISTENING.to_owned())
    );

    let mut world = Script::new(Some(1), Some(1));
    world.refuse = Some("command_not_found");
    let refused = run(&world, Comeback::Window).unwrap_err();
    assert!(refused.contains("command_not_found"), "{refused}");
    assert_eq!(world.started.get(), 0);
}

#[test]
fn the_command_knows_when_it_runs_in_a_pane_of_the_session_it_restarts() {
    let session = Path::new("/made-up/sessions/nirvana/herdr.sock");
    assert!(in_own_pane(Some(session), session));
    assert!(!in_own_pane(
        Some(Path::new("/made-up/sessions/work/herdr.sock")),
        session
    ));
    assert!(!in_own_pane(None, session), "a terminal outside herdr");
}

/// Item 4: the command says what a restart interrupts, and what comes back.
#[test]
fn the_notice_counts_the_working_agents_and_says_what_comes_back() {
    let text = |working, comeback, own| notice("nirvana", working, comeback, own).join("\n");
    let three = text(Some(3), Comeback::Window, false);
    assert!(
        three.contains("3 agents are working now; their turns stop"),
        "{three}"
    );
    assert!(three.contains("the Nirvana window stays"), "{three}");
    assert!(
        text(Some(1), Comeback::Window, false).contains("1 agent is working now; its turn stops")
    );
    assert!(text(Some(0), Comeback::Window, false).contains("no agent is working now"));
    let alone = text(Some(0), Comeback::ServerAlone, false);
    assert!(
        alone.contains("the server comes back with no window"),
        "{alone}"
    );
    let pane = text(Some(2), Comeback::Window, true);
    assert!(
        pane.contains("it stops with it; the restart goes on by itself"),
        "{pane}"
    );
    assert!(!three.contains("stops with it"));
}

#[test]
fn the_command_reads_only_its_own_runs_outcome() {
    let line = r#"{"run":"41-7","state":"failed","why":"made-up reason","at":1}"#;
    assert_eq!(
        read_result(line, "41-7"),
        Some(("failed".to_owned(), "made-up reason".to_owned()))
    );
    assert_eq!(read_result(line, "41-8"), None, "another run's line");
    assert_eq!(read_result("{torn", "41-7"), None);
}

#[test]
fn the_worker_is_started_with_the_session_named() {
    assert_eq!(
        worker_args("nirvana", "41-7", Comeback::Window),
        [
            "--session",
            "nirvana",
            "nirvana",
            "restart",
            "--worker",
            "41-7",
            "window"
        ]
    );
    assert_eq!(
        worker_args("default", "41-7", Comeback::ServerAlone)
            .last()
            .map(String::as_str),
        Some("alone")
    );
}

/// Item 4: the hop that starts the worker out of the caller's process tree runs the same restart with `--hop`, so the
/// worker it starts is the session's and the run's.
#[test]
fn the_hop_starts_the_same_worker() {
    let hop = hop_args("nirvana", "41-7", Comeback::ServerAlone);
    assert_eq!(
        hop,
        [
            "--session",
            "nirvana",
            "nirvana",
            "restart",
            "--hop",
            "41-7",
            "alone"
        ]
    );
}

/// The worker's `command.invoke` stops the server as a restart, as `server.stop` does in the app.
#[tokio::test]
async fn the_restart_command_stops_the_server_as_a_restart() {
    let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = crate::app::App::new(
        &crate::config::Config::default(),
        crate::app::AppPolicy::TEST,
        None,
        api_rx,
        crate::api::EventHub::default(),
    );
    assert!(!app.nirvana_restarting);
    let answer = app.handle_command_invoke(
        "t1".into(),
        crate::api::schema::CommandInvokeParams {
            command_id: RESTART_COMMAND.into(),
            workspace_id: None,
            tab_id: None,
            pane_id: None,
            selection: None,
        },
    );
    assert!(answer.contains("\"result\""), "{answer}");
    assert!(app.state.should_quit, "the server stops");
    assert!(app.nirvana_restarting, "as a restart");
}
