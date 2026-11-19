//! Tab recording against a real headless Chrome. Every test is ignored; run them with
//! `cargo test --test browser -- --ignored`. Set `RECDR_TEST_CHROME` when Chrome is not
//! at its default path.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Child, Output, Stdio};
use std::time::{Duration, Instant};

use common::{Browser, Client, Server, json, out_dir, recdr, serve, text};
use serde_json::{Value, json};
use tungstenite::Message;

/// The tabs `recdr tabs` prints for this `--cdp` value.
fn recdr_tabs(cdp: &str, chrome_dir: Option<&PathBuf>) -> Vec<Value> {
    let mut command = recdr();
    command.args(["tabs", "--cdp", cdp]);
    if let Some(dir) = chrome_dir {
        command.env("RECDR_CHROME_USER_DATA_DIR", dir);
    }
    let output = command.output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{cdp}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    json(&output.stdout)["tabs"].as_array().unwrap().clone()
}

fn ids(tabs: &[Value]) -> Vec<String> {
    tabs.iter()
        .map(|tab| tab["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Success criterion 1.
#[test]
#[ignore]
fn tabs_lists_only_pages() {
    let browser = Browser::start("tabs-pages", &[]);
    let mut client = browser.client();
    client.call("Target.createTarget", json!({ "url": "about:blank" }), None);
    let expected = client.tab_ids();
    assert_eq!(expected.len(), 2, "{expected:?}");

    let tabs = recdr_tabs(&browser.port.to_string(), None);
    let mut got = ids(&tabs);
    let mut want = expected.clone();
    got.sort();
    want.sort();
    assert_eq!(got, want);
    for tab in &tabs {
        assert!(tab["title"].is_string() && tab["url"].is_string(), "{tab}");
    }
}

#[test]
#[ignore]
fn tabs_through_each_endpoint_form() {
    let browser = Browser::start("tabs-forms", &[]);
    let mut expected = browser.client().tab_ids();
    expected.sort();
    let port = browser.port.to_string();
    for (cdp, dir) in [
        (port.clone(), None),
        (format!("http://127.0.0.1:{port}"), None),
        (browser.ws_url(), None),
        ("chrome".to_owned(), Some(&browser.dir)),
    ] {
        let mut got = ids(&recdr_tabs(&cdp, dir));
        got.sort();
        assert_eq!(got, expected, "{cdp}");
    }
}

/// The first tab of a fresh browser, showing `page`, with a client attached to it.
fn tab_showing(browser: &Browser, page: &str) -> (Client, String, String) {
    let mut client = browser.client();
    let tab = client.tab_ids().remove(0);
    let session = client.attach(&tab);
    client.navigate(&session, &format!("http://127.0.0.1:{}{page}", serve()));
    (client, tab, session)
}

/// Runs `recdr <args> --cdp <port> --out <out>`.
fn run(browser: &Browser, out: &Path, args: &[&str]) -> Output {
    recdr()
        .args(args)
        .args(["--cdp", &browser.port.to_string()])
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

fn error_code(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    json(&output.stderr)["error"]["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Sum of the GIF's frame delays, in hundredths of a second.
fn gif_length(path: &str) -> u32 {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::Indexed);
    let mut decoder = options
        .read_info(std::fs::File::open(path).unwrap())
        .unwrap();
    let mut total = 0;
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        total += u32::from(frame.delay);
    }
    total
}

/// Success criterion 2.
#[test]
#[ignore]
fn records_a_tab_at_its_viewport_size() {
    for (name, extra) in [
        ("viewport-1", &[][..]),
        ("viewport-2", &["--force-device-scale-factor=2"][..]),
    ] {
        let browser = Browser::start(name, extra);
        let (mut client, tab, session) = tab_showing(&browser, "/anim");
        let width = client.eval(&session, "innerWidth * devicePixelRatio");
        let height = client.eval(&session, "innerHeight * devicePixelRatio");
        let result = ok(&run(
            &browser,
            &out_dir(name),
            &["record", "--tab-id", &tab, "--duration", "3s"],
        ));
        assert_eq!(result["width"].as_f64(), width.as_f64(), "{name}");
        assert_eq!(result["height"].as_f64(), height.as_f64(), "{name}");
        let length = gif_length(result["gif"].as_str().unwrap());
        assert!((250..=350).contains(&length), "{name}: {length}");
    }
}

/// Success criterion 3.
#[test]
#[ignore]
fn still_page_stores_one_or_two_frames() {
    let browser = Browser::start("still", &[]);
    let (_client, tab, _) = tab_showing(&browser, "/still");
    let result = ok(&run(
        &browser,
        &out_dir("still"),
        &["record", "--tab-id", &tab, "--duration", "3s"],
    ));
    let stored = result["frames_stored"].as_u64().unwrap();
    assert!((1..=2).contains(&stored), "{stored}");
    assert!(Path::new(result["contact_sheet"].as_str().unwrap()).is_file());
}

/// Success criterion 4.
#[test]
#[ignore]
fn animation_keeps_fifteen_frames_per_second() {
    twice(|attempt| {
        let name = format!("fifteen-{attempt}");
        let browser = Browser::start(&name, &[]);
        let (_client, tab, _) = tab_showing(&browser, "/anim");
        let result = ok(&run(
            &browser,
            &out_dir(&name),
            &[
                "record",
                "--tab-id",
                &tab,
                "--duration",
                "10s",
                "--fps",
                "15",
            ],
        ));
        assert_eq!(result["dropped_frames"], 0);
        let stored = result["frames_stored"].as_u64().unwrap();
        (stored < 100).then(|| format!("{stored} frames stored"))
    });
}

/// Runs a check whose bar depends on the machine's speed, and runs it again once when
/// it falls short. The Windows runner has managed 4 frames a second where this machine
/// manages 15; a real slowdown in recdr falls short both times. `check` returns why it
/// fell short, or None.
fn twice(check: impl Fn(u32) -> Option<String>) {
    let Some(first) = check(1) else { return };
    eprintln!("attempt 1: {first}");
    if let Some(second) = check(2) {
        panic!("twice: {first}; then {second}");
    }
}

/// Success criterion 5.
#[test]
#[ignore]
fn tab_text_matches_title_and_url() {
    let browser = Browser::start("tab-text", &[]);
    let (mut client, anim, _) = tab_showing(&browser, "/anim");
    let created = client.call(
        "Target.createTarget",
        json!({ "url": format!("http://127.0.0.1:{}/still", serve()) }),
        None,
    );
    let still = created["targetId"].as_str().unwrap().to_owned();
    let session = client.attach(&still);
    client.navigate(&session, &format!("http://127.0.0.1:{}/still", serve()));
    let out = out_dir("tab-text");

    // "BOX" is only in the title of the first tab and "/STI" only in the URL of the
    // second, so each records exactly one tab or fails.
    for text in ["BOX", "/STI"] {
        ok(&run(
            &browser,
            &out,
            &["record", "--tab", text, "--duration", "1s"],
        ));
    }

    let both = run(
        &browser,
        &out,
        &["record", "--tab", "127.0.0.1", "--duration", "1s"],
    );
    assert_eq!(error_code(&both), "ambiguous_target");
    let candidates = json(&both.stderr)["error"]["candidates"].clone();
    let mut ids: Vec<&str> = candidates
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let mut keys: Vec<&String> = c.as_object().unwrap().keys().collect();
            keys.sort();
            assert_eq!(keys, ["id", "title", "url"]);
            c["id"].as_str().unwrap()
        })
        .collect();
    ids.sort();
    let mut expected = [anim.as_str(), still.as_str()];
    expected.sort();
    assert_eq!(ids, expected);

    for (flag, value, code) in [
        ("--tab", "127.0.0.1", "ambiguous_target"),
        ("--tab", "no-such-tab", "target_not_found"),
        ("--tab-id", "NOPE", "target_not_found"),
    ] {
        let recorded = run(&browser, &out, &["record", flag, value, "--duration", "1s"]);
        let started = run(&browser, &out, &["start", flag, value]);
        assert_eq!(error_code(&recorded), code, "record {flag} {value}");
        assert_eq!(error_code(&started), code, "start {flag} {value}");
    }
}

/// Success criterion 12.
#[test]
#[ignore]
fn start_mark_stop_frames_on_a_tab() {
    let browser = Browser::start("start-stop", &[]);
    let (_client, tab, _) = tab_showing(&browser, "/anim");
    let out = out_dir("tab-start-stop");
    let begun = Instant::now();
    let started = ok(&run(&browser, &out, &["start", "--tab-id", &tab]));
    assert!(
        begun.elapsed() < Duration::from_secs(2),
        "{:?}",
        begun.elapsed()
    );
    let id = started["id"].as_str().unwrap();
    let local = |args: &[&str]| recdr().args(args).arg("--out").arg(&out).output().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    ok(&local(&["mark", id, "halfway"]));
    std::thread::sleep(Duration::from_millis(1000));
    let result = ok(&local(&["stop", id]));
    assert_eq!(result["stop_reason"], "stopped");
    assert_eq!(result["marks"][0]["label"], "halfway");
    let frames = ok(&local(&["frames", id, "--at", "1s"]));
    let path = frames["frames"][0]["path"].as_str().unwrap();
    assert!(Path::new(path).is_file(), "{path}");
}

/// Starts `recdr record <args> --cdp <port> --out <out>` and returns once it has
/// stored its first frame.
fn spawn_record(browser: &Browser, out: &Path, args: &[&str]) -> Child {
    let child = recdr()
        .arg("record")
        .args(args)
        .args(["--cdp", &browser.port.to_string()])
        .arg("--out")
        .arg(out)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !has_a_frame(out) {
        assert!(
            Instant::now() < deadline,
            "no frame stored under {}",
            out.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    child
}

fn has_a_frame(out: &Path) -> bool {
    let Ok(recordings) = std::fs::read_dir(out) else {
        return false;
    };
    recordings.flatten().any(|recording| {
        std::fs::read_dir(recording.path().join("frames"))
            .is_ok_and(|mut frames| frames.next().is_some())
    })
}

fn visibility(client: &mut Client, session: &str) -> Value {
    client.eval(session, "document.visibilityState")
}

/// Success criterion 6, for a tab that is not the selected one.
#[test]
#[ignore]
fn background_tab_is_recorded() {
    twice(|attempt| {
        let name = format!("background-{attempt}");
        let browser = Browser::start(&name, &[]);
        let (mut client, tab, session) = tab_showing(&browser, "/anim");
        let created = client.call("Target.createTarget", json!({ "url": "about:blank" }), None);
        let other = client.attach(created["targetId"].as_str().unwrap());
        assert_eq!(visibility(&mut client, &session), "hidden");

        let child = spawn_record(
            &browser,
            &out_dir(&name),
            &["--tab-id", &tab, "--duration", "3s", "--fps", "15"],
        );
        assert_eq!(visibility(&mut client, &other), "visible");
        let result = ok(&child.wait_with_output().unwrap());
        assert_eq!(visibility(&mut client, &session), "hidden");
        let stored = result["frames_stored"].as_u64().unwrap();
        (stored < 20).then(|| format!("{stored} frames stored"))
    });
}

/// Success criterion 6, for a tab whose window is minimized.
#[test]
#[ignore]
fn minimized_window_is_recorded() {
    let browser = Browser::start("minimized", &[]);
    let (mut client, tab, session) = tab_showing(&browser, "/anim");
    let window = client.call(
        "Browser.getWindowForTarget",
        json!({ "targetId": tab }),
        None,
    );
    client.call(
        "Browser.setWindowBounds",
        json!({ "windowId": window["windowId"], "bounds": { "windowState": "minimized" } }),
        None,
    );
    assert_eq!(visibility(&mut client, &session), "hidden");

    let result = ok(&run(
        &browser,
        &out_dir("minimized"),
        &[
            "record",
            "--tab-id",
            &tab,
            "--duration",
            "3s",
            "--fps",
            "15",
        ],
    ));
    let stored = result["frames_stored"].as_u64().unwrap();
    assert!(stored >= 20, "{stored}");
    assert_eq!(visibility(&mut client, &session), "hidden");
}

/// The result of a recording that ended early, with a GIF that has a frame.
fn ended_with_tab_closed(output: &Output) {
    let result = ok(output);
    assert_eq!(result["stop_reason"], "tab_closed");
    let file = std::fs::File::open(result["gif"].as_str().unwrap()).unwrap();
    let mut decoder = gif::DecodeOptions::new().read_info(file).unwrap();
    assert!(decoder.read_next_frame().unwrap().is_some());
}

/// Success criterion 7, closing the tab.
#[test]
#[ignore]
fn closing_the_tab_ends_with_tab_closed() {
    let browser = Browser::start("close-tab", &[]);
    let (mut client, tab, _) = tab_showing(&browser, "/anim");
    let child = spawn_record(
        &browser,
        &out_dir("close-tab"),
        &["--tab-id", &tab, "--duration", "30s"],
    );
    client.call("Target.closeTarget", json!({ "targetId": tab }), None);
    ended_with_tab_closed(&child.wait_with_output().unwrap());
}

/// Success criterion 7, ending the browser.
#[test]
#[ignore]
fn ending_the_browser_ends_with_tab_closed() {
    let mut browser = Browser::start("end-browser", &[]);
    let (_client, tab, _) = tab_showing(&browser, "/anim");
    let child = spawn_record(
        &browser,
        &out_dir("end-browser"),
        &["--tab-id", &tab, "--duration", "30s"],
    );
    browser.child.kill().unwrap();
    ended_with_tab_closed(&child.wait_with_output().unwrap());
}

/// The center pixel of a stored frame.
fn center(dir: &Path, file: &str) -> [u8; 3] {
    let image = image::open(dir.join(file)).unwrap().to_rgb8();
    let pixel = image.get_pixel(image.width() / 2, image.height() / 2);
    pixel.0
}

/// Success criteria 8 and 13.
#[test]
#[ignore]
fn navigation_does_not_end_the_recording() {
    let browser = Browser::start("navigate", &[]);
    let (mut client, tab, session) = tab_showing(&browser, "/anim");
    let child = spawn_record(
        &browser,
        &out_dir("navigate"),
        &["--tab-id", &tab, "--duration", "4s"],
    );
    // Another site and another page; every call here panics if the browser refuses it.
    client.navigate(&session, &format!("http://localhost:{}/green", serve()));
    assert_eq!(client.eval(&session, "document.title"), "green");

    let result = ok(&child.wait_with_output().unwrap());
    assert_eq!(result["stop_reason"], "duration_reached");
    let dir = PathBuf::from(result["dir"].as_str().unwrap());
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(result["manifest"].as_str().unwrap()).unwrap())
            .unwrap();
    let frames = manifest["frames"].as_array().unwrap();
    let file = |frame: &Value| frame["file"].as_str().unwrap().to_owned();
    assert_eq!(center(&dir, &file(&frames[0])), [255, 255, 255]);
    assert_eq!(
        center(&dir, &file(frames.last().unwrap())),
        [0x00, 0xaa, 0x00]
    );
}

/// What a renderer crash does to a recording. Either stop reason is accepted; the test
/// prints which one happened.
#[test]
#[ignore]
fn renderer_crash_probe() {
    let browser = Browser::start("crash", &[]);
    let (mut client, tab, session) = tab_showing(&browser, "/anim");
    let begun = Instant::now();
    let child = spawn_record(
        &browser,
        &out_dir("crash"),
        &["--tab-id", &tab, "--duration", "8s"],
    );
    // Sent without waiting: the crash may take the reply with it.
    let crash = json!({
        "id": 999_999,
        "method": "Page.navigate",
        "params": { "url": "chrome://crash" },
        "sessionId": session,
    });
    client.ws.send(Message::text(crash.to_string())).unwrap();
    let output = child.wait_with_output().unwrap();
    println!("the crashed recording took {:?}", begun.elapsed());
    // 8 s of recording and about 6 s of GIF encoding when run alone; 30 s means a hang.
    assert!(
        begun.elapsed() < Duration::from_secs(30),
        "{:?}",
        begun.elapsed()
    );
    let result = ok(&output);
    let reason = result["stop_reason"].as_str().unwrap();
    println!("renderer crash gave stop_reason {reason}");
    assert!(
        ["tab_closed", "duration_reached"].contains(&reason),
        "{reason}"
    );
}

fn manifest_of(result: &Value) -> Value {
    serde_json::from_slice(&std::fs::read(result["manifest"].as_str().unwrap()).unwrap()).unwrap()
}

/// The pixels of the 40x40 square centered on (x, y) in an RGBA buffer `width` wide.
fn square(rgba: &[u8], width: u32, x: i32, y: i32) -> Vec<[u8; 3]> {
    let mut pixels = Vec::new();
    for row in y - 20..y + 20 {
        for column in x - 20..x + 20 {
            let at = (row as usize * width as usize + column as usize) * 4;
            pixels.push([rgba[at], rgba[at + 1], rgba[at + 2]]);
        }
    }
    pixels
}

/// The RGBA pixels of the GIF frame on screen at `t_ms`, and the GIF's width.
fn gif_frame_at(path: &str, t_ms: u64) -> (Vec<u8>, u32) {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options
        .read_info(std::fs::File::open(path).unwrap())
        .unwrap();
    let width = u32::from(decoder.width());
    let mut ends_at = 0;
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        ends_at += u64::from(frame.delay) * 10;
        if ends_at > t_ms {
            return (frame.buffer.to_vec(), width);
        }
    }
    panic!("the GIF ends before {t_ms} ms");
}

/// Success criterion 9.
#[test]
#[ignore]
fn press_is_recorded_in_frame_pixels() {
    for (name, scale, extra) in [
        ("press-1", 1, &[][..]),
        ("press-2", 2, &["--force-device-scale-factor=2"][..]),
    ] {
        let browser = Browser::start(name, extra);
        let (mut client, tab, session) = tab_showing(&browser, "/anim");
        let child = spawn_record(
            &browser,
            &out_dir(name),
            &["--tab-id", &tab, "--duration", "5s"],
        );
        std::thread::sleep(Duration::from_millis(500));
        let sent = client.press(&session, 100, 50);
        std::thread::sleep(Duration::from_millis(300));
        client.navigate(&session, &format!("http://localhost:{}/green", serve()));
        std::thread::sleep(Duration::from_secs(1));
        client.press(&session, 100, 50);
        let result = ok(&child.wait_with_output().unwrap());

        let manifest = manifest_of(&result);
        let clicks = manifest["clicks"].as_array().unwrap();
        assert_eq!(clicks.len(), 2, "{name}: {clicks:?}");
        let click = &clicks[0];
        assert_eq!(click["button"], "left", "{name}");
        assert_eq!(
            (click["x"].clone(), click["y"].clone()),
            (json!(100 * scale), json!(50 * scale)),
            "{name}"
        );
        let t_ms = click["t_ms"].as_u64().unwrap();
        let expected = sent - manifest["started_at_unix_ms"].as_u64().unwrap();
        println!("{name}: click at {t_ms} ms, sent at {expected} ms");
        assert!(
            t_ms.abs_diff(expected) <= 200,
            "{name}: t_ms {t_ms}, sent at {expected}"
        );
        let (x, y) = (100 * scale, 50 * scale);

        // The stored frame on screen at the click has no ring.
        let dir = PathBuf::from(result["dir"].as_str().unwrap());
        let shown = manifest["frames"]
            .as_array()
            .unwrap()
            .iter()
            .rfind(|frame| frame["t_ms"].as_u64().unwrap() <= t_ms)
            .unwrap();
        let stored = image::open(dir.join(shown["file"].as_str().unwrap()))
            .unwrap()
            .to_rgba8();
        let width = stored.width();
        assert!(
            square(stored.as_raw(), width, x, y)
                .iter()
                .all(|p| *p == [255, 255, 255]),
            "{name}: the stored frame is not white around the click"
        );

        // The GIF draws one 100 ms later.
        let (rgba, gif_width) = gif_frame_at(result["gif"].as_str().unwrap(), t_ms + 100);
        assert_eq!(gif_width, width, "{name}");
        assert!(
            square(&rgba, gif_width, x, y)
                .iter()
                .any(|p| p.iter().any(|c| *c < 200)),
            "{name}: the GIF shows no ring at the click"
        );
    }
}

/// Success criterion 10.
#[test]
#[ignore]
fn no_click_highlight_installs_nothing() {
    let typeof_binding = "typeof window.__recdrClick";
    let browser = Browser::start("no-click", &[]);
    let (mut client, tab, session) = tab_showing(&browser, "/anim");

    let child = spawn_record(
        &browser,
        &out_dir("no-click"),
        &["--tab-id", &tab, "--duration", "2s", "--no-click-highlight"],
    );
    assert_eq!(client.eval(&session, typeof_binding), "undefined");
    client.press(&session, 100, 50);
    let result = ok(&child.wait_with_output().unwrap());
    assert_eq!(manifest_of(&result)["clicks"], json!([]));

    let child = spawn_record(
        &browser,
        &out_dir("with-click"),
        &["--tab-id", &tab, "--duration", "2s"],
    );
    assert_eq!(client.eval(&session, typeof_binding), "function");
    ok(&child.wait_with_output().unwrap());
    client.navigate(&session, &format!("http://127.0.0.1:{}/still", serve()));
    assert_eq!(client.eval(&session, typeof_binding), "undefined");
}

/// Success criterion 16.
#[test]
#[ignore]
fn mcp_records_a_tab() {
    let browser = Browser::start("mcp-tab", &[]);
    let (mut client, tab, session) = tab_showing(&browser, "/anim");
    let width = client.eval(&session, "innerWidth * devicePixelRatio");
    let port = browser.port.to_string();
    let out = out_dir("mcp-tab");
    let mut server = Server::start(None);

    let listed = server.call("list_tabs", json!({ "cdp": port }));
    let tabs: Value = serde_json::from_str(&text(&listed)).unwrap();
    assert!(
        tabs["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == tab),
        "{listed}"
    );

    let recorded = server.call(
        "record",
        json!({ "cdp": port, "tab_id": tab, "duration": "2s", "out": out }),
    );
    assert_eq!(recorded["content"][1]["type"], "image", "{recorded}");
    let result: Value = serde_json::from_str(&text(&recorded)).unwrap();
    assert_eq!(result["width"].as_f64(), width.as_f64());
}

#[cfg(windows)]
const ECHO_THEN_WAIT: &[&str] = &["cmd", "/c", "echo hi & ping -n 3 127.0.0.1 >nul"];
#[cfg(unix)]
const ECHO_THEN_WAIT: &[&str] = &["sh", "-c", "echo hi; sleep 2"];

/// Success criterion 17, the wrapped command.
#[test]
#[ignore]
fn wraps_a_command_while_recording_a_tab() {
    let browser = Browser::start("wrap", &[]);
    let (_client, tab, _) = tab_showing(&browser, "/anim");
    let port = browser.port.to_string();
    let output = recdr()
        .args(["record", "--cdp", &port, "--tab-id", &tab, "--out"])
        .arg(out_dir("wrap"))
        .arg("--")
        .args(ECHO_THEN_WAIT)
        .output()
        .unwrap();
    let result = ok(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(result["stop_reason"], "command_exited", "{stderr}");
    assert_eq!(result["command_exit_code"], 0, "{stderr}");
    assert!(stderr.contains("hi"), "{stderr}");
}
