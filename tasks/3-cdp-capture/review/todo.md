# Review remediation: cdp-capture (PR #7)

Plan: `tasks/3-cdp-capture/review/plan.md`. PR: https://github.com/dzikrisyairozi/recdr/pull/7

## Phase 1: Important

- [x] Task 1: Bound what the click binding accepts
  - Finding: Important — src/capture/cdp.rs:347
  - Acceptance: an off-frame or huge press yields no click; a flood of binding calls is capped and frames keep arriving
  - Verify: unit tests on `click()`; browser test with a page that calls `__recdrClick` in a loop
  - Files: src/capture/cdp.rs, tests/browser.rs, spec/3-cdp-capture/spec.md
- [ ] Task 2: Bound the held frames and count the ones dropped
  - Finding: Important — src/capture/cdp.rs:240
  - Acceptance: at most four frames wait; `dropped()` reports the rest; criterion 4 still holds
  - Verify: unit test with more than four decoded frames; `animation_keeps_fifteen_frames_per_second`
  - Files: src/capture/cdp.rs, spec/3-cdp-capture/spec.md
- [ ] Checkpoint: fmt, clippy, tests, browser tests and CI green

## Phase 2: Suggestions

- [ ] Task 3: Stamp frames when they arrive
  - Finding: Suggestion — src/capture/cdp.rs:247 (spec drift, code side)
  - Acceptance: a click and the frame showing the page's reaction are within one frame interval
  - Verify: browser test with a page that changes color on mousedown
  - Files: src/capture/cdp.rs, src/model.rs, src/recorder.rs, src/capture/windows.rs, src/capture/macos.rs, src/capture/fake.rs
- [ ] Task 4: One deadline per connect step, and stop a late recorder
  - Finding: Suggestion — src/capture/cdp.rs:465, src/control.rs:101
  - Acceptance: a dripping listener fails within 6 s; a recorder that `start` gave up on ends
  - Verify: test with a local `TcpListener`; `rtk proxy cargo test`
  - Files: src/capture/cdp.rs, src/control.rs, tests/record.rs
- [ ] Task 5: Keep the port form on loopback
  - Finding: Suggestion — src/capture/cdp.rs:508
  - Acceptance: `--cdp <port>` refuses a `webSocketDebuggerUrl` with a remote host
  - Verify: test with a local listener that names a remote host
  - Files: src/capture/cdp.rs
- [ ] Task 6: Make a used retry visible, and give `dropped_frames` a way to fail
  - Finding: Suggestion — tests/browser.rs:449
  - Acceptance: a short first attempt prints `::warning::` in CI; the animation test asserts on the real drop counter
  - Verify: force a short first attempt locally
  - Files: tests/browser.rs, tests/record.rs, .github/workflows/ci.yml
- [ ] Task 7: Move the frame's base64 text
  - Finding: Suggestion — src/capture/cdp.rs:170
  - Acceptance: `handle` takes the `data` string without copying it
  - Verify: browser tests
  - Files: src/capture/cdp.rs
- [ ] Checkpoint: all green; the spec's frame-rate paragraph and "Clicks" section match the code
