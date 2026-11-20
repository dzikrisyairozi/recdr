# Spec: macos-capture

Module `macos-capture` of the [capability map](../1-recdr/capability-map.md). Tracking issue: https://github.com/dzikrisyairozi/recdr/issues/4, a sub-issue of #1.

Depends on `agent-recorder` ([spec](../1-recdr/spec-agent-recorder.md), pull request #2) and `cdp-capture` ([spec](../3-cdp-capture/spec.md), issue #3). This spec was written on 2026-11-18, before `cdp-capture` was built. File names below are the ones the `cdp-capture` spec gives. The plan for this module is written after `cdp-capture` is merged, against the code as it then is.

## Words used here

- **Desktop target**: a window, a screen region or a monitor. The other kind of target is a browser tab.
- **Point**: the unit macOS uses for positions and sizes on screen. On a Retina display one point is two pixels wide and two high.
- **Scale factor**: pixels per point for one display, 1 or 2 on current Macs.
- **Pixel**: one sample of a stored frame. recdr reports and accepts every position and size in pixels, on every system.
- **Screen Recording permission**: the macOS privacy setting "Screen & System Audio Recording". Without it a program cannot list windows or read the screen.
- **Input Monitoring permission**: the macOS privacy setting that lets a program see mouse and keyboard events sent to other programs.
- **Responsible program**: macOS grants both permissions to the program that started recdr, for example Terminal, iTerm or the editor that hosts the agent, and not to the `recdr` file itself.

## Objective

After `cdp-capture`, recdr runs on macOS but records only browser tabs. `recdr windows`, `recdr monitors` and the target flags `--window-id`, `--window`, `--process`, `--region` and `--monitor` fail there with `unsupported_platform`.

This module makes those commands and flags work on macOS, through ScreenCaptureKit, with the same output as on Windows: the same JSON, the same files in the recording directory, the same error codes and stop reasons, the same MCP tools. Everything after capture (frame store, GIF and MP4, contact sheet, `diff.json`, `mark`, `frames`, the MCP server) is not changed.

The user is the same as in the first spec: an AI agent working in a terminal, on a Mac whose owner has given the agent's host program the Screen Recording permission.

### User stories

- As an agent on macOS, I list windows, pick one, record it for ten seconds and read the contact sheet, with the commands I use on Windows.
- As an agent, when the permission is missing I get an error that names the setting to change and the program to change it for, so I can tell the owner what to do.
- As the owner, I am asked for a second permission only when click rings are wanted, and a recording still works when I refuse it.

## Tech stack

Added for macOS only, under `[target.'cfg(target_os = "macos")'.dependencies]`:

| Crate | Version | License | Used for |
|---|---|---|---|
| `screencapturekit` | 11.1, feature `macos_14_0` | MIT OR Apache-2.0 | Listing windows and displays, and the capture stream |
| `objc2-core-graphics` | 0.3 | Zlib OR Apache-2.0 OR MIT | The two permission checks and the mouse event tap |
| `objc2-core-foundation` | 0.3 | Zlib OR Apache-2.0 OR MIT | The run loop the event tap needs |

The smallest macOS version supported for desktop targets is macOS 14.0, because the window's content size in pixels comes from `SCContentFilter.contentRect` and `pointPixelScale`, which arrived in 14.0. This spec does not define what a desktop target does on an older macOS. Tab targets should keep working there; whether a recdr built with the `macos_14_0` feature still starts on macOS 13 is on the "Not checked" list, and if it does not, the README names macOS 14 as the smallest version for recdr on macOS.

`screencapturekit` compiles a small Swift package when it is built, so building recdr on macOS needs the Xcode command line tools. Building on Windows and Linux is not affected, because the crate is a macOS-only dependency.

### What was checked

By reading the crate sources in the local cargo registry and the pages named, on 2026-11-18. Nothing in this module can run on the Windows machine this spec was written on.

1. `screencapturekit` 11.1.2 has `SCShareableContent` with windows (`window_id`, `title`, `frame`, `window_layer`, `is_on_screen`, `owning_application` with `application_name`, `bundle_identifier`, `process_id`) and displays; `SCContentFilter` with `content_rect` and `point_pixel_scale`; stream settings `with_source_rect`, `with_shows_cursor`, `with_minimum_frame_interval`, `with_pixel_format`, `with_scales_to_fit`; a frame status with the values `Complete`, `Idle`, `Blank`, `Suspended`; and a delegate method for a stream that stopped with an error.
2. Its `build.rs` runs `xcrun` and `swift build`, with a deployment target of macOS 12.3.
3. `objc2-core-graphics` 0.3.2 has `CGPreflightScreenCaptureAccess`, `CGRequestScreenCaptureAccess`, `CGPreflightListenEventAccess`, `CGRequestListenEventAccess` and `CGEventTapCreate`.
4. An Apple engineer wrote on the Apple developer forums (thread 707680) that a `CGEventTap` needs the Input Monitoring permission, that an `NSEvent` global monitor needs the Accessibility permission, and that the two `CG...ListenEventAccess` functions check and ask for the first.
5. The script that builds GitHub's hosted macOS images (`configure-tccdb-macos.sh` in `actions/runner-images`) gives the Screen Recording permission to `/bin/bash` and to the runner's own agent. The `screencapturekit` project runs its capture tests on `macos-14` runners.
6. The same script says that on macOS 15 and later a program that reads the screen without the system picker causes a repeating alert, "is requesting to bypass the system private window picker". The system picker is the macOS window in which a person chooses by hand what a program may record (`SCContentSharingPicker`).

### Not checked

Each of these is measured by the first tasks of the plan, on a Mac, before anything is built on it.

- Whether a ScreenCaptureKit stream delivers frames on a hosted `macos-latest` runner.
- Whether an event tap that asks only for mouse-button events needs the Input Monitoring permission, or is allowed without it.
- What ScreenCaptureKit delivers for a window that is on another Space, or minimized.
- How often the alert in fact 6 appears for a program started from a terminal.
- The exact frame size ScreenCaptureKit gives for a window that has a shadow or a toolbar.
- Whether macOS counts the same responsible program for a detached recorder. `recdr start` runs the recorder as a separate process in its own session (see the `cdp-capture` spec, "Linux and macOS"), and the stream is opened there.
- Whether a recdr built with the `macos_14_0` feature starts on macOS 13.
- Whether ScreenCaptureKit's own scaling pads a resized window with black, or stretches it.

## Commands

No new command and no new flag. On macOS these now work as on Windows:

```
recdr windows
recdr monitors
recdr record  (--window-id <id> | --window <text> | --process <name> | --region x,y,w,h | --monitor <index>) ...
recdr start   (the same target flags) ...
```

Build, test and lint commands are the ones in the first spec. The desktop tests run with:

```
cargo test --test desktop -- --ignored --test-threads=1
```

## Behavior

### Positions and sizes

Every number recdr prints or accepts is in pixels, as on Windows. macOS reports positions in points, so recdr converts:

- Let `S` be the largest scale factor among the connected monitors.
- A monitor's `bounds` origin is its position in points times `S`. Its `bounds` size is its size in points times its own scale factor, which is the size of a stored frame of that monitor.
- A position inside a monitor is the monitor's `bounds` origin, plus the offset from the monitor's corner in points times that monitor's own scale factor. A window's `bounds` are worked out this way, on the monitor that holds the window's center, and its size is its size in points times that monitor's scale factor.
- `--region x,y,w,h` must lie inside the `bounds` of one monitor, as on Windows. The region relative to that monitor, divided by the monitor's scale factor, is the rectangle in points that ScreenCaptureKit wants.
- A stored frame of a region is exactly `w` by `h` pixels. A stored frame of a window is the window's content size in points times the scale factor.

Because every origin is scaled by the same `S`, and no monitor is larger in pixels than its size in points times `S`, the `bounds` of two monitors never overlap. When all monitors share a scale factor this is simply points times the scale factor. With a Retina laptop and a 1x external display there are gaps between monitors in this space, which do no harm: a position read off a stored frame, added to the `bounds` origin of what was recorded, is a valid `--region` position on the same monitor.

Choosing the monitor for a region, matching a window by title or process, and refusing a minimized window are already done by `resolve` in `src/capture/mod.rs`, from the lists of windows and monitors. The macOS code supplies those lists in pixels and `resolve` is used unchanged. The only macOS step is the division by the scale factor when the stream is opened.

### Listing

`recdr monitors` prints one entry per display, with `index` starting at 0 in the order CoreGraphics lists the active displays (`CGGetActiveDisplayList`), `bounds`, and `primary` true for the main display.

Both lists come from CoreGraphics (`CGGetActiveDisplayList` and `CGWindowListCopyWindowInfo`), not ScreenCaptureKit; only the recorder opens a ScreenCaptureKit stream. A process that has asked ScreenCaptureKit for its shareable content and is still running when a recorder it started opens a stream keeps that stream from starting (found in the plan's Task 8), and `start` and the MCP server are such processes. Window ids are the same `CGWindowID` in both frameworks.

`recdr windows` prints the windows that are at layer 0 (ordinary application windows), have a title, and belong to an application. This rule is called the window filter below. Each window has:

| Field | Value on macOS |
|---|---|
| `id` | The window number macOS assigns (`CGWindowID`) |
| `title` | The window title |
| `process` | The owning application's name (`kCGWindowOwnerName`), for example `Google Chrome` |
| `pid` | The owning application's process id (`kCGWindowOwnerPID`) |
| `bounds` | In pixels, by the rule above |
| `minimized` | True when macOS reports the window as not on screen: minimized, hidden with its application, or on another Space |

`--window` and `--process` match as on Windows. `--process` is compared with the `process` field.

### Recording

- A window whose `minimized` is true when recording starts fails with `target_minimized`, as on Windows, with the message it has today. The README says what `minimized` covers on macOS.
- Frames arrive at no more than `--fps` per second. A frame that ScreenCaptureKit marks as idle, blank or suspended is not stored, so a screen that does not change produces few frames, as on Windows.
- `--no-cursor` leaves the cursor out of the frames.
- When the recorded window is resized, later frames are scaled to fit the starting size and padded with black, as the first spec requires. The stream is opened with the starting size as its fixed output size. If ScreenCaptureKit's own scaling pads with black, it is used; if it stretches, the stream delivers frames at their real size and recdr fits them. The plan picks one after the check on the "Not checked" list.
- When the recorded window closes, or ScreenCaptureKit stops the stream for any other reason, the recording ends with `stop_reason` `window_closed` for a window target, and fails with `capture_failed` for a region or a monitor.
- recdr draws nothing on screen. macOS itself shows its screen-recording indicator in the menu bar while a stream runs; recdr cannot turn it off.

### The Screen Recording permission

Before listing or resolving a desktop target, recdr asks macOS whether it may read the screen (`CGPreflightScreenCaptureAccess`). This happens in the command the person or agent ran, for `record` and for `start` alike, because both resolve the target before any recorder runs. When the answer is no, recdr calls `CGRequestScreenCaptureAccess`, which makes macOS show its permission dialog the first time a program asks and do nothing on later calls, and fails with a new error code:

| Code | Meaning | Exit code |
|---|---|---|
| `permission_denied` | macOS does not let the responsible program read the screen | 1 |

The message is: `macOS has not given this program the Screen Recording permission. Open System Settings > Privacy & Security > Screen & System Audio Recording, switch on the program that started recdr (your terminal or editor), restart that program, and run recdr again.`

Nothing is written to disk in that case. recdr never changes a privacy setting itself.

If macOS refuses the stream inside a detached recorder although the check above passed, the recorder fails with `permission_denied` too, and `recdr start` reports it through the start-up error path that `cdp-capture` adds.

### Clicks

Clicks are recorded as on Windows: position relative to the top-left corner of the captured area in pixels, button, and time; drawn as rings in the GIF and MP4 and never in the stored frames.

On macOS seeing a mouse press in another program takes an event tap, which needs the Input Monitoring permission. When click recording is on (the default):

1. recdr asks macOS whether it may listen (`CGPreflightListenEventAccess`). When the answer is no, it calls `CGRequestListenEventAccess`, which makes macOS show its dialog the first time a program asks.
2. When the tap cannot be created, the recording runs without clicks. The process that records writes one line to its stderr, which for a detached recorder is the file `recorder.log` in the recording directory: `clicks are not recorded: macOS has not given this program the Input Monitoring permission (System Settings > Privacy & Security > Input Monitoring)`. The manifest's `clicks` list is empty and no ring is drawn. The exit code is 0.
3. When the tap is created, it listens for left, right and other mouse-button presses and for nothing else. It is a listen-only tap: it cannot change or block an event.

With `--no-click-highlight` recdr creates no tap and asks for no permission.

A press is converted to pixels with the scale factor of the captured area, and a press outside the captured area is dropped. Windows has the same step, with a scale factor of 1: after `cdp-capture` it is a function in `capture/windows.rs` that turns a press in screen coordinates into a click relative to the captured area, or nothing. That function moves to `capture/mod.rs`, which is compiled on every system, gains a scale factor argument, and is called by both desktop sources. It is called the click filter below.

### Changes to existing behavior

- On macOS, desktop targets, `windows` and `monitors` work where they returned `unsupported_platform`.
- One new error code, `permission_denied`.
- On Linux nothing changes: desktop targets still fail with `unsupported_platform`.
- On Windows nothing changes.

## Project structure

New and changed files, using the layout the `cdp-capture` spec gives.

```
src/capture/macos.rs      New. The Screen Recording check, the lists of windows and monitors in pixels, the ScreenCaptureKit source
src/capture/tap.rs        New. The mouse event tap and the Input Monitoring check
src/capture/mod.rs        Changed. A third choice of desktop back end: Windows, macOS, or unsupported. Gains the click filter and the point-to-pixel arithmetic
src/capture/windows.rs    Changed. Calls the click filter in capture/mod.rs
src/platform/unix.rs      Unchanged
src/error.rs              Changed. PermissionDenied
tests/desktop.rs          Changed. Compiled on Windows and macOS; a test window per system
tests/browser.rs          Changed. Its page server, pages and CDP client move to tests/common/mod.rs
tests/common/mod.rs       Changed. The macOS test window; the page server, pages and CDP client from tests/browser.rs
tests/layers.rs           Changed. The rules below
.github/workflows/ci.yml  Changed. The macOS job runs the desktop tests, if the first task shows it can
Cargo.toml, Cargo.lock    Changed. The three crates
README.md                 Changed. macOS desktop targets, the two permissions, the limits
```

`CaptureSource` gets its fourth implementation. The macOS source owns its event tap the way the Windows source owns its mouse hook, and reports clicks as `CaptureEvent::Click`. The recorder, the store and the encoders are not touched.

The conversion from points to pixels and the click filter take plain numbers and return plain numbers, and live in `capture/mod.rs`, so their unit tests run on every system.

The layer rules are the ones in the `cdp-capture` spec, checked by `tests/layers.rs`. Changes:

- `unsafe` may also appear in `capture/macos.rs` and `capture/tap.rs`.
- New: `screencapturekit` appears only in `capture/macos.rs`. `objc2_core_graphics` appears only in `capture/macos.rs` (the Screen Recording check) and `capture/tap.rs`. `objc2_core_foundation` appears only in `capture/tap.rs`.
- The rule that system differences are decided only in `capture/mod.rs`, `platform/mod.rs` and `lib.rs` stays as it is.

## Code style

As in the first spec. One addition: every `unsafe` block in the two new files has a comment that says what makes it sound, as the Windows files do.

## Testing strategy

| Level | Covers | Needs |
|---|---|---|
| Unit | Point-to-pixel conversion at scale 1 and 2 and with two monitors of different scale factors, the click filter | Nothing. Runs on all three systems |
| Unit, macOS only | The window filter, and that the tap asks for mouse-button presses and no other event type | A macOS build. No permission |
| Integration, no display (`tests/control.rs` and others) | Unchanged. On Linux, `unsupported_platform` for desktop targets | Nothing |
| Desktop, marked `#[ignore]` (`tests/desktop.rs`) | Every success criterion that names a test window | A Mac with the Screen Recording permission given to the program that runs `cargo test` |
| By hand | The permission dialogs, clicks, a Retina display, a second monitor | The owner's Mac |

The test window on macOS is a Chrome window with a known title and known pixels. The browser tests of `cdp-capture` (`tests/browser.rs`) have what is needed: a small HTTP server on 127.0.0.1 that serves three pages, and a client that sends CDP commands to a test browser. Both move to `tests/common/mod.rs` so the desktop tests can use them. The pages are:

| Path | Title | What it shows |
|---|---|---|
| `/anim` | `anim` | A white page with a red box that moves on every animation frame |
| `/still` | `still` | A white page with one heading, which never changes |
| `/green` | `green` | A page filled with the color `#00aa00` |

A desktop test starts Chrome with `--app=http://127.0.0.1:<port>/<page>`, a temporary `--user-data-dir`, `--window-size=800,600` and `--remote-debugging-port=0`. It minimizes the window with the CDP command `Browser.setWindowBounds`, and closes it by ending the Chrome process it started. Neither needs a macOS permission. Chrome's path comes from `RECDR_TEST_CHROME`, as in the browser tests. On Windows the test window stays what it is today.

No test sends a mouse press, as in the `cdp-capture` spec: an injected press lands on whatever is under the pointer. The click filter and the conversion have unit tests, and one real click is checked by hand.

CI runs the macOS desktop tests on `macos-latest` if the plan's first task shows that a stream delivers frames there. If it does not, they run only on the owner's Mac, and the plan says so. The plan's first task showed that it does, so CI runs them, with one test skipped: `region_inside_the_green_page_is_green`, because on the runner the screen itself shows the page pale at that point. `region_recording_is_the_region_size` still checks criterion 5's 400x300 there.

## Boundaries

The boundaries of the first spec and of `cdp-capture` stay in force. Additions:

Always:
- Stop the stream and remove the event tap when a recording ends, including when it ends because of an error.
- Keep the three CI jobs green before merging. They are the ones `cdp-capture` sets up, on `windows-latest`, `ubuntu-latest` and `macos-latest`.

Ask first:
- Any other macOS permission (Accessibility, Automation, Full Disk Access).
- Recording audio.
- A dependency that is not in the table above.

Never:
- Listen for keyboard events. The tap's event list holds mouse-button presses only.
- Change, block or send an input event.
- Write to the macOS privacy database, or tell the owner to switch off System Integrity Protection.
- Use a private macOS API.

## Non-goals

- macOS older than 14.0 for desktop targets.
- Desktop targets on Linux.
- Audio.
- The system picker (`SCContentSharingPicker`). An agent cannot click it.
- Recording a window that is minimized, hidden or on another Space.
- A signed or notarized application bundle. recdr stays a command-line program that the owner builds or downloads.
- Hiding the macOS screen-recording indicator.
- A result field that reports the missing click permission. The warning goes to stderr and the log.

## Alternatives considered

| Option | Why it lost |
|---|---|
| `objc2-screen-capture-kit`, the generated bindings, in place of `screencapturekit` | No Swift at build time, and `cargo check` for a macOS target would work from Windows. But every call is `unsafe`, and the stream output needs a hand-written Objective-C class and a dispatch queue. None of it can be run on the machine this is developed on, so the smaller, safe API is worth more than the faster type check. |
| `CGDisplayStream` or `CGWindowListCreateImage` | Deprecated by Apple, and removed for new programs in macOS 15. |
| An `NSEvent` global monitor for clicks | It delivers events only on the main thread of a program that runs an `NSApplication` loop, and by fact 4 it needs the Accessibility permission, which also lets a program control the computer. |
| No clicks on macOS | The owner chose the second permission on 2026-11-18, so recordings look the same on both systems. |
| Listing only on-screen windows | Windows lists a minimized window with `minimized: true`, and an agent that looks for a window by title should find it and learn why it cannot be recorded. |
| Reporting positions in points on macOS | A stored frame is in pixels. An agent that reads a position off a frame and passes it to `--region` would be wrong by the scale factor. |

## Risks

| Risk | Mitigation |
|---|---|
| Nothing here can be run on the Windows machine where the first two modules were built. | Build this module in a session on the owner's Mac. Where that is not possible, CI on `macos-latest` is the only place the code runs, and each change costs a CI run. |
| A hosted runner may deliver no frames. | The plan's first task records one monitor frame in CI. The answer decides where the desktop tests run. |
| macOS 15 and later repeat the "bypass the system private window picker" alert. | Measure how often on the owner's Mac. Document it in the README. The alert does not stop a recording that is running. |
| The rule for monitors with different scale factors was worked out on paper. | It has unit tests with a 2x and a 1x monitor side by side, and a by-hand check on two real monitors if the owner has them. |
| The Swift package in `screencapturekit` makes the macOS build slower and adds a build requirement. | Measure the build time in CI in the first task. If it is not acceptable, the alternative in the first row is still open, because all ScreenCaptureKit code is in one file. |
| The crate's API changes in a later major version. | `Cargo.lock` pins the version, and `screencapturekit` is named in one file only. |

## Success criteria

A "test window" is the Chrome window described under Testing strategy, 800 by 600 points, showing `/anim` unless a criterion names another page. Criteria 1 to 10 are desktop tests on macOS; 11 to 15 are by hand on the owner's Mac; the rest are automated.

1. `recdr monitors` prints at least one monitor, exactly one of them with `primary` true, and each `bounds` size equals that display's size in points times its scale factor.
2. `recdr windows` lists the test window with its title, `minimized` false, a `pid` that is a running process, and a `process` equal to the application name of the Chrome the test started.
3. `recdr record --window-id <id> --duration 3s` exits 0 and writes a GIF between 2.5 and 3.5 seconds long with at least 20 stored frames, whose size is the test window's content size in points times the scale factor.
4. `--window <text>` and `--process <the test Chrome's application name>` find the test window; two test windows with matching titles give `ambiguous_target` listing both; no match gives `target_not_found`.
5. With the test window showing `/green`: `recdr record --region x,y,400,300 --duration 2s`, where the region is centered on the window's `bounds`, stores frames of exactly 400 by 300 pixels, and the center pixel of each is green (its green value is more than twice its red and its blue).
6. `recdr record --monitor 0 --duration 2s` stores frames whose size equals the monitor's `bounds` size.
7. A test window showing `/still`, recorded for 3 seconds, stores between 1 and 3 frames and has a contact sheet.
8. Ending the test Chrome during a recording ends the recording with `stop_reason` `window_closed` and a playable GIF.
9. A test window minimized through CDP before recording starts is listed with `minimized` true, and recording it fails with `target_minimized`.
10. `recdr start`, `mark`, `stop` and `frames` work on a window target, and the MCP tools `list_windows`, `list_monitors` and `record` return the same shapes as on Windows.
11. With the Screen Recording permission off, `recdr windows` and `recdr record --monitor 0 --duration 1s` exit 1 with `permission_denied` and the message above, and no recording directory is created. Starting from a permission that was reset with `tccutil reset ScreenCapture`, macOS shows its dialog on the first run and not on the second.
12. With both permissions on, a click inside the recorded window is in the manifest within 4 pixels of where it was made and within 200 ms of when, and is a ring in the GIF and not in the stored frames. On a Retina display the position is in pixels, twice the position in points.
13. With the Input Monitoring permission off, the same recording exits 0, prints the warning above on stderr, and has an empty `clicks` list. With `--no-click-highlight`, no Input Monitoring dialog appears.
14. With `--no-cursor` the cursor is in no frame; without it the cursor is in the frames when it is over the window.
15. When the recorded window is made smaller during a recording, every stored frame keeps the starting size, and the later ones show the whole window with black around it.
16. The unit tests for the conversion and the click filter pass on Windows, Linux and macOS. They include a 2x monitor next to a 1x monitor: the two `bounds` do not overlap, and a position inside each converts to points and back to itself.
17. A unit test on macOS shows that the tap asks for left, right and other mouse-button presses and for no other event type.
18. On Linux, `recdr windows` still exits 1 with `unsupported_platform`.
19. Every desktop test of the first spec still passes on Windows, and a click made by hand on Windows is still recorded as before (the click filter moved).
20. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass on the three CI jobs, and the layer tests pass with the changed rules.

## Decisions made while writing

- The owner chose click rings on macOS with the second permission, and a recording without clicks when it is refused (2026-11-18).
- `screencapturekit` over the generated bindings, for the reason in Alternatives.
- macOS 14.0 as the smallest version, so the frame size of a window comes from the system and is not guessed.
- Positions in pixels on every system.
- One new error code, `permission_denied`, in place of reusing `capture_failed`, so an agent can tell "ask the owner for a permission" from "something broke".
- The test window is Chrome in `--app` mode, which the browser tests already require.
- Changed during the build (2026-11-20): the window and monitor lists come from CoreGraphics, not ScreenCaptureKit, because a running `start` or MCP server that had used ScreenCaptureKit kept its recorder's stream from starting. The owner is told in the pull request.

## Open questions

1. Does the owner have a Mac running macOS 14 or later? Criteria 11 to 15 need one, and the Risks table recommends building the whole module there.
2. If CI cannot run a capture stream, is it acceptable that the macOS desktop tests run only on the owner's Mac?
