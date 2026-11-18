# Tasks: native-mp4

Plan: [plan.md](plan.md). Spec: [spec/5-native-mp4/spec.md](../../spec/5-native-mp4/spec.md). "Contracts" means the Module contracts section of the plan. Every signature, constant, message and rule named below is defined there, and a task implements it exactly as written. "Criterion N" means success criterion N of the spec.

## Before Task 1

1. `cdp-capture` is merged, or the work branch is cut from its branch. Cut `feat/5-native-mp4` and commit `spec/5-native-mp4/` and `tasks/5-native-mp4/` as one commit.
2. Check what this plan took from the cdp plan, and what the cdp plan may have changed under it. Each must hold; if one does not, edit the plan first.
   - `.github/workflows/ci.yml` has one job with the matrix `windows-latest`, `ubuntu-latest`, `macos-latest` and a `cargo test` step, and runs on pull requests.
   - `tests/layers.rs` has `forbid` and `source_files`, and a test that forbids `cfg(` outside a list of files. That list is where `src/encode/mod.rs` will be added.
   - `src/control.rs` still tests `encode::ffmpeg::available()` in `prepare`, and `src/recorder.rs` still calls `encode::ffmpeg::encode` in `finalize`. A search for `ffmpeg::` under `src/` finds no other use outside `src/encode/`.
   - `tests/record.rs` still has `mp4_is_h264_of_the_recording_length` and `mp4_without_ffmpeg_is_refused_before_anything_is_written`, and the helpers `record` and `tool_runs`.
   - `tests/common/mod.rs` still starts with `#![allow(dead_code)]`.
   - `src/store.rs` still has `RecordingDir::create`, `RecordingDir::open`, `read_manifest`, `frame_writer`, `FrameWriter::push`, `load_frame` and `mp4_path` with the signatures the tasks use.
3. `ffmpeg -version` runs on the development machine. Phase 1 needs it.

Every task ends with these three commands passing, then one commit that stages the task's files by path:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Phase 1: shared code, tested with ffmpeg

- [x] **Task 1: a test recording with a chosen picture, and the MP4 reader**
  - Description: The two test tools every later task uses. No production code changes.
  - Steps:
    - `src/encode/mod.rs`: the test helper `recording`, and `one_frame` as a call to it.
    - `tests/common/mp4.rs`: `Mp4Facts` and `facts`. `tests/common/mod.rs`: `pub mod mp4;`.
    - `tests/record.rs`: `const NEEDS_FFMPEG: bool = true;`; `mp4_is_h264_of_the_recording_length` with the new skip and note, reading `common::mp4::facts` in place of running `ffprobe`; the new test `facts_refuses_a_file_that_is_not_an_mp4`.
  - Acceptance:
    - With ffmpeg on PATH `mp4_is_h264_of_the_recording_length` passes and asserts a codec of `avc1` or `avc3`, 320 by 240, and a duration within 67 ms of the result's `duration_ms`.
    - `facts_refuses_a_file_that_is_not_an_mp4` passes, which means `facts` panicked.
    - The existing callers of `one_frame` compile unchanged.
  - Verify: `cargo test --test record mp4`, `cargo test --test record facts`, `cargo test --lib encode`.
  - Dependencies: none
  - Files: `src/encode/mod.rs`, `tests/common/mp4.rs`, `tests/common/mod.rs`, `tests/record.rs`
  - Scope: S

