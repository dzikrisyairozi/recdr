# Review remediation plan: gui-overlay (PR #11)

PR: https://github.com/dzikrisyairozi/recdr/pull/11
Review: https://github.com/dzikrisyairozi/recdr/pull/11#pullrequestreview-5989268416
Reviewed head: `be52796` (`feat/6-gui-overlay`), 2026-11-20. A fresh-context `code-reviewer` read the diff, the spec, the plan and `acceptance.md`; the main session triaged its findings.

## Triage

| # | Finding | Severity | Decision |
|---|---|---|---|
| 1 | An export can write into the recording directory | Important | Fix |
| 2 | Quitting while a recording is starting leaves the recorder running | Important | Fix |
| 3 | Closing the editor during an export leaves a `.part` file | Suggestion | Fix |
| 4 | Exports to one target share a temp name; a failed preview re-enables Save | Suggestion | Fix |
| 5 | A worker thread that panics never reports back | Suggestion | Fix |
| 6 | The first stored frame is exported twice when `--from` is before it | Suggestion (reviewer) | Contest |
| 7 | The gui import rule in `tests/layers.rs` can be passed by code that breaks it | Suggestion | Fix |
| 8 | `image::open` runs on the UI thread | Suggestion | Fix |
| 9 | `export_replaces_only_on_success` does not test a failed encode | Suggestion | Fix |
| 10 | The MCP `export` tool overwrites any `.gif` or `.mp4` without asking | Suggestion | Escalate (matches the spec) |
| 11 | `worker::perform` returns `Option<Event>` only for `Quit` (over-engineering pass) | Suggestion | Fix, optional |
| 12 | `App::new` returns a vec that is always `[ListMonitors]` (over-engineering pass) | Suggestion | Fix, optional |
| 13 | `ExportCmd` copies `ExportRequest` field for field (over-engineering pass) | Suggestion | Decline |

### Contested

6. The reviewer wrote that real recordings store their first frame at the capture time, above 0, so `store::frame_at` falls back to the first frame and the `rest` filter in `export::kept` takes it again. `src/recorder.rs:62` is `writer.push(first, 0)?`: the first stored frame of every recording is at 0 ms. `frame_at(frames, from_ms)` therefore always finds a frame at or before `from_ms`, and the fallback in `src/store.rs:480` is never used by `kept`. Not posted. A manifest written by something other than the recorder could still trigger it; a one-line guard is cheap if the owner wants it.

### Declined

13. Deriving `clap::Args` on `ExportRequest` would put clap in `src/request.rs`, which layer rule 2 in `tests/layers.rs` forbids. About 30 lines are not worth changing the rule.

### Spec drift (owner decides)

- "Stored frames, the manifest, `recording.gif` and `recording.mp4` are never changed" is not enforced for a target inside the recording directory. Recommended: change the code (task 1) and add a `usage` row to the error table.
- "Closing the app while it records stops the recording first" does not hold for a quit while starting. Recommended: change the code (task 2).
- Criterion 1's 2.9 to 3.1 s band was replaced in `export_whole` by `duration_ms` in 3000..=3500 with a 5 ms per frame tolerance. Recommended: change the spec; `acceptance.md` has the reason.
- For an MP4 the spec says one pixel more for an odd size; `encode::video_size` also pads to at least 160x120. Recommended: change the spec.
- The MCP `export` tool replaces existing files, as the spec says. If MCP callers should need an `overwrite: true` argument, the spec changes first.

## Phase 1: Important

### Task 1: Refuse an export target inside the recording directory

**Description:** `export::run` returns `Error::Usage` when the canonical parent of `--file` is the recording directory or below it, before any encoder starts.

**Acceptance criteria:**
- [ ] `recdr export <id> --file <out>/<id>/recording.gif` returns `usage` and leaves `recording.gif` byte for byte the same.
- [ ] A target next to the recording directory still works.
- [ ] The MCP `export` tool returns the same error.

**Verification:**
- [ ] New test in `tests/export.rs`, with the file hashes before and after.
- [ ] `rtk proxy cargo test --test export`

**Dependencies:** None
**Files likely touched:** `src/export.rs`, `tests/export.rs`, `spec/6-gui-overlay/spec.md`
**Estimated scope:** Small

### Task 2: Stop a starting recording on exit

