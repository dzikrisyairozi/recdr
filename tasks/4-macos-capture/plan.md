# Implementation plan: macos-capture

Spec: [spec/4-macos-capture/spec.md](../../spec/4-macos-capture/spec.md). Task list: [todo.md](todo.md). Tracking issue: https://github.com/dzikrisyairozi/recdr/issues/4.

All paths are relative to the repository root. "The spec" means the file above. "The cdp plan" means [tasks/3-cdp-capture/plan.md](../3-cdp-capture/plan.md).

## Overview

`recdr windows`, `recdr monitors` and the five desktop target flags start working on macOS. Two new files hold everything that talks to macOS: `src/capture/macos.rs` (the permission check, the two lists, the ScreenCaptureKit source) and `src/capture/tap.rs` (the mouse event tap). Two pieces of plain arithmetic go into `src/capture/mod.rs`, where they are compiled and tested on every system: the conversion between points and pixels, and the click filter that Windows already has.

The work is in two halves:

- **Phase 1 runs on any machine.** It moves the click filter, adds the arithmetic, and moves the browser test helpers to where the desktop tests can use them. Windows behaves as before.
- **Phases 2 to 4 need a Mac.** Nothing in them can be compiled on Windows, because the `screencapturekit` crate builds a Swift package with Xcode's tools. Phase 2 starts with a throwaway program that answers the spec's eight unchecked questions, and its answers are written into this plan before the tasks after it start.

### When this plan was written

On 2026-11-18, before `cdp-capture` was built and with no Mac at hand. What it describes as existing is of two kinds:

- **Exists today** (checked by reading the files on branch `feat/1-agent-recorder`): `resolve`, `window_target`, `single` and `contains` in `src/capture/mod.rs`; `store::letterbox`; `tests/desktop.rs` with its Windows tests; `tests/common/mod.rs` with `test_window` and `wait_for_window`; `tests/layers.rs`.
- **Exists after `cdp-capture`** (taken from the cdp plan, not from code):
  - `src/capture/hook.rs` with `RawClick`; `to_event` in `src/capture/windows.rs`, and `WgcSource` with `size: Option<(u32, u32)>`.
  - `desktop_source`, `list_windows` and `list_monitors` with two `cfg` definitions each in `src/capture/mod.rs`.
  - `CaptureEvent::Click`, and `Error::UnsupportedPlatform` with the message `window, region and monitor capture need Windows; on this system recdr records browser tabs with --cdp`.
  - `tests/browser.rs` with `Browser`, `Client`, `serve` and the three pages; `Server::start(fake: Option<&str>)` in `tests/common/mod.rs`.
  - `tests/desktop.rs` with `#![cfg(windows)]` as its first line after the doc comment.
  - `tests/tabs.rs` with `#[cfg(not(windows))] mod unsupported`, whose test asserts that `recdr windows`, `recdr monitors` and `recdr record --monitor 0 --duration 1s` exit 1 with `unsupported_platform`.
  - Six layer tests. The sixth forbids `cfg(`, `cfg!(` and `cfg_attr(`, other than `cfg(test)`, outside `src/capture/mod.rs`, `src/platform/mod.rs` and `src/lib.rs`.
  - `.github/workflows/ci.yml`, which runs on pull requests and on pushes to `main`.

"Before Task 1" in [todo.md](todo.md) checks the second list against the real code. A difference is fixed in this plan before any task starts.

### What was measured

Task 4, on 2026-11-19 and 2026-11-20, on the owner's MacBook (Apple M5, one built-in display of 1512 by 982 points at scale 2, macOS 26.6.2, Xcode command line tools only), with the try-out program on `spike/4-sck`. Its output is in [acceptance.md](acceptance.md).

The spec's "Not checked" list:

