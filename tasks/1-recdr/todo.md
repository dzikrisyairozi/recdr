# Tasks: agent-recorder

Plan: [plan.md](plan.md). Spec: [spec/1-recdr/spec-agent-recorder.md](../../spec/1-recdr/spec-agent-recorder.md). "Contracts" means the Module contracts section of the plan. Every signature, constant and rule named below is defined there, and a task implements it exactly as written.

Every task ends with these three commands passing, then one commit that stages the task's files by path:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`cargo test` includes `tests/layers.rs`, which fails when a module breaks a layer rule.

Integration tests set `RECDR_FAKE_SOURCE` and pass `--out` with a fresh directory under the system temp directory, so they need no desktop. Tests in `tests/desktop.rs` are marked `#[ignore]` and run with `cargo test --test desktop -- --ignored`.

Each task adds the `pub mod` line in `src/lib.rs` (or in the parent `mod.rs`) for every file it creates.

## Phase 1: a recording on disk

- [x] **Task 1: Package scaffold and the JSON error contract**
  - Description: Create the Cargo package, the `Error` type, and a `recdr` binary that turns any bad command line into the JSON usage error.
  - Steps:
    - Stop and tell the owner if the directory `recdr/` exists at the repository root. It is a stray clone that the owner removes.
    - Create the branch `feat/1-agent-recorder` from `main` and stay on it for every task.
    - `.gitignore` with the lines `/target`, `.recdr/` and `.serena/`.
    - Commit `spec/1-recdr/` and `tasks/1-recdr/` alone as `docs(agent-recorder): add spec and plan`.
    - `Cargo.toml` with the package fields and the Task 1 dependencies from the plan's table.
    - `src/error.rs` as in the contracts, without the `image::ImageError` conversion. Until Task 2 creates `WindowInfo`, `AmbiguousTarget.candidates` is `Vec<serde_json::Value>`.
    - `src/lib.rs`: `pub mod cli; pub mod error; pub use error::{Error, Result};`.
    - `src/cli.rs`: `run(serve_mcp)` with a `Cli` whose `Command` enum has only `Mcp`.
    - `src/main.rs` as in the contracts, with the private `no_mcp` function.
    - `tests/common/mod.rs`, first line `#![allow(dead_code)]`: `pub fn recdr() -> std::process::Command` (the built binary, with `RECDR_FAKE_SOURCE` removed from its environment), `pub fn out_dir(name: &str) -> PathBuf` (deletes then creates `temp_dir()/recdr-test-<process id>-<name>`), `pub fn json(bytes: &[u8]) -> serde_json::Value`.
    - `tests/cli.rs` and `tests/layers.rs` (the five rule tests in the contracts; a rule about a file that does not exist yet passes).
  - Acceptance:
    - `recdr --bogus` exits 2, stdout is empty, and stderr is JSON with `error.code` equal to `usage`.
    - `recdr mcp` exits 2 with `usage`.
    - A unit test in `error.rs` builds each of the 14 variants and checks `code()` against the 14 strings in the spec's Error codes table, and that only `Usage` has `exit_code()` 2.
  - Verify: `cargo test --test cli`, `cargo test --test layers`, `cargo test --lib error`.
  - Dependencies: none
  - Files: `.gitignore`, `Cargo.toml`, `Cargo.lock`, `src/main.rs`, `src/lib.rs`, `src/error.rs`, `src/cli.rs`, `tests/common/mod.rs`, `tests/cli.rs`, `tests/layers.rs`
  - Scope: M (three of the files are configuration and two are under ten lines)

