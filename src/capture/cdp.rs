//! Browser tabs through the Chrome DevTools Protocol: finding the browser, the
//! WebSocket connection, the list of tabs, and the screencast of one tab.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use image::ImageFormat;
use serde::Deserialize;
use serde_json::{Value, json};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

use super::CaptureSource;
use crate::error::{Error, Result};
use crate::model::{CaptureEvent, Endpoint, Frame, TabInfo, TabQuery, TabTarget};
use crate::platform;

/// Overrides the directory `--cdp chrome` reads `DevToolsActivePort` from.
pub const CHROME_DIR_ENV: &str = "RECDR_CHROME_USER_DATA_DIR";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_REPLY_TIMEOUT: Duration = Duration::from_secs(30);
const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
const IDLE_SLEEP: Duration = Duration::from_millis(1);
const MAX_MESSAGE_BYTES: usize = 256 << 20;
/// Longest `/json/version` head or body recdr reads.
const MAX_HTTP_BYTES: usize = 1 << 20;

/// The tabs of the browser at `endpoint`.
pub fn list_tabs(endpoint: &Endpoint) -> Result<Vec<TabInfo>> {
    Connection::open(endpoint).map(|(_, tabs)| tabs)
}

/// A screencast of one tab. Frames are handed out no faster than the requested rate,
/// and the browser is asked for the next frame only when none is waiting.
pub struct CdpSource {
    conn: Connection,
    session: String,
    interval: Duration,
    held: VecDeque<Frame>,
    /// `sessionId`s of screencast frames not yet acknowledged, oldest first.
    unacked: VecDeque<i64>,
    release_at: Instant,
}

impl CdpSource {
    pub fn open(tab: &TabTarget, fps: u32, _clicks: bool) -> Result<Self> {
        let (mut conn, tabs) = Connection::open(&tab.cdp)?;
        let target = pick(&tabs, &tab.query)?;
        let attached = conn.call(
            "Target.attachToTarget",
            json!({ "targetId": target.id, "flatten": true }),
            None,
            Instant::now() + CALL_TIMEOUT,
        )?;
        let session = attached["sessionId"]
            .as_str()
            .ok_or_else(|| Error::CaptureFailed("Target.attachToTarget gave no session".into()))?
            .to_owned();
        // From here a failure drops the source, and Drop undoes what was switched on.
        let mut source = CdpSource {
            conn,
            session,
            interval: Duration::from_millis(u64::from(1000 / fps)),
            held: VecDeque::new(),
            unacked: VecDeque::new(),
            release_at: Instant::now(),
        };
        source.call(
            "Emulation.setFocusEmulationEnabled",
            json!({ "enabled": true }),
        )?;
        source.call("Page.enable", json!({}))?;
        source.call("Page.startScreencast", json!({ "format": "png" }))?;
        Ok(source)
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.conn.call(
            method,
            params,
            Some(&self.session),
            Instant::now() + CALL_TIMEOUT,
        )
    }

    /// Acts on one message from the browser. Some(event) ends `next_event` with it.
    fn handle(&mut self, incoming: Incoming) -> Result<Option<CaptureEvent>> {
        let method = incoming.method.as_deref().unwrap_or_default();
        // The browser reports a detach outside the session, naming the session.
        if method == "Target.detachedFromTarget" {
            if incoming.params["sessionId"] == self.session.as_str() {
                self.conn.closed = true;
                return Ok(Some(CaptureEvent::Closed));
            }
            return Ok(None);
        }
        if incoming.session_id.as_deref() != Some(self.session.as_str()) {
            return Ok(None);
        }
        if method == "Page.screencastFrame" {
            let data = incoming.params["data"].as_str().unwrap_or_default();
            self.held.push_back(decode(data)?);
            if let Some(id) = incoming.params["sessionId"].as_i64() {
                self.unacked.push_back(id);
            }
        }
        Ok(None)
    }
}

