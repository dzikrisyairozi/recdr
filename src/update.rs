//! `recdr update`: finds the newest release and installs it over this binary by running
//! that release's own install script, so the download, checksum and replace steps exist
//! only in `install.sh` and `install.ps1`.
//!
//! Path handling here is string work on purpose: it behaves the same on every system,
//! so this file needs no `cfg`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::model::{UpdateCheck, UpdateResult};

/// The Rust target triple this binary was built for, set by `build.rs`.
pub const TARGET: &str = env!("RECDR_TARGET");

pub const DEFAULT_RELEASE_BASE: &str = "https://github.com/dzikrisyairozi/recdr/releases";

/// How this copy of recdr was installed. Only `Script` installs are updated in place.
#[derive(Debug, PartialEq)]
pub enum Channel {
    Script,
    Homebrew,
    Cargo,
}

/// Compares this binary's version with the newest release. Changes nothing.
pub fn check() -> Result<UpdateCheck> {
    let latest = latest(&release_base())?;
    Ok(UpdateCheck {
        current: CURRENT.to_owned(),
        update_available: Some(latest) > parse_version(CURRENT),
        latest: format_version(latest),
    })
}

const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Installs the newest release over this binary with that release's install script.
/// Homebrew and cargo installs are refused: their own tool owns the file.
pub fn run() -> Result<UpdateResult> {
    let exe = plain(std::fs::canonicalize(std::env::current_exe()?)?);
    let cargo_bin = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".cargo")))
        .and_then(|home| std::fs::canonicalize(home.join("bin")).ok())
        .map(plain);
    match channel(&exe, cargo_bin.as_deref()) {
        Channel::Homebrew => {
            return Err(Error::ManagedInstall(
                "recdr was installed by Homebrew; run `brew upgrade recdr`".into(),
            ));
        }
        Channel::Cargo => {
            return Err(Error::ManagedInstall(
                "recdr was installed by cargo; run `cargo install recdr`".into(),
            ));
        }
        Channel::Script => {}
    }

    let base = release_base();
    let newest = latest(&base)?;
    let result = |updated| UpdateResult {
        current: CURRENT.to_owned(),
        latest: format_version(newest),
        updated,
        path: exe.clone(),
    };
    if Some(newest) <= parse_version(CURRENT) {
        return Ok(result(false));
    }

    let tmp = std::env::temp_dir().join(format!("recdr-update-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let outcome = install(&base, &format_version(newest), &exe, &tmp);
    let _ = std::fs::remove_dir_all(&tmp);
    outcome.map(|()| result(true))
}

fn install(base: &str, version: &str, exe: &Path, tmp: &Path) -> Result<()> {
    let script = script_name(TARGET);
    let path = tmp.join(script);
    curl(&[
        "-o",
        &path.to_string_lossy(),
        &format!("{base}/download/v{version}/{script}"),
    ])?;
    let (program, args): (&str, &[&str]) = if script == "install.ps1" {
        (
            "powershell",
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"],
        )
    } else {
        ("sh", &[])
    };
    // From pwsh 7, PSModulePath names its own modules, which Windows PowerShell 5.1
    // cannot load; without it 5.1 uses its default.
    let output = Command::new(program)
        .args(args)
        .arg(&path)
        .env_remove("PSModulePath")
        .env("RECDR_VERSION", version)
        .env("RECDR_INSTALL_DIR", exe.parent().unwrap_or(Path::new(".")))
        .env("RECDR_RELEASE_BASE", base)
        .output()
        .map_err(|error| Error::UpdateFailed(format!("could not run {program}: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let line = last_line(&output.stderr);
    Err(Error::UpdateFailed(if line.is_empty() {
        format!(
            "the install script exited with code {}",
            output.status.code().unwrap_or(-1)
        )
    } else {
        line
    }))
}

fn release_base() -> String {
    std::env::var("RECDR_RELEASE_BASE")
        .unwrap_or_else(|_| DEFAULT_RELEASE_BASE.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

/// The newest version, from `version.txt` of the latest release.
fn latest(base: &str) -> Result<(u64, u64, u64)> {
    let url = format!("{base}/latest/download/version.txt");
    let text = curl(&[&url])?;
    let text = String::from_utf8_lossy(&text);
    parse_version(text.lines().next().unwrap_or_default())
        .ok_or_else(|| Error::UpdateFailed(format!("{url} does not hold a version")))
}

/// Runs `curl -fsSL <args>`; the last argument is the URL.
fn curl(args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("curl")
        .arg("-fsSL")
        .args(args)
        .output()
        .map_err(|_| Error::UpdateFailed("curl was not found; rerun the install command".into()))?;
    if !output.status.success() {
        return Err(Error::UpdateFailed(format!(
            "curl {}: {}",
            args.last().unwrap_or(&""),
            last_line(&output.stderr)
        )));
    }
    Ok(output.stdout)
}

fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn format_version((major, minor, patch): (u64, u64, u64)) -> String {
    format!("{major}.{minor}.{patch}")
}

/// `0.1.0`, `v0.1.0` or `0.1.0\n` as three numbers; anything else is `None`.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim();
    let text = text.strip_prefix('v').unwrap_or(text);
    let mut parts = text.split('.').map(|part| part.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// `exe` and `cargo_bin` are canonical paths that went through `plain`.
pub fn channel(exe: &Path, cargo_bin: Option<&Path>) -> Channel {
    let exe = slashes(exe);
    if exe.contains("/Cellar/recdr/") {
        return Channel::Homebrew;
    }
    if let Some(bin) = cargo_bin {
        let bin = slashes(bin);
        if exe.starts_with(&format!("{}/", bin.trim_end_matches('/'))) {
            return Channel::Cargo;
        }
    }
    Channel::Script
}

/// Drops the `\\?\` that `canonicalize` puts before a drive letter on Windows, so the
/// path matches what users and the install script write. UNC paths keep it.
pub fn plain(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return PathBuf::from(rest);
        }
    }
    path
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn script_name(target: &str) -> &'static str {
    if target.contains("windows") {
        "install.ps1"
    } else {
        "install.sh"
    }
}

fn slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_parse_with_or_without_v() {
        assert_eq!(parse_version("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_version("v1.2.3\n"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("x"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(parse_version("0.10.0") > parse_version("0.9.9"));
        assert!(parse_version("1.0.0") > parse_version("0.99.99"));
    }

    #[test]
    fn channel_from_path() {
        let path = |s: &str| PathBuf::from(s);
        assert_eq!(
            channel(&path("/opt/homebrew/Cellar/recdr/0.1.0/bin/recdr"), None),
            Channel::Homebrew
        );
        assert_eq!(
            channel(
                &path(r"C:\Users\a\.cargo\bin\recdr.exe"),
                Some(&path(r"C:\Users\a\.cargo\bin"))
            ),
            Channel::Cargo
        );
        assert_eq!(
            channel(
                &path("/home/a/.cargo/bin/recdr"),
                Some(&path("/home/a/.cargo/bin"))
            ),
            Channel::Cargo
        );
        assert_eq!(
            channel(
                &path("/home/a/.cargo/binx/recdr"),
                Some(&path("/home/a/.cargo/bin"))
            ),
            Channel::Script
        );
        assert_eq!(
            channel(
                &path("/home/a/.local/bin/recdr"),
                Some(&path("/home/a/.cargo/bin"))
            ),
            Channel::Script
        );
        assert_eq!(
            channel(&path("/home/a/.local/bin/recdr"), None),
            Channel::Script
        );
    }

    #[test]
    fn plain_drops_the_verbatim_prefix_before_a_drive() {
        assert_eq!(
            plain(PathBuf::from(r"\\?\C:\x\recdr.exe")),
            PathBuf::from(r"C:\x\recdr.exe")
        );
        assert_eq!(
            plain(PathBuf::from(r"\\?\UNC\srv\x")),
            PathBuf::from(r"\\?\UNC\srv\x")
        );
        assert_eq!(
            plain(PathBuf::from("/usr/bin/recdr")),
            PathBuf::from("/usr/bin/recdr")
        );
    }

    #[test]
    fn script_per_target() {
        assert_eq!(script_name("x86_64-pc-windows-msvc"), "install.ps1");
        for target in [
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-musl",
        ] {
            assert_eq!(script_name(target), "install.sh");
        }
    }
}
