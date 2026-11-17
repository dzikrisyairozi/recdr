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

/// A WinForms window with this title, owned by a child PowerShell. Kill the child to close it.
pub fn test_window(title: &str, minimized: bool) -> std::process::Child {
    let state = if minimized {
        "$f.WindowState = 'Minimized'; "
    } else {
        ""
    };
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form; \
         $f.Text = '{title}'; $f.Width = 640; $f.Height = 480; {state}\
         [Windows.Forms.Application]::Run($f)"
    );
    Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .spawn()
        .expect("start powershell")
}

/// Polls `recdr windows` until a window with exactly this title shows up.
pub fn wait_for_window(title: &str) -> serde_json::Value {
    for _ in 0..50 {
        let output = recdr().arg("windows").output().unwrap();
        let list = json(&output.stdout);
        if let Some(window) = list["windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["title"] == title)
        {
            return window.clone();
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    panic!("window {title:?} did not appear within 10 s");
}
