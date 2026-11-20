# Spec: agent-recorder

Module `agent-recorder` of the [recdr capability map](capability-map.md). It is the first module and depends on no other. The directory name `1-recdr` carries the number of the tracking issue, https://github.com/dzikrisyairozi/recdr/issues/1.

All paths in this document are relative to the repository root, which is the directory that holds `LICENSE` and this `spec/` directory.

"The plan" means `tasks/1-recdr/plan.md`. It does not exist yet; it is written after this spec is approved and turns the spec into ordered tasks.

## Objective

An AI coding agent (Claude Code, or any agent with a shell or an MCP client) can take screenshots of a browser today but cannot record motion. It cannot show a human a bug that only appears during an animation, and it cannot check a transition or a loading state itself.

`agent-recorder` is a Windows command-line program, `recdr`, that records a window, a screen region or a monitor without showing any window of its own. It has two users:

- The agent, which starts and stops recordings and reads the result as still images, because current agents cannot watch a GIF or an MP4.
- The human, who gets a GIF or MP4 file to attach to a pull request or issue.

The repository holds only `LICENSE` (MIT) and `README.md` today. Everything in this spec is new.

### User stories

1. As an agent, I list the open windows, start recording one of them, do my browser actions with another tool, stop the recording, and receive a GIF path plus one contact-sheet image that shows what changed over time.
2. As an agent, I record for a fixed number of seconds with one blocking command.
3. As an agent or a human, I wrap a command (`recdr record --monitor 0 -- npx playwright test --headed`) and get a recording that covers exactly the time the command ran.
4. As an agent, I ask for the frames at given times to look closely at one moment.
5. As an agent, I label moments during a recording ("clicked submit") and see the labels on the contact sheet.
6. As a human, I get a file that plays inline on GitHub.

## Tech stack

Versions and licenses were read from crates.io with `cargo info` on 2026-11-18.

| Piece | Choice | License |
|---|---|---|
| Language | Rust 1.98, edition 2024 | |
| Screen capture | `windows-capture` 2.0.1, a wrapper over the Windows Graphics Capture API (WGC) | MIT |
| Win32 calls (window list, mouse hook) | `windows` crate | MIT OR Apache-2.0 |
| CLI parsing | `clap` 4.6 | MIT OR Apache-2.0 |
| MCP server | `rmcp` 3.11, the Rust SDK for the Model Context Protocol, stdio transport | Apache-2.0 |
| GIF encoding | `gif` 0.14 | MIT OR Apache-2.0 |
| PNG, scaling, drawing | `image` 0.25 | MIT OR Apache-2.0 |
| JSON | `serde`, `serde_json` | MIT OR Apache-2.0 |
| Errors | `thiserror` | MIT OR Apache-2.0 |
| Async runtime for the MCP server | `tokio` 1 | MIT |
| MCP tool parameter schemas | `schemars` 1 | MIT |
| Base64 for images returned by MCP tools | `base64` | MIT OR Apache-2.0 |
| Text on the contact sheet | `font8x8` 0.3, an 8x8 bitmap font | MIT |
| MP4 | an `ffmpeg` executable found on `PATH`, run as a child process | not linked |

Supported systems: Windows 10 version 1903 or later, and Windows 11. WGC does not exist before 1903.

Three things the design needs from `windows-capture` were confirmed by reading the 2.0.1 source on 2026-11-18:

- `Window::from_raw_hwnd` starts a capture from a window handle obtained from the Win32 window list.
- `CursorCaptureSettings::WithCursor` and `WithoutCursor` turn cursor drawing on and off.
- `DrawBorderSettings::WithoutBorder` asks Windows to leave out the yellow capture border. Windows 10 ignores the request.

## Commands

Development:

```
Build:         cargo build --release
Test:          cargo test
Desktop tests: cargo test -- --ignored
Lint:          cargo clippy --all-targets -- -D warnings
Format check:  cargo fmt --check
Run:           cargo run -- windows
```

The product's own command line:

