# Implementation plan: native-mp4

Spec: [spec/5-native-mp4/spec.md](../../spec/5-native-mp4/spec.md). Task list: [todo.md](todo.md). Tracking issue: https://github.com/dzikrisyairozi/recdr/issues/5.

All paths are relative to the repository root. "The spec" means the file above. "The cdp plan" means [tasks/3-cdp-capture/plan.md](../3-cdp-capture/plan.md).

## Overview

On Windows and macOS `--format mp4` is written by the encoder that ships with the system, and ffmpeg is used on other systems only. The work has three parts:

1. The code that is the same for every encoder leaves `src/encode/ffmpeg.rs`: the loop that turns a recording into video frames at a constant rate, padding to an even size, and the choice of encoder. ffmpeg becomes one of three writers with the same three functions.
2. A Windows writer, `src/encode/mf.rs`, on `windows_capture::encoder::VideoEncoder`.
3. A macOS writer, `src/encode/avf.rs`, on `AVAssetWriter`.

Part 1 is built and tested with ffmpeg, which works today, so every test of the video (frame count, size, colors, a click ring) exists and passes before a system encoder is written. Each system encoder then has to pass the same tests.

### When this plan was written

On 2026-11-18, before `cdp-capture` was built. The code it describes as existing is of two kinds:

- **Exists today and is not touched by `cdp-capture`** (checked by reading the files on branch `feat/1-agent-recorder`): everything under `src/encode/`, and `src/draw.rs`.
- **Exists today and is changed by `cdp-capture`**: `src/store.rs`, `src/control.rs`, `src/recorder.rs`, `tests/record.rs`, `tests/common/mod.rs`, `tests/layers.rs`. This plan uses a few named things in each, and the cdp plan does not change those: `RecordingDir::create`, `RecordingDir::open`, `read_manifest`, `frame_writer`, `FrameWriter::push`, `load_frame` and `mp4_path` in `src/store.rs` (the cdp plan changes one test there), the ffmpeg test in `control::prepare`, the encoder call in `recorder::finalize`, the two MP4 tests and the helpers `record` and `tool_runs` in `tests/record.rs`, the `#![allow(dead_code)]` at the top of `tests/common/mod.rs`, and `forbid` and `source_files` in `tests/layers.rs`.
- **Exists after `cdp-capture`** (taken from the cdp plan, not from code): `.github/workflows/ci.yml` with one job `test` on three runners, which runs on pull requests and on pushes to `main`; and the sixth layer test, which forbids `cfg(`, `cfg!(` and `cfg_attr(`, other than `cfg(test)`, outside three files. The `#[cfg(test)]` include that Task 3 adds to `src/encode/mod.rs` is therefore allowed before Task 5 puts that file on the list.

"Before Task 1" in [todo.md](todo.md) checks the second and third lists against the real code. A difference is fixed in this plan before any task starts.

### What was measured

On 2026-11-18 on Windows 11 with `windows-capture` 2.0.1 (the spec lists the results as facts 1 to 7). Read from the crate's `src/encoder.rs` for this plan:

- `VideoSettingsBuilder::new(width: u32, height: u32)`, with `.sub_type(VideoSettingsSubType)`, `.bitrate(u32)` and `.frame_rate(u32)`.
- `AudioSettingsBuilder::default().disabled(true)` and `ContainerSettingsBuilder::default()`.
- `VideoEncoder::new(video, audio, container, path) -> Result<VideoEncoder, VideoEncoderError>`.
- `send_frame_buffer(&mut self, buffer: &[u8], timestamp: i64) -> Result<(), VideoEncoderError>`. The first timestamp it gets becomes time zero.
- `finish(self) -> Result<(), VideoEncoderError>`.

Nothing was run on macOS. Task 7 does that first and writes what it finds into this plan.

## Architecture decisions

Each is a choice the spec left to the plan.

