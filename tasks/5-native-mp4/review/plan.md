# Review remediation plan: native-mp4 (PR #9)

PR: https://github.com/dzikrisyairozi/recdr/pull/9
Review: https://github.com/dzikrisyairozi/recdr/pull/9#pullrequestreview-5989267902
Reviewed head: `0c11db2` (`feat/5-native-mp4`), 2026-11-20. A fresh-context `code-reviewer` read the diff, the spec, the plan and `acceptance.md`; the main session triaged its findings. Every finding is in the same code at the stack top.

## Triage

| # | Finding | Severity | Decision |
|---|---|---|---|
| 1 | The Windows encoder's frame queue has no bound | Critical (reviewer), Important after measuring | Escalate |
| 2 | `check_mp4` uses one temp file name per process, and MCP calls run in parallel | Important | Fix |
| 3 | On macOS a background encoder failure is reported as a 10 s timeout | Suggestion | Fix |
| 4 | On macOS every frame allocates and converts a pixel buffer, also for an unchanged picture | Suggestion | Fix |
| 5 | The test MP4 reader does not check that the track is video | Suggestion | Fix |
| 6 | `check_mp4_leaves_no_file` skips on Linux without a note | Suggestion | Fix |
| 7 | `tests/layers.rs:157` allows `cfg` in `src/lib.rs`, which has none (over-engineering pass) | Suggestion | Fix |

### Finding 1: what was confirmed and what was measured

Confirmed in the source of `windows-capture` 2.0.1 (`src/encoder.rs`): `send_frame_buffer` at line 1258 does `buffer.to_vec()` and sends on a channel created with `mpsc::channel` (lines 918 and 1067), then returns. `mf::Writer::push` (`src/encode/mf.rs:44-55`) does not wait.

The reviewer estimated a peak of 2,500 to 3,500 queued frames (20 to 30 GB) for a 5-minute 1080p recording. Measured on this machine (Windows 11, RTX 3060, release build): a recording with one 1920x1080 frame and `duration_ms` of 300,000 was exported with `recdr export <id> --file out.mp4`. 4,500 video frames, 44.1 s, peak working set 138 MB, output 107.8 MB. With a hardware encoder the queue stays short, so the estimate does not hold here.

Not measured: a machine with no hardware H.264 encoder (a VM, a CI runner) or a 4K recording. There the producer can be faster than the encoder, and the queue is still unbounded. That is why this stays Important and goes to the owner.

Options:
1. Patch `windows-capture` so both channels are `sync_channel(8)`, through `[patch.crates-io]` or an upstream change. Smallest change; a fork to carry.
2. Drive `IMFSinkWriter` through the `windows` crate. `WriteSample` throttles. This brings back the `unsafe` code the spec rejected.
3. Accept it, and record the limit in the spec with a measurement on a machine with no hardware encoder.

### Spec drift (owner decides)

- The spec does not say what happens when frames are handed over faster than they are encoded, and sets no memory limit. Recommended: state a bound and how the Windows encoder meets it.
- The spec says `one_frame` "learns to take a picture and a size"; the code adds `recording(name, image, duration_ms, fps, clicks)` and keeps `one_frame` as a wrapper. Recommended: fix the spec's wording.
- The plan named a pixel-buffer pool on macOS; the code creates a buffer per frame. `plan.md:50` records it. Task 4 covers the cost.

## Phase 1: Important

### Task 1: Bound the Windows encoder's memory (blocked on the owner's choice)

**Description:** Whichever option the owner picks, `mf::Writer::push` must not let more than a fixed number of frames wait in memory. Independently, `video_frames` passes a `changed` flag so the Windows writer converts to BGRA only when the picture changes.

**Acceptance criteria:**
- [ ] With the encoder slowed artificially (or on a machine with no hardware encoder), exporting 4,500 frames of 1920x1080 keeps the peak working set under 500 MB.
- [ ] The existing MP4 tests pass unchanged.

**Verification:**
- [ ] Repeat the measurement above and record time and peak working set in `tasks/5-native-mp4/evidence/`.
- [ ] `rtk proxy cargo test`

**Dependencies:** Owner decision
**Files likely touched:** `src/encode/mf.rs`, `src/encode/mod.rs`, `Cargo.toml`, `spec/5-native-mp4/spec.md`
**Estimated scope:** Small (option 1) to Medium (option 2)

### Task 2: Give each `check_mp4` trial its own file

**Description:** The trial path gets a per-process counter (`recdr-check-<pid>-<n>.mp4`). If two Media Foundation encoders at once fail by themselves, hold a `static Mutex` around the trial as well.

**Acceptance criteria:**
- [ ] Eight threads calling `check_mp4` at once all get `Ok`.
- [ ] No `recdr-check-<pid>-*` file is left afterwards.

**Verification:**
- [ ] New unit test with eight threads; update `check_mp4_leaves_no_file`.
- [ ] `rtk proxy cargo test`

**Dependencies:** None
**Files likely touched:** `src/encode/mod.rs`
**Estimated scope:** XS

### Checkpoint: Important
- [ ] fmt, clippy and tests pass on Windows
- [ ] CI green on all three systems

## Phase 2: Suggestions

### Task 3: Report the macOS encoder's own error

**Acceptance criteria:**
- [ ] `avf::Writer::push` returns the writer's `NSError` text when `status()` is `Failed` during the wait.

**Verification:**
- [ ] `cargo clippy --all-targets --target aarch64-apple-darwin -- -D warnings`; macOS tests

**Dependencies:** A Mac to run it
**Files likely touched:** `src/encode/avf.rs`
**Estimated scope:** XS

### Task 4: Reuse the pixel buffer for an unchanged picture

**Acceptance criteria:**
- [ ] A still recording creates one `CVPixelBuffer` per changed picture, not per video frame.

**Verification:**
- [ ] macOS MP4 tests; frame count unchanged

**Dependencies:** The `changed` flag from task 1
**Files likely touched:** `src/encode/avf.rs`, `src/encode/mod.rs`
**Estimated scope:** Small

### Task 5: Read facts from the video track only

**Acceptance criteria:**
- [ ] `facts` skips a `trak` whose `hdlr` handler type is not `vide` and reads the duration from that track's `mdhd`.

**Verification:**
- [ ] Unit test with a hand-built `moov` whose first track is audio.

**Dependencies:** None
**Files likely touched:** `tests/common/mp4.rs`
**Estimated scope:** Small

### Task 6: Say when `check_mp4_leaves_no_file` does not run

**Acceptance criteria:**
- [ ] The Linux branch prints a "not run" note before returning.

**Verification:**
- [ ] Linux CI log shows the note.

**Dependencies:** None
**Files likely touched:** `src/encode/mod.rs`
**Estimated scope:** XS

### Task 7: Remove `src/lib.rs` from the `cfg` allow list

**Acceptance criteria:**
- [ ] The list in `tests/layers.rs` matches the files that hold `cfg`, and the test's name matches the count.

**Verification:**
- [ ] `rtk proxy cargo test --test layers`

**Dependencies:** None
**Files likely touched:** `tests/layers.rs`
**Estimated scope:** XS

### Checkpoint: Complete
- [ ] CI green on all three systems
- [ ] The spec states the memory bound