```
recdr windows                       List visible top-level windows
recdr monitors                      List monitors
recdr record <target> --duration 10s [options]     Record for a fixed time, then return
recdr record <target> [options] -- <command...>    Record while the command runs
recdr start  <target> [options]     Start a background recording, return its id at once
recdr stop   <id>                   Stop it, encode, return the result
recdr mark   <id> "<label>"         Add a labelled marker at the current time
recdr status [<id>]                 State of one recording, or of all in the output root
recdr frames <id> (--at 1200ms,2s | --every 500ms | --keyframes) [--max-width 1024]
recdr mcp                           Run the MCP server on stdio
```

`<target>` is exactly one of:

| Flag | Meaning |
|---|---|
| `--window-id <id>` | The `id` printed by `recdr windows`: the Win32 window handle as a decimal number. Exact. The MCP tool descriptions tell agents to use this. |
| `--window "<text>"` | The visible window whose title contains the text, compared without regard to case. |
| `--process <name>` | The visible window owned by a process with this executable name, for example `chrome.exe`. |
| `--region x,y,w,h` | A rectangle in physical pixels, in virtual-screen coordinates. It must lie inside one monitor. |
| `--monitor <index>` | A whole monitor, by the `index` printed by `recdr monitors`. Indexes start at 0. |

A window target is resolved once, before recording starts, and the window must already exist and not be minimized. When `--window` or `--process` matches more than one window, recdr does not guess; it fails with `ambiguous_target` and lists the candidates.

In wrap-a-command form this means a window target cannot name a window that the wrapped command will open. Use `--monitor` or `--region` for that. A browser that runs headless has no window at all and is the subject of module `cdp-capture`.

Options for `record` and `start`:

| Option | Default | Meaning |
|---|---|---|
| `--duration <time>` | none | `record` only. Stop after this long. Larger than `--max-duration` is a usage error. |
| `--fps <n>` | 15 | Capture rate limit, 1 to 60. A frame that arrives sooner than 1/n seconds after the last accepted frame is discarded. |
| `--max-duration <time>` | 5m | Applies in every mode. The recording stops itself at this length. |
| `--max-size <size>` | 2GB | Applies in every mode. The recording stops itself when its directory reaches this size. |
| `--format <list>` | gif | `gif`, `mp4`, or both separated by a comma. |
| `--no-cursor` | off | Leave the mouse cursor out of the frames. |
| `--no-click-highlight` | off | Do not record clicks: no mouse hook is installed, the manifest has no clicks, and no rings are drawn. |

Option for every command that reads or writes recordings (`record`, `start`, `stop`, `mark`, `status`, `frames`):

| Option | Default | Meaning |
|---|---|---|
| `--out <dir>` | `./.recdr` | Output root. A recording lives in `<out>/<id>/`. A command finds a recording only when its `--out` names the same directory the recording was started with. |

A `<time>` is a whole number followed by `ms`, `s` or `m`, for example `500ms`, `10s`, `5m`. A `<size>` is a whole number followed by `MB` or `GB`.

## Behavior

### Output of every command

Each command prints one JSON object on stdout and exits 0. On failure it prints `{"error":{"code":"...","message":"..."}}` on stderr and exits 1, or 2 for a command-line usage error. Nothing else is written to stdout, so an agent can parse it without filtering.

In wrap-a-command form, the wrapped command's stdout and stderr are both sent to recdr's stderr, so they stay visible and stdout still holds only the JSON result.

### JSON output

Paths in JSON are absolute. Times are milliseconds from the start of the recording unless named otherwise.

| Command | Fields |
|---|---|
| `windows` | `windows`: list of `{id, title, process, pid, bounds: {x, y, width, height}, minimized}` |
| `monitors` | `monitors`: list of `{index, bounds: {x, y, width, height}, primary}` |
| `start` | `id`, `dir`, `width`, `height` |
| `stop`, `record` | The result object below |
| `mark` | `id`, `label` |
| `status` | `recordings`: list of `{id, dir, status, started_at, elapsed_ms}`; `status` is `recording`, `finished` or `crashed` |
| `frames` | `frames`: list of `{t_ms, path}` |

