# Tasks: gui-overlay

Plan: [plan.md](plan.md). Spec: [spec/6-gui-overlay/spec.md](../../spec/6-gui-overlay/spec.md). "Contracts" means the Module contracts section of the plan. Every signature, constant, message and rule named below is defined there, and a task implements it exactly as written. "Criterion N" means success criterion N of the spec.

## Before Task 1

1. `cdp-capture`, `macos-capture` and `native-mp4` are merged. Cut `feat/6-gui-overlay` from `main` and commit `spec/6-gui-overlay/` and `tasks/6-gui-overlay/` as one commit.
   - Done on 2026-11-19 with the three not yet merged: the owner asked for the branch on top of `feat/5-native-mp4` (3a00892), so the pull request is stacked on #9.
2026-11-19 check of item 2: every point holds. `write` returns `Result<()>` today and Task 2 changes it. `tests/mcp.rs` lists the tools in `lists_the_nine_tools`. `tests/record.rs` has no Windows-runner skip, because `windows-latest` has an H.264 encoder. The synthetic source is 320 by 240.
2. Check what this plan took from the three plans. Each must hold; if one does not, edit the plan first.
   - `src/error.rs` has `Error::UnsupportedPlatform` with the code `unsupported_platform`.
   - `src/encode/mod.rs` has `pub fn mp4(dir: &RecordingDir, manifest: &Manifest) -> Result<PathBuf>`, `pub fn check_mp4() -> Result<()>`, the private `write` and `video_frames`, and the test helper `recording(name, image, duration_ms, fps, clicks)`.
   - `render` is called from `video_frames` and from `src/encode/gif.rs` and nowhere else in `src/`.
   - `src/recorder.rs` calls `encode::gif::encode(dir, &manifest)` and `encode::mp4(dir, &manifest)` in `finalize`, and nothing else in `src/` calls either.
   - `tests/layers.rs` has a test that lists the files that may hold `cfg`, a test that lists what `src/cli.rs` and `src/mcp.rs` may not contain, and a test that allows the texts `"recording.gif"` and `"recording.mp4"` in `src/store.rs` only.
   - `tests/common/mod.rs` has `Server` with `start(fake: Option<&str>)` and `call`, and `pub mod mp4;`.
   - `tests/mcp.rs` has one test that lists the server's tools by name. Note its name; Task 5 changes it.
   - `.github/workflows/ci.yml` runs on pull requests.
3. Note whether `tests/record.rs` has the skip that the `native-mp4` plan calls "If the Windows runner has no encoder". Task 4's MP4 test has the same skip either way.
4. Note the size of the synthetic source's pictures (320 by 240 when this plan was written). The numbers in Task 4 assume it.

Every task ends with these three commands passing, then one commit that stages the task's files by path:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Phase 1: export, on every system

- [x] **Task 1: `--format none`**
  - Description: A recording with stored frames, a manifest and a contact sheet, and no GIF or MP4.
  - Steps:
    - `src/request.rs`: `parse_formats` as in Contracts, with unit tests; the doc comment on `RecordRequest.format`.
    - `src/cli.rs`: the help text of `--format`.
    - `tests/record.rs`: the test `format_none_writes_no_gif`.
  - Acceptance:
    - `parse_formats("none")` is an empty list; `parse_formats("none,gif")` and `parse_formats("gif,none")` are `usage` errors; the message for `parse_formats("webm")` names `none`.
    - `recdr record --monitor 0 --duration 2s --format none` on the synthetic source exits 0; its result has no `gif` and no `mp4` key; `manifest.json`, `contact_sheet.png` and at least one file under `frames/` exist, and the directory has no file whose name ends in `.gif` (criterion 9).
    - The same command with `--format none,gif` exits with `usage` and creates no recording directory (criterion 9).
  - Verify: `cargo test --lib request`, `cargo test --test record format_none`.
  - Dependencies: none
  - Files: `src/request.rs`, `src/cli.rs`, `tests/record.rs`
  - Scope: S

