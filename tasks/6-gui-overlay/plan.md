# Implementation plan: gui-overlay

Spec: [spec/6-gui-overlay/spec.md](../../spec/6-gui-overlay/spec.md). Task list: [todo.md](todo.md). Tracking issue: https://github.com/dzikrisyairozi/recdr/issues/6.

All paths are relative to the repository root. "The spec" means the file above. "Criterion N" means success criterion N of the spec.

## Overview

Three things are built, in this order:

1. **Export**, on every system: `recdr export`, the MCP tool `export`, and `--format none`. It is plain library code with automated tests, and the app needs it.
2. **The logic part of the app**, on every system: which state the app is in, what each event does, and the arithmetic on frames, toolbars and crops. It has unit tests and names no `egui` type.
3. **The windows part of the app**, on Windows and macOS: what is drawn, and how the mouse becomes events. It is checked by hand, first on Windows, then on the owner's Mac.

### When this plan was written

On 2026-11-18, before the three modules in front of this one were built. It describes the code as it will be after `cdp-capture`, `macos-capture` and `native-mp4`, and takes that from their plans:

- From [tasks/3-cdp-capture/plan.md](../3-cdp-capture/plan.md): `Error::UnsupportedPlatform` with the code `unsupported_platform`; the layer tests, the sixth of which lists the files that may hold `cfg`; `Server::start(fake: Option<&str>)` in `tests/common/mod.rs`; CI that runs on pull requests, with one job on three runners.
- From [tasks/5-native-mp4/plan.md](../5-native-mp4/plan.md): `encode::mp4(dir, manifest) -> Result<PathBuf>`, `encode::check_mp4() -> Result<()>`, the private `write` and `video_frames` in `src/encode/mod.rs`, the test helper `recording(name, image, duration_ms, fps, clicks)` there, a seventh layer test, `tests/common/mp4.rs`, and possibly the change that plan calls "If the Windows runner has no encoder".
- From [tasks/4-macos-capture/plan.md](../4-macos-capture/plan.md): `recdr windows` and window and region targets work on macOS; the message of `unsupported_platform` is `window, region and monitor capture need Windows or macOS; on this system recdr records browser tabs with --cdp`; and `src/capture/mod.rs` has public functions named `to_pixels` and `to_points`.

Checked by reading the files on branch `feat/1-agent-recorder`: `src/model.rs`, `src/request.rs`, `src/control.rs`, `src/cli.rs`, `src/mcp.rs`, `src/main.rs`, `src/encode/mod.rs`, `src/encode/gif.rs`, `src/store.rs`, and line 164 onward of `src/recorder.rs`.

"Before Task 1" in [todo.md](todo.md) checks the first list against the real code. A difference is fixed in this plan before any task starts.

### What was measured

Nothing was run for this plan. The spec's "What was checked" lists what was read from `egui` 0.37.0. Task 9 runs a throwaway window on Windows, Task 14 runs it on the Mac, and both write their results here.

## Architecture decisions

Each is a choice the spec left to the plan.

1. **Export options carry the id and the output root.** `ExportOptions` is to `control::export` what `RecordOptions` is to `control::record`: one value, already checked, with `out` inside. A front end writes `request.into_options().and_then(control::export)`.
2. **An export is written next to its target and renamed.** `export::run` gives the encoder the path `<name>.part.<ext>` in the target's directory and renames it over the target when the encoder succeeds. A failed export removes the `.part` file and leaves an existing target as it was. The extension is kept last because ffmpeg picks the container from it.
3. **A file that cannot be written is `encode_failed`.** Inside `export::run` an `Error::Io` from the encoder becomes `Error::EncodeFailed` with the same text. Recording keeps its `io_error`.
4. **The encoders are told the crop and the path, and return a count.** `encode::gif::encode` and `encode::mp4` take `crop: Option<&Rect>` and `path: &Path`, and return the number of pictures written. `recorder::finalize` passes `None` and the path in the recording directory.
5. **Every event goes through one function with the time.** `state::step(app, event, now_ms)` returns the actions to run. The logic part never reads a clock, so a test can move time by hand.
6. **Actions run on threads, in a function that does not know `egui`.** `worker::perform(out, action)` calls `control` and returns the event that reports the outcome. The windows part starts one thread per action and feeds the events back. `control::stop` and `control::export` can take seconds, and the windows must keep drawing.
7. **The logic part is `pub`.** `geometry`, `state` and `worker` are public modules of the public module `gui`. On Linux nothing calls them, and a private item nobody calls fails the build under `clippy -D warnings`.
8. **The frame is four border windows, and the toolbar is the app's main window.** This is the way that needs no call outside `egui` (spec, Risks). Each border window is 8 pixels thick, so the mouse can catch it; the inner 2 pixels are drawn. Task 9 confirms it on Windows and Task 14 on macOS. If either shows that it cannot work, the build stops: the spec says a system call for the windows is a change to the spec first.
9. **The editor is the main window, dressed differently.** On entering the editor the main window gets decorations, the normal window level and a size of 960 by 640 points, and the border windows are not shown. On "New recording" it goes back. One `eframe` app with one main window needs no second event loop.
10. **The preview asks for one stored frame at a time.** The logic part emits `Preview` for the current position only when no earlier `Preview` is unanswered. Playback then shows frames as fast as they load and never queues.
11. **The preview shows stored frames at full size.** `control::frames` is called with no `max_width`, so nothing is written into the recording directory by the editor.
12. **`gui::run` is handed to the CLI by `main`.** `cli::run` takes a second function, as it takes the MCP server today, so front ends still do not import each other.
13. **A drag is measured from where it began.** For a drag on the toolbar or on a border window, the windows part remembers the inside rectangle and the pointer's position on the screen, in pixels, at the press. Each pass sends the rectangle of the press changed by the whole distance moved since then. A border window moves under the pointer while it is dragged, so distances measured inside the window, or added up pass by pass, would feed on themselves.
14. **`recdr gui` on Linux uses the message `unsupported_platform` already has.** That text, after `macos-capture`, is `window, region and monitor capture need Windows or macOS; on this system recdr records browser tabs with --cdp`. It is about capture, and its second half does not help someone who asked for the app. The spec fixes only the code, and a text of its own would mean a change to `src/error.rs` and to the tests of two earlier modules. The README says that the app needs Windows or macOS.
15. **The save dialog proposes `export.gif` and `export.mp4`.** The first layer test allows the names `recording.gif` and `recording.mp4` in `src/store.rs` only. It reads every file under `src/`, the test modules in them included, and `src/gui/editor.rs` is one of those files.