Result object, also saved as `result.json`:

| Field | Meaning |
|---|---|
| `id`, `dir` | Recording id and its directory |
| `stop_reason` | See Stop reasons |
| `command_exit_code` | Wrap-a-command form only |
| `duration_ms`, `width`, `height` | Length and frame size |
| `frames_stored`, `dropped_frames` | Counts |
| `keyframes` | List of `{t_ms, path}` |
| `marks` | List of `{t_ms, label}` |
| `gif`, `mp4` | Path of each file that was asked for |
| `contact_sheet`, `diff`, `manifest` | Paths of `contact_sheet.png`, `diff.json`, `manifest.json` |

### Error codes

The `code` values are part of the interface. This is the full list for this module.

| Code | When |
|---|---|
| `usage` | Bad flags or values, including `--fps` outside 1 to 60. Exit code 2. |
| `target_not_found` | No visible window, or no monitor, matches the target. |
| `ambiguous_target` | `--window` or `--process` matches more than one window. |
| `target_minimized` | The target window is minimized when recording starts. |
| `region_out_of_bounds` | `--region` does not lie inside one monitor. |
| `ffmpeg_not_found` | `mp4` was asked for and `ffmpeg` is not on `PATH`. |
| `recorder_start_failed` | The recorder did not report that it was recording within 5 seconds, or exited first. |
| `recording_not_found` | No recording with this id under the output root. |
| `recording_not_active` | `mark` on a recording that has finished or crashed. |
| `recording_not_finished` | `frames` on a recording that is still running or crashed without a result. |
| `recorder_crashed` | `stop` on a recording whose recorder process is gone and left no result. |
| `too_many_frames` | MCP `get_frames` matched more than 8 frames. |
| `capture_failed` | The capture API returned an error. |
| `encode_failed` | An encoder, including the ffmpeg child process, failed. |
| `io_error` | A file could not be read or written. |

### Recording directory

A recording id has the form `20261118-143502-a3f1` (UTC date, UTC time, four hex digits). Its directory holds:

```
.recdr/
  .gitignore            contains "*", written by recdr so recordings are never committed
  <id>/
    state.json          status, recorder process id, start time, options
    recorder.log        stderr of a detached recorder, for diagnosing a crash
    manifest.json       frame list with times, marks, clicks, stop reason, dropped-frame count
    frames/000001.png   one PNG per stored frame, at capture size
    marks.jsonl         one line per `recdr mark` call
    contact_sheet.png   written when the recording finishes
    diff.json           written when the recording finishes
    recording.gif       and/or recording.mp4
    result.json         the result object
```

A frame is stored only when its pixels are not byte-for-byte equal to the previous stored frame. The manifest gives each stored frame its time, so an unchanged screen costs no disk space and the encoders know how long to hold each frame.

### Control model

`recdr start` launches the recorder: a second, detached `recdr` process run through a hidden internal subcommand. The recorder sets `status` to `recording` in `state.json` once the capture has been set up. `start` waits for that, up to 5 seconds, then prints its JSON. If the recorder exits first or the 5 seconds pass, `start` fails with `recorder_start_failed`.

Later commands talk to the recorder through files in the recording directory:

- `stop` creates an empty file named `stop`. The recorder notices it, stops capturing, encodes, writes `result.json`, and exits. `stop` waits for `result.json` and prints it. While it waits it checks that the recorder process is alive; if the process is gone and there is no `result.json`, `stop` fails with `recorder_crashed`. `stop` on a recording that already finished prints the existing `result.json`.
- `mark` appends a line with the wall-clock time and the label to `marks.jsonl`. The recorder converts wall-clock times to recording times when it finishes.
- `status` reads `state.json` and checks whether the recorder process is alive. A recording whose process is gone and which has no `result.json` is `crashed`; its frames stay on disk.

`recdr record --duration` and `recdr record -- <command>` run the same recorder code inside the calling process and block until it finishes.