impl CaptureSource for CdpSource {
    fn next_event(&mut self, timeout: Duration) -> Result<Option<CaptureEvent>> {
        let deadline = Instant::now() + timeout;
        loop {
            if !self.held.is_empty() && Instant::now() >= self.release_at {
                let frame = self.held.pop_front();
                if self.held.is_empty()
                    && let Some(id) = self.unacked.pop_front()
                {
                    self.conn.send(
                        "Page.screencastFrameAck",
                        json!({ "sessionId": id }),
                        Some(&self.session),
                    );
                }
                self.release_at = Instant::now() + self.interval;
                return Ok(frame.map(CaptureEvent::Frame));
            }
            let incoming = match self.conn.backlog.pop_front() {
                Some(incoming) => incoming,
                None => match self.conn.poll() {
                    Polled::Message(incoming) => incoming,
                    Polled::Closed => return Ok(Some(CaptureEvent::Closed)),
                    Polled::Idle => {
                        if Instant::now() >= deadline {
                            return Ok(None);
                        }
                        thread::sleep(IDLE_SLEEP);
                        continue;
                    }
                },
            };
            if let Some(event) = self.handle(incoming)? {
                return Ok(Some(event));
            }
        }
    }

    fn dropped(&self) -> u64 {
        0
    }

    fn paced(&self) -> bool {
        true
    }
}

impl Drop for CdpSource {
    fn drop(&mut self) {
        if !self.conn.closed {
            for (method, params) in [
                ("Page.stopScreencast", json!({})),
                (
                    "Emulation.setFocusEmulationEnabled",
                    json!({ "enabled": false }),
                ),
            ] {
                let deadline = Instant::now() + CLOSE_TIMEOUT;
                let _ = self
                    .conn
                    .call(method, params, Some(&self.session), deadline);
            }
        }
        let _ = self.conn.ws.close(None);
    }
}

/// The tab a query names: the whole id, or a title or URL holding the text in any case.
fn pick<'a>(tabs: &'a [TabInfo], query: &TabQuery) -> Result<&'a TabInfo> {
    match query {
        TabQuery::Id(id) => tabs
            .iter()
            .find(|tab| tab.id == *id)
            .ok_or_else(|| Error::TargetNotFound(format!("tab id {id:?}"))),
        TabQuery::Text(text) => {
            let needle = text.to_lowercase();
            let found = tabs.iter().filter(|tab| {
                tab.title.to_lowercase().contains(&needle)
                    || tab.url.to_lowercase().contains(&needle)
            });
            super::single(found, text, "tab", format!("tab {text:?}"))
        }
    }
}

/// A base64 PNG to an RGBA frame.
fn decode(data: &str) -> Result<Frame> {
    let not_png = || Error::CaptureFailed("the browser sent a frame that is not a PNG".into());
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| not_png())?;
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|_| not_png())?
        .to_rgba8();
    Ok(Frame {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

/// `DevToolsActivePort` contents (port on line 1, path on line 2) to a ws:// URL on 127.0.0.1.
fn port_file_url(text: &str) -> Option<String> {
    let mut lines = text.lines();
    let port: u16 = lines.next()?.trim().parse().ok()?;
    let path = lines.next()?.trim();
    (port > 0 && path.starts_with('/')).then(|| format!("ws://127.0.0.1:{port}{path}"))
}

/// `host:port` of a ws:// URL; port 80 when the URL has none.
fn authority(ws_url: &str) -> Option<String> {
    let rest = ws_url.strip_prefix("ws://")?;
    let host = rest.split('/').next()?;
    if host.is_empty() {
        return None;
    }
    Some(if host.contains(':') {
        host.to_owned()
    } else {
        format!("{host}:80")
    })
}

/// Status code and Content-Length from the head of an HTTP reply. The header name is
/// matched without regard to case, and the value may follow the colon with no space.
fn reply_head(head: &str) -> Option<(u16, usize)> {
    let mut lines = head.lines();
    let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let length = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse().ok())?
    })?;
    Some((status, length))
}

