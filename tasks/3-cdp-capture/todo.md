# Tasks: cdp-capture

Plan: [plan.md](plan.md). Spec: [spec/3-cdp-capture/spec.md](../../spec/3-cdp-capture/spec.md). "Contracts" means the Module contracts section of the plan. Every signature, constant, message and rule named below is defined there, and a task implements it exactly as written. "Criterion N" means success criterion N of the spec.

Before Task 1: cut `feat/3-cdp-capture` from `feat/1-agent-recorder` and commit `spec/3-cdp-capture/`, `spec/1-recdr/capability-map.md` and `tasks/3-cdp-capture/` as one commit.

Every task ends with these three commands passing, then one commit that stages the task's files by path:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

A browser test is a test in `tests/browser.rs`, marked `#[ignore]`, run with `cargo test --test browser -- --ignored`. It needs Chrome.

## Phase 1: changes to the first module, on Windows

- [x] **Task 1: `start` reports the recorder's own error**
  - Description: A detached recorder that fails writes its error into `state.json`, and `start` prints that error in place of `recorder_start_failed`.
  - Steps:
    - `src/model.rs`: `ErrorInfo`, and `State.error`.
    - `src/error.rs`: `Relayed`, `code() -> &str`, `info()`, `to_json()` through `info()`. The variant test counts 15.
    - `src/control.rs`: `run_noting_error`, used by `record` and `run_recorder`; the `start` loop and `derived_status` as in Contracts. `prepare` writes `error: None`.
    - `src/recorder.rs` and `src/store.rs`: `error: None` in the two test helpers that build a `State`.
    - `src/request.rs`: in the unit test of `parse_selection`, bind the error before reading its code, as Contracts says under `src/error.rs`.
    - `tests/control.rs`: rename `broken_source_fails_to_start_with_its_reason` to `broken_source_fails_to_start_with_its_own_code`; it expects exit 1, code `capture_failed` and a message containing `the synthetic source is broken`. Add to it: `status <id>` for the one directory under the output root reports `crashed`.
  - Acceptance:
    - `RECDR_FAKE_SOURCE=broken recdr start --monitor 0` exits 1 with `capture_failed` (criterion 18), and the JSON on stderr equals what `recdr record --monitor 0 --duration 1s` prints for the same source.
    - The failed directory stays on disk and `status` lists it as `crashed` at once, without waiting 5 seconds. The same holds after a failed `recdr record` with the broken source (a second test in `tests/control.rs`, `failed_record_is_crashed_at_once`).
    - `Error::Relayed(info).to_json()` equals `json!({"error": info})` (unit test in `src/error.rs`).
  - Verify: `cargo test --test control`, `cargo test --lib error`.
  - Dependencies: none
  - Files: `src/model.rs`, `src/error.rs`, `src/control.rs`, `src/recorder.rs`, `src/store.rs`, `src/request.rs`, `tests/control.rs` (four of the seven are one-line edits)
  - Scope: M

- [x] **Task 2: the mouse hook moves under `capture`**
  - Description: A move with no change in behavior, so the next task's diff shows only the new click path.
  - Steps:
    - `git mv src/input.rs src/capture/hook.rs`.
    - `src/lib.rs`: remove `pub mod input;`.
    - `src/capture/mod.rs`: add `pub mod hook;` (it becomes private in Task 3).
    - `src/recorder.rs`: import `crate::capture::hook::ClickHook`.
    - `tests/layers.rs`: rule 3 allows `src/capture/hook.rs` in place of `src/input.rs`; rule 4's list loses `"input::"`.
  - Acceptance:
    - No file under `src/` contains `crate::input`.
    - `cargo test` and `cargo test --test desktop -- --ignored` pass with the same test counts as before the task.
  - Verify: the three commands, and `cargo test --test desktop -- --ignored`.
  - Dependencies: none
  - Files: `src/input.rs` (deleted), `src/capture/hook.rs`, `src/lib.rs`, `src/capture/mod.rs`, `src/recorder.rs`, `tests/layers.rs`
  - Scope: S