The MCP server starts recordings the same way `recdr start` does, so a recording survives a restart of the MCP server. Its output root is `.recdr` under the directory the server was started in, unless a tool call passes `out`.

### Stop reasons

`result.json` carries one `stop_reason`:

| Value | Cause |
|---|---|
| `stopped` | `recdr stop` was called. |
| `duration_reached` | `--duration` elapsed. |
| `cap_reached` | `--max-duration` elapsed. A later `recdr stop` prints the result that already exists. |
| `size_cap_reached` | The recording directory reached `--max-size`. A later `recdr stop` prints the result that already exists. |
| `command_exited` | The wrapped command exited. Its exit code is in `command_exit_code`; recdr itself exits 0 when the recording succeeded. |
| `window_closed` | The recorded window was closed. Frames captured so far are encoded. |

If the recorded window is minimized during a recording, no frames arrive and the recording continues. When the window is resized, later frames are scaled to fit the starting size and padded with black, so every frame in a recording has the same size.

### Encoding

All encoders read the stored PNG frames and the manifest.

- GIF is written with the `gif` crate and its built-in colour quantizer. Each stored frame gets a GIF frame delay equal to the time until the next stored frame. It needs no external program.
- MP4 (H.264) is written by an `ffmpeg` child process at a constant frame rate equal to `--fps`. recdr sends each stored frame as many times as needed to fill the time until the next one, so the video has the same length as the recording. When `--format` asks for `mp4` and `ffmpeg` is not on `PATH`, the command fails before recording starts.

### Cursor, clicks and markers

- The cursor is drawn into the stored frames unless `--no-cursor` is given. This is intended: where the pointer is forms part of what happened. A moving cursor therefore produces stored frames, but it changes far less than 0.5% of the pixels, so it does not produce keyframes.
- A low-level mouse hook records the time, position and button of each click inside the captured area. Clicks are stored in the manifest. Rings are drawn at encode time and on the contact sheet, never into the stored PNG frames, because a ring is an annotation added by recdr and was not on the screen.
- The hook records mouse buttons only. recdr never records keyboard input.

### Agent view

Written when a recording finishes:

- **Keyframes.** A pixel counts as changed when any colour channel differs by more than 16 out of 255. A stored frame is a keyframe when at least 0.5% of its pixels changed compared with the previous keyframe. The first and the last stored frame are always keyframes; a recording with one stored frame has one keyframe.
- **`contact_sheet.png`.** One image with up to 12 keyframes in a grid, each tile at most 480 pixels wide and stamped with its time and any marker label that falls on it. When there are more than 12 keyframes, 12 are chosen at even spacing, keeping the first and last.
- **`diff.json`.** For each stored frame: time, fraction of pixels changed, bounding box of the changed area, and whether it is a keyframe. No images. An agent can check "something changed in the top-right corner at 1.2 s" without spending image tokens.

On demand:

- **`recdr frames`.** `--at` returns, for each given time, the stored frame that was on screen at that time. `--every 500ms` does the same for 0 ms, 500 ms, 1000 ms and so on to the end. `--keyframes` returns all keyframes. The command prints file paths. With `--max-width`, it writes scaled copies to `<id>/samples/` and prints those paths.

### MCP tools

`recdr mcp` exposes eight tools. Each calls the same function as the CLI command beside it and takes the same parameters, including `out`.

| Tool | CLI equivalent | Returns |
|---|---|---|
| `list_windows` | `windows` | JSON |
| `list_monitors` | `monitors` | JSON |
| `start_recording` | `start` | JSON |
| `stop_recording` | `stop` | Result JSON and the contact sheet as an image |
| `record` | `record --duration` | Same as `stop_recording`. The recording runs in a detached recorder, like `start_recording`, and the tool waits for it. |
| `mark` | `mark` | JSON |
| `get_status` | `status` | JSON |
| `get_frames` | `frames` | The frames as images, scaled to at most 1024 pixels wide unless `max_width` is given. More than 8 matching frames is the error `too_many_frames`, whose message tells the caller to ask for fewer times. |