/// The tabs in a `Target.getTargets` result: entries whose `type` is `page`, in order.
fn tabs(result: &Value) -> Vec<TabInfo> {
    let text = |info: &Value, key: &str| info[key].as_str().unwrap_or_default().to_owned();
    result["targetInfos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|info| info["type"] == "page")
        .map(|info| TabInfo {
            id: text(info, "targetId"),
            title: text(info, "title"),
            url: text(info, "url"),
        })
        .collect()
}

/// The ws:// URL in `<dir>/DevToolsActivePort`.
fn chrome_url(dir: &Path) -> Result<String> {
    let path = dir.join("DevToolsActivePort");
    let text = std::fs::read_to_string(&path).map_err(|_| {
        Error::CdpConnectFailed(format!(
            "Chrome is not accepting remote debugging; switch it on at \
             chrome://inspect/#remote-debugging ({} was not found)",
            path.display()
        ))
    })?;
    port_file_url(&text).ok_or_else(|| {
        Error::CdpConnectFailed(format!(
            "{} does not hold a port and a path",
            path.display()
        ))
    })
}

/// A TCP connection to `host:port`, trying each address within one time limit.
fn connect(authority: &str) -> Result<TcpStream> {
    let failed = |e: &dyn std::fmt::Display| Error::CdpConnectFailed(format!("{authority}: {e}"));
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    let mut last = std::io::Error::new(ErrorKind::NotFound, "no address found");
    for address in authority.to_socket_addrs().map_err(|e| failed(&e))? {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            last = std::io::Error::new(ErrorKind::TimedOut, "the connection timed out");
            break;
        }
        match TcpStream::connect_timeout(&address, left) {
            Ok(stream) => return Ok(stream),
            Err(e) => last = e,
        }
    }
    Err(failed(&last))
}

/// `webSocketDebuggerUrl` from `GET /json/version` on `host:port`.
fn version_url(authority: &str) -> Result<String> {
    let mut stream = connect(authority)?;
    let not_cdp = || {
        Error::CdpConnectFailed(format!(
            "{authority} did not answer GET /json/version as a CDP server"
        ))
    };
    stream
        .set_read_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|_| not_cdp())?;
    write!(
        stream,
        "GET /json/version HTTP/1.1\r\nHost: {authority}\r\n\r\n"
    )
    .map_err(|_| not_cdp())?;
    // Chrome keeps the connection open, so the body is read by its length.
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        let read = stream.read(&mut chunk).map_err(|_| not_cdp())?;
        if read == 0 || bytes.len() > MAX_HTTP_BYTES {
            return Err(not_cdp());
        }
        bytes.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&bytes[..head_end]).into_owned();
    let (status, length) = reply_head(&head).ok_or_else(not_cdp)?;
    if status != 200 || length > MAX_HTTP_BYTES {
        return Err(not_cdp());
    }
    let mut body = bytes.split_off(head_end + 4);
    while body.len() < length {
        let read = stream.read(&mut chunk).map_err(|_| not_cdp())?;
        if read == 0 {
            return Err(not_cdp());
        }
        body.extend_from_slice(&chunk[..read]);
    }
    let version: Value = serde_json::from_slice(&body[..length]).map_err(|_| not_cdp())?;
    version["webSocketDebuggerUrl"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(not_cdp)
}

fn websocket_url(endpoint: &Endpoint) -> Result<String> {
    match endpoint {
        Endpoint::Port(port) => version_url(&format!("127.0.0.1:{port}")),
        Endpoint::Http(authority) => version_url(authority),
        Endpoint::Ws(url) => Ok(url.clone()),
        Endpoint::Chrome => {
            let dir = std::env::var_os(CHROME_DIR_ENV)
                .map(Into::into)
                .or_else(platform::chrome_user_data_dir)
                .ok_or_else(|| {
                    Error::CdpConnectFailed(
                        "the Chrome user data directory is unknown on this system".into(),
                    )
                })?;
            chrome_url(&dir)
        }
    }
}