- [x] **Task 2: the encoders take a crop and a path**
  - Found while building: with nine MP4 unit tests encoding at once, Media Foundation on the RTX 3060 failed now and then with 0xC00D6D60. The tests that encode now take turns through a lock in the test module; one thread at a time passed six runs out of six.
  - Description: The two changes to `encode` the spec names. Recording behaves as before.
  - Steps:
    - `src/encode/mod.rs`: `render`, `video_frames`, `write` and `mp4` as in Contracts.
    - `src/encode/gif.rs`: `encode` as in Contracts.
    - `src/recorder.rs`: the two calls in `finalize`.
    - The unit tests of both files pass `None` and the path they used; a test that read the returned path reads the path it passed.
  - Acceptance:
    - `render` of the `one_frame` recording with the crop (10, 20, 30, 16) is 30 by 16.
    - With `recording("crop", image, 1000, 15, Vec::new())`, where `image` is 64 by 48, black, with one white pixel at (10, 20): `render` with the crop (10, 20, 30, 16) has a white pixel at (0, 0).
    - `gif::encode` to a path in the temporary directory writes there, leaves `dir.gif_path()` not existing, and returns 1 for the `one_frame` recording and 3 for the one with a click at 200 ms.
    - `tests/record.rs` passes unchanged.
  - Verify: `cargo test --lib encode`, `cargo test --test record`.
  - Dependencies: none
  - Files: `src/encode/mod.rs`, `src/encode/gif.rs`, `src/recorder.rs`
  - Scope: S

- [x] **Task 3: the manifest of the kept part**
  - Description: `export::kept`, a function on plain data, with the types an export uses.
  - Steps:
    - `src/model.rs`: `ExportOptions` and `ExportResult`.
    - `src/export.rs`: `kept`, with unit tests. `src/lib.rs`: `pub mod export;`.
  - Acceptance, for a manifest of 3000 ms, 100 by 100, with stored frames at 0, 500, 1000, 1500 and 2500 ms:
    - `kept(m, 1200, 2600, None)` has frames at 0, 300 and 1300 ms whose `file` values are those of the entries at 1000, 1500 and 2500 ms, and a length of 1400 ms.
    - `kept(m, 0, 3000, None)` has the same frames, clicks, size and length as `m`, and no marks.
    - With clicks at (60, 50) at 1300 ms, at (5, 5) at 1300 ms and at (60, 50) at 1100 ms: `kept(m, 1200, 2600, Some(&crop))` for the crop (10, 20, 80, 60) has exactly one click, at (50, 30) at 100 ms, and the size 80 by 60 (criterion 4).
    - A click exactly at `until_ms` is left out; one exactly at `from_ms` is kept at 0 ms.
  - Verify: `cargo test --lib export`.
  - Dependencies: none
  - Files: `src/model.rs`, `src/export.rs`, `src/lib.rs`
  - Scope: S

