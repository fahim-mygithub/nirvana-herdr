//! Nirvana fork, feature 4: the servers running in each space, for the sidebar's "servers" block.
//!
//! The owner, 2026-10-15: "Lets work on the next feature. This running server and usage from Nirvana lets
//! port it to the Nirvana herder" (nirvana/plans/2026-10-15-04-servers-and-usage.md).
//!
//! Old Nirvana listed the servers its daemon started. herdr starts none, so a server here is any TCP port
//! listened on by a process inside a space's panes: the pane's shell and everything under it, so a dev
//! server Claude starts with a background Bash counts. The server reads this on one worker thread and sends
//! the ports in the space's existing `tokens`, so herdr's wire format is unchanged.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// The space token holding its servers' ports, e.g. "5173,8770".
pub const SERVERS_TOKEN: &str = "nirvana_servers";
/// How often the worker reads the listener table.
pub const REFRESH: Duration = Duration::from_secs(3);
/// Windows hands a program that asks for any free port one from 49152 up. The live check on 2026-10-15
/// found an idle Claude Code listening on 64839 and agy's app on 60015 and 60016: plumbing, not servers.
/// A dev server listens on a port someone chose (3000, 5173, 8000, 8770).
pub const DYNAMIC_PORTS_START: u16 = 49152;
/// An agent program's own listener is its plumbing (an IDE bridge, a login callback), not a server.
const AGENT_PROGRAMS: &[&str] = &["claude", "codex", "agy", "antigravity"];

/// One running process: what the worker needs to walk a pane's tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
}

/// One listening TCP socket and the process that owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listener {
    pub pid: u32,
    pub port: u16,
}

/// Whether a process name is an agent program: "claude.exe", "Antigravity.exe", "codex".
pub fn is_agent_program(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    AGENT_PROGRAMS.contains(&stem)
}

/// Whether a listener counts as a server: a chosen port, owned by something other than an agent.
pub fn counts(port: u16, owner: Option<&str>) -> bool {
    port != 0 && port < DYNAMIC_PORTS_START && !owner.is_some_and(is_agent_program)
}