- [x] **Task 2: the video-frame loop and even-size padding**
  - Description: The loop and the padding as functions of `src/encode/mod.rs` with unit tests. `ffmpeg::encode` calls them, so behavior is the same.
  - Steps:
    - `src/encode/mod.rs`: `video_frames` and `even`, as `pub(crate)` until Task 3 makes `write` their only caller and they become private.
    - `src/encode/ffmpeg.rs`: `encode` calls `video_frames` and passes ffmpeg the even size; the `-vf pad=...` arguments are removed. Its closure writes `image.as_raw()` to stdin. On the first write error it sets a local `broken = true`; from then on it writes nothing; it always returns `Ok(())`. `encode` then reports ffmpeg's exit status and stderr as today, so a broken pipe still shows ffmpeg's own message.
  - Acceptance:
    - `one_call_per_video_frame`: a test recording of 1000 ms at 10 fps gives 10 calls numbered 0 to 9; one of 1001 ms gives 11 (criterion 4).
    - `a_call_gets_the_picture_of_its_time`: with stored frames at 0 and 250 ms and 10 fps, calls 0 to 2 get the first picture and calls 3 onward the second (criterion 4). The test does not use `recording`, which stores one frame and finishes its writer. It makes its own directory with `RecordingDir::create`, pushes a black frame at 0 ms and a white one at 250 ms through one `dir.frame_writer`, and builds the `Manifest` from the two entries. The frames must differ, because `FrameWriter::push` stores nothing for a frame equal to the one before it.
    - `even_adds_a_black_column_and_row`: `even` of a 321 by 241 picture is 322 by 242, its pixel (321, 0) and its pixel (0, 241) are `[0, 0, 0, 255]`, and its pixel (320, 240) equals the original's. `even` of a 320 by 240 picture is the same picture (criterion 5).
    - `mp4_is_h264_of_the_recording_length` still passes.
  - Verify: `cargo test --lib encode`, `cargo test --test record mp4`.
  - Dependencies: Task 1
  - Files: `src/encode/mod.rs`, `src/encode/ffmpeg.rs`
  - Scope: S

- [x] **Task 3: `encode::mp4`, `encode::check_mp4` and the ffmpeg writer**
  - Description: The shape the system encoders will plug into, with ffmpeg as the only writer.
  - Steps:
    - `src/encode/ffmpeg.rs`: `Writer` with `create`, `push`, `finish`; `available` becomes `pub(super)`; `encode` and its test are removed.
    - `src/encode/mod.rs`: `mod ffmpeg;` is no longer `pub`; `use ffmpeg::Writer;`; the file-level include of `tests/common/mp4.rs`; `bit_rate`, `write`, `mp4`, and `check_mp4` with only its ffmpeg body; `video_frames` and `even` become private. In the test module: `ffmpeg_runs`, `const NEEDS_FFMPEG: bool = true;`, and the tests below.
    - `src/control.rs` and `src/recorder.rs`: the two calls in Contracts.
    - Search `src/` for `ffmpeg::`; the only hits are inside `src/encode/`.
  - Acceptance:
    - `bit_rate_is_clamped`: `bit_rate(1920, 1080, 15)` is 3_888_000, `bit_rate(320, 240, 15)` is 1_000_000 and `bit_rate(3840, 2160, 60)` is 20_000_000.
    - `no_picture_fails_before_the_encoder`: `mp4` on a test recording whose manifest has width 0 returns `encode_failed`, with ffmpeg on PATH or not.
    - `odd_size_is_padded`: the MP4 of a 321 by 241 test recording has the facts 322 by 242 (criterion 5).
    - `a_directory_in_the_way_is_left_alone`: when `dir.mp4_path()` is a directory, `mp4` returns `encode_failed` and the directory still exists (criterion 10).
    - `a_failed_encode_leaves_no_file`: the test adds to the manifest a second entry, `FrameEntry { index: 2, t_ms: 500, file: "frames/missing.png".into() }`, so the first frames are pushed and `render` then fails. `mp4` returns an error, and `dir.mp4_path()` does not exist (criterion 10).
    - `tests/record.rs` passes unchanged, including `mp4_without_ffmpeg_is_refused_before_anything_is_written`.
    - The three tests that write an MP4 start with the skip of Contracts.
  - Verify: `cargo test --lib encode`, `cargo test --test record`.
  - Dependencies: Task 2
  - Files: `src/encode/mod.rs`, `src/encode/ffmpeg.rs`, `src/control.rs`, `src/recorder.rs`
  - Scope: M