### Spec changes made while planning

1. **Moving the frame.** The spec said the frame is moved "by dragging its border or the toolbar" and resized "by dragging a corner or an edge". The border is the edge, so one drag had two meanings. Now: the toolbar moves the frame, a corner or an edge resizes it.
2. **The size in the export result.** The spec did not say which size `width` and `height` are when an MP4 is padded to an even size. Now: they are the size of the pictures in the file.
3. **The example time.** The spec's flag table gave `1.5s` as an example for `--from`. `time::parse_time` takes whole numbers only, and the spec says no second parser is written. The example is now `2s`.

## Module contracts

A task implements these exactly as written. Anything not listed keeps the signature it has after `native-mp4`.

### `src/model.rs`

`WindowInfo` and `MonitorInfo` gain `PartialEq` in their `derive` (Task 7): the app's state holds lists of both and is compared in tests. `Rect`, which both contain, has it already.

```rust
/// A checked request to export part of a finished recording.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportOptions {
    pub id: String,
    pub out: PathBuf,
    pub file: PathBuf,
    pub format: Format,
    pub from_ms: Option<u64>,
    pub until_ms: Option<u64>,
    pub crop: Option<Rect>,
}

/// Printed by `export`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportResult {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub frames: u64,
}
```

### `src/request.rs`

`parse_formats`: the text `none`, alone, gives `Ok(Vec::new())`. `none` with anything else gives `Error::Usage("none cannot be combined with gif or mp4".into())`. The message for an unknown word becomes `{other:?} is not gif, mp4 or none`.

`RecordRequest` gains `PartialEq` in its `derive` (Task 7), and its field `format` gains the doc comment `/// gif, mp4, "gif,mp4" or none; default gif` (Task 1), which is what the MCP tools show for that argument. No other field of it has a doc comment today.

```rust
/// What a user asked `export` for, before validation. Also the MCP tool's arguments.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
pub struct ExportRequest {
    /// Recording id
    pub id: String,
    /// Where to write; the extension picks the format, .gif or .mp4
    pub file: PathBuf,
    /// Start of the part to keep, such as "1500ms" or "2s"; default the start of the recording
    pub from: Option<String>,
    /// End of the part to keep; default the end of the recording
    pub until: Option<String>,
    /// Rectangle to keep, "x,y,width,height" in pixels; default the whole picture
    pub crop: Option<String>,
    /// Output root; default .recdr in the working directory
    pub out: Option<PathBuf>,
}

impl ExportRequest {
    pub fn into_options(self) -> Result<ExportOptions>
}
```

`into_options`: the format is `Format::Gif` when the extension of `file`, lowercased, is `gif`, `Format::Mp4` when it is `mp4`, and otherwise `Error::Usage("--file must end in .gif or .mp4".into())`. `from` and `until` go through `parse_time`, `crop` through `parse_region`, `out` through `resolve_out`, and `file` through `std::path::absolute`.

### `src/encode/mod.rs` and `src/encode/gif.rs`

```rust
// src/encode/mod.rs
pub fn render(dir: &RecordingDir, frame: &OutFrame, crop: Option<&Rect>) -> Result<RgbaImage>
pub fn mp4(dir: &RecordingDir, manifest: &Manifest, crop: Option<&Rect>, path: &Path) -> Result<u64>
fn video_frames(dir: &RecordingDir, manifest: &Manifest, crop: Option<&Rect>,
                each: impl FnMut(u64, &RgbaImage) -> Result<()>) -> Result<u64>
fn write(dir: &RecordingDir, manifest: &Manifest, crop: Option<&Rect>, path: &Path) -> Result<u64>

// src/encode/gif.rs
pub fn encode(dir: &RecordingDir, manifest: &Manifest, crop: Option<&Rect>, path: &Path) -> Result<u64>
```

