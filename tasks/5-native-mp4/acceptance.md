# Acceptance: 5-native-mp4

Results for the checkpoints and success criteria of `spec/5-native-mp4/spec.md`, filled in as the build reaches them. Runs are on Windows 11 with ffmpeg 7.0.2 on PATH unless a line says otherwise.

## Checkpoint A

- The three commands pass after Task 4 (`evidence/T4-build.log`, `evidence/T4-test.log`).
- The MP4 of `recdr record --monitor 0 --duration 3s --format mp4` (release build): exit 0, `duration_ms` 3050. `ffprobe -count_frames` reads `h264`, 1920 by 1080, 46 frames, 3.067 s, and `ffmpeg -i <mp4> -f null -` decodes the whole file with exit 0. Decoded by ffmpeg, not watched in a player; the owner confirms by playing it.
- Kept for plan decision 13: `recdr record --monitor 0 --duration 10s --format mp4 --out <scratchpad>\mp4-kept`, recording id `20261118-160103-1aa7`, `duration_ms` 10012, 136 frames stored, 0 dropped. Motion came from an `ffplay` window showing `testsrc2` at 1280 by 720 and 30 fps, not from a window moved by hand (the build sends no mouse or keyboard input). ffmpeg's MP4 was copied to `<scratchpad>\mp4-kept\ffmpeg-kept.mp4`: 2,097,805 bytes. `<scratchpad>` is the build session's scratchpad directory on the development machine; the recording shows the owner's desktop and is not committed.

## Task 5, Windows

- criterion 11: check_mp4 took 847 ms, 800 ms and 783 ms in three runs alone (`cargo test --lib check_mp4_leaves -- --nocapture`), and 1776 ms once while other encoder tests ran in parallel. The trial is one 160 by 120 frame.
- criterion 12: 1.6 s (`cargo test --release --lib encodes_90 -- --ignored --nocapture`), 90 frames of 1920 by 1080.
- Plan decision 13: `encodes_a_kept_recording` on recording `20261118-160103-1aa7` wrote 4,893,545 bytes; ffmpeg's file of the same recording is 2,097,805 bytes, 2.33 times smaller. `ffprobe` reads both as `h264`, 1920 by 1080, 151 frames, 10.07 s; the video bit rate is 3,887,467 against 1,665,382. The frame at 5 s of each, decoded with ffmpeg, shows the same picture with no blocks or smearing. Under the rule of decision 13 (more than three times the size, or visible damage) `bit_rate` stands. Looked at as decoded frames; the owner confirms by playing both files.
- The Windows file has a second stream, `aac` at 192 kbit/s, of silence. See the plan's "Changes made while building".
- The Windows H.264 encoder on this machine (NVIDIA GeForce RTX 3060) refuses sizes under about 145 by 49, which is why every video is at least 160 by 120. See the plan's "Changes made while building".
- Rule 7: with the code line `const _: &str = "ffmpeg";` added to `src/encode/mf.rs`, `cargo test --test layers rule_7` failed with `src/encode/mf.rs contains "ffmpeg"`. Not committed.

## Checkpoint B

- Draft pull request #9, base `feat/4-macos-capture`. First CI run 44350058854.
- `windows-latest` has an H.264 encoder: `check_mp4_leaves_no_file`, `first_video_frame_has_the_picture_and_the_ring`, `mp4_is_h264_of_the_recording_length` and `mp4_with_no_ffmpeg_on_path` passed there, and `cargo test -- --test-threads=1` ended with exit 0. The change under "If the Windows runner has no encoder" is not needed.
- `ubuntu-latest` (criterion 7): with ffmpeg installed by the new step, the same four MP4 tests passed through ffmpeg.
- `macos-latest` still runs the MP4 tests through ffmpeg at this point (Task 8 not yet in); they passed.
- The Windows job of that run failed in the browser step: `animation_keeps_fifteen_frames_per_second` counted 4 dropped frames where 0 is asserted (`tests/browser.rs:191`). The same test fails on #8's Windows job (5 dropped) and passed on the Windows job of run 44350889597 of the spike branch, so it is a frame-rate flake of the hosted runner and comes from `cdp-capture`, not from this module.
- Criterion 13 for Windows by hand: not done; the owner plays the files.

## Task 7, macOS spike

`spike/avf` on branch `spike/5-avf`, draft pull request #10, run 44350889597, job `test (macos-latest)`, step `cargo run --manifest-path spike/avf/Cargo.toml`. The first attempt of the job was cancelled in the queue before any step ran; the re-run printed:

```
wrote /Users/runner/work/recdr/recdr/out.mp4 in 482.658708ms
codec avc1, 320 by 240, 1000 ms
pixel (40, 30) [233, 0, 2, 255], pixel (280, 210) [3, 0, 244, 255]
h264 true, size true, red true, blue true
```

Exit 0. The plan's "What was measured" and the `avf.rs` contract now hold the results.
