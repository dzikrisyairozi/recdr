# Goldfish review: tasks/1-recdr/plan.md

| Reviewer | Status |
|---|---|
| readiness | completed, two rounds |
| codebase | completed, two rounds |

Round 1 reviewed the first draft of the plan. After it, the plan and task list were rewritten: the findings below were applied, and at the owner's request the modules were rearranged into layers (see "Layers" in the plan). Both lenses raised the same Blocker, so both ran a second round on the rewritten plan. Round 2 raised no Blocker, so the review stopped there. The fixes from round 2 have not been reviewed by a fresh reader.

Totals: round 1, 29 findings after merging duplicates (1 Blocker, 14 Major, 14 Nit), all Fix. Round 2, 21 findings after merging duplicates (9 Major, 12 Nit), all Fix.

## Findings, round 1

### 1. Monitor index 0 never resolves (Blocker)

- Lens: readiness, and separately codebase
- Where: Module contracts, `capture/windows.rs`, `list_monitors`
- Problem: The plan said `MonitorInfo.index` is the crate's `Monitor::index()` and that `Monitor::from_index` accepts it. In `windows-capture` 2.0.1, `from_index` is one-based over `enumerate()` and rejects 0, and `index()` is a number parsed from the device name.
- Why: Every desktop check uses `--monitor 0`. On two monitors the recorded monitor could differ from the one whose bounds were used.
- Decision: Fix
- Evidence or edit: `index` is now the zero-based position in `Monitor::enumerate()`, opened with `Monitor::from_index(index + 1)`. The crate's `index()` is not used. Spec: "Indexes start at 0".

### 2. One unreadable window fails every command (Major)

- Lens: codebase
- Where: `list_windows`, `prepare`
- Problem: `Window::process_name()` returns `Err` for elevated processes, and `prepare` listed windows for every target kind.
- Decision: Fix
- Evidence or edit: `list_windows` skips a window whose title or rect cannot be read and uses an empty `process` or `pid` 0 otherwise. `prepare` lists windows only for a window target and monitors only for a region or monitor target.

### 3. Untracked entries in the working tree (Major)

- Lens: codebase (also readiness Nit 26)
- Where: Overview, File boundary
- Problem: `recdr/` (a nested clone), `.serena/`, `spec/` and `tasks/` are untracked and unmentioned; `git add -A` would commit the wrong things.
- Decision: Fix
- Evidence or edit: New section "State of the working tree before Task 1". Task 1 stops if `recdr/` exists, ignores `.serena/`, commits `spec/` and `tasks/` first, and every commit stages by path.

### 4. Notepad desktop tests (Major)

- Lens: codebase, and readiness
- Where: Tasks 4 and 11 of the first draft
- Problem: Notepad is a Store app behind an execution alias; its process name, its kill behavior, an already-open Notepad, and how to minimize it were all open.
- Decision: Fix
- Evidence or edit: Desktop tests open their own Windows Forms window through PowerShell with a unique title (`test_window` in `tests/common/mod.rs`), find it by title, and close it with `Child::kill`.

### 5. Unused test helpers fail clippy (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `tests/common/mod.rs` starts with `#![allow(dead_code)]`.

### 6. `pub mod ffmpeg;` declared before the file exists (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Each task adds the `mod` line for the file it creates; Task 14 lists `src/encode/mod.rs`.

### 7. Task files lists incomplete (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Task 13 names the file of every test. Flags `--no-cursor` and `--no-click-highlight` are parsed in Task 4, so Task 13 does not touch `cli.rs`.

### 8. Marks, clicks and frames used different clocks (Major)

- Lens: readiness
- Problem: Frame times counted from the loop start; marks and clicks from a timestamp written before the recorder was spawned.
- Decision: Fix
- Evidence or edit: `recorder::capture` step 3 takes `started_at_unix_ms` and the `Instant` at the first frame and rewrites the state; all three count from there. Task 10's assertion is stated with `started_at_unix_ms` from the manifest.

