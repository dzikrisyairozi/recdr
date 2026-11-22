//! `recdr update` against a release folder served by `common::serve()`.

mod common;

use std::path::PathBuf;

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

fn set_latest(folder: &std::path::Path, version: &str) {
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

    let output = std::process::Command::new(&exe)
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