1. **A stream on a hosted runner:** delivers frames. On `macos-latest` (draft pull request #12, closed unmerged) a display stream gave 12 complete frames of 1024 by 768 in 1.4 s. Task 11 adds the CI step.
2. **The tap and Input Monitoring:** with the permission on, `CGEventTapCreate` returns a tap and `start_then_drop_returns` holds (the drop returns in under 1 ms). With it off: could not be tried without a person; it is on the owner's list. `start` refuses on `CGPreflightListenEventAccess` alone, so the answer does not change the code.
3. **A window minimized, on another Space, closed:** minimized gives one frame with status `Suspended` and then nothing, no stop notice, and the window stays listed with `is_on_screen` false. Restored, complete frames resume. Closed gives one frame with status `Stopped`, then `did_stop_with_error` with `SCStream error (No capture source provided): Failed to find any displays or windows to capture`. Another Space: could not be tried without a person.
4. **The alert of fact 6:** could not be tried without a person watching the screen; on the owner's list.
5. **Frame size of a window with a toolbar:** a Chrome `--app` window of 800 by 600 points has a filter `content_rect` of 800 by 600, equal to its frame and to CDP's `Browser.getWindowForTarget` bounds, title bar included, shadow excluded; frames are 1600 by 1200. The difference is 0 pixels.
6. **The responsible program of a detached recorder:** a child started with `setsid()` in `pre_exec` and its output in a file has both permissions when Terminal has them: macOS counts Terminal. No dialog appeared, because the permissions were already given; which program a dialog names could not be seen.
7. **macOS 13:** could not be tried, no such Mac. The README names macOS 14.
8. **A resized window:** padded, not stretched. A window made 500 by 400 during a 1600 by 1200 stream is scaled up by 1.5 into the left 1500 by 1200 pixels, and the rest is black. Decision 6 stands.

Also found:

- **Build time:** the first `cargo build` of the try-out program took 99 s on the M5, the Swift packages included.
- **The build needs a Swift rpath, and without Xcode a search path.** A program that depends on `screencapturekit` does not get the `-Wl,-rpath,/usr/lib/swift` that the crate's `build.rs` prints (a build script's `rustc-link-arg` applies to its own package only), and fails at start in `dyld` with `Library not loaded: @rpath/libswift_Concurrency.dylib`. This happened on the runner with Xcode too. With only the command line tools, linking also fails, because the crate adds `<xcode-select -p>/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx`, which only Xcode has; the libraries are in `<xcode-select -p>/usr/lib/swift/macosx`. recdr therefore gets a `build.rs` (decision 15).
- **A window filter needs a window server connection.** `SCContentFilter::create().with_window(..).build()` aborts the process in `CGS_REQUIRE_INIT` unless CoreGraphics has connected first. A display filter built earlier does that, and so does `CGMainDisplayID()`; `open` calls `CGMainDisplayID()` first.
- **Rows are padded:** 12160 bytes per row for 3024 pixels; the row-by-row copy is needed.
- **`system_profiler SPDisplaysDataType` lists no display** on this Mac, only the GPU. The criterion 1 test reads display sizes from CoreGraphics instead (see `tests/desktop.rs`).
- **A still display stream** delivers mostly `Idle` frames (148 of 150 in 10 s), which are not stored.

## Architecture decisions

Each is a choice the spec left to the plan.

1. **The shared arithmetic is `pub`.** `Display`, `display_bounds`, `to_pixels`, `to_points`, `Press` and `to_event` in `src/capture/mod.rs` are public items of a public module. On Linux nothing calls them, and a private function nobody calls fails the build under `clippy -D warnings`. Public items are not reported, and no `cfg` attribute is needed to silence anything.
2. **A press travels as floating-point numbers in the system's own units.** `Press` is `(u64, f64, f64, &'static str)`. macOS reports a press in points, which can be fractional, and Windows in whole pixels. `to_event` takes the captured area's corner in the same units and a scale factor, 1 on Windows.
3. **The primary monitor is the display whose position in points is (0, 0).** macOS puts the main display at the origin of its coordinate space. This is a rule on plain numbers, so the unit tests cover it.
4. **The macOS source knows its size when it opens.** The stream is configured with a width and a height in pixels, taken from the content filter. The Windows source learns its size from the first frame; the macOS source does not have to wait.
5. **Frames reach the recorder through a channel that holds two.** ScreenCaptureKit calls the output handler on its own queue. The handler converts a complete frame to RGBA and tries to put it in a `sync_channel(2)`; when the channel is full the frame is discarded and counted, and `dropped()` returns the count.
6. **A resized window: the stream keeps its starting size and ScreenCaptureKit scales.** That is one setting and no code. Task 4 step 3 looks at the result. If the picture is padded with black, this decision stands. If it is stretched, the build stops after Task 4: the other way (frames delivered at their real size and fitted with `store::letterbox`, as for Windows) changes `SckSource` in ways this plan does not specify. The plan then gains a task for it, with its contract and its test, and the owner approves the changed plan before Task 5.
7. **A refused stream is told apart from a broken one by asking macOS again.** When starting the stream fails, the source calls the permission check; if that now says no, the error is `permission_denied`, otherwise `capture_failed` with the system's text.
8. **The event tap runs its own thread and run loop.** A tap delivers events to a run loop, and the recorder's thread does not run one. The thread sends each press over a channel, and `Drop` stops the run loop and joins the thread.
9. **The macOS desktop tests are a second module in `tests/desktop.rs`.** The Windows tests stay as they are inside `#[cfg(windows)] mod on_windows`, and the new ones go in `#[cfg(target_os = "macos")] mod on_macos`. The two test windows are too different (a WinForms window with a chosen title, a Chrome window with a chosen page) for one set of tests to serve both.
10. **`Browser::app` starts the macOS test window.** The cdp plan's `Browser::start` starts headless Chrome. The desktop tests need a window on screen, so `Browser` gets a second constructor that shares the start-up wait.
11. **A test window has a title no other window has.** The three test pages set their title from the address's fragment when there is one, so a test opens `/anim#recdr-test-<pid>-<name>` and finds its window by that exact title, as the Windows tests find theirs. A fixed title such as `anim` would also match an editor tab on the owner's Mac.
12. **A try-out program lives on a branch that is never merged.** Task 4's program is a cargo project in `spike/sck/` on the branch `spike/4-sck`, cut from the work branch. It is run on the owner's Mac. Its one CI run is a draft pull request from that branch, with one step added to `ci.yml` on that branch; the pull request is closed unmerged. Nothing on that branch is under this plan's File boundary, because none of it reaches the work branch.
13. **CI runs on pull requests, and it is where the other two systems are checked.** Phase 1 is built on Windows and Phases 2 to 4 on the Mac, so "the three commands" of a task run on one system. At Checkpoint A the owner pushes the work branch and opens a draft pull request, and the build asks once whether it may push to that pull request after each later task. From Task 5 each task's commit is pushed, by the build or by the owner, and the three CI jobs are green before the next task starts. That is how "Windows is unchanged" and "Linux still answers `unsupported_platform`" are checked in Tasks 5 to 11.
15. **A `build.rs` gives every macOS binary the Swift rpath.** Found in Task 4: on macOS it prints `cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift`, which reaches the binary, the test binaries and the examples, and, when `<xcode-select -p>/usr/lib/swift/macosx` exists (the command line tools without Xcode), `cargo:rustc-link-search=native=` of it. It reads `CARGO_CFG_TARGET_OS`, so it has no `cfg` attribute. On Windows and Linux it prints nothing.
14. **Every recording has its own process.** `record` records in the command's process, and `start` and the MCP tools run `_recorder`. So a source, and its event tap, is opened once per process, and what must hold is that dropping it returns.

