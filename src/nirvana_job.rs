//! Nirvana fork, plan 15: Jobs (long commands in view).
//! `herdr job run|exec|list|status|tail|stop|clean`
//! (nirvana/plans/2026-10-19-15-jobs.md §2)

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

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

/// The state of a Claude Code task output file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaudeTaskState {
    Running,
    Passed,
    Failed,
    Unknown,
}

impl ClaudeTaskState {
    pub fn word(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

/// A parsed Claude Code background task file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeTask {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub state: ClaudeTaskState,
    pub exit_code: Option<i32>,
    pub modified: u64,
    pub elapsed: u64,
    pub last_line: Option<String>,
}

/// Encodes a folder path for Claude Code task paths:
/// each character that is not an ASCII letter or digit becomes `-`.
pub fn encode_folder(folder: &Path) -> String {
    let s = folder.to_string_lossy();
    encode_folder_str(&s)
}

pub fn encode_folder_str(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Parses a Claude Code task output content and returns (state, exit_code, last_line).
pub fn parse_claude_task_content(
    content: &str,
    modified_secs: u64,
    now_secs: u64,
) -> (ClaudeTaskState, Option<i32>, Option<String>) {
    let mut exit_code = None;
    let mut last_line = None;

    for line in content.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if last_line.is_none() {
            last_line = Some(trimmed.to_string());
        }
        if exit_code.is_none() {
            if let Some(rest) = trimmed.strip_prefix("[exited with code ") {
                if let Some(code_str) = rest.strip_suffix(']') {
                    if let Ok(code) = code_str.trim().parse::<i32>() {
                        exit_code = Some(code);
                    }
                }
            }
        }
        if exit_code.is_some() && last_line.is_some() {
            break;
        }
    }

    let state = match exit_code {
        Some(0) => ClaudeTaskState::Passed,
        Some(_) => ClaudeTaskState::Failed,
        None => {
            if now_secs.saturating_sub(modified_secs) >= 24 * 3600 {
                ClaudeTaskState::Unknown
            } else {
                ClaudeTaskState::Running
            }
        }
    };

    (state, exit_code, last_line)
}

/// Reads a single Claude task output file.
pub fn read_claude_task(path: &Path, now_secs: u64) -> std::io::Result<ClaudeTask> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let content = std::fs::read_to_string(path)?;
    let (state, exit_code, last_line) = parse_claude_task_content(&content, modified, now_secs);

    let id = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let name = format!("task·{id}");
    let elapsed = now_secs.saturating_sub(modified);

    Ok(ClaudeTask {
        id,
        name,
        path: path.to_path_buf(),
        state,
        exit_code,
        modified,
        elapsed,
        last_line,
    })
}

/// Lists newest 10 Claude task files modified in the last 12 hours under
/// `<temp_base>/claude/<encoded folder>/*/tasks/*.output`.
pub fn list_claude_tasks_in(
    temp_base: &Path,
    space_folder: &Path,
    now_secs: u64,
) -> Vec<ClaudeTask> {
    let encoded = encode_folder(space_folder);
    let base = temp_base.join("claude").join(encoded);
    let Ok(session_entries) = std::fs::read_dir(base) else {
        return Vec::new();
    };

    let mut tasks = Vec::new();
    let twelve_hours = 12 * 3600;

    for session_entry in session_entries.flatten() {
        let session_dir = session_entry.path();
        if !session_dir.is_dir() {
            continue;
        }
        let tasks_dir = session_dir.join("tasks");
        let Ok(task_entries) = std::fs::read_dir(tasks_dir) else {
            continue;
        };
        for task_entry in task_entries.flatten() {
            let task_path = task_entry.path();
            if task_path.is_file() {
                if let Ok(metadata) = std::fs::metadata(&task_path) {
                    let mtime = metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    // Modified in the last 12 hours
                    if now_secs.saturating_sub(mtime) <= twelve_hours {
                        if let Ok(task) = read_claude_task(&task_path, now_secs) {
                            tasks.push(task);
                        }
                    }
                }
            }
        }
    }

    // Sort newest first
    tasks.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| b.id.cmp(&a.id)));
    tasks.truncate(10);
    tasks
}