/// The ports listened on inside the trees under `shell_pids`, given one process table and one listener
/// table read at the same moment. Sorted, without repeats (a server on IPv4 and IPv6 is one row).
pub fn space_ports(shell_pids: &[u32], processes: &[Process], listeners: &[Listener]) -> Vec<u16> {
    let mut children = HashMap::<u32, Vec<u32>>::new();
    let mut names = HashMap::<u32, &str>::new();
    for process in processes {
        // A pid that names itself as parent (pid 0, the idle process) would loop the walk.
        if process.parent_pid != process.pid {
            children
                .entry(process.parent_pid)
                .or_default()
                .push(process.pid);
        }
        names.insert(process.pid, process.name.as_str());
    }
    let mut tree = HashSet::new();
    let mut queue = shell_pids
        .iter()
        .copied()
        .filter(|pid| *pid != 0)
        .collect::<Vec<_>>();
    while let Some(pid) = queue.pop() {
        if tree.insert(pid) {
            queue.extend(children.get(&pid).into_iter().flatten().copied());
        }
    }
    let mut ports = listeners
        .iter()
        .filter(|listener| tree.contains(&listener.pid))
        .filter(|listener| counts(listener.port, names.get(&listener.pid).copied()))
        .map(|listener| listener.port)
        .collect::<Vec<_>>();
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// Whether any of a space's pane shells still runs. The worker keeps reading a space until none does:
/// the server rebuilds a window's snapshot (and asks again) only when the sidebar's state changes, not
/// on pane output, so a space read only while asked stopped being read. The live run on 2026-10-15
/// kept a stopped server's row this way (plan Record).
pub fn shell_alive(shell_pids: &[u32], processes: &[Process]) -> bool {
    processes
        .iter()
        .any(|process| process.pid != 0 && shell_pids.contains(&process.pid))
}

/// The token's value: ports joined by commas.
pub fn encode(ports: &[u16]) -> String {
    ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// The token's value read back; anything that is not a port is skipped.
pub fn decode(value: &str) -> Vec<u16> {
    value
        .split(',')
        .filter_map(|part| part.trim().parse::<u16>().ok())
        .filter(|port| *port != 0)
        .collect()
}

/// `pub(crate)` for O5's gate, whose served axis reads the same two tables for a worker's pane.
#[cfg(windows)]
pub(crate) mod read {
    use super::{Listener, Process};
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
        TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    const AF_INET: u32 = 2;
    const AF_INET6: u32 = 23;
    const NO_ERROR: u32 = 0;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

    pub fn processes() -> Vec<Process> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Vec::new();
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut output = Vec::new();
        let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
        while ok {
            let len = entry
                .szExeFile
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(entry.szExeFile.len());
            output.push(Process {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                name: String::from_utf16_lossy(&entry.szExeFile[..len]),
            });
            ok = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
        }
        unsafe { CloseHandle(snapshot) };
        output
    }

    /// The listener table for one address family, as raw bytes; it grows between the size query and
    /// the read when a socket opens, so the read retries.
    fn table(family: u32) -> Vec<u8> {
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
                    family,
                    TCP_TABLE_OWNER_PID_LISTENER,
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

    /// The rows after the table's leading `dwNumEntries`, read without trusting the buffer's alignment.
    fn rows<T: Copy>(buffer: &[u8]) -> Vec<T> {
        if buffer.len() < 4 {
            return Vec::new();
        }
        let count = u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
        // Rows start at the row type's alignment after the count (4 for both row types).
        let start = 4usize.next_multiple_of(std::mem::align_of::<T>());
        (0..count)
            .filter_map(|index| {
                let offset = start + index * size_of::<T>();
                (offset + size_of::<T>() <= buffer.len()).then(|| unsafe {
                    std::ptr::read_unaligned(buffer.as_ptr().add(offset).cast::<T>())
                })
            })
            .collect()
    }

    /// `dwLocalPort` holds the port in network byte order in its low 16 bits.
    fn port(raw: u32) -> u16 {
        u16::from_be((raw & 0xFFFF) as u16)
    }

    pub fn listeners() -> Vec<Listener> {
        let mut output = rows::<MIB_TCPROW_OWNER_PID>(&table(AF_INET))
            .into_iter()
            .map(|row| Listener {
                pid: row.dwOwningPid,
                port: port(row.dwLocalPort),
            })
            .collect::<Vec<_>>();
        output.extend(
            rows::<MIB_TCP6ROW_OWNER_PID>(&table(AF_INET6))
                .into_iter()
                .map(|row| Listener {
                    pid: row.dwOwningPid,
                    port: port(row.dwLocalPort),
                }),
        );
        output
    }
}

/// Not built for other systems yet: the owner runs Windows. Spaces there show no servers.
#[cfg(not(windows))]
pub(crate) mod read {
    use super::{Listener, Process};

    pub fn processes() -> Vec<Process> {
        Vec::new()
    }

    pub fn listeners() -> Vec<Listener> {
        Vec::new()
    }
}

struct Space {
    shell_pids: Vec<u32>,
    ports: Vec<u16>,
}

struct State {
    spaces: HashMap<String, Space>,
    waker: Option<crate::nirvana_spaces::Waker>,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(State {
            spaces: HashMap::new(),
            waker: None,
        })
    })
}

fn start_worker() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = crate::thread_spawn::spawn_named("nirvana-servers", || loop {
            std::thread::sleep(REFRESH);
            let asked = {
                let state = state().lock().unwrap_or_else(|e| e.into_inner());
                state
                    .spaces
                    .iter()
                    .map(|(id, space)| (id.clone(), space.shell_pids.clone()))
                    .collect::<Vec<_>>()
            };
            if asked.is_empty() {
                continue;
            }
            let processes = read::processes();
            let listeners = read::listeners();
            let mut state = state().lock().unwrap_or_else(|e| e.into_inner());
            let mut changed = false;
            for (id, shell_pids) in asked {
                if !shell_alive(&shell_pids, &processes) {
                    state.spaces.remove(&id);
                    continue;
                }
                let ports = space_ports(&shell_pids, &processes, &listeners);
                if let Some(space) = state.spaces.get_mut(&id) {
                    if space.ports != ports {
                        space.ports = ports;
                        changed = true;
                    }
                }
            }
            if changed {
                if let Some(waker) = &state.waker {
                    waker.dirty.request_generic();
                    waker.notify.notify_one();
                }
            }
        });
    });
}