### Spec changes made while planning

None. Task 4 changed this plan: "What was measured", decision 15 and `build.rs`, the calls under `src/capture/macos.rs` and `src/capture/tap.rs`, the cargo features, and the criterion 1 oracle (CoreGraphics, as `system_profiler` lists no display).

## Module contracts

A task implements these exactly as written. Anything not listed keeps the signature it has after `cdp-capture`.

### `src/error.rs`

One more variant, the eighteenth after `cdp-capture`:

```rust
#[error("macOS has not given this program the Screen Recording permission. Open System Settings > Privacy & Security > Screen & System Audio Recording, switch on the program that started recdr (your terminal or editor), restart that program, and run recdr again.")]
PermissionDenied,
```

`code()` returns `"permission_denied"`. The test that lists every variant counts one more.

The message of `UnsupportedPlatform` becomes `window, region and monitor capture need Windows or macOS; on this system recdr records browser tabs with --cdp`. A test that quotes the old text is changed with it; "Before Task 1" finds them.

### `src/capture/mod.rs`

```rust
/// A mouse press as the system reports it: wall-clock milliseconds, x and y in the
/// system's screen units (pixels on Windows, points on macOS), button.
pub type Press = (u64, f64, f64, &'static str);

/// A press to a click event relative to the captured area in pixels, or None when it
/// is outside. `origin` is the area's top-left corner in the press's units, `size` is
/// the area in pixels, and `scale` is pixels per unit.
pub fn to_event(press: Press, origin: (f64, f64), size: (u32, u32), scale: f64) -> Option<CaptureEvent>

/// A display as macOS reports it: position and size in points, and pixels per point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display { pub x: f64, pub y: f64, pub width: f64, pub height: f64, pub scale: f64 }

/// The `bounds` of each display in pixels, in the same order.
pub fn display_bounds(displays: &[Display]) -> Vec<Rect>

/// A rectangle in points to pixels.
pub fn to_pixels(displays: &[Display], x: f64, y: f64, width: f64, height: f64) -> Rect

/// A rectangle in pixels, relative to a display's corner, to points relative to the same corner.
pub fn to_points(crop: &Rect, scale: f64) -> (f64, f64, f64, f64)
```

- `to_event`: `x = ((press.1 - origin.0) * scale).floor()` and the same for `y`. `None` unless `0 <= x < size.0` and `0 <= y < size.1`. Otherwise `CaptureEvent::Click { unix_ms: press.0, x: x as i32, y: y as i32, button: press.3 }`.
- `display_bounds`: `s` is the largest `scale` in the slice. For each display the origin is `((d.x * s).round(), (d.y * s).round())` and the size `((d.width * d.scale).round(), (d.height * d.scale).round())`.
- `to_pixels`: the display is the first whose rectangle in points contains the point `(cx, cy) = (x + width / 2, y + height / 2)`, with `d.x <= cx < d.x + d.width` and `d.y <= cy < d.y + d.height`, or the first display when none does. With `b` its entry in `display_bounds`: the result's origin is `b.x + ((x - d.x) * d.scale).round()` (and the same for `y`) and its size `(width * d.scale).round()` by `(height * d.scale).round()`. For an empty slice the four numbers are rounded as they are.
- `to_points`: each of `crop.x`, `crop.y`, `crop.width`, `crop.height` divided by `scale`.

The `RawClick` type of the cdp plan stays in `src/capture/hook.rs`; `src/capture/windows.rs` turns one into a `Press` when it calls `to_event`.

Module lines and the three functions that choose a back end, after Task 5:

```rust
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod tap;        // added in Task 9
```

`list_windows`, `list_monitors` and `desktop_source` each have three definitions: under `#[cfg(windows)]` as after `cdp-capture`; under `#[cfg(target_os = "macos")]` calling `macos::list_windows()`, `macos::list_monitors()` and `macos::SckSource::open(resolved, fps, cursor, clicks)`; and under `#[cfg(not(any(windows, target_os = "macos")))]` returning `Err(Error::UnsupportedPlatform)`.