- [x] **Task 4: `recdr export`**
  - Changed while building: `export_whole` checks the GIF's length against the `duration_ms` the export printed, allowing 5 ms a frame. A 3 second recording ran 3180 ms of GIF time on the first try, because each 67 ms frame is stored as 7 hundredths of a second. The MP4 size in the result is `encode::video_size`, see the plan's step 9 of `run`.
  - Description: The command, end to end, with every error of the spec's table.
  - Steps:
    - `src/export.rs`: `run`.
    - `src/request.rs`: `ExportRequest` and `into_options`, with unit tests for the extension rule.
    - `src/control.rs`: `export`.
    - `src/cli.rs`: `Command::Export`, `ExportCmd` and its arm. (`Command::Gui` and the second argument of `run` come in Task 8.)
    - `tests/layers.rs`: `gui::` and `export::` in the list for `src/cli.rs` and `src/mcp.rs`.
    - `tests/export.rs`: a helper `recording(name: &str) -> (PathBuf, String)` that runs `recdr record --monitor 0 --duration 3s --out <out>` with `RECDR_FAKE_SOURCE=moving` and returns the output root and the id; a helper `gif_facts(path: &Path) -> GifFacts`, where `struct GifFacts { width: u32, height: u32, duration_ms: u64, frames: usize, first: RgbaImage }` is read with the `gif` crate (the duration is the sum of the frame delays); and the tests below.
  - Acceptance:
    - `export_whole`: `--file out.gif` writes a GIF of 320 by 240 and 2900 to 3100 ms, and prints `path`, `width`, `height`, `duration_ms` and `frames`, with `frames` equal to `GifFacts.frames` (criterion 1).
    - `export_trimmed`: with `--from 1s --until 2s` the GIF is 900 to 1100 ms long. Every channel of every pixel of its first picture is within 8 of the stored frame `recdr frames <id> --at 1s` names, and the same comparison with the frame at 0 ms fails (criterion 2).
    - `export_cropped`: with `--crop 10,20,100,80` the GIF is 100 by 80, and pixel (0, 0) of its first picture is within 8 per channel of pixel (10, 20) of the stored frame at 0 ms (criterion 3).
    - `export_mp4`: `--file out.mp4 --from 1s --until 2s` writes a file whose `common::mp4::facts` are H.264 and 900 to 1100 ms (criterion 5). The test prints a note and passes without the file in two cases, which are the cases where the MP4 recording tests skip: the command fails with `ffmpeg_not_found`; or it fails with `encode_failed` while `cfg!(windows)` and the environment variable `CI` is set.
    - `export_errors`: an unknown id and the id of a recording still running (`start`, then export, then `stop`) give `recording_not_found`; `--file out.webm`, `--from 2s --until 1s`, `--until 10s`, `--crop 300,200,100,80` and `--crop 0,0,1,1` give `usage`; a `--file` inside a directory that does not exist gives `encode_failed`. After each, the target file does not exist, and no file whose name contains `.part.` is in the target directory (criterion 6).
    - `export_leaves_the_recording_alone`: the name, size and bytes of every file under the recording directory are the same before and after `export_whole`'s command (criterion 7).
    - `export_replaces_only_on_success`: an export onto an existing file replaces it; a failed export onto an existing file (the `--until 10s` case) leaves its bytes as they were.
  - Verify: `cargo test --test export`, `cargo test --test layers`.
  - Dependencies: Tasks 2 and 3
  - Files: `src/export.rs`, `src/request.rs`, `src/control.rs`, `src/cli.rs`, `tests/layers.rs`, `tests/export.rs`
  - Scope: M

- [x] **Task 5: the MCP tool `export`**
  - Description: The same export for an agent.
  - Steps:
    - `src/mcp.rs`: the tool in Contracts.
    - `tests/mcp.rs`: `export` joins the expected names in the test that lists the tools, and the test's name loses its count (`lists_the_tools`); a new test `export_returns_the_result`.
  - Acceptance:
    - The server lists one more tool than before, named `export` (criterion 8).
    - `record` with a 1 second duration, then `export` with that `id`, the same `out` and a `file` ending in `.gif`: the reply parses to an object with the five keys, and the file exists (criterion 8).
    - `export` with a `file` ending in `.webm` is an error reply whose JSON has the code `usage`.
  - Verify: `cargo test --test mcp`.
  - Dependencies: Task 4
  - Files: `src/mcp.rs`, `tests/mcp.rs`
  - Scope: S

### Checkpoint A: export is done
- [x] The three commands pass on the development machine (T5 logs).
- [x] By hand: `recdr record --monitor 0 --duration 5s`, then `recdr export <id> --file cut.gif --from 1s --until 3s --crop 0,0,640,360`; the GIF plays, is 2 seconds long and 640 by 360.
  - 2026-11-19, Windows 11, a 1920 by 1080 monitor: the recording was 5005 ms; export printed `"width":640,"height":360,"duration_ms":2000,"frames":1` (the desktop did not change in that second), and ffprobe read 640 by 360 and 2.000000 s. The recording stayed in the scratchpad and is not committed.

