//! Tab recording against a real headless Chrome. Every test is ignored; run them with
//! `cargo test --test browser -- --ignored`. Set `RECDR_TEST_CHROME` when Chrome is not
//! at its default path.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use common::{json, out_dir, recdr};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

/// White page with a red 200x200 box at top 400 px whose left edge moves 4 px per
/// animation frame.
const ANIMATION: &str = "<!doctype html><title>moving box</title>\
<style>body{margin:0;background:#fff}#b{position:absolute;top:400px;left:0;\
width:200px;height:200px;background:#f00}</style><div id=b></div>\
<script>let x=0;const b=document.getElementById('b');\
function f(){x=(x+4)%1000;b.style.left=x+'px';requestAnimationFrame(f)}\
requestAnimationFrame(f)</script>";
const STILL: &str = "<!doctype html><title>quiet page</title>\
<style>body{margin:0;background:#fff}</style><h1>A page that does not change</h1>";
const GREEN: &str = "<!doctype html><title>green</title>\
<style>html,body{margin:0;height:100%;background:#00aa00}</style>";

/// Serves the three test pages on 127.0.0.1, by path, and returns the port.
fn serve() -> u16 {
    static PORT: OnceLock<u16> = OnceLock::new();
    *PORT.get_or_init(|| {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || answer(stream));
            }
        });
        port
    })
}

fn answer(mut stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request = String::new();
    if reader.read_line(&mut request).is_err() {
        return;
    }
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
    }
    let path = request.split_whitespace().nth(1).unwrap_or_default();
    let (status, body) = match path {
        "/anim" => ("200 OK", ANIMATION),
        "/still" => ("200 OK", STILL),
        "/green" => ("200 OK", GREEN),
        _ => ("404 Not Found", ""),
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
}

fn chrome_path() -> PathBuf {
    if let Some(path) = std::env::var_os("RECDR_TEST_CHROME") {
        return path.into();
    }
    PathBuf::from(if cfg!(windows) {
        r"C:\Program Files\Google\Chrome\Application\chrome.exe"
    } else if cfg!(target_os = "macos") {
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    } else {
        "/usr/bin/google-chrome"
    })
}

/// A headless Chrome with its own user data directory, ended when dropped.
struct Browser {
    child: Child,
    dir: PathBuf,
    port: u16,
}

impl Browser {
    /// Headless Chrome with a fresh user data directory and these extra arguments.
    fn start(name: &str, extra: &[&str]) -> Browser {
        let dir = std::env::temp_dir().join(format!("recdr-browser-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut command = Command::new(chrome_path());
        command
            .arg("--headless=new")
            .arg("--remote-debugging-port=0")
            .arg(format!("--user-data-dir={}", dir.display()))
            .args([
                "--window-size=1280,720",
                "--no-first-run",
                "--no-default-browser-check",
            ])
            .args(extra)
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if std::env::var_os("CI").is_some() {
            command.arg("--no-sandbox");
        }
        let child = command
            .spawn()
            .unwrap_or_else(|e| panic!("start {}: {e}", chrome_path().display()));
        let mut browser = Browser {
            child,
            dir,
            port: 0,
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        browser.port = loop {
            let text = std::fs::read_to_string(browser.port_file()).unwrap_or_default();
            if let [port, _path] = text.lines().collect::<Vec<_>>()[..] {
                break port.parse().unwrap();
            }
            assert!(
                Instant::now() < deadline,
                "Chrome wrote no DevToolsActivePort"
            );
            std::thread::sleep(Duration::from_millis(100));
        };
        browser
    }

    fn port_file(&self) -> PathBuf {
        self.dir.join("DevToolsActivePort")
    }

    /// The browser's own ws:// URL, built from `DevToolsActivePort`.
    fn ws_url(&self) -> String {
        let text = std::fs::read_to_string(self.port_file()).unwrap();
        let path = text.lines().nth(1).unwrap();
        format!("ws://127.0.0.1:{}{path}", self.port)
    }

    fn client(&self) -> Client {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let (ws, _) = tungstenite::client(self.ws_url().as_str(), stream).unwrap();
        Client { ws, next_id: 1 }
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The test's own CDP connection, separate from recdr's.
struct Client {
    ws: WebSocket<TcpStream>,
    next_id: u64,
}

impl Client {
    /// Sends a command and returns its result, skipping events. Panics on an error reply.
    fn call(&mut self, method: &str, params: Value, session: Option<&str>) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let mut command = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            command["sessionId"] = json!(session);
        }
        self.ws.send(Message::text(command.to_string())).unwrap();
        loop {
            let Message::Text(text) = self.ws.read().unwrap() else {
                continue;
            };
            let reply: Value = serde_json::from_str(text.as_str()).unwrap();
            if reply["id"] == id {
                assert!(reply["error"].is_null(), "{method}: {reply}");
                return reply["result"].clone();
            }
        }
    }

    /// Attaches to a tab and returns the session id for later calls.
    fn attach(&mut self, tab: &str) -> String {
        let result = self.call(
            "Target.attachToTarget",
            json!({ "targetId": tab, "flatten": true }),
            None,
        );
        result["sessionId"].as_str().unwrap().to_owned()
    }

    /// The value of a JavaScript expression in the tab.
    fn eval(&mut self, session: &str, expression: &str) -> Value {
        let result = self.call(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true }),
            Some(session),
        );
        result["result"]["value"].clone()
    }

    /// Opens `url` in the tab and waits until it has loaded.
    fn navigate(&mut self, session: &str, url: &str) {
        self.call("Page.navigate", json!({ "url": url }), Some(session));
        let loaded = format!("document.readyState == 'complete' && location.href == '{url}'");
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.eval(session, &loaded) != json!(true) {
            assert!(Instant::now() < deadline, "{url} did not load");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Ids of the page targets, in the browser's order.
    fn tab_ids(&mut self) -> Vec<String> {
        let result = self.call("Target.getTargets", json!({}), None);
        result["targetInfos"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|info| info["type"] == "page")
            .map(|info| info["targetId"].as_str().unwrap().to_owned())
            .collect()
    }
}

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
    let browser = Browser::start("fifteen", &[]);
    let (_client, tab, _) = tab_showing(&browser, "/anim");
    let result = ok(&run(
        &browser,
        &out_dir("fifteen"),
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
    let stored = result["frames_stored"].as_u64().unwrap();
    assert!(stored >= 100, "{stored}");
    assert_eq!(result["dropped_frames"], 0);
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
