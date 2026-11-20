//! Checks that need a real desktop session. Run with `cargo test --test desktop -- --ignored`.
#![cfg(any(windows, target_os = "macos"))]
#![allow(dead_code)]

mod common;

use common::{json, out_dir, recdr};
use serde_json::Value;

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

#[cfg(windows)]
mod on_windows {
    use super::*;
    use common::{test_window, wait_for_window};

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

    /// Success criterion 17, through the CLI. A window that changes color every 50 ms
    /// makes each monitor frame a new stored PNG of several hundred KB, so 1 MB ends the
    /// recording long before --duration. A still desktop stores one frame (about 800 KB
    /// here) and never reaches the cap.
    #[test]
    #[ignore]
    fn size_cap_ends_a_recording() {
        let title = title("size-cap");
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form;              $f.Text = '{title}'; $f.Width = 640; $f.Height = 480; $f.TopMost = $true;              $t = New-Object Windows.Forms.Timer; $t.Interval = 50;              $t.Add_Tick({{ $f.BackColor = [Drawing.Color]::FromArgb((Get-Random 256), (Get-Random 256), (Get-Random 256)) }});              $t.Start(); [Windows.Forms.Application]::Run($f)"
        );
        let mut child = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .spawn()
            .unwrap();
        wait_for_window(&title);
        let args = ["--monitor", "0", "--max-size", "1MB", "--duration", "20s"];
        let output = record("size-cap", &args);
        let _ = child.kill();
        let _ = child.wait();
        let result = ok(&output);
        assert_eq!(result["stop_reason"], "size_cap_reached");
        assert!(result["duration_ms"].as_u64().unwrap() < 20_000);
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
        // The listed bounds are the visible frame, which is what the capture holds.
        for side in ["width", "height"] {
            assert_eq!(result[side], window["bounds"][side], "{side}");
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

    #[test]
    #[ignore]
    fn detached_recorder_captures_the_desktop() {
        let out = out_dir("detached");
        let started = recdr()
            .args(["start", "--monitor", "0", "--out"])
            .arg(&out)
            .output()
            .unwrap();
        let id = ok(&started)["id"].as_str().unwrap().to_owned();
        std::thread::sleep(std::time::Duration::from_secs(1));
        let stopped = recdr()
            .args(["stop", &id, "--out"])
            .arg(&out)
            .output()
            .unwrap();
        assert!(ok(&stopped)["frames_stored"].as_u64().unwrap() >= 1);
    }

    #[test]
    #[ignore]
    fn closing_the_window_ends_the_recording() {
        let title = title("close");
        let out = out_dir("close");
        let mut child = test_window(&title, false);
        wait_for_window(&title);
        let started = recdr()
            .args(["start", "--window", &title, "--out"])
            .arg(&out)
            .output()
            .unwrap();
        let id = ok(&started)["id"].as_str().unwrap().to_owned();
        std::thread::sleep(std::time::Duration::from_secs(1));
        let _ = child.kill();
        let _ = child.wait();
        std::thread::sleep(std::time::Duration::from_secs(2));
        let stopped = recdr()
            .args(["stop", &id, "--out"])
            .arg(&out)
            .output()
            .unwrap();
        let result = ok(&stopped);
        assert_eq!(result["stop_reason"], "window_closed");
        let gif = std::fs::File::open(result["gif"].as_str().unwrap()).unwrap();
        let mut decoder = gif::DecodeOptions::new().read_info(gif).unwrap();
        assert!(decoder.read_next_frame().unwrap().is_some());
    }

    #[test]
    #[ignore]
    fn minimized_window_is_refused() {
        let title = title("min");
        let mut child = test_window(&title, true);
        wait_for_window(&title);
        let output = record("min", &["--window", &title, "--duration", "1s"]);
        let _ = child.kill();
        let _ = child.wait();
        let (code, error) = error(&output);
        assert_eq!(
            (code, error["code"].as_str()),
            (Some(1), Some("target_minimized"))
        );
    }

    /// Success criterion 15. Play a full-screen video on monitor 0 while this runs.
    #[test]
    #[ignore]
    fn criterion_15_dropped_frames() {
        let result = ok(&record(
            "criterion-15",
            &["--monitor", "0", "--fps", "15", "--duration", "30s"],
        ));
        let stored = result["frames_stored"].as_u64().unwrap();
        let dropped = result["dropped_frames"].as_u64().unwrap();
        println!("frames_stored {stored}, dropped_frames {dropped}");
        assert!(
            dropped * 20 <= stored + dropped,
            "{dropped} dropped of {} captured",
            stored + dropped
        );
    }
}

#[cfg(target_os = "macos")]
mod on_macos {
    use super::*;
    use common::{Browser, serve, wait_for_window};

