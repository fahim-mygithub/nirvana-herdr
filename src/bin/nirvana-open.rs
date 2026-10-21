#![windows_subsystem = "windows"]

//! Nirvana launcher: runs `herdr.exe --session nirvana nirvana open [<folder>]`
//! with `CREATE_NO_WINDOW` and exits, so Explorer shows no console (plan 21 §2).

use std::path::PathBuf;

fn herdr_exe() -> PathBuf {
    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            let candidate = parent.join(if cfg!(windows) { "herdr.exe" } else { "herdr" });
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from(if cfg!(windows) { "herdr.exe" } else { "herdr" })
}

fn build_command(herdr: &std::path::Path, folder: Option<&str>) -> std::process::Command {
    let mut cmd = std::process::Command::new(herdr);
    cmd.args(["--session", "nirvana", "nirvana", "open"]);
    if let Some(f) = folder {
        cmd.arg(f);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let herdr = herdr_exe();
    let folder = std::env::args().nth(1);
    let mut cmd = build_command(&herdr, folder.as_deref());
    cmd.spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nirvana_open_command_args() {
        let cmd = build_command(std::path::Path::new("herdr.exe"), Some("C:\\proj"));
        let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
        assert_eq!(
            args,
            &["--session", "nirvana", "nirvana", "open", "C:\\proj"]
        );
    }

    #[test]
    fn test_nirvana_open_command_no_folder() {
        let cmd = build_command(std::path::Path::new("herdr.exe"), None);
        let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
        assert_eq!(args, &["--session", "nirvana", "nirvana", "open"]);
    }
}
