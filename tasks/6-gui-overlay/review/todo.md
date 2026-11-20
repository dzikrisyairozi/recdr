# Review remediation: gui-overlay (PR #11)

Plan: `tasks/6-gui-overlay/review/plan.md`. PR: https://github.com/dzikrisyairozi/recdr/pull/11

## Phase 1: Important

- [x] Task 1: Refuse an export target inside the recording directory
  - Finding: Important — src/export.rs:39
  - Acceptance: `--file` inside the recording directory returns `usage` and changes no file there; the MCP tool returns the same error
  - Verify: new test in `tests/export.rs`; `rtk proxy cargo test --test export`
  - Files: src/export.rs, tests/export.rs, spec/6-gui-overlay/spec.md
- [x] Task 2: Stop a starting recording on exit
  - Finding: Important — src/gui/app.rs:151
  - Acceptance: quitting while a recording is starting leaves no recorder process; `on_exit` returns within its time limit
  - Verify: unit test for the exit loop; manual Quit from the Dock on macOS
  - Files: src/gui/app.rs, src/gui/state.rs
- [ ] Checkpoint: fmt, clippy for three targets, tests and CI green

## Phase 2: Suggestions

- [x] Task 3: Finish an export before the editor closes
  - Finding: Suggestion — src/gui/state.rs:398
  - Acceptance: closing during an export quits after `Exported` or `Failed`, with no `.part.` file left
  - Verify: unit test in `src/gui/state.rs`
  - Files: src/gui/state.rs
- [ ] Task 4: One temp file per export, and a separate failure event
  - Finding: Suggestion — src/export.rs:48, src/gui/state.rs:387
  - Acceptance: the temp name holds the process id; a failed preview does not clear `exporting`
  - Verify: unit test in `state.rs`; `tests/export.rs`
  - Files: src/export.rs, src/gui/state.rs, src/gui/worker.rs
- [ ] Task 5: Report a panicking worker
  - Finding: Suggestion — src/gui/app.rs:75
  - Acceptance: a panic in `worker::perform` becomes `Event::Failed`
  - Verify: unit test
  - Files: src/gui/app.rs
- [ ] Task 6: Tighten the gui import rule
  - Finding: Suggestion — tests/layers.rs:147
  - Acceptance: `crate::time` and aliased imports in `src/gui/` fail the test; `std::time` passes
  - Verify: `rtk proxy cargo test --test layers`
  - Files: tests/layers.rs
- [ ] Task 7: Decode previews off the UI thread
  - Finding: Suggestion — src/gui/app.rs:133
  - Acceptance: `ui()` no longer calls `image::open`
  - Verify: manual playback of a 1080p recording
  - Files: src/gui/worker.rs, src/gui/state.rs, src/gui/app.rs
- [ ] Task 8: Test a failed encode in `export_replaces_only_on_success`
  - Finding: Suggestion — tests/export.rs:300
  - Acceptance: a failed encode leaves the old target and no `.part.` file
  - Verify: `rtk proxy cargo test --test export`
  - Files: tests/export.rs
- [ ] Task 9 (optional): Smaller worker and `App::new` signatures
  - Finding: Suggestion — src/gui/worker.rs:12, src/gui/state.rs:126
  - Acceptance: `perform` returns `Event`; `App::new` returns `App`
  - Verify: `rtk proxy cargo test`
  - Files: src/gui/worker.rs, src/gui/state.rs, src/gui/app.rs
- [ ] Checkpoint: CI green; spec drift items decided
