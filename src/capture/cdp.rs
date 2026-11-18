//! Browser tabs through the Chrome DevTools Protocol: finding the browser, the
//! WebSocket connection, and the list of tabs.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

use crate::error::{Error, Result};
use crate::model::{Endpoint, TabInfo};
use crate::platform;

/// Overrides the directory `--cdp chrome` reads `DevToolsActivePort` from.
pub const CHROME_DIR_ENV: &str = "RECDR_CHROME_USER_DATA_DIR";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_REPLY_TIMEOUT: Duration = Duration::from_secs(30);
const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_SLEEP: Duration = Duration::from_millis(1);
const MAX_MESSAGE_BYTES: usize = 256 << 20;
/// Longest `/json/version` head or body recdr reads.
const MAX_HTTP_BYTES: usize = 1 << 20;

/// The tabs of the browser at `endpoint`.
pub fn list_tabs(endpoint: &Endpoint) -> Result<Vec<TabInfo>> {
    Connection::open(endpoint).map(|(_, tabs)| tabs)
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
    result: Value,
    error: Option<ReplyError>,
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
