# Review remediation plan: agent-recorder (PR #2)

PR: https://github.com/dzikrisyairozi/recdr/pull/2
Review: https://github.com/dzikrisyairozi/recdr/pull/2#pullrequestreview-5989266256
Reviewed head: `1843ebc` (`feat/1-agent-recorder`), 2026-11-20. A fresh-context `code-reviewer` read the diff, the spec and the plan; the main session triaged its findings. Line numbers are in the file at the PR head unless a line says "stack top".

## Triage

| # | Finding | Severity | Decision |
|---|---|---|---|
| 1 | An error after the capture loop is reported as `recorder_crashed` | Important | Fix |
| 2 | MCP `get_frames` checks the 8-frame limit after writing every sample | Important | Fix |
| 3 | Window bounds and click origin include the invisible resize border | Important | Fix (measured) |
| 4 | An error mid-recording leaves the wrapped command running | Important | Fix |
| 5 | A reused process id makes `stop` wait forever | Important | Fix |
| 6 | Handle-inherit flags change with no lock while MCP calls run in parallel | Suggestion | Fix |
| 7 | `start` can give up on a recorder that then records anyway | Suggestion | Fix |
| 8 | The recorder can hang in `Drop for WgcSource` | Suggestion | Fix |
| 9 | GIF frame delay wraps above 655 s | Suggestion | Fix |
| 10 | A recording id can point outside the output root | Suggestion | Fix |
| 11 | `frames` on a running recording says `recording_not_found` | Suggestion | Escalate (the spec has no code for it) |
| 12 | `expect` outside tests | Suggestion | Fix |
| 13 | `--every` with `--max-width` rewrites the same sample | Suggestion | Fix |
| 14 | ffmpeg opens a console window from a detached recorder | Important (reviewer) | Contest |
| 15 | Frame times are read at dequeue | Important (reviewer) | Moved to PR #7's plan |

### Contested

14. The reviewer said this is still present at the stack top. At the stack top, `src/encode/mod.rs:7-10` compiles `ffmpeg` only under `#[cfg(not(any(windows, target_os = "macos")))]`. From PR #9 on, Windows encodes with Media Foundation and never starts ffmpeg, so a fix on PR #2 would be code PR #9 deletes. Not posted.

15. PR #7 moves storing to a writer thread with a one-slot mailbox, so the 8-slot channel no longer backs up. What is left (the time is read when the frame is handed over) is task 3 in `tasks/3-cdp-capture/review/plan.md`.

### Measurement for finding 3

Windows 11, one Docker Desktop window. `recdr windows` reported 1286x728 at x=2142. A 1 s `--window-id` recording was 1272x721. `GetWindowRect` gave 1286x728 at 2142,146 and `DWMWA_EXTENDED_FRAME_BOUNDS` gave 1272x721 at 2149,146.

### Spec drift (owner decides)

- `--max-size`: the spec says the directory's size; `bytes()` counts frame PNGs only. Recommended: change the spec to "stored frames".
- There is no error code for `frames` on a recording that has not finished. Recommended: name one in the spec, then do task 11.
- `size_cap_reached` is covered only by a unit test with a 1-byte cap; the spec puts stop reasons at the integration level. Recommended: add a CLI test.

### Over-engineering pass (optional)

- `src/time.rs:42` and `src/request.rs:116`: `parse_time` and `parse_size` are one function with two suffix tables.
- `src/capture/windows.rs:198`: the Window and Monitor arms repeat `Settings::new` and `start_free_threaded`.
- `src/capture/mod.rs:17`: `dropped()` could have a default body of 0.
- `src/store.rs:280`: `FrameWriter::push` returns a bool that only one test reads.

## Phase 1: Important

All five are unchanged in the later branches except where noted. Fix on `feat/1-agent-recorder` and merge up the stack, or on the stack top if the owner prefers.

### Task 1: Relay a stored recorder error from `await_result`

**Description:** When `state.json` holds an error, `stop`, `record` and the MCP calls return that error, not `recorder_crashed`. `state.error` exists from PR #7 on, so this lands there or higher.

**Acceptance criteria:**
- [ ] A detached recording whose MP4 step fails returns `encode_failed` from `stop`.
- [ ] `status` for that recording still reports `crashed`.

**Verification:**
- [ ] New test in `tests/record.rs`: start, force a failure after the loop, `stop`, assert the code.
- [ ] `rtk proxy cargo test`

**Dependencies:** None
**Files likely touched:** `src/control.rs`, `tests/record.rs`
**Estimated scope:** Small

### Task 2: Check the MCP frame limit before any sample is written

**Description:** `control::frames` takes an optional maximum count and returns `TooManyFrames` before `frame_refs` runs.

**Acceptance criteria:**
- [ ] `get_frames` with `every: "10ms"` on a long recording returns `too_many_frames`.
- [ ] `samples/` is empty after that error.

**Verification:**
- [ ] Extend the MCP test to assert `samples/` is empty.
- [ ] `rtk proxy cargo test --test mcp`

**Dependencies:** None
**Files likely touched:** `src/control.rs`, `src/mcp.rs`, `src/cli.rs`, `tests/mcp.rs`
**Estimated scope:** Small

### Task 3: Use the extended frame bounds for windows

**Description:** `list_windows` reads `DWMWA_EXTENDED_FRAME_BOUNDS` for `bounds`, and the click origin follows.

**Acceptance criteria:**
- [ ] `bounds.width` and `bounds.height` from `recdr windows` equal the frame size of a `--window-id` recording of the same window.
- [ ] A click at a known point of the test window is stored within 2 px of that point.