1. **Three writers with the same three functions, picked by `cfg`. No trait.** `ffmpeg.rs`, `mf.rs` and `avf.rs` each define `pub struct Writer` with `create`, `push` and `finish`. `src/encode/mod.rs` imports the one for the system it is compiled for. Only one exists in any build, so a trait would have one implementation per build.
2. **The loop calls a closure per video frame.** `video_frames` in `src/encode/mod.rs` is the loop that is in `ffmpeg::encode` today. It renders a piece of the timeline once, pads it, and hands the same picture to the closure for every video frame that shows it.
3. **"Linux" means every system that is not Windows or macOS.** The ffmpeg writer is compiled under `cfg(not(any(windows, target_os = "macos")))`, so recdr on a BSD also has MP4.
4. **A recording with no picture fails before any encoder is started.** `encode::mp4` returns `encode_failed` for a width or height of 0. Today that case is left to ffmpeg, and its test needs ffmpeg installed.
5. **A failed encode removes `recording.mp4` only when it is a file.** `encode::mp4` checks `path.is_file()` after a failure. A directory in the way is left alone, which is what success criterion 10 sets up.
6. **The check before recording uses a writer the same way a recording does.** `check_mp4` creates a writer on a file in the temporary directory, pushes one black 160 by 120 frame, finishes and removes the file. No second code path into the system encoder exists.
7. **The MP4 reader for tests is one file used from two places.** `tests/common/mp4.rs` is declared in `tests/common/mod.rs` for the integration tests, and included in the unit tests of `src/encode/mod.rs` with `#[path]`. It uses only the standard library.
8. **CI installs ffmpeg on the Linux and macOS runners.** On Linux it is the encoder, and without it no MP4 test runs there. On macOS it decodes the frame for the pixel check, which is the only automated proof of colors and row order on that system. The Windows runner does not get it: the pixel check for Windows runs on the development machine.
9. **The macOS writer finishes with `finishWriting`.** It is the call that blocks until the file is complete. It is deprecated in favor of a call that takes a completion block, which would need the `block2` crate. Task 7 tries it first. If the bindings do not offer it, adding `block2` is an ask-first change to the spec's Tech stack table.
10. **A writer is always finished, also after a failure.** `write` calls `finish` whether the frames were pushed or not, so the encoder has let go of the file before `mp4` removes it. A writer that is only dropped leaves ffmpeg running, or a system encoder's thread holding the file, and on Windows an open file cannot be removed.
11. **A try-out program lives on a branch that is never merged.** Task 7's program is a cargo project in `spike/avf/` on the branch `spike/5-avf`, cut from the work branch. With a Mac it is run there. Without one, a draft pull request from that branch runs it in CI through one step added to `ci.yml` on that branch, and the pull request is closed unmerged. Nothing on that branch is under this plan's File boundary, because none of it reaches the work branch.
12. **CI runs on pull requests.** A checkpoint that needs CI needs the work branch pushed and a draft pull request open. Both are the owner's to do.
13. **The bit rate rule is compared once with ffmpeg's file, by hand.** The spec asks for this before the rule is final. At Checkpoint A the owner keeps a 10 second recording and the MP4 ffmpeg wrote for it. In Task 5 an `#[ignore]` test encodes the same recording with the Windows writer, and the owner plays both and compares their sizes. The rule stands unless the system's file is more than three times the size of ffmpeg's, or shows blocks or smearing that ffmpeg's does not. In either case the build stops, and `bit_rate` is changed in this plan before Task 5 is committed.

### Spec changes made while planning

None.

### Changes made while building

- Task 5, approved by the owner on 2026-11-18: every video is at least 160 by 120. On the development machine (NVIDIA GeForce RTX 3060) the Windows encoder failed with `0xC00D6D60` ("A valid type has not been set for this stream") for 16 by 16, 64 by 64, 96 by 96, 128 by 64, 64 by 128, 128 by 128, 144 by 144, 64 by 320, 320 by 32 and 32 by 320, and wrote 160 by 120, 176 by 144, 192 by 192, 200 by 200, 240 by 240, 256 by 256 and 320 by 64. `windows-capture` 2.0.1 always switches hardware encoding on (`MediaTranscoder::SetHardwareAccelerationEnabled(true)` in its `encoder.rs`), and the limits fit NVENC's minimum of 145 by 49. So the plan's 16 by 16 trial in `check_mp4` failed on every run. `even` became `padded` plus `video_size`, which also pads up to 160 by 120; the trial uses 160 by 120; the spec's "Encoding" section says so. The padding applies to every writer, so the rule is tested on all three systems.
- Task 5, approved by the owner on 2026-11-18: on Windows the MP4 has a silent AAC track besides the video track. `windows-capture` 2.0.1 adds it whatever the audio settings say, and has no setting to leave it out. `ffprobe` of the 10 second file of decision 13 shows stream 0 `h264` and stream 1 `aac` at 192 kbit/s. The spec's "What the video is" says so; `tests/common/mp4.rs` already reads the first track with a width and skips the audio track.

