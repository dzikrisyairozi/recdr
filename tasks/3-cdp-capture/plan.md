# Implementation plan: cdp-capture

Spec: [spec/3-cdp-capture/spec.md](../../spec/3-cdp-capture/spec.md). Task list: [todo.md](todo.md). Tracking issue: https://github.com/dzikrisyairozi/recdr/issues/3.

All paths are relative to the repository root. "The spec" means the file above. "The first plan" means [tasks/1-recdr/plan.md](../1-recdr/plan.md).

## Overview

recdr gets a second kind of target, a browser tab, recorded through the Chrome DevTools Protocol (CDP). One new file, `src/capture/cdp.rs`, holds everything that talks to a browser, and it is the third implementation of the existing `CaptureSource` trait. Everything after capture is unchanged.

Three changes to the first module come before any CDP code, because the spec needs them and they are easier to check alone:

1. `recdr start` reports the detached recorder's own error.
2. The Windows mouse hook moves into the Windows capture source, and clicks reach the recorder as capture events.
3. Process code moves behind `src/platform/`, with a Windows file and a Unix file, so recdr builds on Linux and macOS.

The order is: those three changes on Windows (Tasks 1 to 4), the Linux and macOS build with CI (Tasks 5 to 7), the `tabs` command (Tasks 8 and 9), tab recording (Tasks 10 to 13), clicks, MCP, CI for browser tests and the README (Tasks 14 to 17).

### State of the working tree before Task 1

