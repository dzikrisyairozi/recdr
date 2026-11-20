# Review remediation plan: cdp-capture (PR #7)

PR: https://github.com/dzikrisyairozi/recdr/pull/7
Review: https://github.com/dzikrisyairozi/recdr/pull/7#pullrequestreview-5989267087
Reviewed head: `b84ac93` (`feat/3-cdp-capture`), 2026-11-20. A fresh-context `code-reviewer` read the diff, the spec and the plan; the main session triaged its findings. `src/capture/cdp.rs` is the same at the stack top, so the line numbers hold there too.

## Triage

| # | Finding | Severity | Decision |
|---|---|---|---|
| 1 | The recorded page can flood the click binding and send coordinates far outside the frame | Important | Fix |
| 2 | `held` and the decoder channel have no bound | Important | Fix |
| 3 | Frame times are read on release, not on arrival | Suggestion | Fix (spec drift, code side) |
| 4 | Connect time limits apply per read; a late Allow click orphans the recorder | Suggestion | Fix |
| 5 | The port form connects to whatever host `/json/version` names | Suggestion | Fix |
| 6 | A used test retry leaves no trace; `dropped_frames == 0` cannot fail for a tab | Suggestion | Fix |
| 7 | Each frame's base64 text is copied | Suggestion | Fix |
| 8 | `Endpoint::Port` duplicates `Endpoint::Http` (over-engineering pass) | Suggestion | Decline |

### Notes on the triage

2. The reviewer rated this the same as finding 1. It is kept Important but ranked lower: a normal Chrome never has more than three frames waiting, and the growth needs a page that reloads more often than `--fps` times a second or an endpoint that ignores acks. The reviewer read this from the code and did not measure it; neither did the main session.

8. Declined because task 5 needs to know that the port form was used in order to require a loopback host.

### Spec drift (owner decides)

- The spec's frame-rate paragraph (`spec/3-cdp-capture/spec.md:145`) says the older of two waiting frames is skipped and counted in `dropped_frames`. Since the CI fixes the code holds frames (`hold_frames`) and the browser waits. Recommended: change the spec to describe holding, and say that the bound from task 2 is what `dropped_frames` counts.
- The spec says a frame's time is read when it arrives. Recommended: change the code (task 3).
- The spec limits connecting to 5 s and the first reply to 30 s in total. Recommended: change the code (task 4).
- "Clicks" does not say what happens to a press outside the viewport. Recommended: say "dropped" and match it (task 1).

## Phase 1: Important

### Task 1: Bound what the click binding accepts

**Description:** `click()` drops a press whose scaled point is outside the frame, and `handle` stops accepting presses past a per-recording cap (10,000) or closer together than 20 ms.

**Acceptance criteria:**
- [ ] A payload with `x: 1e300` yields no click.
- [ ] 100,000 binding calls in a loop leave at most the cap in `manifest.json`, and frames keep arriving meanwhile.
- [ ] A press inside the viewport is stored as before.

**Verification:**
- [ ] Unit tests on `click()` in `src/capture/cdp.rs` for off-frame and huge values.
- [ ] Browser test: a page script calls `__recdrClick` in a loop during a 2 s recording; assert the click count and `frames_stored >= 10`.
- [ ] `rtk proxy cargo test --test browser -- --ignored --test-threads=1`

**Dependencies:** None
**Files likely touched:** `src/capture/cdp.rs`, `tests/browser.rs`, `spec/3-cdp-capture/spec.md`
**Estimated scope:** Small

### Task 2: Bound the held frames and count the ones dropped

**Description:** `CdpSource` keeps at most four decoded frames waiting, drops the oldest beyond that, counts them, and reports the count from `dropped()`. The handler does not send a frame to the decoder while two are already decoding, but still records its id for the ack.

**Acceptance criteria:**
- [ ] `held.len()` never exceeds 4.
- [ ] `dropped()` returns the number of frames removed.
- [ ] A normal recording still reports `dropped_frames` of 0 and at least 100 frames in 10 s (criterion 4).

**Verification:**
- [ ] Unit test feeding more than four decoded frames.
- [ ] `animation_keeps_fifteen_frames_per_second` passes without its second try.

**Dependencies:** None
**Files likely touched:** `src/capture/cdp.rs`, `spec/3-cdp-capture/spec.md`
**Estimated scope:** Small

### Checkpoint: Important
- [ ] `rtk proxy cargo fmt --check`, clippy and `cargo test` pass
- [ ] The browser tests pass on Windows
- [ ] CI is green on all three systems

## Phase 2: Suggestions

### Task 3: Stamp frames when they arrive

**Description:** `handle` records `time::now_unix_ms()` for each `Page.screencastFrame`, the time stays with the frame in `held`, and `CaptureEvent::Frame` carries it. The recorder converts it the way it converts clicks and marks. This also covers what is left of PR #2's "frame times at dequeue" finding.

**Acceptance criteria:**
- [ ] A click and the frame that shows the page's reaction to it are within one frame interval in the manifest.

**Verification:**
- [ ] Browser test: a page that changes color on mousedown; compare the click time with the first frame of the new color.

**Dependencies:** Task 2 (same queue)
**Files likely touched:** `src/capture/cdp.rs`, `src/model.rs`, `src/recorder.rs`, `src/capture/windows.rs`, `src/capture/macos.rs`, `src/capture/fake.rs`
**Estimated scope:** Medium

### Task 4: One deadline per connect step, and stop a late recorder

**Description:** `version_url` and the WebSocket handshake keep one deadline and set the read timeout to what is left before each read. When `start` gives up on a recorder that is still alive, it calls `dir.request_stop()` first (shared with task 7 of `tasks/1-recdr/review/plan.md`).

**Acceptance criteria:**
- [ ] A listener that sends one byte every 4 s makes `recdr tabs` fail within 6 s.
- [ ] After `recorder_start_failed` from a timeout, the recorder process ends.

**Verification:**
- [ ] Test with a local `TcpListener` that drips bytes.
- [ ] `rtk proxy cargo test`

**Dependencies:** None
**Files likely touched:** `src/capture/cdp.rs`, `src/control.rs`, `tests/record.rs`
**Estimated scope:** Small

### Task 5: Keep the port form on loopback

**Acceptance criteria:**
- [ ] For `--cdp <port>`, a `webSocketDebuggerUrl` whose host is not `127.0.0.1`, `localhost` or `[::1]` gives `cdp_connect_failed`.

**Verification:**
- [ ] Test with a local listener that answers `/json/version` with a remote host.

**Dependencies:** None
**Files likely touched:** `src/capture/cdp.rs`
**Estimated scope:** XS

### Task 6: Make a used retry visible, and give `dropped_frames` a way to fail

**Acceptance criteria:**
- [ ] A first attempt that falls short prints `::warning::` and the CI steps run with `--show-output`.
- [ ] After task 2, the animation test asserts on the real drop counter.

**Verification:**
- [ ] Force a short first attempt locally and see the line in the output.

**Dependencies:** Task 2
**Files likely touched:** `tests/browser.rs`, `tests/record.rs`, `.github/workflows/ci.yml`
**Estimated scope:** XS

### Task 7: Move the frame's base64 text

**Acceptance criteria:**
- [ ] `handle` takes the `data` string out of the message without copying it.

**Verification:**
- [ ] The browser tests pass.

**Dependencies:** None
**Files likely touched:** `src/capture/cdp.rs`
**Estimated scope:** XS

### Checkpoint: Complete
- [ ] All tests and CI green
- [ ] The frame-rate paragraph and the "Clicks" section of the spec match the code
