# Tasks: macos-capture

Plan: [plan.md](plan.md). Spec: [spec/4-macos-capture/spec.md](../../spec/4-macos-capture/spec.md). "Contracts" means the Module contracts section of the plan. Every signature, constant, message and rule named below is defined there, and a task implements it exactly as written. "Criterion N" means success criterion N of the spec.

## Before Task 1

1. `cdp-capture` is merged, or the work branch is cut from its branch. Cut `feat/4-macos-capture` and commit `spec/4-macos-capture/` and `tasks/4-macos-capture/` as one commit.
2. Check what this plan took from the cdp plan. Each must hold; if one does not, edit the plan first.
   - `src/capture/windows.rs` has a private `to_event(click: RawClick, origin: (i32, i32), size: (u32, u32)) -> Option<CaptureEvent>` with a unit test, called from `next_event` once `self.size` is `Some`.
   - `src/capture/mod.rs` has two `cfg` definitions each of `list_windows`, `list_monitors` and `desktop_source`.
   - `CaptureEvent::Click { unix_ms, x, y, button }` exists in `src/model.rs`.
   - `src/error.rs` has `UnsupportedPlatform` with the message quoted in the plan. Search `src/` and `tests/` for `need Windows` and note every test that quotes it; Task 5 changes them.
   - `tests/browser.rs` defines `Browser`, `Client`, `serve`, `ANIMATION`, `STILL` and `GREEN`. `tests/common/mod.rs` has `Server` with `start(fake: Option<&str>)`, and `wait_for_window(title)`, which runs `recdr windows` and returns the entry with exactly that title.
   - `tests/desktop.rs` has `#![cfg(windows)]`, the helpers `record`, `ok`, `error` and `title`, and no other item at the top that only Windows uses besides the import of `test_window`.
   - `tests/tabs.rs` has `#[cfg(not(windows))] mod unsupported`.
   - `tests/layers.rs` has six tests, and the variant test in `src/error.rs` counts 17.
   - `.github/workflows/ci.yml` runs on pull requests.
3. Ask the owner whether a Mac running macOS 14 or later is available (plan open question 1). Phase 1 does not need the answer.

Every task ends with these three commands passing, then one commit that stages the task's files by path:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

A desktop test is a test in `tests/desktop.rs`, marked `#[ignore]`, run with `cargo test --test desktop -- --ignored --test-threads=1`.

## Phase 1: shared code, on any machine

- [x] **Task 1: the click filter moves to `capture/mod.rs`**
  - Description: One filter for both desktop sources. Windows passes a scale factor of 1 and behaves as before.
  - Steps:
    - `src/capture/mod.rs`: `Press` and `to_event`, with the unit test that was in `src/capture/windows.rs`, rewritten for the new arguments, and two new cases at scale 2.
    - `src/capture/windows.rs`: remove `to_event` and its test; call `super::to_event` as in Contracts.
  - Acceptance:
    - At scale 1: a press at (110, 220) with the corner at (100, 200) and a size of 50 by 50 is a click at (10, 20); a press at (99, 220), at (150, 220) and at (110, 250) is `None`.
    - At scale 2: a press at (110.5, 220.0) with the corner at (100.0, 200.0) and a size of 100 by 100 is a click at (21, 40); a press at (150.0, 220.0) is `None`.
    - `cargo test --test desktop -- --ignored` passes on Windows.
  - Verify: `cargo test --lib capture`.
  - Dependencies: none
  - Files: `src/capture/mod.rs`, `src/capture/windows.rs`
  - Scope: S

- [x] **Task 2: points and pixels**
  - Description: The arithmetic of the spec's "Positions and sizes", as functions on plain numbers.
  - Steps:
    - `src/capture/mod.rs`: `Display`, `display_bounds`, `to_pixels`, `to_points`, with unit tests.
  - Acceptance (criterion 16):
    - One display of 1512 by 982 points at scale 2 has the `bounds` (0, 0, 3024, 1964).
    - A 2x display of 1512 by 982 at (0, 0) with a 1x display of 1920 by 1080 at (1512, 0): the `bounds` are (0, 0, 3024, 1964) and (3024, 0, 1920, 1080).
    - The same two displays the other way round, 1x at (0, 0) and 2x at (1920, 0): the `bounds` are (0, 0, 1920, 1080) and (3840, 0, 3024, 1964). They do not overlap.
    - In both layouts, for a rectangle of 100 by 50 points placed 10 points inside each display: `to_pixels` gives a rectangle inside that display's `bounds`; subtracting the `bounds` origin and passing the result to `to_points` with that display's scale gives back (10, 10, 100, 50).
    - A rectangle whose center is on no display is placed on the first display. One whose center is exactly on the edge two displays share is placed on the display that starts there.
    - The tests have no `cfg`, so the three CI jobs run them at Checkpoint A.
  - Verify: `cargo test --lib capture`.
  - Dependencies: none
  - Files: `src/capture/mod.rs`
  - Scope: S

