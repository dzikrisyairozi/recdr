#![allow(dead_code)]

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

/// The built binary, with the synthetic-source switch removed so each test opts in.
pub fn recdr() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_recdr"));
    command.env_remove("RECDR_FAKE_SOURCE");
    command
}

/// A fresh, empty directory for one test's recordings.
pub fn out_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("recdr-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test output directory");
    dir
}

pub fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(bytes)))
}

/// A WinForms window with this title, owned by a child PowerShell. Kill the child to close it.
pub fn test_window(title: &str, minimized: bool) -> std::process::Child {
    let state = if minimized {
        "$f.WindowState = 'Minimized'; "
    } else {
        ""
    };
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form; \
         $f.Text = '{title}'; $f.Width = 640; $f.Height = 480; {state}\
         [Windows.Forms.Application]::Run($f)"
    );
    Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .spawn()
        .expect("start powershell")
}

/// Polls `recdr windows` until a window with exactly this title shows up.
pub fn wait_for_window(title: &str) -> serde_json::Value {
    for _ in 0..50 {
        let output = recdr().arg("windows").output().unwrap();
        let list = json(&output.stdout);
        if let Some(window) = list["windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["title"] == title)
        {
            return window.clone();
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    panic!("window {title:?} did not appear within 10 s");
}

/// `recdr mcp` on stdio, one JSON-RPC message per line.
pub struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: u64,
}

impl Server {
    /// `recdr mcp`, initialized. `fake` sets the synthetic source, or leaves the real one.
    pub fn start(fake: Option<&str>) -> Self {
        let mut command = recdr();
        if let Some(kind) = fake {
            command.env("RECDR_FAKE_SOURCE", kind);
        }
        let mut child = command
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        // A reader thread, so a silent server fails the test instead of hanging it.
        let (sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if sender.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let mut server = Self {
            child,
            stdin,
            lines,
            next_id: 1,
        };
        let init = server.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "recdr-test", "version": "0"}
            }),
        );
        assert_eq!(init["serverInfo"]["name"], "recdr", "{init}");
        server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        server
    }

    pub fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(30))
                .expect("no reply within 30 s");
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                return message["result"].clone();
            }
        }
    }

    pub fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": tool, "arguments": arguments}))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap().to_owned()
}

pub fn is_error(result: &Value) -> bool {
    result["isError"] == true
}
/// White page with a red 200x200 box at top 400 px whose left edge moves 4 px per
/// animation frame.
pub const ANIMATION: &str = "<!doctype html><title>moving box</title>\
<style>body{margin:0;background:#fff}#b{position:absolute;top:400px;left:0;\
width:200px;height:200px;background:#f00}</style><div id=b></div>\
<script>if (location.hash) document.title = decodeURIComponent(location.hash.slice(1));\
let x=0;const b=document.getElementById('b');\
function f(){x=(x+4)%1000;b.style.left=x+'px';requestAnimationFrame(f)}\
requestAnimationFrame(f)</script>";
pub const STILL: &str = "<!doctype html><title>quiet page</title>\
<style>body{margin:0;background:#fff}</style><h1>A page that does not change</h1>\
<script>if (location.hash) document.title = decodeURIComponent(location.hash.slice(1));</script>";
pub const GREEN: &str = "<!doctype html><title>green</title>\
<style>html,body{margin:0;height:100%;background:#00aa00}</style>\
<script>if (location.hash) document.title = decodeURIComponent(location.hash.slice(1));</script>";

