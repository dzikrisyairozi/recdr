# Spec: gui-overlay

Module `gui-overlay` of the [capability map](../1-recdr/capability-map.md). Tracking issue: https://github.com/dzikrisyairozi/recdr/issues/6, a sub-issue of #1.

Depends on `agent-recorder` ([spec](../1-recdr/spec-agent-recorder.md), pull request #2). On macOS it also needs `macos-capture` ([spec](../4-macos-capture/spec.md), issue #4), because the app records a region or a window. This spec was written on 2026-11-18, before the modules in front of it were built; the plan is written when this module's turn comes, against the code as it then is.

It is the last module in the build order, so it describes recdr as it will be after `cdp-capture` ([spec](../3-cdp-capture/spec.md)), `macos-capture` and `native-mp4` ([spec](../5-native-mp4/spec.md)). Three things it leans on come from `cdp-capture` and do not exist in the code on the day of writing: the error code `unsupported_platform`, the directory `src/platform/`, and the three CI jobs on `windows-latest`, `ubuntu-latest` and `macos-latest`. The layer table and layer rules it changes are the ones in the `cdp-capture` spec under "Project structure", as changed by the two specs after it.

## Words used here

- **The app**: what `recdr gui` opens.
- **Frame**: a rectangle outline that the app draws on the screen. The area inside it is what gets recorded. (Elsewhere in recdr "frame" means one stored picture; in this spec that is a "stored frame".)
- **Toolbar**: a strip of controls attached to the frame.
- **Overlay**: the frame and the toolbar together.
- **Editor**: the screen shown after a recording stops, where the recording is trimmed, cropped and exported.
- **Trim**: keep only the part of a recording between two times.
- **Crop**: keep only a rectangle of every picture.
- **Export**: write a GIF or an MP4 from a finished recording, with a trim and a crop applied.
- **Pixel**: a physical pixel, the unit recdr uses for `--region` and for stored frames.

From the first spec:

- **Stored frame**: a PNG recdr wrote because the screen changed. **Manifest**: the JSON file listing the stored frames, their times, the clicks and the marks.
- **Click ring**: the circle drawn at a click in a GIF or MP4, and never in a stored frame.
- **Mark**: a named moment an agent adds to a running recording with `recdr mark`.
- **Window target**, **region target**: a recording of one window (`--window-id`), or of a rectangle of the screen (`--region`).
- **Synthetic source**: a test-only source of frames that needs no screen, chosen with the environment variable `RECDR_FAKE_SOURCE`. Integration tests record from it.

## Objective

recdr has no window of its own. A person who wants to record part of the screen must work out pixel coordinates for `--region`, and cannot cut the result afterwards.

This module adds a desktop app in the manner of [ScreenToGif](https://github.com/NickeManarin/ScreenToGif): put a frame over the thing to record, press Record, press Stop, trim and crop, save a GIF or an MP4. It runs on Windows and macOS.

The app is a third front end, next to the CLI and the MCP server. It adds no recording code. It starts and stops recordings through the same library functions the CLI uses, so a recording made in the app is an ordinary recording directory that `recdr status`, `recdr frames` and the MCP tools can read.

The one capability the app needs that recdr does not have is export with a trim and a crop. The capability map says every action is available without a GUI, so export is also a CLI command and an MCP tool.

The users are a person at the desktop, and, for `export`, the agent of the first spec.

### User stories

- As a person, I drag a frame over part of my screen, press Record, do the thing, press Stop, and save a GIF, without typing a coordinate.
- As a person, I pick a window from a list and the frame snaps to it.
- As a person, I cut the first two seconds off and crop away the margins before saving.
- As an agent, I trim and crop a finished recording with one command and get a file.
- As a person, I keep working in the application under the frame while I record: my clicks go to it.

## Tech stack

| Crate | Version | License | Used for |
|---|---|---|---|
| `eframe` (with `egui`) | 0.37 | MIT OR Apache-2.0 | The windows and everything drawn in them |
| `rfd` | 0.18 | MIT | The system's "Save as" dialog |

Both are dependencies on Windows and macOS only. `eframe` 0.37 needs Rust 1.95; the project is on 1.98. The fonts `egui` bundles are under the Ubuntu Font Licence, the SIL Open Font License and MIT, none of them GPL.

### What was checked

By reading `egui` 0.37.0 in the local cargo registry and the current recdr code, on 2026-11-18:

1. `ViewportBuilder` has `with_transparent`, `with_decorations`, `with_always_on_top`, `with_mouse_passthrough`, `with_position`, `with_inner_size` and `with_taskbar`, and `ViewportCommand` has `OuterPosition`, `InnerSize`, `MousePassthrough` and `WindowLevel`. `egui` can show more than one window from one app.
2. `control::start` starts the detached recorder by running the current executable with a hidden first argument. An app in a second executable would have to understand that argument too.
3. `control::frames` returns paths of stored frames, chosen by time, optionally scaled down. The editor's preview can be built on it.
4. `encode::timeline` builds the sequence of pictures of a recording from the manifest alone, and `encode::render` draws one of them with its click rings. Both encoders are built on the two.
5. `encode::gif::encode` and `encode::ffmpeg::encode` always write to the recording directory (`dir.gif_path()`, `dir.mp4_path()`).
6. `RecordOptions.formats` is a list, and the recorder writes a GIF or an MP4 only when the list names it. `request::parse_formats` refuses an empty value, so no front end can ask for none today.
7. `request::parse_region` reads `x,y,w,h`, and `time::parse_time` reads the times `--duration` takes.

### Not checked

Each is settled by the first tasks of the plan with a small throwaway window, on Windows and on the owner's Mac, before the app is built.

- How to let clicks pass through the inside of the frame while its border still takes them. `with_mouse_passthrough` applies to a whole window. Two ways are known: build the frame from four thin border windows, which needs nothing outside `egui`; or cut a hole in one window with a system call per platform.
- Whether a transparent, undecorated, always-on-top `eframe` window draws correctly on Windows 10, Windows 11 and macOS 14 and later.
- The size of the `recdr` executable and the build time with `eframe` added.
- Positions at display scale factors other than 1, and on a second monitor.

## Commands

```
recdr gui [--out <dir>]
recdr export <id> --file <path> [--from <time>] [--until <time>] [--crop x,y,w,h] [--out <dir>]
```

`--out` is the output root, `.recdr` in the current directory by default, as for every other command.

One existing flag gains a value: `--format none` on `record` and `start` (and `format: "none"` in the MCP tools) records stored frames, the manifest, the contact sheet and `diff.json`, and writes no GIF and no MP4. `none` cannot be combined with `gif` or `mp4`. The app records this way, so that Stop does not wait for a GIF nobody asked for, and an agent that only wants frames can use it too.

### `recdr export`

Writes one file from a finished recording.

| Flag | Meaning |
|---|---|
| `<id>` | A recording id, as printed by `record`, `start` and `status` |
| `--file <path>` | Where to write. The extension picks the format: `.gif` or `.mp4`. An existing file is replaced |
| `--from <time>` | Start of the part to keep, for example `2s` or `1500ms` (a whole number, as `--duration` takes). Default: the start of the recording |
| `--until <time>` | End of the part to keep. Default: the end of the recording |
| `--crop x,y,w,h` | Rectangle to keep, in pixels, relative to the top-left corner of the recording. Default: the whole picture |

It prints `{"path", "width", "height", "duration_ms", "frames"}`, where `frames` is the number of pictures written, and `width` and `height` are the size of the pictures in the file (for an MP4 of an odd size, one pixel more, as in a recording).

Rules:

- The recording directory is only read. Stored frames, the manifest, `recording.gif` and `recording.mp4` are never changed by an export.
- Click rings and their timing are drawn as in a normal GIF or MP4, moved by the crop and the trim. A click outside the crop or outside the kept time draws nothing.
- Times are read by the parser `--duration` uses, and `--crop` by the parser `--region` uses. No second parser is written.
- An MP4 is written by whatever writes MP4 on that system at the time: ffmpeg today, the system encoder after `native-mp4`.

Errors. Every code here exists in recdr before this module:

| Case | Code |
|---|---|
| The id is unknown, or the recording has not finished | `recording_not_found` |
| `--file` has an extension other than `.gif` or `.mp4` | `usage` |
| `--file` is inside the recording directory | `usage`, and no file there changes |
| `--from` is not before `--until`, or either lies outside the recording | `usage` |
| `--crop` is not inside the picture, or is smaller than 2 by 2 | `usage` |
| MP4 asked for where it cannot be written | The code the check before recording gives on that system: `ffmpeg_not_found`, or after `native-mp4` `encode_failed` on Windows and macOS |
| The file cannot be written, or the encoder fails | `encode_failed` |

The MCP server gains one tool, `export`, with the arguments `id`, `file`, `from`, `until`, `crop` and `out`.

`export` works on Windows, macOS and Linux.

### `recdr gui`

Opens the app. It runs until its window is closed. On Linux it exits 1 with `unsupported_platform`, the code `cdp-capture` adds for a thing recdr cannot do on the system it runs on.

## Behavior of the app

The app is in one of three states.

### Idle: the overlay

- The frame is a rectangle outline, 2 pixels wide, always on top of other windows. The toolbar sits directly under it, or directly above it when there is no room under it.
- The person moves the frame by dragging the toolbar, and resizes it by dragging a corner or an edge. The smallest inside size is 32 by 32 pixels. The frame stays inside one monitor, as `--region` requires; dragged across two monitors, it belongs to the monitor that holds its center and is moved wholly onto it. Its inside is never taller than the monitor minus the toolbar, so the toolbar always fits.
- A click inside the frame goes to whatever is under it.
- The toolbar shows: the inside size in pixels; a target menu; frames per second (1 to 60, default 15); a switch for the mouse cursor; a switch for click rings; a Record button; a Close button.
- The target menu holds "Region" and the windows `recdr windows` lists, read each time the menu is opened. With "Region", the inside of the frame is recorded. With a window chosen, the frame is placed around that window once, cannot be resized, and that window is recorded as a window target. The frame is then only a guide: a window target records the window wherever it is, so the frame does not follow a window that moves. Choosing the window again places the frame again. Moving the frame by hand returns the menu to "Region".
- A chosen window can fill the monitor, leaving no room for the toolbar under or above the frame. The toolbar then sits inside the frame, along its bottom edge. It is not in the recording, because a window target records only the window's own pixels.

### Recording

- Record starts a recording exactly as `recdr start` does, with the target above, the chosen frame rate and switches, the default limits of the first spec (5 minutes, 2 GB), and `--format none`.
- The border turns red. The frame cannot be moved or resized. The toolbar shows the time elapsed and a Stop button.
- Clicks inside the frame still go to whatever is under it.
- Neither the border nor the toolbar is in the recording: both lie outside the recorded rectangle, and a window target records only the window.
- Stop ends the recording as `recdr stop` does. While it records, the app asks for the recording's status once a second, with the library function behind `recdr status`. That is how it notices a recording that ended another way (a limit reached, the window closed, `recdr stop` from a terminal). Either way the app goes to the editor.
- When a recording cannot start, for example because the chosen window has gone, the toolbar shows the error message recdr gives, and the app stays idle.

### Editing: the editor

An ordinary window, which replaces the overlay.

- A preview of the recording, and under it a timeline as long as the recording.
- Play and pause; dragging on the timeline shows the stored frame at that time. Space toggles play and pause while the editor has focus.
- Two handles on the timeline set where the kept part starts and ends. At the start they are at the two ends.
- A rectangle dragged on the preview sets the crop. A button clears it.
- Readouts: kept length in seconds, and the size in pixels after the crop.
- "Save GIF" and "Save MP4" open the system's save dialog, then export with the trim and the crop. While an export runs the buttons are disabled and a line says so; when it ends the line shows the path, or the error message.
- "New recording" returns to the overlay. The recording directory stays where it is; the app deletes nothing.
- The preview shows stored frames, so it shows no click rings. The exported file has them, when the click switch was on.

### Closing

Closing the app while it records stops the recording first. Closing it at any other time just closes it. Recording directories are kept.

### Changes to existing behavior

- Two new commands, `gui` and `export`, and one new MCP tool, `export`.
- `--format` accepts `none`.
- The `recdr` executable grows, because it now contains the app on Windows and macOS.

Nothing else changes. The first spec's non-goal "a border or other on-screen sign drawn by recdr while recording" still holds for the CLI and the MCP server: only `recdr gui` draws anything.

## Project structure

```
src/gui/              New. The app. Two parts, described below; how each part is split into files is for the plan
src/export.rs         New. Checks a trim and a crop against a recording, builds the manifest of the kept part, and hands it to an encoder
src/encode/mod.rs     Changed. render takes an optional crop
src/encode/gif.rs     Changed. Takes the path to write to
src/encode/ (MP4)     Changed. Each MP4 encoder takes the path to write to
src/control.rs        Changed. export
src/request.rs        Changed. --file, --from, --until, --crop, with the two existing parsers; format none
src/model.rs          Changed. Export options and the export result
src/cli.rs            Changed. gui and export commands
src/mcp.rs            Changed. export tool
tests/export.rs       New. Export on the synthetic source
tests/layers.rs       Changed. The rules below
Cargo.toml, Cargo.lock  Changed. eframe, rfd
README.md             Changed. The app, export, format none
```

How export works: by fact 4 the pictures of a recording follow from its manifest. Export does not teach the encoders about trims. It builds a second manifest in memory for the kept part (the stored frames from the one current at `--from` onward, with times moved back by `--from`; the clicks inside the kept time and the crop, moved the same way; the length; the cropped size) and gives it to the encoder that already exists. Two things change in `encode`: `render` cuts the picture to the crop, and an encoder is told where to write, because by fact 5 it can only write into the recording directory today.

The app is one executable with the CLI, started by a subcommand, because of fact 2: the detached recorder is started by running the current executable again.

`src/gui/` has two parts, and the line between them is what this spec fixes:

- **Logic**: which state the app is in, what each event does, and all arithmetic on frames, toolbars and crops. It does not name `egui`, opens no window, and is compiled and unit-tested on Windows, macOS and Linux.
- **Windows**: what is drawn and how input becomes events. It is compiled on Windows and macOS only.

How the frame is built (one window or four) is decided by the first check on the "Not checked" list, so the file layout of the second part is left to the plan.

The layer table gains one front end and one middle module:

```
cli, mcp, gui             front ends; they never import each other
control, request
recorder, export
analyze, encode
capture, store
draw, platform, time
error
model
```

Layer rules, changes only:

- `gui` is a front end: it imports only `control`, `request`, `model` and `error`.
- `eframe`, `egui` and `rfd` appear only under `src/gui/`, and not in its logic part.
- `unsafe` does not appear under `src/gui/`. The app is built with what `egui` offers. If the checks on the "Not checked" list show that a system call is needed, this spec is changed first: a front end cannot reach `platform/`, and `platform/` cannot take an `egui` window, so where such a call would live is a design question and not a detail.
- Which part of `gui` is compiled where is decided in `src/gui/mod.rs`, which joins the files where system differences are decided (`cfg`), next to `capture/mod.rs`, `platform/mod.rs`, `encode/mod.rs` and `lib.rs`.

## Code style

As in the first spec. Two additions:

- The logic part decides; the windows part draws. Whatever the app does to recdr (start a recording, stop it, ask for its status, list windows, export) is decided in the logic part, where a test can see it without a window.
- Sizes cross between the app and recdr in pixels. `egui` works in points; the conversion is in one place in the logic part.

## Testing strategy

| Level | Covers | Needs |
|---|---|---|
| Unit | The logic part: every event in every state; frame and toolbar placement, including a window that fills the monitor; keeping the frame inside one monitor; points to pixels at scale 1, 1.5 and 2; trim and crop checks; the manifest export builds for a trim and a crop, including a moved click | Nothing. Runs on all three systems |
| Integration (`tests/export.rs`, `tests/mcp.rs`) | `recdr export` and the MCP tool on a recording of the synthetic source: length after a trim, size after a crop, every error in the table; `--format none` | Nothing for GIF. An MP4 export needs what an MP4 recording needs on that system, and is skipped where the MP4 recording tests are skipped |
| By hand | Everything the person sees: the frame, clicks passing through, a recording made from the app, the editor, the save dialog | The owner's Windows machine and Mac. A checklist in `tasks/6-gui-overlay/acceptance.md` |

No automated test opens the app's windows, and no test sends mouse or keyboard input to the desktop. A hosted CI runner has no dependable way to show a transparent always-on-top window, and an injected click lands on whatever is under the pointer.

## Boundaries

The boundaries of the earlier specs stay in force. Additions:

Always:
- Stop the recording when the app closes.
- Keep the logic part covered by unit tests.
- Keep the three CI jobs green before merging.

Ask first:
- A dependency that is not in the Tech stack table.
- Deleting or renaming anything in a recording directory from the app.
- Any system call made for the app's windows outside `egui`.

Never:
- Listen for keys outside the app's own windows. There is no global shortcut.
- Send input to another program.
- Put the frame or the toolbar in a recording.
- Send a recording anywhere. Export writes a local file.

## Non-goals

- Recording a whole monitor from the app. The toolbar would be in the picture, and keeping a window out of a capture is done differently on each system and was not checked on macOS 15. The CLI records monitors.
- Recording a browser tab from the app.
- Editing beyond trim and crop: deleting single frames, captions, drawing, speed changes, transitions.
- Pause and resume.
- A global shortcut, a tray icon, or a countdown.
- Sound, or a webcam.
- An installer, an application bundle, or a signed executable. The app is started with `recdr gui`.
- The app on Linux.
- Click rings in the editor's preview.
- Marks in the app.
- A frame that follows a moving window.

## Alternatives considered

| Option | Why it lost |
|---|---|
| A second executable for the app | By fact 2 the recorder is started by running the current executable again, so the second executable would need the recorder inside it as well, or a way to find the first. One executable needs neither. |
| The app records in its own process | It would be a second way to record, with its own failure cases. Going through `start` and `stop` means a recording survives an app crash and is visible to the CLI. |
| Trim and crop only inside the app | The capability map says every action is available without a GUI. An agent can use export too. |
| Export rewrites the recording directory | A trim that removes stored frames cannot be undone. Export reads the recording and writes one new file. |
| Giving the encoders a time range | Three encoders would each learn about trims. A manifest for the kept part gives the same result with no change to their loops. |
| The app records with the default GIF | Stop would wait for a GIF of the whole recording before the editor opens, and the person may want an MP4 or a trimmed GIF. |
| The frame follows a chosen window | It needs a polling loop and its own rules for a guide that the recording does not depend on. |
| A cargo feature that leaves the app out | Two builds to test, for a saving nobody asked for. Revisit if the executable size measured in the plan is a problem. |
| Tauri | Rejected in the capability map: it draws the UI in a WebView. |
| A path typed into a text box in place of a save dialog | A desktop app without the system's save dialog is awkward to use, and `rfd` is one small MIT dependency. |
| Automated tests that drive the app's windows | They need input injection or a display on CI. The logic is tested without a window, and the rest is a short checklist. |

## Risks

| Risk | Mitigation |
|---|---|
| Clicks may not pass through the inside of the frame with what `egui` offers. | First task of the plan: a throwaway window on each system. Four border windows is the way that needs no system call. If that fails too, the spec is changed before anything else is built. |
| The macOS half cannot be run on the Windows machine. | Build and check it in a session on the owner's Mac. The by-hand checklist is run on both systems before the module is called done. |
| Mixed display scale factors put the frame a few pixels off. | All conversion is in one place in the logic part, with unit tests at three scale factors, and the checklist has a second-monitor item. |
| On macOS an always-on-top window does not appear over an application in full-screen mode. | Documented as a limit. |
| `eframe` makes the executable much larger and the build slower for people who only use the CLI. | Measure both in the first task. If the owner finds them too large, the cargo feature in Alternatives is the answer. |
| On Windows 10 the capture API draws its own yellow border around a captured window, next to the app's frame. | Known from the first spec; documented. |

## Success criteria

Criteria 1 to 10 are automated and run on every system. Criteria 11 to 23 are the by-hand checklist, run on Windows and on macOS.

1. `recdr export <id> --file out.gif` on a 3-second recording of the synthetic source writes a GIF of the recording's size and between 2.9 and 3.1 seconds, and prints the result object.
2. With `--from 1s --until 2s` the GIF is between 0.9 and 1.1 seconds long, and its first picture equals the stored frame current at 1 second.
3. With `--crop 10,20,100,80` every picture is 100 by 80, and its top-left pixel equals pixel (10, 20) of the matching stored frame.
4. For a recording with a click at (60, 50), the manifest export builds for `--crop 10,20,100,80` has that click at (50, 30). A click outside the crop, or before `--from`, is not in it (unit test, because the synthetic source reports no clicks).
5. `--file out.mp4` writes an H.264 MP4 of the trimmed length where an MP4 can be written.
6. Every row of the error table gives the stated code, and no file is written.
7. After an export, the recording directory's files are byte-for-byte what they were.
8. The MCP tool `export` returns the same result object, and the server lists one more tool than before.
9. `recdr record --format none --duration 2s` on the synthetic source exits 0 and leaves stored frames, a manifest and a contact sheet, and no `recording.gif`. `--format none,gif` is a `usage` error.
10. The unit tests of the logic part pass on all three systems, and the layer tests pass with the changed rules. On Linux `recdr gui` exits 1 with `unsupported_platform`.
11. `recdr gui` shows a frame and a toolbar on top of other windows. The toolbar's size readout matches the frame's inside in pixels, checked against a screenshot.
12. The frame moves and resizes with the mouse, stops at 32 by 32, and cannot be left half on one monitor and half on another.
13. A click inside the frame presses a button in the application under it, before and during a recording.
14. Choosing a window in the target menu places the frame around it within 4 pixels. Choosing a maximized window puts the toolbar inside the frame's bottom edge, and the toolbar is in no stored frame of that recording.
15. Record, wait 5 seconds, Stop: the editor opens within 2 seconds of Stop, and `recdr status` in a terminal lists the recording as finished with a length between 4.5 and 5.5 seconds.
16. The stored frames of a region recording are exactly the size the toolbar showed, and no pixel of the border or the toolbar is in any of them.
17. During a recording the border is red and the toolbar counts seconds.
18. `recdr stop <id>` from a terminal during a recording sends the app to the editor within 2 seconds.
19. In the editor, play shows motion; dragging the timeline shows the matching picture; the two handles change the length readout; a dragged rectangle changes the size readout.
20. "Save GIF" with a trim and a crop writes a file whose length and size match the two readouts, with click rings when the switch was on.
21. "Save MP4" does the same where an MP4 can be written, and shows recdr's error message where it cannot.
22. Closing the app during a recording leaves a finished recording, not one that `recdr status` calls `recording` or `crashed`.
23. With the cursor switch off the cursor is in no stored frame; with the click switch off the manifest has no clicks.

## Decisions made while writing

- The owner chose Windows and macOS for the app (2026-11-18).
- One executable, started with `recdr gui`.
- The app is a front end over `control`, and export is a library function with a CLI command and an MCP tool.
- Recordings made in the app are written with `--format none`; GIF and MP4 come from export. `none` is a value any front end can use.
- Export builds a manifest for the kept part and uses the encoders as they are.
- The frame is placed around a chosen window once and does not follow it.
- The capability map's "Depends on" cell for this module now reads "`agent-recorder`; on macOS also `macos-capture`". The build order does not change: this module is last.
- The app records a region or a window, not a monitor and not a tab.
- The default output root is `.recdr` in the current directory, as for every other command. A shortcut to the app should pass `--out`.
- The app is always built in on Windows and macOS; no cargo feature.

## Open questions

1. Is a Mac available for the macOS half (the same question as in the `macos-capture` spec)?