- The current branch is `feat/1-agent-recorder` (pull request #2, not merged). The spec, its goldfish report, this plan and an edit to `spec/1-recdr/capability-map.md` are uncommitted on it.
- Before Task 1, cut `feat/3-cdp-capture` from `feat/1-agent-recorder` and make one commit holding `spec/3-cdp-capture/`, `spec/1-recdr/capability-map.md` and `tasks/3-cdp-capture/`.
- Nothing is pushed unless the owner asks. Checkpoints B and E need a push, because CI runs only on GitHub; the build stops there and asks.

### What was measured for this plan

On 2026-11-18, with a throwaway Rust program using `tungstenite` 0.30 against headless Chrome 158 on Windows 11. Six of these are in the spec's list "What was checked" as facts 12 to 17. The other three (the `tungstenite` calls, the socket and sleep timings, and what headless Chrome reports for a hidden tab) are recorded only here.

- The `tungstenite` calls named under "Module contracts" compile and work with `default-features = false, features = ["handshake"]`.
- Chrome answers `GET /json/version` with the header `Content-Length:464`, with no space after the colon, and keeps the connection open even when the request has `Connection: close`. Reading to the end of the stream waits forever.
- With no acknowledgement Chrome sends three frames and stops.
- Handing the recorder one frame per 67 ms, and acknowledging one frame per frame handed over only when no other frame is waiting, gave 147 frames in 10 seconds at 15 frames per second and 286 at 30, with a frame waiting 49 ms and 15 ms on average. Acknowledging each frame when it is handed over gave the same counts and a wait of 180 ms.
- Decoding one 1264x625 PNG frame takes about 3 ms.
- A read timeout of 1, 10, 50 and 100 ms on a blocking socket returned after 8, 16, 63 and 109 ms. `thread::sleep` of 1 ms took 1.5 ms.
- Connecting to a local port nothing listens on fails with `ConnectionRefused` after 2.0 seconds.
- After a client disconnects without removing its binding, a document loaded later has no `window.__recdrClick`.
- In headless Chrome, a tab that is not selected and a tab in a window minimized through `Browser.setWindowBounds` both report `visibilityState` `hidden`.

## Architecture decisions

Each of these is a choice the spec left to the plan.

1. **One thread, a non-blocking socket.** The CDP source reads and writes the WebSocket on the recorder's thread. After the handshake the socket is switched to non-blocking, and a read that finds nothing sleeps 1 ms. A reader thread was not chosen because `tungstenite`'s socket cannot be split into a reading half and a writing half, and the measured cost of decoding on the recorder's thread is 3 ms per frame.
2. **Frame pacing.** The source keeps decoded frames in a queue and hands the recorder at most one per `1000 / fps` ms. For each frame it hands over, it acknowledges the oldest unacknowledged frame, and only when the queue is empty. This works for any number of frames the browser allows in flight. The recorder has its own rate check (`elapsed_ms - last_accepted_ms >= 1000 / fps`, in whole milliseconds, read from its own clock a moment after the source's), and a frame that fails it is discarded without being counted. A tab frame must never be discarded, so `CaptureSource` gets a method `paced()` that is true for the CDP source, and the recorder skips its check for a paced source. The measurement above used 67 ms; with `paced()` no margin is needed.
3. **A tab target travels unresolved.** `Target::Tab` and `ResolvedKind::Tab` both hold a `TabTarget` (the endpoint and the tab query). `capture::resolve` copies it, and `CdpSource::open` finds the tab after connecting. No list of tabs is fetched in the calling command.
4. **The endpoint is parsed once, in `request.rs`, into a model type** (`Endpoint`), as `parse_region` does for `Rect`. Usage errors for a bad endpoint therefore come from the calling command with exit code 2, and `capture/cdp.rs` never sees endpoint text.
5. **The relayed error is data.** `State` gets `error: Option<ErrorInfo>`. `Error::info()` builds an `ErrorInfo` (code, message, candidates), `Error::to_json()` prints it, and `Error::Relayed(ErrorInfo)` prints a stored one unchanged. `Error::code()` returns `&str` in place of `&'static str` so a relayed code can be returned.
6. **`AmbiguousTarget` holds JSON candidates and a kind.** `candidates: Vec<serde_json::Value>` and `kind: &'static str` (`"window"` or `"tab"`), so `single` in `src/capture/mod.rs` serves both.
7. **`unsupported_platform` is decided in `src/capture/mod.rs`.** `list_windows`, `list_monitors` and the desktop arm of `open` each have a `cfg(windows)` body and a `cfg(not(windows))` body that returns the error.
8. **Rule 6 is checked as "no `cfg` other than `cfg(test)`"** outside the three allowed files. This is stricter than the spec's three substrings and also catches `cfg(not(windows))`.
9. **A test seam for the `chrome` endpoint.** When the environment variable `RECDR_CHROME_USER_DATA_DIR` is set, `--cdp chrome` reads `DevToolsActivePort` from that directory. The browser tests point it at the test browser's directory, so the `chrome` path runs in CI. The spec's Testing strategy section names it.
10. **CI pins Rust 1.98**, the version on the development machine, so a newer clippy cannot fail a pull request that passes locally.

### Spec changes made while planning

The measurements changed two statements in the spec, and decision 9 added one. They are edited in the spec file and listed here for the review:

- "How a tab recording runs", the frame rate paragraph: the acknowledgement rule is now the one in decision 2. What the spec promises did not change: at most `--fps` frames per second, none discarded, `dropped_frames` always 0.
- "What was checked": facts 12 to 17 added.
- "Testing strategy": the `RECDR_CHROME_USER_DATA_DIR` sentence added.

### Layers after this module

```
cli, mcp                  front ends; they never import each other
control, request
recorder
analyze, encode
capture, store
draw, platform, time
error
model
```

`capture/cdp.rs` imports `model`, `error`, `time` and `platform` (for the Chrome user data directory). `request.rs` imports `model`, `error` and `time`, as today.

## Module contracts

A task implements these exactly as written. Anything not listed keeps its current signature.

### `Cargo.toml`

```toml
[dependencies]
tungstenite = { version = "0.30", default-features = false, features = ["handshake"] }

[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

`Cargo.lock` changes with them. No other dependency is added. Integration tests use `tungstenite`, `serde_json`, `image` and `gif` through the package's own dependencies.

### `src/model.rs`

Data only, no functions. New types derive `Debug, Clone, PartialEq, Serialize, Deserialize`, and enums use `#[serde(rename_all = "snake_case")]`, as the existing ones do.

```rust
/// Where a browser's CDP server is. `Http` holds `host:port`; `Ws` holds the whole URL.
pub enum Endpoint { Port(u16), Http(String), Ws(String), Chrome }

pub enum TabQuery { Id(String), Text(String) }

/// A tab is found by the capture source when it connects, so the query travels unresolved.
pub struct TabTarget { pub cdp: Endpoint, pub query: TabQuery }

pub struct TabInfo { pub id: String, pub title: String, pub url: String }
pub struct TabList { pub tabs: Vec<TabInfo> }

/// An error in the shape it is printed, so a detached recorder can hand it to `start`.
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub candidates: Option<Vec<serde_json::Value>>,
}
```

Changes to existing types:

- `Target` gains `Tab(TabTarget)`. `ResolvedKind` gains `Tab(TabTarget)`.
- `StopReason` gains `TabClosed`, serialized as `tab_closed`.
- `CaptureEvent` gains `Click { unix_ms: u64, x: i32, y: i32, button: &'static str }`. `x` and `y` are frame pixels relative to the captured area. `button` is `"left"`, `"middle"` or `"right"`.
- `State` gains `#[serde(skip_serializing_if = "Option::is_none", default)] pub error: Option<ErrorInfo>`. The three `State { .. }` literals (`prepare` in `src/control.rs`, `prepared` in `src/recorder.rs`, `state` in the tests of `src/store.rs`) gain `error: None`.

### `src/error.rs`

Seventeen variants: the fourteen of today, and

```rust
/// An error a detached recorder wrote to its state file, reported by `start` as it was.
#[error("{}", .0.message)]
Relayed(ErrorInfo),
#[error("window, region and monitor capture need Windows; on this system recdr records browser tabs with --cdp")]
UnsupportedPlatform,
#[error("could not connect to the browser: {0}")]
CdpConnectFailed(String),
```

Changed variants:

```rust
#[error("no target matches {0}")]
TargetNotFound(String),
#[error("{} {kind}s match {query:?}; pass --{kind}-id", candidates.len())]
AmbiguousTarget { query: String, kind: &'static str, candidates: Vec<serde_json::Value> },
```

Functions:

- `pub fn code(&self) -> &str`. `Relayed(info)` returns `&info.code`; `UnsupportedPlatform` returns `"unsupported_platform"`; `CdpConnectFailed` returns `"cdp_connect_failed"`.
- `pub fn info(&self) -> ErrorInfo`. `Relayed(info)` returns a clone of `info`. Every other variant returns its `code()`, its `to_string()`, and `candidates` only for `AmbiguousTarget`.
- `pub fn to_json(&self) -> serde_json::Value` is `json!({ "error": self.info() })`.
- `exit_code` is unchanged: 2 for `Usage`, 1 for everything else.

The test `every_variant_has_the_code_from_the_spec` lists every variant, and its count goes 14, 15 (Task 1), 16 (Task 5), 17 (Task 8).

Two things follow from these changes in other places:

- `code()` no longer returns a `'static` string, so a caller cannot keep the code after the error is dropped. One place does that today: the unit test at the end of `src/request.rs` has `let code = parse_selection(at, every, keyframes).unwrap_err().code();`. Task 1 changes it to bind the error first (`let error = ...unwrap_err();`) and compare `error.code()`.
- Until Task 10, `AmbiguousTarget.candidates` is still `Vec<WindowInfo>`, and `info()` turns each into JSON with `serde_json::json!(candidate)`. In Task 10 the two tests in `src/error.rs` that build an `AmbiguousTarget` (`ambiguous_target_lists_candidates_inside_error` and `every_variant_has_the_code_from_the_spec`) get `kind: "window"` and JSON candidates, and the `WindowInfo` import moves into the test module.

### `src/platform/`

`src/platform/mod.rs`:

```rust
//! Process and system calls that differ by operating system.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

/// Default user data directory of Chrome Stable, where `--cdp chrome` looks for `DevToolsActivePort`.
pub fn chrome_user_data_dir() -> Option<PathBuf>
```

`chrome_user_data_dir` has three `cfg` bodies: on Windows `%LOCALAPPDATA%\Google\Chrome\User Data`; on macOS (`cfg(target_os = "macos")`) `$HOME/Library/Application Support/Google/Chrome`; on other Unix `$HOME/.config/google-chrome`. It returns `None` when the environment variable is not set.

`src/platform/windows.rs` and `src/platform/unix.rs` each define the same five functions:

```rust
pub fn set_dpi_aware()
pub fn process_alive(pid: u32) -> bool
pub fn spawn_detached(exe: &Path, args: &[&OsStr], stderr: File) -> Result<u32>
/// Starts the wrapped command with its stdout going to our stderr.
pub fn spawn_command(command: &[String]) -> Result<Child>
/// Ends the command and every process it started, then waits for it.
pub fn kill_tree(child: &mut Child)
```

| Function | Windows | Unix |
|---|---|---|
| `set_dpi_aware` | as today | does nothing |
| `process_alive` | as today | false for pid 0. Then `libc::waitpid(pid, null, WNOHANG)`; when it returns `pid` the process was our own child and has exited, so false. Otherwise true when `libc::kill(pid, 0)` returns 0 or fails with `EPERM`. |
| `spawn_detached` | as today | `Command` with stdin and stdout null, stderr the file, and `pre_exec` calling `libc::setsid()` |
| `spawn_command` | `cmd /C <command>`, the code that is in `src/recorder.rs` today | `Command::new(&command[0]).args(&command[1..])` with `process_group(0)` from `std::os::unix::process::CommandExt`. No shell. |
| `kill_tree` | `taskkill /T /F /PID <id>`, the code that is in `src/recorder.rs` today, then `child.wait()` | `libc::kill(-(pid), SIGKILL)`, then `child.wait()` |

`spawn_command` returns `Error::Usage("the command is empty")` for an empty slice on both systems. The `waitpid` call is there because a detached recorder that has exited stays a zombie while the process that started it is alive (the MCP server, for example), and `kill(pid, 0)` succeeds on a zombie.

### `src/capture/hook.rs` (was `src/input.rs`)

```rust
/// Wall-clock milliseconds, virtual-screen x and y, button.
pub type RawClick = (u64, i32, i32, &'static str);

impl ClickHook {
    pub fn start() -> Result<Self>                    // unchanged
    /// The next press the hook has seen, without waiting.
    pub fn try_next(&self) -> Option<RawClick>
}
impl Drop for ClickHook                               // posts WM_QUIT, joins the thread, clears SENDER
```

`finish` is removed; its filtering moves to `WgcSource` and its time conversion to the recorder. The field `thread` becomes `Option<JoinHandle<()>>` so `Drop` can take and join it. The ignored test `only_one_hook_at_a_time` keeps its meaning: a second `start` fails with `capture_failed` while the first is alive, and succeeds after the first is dropped.

### `src/capture/mod.rs`

```rust
mod cdp;
mod fake;
#[cfg(windows)]
mod hook;
#[cfg(windows)]
mod windows;

pub fn open(resolved: &ResolvedTarget, fps: u32, cursor: bool, clicks: bool) -> Result<Box<dyn CaptureSource>>
pub fn list_windows() -> Result<Vec<WindowInfo>>
pub fn list_monitors() -> Result<Vec<MonitorInfo>>
pub fn list_tabs(endpoint: &Endpoint) -> Result<Vec<TabInfo>>
pub fn resolve(target: &Target, windows: &[WindowInfo], monitors: &[MonitorInfo]) -> Result<ResolvedTarget>

/// The one item of `found`, or `target_not_found` for none and `ambiguous_target` for several.
fn single<'a, T: Serialize>(
    found: impl Iterator<Item = &'a T>,
    query: &str,
    kind: &'static str,
    described: String,
) -> Result<&'a T>
```

- `open` matches `&resolved.kind`: `Fake` as today (the `clicks` flag is ignored); `Tab(tab)` returns `cdp::CdpSource::open(tab, fps, clicks)`; `Window` and `Monitor` call the private `desktop_source(resolved, fps, cursor, clicks)`.
- Three functions have two definitions each, one under `#[cfg(windows)]` and one under `#[cfg(not(windows))]`: `list_windows`, `list_monitors` and `desktop_source`. The Windows ones call `windows::list_windows()`, `windows::list_monitors()` and `windows::WgcSource::open(..)`. The others return `Err(Error::UnsupportedPlatform)`, and the second `desktop_source` names its four parameters with a leading underscore so the Unix build has no unused-variable warning.
- `resolve` for `Target::Tab(tab)` returns `ResolvedTarget { kind: ResolvedKind::Tab(tab.clone()), origin: (0, 0) }`.
- The window arms call `window_target(single(found, text, "window", described)?)`.
- `single` stays private. `cdp.rs` is a child module and calls it as `super::single`.
- `CaptureSource` gains one method with a default body, added in Task 11:

```rust
/// True when the source hands out frames no faster than the requested rate, so the
/// recorder stores every frame it gets.
fn paced(&self) -> bool {
    false
}
```

`CdpSource` returns true. `FakeSource` and `WgcSource` keep the default.

### `src/capture/windows.rs`

`WgcSource::open(resolved: &ResolvedTarget, fps: u32, cursor: bool, clicks: bool)`. The struct gains `hook: Option<ClickHook>` (started in `open` when `clicks` is true), `origin: (i32, i32)` (from `resolved.origin`) and `size: Option<(u32, u32)>` (the size of the first frame it handed out).

The filter that was in `ClickHook::finish` becomes a private function with a unit test:

```rust
/// A press in virtual-screen coordinates to a click event relative to the captured
/// area, or None when it is outside the area.
fn to_event(click: RawClick, origin: (i32, i32), size: (u32, u32)) -> Option<CaptureEvent>
```

`next_event` first drains the hook: while `size` is known and `hook.try_next()` returns a press, it returns the first one `to_event` accepts. Presses outside are skipped. Then it waits for a frame as today, and records `size` from the first frame. A press made before the first frame is reported after it, with its own earlier time, and the recorder discards it.

The `match resolved.kind` in `WgcSource::open` lists every variant. In Task 11 its last arm becomes `ResolvedKind::Fake(_) | ResolvedKind::Tab(_)` with the message `this target has no WGC source`, and the match is on `&resolved.kind` because `ResolvedKind` then holds a `String`.

No test sends a real mouse press: an injected press lands on whatever is under the pointer of the owner's desktop. The hook itself does not change, `to_event` and the recorder's time conversion have unit tests, and the path from a real press to the manifest is checked by hand at Checkpoint A.

### `src/capture/cdp.rs`

Public items:

```rust
pub const CHROME_DIR_ENV: &str = "RECDR_CHROME_USER_DATA_DIR";

pub fn list_tabs(endpoint: &Endpoint) -> Result<Vec<TabInfo>>

pub struct CdpSource { /* private */ }
impl CdpSource {
    pub fn open(tab: &TabTarget, fps: u32, clicks: bool) -> Result<Self>
}
impl CaptureSource for CdpSource   // dropped() returns 0
impl Drop for CdpSource
```

Constants: `CONNECT_TIMEOUT` 5 s, `FIRST_REPLY_TIMEOUT` 30 s, `CALL_TIMEOUT` 5 s, `CLOSE_TIMEOUT` 1 s, `IDLE_SLEEP` 1 ms, `MAX_MESSAGE_BYTES` `256 << 20`, `BINDING` `"__recdrClick"`, and `CLICK_LISTENER` exactly as in the spec's Code style section.

Private pure functions, each with unit tests on fixed text:

```rust
/// `DevToolsActivePort` contents (port on line 1, path on line 2) to a ws:// URL on 127.0.0.1.
fn port_file_url(text: &str) -> Option<String>
/// `host:port` of a ws:// URL; port 80 when the URL has none.
fn authority(ws_url: &str) -> Option<String>
/// Status code and Content-Length from the head of an HTTP reply. The header name is
/// matched without regard to case, and the value may follow the colon with no space.
fn reply_head(head: &str) -> Option<(u16, usize)>
/// The tabs in a `Target.getTargets` result: entries whose `type` is `page`, in order.
fn tabs(result: &serde_json::Value) -> Vec<TabInfo>
fn pick<'a>(tabs: &'a [TabInfo], query: &TabQuery) -> Result<&'a TabInfo>
/// A `Runtime.bindingCalled` payload to frame pixels and a button name.
fn click(payload: &str, frame: (u32, u32)) -> Option<(i32, i32, &'static str)>
/// A base64 PNG to an RGBA frame.
fn decode(data: &str) -> Result<Frame>
```

- `pick`: `TabQuery::Id(id)` finds the tab whose `id` equals it, or `TargetNotFound(format!("tab id {id:?}"))`. `TabQuery::Text(text)` filters tabs whose lowercased `title` or `url` contains the lowercased text and calls `super::single(found, text, "tab", format!("tab {text:?}"))`.
- `click`: the payload is `{x, y, button, w, h}` as numbers. It returns `None` when the JSON does not parse, when `w` or `h` is not above 0, or when `button` is not 0, 1 or 2. Otherwise `x` is `(x * frame.0 / w).round()` and `y` is `(y * frame.1 / h).round()`, and the button is `left`, `middle` or `right` for 0, 1, 2.
- `decode`: `base64::engine::general_purpose::STANDARD.decode`, then `image::load_from_memory_with_format(.., ImageFormat::Png)` and `to_rgba8()`. A failure is `Error::CaptureFailed("the browser sent a frame that is not a PNG")`.

Finding the WebSocket URL (`fn websocket_url(endpoint: &Endpoint) -> Result<String>`, every failure is `CdpConnectFailed`):

| Endpoint | Steps |
|---|---|
| `Port(p)` | as `Http("127.0.0.1:<p>")` |
| `Http(authority)` | TCP connect within `CONNECT_TIMEOUT`. Write `GET /json/version HTTP/1.1\r\nHost: <authority>\r\n\r\n`. With a 5 s read timeout, read until `\r\n\r\n`, pass the head to `reply_head`, require status 200, read exactly Content-Length body bytes, parse JSON, return its `webSocketDebuggerUrl`. |
| `Ws(url)` | the URL as it is |
| `Chrome` | The directory is `CHROME_DIR_ENV` when set, else `platform::chrome_user_data_dir()`. Then `chrome_url(&dir)`, a private function `fn chrome_url(dir: &Path) -> Result<String>` that reads `<dir>/DevToolsActivePort` and passes it to `port_file_url`. It takes the directory as an argument so its unit test needs no environment variable (setting one is `unsafe` in edition 2024, and `unsafe` is not allowed in this file). |

A TCP connect resolves `host:port` with `ToSocketAddrs` and tries each address with `TcpStream::connect_timeout`, all within one 5 s limit.

The text inside `CdpConnectFailed` for each failure, where `<endpoint>` is `host:port` or the ws:// URL:

| Failure | Text |
|---|---|
| `DevToolsActivePort` does not exist | `Chrome is not accepting remote debugging; switch it on at chrome://inspect/#remote-debugging (<path> was not found)` |
| `port_file_url` returns `None` | `<path> does not hold a port and a path` |
| No directory is known | `the Chrome user data directory is unknown on this system` |
| TCP connect fails or times out | `<endpoint>: <the io error>` |
| The HTTP reply is not status 200 with a Content-Length and a JSON body holding `webSocketDebuggerUrl` | `<endpoint> did not answer GET /json/version as a CDP server` |
| The WebSocket handshake fails | `<endpoint>: <the tungstenite error>` |
| The first call misses its deadline | `the browser did not answer within 30 seconds` |
| The first call fails any other way | the message of the `CaptureFailed` that `call` returned |

The connection (`struct Connection { ws: WebSocket<TcpStream>, next_id: u64, backlog: VecDeque<Incoming>, closed: bool }`):

- `Connection::open(endpoint) -> Result<(Connection, Vec<TabInfo>)>`. It finds the URL, connects TCP to `authority(url)`, sets `set_nodelay(true)` and a read timeout of `FIRST_REPLY_TIMEOUT`, and runs `tungstenite::client::client_with_config(url.as_str(), stream, Some(config))` with `WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE_BYTES)).max_frame_size(Some(MAX_MESSAGE_BYTES))`. Then `ws.get_ref().set_nonblocking(true)`. Then it calls `Target.getTargets` with a deadline of `FIRST_REPLY_TIMEOUT` counted from the end of the TCP connect, and returns `tabs(&result)`. Every failure in this function, including the first call's, is `CdpConnectFailed`; a missed deadline has the message `the browser did not answer within 30 seconds`.
- `send(&mut self, method: &str, params: Value, session: Option<&str>) -> u64` writes `{"id", "method", "params"}` plus `"sessionId"` when given, with `ws.send(Message::text(..))`. On `Error::Io` of kind `WouldBlock` it calls `ws.flush()` every `IDLE_SLEEP` until it succeeds or `CALL_TIMEOUT` passes. Any other error, or the timeout, sets `closed`.
- `poll(&mut self) -> Polled`, with `enum Polled { Message(Incoming), Idle, Closed }`. `ws.read()` returning a text message that parses gives `Message`; `Message::Close` gives `Closed`; any other message, a text message that does not parse, and `Error::Io` of kind `WouldBlock` or `TimedOut` give `Idle`; any other error sets `closed` and gives `Closed`. Once `closed` is set, `poll` returns `Closed` without reading.
- `call(&mut self, method, params, session, deadline: Instant) -> Result<Value>` sends, then polls: the reply with its id returns `result`, or `Error::CaptureFailed(format!("{method}: {message}"))` when it has `error`; a message with a `method` goes to `backlog`; a reply to another id is dropped; `Idle` sleeps `IDLE_SLEEP`, and past the deadline is `CaptureFailed(format!("{method} was not answered"))`; `Closed` is `CaptureFailed("the browser closed the connection")`.

```rust
#[derive(Deserialize)]
struct Incoming {
    id: Option<u64>,
    method: Option<String>,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(default)]
    result: serde_json::Value,
    error: Option<ReplyError>,          // struct ReplyError { message: String }
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}
```

`list_tabs` is `Connection::open(endpoint)` and returns the tabs.

Items arrive in the task that first uses them, so every commit passes clippy with no `allow`:

- Task 8 defines `Incoming` with `id`, `method`, `result` and `error`. Task 11 adds `params` and `session_id`.
- Task 11 defines `pick`, `decode` and `CdpSource` with the fields `conn`, `session`, `interval`, `held`, `unacked` and `release_at`, and names the third parameter of `open` `_clicks`. Task 14 renames it to `clicks` and adds `click`, `CLICK_LISTENER`, `BINDING` and the fields `frame_size`, `clicks` and `click_script`.

`CdpSource::open`:

1. `Connection::open(&tab.cdp)`, then `pick(&tabs, &tab.query)`.
2. `Target.attachToTarget {targetId, flatten: true}` and keep `sessionId`. From here the `CdpSource` value exists, so a failure in a later step drops it and the clean-up below runs.
3. `Emulation.setFocusEmulationEnabled {enabled: true}`, then `Page.enable`.
4. When `clicks` is true: `Runtime.addBinding {name: BINDING}`, `Page.addScriptToEvaluateOnNewDocument {source: CLICK_LISTENER}` (keep the returned `identifier`), `Runtime.evaluate {expression: CLICK_LISTENER}`.
5. `Page.startScreencast {format: "png"}`.

Steps 2 to 5 use `call` with a deadline of `CALL_TIMEOUT` each and the tab's session id.

State of a `CdpSource`: the connection, the session id, `interval` (`1000 / fps` ms), `held: VecDeque<Frame>`, `unacked: VecDeque<i64>`, `release_at: Instant`, `frame_size: Option<(u32, u32)>` (size of the latest frame received), `clicks: VecDeque<CaptureEvent>`, and `click_script: Option<String>`.

`next_event(timeout)` loops until it has something to return:

1. A queued click is returned first.
2. When `held` is not empty and `release_at` has passed: pop the front frame; when `held` is now empty and `unacked` has an id, pop the oldest and `send` `Page.screencastFrameAck {sessionId: id}`; set `release_at` to now plus `interval`; return the frame.
3. Take the next message from `backlog`, or `poll`. `Closed` returns `CaptureEvent::Closed`. `Idle` returns `Ok(None)` when `timeout` has passed and sleeps `IDLE_SLEEP` otherwise.
4. A message is handled by its `method`. A message whose `session_id` is not the tab's session is ignored, except `Target.detachedFromTarget`, which the browser sends outside the session and which names the session in its `params`.

| Method | Action |
|---|---|
| `Page.screencastFrame` | `decode(params.data)`, set `frame_size`, push the frame to `held` and `params.sessionId` to `unacked` |
| `Page.frameNavigated` with no `params.frame.parentId`, when `click_script` is set | `send` `Runtime.addBinding {name: BINDING}` |
| `Runtime.bindingCalled` with `params.name == BINDING`, when `frame_size` is known | `click(params.payload, frame_size)`; on `Some`, queue `CaptureEvent::Click` with `unix_ms` from `time::now_unix_ms()` |
| `Target.detachedFromTarget` with `params.sessionId` equal to the tab's session | set `closed`; return `CaptureEvent::Closed` |
| anything else | ignored |

`Drop`: when the connection is not closed, `call` in this order, each with a deadline of `CLOSE_TIMEOUT` and its error ignored: `Page.stopScreencast`; `Emulation.setFocusEmulationEnabled {enabled: false}`; and when `click_script` is set, `Runtime.removeBinding {name: BINDING}` and `Page.removeScriptToEvaluateOnNewDocument {identifier}`. Then `ws.close(None)`, error ignored.

### `src/request.rs`

`RecordRequest` gains `pub cdp: Option<String>`, `pub tab_id: Option<String>`, `pub tab: Option<String>`.

```rust
/// A port, an http:// URL, a ws:// URL, or `chrome`.
pub fn parse_endpoint(text: &str) -> Result<Endpoint>
```

Rules, in order, each failure an `Error::Usage`:

1. `chrome` is `Endpoint::Chrome`.
2. Text made only of ASCII digits must parse to a `u16` above 0 and is `Endpoint::Port`. Otherwise: `<text> is not a port between 1 and 65535`.
3. After `http://`: remove one trailing `/`; the rest must be non-empty and hold no `/`; add `:80` when it has no `:`; the result is `Endpoint::Http`. Otherwise: `<text> is not an http:// endpoint; use http://host:port`.
4. After `ws://`: the part before the first `/` must be non-empty; the whole text is `Endpoint::Ws`. Otherwise: `<text> is not a ws:// endpoint; use ws://host:port/path`.
5. Text starting with `https://` or `wss://` fails with `recdr does not connect over TLS; use an http:// or ws:// endpoint`.
6. Anything else fails with `<text> is not a CDP endpoint; use a port, an http:// URL, a ws:// URL, or chrome`.

`into_options`: `cdp` is parsed first when given. `tab_id` and `tab` count as targets next to the five desktop flags, and the count must be 1, else `give exactly one of --window-id, --window, --process, --region, --monitor, --tab-id, --tab`. A tab flag without `cdp` fails with `--tab-id and --tab need --cdp`. `cdp` with a desktop flag fails with `--cdp goes with --tab-id or --tab`. A tab flag with `cdp` gives `Target::Tab(TabTarget { cdp, query })`. The `RECDR_FAKE_SOURCE` override stays after these checks.

### `src/control.rs`

```rust
pub fn list_tabs(endpoint: &Endpoint) -> Result<TabList>
```

- Constants: `START_TIMEOUT` 5 s stays; `TAB_START_TIMEOUT` is 40 s. `fn start_timeout(target: &Target) -> Duration` returns the second for `Target::Tab` and the first otherwise. `start` and `derived_status` use it in place of `START_TIMEOUT`.
- `prepare`: `Target::Tab(_)` lists nothing, like `Target::Fake`.
- A private `fn run_noting_error(dir: &RecordingDir) -> Result<RecordingResult>` calls `recorder::run(dir)`, and when it fails reads the state, sets `error` to `Some(e.info())` and writes the state. A failure to read or write there is ignored. Both `record` and `run_recorder` call it in place of `recorder::run`; `run_recorder` still prints the error to stderr. `record` needs it too: a tab is found inside the recorder, after `prepare` has made the directory, so without it a failed `recdr record --cdp ...` would leave a directory that `status` calls `recording` for 40 seconds.
- The doc comment on `prepare` becomes: "Resolves a desktop target and writes the initial state. Nothing is created on disk when a desktop target cannot be resolved; a tab is resolved later, by the recorder."
- `start`, each turn of its loop, in this order: read whether the recorder is alive and whether the time limit has passed; read the state; a state with `error` returns `Error::Relayed`; a state that is `Recording` or `Finished` returns the `StartInfo`; then a recorder that was not alive, or a passed limit, returns `RecorderStartFailed(dir.last_log_line())`. Checking the process before reading the state means an error written just before the recorder exited is seen.
- `derived_status`: after the result check, a state with `error` is `Crashed`.

### `src/recorder.rs`

- `capture::open(&state.resolved, options.fps, options.cursor, options.clicks)`.
- The import of `ClickHook`, the `cmd` spawn and the `taskkill` call are gone. The wrapped command is `platform::spawn_command(command)?`, and the kill is `platform::kill_tree(&mut child)`.
- The first-frame loop ignores `CaptureEvent::Click`.
- The main loop collects `CaptureEvent::Click { unix_ms, x, y, button }` into a list. After the loop each becomes a `Click` with `t_ms = unix_ms.checked_sub(state.started_at_unix_ms)?`, kept when `t_ms <= duration_ms`, the rule marks already follow.
- `CaptureEvent::Closed` gives `StopReason::TabClosed` when `state.resolved.kind` is `ResolvedKind::Tab(_)`, and `WindowClosed` otherwise.
- The frame arm of the main loop becomes `Some(CaptureEvent::Frame(frame)) if source.paced() || elapsed_ms - last_accepted_ms >= interval_ms` (Task 11).
- Imports: Task 3 removes `ResolvedKind` from the `use crate::model::{..}` list, because the hook condition was its only use; Task 11 adds it back for `TabClosed`. In `src/capture/hook.rs`, Task 3 removes `use crate::model::Click;` and rewrites the comment in `hook_thread` that names `finish` to name `Drop`.

A press in the last poll of a recording (at most 50 ms) can be missed, because the source reports clicks only when asked for the next event. Today the hook is drained once at the end.

### `src/cli.rs`

- `Command::Tabs(TabsCmd)`, help text "List the tabs of a browser", with `#[arg(long)] cdp: String`. It runs `request::parse_endpoint(&cmd.cdp).and_then(|endpoint| control::list_tabs(&endpoint))`.
- `TargetFlags` gains `cdp: Option<String>` ("Browser to attach to: a port, an http:// or ws:// URL, or chrome"), `tab_id: Option<String>` ("Tab id from `recdr tabs`") and `tab: Option<String>` ("Tab whose title or URL contains this text, ignoring case"). `into_request` passes them on.

### `src/mcp.rs`

- A ninth tool `list_tabs` with `struct TabsParams { cdp: String }` ("A port, an http:// or ws:// URL, or chrome"). Description: `List the tabs of a browser reached through CDP, with the id to pass as tab_id.`
- `start_recording` description: `Start recording a window, region, monitor or browser tab in the background and return its id. For a window run list_windows and pass window_id. For a tab run list_tabs and pass cdp and tab_id.`
- `record` description: `Record for a fixed duration such as 5s and return the files and a contact sheet. For a window run list_windows and pass window_id. For a tab run list_tabs and pass cdp and tab_id.`

### `tests/layers.rs`

Six tests. `source_files` and `forbid` are unchanged.

| Rule | Check |
|---|---|
| 1 | unchanged |
| 2 | `clap` only in `src/cli.rs`; `rmcp` and `tokio` only in `src/mcp.rs`; `base64` only in `src/mcp.rs` and `src/capture/cdp.rs`; `tungstenite` only in `src/capture/cdp.rs` |
| 3 | `unsafe` only in `src/capture/windows.rs`, `src/capture/hook.rs`, `src/platform/windows.rs`, `src/platform/unix.rs` |
| 4 | unchanged, without `"input::"` in the list |
| 5 | unchanged |
| 6 | For every file except `src/capture/mod.rs`, `src/platform/mod.rs` and `src/lib.rs`: after removing every `cfg(test)`, the text contains none of `cfg(`, `cfg!(`, `cfg_attr(` |

### `tests/browser.rs`

Every test is `#[ignore]`. Helpers in this file:

```rust
struct Browser { child: Child, dir: PathBuf, port: u16 }
impl Browser {
    /// Headless Chrome with a fresh user data directory and these extra arguments.
    fn start(name: &str, extra: &[&str]) -> Browser
    fn client(&self) -> Client
}
impl Drop for Browser                      // kill, wait, remove the directory

/// The test's own CDP connection, separate from recdr's.
struct Client { /* a blocking tungstenite socket */ }
impl Client {
    fn call(&mut self, method: &str, params: Value, session: Option<&str>) -> Value   // panics on an error reply
    fn tab_ids(&mut self) -> Vec<String>
    fn attach(&mut self, tab: &str) -> String
    fn eval(&mut self, session: &str, expression: &str) -> Value
    fn navigate(&mut self, session: &str, url: &str)
    fn press(&mut self, session: &str, x: i32, y: i32)
}

/// Serves the three test pages on 127.0.0.1, by path, and returns the port.
fn serve() -> u16
```

Helpers arrive in the task whose tests first use them, so no commit has unused test code: Task 9 adds `Browser`, `Client::call` and `Client::tab_ids`; Task 12 adds `serve`, the pages, `attach`, `eval` and `navigate`; Task 14 adds `press`.

- Chrome's path is the environment variable `RECDR_TEST_CHROME`, else `C:\Program Files\Google\Chrome\Application\chrome.exe` on Windows, `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome` on macOS, `/usr/bin/google-chrome` on Linux.
- Arguments: `--headless=new --remote-debugging-port=0 --user-data-dir=<dir> --window-size=1280,720 --no-first-run --no-default-browser-check`, the extra ones, `about:blank`, and `--no-sandbox` when the environment variable `CI` is set.
- `start` waits up to 20 s for `<dir>/DevToolsActivePort` to hold two lines, and reads the port from line 1.
- `Client` uses a blocking socket with a 10 s read timeout, and `call` skips events.
- `press` sends `Input.dispatchMouseEvent` twice, `mousePressed` then `mouseReleased`, with `button: "left"` and `clickCount: 1`.
- Pages, each a string constant of HTML: `ANIMATION` (white page, a red 200x200 box whose `top` is fixed at 400 px and whose `left` moves 4 px per animation frame, title `anim`), `STILL` (white page, title `still`, one heading), `GREEN` (title `green`, background `#00aa00`).
- `serve` answers `GET /anim`, `/still` and `/green` with those pages and anything else with 404, each reply with `Content-Type: text/html`, a `Content-Length` and `Connection: close`. It runs on a thread for the life of the test process.
- A test opens a page with `Client::navigate(session, "http://127.0.0.1:<port>/anim")`. Navigation tests go from `http://127.0.0.1:<port>/anim` to `http://localhost:<port>/green`: a different site and a different page, with no internet needed.
- The MCP `Server` helper moves from `tests/mcp.rs` to `tests/common/mod.rs` in Task 15 with one change: `pub fn start(fake: Option<&str>)` sets `RECDR_FAKE_SOURCE` only for `Some`, and its methods become `pub`. `tests/mcp.rs` passes `Some("moving")`; the browser test passes `None`, because the variable replaces any target, a tab included.

### `.github/workflows/ci.yml`

One job `test` with `strategy.fail-fast: false` and a matrix of `windows-latest`, `ubuntu-latest`, `macos-latest`. Triggers: `pull_request`, and `push` to `main`. Steps:

1. `actions/checkout@v4`
2. `rustup toolchain install 1.98 --profile minimal --component rustfmt --component clippy` and `rustup default 1.98`
3. `cargo fmt --check`
4. `cargo clippy --all-targets -- -D warnings`
5. `cargo test`
6. `cargo test --test browser -- --ignored` (added in Task 16)

No ffmpeg is installed: the MP4 test skips itself when ffmpeg is missing.

## File boundary

Create:
- `src/capture/cdp.rs`
- `src/capture/hook.rs` (moved from `src/input.rs` with `git mv`)
- `src/platform/mod.rs`
- `src/platform/windows.rs` (moved from `src/platform.rs` with `git mv`)
- `src/platform/unix.rs`
- `tests/tabs.rs`
- `tests/browser.rs`
- `.github/workflows/ci.yml`
- `tasks/3-cdp-capture/acceptance.md`

Modify:
- `Cargo.toml`
- `Cargo.lock`
- `README.md`
- `src/lib.rs`
- `src/model.rs`
- `src/error.rs`
- `src/store.rs`
- `src/request.rs`
- `src/control.rs`
- `src/recorder.rs`
- `src/cli.rs`
- `src/mcp.rs`
- `src/capture/mod.rs`
- `src/capture/windows.rs`
- `tests/common/mod.rs`
- `tests/control.rs`
- `tests/record.rs`
- `tests/desktop.rs`
- `tests/mcp.rs`
- `tests/layers.rs`

Delete:
- `src/input.rs` (becomes `src/capture/hook.rs`)
- `src/platform.rs` (becomes `src/platform/windows.rs`)

Files under `spec/` and `tasks/` may always be edited. `src/main.rs`, `src/capture/fake.rs`, `src/time.rs`, `src/draw.rs`, `src/analyze/`, `src/encode/`, `tests/cli.rs` and `tests/frames.rs` are not touched.

## Task list

Details, acceptance and verification for each task are in [todo.md](todo.md).

### Phase 1: changes to the first module, on Windows
- Task 1: `start` reports the recorder's own error
- Task 2: the mouse hook moves under `capture`
- Task 3: clicks arrive as capture events
- Task 4: process code behind `platform/`

### Checkpoint A: the first module still works
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` pass
- `cargo test --test desktop -- --ignored` passes
- By hand, by the owner: a click during `recdr record --monitor 0 --duration 5s` is in `manifest.json` and is a ring in the GIF (success criterion 19)

### Phase 2: Linux and macOS
- Task 5: recdr compiles off Windows
- Task 6: the tests and the six layer rules run on every system
- Task 7: CI workflow

### Checkpoint B: three systems (needs a push)
- The owner pushes `feat/3-cdp-capture` and opens a draft pull request; the build stops and asks for this
- The `test` job is green on all three runners

### Phase 3: tabs
- Task 8: CDP connection and tab list
- Task 9: the `tabs` command and the browser test harness

### Checkpoint C: recdr talks to Chrome
- `cargo test --test browser -- --ignored` passes on Windows
- `recdr tabs --cdp <port>` by hand against a Chrome started with `--remote-debugging-port=9222 --user-data-dir=<dir>` prints its tabs

### Phase 4: tab recording
- Task 10: one matcher for windows and tabs
- Task 11: the screencast source
- Task 12: `--cdp`, `--tab-id` and `--tab` on `record` and `start`
- Task 13: hidden tabs, closed tabs, navigation and a second client

### Checkpoint D: a tab records
- All tests and browser tests pass on Windows
- By hand: open the GIF of a 3 second recording of an animating tab and see the animation

### Phase 5: the rest
- Task 14: clicks from the page
- Task 15: MCP `list_tabs` and tab arguments
- Task 16: browser tests in CI and the Unix wrapped command
- Task 17: README and the acceptance run

### Checkpoint E: complete (needs a push)
- CI is green on the pull request on all three runners, browser tests included
- `tasks/3-cdp-capture/acceptance.md` has a result for each of the 21 success criteria
- The owner runs criteria 14 and 15 by hand

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| The Unix code cannot be run on the Windows development machine. | High | Task 5 type-checks it with `cargo check --target`, and Checkpoint B runs it on CI before any CDP code depends on it. |
| `cargo check --target x86_64-unknown-linux-gnu` may need a C compiler for the target if a dependency's build script wants one. This was not tried. | Low | If it fails for that reason, Task 5 skips the cross check and Task 7's CI run is the check. |
| The first module's tests have never run on a hosted Windows runner. | Med | Checkpoint B is where that shows. A failure there is fixed in the task that owns the test file, before Phase 3. |
| A hosted runner may not have Chrome at the default path. | Med | `RECDR_TEST_CHROME` overrides the path, and Task 16 sets it in the workflow if the default is wrong. |
| A tab frame is stored about 50 ms after the browser sent it, while clicks and marks use the time they arrive. A ring can appear up to about three frames before the page's reaction at 15 frames per second. | Low | Measured, and written in the README. The pacing rule in decision 2 cut this from 180 ms. |
| A press in the last 50 ms of a recording can be missed (see `src/recorder.rs` above). | Low | Written in the README's limits. A ring that late would show for less than one frame. |
| On Unix, a wrapped command that does not exist fails the recording with `io_error` after the first frame, where Windows reports `command_exited` with code 1 through `cmd`. | Low | Written in the README. The spec says the command is started directly on Unix. |
| A renderer crash may send no `Target.detachedFromTarget`. | Med | Task 13 has a probe test with `chrome://crash` that accepts `tab_closed` or `duration_reached` and records which one happened. Acting on a new event is an ask-first change to the spec. |
| A recorder that is killed cannot run its clean-up. | Low | Measured: a document loaded after the client disconnects has no binding. Whether focus emulation ends with the session was not measured; the README says to reload the tab after a killed recorder. |
| Moving the hook changes working code, and no automated test sends a real mouse press. | Med | Tasks 2 and 3 are separate commits. The moved filter (`to_event`) and the recorder's time conversion have unit tests, and the owner checks one real click by hand at Checkpoint A. |

## Open questions

None block the build. Three things need the owner at a point in it:

1. Checkpoints B and E need the branch pushed and a pull request opened. Its base is `feat/1-agent-recorder` while pull request #2 is open, and `main` after it merges.
2. Task 5 runs `rustup target add x86_64-unknown-linux-gnu aarch64-apple-darwin` on the development machine. Approving this plan approves that.
3. Success criteria 14 and 15 are checked by hand on the owner's machine.