- [x] **Task 4: the pixel check**
  - Description: The test that catches reversed rows and exchanged colors, passing on ffmpeg before any system encoder exists.
  - Steps:
    - `src/encode/mod.rs`, test module: `quarters`, `first_frame` (which asks `ffmpeg_runs()`, as Contracts says), and the test `first_video_frame_has_the_picture_and_the_ring`.
  - Acceptance:
    - The test builds `recording("pixels", quarters(320, 240), 1000, 10, vec![click])` with a left click at (240, 60) at 0 ms, runs `mp4`, and decodes the first frame.
    - It asserts criterion 9: at (40, 30) red over 180 with green and blue under 90; at (280, 210) blue over 180 with red and green under 90; at (253, 60) green under 160; at (240, 60) green over 200.
    - Shown to fail when it should: with `quarters` temporarily flipped top to bottom the test fails at (40, 30). The temporary change is not committed.
    - Without ffmpeg the test prints a note and passes.
  - Verify: `cargo test --lib first_video_frame`.
  - Dependencies: Task 3
  - Files: `src/encode/mod.rs`
  - Scope: S

### Checkpoint A: nothing changed for the user
- [x] The three commands pass on the development machine with ffmpeg on PATH.
- [x] By hand: the MP4 of `recdr record --monitor 0 --duration 3s --format mp4` plays. (Decoded in full by ffmpeg; the owner confirms by playing it. See `acceptance.md`.)
- [x] By hand, for plan decision 13: the owner runs `recdr record --monitor 0 --duration 10s --format mp4 --out <a directory to keep>` while moving a window about, and copies the `mp4` of the result to a file outside the recording directory. The output directory and the recording's id are noted in `acceptance.md`.

## Phase 2: Windows

- [ ] **Task 5: the Windows writer, and Windows records MP4 with no ffmpeg**
  - Description: `src/encode/mf.rs`, selected on Windows. From this commit Windows no longer starts ffmpeg to encode, so the test that expected `ffmpeg_not_found` on every system changes in the same commit.
  - Steps:
    - `src/encode/mf.rs`: `Writer`, as in Contracts.
    - `src/encode/mod.rs`: the `cfg` lines for Windows; `ffmpeg` under `cfg(not(windows))`; the Windows body of `check_mp4` with `HINT`; in the test module `NEEDS_FFMPEG` becomes `cfg!(not(windows))`. New tests: `check_mp4_leaves_no_file`, `finish_with_no_frame_returns`, and the two `#[ignore]` tests `encodes_90_large_frames_in_time` and `encodes_a_kept_recording`.
    - `tests/layers.rs`: the rule 2 line for `windows_capture`, `src/encode/mod.rs` in rule 6, and rule 7.
    - `tests/record.rs`: `NEEDS_FFMPEG` becomes `cfg!(not(windows))`; `mp4_without_ffmpeg_is_refused_before_anything_is_written` becomes `mp4_with_no_ffmpeg_on_path` with its two bodies.
  - Acceptance:
    - On Windows every encoder test of Tasks 3 and 4 passes through `mf.rs` and none skips: 322 by 242 for the odd size, the directory left alone, no file after a failed encode, and the pixel check (criteria 5, 9, 10).
    - `check_mp4_leaves_no_file` (it returns at once where `NEEDS_FFMPEG`): `check_mp4()` is `Ok(())`, and the temporary directory has no `recdr-check-<pid>.mp4` afterwards. It prints the time the call took (criterion 11). The time goes into `acceptance.md` as the line `criterion 11: check_mp4 took <n> ms`; over one second is reported to the owner (spec, Risks).
    - `finish_with_no_frame_returns`: a recording whose only frame entry names a missing file. `mp4` runs on a second thread, and the test fails if it has not returned an error within 10 seconds (plan, Risks).
    - On Windows, with a `PATH` that is an empty directory, `mp4_with_no_ffmpeg_on_path` passes: exit 0, H.264, 320 by 240 (criterion 1), and a duration within 67 ms of `duration_ms` (criterion 3). `mp4_is_h264_of_the_recording_length` records with `--format gif,mp4` and finds both files (criterion 6).
    - `cargo test --test layers` passes; rule 7 fails when the word `ffmpeg` is put into `src/encode/mf.rs` (tried, not committed).
    - By hand, criterion 12: `cargo test --release --lib encodes_90 -- --ignored` passes. The test builds `recording("large", quarters(1920, 1080), 6000, 15, Vec::new())`, times `mp4`, prints the time and asserts it is under 10 seconds. The time goes into `acceptance.md` as `criterion 12: <n> s`.
    - By hand, plan decision 13: `encodes_a_kept_recording` reads the environment variables `RECDR_KEPT_OUT` and `RECDR_KEPT_ID`, and returns with a printed note when either is not set. Otherwise it opens the recording with `RecordingDir::open`, reads its manifest, runs `mp4`, and prints the size of the file in bytes. The owner runs it on the recording kept at Checkpoint A with `cargo test --release --lib encodes_a_kept -- --ignored --nocapture`, plays the new file and the copy of ffmpeg's, and writes both sizes and what was seen into `acceptance.md`. The rule of decision 13 says whether the build goes on.
  - Verify: `cargo test --lib encode`, `cargo test --test layers`, `cargo test --test record mp4`.
  - Dependencies: Task 4
  - Files: `src/encode/mf.rs`, `src/encode/mod.rs`, `tests/layers.rs`, `tests/record.rs`, `tasks/5-native-mp4/acceptance.md`
  - Scope: M