In Tasks 5 and 6 `SckSource` does not exist yet. The macOS definition of `desktop_source` then has its four parameters named with a leading underscore and returns `Err(Error::CaptureFailed("recording is not built yet on macOS".into()))`. Task 7 replaces that body with the call.

These are the only `cfg` lines the module adds. `src/capture/macos.rs` and `src/capture/tap.rs` hold none: the sixth layer test would fail on one.

`resolve`, `single`, `window_target` and `contains` do not change.

### `src/capture/windows.rs`

The private `to_event` and its unit test are removed. Where `next_event` called it, with `size` the pair it has taken out of `self.size` (the hook is drained only once the size is known), it calls:

```rust
super::to_event(
    (click.0, f64::from(click.1), f64::from(click.2), click.3),
    (f64::from(self.origin.0), f64::from(self.origin.1)),
    size,
    1.0,
)
```

### `src/capture/macos.rs`

```rust
pub fn list_windows() -> Result<Vec<WindowInfo>>
pub fn list_monitors() -> Result<Vec<MonitorInfo>>

pub struct SckSource { /* private */ }
impl SckSource {
    pub fn open(resolved: &ResolvedTarget, fps: u32, cursor: bool, clicks: bool) -> Result<Self>
}
impl CaptureSource for SckSource
impl Drop for SckSource                 // stops the stream; the tap field is dropped with it

/// Ok when macOS lets this program read the screen. Otherwise asks macOS to show its
/// dialog and returns `permission_denied`.
fn permitted() -> Result<()>

/// The window filter: an ordinary window (layer 0) with a title, owned by an application.
fn listed(layer: i32, title: &str, has_application: bool) -> bool

/// The displays ScreenCaptureKit reports, in its order.
fn displays(content: &SCShareableContent) -> Vec<Display>
```

- `permitted`: `CGPreflightScreenCaptureAccess()`; when false, `CGRequestScreenCaptureAccess()` and `Err(Error::PermissionDenied)`.
- `list_monitors`: `permitted()?`, the shareable content, `displays`, `display_bounds`. Entry `i` is `MonitorInfo { index: i, bounds, primary: d.x == 0.0 && d.y == 0.0 }`.
- `list_windows`: `permitted()?`, the shareable content, every window for which `listed` is true. `id` is the window id as `u64`; `title`; `process` is the owning application's name; `pid` its process id; `bounds` is `to_pixels(&displays, frame.x, frame.y, frame.width, frame.height)`; `minimized` is true when `is_on_screen` is false.
- `listed`: `layer == 0 && !title.is_empty() && has_application`.
- A display's `scale` is `point_pixel_scale` of a content filter made for that display.
- `open`, for `ResolvedKind::Window(id)`: `permitted()?`; the window with that id, or `Error::TargetNotFound(format!("window id {id}"))`; a filter for that one window. The size is the filter's `content_rect` size times its `point_pixel_scale`, rounded. The source keeps the window's corner in points and that scale for the click filter.
- `open`, for `ResolvedKind::Monitor { index, crop }`: `permitted()?`; the display at `index`, or `Error::TargetNotFound(format!("monitor {index}"))`; a filter for that display. With a crop, the stream's source rectangle is `to_points(crop, scale)` and the size is the crop's; without one the size is the display's `bounds` size. The corner in points is the display's corner, plus the crop's corner in points.
- `open`, any other kind: `Error::CaptureFailed("this target has no ScreenCaptureKit source".into())`. Between Tasks 7 and 8 `ResolvedKind::Window` is answered with `Error::CaptureFailed("window targets are not built yet on macOS".into())`.
- The stream is configured with the size above, a minimum frame interval of `1 / fps` seconds, `with_shows_cursor(cursor)`, a BGRA pixel format and scaling to fit (decision 6). When `clicks` is true the source holds `tap::ClickTap::start()`.
- The channel carries `enum Item { Frame(Frame), Stopped(String) }` and is a `sync_channel::<Item>(2)`.
- The output handler ignores a frame whose status is not `Complete`. A complete one is copied row by row, honoring the buffer's bytes per row, with blue and red exchanged, into a `Frame`, and offered with `try_send`; when the channel is full the frame is discarded and a shared counter goes up by one.
- The handler for a stopped stream sends `Item::Stopped(text)` with the blocking `send`, so the notice is never the one discarded. `WgcSource` does the same for its closed notice.
- Only the stop notice ends a window recording. While the window is minimized or on another Space the stream may deliver no complete frame; then no frame is stored, and the recording runs on to its duration or to `stop`. If Task 4 step 4 shows that macOS sends the stop notice for a minimized window too, that is `window_closed` as well, and Task 11 says so in the README.
- `next_event(timeout)`: first, while the tap has a press, return the first that `super::to_event(press, corner, size, scale)` accepts. Then wait on the channel for `timeout`: `Item::Frame` is `CaptureEvent::Frame`; `Item::Stopped(message)` is `CaptureEvent::Closed` for a window target and `Err(Error::CaptureFailed(format!("macOS stopped the stream: {message}")))` otherwise; a timeout is `Ok(None)`; a disconnected channel is `Err(Error::CaptureFailed("the capture stream ended".into()))`.
- A failure to start the stream is handled as decision 7 says.
- Every `unsafe` block has a comment that says what makes it sound.