## Phase 2: the logic part, on every system

- [x] **Task 6: frame and toolbar arithmetic**
  - Description: `src/gui/geometry.rs`, with unit tests.
  - Steps:
    - `src/gui/mod.rs`: `pub mod geometry;`. `src/lib.rs`: `pub mod gui;`.
    - `src/gui/geometry.rs`: all of Contracts.
    - `tests/layers.rs`: the rule for files under `src/gui/`.
  - Acceptance (unit tests; one monitor of 1920 by 1080 at (0, 0) unless stated; the toolbar is `toolbar_size(1.0)`, which is (560, 40)):
    - `to_pixels(100.0, s)` is 100, 150 and 200 for `s` of 1, 1.5 and 2, and `to_points` gives 100 back for each. `toolbar_size(1.5)` is (840, 60).
    - `initial` is (640, 300, 640, 480).
    - `keep_inside` of (100, 100, 10, 10) is 32 by 32; of (0, 0, 5000, 5000) is (8, 8, 1904, 1024).
    - With a second monitor of 1920 by 1080 at (1920, 0), `keep_inside` of (1500, 500, 640, 480) is (1272, 500, 640, 480), on the first monitor, and of (1700, 500, 640, 480) is (1928, 500, 640, 480), on the second.
    - `keep_inside` of (100, 30, 640, 1024), where the toolbar fits neither under nor above, is (100, 8, 640, 1024), and `toolbar_place` of that is `Under`. For 200 rectangles from a fixed list or a seeded generator, `toolbar_place` of `keep_inside` is never `Inside`.
    - `toolbar_place` is `Under` for (100, 100, 640, 480), `Above` for (100, 500, 640, 560), and `Inside` for (0, 0, 1920, 1080).
    - `toolbar_rect` for the `Inside` case is (0, 1040, 560, 40); for a frame at `x` 1800 its `x` is 1360.
    - `borders` of (100, 100, 200, 100): the four rectangles do not overlap each other or the inside, and together with the inside they cover exactly (92, 92, 216, 116).
    - `edge_at(0, 5, 216)` is `TopLeft`, `edge_at(0, 100, 216)` is `Top`, `edge_at(3, 95, 100)` is `BottomRight`.
    - `resized` of (100, 100, 200, 100) by `Left` and `dx` 50 is (150, 100, 150, 100); by `Left` and `dx` 190 is (268, 100, 32, 100).
    - `crop_of((10.0, 10.0), (110.0, 60.0), (320.0, 240.0), (640, 480))` is (20, 20, 200, 100); the two points in the other order give the same; a drag of half a shown pixel gives `None`.
    - `time_at(50.0, 200.0, 3000)` is 750; a negative `x` gives 0; an `x` past the width gives 3000.
  - Verify: `cargo test --lib gui::geometry`, `cargo test --test layers`.
  - Dependencies: none
  - Files: `src/gui/mod.rs`, `src/gui/geometry.rs`, `src/lib.rs`, `tests/layers.rs`
  - Scope: M

