//! Which local process opened a connection to Nirvana Remote, and whether it is the Tailscale service.
//!
//! The login route is `tailscale serve` (record p39a, "The login route"): tailscaled terminates HTTPS, strips any
//! `Tailscale-User-*` header the caller sent and sets its own, and connects to 127.0.0.1. The header is only as good
//! as the process that sent it, and any process of any Windows account can connect to 127.0.0.1, while herdr's own
//! pipe admits SYSTEM and its owner only (`D:P(A;;GA;;;SY)(A;;GA;;;OW)`, src/platform/windows.rs). So the header is
//! read only from a connection the Tailscale service's own process tree opened; everything else is refused before
//! its headers are read. A non-admin cannot start a process under the service, so a process that dials 127.0.0.1
//! directly is refused whatever its account.
//!
//! That is not the whole claim (review round, S1): tailscaled also serves the mount on this PC's own Tailscale
//! address, so a local process can reach it through tailscaled and arrive here from the service with the owner's
//! login. `routes::from_another_device` refuses a request whose Tailscale source is this PC. Until the owner's live
//! check shows that refusal on a real request, the claim stands at "as tight as any process of the owner's Windows
//! account" (record p39a, review round).

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::nirvana_servers::Process;

/// Why a connection was refused, and the pid that opened it when one was found (the log is limited per pid).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub pid: Option<u32>,
    pub why: String,
}

pub trait PeerCheck: Send + Sync {
    /// `Ok` when the connection from `peer` to `local` was opened by the Tailscale service; otherwise who did.
    fn check(&self, peer: SocketAddr, local: SocketAddr) -> Result<(), Refusal>;
}

/// How long one process snapshot answers a refusal (review round, S4): a connect loop costs one snapshot a second,
/// not one per connection.
pub const SNAPSHOT_FOR: Duration = Duration::from_secs(1);

/// The last process snapshot. It may only refuse: a caller it reads as the Tailscale service is read again from a
/// fresh snapshot before it is trusted, so a pid reused within the second is never let in on old data.
pub struct Snapshot {
    every: Duration,
    last: Mutex<Option<(Instant, Arc<Vec<Process>>)>>,
}

impl Snapshot {
    pub fn new(every: Duration) -> Self {
        Self {
            every,
            last: Mutex::new(None),
        }
    }

    fn fresh(&self, read: &dyn Fn() -> Vec<Process>, now: Instant) -> Arc<Vec<Process>> {
        let processes = Arc::new(read());
        if let Ok(mut last) = self.last.lock() {
            *last = Some((now, processes.clone()));
        }
        processes
    }

    /// Whether `owner` is the Tailscale service, and the processes the answer was read from.
    pub fn trusted(
        &self,
        owner: u32,
        service: Option<u32>,
        read: &dyn Fn() -> Vec<Process>,
        now: Instant,
    ) -> (bool, Arc<Vec<Process>>) {
        let cached = self.last.lock().ok().and_then(|last| {
            last.as_ref()
                .filter(|(at, _)| now.saturating_duration_since(*at) < self.every)
                .map(|(_, processes)| processes.clone())
        });
        let Some(cached) = cached else {
            let processes = self.fresh(read, now);
            return (is_tailscale(owner, service, &processes), processes);
        };
        if !is_tailscale(owner, service, &cached) {
            return (false, cached);
        }
        let processes = self.fresh(read, now);
        (is_tailscale(owner, service, &processes), processes)
    }
}

/// The Windows service name of Tailscale (`sc queryex Tailscale`, measured 2026-11-04: RUNNING, PID 6716).
pub const SERVICE: &str = "Tailscale";
/// The daemon's image name. The service runs it, and it runs a second `tailscaled.exe` as its child (measured
/// 2026-11-04: 8672, parent 6716); which of the two dials the serve backend is not measured, so both pass.
pub const DAEMON: &str = "tailscaled.exe";

/// The decision, kept apart from the system reads so a test can drive it.
pub fn is_tailscale(owner_pid: u32, service_pid: Option<u32>, processes: &[Process]) -> bool {
    let Some(service_pid) = service_pid.filter(|pid| *pid != 0) else {
        return false;
    };
    if owner_pid == service_pid {
        return true;
    }
    processes.iter().any(|process| {
        process.pid == owner_pid
            && process.parent_pid == service_pid
            && process.name.eq_ignore_ascii_case(DAEMON)
    })
}

/// The real check: the connection table, the service's pid, the process table.
pub struct System {
    snapshot: Snapshot,
}

impl System {
    pub fn new() -> Self {
        Self {
            snapshot: Snapshot::new(SNAPSHOT_FOR),
        }
    }
}

