# recdr

recdr records a window, a screen region or a monitor on Windows, or a browser tab through the Chrome DevTools Protocol on Windows, Linux and macOS, and leaves files an AI agent can read: a GIF or MP4 for people, and for the agent a contact sheet of the moments where the screen changed, a per-frame change log and the stored frames as PNG. Agents drive it through a CLI that prints JSON, or through an MCP server.

It exists because browser tools give an agent screenshots, not recordings. With recdr an agent can start a recording, run a test or click through a flow, stop, and look at what happened.

## Build

Requires Rust 1.98 or later. Desktop targets (windows, regions, monitors) need Windows 10 1903 or later (Windows 11 recommended). Tab targets run on Windows, Linux and macOS and need a Chromium-based browser with remote debugging switched on.

```
cargo build --release
```

The binary is `target/release/recdr` (`recdr.exe` on Windows). MP4 output also needs `ffmpeg` on `PATH`.

On Linux and macOS, `windows`, `monitors` and the desktop targets fail with `unsupported_platform`. A command after `--` runs without a shell there, so write `-- sh -c "npm test && echo done"` when you need one.

## Commands

```
recdr windows                       List visible top-level windows
recdr monitors                      List monitors
recdr tabs --cdp <endpoint>         List the tabs of a browser
recdr record <target> --duration 10s [options]     Record for a fixed time, then return
recdr record <target> [options] -- <command...>    Record while the command runs
recdr start  <target> [options]     Start a background recording, return its id at once
recdr stop   <id>                   Stop it, encode, return the result
recdr mark   <id> "<label>"         Add a labelled marker at the current time
recdr status [<id>]                 State of one recording, or of all in the output root
recdr frames <id> (--at 1200ms,2s | --every 500ms | --keyframes) [--max-width 1024]
recdr mcp                           Run the MCP server on stdio
```

Every command prints one JSON object on stdout and exits 0. A failure prints `{"error":{"code":"...","message":"..."}}` on stderr and exits 1, or 2 for a usage error.

`<target>` is exactly one of:

| Flag | Meaning |
|---|---|
| `--window-id <id>` | The `id` from `recdr windows`. Exact; prefer this. |
| `--window "<text>"` | The visible window whose title contains the text, ignoring case. |
| `--process <name>` | The visible window of a process with this executable name, such as `chrome.exe`. |
| `--region x,y,w,h` | A rectangle in physical pixels that lies inside one monitor. |
| `--monitor <index>` | A whole monitor, by the `index` from `recdr monitors`, starting at 0. |
| `--tab-id <id>` with `--cdp <endpoint>` | The `id` from `recdr tabs`. Exact; prefer this. |
| `--tab "<text>"` with `--cdp <endpoint>` | The tab whose title or URL contains the text, ignoring case. |

A window target is resolved once, when recording starts. A tab is found when the recorder connects to the browser. When `--window`, `--process` or `--tab` matches more than one window or tab, recdr fails with `ambiguous_target` and lists the candidates instead of guessing. To record a window that the wrapped command will open, use `--monitor` or `--region`.

Options for `record` and `start`:

| Option | Default | Meaning |
|---|---|---|
| `--duration <time>` | none | `record` only. Stop after this long. |
| `--fps <n>` | 15 | Capture rate limit, 1 to 60. |
| `--max-duration <time>` | 5m | The recording stops itself at this length. |
| `--max-size <size>` | 2GB | The recording stops itself when its directory reaches this size. |
| `--format <list>` | gif | `gif`, `mp4`, or `gif,mp4`. |
| `--no-cursor` | off | Leave the mouse cursor out of the frames. A tab recording never shows the cursor. |
| `--no-click-highlight` | off | Do not record clicks. No mouse hook is installed and nothing is added to the page. |

`--out <dir>` (default `./.recdr`) sets the output root for every command that reads or writes recordings. A `<time>` is a whole number with `ms`, `s` or `m`; a `<size>` is a whole number with `MB` or `GB`.

### Examples

Record a window for five seconds:

```
recdr windows
recdr record --window-id 70628 --duration 5s
```

Record a monitor while a test suite runs:

```
recdr record --monitor 0 -- npm test
```

Start in the background, mark a moment, stop:

```
recdr start --window-id 70628
recdr mark 20261118-143502-1a2b "clicked submit"
recdr stop 20261118-143502-1a2b
```

## Record a browser tab

recdr attaches to a running Chromium-based browser over the Chrome DevTools Protocol and records one tab's page as the browser draws it, whether or not the tab is selected, covered or in a minimized window. Nothing on screen changes.

List the tabs, then record one:

```
recdr tabs --cdp 9222
recdr record --cdp 9222 --tab-id 6A1B5C... --duration 5s
recdr record --cdp 9222 --tab "checkout" --duration 5s
```

`--cdp` takes four forms:

| Form | Example | Fits |
|---|---|---|
| A port | `9222` | A browser started with `--remote-debugging-port=9222 --user-data-dir=<dir>`, such as one that Playwright MCP attaches to with `--cdp-endpoint`, or a headless Chrome in a script or CI job. |
| An `http://` URL | `http://127.0.0.1:9222` | The same, on another address. recdr asks `<url>/json/version` for the WebSocket URL. |
| A `ws://` URL | `ws://127.0.0.1:9222/devtools/browser/4f0c...` | Used as it is. agent-browser prints one with `get cdp-url`. |
| `chrome` | `chrome` | Your everyday Chrome 144 or later, after you switch on remote debugging at `chrome://inspect/#remote-debugging`. Chrome asks you to Allow each connection, so `tabs` is one click and `record` or `start` is another. |