/// Lists newest 10 Claude tasks for `space_folder` in the system `%TEMP%`.
pub fn list_claude_tasks(space_folder: &Path) -> Vec<ClaudeTask> {
    list_claude_tasks_in(&std::env::temp_dir(), space_folder, now())
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RunArgs {
    pub name: String,
    pub cwd: Option<String>,
    pub wait: bool,
    pub argv: Vec<String>,
}

/// Parses arguments for `herdr job run`.
pub fn parse_run_args(args: &[String]) -> Result<RunArgs, String> {
    let mut name = None;
    let mut cwd = None;
    let mut wait = false;
    let mut argv = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            argv.extend(args[i + 1..].iter().cloned());
            break;
        } else if arg == "--name" {
            i += 1;
            if i >= args.len() {
                return Err("missing value for --name".into());
            }
            name = Some(args[i].clone());
        } else if let Some(n) = arg.strip_prefix("--name=") {
            name = Some(n.to_string());
        } else if arg == "--cwd" {
            i += 1;
            if i >= args.len() {
                return Err("missing value for --cwd".into());
            }
            cwd = Some(args[i].clone());
        } else if let Some(c) = arg.strip_prefix("--cwd=") {
            cwd = Some(c.to_string());
        } else if arg == "--wait" {
            wait = true;
        } else if !arg.starts_with('-') {
            argv.extend(args[i..].iter().cloned());
            break;
        } else {
            return Err(format!("unknown option: {arg}"));
        }
        i += 1;
    }
    let Some(name) = name else {
        return Err("missing required argument --name".into());
    };
    if argv.is_empty() {
        return Err("missing command to run".into());
    }
    Ok(RunArgs {
        name,
        cwd,
        wait,
        argv,
    })
}

fn herdr_command(args: &[&str]) -> std::process::Command {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("herdr"));
    let mut command = std::process::Command::new(exe);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    crate::platform::configure_background_command(&mut command);
    command
}

fn herdr_json(args: &[&str]) -> Result<Value, String> {
    let out = herdr_command(args)
        .output()
        .map_err(|err| format!("herdr {}: {err}", args.join(" ")))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let value: Option<Value> = serde_json::from_str(text.trim()).ok();
    if !out.status.success() {
        let errors: Option<Value> =
            serde_json::from_str(String::from_utf8_lossy(&out.stderr).trim()).ok();
        let why = value
            .as_ref()
            .or(errors.as_ref())
            .and_then(|value| value.pointer("/error/message"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "{} {}",
                    text.trim(),
                    String::from_utf8_lossy(&out.stderr).trim()
                )
            });
        return Err(why);
    }
    value.ok_or_else(|| format!("herdr {}: not JSON: {}", args.join(" "), text.trim()))
}

fn herdr_ok(args: &[&str]) -> Result<(), String> {
    let out = herdr_command(args)
        .output()
        .map_err(|err| format!("herdr {}: {err}", args.join(" ")))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "herdr {}: {} {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

pub(crate) fn herdr_text(args: &[&str]) -> Option<String> {
    let out = herdr_command(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn find_str<'a>(value: &'a Value, pointers: &[&str]) -> Option<&'a str> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
}

pub fn in_herdr() -> bool {
    std::env::var_os(crate::api::SOCKET_PATH_ENV_VAR).is_some()
        || std::env::var_os("HERDR_PANE_ID").is_some()
}

fn pane_env() -> Vec<String> {
    let mut env = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        let path = std::env::var("PATH").unwrap_or_default();
        let sep = if cfg!(windows) { ';' } else { ':' };
        env.push(format!("PATH={}{sep}{path}", dir.display()));
    }
    if let Ok(home) = std::env::var(crate::nirvana::HOME_ENV) {
        env.push(format!("{}={home}", crate::nirvana::HOME_ENV));
    }
    if let Ok(sock) = std::env::var(crate::api::SOCKET_PATH_ENV_VAR) {
        env.push(format!("{}={sock}", crate::api::SOCKET_PATH_ENV_VAR));
    }
    env
}