    /// A Chrome application window showing `page`, titled `title(name)`, and its entry
    /// in `recdr windows`.
    fn test_window(name: &str, page: &str) -> (Browser, Value) {
        let port = serve();
        let browser = Browser::app(
            name,
            &format!("http://127.0.0.1:{port}/{page}#{}", title(name)),
        );
        let window = wait_for_window(&title(name));
        (browser, window)
    }

    #[test]
    #[ignore]
    fn windows_lists_the_test_window() {
        let (_browser, window) = test_window("list", "anim");
        assert_eq!(window["title"], title("list"));
        assert_eq!(window["minimized"], false);
        // A macOS application's name is its executable's name.
        let chrome = common::chrome_path();
        let name = chrome.file_name().unwrap().to_str().unwrap();
        assert_eq!(window["process"], name);
        let pid = window["pid"].as_u64().unwrap().to_string();
        let alive = std::process::Command::new("kill")
            .args(["-0", &pid])
            .status()
            .unwrap();
        assert!(alive.success(), "kill -0 {pid}");
    }

    #[test]
    #[ignore]
    fn window_and_process_find_it_and_two_are_ambiguous() {
        let (_one, one) = test_window("one", "anim");
        let found = record(
            "one-title",
            &["--window", &title("one"), "--duration", "1s"],
        );
        ok(&found);
        let process = one["process"].as_str().unwrap();
        let by_process = record("one-process", &["--process", process, "--duration", "1s"]);
        if by_process.status.code() != Some(0) {
            // The owner's own Chrome may run under the same name.
            let (code, err) = error(&by_process);
            assert_eq!(
                (code, err["code"].as_str()),
                (Some(1), Some("ambiguous_target"))
            );
            let ids: Vec<&Value> = err["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| &c["id"])
                .collect();
            assert!(ids.contains(&&one["id"]), "{err}");
        }
        let (_a, a) = test_window("two-a", "anim");
        let (_b, b) = test_window("two-b", "anim");
        let (code, err) = error(&record(
            "two",
            &["--window", &title("two"), "--duration", "1s"],
        ));
        assert_eq!(
            (code, err["code"].as_str()),
            (Some(1), Some("ambiguous_target"))
        );
        let mut ids: Vec<&Value> = err["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| &c["id"])
            .collect();
        ids.sort_by_key(|id| id.as_u64());
        let mut expected = vec![&a["id"], &b["id"]];
        expected.sort_by_key(|id| id.as_u64());
        assert_eq!(ids, expected);
        let missing = format!("no-such-window-{}", std::process::id());
        let (code, err) = error(&record(
            "missing",
            &["--window", &missing, "--duration", "1s"],
        ));
        assert_eq!(
            (code, err["code"].as_str()),
            (Some(1), Some("target_not_found"))
        );
    }

    #[test]
    #[ignore]
    fn minimized_window_is_listed_and_refused() {
        let (browser, window) = test_window("min", "anim");
        let mut client = browser.client();
        let tab = client.tab_ids()[0].clone();
        client.minimize(&tab);
        let id = window["id"].as_u64().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let list = ok(&recdr().arg("windows").output().unwrap());
            let entry = list["windows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|w| w["id"] == id)
                .cloned();
            if entry.as_ref().is_some_and(|w| w["minimized"] == true) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "not minimized: {entry:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        let (code, err) = error(&record(
            "min",
            &["--window-id", &id.to_string(), "--duration", "1s"],
        ));
        assert_eq!(
            (code, err["code"].as_str()),
            (Some(1), Some("target_minimized"))
        );
    }

    /// Criterion 1. Each display's size in pixels from CoreGraphics display modes,
    /// which ScreenCaptureKit does not use for this (`system_profiler` lists none on
    /// the Mac this was built on).
    fn display_pixel_sizes() -> Vec<(u64, u64)> {
        use objc2_core_graphics::{
            CGDisplayCopyDisplayMode, CGDisplayMode, CGGetActiveDisplayList,
        };
        let mut ids = [0u32; 16];
        let mut count = 0u32;
        // SAFETY: `ids` holds 16 entries and `count` is a valid out pointer.
        let status = unsafe { CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut count) };
        assert_eq!(status.0, 0, "CGGetActiveDisplayList");
        ids[..count as usize]
            .iter()
            .map(|&id| {
                let mode = CGDisplayCopyDisplayMode(id).expect("display mode");
                (
                    CGDisplayMode::pixel_width(Some(&mode)) as u64,
                    CGDisplayMode::pixel_height(Some(&mode)) as u64,
                )
            })
            .collect()
    }

