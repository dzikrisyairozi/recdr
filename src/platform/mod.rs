//! Process and system calls that differ by operating system.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::path::PathBuf;

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

/// Default user data directory of Chrome Stable, where `--cdp chrome` looks for
/// `DevToolsActivePort`. None when the environment variable it starts from is unset.
#[cfg(windows)]
pub fn chrome_user_data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join(r"Google\Chrome\User Data"))
}

#[cfg(target_os = "macos")]
pub fn chrome_user_data_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/Google/Chrome"))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn chrome_user_data_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/google-chrome"))
}

/// When a process started, for `unix::process_started`: clock ticks since boot, field 22 of /proc/<pid>/stat.
#[cfg(target_os = "linux")]
fn start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name, field 2, is in parentheses and may hold spaces.
    let (_, after_name) = stat.rsplit_once(')')?;
    after_name.split_whitespace().nth(19)?.parse().ok()
}

/// The start time as `ps` prints it, folded into a number: only equality is used.
#[cfg(target_os = "macos")]
fn start_time(pid: u32) -> Option<u64> {
    let out = std::process::Command::new("ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| {
        text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
    })
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn start_time(_pid: u32) -> Option<u64> {
    None
}