`https://` and `wss://` endpoints are refused.

For Chrome Beta, Canary, Edge or a profile in another directory, build the `ws://` URL from the `DevToolsActivePort` file in that browser's user data directory. The browser writes it when remote debugging is switched on at `chrome://inspect/#remote-debugging` or when it was started with `--remote-debugging-port=0` (a headless Chrome given a fixed port did not write it in testing). Its first line is the port and its second a path:

```
$ cat "<user data dir>/DevToolsActivePort"
53122
/devtools/browser/0e2d5f44-...
$ recdr tabs --cdp ws://127.0.0.1:53122/devtools/browser/0e2d5f44-...
```

Since Chrome 136, `--remote-debugging-port` is ignored for the default profile, so a browser started that way needs its own `--user-data-dir`.

Clicks in a tab come from the page: recdr adds a listener for mouse presses to the top-level document, so clicks that an automation tool sends through CDP get rings too. The manifest stores them in frame pixels.

### Limits of tab recording

- While it is recorded, the page sees itself as visible and focused, so a page that pauses when hidden keeps running.
- `window.__recdrClick` and `window.__recdrListening` are added to the page, so a page can tell it is recorded. `--no-click-highlight` adds nothing.
- Clicks inside iframes are not reported. A click made after the viewport changed size may be drawn in the wrong place.
- A press made just before the recording stops, or in the moment between a navigation and the listener being added again, can be missed.
- A frame is stored up to one frame interval (about 67 ms at 15 fps) after the browser sent it.
- If a recorder is killed, its listener stays in the open document; reload the tab.

## Stop reasons

`stop_reason` in the result is one of: `stopped` (`recdr stop`), `duration_reached` (`--duration`), `cap_reached` (`--max-duration`), `size_cap_reached` (`--max-size`), `command_exited` (the wrapped command ended; `command_exit_code` holds its code), `window_closed` (the recorded window closed) and `tab_closed` (the recorded tab closed or the browser went away).

## Error codes

| Code | Meaning |
|---|---|
| `usage` | Bad flags or arguments. Exit code 2. |
| `target_not_found` | No window, monitor or tab matches. |
| `ambiguous_target` | More than one matches; `candidates` lists them. |
| `target_minimized` | The window is minimized and has nothing to capture. |
| `region_out_of_bounds` | `--region` is not inside one monitor. |
| `ffmpeg_not_found` | `--format mp4` without `ffmpeg` on `PATH`. |
| `recorder_start_failed` | The background recorder exited without saying why, or did not report its first frame in time. |
| `recording_not_found` | No recording with that id under the output root. |
| `recording_not_active` | `stop` or `mark` on a recording that has ended. |
| `recorder_crashed` | The background recorder exited without finishing. |
| `too_many_frames` | `get_frames` would return more than 8 frames. |
| `capture_failed` | The capture source failed. |
| `encode_failed` | Writing the GIF or MP4 failed. |
| `unsupported_platform` | `windows`, `monitors` or a desktop target on Linux or macOS. |
| `cdp_connect_failed` | The browser cannot be reached, does not speak CDP, or did not answer within 30 seconds. |
| `io_error` | A file or process operation failed. |

## Tests

```
cargo test
cargo test --test desktop -- --ignored                       # Windows; records the real screen
cargo test --test browser -- --ignored --test-threads=1      # starts headless Chrome
```

The browser tests find Chrome at its default install path; set `RECDR_TEST_CHROME` to use another binary.

## What a recording leaves

A recording lives in `<out>/<id>/`:

| File | Contents |
|---|---|
| `frames/000001.png`, ... | Each frame that differs from the one before it. |
| `manifest.json` | Size, fps, duration, each frame's time, marks, clicks, stop reason, dropped frames. |
| `diff.json` | For each frame: the share of pixels that changed, the box around them, and whether it is a keyframe. |
| `contact_sheet.png` | Up to 12 keyframes in a grid, each labelled with its time and any marks. |
| `recording.gif`, `recording.mp4` | The formats asked for, with a ring drawn for 400 ms after each click. |
| `result.json` | What `stop` and `record` print. |

`recdr` writes `<out>/.gitignore` containing `*`, so recordings are not committed by accident.

## MCP

Register the server with Claude Code:

```
claude mcp add recdr -- /path/to/recdr mcp
```

Tools: `list_windows`, `list_monitors`, `list_tabs`, `start_recording`, `stop_recording`, `record`, `mark`, `get_status` and `get_frames`. Each returns the same JSON the CLI prints. `stop_recording` and `record` also return the contact sheet as an image, and `get_frames` returns up to 8 frames as images.

## Limits

- On Windows 10 the system draws a yellow border around the captured window or monitor, and it cannot be turned off there. Windows 11 turns it off.
- Clicks in windows of elevated (administrator) processes are not seen by the mouse hook, so they get no ring. Security software may flag a program that hooks the mouse and captures the screen; `--no-click-highlight` installs no hook.
- Protected video and some full-screen exclusive applications capture as black.
- When a recorded window is resized, the first frame at the new size is discarded by the capture library. A window that then never repaints keeps showing its old image in the recording. Frames of another size are scaled to fit the first frame's size, on black.
- A recording holds whatever was on screen, including secrets. recdr never uploads anything; recordings stay in the output root.
- Keyboard input is never recorded.

## License

MIT