- `render`: after loading the stored frame and before drawing rings, a crop replaces the picture with `image::imageops::crop_imm(&image, crop.x as u32, crop.y as u32, crop.width, crop.height).to_image()`. The caller has checked that the crop is inside the picture.
- `mp4`: as after `native-mp4`, with `path` in place of `dir.mp4_path()`, the crop passed down to `render`, and the count from `video_frames` returned. `write` ends with `pushed.and_then(|count| finished.map(|()| count))`: the writer is still always finished, and an error from the frames still wins over one from `finish`.
- After `native-mp4`, `render` is called from `video_frames` and from `gif::encode` and nowhere else, so the three MP4 writers do not change.
- `gif::encode`: as today, with `path` in place of `dir.gif_path()`, the crop passed to `render`, and the number of GIF frames written returned.
- The manifest an encoder gets already has the size of the pictures it will be given: `export::kept` sets `width` and `height` to the crop's.
- The test helpers and the existing unit tests pass `None` and the path they used before. A test that read the path a function returned, such as `a_click_adds_frames_for_its_ring` in `src/encode/gif.rs`, which counts the frames of the file `encode` returns, now reads the path it passed.

### `src/recorder.rs`

In `finalize`:

```rust
let gif = if state.options.formats.contains(&Format::Gif) {
    encode::gif::encode(dir, &manifest, None, &dir.gif_path())?;
    Some(dir.gif_path())
} else {
    None
};
```

and the same shape for `mp4` with `encode::mp4` and `dir.mp4_path()`.

### `src/export.rs`

```rust
/// The manifest of the part of a recording between two times, cut to `crop`.
pub fn kept(manifest: &Manifest, from_ms: u64, until_ms: u64, crop: Option<&Rect>) -> Manifest

/// Checks the options against the recording and writes the file.
pub fn run(dir: &RecordingDir, options: &ExportOptions) -> Result<ExportResult>
```

`kept` copies `id`, `fps`, `started_at_unix_ms`, `stop_reason` and `dropped_frames`, and sets:

- `frames`: first the entry `store::frame_at(&manifest.frames, from_ms)` returns, with `t_ms` 0; then every entry with `from_ms < t_ms < until_ms`, with `t_ms - from_ms`. `index` and `file` are not changed. An empty list stays empty.
- `clicks`: every click with `from_ms <= t_ms < until_ms` that is inside the crop (`crop.x <= x < crop.x + width`, and the same for `y`), with `t_ms - from_ms`, `x - crop.x` and `y - crop.y`. With no crop only the time is checked and moved.
- `marks`: empty.
- `width`, `height`: the crop's, or the manifest's with no crop.
- `duration_ms`: `until_ms - from_ms`.

`run`, in this order. Nothing is written before step 6.

1. `dir.read_result()?` is `None`: `Error::RecordingNotFound(dir.id().to_owned())`.
2. `manifest = dir.read_manifest()?`, `from = options.from_ms.unwrap_or(0)`, `until = options.until_ms.unwrap_or(manifest.duration_ms)`.
3. Unless `from < until && until <= manifest.duration_ms`: `Error::Usage(format!("--from must be before --until, and both inside the recording (0 to {} ms)", manifest.duration_ms))`.
4. With a crop, unless `x >= 0`, `y >= 0`, `width >= 2`, `height >= 2`, `x + width <= manifest.width` and `y + height <= manifest.height` (computed in `i64`): `Error::Usage(format!("--crop must lie inside the {}x{} picture and be at least 2x2", manifest.width, manifest.height))`.
5. For `Format::Mp4`: `encode::check_mp4()?`.
6. `part = kept(&manifest, from, until, crop)`. `temp = options.file.with_extension(format!("part.{ext}"))`, where `ext` is `gif` or `mp4`. The encoder for the format is called with `(dir, &part, crop, &temp)`.
7. On an error: `temp` is removed when `temp.is_file()`, an `Error::Io(e)` becomes `Error::EncodeFailed(e.to_string())`, and the error is returned.
8. `std::fs::rename(&temp, &options.file)`; a failure removes `temp` and is `Error::EncodeFailed` with the error's text.
9. The result: `path` is `options.file`; `width` and `height` are `part`'s, each rounded up to even for `Format::Mp4`; `duration_ms` is `part.duration_ms`; `frames` is the encoder's count.

### `src/control.rs`

```rust
/// Writes a GIF or an MP4 from part of a finished recording.
pub fn export(options: ExportOptions) -> Result<ExportResult> {
    export::run(&RecordingDir::open(&options.out, &options.id)?, &options)
}
```

`RecordingDir::open` gives `recording_not_found` for an unknown id.

### `src/cli.rs`

Two new variants of `Command`:

```rust
/// Write a GIF or an MP4 from part of a finished recording
Export(ExportCmd),
/// Open the recorder window
Gui(GuiCmd),
```

`ExportCmd` has the positional `id: String` and the flags `--file <PathBuf>` (required), `--from`, `--until`, `--crop` (with `allow_hyphen_values = true`, as `--region` has) and `--out`, each documented with the text of the spec's flag table. `GuiCmd` has `--out`.

`run` becomes `pub fn run(serve_mcp: fn() -> Result<()>, open_gui: fn(PathBuf) -> Result<()>) -> i32`, with the arms:

```rust
Command::Export(cmd) => text(cmd.into_request().into_options().and_then(control::export)),
Command::Gui(cmd) => request::resolve_out(cmd.out).and_then(open_gui).map(|()| None),
```