The calls that worked in Task 4, from `screencapturekit` 11.1.2 (`use screencapturekit::prelude::*;` and `use screencapturekit::stream::delegate_trait::StreamCallbacks;`), all blocking:

- `SCShareableContent::get() -> Result<SCShareableContent, SCError>`; `.displays() -> Vec<SCDisplay>`, `.windows() -> Vec<SCWindow>`.
- `SCWindow`: `window_id() -> u32`, `title() -> Option<String>`, `frame() -> CGRect` (points, origin top left), `window_layer() -> i32`, `is_on_screen() -> bool`, `owning_application() -> Option<SCRunningApplication>` with `application_name() -> String` and `process_id() -> i32`.
- `SCDisplay::frame() -> CGRect` in points.
- `SCContentFilter::create().with_display(&d).with_excluding_windows(&[]).build()` and `SCContentFilter::create().with_window(&w).build()`, each `SCResult<SCContentFilter>`; `.content_rect() -> CGRect` in points, `.point_pixel_scale() -> f32`. Before the first window filter of a process, `objc2_core_graphics::CGMainDisplayID()` is called (it aborts otherwise, see "What was measured").
- `SCStreamConfiguration::new().with_width(u32).with_height(u32).with_minimum_frame_interval(&CMTime::new(1, fps as i32)).with_shows_cursor(bool).with_pixel_format(PixelFormat::BGRA).with_scales_to_fit(true)`, and `.with_source_rect(CGRect::new(x, y, w, h))` in points relative to the display for a crop.
- `SCStream::new_with_delegate(&filter, &config, StreamCallbacks::new().on_error(move |e: SCError| ..)) -> SCResult<SCStream>`; `stream.add_output_handler(move |sample: CMSampleBuffer, kind: SCStreamOutputType| .., SCStreamOutputType::Screen)`; `start_capture()`, `stop_capture()`, each `Result<(), SCError>`. `on_error` is `did_stop_with_error`, the stop notice; `stop_capture` does not call it.
- In the handler: `sample.frame_status() -> Option<SCFrameStatus>` (`SCFrameStatus::Complete` is a complete frame); `sample.pixel_buffer()`, then `.lock(CVPixelBufferLockFlags::READ_ONLY)` (from `screencapturekit::cv`), whose guard has `width()`, `height()`, `bytes_per_row()` and `unsafe fn as_slice() -> Option<&[u8]>`.
- Decision 6: a frame's real content is its attachment `sample.content_rect()` (points of the output) and `sample.content_scale()`: the window's size in points is `content_rect` size divided by `content_scale`. A running stream's size changes with `stream.update_configuration(&config)`. Neither is needed while decision 6 stands.

What Task 4 fills in, and the rule for each, so that nothing is chosen freely there:

| Open until Task 4 | Rule |
|---|---|
| Open until Task 4 | Rule | Filled in |
|---|---|---|
| The exact `screencapturekit` calls | The calls the spec's fact 1 names. Where the crate has a blocking and an async form of one call, the blocking one. | Listed above; all blocking. |
| The cargo features of the two `objc2` crates | `default-features = false`, plus each feature the compiler names as missing, and no other. | Under `Cargo.toml`. |
| The type of `listed`'s first parameter | The type the crate's window layer has; `i64` above changes to it. | `i32`. |
| The field names in `system_profiler`'s output | Measured (step 8). Expected: `_spdisplays_pixels`, a text such as `3024 x 1964`. | None: it lists no display here. CoreGraphics instead (`tests/desktop.rs`). |
| The allowed difference in the criterion 3 test | The largest difference seen in step 2, plus 2 pixels. A difference above 8 pixels stops the build, because the spec says the frames have the size of the window's content. | 0 seen, so 2. |
| The title ScreenCaptureKit reports for the test window | Must equal the page's title (step 2). If it does not, the build stops: decision 11 and every macOS test rest on it. | `recdr-spike-title`, equal. |

### `src/capture/tap.rs`

```rust
/// What the recording prints when macOS does not allow the tap.
const WARNING: &str = "clicks are not recorded: macOS has not given this program the Input Monitoring permission (System Settings > Privacy & Security > Input Monitoring)";

pub struct ClickTap { /* private */ }
impl ClickTap {
    /// None, after printing `WARNING` to stderr, when macOS does not allow the tap.
    pub fn start() -> Option<Self>
    /// The next press the tap has seen, without waiting.
    pub fn try_next(&self) -> Option<super::Press>
}
impl Drop for ClickTap                  // stops the run loop and joins the thread

/// The tap's event mask: left, right and other mouse-button presses.
fn mask() -> u64
```