### 9. `RecordingResult` had no fields (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Full struct in `src/model.rs` contracts, with the Task 3 to Task 7 change of `contact_sheet` and `diff` stated. `FrameRef` exists from Task 3.

### 10. Wrapped command described two ways; child's fate unstated (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `Command::new("cmd").arg("/C").args(&command)`. A child still running when the recording ends for another reason is killed and `command_exit_code` is absent. Task 11 tests it.

### 11. Polled files written without atomicity (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Every JSON write in `store.rs` goes to a `.tmp` file and is renamed.

### 12. Window size from `GetWindowRect`; mismatch fatal (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `ResolvedTarget` has no size. The first frame fixes the recording's size. `letterbox` is part of the store from Task 3. Task 5 checks the window size within 40 pixels of the listed bounds.

### 13. DPI call is `unsafe` in `cli.rs` (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `platform::set_dpi_aware()` in the new `src/platform.rs`; the spec's `unsafe` rule now lists three files.

### 14. Mark labels lost when keyframes are sub-sampled (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: A tile's time span runs to the next tile, so spans cover the whole recording and every mark and click lands on a tile.

### 15. `ClickHook` static could be set once per process (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `static SENDER: Mutex<Option<Sender>>`, cleared in `finish`; a second concurrent start is an error. The MCP `record` tool now uses a detached recorder (`control::start` then `control::wait`), so a process holds at most one in-process recording.

### 16. `WgcSource` drop and channel disconnect (Nit)

- Lens: codebase, and readiness
- Decision: Fix
- Evidence or edit: `control: Option<CaptureControl<..>>` taken in `Drop`; a disconnected channel is `CaptureFailed("the capture thread ended")`.

### 17. Risk row about a COM apartment (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: The row now names the real unknown (frames in a detached process with no console) and the contract notes that `start_free_threaded` initialises WinRT itself.

### 18. `base64` version unpinned (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: 0.22.

### 19. No error message text; `FfmpegNotFound` shape differs from the spec sample (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Each variant's `#[error]` text is in the contract; the spec sample now uses the unit variant.

### 20. Derives and serde form unspecified (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: One rule at the top of the `model.rs` contract.

### 21. `recording_dir` missing from contracts (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Replaced by `RecordingDir::open` in the `store.rs` contract.

### 22. CLI argument structs and parsers unnamed (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: clap structs listed in the `cli.rs` contract; `parse_region`, `parse_formats`, `resolve_out` in `request.rs`; `--format` validated from Task 4.

### 23. Small unstated behaviors of `mark`, `status`, `stop` (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `derived_status` defines the `Starting` window; `mark` on an unknown id is `RecordingNotFound` and uses the derived status.

### 24. Contact-sheet margins, text offset, long labels (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: 4-pixel margin, text at 4 in and 2 down, cut to `(tile width - 8) / 16` characters.

### 25. `get_frames` content and parameter struct names (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Text block first, then images; four named parameter structs.

### 26. Id collision (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `RecordingDir::create` retries with the 16-bit suffix plus 1, up to 16 times.

### 27. `Cargo.lock` missing from tasks that add dependencies (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Listed in Tasks 1, 2, 3, 6, 7 and 15.

### 28. Writer-thread fallback had no design (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: The fallback is removed. If criterion 15 fails, Task 16 stops and reports the numbers to the owner.

### 29. Issue checklist not in a readable file (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: The final checkpoint links the issue URL.

## Findings, round 2

### 30. Front ends were allowed to import `time` and `platform`, against the spec (Major)

- Lens: codebase
- Where: Layers rule 4, `tests/layers.rs`
- Decision: Fix
- Evidence or edit: `--at` and `--every` parsing moved to `request::parse_selection`, and `set_dpi_aware` is called from `main.rs`. Rule 4 is now `control`, `request`, `model`, `error` only, in the plan, the test and the spec.

### 31. Rule 1 was wider than its test and broken by `error.rs` (Major)

