//! The job object: a process's whole tree ends with it (§4, Argon's review F5), and the pipe reading every supervisor
//! of a child process shares.
//!
//! Plan 40 review B2: lifted out of `nirvana_worker` so the gate's test runner uses it too. The gate killed only the
//! direct child on a timeout (`cmd /C cargo test`), a hung test binary kept the pipe open, the readers never reached
//! EOF, and the gate held the project's gate lock until the owner killed it. Review B3: the readers decode lossily,
//! so one byte that is not UTF-8 never ends a reader (it ended the worker's stdout reader and the job runner's).

use std::io::BufRead;
use std::time::{Duration, Instant};

#[cfg(windows)]
mod imp {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    /// A kill-on-close job: closing it, or `kill`, ends every process the child started.
    pub struct Job(HANDLE);

    impl Job {
        pub fn new() -> std::io::Result<Self> {
            let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let job = Self(job);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&limits).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(job)
        }

        /// Puts a child started suspended in the job, then lets it run, so nothing it starts escapes.
        pub fn adopt(&self, child: &std::process::Child) -> std::io::Result<()> {
            if unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle().cast()) } == 0 {
                return Err(std::io::Error::last_os_error());
            }
            resume(child.id())
        }

        pub fn kill(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn resume(pid: u32) -> std::io::Result<()> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        let mut resumed = false;
        let mut ok = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        while ok {
            if entry.th32OwnerProcessID == pid {
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if !thread.is_null() {
                    unsafe {
                        ResumeThread(thread);
                        CloseHandle(thread);
                    }
                    resumed = true;
                }
            }
            ok = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        unsafe {
            CloseHandle(snapshot);
        }
        if resumed {
            Ok(())
        } else {
            Err(std::io::Error::other(
                "the worker's thread could not be resumed",
            ))
        }
    }

    /// Started suspended until it is in the job, so nothing it starts escapes the job; no console window.
    pub fn prepare(command: &mut std::process::Command) {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
    }
}

/// Off Windows the child is killed alone; not built or measured there.
#[cfg(not(windows))]
mod imp {
    pub struct Job;
    impl Job {
        pub fn new() -> std::io::Result<Self> {
            Ok(Self)
        }
        pub fn adopt(&self, _child: &std::process::Child) -> std::io::Result<()> {
            Ok(())
        }
        pub fn kill(&self) {}
    }
    pub fn prepare(_command: &mut std::process::Command) {}
}

pub use imp::Job;

/// Starts `command` inside a new job. On Windows it is created suspended and resumed once adopted; its creation
/// flags are set here, so a caller sets none of its own.
pub fn spawn(command: &mut std::process::Command) -> std::io::Result<(Job, std::process::Child)> {
    imp::prepare(command);
    let job = Job::new()?;
    let mut child = command.spawn()?;
    if let Err(err) = job.adopt(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(err);
    }
    Ok((job, child))
}

/// Each line of `reader`, `\r\n` or `\n` cut off, decoded lossily: a byte that is not UTF-8 becomes U+FFFD and the
/// reading goes on (review B3). `each` returns false to stop early. Ends at EOF or a read error.
pub fn lossy_lines(reader: impl std::io::Read, mut each: impl FnMut(String) -> bool) {
    let mut reader = std::io::BufReader::new(reader);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                if bytes.last() == Some(&b'\n') {
                    bytes.pop();
                    if bytes.last() == Some(&b'\r') {
                        bytes.pop();
                    }
                }
                if !each(String::from_utf8_lossy(&bytes).into_owned()) {
                    return;
                }
            }
        }
    }
}

/// Joins `handle` if it finishes within `deadline`, else leaves the thread to end on its own and returns `None`.
/// Review B2: after a kill a reader can still wait on a pipe some escaped process holds; the caller goes on.
pub fn join_within<T: Send + 'static>(
    handle: std::thread::JoinHandle<T>,
    deadline: Duration,
) -> Option<T> {
    let started = Instant::now();
    while !handle.is_finished() {
        if started.elapsed() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    handle.join().ok()
}
