//! Checks that need a real desktop session. Run with `cargo test --test desktop -- --ignored`.

mod common;

use common::{json, out_dir, recdr, test_window, wait_for_window};
use serde_json::Value;

#[test]
#[ignore]
fn monitors_lists_index_zero_first() {
    let output = recdr().arg("monitors").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let monitors = json(&output.stdout)["monitors"].clone();
    assert!(!monitors.as_array().unwrap().is_empty());
    assert_eq!(monitors[0]["index"], 0);
}

#[test]
#[ignore]
fn windows_prints_a_list() {
    let output = recdr().arg("windows").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(json(&output.stdout)["windows"].is_array());
}

fn record(name: &str, args: &[&str]) -> std::process::Output {
    recdr()
        .arg("record")
        .args(args)
        .arg("--out")
        .arg(out_dir(name))
        .output()
        .unwrap()
}

fn ok(output: &std::process::Output) -> Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    json(&output.stdout)
}

fn error(output: &std::process::Output) -> (Option<i32>, Value) {
    (output.status.code(), json(&output.stderr)["error"].clone())
}

fn title(name: &str) -> String {
    format!("recdr-test-{}-{name}", std::process::id())
}

#[test]
#[ignore]
fn monitor_recording_has_the_monitor_size() {
    let monitors = json(&recdr().arg("monitors").output().unwrap().stdout);
    let bounds = &monitors["monitors"][0]["bounds"];
    let result = ok(&record("monitor", &["--monitor", "0", "--duration", "2s"]));
    assert_eq!(result["width"], bounds["width"]);
    assert_eq!(result["height"], bounds["height"]);
    assert!(result["frames_stored"].as_u64().unwrap() >= 1);
}

#[test]
#[ignore]
fn region_recording_is_cropped() {
    let result = ok(&record(
        "region",
        &["--region", "0,0,640,480", "--duration", "2s"],
    ));
    assert_eq!(result["width"], 640);
    assert_eq!(result["height"], 480);
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(result["manifest"].as_str().unwrap()).unwrap())
            .unwrap();
    let first = std::path::Path::new(result["dir"].as_str().unwrap())
        .join(manifest["frames"][0]["file"].as_str().unwrap());
    let png = std::fs::read(first).unwrap();
    // PNG IHDR: width and height are big-endian u32 at bytes 16..24.
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    assert_eq!((width, height), (640, 480));
}

#[test]
#[ignore]
fn window_recording_is_about_the_window_size() {
    let title = title("capture");
    let mut child = test_window(&title, false);
    let window = wait_for_window(&title);
    let output = record("window", &["--window", &title, "--duration", "2s"]);
    let _ = child.kill();
    let _ = child.wait();
    let result = ok(&output);
    assert!(result["frames_stored"].as_u64().unwrap() >= 1);
    for side in ["width", "height"] {
        let got = result[side].as_i64().unwrap();
        let listed = window["bounds"][side].as_i64().unwrap();
        assert!((got - listed).abs() <= 40, "{side}: {got} vs {listed}");
    }
}

#[test]
#[ignore]
fn region_off_every_monitor_is_out_of_bounds() {
    let (code, error) = error(&record(
        "oob",
        &["--region", "0,0,99999,99999", "--duration", "1s"],
    ));
    assert_eq!(
        (code, error["code"].as_str()),
        (Some(1), Some("region_out_of_bounds"))
    );
}

#[test]
#[ignore]
fn unknown_window_is_not_found() {
    let (code, error) = error(&record(
        "missing",
        &["--window", "zzz-no-such-window", "--duration", "1s"],
    ));
    assert_eq!(
        (code, error["code"].as_str()),
        (Some(1), Some("target_not_found"))
    );
}

#[test]
#[ignore]
fn two_matching_windows_are_ambiguous() {
    let (a, b) = (title("dup-a"), title("dup-b"));
    let mut children = [test_window(&a, false), test_window(&b, false)];
    wait_for_window(&a);
    wait_for_window(&b);
    let output = record("dup", &["--window", &title("dup"), "--duration", "1s"]);
    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }
    let (code, error) = error(&output);
    assert_eq!(
        (code, error["code"].as_str()),
        (Some(1), Some("ambiguous_target"))
    );
    assert_eq!(error["candidates"].as_array().unwrap().len(), 2);
}