- [x] **Task 2: `recdr windows` and `recdr monitors`**
  - Description: List real windows and monitors as JSON, and resolve a target against such a list.
  - Steps:
    - Add `windows-capture` and `windows` to `Cargo.toml`.
    - `src/model.rs`: `Rect`, `WindowInfo`, `MonitorInfo`, `FakeKind`, `Target`, `ResolvedKind`, `ResolvedTarget`. Switch `Error::AmbiguousTarget.candidates` to `Vec<WindowInfo>`.
    - `src/platform.rs`: `set_dpi_aware`.
    - `src/capture/mod.rs`: `list_windows`, `list_monitors`, `resolve`.
    - `src/capture/windows.rs`: `list_windows`, `list_monitors`.
    - `src/control.rs`: `list_windows` and `list_monitors`.
    - `src/main.rs`: add the `recdr::platform::set_dpi_aware();` line.
    - `src/cli.rs`: add `Command::Windows` and `Command::Monitors`.
  - Acceptance:
    - Unit test in `error.rs`: `to_json` of an `AmbiguousTarget` with two candidates has `error.code` `ambiguous_target` and an `error.candidates` array of length 2.
    - Unit tests for `resolve` in `capture/mod.rs` against a fixed list of three windows (one minimized) and two monitors at `0,0 1920x1080` and `1920,0 1280x720`: `WindowId` hit and miss; `WindowTitle` with different letter case; a title matching two windows gives `AmbiguousTarget` with both as candidates; `Process` match; the minimized window gives `TargetMinimized`; `Region(2000,100,640,480)` gives `Monitor { index: 1, crop: Some(Rect{80,100,640,480}) }` and origin `(2000, 100)`; `Region(1800,0,400,400)` gives `RegionOutOfBounds`; `Monitor(5)` gives `TargetNotFound`.
    - `tests/desktop.rs` (created here): `recdr monitors` exits 0, prints a non-empty `monitors` array whose first entry has `index` 0; `recdr windows` exits 0 and prints a `windows` array.
  - Verify: `cargo test --lib capture` and `cargo test --test desktop -- --ignored`. Manual check: `cargo run -- windows` lists this terminal's window with its title.
  - Dependencies: Task 1
  - Files: `Cargo.toml`, `Cargo.lock`, `src/main.rs`, `src/lib.rs`, `src/error.rs`, `src/model.rs`, `src/platform.rs`, `src/capture/mod.rs`, `src/capture/windows.rs`, `src/control.rs`, `src/cli.rs`, `tests/desktop.rs`
  - Scope: L by file count; four of the files get one or two lines