- [x] **Task 3: the browser test helpers move to `tests/common`**
  - Description: A move with no change in behavior, so the desktop tests can start a Chrome window and talk to it.
  - Steps:
    - `tests/common/mod.rs`: `Browser`, `Client`, `serve` and the three page constants, each `pub`.
    - `tests/browser.rs`: remove them and import them from `common`.
  - Acceptance:
    - `cargo test --test browser -- --ignored` passes on Windows, with the same number of tests as before.
    - No test file other than `tests/browser.rs` changes its tests.
  - Verify: `cargo test --test browser -- --ignored`.
  - Dependencies: none
  - Files: `tests/common/mod.rs`, `tests/browser.rs`
  - Scope: S

### Checkpoint A: Windows is unchanged (needs a pull request)
- [x] The three commands, the desktop tests and the browser tests pass on Windows. (`evidence/checkpoint-A-test.log`: `criterion_15_dropped_frames` failed once in a 73 s run, then passed on the unchanged tree with 12 stored and 0 dropped; it also passed at Task 1. Browser tests: `evidence/T3-test.log`.)
- [ ] By hand, by the owner: a click during `recdr record --monitor 0 --duration 5s` on Windows is in `manifest.json` and is a ring in the GIF (criterion 19).
- [ ] The owner pushes the branch and opens a draft pull request; the build stops and asks, and asks whether it may push after each later task (plan decision 13). The three CI jobs are green.

## Phase 2: listing, on a Mac

- [x] **Task 4: try ScreenCaptureKit and the event tap, and write the results into this plan**
  - Description: A try-out program on the branch `spike/4-sck`, which is never merged (plan decision 12). It makes each call the two new files will make, once, on the owner's Mac, and answers the spec's "Not checked" list.
  - Steps, on `spike/4-sck`, in a cargo project in `spike/sck/` with `screencapturekit` 11.1 (feature `macos_14_0`), `objc2-core-graphics` 0.3 and `objc2-core-foundation` 0.3. Each numbered step is a subcommand of the program:
    1. `list`: print the displays and the windows; for each display and for one window, the `content_rect` and `point_pixel_scale` of its filter. Time the first `cargo build` and note it (the Swift package).
    2. `frames <display|region|window>`: record ten frames and print each frame's size. For the window, open a Chrome `--app` window of 800 by 600 on a local page whose script sets `document.title` to `recdr-spike-title`, and compare the frame size with 800 by 600 times the scale factor (spec item 5). Print the title ScreenCaptureKit reports for that window; it must be `recdr-spike-title` (plan decision 11).
    3. `resize`: record a window while it is made smaller by hand; save a frame before and after, and note whether the picture is padded or stretched, which value of a frame gives its real size, and the call that changes a running stream's size (spec item 8, decision 6).
    4. `away`: record a window, then minimize it, then move it to another Space, then close it; print what the stream delivers each time (spec item 3).
    5. `tap`: create the tap of Contracts, wait for three presses, drop it. Run it from the terminal with the Input Monitoring permission on and off (spec item 2). Note whether the run loop type is `Send`, and that the drop returns.
    6. `detached`: the program starts itself again with `std::process::Command`, with `libc::setsid()` called in `pre_exec` and its output sent to a file, as `platform::spawn_detached` starts the recorder (the try-out project adds the `libc` crate for this), and exits. The child runs `frames display` and `tap`. Note which program macOS names in its two permission dialogs (spec item 6).
    7. Run `frames display` ten times in a row from a terminal on macOS 15 and note how often the alert of the spec's fact 6 appears (spec item 4).
    8. Run `system_profiler SPDisplaysDataType -json` and note the fields that hold a display's size in pixels.
    9. CI: on the branch, add to `.github/workflows/ci.yml` a step with `if: runner.os == 'macOS'` and `run: cargo run --manifest-path spike/sck/Cargo.toml -- frames display`. The owner opens a draft pull request from the branch and closes it unmerged afterwards (spec item 1).
    10. Build the program on macOS 13 if such a machine is at hand (spec item 7); otherwise the item stays open and the README names macOS 14.
  - Acceptance:
    - The plan's "What was measured" has an answer, or "could not be tried" with the reason, for each of the eight items of the spec's "Not checked" list, and the build time.
    - The plan's `src/capture/macos.rs` and `src/capture/tap.rs` sections name the Rust calls that worked, with the cargo features the `objc2` crates need.
    - Each row of the plan's table "What Task 4 fills in" is filled in by its rule.
    - Decision 6 is confirmed, or the build stops as that decision says.
    - Anything that contradicts the spec stops the build: the spec is changed first, and the owner is asked.
  - Verify: the program's output for each step, pasted into `acceptance.md`.
  - Dependencies: Checkpoint A, and a Mac
  - Files: `tasks/4-macos-capture/plan.md`, `tasks/4-macos-capture/acceptance.md` (on the work branch; the program is on `spike/4-sck` only)
  - Scope: M