/// A message from the browser: a reply (with `id`) or an event (with `method`).
#[derive(Deserialize)]
struct Incoming {
    id: Option<u64>,
    method: Option<String>,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    result: Value,
    error: Option<ReplyError>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

#[derive(Deserialize)]
struct ReplyError {
    message: String,
}

enum Polled {
    Message(Incoming),
    Idle,
    Closed,
}

/// One WebSocket to a browser, read and written on the caller's thread.
struct Connection {
    ws: WebSocket<TcpStream>,
    next_id: u64,
    /// Events that arrived while `call` waited for its reply.
    backlog: VecDeque<Incoming>,
    closed: bool,
}

impl Connection {
    /// Connects and lists the tabs, which also proves the other end speaks CDP.
    fn open(endpoint: &Endpoint) -> Result<(Connection, Vec<TabInfo>)> {
        let url = websocket_url(endpoint)?;
        let authority = authority(&url).ok_or_else(|| {
            Error::CdpConnectFailed(format!("{url} is not a ws:// URL with a host"))
        })?;
        let label = match endpoint {
            Endpoint::Port(_) | Endpoint::Http(_) => authority.clone(),
            Endpoint::Ws(_) | Endpoint::Chrome => url.clone(),
        };
        let stream = connect(&authority)?;
        let deadline = Instant::now() + FIRST_REPLY_TIMEOUT;
        let failed = |e: &dyn std::fmt::Display| Error::CdpConnectFailed(format!("{label}: {e}"));
        stream.set_nodelay(true).map_err(|e| failed(&e))?;
        stream
            .set_read_timeout(Some(FIRST_REPLY_TIMEOUT))
            .map_err(|e| failed(&e))?;
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let (ws, _) = tungstenite::client::client_with_config(url.as_str(), stream, Some(config))
            .map_err(|e| failed(&e))?;
        ws.get_ref().set_nonblocking(true).map_err(|e| failed(&e))?;
        let mut connection = Connection {
            ws,
            next_id: 1,
            backlog: VecDeque::new(),
            closed: false,
        };
        let result = connection
            .call("Target.getTargets", json!({}), None, deadline)
            .map_err(|e| {
                if Instant::now() >= deadline {
                    Error::CdpConnectFailed("the browser did not answer within 30 seconds".into())
                } else {
                    match e {
                        Error::CaptureFailed(message) => Error::CdpConnectFailed(message),
                        other => Error::CdpConnectFailed(other.to_string()),
                    }
                }
            })?;
        Ok((connection, tabs(&result)))
    }

    /// Writes one command and returns its id. A failure marks the connection closed.
    fn send(&mut self, method: &str, params: Value, session: Option<&str>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut command = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            command["sessionId"] = json!(session);
        }
        match self.ws.send(Message::text(command.to_string())) {
            Ok(()) => {}
            Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => {
                // The message is queued; push it out.
                let deadline = Instant::now() + CALL_TIMEOUT;
                loop {
                    match self.ws.flush() {
                        Ok(()) => break,
                        Err(tungstenite::Error::Io(e))
                            if e.kind() == ErrorKind::WouldBlock && Instant::now() < deadline =>
                        {
                            thread::sleep(IDLE_SLEEP);
                        }
                        Err(_) => {
                            self.closed = true;
                            break;
                        }
                    }
                }
            }
            Err(_) => self.closed = true,
        }
        id
    }

