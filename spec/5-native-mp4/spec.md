# Spec: native-mp4

Module `native-mp4` of the [capability map](../1-recdr/capability-map.md). Tracking issue: https://github.com/dzikrisyairozi/recdr/issues/5, a sub-issue of #1.

Depends on `agent-recorder` ([spec](../1-recdr/spec-agent-recorder.md), pull request #2) and on `cdp-capture` ([spec](../3-cdp-capture/spec.md), issue #3). `cdp-capture` makes recdr build on Linux and macOS, sets up the three CI jobs (`windows-latest`, `ubuntu-latest`, `macos-latest`), turns `src/platform.rs` into `src/platform/`, and adds the layer rule about where system differences are decided. This spec assumes all of that, for both of its halves. It was written on 2026-11-18, before `cdp-capture` was built; the plan is written when this module's turn comes, against the code as it then is.

## Words used here

- **The first spec**: the [`agent-recorder` spec](../1-recdr/spec-agent-recorder.md). **The owner**: the person who owns the recdr repository and the machines it is tested on.
- **System encoder**: the H.264 encoder that ships with the operating system. On Windows it is reached through Media Foundation, on macOS through AVFoundation, which uses VideoToolbox.
- **Stored frame**, **manifest**, **click ring**: as in the first spec. A stored frame is a PNG that recdr wrote because the screen changed. The manifest is the JSON file that lists the stored frames, their times and the clicks. A click ring is the circle drawn at a click in a GIF or MP4, and never in a stored frame.
- **Video frame**: one picture of the finished video. Stored frames are irregular in time, so the encoder repeats them to fill a constant frame rate. (The code has a type `OutFrame` in `src/encode/mod.rs`, which is something else: one stretch of the recording during which the picture does not change. It keeps its name and its meaning.)
- **Even size**: a width and a height that are both multiples of 2. H.264 with the usual color format needs that.
- **Test recording**: a recording directory that a unit test builds by hand from pictures it chooses, with the helper `one_frame` that `src/encode/mod.rs` has today for its own tests.

## Objective

Today `--format mp4` works only when `ffmpeg` is on PATH, and fails with `ffmpeg_not_found` otherwise. Windows and macOS both ship an H.264 encoder.

After this module, on Windows and macOS recdr writes `recording.mp4` with the system encoder and never looks for or starts ffmpeg. Linux has no system encoder and keeps using ffmpeg.

What an MP4 from recdr is does not change: H.264, the recording's frame rate, the recording's length, click rings drawn in, no sound.

The user is the agent of the first spec, and its owner, who no longer has to install ffmpeg on Windows or macOS.

### User stories

- As an agent on a fresh Windows machine, I pass `--format mp4` and get a video, with nothing to install.
- As an agent on Linux without ffmpeg, I get `ffmpeg_not_found` before recording starts, as today.
- As the owner, I get the same picture from every system: same size, same length, same rings.

## Tech stack

| System | How the MP4 is written | Crates |
|---|---|---|
| Windows | `windows_capture::encoder::VideoEncoder`, which feeds a `MediaStreamSource` to the Windows `MediaTranscoder` | `windows-capture` 2.0.1, already a dependency. Nothing new |
| macOS | `AVAssetWriter` with one H.264 video input and a pixel buffer adaptor | `objc2` 0.6, `objc2-foundation` 0.3, `objc2-av-foundation` 0.3, `objc2-core-video` 0.3, `objc2-core-media` 0.3, and `objc2-core-foundation` 0.3, which `macos-capture` also uses. Licenses: MIT, or Zlib OR Apache-2.0 OR MIT |
| Linux | `ffmpeg` from PATH, as today | None |

The capability map says "VideoToolbox on macOS". `AVAssetWriter` is the macOS API that encodes with VideoToolbox and also writes the MP4 container. Using VideoToolbox directly would mean writing the container by hand.

### What was measured

On 2026-11-18, with a throwaway Rust program on Windows 11, using `windows-capture` 2.0.1 and checking the files with `ffprobe`:

1. `VideoEncoder::new` with `VideoSettingsBuilder::new(w, h).frame_rate(fps).sub_type(VideoSettingsSubType::H264)`, audio disabled, then `send_frame_buffer(&bgra, ticks)` per frame and `finish()`, writes a file ffprobe reads as `h264`, profile Main, `yuv420p`, brand `mp42`.
2. 45 frames sent at 15 per second give 45 frames and a duration of 2.9999 seconds. One frame gives a file with one frame.
3. The encoder's default codec is HEVC and its default bit rate is 15 Mbit/s, so both must be set.
4. The buffer is BGRA, and its first row is shown at the bottom of the picture. A frame must be handed over with its rows in reverse order.
5. The time passed with each frame is in units of 100 nanoseconds, so video frame `k` is sent with the time `k × 10,000,000 / fps`.
6. A size of 321 by 241 is accepted and the file reports 321 by 241.
7. 90 frames of 1920 by 1080 take 3.9 seconds to encode. The caller needs no Direct3D device and no `unsafe`.

### Not checked

- Anything on macOS. No Mac was at hand. The design for macOS is: pixel buffers in the format `kCVPixelFormatType_32BGRA`, rows from top to bottom, and video frame `k` appended with the time `k / fps` seconds. None of it was run. The pixel check is what shows whether the colors and the row order are right.
- Whether, on a Windows machine with no H.264 encoder, the failure shows when a one-frame file is finished. In `windows-capture` 2.0.1 the encoder is built without reading whether the system can transcode, and the transcode runs on another thread.
- Whether the H.264 encoder is present on a hosted `windows-latest` runner, which is Windows Server.
- Whether `AVAssetWriter` can encode H.264 on a hosted macOS runner, which is a virtual machine.
- Windows "N" editions, which ship without media features until the owner installs the Media Feature Pack.
- Which hosted runners have `ffmpeg`, which the pixel check uses to decode a frame.
- How long the check before recording takes.

The plan's first tasks run the Windows encoder and the macOS encoder in CI and answer every item but the one about N editions. For that one, the first task also looks for a Windows runner or setting without the encoder; if none is found, the item stays open and the README says so.

## Commands

No new command and no new flag. `--format mp4` and `--format gif,mp4` behave as today, with these differences:

| System | Before | After |
|---|---|---|
| Windows | Needs ffmpeg; `ffmpeg_not_found` without it | System encoder. ffmpeg is never looked for |
| macOS | Needs ffmpeg; `ffmpeg_not_found` without it | System encoder. ffmpeg is never looked for |
| Linux | Needs ffmpeg; `ffmpeg_not_found` without it | The same |

## Behavior

### What the video is

These hold on every system, so that a recording gives the same video wherever it is encoded:

- Codec H.264, color format `yuv420p`, one video track, no audio track, MP4 container.
- Constant frame rate equal to the recording's `fps`. Both system encoders would also accept a time per frame, so stored frames could be handed over as they are. The constant rate is kept on purpose: it is what the ffmpeg encoder produces today, it makes frame count and length the same on every system, and every player handles it.
- The number of video frames is `duration_ms × fps / 1000`, rounded up. Video frame `k` shows the stored frame that was current at time `k × 1000 / fps` ms, with the click rings that are visible at that time. This is the loop inside `encode::ffmpeg::encode` today, built on `encode::timeline` and `encode::render`. It moves out of the ffmpeg file so the three encoders share it; it is not written again.
- The picture has an even size. A recording whose width or height is odd gets one black column on the right or one black row at the bottom, as the ffmpeg encoder does today with its `pad` filter. The padding is done by recdr before a frame reaches any encoder, so the `pad` filter is removed from the ffmpeg arguments.
- The bit rate asked of the system encoder is `width × height × fps / 8` bits per second, and never less than 1,000,000 or more than 20,000,000. For 1920 by 1080 at 15 frames per second that is 3.9 Mbit/s. On Linux, ffmpeg keeps its own quality setting.

### Errors

Today, when MP4 is asked for, recdr checks that ffmpeg runs before it records anything, so a recording is not made and then lost to a missing encoder. That check stays, with a different question on each system:

- On Linux: does `ffmpeg` on PATH run? If not, `ffmpeg_not_found`, as today.
- On Windows and macOS: can the system encoder write a file? recdr writes a whole MP4 of one black 16 by 16 frame with it, in the system's temporary directory (the recording directory does not exist yet), and removes the file. Building the encoder and discarding it is not enough: in `windows-capture` 2.0.1, building it does not ask the system whether it can transcode. If the trial fails the error is `encode_failed`, before anything is recorded, and the message carries the system's own error text. On Windows the message of a failed trial ends with `(on Windows N editions, install the Media Feature Pack)`; no other error has that ending.

After that:

- On Windows and macOS, `ffmpeg_not_found` is never returned.
- When an encoder fails while writing the video, the error is `encode_failed`, as it is today when ffmpeg fails.
- After a failed encode the stored frames, the manifest, and a GIF that was written first all stay, as today. If the encoder had created `recording.mp4`, that file is removed, which is new on every system. An encoder that could not create the file removes nothing.

### Changes to existing behavior

- An MP4 from Windows or macOS is made by a different encoder than before, so its bytes, its bit rate and its H.264 profile differ from an ffmpeg file of the same recording. Its size in pixels, its frame count, its frame rate and its length are the same.
- On Windows and macOS the check before recording can fail with `encode_failed` where it failed with `ffmpeg_not_found`.
- On Linux two small things change: recdr pads an odd size itself and no longer asks ffmpeg to, which gives the same picture; and a partly written `recording.mp4` is removed when ffmpeg fails.
- The README's requirement "ffmpeg on PATH for MP4" applies to Linux only.

Nothing else changes.

## Project structure

```
src/encode/mod.rs        Changed. Picks the encoder for the system; receives the video-frame loop from ffmpeg.rs; pads to an even size. Its test helper one_frame learns to take a picture and a size
src/encode/ffmpeg.rs     Changed. Keeps starting ffmpeg; the loop and the pad filter leave. Compiled on Linux only
src/encode/mf.rs         New. Windows: hands video frames to windows_capture::encoder
src/encode/avf.rs        New. macOS: hands video frames to AVAssetWriter
src/control.rs           Changed. The check before recording asks encode whether MP4 can be written here
src/recorder.rs          Changed. Calls encode::mp4 in place of encode::ffmpeg::encode
tests/record.rs          Changed. See "Existing tests" under Testing strategy
tests/common/mp4.rs      New. A reader for the MP4 facts the tests need
tests/common/mod.rs      Changed. Declares mp4.rs
tests/layers.rs          Changed. The rules below
Cargo.toml, Cargo.lock   Changed. The macOS crates
README.md                Changed. ffmpeg is needed on Linux only
```

The three encoders are three ways to do one job: take video frames in order and write a file. The loop that produces video frames exists once, in `encode/mod.rs`, and each encoder file holds only what is specific to its system. `encode/mod.rs` offers two functions to the rest of recdr: `encode::check_mp4`, the check described under Errors, and `encode::mp4`, which writes the file.

Padding and reversing rows use `image::imageops`, from the `image` crate recdr already depends on. The only new pixel code is swapping red and blue, because stored frames are RGBA and both system encoders take BGRA. Rows are reversed for Windows only.

The layer rules are the ones `tests/layers.rs` holds when this module is built: those of the `cdp-capture` spec, and those of `macos-capture` if it was built first. The test looks for plain text in each source file. Changes:

- `unsafe` may also appear in `encode/avf.rs`.
- New: `windows_capture` appears only in `capture/windows.rs` and `encode/mf.rs`.
- New: `objc2_av_foundation`, `objc2_core_video` and `objc2_core_media` appear only in `encode/avf.rs`. (`macos-capture` uses other `objc2` crates in its own files, so the rule names crates, not the prefix.)
- `objc2_core_foundation` may appear in `encode/avf.rs`, because making a pixel buffer takes Core Foundation types. `macos-capture` allows that crate in `capture/tap.rs` only. Whichever of the two modules is built second widens the rule to both files.
- New: the word `ffmpeg` does not appear in `encode/mf.rs` or `encode/avf.rs`.
- `encode/mod.rs` joins the files where system differences are decided (`cfg`). After this module they are `capture/mod.rs`, `platform/mod.rs`, `encode/mod.rs` and `lib.rs`.

## Code style

As in the first spec. Every `unsafe` block in `encode/avf.rs` has a comment that says what makes it sound.

## Testing strategy

| Level | Covers | Needs |
|---|---|---|
| Unit | The video-frame loop (count, which stored frame each video frame shows), padding to an even size, the bit rate rule | Nothing. Runs on all three systems |
| Encoder tests, inside the crate | Each system's encoder on a test recording: codec, size, length, an odd size, a path that cannot be written | The system encoder on Windows and macOS; ffmpeg on Linux, where they skip without it as today |
| Integration (`tests/record.rs`) | `recdr record --format mp4` on the synthetic source of the first spec, from the command line to the file | The same |
| Pixel check, inside the crate | Colors, row order and a click ring in a frame decoded from the MP4 of a test recording | `ffmpeg` on PATH to decode. Skipped with a printed note where it is missing |
| By hand | Playing the file in the system's own player and in a browser | The owner's machines |

The synthetic source of the first spec is a white square on black, 320 by 240, with no clicks. A picture like that looks the same upside down and with red and blue swapped, so it cannot show the mistakes fact 4 makes possible. The encoder tests and the pixel check therefore use a test recording whose picture the test chooses: red in the top-left quarter, blue in the bottom-right quarter, white elsewhere, at 320 by 240 and at 321 by 241. It is 1 second long at 10 frames per second, has one stored frame at 0 ms, and one click at (240, 60) at 0 ms. A click ring is a red outline (`255, 64, 64`) 3 pixels thick with a radius of 14, shown for 400 ms, so the first video frame has it.

The pixel check decodes the first video frame to a PNG by running `ffmpeg` as a command from the test itself, in the test module of `encode/mod.rs`. It does not use `encode/ffmpeg.rs`, which is compiled on Linux only. H.264 changes colors a little, so the check allows for it: "red" means a red value over 180 with green and blue under 90, and "blue" the same with red and blue exchanged.

The tests that do not check pixels must not need ffmpeg, because the point of the module is to work without it. They read the codec, the size and the length from the MP4 file's own headers, with a reader written for the tests and no new crate. The reader is one file, `tests/common/mp4.rs`. The integration tests reach it through `tests/common/mod.rs`, and the encoder tests inside the crate include the same file with a `#[path]` attribute, so there is one copy.

The synthetic source is chosen as `tests/record.rs` does today: the environment variable `RECDR_FAKE_SOURCE=moving`, with `--monitor 0` as the target.

### Existing tests

Two tests in `tests/record.rs` change:

- `mp4_is_h264_of_the_recording_length` checks its file with `ffprobe` today and skips without it. It is rewritten to read the file's headers, so it runs on Windows and macOS with no ffmpeg installed. On Linux it still skips when ffmpeg is missing, because ffmpeg is the encoder there.
- `mp4_without_ffmpeg_is_refused_before_anything_is_written` runs recdr with a `PATH` that holds no ffmpeg and expects `ffmpeg_not_found`. It keeps that meaning on Linux. On Windows and macOS the same set-up must now succeed and write an MP4, which is also the proof that ffmpeg is not used there.

## Boundaries

The boundaries of the earlier specs stay in force. Additions:

Always:
- Keep the three CI jobs green before merging.
- Remove a partly written `recording.mp4` when an encoder fails, and the temporary file of the check before recording.

Ask first:
- A dependency that is not in the Tech stack table.
- An encoder setting that changes what the video is (codec, color format, frame rate rule).
- Hardware-specific encoder options.

Never:
- Bundle, download or build ffmpeg or any codec. On Windows and macOS only the encoder that ships with the system is used.
- Add a GPL or AGPL dependency. This rules out linking `x264`.
- Add an audio track.

## Non-goals

- A system encoder on Linux.
- HEVC, AV1, WebM or any codec other than H.264.
- A flag to choose the encoder, the bit rate or the quality.
- Falling back to ffmpeg on Windows or macOS when the system encoder fails. The owner chose this on 2026-11-18.
- Encoding while recording. The MP4 is still written after the recording ends, from the stored frames.
- Byte-for-byte equal files across systems.

## Alternatives considered

| Option | Why it lost |
|---|---|
| `IMFSinkWriter` called through the `windows` crate | It works, but takes a few hundred lines of `unsafe` COM calls. `windows-capture` already wraps the Windows encoder, is already a dependency, and was measured to do the job in fifteen safe lines. |
| VideoToolbox (`VTCompressionSession`) directly on macOS | It encodes but does not write an MP4 file. A container writer would have to be added or written. |
| A pure-Rust H.264 encoder, or `openh264` | `openh264` downloads or builds a codec library, and pure-Rust H.264 encoders are slow and young. Both systems already have a tested encoder. |
| System encoder first, ffmpeg as a fallback | Two code paths on every system, and a failure of the first would be hidden. The owner chose no fallback. |
| Keeping ffmpeg as the default and adding a flag | The module exists to remove the install step. |
| Leaving odd sizes to each encoder | Windows accepts an odd size (fact 6), ffmpeg pads, and macOS is unknown. Padding in one place gives the same picture everywhere. |

## Risks

| Risk | Mitigation |
|---|---|
| The macOS half cannot be run on the Windows machine. | Build it in a session on the owner's Mac, or accept that CI is the only place it runs. The two halves are separate files and can be separate pull requests. |
| The check before recording adds a delay to every MP4 recording on Windows and macOS. | Measure it in the first task. If it takes more than one second, the owner decides whether to keep it. |
| A hosted Windows runner may have no H.264 encoder. | First task of the plan. If it has none, the Windows MP4 tests run on the owner's machine and CI checks that the error is `encode_failed` with the stated message. |
| A virtual macOS runner may have no hardware encoder. | `AVAssetWriter` falls back to a software encoder on its own; the first macOS task confirms it in CI. |
| The bit rate rule gives a blurry or a huge file for some content. | The plan compares a real recording against the ffmpeg file of the same recording, by eye and by size, before the rule is final. The numbers live in one function. |
| `windows-capture` changes its encoder in a later version. | `Cargo.lock` pins the version and the crate's encoder is named in one file. |
| Fact 4 (rows reversed) is a property of one crate version. | The pixel check fails if it changes. |

## Success criteria

"The synthetic source" is the test source of the first spec. A "test recording" and its picture are described under Testing strategy. Each criterion is automated unless it says otherwise.

1. On Windows, with a `PATH` that holds no ffmpeg, `recdr record --format mp4 --duration 2s` on the synthetic source exits 0, and the headers of `recording.mp4` say it is an MP4 file with one H.264 video track of 320 by 240.
2. The same on macOS.
3. On Windows and macOS, the length read from the file is within one video frame of the manifest's `duration_ms`.
4. On every system, the video-frame loop gives `ceil(duration_ms × fps / 1000)` frames, and frame `k` is the stored frame current at `k × 1000 / fps` ms (unit test).
5. Padding a 321 by 241 picture gives 322 by 242 with a black last column and a black last row (unit test). On each system, the MP4 of a 321 by 241 test recording is 322 by 242 (encoder test).
6. `--format gif,mp4` writes both files on every system.
7. On Linux with ffmpeg, `recdr record --format mp4` gives an H.264 MP4 of the recording's length, as today. On Linux without ffmpeg it exits with `ffmpeg_not_found` before a recording directory is created, as today.
8. On Windows and macOS no code path returns `ffmpeg_not_found`, and the word `ffmpeg` does not appear in `encode/mf.rs` or `encode/avf.rs` (layer test).
9. Where `ffmpeg` is on PATH to decode with: in a frame decoded from the MP4 of the 320 by 240 test recording, the pixel at (40, 30) is red and the pixel at (280, 210) is blue, within the limits under Testing strategy. The pixel at (253, 60), on the ring, has a green value under 160, and the pixel at (240, 60), inside the ring, has a green value over 200. Rows in the wrong order put blue or white at (40, 30); red and blue swapped put blue there.
10. In an encoder test, when a directory named `recording.mp4` already sits where the file should go, the result is `encode_failed` and the directory is still there (on each system).
11. On Windows and macOS `encode::check_mp4` succeeds where the system encoder works, and leaves no file behind (encoder test).
12. Encoding 90 frames of 1920 by 1080 takes under 10 seconds on the owner's Windows machine (by hand; 3.9 seconds was measured).
13. By hand: an MP4 from Windows plays in the Windows media player and in Chrome, and an MP4 from macOS plays in QuickTime Player and in Safari.
14. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass on the three CI jobs, and the layer tests pass with the changed rules.
15. The README says ffmpeg is needed on Linux only.

## Decisions made while writing

- The owner chose: system encoder on Windows and macOS with no ffmpeg fallback, and ffmpeg on Linux only (2026-11-18).
- Windows uses the encoder inside `windows-capture`, because it is already there and was measured.
- macOS uses `AVAssetWriter`.
- Padding to an even size and the video-frame loop are shared code, so the three encoders cannot drift apart.
- Tests read the MP4 headers themselves, so they need no ffmpeg.
- The check before recording stays on every system, and on Windows and macOS it writes a one-frame file with the system encoder.
- The MP4 reader for tests is hand-written and kept in one file that both kinds of test use.
- Both halves are built after `cdp-capture`. Neither needs `macos-capture`.

## Open questions

1. The capability map builds this module after `macos-capture`. Since neither half needs it, should the Windows half be built right after `cdp-capture`, as its own pull request, so Windows stops needing ffmpeg sooner?
2. Is a Mac available for the macOS half (the same question as in the `macos-capture` spec)?
