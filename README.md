<div align="center">

<img src="https://raw.githubusercontent.com/dzikrisyairozi/recdr/main/assets/logo.svg" width="100" alt="recdr">

<h1>recdr</h1>

<a href="#install">install</a> · <a href="#quick-start">quick start</a> · <a href="#commands">docs</a>

English · <a href="README.zh-CN.md">简体中文</a>

[![license](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![downloads](https://img.shields.io/github/downloads/dzikrisyairozi/recdr/total)](https://github.com/dzikrisyairozi/recdr/releases)
[![stars](https://img.shields.io/github/stars/dzikrisyairozi/recdr)](https://github.com/dzikrisyairozi/recdr/stargazers)
[![release](https://img.shields.io/github/v/release/dzikrisyairozi/recdr)](https://github.com/dzikrisyairozi/recdr/releases/latest)
[![homebrew](https://img.shields.io/github/v/release/dzikrisyairozi/recdr?label=homebrew)](https://github.com/dzikrisyairozi/homebrew-tap)
[![crates.io](https://img.shields.io/crates/v/recdr)](https://crates.io/crates/recdr)

</div>

recdr records a window, a screen region or a monitor on Windows and macOS, or a browser tab through the Chrome DevTools Protocol on Windows, Linux and macOS, and leaves files an AI agent can read: a GIF or MP4 for people, and for the agent a contact sheet of the moments where the screen changed, a per-frame change log and the stored frames as PNG. Agents drive it through a CLI that prints JSON, or through an MCP server.

It exists because browser tools give an agent screenshots, not recordings. With recdr an agent can start a recording, run a test or click through a flow, stop, and look at what happened.

## Install

macOS and Linux:

```
curl -fsSL https://recdr.dev/install.sh | sh
```

Windows (PowerShell):

```
irm https://recdr.dev/install.ps1 | iex
```

The script downloads the build for your system from the latest GitHub release, checks its SHA-256, and puts `recdr` in `~/.local/bin`, or `recdr.exe` in `%LOCALAPPDATA%\Programs\recdr` on Windows, which it adds to your user `Path`. It ends by printing the command that registers the MCP server.

With Homebrew on macOS or Linux, or with cargo:

```
brew install dzikrisyairozi/tap/recdr
cargo install recdr
```

`recdr update` installs the newest release over the binary it runs from, with the same script. `recdr update --check` only reports whether one exists. A copy installed by Homebrew or cargo is refused with `managed_install`; update it with `brew upgrade recdr` or `cargo install recdr`.

Both scripts read three environment variables:

| Variable | Default | Meaning |
|---|---|---|
| `RECDR_VERSION` | the newest | Install this version, as `0.1.0` or `v0.1.0`. |
| `RECDR_INSTALL_DIR` | see above | Install into this folder. On Windows a folder set here is not added to `Path`. |
| `RECDR_RELEASE_BASE` | `https://github.com/dzikrisyairozi/recdr/releases` | Read releases from another address with the same layout. `recdr update` reads it too. |

```
curl -fsSL https://recdr.dev/install.sh | RECDR_VERSION=0.1.0 sh
$env:RECDR_VERSION = '0.1.0'; irm https://recdr.dev/install.ps1 | iex
```

To uninstall, delete the binary. On Windows also remove `%LOCALAPPDATA%\Programs\recdr` from your user `Path`.

The Windows binary is not signed, so Defender or SmartScreen may warn about it. recdr installs a low-level mouse hook while it records, to draw click rings; `--no-click-highlight` installs none.

## Quick start

Register the MCP server with Claude Code:

```
claude mcp add recdr -- recdr mcp
```

Then ask the agent to record something, for example: "record the Chrome tab with the checkout page while you click Buy, then show me the contact sheet". The same recording from the command line, for 10 seconds:

```
recdr record --cdp chrome --tab checkout --duration 10s
```

`--cdp chrome` needs remote debugging switched on at `chrome://inspect/#remote-debugging`; see [Record a browser tab](#record-a-browser-tab).

## Build

For contributors, or to run recdr from a checkout.

Requires Rust 1.98 or later. Desktop targets (windows, regions, monitors) need Windows 10 1903 or later (Windows 11 recommended) or macOS 14 or later. Building on macOS needs Xcode's command line tools (`xcode-select --install`); full Xcode is not needed. Tab targets run on Windows, Linux and macOS and need a Chromium-based browser with remote debugging switched on.

```
cargo build --release
```

The binary is `target/release/recdr` (`recdr.exe` on Windows).

MP4 output uses the H.264 encoder of the system on Windows (Media Foundation) and macOS (AVFoundation), and `ffmpeg` on `PATH` on Linux. Windows "N" editions have no Media Foundation encoder until the Media Feature Pack is installed. On Windows every MP4 is at least 160 by 120 (smaller recordings get black columns and rows) and carries a silent AAC track, both from the `windows-capture` encoder.

On Linux, `windows`, `monitors` and the desktop targets fail with `unsupported_platform`. A command after `--` runs without a shell there, so write `-- sh -c "npm test && echo done"` when you need one.

### macOS permissions

macOS asks for two permissions, and gives them to the program that started recdr: your terminal (Terminal, iTerm2) or your editor, not recdr itself. A recording started with `start`, and one started by the MCP server, counts as that same program.

- **Screen Recording** (System Settings > Privacy & Security > Screen & System Audio Recording), for `windows`, `monitors` and every desktop target. Without it recdr asks macOS to show its dialog and fails with `permission_denied`. Switch the program on and restart it.
- **Input Monitoring** (System Settings > Privacy & Security > Input Monitoring), for click rings. Without it a recording still runs, without clicks, and prints `clicks are not recorded: ...` on stderr (for `start`, in `recorder.log`). `--no-click-highlight` asks for nothing.

While a recording runs, macOS shows its screen-recording indicator in the menu bar. On macOS 15 and later the system may also show a periodic alert that the program is reading the screen without the system's window picker.

On macOS, `minimized` in `recdr windows` is true for any window that is not on screen: minimized, hidden with its application, or on another Space. Such a window is refused with `target_minimized`. Positions are in pixels: each monitor's `bounds` origin is its position in points times the largest scale factor, so a Retina laptop with a 1x display has gaps between the monitors' `bounds`, and a position read off a frame plus the `bounds` origin is still a valid `--region` position.

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
recdr export <id> --file <out.gif|out.mp4> [--from 2s] [--until 5s] [--crop x,y,w,h]
recdr gui                           Open the desktop app (Windows and macOS)
recdr mcp                           Run the MCP server on stdio
recdr update [--check]              Install the newest release over this binary
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
| `--format <list>` | gif | `gif`, `mp4`, `gif,mp4`, or `none` to keep only the frames and the analysis and export later. |
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

## Export part of a recording

`recdr export` writes a GIF or MP4 of a finished recording, trimmed and cropped, without touching the recording's own files:

```
recdr export 20261118-143502-1a2b --file cut.gif --from 2s --until 5s --crop 0,0,640,360
```

| Flag | Meaning |
|---|---|
| `--file <path>` | Required. `.gif` or `.mp4`; the extension picks the format. An existing file is replaced only when the export succeeds. |
| `--from <time>` | Start of the kept part. Default: the start. |
| `--until <time>` | End of the kept part. Default: the end. |
| `--crop x,y,w,h` | A rectangle in the recording's pixels, at least 2 by 2, inside the picture. |

It prints `{"path", "width", "height", "duration_ms", "frames"}`; for an MP4 the size is that of the pictures in the file, after the padding described under Build. Click rings are drawn when the recording kept clicks. A recording that is still running is `recording_not_found`; bad times, a crop outside the picture or another extension are `usage`; a file that cannot be written is `encode_failed`.

## The app

`recdr gui` opens a frame and a toolbar on top of other windows, on Windows and macOS. Move the frame by dragging the toolbar, resize it by its edges and corners, or pick a window in the target menu to put the frame around it. Record and Stop record what is inside the frame; the frame turns red while it records. Stop opens an editor that plays the recording, trims it with the two handles on the timeline, crops it with a rectangle dragged on the picture, and saves a GIF or MP4 through `recdr export`. "New recording" goes back to the frame.

The app writes recordings with `--format none` into the output root, `./.recdr` by default, so a shortcut to it should pass `--out`. Every recording it makes is an ordinary recording: `recdr status`, `frames` and `export` work on it, and `recdr stop <id>` from a terminal ends it and opens the editor.

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
| `ffmpeg_not_found` | `--format mp4` on Linux without `ffmpeg` on `PATH`. |
| `recorder_start_failed` | The background recorder exited without saying why, or did not report its first frame in time. |
| `recording_not_found` | No recording with that id under the output root. |
| `recording_not_active` | `stop` or `mark` on a recording that has ended. |
| `recorder_crashed` | The background recorder exited without finishing. |
| `too_many_frames` | `get_frames` would return more than 8 frames. |
| `capture_failed` | The capture source failed. |
| `encode_failed` | Writing the GIF or MP4 failed, or, before recording, the system has no H.264 encoder that works. |
| `unsupported_platform` | `windows`, `monitors` or a desktop target on Linux. |
| `permission_denied` | macOS has not given the program that started recdr the Screen Recording permission. |
| `cdp_connect_failed` | The browser cannot be reached, does not speak CDP, or did not answer within 30 seconds. |
| `managed_install` | `recdr update` on a copy that Homebrew or cargo installed; the message names the command to run. |
| `update_failed` | `recdr update` could not read the latest version, download the install script, or the script failed; the message says which. |
| `io_error` | A file or process operation failed. |

## Tests

```
cargo test
cargo test --test desktop -- --ignored --test-threads=1      # Windows and macOS; records the real screen
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

Tools: `list_windows`, `list_monitors`, `list_tabs`, `start_recording`, `stop_recording`, `record`, `mark`, `get_status`, `get_frames` and `export`. Each returns the same JSON the CLI prints. `stop_recording` and `record` also return the contact sheet as an image, and `get_frames` returns up to 8 frames as images.

## Limits

- On Windows 10 the system draws a yellow border around the captured window or monitor, and it cannot be turned off there. Windows 11 turns it off.
- Clicks in windows of elevated (administrator) processes are not seen by the mouse hook, so they get no ring. Security software may flag a program that hooks the mouse and captures the screen; `--no-click-highlight` installs no hook.
- Protected video and some full-screen exclusive applications capture as black.
- On macOS a window recorded while it is minimized or on another Space stores no frames for that time; the recording runs on. A resized window is scaled to fit the starting size and padded with black by macOS itself.
- When a recorded window is resized, the first frame at the new size is discarded by the capture library. A window that then never repaints keeps showing its old image in the recording. Frames of another size are scaled to fit the first frame's size, on black.
- A recording holds whatever was on screen, including secrets. recdr never uploads anything; recordings stay in the output root.
- Keyboard input is never recorded.
- The MP4 path on a Windows "N" edition without the Media Feature Pack has not been tried. recdr checks for a working encoder before it records, so the expected result is `encode_failed` before anything is captured.
- The tests check MP4 files by their headers and by decoding frames with ffmpeg. A Windows MP4 plays in Chrome and opens in the Windows media stack; QuickTime Player and Safari have not been checked yet.
- The app needs Windows or macOS; on Linux `recdr gui` fails with `unsupported_platform`. It records a region or a window, not a monitor or a tab. A frame put around a window stays where it was put when the window moves.
- On Windows 11 each of the app's four border windows has a thin gray outline and rounded ends drawn by the system, which egui cannot switch off.
- On macOS, how the frame behaves over a full-screen application (which has its own Space) has not been tried.

## License

MIT