- [x] **Task 3: The recording directory**
  - Description: Everything that reads or writes a recording directory, tested without any capture.
  - Steps:
    - Add `image` to `Cargo.toml`; add `From<image::ImageError>` in `src/error.rs`.
    - `src/model.rs`: every remaining type in the contracts. `RecordingResult.contact_sheet` and `.diff` are `Option<PathBuf>` until Task 7.
    - `src/time.rs`: `now_unix_ms`, `utc_stamp`, `parse_time`.
    - `src/store.rs`: `RecordingDir`, `FrameWriter`, `letterbox`, `frame_at`.
  - Acceptance (unit tests, each using a fresh directory under `std::env::temp_dir()`):
    - `time`: `utc_stamp(1795012502000)` is `20261118-143502`. `parse_time` gives 500 for `500ms`, 10000 for `10s`, 300000 for `5m`, and `Usage` for `10`, `1h`, `-1s` and the empty string.
    - `RecordingDir::create` twice with the same `now_unix_ms` gives two different ids, both starting with the stamp; `<out>/.gitignore` contains `*`; `frames/` exists.
    - `open` on an unknown id is `RecordingNotFound`. `list` on a missing `out` is empty.
    - `write_state` then `read_state` round-trips, and no `.tmp` file is left behind. `read_result` is `None` before `write_result`.
    - `stop_requested` is false, then true after `request_stop`. Two `append_mark` calls give two entries from `read_marks`.
    - `FrameWriter`: pushing the same 4x4 frame twice returns `true` then `false`, and one PNG exists; a different frame returns `true`; `bytes()` equals the sum of the two file sizes; `finish()` has two entries with the given `t_ms`.
    - Pushing an 8x4 frame into a 4x4 writer stores a 4x4 PNG.
    - `letterbox` of a 200x100 red frame into 100x100 gives a 100x100 frame whose centre pixel is red and whose top-left pixel is black.
    - `frame_at` returns the first entry before the first time, the entry at an exact time, the last entry past the end, and `None` for an empty list.
  - Verify: `cargo test --lib store` and `cargo test --lib time`.
  - Dependencies: Task 2
  - Files: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/error.rs`, `src/model.rs`, `src/time.rs`, `src/store.rs`
  - Scope: M

- [x] **Task 4: `recdr record --duration` with the synthetic source**
  - Description: The blocking recording path, from command line to PNG frames, `manifest.json` and `result.json`, driven by the synthetic source.
  - Steps:
    - `src/capture/mod.rs`: `CaptureSource`, and `open`, which handles `ResolvedKind::Fake` and returns `CaptureFailed("no capture backend")` for the other kinds until Task 5.
    - `src/capture/fake.rs`: `FakeSource`.
    - `src/request.rs`: everything in the contracts except `parse_selection` (Task 8) and the `JsonSchema` derive (Task 15).
    - `src/recorder.rs`: `run`, `capture`, `finalize`. In this task `capture` skips steps that later tasks add (the click hook in Task 13, the wrapped command in Task 11) and checks every stop reason except `Stopped` and `CommandExited`. `finalize` writes the manifest, the result (`keyframes` empty, `gif`, `mp4`, `contact_sheet`, `diff` all `None`) and the finished state.
    - `src/control.rs`: private `prepare` (without the ffmpeg check, which Task 14 adds) and public `record`.
    - `src/cli.rs`: `TargetFlags`, `RecordFlags`, `RecordCmd` with `--duration` (the trailing command arrives in Task 11), `into_request`, and `Command::Record`.
    - `tests/record.rs`.
  - Acceptance (`tests/record.rs`, with `RECDR_FAKE_SOURCE` as named):
    - `moving`, `record --monitor 0 --duration 1s`: exit 0; the result has `stop_reason` `duration_reached`, `width` 320, `height` 240, `duration_ms` between 1000 and 1500, `frames_stored` between 10 and 17 (the 15 fps limiter is working); `manifest.json` lists that many frames, each file exists, and the first has `t_ms` 0.
    - `static`, `--duration 1s`: `frames_stored` is 1.
    - `closing`, `--duration 5s`: `stop_reason` is `window_closed` and `duration_ms` is under 2000.
    - `moving`, `--duration 5s --max-duration 1s`: exit 2 with `usage`. `record --monitor 0` with no `--duration`: exit 2 with `usage`.
    - Two target flags together, or none: exit 2 with `usage`. `--fps 0`, `--fps 61`, `--format webm`, `--max-size 2`, `--region 1,2,3`: each exits 2 with `usage`. `RECDR_FAKE_SOURCE=nonsense` exits 2 with `usage`.
    - `broken`, `--duration 1s`: exit 1 with `capture_failed`.
    - `moving`, `--duration 1s --out <path of an existing file>`: exit 1 with `io_error`.
    - Unit test in `recorder.rs`: a directory prepared with a `Fake(Moving)` target and `max_size_bytes` 1 finishes with `StopReason::SizeCapReached`. Another with `max_duration_ms` 300 and no duration finishes with `CapReached`.
    - Unit tests in `request.rs`: `parse_size` gives 2000000000 for `2GB` and 1000000 for `1MB`; `parse_region("10,20,640,480")` gives that `Rect` and rejects a zero width; `parse_formats("gif,mp4")` gives both.
  - Verify: `cargo test --test record`, `cargo test --lib recorder`, `cargo test --lib request`.
  - Dependencies: Task 3
  - Files: `src/lib.rs`, `src/capture/mod.rs`, `src/capture/fake.rs`, `src/request.rs`, `src/recorder.rs`, `src/control.rs`, `src/cli.rs`, `tests/record.rs`
  - Scope: L. It is one path and cannot be cut smaller without leaving a task that records nothing.

- [x] **Task 5: WGC backend for monitor, region and window**
  - Description: Real capture. This is the highest-risk task, so it comes before any output format.
  - Steps:
    - `src/capture/windows.rs`: `WgcSource`, its `CaptureSource` impl and its `Drop`.
    - `src/capture/mod.rs`: `open` returns `WgcSource` for `Window` and `Monitor`.
    - `tests/common/mod.rs`: `test_window(title, minimized)` as in the plan's Other decisions, and `wait_for_window(title) -> serde_json::Value`, which calls `recdr windows` every 200 ms for up to 10 s until an entry's `title` equals `title`, and panics after that.
    - `tests/desktop.rs`: the checks below, each `#[ignore]`.
  - Acceptance:
    - `record --monitor 0 --duration 2s`: `width` and `height` equal the `bounds` of index 0 from `recdr monitors`, and `frames_stored` is at least 1.
    - `record --region 0,0,640,480 --duration 2s`: `width` 640, `height` 480, and the first PNG is 640x480.
    - `test_window("recdr-test-<pid>-capture", false)`, `wait_for_window`, then `record --window "<title>" --duration 2s`: exit 0, `frames_stored` at least 1, and `width` and `height` each within 40 pixels of the listed `bounds`. The test kills the child at the end.
    - `record --region 0,0,99999,99999 --duration 1s` exits 1 with `region_out_of_bounds`.
    - `record --window "zzz-no-such-window" --duration 1s` exits 1 with `target_not_found`.
    - Two test windows titled `recdr-test-<pid>-dup-a` and `recdr-test-<pid>-dup-b`: `record --window "recdr-test-<pid>-dup" --duration 1s` exits 1 with `ambiguous_target`, and `error.candidates` on stderr has 2 entries.
  - Verify: `cargo test --test desktop -- --ignored`. Manual check: open one of the PNGs and see the desktop in it.
  - Dependencies: Task 4
  - Files: `src/capture/mod.rs`, `src/capture/windows.rs`, `tests/common/mod.rs`, `tests/desktop.rs`
  - Scope: M

