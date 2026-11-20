//! Turns stored frames into output files.

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

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../tests/common/mp4.rs"]
mod mp4;

use std::path::{Path, PathBuf};

use image::RgbaImage;

use crate::draw;
use crate::error::{Error, Result};
use crate::model::{FrameEntry, Manifest};
use crate::store::RecordingDir;

/// Writes `recording.mp4` (H.264) with the encoder this system uses.
pub fn mp4(dir: &RecordingDir, manifest: &Manifest) -> Result<PathBuf> {
    if manifest.width == 0 || manifest.height == 0 {
        return Err(Error::EncodeFailed("the recording has no picture".into()));
    }
    let path = dir.mp4_path();
    if let Err(error) = write(dir, manifest, &path) {
        // A directory in the way is not ours to remove.
        if path.is_file() {
            let _ = std::fs::remove_file(&path);
        }
        return Err(error);
    }
    Ok(path)
}

/// Fails when an MP4 cannot be written on this machine, before anything is recorded.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn check_mp4() -> Result<()> {
    if ffmpeg::available() {
        Ok(())
    } else {
        Err(Error::FfmpegNotFound)
    }
}

/// Fails when an MP4 cannot be written on this machine, before anything is recorded:
/// a one-frame video is written to the temporary directory with the system encoder.
#[cfg(any(windows, target_os = "macos"))]
pub fn check_mp4() -> Result<()> {
    // A name per call: MCP calls run on their own threads, and two checks sharing a
    // file break each other.
    static TRIALS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = TRIALS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("recdr-check-{}-{n}.mp4", std::process::id()));
    let black = RgbaImage::from_pixel(160, 120, image::Rgba([0, 0, 0, 255]));
    let trial = Writer::create(&path, 160, 120, 1, 1_000_000).and_then(|mut writer| {
        let pushed = writer.push(0, &black, true);
        let finished = writer.finish();
        pushed.and(finished)
    });
    let _ = std::fs::remove_file(&path);
    trial.map_err(|error| match error {
        Error::EncodeFailed(message) => {
            let hint = if cfg!(windows) {
                " (on Windows N editions, install the Media Feature Pack)"
            } else {
                ""
            };
            Error::EncodeFailed(format!("{message}{hint}"))
        }
        other => other,
    })
}

/// Bits per second asked of a system encoder.
fn bit_rate(width: u32, height: u32, fps: u32) -> u32 {
    (u64::from(width) * u64::from(height) * u64::from(fps) / 8).clamp(1_000_000, 20_000_000) as u32
}

/// Writes one video file through this system's writer.
fn write(dir: &RecordingDir, manifest: &Manifest, path: &Path) -> Result<()> {
    let (width, height) = video_size(manifest.width, manifest.height);
    let fps = manifest.fps.max(1);
    let mut writer = Writer::create(path, width, height, fps, bit_rate(width, height, fps))?;
    let pushed = video_frames(dir, manifest, |k, image, changed| {
        writer.push(k, image, changed)
    });
    // Always finished, so the encoder lets go of the file before `mp4` removes it.
    let finished = writer.finish();
    pushed.and(finished)
}

/// How long a click ring stays on screen.
pub const RING_MS: u64 = 400;

/// One stretch of output time showing one stored frame and the click rings visible during it.
#[derive(Debug, Clone, PartialEq)]
pub struct OutFrame {
    pub entry: FrameEntry,
    pub t_ms: u64,
    pub duration_ms: u64,
    pub rings: Vec<(i32, i32)>,
}

