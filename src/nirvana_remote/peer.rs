//! Which local process opened a connection to Nirvana Remote, and whether it is the Tailscale service.
//!
//! The login route is `tailscale serve` (record p39a, "The login route"): tailscaled terminates HTTPS, strips any
//! `Tailscale-User-*` header the caller sent and sets its own, and connects to 127.0.0.1. The header is only as good
//! as the process that sent it, and any process of any Windows account can connect to 127.0.0.1, while herdr's own
//! pipe admits SYSTEM and its owner only (`D:P(A;;GA;;;SY)(A;;GA;;;OW)`, src/platform/windows.rs). So the header is
//! read only from a connection the Tailscale service's own process tree opened; everything else is refused before
//! its headers are read. A non-admin cannot start a process under the service, so this check is the pipe's ACL or
//! tighter.

use std::net::SocketAddr;

pub trait PeerCheck: Send + Sync {
    /// `Ok` when the connection from `peer` to `local` was opened by the Tailscale service; otherwise who did.
    fn check(&self, peer: SocketAddr, local: SocketAddr) -> Result<(), String>;
}

/// The Windows service name of Tailscale (`sc queryex Tailscale`, measured 2026-11-04: RUNNING, PID 6716).
pub const SERVICE: &str = "Tailscale";
/// The daemon's image name. The service runs it, and it runs a second `tailscaled.exe` as its child (measured
/// 2026-11-04: 8672, parent 6716); which of the two dials the serve backend is not measured, so both pass.
pub const DAEMON: &str = "tailscaled.exe";

/// The decision, kept apart from the system reads so a test can drive it.
pub fn is_tailscale(
    owner_pid: u32,
    service_pid: Option<u32>,
    processes: &[crate::nirvana_servers::Process],
) -> bool {
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
pub struct System;

impl PeerCheck for System {
    fn check(&self, peer: SocketAddr, local: SocketAddr) -> Result<(), String> {
        let owner = read::connection_owner(peer, local)
            .ok_or_else(|| format!("no connection row for {peer}"))?;
        let service = read::service_pid(SERVICE);
        let processes = crate::nirvana_servers::read::processes();
        if is_tailscale(owner, service, &processes) {
            return Ok(());
        }
        let name = processes
            .iter()
            .find(|process| process.pid == owner)
            .map_or("unknown", |process| process.name.as_str());
        Err(format!("pid {owner} ({name}), not the Tailscale service"))
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
