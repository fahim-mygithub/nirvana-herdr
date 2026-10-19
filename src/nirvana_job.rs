//! Nirvana fork, plan 15: Jobs (long commands in view).
//! `herdr job run|exec|list|status|tail|stop|clean`
//! (nirvana/plans/2026-10-19-15-jobs.md §2)

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 7 days in seconds for `herdr job clean`.
pub const CLEAN_MAX_AGE_SECS: u64 = 7 * 24 * 3600;

/// The state of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Running,
    Passed,
    Failed,
    Lost,
}

impl JobState {
    pub fn word(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Lost => "lost",
        }
    }
}

/// The record stored in `<home>/nirvana/jobs/<id>/job.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobRecord {
    pub id: String,
    pub name: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub space: Option<String>,
    pub pane: Option<String>,
    pub started: u64,
    pub ended: Option<u64>,
    pub exit: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

impl JobRecord {
    /// Determines the job's state given whether its process is currently alive.
    pub fn state(&self, is_alive: bool) -> JobState {
        if let Some(code) = self.exit {
            if code == 0 {
                JobState::Passed
            } else {
                JobState::Failed
            }
        } else if self.ended.is_some() {
            JobState::Failed
        } else if is_alive {
            JobState::Running
        } else {
            JobState::Lost
        }
    }
}

/// The root directory for jobs: `<home>/nirvana/jobs`.
pub fn jobs_dir(home: &Path) -> PathBuf {
    home.join("nirvana").join("jobs")
}

/// Directory for a specific job: `<home>/nirvana/jobs/<id>`.
pub fn job_dir(home: &Path, id: &str) -> PathBuf {
    jobs_dir(home).join(id)
}

/// Path to `job.json` for a specific job.
pub fn record_path(home: &Path, id: &str) -> PathBuf {
    job_dir(home, id).join("job.json")
}

/// Path to `out.log` for a specific job.
pub fn log_path(home: &Path, id: &str) -> PathBuf {
    job_dir(home, id).join("out.log")
}

/// Current Unix timestamp in seconds.
pub fn now() -> u64 {
    #[cfg(test)]
    if let Some(now) = TEST_NOW.with_borrow(|now| *now) {
        return now;
    }
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
thread_local! {
    static TEST_NOW: std::cell::RefCell<Option<u64>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn with_test_now<T>(time: u64, f: impl FnOnce() -> T) -> T {
    TEST_NOW.with_borrow_mut(|now| *now = Some(time));
    let result = f();
    TEST_NOW.with_borrow_mut(|now| *now = None);
    result
}

/// Generates a unique, valid job ID.
pub fn generate_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("job-{now}-{count:04x}")
}

/// Creates a new job record and its directory on disk.
pub fn create_job(
    home: &Path,
    id: &str,
    name: &str,
    argv: Vec<String>,
    cwd: &str,
    space: Option<String>,
    pane: Option<String>,
    pid: Option<u32>,
) -> std::io::Result<JobRecord> {
    let dir = job_dir(home, id);
    std::fs::create_dir_all(&dir)?;
    let record = JobRecord {
        id: id.to_owned(),
        name: name.to_owned(),
        argv,
        cwd: cwd.to_owned(),
        space,
        pane,
        started: now(),
        ended: None,
        exit: None,
        pid,
    };
    write_job(home, &record)?;
    Ok(record)
}

/// Reads a job's record from disk.
pub fn read_job(home: &Path, id: &str) -> std::io::Result<JobRecord> {
    let path = record_path(home, id);
    read_job_file(&path)
}

/// Reads a `job.json` file.
pub fn read_job_file(path: &Path) -> std::io::Result<JobRecord> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Writes a job's record to disk atomically.
pub fn write_job(home: &Path, record: &JobRecord) -> std::io::Result<()> {
    let dir = job_dir(home, &record.id);
    std::fs::create_dir_all(&dir)?;
    let path = record_path(home, &record.id);
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = dir.join(format!("job.json.tmp.{}", std::process::id()));
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

/// Returns whether a process with `pid` is currently alive.
pub fn is_process_alive(pid: u32) -> bool {
    if pid <= 4 {
        return false;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        const STILL_ACTIVE: u32 = 259;
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let ok = unsafe { GetExitCodeProcess(handle, &mut exit_code) } != 0;
        unsafe { CloseHandle(handle) };
        ok && exit_code == STILL_ACTIVE
    }
    #[cfg(not(windows))]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
}

/// Evaluates the current state of a job record, inspecting process liveness if needed.
pub fn job_state(record: &JobRecord) -> JobState {
    let is_alive = record.pid.is_some_and(is_process_alive);
    record.state(is_alive)
}

/// Lists all jobs in `<home>/nirvana/jobs`, newest first (by `started` descending).
pub fn list_jobs(home: &Path) -> Vec<JobRecord> {
    let dir = jobs_dir(home);
    let mut jobs = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return jobs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let id = path.file_name().unwrap_or_default().to_string_lossy();
            if let Ok(record) = read_job(home, &id) {
                jobs.push(record);
            }
        }
    }
    jobs.sort_by(|a, b| b.started.cmp(&a.started).then_with(|| b.id.cmp(&a.id)));
    jobs
}