### Checkpoint A

- [x] `cargo test` and `cargo test --test desktop -- --ignored` pass.
- [x] A real monitor recording leaves PNG frames and a manifest under `.recdr/`.
- [x] Review with the owner before Phase 2. If Task 5 needed the fallback named in the plan's risk table, say so.

## Phase 2: outputs

- [x] **Task 6: GIF encoding**
  - Description: A finished recording gets `recording.gif`.
  - Steps:
    - Add `gif` to `Cargo.toml`.
    - `src/encode/mod.rs`: `pub mod gif;`, `RING_MS`, `OutFrame`, `timeline`, `render`. `render` returns the loaded frame; Task 13 adds the ring drawing.
    - `src/encode/gif.rs`: `encode`.
    - `src/recorder.rs`: `finalize` calls `encode::gif::encode` when `formats` contains `Gif` and sets the result's `gif`.
  - Acceptance:
    - `tests/record.rs`, `moving`, `--duration 3s`: the result's `gif` exists; decoding it with the `gif` crate gives 320x240 and delays that add up to between 2.5 and 3.5 seconds.
    - `moving`, `--duration 1s --format mp4` is not tested here (Task 14).
    - Unit tests for `timeline` in `encode/mod.rs`: frames at 0, 100 and 250 ms with `duration_ms` 400 and no clicks give durations 100, 150 and 150 and no rings; one frame at 0 ms with `duration_ms` 1000 and a click at 200 ms give three pieces of 200, 400 and 400 ms where only the middle one has a ring.
  - Verify: `cargo test --test record` and `cargo test --lib encode`. Manual check: open the GIF and see the square move.
  - Dependencies: Task 4
  - Files: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/encode/mod.rs`, `src/encode/gif.rs`, `src/recorder.rs`, `tests/record.rs`
  - Scope: M

- [x] **Task 7: Keyframes, `diff.json` and the contact sheet**
  - Description: The agent view that is written when a recording finishes.
  - Steps:
    - Add `font8x8` to `Cargo.toml`.
    - `src/draw.rs`: `text`. (`ring` arrives in Task 13.)
    - `src/analyze/mod.rs`: `pub mod diff; pub mod sheet; pub mod select;` and `run`.
    - `src/analyze/diff.rs`: the two constants and `diff`.
    - `src/analyze/sheet.rs`: `MAX_TILES`, `pick_tiles`, `contact_sheet` with times and mark labels. Click rings on tiles arrive in Task 13.
    - `src/analyze/select.rs`: `select` and `frame_refs`.
    - `src/model.rs`: `RecordingResult.contact_sheet` and `.diff` become `PathBuf`.
    - `src/recorder.rs`: `finalize` calls `analyze::run` and `contact_sheet` before encoding and fills `keyframes`, `contact_sheet` and `diff`.
  - Acceptance:
    - Unit tests in `analyze/diff.rs`: equal buffers give `(0.0, None)`; a 100x100 black frame against one with a white 10x10 square at 20,30 gives `(0.01, Some(Rect{20,30,10,10}))`; a channel difference of 16 is not a change and 17 is.
    - Unit tests in `analyze/sheet.rs`: `pick_tiles` of 5 items returns all 5; of 30 items returns 12 that start with the first and end with the last. A sheet built from a two-frame manifest with a mark at the first frame's time differs, inside the first tile's strip, from one built without the mark.
    - Unit test in `analyze/select.rs`: for frames at 0, 400 and 900 ms and `duration_ms` 1000, `Every(500)` returns the frames at 0, 400 and 900; `At([450])` returns the frame at 400.
    - Unit test in `draw.rs`: `text` of `"A"` at scale 2 on a black 32x32 image sets at least one pixel white and none outside the 16x16 cell.
    - `tests/record.rs`, `moving`, `--duration 2s`: `diff.json` has one entry per stored frame; the result's `keyframes` has at least 2 entries and each `path` exists; `contact_sheet.png` exists.
    - `static`, `--duration 1s`: `keyframes` has exactly 1 entry.
  - Verify: `cargo test --lib analyze`, `cargo test --lib draw`, `cargo test --test record`. Manual check: open `contact_sheet.png` and read the time under each tile.
  - Dependencies: Task 6
  - Files: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/draw.rs`, `src/analyze/mod.rs`, `src/analyze/diff.rs`, `src/analyze/sheet.rs`, `src/analyze/select.rs`, `src/model.rs`, `src/recorder.rs`, `tests/record.rs`
  - Scope: L by file count; the four `analyze` files and `draw.rs` are small and are one concern each

