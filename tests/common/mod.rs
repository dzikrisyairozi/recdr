#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

/// The built binary, with the synthetic-source switch removed so each test opts in.
pub fn recdr() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_recdr"));
    command.env_remove("RECDR_FAKE_SOURCE");
    command
}

/// A fresh, empty directory for one test's recordings.
pub fn out_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("recdr-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test output directory");
    dir
}

pub fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(bytes)))
}