/// Extracts the last N lines from a multiline string.
pub fn tail_lines(content: &str, n: usize) -> Vec<String> {
    if n == 0 {
        return Vec::new();
    }
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].iter().map(|s| s.to_string()).collect()
}

/// Reads the last N lines of a job's log file.
pub fn tail_job(home: &Path, id: &str, n: usize) -> std::io::Result<Vec<String>> {
    let path = log_path(home, id);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    Ok(tail_lines(&content, n))
}

/// Returns the last non-empty line of a job's log, if any.
pub fn last_line(home: &Path, id: &str) -> Option<String> {
    tail_job(home, id, 1).ok()?.into_iter().next()
}

/// Formats a single job's status line for `job list` or `job status`.
pub fn format_job_line(record: &JobRecord, last: Option<&str>, now_secs: u64) -> String {
    let is_alive = record.pid.is_some_and(is_process_alive);
    let state = record.state(is_alive);
    let elapsed_secs = match record.ended {
        Some(ended) => ended.saturating_sub(record.started),
        None => now_secs.saturating_sub(record.started),
    };
    let elapsed = crate::nirvana_running::elapsed(elapsed_secs);
    let line = last.unwrap_or("").trim();
    if line.is_empty() {
        format!(
            "{:<20}  {:<16}  {:<8}  {:>8}",
            record.id,
            record.name,
            state.word(),
            elapsed
        )
    } else {
        format!(
            "{:<20}  {:<16}  {:<8}  {:>8}  {}",
            record.id,
            record.name,
            state.word(),
            elapsed,
            line
        )
    }
}

/// Removes jobs that ended over 7 days ago. Returns count of removed jobs.
pub fn clean_jobs(home: &Path, now_secs: u64) -> std::io::Result<usize> {
    let dir = jobs_dir(home);
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut cleaned = 0;
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let job_file = path.join("job.json");
        if let Ok(record) = read_job_file(&job_file) {
            if let Some(ended) = record.ended {
                if now_secs.saturating_sub(ended) > CLEAN_MAX_AGE_SECS {
                    let _ = std::fs::remove_dir_all(&path);
                    cleaned += 1;
                }
            }
        }
    }
    Ok(cleaned)
}