- `mask` is `(1 << 1) | (1 << 3) | (1 << 25)`, built from the three `CGEventType` values for a left, right and other mouse-button press.
- The struct holds the receiving end of an `mpsc::channel::<Press>()`, the thread's run loop, and the thread's join handle.
- `start`: `let allowed = CGPreflightListenEventAccess() || CGRequestListenEventAccess();`. When `allowed` is false, print `WARNING` and return `None` without starting a thread: on some versions macOS creates a tap for a program without the permission and sends it nothing, and a recording must not report clicks as recorded then. Otherwise start the thread and wait on a `sync_channel(1)` for its report, which is `Option` of the thread's run loop: `None` when the tap is null. On `None`, `start` joins the thread, prints `WARNING` and returns `None`.
- The thread creates the tap with `CGEventTapCreate` for the session, at the head, with the listen-only option and `mask()`. Its `user_info` is `Box::into_raw` of a box holding the `Sender<Press>` and a cell for the tap's port; the thread turns it back into a box, and so frees it, after the run loop returns. With a tap, the thread stores the port in the cell, adds the tap's source to its run loop, reports the run loop, and runs it.
- The callback, by event type: a left, right or other mouse-button press sends `(time::now_unix_ms(), location.x, location.y, button)` with the button `"left"`, `"right"` or `"middle"`. The notice that the tap was disabled by a timeout (type `0xFFFF_FFFE`) switches the tap on again with `CGEventTapEnable`. Every other type, the notice of a tap disabled by user input among them, is ignored. The callback always returns the event unchanged.
- `Drop` calls `CFRunLoopStop` on the thread's run loop and joins the thread. `CFRunLoopStop` may be called from another thread. The binding's run loop type, `CFRetained<CFRunLoop>`, is not `Send` (Task 4), so the struct keeps it in a private wrapper with `unsafe impl Send` and a comment that says so.
- The calls that worked in Task 4: `CGEvent::tap_create(CGEventTapLocation::SessionEventTap, CGEventTapPlacement::HeadInsertEventTap, CGEventTapOptions::ListenOnly, mask(), Some(callback), user_info) -> Option<CFRetained<CFMachPort>>` (unsafe); the callback is `unsafe extern "C-unwind" fn(CGEventTapProxy, CGEventType, NonNull<CGEvent>, *mut c_void) -> *mut CGEvent` and returns `event.as_ptr()`; `CGEvent::location(Some(event)) -> CGPoint`; `CGEvent::tap_enable(&port, true)`; `CFMachPort::new_run_loop_source(None, Some(&port), 0)`; `CFRunLoop::current()`, `rl.add_source(Some(&source), unsafe { kCFRunLoopCommonModes })`, `CFRunLoop::run()`, `rl.stop()`. The event types are `CGEventType::LeftMouseDown`, `RightMouseDown`, `OtherMouseDown` and `TapDisabledByTimeout`; `CGEventType` is a newtype whose `.0` is the number for `mask`.
- Unit tests: `mask() == 0x0200_000A`; and `start_then_drop_returns`, which starts a tap, drops it on a second thread and fails when the drop has not returned within 2 seconds (with no permission `start` is `None` and the test has nothing to drop).

### `tests/common/mod.rs`

Moved here from `tests/browser.rs`, unchanged except that each becomes `pub`: `Browser` with `start` and `client`, `Client` with its six functions, `serve`, and the constants `ANIMATION`, `STILL` and `GREEN`. Task 3 found that these need more than the list above: the private helpers `answer`, `chrome_path` and `unix_ms` moved with them, `Browser::port_file` and `Browser::ws_url` became `pub`, and so did the fields `Browser.child`, `Browser.dir`, `Browser.port` and `Client.ws`, which browser tests read. Added in Task 6:

```rust
impl Browser {
    /// Chrome showing one page as an application window of 800 by 600 points.
    pub fn app(name: &str, url: &str) -> Browser
}
impl Client {
    /// Minimizes the browser window that holds this tab.
    pub fn minimize(&mut self, tab: &str)
}
```

- `app` uses the arguments `--app=<url> --user-data-dir=<dir> --window-size=800,600 --remote-debugging-port=0 --no-first-run --no-default-browser-check`, and waits for `DevToolsActivePort` as `start` does. The code the two share becomes a private `fn launch(name: &str, args: Vec<String>) -> Browser`, which also adds `--no-sandbox` when the environment variable `CI` is set, as `start` does today.
- A test gets the `tab` argument of `minimize` and `window_size` as the first entry of `Client::tab_ids()`, and the `session` argument of `Client::eval` from `Client::attach(tab)`. Both exist after `cdp-capture`.
- `minimize` calls `Browser.getWindowForTarget {targetId: tab}` and then `Browser.setWindowBounds {windowId, bounds: {windowState: "minimized"}}`.
- Each of the three pages gains one script line, `if (location.hash) document.title = decodeURIComponent(location.hash.slice(1));`. Opened without a fragment a page keeps its title, so the browser tests are not affected.
- `Client` gains `pub fn window_size(&mut self, tab: &str) -> (f64, f64)`, the `width` and `height` of `bounds` in the reply to `Browser.getWindowForTarget {targetId: tab}`, in points.

### `tests/desktop.rs`