## Module contracts

A task implements these exactly as written. Anything not listed keeps its current signature.

### `src/encode/mod.rs`

After Task 8 the top of the file is:

```rust
pub mod gif;

#[cfg(target_os = "macos")]
mod avf;
#[cfg(not(any(windows, target_os = "macos")))]
mod ffmpeg;
#[cfg(windows)]
mod mf;

#[cfg(target_os = "macos")]
use avf::Writer;
#[cfg(not(any(windows, target_os = "macos")))]
use ffmpeg::Writer;
#[cfg(windows)]
use mf::Writer;
```

In Tasks 1 and 2 it is as today, `pub mod ffmpeg; pub mod gif;`. From Task 3 until Task 5 it is `pub mod gif; mod ffmpeg; use ffmpeg::Writer;` with no `cfg`. Task 5 adds the Windows lines and puts `ffmpeg` under `cfg(not(windows))`. Task 8 adds the macOS lines.

From Task 3 the file also has, at file level and not inside `mod tests`:

```rust
#[cfg(test)]
#[allow(dead_code)]
#[path = "../../tests/common/mp4.rs"]
mod mp4;
```

At file level the path is read from `src/encode/`, so it reaches `tests/common/mp4.rs`. Inside `mod tests` it would be read from `src/encode/tests/` and miss. The unit tests call `super::mp4::facts`.

Functions:

```rust
/// Writes `recording.mp4` (H.264) with the encoder this system uses.
pub fn mp4(dir: &RecordingDir, manifest: &Manifest) -> Result<PathBuf>

/// Fails when an MP4 cannot be written on this machine, before anything is recorded.
pub fn check_mp4() -> Result<()>

/// Calls `each` once per video frame, in order, with the frame's number and its picture.
/// Frame `k` is the picture on screen at `k * 1000 / fps` ms, padded to `video_size`.
/// Returns the number of frames.
fn video_frames(
    dir: &RecordingDir,
    manifest: &Manifest,
    each: impl FnMut(u64, &RgbaImage) -> Result<()>,
) -> Result<u64>

/// The picture with black columns on the right and rows at the bottom up to `video_size`.
fn padded(image: RgbaImage) -> RgbaImage

/// The size of the video for a recording of `width` by `height`: even, and at least 160 by 120.
fn video_size(width: u32, height: u32) -> (u32, u32)

/// Bits per second asked of a system encoder.
fn bit_rate(width: u32, height: u32, fps: u32) -> u32

/// Writes one video file through this system's writer.
fn write(dir: &RecordingDir, manifest: &Manifest, path: &Path) -> Result<()>
```