fn open_job_pane(cwd: &str, name: &str, id: &str) -> Result<(Option<String>, String), String> {
    let space_id = std::env::var("HERDR_WORKSPACE_ID").ok().or_else(|| {
        let ws_list = herdr_json(&["workspace", "list"]).ok()?;
        let workspaces = ws_list.pointer("/result/workspaces")?.as_array()?;
        workspaces
            .iter()
            .find(|ws| ws.get("focused") == Some(&Value::Bool(true)))
            .or_else(|| workspaces.first())
            .and_then(|ws| ws.get("workspace_id"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    });

    let env = pane_env();
    let mut env_args: Vec<&str> = Vec::new();
    for pair in &env {
        env_args.extend(["--env", pair.as_str()]);
    }

    // Check if Jobs tab exists
    let mut tab_list_args = vec!["tab", "list"];
    if let Some(space) = &space_id {
        tab_list_args.extend(["--workspace", space.as_str()]);
    }
    let tabs_val = herdr_json(&tab_list_args)?;
    let existing_jobs_tab = tabs_val
        .pointer("/result/tabs")
        .and_then(Value::as_array)
        .and_then(|tabs| {
            tabs.iter()
                .find(|tab| tab.get("label").and_then(Value::as_str) == Some("Jobs"))
        });

    let pane_id = if let Some(jobs_tab) = existing_jobs_tab {
        let tab_id = jobs_tab
            .get("tab_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing tab_id in tab list".to_string())?;

        // Find existing panes in this Jobs tab
        let mut pane_list_args = vec!["pane", "list"];
        if let Some(space) = &space_id {
            pane_list_args.extend(["--workspace", space.as_str()]);
        }
        let panes_val = herdr_json(&pane_list_args)?;
        let target_pane_id = panes_val
            .pointer("/result/panes")
            .and_then(Value::as_array)
            .and_then(|panes| {
                panes
                    .iter()
                    .rev()
                    .find(|p| p.get("tab_id").and_then(Value::as_str) == Some(tab_id))
                    .and_then(|p| p.get("pane_id").and_then(Value::as_str))
            });

        if let Some(target) = target_pane_id {
            let mut args = vec![
                "pane",
                "split",
                target,
                "--direction",
                "right",
                "--cwd",
                cwd,
                "--no-focus",
            ];
            args.extend(env_args.iter().copied());
            let value = herdr_json(&args)?;
            find_str(&value, &["/result/pane/pane_id", "/pane/pane_id"])
                .ok_or_else(|| "pane split gave no pane id".to_string())?
                .to_owned()
        } else {
            let mut args = vec![
                "tab",
                "create",
                "--label",
                "Jobs",
                "--cwd",
                cwd,
                "--no-focus",
            ];
            if let Some(space) = &space_id {
                args.extend(["--workspace", space.as_str()]);
            }
            args.extend(env_args.iter().copied());
            let value = herdr_json(&args)?;
            find_str(&value, &["/result/root_pane/pane_id", "/root_pane/pane_id"])
                .ok_or_else(|| "tab create gave no pane id".to_string())?
                .to_owned()
        }
    } else {
        let mut args = vec![
            "tab",
            "create",
            "--label",
            "Jobs",
            "--cwd",
            cwd,
            "--no-focus",
        ];
        if let Some(space) = &space_id {
            args.extend(["--workspace", space.as_str()]);
        }
        args.extend(env_args.iter().copied());
        let value = herdr_json(&args)?;
        find_str(&value, &["/result/root_pane/pane_id", "/root_pane/pane_id"])
            .ok_or_else(|| "tab create gave no pane id".to_string())?
            .to_owned()
    };

    let _ = herdr_ok(&["pane", "rename", &pane_id, &format!("job·{name}")]);

    // Wait for the shell prompt to be drawn
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if herdr_text(&["pane", "read", &pane_id, "--source", "visible"])
            .is_some_and(|screen| !screen.trim().is_empty())
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_millis(300));

    herdr_ok(&["pane", "run", &pane_id, &format!("herdr job exec {id}")])?;

    Ok((space_id, pane_id))
}

pub fn wait_job(home: &Path, id: &str) -> std::io::Result<i32> {
    loop {
        std::thread::sleep(Duration::from_millis(250));
        if let Ok(rec) = read_job(home, id) {
            let is_alive = rec.pid.is_some_and(is_process_alive);
            let state = rec.state(is_alive);
            if rec.ended.is_some() || rec.exit.is_some() || state == JobState::Lost {
                let lines = tail_job(home, id, 20).unwrap_or_default();
                for line in lines {
                    println!("{line}");
                }
                let code = rec
                    .exit
                    .unwrap_or(if state == JobState::Passed { 0 } else { 1 });
                return Ok(code);
            }
        }
    }
}

pub fn run_job(
    home: &Path,
    parsed: &RunArgs,
    in_herdr_mode: bool,
) -> std::io::Result<(String, Option<i32>)> {
    let id = generate_id();
    let cwd = parsed.cwd.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".into())
    });

    if !in_herdr_mode {
        // Outside herdr, with no server, run in place with the same log and record
        create_job(
            home,
            &id,
            &parsed.name,
            parsed.argv.clone(),
            &cwd,
            None,
            None,
            None,
        )?;
        println!("{id}");
        let code = run_exec(home, &id)?;
        return Ok((id, Some(code)));
    }

    create_job(
        home,
        &id,
        &parsed.name,
        parsed.argv.clone(),
        &cwd,
        None,
        None,
        None,
    )?;

    match open_job_pane(&cwd, &parsed.name, &id) {
        Ok((space_id, pane_id)) => {
            if let Ok(mut rec) = read_job(home, &id) {
                rec.space = space_id;
                rec.pane = Some(pane_id);
                let _ = write_job(home, &rec);
            }
            println!("{id}");
        }
        Err(why) => {
            eprintln!("herdr job: opening pane failed ({why}), running in place");
            println!("{id}");
            let code = run_exec(home, &id)?;
            return Ok((id, Some(code)));
        }
    }

    if parsed.wait {
        let code = wait_job(home, &id)?;
        return Ok((id, Some(code)));
    }

    Ok((id, None))
}

