# Spec: cdp-capture

Module `cdp-capture` of the [recdr capability map](../1-recdr/capability-map.md). It is the second module and depends on `agent-recorder`. Its tracking issue is https://github.com/dzikrisyairozi/recdr/issues/3, a sub-issue of #1, and the directory name `3-cdp-capture` carries that number.

All paths in this document are relative to the repository root. "The plan" means `tasks/3-cdp-capture/plan.md`, which is written after this spec is approved. "The first spec" means [spec/1-recdr/spec-agent-recorder.md](../1-recdr/spec-agent-recorder.md); anything this document does not mention behaves as the first spec says.

At the time of writing, `agent-recorder` is on branch `feat/1-agent-recorder` (pull request #2) and is not merged. This module is built on top of that branch.

## Words used here

- **CDP**: the Chrome DevTools Protocol, the JSON protocol over a WebSocket that Chrome, Edge and other Chromium browsers offer to debuggers and automation tools.
- **Tab**: a CDP target whose `type` is `page`. The browser also reports targets of other types (service workers, its own interface); those are not tabs.
- **Endpoint**: the value of `--cdp`, which tells recdr where the browser's CDP server is.
- **Screencast**: the CDP feature (`Page.startScreencast`) that sends an image of a tab each time the tab draws.
- **Hidden tab**: a tab Chrome has stopped drawing. Chrome does this to every tab that is not the selected tab of its window, and to every tab of a window that is minimized or fully covered by other windows.
- **Focus emulation**: the CDP setting `Emulation.setFocusEmulationEnabled`, which DevTools shows as "Emulate a focused page". While it is on for a tab, Chrome draws that tab as if it were selected and in front, without changing anything on screen.
- **Desktop target**: a window, region or monitor target from the first spec.
- **Detached recorder**: the second `recdr` process that `recdr start` launches to do the recording, described under "Control model" in the first spec. `recdr record` runs the same recorder code inside the calling process.

## Objective

`agent-recorder` records what Windows shows on a monitor. It cannot record a browser that has no window (headless Chrome in a test script or in CI), a tab that is not in front, or anything on Linux or macOS.

`cdp-capture` adds a second kind of target, a browser tab, recorded through CDP. recdr attaches to a browser that is already running, as a second client next to whatever tool is driving it, and receives the tab's frames from the browser itself. The tab may be in the background, in a minimized or covered window, or in a browser with no window at all. The recording shows only the page.

Everything after capture is unchanged: the frame store, the GIF and MP4 encoders, the contact sheet, `diff.json`, `mark`, `frames`, `status`, `stop` and the MCP server work on a tab recording exactly as on a window recording.

The owner also chose, on 2026-11-18, to make recdr build and run on Linux and macOS for tab targets in this module, so that it works in Linux CI and on the owner's Mac. This is not free. Capture through CDP needs no Windows API, but the recorder around it does: starting the detached recorder, checking that it is alive, starting and ending a wrapped command, and the mouse hook. This module therefore splits that code by system, adds Unix versions, and adds CI on three systems. Desktop targets stay Windows-only until `macos-capture`.

### Browsers this must work with

The owner asked for all four on 2026-11-18. The last column is the `--cdp` form each one uses (see Commands).

| Browser | How it is reached | `--cdp` form |
|---|---|---|
| A Chrome that Playwright MCP is attached to. [Playwright MCP](https://github.com/microsoft/playwright-mcp) is Microsoft's browser automation server for agents; its `--cdp-endpoint` option attaches it to a Chrome the user started with `--remote-debugging-port=9222 --user-data-dir=<dir>`. | recdr attaches to the same port. | port or `http://` |
| The browser of [agent-browser](https://github.com/vercel-labs/agent-browser), a browser automation CLI and MCP server from Vercel Labs. Its `get cdp-url` command (MCP tool `agent_browser_get_cdp_url`) prints the CDP URL of its browser. | The printed URL is passed to `--cdp` as it is. Whether it is an `http://` or a `ws://` URL was not checked; recdr accepts both. | `http://` or `ws://` |
| Headless Chrome started by a script or a CI job with `--remote-debugging-port`. | The script knows the port. | port |
| The owner's everyday Chrome, version 144 or later, with remote debugging switched on at `chrome://inspect/#remote-debugging`. | recdr finds it through a file Chrome writes. | `chrome` |

### User stories

1. As an agent driving a page with Playwright MCP, I list the browser's tabs, start recording the one I am testing, click through a flow, stop, and get a GIF and a contact sheet of the page only, even though my terminal covers the browser window.
2. As a test script in CI on Linux, I start headless Chrome, start a recording of its tab, run my steps and stop, on a machine with no display.
3. As a human, I ask an agent to record a tab of the Chrome I am using, click Allow once in Chrome's dialog, and get a recording without restarting my browser.
4. As an agent, I see where my clicks landed as rings in the GIF, although a tab recording has no mouse cursor.
5. As the owner, I run the same commands on my Mac.

## Tech stack

Additions to the first spec's table. Versions and licenses were read with `cargo info` on 2026-11-18.

| Piece | Choice | License |
|---|---|---|
| WebSocket client | `tungstenite` 0.30, blocking, built without its TLS features | MIT OR Apache-2.0 |
| Unix process calls (`kill`, `setsid`) | `libc` 0.2, only under `cfg(unix)` | MIT OR Apache-2.0 |
| Base64 of screencast frames | `base64`, already a dependency | MIT OR Apache-2.0 |
| CDP discovery over HTTP | One `GET /json/version` written against `std::net::TcpStream`. No HTTP crate. | |
| CI | GitHub Actions, hosted runners `windows-latest`, `ubuntu-latest`, `macos-latest` | |

No async code is added. `tokio` stays inside `src/mcp.rs`.

Supported browsers: Chrome, Edge and Chromium. The `chrome` endpoint needs version 144 or later. Firefox and Safari do not speak CDP.

### What was checked

Read on 2026-11-18:

- Since Chrome 136, `--remote-debugging-port` is ignored unless `--user-data-dir` names a directory other than the default one ([Chrome blog](https://developer.chrome.com/blog/remote-debugging-port)).
- Since Chrome 144, the switch at `chrome://inspect/#remote-debugging` lets a client attach to the running default profile, and Chrome shows an Allow dialog for each connection ([Chrome blog](https://developer.chrome.com/blog/chrome-devtools-mcp-debug-your-browser-session)). The HTTP discovery routes answer 404 in this mode, and the client finds the WebSocket URL in the `DevToolsActivePort` file (`src/BrowserManager.ts` of `ChromeDevTools/chrome-devtools-mcp`, and a maintainer's comment on its issue 2283).

Measured on 2026-11-18 with a throwaway script against Chrome 158.0.8127.36 on Windows 11, headless and with a window:

1. A Chrome started with `--remote-debugging-port=0` writes `DevToolsActivePort` into its user data directory: line 1 is the port, line 2 is a path, and `ws://127.0.0.1:<port><path>` equals the `webSocketDebuggerUrl` that `GET /json/version` returns.
2. `Target.getTargets` reports tabs as type `page` and the browser's own interface as type `browser_ui`.
3. A screencast frame in PNG format is `innerWidth × devicePixelRatio` by `innerHeight × devicePixelRatio` pixels (1264x625 at scale factor 1 and 2528x1250 at scale factor 2 for the same window).
4. A page that is not changing still gets one frame when the screencast starts.
5. Chrome sends 60 PNG frames per second for an animating page when each is acknowledged at once, at both sizes above.
6. A hidden tab draws nothing: a background tab, a tab in a minimized window, and a tab in a window covered by other windows each reported `visibilityState` `hidden`, ran no animation frames, and sent no frames after the first.
7. With focus emulation on, each of those three reported `visible` and sent 57 to 60 frames per second. The window stayed minimized, the other tab stayed selected, and turning the setting off made the tab hidden again.
8. A screencast keeps running when the tab navigates to another site.
9. A binding added with `Runtime.addBinding` exists in the document that is open when it is added and in no later document, unless `Runtime.enable` was sent first or the binding is added again after each navigation. Both were tried and both work.
10. When a tab is closed, a client attached to it receives `Target.detachedFromTarget`. When the browser process ends, the WebSocket closes.
11. A second client attached to the same tab can send it input and navigate it while the first client runs a screencast.

Measured on 2026-11-18 while planning, with a throwaway Rust program using `tungstenite` 0.30 against the same Chrome, headless:

12. Chrome answers `GET /json/version` with a `Content-Length` header that has no space after the colon, and does not close the connection, also when the request asks it to with `Connection: close`.
13. When no frame is acknowledged, Chrome sends three frames and stops.
14. Handing over one frame per 67 ms, with the acknowledgement rule under "How a tab recording runs", gave 147 frames in 10 seconds at 15 frames per second and 286 at 30, and a frame waited 49 ms and 15 ms on average before it was handed over. Acknowledging each frame at the moment it is handed over gave the same counts with a wait of 180 ms.
15. Decoding one 1264x625 PNG frame takes about 3 ms.
16. After a client disconnects without removing its binding, a document loaded later has no `window.__recdrClick`.
17. Connecting to a local port nothing listens on fails after 2 seconds on Windows.

Not checked: what a client receives when a tab's renderer crashes; the `chrome` endpoint on a real default profile; whether focus emulation ends when a client disconnects without turning it off; Linux and macOS.

## Commands

Development commands are those of the first spec, with the ignored tests split by what they need:

```
Desktop tests: cargo test --test desktop -- --ignored
Browser tests: cargo test --test browser -- --ignored
```

New product command:

```
recdr tabs --cdp <endpoint>         List the tabs of a browser
```

New targets for `record` and `start`. A tab target is one of these two flags, and it needs `--cdp`:

| Flag | Meaning |
|---|---|
| `--tab-id <id>` | The `id` printed by `recdr tabs`, which is the CDP target id. It does not change while the tab exists. Exact. The MCP tool descriptions tell agents to use this. |
| `--tab "<text>"` | The tab whose title or URL contains the text, compared without regard to case. More than one match fails with `ambiguous_target` and lists the candidates. |

`--cdp <endpoint>` takes one of four forms:

| Form | Example | How recdr finds the browser's WebSocket URL |
|---|---|---|
| A port number | `9222` | `GET http://127.0.0.1:9222/json/version` |
| An `http://` URL | `http://127.0.0.1:9222` | `GET <url>/json/version` |
| A `ws://` URL | `ws://127.0.0.1:9222/devtools/browser/4f0c...` | Used as it is |
| The word `chrome` | `chrome` | Reads `DevToolsActivePort` in the default user data directory of Chrome Stable: `%LOCALAPPDATA%\Google\Chrome\User Data` on Windows, `~/Library/Application Support/Google/Chrome` on macOS, `~/.config/google-chrome` on Linux |

The target rule of the first spec becomes: exactly one of `--window-id`, `--window`, `--process`, `--region`, `--monitor`, `--tab-id`, `--tab`. `--cdp` with a desktop target, a tab target without `--cdp`, and an `https://` or `wss://` endpoint are usage errors.

Every other option of `record` and `start` applies to a tab target unchanged, with one exception: `--no-cursor` is accepted and does nothing, because a screencast never contains the cursor.

## Behavior

### How a tab recording runs

The command the user ran checks only the flags. Everything that touches the browser happens in the recorder, over one WebSocket connection:

1. It finds the WebSocket URL as the table above says and connects.
2. It asks for the list of targets and picks the tab. No match is `target_not_found`; more than one match for `--tab` is `ambiguous_target`.
3. It attaches to the tab. Attaching does not detach other clients.
4. It turns focus emulation on for the tab, so the tab draws even when it is hidden. Nothing on screen changes: recdr does not select the tab or raise its window.
5. It starts a screencast in PNG format, and stores each frame as the first spec describes.
6. When the recording ends, for any reason, it stops the screencast, turns focus emulation off and removes what it added for clicks.

Frame rate: the browser stops sending frames while three are unacknowledged (measured fact 13). recdr hands the recorder at most one frame per 1/n seconds, where n is `--fps`. When it has handed over every frame it received, it acknowledges all of them; it acknowledges nothing while a frame is still waiting. The browser therefore sends at most n frames per second, recdr discards none, and a frame reaches the recorder about 50 ms after the browser sent it (measured fact 14). Frames are decoded on their own thread and stored on the frame writer's thread, so the recorder keeps reading the browser, timing clicks and checking for the end while a frame is stored. When the writer is still busy with one frame and two more arrive, the older of the two is skipped and counted in `dropped_frames`; on a fast machine that count is 0, and a slow one records at a lower frame rate.

The recording's size is the size of the first frame, which is the tab's viewport in device pixels (measured fact 3). When the viewport changes size during a recording, later frames are scaled and padded as the first spec describes for a resized window.

A page that is not changing gives a recording with one frame (measured fact 4). Navigating the tab, including to another site, does not end the recording.

Time in a tab recording is recdr's own clock, read when each frame arrives, as for desktop targets. The timestamps the browser attaches to frames are not used.

While a tab is being recorded, its page sees itself as visible and focused, because of focus emulation. A page that pauses when it is hidden does not pause during the recording.

### Start-up errors from a detached recorder

Because the recorder now finds the target itself, `recdr start` must be able to report why a detached recorder could not begin. The rule, for every kind of target:

- A detached recorder that fails before it starts recording writes its error to its state file. `start` reports that error with the same code and message `record` would have printed.
- `recorder_start_failed` is left for a recorder that exits without writing an error, or that does not report within the time limit.
- The recording directory of a failed start stays on disk with its log, and `status` lists it as `crashed`, as today.

The time limit is 5 seconds for a desktop target, as today. For a tab target it is 40 seconds: the recorder allows 30 seconds for the browser to answer its first command, which covers a person clicking Allow, and the rest is for the first frame. A browser that shows no dialog answers in milliseconds, so `start` still returns at once.

Connecting has its own limit: if the endpoint cannot be reached within 5 seconds, or does not answer the first command within 30, the error is `cdp_connect_failed`. The limits are the same for all four endpoint forms.

### Clicks

A tab recording has no mouse cursor, and the Windows mouse hook cannot see clicks that an automation tool sends through CDP. So for a tab target, clicks come from the page:

- recdr registers a CDP binding named `__recdrClick` and installs a script in the tab that listens for `mousedown` on the top-level document, in the capture phase, and reports each press's position and button through the binding.
- The script is registered to run in each new document, and recdr adds the binding again each time the tab's main frame navigates (measured fact 9). A press in the few milliseconds between a navigation and the binding being added again is missed.
- Positions arrive in CSS pixels relative to the viewport. recdr multiplies x by frame width over viewport width, and y by frame height over viewport height, using the size of the latest frame. The manifest stores clicks in frame pixels, as in the first spec.
- Buttons 0, 1 and 2 are stored as `left`, `middle` and `right`. Other buttons are ignored.
- Rings are drawn at encode time and on the contact sheet, never into stored frames, as in the first spec.
- When the recording ends, recdr removes the binding and the new-document script. The listener stays in the document that is open at that moment and does nothing; it is gone after the next navigation.
- `--no-click-highlight` installs nothing in the page. The manifest then has no clicks.

Limits: clicks inside an `<iframe>` are not reported; a click made after the viewport changed size during a recording may be drawn in the wrong place; and a page can detect the recording by looking for `window.__recdrClick`.

Only mouse buttons are reported. recdr never listens for keyboard events.

### What recdr sends to and reads from a browser

This is the whole list. Adding to either table is a change to this spec.

| Method | Purpose |
|---|---|
| `Target.getTargets` | List tabs |
| `Target.attachToTarget` (with `flatten: true`) | Open a session on the tab |
| `Emulation.setFocusEmulationEnabled` | Make a hidden tab draw; turned off at the end |
| `Page.enable`, `Page.startScreencast`, `Page.screencastFrameAck`, `Page.stopScreencast` | Frames |
| `Runtime.addBinding`, `Runtime.removeBinding` | Click reports |
| `Runtime.evaluate` | Install the click listener in the open document. Nothing is read back. |
| `Page.addScriptToEvaluateOnNewDocument`, `Page.removeScriptToEvaluateOnNewDocument` | Keep the click listener across navigations |

| Event | Meaning to recdr |
|---|---|
| `Page.screencastFrame` | A frame |
| `Page.frameNavigated` for the main frame | Add the click binding again |
| `Page.loadEventFired` | Restart the screencast. Acks sent while a navigation is pending can be held or lost, and the browser sends no frames while three are unacked (measured fact 13), so a page that paints once after a navigation would never arrive |
| `Runtime.bindingCalled` | A click report |
| `Target.detachedFromTarget` for the recorded tab | The tab is gone: stop with `tab_closed` |
| The WebSocket closing | The browser is gone: stop with `tab_closed` |

`Runtime.enable` is not sent, so the browser does not send recdr the page's console output or exceptions. `Page.enable` makes the browser send page lifecycle events, which carry URLs; recdr ignores all of them except `Page.frameNavigated`, from which it uses only whether the frame is the main frame, and `Page.loadEventFired`, from which it uses only that it arrived. recdr reads no DOM content, cookies, storage or network traffic, and sends no input to the page.

### The `chrome` endpoint

With `--cdp chrome`, recdr reads the `DevToolsActivePort` file and connects to the URL it gives. Two things differ from the other forms:

- Chrome shows an Allow dialog for each connection, so a person must be at the machine. Each recdr command makes one connection: `tabs` is one dialog, and `start` or `record` is one dialog.
- When the file does not exist, the error message says to switch on remote debugging at `chrome://inspect/#remote-debugging`.

Chrome Beta, Chrome Canary, Edge and a non-default user data directory are reached by passing the `ws://` URL built from their own `DevToolsActivePort` file. The README shows how. The same Allow dialog and the same time limits apply.

### Output of the new command

| Command | Fields |
|---|---|
| `tabs` | `tabs`: list of `{id, title, url}`, in the order the browser reports them |

`start`, `stop`, `record`, `mark`, `status` and `frames` print what the first spec says.

### Error codes

Two codes are added. The others keep their meaning; three get a wider one.

| Code | When |
|---|---|
| `cdp_connect_failed` | New. The endpoint cannot be reached, it does not answer as a CDP server, `DevToolsActivePort` is missing or malformed, the connection was refused, or the first command was not answered in time. |
| `unsupported_platform` | New. `windows`, `monitors`, or a desktop target, on Linux or macOS. |
| `target_not_found` | Also: no tab has the given id, or no tab's title or URL contains the given text. |
| `ambiguous_target` | Also: `--tab` matches more than one tab. `candidates` then lists tabs as `{id, title, url}`. |
| `recorder_start_failed` | Narrower: see "Start-up errors from a detached recorder". |

A lost connection during a recording is not an error; see the next section.

### Stop reasons

One value is added:

| Value | Cause |
|---|---|
| `tab_closed` | The recorded tab was closed, or the connection to the browser ended because the browser exited or the network dropped. Frames captured so far are encoded. |

`window_closed` keeps its meaning and is never used for a tab.

### MCP tools

`recdr mcp` exposes nine tools: the eight of the first spec and `list_tabs`.

| Tool | CLI equivalent | Returns |
|---|---|---|
| `list_tabs` | `tabs` | JSON |

`start_recording` and `record` accept `cdp`, `tab_id` and `tab`. Their descriptions tell the agent to run `list_tabs` and pass `tab_id`.

### Linux and macOS

On Linux and macOS, recdr builds and every command works for tab targets:

- `tabs`, `record`, `start`, `stop`, `mark`, `status`, `frames` and `mcp` behave as on Windows.
- `windows`, `monitors` and desktop targets fail with `unsupported_platform`.
- In wrap-a-command form the command is started directly, without a shell. On Windows it is still started through `cmd /C`, as today. When a recording ends before the command does, recdr ends the command and the processes it started, by force: with `taskkill /T /F` on Windows as today, and by sending `SIGKILL` to the command's process group on Linux and macOS.
- The detached recorder is started in its own session, so it survives the terminal that started it.

No display is needed. recdr itself opens no window on any system.

### Changes to existing behavior

- The target rule lists seven flags instead of five (see Commands).
- `recdr mcp` lists nine tools instead of eight.
- The `ambiguous_target` error can carry tab candidates.
- `start` reports a detached recorder's own start-up error. Today every such failure is `recorder_start_failed` with the recorder's last log line as the message; for example a capture API error is now `capture_failed` from `start`, as it already is from `record`.
- Desktop clicks reach the recorder through the capture source (see Project structure). What ends up in the manifest does not change.

Nothing else in the first spec changes.

## Project structure

New and changed files. Files not listed are untouched.

```
src/capture/cdp.rs        New. CDP connection, endpoint discovery, tab list, tab choice, the screencast source
src/capture/hook.rs       Was src/input.rs. The Windows mouse hook, now owned by the Windows capture source
src/capture/mod.rs        Changed. Tab arm in resolve and open; desktop back end behind cfg(windows)
src/capture/windows.rs    Changed. Starts the hook when clicks are on and reports clicks as events
src/platform/mod.rs       Was src/platform.rs. Picks the implementation for the system
src/platform/windows.rs   The Win32 code that is in platform.rs today, plus starting and ending a wrapped command
src/platform/unix.rs      New. The same functions for Linux and macOS
src/model.rs              Changed. Tab target, tab info, the new stop reason, the click event
src/error.rs              Changed. Two new variants; AmbiguousTarget can hold tabs; a variant for an error read back from a detached recorder
src/store.rs              Changed. The start-up error in the state file
src/request.rs            Changed. cdp, tab_id, tab
src/control.rs            Changed. list_tabs; start reports the recorder's error; the longer wait for a tab target
src/recorder.rs           Changed. No longer names the hook or cmd or taskkill; takes clicks from capture events
src/cli.rs                Changed. tabs command, three flags
src/mcp.rs                Changed. list_tabs tool
tests/tabs.rs             New. Tab commands that need no browser: usage errors, an unreachable endpoint, unsupported_platform
tests/browser.rs          New. #[ignore] tests against a real headless Chrome
tests/desktop.rs          Changed. Compiled only on Windows
tests/layers.rs           Changed. The rules below
.github/workflows/ci.yml  New
README.md                 Changed. Tab recording, the four browsers, Linux and macOS
spec/3-cdp-capture/       This spec
tasks/3-cdp-capture/      Plan and task list
```

The layer table loses `input`, which becomes part of `capture`:

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

`CaptureSource` gets its third implementation, and clicks get one path for every source. `CaptureEvent` gains a `Click` variant that carries the position, the button and the wall-clock time at which the source saw the press. The recorder converts that time to recording time the way it already does for marks, and no longer knows where clicks come from: the Windows source owns the mouse hook, the CDP source owns the page listener, and the synthetic source reports none.

Choosing one tab from a list by text is the same operation as choosing one window by title, which `single` in `src/capture/mod.rs` does today for `WindowInfo`. That function is made to work for both; a second matcher is not written.

The layer rules become:

- Only `store.rs` names a file inside a recording directory. (unchanged)
- Only `cli.rs` uses `clap`; only `mcp.rs` uses `rmcp` and `tokio`; only `mcp.rs` and `capture/cdp.rs` use `base64`; only `capture/cdp.rs` uses `tungstenite`.
- `unsafe` appears only in `capture/windows.rs`, `capture/hook.rs`, `platform/windows.rs` and `platform/unix.rs`.
- The front ends import only `control`, `request`, `model` and `error`. (unchanged)
- `model.rs` holds data types and no functions. (unchanged)
- New: under `src/`, `cfg(windows)`, `cfg(unix)` and `cfg(target_os` appear only in `capture/mod.rs`, `platform/mod.rs` and `lib.rs`. System differences are decided in those three files and nowhere else.

## Code style

As in the first spec. Two additions:

- A CDP message is built with `serde_json::json!` and read with `serde` structs that name only the fields recdr uses. Unknown fields and unknown events are ignored, so a newer browser does not break recdr.
- The click listener is one string constant in `capture/cdp.rs`, plain JavaScript with no build step. This sample shows the style; "Clicks" above is what it must do.

```rust
/// Runs in the recorded tab. Reports top-level mouse presses through the binding;
/// does nothing in iframes, or once the binding is gone.
const CLICK_LISTENER: &str = r#"
(() => {
  if (window !== window.top || window.__recdrListening) return;
  window.__recdrListening = true;
  addEventListener('mousedown', (e) => {
    if (typeof window.__recdrClick !== 'function') return;
    window.__recdrClick(JSON.stringify({
      x: e.clientX, y: e.clientY, button: e.button,
      w: innerWidth, h: innerHeight,
    }));
  }, true);
})();
"#;
```

## Testing strategy

| Level | Covers | Needs |
|---|---|---|
| Unit | Endpoint forms, `DevToolsActivePort` parsing, choosing a tab and ambiguity, click scaling from CSS pixels to frame pixels, reading CDP messages from fixed JSON text | Nothing |
| Integration, no browser (`tests/tabs.rs`, `tests/mcp.rs`) | Usage errors for the new flags, `cdp_connect_failed` for a port nothing listens on, `unsupported_platform`, the nine MCP tools, `start` reporting a detached recorder's own error (with the synthetic source) | Nothing. Runs on all three systems in `cargo test` |
| Browser, marked `#[ignore]` (`tests/browser.rs`) | Every success criterion that names a test browser | Chrome. Its path comes from the environment variable `RECDR_TEST_CHROME`, or the default install location |
| Desktop, marked `#[ignore]` | As in the first spec. No test sends a real mouse press, because an injected press lands on whatever is under the pointer; the click path through the capture source is covered by unit tests of its filter and its time conversion, and by one click by hand (success criterion 19) | Windows desktop |
| By hand | The `chrome` endpoint; Playwright MCP; agent-browser | The owner's machine |

There is no fake CDP server. Everything after capture is already tested without a browser through the synthetic source of the first spec, and the protocol handling is tested against the real thing.

A browser test starts Chrome with `--headless=new --remote-debugging-port=0` and a temporary `--user-data-dir`, and learns the port by reading the `DevToolsActivePort` file Chrome writes there, which exercises the parser the `chrome` endpoint uses. It drives the page through its own CDP connection, which also shows that a second client does not disturb the recording.

For tests only: when the environment variable `RECDR_CHROME_USER_DATA_DIR` is set, the `chrome` endpoint reads `DevToolsActivePort` from that directory in place of the default one. A browser test sets it to the test browser's directory, so the `chrome` form runs without the owner's Chrome.

CI runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and the browser tests on the three hosted runners, on each pull request and on each push to `main`. Hosted runners come with Chrome installed.

## Boundaries

The first spec's boundaries stay in force. Additions:

Always:
- Turn focus emulation off, and remove the binding and the new-document script, when a recording ends, including when it ends because of an error.
- Keep the three CI jobs green before merging.

Ask first:
- Sending a CDP method, or acting on a CDP event, that is not in the tables under "What recdr sends to and reads from a browser".
- Selecting a tab, raising a window, or changing anything else the person can see in their browser.
- Adding TLS (`wss://`, `https://`) or any dependency that brings a TLS library.

Never:
- Read page content: DOM, cookies, storage, network traffic, console output.
- Send input to a page.
- Listen for keyboard events in a page.
- Start, restart or close a browser.
- Write to the browser's user data directory. `DevToolsActivePort` is only read.

The first spec's rule "never send a recording or a frame over the network" holds: frames travel from the browser to recdr, and recdr sends them nowhere.

## Non-goals

- Starting a browser. recdr attaches to one that is running.
- Waiting for a tab or a browser that does not exist yet. In wrap-a-command form the wrapped command therefore cannot be the thing that starts the browser; start the browser first, or call `recdr start` from the script.
- Recording part of a tab, an element, or the full scrollable page.
- Recording several tabs in one recording, or following a click into a new tab.
- A mouse cursor in tab recordings.
- Clicks inside iframes, and correct click positions after the viewport changes size.
- A way to record a tab without focus emulation.
- `wss://` and `https://` endpoints, and with them hosted browser services.
- Firefox and Safari.
- Finding Chrome Beta, Canary, Edge or a custom profile by name. They work through a `ws://` URL.
- Keeping one browser connection open across recdr commands.
- Desktop capture on Linux and macOS. Module `macos-capture`; Linux desktop capture is not on the map.
- Audio.

## Alternatives considered

- **Record the browser window with `agent-recorder`.** Works today for a visible window on Windows. It lost for this module's cases: a headless browser has no window, Chrome stops drawing a covered window (measured fact 6), and it needs Windows.
- **Refuse a hidden tab** with an error that tells the caller to bring it to the front. This was the first draft. It lost when the measurement showed that focus emulation makes a hidden tab draw without changing the screen, and that a browser window behind the agent's own terminal is the usual case, not a rare one.
- **Bring the tab to the front** (`Page.bringToFront`). It lost because it changes what the person sees and can take keyboard focus from what they are typing in.
- **`Page.captureScreenshot` in a loop** instead of a screencast. It lost because each screenshot forces a full redraw, and a screencast already sends 60 frames per second (measured fact 5).
- **`Page.startScreenRecording`**, a newer CDP command that writes a video in the browser. It lost because the agent view needs individual frames with times, and the method is experimental and absent from older browsers.
- **An async CDP crate** (`chromiumoxide`, `headless_chrome`). They do far more than recdr needs, and bring an async runtime or a browser launcher into the capture layer. A blocking WebSocket and the methods listed above are enough.
- **Find the tab in the command the user ran, then connect again in the recorder.** This keeps a tab target resolved in the same place as a window target, and needs no way to pass an error back from a detached recorder. It lost because it costs a second connection, which on the owner's everyday Chrome is a second Allow dialog for each recording, and because the second connection can still fail in ways `start` would have reported as `recorder_start_failed`.
- **`Runtime.enable`** to make the click binding survive navigation, in place of adding it again. One call fewer. It lost because it makes the browser send recdr every console message and exception of the page, which recdr has no reason to receive.
- **JPEG screencast frames.** Smaller. PNG was chosen so stored frames are exact and text in them stays sharp for the agent; the measurement shows PNG is not the limit.
- **A fake CDP server for tests.** It would let every tab path run in `cargo test` without Chrome. It lost because it is a second implementation of the protocol written from the same reading as the client, and every path it covers needs a browser test anyway.
- **Reuse `window_closed`** for a closed tab. It lost because an agent reading `window_closed` for a recording that never had a window would be misled.
- **Leave Linux and macOS to `macos-capture`.** Smaller module. The owner chose to include them here.

## Risks

| Risk | Mitigation |
|---|---|
| The `chrome` endpoint rests on behavior Chrome does not document: the `DevToolsActivePort` file and the dialog flow were read from `chrome-devtools-mcp`, and were not run against a real default profile. | It is one of four endpoint forms and shares all code after the URL is known. Success criterion 14 is checked by hand on the owner's Chrome. If it breaks, the `ws://` form is the workaround. |
| It is not known whether Chrome holds the WebSocket handshake until the person clicks Allow, or accepts it and delays the first reply. | The 30-second limit covers connecting and the first reply together, so either order works. |
| Focus emulation changes what the page under test sees: it reports itself visible and focused while recorded. | Documented in the README. Playwright turns the same setting on for every page it drives, so for those pages nothing changes. |
| recdr decodes each PNG frame and the store encodes it again, which may not hold 15 frames per second at large sizes even though the browser can send 60. | Success criterion 4 measures it at about 1280x720. The frame rate degrades, and nothing is lost or wrong, when recdr is slow. |
| The measurements were made on Chrome 158 on Windows only. | The browser tests repeat each one, and CI runs them on Linux and macOS. |
| A renderer crash may not produce `Target.detachedFromTarget`, leaving a recording that gets no more frames. | `--max-duration` ends it. The plan tests a crash with `chrome://crash`; if a different event arrives, it is added to the event table. |
| The click listener changes the page under test by adding two properties to `window`. | Documented in the README. `--no-click-highlight` installs nothing. |
| Chrome on `ubuntu-latest` may refuse to start in its sandbox. | The browser tests pass `--no-sandbox` when the environment variable `CI` is set. |
| The Linux and macOS code cannot be run on the development machine, which is Windows. | CI runs the full test set on both. The owner has a Mac for a check by hand. |
| Moving the mouse hook into the capture source changes working code from the first module, and no automated test sends a real mouse press. | The move and the new click path are separate commits, the moved filter has a unit test, and the owner checks one real click by hand after the move. |

## Success criteria

Checked on Windows 11 with Chrome Stable unless stated. "A test browser" is a headless Chrome started by the test with `--headless=new --remote-debugging-port=0 --window-size=1280,720` and a temporary `--user-data-dir`.

1. `recdr tabs --cdp <port>` on a test browser with two tabs prints both with `id`, `title` and `url`, and prints no target of another type.
2. `recdr record --cdp <port> --tab-id <id> --duration 3s` exits 0 and prints a result whose `gif` is a GIF between 2.5 and 3.5 seconds long, and whose `width` and `height` equal the tab's `innerWidth` and `innerHeight` multiplied by its `devicePixelRatio`. This holds for a test browser started with `--force-device-scale-factor=2` as well.
3. A tab showing a page that does not change, recorded for 3 seconds, gives a result with `frames_stored` of 1 or 2 and a `contact_sheet.png`.
4. A tab showing a continuous animation, recorded for 10 seconds with `--fps 15`, has `frames_stored` of at least 100 and `dropped_frames` of 0.
5. `--tab "<text>"` finds a tab by a part of its title and by a part of its URL, in any letter case. Text that matches two tabs gives `ambiguous_target` whose `candidates` lists both as `{id, title, url}`. Text or an id that matches none gives `target_not_found`. Each exits 1, and each gives the same code from `recdr start` as from `recdr record`.
6. A tab that is not the selected tab, and a tab whose window was minimized through CDP, each showing a continuous animation and recorded for 3 seconds with `--fps 15`, give `frames_stored` of at least 20. During the recording the other tab is still the selected one, and after it the recorded tab reports `visibilityState` `hidden` again.
7. Closing the recorded tab ends the recording with `stop_reason` `tab_closed` and a playable GIF. Ending the browser process does the same.
8. Navigating the recorded tab to a page on another origin does not end the recording, and a frame stored after the navigation shows the new page.
9. A left mouse press sent to the tab at CSS position (100, 50) through `Input.dispatchMouseEvent` by another CDP client appears in the manifest as a click with button `left`, at (100, 50) at scale factor 1 and at (200, 100) at scale factor 2, at a time within 200 ms of when it was sent, and as a ring in the GIF. The stored PNG frames contain no ring. A press sent 1 second after a navigation to another origin is recorded too.
10. With `--no-click-highlight`, `typeof window.__recdrClick` in the tab is `"undefined"` during the recording and the manifest has no clicks. Without the flag, it is `"undefined"` in a document loaded after the recording ended.
11. `--cdp` naming a port nothing listens on gives `cdp_connect_failed` within 6 seconds. `--tab` without `--cdp`, `--cdp` with `--monitor`, and `--cdp wss://example.test` each give `usage` with exit code 2.
12. On a tab, `recdr start` returns an id in under 2 seconds, `recdr mark` puts its label in the manifest, `recdr stop` returns `stop_reason` `stopped`, and `recdr frames <id> --at 1s` prints a frame path.
13. While another CDP client is attached to the recorded tab and sends it commands, the recording completes and every command of the other client succeeds.
14. With the owner's Chrome 144 or later and remote debugging switched on, `recdr tabs --cdp chrome` lists its tabs after one Allow click, and `recdr record --cdp chrome --tab-id <id> --duration 3s` produces a GIF after one Allow click. With remote debugging switched off, both fail with `cdp_connect_failed` and a message that names `chrome://inspect/#remote-debugging`. Checked by hand.
15. With Playwright MCP attached to a Chrome through `--cdp-endpoint`, and again with agent-browser, an agent records a tab while the tool drives it and the agent's terminal covers the browser window, and the GIF shows the actions. Checked by hand.
16. `recdr mcp` lists nine tools. `record` called with `cdp` and `tab_id` returns the contact sheet as an image.
17. On `ubuntu-latest` and `macos-latest`, `cargo test` and the browser tests pass; `recdr windows`, `recdr monitors` and `recdr record --monitor 0 --duration 1s` fail with `unsupported_platform` and exit 1; and `recdr record --cdp <port> --tab-id <id> -- sh -c "echo hi; sleep 2"` gives `stop_reason` `command_exited`, `command_exit_code` 0 and `hi` on stderr.
18. `recdr start` with the synthetic source set to `broken` fails with `capture_failed`, the code `recdr record` gives for it.
19. On Windows, every other test of `agent-recorder` still passes, including the desktop tests, and a real click during a monitor recording is still in the manifest.
20. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass on all three systems, and the CI workflow is green on the pull request.
21. `tests/layers.rs` enforces the six rules under Project structure.

## Decisions made while writing

The owner answered four questions on 2026-11-18: a sub-issue of #1; all four browsers; clicks through a listener in the page; Linux and macOS included. The rest were decided by the author and can be overturned at review:

1. recdr turns on focus emulation for the recorded tab, so any tab can be recorded and nothing on screen changes. The page sees itself as visible while recorded.
2. The recorder finds the tab itself, over one connection, and `start` reports its error. This changes one behavior of the first module (success criterion 18).
3. The mouse hook moves into the Windows capture source, so clicks have one path.
4. Screencast frames are PNG, paced by acknowledgement to `--fps`.
5. A closed tab is `tab_closed`, not `window_closed`.
6. No TLS endpoints.
7. `--cdp` alone is not a target, even when the browser has one tab.
8. Only Chrome Stable is found by name.
9. No fake CDP server; the protocol is tested against Chrome.

## Open questions

None that block the plan.