- [x] **Task 7: the state machine**
  - Seen while building: one full `cargo test` run took 14.7 s for the library tests (5.6 s usual) and `encode::tests::finish_with_no_frame_returns` (native-mp4) hit its 10 second limit. The next nine runs passed. Not changed here.
  - Description: `src/gui/state.rs`: every row of the table in Contracts, with a unit test per row.
  - Steps:
    - `src/gui/mod.rs`: `pub mod state;`.
    - `src/gui/state.rs`: all of Contracts.
    - `src/request.rs`: `PartialEq` on `RecordRequest`. `src/model.rs`: `PartialEq` on `WindowInfo` and `MonitorInfo`.
  - Acceptance (unit tests):
    - `App::new` gives `Idle`, 15 frames per second, both switches on, and `[ListMonitors]`.
    - `RecordPressed` with the frame at (100, 200, 640, 480) gives a `Start` whose request has `region` `Some("100,200,640,480")`, `format` `Some("none")`, `fps` `Some(15)` and `out`; with a window picked it has `window_id` and no `region`. A second `RecordPressed` before `Started` gives nothing.
    - `Picked(Some(id))` puts `inside` at the window's bounds; `Resized` is then ignored; `Moved` sets `window` to `None`.
    - In `Recording`: `Tick` 999 ms after the start gives nothing, at 1000 ms gives `AskStatus`, and at 1500 ms nothing again. `Status(Status::Finished)` gives `Stop` once. `StopPressed` twice gives `Stop` once. `Stopped` leads to `Editing` with the handles at 0 and `duration_ms`, and a `Preview` for 0 ms.
    - `Failed` while starting returns to `Idle` with the message; `Failed` after `StopPressed` returns to `Idle` with the message.
    - `CloseRequested`: `[Quit]` in `Idle` and `Editing`. In `Recording` it gives `Stop`, and the `Stopped` that follows gives `[Quit]` and not the editor. Between `RecordPressed` and `Started` it gives nothing; the `Started` that follows gives `Stop`, and the `Stopped` after that `[Quit]`; a `Failed` in place of `Started` gives `[Quit]` (criterion 22's rule).
    - In `Editing`: with `playing`, two `Tick`s 40 ms apart move `position_ms` by 40; at `until_ms` it returns to `from_ms`. While `loading`, a `Seek` gives no second `Preview`; the `Picture` that follows gives a `Preview` for the new position. `From` cannot pass `until_ms - 100`. `Save` gives an `Export` whose request has `from` `"0ms"`, `until` as `"{duration}ms"` and the crop text, and a second `Save` before `Exported` gives nothing.
    - A `step` with a `now_ms` earlier than the last one does not panic.
    - For each screen, every event variant that the table does not list for it leaves the `App` equal to a copy taken before whose `last_ms` was set to the new time, and returns no action.
  - Verify: `cargo test --lib gui::state`.
  - Dependencies: Task 6, Task 4 (for `ExportRequest`)
  - Files: `src/gui/mod.rs`, `src/gui/state.rs`, `src/request.rs`, `src/model.rs`
  - Scope: M

- [x] **Task 8: the worker, and `recdr gui` as a command that is refused everywhere**
  - Description: The last piece that needs no window. After it `recdr gui` exists and answers `unsupported_platform` on every system; Task 10 makes it open the app on Windows and macOS.
  - Steps:
    - `src/gui/mod.rs`: `pub mod worker;`, and `run` returning `Err(Error::UnsupportedPlatform)`.
    - `src/gui/worker.rs`: `perform`.
    - `src/cli.rs`: `Command::Gui`, `GuiCmd`, the second argument of `run`, and its arm. `src/main.rs`: passes `recdr::gui::run`.
    - `tests/gui.rs`: `gui_is_refused_where_there_is_no_app` and `help_lists_export_and_gui`.
  - Acceptance:
    - Unit tests of `perform` with an empty temporary directory as `out`: `AskStatus("nope")`, `Stop("nope")`, `Preview` and `Export` for the id `nope` each give `Failed` with a text that contains `nope`; `Quit` gives `None`.
    - `gui_is_refused_where_there_is_no_app`: `recdr gui` exits 1 with the code `unsupported_platform`. From Task 10 the test returns at once when `cfg!(any(windows, target_os = "macos"))`; in this task it runs on every system (criterion 10).
    - `help_lists_export_and_gui`: the output of `recdr --help` contains the words `export` and `gui`.
  - Verify: `cargo test --lib gui::worker`, `cargo test --test gui`.
  - Dependencies: Task 7
  - Files: `src/gui/mod.rs`, `src/gui/worker.rs`, `src/cli.rs`, `src/main.rs`, `tests/gui.rs`
  - Scope: S

### Checkpoint B: the logic is done (needs a pull request)
- [x] The owner pushes the branch and opens a draft pull request. The build stops and asks.
  - #11, draft, base `feat/5-native-mp4`, pushed on the owner's go-ahead.
- [ ] The three CI jobs are green (criterion 10).
  - Run 44379181138: macOS and Linux green in every step. Windows passes `cargo fmt`, `clippy` and `cargo test`, then fails in the browser step on `animation_keeps_fifteen_frames_per_second` (`tests/browser.rs:191`, cdp-capture), the same failure as on #8 and #9. Left open until that test is fixed.

## Phase 3: the windows part, on Windows

- [x] **Task 9: try the windows, and write the results into this plan**
  - 2026-11-19: answered on the app itself instead of a throwaway program; the eight answers are in the plan's "What was measured". 2 and 6 are yes, 3 is "the grip takes the press", 4 is answered with a 1-pixel correction for the outer corner, 8 is 11.9 MB and glow. 1, 5 and 7 need a drag or a click and are left for the owner; 7 led to the toolbar growing while the menu is open. None is a no, so the build did not stop (decision 8). Found: a gray system outline around each border window that egui cannot switch off on Windows; for the owner.
  - Description: A throwaway cargo project with `eframe` 0.37, in a directory outside the repository. Nothing from it is committed.
  - Steps: the program shows four border windows around a 640 by 480 area and a toolbar window under it, and is used to answer, on the development machine:
    1. Do the four windows stay together while the area is moved and resized by dragging, and stay above other windows?
    2. Does a click in the middle of the area reach the application under it?
    3. Does a transparent part of a border window take a press, or let it through?
    4. Which `ViewportBuilder` and `ViewportCommand` calls place a window at a position given in pixels, on the primary monitor and on a second monitor with another scale factor, if one is at hand?
    5. How is the pointer's position on the screen, in pixels, read during a drag that moves the window under the pointer?
    6. Can the main window change from a bare always-on-top strip to a decorated window of 960 by 640 points and back, with `ViewportCommand` alone?
    7. Does a combo box in an undecorated always-on-top window open its list?
    8. How large is a release build of the program, and how long does a first build take? Which `eframe` features does it need?
  - Acceptance:
    - The plan's "What was measured" has an answer to each of the eight, or "could not be tried" with the reason.
    - The plan's section for `app.rs`, `overlay.rs` and `editor.rs` names the calls that worked, and `Cargo.toml` in Contracts names the features.
    - If answer 1 or 2 is no, or 5 or 6 is no and no other way inside `egui` is found, the build stops and asks the owner (decision 8).
    - The size and the build time are reported to the owner.
  - Verify: the eight answers are in the plan's "What was measured".
  - Dependencies: Checkpoint B
  - Files: `tasks/6-gui-overlay/plan.md`
  - Scope: S

- [ ] **Task 10: the app opens, with a toolbar that can record and stop**
  - 2026-11-19: code done with Tasks 11 to 13 in one commit (the windows part was written whole from the contracts). Checked on Windows, see `acceptance.md`: on top, 640 x 480, Record/Stop and status (15), terminal stop (18), close while recording and the exit codes (22), the switches reach the request (23), layers pass, clippy clean for macOS and Linux. Left: the cursor half of 23 with the pointer inside the frame.
  - Description: `recdr gui` opens one window on Windows and macOS: the toolbar, always on top. The whole loop from input to `state::step` to `worker::perform` and back runs. No frame is drawn yet, so a recording made now is of the starting region.
  - Steps:
    - `Cargo.toml`, `Cargo.lock`: `eframe` and `rfd`.
    - `src/gui/mod.rs`: the `cfg` lines and the two `run` functions of Contracts.
    - `src/gui/app.rs`: `run`, the app value, the event and action loop, the close rule, the repaint every 100 ms while recording.
    - `src/gui/overlay.rs`: `show` draws the toolbar of Contracts for `Idle` and for `Recording`, at `geometry::toolbar_rect`, and no border window.
    - `src/gui/editor.rs`: `show` draws the text `recording <id> finished`, `message`, and "New recording". Task 13 replaces it.
    - `tests/layers.rs`: the rule 2 line, and `src/gui/mod.rs` in rule 6.
    - `tests/gui.rs`: the early return on Windows and macOS.
  - Acceptance (by hand on Windows, each result into `acceptance.md`):
    - `recdr gui` shows the toolbar above other windows, with the size 640 x 480.
    - Record, 5 seconds, Stop: the text `recording <id> finished` appears within 2 seconds, and `recdr status` in a terminal lists the recording as `finished` with 4.5 to 5.5 seconds (criterion 15).
    - `recdr stop <id>` in a terminal during a recording brings the same text within 2 seconds (criterion 18).
    - Closing the app while it records leaves a recording that `recdr status` calls `finished` (criterion 22). "Close" and the window's own close both end the process with exit code 0.
    - With "Cursor" off no stored frame has the cursor; with "Clicks" off the manifest's `clicks` list is empty (criterion 23).
    - `cargo test --test layers` passes; on Linux `recdr gui` still exits 1 with `unsupported_platform`.
  - Verify: the three commands, and the checks above.
  - Dependencies: Task 9
  - Files: `Cargo.toml`, `Cargo.lock`, `src/gui/mod.rs`, `src/gui/app.rs`, `src/gui/overlay.rs`, `src/gui/editor.rs`, `tests/layers.rs`, `tests/gui.rs`
  - Scope: M (eight files, because the three files of the windows part must appear together; `Cargo.lock` is generated, and the two test files change by one line each)

- [ ] **Task 11: the frame**
  - 2026-11-19: code done. Checked: inside equals the toolbar's size to the pixel (11), center click goes through (13), stored frames have the toolbar's size and no border pixel (16). Left for the owner: every drag (12), the target menu and window placement (14).
  - Description: The four border windows, moving and resizing, and the target menu placing the frame.
  - Steps:
    - `src/gui/overlay.rs`: the border windows in `Idle`, the drags of Contracts, and the toolbar following the frame.
    - `src/gui/app.rs`: what the drags need from the app value, if Task 9's answer 5 needs something kept there.
  - Acceptance (by hand on Windows, each result into `acceptance.md`):
    - The frame is above other windows; the size in the toolbar equals the inside of the frame in a screenshot, in pixels (criterion 11).
    - The toolbar moves the frame; edges and corners resize it; it stops at 32 by 32; dragged across two monitors it ends wholly on one (criterion 12).
    - A click inside the frame presses a button of the application under it (criterion 13, before recording).
    - Choosing a window in the menu puts the frame around it within 4 pixels; a maximized window puts the toolbar inside the bottom edge (criterion 14, the placement half). Moving the frame afterwards puts the menu back on "Region".
    - The stored frames of a region recording have exactly the size the toolbar showed, and none has a pixel of the border or the toolbar (criterion 16).
  - Verify: the three commands, and the checks above.
  - Dependencies: Task 10
  - Files: `src/gui/overlay.rs`, `src/gui/app.rs`
  - Scope: M

- [ ] **Task 12: the look of a recording**
  - 2026-11-19: code done. Checked: red line and the seconds counter (17); border windows pass presses through while recording. Left for the owner: that a drag does nothing, the maximized-window case (14), the closed-window message.
  - Description: What the person sees while it records.
  - Steps:
    - `src/gui/overlay.rs`: in `Recording` the border windows are red and take no drag; the toolbar shows the elapsed time.
  - Acceptance (by hand on Windows, each result into `acceptance.md`):
    - While recording the border is red and the toolbar counts seconds (criterion 17); the frame cannot be moved or resized.
    - A click inside the frame still reaches the application under it (criterion 13).
    - A maximized window recorded as a window target has the toolbar in no stored frame (criterion 14).
    - Picking a window, closing that window, then Record: the toolbar shows recdr's message and the app stays idle.
  - Verify: the three commands, and the checks above.
  - Dependencies: Task 11
  - Files: `src/gui/overlay.rs`
  - Scope: S

- [ ] **Task 13: the editor**
  - 2026-11-19: code done. Checked: Play shows motion and swaps to Pause, Save GIF equals `recdr export` byte for byte, Save MP4 matches it, "exporting" and disabled buttons during an export, New recording returns and keeps the directory. Left for the owner: timeline drags, handles, the crop rectangle, a trimmed and cropped save, Space, and the MP4 error case (the save dialog would not take an unwritable folder).
  - Description: Preview, trim, crop and save.
  - Steps:
    - `src/gui/editor.rs`: `show` as in Contracts.
    - `src/gui/app.rs`: loading the picture into a texture; the change of the main window on entering and leaving the editor; the repaint while playing.
  - Acceptance (by hand on Windows, each result into `acceptance.md`):
    - Play shows motion; dragging the timeline shows the picture of that time; the handles change the length readout; a dragged rectangle changes the size readout; Space toggles play (criterion 19).
    - "Save GIF" with a trim and a crop writes a file whose length and size match the two readouts, with click rings when "Clicks" was on. The length and size are read by exporting the same trim and crop with `recdr export` and comparing its printed result, and by opening the file (criterion 20).
    - "Save MP4" does the same; with the save target in a folder that cannot be written, the editor shows recdr's message (criterion 21).
    - While an export runs the three buttons are disabled and the line says "exporting".
    - "New recording" returns to the overlay, and the recording directory is still there.
  - Verify: the three commands, and the checks above.
  - Dependencies: Task 12
  - Files: `src/gui/editor.rs`, `src/gui/app.rs`
  - Scope: M

### Checkpoint C: the app works on Windows
- [ ] By hand, by the owner: criteria 11 to 23 on Windows, each with its result in `acceptance.md`.

## Phase 4: macOS, and the rest

- [ ] **Task 14: the app on the Mac**
  - Description: The same app on macOS 14 or later. Needs the owner's Mac.
  - Steps:
    - Run Task 9's program on the Mac and write the eight answers for macOS into the plan.
    - `cargo build` and the three commands on the Mac.
    - Run the checklist of criteria 11 to 23. Fix what differs in the three files of the windows part; a fix that needs a rule changed goes into `geometry` or `state` with a unit test.
  - Acceptance:
    - Criteria 11 to 23 on macOS, each with its result in `acceptance.md`.
    - A difference that cannot be fixed inside `egui` stops the build and goes to the owner (decision 8).
    - The limit about full-screen applications (spec, Risks) is confirmed or corrected, for the README.
  - Verify: the checklist.
  - Dependencies: Checkpoint C, and a Mac
  - Files: `src/gui/app.rs`, `src/gui/overlay.rs`, `src/gui/editor.rs`, `src/gui/geometry.rs`, `src/gui/state.rs`, `tasks/6-gui-overlay/plan.md`, `tasks/6-gui-overlay/acceptance.md`
  - Scope: M

- [ ] **Task 15: README and the acceptance run**
  - Description: The README describes the app, export and `--format none`, and every criterion has a recorded result.
  - Steps:
    - `README.md`: `recdr gui`, what it records, and that it needs Windows or macOS; `recdr export` with its flags; the MCP tool `export`; `--format none`; the limits (no monitor or tab recording from the app, the frame does not follow a window, full-screen applications on macOS, the yellow border on Windows 10); that a shortcut to the app should pass `--out`.
    - `tasks/6-gui-overlay/acceptance.md`: one line per criterion, 1 to 23, with the test name or the by-hand result, per system for 11 to 23.
  - Acceptance:
    - Every criterion has a result or a stated reason it could not be run.
    - The three CI jobs are green.
  - Verify: the three commands, and the three CI jobs.
  - Dependencies: Task 14
  - Files: `README.md`, `tasks/6-gui-overlay/acceptance.md`
  - Scope: S

### Checkpoint D: complete
- [ ] The three CI jobs are green.
- [ ] `acceptance.md` has a result for each of the 23 criteria.
