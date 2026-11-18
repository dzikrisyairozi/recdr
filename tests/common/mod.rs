#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde_json::{Value, json};

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

/// `recdr mcp` on stdio, one JSON-RPC message per line.
pub struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: u64,
}

impl Server {
    /// `recdr mcp`, initialized. `fake` sets the synthetic source, or leaves the real one.
    pub fn start(fake: Option<&str>) -> Self {
        let mut command = recdr();
        if let Some(kind) = fake {
            command.env("RECDR_FAKE_SOURCE", kind);
        }
        let mut child = command
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        // A reader thread, so a silent server fails the test instead of hanging it.
        let (sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if sender.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let mut server = Self {
            child,
            stdin,
            lines,
            next_id: 1,
        };
        let init = server.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "recdr-test", "version": "0"}
            }),
        );
        assert_eq!(init["serverInfo"]["name"], "recdr", "{init}");
        server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        server
    }

    pub fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(30))
                .expect("no reply within 30 s");
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                return message["result"].clone();
            }
        }
    }

    pub fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": tool, "arguments": arguments}))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap().to_owned()
}

pub fn is_error(result: &Value) -> bool {
    result["isError"] == true
}