fn run_cli(args: &[String]) -> std::io::Result<()> {
    let parsed = match parse_run_args(args) {
        Ok(p) => p,
        Err(why) => {
            eprintln!("herdr job run: {why}");
            eprintln!("usage: herdr job run --name <name> [--cwd <dir>] [--wait] -- <command...>");
            std::process::exit(2);
        }
    };
    let home =
        crate::nirvana::home().ok_or_else(|| std::io::Error::other("no home folder found"))?;
    let in_herdr_mode = in_herdr() && herdr_json(&["tab", "list"]).is_ok();
    let (_id, exit_code) = run_job(&home, &parsed, in_herdr_mode)?;
    if let Some(code) = exit_code {
        std::process::exit(code);
    }
    Ok(())
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

    #[test]
    fn run_args_parser_handles_valid_flags_and_errors() {
        // Standard syntax: --name <name> -- <command...>
        let args = vec![
            "--name".into(),
            "my_job".into(),
            "--".into(),
            "python".into(),
            "-c".into(),
            "print(1)".into(),
        ];
        let parsed = parse_run_args(&args).unwrap();
        assert_eq!(parsed.name, "my_job");
        assert_eq!(parsed.cwd, None);
        assert!(!parsed.wait);
        assert_eq!(parsed.argv, vec!["python", "-c", "print(1)"]);

        // With --cwd and --wait
        let args2 = vec![
            "--name=demo_wait".into(),
            "--cwd=C:/temp".into(),
            "--wait".into(),
            "--".into(),
            "echo".into(),
            "hello".into(),
        ];
        let parsed2 = parse_run_args(&args2).unwrap();
        assert_eq!(parsed2.name, "demo_wait");
        assert_eq!(parsed2.cwd, Some("C:/temp".into()));
        assert!(parsed2.wait);
        assert_eq!(parsed2.argv, vec!["echo", "hello"]);

        // Missing --name
        let err1 = parse_run_args(&["--".into(), "echo".into()]);
        assert!(err1.is_err());

        // Missing command
        let err2 = parse_run_args(&["--name".into(), "job1".into()]);
        assert!(err2.is_err());
    }

    #[test]
    fn run_job_with_no_server_runs_in_place_and_records_job() {
        let guard = crate::nirvana::with_test_home("job_run_inplace");
        let parsed = RunArgs {
            name: "test_in_place".into(),
            cwd: None,
            wait: true,
            argv: vec![
                "python".into(),
                "-c".into(),
                "print('running in place'); print('done')".into(),
            ],
        };

        let (id, code) = run_job(&guard.path, &parsed, false).unwrap();
        assert_eq!(code, Some(0));

        let rec = read_job(&guard.path, &id).unwrap();
        assert!(rec.ended.is_some());
        assert_eq!(rec.exit, Some(0));
        assert_eq!(rec.state(false), JobState::Passed);

        let lines = tail_job(&guard.path, &id, 10).unwrap();
        assert!(lines.iter().any(|l| l.contains("running in place")));
        assert!(lines.iter().any(|l| l.contains("done")));
        assert!(lines
            .last()
            .unwrap()
            .starts_with("[job test_in_place: exit 0 after "));
    }

    #[test]
    fn run_job_with_no_server_records_nonzero_exit() {
        let guard = crate::nirvana::with_test_home("job_run_inplace_fail");
        let parsed = RunArgs {
            name: "test_in_place_fail".into(),
            cwd: None,
            wait: true,
            argv: vec![
                "python".into(),
                "-c".into(),
                "import sys; print('fail step'); sys.exit(42)".into(),
            ],
        };

        let (id, code) = run_job(&guard.path, &parsed, false).unwrap();
        assert_eq!(code, Some(42));

        let rec = read_job(&guard.path, &id).unwrap();
        assert!(rec.ended.is_some());
        assert_eq!(rec.exit, Some(42));
        assert_eq!(rec.state(false), JobState::Failed);

        let lines = tail_job(&guard.path, &id, 10).unwrap();
        assert!(lines.iter().any(|l| l.contains("fail step")));
        assert!(lines
            .last()
            .unwrap()
            .starts_with("[job test_in_place_fail: exit 42 after "));
    }

    #[test]
    fn claude_folder_encoding_replaces_non_alphanumeric_with_hyphens() {
        assert_eq!(
            encode_folder_str(r"C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-p15"),
            "C--Users-fahim-Desktop-Pojects-Nirvana-Herdr-p15"
        );
        assert_eq!(
            encode_folder_str("/home/user/my_project.1"),
            "-home-user-my-project-1"
        );
    }

    #[test]
    fn claude_task_state_and_exit_parsing_covers_running_passed_failed_unknown() {
        let now = 100_000u64;

        // 1. Running: recent modified, no exit line
        let running_content = "Building...\nRunning tests...";
        let (state, code, last) = parse_claude_task_content(running_content, now - 600, now);
        assert_eq!(state, ClaudeTaskState::Running);
        assert_eq!(code, None);
        assert_eq!(last, Some("Running tests...".into()));

        // 2. Passed: has [exited with code 0]
        let passed_content = "Running tests...\nok\n[exited with code 0]";
        let (state, code, last) = parse_claude_task_content(passed_content, now - 3600, now);
        assert_eq!(state, ClaudeTaskState::Passed);
        assert_eq!(code, Some(0));
        assert_eq!(last, Some("[exited with code 0]".into()));

        // 3. Failed: has [exited with code 1]
        let failed_content = "error: test failed\n[exited with code 1]\n";
        let (state, code, last) = parse_claude_task_content(failed_content, now - 7200, now);
        assert_eq!(state, ClaudeTaskState::Failed);
        assert_eq!(code, Some(1));
        assert_eq!(last, Some("[exited with code 1]".into()));

        // 4. Unknown: no exit line and modified over 24 hours ago
        let unknown_content = "Started build...\nStill building...";
        let (state, code, last) = parse_claude_task_content(unknown_content, now - 25 * 3600, now);
        assert_eq!(state, ClaudeTaskState::Unknown);
        assert_eq!(code, None);
        assert_eq!(last, Some("Still building...".into()));
    }

    #[test]
    fn claude_task_list_filters_twelve_hours_sorts_newest_first_and_limits_to_ten() {
        let guard = crate::nirvana::with_test_home("claude_task_list");
        let temp_dir = guard.path.join("temp");
        let space_folder = Path::new(r"C:\Projects\TestSpace");
        let encoded = encode_folder(space_folder);
        let session_tasks_dir = temp_dir
            .join("claude")
            .join(&encoded)
            .join("session-xyz")
            .join("tasks");
        std::fs::create_dir_all(&session_tasks_dir).unwrap();

        // Create 12 files within 12h and 2 files older than 12h
        // We write task-01 to task-12 inside the 12h window, and task-old1, task-old2 outside 12h.
        for i in 1..=12 {
            let file_path = session_tasks_dir.join(format!("task-{i:02}.output"));
            std::fs::write(
                &file_path,
                format!("output from task {i}\n[exited with code 0]"),
            )
            .unwrap();
        }
        let old_file = session_tasks_dir.join("task-old.output");
        std::fs::write(&old_file, "old output").unwrap();

        // When reading from list_claude_tasks_in, it checks filesystem mtime against `now`.
        // Since newly written files have mtime close to real system time, let's use the real system time as `now`.
        let real_now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let tasks = list_claude_tasks_in(&temp_dir, space_folder, real_now);
        // All 13 files are just created, so they are within 12h, but truncate(10) limits to 10!
        assert_eq!(tasks.len(), 10);

        // If now is shifted 13 hours into the future, all files should be excluded (>12h old)
        let future_now = real_now + 13 * 3600;
        let empty_tasks = list_claude_tasks_in(&temp_dir, space_folder, future_now);
        assert!(empty_tasks.is_empty());
    }
}
