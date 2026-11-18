//! Tab recording against a real headless Chrome. Every test is ignored; run them with
//! `cargo test --test browser -- --ignored`. Set `RECDR_TEST_CHROME` when Chrome is not
//! at its default path.

mod common;

use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{json, recdr};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

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
