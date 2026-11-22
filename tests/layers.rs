//! Enforces the layer rules in the Project structure section of
//! spec/3-cdp-capture/spec.md. Comment lines are ignored; everything else is matched
//! as plain substrings.

use std::path::{Path, PathBuf};

fn source_files() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    walk(&src, &mut paths);
    paths
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let code = std::fs::read_to_string(&path)
                .unwrap()
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            (relative, code)
        })
        .collect()
}

fn forbid(needles: &[&str], allowed: &[&str]) {
    for (path, code) in source_files() {
        if allowed.contains(&path.as_str()) {
            continue;
        }
        for needle in needles {
            assert!(!code.contains(needle), "{path} contains {needle:?}");
        }
    }
}

#[test]
fn rule_1_only_store_names_recording_files() {
    forbid(
        &[
            "\"state.json\"",
            "\"manifest.json\"",
            "\"result.json\"",
            "\"diff.json\"",
            "\"marks.jsonl\"",
            "\"recorder.log\"",
            "\"contact_sheet.png\"",
            "\"recording.gif\"",
            "\"recording.mp4\"",
        ],
        &["src/store.rs"],
    );
}

#[test]
fn rule_2_each_front_end_library_stays_in_its_front_end() {
    forbid(&["clap"], &["src/cli.rs"]);
    forbid(&["rmcp", "tokio"], &["src/mcp.rs"]);
    forbid(&["base64"], &["src/mcp.rs", "src/capture/cdp.rs"]);
    forbid(&["tungstenite"], &["src/capture/cdp.rs"]);
    forbid(
        &["windows_capture"],
        &["src/capture/windows.rs", "src/encode/mf.rs"],
    );
    forbid(
        &[
            "objc2_av_foundation",
            "objc2_core_video",
            "objc2_core_media",
        ],
        &["src/encode/avf.rs"],
    );
    forbid(
        &["eframe", "egui", "rfd::"],
        &["src/gui/app.rs", "src/gui/overlay.rs", "src/gui/editor.rs"],
    );
    forbid(&["screencapturekit"], &["src/capture/macos.rs"]);
    forbid(
        &["objc2_core_graphics"],
        &["src/capture/macos.rs", "src/capture/tap.rs"],
    );
    forbid(
        &["objc2_core_foundation"],
        &[
            "src/capture/macos.rs",
            "src/capture/tap.rs",
            "src/encode/avf.rs",
        ],
    );
}

#[test]
fn rule_3_unsafe_only_in_seven_files() {
    forbid(
        &["unsafe"],
        &[
            "src/capture/windows.rs",
            "src/capture/hook.rs",
            "src/platform/windows.rs",
            "src/platform/unix.rs",
            "src/encode/avf.rs",
            "src/capture/macos.rs",
            "src/capture/tap.rs",
        ],
    );
}

#[test]
fn rule_4_front_ends_import_only_control_request_model_error() {
    let below = [
        "recorder::",
        "update::",
        "capture::",
        "store::",
        "analyze::",
        "encode::",
        "draw::",
        "platform::",
        "time::",
        "crate::{",
    ];
    for (path, code) in source_files() {
        let extra = match path.as_str() {
            "src/cli.rs" => "mcp::",
            "src/mcp.rs" => "cli::",
            _ => continue,
        };
        for needle in below.iter().chain([&extra, &"gui::", &"export::"]) {
            assert!(!code.contains(needle), "{path} contains {needle:?}");
        }
    }
    // The app is a front end too: below it, it may name only these modules, by path,
    // so an alias or a grouped import cannot hide another one. Checking what follows
    // `crate::` leaves `std::time` alone.
    let gui_may_use = ["control", "error", "model", "request"];
    for (path, code) in source_files() {
        if !path.starts_with("src/gui/") {
            continue;
        }
        for (at, _) in code.match_indices("crate::") {
            let rest = &code[at + "crate::".len()..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            assert!(
                gui_may_use.contains(&name.as_str()),
                "{path} uses crate::{}",
                rest.chars().take(20).collect::<String>()
            );
        }
    }
}

#[test]
fn rule_5_model_has_no_functions() {
    for (path, code) in source_files() {
        if path == "src/model.rs" {
            assert!(!code.contains("fn "), "src/model.rs contains a function");
        }
    }
}

#[test]
fn rule_6_system_differences_only_in_three_files() {
    let allowed = [
        "src/capture/mod.rs",
        "src/platform/mod.rs",
        "src/encode/mod.rs",
        "src/gui/mod.rs",
    ];
    for (path, code) in source_files() {
        if allowed.contains(&path.as_str()) {
            continue;
        }
        let code = code.replace("cfg(test)", "");
        for needle in ["cfg(", "cfg!(", "cfg_attr("] {
            assert!(!code.contains(needle), "{path} contains {needle:?}");
        }
    }
}

#[test]
fn rule_7_system_encoders_do_not_name_ffmpeg() {
    for (path, code) in source_files() {
        if ["src/encode/mf.rs", "src/encode/avf.rs"].contains(&path.as_str()) {
            assert!(!code.contains("ffmpeg"), "{path} contains \"ffmpeg\"");
        }
    }
}
