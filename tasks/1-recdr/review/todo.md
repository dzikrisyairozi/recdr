# Review remediation: agent-recorder (PR #2)

Plan: `tasks/1-recdr/review/plan.md`. PR: https://github.com/dzikrisyairozi/recdr/pull/2

## Phase 1: Important

- [ ] Task 1: Relay a stored recorder error from `await_result`
  - Finding: Important — src/control.rs:163
  - Acceptance: a detached recording whose MP4 step fails returns `encode_failed` from `stop`, not `recorder_crashed`
  - Verify: new test in `tests/record.rs`; `rtk proxy cargo test`
  - Files: src/control.rs, tests/record.rs
- [x] Task 2: Check the MCP frame limit before any sample is written
  - Finding: Important — src/mcp.rs:175
  - Acceptance: `get_frames` over the limit returns `too_many_frames` and `samples/` stays empty
  - Verify: `rtk proxy cargo test --test mcp`
  - Files: src/control.rs, src/mcp.rs, src/cli.rs, tests/mcp.rs
- [x] Task 3: Use the extended frame bounds for windows
  - Finding: Important — src/capture/windows.rs:44
  - Acceptance: `recdr windows` bounds equal the recorded frame size; a click is stored within 2 px of where it happened
  - Verify: desktop test comparing bounds and frame size; one manual click
  - Files: src/capture/windows.rs, Cargo.toml, tests/desktop.rs
- [x] Task 4: End the wrapped command when the capture loop fails
  - Finding: Important — src/recorder.rs:85
  - Acceptance: after a capture error in `record -- <cmd>`, the child process is gone
  - Verify: `rtk proxy cargo test --test record`
  - Files: src/recorder.rs, tests/record.rs
- [x] Task 5: Tell a recorder from a reused process id
  - Finding: Important — src/platform.rs:33
  - Acceptance: a state whose pid belongs to another process reports `crashed`, and `stop` does not wait
  - Verify: unit test with a wrong creation time; `rtk proxy cargo test`
  - Files: src/platform/windows.rs, src/platform/unix.rs, src/model.rs, src/control.rs, src/recorder.rs
- [ ] Checkpoint: fmt, clippy, tests, desktop tests and CI green

## Phase 2: Suggestions

- [x] Task 6: Lock around the handle-inherit steps in `spawn_detached`
  - Finding: Suggestion — src/platform.rs:60
  - Acceptance: inherit off, spawn and inherit on run under one `Mutex`
  - Verify: `rtk proxy cargo test`
  - Files: src/platform/windows.rs
- [ ] Task 7: Ask the recorder to stop when `start` times out
  - Finding: Suggestion — src/control.rs:72
  - Acceptance: after `recorder_start_failed` on a timeout, the recorder process ends
  - Verify: test with a fake source that delays its first frame
  - Files: src/control.rs, src/capture/fake.rs, tests/record.rs
- [ ] Task 8: Drop the receiver before stopping the capture
  - Finding: Suggestion — src/capture/windows.rs:259
  - Acceptance: `Drop for WgcSource` takes the receiver before `control.stop()`
  - Verify: desktop tests
  - Files: src/capture/windows.rs
- [ ] Task 9: Split GIF pieces longer than 655,350 ms
  - Finding: Suggestion — src/encode/gif.rs:24
  - Acceptance: a 900,000 ms piece gives delays that add up to 90,000 hundredths
  - Verify: unit test on the delays
  - Files: src/encode/gif.rs
- [ ] Task 10: Reject ids that are not recording ids
  - Finding: Suggestion — src/store.rs:73
  - Acceptance: `..\x` and absolute paths give `recording_not_found`
  - Verify: unit test
  - Files: src/store.rs
- [ ] Task 11: Error code for `frames` on an unfinished recording
  - Finding: Suggestion — src/control.rs:178 (needs the spec change first)
  - Acceptance: the code named in the spec is returned
  - Verify: `rtk proxy cargo test --test record`
  - Files: spec/1-recdr/spec-agent-recorder.md, src/control.rs, tests/record.rs
- [ ] Task 12: No `expect` outside tests
  - Finding: Suggestion — src/encode/ffmpeg.rs:58
  - Acceptance: `git grep -n "expect(" -- src` shows only test code
  - Verify: clippy and tests
  - Files: src/encode/ffmpeg.rs, src/store.rs, src/encode/mod.rs, src/capture/cdp.rs
- [ ] Task 13: Write each sample once
  - Finding: Suggestion — src/analyze/select.rs:38
  - Acceptance: `--every` on a still recording writes one sample per stored frame
  - Verify: unit test counting files in `samples/`
  - Files: src/analyze/select.rs
- [ ] Checkpoint: all green; spec drift items decided
