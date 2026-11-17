mod common;

use std::process::Output;

use common::{json, out_dir, recdr};
use serde_json::Value;

fn record(fake: &str, name: &str, args: &[&str]) -> (Output, std::path::PathBuf) {
    let out = out_dir(name);
    let output = recdr()
        .env("RECDR_FAKE_SOURCE", fake)
        .arg("record")
        .args(args)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    (output, out)
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

fn error_code(output: &Output) -> (Option<i32>, String) {
    let code = json(&output.stderr)["error"]["code"]
        .as_str()
        .unwrap()
        .to_owned();
    (output.status.code(), code)
}

#[test]
fn moving_for_one_second() {
    let (output, out) = record(
        "moving",
        "moving-1s",
        &["--monitor", "0", "--duration", "1s"],
    );
    let result = ok(&output);
    assert_eq!(result["stop_reason"], "duration_reached");
    assert_eq!(result["width"], 320);
    assert_eq!(result["height"], 240);
    let duration = result["duration_ms"].as_u64().unwrap();
    assert!((1000..=1500).contains(&duration), "{duration}");
    let stored = result["frames_stored"].as_u64().unwrap();
    assert!(
        (10..=17).contains(&stored),
        "{stored} frames: limiter not at 15 fps"
    );

    let manifest: Value =
        serde_json::from_slice(&std::fs::read(result["manifest"].as_str().unwrap()).unwrap())
            .unwrap();
    let frames = manifest["frames"].as_array().unwrap();
    assert_eq!(frames.len() as u64, stored);
    assert_eq!(frames[0]["t_ms"], 0);
    let dir = std::path::Path::new(result["dir"].as_str().unwrap());
    for frame in frames {
        assert!(dir.join(frame["file"].as_str().unwrap()).is_file());
    }
    assert_eq!(
        std::fs::read_to_string(out.join(".gitignore")).unwrap(),
        "*\n"
    );
}

#[test]
fn static_screen_stores_one_frame() {
    let (output, _) = record(
        "static",
        "static-1s",
        &["--monitor", "0", "--duration", "1s"],
    );
    assert_eq!(ok(&output)["frames_stored"], 1);
}

#[test]
fn closing_target_ends_with_window_closed() {
    let (output, _) = record(
        "closing",
        "closing",
        &["--monitor", "0", "--duration", "5s"],
    );
    let result = ok(&output);
    assert_eq!(result["stop_reason"], "window_closed");
    assert!(result["duration_ms"].as_u64().unwrap() < 2000);
}

#[test]
fn usage_errors() {
    let cases: &[&[&str]] = &[
        &["--monitor", "0", "--duration", "5s", "--max-duration", "1s"],
        &["--monitor", "0"],
        &["--monitor", "0", "--window", "x", "--duration", "1s"],
        &["--duration", "1s"],
        &["--monitor", "0", "--duration", "1s", "--fps", "0"],
        &["--monitor", "0", "--duration", "1s", "--fps", "61"],
        &["--monitor", "0", "--duration", "1s", "--format", "webm"],
        &["--monitor", "0", "--duration", "1s", "--max-size", "2"],
        &["--region", "1,2,3", "--duration", "1s"],
    ];
    for (i, args) in cases.iter().enumerate() {
        let (output, _) = record("moving", &format!("usage-{i}"), args);
        assert_eq!(error_code(&output), (Some(2), "usage".into()), "{args:?}");
    }
}

#[test]
fn unknown_fake_source_is_usage() {
    let (output, _) = record(
        "nonsense",
        "nonsense",
        &["--monitor", "0", "--duration", "1s"],
    );
    assert_eq!(error_code(&output), (Some(2), "usage".into()));
}

#[test]
fn broken_source_is_capture_failed() {
    let (output, _) = record("broken", "broken", &["--monitor", "0", "--duration", "1s"]);
    assert_eq!(error_code(&output), (Some(1), "capture_failed".into()));
}

#[test]
fn output_root_that_is_a_file_is_io_error() {
    let out = out_dir("io-error");
    let file = out.join("not-a-dir");
    std::fs::write(&file, "x").unwrap();
    let output = recdr()
        .env("RECDR_FAKE_SOURCE", "moving")
        .args(["record", "--monitor", "0", "--duration", "1s", "--out"])
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(error_code(&output), (Some(1), "io_error".into()));
}

#[test]
fn three_seconds_make_a_looping_gif_of_the_same_length() {
    let (output, _) = record("moving", "gif-3s", &["--monitor", "0", "--duration", "3s"]);
    let result = ok(&output);
    let path = result["gif"].as_str().expect("gif in result");
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options
        .read_info(std::fs::File::open(path).unwrap())
        .unwrap();
    assert_eq!((decoder.width(), decoder.height()), (320, 240));
    let mut hundredths = 0u64;
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        hundredths += u64::from(frame.delay);
    }
    assert!((250..=350).contains(&hundredths), "{hundredths} hundredths");
}

#[test]
fn moving_gets_keyframes_a_diff_and_a_contact_sheet() {
    let (output, _) = record(
        "moving",
        "analyze-2s",
        &["--monitor", "0", "--duration", "2s"],
    );
    let result = ok(&output);
    let diff: Value =
        serde_json::from_slice(&std::fs::read(result["diff"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(
        diff["frames"].as_array().unwrap().len() as u64,
        result["frames_stored"].as_u64().unwrap()
    );
    let keyframes = result["keyframes"].as_array().unwrap();
    assert!(keyframes.len() >= 2, "{keyframes:?}");
    for keyframe in keyframes {
        assert!(std::path::Path::new(keyframe["path"].as_str().unwrap()).is_file());
    }
    assert!(std::path::Path::new(result["contact_sheet"].as_str().unwrap()).is_file());
}

#[test]
fn static_screen_has_one_keyframe() {
    let (output, _) = record(
        "static",
        "static-keyframes",
        &["--monitor", "0", "--duration", "1s"],
    );
    assert_eq!(ok(&output)["keyframes"].as_array().unwrap().len(), 1);
}