- `video_frames`: `fps = u64::from(manifest.fps.max(1))`, `frames = (manifest.duration_ms * fps).div_ceil(1000)`, and for each `k` the piece is found with `pieces.iter().rposition(|p| p.t_ms <= k * 1000 / fps)`, or piece 0 when there is none, exactly as lines 65 to 86 of `src/encode/ffmpeg.rs` do today. The rendered and padded picture is kept while the piece does not change. An empty timeline returns `Ok(0)` without calling `each`.
- `video_size`: `((width + width % 2).max(160), (height + height % 2).max(120))`. 160 by 120 is the smallest size recdr asks of an encoder: on the development machine (NVIDIA RTX 3060) the Windows encoder refused 144 by 144 and 320 by 32, and wrote 160 by 120 and 320 by 64. Changed in Task 5, see "Changes made while building".
- `padded`: when the picture already has `video_size` it is returned as it is. Otherwise a picture of `video_size`, filled with `Rgba([0, 0, 0, 255])`, gets the original at (0, 0) with `image::imageops::replace`.
- `bit_rate`: `(u64::from(width) * u64::from(height) * u64::from(fps) / 8).clamp(1_000_000, 20_000_000) as u32`.
- `write`: the size is `video_size(manifest.width, manifest.height)`, and `fps = manifest.fps.max(1)`. It calls `Writer::create(path, width, height, fps, bit_rate(width, height, fps))`, then `let pushed = video_frames(dir, manifest, |k, image| writer.push(k, image));`, then `let finished = writer.finish();`, and returns `pushed.and(finished)`: `finish` always runs (decision 10), and an error from the frames wins over one from `finish`.
- `mp4`: a width or height of 0 returns `Error::EncodeFailed("the recording has no picture".into())`. Otherwise `path = dir.mp4_path()` and `write(dir, manifest, &path)`. When `write` fails and `path.is_file()`, the file is removed (an error from the removal is ignored) and the error from `write` is returned.
- `check_mp4` has one definition until Task 5 and two from then on, each under a `cfg` attribute: one where ffmpeg is the encoder, one for Windows and macOS.
- `check_mp4`, where ffmpeg is the encoder: `Ok(())` when `ffmpeg::available()`, else `Err(Error::FfmpegNotFound)`.
- `check_mp4`, on Windows and macOS: the path is `std::env::temp_dir().join(format!("recdr-check-{}.mp4", std::process::id()))`. The trial is `Writer::create(&path, 160, 120, 1, 1_000_000)` and, when that worked, `let pushed = writer.push(0, &black);` with a 160 by 120 picture of `Rgba([0, 0, 0, 255])`, `let finished = writer.finish();` and `pushed.and(finished)`, so a created writer is always finished (decision 10). Then `let _ = std::fs::remove_file(&path);`, whatever the trial returned: the file may not exist, and an error from the removal is ignored. An `Error::EncodeFailed(message)` from the trial is returned as `Error::EncodeFailed(format!("{message}{hint}"))`, with `let hint = if cfg!(windows) { " (on Windows N editions, install the Media Feature Pack)" } else { "" };` inside the function. No constant holds the text, so nothing is unused on a system that does not compile this definition. A writer returns no other variant.

Test helpers, under `#[cfg(test)]`:

```rust
/// A recording directory with one stored frame at 0 ms showing `image`.
pub(crate) fn recording(
    name: &str,
    image: RgbaImage,
    duration_ms: u64,
    fps: u32,
    clicks: Vec<crate::model::Click>,
) -> (RecordingDir, Manifest)

/// As today: one solid black 64x48 frame, 1000 ms, 15 fps.
pub(crate) fn one_frame(name: &str, clicks: Vec<crate::model::Click>) -> (RecordingDir, Manifest)
```

`one_frame` becomes a call to `recording`, so its callers in `src/encode/gif.rs` and `src/encode/mod.rs` do not change (the one in `src/encode/ffmpeg.rs` goes with its test in Task 3). In the test module:

```rust
/// Red in the top-left quarter, blue in the bottom-right quarter, white elsewhere.
fn quarters(width: u32, height: u32) -> RgbaImage
/// The first frame of a video, decoded by ffmpeg. None, with a printed note, when ffmpeg does not run.
fn first_frame(mp4: &Path) -> Option<RgbaImage>
/// True when `ffmpeg -version` runs and exits 0.
fn ffmpeg_runs() -> bool
/// True where ffmpeg is the encoder: `true` until Task 5, then `cfg!(not(windows))`,
/// and from Task 8 `cfg!(not(any(windows, target_os = "macos")))`.
const NEEDS_FFMPEG: bool
```

Every unit test that writes an MP4 begins `if NEEDS_FFMPEG && !ffmpeg_runs() { println!("skipped: ffmpeg not on PATH"); return; }`. On Windows from Task 5, and on macOS from Task 8, such a test never skips. The pixel test then encodes, and passes with a printed note when `first_frame` is `None`.

