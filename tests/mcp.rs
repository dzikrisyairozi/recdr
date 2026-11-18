//! `recdr mcp` over stdio, one JSON-RPC message per line, on the synthetic source.

mod common;

use common::{Server, is_error, out_dir, text};
use serde_json::{Value, json};

#[test]
fn lists_the_nine_tools() {
    let mut server = Server::start(Some("moving"));
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
            "list_tabs",
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
    let mut server = Server::start(Some("moving"));

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

    let many = server.call("get_frames", json!({"id": id, "every": "10ms", "out": out}));
    assert!(is_error(&many), "{many}");
    assert!(text(&many).contains("too_many_frames"), "{many}");

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
    let mut server = Server::start(Some("moving"));
    let result = server.call("record", json!({"monitor": 0, "out": out}));
    assert!(is_error(&result), "{result}");
    assert!(text(&result).contains("usage"), "{result}");
}

#[test]
fn list_tabs_on_a_closed_port_is_an_error() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut server = Server::start(Some("moving"));
    let result = server.call("list_tabs", json!({"cdp": port.to_string()}));
    assert!(is_error(&result), "{result}");
    assert!(text(&result).contains("cdp_connect_failed"), "{result}");
}