**Description:** `Gui::on_exit` keeps stepping the state machine until it yields `Quit`: it waits on the receiver (with a time limit) for `Started` or `Failed`, and performs the `Stop` that follows.

**Acceptance criteria:**
- [ ] In `state.rs`, the event sequence `CloseRequested`, `Started(id)` from `Idle { starting: true }` yields `Stop(id)` and then `Quit` (already true; keep the unit test).
- [ ] Quitting from the Dock while a recording is starting leaves no recorder process and a finished recording.
- [ ] `on_exit` returns within its time limit when no event arrives.

**Verification:**
- [ ] Unit test for the exit loop, driven with a channel and no window.
- [ ] Manual on macOS: Record, then Quit from the Dock within the first second; `recdr status <id>` says `finished`.
- [ ] `rtk proxy cargo test`

**Dependencies:** None
**Files likely touched:** `src/gui/app.rs`, `src/gui/state.rs`
**Estimated scope:** Small

### Checkpoint: Important
- [ ] fmt, clippy for the three targets, and tests pass
- [ ] CI green on all three systems

## Phase 2: Suggestions

### Task 3: Finish an export before the editor closes

**Acceptance criteria:**
- [ ] `CloseRequested` while `edit.exporting` sets a closing flag; `Quit` follows `Exported` or `Failed`.
- [ ] No `.part.` file is left after closing during an export.

**Verification:**
- [ ] Unit test in `src/gui/state.rs`.

**Dependencies:** None
**Files likely touched:** `src/gui/state.rs`
**Estimated scope:** Small

### Task 4: One temp file per export, and a separate failure event

**Acceptance criteria:**
- [ ] The temp name holds the process id.
- [ ] A failed preview does not clear `exporting`.

**Verification:**
- [ ] Unit test in `state.rs` for the preview failure; `tests/export.rs` still passes.

**Dependencies:** None
**Files likely touched:** `src/export.rs`, `src/gui/state.rs`, `src/gui/worker.rs`
**Estimated scope:** Small

### Task 5: Report a panicking worker

**Acceptance criteria:**
- [ ] A panic inside `worker::perform` reaches the state machine as `Event::Failed`.

**Verification:**
- [ ] Unit test around the spawn helper.

**Dependencies:** None
**Files likely touched:** `src/gui/app.rs`
**Estimated scope:** XS

### Task 6: Tighten the gui import rule

**Acceptance criteria:**
- [ ] `use crate::time::parse_time;` and `use crate::store as s;` in `src/gui/` fail `tests/layers.rs`.
- [ ] `std::time::Duration` still passes.

**Verification:**
- [ ] `rtk proxy cargo test --test layers`, plus a temporary bad import to see it fail.

**Dependencies:** None
**Files likely touched:** `tests/layers.rs`
**Estimated scope:** XS

### Task 7: Decode previews off the UI thread

**Acceptance criteria:**
- [ ] `worker::perform` decodes the PNG for `Action::Preview`, and `Event::Picture` carries the RGBA buffer.
- [ ] `ui()` no longer calls `image::open`.

**Verification:**
- [ ] Manual: play a 1080p recording in the editor; the timeline moves without stalls.

**Dependencies:** None
**Files likely touched:** `src/gui/worker.rs`, `src/gui/state.rs`, `src/gui/app.rs`
**Estimated scope:** Small

### Task 8: Test a failed encode in `export_replaces_only_on_success`

**Acceptance criteria:**
- [ ] A case where encoding fails after it starts asserts the old target bytes and that no `.part.` file remains.

**Verification:**
- [ ] `rtk proxy cargo test --test export`

**Dependencies:** None
**Files likely touched:** `tests/export.rs`
**Estimated scope:** XS

### Task 9 (optional): Smaller worker and `App::new` signatures

**Acceptance criteria:**
- [ ] `worker::perform` returns `Event`.
- [ ] `App::new` returns `App`, and `app.rs` performs `ListMonitors` once.

**Verification:**
- [ ] `rtk proxy cargo test`

**Dependencies:** Task 2 (same code in `on_exit`)
**Files likely touched:** `src/gui/worker.rs`, `src/gui/state.rs`, `src/gui/app.rs`
**Estimated scope:** XS

### Checkpoint: Complete
- [ ] CI green on all three systems
- [ ] Spec drift items decided and the spec updated