Wrapping a command is not an MCP tool; an agent with MCP uses `start_recording` and `stop_recording`.

Registering with Claude Code: `claude mcp add recdr -- recdr mcp`.

## Project structure

One Cargo package with a library and a binary, at the repository root.

```
Cargo.toml
src/main.rs              Binary entry: process set-up, then cli::run
src/lib.rs               Module declarations
src/cli.rs               Front end: clap definitions, JSON printing
src/mcp.rs               Front end: rmcp server
src/control.rs           The one interface both front ends call: record, start, stop, mark, status, frames
src/request.rs           Turns raw user input into validated record options
src/recorder.rs          Capture loop, then analysis and encoding
src/capture/mod.rs       CaptureSource trait, target resolution, window and monitor lists
src/capture/windows.rs   WGC backend
src/capture/fake.rs      Synthetic frame source used by tests
src/input.rs             Mouse click hook
src/store.rs             The recording directory: every file name and every read and write in it
src/analyze/mod.rs       Runs the analysis and writes diff.json
src/analyze/diff.rs      Pixel difference and keyframes
src/analyze/sheet.rs     Contact sheet
src/analyze/select.rs    Frame selection for `recdr frames`
src/encode/mod.rs        Output timeline shared by the encoders
src/encode/gif.rs        GIF writer
src/encode/ffmpeg.rs     ffmpeg child process for MP4
src/draw.rs              Text and ring drawing
src/platform.rs          Win32 calls that are not capture: DPI awareness, process liveness, detached spawn
src/time.rs              Clock, UTC stamp, `<time>` parsing
src/model.rs             Plain data types shared by every layer
src/error.rs             The error type and its codes
tests/                   Integration tests that run the built binary
spec/1-recdr/            Capability map and module specs
tasks/1-recdr/           Plan and task list
```

Modules form layers, and a module imports only from rows below its own row. `main.rs` sits above the table: it is the only file that names both front ends.

```
cli, mcp                  front ends; they never import each other
control, request
recorder
analyze, encode
capture, input, store
draw, platform, time
error
model
```

Five rules keep each concern in one place, and a test enforces each of them:

- Only `store.rs` names a file inside a recording directory.
- Only `cli.rs` uses `clap`, and only `mcp.rs` uses `rmcp`.
- `unsafe` appears only in `capture/windows.rs`, `input.rs` and `platform.rs`.
- The front ends import only `control`, `request`, `model` and `error`.
- `model.rs` holds data types and no functions.

`CaptureSource` is a trait because it has two implementations: the WGC backend and the synthetic source that lets the whole recording path be tested on a machine with no desktop.

## Code style

`rustfmt` defaults. `cargo clippy --all-targets -- -D warnings` must pass. Names follow the Rust API guidelines. No `unwrap` or `expect` outside tests. `unsafe` only in `capture/windows.rs`, `input.rs` and `platform.rs`, each block with a `// SAFETY:` comment. Comments explain why, not what.

Every failure the user can see is a variant of one error enum, and the variant decides the JSON error code. The sample shows the style with four of the codes; the full list is under Error codes.

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no visible window matches {0:?}")]
    TargetNotFound(String),
    #[error("{} windows match {query:?}; pass --window-id", candidates.len())]
    AmbiguousTarget { query: String, candidates: Vec<WindowInfo> },
    #[error("ffmpeg was not found on PATH; it is needed for mp4")]
    FfmpegNotFound,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Stable identifier printed in the JSON error object. Agents match on this,
    /// so renaming one is a breaking change.
    pub fn code(&self) -> &'static str {
        match self {
            Error::TargetNotFound(_) => "target_not_found",
            Error::AmbiguousTarget { .. } => "ambiguous_target",
            Error::FfmpegNotFound => "ffmpeg_not_found",
            Error::Io(_) => "io_error",
        }
    }
}
```

## Testing strategy

Framework: the built-in `cargo test`. Unit tests sit in a `#[cfg(test)]` module in the file they test. Integration tests sit in `tests/` and run the compiled binary.