`ExportCmd::into_request` builds the `ExportRequest` field by field, as `RecordFlags::into_request` does. The help text of `--format` becomes `gif, mp4, gif,mp4 or none [default: gif]`.

`src/main.rs` becomes `std::process::exit(recdr::cli::run(recdr::mcp::serve, recdr::gui::run))`, after the `set_dpi_aware` call it has.

### `src/mcp.rs`

```rust
#[tool(
    description = "Write a GIF or an MP4 from a finished recording, optionally trimmed with from and until and cropped with crop."
)]
async fn export(
    &self,
    Parameters(params): Parameters<ExportRequest>,
) -> std::result::Result<CallToolResult, ErrorData> {
    let exported = run(move || params.into_options().and_then(control::export));
    Ok(reply(exported.await, |_| Ok(Vec::new())))
}
```

### `src/gui/mod.rs`

```rust
pub mod geometry;
pub mod state;
pub mod worker;

#[cfg(any(windows, target_os = "macos"))]
mod app;
#[cfg(any(windows, target_os = "macos"))]
mod editor;
#[cfg(any(windows, target_os = "macos"))]
mod overlay;

/// Opens the app and returns when it is closed.
#[cfg(any(windows, target_os = "macos"))]
pub fn run(out: PathBuf) -> Result<()> { app::run(out) }

#[cfg(not(any(windows, target_os = "macos")))]
pub fn run(_out: PathBuf) -> Result<()> { Err(Error::UnsupportedPlatform) }
```

Until Task 10 the file has the `pub mod` lines that exist so far and, from Task 8, one `run` with no `cfg`, which returns `Err(Error::UnsupportedPlatform)` on every system. Task 10 creates the three files of the windows part together, so the lines above compile from the commit that adds them. `src/lib.rs` gains `pub mod export;` and `pub mod gui;`.

### `src/gui/geometry.rs`

Every length is in pixels unless the name says points. A `Rect` named `inside` is the recorded area: the frame without its border.

```rust
pub const BORDER: u32 = 2;                         // drawn width of the outline
pub const GRIP: u32 = 8;                           // thickness of a border window
pub const CORNER: u32 = 16;                        // length of the corner zone at each end of an edge
pub const MIN_INSIDE: u32 = 32;
pub const TOOLBAR_POINTS: (f64, f64) = (560.0, 40.0);
pub const START_INSIDE: (u32, u32) = (640, 480);
pub const MIN_KEPT_MS: u64 = 100;

pub fn to_pixels(points: f64, scale: f64) -> i32      // (points * scale).round()
pub fn to_points(pixels: f64, scale: f64) -> f64      // pixels / scale
pub fn toolbar_size(scale: f64) -> (u32, u32)         // TOOLBAR_POINTS through to_pixels

/// The monitor that holds the rectangle's center, or the first monitor.
pub fn monitor_of<'a>(rect: &Rect, monitors: &'a [MonitorInfo]) -> Option<&'a MonitorInfo>

/// The frame a new app starts with: START_INSIDE centered on the primary monitor.
pub fn initial(monitors: &[MonitorInfo]) -> Rect

/// `inside` made to fit: at least MIN_INSIDE each way, at most the monitor less the
/// grips (and less the toolbar's height), and moved wholly onto the monitor of its center.
pub fn keep_inside(inside: Rect, monitors: &[MonitorInfo], toolbar: (u32, u32)) -> Rect

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place { Under, Above, Inside }
pub fn toolbar_place(inside: &Rect, monitor: &Rect, toolbar: (u32, u32)) -> Place
pub fn toolbar_rect(inside: &Rect, monitor: &Rect, toolbar: (u32, u32)) -> Rect

/// The four border windows: top, bottom, left, right.
pub fn borders(inside: &Rect) -> [Rect; 4]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge { Top, Bottom, Left, Right, TopLeft, TopRight, BottomLeft, BottomRight }
/// Which edge or corner a press at `along` pixels from the start of border window `side` takes.
pub fn edge_at(side: usize, along: u32, length: u32) -> Edge
/// `inside` after the edge was dragged by (dx, dy) pixels.
pub fn resized(inside: &Rect, edge: Edge, dx: i32, dy: i32) -> Rect

/// The crop a drag on the preview means, in picture pixels, or None when it is under 2 by 2.
pub fn crop_of(a: (f32, f32), b: (f32, f32), shown: (f32, f32), picture: (u32, u32)) -> Option<Rect>
/// The time a position on the timeline means.
pub fn time_at(x: f32, width: f32, duration_ms: u64) -> u64
```