- [ ] **Task 6: CI installs ffmpeg where it is needed**
  - Description: Linux gets its encoder in CI, and macOS gets a decoder for the pixel check.
  - Steps:
    - `.github/workflows/ci.yml`: the two install steps of Contracts, before the `cargo test` step.
  - Acceptance:
    - The two steps are before `cargo test`, with the indentation of the steps around them.
    - The effect, and that GitHub accepts the file, is checked at Checkpoint B.
  - Verify: the three commands locally; Checkpoint B.
  - Dependencies: Task 5
  - Files: `.github/workflows/ci.yml`
  - Scope: XS

### Checkpoint B: Windows is done (needs a pull request)
- [ ] The owner pushes the branch and opens a draft pull request. The build stops and asks.
- [ ] The `test` job is green on the three runners. On `ubuntu-latest` the log shows the MP4 tests ran and printed no `skipped` note (criterion 7).
- [ ] Record in `acceptance.md` whether `windows-latest` has an H.264 encoder. If it has none, make the change under "If the Windows runner has no encoder" in the plan, as its own commit, and push again.
- [ ] By hand: criterion 13 for Windows.

## Phase 3: macOS

- [ ] **Task 7: try `AVAssetWriter`, and write the results into this plan**
  - Description: A try-out program on the branch `spike/5-avf`, which is never merged (plan decision 11).
  - Steps:
    - On `spike/5-avf`: a cargo project in `spike/avf/` with the macOS dependencies of Contracts and `image`. Its `main` writes `out.mp4`, one second at 10 frames per second of a 320 by 240 picture with a red top-left quarter and a blue bottom-right quarter, with the calls listed for `avf.rs` in Contracts. It then reads the file with `tests/common/mp4.rs` (included with `#[path]`), runs `ffmpeg -y -i out.mp4 -frames:v 1 out.png`, loads the PNG, prints the codec, the size and the pixels at (40, 30) and (280, 210), and exits 1 when the codec is not H.264, the size is not 320 by 240, the first pixel is not red or the second not blue.
    - With a Mac: `cargo run --manifest-path spike/avf/Cargo.toml` there. Without one: on that branch add to `.github/workflows/ci.yml`, after the ffmpeg install step, a step with `if: runner.os == 'macOS'` and `run: cargo run --manifest-path spike/avf/Cargo.toml`; the owner opens a draft pull request from the branch, and closes it unmerged when the task is done.
    - Record in the plan, under "What was measured": the Rust calls that compile, the cargo features each crate needs, whether `finishWriting` is available, the pixel format and row order that give the right picture, and whether a hosted `macos-latest` runner encodes H.264.
    - Replace the Objective-C list for `avf.rs` in Contracts with the Rust calls.
  - Acceptance:
    - The program exits 0, on the Mac or in the CI run.
    - The plan's `avf.rs` section names Rust calls only.
    - If `finishWriting` is not available, the build stops and asks about `block2` (decision 9).
  - Verify: the program's output, pasted into `acceptance.md`.
  - Dependencies: Checkpoint B
  - Files: `tasks/5-native-mp4/plan.md`, `tasks/5-native-mp4/acceptance.md` (on the work branch; the program is on `spike/5-avf` only)
  - Scope: S