- [x] **Task 8: `recdr frames`**
  - Description: Return stored frames by time, by interval, or the keyframes.
  - Steps:
    - `src/request.rs`: `parse_selection` (Task 4 leaves it out because `FrameSelection` has no user until now).
    - `src/control.rs`: `frames`.
    - `src/cli.rs`: `FramesCmd` and `Command::Frames` as in the contracts.
    - `tests/frames.rs`.
  - Acceptance (`tests/frames.rs`, on a 2 s `moving` recording):
    - `--at 1s` prints one frame whose `t_ms` is at most 1000 and whose file exists.
    - `--at 500ms,1s` prints two frames.
    - `--every 500ms` prints 5 frames.
    - `--keyframes` prints the same count as the result's `keyframes`.
    - `--at 1s --max-width 160` prints a path under `samples/` whose image is 160 pixels wide.
    - `--at 1s --keyframes`, no selection flag at all, and `--every 0ms` each exit 2 with `usage`. An unknown id exits 1 with `recording_not_found`.
    - Unit tests in `request.rs` for `parse_selection`: `(["500ms","1s"], None, false)` gives `At([500, 1000])`; `([], Some("250ms"), false)` gives `Every(250)`; `([], None, true)` gives `Keyframes`; none set, two set, and `Every` of 0 each give `Usage`.
  - Verify: `cargo test --test frames`.
  - Dependencies: Task 7
  - Files: `src/request.rs`, `src/control.rs`, `src/cli.rs`, `tests/frames.rs`
  - Scope: S

### Checkpoint B

- [x] `cargo test` passes.
- [x] On the desktop, `cargo run -- record --monitor 0 --duration 3s` gives a playable GIF and a contact sheet (success criteria 2, 8, 9, 11).
- [x] Review with the owner before Phase 3.

## Phase 3: control

