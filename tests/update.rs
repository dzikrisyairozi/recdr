//! `recdr update` against a release folder served by `common::serve()`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{json, recdr, serve};

const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// An empty release folder for one test, and its release base URL.
fn release(name: &str) -> (String, PathBuf) {
    let folder_name = format!("recdr-release-{}-{name}", std::process::id());
    let folder = std::env::temp_dir().join(&folder_name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("latest/download")).unwrap();
    (
        format!("http://127.0.0.1:{}/fs/{folder_name}", serve()),
        folder,
    )
}

fn set_latest(folder: &Path, version: &str) {
    std::fs::write(
        folder.join("latest/download/version.txt"),
        format!("{version}\n"),
    )
    .unwrap();
}

/// A copy of the built binary in its own folder, so a test can replace or watch it.
fn binary_copy(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("recdr-bin-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join(
        PathBuf::from(env!("CARGO_BIN_EXE_recdr"))
            .file_name()
            .unwrap(),
    );
    std::fs::copy(env!("CARGO_BIN_EXE_recdr"), &exe).unwrap();
    exe
}

#[test]
fn check_reports_newer_version() {
    let (base, folder) = release("check-newer");
    set_latest(&folder, "9.9.9");
    let exe = binary_copy("check-newer");
    let before = std::fs::read(&exe).unwrap();

    let output = Command::new(&exe)
        .args(["update", "--check"])
        .env("RECDR_RELEASE_BASE", &base)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let result = json(&output.stdout);
    assert_eq!(result["current"], CURRENT);
    assert_eq!(result["latest"], "9.9.9");
    assert_eq!(result["update_available"], true);

    // --check changes nothing: the folder holds only the same binary.
    let files: Vec<_> = std::fs::read_dir(exe.parent().unwrap()).unwrap().collect();
    assert_eq!(files.len(), 1);
    assert_eq!(std::fs::read(&exe).unwrap(), before);
}

#[test]
fn check_reports_same_version() {
    let (base, folder) = release("check-same");
    set_latest(&folder, CURRENT);
    let output = recdr()
        .args(["update", "--check"])
        .env("RECDR_RELEASE_BASE", &base)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let result = json(&output.stdout);
    assert_eq!(result["latest"], CURRENT);
    assert_eq!(result["update_available"], false);
}

#[test]
fn missing_version_file_is_update_failed() {
    let (base, _folder) = release("check-missing");
    let output = recdr()
        .args(["update", "--check"])
        .env("RECDR_RELEASE_BASE", &base)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json(&output.stderr)["error"]["code"], "update_failed");
}

/// Publishes `version` into `folder` the way the release workflow does: the real install
/// script, an archive holding `payload` as the binary, its `.sha256`, and version.txt.
fn publish(folder: &Path, version: &str, payload: &[u8]) {
    let windows = cfg!(windows);
    let dl = folder.join(format!("download/v{version}"));
    std::fs::create_dir_all(&dl).unwrap();
    let script = recdr::update::script_name(recdr::update::TARGET);
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(script),
        dl.join(script),
    )
    .unwrap();

    let stage = folder.join(format!("stage-{version}"));
    std::fs::create_dir_all(&stage).unwrap();
    let bin = if windows { "recdr.exe" } else { "recdr" };
    std::fs::write(stage.join(bin), payload).unwrap();
    // The Linux test build is gnu; the script asks for the musl archive.
    let target = recdr::update::TARGET.replace("-gnu", "-musl");
    let archive = format!("recdr-{target}.{}", if windows { "zip" } else { "tar.gz" });
    let flags = if windows { "-a -cf" } else { "-czf" };
    let mut tar = Command::new("tar");
    tar.args(flags.split(' '))
        .arg(dl.join(&archive))
        .arg("-C")
        .arg(&stage)
        .arg(bin);
    assert!(tar.status().unwrap().success());

    let hash = sha256(&dl.join(&archive));
    std::fs::write(
        dl.join(format!("{archive}.sha256")),
        format!("{hash}  {archive}\n"),
    )
    .unwrap();
    set_latest(folder, version);
}

fn sha256(path: &Path) -> String {
    let output = if cfg!(windows) {
        Command::new("powershell")
            .env_remove("PSModulePath")
            .args(["-NoProfile", "-Command"])
            .arg(format!(
                "(Get-FileHash -Algorithm SHA256 '{}').Hash.ToLower()",
                path.display()
            ))
            .output()
    } else {
        Command::new("sha256sum").arg(path).output().or_else(|_| {
            Command::new("shasum")
                .args(["-a", "256"])
                .arg(path)
                .output()
        })
    }
    .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_else(|| panic!("no hash: {}", String::from_utf8_lossy(&output.stderr)))
        .to_owned()
}