- `initial`: on the monitor with `primary` true, or the first; an empty list gives `Rect { x: 0, y: 0, width: 640, height: 480 }`.
- `keep_inside`, with `m` the bounds of `monitor_of(&inside, monitors)` (no monitors: `inside` is returned): `width` is clamped to `MIN_INSIDE ..= m.width - 2 * GRIP`, `height` to `MIN_INSIDE ..= m.height - 2 * GRIP - toolbar.1`, then `x` to `m.x + GRIP ..= m.x + m.width - GRIP - width` and `y` to `m.y + GRIP ..= m.y + m.height - GRIP - height`. Last, when `toolbar_place` of the result is `Inside`, `y` becomes `m.y + m.height - GRIP - toolbar.1 - height`, where the toolbar fits under the frame. So the toolbar of a region is never inside it; `Inside` is left for a chosen window, whose bounds do not go through this function.
- `toolbar_place`: `Under` when `inside.y + inside.height + GRIP + toolbar.1 <= monitor.y + monitor.height`; else `Above` when `inside.y - GRIP - toolbar.1 >= monitor.y`; else `Inside`.
- `toolbar_rect`: the size is `toolbar`. `y` is `inside.y + inside.height + GRIP` for `Under`, `inside.y - GRIP - toolbar.1` for `Above`, and `inside.y + inside.height - toolbar.1` for `Inside`. `x` is `inside.x`, clamped to `monitor.x ..= monitor.x + monitor.width - toolbar.0`.
- `borders`: top is `(inside.x - GRIP, inside.y - GRIP, inside.width + 2 * GRIP, GRIP)`, bottom the same at `y = inside.y + inside.height`; left is `(inside.x - GRIP, inside.y, GRIP, inside.height)`, right the same at `x = inside.x + inside.width`.
- `edge_at`: `side` is the index in `borders`. For top and bottom, `along < CORNER + GRIP` is the left corner and `along >= length - CORNER - GRIP` the right one. For left and right, `along < CORNER` is the upper corner and `along >= length - CORNER` the lower one. Otherwise the side's own edge.
- `resized`: an edge with "Left" in its name adds `dx` to `x` and takes it from `width`; "Right" adds `dx` to `width`; "Top" and "Bottom" do the same with `dy`, `y` and `height`. A size under `MIN_INSIDE` is set to `MIN_INSIDE` with the opposite edge kept where it was.
- `crop_of`: the two points are clamped to `0 ..= shown`, scaled by `picture / shown`, and ordered. The result is rounded to whole pixels and clamped inside the picture.
- `time_at`: `(x / width).clamp(0.0, 1.0) * duration_ms`, rounded.

### `src/gui/state.rs`

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub out: PathBuf,
    pub screen: Screen,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
    pub scale: f64,              // pixels per point where the frame is; 1.0 at the start
    pub inside: Rect,
    pub window: Option<u64>,     // the window chosen in the target menu; None is "Region"
    pub fps: u32,                // 15 at the start
    pub cursor: bool,            // true at the start
    pub clicks: bool,            // true at the start
    pub message: Option<String>, // an error, or the path of the last export
    pub last_ms: u64,            // the time of the last event
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Idle { starting: bool, closing: bool },
    Recording { id: String, started_ms: u64, asked_ms: u64, stopping: bool, closing: bool },
    Editing(Edit),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub from_ms: u64,
    pub until_ms: u64,
    pub crop: Option<Rect>,
    pub position_ms: u64,
    pub playing: bool,
    pub picture: Option<(u64, PathBuf)>,   // the stored frame on show, and the time it was asked for
    pub loading: bool,
    pub exporting: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    // from the windows part
    Tick,
    Scale(f64),
    MenuOpened,
    Picked(Option<u64>),
    Moved { x: i32, y: i32 },
    Resized(Rect),
    Fps(u32),
    Cursor(bool),
    Clicks(bool),
    RecordPressed,
    StopPressed,
    PlayPressed,
    Seek(u64),
    From(u64),
    Until(u64),
    Crop(Option<Rect>),
    Save(PathBuf),
    NewRecording,
    CloseRequested,
    // from the worker
    Monitors(Vec<MonitorInfo>),
    Windows(Vec<WindowInfo>),
    Started(String),
    Status(Status),
    Stopped { id: String, width: u32, height: u32, duration_ms: u64 },
    Picture { t_ms: u64, path: PathBuf },
    Exported(PathBuf),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    ListMonitors,
    ListWindows,
    Start(RecordRequest),
    AskStatus(String),
    Stop(String),
    Preview { id: String, t_ms: u64 },
    Export(ExportRequest),
    Quit,
}

impl App {
    /// The app as it opens, and the first action to run.
    pub fn new(out: PathBuf) -> (App, Vec<Action>)
}