- [ ] **Task 5: the permission check and `recdr monitors`**
  - Description: The first macOS code in the tree: recdr builds with ScreenCaptureKit, asks for the permission, and lists displays.
  - Steps:
    - `Cargo.toml`, `Cargo.lock`: `screencapturekit` and `objc2-core-graphics`.
    - `src/error.rs`: `PermissionDenied`, and the new message of `UnsupportedPlatform`; the tests found in "Before Task 1" that quote the old message.
    - `src/capture/macos.rs`: `permitted`, `displays`, `list_monitors`, and `list_windows` returning `Ok(Vec::new())` until Task 6.
    - `src/capture/mod.rs`: `mod macos;` and the three definitions of `list_windows`, `list_monitors` and `desktop_source`, the macOS `desktop_source` with the body Contracts gives for Tasks 5 and 6.
    - `tests/layers.rs`: the two rule 2 lines for `screencapturekit` and `objc2_core_graphics`, and `src/capture/macos.rs` in rule 3.
    - `tests/tabs.rs`: the attribute on `mod unsupported`.
    - `tests/desktop.rs`: the two file attributes, the two modules of Contracts with the existing tests moved into `on_windows`, and `monitors_have_one_primary_and_pixel_sizes` in `on_macos`, whose only import in this task is `use super::*;`.
  - Acceptance:
    - On the Mac, `recdr monitors` prints one entry per display, exactly one with `primary` true, and the desktop test passes (criterion 1).
    - With the Screen Recording permission off, `recdr monitors` exits 1 with `permission_denied` and the message of Contracts (part of criterion 11, by hand).
    - On Linux `recdr windows` still exits 1 with `unsupported_platform` (criterion 18): the test in `tests/tabs.rs` runs there and passes, and is not compiled on macOS.
    - The Windows build and its tests are unchanged; `cargo clippy --all-targets -- -D warnings` reports nothing in `tests/desktop.rs` on Windows or on macOS.
  - Verify: `cargo test` and the desktop test on the Mac; the three CI jobs after the push (plan decision 13).
  - Dependencies: Task 4
  - Files: `Cargo.toml`, `Cargo.lock`, `src/error.rs`, `src/capture/macos.rs`, `src/capture/mod.rs`, `tests/layers.rs`, `tests/tabs.rs`, `tests/desktop.rs`
  - Scope: M

