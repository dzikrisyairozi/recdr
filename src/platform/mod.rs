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
