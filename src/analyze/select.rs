use image::imageops::{self, FilterType};

use crate::error::Result;
use crate::model::{DiffEntry, FrameEntry, FrameRef, FrameSelection, Manifest};
use crate::store::{self, RecordingDir};

/// The stored frames a selection asks for.
pub fn select(
    manifest: &Manifest,
    diffs: &[DiffEntry],
    selection: &FrameSelection,
) -> Vec<FrameEntry> {
    let at = |t_ms| store::frame_at(&manifest.frames, t_ms).cloned();
    match selection {
        FrameSelection::At(times) => times.iter().filter_map(|&t| at(t)).collect(),
        FrameSelection::Every(step) => (0..=manifest.duration_ms)
            .step_by((*step).max(1) as usize)
            .filter_map(at)
            .collect(),
        FrameSelection::Keyframes => manifest
            .frames
            .iter()
            .zip(diffs)
            .filter(|(_, d)| d.keyframe)
            .map(|(f, _)| f.clone())
            .collect(),
    }
}

/// The size to scale a `width` by `height` frame to, or `None` when it already fits.
/// Never enlarges. Rounds down, like the resize in `frame_refs`.
pub fn fit(
    width: u32,
    height: u32,
    max_width: Option<u32>,
    max_edge: Option<u32>,
) -> Option<(u32, u32)> {
    let scale = |a: u32, b: u32, c: u32| (u64::from(a) * u64::from(b) / u64::from(c.max(1))) as u32;
    let mut limit = max_width.unwrap_or(width);
    if let Some(edge) = max_edge {
        limit = limit.min(if height > width {
            scale(edge, width, height)
        } else {
            edge
        });
    }
    if limit >= width {
        return None;
    }
    let new_width = limit.max(1);
    Some((new_width, scale(height, new_width, width).max(1)))
}

/// Paths to the frames, scaled down into `samples/` when wider than `max_width`.
pub fn frame_refs(
    dir: &RecordingDir,
    entries: &[FrameEntry],
    max_width: Option<u32>,
) -> Result<Vec<FrameRef>> {
    // `--every` on a still screen names one stored frame many times; scale it once.
    let mut scaled = std::collections::HashMap::<u32, std::path::PathBuf>::new();
    entries
        .iter()
        .map(|entry| {
            let path = match max_width {
                Some(_) if scaled.contains_key(&entry.index) => scaled[&entry.index].clone(),
                Some(max) => {
                    let image = dir.load_frame(entry)?;
                    if image.width() > max {
                        let height = (u64::from(image.height()) * u64::from(max)
                            / u64::from(image.width()))
                        .max(1) as u32;
                        let path = dir.sample_path(entry, max)?;
                        imageops::resize(&image, max, height, FilterType::Triangle).save(&path)?;
                        scaled.insert(entry.index, path.clone());
                        path
                    } else {
                        dir.frame_path(entry)
                    }
                }
                None => dir.frame_path(entry),
            };
            Ok(FrameRef {
                t_ms: entry.t_ms,
                path,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StopReason;

    fn manifest() -> Manifest {
        Manifest {
            id: String::new(),
            width: 1,
            height: 1,
            fps: 15,
            started_at_unix_ms: 0,
            duration_ms: 1000,
            frames: [0, 400, 900]
                .iter()
                .enumerate()
                .map(|(i, &t_ms)| FrameEntry {
                    index: i as u32 + 1,
                    t_ms,
                    file: String::new(),
                })
                .collect(),
            marks: Vec::new(),
            clicks: Vec::new(),
            stop_reason: StopReason::Stopped,
            dropped_frames: 0,
        }
    }

    fn times(entries: Vec<FrameEntry>) -> Vec<u64> {
        entries.iter().map(|e| e.t_ms).collect()
    }

    #[test]
    fn fit_caps_width_and_long_edge() {
        assert_eq!(fit(1080, 2400, Some(1024), Some(2000)), Some((900, 2000)));
        assert_eq!(fit(1080, 2400, Some(4000), Some(2000)), Some((900, 2000)));
        assert_eq!(fit(1080, 2400, Some(1024), None), Some((1024, 2275)));
        assert_eq!(fit(320, 240, Some(1024), Some(2000)), None);
        assert_eq!(fit(3000, 1000, None, Some(2000)), Some((2000, 666)));
    }

    #[test]
    fn every_and_at_map_times_to_the_frame_on_screen() {
        let m = manifest();
        assert_eq!(
            times(select(&m, &[], &FrameSelection::Every(500))),
            [0, 400, 900]
        );
        assert_eq!(
            times(select(&m, &[], &FrameSelection::At(vec![450]))),
            [400]
        );
    }
}