/// The ports last read for this space, and the space's shells recorded for the next read. Never blocks
/// on a process or socket read: the server's loop calls this while it builds each client's snapshot.
pub fn cached_ports(
    workspace_id: &str,
    shell_pids: Vec<u32>,
    waker: &crate::nirvana_spaces::Waker,
) -> Vec<u16> {
    start_worker();
    let mut state = state().lock().unwrap_or_else(|e| e.into_inner());
    if state.waker.is_none() {
        state.waker = Some(waker.clone());
    }
    let space = state
        .spaces
        .entry(workspace_id.to_owned())
        .or_insert_with(|| Space {
            shell_pids: Vec::new(),
            ports: Vec::new(),
        });
    space.shell_pids = shell_pids;
    space.ports.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, parent_pid: u32, name: &str) -> Process {
        Process {
            pid,
            parent_pid,
            name: name.to_owned(),
        }
    }

    fn listener(pid: u32, port: u16) -> Listener {
        Listener { pid, port }
    }

    #[test]
    fn a_server_anywhere_under_the_pane_shell_is_the_spaces() {
        // shell 10 -> claude 11 -> bash 12 -> node 13 listening on 5173; another pane's 20 on 3000.
        let processes = [
            process(10, 1, "pwsh.exe"),
            process(11, 10, "claude.exe"),
            process(12, 11, "bash.exe"),
            process(13, 12, "node.exe"),
            process(20, 1, "pwsh.exe"),
            process(21, 20, "python.exe"),
        ];
        let listeners = [listener(13, 5173), listener(21, 3000)];
        assert_eq!(space_ports(&[10], &processes, &listeners), vec![5173]);
        assert_eq!(space_ports(&[20], &processes, &listeners), vec![3000]);
        assert_eq!(
            space_ports(&[10, 20], &processes, &listeners),
            vec![3000, 5173]
        );
    }

    #[test]
    fn plumbing_ports_are_not_servers() {
        let processes = [
            process(10, 1, "pwsh.exe"),
            process(11, 10, "claude.exe"),
            process(12, 11, "node.exe"),
            process(13, 10, "Antigravity.exe"),
        ];
        let listeners = [
            // Claude Code's own listener on a chosen port, agy's app, and a dynamic port.
            listener(11, 8123),
            listener(13, 9000),
            listener(12, 64839),
            listener(12, 8770),
        ];
        assert_eq!(space_ports(&[10], &processes, &listeners), vec![8770]);
        assert!(is_agent_program("claude.exe") && is_agent_program("Antigravity.exe"));
        assert!(is_agent_program("codex") && !is_agent_program("node.exe"));
        assert!(counts(49151, Some("node.exe")) && !counts(49152, Some("node.exe")));
    }

    #[test]
    fn a_server_on_both_address_families_is_one_port() {
        let processes = [process(10, 1, "pwsh.exe"), process(11, 10, "node.exe")];
        let listeners = [listener(11, 8770), listener(11, 8770), listener(11, 3000)];
        assert_eq!(space_ports(&[10], &processes, &listeners), vec![3000, 8770]);
    }

    #[test]
    fn a_parent_loop_does_not_hang_the_walk() {
        let processes = [
            process(0, 0, "Idle"),
            process(10, 11, "a"),
            process(11, 10, "b"),
        ];
        let listeners = [listener(11, 4000), listener(0, 135)];
        assert_eq!(space_ports(&[10], &processes, &listeners), vec![4000]);
        assert!(space_ports(&[0], &processes, &listeners).is_empty());
    }

    #[test]
    fn a_space_is_read_while_a_shell_lives() {
        let processes = [process(0, 0, "Idle"), process(10, 1, "pwsh.exe")];
        assert!(shell_alive(&[10], &processes));
        assert!(shell_alive(&[99, 10], &processes));
        assert!(!shell_alive(&[99], &processes));
        assert!(!shell_alive(&[0], &processes));
    }

    #[test]
    fn the_token_round_trips() {
        assert_eq!(encode(&[3000, 8770]), "3000,8770");
        assert_eq!(decode("3000,8770"), vec![3000, 8770]);
        assert_eq!(decode(" 80 ,x,,0,70000,443"), vec![80, 443]);
        assert!(decode("").is_empty());
    }

    /// The live table: whatever this machine listens on now, read without a crash, ports not zero.
    #[cfg(windows)]
    #[test]
    fn the_listener_table_reads_this_machine() {
        let listeners = read::listeners();
        assert!(listeners.iter().all(|listener| listener.port != 0));
        assert!(!read::processes().is_empty());
    }
}