/// Kills the entire process tree of a given PID.
pub fn kill_process_tree(pid: u32) {
    if pid <= 4 {
        return;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };
        let mut to_kill = vec![pid];
        let mut index = 0;
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot != INVALID_HANDLE_VALUE {
            let mut processes = Vec::new();
            let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
            while ok {
                processes.push((entry.th32ProcessID, entry.th32ParentProcessID));
                ok = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
            }
            unsafe { CloseHandle(snapshot) };

            while index < to_kill.len() {
                let current_parent = to_kill[index];
                index += 1;
                for &(child_pid, parent_pid) in &processes {
                    if parent_pid == current_parent
                        && child_pid > 4
                        && !to_kill.contains(&child_pid)
                    {
                        to_kill.push(child_pid);
                    }
                }
            }
        }
        to_kill.reverse();
        crate::nirvana::terminate_processes(&to_kill);
    }
    #[cfg(not(windows))]
    {
        crate::nirvana::terminate_processes(&[pid]);
    }
}

/// Terminates a named Win32 job object if one was created for this job ID.
#[cfg(windows)]
pub fn terminate_named_job(id: &str) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::{OpenJobObjectW, TerminateJobObject};
    const JOB_OBJECT_TERMINATE: u32 = 0x0001;
    let name_wide: Vec<u16> = format!("nirvana-job-{id}\0").encode_utf16().collect();
    let job = unsafe { OpenJobObjectW(JOB_OBJECT_TERMINATE, 0, name_wide.as_ptr()) };
    if !job.is_null() {
        unsafe {
            TerminateJobObject(job, 1);
            CloseHandle(job);
        }
    }
}

/// Stops a job's process tree and marks it ended if not already ended.
pub fn stop_job(home: &Path, id: &str) -> std::io::Result<()> {
    let mut record = read_job(home, id)?;
    #[cfg(windows)]
    terminate_named_job(id);
    if let Some(pid) = record.pid {
        kill_process_tree(pid);
    }
    if record.ended.is_none() {
        record.ended = Some(now());
        record.exit = Some(1);
        let _ = write_job(home, &record);
    }
    Ok(())
}

/// CLI entry point for `herdr job`.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    match args.first().map(String::as_str) {
        Some("list") => list_cli(&args[1..]),
        Some("status") => status_cli(&args[1..]),
        Some("tail") => tail_cli(&args[1..]),
        Some("stop") => stop_cli(&args[1..]),
        Some("clean") => clean_cli(&args[1..]),
        Some("exec") => exec_cli(&args[1..]),
        Some("run") => run_cli(&args[1..]),
        _ => {
            eprintln!("usage: herdr job <run|exec|list|status|tail|stop|clean> ...");
            std::process::exit(2);
        }
    }
}

fn list_cli(_args: &[String]) -> std::io::Result<()> {
    let home =
        crate::nirvana::home().ok_or_else(|| std::io::Error::other("no home folder found"))?;
    let jobs = list_jobs(&home);
    let now_secs = now();
    for job in jobs {
        let last = last_line(&home, &job.id);
        println!("{}", format_job_line(&job, last.as_deref(), now_secs));
    }
    Ok(())
}

fn status_cli(args: &[String]) -> std::io::Result<()> {
    let Some(id) = args.first() else {
        eprintln!("usage: herdr job status <id>");
        std::process::exit(2);
    };
    let home =
        crate::nirvana::home().ok_or_else(|| std::io::Error::other("no home folder found"))?;
    let job = match read_job(&home, id) {
        Ok(job) => job,
        Err(_) => {
            eprintln!("no job {id}");
            std::process::exit(1);
        }
    };
    let now_secs = now();
    let last = last_line(&home, id);
    println!("{}", format_job_line(&job, last.as_deref(), now_secs));
    Ok(())
}

fn tail_cli(args: &[String]) -> std::io::Result<()> {
    let mut id = None;
    let mut n = 20usize;
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "-n" {
            if let Some(val) = iter.next() {
                n = val.parse().unwrap_or(20);
            }
        } else if let Some(val) = arg.strip_prefix("-n") {
            n = val.parse().unwrap_or(20);
        } else if !arg.starts_with('-') && id.is_none() {
            id = Some(arg.clone());
        }
    }
    let Some(id) = id else {
        eprintln!("usage: herdr job tail <id> [-n N]");
        std::process::exit(2);
    };
    let home =
        crate::nirvana::home().ok_or_else(|| std::io::Error::other("no home folder found"))?;
    let lines = tail_job(&home, &id, n)?;
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