- [x] **Task 9: `recdr start`, `stop` and `status`**
  - Description: A recording that outlives the command that started it.
  - Steps:
    - `src/platform.rs`: `process_alive`, `spawn_detached`.
    - `src/recorder.rs`: the `Stopped` check in the loop.
    - `src/control.rs`: `derived_status`, `await_result`, `start`, `wait`, `stop`, `status`, `run_recorder`.
    - `src/cli.rs`: `StartCmd`, `IdCmd`, `StatusCmd`, and the `Start`, `Stop`, `Status` and hidden `Recorder` commands.
    - `tests/control.rs`.
  - Acceptance (`tests/control.rs`, `moving` unless named):
    - `start --monitor 0`, run with `Command::output()` so the test reads its stdout and stderr to end of file, returns in under 2 seconds with exit 0 and prints `id`, `dir`, `width` 320, `height` 240. (This fails if the recorder inherited the pipes, which is what the handle-flag step in `spawn_detached` prevents.) `status <id>` shows `recording`. `stop <id>`, run as a separate process, prints a result with `stop_reason` `stopped`; then `status <id>` shows `finished`, and within 2 seconds the output of `tasklist /FI "PID eq <pid>" /NH` no longer contains the `pid` from `state.json`.
    - `broken`: `start --monitor 0` exits 1 with `recorder_start_failed`, and the message contains `the synthetic source is broken`.
    - `stop` a second time prints the same result.
    - `start --max-duration 1s`, wait 3 s: `status` shows `finished`, and `stop` prints `stop_reason` `cap_reached`.
    - `start`, kill the `pid` from `state.json` with `taskkill /F /PID`, then `stop`: exit 1 with `recorder_crashed`, and `status` shows `crashed`.
    - `stop no-such-id`: exit 1 with `recording_not_found`.
    - `closing`: `start`, wait 2 s, `stop` prints `stop_reason` `window_closed`.
    - `status` with no id after two recordings lists both.
    - Desktop (`tests/desktop.rs`): `start --monitor 0`, wait 1 s, `stop`: `frames_stored` is at least 1. This proves WGC works in a detached process.
  - Verify: `cargo test --test control` and `cargo test --test desktop -- --ignored`.
  - Dependencies: Task 7
  - Files: `src/platform.rs`, `src/recorder.rs`, `src/control.rs`, `src/cli.rs`, `tests/control.rs`, `tests/desktop.rs`
  - Scope: M

- [x] **Task 10: `recdr mark`**
  - Description: Labelled markers during a recording, shown on the contact sheet.
  - Steps:
    - `src/control.rs`: `mark`.
    - `src/cli.rs`: `MarkCmd` and `Command::Mark`.
    - `tests/control.rs`: the checks below. Marks already reach the manifest through `recorder::capture` step 6, written in Task 4; this task is the first that can test it.
  - Acceptance (`tests/control.rs`, `moving`):
    - `start`, sleep 500 ms, note the wall-clock time `T` in Unix milliseconds, run `mark <id> "clicked submit"`, `stop`. The result's `marks` has one entry with that label. With `S` = `started_at_unix_ms` from `manifest.json`, `mark.t_ms` is between `T - S` and `T - S + 1000` (the upper bound allows for the time the `mark` process takes to start).
    - `mark` on the finished recording exits 1 with `recording_not_active`. `mark no-such-id x` exits 1 with `recording_not_found`.
  - Verify: `cargo test --test control`. Manual check: the label is readable on `contact_sheet.png`.
  - Dependencies: Task 9
  - Files: `src/control.rs`, `src/cli.rs`, `tests/control.rs`
  - Scope: S

- [x] **Task 11: Wrap a command**
  - Description: `recdr record <target> -- <command...>` records for as long as the command runs.
  - Steps:
    - `src/cli.rs`: `RecordCmd.command` with `#[arg(last = true)]`; an empty vector becomes `None` in the request.
    - `src/recorder.rs`: step 4 (start the child), the `CommandExited` check, and the `taskkill /T /F` of the child's process tree in step 6.
  - Acceptance (`tests/record.rs`, `moving`):
    - `record --monitor 0 -- cmd /c "echo hi & ping -n 3 127.0.0.1 >nul"`: exit 0; stdout parses as exactly one JSON object with `stop_reason` `command_exited` and `command_exit_code` 0; stderr contains `hi`; `duration_ms` is between 1500 and 4000.
    - `record --monitor 0 -- cmd /c "exit 3"`: recdr exits 0 and `command_exit_code` is 3.
    - `record --monitor 0 --max-duration 1s -- ping -n 30 127.0.0.1`: run with `Command::output()`, it returns within 5 seconds; `stop_reason` is `cap_reached` and the result has no `command_exit_code`. (`output()` returns only when `ping.exe` no longer holds the stderr pipe, so this proves the whole tree was killed.)
    - `record --monitor 0 --duration 1s -- cmd /c echo` exits 2 with `usage`.
  - Verify: `cargo test --test record`.
  - Dependencies: Task 7
  - Files: `src/cli.rs`, `src/recorder.rs`, `tests/record.rs`
  - Scope: S

### Checkpoint C