/// Applies one event at time `now_ms` and returns what to do about it.
pub fn step(app: &mut App, event: Event, now_ms: u64) -> Vec<Action>
```

`App::new` returns `Screen::Idle { starting: false, closing: false }`, `inside` of `geometry::initial(&[])`, and `[ListMonitors]`. In the table, "`Idle`" as a new screen means that value.

`step` first computes `elapsed = now_ms.saturating_sub(app.last_ms)` and sets `last_ms = now_ms`, for every event. An event not listed for the current screen changes nothing else and returns no action. "Fit" means `geometry::keep_inside(rect, &app.monitors, geometry::toolbar_size(app.scale))`.

| Screen | Event | Change | Actions |
|---|---|---|---|
| any | `Scale(s)` | `scale = s` | |
| any | `Monitors(list)` | `monitors = list`; in `Idle`, `inside = geometry::initial(&monitors)` when `window` is `None` | |
| any | `Windows(list)` | `windows = list` without the minimized ones | |
| `Idle`, not starting | `MenuOpened` | | `ListWindows` |
| `Idle`, not starting | `Picked(None)` | `window = None` | |
| `Idle`, not starting | `Picked(Some(id))`, in `windows` | `window = Some(id)`, `inside` = that window's `bounds` | |
| `Idle`, not starting | `Moved { x, y }` | `inside` = fit of `inside` at `(x, y)`; `window = None` | |
| `Idle`, not starting | `Resized(rect)`, `window` is `None` | `inside` = fit of `rect` | |
| `Idle`, not starting | `Fps(n)` | `fps = n.clamp(1, 60)` | |
| `Idle`, not starting | `Cursor(b)`, `Clicks(b)` | the switch | |
| `Idle`, not starting | `RecordPressed` | `starting = true`, `message = None` | `Start(request)` |
| `Idle`, starting, not closing | `Started(id)` | `Recording { id, started_ms: now_ms, asked_ms: now_ms, stopping: false, closing: false }` | |
| `Idle`, starting, not closing | `Failed(text)` | `starting = false`, `message = Some(text)` | |
| `Idle`, not starting | `CloseRequested` | | `Quit` |
| `Idle`, starting | `CloseRequested` | `closing = true` | |
| `Idle`, starting, closing | `Started(id)` | `Recording { id, started_ms: now_ms, asked_ms: now_ms, stopping: true, closing: true }` | `Stop(id)` |
| `Idle`, starting, closing | `Failed(_)` | | `Quit` |
| `Recording`, not stopping | `Tick`, `now_ms - asked_ms >= 1000` | `asked_ms = now_ms` | `AskStatus(id)` |
| `Recording`, not stopping | `Status(s)`, `s != Status::Recording` | `stopping = true` | `Stop(id)` |
| `Recording`, not stopping | `StopPressed` | `stopping = true` | `Stop(id)` |
| `Recording`, not stopping | `CloseRequested` | `stopping = true`, `closing = true` | `Stop(id)` |
| `Recording`, stopping | `CloseRequested` | `closing = true` | |
| `Recording`, not closing | `Stopped { .. }` | `Editing(Edit { from_ms: 0, until_ms: duration_ms, crop: None, position_ms: 0, playing: false, picture: None, loading: false, exporting: false, .. })` | preview rule |
| `Recording`, closing | `Stopped { .. }` or `Failed(_)` | | `Quit` |
| `Recording`, stopping, not closing | `Failed(text)` | `Idle`, `message = Some(text)` | |
| `Recording`, not stopping | `Failed(text)` | `message = Some(text)` | |
| `Editing` | `Tick`, playing | `position_ms += elapsed`; at or past `until_ms` it becomes `from_ms` | preview rule |
| `Editing` | `PlayPressed` | `playing` flips; when it becomes true and `position_ms` is outside `from_ms .. until_ms`, `position_ms = from_ms` | preview rule |
| `Editing` | `Seek(t)` | `position_ms = t.min(duration_ms)`, `playing = false` | preview rule |
| `Editing` | `From(t)` | `from_ms = t.min(until_ms.saturating_sub(MIN_KEPT_MS))` | |
| `Editing` | `Until(t)` | `until_ms = t.clamp(from_ms + MIN_KEPT_MS, duration_ms)`, or `duration_ms` when the recording is shorter than `MIN_KEPT_MS` | |
| `Editing` | `Crop(c)` | `crop = c` | |
| `Editing` | `Picture { t_ms, path }` | `picture = Some((t_ms, path))`, `loading = false` | preview rule |
| `Editing`, not exporting | `Save(file)` | `exporting = true`, `message = None` | `Export(request)` |
| `Editing`, exporting | `Exported(path)` | `exporting = false`, `message` = the path as text | |
| `Editing` | `Failed(text)` | `exporting = false`, `loading = false`, `message = Some(text)` | |
| `Editing`, not exporting | `NewRecording` | `Idle`, `message = None` | |
| `Editing` | `CloseRequested` | | `Quit` |

- The preview rule: when `loading` is false and `picture` is not for `position_ms`, set `loading = true` and emit `Preview { id, t_ms: position_ms }`.
- The `Start` request is `RecordRequest { window_id: app.window, region, fps: Some(app.fps), format: Some("none".into()), no_cursor: !app.cursor, no_click_highlight: !app.clicks, out: Some(app.out.clone()), ..Default::default() }`, where `region` is `None` with a window chosen and otherwise `Some(format!("{},{},{},{}", inside.x, inside.y, inside.width, inside.height))`.
- The `Export` request is `ExportRequest { id, file, from: Some(format!("{from_ms}ms")), until: Some(format!("{until_ms}ms")), crop, out: Some(app.out.clone()) }`, where `crop` is the crop in the same `x,y,width,height` text or `None`.

### `src/gui/worker.rs`

```rust
/// Runs one action against recdr and returns the event that reports the outcome.
/// `Quit` returns None: the windows part handles it.
pub fn perform(out: &Path, action: Action) -> Option<Event>
```

| Action | Call | Event on success |
|---|---|---|
| `ListMonitors` | `control::list_monitors()` | `Monitors(list.monitors)` |
| `ListWindows` | `control::list_windows()` | `Windows(list.windows)` |
| `Start(request)` | `request.into_options().and_then(control::start)` | `Started(info.id)` |
| `AskStatus(id)` | `control::status(out, Some(&id))` | `Status` of the first entry; with no entry, `Failed(format!("no status for recording {id}"))` |
| `Stop(id)` | `control::stop(out, &id)` | `Stopped` with the result's `id`, `width`, `height` and `duration_ms` |
| `Preview { id, t_ms }` | `control::frames(out, &id, &FrameSelection::At(vec![t_ms]), None)` | `Picture { t_ms, path }` with the first frame's path; with no frame, `Failed(format!("recording {id} has no stored frame"))` |
| `Export(request)` | `request.into_options().and_then(control::export)` | `Exported(result.path)` |

Any error is `Event::Failed(error.to_string())`.

### `src/gui/app.rs`, `src/gui/overlay.rs`, `src/gui/editor.rs`

The windows part. Task 9 writes the exact `ViewportBuilder` and `ViewportCommand` calls under this heading; what each file does is fixed here.

```rust
// app.rs
pub fn run(out: PathBuf) -> Result<()>
// overlay.rs
pub fn show(ctx: &egui::Context, app: &state::App, now_ms: u64) -> Vec<Event>
// editor.rs
pub fn show(ctx: &egui::Context, app: &state::App, picture: Option<&egui::TextureHandle>) -> Vec<Event>
```

- `app::run` calls `eframe::run_native` with the window title `recdr`. An error from it is `Error::Io(std::io::Error::other(text))`.
- The `eframe::App` value holds the `state::App`, the two ends of a `std::sync::mpsc` channel of `Event`, the texture of the picture on show with its path, and `quitting: bool`.
- Each pass of `update`: `now_ms` is `ctx.input(|i| i.time)` times 1000. The events are those waiting on the channel, then `Scale(ctx.pixels_per_point())` when it differs from `app.scale`, then those `overlay::show` or `editor::show` returned, then `Tick`. Each goes through `state::step`. Each action but `Quit` starts a thread that calls `worker::perform`, sends the event and calls `ctx.request_repaint()`. `Quit` sets `quitting` and sends `ViewportCommand::Close`.
- A close request while `quitting` is false is answered with `ViewportCommand::CancelClose` and the event `CloseRequested`.
- While the screen is `Recording`, or `Editing` with `playing`, `update` asks for a repaint after 100 ms.
- `overlay::show` draws, for `Idle` and `Recording`: the four border windows at `geometry::borders`, each a viewport with no decorations, always on top, transparent, with the inner `BORDER` pixels filled with `(0, 120, 215)` when idle and `(220, 0, 0)` when recording; and the toolbar in the main window at `geometry::toolbar_rect`.
- Drags, in `Idle` only, by decision 13. At the press on a border window or on the toolbar's empty area, `overlay.rs` stores `app.inside` and the pointer's screen position in pixels in `egui`'s temporary memory. On each later pass of the drag, with `(dx, dy)` the pointer's screen position now less the stored one: a border window gives `Resized(geometry::resized(&stored_inside, edge, dx, dy))` for the edge `geometry::edge_at` named at the press, and the toolbar gives `Moved { x: stored_inside.x + dx, y: stored_inside.y + dy }`. Positions become pixels through `geometry::to_pixels` and nothing else. Task 9 writes down how the pointer's screen position is read.
- The toolbar holds, in `Idle`: the text `{width} x {height}`, the target menu (a combo box whose opening gives `MenuOpened`, with "Region" and one row per window showing its `title`; a choice gives `Picked`), frames per second as an `egui::DragValue` with the range 1 to 60 (`Fps`), the two checkboxes "Cursor" and "Clicks" (`Cursor`, `Clicks`), "Record" (`RecordPressed`) and "Close" (`CloseRequested`), and `message` when there is one. In `Recording`: the elapsed time as `m:ss` from `now_ms - started_ms`, and "Stop" (`StopPressed`).
- `editor::show` draws: the picture, scaled to fit, with the crop rectangle over it; under it the timeline with the two handles and the position; "Play" or "Pause" (`PlayPressed`); "Clear crop" (`Crop(None)`); the kept length as seconds with one decimal and the size after the crop; "Save GIF", "Save MP4" and "New recording" (`NewRecording`), the three of them disabled while `exporting` is true; and `message`, or "exporting" while `exporting` is true. A drag on the picture gives `Crop(geometry::crop_of(..))`. A press on the timeline within 6 points of a handle drags that handle and gives `From` or `Until`; any other press or drag on it gives `Seek(geometry::time_at(..))`. Space gives `PlayPressed`.
- "Save GIF" and "Save MP4" open `rfd::FileDialog::new().add_filter(..).set_file_name("export.gif")` (or `"export.mp4"`, decision 15) with `save_file()`. A chosen path gets the extension when it lacks it, and gives `Save(path)`.
- `app.rs` loads a new `picture` path with `image::open`, turns it into an `egui::ColorImage` with `from_rgba_unmultiplied`, and keeps the texture until the path changes.
- No file of the windows part decides what to do to recdr: each only turns input into events and draws `state::App`.

### `tests/layers.rs`

Starting from the layer tests as they are after `native-mp4`:

| Rule | Change | Task |
|---|---|---|
| 2 | add `forbid(&["eframe", "egui", "rfd::"], &["src/gui/app.rs", "src/gui/overlay.rs", "src/gui/editor.rs"])` | 10 |
| 4 | `src/cli.rs` and `src/mcp.rs` may also not contain `gui::` or `export::` | 4 |
| 4 | no file under `src/gui/` contains `recorder::`, `capture::`, `store::`, `analyze::`, `encode::`, `draw::`, `platform::`, `export::`, `cli::`, `mcp::` or `crate::{` | 6 |
| 6 | `src/gui/mod.rs` joins the files that may hold `cfg` | 10 |

The `gui` list leaves out `time::`, which the list for `cli.rs` has, because `std::time::Duration` contains it and the windows part needs that type. Rule 3 already keeps `unsafe` out of `src/gui/`, because it names the only files that may hold it. Rule 1 is not changed, and it binds this module: the texts `"recording.gif"` and `"recording.mp4"` may not appear in any file under `src/` but `src/store.rs`, the test modules in those files included, so code and unit tests here use `dir.gif_path()`, `dir.mp4_path()` or another name. The test reads only `src/`, so files under `tests/` are not held to it.

`geometry::to_pixels` and `geometry::to_points` have the names of two public functions that `macos-capture` puts in `src/capture/mod.rs`, with other arguments: those convert a rectangle using a list of displays. The app cannot use them, because rule 4 keeps `capture::` out of `src/gui/`, and it needs only a multiplication.

### `Cargo.toml`

```toml
[target.'cfg(any(windows, target_os = "macos"))'.dependencies]
eframe = "0.37"
rfd = "0.18"
```

Task 9 writes down the cargo features of `eframe` that are needed, and turns the default ones off where the app does not use them.

## File boundary

Create:
- `src/export.rs`
- `src/gui/mod.rs`
- `src/gui/geometry.rs`
- `src/gui/state.rs`
- `src/gui/worker.rs`
- `src/gui/app.rs`
- `src/gui/overlay.rs`
- `src/gui/editor.rs`
- `tests/export.rs`
- `tests/gui.rs`
- `tasks/6-gui-overlay/acceptance.md`

Modify:
- `src/lib.rs`
- `src/main.rs`
- `src/cli.rs`
- `src/mcp.rs`
- `src/control.rs`
- `src/request.rs`
- `src/model.rs`
- `src/recorder.rs`
- `src/encode/mod.rs`
- `src/encode/gif.rs`
- `tests/record.rs`
- `tests/mcp.rs`
- `tests/layers.rs`
- `Cargo.toml`
- `Cargo.lock`
- `README.md`

Delete:
- none

Files under `spec/` and `tasks/` may always be edited. `src/capture/`, `src/platform/`, `src/store.rs`, `src/analyze/`, `src/draw.rs`, `src/error.rs` and the three MP4 writers are not touched.

## Task list

Details, acceptance and verification for each task are in [todo.md](todo.md).

### Phase 1: export, on every system
- Task 1: `--format none`
- Task 2: the encoders take a crop and a path
- Task 3: the manifest of the kept part
- Task 4: `recdr export`
- Task 5: the MCP tool `export`

### Checkpoint A: export is done
- The three commands pass on the development machine
- By hand: a trimmed and cropped GIF of a real recording plays

### Phase 2: the logic part, on every system
- Task 6: frame and toolbar arithmetic
- Task 7: the state machine
- Task 8: the worker, and `recdr gui` as a command that is refused everywhere

### Checkpoint B: the logic is done (needs a pull request)
- The owner pushes the branch and opens a draft pull request; the build stops and asks
- The three CI jobs are green

### Phase 3: the windows part, on Windows
- Task 9: try the windows, and write the results into this plan
- Task 10: the app opens, with a toolbar that can record and stop
- Task 11: the frame
- Task 12: the look of a recording
- Task 13: the editor

### Checkpoint C: the app works on Windows
- By hand, by the owner: criteria 11 to 23 on Windows, each with its result in `acceptance.md`

### Phase 4: macOS, and the rest
- Task 14: the app on the Mac
- Task 15: README and the acceptance run

### Checkpoint D: complete
- The three CI jobs are green
- `tasks/6-gui-overlay/acceptance.md` has a result for each of the 23 criteria, on both systems for 11 to 23

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| This plan was written before three modules it stands on were built. | Med | "Before Task 1" checks every symbol taken from their plans. Phase 1 touches only `encode` among the files they change. |
| Four border windows may not behave as one frame (they lag each other in a drag, or one drops behind another window). | High | Task 9 tries exactly this before any app code. A failure stops the build and goes to the owner, because the other way needs a system call and a spec change. |
| A transparent part of a window may let clicks through on one system and not on the other. | Med | Task 9 and Task 14 each try it. The grip needs the window to take the click; if it does not where it is transparent, the grip is filled with a color of alpha 1. |
| The windows part has no automated test. | Med | It holds no decision: every rule is in `state` and `geometry`, which have tests. The by-hand checklist covers what is drawn. |
| No Mac is available. | High | Phases 1 to 3 do not need one. Task 14 cannot be done, and the app ships for Windows only until it can; the README then says so. |
| Positions are off on a monitor with another scale factor. | Med | Task 9 measures what unit `egui` wants for a window position on a second monitor. All conversion is in `geometry`. |
| `eframe` makes the build slow and the executable large. | Low | Measured in Task 9 and reported to the owner, who can ask for the cargo feature the spec lists under Alternatives. |

## Open questions

1. Is a Mac running macOS 14 or later available (spec open question 1)? Task 14 needs it.
2. Checkpoint B needs a pull request for CI to run.