`quarters`: a pixel is `[255, 0, 0, 255]` when `x < width / 2 && y < height / 2`, `[0, 0, 255, 255]` when `x >= width / 2 && y >= height / 2`, and `[255, 255, 255, 255]` otherwise. `first_frame` runs `ffmpeg -y -loglevel error -i <mp4> -frames:v 1 <mp4 with the extension png>` and loads the PNG with `image::open`.

### The three writers

Each of `src/encode/ffmpeg.rs`, `src/encode/mf.rs` and `src/encode/avf.rs` has:

```rust
pub struct Writer { /* private */ }

impl Writer {
    /// Starts a video of `width` by `height` (both even) at `fps` frames per second.
    pub fn create(path: &Path, width: u32, height: u32, fps: u32, bit_rate: u32) -> Result<Self>
    /// Adds video frame `k`. Frames arrive in order, starting at 0.
    pub fn push(&mut self, k: u64, image: &RgbaImage) -> Result<()>
    /// Completes the file.
    pub fn finish(self) -> Result<()>
}
```

Every failure is `Error::EncodeFailed` with the underlying error's text. That includes I/O errors, which a writer maps with `.map_err(|e| Error::EncodeFailed(e.to_string()))` and does not pass on with `?`. `finish` returns only when the encoder has closed the file, also when it reports a failure.

**`ffmpeg.rs`.** `available()` stays, as `pub(super) fn`. `create` starts the process that `encode` starts today, with three differences: `-s` is the size it was given, the two arguments `-vf` and `pad=ceil(iw/2)*2:ceil(ih/2)*2` are gone, and the parameter is named `_bit_rate` because ffmpeg keeps its own quality setting. The struct holds the child, its stdin, the thread that reads stderr, and `broken: bool`. `push` writes `image.as_raw()` to stdin; the first write error sets `broken`, and a push after that does nothing and returns `Ok(())`, because ffmpeg's own message is better than a broken-pipe error. `finish` drops stdin, waits (a failure to wait is `EncodeFailed` with the I/O error's text), joins the stderr thread and returns `EncodeFailed(format!("ffmpeg exited with {status}: {}", errors.trim()))` for a failed status, as today. The function `encode` and the test `a_zero_size_recording_fails_to_encode` are removed; the test's case is covered by `mp4` (decision 4).

**`mf.rs`.** The struct holds the `VideoEncoder` and `fps: u64`.

- `create`: `VideoEncoder::new(VideoSettingsBuilder::new(width, height).sub_type(VideoSettingsSubType::H264).bitrate(bit_rate).frame_rate(fps), AudioSettingsBuilder::default().disabled(true), ContainerSettingsBuilder::default(), path)`.
- `push`: `let mut bgra = image::imageops::flip_vertical(image).into_raw();`, then `for pixel in bgra.chunks_exact_mut(4) { pixel.swap(0, 2); }`, then `send_frame_buffer(&bgra, (k * 10_000_000 / self.fps) as i64)`.
- `finish`: `self.encoder.finish()`.
- No `unsafe`.

**`avf.rs`.** The Objective-C calls, in order. Task 7 replaces this list with the Rust calls it found to compile and run.

- `create`: an `AVAssetWriter` for the file URL with the file type `AVFileTypeMPEG4`. An `AVAssetWriterInput` for video with the settings `AVVideoCodecKey` = `AVVideoCodecTypeH264`, `AVVideoWidthKey`, `AVVideoHeightKey`, and `AVVideoCompressionPropertiesKey` holding `AVVideoAverageBitRateKey`; `expectsMediaDataInRealTime` is false. An `AVAssetWriterInputPixelBufferAdaptor` whose buffer attributes are the pixel format `kCVPixelFormatType_32BGRA`, the width and the height. Then `startWriting` and `startSessionAtSourceTime` with time zero. Before any of this, when `path.is_file()`, the file is removed, because `AVAssetWriter` refuses to replace one. A directory at the path is not removed; the writer then fails, which is the case of criterion 10.
- `push`: wait in steps of 1 ms while the input's `isReadyForMoreMediaData` is false, for at most 10 seconds; after that the error is `EncodeFailed("the encoder did not take a frame within 10 seconds".into())`. Take a buffer from the adaptor's pool, lock it, copy the picture row by row from top to bottom with red and blue exchanged, honoring the buffer's bytes per row, unlock it, and append it with the time `CMTimeMake(k, fps)`. The struct counts the frames it has appended.
- `finish`: `markAsFinished` on the input, `endSessionAtSourceTime` with `CMTimeMake(count, fps)`, then `finishWriting`. A writer status other than completed is a failure with the writer's `error` text.
- Every `unsafe` block has a comment that says what makes it sound.