- [x] `cargo test` passes.
- [x] On the desktop: `start --window-id`, `mark`, `stop` on a real browser window gives a GIF and a contact sheet that shows the label (user stories 1 and 5; success criteria 3, 4, 5, 10).
- [x] Review with the owner before Phase 4.

## Phase 4: the rest of the surface

- [x] **Task 12: Window close and minimize on the real desktop**
  - Description: Prove on a real window the behavior that Tasks 4 and 9 proved with the synthetic source.
  - Steps:
    - `tests/desktop.rs`: the two checks below. No source change is expected; if one is needed it is in `src/capture/windows.rs`.
  - Acceptance:
    - `test_window("recdr-test-<pid>-close", false)`, `wait_for_window`, `start --window "<title>"`, sleep 1 s, `Child::kill` on the window process, sleep 2 s, `stop`: `stop_reason` is `window_closed` and the GIF decodes with at least one frame.
    - `test_window("recdr-test-<pid>-min", true)`, `wait_for_window`, `record --window "<title>" --duration 1s`: exit 1 with `target_minimized`. The test kills the child at the end.
  - Verify: `cargo test --test desktop -- --ignored`. Manual check: resize a window during a recording and see every GIF frame at the starting size.
  - Dependencies: Task 9
  - Files: `tests/desktop.rs`, `src/capture/windows.rs`
  - Scope: S

- [x] **Task 13: Cursor flag, click hook and click rings**
  - Description: Click capture, and rings in the GIF and on the contact sheet. `--no-cursor` and `--no-click-highlight` already parse and reach `RecordOptions` since Task 4.
  - Steps:
    - `src/input.rs`: `ClickHook`.
    - `src/draw.rs`: `ring`.
    - `src/recorder.rs`: start the hook in step 4 and finish it in step 6.
    - `src/encode/mod.rs`: `render` draws the rings of an `OutFrame`.
    - `src/analyze/sheet.rs`: rings on tiles.
  - Acceptance:
    - Unit test in `draw.rs`: `ring` on a black 64x64 image at 32,32 with radius 14 leaves the centre pixel black and makes the pixel at 45,32 red.
    - Unit test in `encode/mod.rs`: `render` of an `OutFrame` with one ring differs from the stored frame, and the PNG file's bytes on disk are unchanged.
    - Unit test in `encode/gif.rs`: a one-frame manifest with a click at 200 ms and `duration_ms` 1000 encodes to a GIF with 3 frames; without the click, 1 frame.
    - Unit test in `input.rs`: `ClickHook::start()` twice without `finish` returns `CaptureFailed` the second time, and succeeds again after `finish`. Marked `#[ignore]` because it needs a desktop session.
    - Manual check on the desktop (success criterion 12): record a region, click inside it once, stop. `manifest.json` has one click with coordinates inside the region; the GIF shows a ring; no stored PNG shows one. Record with `--no-click-highlight`, click, stop: `clicks` is empty. Record with `--no-cursor`: no pointer in the frames.
  - Verify: `cargo test --lib`, `cargo test --lib input -- --ignored`, and the manual check.
  - Dependencies: Task 7
  - Files: `src/lib.rs`, `src/input.rs`, `src/draw.rs`, `src/recorder.rs`, `src/encode/mod.rs`, `src/encode/gif.rs`, `src/analyze/sheet.rs`
  - Scope: M

- [x] **Task 14: MP4 through ffmpeg**
  - Description: `--format mp4`.
  - Steps:
    - `src/encode/mod.rs`: `pub mod ffmpeg;`.
    - `src/encode/ffmpeg.rs`: `available`, `encode`.
    - `src/control.rs`: the ffmpeg check at the top of `prepare`.
    - `src/recorder.rs`: `finalize` calls `encode::ffmpeg::encode` when `formats` contains `Mp4` and sets the result's `mp4`.
  - Acceptance (`tests/record.rs`, `moving`):
    - `--duration 2s --format gif,mp4` with ffmpeg on `PATH`: both files exist, and `ffprobe -v error -show_entries stream=codec_name:format=duration -of json <mp4>` reports `h264` and a duration within 0.5 s of `duration_ms`. When `ffmpeg -version` or `ffprobe -version` fails, the test prints `skipped: ffmpeg or ffprobe not on PATH` and returns.
    - Unit test in `encode/ffmpeg.rs` (same skip rule): `encode` on a manifest with `width` 0 and `height` 0 returns `EncodeFailed`.
    - With the child's `PATH` set to an empty temp directory: `--duration 1s --format mp4` exits 1 with `ffmpeg_not_found`, and the `--out` directory has no sub-directory.
  - Verify: `cargo test --test record`. Manual check: play the MP4.
  - Dependencies: Task 7
  - Files: `src/encode/mod.rs`, `src/encode/ffmpeg.rs`, `src/control.rs`, `src/recorder.rs`, `tests/record.rs`
  - Scope: S

