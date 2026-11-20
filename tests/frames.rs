mod common;

use std::path::PathBuf;
use std::process::Output;
use std::sync::OnceLock;

use common::{json, out_dir, recdr};
use serde_json::Value;

/// One 2 s `moving` recording shared by every test here: (out, result).
fn recording() -> &'static (PathBuf, Value) {
    static RECORDING: OnceLock<(PathBuf, Value)> = OnceLock::new();
    RECORDING.get_or_init(|| {
        let out = out_dir("frames");
        let output = recdr()
            .env("RECDR_FAKE_SOURCE", "moving")
            .args(["record", "--monitor", "0", "--duration", "2s", "--out"])
            .arg(&out)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        (out, json(&output.stdout))
    })
}

fn frames(args: &[&str]) -> Output {
    let (out, result) = recording();
    recdr()
        .arg("frames")
        .arg(result["id"].as_str().unwrap())
        .args(args)
        .arg("--out")
        .arg(out)
        .output()
        .unwrap()
}

fn listed(args: &[&str]) -> Vec<Value> {
    let output = frames(args);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    json(&output.stdout)["frames"].as_array().unwrap().clone()
}

fn error_code(output: &Output) -> (Option<i32>, String) {
    let code = json(&output.stderr)["error"]["code"]
        .as_str()
        .unwrap()
        .to_owned();
    (output.status.code(), code)
}

#[test]
fn at_one_time() {
    let list = listed(&["--at", "1s"]);
    assert_eq!(list.len(), 1);
    assert!(list[0]["t_ms"].as_u64().unwrap() <= 1000);
    assert!(PathBuf::from(list[0]["path"].as_str().unwrap()).is_file());
}

#[test]
fn at_two_times() {
    assert_eq!(listed(&["--at", "500ms,1s"]).len(), 2);
}

#[test]
fn every_half_second() {
    assert_eq!(listed(&["--every", "500ms"]).len(), 5);
}

#[test]
fn keyframes_match_the_result() {
    let expected = recording().1["keyframes"].as_array().unwrap().len();
    assert_eq!(listed(&["--keyframes"]).len(), expected);
}

#[test]
fn max_width_writes_a_sample() {
    let list = listed(&["--at", "1s", "--max-width", "160"]);
    let path = PathBuf::from(list[0]["path"].as_str().unwrap());
    assert!(path.parent().unwrap().ends_with("samples"), "{path:?}");
    let png = std::fs::read(&path).unwrap();
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 160);
}

#[test]
fn bad_selections_are_usage() {
    for args in [
        &["--at", "1s", "--keyframes"][..],
        &[][..],
        &["--every", "0ms"][..],
    ] {
        assert_eq!(
            error_code(&frames(args)),
            (Some(2), "usage".into()),
            "{args:?}"
        );
    }
}

#[test]
fn unknown_id_is_not_found() {
    let (out, _) = recording();
    let output = recdr()
        .args(["frames", "nope", "--keyframes", "--out"])
        .arg(out)
        .output()
        .unwrap();
    assert_eq!(error_code(&output), (Some(1), "recording_not_found".into()));
}