### `src/control.rs`

In `prepare`, the `if` that tests `encode::ffmpeg::available()` and returns `Error::FfmpegNotFound` becomes:

```rust
if options.formats.contains(&Format::Mp4) {
    encode::check_mp4()?;
}
```

### `src/recorder.rs`

In `finalize`, `encode::ffmpeg::encode(dir, &manifest)?` becomes `encode::mp4(dir, &manifest)?`.

These are the only two uses of `encode::ffmpeg` outside `src/encode/`. Task 3 confirms it with a search for `ffmpeg::` under `src/`.

### `tests/common/mp4.rs`

```rust
pub struct Mp4Facts {
    /// The four characters that name the video codec, `avc1` for H.264.
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
}

/// Reads the facts from an MP4 file's headers. Panics with the path when the file is not an MP4.
pub fn facts(path: &Path) -> Mp4Facts

/// The facts, or None when the bytes are not an MP4 with a video track.
fn parse(bytes: &[u8]) -> Option<Mp4Facts>

/// The content of every box named `name` directly inside `content`, in order.
fn boxes<'a>(content: &'a [u8], name: &[u8; 4]) -> Vec<&'a [u8]>
```

`facts` reads the whole file and returns `parse(&bytes).unwrap_or_else(|| panic!("{} is not an MP4 file", path.display()))`. A file that cannot be read panics with the path too.

An MP4 file is a sequence of boxes: a 4-byte big-endian size, a 4-byte name, then the content; a size of 1 means an 8-byte size follows the name. `boxes` walks them and stops at a box whose size is smaller than its header or runs past the end of `content`. Every read in the file goes through `slice::get`, so bytes that are not an MP4 give `None` and never an index panic. `parse`:

1. Requires the first top-level box to be named `ftyp`.
2. Takes the first top-level box named `moov`.
3. In `moov`, reads the first `mvhd`: one version byte and three flag bytes; for version 0 the time scale is the `u32` at content offset 12 and the duration the `u32` at 16; for version 1 the time scale is the `u32` at 20 and the duration the `u64` at 24. `duration_ms = duration * 1000 / time scale`, and a time scale of 0 gives `None`.
4. For each `trak` in `moov`, in order, takes the first `mdia`, in it the first `minf`, then `stbl`, then `stsd`; a track that lacks one of them is skipped. The content of `stsd` is 4 bytes of version and flags, a 4-byte entry count, then the first entry: a 4-byte size, the 4-byte codec name, and at offsets 32 and 34 of the entry the width and the height as `u16`.
5. A track whose first entry has a width of 0 is not video and is skipped. The first track left gives the codec, the width and the height; with none left the result is `None`.

`tests/common/mod.rs` gains `pub mod mp4;`. `src/encode/mod.rs` includes the same file for its unit tests, with the lines shown under its heading above.

A test accepts a codec of `avc1` or `avc3` as H.264.

### `tests/record.rs`

A constant at the top of the file says where ffmpeg is the encoder: `const NEEDS_FFMPEG: bool = true;` in Task 1, `cfg!(not(windows))` from Task 5, and `cfg!(not(any(windows, target_os = "macos")))` from Task 8.

