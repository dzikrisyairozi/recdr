# Review remediation: native-mp4 (PR #9)

Plan: `tasks/5-native-mp4/review/plan.md`. PR: https://github.com/dzikrisyairozi/recdr/pull/9

## Phase 1: Important

- [ ] Task 1: Bound the Windows encoder's memory (blocked on the owner's choice of option)
  - Finding: Important — src/encode/mf.rs:52 (the reviewer said Critical; measured 138 MB peak for 4,500 frames of 1080p on an RTX 3060)
  - Acceptance: with a slow encoder, exporting 4,500 frames of 1920x1080 stays under 500 MB peak working set
  - Verify: repeat the measurement and save it under `tasks/5-native-mp4/evidence/`; `rtk proxy cargo test`
  - Files: src/encode/mf.rs, src/encode/mod.rs, Cargo.toml, spec/5-native-mp4/spec.md
- [x] Task 2: Give each `check_mp4` trial its own file
  - Finding: Important — src/encode/mod.rs:63
  - Acceptance: eight threads calling `check_mp4` at once all get `Ok`, and no trial file is left
  - Verify: new unit test; `rtk proxy cargo test`
  - Files: src/encode/mod.rs
- [ ] Checkpoint: fmt, clippy, tests and CI green

## Phase 2: Suggestions

- [ ] Task 3: Report the macOS encoder's own error
  - Finding: Suggestion — src/encode/avf.rs:130
  - Acceptance: a failed writer returns its `NSError` text, not a 10 s timeout
  - Verify: clippy for `aarch64-apple-darwin`; macOS tests
  - Files: src/encode/avf.rs
- [ ] Task 4: Reuse the pixel buffer for an unchanged picture
  - Finding: Suggestion — src/encode/avf.rs:137
  - Acceptance: one `CVPixelBuffer` per changed picture
  - Verify: macOS MP4 tests
  - Files: src/encode/avf.rs, src/encode/mod.rs
- [ ] Task 5: Read facts from the video track only
  - Finding: Suggestion — tests/common/mp4.rs:49
  - Acceptance: `facts` skips non-`vide` tracks and reads the track's `mdhd`
  - Verify: unit test with an audio-first `moov`
  - Files: tests/common/mp4.rs
- [x] Task 6: Say when `check_mp4_leaves_no_file` does not run
  - Finding: Suggestion — src/encode/mod.rs:530
  - Acceptance: the Linux branch prints a "not run" note
  - Verify: Linux CI log
  - Files: src/encode/mod.rs
- [x] Task 7: Remove `src/lib.rs` from the `cfg` allow list
  - Finding: Suggestion — tests/layers.rs:157
  - Acceptance: the list matches the files that hold `cfg`
  - Verify: `rtk proxy cargo test --test layers`
  - Files: tests/layers.rs
- [ ] Checkpoint: CI green; the spec states the memory bound
