# Implementation plan: agent-recorder

Spec: [spec/1-recdr/spec-agent-recorder.md](../../spec/1-recdr/spec-agent-recorder.md). Task list: [todo.md](todo.md). Paths are relative to the repository root, `D:\recdr` on the owner's machine: the directory that holds `LICENSE`, `spec/` and `tasks/`.

## Overview

Build `recdr`, a Windows command-line screen recorder for agents, as one Cargo package (library plus binary). Git tracks only `LICENSE` and `README.md` today, so every source file is new. The work is 16 tasks in four phases. The recording path is built first against a synthetic frame source so it can be tested without a desktop, and the real Windows Graphics Capture (WGC) backend follows straight after because it carries the most risk.

### State of the working tree before Task 1

Untracked today: `spec/`, `tasks/`, `.serena/` (editor tooling state), and `recdr/`, a stray second clone of this same repository.

- The owner deletes `recdr/` before Task 1. Task 1 stops if it still exists.
- Task 1 creates the branch `feat/1-agent-recorder` from `main`. Every commit of this plan goes on that branch. Opening a pull request is the owner's call after Task 16.
- Task 1 puts `.serena/` in `.gitignore`.
- Task 1 commits `spec/` and `tasks/` on their own first, as `docs(agent-recorder): add spec and plan`. That includes the two review logs, `spec/1-recdr/goldfish.md` and `tasks/1-recdr/goldfish.md`, which are final before Task 1 starts.
- Every commit stages the files named in its task by path. `git add -A` and `git add .` are not used.

## Architecture

### Layers

A module imports only from rows below its own row. This is the same table as the spec's Project structure. `main.rs` sits above the table: it is the composition root and the only file that names both front ends.

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

Rules, each enforced by a test in `tests/layers.rs`:

1. Only `src/store.rs` contains one of the nine quoted recording file names: `"state.json"`, `"manifest.json"`, `"result.json"`, `"diff.json"`, `"marks.jsonl"`, `"recorder.log"`, `"contact_sheet.png"`, `"recording.gif"`, `"recording.mp4"`.
2. Only `src/cli.rs` mentions `clap`. Only `src/mcp.rs` mentions `rmcp`, `tokio` or `base64`. (`schemars` is also allowed in `src/request.rs`, for one derive.)
3. `unsafe` appears only in `src/capture/windows.rs`, `src/input.rs` and `src/platform.rs`.
4. `src/cli.rs` and `src/mcp.rs` import only `control`, `request`, `model` and `error`.
5. `src/model.rs` contains no `fn`.

### What each module owns

| Module | Owns | Does not do |
|---|---|---|
| `error` | `Error`, its codes, exit codes, JSON form | |
| `model` | Every plain data type that crosses a module boundary | Logic |
| `time` | Wall clock, UTC stamp, `<time>` parsing | |
| `platform` | DPI awareness, process liveness, detached spawn | Capture |
| `draw` | Bitmap text and ring outlines on an image | Layout |
| `store` | The recording directory: paths, atomic JSON, PNG frames, de-duplication, letterboxing | Deciding when to stop |
| `capture` | Listing windows and monitors, resolving a target, producing frames | Writing files |
| `input` | Mouse click hook | |
| `analyze` | Pixel difference, keyframes, contact sheet, frame selection | Encoding |
| `encode` | Output timeline, GIF, MP4 | Analysis |
| `recorder` | One recording from first frame to `result.json` | Process management |
| `request` | Validating raw user input into `RecordOptions` | Anything with side effects except reading one environment variable |
| `control` | The operations a user can ask for; process management for detached recorders | Parsing arguments, printing |
| `cli`, `mcp` | Turning arguments or tool calls into `control` calls, and results into output | Anything else |

### Package and dependencies

`Cargo.toml`: package `recdr`, version `0.1.0`, edition `2024`, license `MIT`. A `[lib]` at `src/lib.rs` and a `[[bin]]` named `recdr` at `src/main.rs`. Each task adds a dependency in the task that first uses it, and `Cargo.lock` changes with it.

| Dependency | Version | Features | Added in |
|---|---|---|---|
| `clap` | 4.6 | `derive` | Task 1 |
| `serde` | 1 | `derive` | Task 1 |
| `serde_json` | 1 | | Task 1 |
| `thiserror` | 2 | | Task 1 |
| `windows-capture` | 2.0.1 | under `[target.'cfg(windows)'.dependencies]` | Task 2 |
| `windows` | 0.62 | same section; `Win32_Foundation`, `Win32_UI_WindowsAndMessaging`, `Win32_System_Threading`, `Win32_Graphics_Gdi`, `Win32_UI_HiDpi` | Task 2 |
| `image` | 0.25 | `default-features = false`, `png` | Task 3 |
| `gif` | 0.14 | | Task 6 |
| `font8x8` | 0.3 | | Task 7 |
| `rmcp` | 3.11 | `server`, `macros`, `transport-io` | Task 15 |
| `tokio` | 1 | `rt`, `macros`, `io-std` | Task 15 |
| `schemars` | 1 | | Task 15 |
| `base64` | 0.23 | | Task 15 |

`windows` 0.62 is the version `windows-capture` 2.0.1 depends on, and `base64` 0.23 is the version `rmcp` 3.11 depends on, so one copy of each is built.

## Module contracts

These are fixed. A task that needs to change one edits this section first.