The first line after the doc comment changes from `#![cfg(windows)]` to `#![cfg(any(windows, target_os = "macos"))]`. The helpers `record`, `ok`, `error` and `title`, and the import of `json`, `out_dir` and `recdr` from `common`, stay at the top. The existing tests move, unchanged, into a new `#[cfg(windows)] mod on_windows`, which starts with `use super::*;` and its own `use common::{test_window, wait_for_window};`. The macOS tests are in `#[cfg(target_os = "macos")] mod on_macos`, which starts with `use super::*;`. In Task 5 that is its only import, because its one test runs `recdr monitors` and `system_profiler` and nothing from `common` beyond what the top imports. Task 6 adds `use common::{serve, wait_for_window, Browser};`. No import that only one system uses stays at the top.

The file gets `#![allow(dead_code)]` on the line after the `cfg` attribute, as `tests/common/mod.rs` has: from Task 5 to Task 8 one or more of the four helpers at the top is called on Windows only, and `clippy -D warnings` on macOS would report it. The attribute stays.

How a macOS test gets its window: `let port = serve();`, then `Browser::app(name, &format!("http://127.0.0.1:{port}/anim#{}", title(name)))`, then `wait_for_window(&title(name))`, which returns the entry of `recdr windows` with exactly that title. The ambiguity test first opens one window with the title `title("one")`. Its two later windows have the titles `title("two-a")` and `title("two-b")`, and the test asks for `--window` with `title("two")`, which both contain and `title("one")` does not.

Each macOS test is marked `#[ignore]`:

| Test | Criterion |
|---|---|
| `monitors_have_one_primary_and_pixel_sizes` | 1 |
| `windows_lists_the_test_window` | 2 |
| `window_recording_has_the_content_size` | 3 |
| `window_and_process_find_it_and_two_are_ambiguous` | 4 |
| `region_inside_the_green_page_is_green` | 5 |
| `monitor_recording_has_the_monitor_size` | 6 |
| `still_page_stores_few_frames` | 7 |
| `ending_chrome_ends_the_recording` | 8 |
| `minimized_window_is_listed_and_refused` | 9 |
| `detached_recording_and_mcp_work_on_a_window` | 10 |

Three of these tests compare recdr with something else, and what that is, is fixed here:

- Criterion 1 needs each display's size in pixels from somewhere other than recdr. `system_profiler SPDisplaysDataType -json` lists no display on the owner's Mac (Task 4), so the test asks CoreGraphics, which ScreenCaptureKit does not use for this: `CGGetActiveDisplayList`, then for each display `CGDisplayCopyDisplayMode` and `CGDisplayModeGetPixelWidth` and `CGDisplayModeGetPixelHeight`. The two lists are compared as sorted lists of (width, height), because their orders can differ.
- Criterion 2 compares `process` with the `process` of no other source. The test asserts that it is not empty and that the entry's `pid` is running (`kill -0 <pid>` exits 0).
- Criterion 3 compares the stored frame size with `Client::window_size` times the page's `devicePixelRatio` (read with `Client::eval`), each side rounded, and allows a difference of 2 pixels each way. Task 4 measured a difference of 0 (800 by 600 points, 1600 by 1200 frames), so the allowance is 0 + 2 = 2.
- Criterion 4, `--process`: the owner's own Chrome may be running under the same name. The command must either exit 0 or fail with `ambiguous_target` whose candidates include the test window's id.

### `tests/tabs.rs`

The attribute on `mod unsupported` changes from `#[cfg(not(windows))]` to `#[cfg(not(any(windows, target_os = "macos")))]`, in Task 5. On macOS the three commands it runs stop answering `unsupported_platform` in that task.

### `tests/layers.rs`

Starting from the six tests of the cdp plan (and rule 7 of the `native-mp4` plan, when that module was built first):

| Rule | Change | Task |
|---|---|---|
| 2 | add `forbid(&["screencapturekit"], &["src/capture/macos.rs"])` | 5 |
| 2 | add `forbid(&["objc2_core_graphics"], &["src/capture/macos.rs", "src/capture/tap.rs"])`, whole in Task 5: a file in the allowed list need not exist yet | 5 |
| 2 | `objc2_core_foundation`: allowed in `src/capture/tap.rs`, and in `src/encode/avf.rs` when `native-mp4` was built first and already has a rule for it | 9 |
| 3 | `src/capture/macos.rs` joins the files that may hold `unsafe` in Task 5, and `src/capture/tap.rs` in Task 9 | 5 and 9 |

### `Cargo.toml`

```toml
[target.'cfg(target_os = "macos")'.dependencies]
screencapturekit = { version = "11.1", features = ["macos_14_0"] }
objc2-core-graphics = "0.3"
objc2-core-foundation = "0.3"       # added in Task 9
```