fn clean_cli(_args: &[String]) -> std::io::Result<()> {
    let home =
        crate::nirvana::home().ok_or_else(|| std::io::Error::other("no home folder found"))?;
    let count = clean_jobs(&home, now())?;
    if count > 0 {
        println!("cleaned {count} job{}", if count == 1 { "" } else { "s" });
    }
    Ok(())
}

fn stop_cli(args: &[String]) -> std::io::Result<()> {
    let Some(id) = args.first() else {
        eprintln!("usage: herdr job stop <id>");
        std::process::exit(2);
    };
    let home =
        crate::nirvana::home().ok_or_else(|| std::io::Error::other("no home folder found"))?;
    stop_job(&home, id)?;
    println!("stopped {id}");
    Ok(())
}

/// Runs the command for a job, piping output, writing to out.log and stdout,
/// updating ended and exit in job.json, and appending `[job <name>: exit N after Xs]`.
pub fn run_exec(home: &Path, id: &str) -> std::io::Result<i32> {
    let mut record = read_job(home, id)?;
    let log_p = log_path(home, id);
    let mut log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_p)?;

    if record.argv.is_empty() {
        let elapsed = now().saturating_sub(record.started);
        let end_line = format!("[job {}: exit 0 after {elapsed}s]", record.name);
        println!("{end_line}");
        let _ = writeln!(log_file, "{end_line}");
        record.ended = Some(now());
        record.exit = Some(0);
        write_job(home, &record)?;
        return Ok(0);
    }

    let mut command = std::process::Command::new(&record.argv[0]);
    command.args(&record.argv[1..]);
    if !record.cwd.is_empty() {
        command.current_dir(&record.cwd);
    }
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            let err_line = format!("herdr job: failed to spawn {}: {err}", record.argv[0]);
            eprintln!("{err_line}");
            let _ = writeln!(log_file, "{err_line}");
            let elapsed = now().saturating_sub(record.started);
            let end_line = format!("[job {}: exit 127 after {elapsed}s]", record.name);
            println!("{end_line}");
            let _ = writeln!(log_file, "{end_line}");
            record.ended = Some(now());
            record.exit = Some(127);
            let _ = write_job(home, &record);
            return Ok(127);
        }
    };

    let child_pid = child.id();
    record.pid = Some(child_pid);
    let _ = write_job(home, &record);

    #[cfg(windows)]
    let _job_handle = {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        struct ScopedJob(windows_sys::Win32::Foundation::HANDLE);
        impl Drop for ScopedJob {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe { CloseHandle(self.0) };
                }
            }
        }
        let name_wide: Vec<u16> = format!("nirvana-job-{id}\0").encode_utf16().collect();
        let job = unsafe { CreateJobObjectW(std::ptr::null(), name_wide.as_ptr()) };
        if !job.is_null() {
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&limits).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                AssignProcessToJobObject(job, child.as_raw_handle().cast());
            }
        }
        ScopedJob(job)
    };

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let tx_err = tx.clone();

    let stdout = child.stdout.take();
    let t_out = std::thread::spawn(move || {
        if let Some(stdout) = stdout {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        }
    });

    let stderr = child.stderr.take();
    let t_err = std::thread::spawn(move || {
        if let Some(stderr) = stderr {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if tx_err.send(line).is_err() {
                    break;
                }
            }
        }
    });

    while let Ok(line) = rx.recv() {
        println!("{line}");
        let _ = writeln!(log_file, "{line}");
        let _ = log_file.flush();
    }
    let _ = t_out.join();
    let _ = t_err.join();

    let status = child.wait()?;
    let exit_code = status.code().unwrap_or(1);
    let elapsed = now().saturating_sub(record.started);
    let end_line = format!("[job {}: exit {exit_code} after {elapsed}s]", record.name);
    println!("{end_line}");
    let _ = writeln!(log_file, "{end_line}");
    let _ = log_file.flush();

    record.ended = Some(now());
    record.exit = Some(exit_code);
    write_job(home, &record)?;

    Ok(exit_code)
}

