//! `start`, `stop`, `status` and `mark` against detached recorders on the synthetic source.

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;
use std::thread::sleep;
use std::time::{Duration, Instant};

use common::{json, out_dir, recdr};
use serde_json::Value;

fn run(fake: &str, out: &Path, args: &[&str]) -> Output {
    recdr()
        .env("RECDR_FAKE_SOURCE", fake)
        .args(args)
        .arg("--out")
        .arg(out)
        .output()
        .unwrap()
}

fn ok(output: &Output) -> Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    json(&output.stdout)
}

fn error(output: &Output) -> (Option<i32>, Value) {
    (output.status.code(), json(&output.stderr)["error"].clone())
}

fn start(fake: &str, out: &Path, extra: &[&str]) -> String {
    let mut args = vec!["start", "--monitor", "0"];
    args.extend_from_slice(extra);
    ok(&run(fake, out, &args))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn status_of(out: &Path, id: &str) -> String {
    let list = ok(&run("moving", out, &["status", id]));
    list["recordings"][0]["status"].as_str().unwrap().to_owned()
}

fn recorder_pid(out: &Path, id: &str) -> u32 {
    let state: Value =
        serde_json::from_slice(&std::fs::read(out.join(id).join("state.json")).unwrap()).unwrap();
    state["pid"].as_u64().unwrap() as u32
}

#[cfg(windows)]
fn pid_listed(pid: u32) -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
}

#[cfg(unix)]
fn pid_listed(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .unwrap()
        .status
        .success()
}

#[cfg(windows)]
fn kill(pid: u32) -> Output {
    std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output()
        .unwrap()
}

#[cfg(unix)]
fn kill(pid: u32) -> Output {
    std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .unwrap()
}

#[test]
fn start_returns_quickly_and_stop_ends_the_recorder() {
    let out = out_dir("start-stop");
    let begun = Instant::now();
    let started = ok(&run("moving", &out, &["start", "--monitor", "0"]));
    assert!(
        begun.elapsed() < Duration::from_secs(2),
        "{:?}",
        begun.elapsed()
    );
    assert_eq!(
        (started["width"].as_u64(), started["height"].as_u64()),
        (Some(320), Some(240))
    );
    assert!(PathBuf::from(started["dir"].as_str().unwrap()).is_dir());
    let id = started["id"].as_str().unwrap();
    assert_eq!(status_of(&out, id), "recording");
    let pid = recorder_pid(&out, id);

    let result = ok(&run("moving", &out, &["stop", id]));
    assert_eq!(result["stop_reason"], "stopped");
    assert_eq!(status_of(&out, id), "finished");
    let deadline = Instant::now() + Duration::from_secs(2);
    while pid_listed(pid) {
        assert!(Instant::now() < deadline, "recorder {pid} still running");
        sleep(Duration::from_millis(100));
    }

    assert_eq!(ok(&run("moving", &out, &["stop", id])), result);
}

/// The id of the only recording directory under `out`.
fn only_recording(out: &Path) -> String {
    let ids: Vec<String> = std::fs::read_dir(out)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(ids.len(), 1, "{ids:?}");
    ids[0].clone()
}

#[test]
fn broken_source_fails_to_start_with_its_own_code() {
    let out = out_dir("start-broken");
    let started = run("broken", &out, &["start", "--monitor", "0"]);
    let (code, error) = error(&started);
    assert_eq!(
        (code, error["code"].as_str()),
        (Some(1), Some("capture_failed"))
    );
    let message = error["message"].as_str().unwrap();
    assert!(
        message.contains("the synthetic source is broken"),
        "{message}"
    );

    let recorded = run(
        "broken",
        &out_dir("record-broken-compare"),
        &["record", "--monitor", "0", "--duration", "1s"],
    );
    assert_eq!(json(&started.stderr), json(&recorded.stderr));

    let begun = Instant::now();
    assert_eq!(status_of(&out, &only_recording(&out)), "crashed");
    assert!(begun.elapsed() < Duration::from_secs(2));
}

#[test]
fn failed_record_is_crashed_at_once() {
    let out = out_dir("record-broken");
    let (code, _) = error(&run(
        "broken",
        &out,
        &["record", "--monitor", "0", "--duration", "1s"],
    ));
    assert_eq!(code, Some(1));
    assert_eq!(status_of(&out, &only_recording(&out)), "crashed");
}

#[test]
fn time_cap_finishes_a_detached_recording() {
    let out = out_dir("start-cap");
    let id = start("moving", &out, &["--max-duration", "1s"]);
    sleep(Duration::from_secs(3));
    assert_eq!(status_of(&out, &id), "finished");
    assert_eq!(
        ok(&run("moving", &out, &["stop", &id]))["stop_reason"],
        "cap_reached"
    );
}

#[test]
fn killed_recorder_is_crashed() {
    let out = out_dir("start-crash");
    let id = start("moving", &out, &[]);
    let pid = recorder_pid(&out, &id);
    let killed = kill(pid);
    assert!(killed.status.success(), "{killed:?}");
    let (code, error) = error(&run("moving", &out, &["stop", &id]));
    assert_eq!(
        (code, error["code"].as_str()),
        (Some(1), Some("recorder_crashed"))
    );
    assert_eq!(status_of(&out, &id), "crashed");
}

#[test]
fn stop_unknown_id_is_not_found() {
    let out = out_dir("stop-missing");
    let (code, error) = error(&run("moving", &out, &["stop", "no-such-id"]));
    assert_eq!(
        (code, error["code"].as_str()),
        (Some(1), Some("recording_not_found"))
    );
}

#[test]
fn closing_target_ends_a_detached_recording() {
    let out = out_dir("start-closing");
    let id = start("closing", &out, &[]);
    sleep(Duration::from_secs(2));
    assert_eq!(
        ok(&run("closing", &out, &["stop", &id]))["stop_reason"],
        "window_closed"
    );
}

#[test]
fn status_without_id_lists_every_recording() {
    let out = out_dir("status-all");
    for _ in 0..2 {
        ok(&run(
            "moving",
            &out,
            &["record", "--monitor", "0", "--duration", "500ms"],
        ));
    }
    let list = ok(&run("moving", &out, &["status"]));
    assert_eq!(list["recordings"].as_array().unwrap().len(), 2);
}

#[test]
fn mark_lands_at_the_time_it_was_made() {
    let out = out_dir("mark");
    let id = start("moving", &out, &[]);
    sleep(Duration::from_millis(500));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let ack = ok(&run("moving", &out, &["mark", &id, "clicked submit"]));
    assert_eq!(ack["label"], "clicked submit");
    let result = ok(&run("moving", &out, &["stop", &id]));
    let marks = result["marks"].as_array().unwrap();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0]["label"], "clicked submit");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(result["manifest"].as_str().unwrap()).unwrap())
            .unwrap();
    let since_start = now - manifest["started_at_unix_ms"].as_u64().unwrap();
    let t_ms = marks[0]["t_ms"].as_u64().unwrap();
    assert!(
        (since_start..=since_start + 1000).contains(&t_ms),
        "{t_ms} not within 1 s after {since_start}"
    );

    let (code, late) = error(&run("moving", &out, &["mark", &id, "late"]));
    assert_eq!(
        (code, late["code"].as_str()),
        (Some(1), Some("recording_not_active"))
    );
    let (code, error) = error(&run("moving", &out, &["mark", "no-such-id", "x"]));
    assert_eq!(
        (code, error["code"].as_str()),
        (Some(1), Some("recording_not_found"))
    );
}