- [x] **Task 15: MCP server**
  - Description: `recdr mcp` with the eight tools.
  - Steps:
    - Add `rmcp`, `tokio`, `schemars` and `base64` to `Cargo.toml`.
    - `src/request.rs`: add `schemars::JsonSchema` to `RecordRequest`'s derives, with `#[schemars(skip)]` on `command`.
    - `src/mcp.rs`: `serve`, `RecdrServer`, the four parameter structs and the eight tools with the descriptions in the contracts' table.
    - `src/main.rs`: pass `recdr::mcp::serve`; delete `no_mcp`.
    - `tests/cli.rs`: delete the `recdr mcp` exits-2 test from Task 1.
    - `tests/mcp.rs`.
  - Acceptance (`tests/mcp.rs`, `moving`): the test starts `recdr mcp`, writes one JSON-RPC message per line to its stdin and reads replies from stdout.
    - After `initialize` and `notifications/initialized`, `tools/list` returns exactly the eight names in the spec's MCP tools table.
    - `tools/call` of `record` with `{"monitor":0,"duration":"1s","out":"<temp dir>"}` returns two content blocks: text that parses as a result with `stop_reason` `duration_reached`, and an image with `mimeType` `image/png`.
    - `tools/call` of `get_frames` with `{"id":"<that id>","every":"10ms","out":"<temp dir>"}` returns `isError` true and text containing `too_many_frames`.
    - `tools/call` of `get_frames` with `"at":["500ms"]` returns a text block and one image block.
    - `tools/call` of `record` without `duration` returns `isError` true and text containing `usage`.
  - Verify: `cargo test --test mcp` and `cargo test --test layers`. Manual check (success criterion 14): `claude mcp add recdr -- <absolute path to target/release/recdr.exe> mcp`, then ask Claude Code to record the primary monitor for 3 seconds and describe the contact sheet.
  - Dependencies: Tasks 8, 9, 10
  - Files: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/request.rs`, `src/mcp.rs`, `src/main.rs`, `tests/cli.rs`, `tests/mcp.rs`
  - Scope: M

- [x] **Task 16: README and the acceptance run**
  - Description: Document the tool and check all 17 success criteria on the real desktop.
  - Steps:
    - `README.md`: what recdr is; build with `cargo build --release`; the commands and options from the spec; the MCP registration line; the limits from the spec's Risks table (yellow border on Windows 10, no clicks from elevated windows, protected content is black, recordings may hold secrets and are never uploaded).
    - `tests/desktop.rs`: an `#[ignore]` test named `criterion_15_dropped_frames` that runs `record --monitor 0 --fps 15 --duration 30s` and fails when `dropped_frames` is more than 5% of `frames_stored + dropped_frames`. The tester plays a full-screen video on that monitor while it runs.
    - `tasks/1-recdr/acceptance.md`: one line per success criterion, 1 to 17, with `pass` or `fail` and the test name or manual step that showed it.
  - Acceptance:
    - `cargo test`, `cargo test --test desktop -- --ignored`, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
    - `acceptance.md` has 17 lines. Any `fail` line, including `criterion_15_dropped_frames`, stops the task: report it to the owner with the measured numbers. This task does not edit source files.
  - Verify: the commands above and `acceptance.md`.
  - Dependencies: Tasks 11, 12, 13, 14, 15
  - Files: `README.md`, `tests/desktop.rs`, `tasks/1-recdr/acceptance.md`
  - Scope: S

### Checkpoint: complete

- [ ] All 17 success criteria pass on Windows 11.
- [ ] The acceptance checklist on https://github.com/dzikrisyairozi/recdr/issues/1 is ticked.
- [ ] Ready for review.