/// Serves the three test pages on 127.0.0.1, by path, and returns the port.
pub fn serve() -> u16 {
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

pub fn chrome_path() -> PathBuf {
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
pub struct Browser {
    pub child: Child,
    pub dir: PathBuf,
    pub port: u16,
}

impl Browser {
    /// Headless Chrome with a fresh user data directory and these extra arguments.
    /// A cold Windows runner's Chrome has written no `DevToolsActivePort` within 20 s,
    /// twice in a row; it gets 60 s, and a start that hangs is ended and tried once more.
    /// Chrome's stderr is kept next to its directory and printed when a start fails.
    pub fn start(name: &str, extra: &[&str]) -> Browser {
        let mut args: Vec<String> = [
            "--headless=new",
            "--remote-debugging-port=0",
            "--window-size=1280,720",
            "--no-first-run",
            "--no-default-browser-check",
        ]
        .iter()
        .chain(extra)
        .map(|a| a.to_string())
        .collect();
        args.push("about:blank".into());
        Self::launch(name, args)
    }

    /// Chrome showing one page as an application window of 800 by 600 points.
    pub fn app(name: &str, url: &str) -> Browser {
        let args = [
            format!("--app={url}"),
            "--window-size=800,600".into(),
            "--remote-debugging-port=0".into(),
            "--no-first-run".into(),
            "--no-default-browser-check".into(),
        ];
        Self::launch(name, args.into())
    }

    fn launch(name: &str, args: Vec<String>) -> Browser {
        Self::try_launch(name, &args).unwrap_or_else(|| {
            eprintln!("Chrome wrote no DevToolsActivePort in 60 s; starting it again");
            Self::try_launch(name, &args).expect("Chrome wrote no DevToolsActivePort, twice")
        })
    }

    fn try_launch(name: &str, args: &[String]) -> Option<Browser> {
        let dir = std::env::temp_dir().join(format!("recdr-browser-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut command = Command::new(chrome_path());
        command
            .arg(format!("--user-data-dir={}", dir.display()))
            .args(args)
            .stdout(Stdio::null())
            .stderr(File::create(dir.with_extension("log")).unwrap());
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
        let deadline = Instant::now() + Duration::from_secs(60);
        browser.port = loop {
            let text = std::fs::read_to_string(browser.port_file()).unwrap_or_default();
            if let [port, _path] = text.lines().collect::<Vec<_>>()[..] {
                break port.parse().unwrap();
            }
            if Instant::now() >= deadline {
                let log = std::fs::read_to_string(browser.dir.with_extension("log"));
                eprintln!(
                    "Chrome's stderr:
{}",
                    log.unwrap_or_default()
                );
                // Dropping the browser ends Chrome and removes its directory.
                return None;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        Some(browser)
    }

    pub fn port_file(&self) -> PathBuf {
        self.dir.join("DevToolsActivePort")
    }

    /// The browser's own ws:// URL, built from `DevToolsActivePort`.
    pub fn ws_url(&self) -> String {
        let text = std::fs::read_to_string(self.port_file()).unwrap();
        let path = text.lines().nth(1).unwrap();
        format!("ws://127.0.0.1:{}{path}", self.port)
    }

    pub fn client(&self) -> Client {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream
            // A CI runner's Chrome has taken over 10 s to answer while recording.
            .set_read_timeout(Some(Duration::from_secs(30)))
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
pub struct Client {
    pub ws: WebSocket<TcpStream>,
    next_id: u64,
}

impl Client {
    /// Sends a command and returns its result, skipping events. Panics on an error reply.
    pub fn call(&mut self, method: &str, params: Value, session: Option<&str>) -> Value {
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
    pub fn attach(&mut self, tab: &str) -> String {
        let result = self.call(
            "Target.attachToTarget",
            json!({ "targetId": tab, "flatten": true }),
            None,
        );
        result["sessionId"].as_str().unwrap().to_owned()
    }

    /// The value of a JavaScript expression in the tab.
    pub fn eval(&mut self, session: &str, expression: &str) -> Value {
        let result = self.call(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true }),
            Some(session),
        );
        result["result"]["value"].clone()
    }

    /// Opens `url` in the tab and waits until it has loaded.
    pub fn navigate(&mut self, session: &str, url: &str) {
        self.call("Page.navigate", json!({ "url": url }), Some(session));
        let loaded = format!("document.readyState == 'complete' && location.href == '{url}'");
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.eval(session, &loaded) != json!(true) {
            assert!(Instant::now() < deadline, "{url} did not load");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// A left press and release at CSS position (x, y). Returns when the press was
    /// sent, in Unix milliseconds.
    pub fn press(&mut self, session: &str, x: u32, y: u32) -> u64 {
        let sent = unix_ms();
        for kind in ["mousePressed", "mouseReleased"] {
            let event = json!({ "type": kind, "x": x, "y": y, "button": "left", "clickCount": 1 });
            self.call("Input.dispatchMouseEvent", event, Some(session));
        }
        sent
    }

    /// Minimizes the browser window that holds this tab.
    pub fn minimize(&mut self, tab: &str) {
        let window = self.call(
            "Browser.getWindowForTarget",
            json!({ "targetId": tab }),
            None,
        );
        let bounds =
            json!({ "windowId": window["windowId"], "bounds": { "windowState": "minimized" } });
        self.call("Browser.setWindowBounds", bounds, None);
    }

    /// The width and height of the browser window that holds this tab, in points.
    pub fn window_size(&mut self, tab: &str) -> (f64, f64) {
        let window = self.call(
            "Browser.getWindowForTarget",
            json!({ "targetId": tab }),
            None,
        );
        let bounds = &window["bounds"];
        (
            bounds["width"].as_f64().unwrap(),
            bounds["height"].as_f64().unwrap(),
        )
    }

    /// Ids of the page targets, in the browser's order.
    pub fn tab_ids(&mut self) -> Vec<String> {
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

fn unix_ms() -> u64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    since.unwrap().as_millis() as u64
}