    /// The stored frames of a recording result, decoded.
    fn stored_frames(result: &Value) -> Vec<image::RgbaImage> {
        let manifest = json(&std::fs::read(result["manifest"].as_str().unwrap()).unwrap());
        let dir = std::path::Path::new(result["dir"].as_str().unwrap());
        manifest["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                image::open(dir.join(f["file"].as_str().unwrap()))
                    .unwrap()
                    .to_rgba8()
            })
            .collect()
    }

    #[test]
    #[ignore]
    fn monitor_recording_has_the_monitor_size() {
        // The size macOS's own screenshot of the main display has, which does not go
        // through recdr's scale code.
        let shot =
            std::env::temp_dir().join(format!("recdr-main-display-{}.png", std::process::id()));
        let status = std::process::Command::new("screencapture")
            .args(["-x", "-D", "1"])
            .arg(&shot)
            .status()
            .unwrap();
        assert!(status.success(), "screencapture failed");
        let expected = image::image_dimensions(&shot).unwrap();
        let _ = std::fs::remove_file(&shot);
        let result = ok(&record("monitor", &["--monitor", "0", "--duration", "2s"]));
        assert!(result["frames_stored"].as_u64().unwrap() >= 1);
        for frame in stored_frames(&result) {
            assert_eq!(frame.dimensions(), expected);
        }
    }

    /// Criterion 5, the size half. It needs nothing on the screen, so CI runs it where the
    /// color half below is skipped.
    #[test]
    #[ignore]
    fn region_recording_is_the_region_size() {
        let args = ["--region", "0,0,400,300", "--duration", "2s"];
        let result = ok(&record("region-size", &args));
        let frames = stored_frames(&result);
        assert!(!frames.is_empty());
        for frame in frames {
            assert_eq!((frame.width(), frame.height()), (400, 300));
        }
    }

    /// Criterion 5. `--no-cursor`, so the pointer resting on the window cannot be the
    /// center pixel.
    #[test]
    #[ignore]
    fn region_inside_the_green_page_is_green() {
        let (_browser, window) = test_window("green", "green");
        let b = &window["bounds"];
        let x = b["x"].as_i64().unwrap() + (b["width"].as_i64().unwrap() - 400) / 2;
        let y = b["y"].as_i64().unwrap() + (b["height"].as_i64().unwrap() - 300) / 2;
        let region = format!("{x},{y},400,300");
        let args = ["--region", &region, "--no-cursor", "--duration", "2s"];
        let result = ok(&record("green", &args));
        let frames = stored_frames(&result);
        assert!(!frames.is_empty());
        let centers: Vec<[u32; 3]> = frames
            .iter()
            .map(|frame| {
                assert_eq!((frame.width(), frame.height()), (400, 300));
                let [r, g, b, _] = frame.get_pixel(200, 150).0;
                [r, g, b].map(u32::from)
            })
            .collect();
        let green = |&[r, g, b]: &[u32; 3]| g > 2 * r && g > 2 * b;
        if !centers.iter().all(green) {
            // The same point of the screen in a monitor recording, to tell the region
            // code from what is on the screen.
            let whole = ok(&record(
                "green-whole",
                &["--monitor", "0", "--duration", "1s"],
            ));
            let frame = stored_frames(&whole).pop().unwrap();
            let monitor = frame.get_pixel((x + 200) as u32, (y + 150) as u32).0;
            panic!(
                "center pixel of each frame: {centers:?}; the same point in a monitor recording: {monitor:?}; window {window}"
            );
        }
    }

    /// Criterion 3. The window's size from CDP, in points, times the page's
    /// devicePixelRatio; Task 4 measured a difference of 0, so 2 pixels are allowed.
    #[test]
    #[ignore]
    fn window_recording_has_the_content_size() {
        let (browser, window) = test_window("content", "anim");
        let mut client = browser.client();
        let tab = client.tab_ids()[0].clone();
        let (width, height) = client.window_size(&tab);
        let session = client.attach(&tab);
        let ratio = client.eval(&session, "devicePixelRatio").as_f64().unwrap();
        let id = window["id"].as_u64().unwrap().to_string();
        let result = ok(&record(
            "content",
            &["--window-id", &id, "--duration", "3s"],
        ));
        let ms = result["duration_ms"].as_u64().unwrap();
        assert!((2500..=3500).contains(&ms), "duration_ms {ms}");
        assert!(result["frames_stored"].as_u64().unwrap() >= 20, "{result}");
        let expected = (
            (width * ratio).round() as i64,
            (height * ratio).round() as i64,
        );
        for frame in stored_frames(&result) {
            let got = (i64::from(frame.width()), i64::from(frame.height()));
            assert!(
                (got.0 - expected.0).abs() <= 2 && (got.1 - expected.1).abs() <= 2,
                "frame {got:?}, window {expected:?}"
            );
        }
    }