| Level | Covers | Needs a desktop |
|---|---|---|
| Unit | Manifest read and write, frame de-duplication, keyframe and diff maths, contact-sheet layout, target resolution against a fixed window list, `<time>` parsing, GIF encoding of synthetic frames | No |
| Integration, synthetic source | The full path `record --duration`, `start`/`mark`/`stop`, `frames`, time limits, stop reasons and error codes, with the recorder reading from `capture/fake.rs` (selected by the environment variable `RECDR_FAKE_SOURCE`, which works for the CLI, the detached recorder and the MCP server alike) | No |
| Integration, MCP | Start `recdr mcp`, send `tools/list` and one `record` call over stdio, check the reply | No |
| Desktop, marked `#[ignore]` | Real WGC capture of the primary monitor and of a window the test opens; frame size equals target size; at least one frame arrives | Yes |

Desktop tests run by hand with `cargo test -- --ignored` before a release. Tests that need `ffmpeg` skip themselves with a printed notice when it is not on `PATH`.

No coverage percentage is set. Every success criterion below has at least one test, and the desktop criteria have a desktop test.

## Boundaries

Always:
- Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` before each commit.
- Keep stdout to one JSON object per command.
- Write a `// SAFETY:` comment on each `unsafe` block.
- Update this spec first when behavior changes.

Ask first:
- Adding a dependency that is not in the Tech stack table.
- Renaming or removing a JSON field, an error code, a CLI flag or an MCP tool after the first tagged release.
- Adding CI configuration.
- Writing any file outside the output root.

Never:
- Record keyboard input.
- Add a GPL or AGPL dependency.
- Send a recording or a frame over the network.
- Delete a recording unless a command asked for it.
- Commit the `.recdr/` directory.

## Non-goals

- Any GUI, including the overlay frame and the trim and crop screen. Module `gui-overlay`.
- Browser-tab capture through the Chrome DevTools Protocol, and with it headless browsers. Module `cdp-capture`.
- macOS and Linux. Module `macos-capture`; Linux is not on the map.
- MP4 without ffmpeg. Module `native-mp4`.
- WebM. GIF and MP4 both play inline on GitHub.
- Recording whichever window has focus. An agent's own terminal usually has it.
- Audio.
- Trimming or cropping from the command line.
- Encoding another format from a finished recording. Ask for every format needed in `--format` when recording.
- Waiting for a window that does not exist yet.
- A region that spans two monitors.
- A border or other on-screen sign drawn by recdr while recording.
- Launching or controlling the application being recorded.
- Installers, code signing and published binaries.

## Alternatives considered

- **Wrap ffmpeg for capture as well** (`ffmpeg -f gdigrab -i title=...`). This would remove most of the capture code. It lost because ffmpeg would become a hard requirement even for GIF, and because the recorder would get no per-frame times or change data without decoding the video again.
- **`gifski` for GIF.** Better colours than the `gif` crate. It is AGPL-3.0-or-later, which would force the whole program off MIT.
- **A long-running daemon with a named pipe** for start, stop and mark. Lower stop latency than watching for a file. It lost because the file protocol needs no connection handling and leaves state a human can read with a text editor after a crash.
- **Store the recording only as a video file** and extract frames when asked. Much less disk. It lost because extracting frames would need ffmpeg, and the agent view is half the purpose of the module.
- **MCP server only, no CLI.** It lost because agents without an MCP client, and humans in a terminal, could not use it, and the wrap-a-command form only makes sense as a CLI.
- **`xcap` for capture.** One crate that covers Windows and macOS. Its window-recording path has not been evaluated.

## Risks