    /// The next message, without waiting.
    fn poll(&mut self) -> Polled {
        if self.closed {
            return Polled::Closed;
        }
        match self.ws.read() {
            Ok(Message::Text(text)) => match serde_json::from_str(text.as_str()) {
                Ok(incoming) => Polled::Message(incoming),
                Err(_) => Polled::Idle,
            },
            Ok(Message::Close(_)) => {
                self.closed = true;
                Polled::Closed
            }
            Ok(_) => Polled::Idle,
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                Polled::Idle
            }
            Err(_) => {
                self.closed = true;
                Polled::Closed
            }
        }
    }

    /// Sends a command and waits for its reply. Events that arrive meanwhile are kept.
    fn call(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
        deadline: Instant,
    ) -> Result<Value> {
        let id = self.send(method, params, session);
        loop {
            match self.poll() {
                Polled::Message(incoming) if incoming.id == Some(id) => {
                    return match incoming.error {
                        Some(error) => {
                            Err(Error::CaptureFailed(format!("{method}: {}", error.message)))
                        }
                        None => Ok(incoming.result),
                    };
                }
                Polled::Message(incoming) => {
                    if incoming.method.is_some() {
                        self.backlog.push_back(incoming);
                    }
                }
                Polled::Idle if Instant::now() >= deadline => {
                    return Err(Error::CaptureFailed(format!("{method} was not answered")));
                }
                Polled::Idle => thread::sleep(IDLE_SLEEP),
                Polled::Closed => {
                    return Err(Error::CaptureFailed(
                        "the browser closed the connection".into(),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_reads_a_png_and_refuses_anything_else() {
        let rgba = vec![255, 0, 0, 255, 0, 0, 255, 128];
        let image = image::RgbaImage::from_raw(2, 1, rgba.clone()).unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, ImageFormat::Png).unwrap();
        let data = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
        let frame = decode(&data).unwrap();
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(frame.rgba, rgba);
        assert_eq!(decode("AAAA").unwrap_err().code(), "capture_failed");
    }

    fn tab(id: &str, title: &str, url: &str) -> TabInfo {
        TabInfo {
            id: id.into(),
            title: title.into(),
            url: url.into(),
        }
    }

    fn tabs_to_pick() -> Vec<TabInfo> {
        vec![
            tab("AB12", "Docs", "http://docs.test/"),
            tab("CD34", "Inbox", "http://mail.test/inbox"),
            tab("EF56", "Inbox (2)", "http://other.test/"),
        ]
    }

    #[test]
    fn pick_by_text_in_title_or_url() {
        let tabs = tabs_to_pick();
        let text = |t: &str| pick(&tabs, &TabQuery::Text(t.into()));
        assert_eq!(text("DOC").unwrap().id, "AB12");
        assert_eq!(text("MAIL.test").unwrap().id, "CD34");
        assert_eq!(text("inbox").unwrap_err().code(), "ambiguous_target");
        assert_eq!(text("nothing").unwrap_err().code(), "target_not_found");
    }

    #[test]
    fn pick_by_id_matches_the_whole_id() {
        let tabs = tabs_to_pick();
        let id = |t: &str| pick(&tabs, &TabQuery::Id(t.into()));
        assert_eq!(id("CD34").unwrap().title, "Inbox");
        assert_eq!(id("CD").unwrap_err().code(), "target_not_found");
    }

    #[test]
    fn port_file_url_reads_port_and_path() {
        assert_eq!(
            port_file_url("9222\n/devtools/browser/abc\n").as_deref(),
            Some("ws://127.0.0.1:9222/devtools/browser/abc")
        );
        for text in ["9222\n", "", "x\n/devtools/browser/abc\n"] {
            assert_eq!(port_file_url(text), None, "{text:?}");
        }
    }

    #[test]
    fn reply_head_reads_status_and_length() {
        let tight = "HTTP/1.1 200 OK\r\nContent-Length:464\r\nContent-Type: application/json";
        let spaced = "HTTP/1.1 200 OK\r\ncontent-length: 464";
        assert_eq!(reply_head(tight), Some((200, 464)));
        assert_eq!(reply_head(spaced), Some((200, 464)));
        assert_eq!(reply_head("HTTP/1.1 200 OK\r\nServer: x"), None);
    }

    #[test]
    fn tabs_keeps_pages_in_order() {
        let target = |id: &str, kind: &str| json!({ "targetId": id, "type": kind, "title": format!("t{id}"), "url": format!("u{id}") });
        let result = json!({ "targetInfos": [
            target("1", "page"),
            target("2", "browser_ui"),
            target("3", "service_worker"),
            target("4", "page"),
        ]});
        let ids: Vec<String> = tabs(&result).into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["1", "4"]);
        assert_eq!(tabs(&result)[0].title, "t1");
        assert_eq!(tabs(&result)[0].url, "u1");
    }

    #[test]
    fn authority_of_a_ws_url() {
        assert_eq!(
            authority("ws://127.0.0.1:9222/devtools/browser/abc").as_deref(),
            Some("127.0.0.1:9222")
        );
        assert_eq!(
            authority("ws://example.test/x").as_deref(),
            Some("example.test:80")
        );
        assert_eq!(authority("http://x"), None);
        assert_eq!(authority("ws:///x"), None);
    }

    #[test]
    fn closed_port_fails_quickly() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let begun = Instant::now();
        let error = list_tabs(&Endpoint::Port(port)).unwrap_err();
        assert_eq!(error.code(), "cdp_connect_failed");
        assert!(
            begun.elapsed() < Duration::from_secs(6),
            "{:?}",
            begun.elapsed()
        );
    }

    #[test]
    fn chrome_url_explains_a_missing_or_bad_port_file() {
        let dir = std::env::temp_dir().join(format!("recdr-unit-{}-chrome", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let missing = chrome_url(&dir).unwrap_err();
        assert_eq!(missing.code(), "cdp_connect_failed");
        assert!(
            missing
                .to_string()
                .contains("chrome://inspect/#remote-debugging"),
            "{missing}"
        );
        std::fs::write(dir.join("DevToolsActivePort"), "x").unwrap();
        let bad = chrome_url(&dir).unwrap_err();
        assert_eq!(bad.code(), "cdp_connect_failed");
        assert!(
            bad.to_string().contains("does not hold a port and a path"),
            "{bad}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