    /// Criterion 7.
    #[test]
    #[ignore]
    fn still_page_stores_few_frames() {
        let (_browser, window) = test_window("still", "still");
        let id = window["id"].as_u64().unwrap().to_string();
        let result = ok(&record("still", &["--window-id", &id, "--duration", "3s"]));
        let stored = result["frames_stored"].as_u64().unwrap();
        assert!((1..=3).contains(&stored), "frames_stored {stored}");
        assert!(std::path::Path::new(result["contact_sheet"].as_str().unwrap()).is_file());
    }

    /// Criterion 8.
    #[test]
    #[ignore]
    fn ending_chrome_ends_the_recording() {
        let (browser, window) = test_window("end", "anim");
        let out = out_dir("end");
        let id = window["id"].as_u64().unwrap().to_string();
        let started = recdr()
            .args(["start", "--window-id", &id, "--out"])
            .arg(&out)
            .output()
            .unwrap();
        let recording = ok(&started)["id"].as_str().unwrap().to_owned();
        std::thread::sleep(std::time::Duration::from_secs(1));
        drop(browser);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let status = recdr()
                .args(["status", &recording, "--out"])
                .arg(&out)
                .output()
                .unwrap();
            if ok(&status)["recordings"][0]["status"] == "finished" {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "not finished: {}",
                ok(&status)
            );
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        let stopped = recdr()
            .args(["stop", &recording, "--out"])
            .arg(&out)
            .output()
            .unwrap();
        let result = ok(&stopped);
        assert_eq!(result["stop_reason"], "window_closed");
        let gif = std::fs::File::open(result["gif"].as_str().unwrap()).unwrap();
        let mut decoder = gif::DecodeOptions::new().read_info(gif).unwrap();
        assert!(decoder.read_next_frame().unwrap().is_some());
    }

    /// Criterion 10. If macOS refuses the detached recorder the screen although this
    /// process may read it, `start` fails with `permission_denied` here.
    #[test]
    #[ignore]
    fn detached_recording_and_mcp_work_on_a_window() {
        let (_browser, window) = test_window("mcp", "anim");
        let id = window["id"].as_u64().unwrap();
        let out = out_dir("mcp");
        let local = |args: &[&str]| recdr().args(args).arg("--out").arg(&out).output().unwrap();
        let started = ok(&local(&["start", "--window-id", &id.to_string()]));
        let recording = started["id"].as_str().unwrap().to_owned();
        std::thread::sleep(std::time::Duration::from_millis(500));
        ok(&local(&["mark", &recording, "halfway"]));
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let result = ok(&local(&["stop", &recording]));
        assert_eq!(result["marks"][0]["label"], "halfway");
        let frames = ok(&local(&["frames", &recording, "--keyframes"]));
        assert!(!frames["frames"].as_array().unwrap().is_empty(), "{frames}");

        let mut server = common::Server::start(None);
        let windows = server.call("list_windows", serde_json::json!({}));
        let windows: Value = serde_json::from_str(&common::text(&windows)).unwrap();
        assert!(
            windows["windows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["id"] == id)
        );
        let monitors = server.call("list_monitors", serde_json::json!({}));
        let monitors: Value = serde_json::from_str(&common::text(&monitors)).unwrap();
        assert!(!monitors["monitors"].as_array().unwrap().is_empty());
        let args = serde_json::json!({"window_id": id, "duration": "1s", "out": out});
        let recorded = server.call("record", args);
        assert!(!common::is_error(&recorded), "{recorded}");
        let result: Value = serde_json::from_str(&common::text(&recorded)).unwrap();
        assert!(result["contact_sheet"].is_string(), "{result}");
    }

    #[test]
    #[ignore]
    fn monitors_have_one_primary_and_pixel_sizes() {
        let listed = ok(&recdr().arg("monitors").output().unwrap());
        let monitors = listed["monitors"].as_array().unwrap();
        let primaries = monitors.iter().filter(|m| m["primary"] == true).count();
        assert_eq!(primaries, 1, "{listed}");
        let mut got: Vec<(u64, u64)> = monitors
            .iter()
            .map(|m| {
                let b = &m["bounds"];
                (b["width"].as_u64().unwrap(), b["height"].as_u64().unwrap())
            })
            .collect();
        let mut expected = display_pixel_sizes();
        got.sort();
        expected.sort();
        assert_eq!(got, expected);
    }
}