### `src/error.rs`

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]                                                              Usage(String),
    #[error("no visible window or monitor matches {0}")]                         TargetNotFound(String),
    #[error("{} windows match {query:?}; pass --window-id", candidates.len())]  AmbiguousTarget { query: String, candidates: Vec<WindowInfo> },
    #[error("window {0:?} is minimized")]                                        TargetMinimized(String),
    #[error("the region must lie inside one monitor")]                           RegionOutOfBounds,
    #[error("ffmpeg was not found on PATH; it is needed for mp4")]               FfmpegNotFound,
    #[error("the recorder did not start: {0}")]                                  RecorderStartFailed(String),
    #[error("no recording {0:?} under the output root")]                         RecordingNotFound(String),
    #[error("recording {0:?} is not running")]                                   RecordingNotActive(String),
    #[error("the recorder for {0:?} exited without a result; its log is in the recording directory")] RecorderCrashed(String),
    #[error("{0} frames match; ask for at most 8")]                              TooManyFrames(usize),
    #[error("capture failed: {0}")]                                              CaptureFailed(String),
    #[error("encoding failed: {0}")]                                             EncodeFailed(String),
    #[error(transparent)]                                                        Io(#[from] std::io::Error),
}
impl Error {
    pub fn code(&self) -> &'static str;          // the variant name in snake_case; Io is "io_error"; Usage is "usage"
    pub fn exit_code(&self) -> i32;              // 2 for Usage, 1 otherwise
    pub fn to_json(&self) -> serde_json::Value;  // {"error":{"code","message"}}; AmbiguousTarget is {"error":{"code","message","candidates":[WindowInfo..]}}
}
impl From<serde_json::Error> for Error;          // -> Io(std::io::Error::other(e))
impl From<image::ImageError> for Error;          // -> Io(std::io::Error::other(e)); added in Task 3
pub type Result<T> = std::result::Result<T, Error>;
```

The strings the target errors carry: `TargetNotFound` gets `window id 123`, `window "text"`, `process "name"` or `monitor 5`; `TargetMinimized` gets the window's title; `AmbiguousTarget.query` is the text the user gave.

`src/lib.rs` declares the modules and has `pub use error::{Error, Result};`. Each task adds the `pub mod` line for a file in the task that creates the file.

### `src/model.rs`

Every type derives `Debug, Clone, serde::Serialize, serde::Deserialize`. `Rect` also derives `Copy, PartialEq`. Every enum has `#[serde(rename_all = "snake_case")]` and uses serde's default external tagging. `Option` fields have `#[serde(skip_serializing_if = "Option::is_none", default)]`.

```rust
pub struct Rect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
pub struct WindowInfo { pub id: u64, pub title: String, pub process: String, pub pid: u32, pub bounds: Rect, pub minimized: bool }
pub struct MonitorInfo { pub index: usize, pub bounds: Rect, pub primary: bool }
pub enum FakeKind { Static, Moving, Closing, Broken }
pub enum Target { WindowId(u64), WindowTitle(String), Process(String), Region(Rect), Monitor(usize), Fake(FakeKind) }
pub enum ResolvedKind { Window(u64), Monitor { index: usize, crop: Option<Rect> }, Fake(FakeKind) }
pub struct ResolvedTarget { pub kind: ResolvedKind, pub origin: (i32, i32) }
pub struct Frame { pub width: u32, pub height: u32, pub rgba: Vec<u8> }        // derives Debug, Clone only
pub enum CaptureEvent { Frame(Frame), Closed }                                 // derives Debug, Clone only
pub enum Format { Gif, Mp4 }
pub enum StopReason { Stopped, DurationReached, CapReached, SizeCapReached, CommandExited, WindowClosed }
pub enum Status { Starting, Recording, Finished, Crashed }
pub struct RecordOptions {
    pub target: Target, pub fps: u32, pub max_duration_ms: u64, pub max_size_bytes: u64,
    pub formats: Vec<Format>, pub cursor: bool, pub clicks: bool, pub out: PathBuf,
    pub duration_ms: Option<u64>, pub command: Option<Vec<String>>,
}
pub struct State {
    pub id: String, pub status: Status, pub pid: u32, pub started_at_unix_ms: u64,
    pub width: u32, pub height: u32, pub resolved: ResolvedTarget, pub options: RecordOptions,
}
pub struct FrameEntry { pub index: u32, pub t_ms: u64, pub file: String }      // file is relative to the recording directory
pub struct Mark { pub t_ms: u64, pub label: String }
pub struct Click { pub t_ms: u64, pub x: i32, pub y: i32, pub button: String } // "left" | "right" | "middle"
pub struct Manifest {
    pub id: String, pub width: u32, pub height: u32, pub fps: u32,
    pub started_at_unix_ms: u64, pub duration_ms: u64,
    pub frames: Vec<FrameEntry>, pub marks: Vec<Mark>, pub clicks: Vec<Click>,
    pub stop_reason: StopReason, pub dropped_frames: u64,
}
pub struct DiffEntry { pub t_ms: u64, pub changed_fraction: f32, pub bbox: Option<Rect>, pub keyframe: bool }
pub struct FrameRef { pub t_ms: u64, pub path: PathBuf }
pub enum FrameSelection { At(Vec<u64>), Every(u64), Keyframes }               // derives Debug, Clone only
pub struct StartInfo { pub id: String, pub dir: PathBuf, pub width: u32, pub height: u32 }
pub struct StatusEntry { pub id: String, pub dir: PathBuf, pub status: Status, pub started_at: u64, pub elapsed_ms: u64 }
// What each list command returns. Both front ends serialize these as they are.
pub struct WindowList { pub windows: Vec<WindowInfo> }
pub struct MonitorList { pub monitors: Vec<MonitorInfo> }
pub struct StatusList { pub recordings: Vec<StatusEntry> }
pub struct FrameList { pub frames: Vec<FrameRef> }
pub struct MarkAck { pub id: String, pub label: String }
pub struct RecordingResult {
    pub id: String, pub dir: PathBuf, pub stop_reason: StopReason,
    pub command_exit_code: Option<i32>,
    pub duration_ms: u64, pub width: u32, pub height: u32,
    pub frames_stored: u32, pub dropped_frames: u64,
    pub keyframes: Vec<FrameRef>, pub marks: Vec<Mark>,
    pub gif: Option<PathBuf>, pub mp4: Option<PathBuf>,
    pub contact_sheet: PathBuf, pub diff: PathBuf, pub manifest: PathBuf,
}
```

Task 3 creates `RecordingResult` with `contact_sheet` and `diff` as `Option<PathBuf>` and `keyframes` empty, because the analysis does not exist yet. Task 7 changes the two fields to `PathBuf`, which is the final form above.

`ResolvedTarget` carries no size. The size of a recording is the size of its first frame (see `recorder`).

### `src/time.rs`

```rust
pub fn now_unix_ms() -> u64;
pub fn utc_stamp(unix_ms: u64) -> String;        // "20261118-143502"
pub fn parse_time(text: &str) -> Result<u64>;    // "500ms" | "10s" | "5m" -> milliseconds; anything else is Usage
```

