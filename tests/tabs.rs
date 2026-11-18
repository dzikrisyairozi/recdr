//! `recdr tabs` and the tab flags, without a browser. Browser tests are in browser.rs.

mod common;

use std::process::Output;
use std::time::{Duration, Instant};

use common::{json, out_dir, recdr};
use serde_json::Value;

/// A local port nothing listens on.
fn closed_port() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port().to_string()
}

fn error(output: &Output) -> (Option<i32>, Value) {
    (output.status.code(), json(&output.stderr)["error"].clone())
}

#[test]
fn tabs_on_a_closed_port_fails_quickly() {
    let begun = Instant::now();
    let output = recdr()
        .args(["tabs", "--cdp", &closed_port()])
        .output()
        .unwrap();
    let (code, error) = error(&output);
    assert_eq!(
        (code, error["code"].as_str()),
        (Some(1), Some("cdp_connect_failed"))
    );
    assert!(
        begun.elapsed() < Duration::from_secs(6),
        "{:?}",
        begun.elapsed()
    );
}

#[test]
fn tabs_refuses_tls_and_a_missing_endpoint() {
    for args in [
        &["tabs", "--cdp", "https://example.test"][..],
        &["tabs", "--cdp", "wss://example.test"],
        &["tabs"],
    ] {
        let output = recdr().args(args).output().unwrap();
        let (code, error) = error(&output);
        assert_eq!(
            (code, error["code"].as_str()),
            (Some(2), Some("usage")),
            "{args:?}"
        );
    }
}

#[test]
fn tabs_chrome_without_remote_debugging_says_how_to_switch_it_on() {
    let dir = out_dir("tabs-chrome-empty");
    let output = recdr()
        .args(["tabs", "--cdp", "chrome"])
        .env("RECDR_CHROME_USER_DATA_DIR", &dir)
        .output()
        .unwrap();
    let (code, error) = error(&output);
    assert_eq!(
        (code, error["code"].as_str()),
        (Some(1), Some("cdp_connect_failed"))
    );
    let message = error["message"].as_str().unwrap();
    assert!(
        message.contains("chrome://inspect/#remote-debugging"),
        "{message}"
    );
}

#[cfg(not(windows))]
mod unsupported {
    use super::{json, recdr};

    #[test]
    fn desktop_commands_are_unsupported_off_windows() {
        for args in [
            &["windows"][..],
            &["monitors"],
            &["record", "--monitor", "0", "--duration", "1s"],
        ] {
            let output = recdr().args(args).output().unwrap();
            assert_eq!(output.status.code(), Some(1), "{args:?}");
            assert_eq!(
                json(&output.stderr)["error"]["code"],
                "unsupported_platform",
                "{args:?}"
            );
        }
    }
}