| Risk | Mitigation |
|---|---|
| Writing a PNG per frame cannot keep up at high resolution, and frames are dropped. | Dropped frames are counted in `dropped_frames` in the manifest and the result, and success criterion 15 sets the limit. The plan picks the write mechanism after the first measured run. Unchanged frames are never written. |
| A five-minute recording of a busy 1080p window uses several gigabytes. | Unchanged frames are not stored; the defaults are 15 fps, 5 minutes and a 2 GB size limit. |
| Windows 10 draws a yellow border around any WGC capture and it cannot be turned off there. | Accept it on Windows 10 and document it. On Windows 11 turn it off if the crate allows. |
| The mouse hook does not see clicks in windows of elevated processes, and security software may flag a program that hooks the mouse and captures the screen. | Missing clicks only cost the click rings. Document both. `--no-click-highlight` installs no hook. |
| Protected video and some full-screen exclusive applications capture as black. | Document as a WGC limit. |
| A recording holds whatever was on screen, including secrets. | recdr writes `.recdr/.gitignore`, never uploads anything, and says so in the README. |
| An agent starts a recording and never stops it. | `--max-duration` and `--max-size` stop it. |

## Success criteria

Checked on Windows 11 unless stated.

1. `recdr windows` prints the fields listed under JSON output for every visible top-level window, and `recdr monitors` does the same for every monitor.
2. `recdr record --window-id <id> --duration 3s` exits 0 and prints a result whose `gif` is a GIF between 2.5 and 3.5 seconds long with the window's pixel size.
3. `recdr start --monitor 0` returns an id in under 2 seconds. `recdr stop <id>` returns a result with `stop_reason` `stopped`, the recorder stops capturing within 500 ms of the `stop` call, and the recorder process is gone afterwards.
4. `recdr record --region 0,0,640,480 -- cmd /c "echo hi & timeout /t 2"` produces a recording of about 2 seconds with `stop_reason` `command_exited` and `command_exit_code` 0. stdout holds only the JSON result, and `hi` appears on stderr.
5. A recording started with `--max-duration 2s` and never stopped ends by itself with `stop_reason` `cap_reached`.
6. Each code in the Error codes table is produced by the condition beside it. In particular `--window "zzz-no-such-window"` gives `target_not_found` with exit code 1, a title matching two windows gives `ambiguous_target` listing both, and `stop` after the recorder process was killed gives `recorder_crashed`.
7. `--format mp4` writes an MP4 that `ffprobe` reads as H.264 with a duration within 0.5 seconds of `duration_ms` when ffmpeg is on `PATH`, and fails with `ffmpeg_not_found` before recording when it is not.
8. Every finished recording has `contact_sheet.png` with at most 12 tiles and a `diff.json` with at least 1 keyframe.
9. A screen that does not change for 5 seconds, with the mouse not moving, produces at most 2 stored frames.
10. `recdr mark <id> "clicked submit"` during a recording puts a mark with that label in the manifest, with a time within 200 ms of when the command ran, and the label is drawn on the contact sheet.
11. `recdr frames <id> --at 1s` prints the path of the stored frame that was on screen at 1 second. With `--max-width 512` the printed file is at most 512 pixels wide.
12. A mouse click inside the recorded area appears in the manifest with coordinates relative to the capture origin, and as a ring in the GIF. The stored PNG frames contain no ring.
13. Closing the recorded window ends the recording with `stop_reason` `window_closed` and a playable GIF.
14. `recdr mcp`, registered in Claude Code, lists the eight tools in this spec. `stop_recording` returns the contact sheet as an image the agent can see.
15. Recording a 1920x1080 monitor at 15 fps for 30 seconds while a video plays reports `dropped_frames` of at most 5% of the frames captured.
16. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` all pass.
17. A recording started with `--max-size 1MB` on a changing screen ends by itself with `stop_reason` `size_cap_reached`.

## Decisions made after review

The owner delegated these on 2026-11-18.

1. Size limit: added, `--max-size`, default 2 GB.
2. Wrap-a-command exit code: recdr exits 0 when the recording succeeded and reports the command's code in `command_exit_code`. One rule for exit codes in every form.
3. Contact-sheet font: `font8x8`, a bitmap font with no font file to ship.
4. Click capture stays in this module. `--no-click-highlight` installs no hook.
5. `--format webm` and `--foreground` are cut. `--process` and `frames --every` stay.

## Open questions

None.
