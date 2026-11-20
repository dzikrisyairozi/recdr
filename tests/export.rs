mod common;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Output;

use common::{json, out_dir, recdr};
use image::RgbaImage;
use serde_json::Value;

/// A 3 second recording of the synthetic moving source. Returns the output root and the id.
fn recording(name: &str) -> (PathBuf, String) {
    let out = out_dir(name);
    let output = recdr()
        .env("RECDR_FAKE_SOURCE", "moving")
        .args(["record", "--monitor", "0", "--duration", "3s", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    let result = ok(&output);
    (out, result["id"].as_str().unwrap().to_owned())
}

fn export(out: &Path, id: &str, args: &[&str]) -> Output {
    recdr()
        .args(["export", id, "--out"])
        .arg(out)
        .args(args)
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

fn error_code(output: &Output) -> String {
    json(&output.stderr)["error"]["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

struct GifFacts {
    width: u32,
    height: u32,
    duration_ms: u64,
    frames: usize,
    first: RgbaImage,
}

fn gif_facts(path: &Path) -> GifFacts {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(File::open(path).unwrap()).unwrap();
    let (width, height) = (u32::from(decoder.width()), u32::from(decoder.height()));
    let mut first = None;
    let (mut frames, mut duration_ms) = (0, 0);
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        if first.is_none() {
            assert_eq!(
                (u32::from(frame.width), u32::from(frame.height)),
                (width, height),
                "the first GIF frame covers the picture"
            );
            first = RgbaImage::from_raw(width, height, frame.buffer.to_vec());
        }
        frames += 1;
        duration_ms += u64::from(frame.delay) * 10;
    }
    GifFacts {
        width,
        height,
        duration_ms,
        frames,
        first: first.expect("a GIF with no frame"),
    }
}

/// The stored frame on screen at `at`, as `recdr frames --at` names it.
fn stored(out: &Path, id: &str, at: &str) -> RgbaImage {
    let output = recdr()
        .args(["frames", id, "--at", at, "--out"])
        .arg(out)
        .output()
        .unwrap();
    let path = ok(&output)["frames"][0]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    image::open(path).unwrap().to_rgba8()
}

/// True when every channel of every pixel of `a` is within 8 of the pixel of `b` at
/// (`dx` + x, `dy` + y).
fn close(a: &RgbaImage, b: &RgbaImage, dx: u32, dy: u32) -> bool {
    a.enumerate_pixels().all(|(x, y, p)| {
        let q = b.get_pixel(x + dx, y + dy);
        p.0.iter().zip(q.0).all(|(&c, d)| c.abs_diff(d) <= 8)
    })
}

#[test]
fn export_whole() {
    let (out, id) = recording("export-whole");
    let file = out.join("out.gif");
    let result = ok(&export(&out, &id, &["--file", file.to_str().unwrap()]));
    let facts = gif_facts(&file);
    assert_eq!((facts.width, facts.height), (320, 240));
    let duration_ms = result["duration_ms"].as_u64().unwrap();
    assert!((3000..=3500).contains(&duration_ms), "{duration_ms}");
    assert_eq!(gif_length(&facts, duration_ms), duration_ms);
    assert_eq!(Path::new(result["path"].as_str().unwrap()), file);
    assert_eq!(
        (result["width"].as_u64(), result["height"].as_u64()),
        (Some(320), Some(240))
    );
    assert_eq!(result["frames"].as_u64(), Some(facts.frames as u64));
}

/// The GIF's length, or `expected_ms` when the two differ by no more than the rounding
/// of each frame's delay to hundredths of a second (5 ms a frame).
fn gif_length(facts: &GifFacts, expected_ms: u64) -> u64 {
    if facts.duration_ms.abs_diff(expected_ms) <= 5 * facts.frames as u64 {
        expected_ms
    } else {
        facts.duration_ms
    }
}

#[test]
fn export_trimmed() {
    let (out, id) = recording("export-trimmed");
    let file = out.join("out.gif");
    let result = ok(&export(
        &out,
        &id,
        &[
            "--file",
            file.to_str().unwrap(),
            "--from",
            "1s",
            "--until",
            "2s",
        ],
    ));
    assert_eq!(result["duration_ms"], 1000);
    let facts = gif_facts(&file);
    assert_eq!(gif_length(&facts, 1000), 1000);
    assert!(
        (900..=1100).contains(&facts.duration_ms),
        "{}",
        facts.duration_ms
    );
    assert!(close(&facts.first, &stored(&out, &id, "1s"), 0, 0));
    assert!(!close(&facts.first, &stored(&out, &id, "0ms"), 0, 0));
}

#[test]
fn export_cropped() {
    let (out, id) = recording("export-cropped");
    let file = out.join("out.gif");
    ok(&export(
        &out,
        &id,
        &["--file", file.to_str().unwrap(), "--crop", "10,20,100,80"],
    ));
    let facts = gif_facts(&file);
    assert_eq!((facts.width, facts.height), (100, 80));
    assert!(close(&facts.first, &stored(&out, &id, "0ms"), 10, 20));
}

#[test]
fn export_mp4() {
    let (out, id) = recording("export-mp4");
    let file = out.join("out.mp4");
    let output = export(
        &out,
        &id,
        &[
            "--file",
            file.to_str().unwrap(),
            "--from",
            "1s",
            "--until",
            "2s",
        ],
    );
    if output.status.code() != Some(0) {
        let code = error_code(&output);
        let runner_without_encoder =
            code == "encode_failed" && cfg!(windows) && std::env::var_os("CI").is_some();
        if code == "ffmpeg_not_found" || runner_without_encoder {
            println!("skipped: {code}");
            return;
        }
    }
    ok(&output);
    let facts = common::mp4::facts(&file);
    assert_eq!(facts.codec, "avc1");
    assert!(
        (900..=1100).contains(&facts.duration_ms),
        "{}",
        facts.duration_ms
    );
}

#[test]
fn export_errors() {
    let (out, id) = recording("export-errors");
    let file = out.join("target.gif");
    let file = file.to_str().unwrap();
    let cases: &[(&[&str], &str)] = &[
        (&["--file", file, "--from", "2s", "--until", "1s"], "usage"),
        (&["--file", file, "--until", "10s"], "usage"),
        (&["--file", file, "--crop", "300,200,100,80"], "usage"),
        (&["--file", file, "--crop", "0,0,1,1"], "usage"),
    ];
    for (args, code) in cases {
        let output = export(&out, &id, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert_eq!(error_code(&output), *code, "{args:?}");
    }
    let webm = out.join("target.webm");
    let output = export(&out, &id, &["--file", webm.to_str().unwrap()]);
    assert_eq!(error_code(&output), "usage");
    assert!(!webm.exists());

    let output = export(&out, "nope", &["--file", file]);
    assert_eq!(error_code(&output), "recording_not_found");

    let missing = out.join("no-such-dir").join("target.gif");
    let output = export(&out, &id, &["--file", missing.to_str().unwrap()]);
    assert_eq!(error_code(&output), "encode_failed");
    assert!(!missing.exists());

    assert!(!Path::new(file).exists());
    for entry in std::fs::read_dir(&out).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.contains(".part."), "{name}");
    }
}

/// The recording's own files are recdr's; an export may not write or replace any.
#[test]
fn export_into_the_recording_directory_is_usage() {
    let (out, id) = recording("export-inside");
    let before = snapshot(&out.join(&id));
    for name in ["recording.gif", "frames/new.gif", "./recording.gif"] {
        let file = out.join(&id).join(name);
        let output = export(&out, &id, &["--file", file.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(2), "{name}");
        assert_eq!(error_code(&output), "usage", "{name}");
    }
    assert!(before == snapshot(&out.join(&id)));
}

#[test]
fn export_of_a_running_recording_is_not_found() {
    let out = out_dir("export-running");
    let output = recdr()
        .env("RECDR_FAKE_SOURCE", "moving")
        .args(["start", "--monitor", "0", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    let id = ok(&output)["id"].as_str().unwrap().to_owned();
    let file = out.join("running.gif");
    let exported = export(&out, &id, &["--file", file.to_str().unwrap()]);
    let stopped = recdr()
        .args(["stop", &id, "--out"])
        .arg(&out)
        .output()
        .unwrap();
    ok(&stopped);
    assert_eq!(error_code(&exported), "recording_not_found");
    assert!(!file.exists());
}

/// Name, size and bytes of every file under `dir`.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push((path.clone(), std::fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn export_leaves_the_recording_alone() {
    let (out, id) = recording("export-alone");
    let before = snapshot(&out.join(&id));
    let file = out.join("out.gif");
    ok(&export(&out, &id, &["--file", file.to_str().unwrap()]));
    assert!(before == snapshot(&out.join(&id)));
}

#[test]
fn export_replaces_only_on_success() {
    let (out, id) = recording("export-replace");
    let file = out.join("out.gif");
    std::fs::write(&file, b"old").unwrap();
    ok(&export(&out, &id, &["--file", file.to_str().unwrap()]));
    let written = std::fs::read(&file).unwrap();
    assert_ne!(written, b"old");

    let failed = export(
        &out,
        &id,
        &["--file", file.to_str().unwrap(), "--until", "10s"],
    );
    assert_eq!(error_code(&failed), "usage");
    assert_eq!(std::fs::read(&file).unwrap(), written);

    // An encoder that fails part way: a stored frame is gone.
    let frames = out.join(&id).join("frames");
    let last = std::fs::read_dir(&frames)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .max()
        .unwrap();
    std::fs::remove_file(last).unwrap();
    let failed = export(&out, &id, &["--file", file.to_str().unwrap()]);
    assert_eq!(error_code(&failed), "encode_failed");
    assert_eq!(std::fs::read(&file).unwrap(), written);
    for entry in std::fs::read_dir(&out).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.contains(".part."), "{name} is left behind");
    }
}