`utc_stamp` uses a private `civil_from_days(days: i64) -> (i32, u32, u32)` (Howard Hinnant's algorithm), so no date crate is needed.

### `src/platform.rs` (`#[cfg(windows)]`)

```rust
pub fn set_dpi_aware();                          // SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); a failure is ignored
pub fn process_alive(pid: u32) -> bool;          // OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION) + GetExitCodeProcess == 259; false for pid 0
pub fn spawn_detached(exe: &Path, args: &[&OsStr], stderr: std::fs::File) -> Result<u32>; // returns the child pid
```

`spawn_detached` uses `std::os::windows::process::CommandExt::creation_flags(0x0000_0008 | 0x0000_0200)` (`DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`), with stdin and stdout null.

The child must not inherit the caller's own stdin, stdout and stderr. If it did, a program reading `recdr start`'s output to end of file would wait until the recorder exits. So `spawn_detached` calls `SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0)` on this process's three standard handles (from `std::io::stdin().as_raw_handle()` and the other two) before the spawn, and sets the flag back after it. A handle that is null or invalid is skipped.

### `src/draw.rs`

```rust
pub fn text(image: &mut image::RgbaImage, x: u32, y: u32, text: &str, scale: u32);
pub fn ring(image: &mut image::RgbaImage, cx: i32, cy: i32, radius: i32);
```

- `text`: 8x8 glyphs from `font8x8::BASIC_FONTS`, each set bit drawn as a white `scale` by `scale` square. A character outside basic ASCII is drawn as `?`. Pixels outside the image are skipped.
- `ring`: an outline 3 pixels thick in RGB 255, 64, 64, made by testing each pixel of the bounding square against `radius - 3` and `radius`. Pixels outside the image are skipped.

### `src/store.rs`

```rust
pub struct RecordingDir { /* path: PathBuf */ }
impl RecordingDir {
    pub fn create(out: &Path, now_unix_ms: u64) -> Result<Self>;
    pub fn open(out: &Path, id: &str) -> Result<Self>;                 // RecordingNotFound when <out>/<id>/state.json is missing
    pub fn at(path: PathBuf) -> Self;                                  // for the hidden `_recorder <dir>` command
    pub fn list(out: &Path) -> Result<Vec<Self>>;                      // sub-directories of out that hold state.json; empty when out is missing
    pub fn id(&self) -> &str;                                          // the directory name
    pub fn path(&self) -> &Path;

    pub fn read_state(&self) -> Result<State>;
    pub fn write_state(&self, state: &State) -> Result<()>;
    pub fn read_manifest(&self) -> Result<Manifest>;
    pub fn write_manifest(&self, manifest: &Manifest) -> Result<PathBuf>;
    pub fn read_diff(&self) -> Result<Vec<DiffEntry>>;
    pub fn write_diff(&self, entries: &[DiffEntry]) -> Result<PathBuf>; // file content is {"frames":[..]}
    pub fn read_result(&self) -> Result<Option<RecordingResult>>;      // None when the file is missing
    pub fn write_result(&self, result: &RecordingResult) -> Result<()>;

    pub fn request_stop(&self) -> Result<()>;                          // creates the empty file `stop`
    pub fn stop_requested(&self) -> bool;
    pub fn append_mark(&self, unix_ms: u64, label: &str) -> Result<()>; // one line {"unix_ms":..,"label":..}
    pub fn read_marks(&self) -> Result<Vec<(u64, String)>>;            // empty when the file is missing

    pub fn frame_writer(&self, width: u32, height: u32) -> Result<FrameWriter>;
    pub fn frame_path(&self, entry: &FrameEntry) -> PathBuf;
    pub fn load_frame(&self, entry: &FrameEntry) -> Result<image::RgbaImage>;
    pub fn sample_path(&self, entry: &FrameEntry, max_width: u32) -> Result<PathBuf>; // samples/<index>-w<max_width>.png; creates samples/
    pub fn contact_sheet_path(&self) -> PathBuf;
    pub fn gif_path(&self) -> PathBuf;
    pub fn mp4_path(&self) -> PathBuf;
    pub fn log_file(&self) -> Result<std::fs::File>;                   // creates recorder.log
    pub fn last_log_line(&self) -> String;                             // "" when the file is missing or empty
}
pub struct FrameWriter { /* dir, width, height, frames, last_rgba, bytes */ }
impl FrameWriter {
    pub fn push(&mut self, frame: Frame, t_ms: u64) -> Result<bool>;   // false when equal to the last stored frame
    pub fn bytes(&self) -> u64;                                        // total size of the PNG files written
    pub fn finish(self) -> Vec<FrameEntry>;
}
pub fn letterbox(frame: Frame, width: u32, height: u32) -> Frame;
pub fn frame_at(frames: &[FrameEntry], t_ms: u64) -> Option<&FrameEntry>;
```

- `create`: makes `out`; writes `<out>/.gitignore` containing `*` when it is missing; builds the id as `utc_stamp(now) + "-" + 4 hex digits`, where the digits start as the low 16 bits of `now_unix_ms ^ std::process::id()`; calls `std::fs::create_dir`. On `AlreadyExists` it adds 1 to the 16-bit value and tries again, up to 16 times, then returns the `Io` error. It then creates `frames/`.
- Every JSON write goes to `<name>.tmp` in the same directory and is then moved over the real name with `std::fs::rename`, so a reader never sees half a file.
- `push`: calls `letterbox` when the frame size differs from the writer's size, then compares `rgba` byte-for-byte with the last stored frame. PNG is written with `image::codecs::png::PngEncoder::new_with_quality(file, CompressionType::Fast, FilterType::Adaptive)`. Files are `frames/000001.png`, numbered from 1.
- `letterbox`: scales with `image::imageops::resize(.., FilterType::Triangle)` to the largest size that fits while keeping the aspect ratio, centred on a black frame.
- `frame_at`: the last entry with `entry.t_ms <= t_ms`; the first entry when `t_ms` is before it; `None` for an empty list.

### `src/capture/mod.rs`

```rust
pub trait CaptureSource {
    /// Ok(None) when nothing arrived within `timeout`.
    fn next_event(&mut self, timeout: Duration) -> Result<Option<CaptureEvent>>;
    /// Frames the source discarded because the consumer was behind.
    fn dropped(&self) -> u64;
}
pub fn list_windows() -> Result<Vec<WindowInfo>>;
pub fn list_monitors() -> Result<Vec<MonitorInfo>>;
pub fn resolve(target: &Target, windows: &[WindowInfo], monitors: &[MonitorInfo]) -> Result<ResolvedTarget>;
pub fn open(resolved: &ResolvedTarget, fps: u32, cursor: bool) -> Result<Box<dyn CaptureSource>>;
```

`list_windows` and `list_monitors` forward to `capture::windows`. `open` returns `CaptureFailed("the synthetic source is broken")` for `Fake(Broken)`, `fake::FakeSource` for the other `Fake` kinds, and `windows::WgcSource` otherwise. No other module names `capture::windows` or `capture::fake`.

`resolve` rules:

- `WindowId`: the window with that `id`, else `TargetNotFound`.
- `WindowTitle`: windows whose `title` contains the text, both lowercased. `Process`: windows whose `process` equals the name, both lowercased. No match is `TargetNotFound`; more than one is `AmbiguousTarget`.
- A resolved window with `minimized == true` is `TargetMinimized`. `origin` is its `bounds.x, bounds.y`.
- `Region`: the monitor whose `bounds` fully contain the rectangle, else `RegionOutOfBounds`. `crop` is the rectangle relative to that monitor's top-left corner. `origin` is the rectangle's `x, y`.
- `Monitor`: the monitor with that `index`, else `TargetNotFound`. `crop` is `None`. `origin` is its `bounds.x, bounds.y`.
- `Fake`: `origin` is `(0, 0)`.

### `src/capture/windows.rs` (`#[cfg(windows)]`)

```rust
pub fn list_windows() -> Result<Vec<WindowInfo>>;
pub fn list_monitors() -> Result<Vec<MonitorInfo>>;
pub struct WgcSource { /* rx: mpsc::Receiver<CaptureEvent>, control: Option<CaptureControl<..>>, dropped: Arc<AtomicU64> */ }
impl WgcSource { pub fn open(resolved: &ResolvedTarget, fps: u32, cursor: bool) -> Result<Self>; }
impl CaptureSource for WgcSource { .. }
impl Drop for WgcSource { .. }                   // takes `control` out of the Option and calls CaptureControl::stop, which consumes it
```

- `list_windows`: `windows_capture::window::Window::enumerate()`. For each window: `id` is `as_raw_hwnd() as u64`; `minimized` is `IsIconic`. A window whose `title()` or `rect()` returns `Err`, or whose title is empty, is left out. When `process_name()` returns `Err` (it does for elevated processes) `process` is the empty string; when `process_id()` returns `Err`, `pid` is 0. One unreadable window never fails the call.
- `list_monitors`: `windows_capture::monitor::Monitor::enumerate()`. `index` is the zero-based position in that list. `bounds` come from `GetMonitorInfoW` on `as_raw_hmonitor()`; `primary` is the `MONITORINFOF_PRIMARY` flag. The crate's own `Monitor::index()` is not used: it is a number parsed from the device name.
- `WgcSource::open`: the item is `Window::from_raw_hwnd(id as *mut c_void)`, or `Monitor::from_index(index + 1)` (the crate's `from_index` is one-based over the same enumeration). Settings: `CursorCaptureSettings::WithCursor` or `WithoutCursor`; `DrawBorderSettings::WithoutBorder`; `SecondaryWindowSettings::Default`; `MinimumUpdateIntervalSettings::Custom(1000 ms / fps)`; `DirtyRegionSettings::Default`; `ColorFormat::Rgba8`. It starts a private `GraphicsCaptureApiHandler` with `start_free_threaded`, which makes its own thread and initialises WinRT there.
- `on_frame_arrived`: `frame.buffer()`, or `frame.buffer_crop(x, y, x + width, y + height)` when `crop` is set; copied with `as_nopadding_buffer`; sent as `CaptureEvent::Frame` with `try_send` on a `std::sync::mpsc::sync_channel(8)`. A full channel adds 1 to `dropped`. `on_closed` sends `CaptureEvent::Closed`.
- `next_event`: `recv_timeout`. A timeout is `Ok(None)`. A disconnected channel is `Err(CaptureFailed("the capture thread ended"))`.

### `src/capture/fake.rs`

```rust
pub struct FakeSource { /* kind, frame counter, next due Instant */ }
impl FakeSource { pub fn new(kind: FakeKind) -> Self; }
```

320x240 frames, 30 per second. `Static`: solid black. `Moving`: a white 40x40 square on black with its left edge at `(frame_number * 8) % 280` and its top at 100. `Closing`: ten `Moving` frames, then `CaptureEvent::Closed` on every later call. `Broken` is never constructed; `capture::open` rejects it, which gives tests a way to make a recorder fail. `next_event` sleeps until the next frame is due or `timeout` passes. `dropped()` is 0.

### `src/input.rs` (`#[cfg(windows)]`)

```rust
pub struct ClickHook { /* thread: JoinHandle<()>, thread_id: u32, rx: mpsc::Receiver<(u64, i32, i32, &'static str)> */ }
impl ClickHook {
    pub fn start() -> Result<Self>;
    pub fn finish(self, origin: (i32, i32), width: u32, height: u32, started_at_unix_ms: u64) -> Vec<Click>;
}
```

A thread calls `SetWindowsHookExW(WH_MOUSE_LL, ..)` and runs a `GetMessageW` loop. The hook procedure handles `WM_LBUTTONDOWN`, `WM_RBUTTONDOWN` and `WM_MBUTTONDOWN`, sends `(now_unix_ms, pt.x, pt.y, button)` on the sender held in `static SENDER: Mutex<Option<mpsc::Sender<..>>>`, and returns `CallNextHookEx`. `start` returns `CaptureFailed("a click hook is already running in this process")` when `SENDER` is already `Some`. `finish` posts `WM_QUIT` with `PostThreadMessageW`, joins the thread, sets `SENDER` to `None`, keeps the clicks inside the rectangle `origin` + `width` x `height`, makes `x, y` relative to `origin`, and sets `t_ms` to `unix_ms - started_at_unix_ms`.

Only one in-process recording exists per process (CLI `record`), because the MCP server always uses detached recorders. The `Mutex<Option<..>>` makes a second start an error instead of silent loss.

### `src/analyze/`

```rust
// mod.rs
pub mod diff; pub mod sheet; pub mod select;
pub fn run(dir: &RecordingDir, manifest: &Manifest) -> Result<Vec<DiffEntry>>;   // computes entries, calls dir.write_diff

// diff.rs
pub const CHANNEL_THRESHOLD: u8 = 16;
pub const KEYFRAME_FRACTION: f32 = 0.005;
pub fn diff(a: &[u8], b: &[u8], width: u32, height: u32) -> (f32, Option<Rect>);

// sheet.rs
pub const MAX_TILES: usize = 12;
pub fn pick_tiles(keyframes: &[usize], max: usize) -> Vec<usize>;
pub fn contact_sheet(dir: &RecordingDir, manifest: &Manifest, diffs: &[DiffEntry]) -> Result<PathBuf>;

// select.rs
pub fn select(manifest: &Manifest, diffs: &[DiffEntry], selection: &FrameSelection) -> Vec<FrameEntry>;
pub fn frame_refs(dir: &RecordingDir, entries: &[FrameEntry], max_width: Option<u32>) -> Result<Vec<FrameRef>>;
```

- `diff`: a pixel is changed when any of R, G, B differs by more than `CHANNEL_THRESHOLD`. Returns changed pixels divided by all pixels, and the smallest rectangle holding every changed pixel (`None` when none changed).
- `run`: walks the stored frames in order, holding the previous frame and the previous keyframe in memory. `changed_fraction` and `bbox` compare a frame with the frame before it (0 and `None` for the first). `keyframe` is true for the first frame, the last frame, and any frame whose `diff` against the previous keyframe is at least `KEYFRAME_FRACTION`.
- `pick_tiles`: all items when there are `max` or fewer; otherwise the items at positions `round(i * (n - 1) / (max - 1))` for `i` in `0..max`.
- `contact_sheet`: tiles are the keyframes chosen by `pick_tiles(.., MAX_TILES)`. Columns = `ceil(sqrt(tiles))`. A tile is the frame scaled with `FilterType::Triangle` to a width of `min(480, frame width)` keeping the aspect ratio, with a 20-pixel black strip under it. The background is RGB 32, 32, 32, with 4 pixels around the sheet and between tiles. A tile covers the time from its own `t_ms` up to the next tile's `t_ms` (the last tile runs to `duration_ms`). The strip text is drawn with `draw::text` at 4 pixels in and 2 pixels down, scale 2: the time as `format!("{:.1}s", t_ms as f64 / 1000.0)`, then one space, then the labels of every mark in the tile's time span joined with `, `; the whole string is cut to `(tile width - 8) / 16` characters. Every click in the span is drawn with `draw::ring` at its scaled position, radius 10. Because spans cover the whole recording, no mark is left off the sheet.
- `select`: `At(times)` maps each time through `store::frame_at`. `Every(n)` does the same for 0, `n`, `2n`, and so on while the time is at most `duration_ms`. `Keyframes` returns the frames whose `DiffEntry.keyframe` is true.
- `frame_refs`: without `max_width`, `dir.frame_path` of each entry. With it, a frame wider than `max_width` is scaled to that width and saved at `dir.sample_path`; a frame that is not wider keeps its stored path.

### `src/encode/`

```rust
// mod.rs
pub mod gif; pub mod ffmpeg;                     // `pub mod ffmpeg;` is added by Task 14
pub const RING_MS: u64 = 400;
pub struct OutFrame { pub entry: FrameEntry, pub t_ms: u64, pub duration_ms: u64, pub rings: Vec<(i32, i32)> }
pub fn timeline(manifest: &Manifest) -> Vec<OutFrame>;
pub fn render(dir: &RecordingDir, frame: &OutFrame) -> Result<image::RgbaImage>;

// gif.rs
pub fn encode(dir: &RecordingDir, manifest: &Manifest) -> Result<PathBuf>;   // writes dir.gif_path()

// ffmpeg.rs
pub fn available() -> bool;                                                   // `ffmpeg -version` exits 0
pub fn encode(dir: &RecordingDir, manifest: &Manifest) -> Result<PathBuf>;   // writes dir.mp4_path()
```

- `timeline`: each stored frame becomes the span from its `t_ms` to the next frame's `t_ms` (the last ends at `duration_ms`). Each span is split at every `click.t_ms` and `click.t_ms + RING_MS` that falls strictly inside it. A piece carries the position of each click with `click.t_ms <= piece.t_ms < click.t_ms + RING_MS`. Pieces of zero length are dropped.
- `render`: `dir.load_frame`, then `draw::ring` with radius 14 for each ring.
- `gif::encode`: one GIF frame per `OutFrame` through `gif::Frame::from_rgba_speed(width, height, &mut rgba, 10)`, delay `max(2, round(duration_ms / 10))` hundredths of a second, `Repeat::Infinite`.
- `ffmpeg::encode`: runs `ffmpeg -y -loglevel error -f rawvideo -pix_fmt rgba -s <W>x<H> -r <fps> -i - -vf pad=ceil(iw/2)*2:ceil(ih/2)*2 -c:v libx264 -pix_fmt yuv420p -movflags +faststart <out>` and writes raw RGBA to its stdin. For `k` in `0..ceil(duration_ms * fps / 1000)` it writes the rendered `OutFrame` covering time `k * 1000 / fps`. A non-zero exit is `EncodeFailed` carrying ffmpeg's stderr.

### `src/recorder.rs`

```rust
pub fn run(dir: &RecordingDir) -> Result<RecordingResult>;
```

`run` has two private steps.

`capture(dir, &mut state) -> Result<(Manifest, Option<i32>)>`:

1. Open the source with `capture::open(&state.resolved, fps, cursor)`.
2. Wait for the first frame with `next_event(100 ms)` in a loop, for up to 5 s. No frame is `CaptureFailed("no frame arrived within 5 seconds")`. A `Closed` first is `CaptureFailed("the target closed before the first frame")`.
3. The first frame fixes the recording's `width` and `height`. Take `started_at_unix_ms = time::now_unix_ms()` and an `Instant` at this same moment; every `t_ms` in the recording, for frames, marks and clicks, counts from here. Write the state with `status: Recording`, the process's own `pid`, `started_at_unix_ms`, `width`, `height`. Create the `FrameWriter` and push the first frame at `t_ms` 0.
4. Start `input::ClickHook` when `options.clicks` is true and the target is not `Fake`. Start the wrapped command when `options.command` is `Some`: `Command::new("cmd").arg("/C").args(&command)`, with `stdout(Stdio::from(std::io::stderr()))` and stderr inherited. `cmd /C` is needed because `npx` and `npm` are `.cmd` files that cannot be started directly.
5. Loop on `next_event(50 ms)`. A frame is ignored when less than `1000 / fps` ms have passed since the last accepted frame; otherwise it goes to `FrameWriter::push` with the elapsed milliseconds. After each event or timeout, check in this order and stop at the first that holds:
   - `CaptureEvent::Closed`: `WindowClosed`
   - `dir.stop_requested()`: `Stopped`
   - the child's `try_wait` returned an exit status: `CommandExited`, and its code is the second return value
   - elapsed >= `duration_ms` (when set): `DurationReached`
   - elapsed >= `max_duration_ms`: `CapReached`
   - `writer.bytes() >= max_size_bytes`: `SizeCapReached`
6. `duration_ms` is the elapsed time at the moment the stop condition was seen. Drop the source. If a wrapped command is still running, end its whole process tree by running `taskkill /T /F /PID <child id>` with its output discarded, then wait for the child; `command_exit_code` stays `None`. (`Child::kill` alone would end `cmd.exe` and leave the real command running.) Finish the hook. Build the `Manifest`: marks from `dir.read_marks()` with `t_ms = unix_ms - started_at_unix_ms`, keeping those in `0..=duration_ms`; clicks from the hook; `dropped_frames` from `source.dropped()`.

`finalize(dir, manifest, options, command_exit_code) -> Result<RecordingResult>`: `dir.write_manifest`; `analyze::run`; `analyze::sheet::contact_sheet`; `encode::gif::encode` and `encode::ffmpeg::encode` for the formats asked for; build the result (`keyframes` through `analyze::select::select(.., Keyframes)` and `frame_refs(.., None)`); `dir.write_result`; write the state with `status: Finished`.

### `src/request.rs`

```rust
pub const DEFAULT_FPS: u32 = 15;
pub const DEFAULT_MAX_DURATION: &str = "5m";
pub const DEFAULT_MAX_SIZE: &str = "2GB";
pub const DEFAULT_FORMAT: &str = "gif";
pub const DEFAULT_OUT: &str = ".recdr";
pub const FAKE_SOURCE_ENV: &str = "RECDR_FAKE_SOURCE";

#[derive(Debug, Clone, Default, serde::Deserialize)]   // Task 15 adds schemars::JsonSchema
#[serde(default)]
pub struct RecordRequest {
    pub window_id: Option<u64>, pub window: Option<String>, pub process: Option<String>,
    pub region: Option<String>, pub monitor: Option<usize>,
    pub fps: Option<u32>, pub max_duration: Option<String>, pub max_size: Option<String>,
    pub format: Option<String>, pub no_cursor: bool, pub no_click_highlight: bool,
    pub out: Option<PathBuf>, pub duration: Option<String>,
    #[serde(skip)] pub command: Option<Vec<String>>,
}
impl RecordRequest { pub fn into_options(self) -> Result<RecordOptions>; }
pub fn parse_size(text: &str) -> Result<u64>;            // "500MB" | "2GB" -> bytes, 1 MB = 1_000_000; anything else is Usage
pub fn parse_region(text: &str) -> Result<Rect>;         // "x,y,w,h"; w and h must be above 0
pub fn parse_formats(text: &str) -> Result<Vec<Format>>; // "gif" | "mp4" | "gif,mp4" | "mp4,gif"
pub fn resolve_out(out: Option<PathBuf>) -> Result<PathBuf>; // default DEFAULT_OUT; made absolute against the current directory
pub fn parse_selection(at: &[String], every: Option<&str>, keyframes: bool) -> Result<FrameSelection>;
```

`parse_selection`: exactly one of a non-empty `at`, `every`, and `keyframes == true`, else `Usage`. Each `at` item and `every` go through `time::parse_time`. An `every` of 0 is `Usage`. `parse_formats` drops a repeated format, so `gif,gif` is `[Gif]`.

`into_options`, every failure being `Usage`, in this order:

1. Exactly one of `window_id`, `window`, `process`, `region`, `monitor` must be set.
2. When the environment variable `RECDR_FAKE_SOURCE` is set, it must be `static`, `moving`, `closing` or `broken`, and the target becomes `Target::Fake` of that kind, whatever was given in step 1.
3. `fps` must be 1 to 60. `duration`, when set, must not exceed `max_duration`.
4. `duration` and `command` must not both be set. Whether one of them is required depends on the operation, so `control` checks that.

### `src/control.rs`

```rust
pub fn list_windows() -> Result<WindowList>;
pub fn list_monitors() -> Result<MonitorList>;
pub fn record(options: RecordOptions) -> Result<RecordingResult>;      // in this process
pub fn start(options: RecordOptions) -> Result<StartInfo>;             // detached
pub fn wait(out: &Path, id: &str) -> Result<RecordingResult>;          // until the detached recorder finishes by itself
pub fn stop(out: &Path, id: &str) -> Result<RecordingResult>;
pub fn mark(out: &Path, id: &str, label: &str) -> Result<MarkAck>;
pub fn status(out: &Path, id: Option<&str>) -> Result<StatusList>;
pub fn frames(out: &Path, id: &str, selection: &FrameSelection, max_width: Option<u32>) -> Result<FrameList>;
pub fn run_recorder(dir: PathBuf) -> Result<()>;                       // body of the hidden `_recorder` command
```

Private helpers:

- `prepare(options) -> Result<RecordingDir>`: `FfmpegNotFound` when `Mp4` is asked for and `encode::ffmpeg::available()` is false. It lists windows only for a window target, monitors only for a region or monitor target, and neither for `Fake`. It calls `capture::resolve`, then `RecordingDir::create`, then writes the state with `status: Starting`, `pid` 0, `width` 0, `height` 0, `started_at_unix_ms` = now. Nothing is created on disk when an earlier step fails.
- `derived_status(dir) -> Result<Status>`: `Finished` when `read_result()` is `Some`. Otherwise `Recording` when the stored status is `Starting` and less than 5 s have passed since `started_at_unix_ms`. Otherwise `Recording` when `platform::process_alive(pid)`. Otherwise `Crashed`. It never returns `Starting`.
- `await_result(dir) -> Result<RecordingResult>`: every 50 ms, return `read_result()` when it is `Some`; return `RecorderCrashed` when `derived_status` is `Crashed`.

Public functions:

- `list_windows`, `list_monitors`: the `capture` function of the same name, wrapped in the list type.
- `record`: `Usage("record needs --duration or a command after --")` when `duration_ms` and `command` are both `None`; then `prepare`, then `recorder::run`.
- `start`: `Usage("start does not take a command")` when `command` is `Some`; then `prepare`; `platform::spawn_detached(current_exe, ["_recorder", <dir>], dir.log_file())`; then every 50 ms for up to 5 s read the state. A stored status of `Recording` or `Finished` returns `StartInfo` with the state's `width` and `height`. The child pid no longer alive, or the 5 s passing, is `RecorderStartFailed(dir.last_log_line())`.
- `wait`: `RecordingDir::open`, then `await_result`.
- `stop`: `RecordingDir::open`; when `read_result()` is `Some`, return it; otherwise `request_stop()` and `await_result`.
- `mark`: `RecordingDir::open` (so an unknown id is `RecordingNotFound`); `derived_status` must be `Recording`, else `RecordingNotActive`; `append_mark(time::now_unix_ms(), label)`.
- `status`: one `RecordingDir::open`, or `RecordingDir::list`. `started_at` is the state's `started_at_unix_ms`. `elapsed_ms` is the result's `duration_ms` when finished, else now minus `started_at_unix_ms`.
- `frames`: `RecordingDir::open`; `RecordingNotFound` when `read_result()` is `None`; `read_manifest`, `read_diff`, `analyze::select::select`, `analyze::select::frame_refs`.
- `run_recorder`: `recorder::run(&RecordingDir::at(dir))`, discarding the result value. On `Err` it writes the error's message to stderr, which the starter pointed at the recording's log file, and returns the error.

### `src/cli.rs`

```rust
pub fn run(serve_mcp: fn() -> Result<()>) -> i32;   // the Mcp subcommand calls serve_mcp
```

clap types, all private to this file:

```rust
struct Cli { command: Command }
enum Command { Windows, Monitors, Record(RecordCmd), Start(StartCmd), Stop(IdCmd), Mark(MarkCmd), Status(StatusCmd), Frames(FramesCmd), Mcp, #[command(name = "_recorder", hide = true)] Recorder { dir: PathBuf } }
struct TargetFlags { window_id: Option<u64>, window: Option<String>, process: Option<String>, region: Option<String>, monitor: Option<usize> }
struct RecordFlags { target: TargetFlags, fps: Option<u32>, max_duration: Option<String>, max_size: Option<String>, format: Option<String>, no_cursor: bool, no_click_highlight: bool, out: Option<PathBuf> }
struct RecordCmd { flags: RecordFlags, duration: Option<String>, #[arg(last = true)] command: Vec<String> }
struct StartCmd { flags: RecordFlags }
struct IdCmd { id: String, out: Option<PathBuf> }
struct MarkCmd { id: String, label: String, out: Option<PathBuf> }
struct StatusCmd { id: Option<String>, out: Option<PathBuf> }
struct FramesCmd { id: String, at: Vec<String>, every: Option<String>, keyframes: bool, max_width: Option<u32>, out: Option<PathBuf> }
impl RecordFlags { fn into_request(self) -> RecordRequest }
```

- clap attributes: `flags` and `target` are `#[command(flatten)]`. `id`, `label` and `dir` are positional. Every other field is a long flag named after the field with `-` for `_` (`--window-id`, `--max-duration`, `--no-click-highlight`), matching the spec's Commands section. `--at` has `value_delimiter = ','` and is a `Vec<String>`.
- `run` calls `Cli::try_parse()`. `--help` and `--version` print clap's text and return 0. Any other clap error becomes `Error::Usage` with clap's message.
- Flags carry no clap defaults; the defaults live in `request.rs` and the help text of each flag names them.
- `frames` builds its selection with `request::parse_selection`.
- Every `out` goes through `request::resolve_out`.
- Each command prints exactly what its `control` function returns, serialized with `serde_json::to_string`, on stdout, and returns 0. The hidden `_recorder` command prints nothing. Failure prints `Error::to_json` on stderr and returns `exit_code()`.

### `src/mcp.rs`

```rust
pub fn serve() -> Result<()>;
```

`serve` builds a current-thread tokio runtime and serves `RecdrServer` on `rmcp::transport::stdio()`. `RecdrServer` is an empty struct whose `impl` has `#[tool_router(server_handler)]`, following `rmcp-macros-3.11.0/README.md` in the Cargo registry. `cli.rs` does not import `mcp`; `main.rs` passes `serve` to `cli::run`.

Parameter structs, private to this file, each deriving `Deserialize` and `JsonSchema`:

```rust
struct IdParams { id: String, out: Option<PathBuf> }
struct MarkParams { id: String, label: String, out: Option<PathBuf> }
struct StatusParams { id: Option<String>, out: Option<PathBuf> }
struct FramesParams { id: String, at: Option<Vec<String>>, every: Option<String>, keyframes: Option<bool>, max_width: Option<u32>, out: Option<PathBuf> }
```

The server reports the name `recdr` and the version `env!("CARGO_PKG_VERSION")`.

Tools. Each runs its `control` call inside `tokio::task::spawn_blocking`. A success is `CallToolResult::success` whose first block is the text `serde_json::to_string` of what `control` returned, the same JSON the CLI prints. A failure is `CallToolResult::error` with `Error::to_json` as text. Every `out` goes through `request::resolve_out`.

| Tool | Parameters | Calls | Extra content | Description |
|---|---|---|---|---|
| `list_windows` | none | `control::list_windows` | | List visible windows with the id to pass as window_id. |
| `list_monitors` | none | `control::list_monitors` | | List monitors with the index to pass as monitor. |
| `start_recording` | `RecordRequest` | `into_options`, `control::start` | | Start recording a window, region or monitor in the background and return its id. Run list_windows first and pass window_id. |
| `stop_recording` | `IdParams` | `control::stop` | contact sheet as `ContentBlock::image(base64, "image/png")` | Stop a recording and return its files and a contact sheet of what changed. |
| `record` | `RecordRequest` | see below | same as `stop_recording` | Record for a fixed duration such as 5s and return the files and a contact sheet. Run list_windows first and pass window_id. |
| `mark` | `MarkParams` | `control::mark` | | Label the current moment of a running recording. |
| `get_status` | `StatusParams` | `control::status` | | Show whether recordings are running, finished or crashed. |
| `get_frames` | `FramesParams` | see below | one image block per frame | Return frames of a finished recording at given times, at an interval, or its keyframes. At most 8 per call. |

- `record`: `Usage("record needs duration")` when `duration` is `None`; then `into_options`, `control::start`, `control::wait`.
- `get_frames`: `request::parse_selection(at.unwrap_or_default(), every, keyframes.unwrap_or(false))`, then `control::frames` with `max_width.unwrap_or(1024)`. More than 8 frames in the returned list is `Error::TooManyFrames`.

### `src/main.rs`

```rust
fn main() {
    recdr::platform::set_dpi_aware();
    std::process::exit(recdr::cli::run(recdr::mcp::serve))
}
```

`main.rs` is the composition root: the only file that names both front ends, and the place where process-wide set-up happens. It hands the MCP server to the CLI as a function, so `cli.rs` never imports `mcp`. Until Task 15 creates `mcp.rs`, `main.rs` passes a private function `fn no_mcp() -> recdr::Result<()>` that returns `Error::Usage("the mcp command is not built yet")`. Until Task 2 creates `platform.rs`, the first line is absent.

### `tests/layers.rs`

Five tests, one per layer rule. Each reads the `.rs` files under `src/` as text, drops every line whose first non-space characters are `//`, looks for plain substrings, and fails with the path and the substring it found:

1. Any of the nine quoted file names of rule 1, in a file other than `src/store.rs`.
2. `clap` in a file other than `src/cli.rs`; `rmcp`, `tokio` or `base64` in a file other than `src/mcp.rs`.
3. `unsafe` in a file other than `src/capture/windows.rs`, `src/input.rs` and `src/platform.rs`.
4. In `src/cli.rs` and `src/mcp.rs`: any of `recorder::`, `capture::`, `input::`, `store::`, `analyze::`, `encode::`, `draw::`, `platform::`, `time::`, or `crate::{`. Also `mcp::` in `src/cli.rs` and `cli::` in `src/mcp.rs`. The ban on `crate::{` means the front ends write one `use crate::<module>::...;` line per module, so the other substrings cannot be hidden in a group. Neither front end needs `std::time`; if one ever does, it imports the item by name (`use std::time::Duration;` would trip `time::`, so the rule is changed first).
5. `fn ` in `src/model.rs`.

## Other decisions

- **Frame writing** is synchronous in the recorder loop, fed by the capacity-8 channel. Task 16 measures success criterion 15. If more than 5% of frames drop, the task stops and reports the numbers to the owner; no fallback is designed in advance.
- **Tests** call the built binary through `env!("CARGO_BIN_EXE_recdr")` with `--out` set to a fresh directory under `std::env::temp_dir()`, and set `RECDR_FAKE_SOURCE`. Shared helpers live in `tests/common/mod.rs`, whose first line is `#![allow(dead_code)]` because each test file uses a different subset.
- **Desktop tests** open their own window so they never depend on what the tester has open. `tests/common/mod.rs` has `test_window(title, minimized) -> std::process::Child`, which runs `powershell -NoProfile -Command "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form; $f.Text = '<title>'; $f.Width = 640; $f.Height = 480; [if minimized: $f.WindowState = 'Minimized';] [Windows.Forms.Application]::Run($f)"`. The title is `recdr-test-<process id>-<name>`. The test finds the window with `--window "<title>"` and closes it with `Child::kill`.
- **`windows-capture` ships a `VideoEncoder`** (`src/encoder.rs` in the crate). It is not used here. It is the likely starting point for module `native-mp4`.
- **Commits**: one per task, message `feat(agent-recorder): <task title in lowercase>`.

## File boundary

Create:
- `Cargo.toml`
- `Cargo.lock`
- `.gitignore`
- `src/main.rs`
- `src/lib.rs`
- `src/error.rs`
- `src/model.rs`
- `src/time.rs`
- `src/platform.rs`
- `src/draw.rs`
- `src/store.rs`
- `src/input.rs`
- `src/recorder.rs`
- `src/request.rs`
- `src/control.rs`
- `src/cli.rs`
- `src/mcp.rs`
- `src/capture/mod.rs`
- `src/capture/windows.rs`
- `src/capture/fake.rs`
- `src/analyze/mod.rs`
- `src/analyze/diff.rs`
- `src/analyze/sheet.rs`
- `src/analyze/select.rs`
- `src/encode/mod.rs`
- `src/encode/gif.rs`
- `src/encode/ffmpeg.rs`
- `tests/common/mod.rs`
- `tests/cli.rs`
- `tests/record.rs`
- `tests/control.rs`
- `tests/frames.rs`
- `tests/mcp.rs`
- `tests/desktop.rs`
- `tests/layers.rs`
- `tasks/1-recdr/acceptance.md`

Modify:
- `README.md`

Delete:
- none

`spec/1-recdr/` and `tasks/1-recdr/` are committed as they are in Task 1. After that no task edits them, with two exceptions: `/build` ticks boxes in `tasks/1-recdr/todo.md`, and Task 16 creates `tasks/1-recdr/acceptance.md`.

## Task list

Full task text is in [todo.md](todo.md).

### Phase 1: a recording on disk
- Task 1: Package scaffold and the JSON error contract
- Task 2: `recdr windows` and `recdr monitors`
- Task 3: The recording directory
- Task 4: `recdr record --duration` with the synthetic source
- Task 5: WGC backend for monitor, region and window

### Checkpoint A
- A real monitor recording leaves PNG frames and a manifest on disk.

### Phase 2: outputs
- Task 6: GIF encoding
- Task 7: Keyframes, `diff.json` and the contact sheet
- Task 8: `recdr frames`

### Checkpoint B
- User story 2 works end to end on the real desktop.

### Phase 3: control
- Task 9: `recdr start`, `stop` and `status`
- Task 10: `recdr mark`
- Task 11: Wrap a command

### Checkpoint C
- User stories 1, 3, 4 and 5 work from the CLI.

### Phase 4: the rest of the surface
- Task 12: Window close and minimize on the real desktop
- Task 13: Cursor flag, click hook and click rings
- Task 14: MP4 through ffmpeg
- Task 15: MCP server
- Task 16: README and the acceptance run

### Checkpoint: complete
- All 17 success criteria pass.

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Synchronous PNG writing drops more than 5% of frames at 1080p | Medium | Measured in Task 16. On failure the task stops and the owner decides; `FrameWriter::push` is the only place that would change. |
| `start_free_threaded` does not deliver frames from a detached process with no console | High | Task 5 is the fifth task and has desktop tests; Task 9 repeats one of them through `start`. If it fails, run the crate's blocking `start` on a thread owned by `WgcSource`; nothing outside `capture/windows.rs` changes. |
| WGC delivers a window frame whose size differs from `GetWindowRect` | Low | The recording's size is the first frame's size, never the listed bounds. |
| A detached recorder is killed when the starter's console closes | Medium | `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`, and a Task 9 test that stops the recording from a different process than the one that started it. |
| The mouse hook delays system mouse input | Medium | The hook procedure only sends on a channel and calls `CallNextHookEx`. |
| `cmd /C` mis-parses a wrapped command | Low | The command's arguments are passed as separate arguments after `/C`. The Task 11 test command contains spaces and `&`. |

## Open questions

None.
