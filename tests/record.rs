mod common;

use std::process::Output;

use common::{json, out_dir, recdr};
use serde_json::Value;

fn record(fake: &str, name: &str, args: &[&str]) -> (Output, std::path::PathBuf) {
    let out = out_dir(name);
    let output = recdr()
        .env("RECDR_FAKE_SOURCE", fake)
        .arg("record")
        .arg("--out")
        .arg(&out)
        .args(args)
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
    // On a loaded macOS runner the synthetic source's 33 ms sleep has woken only every
    // 160 ms, so one run in a few stored 9 frames. A slow limiter fails both runs; a
    // starved runner almost never does, so the recording gets a second try.
    let mut attempt = 0;
    let (result, out, manifest) = loop {
        attempt += 1;
        let (output, out) = record(
            "moving",
            &format!("moving-1s-{attempt}"),
            &["--monitor", "0", "--duration", "1s"],
        );
        let result = ok(&output);
        let manifest: Value =
            serde_json::from_slice(&std::fs::read(result["manifest"].as_str().unwrap()).unwrap())
                .unwrap();
        // The limiter's output is what was stored plus what the writer replaced while
        // it was busy (`dropped_frames`; the synthetic source drops none).
        let stored = result["frames_stored"].as_u64().unwrap();
        let passed = stored + result["dropped_frames"].as_u64().unwrap();
        let times: Vec<u64> = manifest["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["t_ms"].as_u64().unwrap())
            .collect();
        let message = format!(
            "{stored} stored and {passed} through the limiter: not at 15 fps; stored at {times:?} ms"
        );
        assert!(passed <= 17, "{message}");
        if passed >= 10 {
            break (result, out, manifest);
        }
        assert!(attempt < 2, "twice: {message}");
        eprintln!("attempt {attempt}: {message}");
    };
    assert_eq!(result["stop_reason"], "duration_reached");
    assert_eq!(result["width"], 320);
    assert_eq!(result["height"], 240);
    let duration = result["duration_ms"].as_u64().unwrap();
    assert!((1000..=1500).contains(&duration), "{duration}");
    let stored = result["frames_stored"].as_u64().unwrap();
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

/// Prints "hi", then waits about two seconds.
#[cfg(windows)]
const ECHO_THEN_WAIT: &[&str] = &["cmd", "/c", "echo hi & ping -n 3 127.0.0.1 >nul"];
#[cfg(unix)]
const ECHO_THEN_WAIT: &[&str] = &["sh", "-c", "echo hi; sleep 2"];

#[cfg(windows)]
const EXIT_3: &[&str] = &["cmd", "/c", "exit 3"];
#[cfg(unix)]
const EXIT_3: &[&str] = &["sh", "-c", "exit 3"];

/// Runs for about 30 seconds through a child process, so only a tree kill ends it.
#[cfg(windows)]
const LONG: &[&str] = &["ping", "-n", "30", "127.0.0.1"];
#[cfg(unix)]
const LONG: &[&str] = &["sh", "-c", "sleep 30 & wait"];

#[test]
fn wrapped_command_sets_the_length_and_keeps_stdout_clean() {
    let (output, _) = record(
        "moving",
        "wrap",
        &[&["--monitor", "0", "--"][..], ECHO_THEN_WAIT].concat(),
    );
    let result = ok(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(result["stop_reason"], "command_exited", "{stderr}");
    assert_eq!(result["command_exit_code"], 0, "{stderr}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("hi"));
    let duration = result["duration_ms"].as_u64().unwrap();
    assert!((1500..=4000).contains(&duration), "{duration}");
}

#[test]
fn wrapped_command_exit_code_is_reported_not_returned() {
    let (output, _) = record(
        "moving",
        "wrap-exit",
        &[&["--monitor", "0", "--"][..], EXIT_3].concat(),
    );
    assert_eq!(ok(&output)["command_exit_code"], 3);
}

#[test]
fn time_cap_kills_the_whole_command_tree() {
    let begun = std::time::Instant::now();
    let (output, _) = record(
        "moving",
        "wrap-cap",
        &[&["--monitor", "0", "--max-duration", "1s", "--"][..], LONG].concat(),
    );
    assert!(
        begun.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        begun.elapsed()
    );
    let result = ok(&output);
    assert_eq!(
        result["stop_reason"],
        "cap_reached",
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(result.get("command_exit_code").is_none());
}

#[test]
fn duration_and_command_together_is_usage() {
    let (output, _) = record(
        "moving",
        "wrap-usage",
        &[
            "--monitor",
            "0",
            "--duration",
            "1s",
            "--",
            "cmd",
            "/c",
            "echo",
        ],
    );
    assert_eq!(error_code(&output), (Some(2), "usage".into()));
}

fn tool_runs(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn mp4_is_h264_of_the_recording_length() {
    if !tool_runs("ffmpeg") || !tool_runs("ffprobe") {
        println!("skipped: ffmpeg or ffprobe not on PATH");
        return;
    }
    let (output, _) = record(
        "moving",
        "mp4",
        &["--monitor", "0", "--duration", "2s", "--format", "gif,mp4"],
    );
    let result = ok(&output);
    assert!(std::path::Path::new(result["gif"].as_str().unwrap()).is_file());
    let mp4 = result["mp4"].as_str().unwrap();
    let probe = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name:format=duration",
            "-of",
            "json",
            mp4,
        ])
        .output()
        .unwrap();
    let probe = json(&probe.stdout);
    assert_eq!(probe["streams"][0]["codec_name"], "h264");
    let seconds: f64 = probe["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let expected = result["duration_ms"].as_f64().unwrap() / 1000.0;
    assert!((seconds - expected).abs() <= 0.5, "{seconds} vs {expected}");
}

#[test]
fn mp4_without_ffmpeg_is_refused_before_anything_is_written() {
    let out = out_dir("no-ffmpeg");
    let empty = out_dir("no-ffmpeg-path");
    let output = recdr()
        .env("RECDR_FAKE_SOURCE", "moving")
        .env("PATH", &empty)
        .args(["record", "--out"])
        .arg(&out)
        .args(["--monitor", "0", "--duration", "1s", "--format", "mp4"])
        .output()
        .unwrap();
    assert_eq!(error_code(&output), (Some(1), "ffmpeg_not_found".into()));
    let made = std::fs::read_dir(&out)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().is_dir())
        .count();
    assert_eq!(made, 0);
}