- [ ] **Task 6: `recdr windows` and the test window**
  - Description: Windows are listed in pixels, and the desktop tests get a window they control.
  - Steps:
    - `src/capture/macos.rs`: `listed` with a unit test, and `list_windows`.
    - `tests/common/mod.rs`: `Browser::app`, `launch`, `Client::minimize`, `Client::window_size`, and the script line in the three pages.
    - `tests/desktop.rs`, `on_macos`: the import from `common`, and `windows_lists_the_test_window`, `window_and_process_find_it_and_two_are_ambiguous` and `minimized_window_is_listed_and_refused`. In this task the second test asserts only the two errors; Task 8 adds its recording half.
  - Acceptance:
    - `listed(0, "anim", true)` is true; `listed(25, "anim", true)`, `listed(0, "", true)` and `listed(0, "anim", false)` are false.
    - `windows_lists_the_test_window`: the entry `wait_for_window` returns has the test's title, `minimized` false, a `process` that is not empty, and a `pid` for which `kill -0` exits 0 (criterion 2).
    - `window_and_process_find_it_and_two_are_ambiguous`: with the two windows of Contracts open, `--window <title("two")>` fails with `ambiguous_target` whose candidates are exactly those two ids, and `--window no-such-window-<pid>` fails with `target_not_found` (criterion 4, the error half).
    - `minimized_window_is_listed_and_refused`: after `Client::minimize` the window is listed with `minimized` true, and `recdr record --window-id <id> --duration 1s` fails with `target_minimized` (criterion 9).
    - `cargo test --test browser -- --ignored` still passes: a page opened without a fragment keeps its title.
  - Verify: `cargo test --lib capture`, the three desktop tests on the Mac.
  - Dependencies: Task 5, Task 3
  - Files: `src/capture/macos.rs`, `tests/common/mod.rs`, `tests/desktop.rs`
  - Scope: M

### Checkpoint B: recdr sees the Mac's screen
- [ ] `recdr monitors` and `recdr windows` print the Mac's displays and windows.
- [ ] The three CI jobs are green on the pull request.

## Phase 3: recording

- [ ] **Task 7: record a monitor and a region**
  - Description: The ScreenCaptureKit source for the two targets that need no window.
  - Steps:
    - `src/capture/macos.rs`: `SckSource` with `open` for `ResolvedKind::Monitor` (a window target gets the "not built yet" error of Contracts), the two handlers, `next_event`, `dropped` and `Drop`. The `clicks` argument is named `_clicks` until Task 9.
    - `src/capture/mod.rs`: the macOS `desktop_source` calls `macos::SckSource::open`.
    - `tests/desktop.rs`, `on_macos`: `monitor_recording_has_the_monitor_size` and `region_inside_the_green_page_is_green`.
  - Acceptance:
    - `recdr record --monitor 0 --duration 2s` stores frames whose size equals the monitor's `bounds` size (criterion 6).
    - With a test window showing `/green`, a region of 400 by 300 centered on the window's `bounds` stores frames of exactly 400 by 300, and the center pixel of each has a green value more than twice its red and its blue (criterion 5).
    - By hand: with the permission off, `recdr record --monitor 0 --duration 1s` fails with `permission_denied`.
  - Verify: the two desktop tests on the Mac.
  - Dependencies: Task 6
  - Files: `src/capture/macos.rs`, `src/capture/mod.rs`, `tests/desktop.rs`
  - Scope: M

- [ ] **Task 8: record a window**
  - Description: The window target, with its end and its resize.
  - Steps:
    - `src/capture/macos.rs`: `open` for `ResolvedKind::Window`, and the stop notice as `CaptureEvent::Closed`.
    - `tests/desktop.rs`, `on_macos`: `window_recording_has_the_content_size`, `still_page_stores_few_frames`, `ending_chrome_ends_the_recording`; and in `window_and_process_find_it_and_two_are_ambiguous`, before its two windows are opened, the window titled `title("one")` found and recorded for 1 second with `--window <its title>` and with `--process <its process>`.
  - Acceptance:
    - `window_recording_has_the_content_size`: `recdr record --window-id <id> --duration 3s` exits 0 with a GIF of 2.5 to 3.5 seconds and at least 20 stored frames, whose size is within the allowed difference of `Client::window_size` times `devicePixelRatio` (criterion 3; the oracle is in Contracts).
    - `--window <title>` records the test window. `--process <name>` records it, or fails with `ambiguous_target` whose candidates include its id (criterion 4).
    - `still_page_stores_few_frames`: a test window showing `/still`, recorded for 3 seconds, stores 1 to 3 frames and has a contact sheet (criterion 7).
    - `ending_chrome_ends_the_recording`: `recdr start --window-id <id>`, one second of sleep, the `Browser` is dropped (which ends Chrome), then `recdr status <id>` is polled every 200 ms for at most 10 seconds until it says `finished`. `recdr stop <id>` then returns a result with `stop_reason` `window_closed`, and its GIF decodes (criterion 8).
    - By hand: criterion 15, a window made smaller during a recording.
  - Verify: the desktop tests on the Mac.
  - Dependencies: Task 7
  - Files: `src/capture/macos.rs`, `tests/desktop.rs`
  - Scope: M

