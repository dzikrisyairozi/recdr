//! `recdr mcp` over stdio, one JSON-RPC message per line, on the synthetic source.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use common::{out_dir, recdr};
use serde_json::{Value, json};

struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: u64,
}

impl Server {
    fn start() -> Self {
        let mut child = recdr()
            .env("RECDR_FAKE_SOURCE", "moving")
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

    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
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

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": tool, "arguments": arguments}))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap().to_owned()
}

fn is_error(result: &Value) -> bool {
    result["isError"] == true
}

#[test]
fn lists_the_eight_tools() {
    let mut server = Server::start();
    let tools = server.request("tools/list", json!({}));
    let mut names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "get_frames",
            "get_status",
            "list_monitors",
            "list_windows",
            "mark",
            "record",
            "start_recording",
            "stop_recording"
        ]
    );
}

#[test]
fn record_then_get_frames() {
    let out = out_dir("mcp");
    let out = out.to_str().unwrap();
    let mut server = Server::start();

    let recorded = server.call(
        "record",
        json!({"monitor": 0, "duration": "1s", "out": out}),
    );
    assert!(!is_error(&recorded), "{recorded}");
    let result: Value = serde_json::from_str(&text(&recorded)).unwrap();
    assert_eq!(result["stop_reason"], "duration_reached");
    assert_eq!(recorded["content"][1]["type"], "image");
    assert_eq!(recorded["content"][1]["mimeType"], "image/png");
    let id = result["id"].as_str().unwrap();

    // A small max_width makes every frame a sample, so a late limit check would write them.
    let many = server.call(
        "get_frames",
        json!({"id": id, "every": "10ms", "max_width": 100, "out": out}),
    );
    assert!(is_error(&many), "{many}");
    assert!(text(&many).contains("too_many_frames"), "{many}");
    let samples = std::path::Path::new(out).join(id).join("samples");
    let written = std::fs::read_dir(&samples).map_or(0, |dir| dir.count());
    assert_eq!(written, 0, "samples written before the limit was checked");

    let one = server.call("get_frames", json!({"id": id, "at": ["500ms"], "out": out}));
    assert!(!is_error(&one), "{one}");
    let content = one["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image");
}

#[test]
fn record_without_duration_is_usage() {
    let out = out_dir("mcp-usage");
    let mut server = Server::start();
    let result = server.call("record", json!({"monitor": 0, "out": out}));
    assert!(is_error(&result), "{result}");
    assert!(text(&result).contains("usage"), "{result}");
}