**Verification:**
- [ ] Desktop test: compare `windows` bounds with the recorded frame size.
- [ ] `rtk proxy cargo test --test desktop -- --ignored --test-threads=1`
- [ ] Manual: the owner clicks once during a window recording. This also closes the open click check of Checkpoint A.

**Dependencies:** None
**Files likely touched:** `src/capture/windows.rs`, `Cargo.toml` (feature `Win32_Graphics_Dwm`), `tests/desktop.rs`
**Estimated scope:** Small

### Task 4: End the wrapped command when the capture loop fails

**Description:** An error from `next_event` or `push` no longer skips `platform::kill_tree`.

**Acceptance criteria:**
- [ ] `record -- <cmd>` with the `broken` fake source exits with the capture error.
- [ ] The child process is not running afterwards.

**Verification:**
- [ ] Test in `tests/record.rs`: the wrapped command writes its pid to a file; assert the process is gone.
- [ ] `rtk proxy cargo test --test record`

**Dependencies:** None
**Files likely touched:** `src/recorder.rs`, `tests/record.rs`
**Estimated scope:** Small

### Task 5: Tell a recorder from a reused process id

**Description:** `state.json` stores the recorder's creation time (or image name) and the liveness check compares it.

**Acceptance criteria:**
- [ ] A `state.json` whose pid belongs to another live process reports `crashed`.
- [ ] `stop` on it returns `recorder_crashed` and does not wait.

**Verification:**
- [ ] Unit test: a state with the test process's own pid and a wrong creation time gives `Crashed`.
- [ ] `rtk proxy cargo test`

**Dependencies:** None
**Files likely touched:** `src/platform/windows.rs`, `src/platform/unix.rs`, `src/model.rs`, `src/control.rs`, `src/recorder.rs`
**Estimated scope:** Medium

### Checkpoint: Important
- [ ] `rtk proxy cargo fmt --check`, `rtk proxy cargo clippy --all-targets -- -D warnings` and `rtk proxy cargo test` pass
- [ ] The 12 desktop tests pass
- [ ] CI is green on all three systems

## Phase 2: Suggestions

### Task 6: Lock around the handle-inherit steps

**Description:** A `static SPAWN: Mutex<()>` is held across inherit off, spawn and inherit on in `spawn_detached`.

**Acceptance criteria:**
- [ ] The three steps run under one lock.

**Verification:**
- [ ] `rtk proxy cargo test`

**Dependencies:** None
**Files likely touched:** `src/platform/windows.rs`
**Estimated scope:** XS

### Task 7: Ask the recorder to stop when `start` times out

**Description:** On the start timeout, `start` calls `dir.request_stop()` before it returns the error. This also covers the 40 s tab window in PR #7.

**Acceptance criteria:**
- [ ] After `recorder_start_failed` on a timeout, the recorder process ends within a few seconds.

**Verification:**
- [ ] Test with a fake source that delays its first frame past the timeout.

**Dependencies:** None
**Files likely touched:** `src/control.rs`, `src/capture/fake.rs`, `tests/record.rs`
**Estimated scope:** Small

### Task 8: Drop the receiver before stopping the capture

**Description:** `WgcSource::events` becomes an `Option<Receiver<_>>` and `Drop` takes it before `control.stop()`.

**Acceptance criteria:**
- [ ] A blocked `on_closed` send fails and the capture thread ends.

**Verification:**
- [ ] The desktop tests pass.

**Dependencies:** None
**Files likely touched:** `src/capture/windows.rs`
**Estimated scope:** XS

### Task 9: Split GIF pieces longer than 655,350 ms

**Acceptance criteria:**
- [ ] A piece of 900,000 ms becomes frames whose delays add up to 90,000 hundredths.

**Verification:**
- [ ] Unit test on the delays in `src/encode/gif.rs`.

**Dependencies:** None
**Files likely touched:** `src/encode/gif.rs`
**Estimated scope:** XS

### Task 10: Reject ids that are not recording ids

**Acceptance criteria:**
- [ ] `RecordingDir::open` returns `RecordingNotFound` for `..\x` and for an absolute path.

**Verification:**
- [ ] Unit test in `src/store.rs`.

**Dependencies:** None
**Files likely touched:** `src/store.rs`
**Estimated scope:** XS

### Task 11: Error code for `frames` on an unfinished recording

**Description:** Blocked on the spec naming a code.

**Acceptance criteria:**
- [ ] `frames` on a running recording returns the code the spec names.

**Verification:**
- [ ] `rtk proxy cargo test --test record`

**Dependencies:** The spec change
**Files likely touched:** `spec/1-recdr/spec-agent-recorder.md`, `src/control.rs`, `tests/record.rs`
**Estimated scope:** Small

### Task 12: No `expect` outside tests

**Acceptance criteria:**
- [ ] `git grep -n "expect(" -- src` shows only test code.

**Verification:**
- [ ] clippy and the tests pass.

**Dependencies:** None
**Files likely touched:** `src/encode/ffmpeg.rs`, `src/store.rs`, `src/encode/mod.rs`, `src/capture/cdp.rs`
**Estimated scope:** Small

### Task 13: Write each sample once

**Acceptance criteria:**
- [ ] `frames --every 100ms --max-width 200` on a still recording writes one sample file per stored frame.

**Verification:**
- [ ] Unit test counting files in `samples/`.

**Dependencies:** None
**Files likely touched:** `src/analyze/select.rs`
**Estimated scope:** XS

### Checkpoint: Complete
- [ ] All tests and CI green
- [ ] Spec drift items decided and the spec updated