- `mp4_is_h264_of_the_recording_length`: today its skip is `!tool_runs("ffmpeg") || !tool_runs("ffprobe")` with the note `skipped: ffmpeg or ffprobe not on PATH`. The skip becomes `NEEDS_FFMPEG && !tool_runs("ffmpeg")` with the note `skipped: ffmpeg not on PATH`, and the `ffprobe` call goes. It asserts, from `common::mp4::facts`: an H.264 codec, 320 by 240, and a duration within `1000 / 15 + 1` ms (67) of the result's `duration_ms`.
- `mp4_without_ffmpeg_is_refused_before_anything_is_written` is renamed `mp4_with_no_ffmpeg_on_path` in Task 5 and has two bodies chosen with `NEEDS_FFMPEG`. Its command changes from `--duration 1s` to `--duration 2s`, which is what criterion 1 names. Where `NEEDS_FFMPEG` is true the body is otherwise the test of today. Where it is false the same command must exit 0, and the facts of the `mp4` in its result are an H.264 codec, 320 by 240, and a duration within 67 ms of the result's `duration_ms`.
- `facts_refuses_a_file_that_is_not_an_mp4`, new in Task 1 and marked `#[should_panic]`: it writes the ten bytes `not an mp4` to a file under `out_dir("not-mp4")` and calls `common::mp4::facts` on it.

### `tests/layers.rs`

Starting from the six tests of the cdp plan:

| Rule | Change | Task |
|---|---|---|
| 2 | add `forbid(&["windows_capture"], &["src/capture/windows.rs", "src/encode/mf.rs"])` | 5 |
| 2 | add `forbid(&["objc2_av_foundation", "objc2_core_video", "objc2_core_media"], &["src/encode/avf.rs"])` | 8 |
| 2 | `objc2_core_foundation`: when a rule for it exists (from `macos-capture`), `src/encode/avf.rs` joins its allowed list; otherwise add `forbid(&["objc2_core_foundation"], &["src/encode/avf.rs"])` | 8 |
| 3 | `src/encode/avf.rs` joins the files that may hold `unsafe` | 8 |
| 6 | `src/encode/mod.rs` joins the files that may hold `cfg` | 5 |
| 7 (new) | `rule_7_system_encoders_do_not_name_ffmpeg`: for each of `src/encode/mf.rs` and `src/encode/avf.rs` that exists, its code does not contain `ffmpeg` | 5 |

`forbid` skips nothing by name except its allowed list, so rule 7 is its own loop over `source_files()`, like rule 4.

### `.github/workflows/ci.yml`

Two steps before `cargo test`, added in Task 6:

```yaml
- if: runner.os == 'Linux'
  run: sudo apt-get update && sudo apt-get install -y ffmpeg
- if: runner.os == 'macOS'
  run: brew install ffmpeg
```

### `Cargo.toml`

Task 8 adds, with the versions the spec's Tech stack table gives:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-foundation = "0.3"
objc2-av-foundation = "0.3"
objc2-core-video = "0.3"
objc2-core-media = "0.3"
objc2-core-foundation = "0.3"
```

When `macos-capture` was built first the table already exists and has `objc2-core-foundation`; the other five lines are added to it. The cargo features each crate needs for the calls above are found in Task 7 and written here.

## File boundary

Create:
- `src/encode/mf.rs`
- `src/encode/avf.rs`
- `tests/common/mp4.rs`
- `tasks/5-native-mp4/acceptance.md`

Modify:
- `src/encode/mod.rs`
- `src/encode/ffmpeg.rs`
- `src/control.rs`
- `src/recorder.rs`
- `tests/record.rs`
- `tests/common/mod.rs`
- `tests/layers.rs`
- `.github/workflows/ci.yml` (exists after `cdp-capture`)
- `Cargo.toml`
- `Cargo.lock`
- `README.md`

Delete:
- none

Files under `spec/` and `tasks/` may always be edited; Task 7 edits this plan. `src/encode/gif.rs`, `src/capture/`, `src/store.rs`, `src/draw.rs`, `src/error.rs` and `src/model.rs` are not touched.

## Task list

Details, acceptance and verification for each task are in [todo.md](todo.md).

### Phase 1: shared code, tested with ffmpeg
- Task 1: a test recording with a chosen picture, and the MP4 reader
- Task 2: the video-frame loop and even-size padding
- Task 3: `encode::mp4`, `encode::check_mp4` and the ffmpeg writer
- Task 4: the pixel check

### Checkpoint A: nothing changed for the user
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` pass on the development machine, with ffmpeg on PATH
- The MP4 of `recdr record --monitor 0 --duration 3s --format mp4` plays, by hand
- The owner keeps a 10 second recording and ffmpeg's MP4 of it, for decision 13

