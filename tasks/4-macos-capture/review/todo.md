# Review remediation: macos-capture (PR #8)

Plan: `tasks/4-macos-capture/review/plan.md`. PR: https://github.com/dzikrisyairozi/recdr/pull/8

## Phase 1: Important

- [x] Task 1: Make the tap's stop impossible to lose
  - Finding: Important — src/capture/tap.rs:73
  - Acceptance: `Drop for ClickTap` stops the run loop until the thread has finished; `start_then_drop_returns` passes 1,000 times in a row on a Mac
  - Verify: loop the unit test on macOS; macOS desktop tests; macOS CI job
  - Files: src/capture/tap.rs
- [ ] Checkpoint: macOS CI job green; Windows and Linux unchanged

## Phase 2: Suggestions

- [x] Task 2: Keep a `--region` size check in macOS CI
  - Finding: Suggestion — .github/workflows/ci.yml:34
  - Acceptance: CI runs a `--region` recording that asserts 400x300; only the color test is skipped
  - Verify: macOS CI log
  - Files: tests/desktop.rs, .github/workflows/ci.yml, spec/4-macos-capture/spec.md
- [x] Task 3: Report a failed display query
  - Finding: Suggestion — src/capture/macos.rs:125
  - Acceptance: a non-zero `CGGetActiveDisplayList` status gives `capture_failed`
  - Verify: clippy for `aarch64-apple-darwin`; macOS desktop tests
  - Files: src/capture/macos.rs
- [x] Task 4: Make two desktop tests match their criteria
  - Finding: Suggestion — tests/desktop.rs:263, tests/desktop.rs:368, tests/desktop.rs:477
  - Acceptance: `process` equals Chrome's application name; the monitor size comes from an independent source; criterion 3 reads the GIF or the spec names `duration_ms`
  - Verify: macOS desktop tests
  - Files: tests/desktop.rs, spec/4-macos-capture/spec.md
- [x] Task 5 (optional): One `desktop` alias per system — declined: Linux needs a stub `open` and both list functions anyway, so it saves about 15 lines in macOS code that can only be checked by clippy here
  - Finding: Suggestion — src/capture/mod.rs:142
  - Acceptance: `desktop_source`, `list_windows` and `list_monitors` each have one body
  - Verify: clippy for the three targets; `tests/layers.rs`
  - Files: src/capture/mod.rs, src/capture/windows.rs, src/capture/macos.rs
- [x] Task 6 (optional): Have the Windows hook send `Press`
  - Finding: Suggestion — src/capture/hook.rs:19
  - Acceptance: `RawClick` is gone
  - Verify: Windows desktop tests
  - Files: src/capture/hook.rs, src/capture/windows.rs
- [ ] Checkpoint: CI green on all three systems; spec drift items decided