impl Default for System {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerCheck for System {
    fn check(&self, peer: SocketAddr, local: SocketAddr) -> Result<(), Refusal> {
        let owner = read::connection_owner(peer, local).ok_or_else(|| Refusal {
            pid: None,
            why: format!("no connection row for {peer}"),
        })?;
        let service = read::service_pid(SERVICE);
        let (trusted, processes) = self.snapshot.trusted(
            owner,
            service,
            &crate::nirvana_servers::read::processes,
            Instant::now(),
        );
        if trusted {
            return Ok(());
        }
        let name = processes
            .iter()
            .find(|process| process.pid == owner)
            .map_or("unknown", |process| process.name.as_str());
        Err(Refusal {
            pid: Some(owner),
            why: format!("pid {owner} ({name}), not the Tailscale service"),
        })
    }
}

#[cfg(windows)]
pub(crate) mod read {
    use std::mem::size_of;
    use std::net::{IpAddr, SocketAddr};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_CONNECTIONS,
    };
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_MANAGER_CONNECT,
        SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_STATUS_PROCESS,
    };

    const AF_INET: u32 = 2;
    const NO_ERROR: u32 = 0;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

    /// The IPv4 connection table with owning pids. Nirvana Remote binds 127.0.0.1 only, so IPv4 is enough.
    /// The same retry as `nirvana_servers::read::table`, for the connections class instead of listeners.
    fn table() -> Vec<u8> {
        let mut size = 0u32;
        for _ in 0..4 {
            let mut buffer = vec![0u8; size as usize];
            let pointer = if buffer.is_empty() {
                std::ptr::null_mut()
            } else {
                buffer.as_mut_ptr().cast()
            };
            let result = unsafe {
                GetExtendedTcpTable(
                    pointer,
                    &mut size,
                    0,
                    AF_INET,
                    TCP_TABLE_OWNER_PID_CONNECTIONS,
                    0,
                )
            };
            match result {
                NO_ERROR if !buffer.is_empty() => return buffer,
                NO_ERROR | ERROR_INSUFFICIENT_BUFFER => continue,
                _ => return Vec::new(),
            }
        }
        Vec::new()
    }

    fn port(raw: u32) -> u16 {
        u16::from_be((raw & 0xFFFF) as u16)
    }

    /// `dwLocalAddr` is the address in network byte order.
    fn addr(raw: u32) -> std::net::Ipv4Addr {
        std::net::Ipv4Addr::from(raw.to_ne_bytes())
    }

    /// The pid that owns the caller's end: the row whose local end is `peer` and whose remote end is `local`.
    pub fn connection_owner(peer: SocketAddr, local: SocketAddr) -> Option<u32> {
        let (IpAddr::V4(peer_ip), IpAddr::V4(local_ip)) = (peer.ip(), local.ip()) else {
            return None;
        };
        let buffer = table();
        if buffer.len() < 4 {
            return None;
        }
        let count = u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
        let start = 4usize.next_multiple_of(std::mem::align_of::<MIB_TCPROW_OWNER_PID>());
        (0..count).find_map(|index| {
            let offset = start + index * size_of::<MIB_TCPROW_OWNER_PID>();
            if offset + size_of::<MIB_TCPROW_OWNER_PID>() > buffer.len() {
                return None;
            }
            let row = unsafe {
                std::ptr::read_unaligned(buffer.as_ptr().add(offset).cast::<MIB_TCPROW_OWNER_PID>())
            };
            (addr(row.dwLocalAddr) == peer_ip
                && port(row.dwLocalPort) == peer.port()
                && addr(row.dwRemoteAddr) == local_ip
                && port(row.dwRemotePort) == local.port())
            .then_some(row.dwOwningPid)
        })
    }

    /// The pid of a running service, read through the service manager (a non-admin may query status).
    pub fn service_pid(name: &str) -> Option<u32> {
        let wide = name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();
        unsafe {
            let manager = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
            if manager.is_null() {
                return None;
            }
            let service = OpenServiceW(manager, wide.as_ptr(), SERVICE_QUERY_STATUS);
            let mut pid = None;
            if !service.is_null() {
                let mut status: SERVICE_STATUS_PROCESS = std::mem::zeroed();
                let mut needed = 0u32;
                let ok = QueryServiceStatusEx(
                    service,
                    SC_STATUS_PROCESS_INFO,
                    (&mut status as *mut SERVICE_STATUS_PROCESS).cast(),
                    size_of::<SERVICE_STATUS_PROCESS>() as u32,
                    &mut needed,
                );
                if ok != 0 && status.dwProcessId != 0 {
                    pid = Some(status.dwProcessId);
                }
                CloseServiceHandle(service);
            }
            CloseServiceHandle(manager);
            pid
        }
    }
}

/// Not built for other systems: there `start` reports `Off::NotWindows` and nothing listens.
#[cfg(not(windows))]
pub(crate) mod read {
    use std::net::SocketAddr;

    pub fn connection_owner(_peer: SocketAddr, _local: SocketAddr) -> Option<u32> {
        None
    }

    pub fn service_pid(_name: &str) -> Option<u32> {
        None
    }
}