fn exec_cli(args: &[String]) -> std::io::Result<()> {
    let Some(id) = args.first() else {
        eprintln!("usage: herdr job exec <id>");
        std::process::exit(2);
    };
    let home =
        crate::nirvana::home().ok_or_else(|| std::io::Error::other("no home folder found"))?;
    let code = run_exec(&home, id)?;
    std::process::exit(code);
}

fn run_cli(_args: &[String]) -> std::io::Result<()> {
    // Implemented in Step 3
    todo!("run implemented in step 3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_record_round_trips_json_with_spec_fields() {
        let guard = crate::nirvana::with_test_home("job_round_trip");
        let record = JobRecord {
            id: "job-1".into(),
            name: "test_build".into(),
            argv: vec!["cargo".into(), "build".into()],
            cwd: "C:/work".into(),
            space: Some("workspace-1".into()),
            pane: Some("pane-1".into()),
            started: 1000,
            ended: Some(1040),
            exit: Some(0),
            pid: Some(12345),
        };
        write_job(&guard.path, &record).expect("write job");
        let loaded = read_job(&guard.path, "job-1").expect("read job");
        assert_eq!(record, loaded);

        // Verify json fields explicitly
        let path = record_path(&guard.path, "job-1");
        let raw = std::fs::read_to_string(&path).expect("read file");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse json");
        assert_eq!(v["id"], "job-1");
        assert_eq!(v["name"], "test_build");
        assert_eq!(v["argv"], serde_json::json!(["cargo", "build"]));
        assert_eq!(v["cwd"], "C:/work");
        assert_eq!(v["space"], "workspace-1");
        assert_eq!(v["pane"], "pane-1");
        assert_eq!(v["started"], 1000);
        assert_eq!(v["ended"], 1040);
        assert_eq!(v["exit"], 0);
    }

    #[test]
    fn job_states_cover_running_passed_failed_lost() {
        let mut record = JobRecord {
            id: "job-state-test".into(),
            name: "demo".into(),
            argv: vec!["python".into()],
            cwd: "C:/work".into(),
            space: None,
            pane: None,
            started: 1000,
            ended: None,
            exit: None,
            pid: Some(1234),
        };

        // 1. Running: no exit, no ended, process alive
        assert_eq!(record.state(true), JobState::Running);
        assert_eq!(record.state(true).word(), "running");

        // 2. Lost: no exit, no ended, process is gone
        assert_eq!(record.state(false), JobState::Lost);
        assert_eq!(record.state(false).word(), "lost");

        // 3. Passed: exit is 0
        record.exit = Some(0);
        record.ended = Some(1010);
        assert_eq!(record.state(false), JobState::Passed);
        assert_eq!(record.state(false).word(), "passed");

        // 4. Failed: non-zero exit
        record.exit = Some(3);
        assert_eq!(record.state(false), JobState::Failed);
        assert_eq!(record.state(false).word(), "failed");

        // 5. Failed: ended with no exit
        record.exit = None;
        assert_eq!(record.state(false), JobState::Failed);
    }

    #[test]
    fn lost_detection_identifies_dead_process_without_exit() {
        let guard = crate::nirvana::with_test_home("job_lost_detection");
        // PID 9999999 is dead/non-existent
        let record = JobRecord {
            id: "job-lost".into(),
            name: "lost_job".into(),
            argv: vec!["foo".into()],
            cwd: ".".into(),
            space: None,
            pane: None,
            started: 100,
            ended: None,
            exit: None,
            pid: Some(9999999),
        };
        write_job(&guard.path, &record).expect("write");
        let state = job_state(&record);
        assert_eq!(state, JobState::Lost);

        // Current process is alive
        let live_record = JobRecord {
            id: "job-live".into(),
            name: "live_job".into(),
            argv: vec!["bar".into()],
            cwd: ".".into(),
            space: None,
            pane: None,
            started: 100,
            ended: None,
            exit: None,
            pid: Some(std::process::id()),
        };
        assert_eq!(job_state(&live_record), JobState::Running);
    }

    #[test]
    fn tail_lines_and_tail_job_read_last_n_lines() {
        let guard = crate::nirvana::with_test_home("job_tail");
        let dir = job_dir(&guard.path, "job-tail");
        std::fs::create_dir_all(&dir).unwrap();
        let log = log_path(&guard.path, "job-tail");

        // Empty / missing log
        assert!(tail_job(&guard.path, "job-tail", 20).unwrap().is_empty());
        assert_eq!(last_line(&guard.path, "job-tail"), None);

        // Write 30 lines
        let lines: Vec<String> = (1..=30).map(|i| format!("line {i}")).collect();
        std::fs::write(&log, lines.join("\n")).unwrap();

        let tailed_20 = tail_job(&guard.path, "job-tail", 20).unwrap();
        assert_eq!(tailed_20.len(), 20);
        assert_eq!(tailed_20.first().unwrap(), "line 11");
        assert_eq!(tailed_20.last().unwrap(), "line 30");

        let tailed_5 = tail_job(&guard.path, "job-tail", 5).unwrap();
        assert_eq!(tailed_5.len(), 5);
        assert_eq!(tailed_5.first().unwrap(), "line 26");
        assert_eq!(tailed_5.last().unwrap(), "line 30");

        assert_eq!(last_line(&guard.path, "job-tail"), Some("line 30".into()));
    }

    #[test]
    fn format_job_line_includes_id_name_state_elapsed_and_last_line() {
        let record = JobRecord {
            id: "job-100".into(),
            name: "gate".into(),
            argv: vec!["cargo".into(), "test".into()],
            cwd: ".".into(),
            space: None,
            pane: None,
            started: 1000,
            ended: Some(1042),
            exit: Some(3),
            pid: None,
        };
        let line = format_job_line(&record, Some("[job gate: exit 3 after 42s]"), 1050);
        assert!(line.contains("job-100"));
        assert!(line.contains("gate"));
        assert!(line.contains("failed"));
        assert!(line.contains("42s"));
        assert!(line.contains("[job gate: exit 3 after 42s]"));
    }

    #[test]
    fn clean_jobs_removes_only_jobs_ended_over_7_days_ago() {
        let guard = crate::nirvana::with_test_home("job_clean");
        let now = 1_000_000;
        let eight_days_ago = now - 8 * 86400;
        let two_days_ago = now - 2 * 86400;

        // Old ended job -> must be removed
        let old = JobRecord {
            id: "job-old".into(),
            name: "old".into(),
            argv: vec![],
            cwd: ".".into(),
            space: None,
            pane: None,
            started: eight_days_ago - 100,
            ended: Some(eight_days_ago),
            exit: Some(0),
            pid: None,
        };
        write_job(&guard.path, &old).unwrap();

        // Recent ended job -> must be retained
        let recent = JobRecord {
            id: "job-recent".into(),
            name: "recent".into(),
            argv: vec![],
            cwd: ".".into(),
            space: None,
            pane: None,
            started: two_days_ago - 100,
            ended: Some(two_days_ago),
            exit: Some(0),
            pid: None,
        };
        write_job(&guard.path, &recent).unwrap();

        // Running job with no ended -> must be retained
        let running = JobRecord {
            id: "job-running".into(),
            name: "running".into(),
            argv: vec![],
            cwd: ".".into(),
            space: None,
            pane: None,
            started: eight_days_ago,
            ended: None,
            exit: None,
            pid: Some(std::process::id()),
        };
        write_job(&guard.path, &running).unwrap();

        let count = clean_jobs(&guard.path, now).unwrap();
        assert_eq!(count, 1);
        assert!(!job_dir(&guard.path, "job-old").exists());
        assert!(job_dir(&guard.path, "job-recent").exists());
        assert!(job_dir(&guard.path, "job-running").exists());
    }

    #[test]
    fn stop_job_ends_only_job_process_tree() {
        let guard = crate::nirvana::with_test_home("job_stop");
        // Start a real child process
        let mut child = if cfg!(windows) {
            std::process::Command::new("ping")
                .args(["-n", "30", "127.0.0.1"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap()
        } else {
            std::process::Command::new("sleep")
                .arg("30")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap()
        };
        let pid = child.id();
        let record = JobRecord {
            id: "job-to-stop".into(),
            name: "child".into(),
            argv: vec!["sleep".into()],
            cwd: ".".into(),
            space: None,
            pane: None,
            started: now(),
            ended: None,
            exit: None,
            pid: Some(pid),
        };
        write_job(&guard.path, &record).unwrap();
        assert!(is_process_alive(pid));

        stop_job(&guard.path, "job-to-stop").unwrap();

        // Child process must be dead
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut dead = false;
        while std::time::Instant::now() < deadline {
            if !is_process_alive(pid) {
                dead = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(dead, "child process was not killed by stop_job");

        let updated = read_job(&guard.path, "job-to-stop").unwrap();
        assert!(updated.ended.is_some());
        assert_eq!(updated.exit, Some(1));
    }

    #[test]
    fn exec_runs_command_copies_output_to_log_and_records_exit() {
        let guard = crate::nirvana::with_test_home("job_exec_pass");
        let record = create_job(
            &guard.path,
            "job-exec-pass",
            "demo_pass",
            vec![
                "python".into(),
                "-c".into(),
                "print('first line'); print('second line')".into(),
            ],
            ".",
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(record.ended, None);
        assert_eq!(record.exit, None);

        let code = run_exec(&guard.path, "job-exec-pass").unwrap();
        assert_eq!(code, 0);

        let updated = read_job(&guard.path, "job-exec-pass").unwrap();
        assert!(updated.ended.is_some());
        assert_eq!(updated.exit, Some(0));
        assert_eq!(updated.state(false), JobState::Passed);

        let lines = tail_job(&guard.path, "job-exec-pass", 10).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "first line");
        assert_eq!(lines[1], "second line");
        assert!(lines[2].starts_with("[job demo_pass: exit 0 after "));
        assert!(lines[2].ends_with("s]"));
    }

    #[test]
    fn exec_records_nonzero_exit_and_ended() {
        let guard = crate::nirvana::with_test_home("job_exec_fail");
        let _ = create_job(
            &guard.path,
            "job-exec-fail",
            "demo_fail",
            vec![
                "python".into(),
                "-c".into(),
                "import sys; print('failing output'); sys.exit(3)".into(),
            ],
            ".",
            None,
            None,
            None,
        )
        .unwrap();

        let code = run_exec(&guard.path, "job-exec-fail").unwrap();
        assert_eq!(code, 3);

        let updated = read_job(&guard.path, "job-exec-fail").unwrap();
        assert!(updated.ended.is_some());
        assert_eq!(updated.exit, Some(3));
        assert_eq!(updated.state(false), JobState::Failed);

        let lines = tail_job(&guard.path, "job-exec-fail", 10).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "failing output");
        assert!(lines[1].starts_with("[job demo_fail: exit 3 after "));
        assert!(lines[1].ends_with("s]"));
    }
}
