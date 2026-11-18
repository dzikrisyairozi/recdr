//! Turns stored frames into output files.

pub mod gif;

mod ffmpeg;

use ffmpeg::Writer;

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
pub fn check_mp4() -> Result<()> {
    if ffmpeg::available() {
        Ok(())
    } else {
        Err(Error::FfmpegNotFound)
    }
}

/// Bits per second asked of a system encoder.
fn bit_rate(width: u32, height: u32, fps: u32) -> u32 {
    (u64::from(width) * u64::from(height) * u64::from(fps) / 8).clamp(1_000_000, 20_000_000) as u32
}

/// Writes one video file through this system's writer.
fn write(dir: &RecordingDir, manifest: &Manifest, path: &Path) -> Result<()> {
    let width = manifest.width + manifest.width % 2;
    let height = manifest.height + manifest.height % 2;
    let fps = manifest.fps.max(1);
    let mut writer = Writer::create(path, width, height, fps, bit_rate(width, height, fps))?;
    let pushed = video_frames(dir, manifest, |k, image| writer.push(k, image));
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
/// Frame `k` is the picture on screen at `k * 1000 / fps` ms, padded to an even size.
/// Returns the number of frames.
fn video_frames(
    dir: &RecordingDir,
    manifest: &Manifest,
    mut each: impl FnMut(u64, &RgbaImage) -> Result<()>,
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
        if shown.as_ref().is_none_or(|(i, _)| *i != at) {
            shown = Some((at, even(render(dir, &pieces[at])?)));
        }
        each(k, &shown.as_ref().expect("set above").1)?;
    }
    Ok(frames)
}

/// The picture with one black column or row added where the width or height is odd.
fn even(image: RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    if width % 2 == 0 && height % 2 == 0 {
        return image;
    }
    let mut padded = RgbaImage::from_pixel(
        width + width % 2,
        height + height % 2,
        image::Rgba([0, 0, 0, 255]),
    );
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
        let count = video_frames(dir, manifest, |k, _| {
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
        video_frames(&dir, &manifest, |_, image| {
            shades.push(image.get_pixel(0, 0)[0]);
            Ok(())
        })
        .unwrap();
        assert_eq!(shades, [0, 0, 0, 255, 255]);
    }

    #[test]
    fn even_adds_a_black_column_and_row() {
        let white = RgbaImage::from_pixel(321, 241, image::Rgba([255, 255, 255, 255]));
        let padded = even(white.clone());
        assert_eq!(padded.dimensions(), (322, 242));
        assert_eq!(padded.get_pixel(321, 0).0, [0, 0, 0, 255]);
        assert_eq!(padded.get_pixel(0, 241).0, [0, 0, 0, 255]);
        assert_eq!(padded.get_pixel(320, 240), white.get_pixel(320, 240));

        let already = RgbaImage::from_pixel(320, 240, image::Rgba([9, 8, 7, 255]));
        assert_eq!(even(already.clone()), already);
    }

    /// True where ffmpeg is the encoder.
    const NEEDS_FFMPEG: bool = true;

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