### Phase 2: Windows
- Task 5: the Windows writer, and Windows records MP4 with no ffmpeg
- Task 6: CI installs ffmpeg where it is needed

### Checkpoint B: Windows is done (needs a pull request)
- The owner pushes the branch and opens a draft pull request; the build stops and asks for this
- The `test` job is green on the three runners. The Windows job shows whether a hosted runner has an H.264 encoder (see Risks)
- By hand: success criteria 12 and 13 for Windows

### Phase 3: macOS
- Task 7: try `AVAssetWriter` on a Mac or in CI, and write the results into this plan
- Task 8: the macOS writer

### Checkpoint C: macOS is done (needs the pull request)
- The `test` job is green on `macos-latest`, the pixel check included
- By hand, on the owner's Mac if there is one: success criterion 13 for macOS

### Phase 4: the rest
- Task 9: README and the acceptance run

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| This plan was written before `cdp-capture` was built. | Med | "Before Task 1" checks the three things taken from the cdp plan. Phase 1 touches only code that exists today. |
| A hosted Windows runner may have no H.264 encoder. | Med | Checkpoint B shows it. If so, the change under "If the Windows runner has no encoder" below is made as its own commit and noted in `acceptance.md`. |
| A system encoder's `finish` may not return when no frame was pushed. | Med | Task 5 tries exactly that case with a time limit. If it hangs, the build stops and this plan gets a rule for it before Task 5 is committed. |
| It is not known whether a machine with no H.264 encoder fails `check_mp4` or only the real encode. | Med | The same Checkpoint B run answers it when the runner has no encoder. If no such machine is found, the item stays open and the README says so. |
| The macOS writer cannot be compiled or run on the development machine. | High | Task 7 comes first and is small. Without a Mac each try costs a CI run of several minutes; the three writer functions are about 100 lines. |
| The colors of a decoded frame differ between encoders and decoders. | Low | The limits in criterion 9 are wide: a value over 180 or under 90, where the exact values are 255 and 0. |
| An empty `PATH` could stop the Windows encoder from loading. | Low | `mp4_with_no_ffmpeg_on_path` in Task 5 is exactly that case. If it fails for this reason the test keeps the system directories and removes only the directory that holds ffmpeg, and the plan is updated. |
| `brew install ffmpeg` makes the macOS job slower. | Low | Measured at Checkpoint C. If it adds more than five minutes, the pixel check on macOS moves to the owner's Mac and the step is removed. |

### If the Windows runner has no encoder

Made only when the `windows-latest` job of Checkpoint B fails because the runner cannot encode H.264. Two files change, both in the File boundary.

- `src/encode/mod.rs`, test module: `fn encoder_missing() -> bool`, true when `cfg!(windows)`, the environment variable `CI` is set, and `check_mp4()` returns an error. Every unit test that writes an MP4 returns at its start, after printing `skipped: no H.264 encoder on this runner`, when it is true. That includes `check_mp4_leaves_no_file` and `finish_with_no_frame_returns`. A new test, `a_missing_encoder_is_reported`, does nothing unless `encoder_missing()`, and then asserts that the error of `check_mp4()` has the code `encode_failed` and a message containing `Media Feature Pack`.
- `tests/record.rs`: the two MP4 tests print the same note and return when `cfg!(windows)`, `CI` is set, and their `recdr record` command exits with the code `encode_failed`.

On the development machine `CI` is not set, so nothing is skipped there.

## Open questions

None block the build. Two need the owner at a point in it:

1. Checkpoints B and C need the branch pushed and a draft pull request open.
2. Spec open question 1: whether the Windows half is built right after `cdp-capture`. This plan allows it: Phases 1 and 2 need nothing from `macos-capture`.