- [ ] **Task 9: clicks**
  - Description: The event tap, owned by the source.
  - Steps:
    - `Cargo.toml`, `Cargo.lock`: `objc2-core-foundation`.
    - `src/capture/tap.rs`: all of Contracts.
    - `src/capture/mod.rs`: `mod tap;`.
    - `src/capture/macos.rs`: `clicks` starts the tap; `next_event` drains it first.
    - `tests/layers.rs`: the `objc2_core_foundation` line of rule 2, and `src/capture/tap.rs` in rule 3.
  - Acceptance:
    - The two unit tests of `tap.rs` pass on macOS (criterion 17).
    - By hand: criterion 12 (a click is in the manifest, in pixels, and is a ring in the GIF) and criterion 13 (with the permission off: exit 0, the warning on stderr, an empty `clicks` list, also when macOS would still have created the tap; with `--no-click-highlight`: no dialog).
    - By hand: a 30 second recording with a click near its start and one near its end has both in the manifest (the tap is still on at the end).
  - Verify: `cargo test --lib tap` on the Mac, `cargo test --test layers`.
  - Dependencies: Task 8
  - Files: `Cargo.toml`, `Cargo.lock`, `src/capture/tap.rs`, `src/capture/mod.rs`, `src/capture/macos.rs`, `tests/layers.rs`
  - Scope: M

- [ ] **Task 10: a detached recording and the MCP tools**
  - Description: No production code is expected: `start`, `mark`, `stop`, `frames` and the MCP server sit above the capture layer. This task proves it on macOS.
  - Steps:
    - `tests/desktop.rs`, `on_macos`: `detached_recording_and_mcp_work_on_a_window`. It runs `start --window-id`, `mark`, `stop` and `frames --keyframes`, then starts the MCP server with `common::Server::start(None)` and calls `list_windows`, `list_monitors` and `record` with `window_id` and a 1 second duration.
  - Acceptance:
    - The four commands succeed, the mark is in the manifest, and `frames` returns at least one path (criterion 10).
    - The three MCP replies parse into the same shapes the Windows tests assert: `windows`, `monitors`, and a result with `contact_sheet` (criterion 10).
    - By hand: criterion 14, the cursor with and without `--no-cursor`.
    - If the detached recorder is refused the screen although the terminal is allowed, the test fails with `permission_denied` in its output. That result goes into `acceptance.md`, and Task 11 writes into the README which program to allow.
  - Verify: the desktop test on the Mac.
  - Dependencies: Task 9
  - Files: `tests/desktop.rs`, `tasks/4-macos-capture/acceptance.md`
  - Scope: S

### Checkpoint C: everything records
- [ ] `cargo test --test desktop -- --ignored --test-threads=1` passes on the Mac.
- [ ] By hand, by the owner: criteria 11 to 15, each with its result in `acceptance.md`.

## Phase 4: the rest

- [ ] **Task 11: CI, README and the acceptance run**
  - Description: The desktop tests run in CI if Task 4 showed they can, the README describes macOS, and every criterion has a recorded result.
  - Steps:
    - `.github/workflows/ci.yml`: the step in Contracts, only if Task 4's CI run delivered frames.
    - `README.md`: desktop targets on macOS 14 and later; the two permissions and which program gets them, for a command and for a detached recording; what `minimized` covers on macOS; positions in pixels with more than one display; the alert of macOS 15; that Xcode's command line tools are needed to build.
    - `tasks/4-macos-capture/acceptance.md`: one line per criterion, 1 to 20.
  - Acceptance:
    - Criterion 20 holds on the three CI jobs.
    - Every criterion in `acceptance.md` has a result or a stated reason it could not be run.
  - Verify: the three commands, and the three CI jobs.
  - Dependencies: Task 10
  - Files: `.github/workflows/ci.yml`, `README.md`, `tasks/4-macos-capture/acceptance.md`
  - Scope: S

### Checkpoint D: complete (needs the pull request)
- [ ] The three CI jobs are green.
- [ ] `acceptance.md` has a result for each of the 20 criteria.