/// Each stored frame lasts until the next one (the last until `duration_ms`), split
/// wherever a click ring appears or disappears.
pub fn timeline(manifest: &Manifest) -> Vec<OutFrame> {
    let mut out = Vec::new();
    for (i, entry) in manifest.frames.iter().enumerate() {
        let start = entry.t_ms;
        let end = manifest
            .frames
            .get(i + 1)
            .map_or(manifest.duration_ms, |next| next.t_ms)
            .max(start);
        let mut cuts = vec![start, end];
        for click in &manifest.clicks {
            for cut in [click.t_ms, click.t_ms + RING_MS] {
                if start < cut && cut < end {
                    cuts.push(cut);
                }
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        for pair in cuts.windows(2) {
            let (t_ms, until) = (pair[0], pair[1]);
            let rings = manifest
                .clicks
                .iter()
                .filter(|c| c.t_ms <= t_ms && t_ms < c.t_ms + RING_MS)
                .map(|c| (c.x, c.y))
                .collect();
            out.push(OutFrame {
                entry: entry.clone(),
                t_ms,
                duration_ms: until - t_ms,
                rings,
            });
        }
    }
    out
}

/// The image shown for one piece of the timeline: the stored frame plus its click
/// rings. Stored frames on disk stay clean.
pub fn render(dir: &RecordingDir, frame: &OutFrame) -> Result<RgbaImage> {
    let mut image = dir.load_frame(&frame.entry)?;
    for &(x, y) in &frame.rings {
        draw::ring(&mut image, x, y, 14);
    }
    Ok(image)
}

/// Calls `each` once per video frame, in order, with the frame's number and its picture.
/// Frame `k` is the picture on screen at `k * 1000 / fps` ms, padded to `video_size`.
/// Returns the number of frames.
fn video_frames(
    dir: &RecordingDir,
    manifest: &Manifest,
    mut each: impl FnMut(u64, &RgbaImage, bool) -> Result<()>,
) -> Result<u64> {
    let pieces = timeline(manifest);
    if pieces.is_empty() {
        return Ok(0);
    }
    let fps = u64::from(manifest.fps.max(1));
    let frames = (manifest.duration_ms * fps).div_ceil(1000);
    let mut shown: Option<(usize, RgbaImage)> = None;
    for k in 0..frames {
        let t_ms = k * 1000 / fps;
        let at = pieces.iter().rposition(|p| p.t_ms <= t_ms).unwrap_or(0);
        let changed = shown.as_ref().is_none_or(|(i, _)| *i != at);
        if changed {
            shown = Some((at, padded(render(dir, &pieces[at])?)));
        }
        if let Some((_, image)) = &shown {
            each(k, image, changed)?;
        }
    }
    Ok(frames)
}

/// The size of the video for a recording of `width` by `height`: even, and at least
/// 160 by 120, because a GPU encoder can refuse anything smaller (NVENC's minimum is
/// about 145 by 49).
fn video_size(width: u32, height: u32) -> (u32, u32) {
    ((width + width % 2).max(160), (height + height % 2).max(120))
}

/// The picture with black columns on the right and rows at the bottom up to `video_size`.
fn padded(image: RgbaImage) -> RgbaImage {
    let size = video_size(image.width(), image.height());
    if image.dimensions() == size {
        return image;
    }
    let mut padded = RgbaImage::from_pixel(size.0, size.1, image::Rgba([0, 0, 0, 255]));
    image::imageops::replace(&mut padded, &image, 0, 0);
    padded
}

/// A recording directory with one stored frame at 0 ms showing `image`.
#[cfg(test)]
pub(crate) fn recording(
    name: &str,
    image: RgbaImage,
    duration_ms: u64,
    fps: u32,
    clicks: Vec<crate::model::Click>,
) -> (RecordingDir, Manifest) {
    use crate::model::{Frame, StopReason};
    let out = std::env::temp_dir().join(format!("recdr-unit-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let dir = RecordingDir::create(&out, 0).unwrap();
    let (width, height) = image.dimensions();
    let mut writer = dir.frame_writer(width, height).unwrap();
    let frame = Frame {
        width,
        height,
        rgba: image.into_raw(),
    };
    writer.push(frame, 0).unwrap();
    let manifest = Manifest {
        id: String::new(),
        width,
        height,
        fps,
        started_at_unix_ms: 0,
        duration_ms,
        frames: writer.finish(),
        marks: Vec::new(),
        clicks,
        stop_reason: StopReason::Stopped,
        dropped_frames: 0,
    };
    (dir, manifest)
}

/// One solid black 64x48 frame, 1000 ms, 15 fps.
#[cfg(test)]
pub(crate) fn one_frame(name: &str, clicks: Vec<crate::model::Click>) -> (RecordingDir, Manifest) {
    let black = RgbaImage::from_pixel(64, 48, image::Rgba([0, 0, 0, 255]));
    recording(name, black, 1000, 15, clicks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Click, StopReason};

    fn manifest(frame_times: &[u64], duration_ms: u64, clicks: Vec<Click>) -> Manifest {
        Manifest {
            id: String::new(),
            width: 1,
            height: 1,
            fps: 15,
            started_at_unix_ms: 0,
            duration_ms,
            frames: frame_times
                .iter()
                .enumerate()
                .map(|(i, &t_ms)| FrameEntry {
                    index: i as u32 + 1,
                    t_ms,
                    file: String::new(),
                })
                .collect(),
            marks: Vec::new(),
            clicks,
            stop_reason: StopReason::Stopped,
            dropped_frames: 0,
        }
    }

    #[test]
    fn each_frame_lasts_until_the_next() {
        let pieces = timeline(&manifest(&[0, 100, 250], 400, Vec::new()));
        let durations: Vec<u64> = pieces.iter().map(|p| p.duration_ms).collect();
        assert_eq!(durations, [100, 150, 150]);
        assert!(pieces.iter().all(|p| p.rings.is_empty()));
    }

    #[test]
    fn a_click_splits_the_frame_and_rings_only_while_visible() {
        let click = Click {
            t_ms: 200,
            x: 5,
            y: 6,
            button: "left".into(),
        };
        let pieces = timeline(&manifest(&[0], 1000, vec![click]));
        let durations: Vec<u64> = pieces.iter().map(|p| p.duration_ms).collect();
        assert_eq!(durations, [200, 400, 400]);
        let rings: Vec<usize> = pieces.iter().map(|p| p.rings.len()).collect();
        assert_eq!(rings, [0, 1, 0]);
        assert_eq!(pieces[1].rings, [(5, 6)]);
    }

    fn black(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 255]))
    }

    fn calls(dir: &RecordingDir, manifest: &Manifest) -> Vec<u64> {
        let mut seen = Vec::new();
        let count = video_frames(dir, manifest, |k, _, _| {
            seen.push(k);
            Ok(())
        })
        .unwrap();
        assert_eq!(count, seen.len() as u64);
        seen
    }

    #[test]
    fn one_call_per_video_frame() {
        let (dir, manifest) = recording("calls-1000", black(8, 8), 1000, 10, Vec::new());
        assert_eq!(calls(&dir, &manifest), (0..10).collect::<Vec<_>>());
        let (dir, manifest) = recording("calls-1001", black(8, 8), 1001, 10, Vec::new());
        assert_eq!(calls(&dir, &manifest), (0..11).collect::<Vec<_>>());
    }

    #[test]
    fn a_call_gets_the_picture_of_its_time() {
        use crate::model::Frame;
        let out = std::env::temp_dir().join(format!("recdr-unit-{}-times", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let dir = RecordingDir::create(&out, 0).unwrap();
        let mut writer = dir.frame_writer(8, 8).unwrap();
        for (value, t_ms) in [(0, 0), (255, 250)] {
            let rgba = [value, value, value, 255].repeat(64);
            let frame = Frame {
                width: 8,
                height: 8,
                rgba,
            };
            writer.push(frame, t_ms).unwrap();
        }
        let mut manifest = manifest(&[], 500, Vec::new());
        (manifest.width, manifest.height, manifest.fps) = (8, 8, 10);
        manifest.frames = writer.finish();
        assert_eq!(manifest.frames.len(), 2);

        let mut shades = Vec::new();
        let mut changes = Vec::new();
        video_frames(&dir, &manifest, |_, image, changed| {
            shades.push(image.get_pixel(0, 0)[0]);
            changes.push(changed);
            Ok(())
        })
        .unwrap();
        assert_eq!(shades, [0, 0, 0, 255, 255]);
        // A writer converts the picture only when it is a new one.
        assert_eq!(changes, [true, false, false, true, false]);
    }

    #[test]
    fn padding_adds_a_black_column_and_row_to_an_odd_size() {
        let white = RgbaImage::from_pixel(321, 241, image::Rgba([255, 255, 255, 255]));
        let image = padded(white.clone());
        assert_eq!(image.dimensions(), (322, 242));
        assert_eq!(image.get_pixel(321, 0).0, [0, 0, 0, 255]);
        assert_eq!(image.get_pixel(0, 241).0, [0, 0, 0, 255]);
        assert_eq!(image.get_pixel(320, 240), white.get_pixel(320, 240));

        let already = RgbaImage::from_pixel(320, 240, image::Rgba([9, 8, 7, 255]));
        assert_eq!(padded(already.clone()), already);
    }

    #[test]
    fn a_video_is_at_least_160_by_120() {
        assert_eq!(video_size(64, 48), (160, 120));
        assert_eq!(video_size(321, 33), (322, 120));
        assert_eq!(video_size(99, 241), (160, 242));
        assert_eq!(video_size(1920, 1080), (1920, 1080));

        let white = RgbaImage::from_pixel(64, 48, image::Rgba([255, 255, 255, 255]));
        let image = padded(white);
        assert_eq!(image.dimensions(), (160, 120));
        assert_eq!(image.get_pixel(63, 47).0, [255, 255, 255, 255]);
        assert_eq!(image.get_pixel(64, 0).0, [0, 0, 0, 255]);
        assert_eq!(image.get_pixel(0, 48).0, [0, 0, 0, 255]);
    }

    #[test]
    fn a_small_recording_makes_a_video() {
        if NEEDS_FFMPEG && !ffmpeg_runs() {
            println!("skipped: ffmpeg not on PATH");
            return;
        }
        let (dir, manifest) = one_frame("small", Vec::new());
        let facts = super::mp4::facts(&mp4(&dir, &manifest).unwrap());
        assert_eq!((facts.width, facts.height), (160, 120));
    }

    /// True where ffmpeg is the encoder.
    const NEEDS_FFMPEG: bool = cfg!(not(any(windows, target_os = "macos")));

    /// True when `ffmpeg -version` runs and exits 0.
    fn ffmpeg_runs() -> bool {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    #[test]
    fn bit_rate_is_clamped() {
        assert_eq!(bit_rate(1920, 1080, 15), 3_888_000);
        assert_eq!(bit_rate(320, 240, 15), 1_000_000);
        assert_eq!(bit_rate(3840, 2160, 60), 20_000_000);
    }

    #[test]
    fn no_picture_fails_before_the_encoder() {
        let (dir, mut manifest) = one_frame("no-picture", Vec::new());
        manifest.width = 0;
        assert_eq!(mp4(&dir, &manifest).unwrap_err().code(), "encode_failed");
    }

    #[test]
    fn odd_size_is_padded() {
        if NEEDS_FFMPEG && !ffmpeg_runs() {
            println!("skipped: ffmpeg not on PATH");
            return;
        }
        let (dir, manifest) = recording("odd", black(321, 241), 1000, 10, Vec::new());
        let facts = super::mp4::facts(&mp4(&dir, &manifest).unwrap());
        assert_eq!((facts.width, facts.height), (322, 242));
    }

    #[test]
    fn a_directory_in_the_way_is_left_alone() {
        if NEEDS_FFMPEG && !ffmpeg_runs() {
            println!("skipped: ffmpeg not on PATH");
            return;
        }
        let (dir, manifest) = one_frame("dir-in-the-way", Vec::new());
        std::fs::create_dir(dir.mp4_path()).unwrap();
        assert_eq!(mp4(&dir, &manifest).unwrap_err().code(), "encode_failed");
        assert!(dir.mp4_path().is_dir());
    }

    #[test]
    fn a_failed_encode_leaves_no_file() {
        if NEEDS_FFMPEG && !ffmpeg_runs() {
            println!("skipped: ffmpeg not on PATH");
            return;
        }
        let (dir, mut manifest) = one_frame("failed-encode", Vec::new());
        manifest.frames.push(FrameEntry {
            index: 2,
            t_ms: 500,
            file: "frames/missing.png".into(),
        });
        assert!(mp4(&dir, &manifest).is_err());
        assert!(!dir.mp4_path().exists());
    }

    /// Red in the top-left quarter, blue in the bottom-right quarter, white elsewhere.
    fn quarters(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba(if x < width / 2 && y < height / 2 {
                [255, 0, 0, 255]
            } else if x >= width / 2 && y >= height / 2 {
                [0, 0, 255, 255]
            } else {
                [255, 255, 255, 255]
            })
        })
    }

    /// The first frame of a video, decoded by ffmpeg. None, with a printed note, when
    /// ffmpeg does not run.
    fn first_frame(mp4: &Path) -> Option<RgbaImage> {
        if !ffmpeg_runs() {
            println!("not checked: ffmpeg is needed to decode the frame");
            return None;
        }
        let png = mp4.with_extension("png");
        let status = std::process::Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-i"])
            .arg(mp4)
            .args(["-frames:v", "1"])
            .arg(&png)
            .status()
            .unwrap();
        assert!(
            status.success(),
            "ffmpeg could not decode {}",
            mp4.display()
        );
        Some(image::open(&png).unwrap().to_rgba8())
    }

    #[test]
    fn first_video_frame_has_the_picture_and_the_ring() {
        if NEEDS_FFMPEG && !ffmpeg_runs() {
            println!("skipped: ffmpeg not on PATH");
            return;
        }
        let click = Click {
            t_ms: 0,
            x: 240,
            y: 60,
            button: "left".into(),
        };
        let (dir, manifest) = recording("pixels", quarters(320, 240), 1000, 10, vec![click]);
        let Some(frame) = first_frame(&mp4(&dir, &manifest).unwrap()) else {
            return;
        };
        let at = |x, y| frame.get_pixel(x, y).0;
        let [r, g, b, _] = at(40, 30);
        assert!(r > 180 && g < 90 && b < 90, "top left is {:?}", at(40, 30));
        let [r, g, b, _] = at(280, 210);
        assert!(
            b > 180 && r < 90 && g < 90,
            "bottom right is {:?}",
            at(280, 210)
        );
        assert!(
            at(253, 60)[1] < 160,
            "the ring at (253, 60) is {:?}",
            at(253, 60)
        );
        assert!(at(240, 60)[1] > 200, "inside the ring is {:?}", at(240, 60));
    }

    /// A box: big-endian size, name, content.
    fn mp4_box(name: &[u8; 4], content: &[u8]) -> Vec<u8> {
        let mut out = u32::try_from(content.len() + 8)
            .unwrap()
            .to_be_bytes()
            .to_vec();
        out.extend_from_slice(name);
        out.extend_from_slice(content);
        out
    }

    /// A track: its handler, a tkhd of `duration` in the movie's 1000 units a second,
    /// an mdhd 133 units longer, as libx264's frame delay makes it before the edit
    /// list, and a sample entry named `codec` of `width` by 90.
    fn trak(handler: &[u8; 4], codec: &[u8; 4], width: u16, duration: u32) -> Vec<u8> {
        let mut tkhd = vec![0u8; 20];
        tkhd.extend_from_slice(&duration.to_be_bytes());
        let mut mdhd = vec![0u8; 12];
        mdhd.extend_from_slice(&1000u32.to_be_bytes());
        mdhd.extend_from_slice(&(duration + 133).to_be_bytes());
        let mut hdlr = vec![0u8; 8];
        hdlr.extend_from_slice(handler);
        let mut entry = vec![0u8; 4];
        entry.extend_from_slice(codec);
        entry.resize(32, 0);
        entry.extend_from_slice(&width.to_be_bytes());
        entry.extend_from_slice(&90u16.to_be_bytes());
        let mut stsd = vec![0u8; 8];
        stsd.extend_from_slice(&entry);
        let stbl = mp4_box(b"stbl", &mp4_box(b"stsd", &stsd));
        let minf = mp4_box(b"minf", &stbl);
        let mdia = [mp4_box(b"mdhd", &mdhd), mp4_box(b"hdlr", &hdlr), minf].concat();
        let trak = [mp4_box(b"tkhd", &tkhd), mp4_box(b"mdia", &mdia)].concat();
        mp4_box(b"trak", &trak)
    }

    #[test]
    fn mp4_facts_come_from_the_video_track() {
        let mut mvhd = vec![0u8; 12];
        mvhd.extend_from_slice(&1000u32.to_be_bytes());
        mvhd.extend_from_slice(&9000u32.to_be_bytes());
        // An audio track whose sample entry has a nonzero field where a width would be.
        let moov = [
            mp4_box(b"mvhd", &mvhd),
            trak(b"soun", b"mp4a", 2, 9000),
            trak(b"vide", b"avc1", 160, 2000),
        ]
        .concat();
        let file = [mp4_box(b"ftyp", b"isom"), mp4_box(b"moov", &moov)].concat();
        let path =
            std::env::temp_dir().join(format!("recdr-unit-{}-audio-first.mp4", std::process::id()));
        std::fs::write(&path, file).unwrap();
        let facts = mp4::facts(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            (
                facts.codec.as_str(),
                facts.width,
                facts.height,
                facts.duration_ms
            ),
            ("avc1", 160, 90, 2000)
        );
    }

    #[test]
    fn check_mp4_leaves_no_file() {
        if NEEDS_FFMPEG {
            println!("not run: on this system check_mp4 only looks for ffmpeg and writes no file");
            return;
        }
        let begun = std::time::Instant::now();
        let checked = check_mp4();
        println!(
            "criterion 11: check_mp4 took {} ms",
            begun.elapsed().as_millis()
        );
        checked.unwrap();
        assert_eq!(trials_left(), Vec::<String>::new());
        // MCP calls run on their own threads, so checks can overlap. In this test, so
        // that no other test sees its files half made.
        let checks: Vec<_> = (0..8).map(|_| std::thread::spawn(check_mp4)).collect();
        for check in checks {
            check.join().unwrap().unwrap();
        }
        assert_eq!(trials_left(), Vec::<String>::new());
    }

    /// This process's check files in the temporary directory.
    fn trials_left() -> Vec<String> {
        let prefix = format!("recdr-check-{}-", std::process::id());
        std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| name.starts_with(&prefix))
            .collect()
    }

    #[test]
    fn finish_with_no_frame_returns() {
        if NEEDS_FFMPEG && !ffmpeg_runs() {
            println!("skipped: ffmpeg not on PATH");
            return;
        }
        let (dir, mut manifest) = one_frame("no-frame", Vec::new());
        manifest.frames[0].file = "frames/missing.png".into();
        let (done, result) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(mp4(&dir, &manifest).map(|_| ()));
        });
        let result = result
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("mp4 did not return within 10 seconds");
        assert!(result.is_err());
    }

    #[test]
    #[ignore = "criterion 12, run by hand with --release"]
    fn encodes_90_large_frames_in_time() {
        let (dir, manifest) = recording("large", quarters(1920, 1080), 6000, 15, Vec::new());
        let begun = std::time::Instant::now();
        mp4(&dir, &manifest).unwrap();
        let took = begun.elapsed();
        println!("criterion 12: {:.1} s", took.as_secs_f64());
        assert!(took < std::time::Duration::from_secs(10), "{took:?}");
    }

    #[test]
    #[ignore = "plan decision 13, run by hand on a kept recording"]
    fn encodes_a_kept_recording() {
        let (Some(out), Some(id)) = (
            std::env::var_os("RECDR_KEPT_OUT"),
            std::env::var("RECDR_KEPT_ID").ok(),
        ) else {
            println!("not run: set RECDR_KEPT_OUT and RECDR_KEPT_ID");
            return;
        };
        let dir = RecordingDir::open(Path::new(&out), &id).unwrap();
        let manifest = dir.read_manifest().unwrap();
        let path = mp4(&dir, &manifest).unwrap();
        let bytes = std::fs::metadata(&path).unwrap().len();
        println!("{} is {bytes} bytes", path.display());
    }

    #[test]
    fn render_draws_rings_without_touching_the_stored_frame() {
        let (dir, manifest) = one_frame("render", Vec::new());
        let entry = manifest.frames[0].clone();
        let before = std::fs::read(dir.frame_path(&entry)).unwrap();
        let piece = OutFrame {
            entry: entry.clone(),
            t_ms: 0,
            duration_ms: 100,
            rings: vec![(32, 24)],
        };
        assert_ne!(
            render(&dir, &piece).unwrap(),
            dir.load_frame(&entry).unwrap()
        );
        assert_eq!(std::fs::read(dir.frame_path(&entry)).unwrap(), before);
    }
}