- [x] **Task 3: clicks arrive as capture events**
  - Description: The Windows capture source owns the hook and reports presses as `CaptureEvent::Click`. The recorder stops naming the hook.
  - Steps:
    - `src/model.rs`: `CaptureEvent::Click`.
    - `src/capture/hook.rs`: `RawClick` public, `try_next`, `Drop`, no `finish`, the rewritten ignored test, and the import and comment edits Contracts lists under `src/recorder.rs`.
    - `src/capture/windows.rs`: `WgcSource::open` takes `clicks`; fields `hook`, `origin`, `size`; `to_event`; `next_event` as in Contracts. Unit test of `to_event` with origin (100, 50) and size (640, 480): a press at (100, 50) is a click at (0, 0); at (739, 529) a click at (639, 479); at (740, 50), (99, 50) and (100, 530) `None`.
    - `src/capture/mod.rs`: `open` takes `clicks`; `mod hook;` is private.
    - `src/recorder.rs`: `capture::open` with `options.clicks`; the click list and its conversion as in Contracts; no `ClickHook` import and no `ResolvedKind` import. Add a unit test next to the existing two: given `started_at_unix_ms` 1000 and `duration_ms` 500, presses at 900, 1200 and 1600 give one click with `t_ms` 200. Put the conversion in a private function `fn clicks(raw: Vec<(u64, i32, i32, &'static str)>, started_at_unix_ms: u64, duration_ms: u64) -> Vec<Click>` so the test can call it.
  - Acceptance:
    - `src/recorder.rs` contains neither `ClickHook` nor `hook::`.
    - The two new unit tests pass (`to_event`, and the recorder's time conversion).
    - `cargo test --test desktop -- --ignored` passes, and `cargo test --lib hook -- --ignored` passes.
  - Verify: the three commands, the two ignored runs above. The by-hand click check is at Checkpoint A.
  - Dependencies: Task 2
  - Files: `src/model.rs`, `src/capture/hook.rs`, `src/capture/windows.rs`, `src/capture/mod.rs`, `src/recorder.rs`
  - Scope: M

- [x] **Task 4: process code behind `platform/`**
  - Description: `src/platform.rs` becomes a directory, and the wrapped command is started and ended through it. Still Windows only.
  - Steps:
    - `git mv src/platform.rs src/platform/windows.rs`.
    - `src/platform/mod.rs`: `mod windows; pub use windows::*;` (the `cfg` lines and `chrome_user_data_dir` come in Task 5).
    - `src/platform/windows.rs`: `spawn_command` and `kill_tree`, taking the code and its two comments from `src/recorder.rs`.
    - `src/recorder.rs`: call them; remove `Command` and `Stdio` from its imports.
    - `tests/layers.rs`: rule 3 allows the four files in Contracts, and the test is renamed `rule_3_unsafe_only_in_four_files`.
  - Acceptance:
    - `src/recorder.rs` contains none of `"cmd"`, `taskkill`, `Command::new`.
    - The three wrapped-command tests in `tests/record.rs` pass unchanged.
  - Verify: `cargo test --test record`, then the three commands.
  - Dependencies: Task 2 (rule 3 names `src/capture/hook.rs`)
  - Files: `src/platform.rs` (deleted), `src/platform/mod.rs`, `src/platform/windows.rs`, `src/recorder.rs`, `tests/layers.rs`
  - Scope: S

### Checkpoint A: the first module still works

- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass
- [x] `cargo test --test desktop -- --ignored` passes
- [ ] By hand, by the owner: click once during `recdr record --monitor 0 --duration 5s`; `manifest.json` has one click and the GIF shows a ring (criterion 19)

## Phase 2: Linux and macOS

- [x] **Task 5: recdr compiles off Windows**
  - Description: Unix versions of the process calls, `cfg` gates in the two `mod.rs` files, and `unsupported_platform`.
  - Steps:
    - `Cargo.toml`: `libc` under `[target.'cfg(unix)'.dependencies]`.
    - `src/platform/unix.rs`: the five functions in Contracts.
    - `src/platform/mod.rs`: the `cfg` lines and `chrome_user_data_dir`.
    - `src/error.rs`: `UnsupportedPlatform`. The variant test counts 16.
    - `src/capture/mod.rs`: `cfg(windows)` on `mod hook;` and `mod windows;`; the two definitions each of `list_windows`, `list_monitors` and `desktop_source`, as in Contracts under `src/capture/mod.rs`.
    - Run `rustup target add x86_64-unknown-linux-gnu aarch64-apple-darwin` once.
  - Acceptance:
    - `cargo check --all-targets --target x86_64-unknown-linux-gnu` and `cargo check --all-targets --target aarch64-apple-darwin` finish without errors or warnings.
    - On Windows the three commands pass and nothing a user can see changed.
  - Verify: the two `cargo check` commands, then the three commands. If a cross check fails because a build script needs a C compiler for the target, note it in the commit message and go on; Task 7 is then the check.
  - Dependencies: Task 4
  - Files: `Cargo.toml`, `Cargo.lock`, `src/platform/unix.rs`, `src/platform/mod.rs`, `src/error.rs`, `src/capture/mod.rs`
  - Scope: M

- [x] **Task 6: the tests and the six layer rules run on every system**
  - Description: Test files stop assuming Windows, and `tests/layers.rs` reaches its final form.
  - Steps:
    - `tests/layers.rs`: rules 2 and 6 as in Contracts; the header comment points to the spec's Project structure section.
    - `tests/desktop.rs`: `#![cfg(windows)]` as the first line after the doc comment.
    - `tests/record.rs`: three constants with a `cfg(windows)` and a `cfg(unix)` value each, used by the three wrapped-command tests: `ECHO_THEN_WAIT` (`cmd /c "echo hi & ping -n 3 127.0.0.1 >nul"` and `sh -c "echo hi; sleep 2"`), `EXIT_3` (`cmd /c "exit 3"` and `sh -c "exit 3"`), `LONG` (`ping -n 30 127.0.0.1` and `sh -c "sleep 30 & wait"`).
    - `tests/control.rs`: `pid_listed` and a new `kill(pid)` helper each get a `cfg(unix)` version (`kill -0 <pid>` succeeding, and `kill -9 <pid>`); `killed_recorder_is_crashed` calls `kill`.
    - `tests/tabs.rs`: `mod common;` and a `#[cfg(not(windows))] mod unsupported` block with its own `use` lines and one test: `recdr windows`, `recdr monitors` and `recdr record --monitor 0 --duration 1s` each exit 1 with code `unsupported_platform`.
  - Acceptance:
    - `cargo test --test layers` runs six tests and they pass.
    - `cargo check --all-targets --target x86_64-unknown-linux-gnu` and the same for `aarch64-apple-darwin` pass, and `cargo clippy --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` passes (skipped under the condition in Task 5).
    - On Windows `cargo test` passes with the same results as before.
  - Verify: the commands above. The Unix test runs themselves happen at Checkpoint B.
  - Dependencies: Task 5
  - Files: `tests/layers.rs`, `tests/desktop.rs`, `tests/record.rs`, `tests/control.rs`, `tests/tabs.rs`
  - Scope: M

- [x] **Task 7: CI workflow**
  - Description: The workflow in Contracts, without the browser step.
  - Steps:
    - `.github/workflows/ci.yml`: steps 1 to 5 of Contracts.
  - Acceptance:
    - The file parses as YAML and names the three runners, both triggers and Rust 1.98.
  - Verify: read the file against Contracts. The run itself is Checkpoint B.
  - Dependencies: Task 6
  - Files: `.github/workflows/ci.yml`
  - Scope: S

### Checkpoint B: three systems (needs a push)

- [x] Stop and ask the owner to push `feat/3-cdp-capture` and open a draft pull request
- [x] The `test` job is green on `windows-latest`, `ubuntu-latest` and `macos-latest`
- [x] A failure is fixed in the task that owns the failing file, as a new commit, before Phase 3

## Phase 3: tabs

- [x] **Task 8: CDP connection and tab list**
  - Description: `src/capture/cdp.rs` up to `list_tabs`: finding the WebSocket URL, connecting, the first call. No command uses it yet; Task 9 does.
  - Steps:
    - `Cargo.toml`: `tungstenite`.
    - `src/model.rs`: `Endpoint`, `TabInfo`, `TabList`.
    - `src/error.rs`: `CdpConnectFailed`. The variant test counts 17.
    - `src/capture/cdp.rs`: the constants it needs, `port_file_url`, `authority`, `reply_head`, `tabs`, `chrome_url`, `websocket_url`, `Incoming` with its first four fields, `Connection` (`open`, `send`, `poll`, `call`), `list_tabs`.
    - `src/capture/mod.rs`: `mod cdp;` and `pub fn list_tabs`.
  - Acceptance (unit tests in `src/capture/cdp.rs`):
    - `port_file_url("9222\n/devtools/browser/abc\n")` is `ws://127.0.0.1:9222/devtools/browser/abc`; one line, an empty file and a first line that is not a number give `None`.
    - `reply_head` reads `(200, 464)` from a head whose header is `Content-Length:464` and from one written `content-length: 464`; a head with no such header gives `None`.
    - `tabs` on a fixed `Target.getTargets` result with types `page`, `browser_ui`, `service_worker`, `page` returns the two pages in order.
    - `authority("ws://127.0.0.1:9222/devtools/browser/abc")` is `127.0.0.1:9222`; `authority("ws://example.test/x")` is `example.test:80`; `authority("http://x")` and `authority("ws:///x")` are `None`.
    - `list_tabs(&Endpoint::Port(p))` for a port nothing listens on returns `cdp_connect_failed` in under 6 seconds. `chrome_url` on an empty temporary directory returns `cdp_connect_failed` with `chrome://inspect/#remote-debugging` in the message, and on a directory whose `DevToolsActivePort` holds `x` returns `cdp_connect_failed` with `does not hold a port and a path`. No test in `src/` sets an environment variable.
  - Verify: `cargo test --lib cdp`, then the three commands.
  - Dependencies: Task 6 (the layer rules already allow `tungstenite` and `base64` in this file)
  - Files: `Cargo.toml`, `Cargo.lock`, `src/model.rs`, `src/error.rs`, `src/capture/cdp.rs`, `src/capture/mod.rs`
  - Scope: M

- [x] **Task 9: the `tabs` command and the browser test harness**
  - Description: `recdr tabs --cdp <endpoint>` end to end, and the helpers every later browser test uses.
  - Steps:
    - `src/request.rs`: `parse_endpoint` with a unit test for each of the six rules.
    - `src/control.rs`: `list_tabs`.
    - `src/cli.rs`: the `tabs` command.
    - `tests/tabs.rs`: `tabs --cdp <closed port>` exits 1 with `cdp_connect_failed` in under 6 seconds; `tabs --cdp https://example.test` and `tabs --cdp wss://example.test` exit 2 with `usage`; `tabs` without `--cdp` exits 2; `tabs --cdp chrome` with `RECDR_CHROME_USER_DATA_DIR` set on the child process to an empty directory exits 1 with `cdp_connect_failed` and a message containing `chrome://inspect/#remote-debugging`. A closed port is one a test got from `TcpListener::bind("127.0.0.1:0")` and then dropped.
    - `tests/browser.rs`: `Browser`, `Client` with `call` and `tab_ids` only, and two tests: `tabs_lists_only_pages` (two tabs, the second made with `Target.createTarget {url: "about:blank"}`; output has exactly those two ids, each with `title` and `url`), and `tabs_through_each_endpoint_form` (the same list through the port, `http://127.0.0.1:<port>`, the `ws://` URL built from `DevToolsActivePort`, and `chrome` with `RECDR_CHROME_USER_DATA_DIR` set to the test browser's directory on the `recdr` child process).
  - Acceptance:
    - Criterion 1 holds: the browser test passes.
    - The second browser test passes for all four forms.
    - `cargo test --test tabs` passes without Chrome.
  - Verify: `cargo test --test tabs`, `cargo test --test browser -- --ignored`, then the three commands.
  - Dependencies: Task 8
  - Files: `src/request.rs`, `src/control.rs`, `src/cli.rs`, `tests/tabs.rs`, `tests/browser.rs`
  - Scope: M

### Checkpoint C: recdr talks to Chrome

- [x] `cargo test --test browser -- --ignored` passes on Windows
- [ ] By hand: start Chrome with `--remote-debugging-port=9222 --user-data-dir=<an empty directory>`, run `recdr tabs --cdp 9222`, and see its tabs (covered by `tabs_through_each_endpoint_form`, which runs the same command against a test Chrome; left for the owner)

## Phase 4: tab recording

- [x] **Task 10: one matcher for windows and tabs**
  - Description: `single` becomes generic and `ambiguous_target` can list tabs. Tabs start using it in Task 11.
  - Steps:
    - `src/error.rs`: `AmbiguousTarget` and `TargetNotFound` as in Contracts; `info()` passes the candidates through. The two tests that build an `AmbiguousTarget` change as Contracts says.
    - `src/capture/mod.rs`: generic `single`; the window arms call it with `"window"`. The existing unit test reads candidate ids as `c["id"].as_u64()`. A new unit test calls `single` over two `TabInfo` values with kind `"tab"`.
  - Acceptance (unit tests):
    - `single` over two `TabInfo` values returns `ambiguous_target` whose two candidates have exactly the keys `id`, `title`, `url` and whose message is `2 tabs match "..."; pass --tab-id`; over one it returns that tab; over none `target_not_found` with the message `no target matches tab "..."`.
    - Two windows matching a title still give the message `2 windows match "..."; pass --window-id`.
  - Verify: `cargo test --lib`, `cargo test --test desktop -- --ignored two_matching_windows_are_ambiguous`, then the three commands.
  - Dependencies: Task 8
  - Files: `src/error.rs`, `src/capture/mod.rs`
  - Scope: S

- [x] **Task 11: the screencast source**
  - Description: `CdpSource` produces frames, with focus emulation, pacing and clean-up. No clicks yet: the third parameter of `open` is named `_clicks`, and step 4 of Contracts comes in Task 14.
  - Steps:
    - `src/model.rs`: `TabQuery`, `TabTarget`, `Target::Tab`, `ResolvedKind::Tab`, `StopReason::TabClosed`.
    - `src/capture/cdp.rs`: `pick`, `decode`, the `params` and `session_id` fields of `Incoming`, `CdpSource` with the six fields Contracts gives this task, `open` (steps 1, 2, 3, 5), `next_event` (the `Page.screencastFrame` and `Target.detachedFromTarget` rows of the method table, and the pacing), `dropped`, `Drop` (the first two calls).
    - `src/capture/mod.rs`: the `Tab` arms of `resolve` and `open`, and `CaptureSource::paced`.
    - `src/capture/windows.rs`: the last arm of the match in `WgcSource::open`, as in Contracts.
    - `src/recorder.rs`: `TabClosed` for a tab (with the `ResolvedKind` import back), and the `paced()` condition on the frame arm.
    - `src/control.rs`: `prepare` lists nothing for a tab; `start_timeout` and `TAB_START_TIMEOUT`.
  - Acceptance:
    - Unit test: `decode` of a 2x1 PNG built in the test with the `image` crate returns width 2, height 1 and the same 8 bytes; `decode("AAAA")` returns `capture_failed`.
    - Unit test: `resolve` of a `Target::Tab` returns `ResolvedKind::Tab` holding the same `TabTarget`.
    - Unit tests of `pick`: `TabQuery::Text("DOC")` finds the one tab titled `Docs`; text found only in one tab's URL finds that tab; text in two tabs is `ambiguous_target`; no match is `target_not_found`; `TabQuery::Id` matches the whole id only.
    - The three commands pass. Recording a tab is first exercised in Task 12, which is the next commit.
  - Verify: `cargo test --lib`, then the three commands.
  - Dependencies: Task 10
  - Files: `src/model.rs`, `src/capture/cdp.rs`, `src/capture/mod.rs`, `src/capture/windows.rs`, `src/recorder.rs`, `src/control.rs`
  - Scope: M

- [x] **Task 12: `--cdp`, `--tab-id` and `--tab` on `record` and `start`**
  - Description: The flags, and the first real tab recordings.
  - Steps:
    - `src/request.rs`: the three fields and the target rules of Contracts, with unit tests for the three usage messages.
    - `src/cli.rs`: the three flags.
    - `tests/tabs.rs`: `record --tab x --duration 1s`, `record --cdp 9222 --monitor 0 --duration 1s`, `record --cdp wss://example.test --tab-id x --duration 1s` and `record --cdp 1 --tab-id a --tab b --duration 1s` each exit 2 with `usage`; `record --cdp <closed port> --tab-id x --duration 1s` and `start --cdp <closed port> --tab-id x` each exit 1 with `cdp_connect_failed`.
    - `tests/browser.rs`: `serve`, the three pages, `Client::attach`, `eval` and `navigate`, and the tests named under Acceptance.
  - Acceptance (browser tests):
    - `records_a_tab_at_its_viewport_size`: `record --cdp <port> --tab-id <id> --duration 3s` on `/anim` exits 0; `width` and `height` equal `innerWidth * devicePixelRatio` and `innerHeight * devicePixelRatio` read through `Client::eval`; the GIF's frame delays sum to between 250 and 350 hundredths. Run once plain and once with `--force-device-scale-factor=2` (criterion 2).
    - `still_page_stores_one_or_two_frames` (criterion 3) and `animation_keeps_fifteen_frames_per_second` (`--duration 10s --fps 15`: `frames_stored` at least 100, `dropped_frames` 0; criterion 4).
    - `tab_text_matches_title_and_url`: `--tab` finds a tab by part of its title and by part of its URL in another letter case; text in two tabs gives `ambiguous_target` with two `{id, title, url}` candidates; unknown text and an unknown `--tab-id` give `target_not_found`; each of the three errors has the same code from `start` as from `record` (criterion 5).
    - `start_mark_stop_frames_on_a_tab`: `start` returns in under 2 seconds; `mark` puts its label in the manifest; `stop` gives `stopped`; `frames <id> --at 1s` prints a path (criterion 12).
  - Verify: `cargo test --test tabs`, `cargo test --test browser -- --ignored`, then the three commands.
  - Dependencies: Task 9, Task 11
  - Files: `src/request.rs`, `src/cli.rs`, `tests/tabs.rs`, `tests/browser.rs`
  - Scope: M

- [x] **Task 13: hidden tabs, closed tabs, navigation and a second client**
  - Description: Browser tests for behavior Task 11 already implements. A failure here is fixed in `src/capture/cdp.rs`.
  - Steps:
    - `tests/browser.rs`: the tests named under Acceptance. Tests that act during a recording start `recdr record` as a child process, wait 1.5 seconds, act through `Client`, then wait for the child.
  - Acceptance (browser tests):
    - `background_tab_is_recorded`: with a second tab created after it, the first tab reports `hidden`; recording it for 3 seconds at `--fps 15` stores at least 20 frames; during the recording the second tab reports `visible`; afterwards the first reports `hidden`. `minimized_window_is_recorded`: the same after `Browser.setWindowBounds` with `windowState: "minimized"` (criterion 6).
    - `closing_the_tab_ends_with_tab_closed` (`Target.closeTarget` during `--duration 30s`) and `ending_the_browser_ends_with_tab_closed` (the test kills Chrome): exit 0, `stop_reason` `tab_closed`, and the GIF decodes with at least one frame (criterion 7).
    - `navigation_does_not_end_the_recording`: record `http://127.0.0.1:<port>/anim` and, 1.5 seconds in, navigate to `http://localhost:<port>/green`; `stop_reason` is `duration_reached`; the center pixel of the first stored PNG is white and the center pixel of the last is `#00aa00`; every `Client` call made during the recording returned without an error (criteria 8 and 13).
    - `renderer_crash_probe`: navigate the recorded tab to `chrome://crash` during `--duration 8s`, sending the command without waiting for its reply; the recording exits 0 within 15 seconds with `tab_closed` or `duration_reached`, and the test prints which. Write the result into the commit message. If the recorder hangs or fails, stop and ask: acting on a new CDP event changes the spec.
  - Verify: `cargo test --test browser -- --ignored`, then the three commands.
  - Dependencies: Task 12
  - Files: `tests/browser.rs`, `src/capture/cdp.rs` (only if a test finds a defect)
  - Scope: M

### Checkpoint D: a tab records

- [x] `cargo test`, `cargo test --test browser -- --ignored` and `cargo test --test desktop -- --ignored` pass on Windows
- [ ] By hand: open the GIF from a 3 second recording of `ANIMATION` and see the box move (stored frames checked: the box moves from x 12 to x 364; GIF playback left for the owner)

## Phase 5: the rest

- [x] **Task 14: clicks from the page**
  - Description: The binding, the listener, the scaling, and their removal at the end.
  - Steps:
    - `src/capture/cdp.rs`: `CLICK_LISTENER`, `BINDING`, `click`, the fields `frame_size`, `clicks` and `click_script`, the parameter `clicks` of `open` and its step 4, the `Page.frameNavigated` and `Runtime.bindingCalled` rows of the method table, the two removals in `Drop`.
    - `tests/browser.rs`: `Client::press`, and the tests named under Acceptance.
  - Acceptance:
    - Unit test: `click` maps `{"x":100,"y":50,"button":0,"w":1264,"h":625}` with a 2528x1250 frame to `(200, 100, "left")`; button 1 is `middle`, 2 is `right`, 3 is `None`; `w` of 0 is `None`; text that is not JSON is `None`.
    - Browser test `press_is_recorded_in_frame_pixels`, recording `/anim` (white at that spot): a press at CSS (100, 50) sent through `Client::press` is in the manifest with button `left` at (100, 50), and at (200, 100) with `--force-device-scale-factor=2`, with `t_ms` within 200 of the send time measured against `started_at_unix_ms`. In the stored PNG shown at that time every pixel of the 40x40 square around the click is white; in the GIF frame shown 100 ms after the click at least one pixel of that square is not white. A press sent 1 second after navigating to `http://localhost:<port>/green` is in the manifest too (criterion 9).
    - Browser test `no_click_highlight_installs_nothing`: with the flag, `typeof window.__recdrClick` is `"undefined"` during the recording and the manifest has no clicks; without it, the value is `"undefined"` in a document loaded after the recording ended (criterion 10).
  - Verify: `cargo test --lib cdp`, `cargo test --test browser -- --ignored`, then the three commands.
  - Dependencies: Task 13
  - Files: `src/capture/cdp.rs`, `tests/browser.rs`
  - Scope: M

- [x] **Task 15: MCP `list_tabs` and tab arguments**
  - Description: The ninth tool and the two changed descriptions. `start_recording` and `record` already accept `cdp`, `tab_id` and `tab` through `RecordRequest`.
  - Steps:
    - `src/mcp.rs`: `TabsParams`, `list_tabs`, the two descriptions.
    - `tests/common/mod.rs`: move the `Server` helper and the `text` and `is_error` functions here from `tests/mcp.rs`, made `pub`, with `Server::start(fake: Option<&str>)` as in Contracts.
    - `tests/mcp.rs`: use them from `common` with `Server::start(Some("moving"))`; `lists_the_eight_tools` becomes `lists_the_nine_tools`; add `list_tabs_on_a_closed_port_is_an_error` (the reply is an error whose text has `cdp_connect_failed`).
    - `tests/browser.rs`: `mcp_records_a_tab` starts `Server::start(None)`, calls `list_tabs`, then `record` with `cdp`, `tab_id` of the `/anim` tab and `duration` `2s`; the reply's second content block has type `image`, and the `width` in its JSON text equals the tab's `innerWidth * devicePixelRatio`, which the synthetic source's 320 would not.
  - Acceptance:
    - Criterion 16 holds: `tools/list` returns nine names, and the browser test passes.
    - `cargo test --test mcp` passes without Chrome.
  - Verify: `cargo test --test mcp`, `cargo test --test browser -- --ignored mcp_records_a_tab`, then the three commands.
  - Dependencies: Task 12
  - Files: `src/mcp.rs`, `tests/common/mod.rs`, `tests/mcp.rs`, `tests/browser.rs`
  - Scope: S

- [x] **Task 16: browser tests in CI and the Unix wrapped command**
  - Description: The browser step in the workflow, and criterion 17's wrapped command.
  - Steps:
    - `.github/workflows/ci.yml`: step 6 of Contracts.
    - `tests/browser.rs`: `wraps_a_command_while_recording_a_tab`, using the system's echo-then-wait command (`sh -c "echo hi; sleep 2"` on Unix, `cmd /c "echo hi & ping -n 3 127.0.0.1 >nul"` on Windows): `stop_reason` `command_exited`, `command_exit_code` 0, `hi` on stderr.
  - Acceptance:
    - The new browser test passes on Windows.
    - The workflow has six steps.
  - Verify: `cargo test --test browser -- --ignored`, then the three commands. The CI run is Checkpoint E. If Chrome is not at the default path on a runner, set `RECDR_TEST_CHROME` for that runner in the workflow.
  - Dependencies: Task 14, Task 15
  - Files: `.github/workflows/ci.yml`, `tests/browser.rs`
  - Scope: S

- [x] **Task 17: README and the acceptance run**
  - Description: Documentation, and one pass over the 21 criteria.
  - Steps:
    - `README.md`, existing text that this module makes untrue: the opening line and the requirements line that say Windows only (desktop targets need Windows 10 or later; tab targets run on Windows, Linux and macOS); the two places that write `recdr.exe` where the path is not Windows-specific; the `<target>` table, which gets `--tab-id` and `--tab` with `--cdp`, and its sentence about when a target is resolved (a tab is found when the recorder connects); and the `--no-click-highlight` sentence "No mouse hook is installed", which gains "and nothing is added to the page".
    - `README.md`, new lists it does not have today: stop reasons (the six of the first spec and `tab_closed`), error codes (all sixteen), and the three test commands.
    - `README.md`: a "Record a browser tab" section (the `tabs` command, the three flags, the four `--cdp` forms and the browser each fits, how to build a `ws://` URL from another browser's `DevToolsActivePort`); Linux and macOS (tab targets only, the wrapped command runs without a shell); the nine MCP tools. A "Limits of tab recording" list: the page sees itself as visible and focused while recorded; `window.__recdrClick` and `window.__recdrListening` are added to the page; no clicks inside iframes; a click after the viewport changes size may be drawn in the wrong place; a press in the last 50 ms can be missed; a frame is stored about 50 ms after the browser sent it; reload the tab after a recorder was killed.
    - `tasks/3-cdp-capture/acceptance.md`: one line per criterion with the test or command that showed it and the result. Criteria 14 and 15 are marked "by hand, owner" with the exact commands to run.
  - Acceptance:
    - Every command in the new README section runs as written against a test browser.
    - No sentence in `README.md` says recdr runs only on Windows or has five kinds of target.
    - `acceptance.md` has 21 entries, and each automated one names a passing test.
  - Verify: run the README's commands by hand; `cargo test`, `cargo test --test browser -- --ignored`, `cargo test --test desktop -- --ignored`.
  - Dependencies: Task 16
  - Files: `README.md`, `tasks/3-cdp-capture/acceptance.md`
  - Scope: S

### Checkpoint E: complete (needs a push)

- [x] Stop and ask the owner to push; CI is green on all three runners, browser tests included (criteria 17 and 20)
- [x] `tests/layers.rs` runs six rules (criterion 21)
- [ ] The owner checks criteria 14 and 15 by hand with the commands in `acceptance.md`
- [x] Ready for `/review`
