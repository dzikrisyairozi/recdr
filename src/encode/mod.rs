//! Turns stored frames into output files.

pub mod ffmpeg;
pub mod gif;

use image::RgbaImage;

use crate::draw;
use crate::error::Result;
use crate::model::{FrameEntry, Manifest};
use crate::store::RecordingDir;

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