- [ ] **Task 8: the macOS writer**
  - Description: `src/encode/avf.rs`, selected on macOS.
  - Steps:
    - `Cargo.toml`, `Cargo.lock`: the macOS dependencies.
    - `src/encode/avf.rs`: `Writer`, with the calls Task 7 wrote into Contracts.
    - `src/encode/mod.rs`: the `cfg` lines for macOS; `ffmpeg` under `cfg(not(any(windows, target_os = "macos")))`; `check_mp4` runs the trial on macOS with an empty `HINT`; `NEEDS_FFMPEG` in the test module becomes `cfg!(not(any(windows, target_os = "macos")))`.
    - `tests/layers.rs`: the two `objc2` lines of rule 2 and `src/encode/avf.rs` in rule 3.
    - `tests/record.rs`: `NEEDS_FFMPEG` becomes `cfg!(not(any(windows, target_os = "macos")))`.
  - Acceptance:
    - On macOS the encoder tests pass through `avf.rs` and none skips (criteria 5, 9, 10, 11), and the two MP4 tests in `tests/record.rs` pass (criteria 2, 3, 6).
    - `cargo test --test layers` passes on the three systems (criterion 8).
    - The Windows and Linux jobs are unchanged and green.
  - Verify: `cargo test` on a Mac, or the `macos-latest` job of the pull request.
  - Dependencies: Task 7
  - Files: `Cargo.toml`, `Cargo.lock`, `src/encode/avf.rs`, `src/encode/mod.rs`, `tests/layers.rs`, `tests/record.rs`
  - Scope: M

### Checkpoint C: macOS is done (needs the pull request)
- [ ] The `test` job is green on `macos-latest`, and its log shows the pixel check ran and printed no note about a missing ffmpeg.
- [ ] By hand, on the owner's Mac if there is one: criterion 13 for macOS.

## Phase 4: the rest

- [ ] **Task 9: README and the acceptance run**
  - Description: The README says where ffmpeg is needed, and every criterion has a recorded result.
  - Steps:
    - `README.md`: ffmpeg is needed for MP4 on Linux only; Windows and macOS use the system encoder; Windows N editions need the Media Feature Pack; what is still unchecked from the spec's "Not checked" list.
    - `tasks/5-native-mp4/acceptance.md`: one line per criterion, 1 to 15, with the test name or the by-hand result.
  - Acceptance:
    - Criteria 14 and 15 hold.
    - Every criterion in `acceptance.md` has a result or a stated reason it could not be run.
  - Verify: the three commands, and the three CI jobs.
  - Dependencies: Task 8
  - Files: `README.md`, `tasks/5-native-mp4/acceptance.md`
  - Scope: S