fn archive_path(folder: &Path, version: &str) -> PathBuf {
    let target = recdr::update::TARGET.replace("-gnu", "-musl");
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    folder.join(format!("download/v{version}/recdr-{target}.{ext}"))
}

fn update(exe: &Path, base: &str) -> std::process::Output {
    Command::new(exe)
        .arg("update")
        .env("RECDR_RELEASE_BASE", base)
        .output()
        .unwrap()
}

fn plain_canonical(path: &Path) -> String {
    recdr::update::plain(std::fs::canonicalize(path).unwrap())
        .to_string_lossy()
        .into_owned()
}

#[test]
fn update_replaces_binary() {
    let (base, folder) = release("replace");
    publish(&folder, "9.9.9", b"first");
    let exe = binary_copy("replace");

    let output = update(&exe, &base);
    assert!(output.status.success(), "{output:?}");
    let result = json(&output.stdout);
    assert_eq!(result["updated"], true);
    assert_eq!(result["latest"], "9.9.9");
    assert_eq!(result["path"], plain_canonical(&exe));
    assert_eq!(std::fs::read(&exe).unwrap(), b"first");

    let dir = exe.parent().unwrap();
    if cfg!(windows) {
        // The running copy could not delete itself; the next install removes it.
        let old = dir.join("recdr.exe.old");
        assert!(old.exists());
        publish(&folder, "9.9.10", b"second");
        let status = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(folder.join("download/v9.9.10/install.ps1"))
            .env_remove("PSModulePath")
            .env("RECDR_INSTALL_DIR", dir)
            .env("RECDR_RELEASE_BASE", &base)
            .env_remove("RECDR_VERSION")
            .status()
            .unwrap();
        assert!(status.success());
        assert!(!old.exists());
        assert_eq!(std::fs::read(&exe).unwrap(), b"second");
    } else {
        assert!(!dir.join("recdr.old").exists());
    }
}

#[test]
fn update_same_version_changes_nothing() {
    let (base, folder) = release("same");
    publish(&folder, CURRENT, b"never installed");
    let exe = binary_copy("same");
    let before = std::fs::read(&exe).unwrap();

    let output = update(&exe, &base);
    assert!(output.status.success(), "{output:?}");
    let result = json(&output.stdout);
    assert_eq!(result["updated"], false);
    assert_eq!(result["latest"], CURRENT);
    assert_eq!(std::fs::read(&exe).unwrap(), before);
}

#[test]
fn checksum_mismatch_keeps_binary() {
    let (base, folder) = release("checksum");
    publish(&folder, "9.9.9", b"bad");
    let archive = archive_path(&folder, "9.9.9");
    let name = archive.file_name().unwrap().to_string_lossy().into_owned();
    std::fs::write(
        archive.with_file_name(format!("{name}.sha256")),
        format!("{}  {name}\n", "0".repeat(64)),
    )
    .unwrap();
    let exe = binary_copy("checksum");
    let before = std::fs::read(&exe).unwrap();

    let output = update(&exe, &base);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let error = json(&output.stderr);
    assert_eq!(error["error"]["code"], "update_failed");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("checksum"),
        "{error}"
    );
    assert_eq!(std::fs::read(&exe).unwrap(), before);
}

#[test]
fn homebrew_path_is_refused() {
    let (base, folder) = release("homebrew");
    publish(&folder, "9.9.9", b"never installed");
    let cellar = std::env::temp_dir()
        .join(format!("recdr-brew-{}", std::process::id()))
        .join("Cellar/recdr/0.1.0/bin");
    let _ = std::fs::remove_dir_all(&cellar);
    std::fs::create_dir_all(&cellar).unwrap();
    let exe = cellar.join(binary_copy("homebrew").file_name().unwrap());
    std::fs::copy(env!("CARGO_BIN_EXE_recdr"), &exe).unwrap();
    let before = std::fs::read(&exe).unwrap();

    let output = update(&exe, &base);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(json(&output.stderr)["error"]["code"], "managed_install");
    assert_eq!(std::fs::read(&exe).unwrap(), before);

    let check = Command::new(&exe)
        .args(["update", "--check"])
        .env("RECDR_RELEASE_BASE", &base)
        .output()
        .unwrap();
    assert!(check.status.success(), "{check:?}");
}

#[test]
fn cargo_path_is_refused() {
    let (base, folder) = release("cargo");
    publish(&folder, "9.9.9", b"never installed");
    let home = std::env::temp_dir().join(format!("recdr-cargo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("bin")).unwrap();
    let exe = home
        .join("bin")
        .join(binary_copy("cargo").file_name().unwrap());
    std::fs::copy(env!("CARGO_BIN_EXE_recdr"), &exe).unwrap();

    let output = Command::new(&exe)
        .arg("update")
        .env("RECDR_RELEASE_BASE", &base)
        .env("CARGO_HOME", &home)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(json(&output.stderr)["error"]["code"], "managed_install");
}