When `native-mp4` was built first the table exists and may already hold `objc2-core-foundation`. The cargo features found in Task 4 (`default-features = false` plus what the compiler named): `objc2-core-graphics` needs `CGDirectDisplay` (`CGMainDisplayID`, and the test's display modes) and `CGWindow` (the screen permission) from Task 5, and `CGEvent` and `CGEventTypes` from Task 9; `objc2-core-foundation` needs `CFMachPort` and `CFRunLoop` from Task 9.

### `build.rs`

Decision 15. Created in Task 5:

```rust
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
        // With only the command line tools, screencapturekit's own search path is missing.
        if let Ok(out) = std::process::Command::new("xcode-select").arg("-p").output() {
            let dir = format!("{}/usr/lib/swift/macosx", String::from_utf8_lossy(&out.stdout).trim());
            if std::path::Path::new(&dir).is_dir() {
                println!("cargo:rustc-link-search=native={dir}");
            }
        }
    }
}
```

### `.github/workflows/ci.yml`

Only when Task 4 shows that a stream delivers frames on `macos-latest`, Task 11 adds a step after the browser tests:

```yaml
- if: runner.os == 'macOS'
  run: cargo test --test desktop -- --ignored --test-threads=1
```

## File boundary

Create:
- `build.rs` (decision 15, found in Task 4)
- `src/capture/macos.rs`
- `src/capture/tap.rs`
- `tasks/4-macos-capture/acceptance.md`

Modify:
- `src/capture/mod.rs`
- `src/capture/windows.rs`
- `src/error.rs`
- `tests/common/mod.rs`
- `tests/browser.rs` (exists after `cdp-capture`)
- `tests/tabs.rs` (exists after `cdp-capture`)
- `tests/desktop.rs`
- `tests/layers.rs`
- `.github/workflows/ci.yml` (exists after `cdp-capture`)
- `Cargo.toml`
- `Cargo.lock`
- `README.md`

Delete:
- none

Files under `spec/` and `tasks/` may always be edited. `src/recorder.rs`, `src/control.rs`, `src/store.rs`, `src/encode/`, `src/capture/hook.rs`, `src/capture/cdp.rs`, `src/platform/` and `src/model.rs` are not touched.

## Task list

Details, acceptance and verification for each task are in [todo.md](todo.md).

### Phase 1: shared code, on any machine
- Task 1: the click filter moves to `capture/mod.rs`
- Task 2: points and pixels
- Task 3: the browser test helpers move to `tests/common`

### Checkpoint A: Windows is unchanged (needs a pull request)
- The three commands, the desktop tests and the browser tests pass on Windows
- By hand, by the owner: a click during a Windows recording is still in the manifest (success criterion 19)
- The owner pushes the branch and opens a draft pull request; the three CI jobs are green (decision 13)

### Phase 2: listing, on a Mac
- Task 4: try ScreenCaptureKit and the event tap, and write the results into this plan
- Task 5: the permission check and `recdr monitors`
- Task 6: `recdr windows` and the test window

### Checkpoint B: recdr sees the Mac's screen
- `recdr monitors` and `recdr windows` print the Mac's displays and windows
- The three CI jobs are green on the pull request

### Phase 3: recording
- Task 7: record a monitor and a region
- Task 8: record a window
- Task 9: clicks
- Task 10: a detached recording and the MCP tools

### Checkpoint C: everything records
- `cargo test --test desktop -- --ignored --test-threads=1` passes on the Mac
- By hand, by the owner: success criteria 11 to 15

### Phase 4: the rest
- Task 11: CI, README and the acceptance run

### Checkpoint D: complete (needs the pull request)
- The three CI jobs are green
- `tasks/4-macos-capture/acceptance.md` has a result for each of the 20 success criteria

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| No Mac is available (spec open question 1). | High | Phase 1 can still be built. Phases 2 to 4 then run only in CI, one push per try, and criteria 11 to 15 cannot be checked at all. The build stops before Task 4 and asks the owner. |
| This plan was written before `cdp-capture` was built. | Med | "Before Task 1" checks every symbol taken from the cdp plan. |
| The exact `screencapturekit` calls are not in this plan. | High | Task 4 is a throwaway program that makes each call once and writes the working code into the Contracts section. No task after it starts until that is done. |
| A hosted runner may deliver no frames. | Med | Task 4 tries one monitor frame in CI. The answer decides whether Task 11 adds the CI step. |
| ScreenCaptureKit may stretch a resized window. | Low | Decision 6 says what happens then: the build stops after Task 4 and the plan is changed first. Criterion 15 is checked by hand. |
| An event tap may be refused when recdr runs detached. | Med | Task 4 tries the tap from a process started the way `start` starts the recorder. A refusal there is the case the spec already defines: the recording runs without clicks and the warning is in `recorder.log`. |
| The click filter moves under working Windows code. | Med | Task 1 is one commit, the filter keeps its unit test, and the owner checks one real click at Checkpoint A. |
| The Swift build makes the macOS CI job slow. | Low | Measured in Task 4 and written into this plan's "What was measured". |

## Open questions

1. Is a Mac running macOS 14 or later available? Without one, see the first risk.
2. If CI cannot run a capture stream, is it acceptable that the macOS desktop tests run only on the owner's Mac? This plan assumes yes.
3. Checkpoint A needs the branch pushed and a draft pull request open, every task from Task 5 needs a push to it (decision 13), and Task 4's CI run needs a draft pull request from `spike/4-sck`.