- Lens: codebase, and readiness
- Decision: Fix
- Evidence or edit: Rule 1 is the nine quoted file names the test checks. The `RecorderCrashed` message no longer contains a file name.

### 32. Detached recorder inherits the caller's pipes (Major)

- Lens: readiness
- Problem: A program reading `recdr start`'s output to end of file would wait for the recorder to exit.
- Decision: Fix
- Evidence or edit: `platform::spawn_detached` clears `HANDLE_FLAG_INHERIT` on the three standard handles around the spawn. The Task 9 test runs `start` with `Command::output()`, which fails if the pipes leak.

### 33. Killing `cmd.exe` leaves the wrapped command running (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `taskkill /T /F /PID` on the child, then wait. The Task 11 test uses `Command::output()` so it only passes when the whole tree is gone.

### 34. Position of `candidates` in the error JSON; no test of it (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `{"error":{"code","message","candidates":[..]}}` in the contract; a unit test in Task 2 and a desktop test with two windows in Task 5.

### 35. Who rejects `record` with neither duration nor command (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `control::record` returns `Usage`; `control::start` rejects a command; the MCP `record` tool returns `Usage` when `duration` is missing.

### 36. List wrappers defined only in `cli.rs` (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `WindowList`, `MonitorList`, `StatusList`, `FrameList` and `MarkAck` are in `model.rs` and are what `control` returns. Both front ends serialize the same value.

### 37. Frame selection rules for MCP; `--every 0ms` never ends (Major)

- Lens: readiness (two findings)
- Decision: Fix
- Evidence or edit: One function, `request::parse_selection`, used by both front ends; an interval of 0 is `Usage`. Tests in Task 8.

### 38. No branch, no pull request step, six error codes untested (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Task 1 creates `feat/1-agent-recorder`. Task 16 writes `tasks/1-recdr/acceptance.md` instead of a pull request description; opening a pull request is left to the owner. A fourth synthetic kind, `broken`, produces `capture_failed` (Task 4) and `recorder_start_failed` (Task 9). `io_error` is tested in Task 4, `encode_failed` in Task 14, `target_not_found` and the `ambiguous_target` listing in Task 5.

### 39. Layer table contradicted the contracts (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Rows reordered so every import points to a lower row: `analyze, encode` above `capture, input, store`; `error` above `model`. Same table in the spec.

### 40. Rule 5 untested; matching method open (Nit)

- Lens: readiness, and codebase
- Decision: Fix
- Evidence or edit: Five tests. The contract states the method: comment lines dropped, plain substrings, and the exact substrings for rule 4.

### 41. Review logs not mentioned in the commit plan (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: Both `goldfish.md` files are named in the docs commit.

### 42. `base64` 0.22 would build a second copy (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: 0.23, the version `rmcp` 3.11 uses.

### 43. Derives on `Frame`, `CaptureEvent`, `FrameSelection` (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `Debug, Clone` only, stated beside each.

### 44. Strings carried by the target errors (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Listed under the `error.rs` contract.

### 45. `Manifest.duration_ms` undefined (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: The elapsed time when the stop condition was seen (`recorder` step 6).

### 46. `into_options` edge inputs (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Ordered steps; an unknown `RECDR_FAKE_SOURCE` value is `Usage`; the one-target check runs first; `gif,gif` is one format.

### 47. Contact-sheet text and filter details (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `format!("{:.1}s", ..)`, one space, `FilterType::Triangle`.

### 48. `_recorder` output and clap attributes (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `_recorder` prints nothing; flatten, positional and long-flag rules are in the `cli.rs` contract.

### 49. Task file lists and test methods (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Task 10 no longer lists `recorder.rs`; Task 16 edits no source and stops on any failure; Task 9 checks liveness with `tasklist`; Task 14 skips when ffmpeg or ffprobe is missing.

### 50. MCP tool descriptions, server name, `out` handling (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: A description per tool in the table; name `recdr`, version from `CARGO_PKG_VERSION`; every `out` goes through `request::resolve_out`.
