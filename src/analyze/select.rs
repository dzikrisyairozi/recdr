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
/// Never enlarges. Rounds down, like the resize in `frame_refs`. A frame thinner than
/// 1:`max_edge` is squashed, since 1 px is as narrow as it gets.
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
    let new_height = scale(height, new_width, width).min(max_edge.unwrap_or(u32::MAX));
    Some((new_width, new_height.max(1)))
}

/// Paths to the frames, scaled down into `samples/` when wider than `max_width` or
/// longer than `max_edge` on either edge.
pub fn frame_refs(
    dir: &RecordingDir,
    entries: &[FrameEntry],
    max_width: Option<u32>,
    max_edge: Option<u32>,
) -> Result<Vec<FrameRef>> {
    // `--every` on a still screen names one stored frame many times; scale it once.
    let mut scaled = std::collections::HashMap::<u32, std::path::PathBuf>::new();
    entries
        .iter()
        .map(|entry| {
            let path = if max_width.is_none() && max_edge.is_none() {
                dir.frame_path(entry)
            } else if let Some(path) = scaled.get(&entry.index) {
                path.clone()
            } else {
                // The header gives the size; decode only a frame that needs scaling.
                let (width, height) = image::image_dimensions(dir.frame_path(entry))?;
                match fit(width, height, max_width, max_edge) {
                    Some((width, height)) => {
                        let image = dir.load_frame(entry)?;
                        let path = dir.sample_path(entry, width)?;
                        imageops::resize(&image, width, height, FilterType::Triangle)
                            .save(&path)?;
                        scaled.insert(entry.index, path.clone());
                        path
                    }
                    None => dir.frame_path(entry),
                }
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
    use crate::model::{Frame, StopReason};

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
        // Thinner than 1:2000: only squashing the height keeps it inside the edge.
        assert_eq!(fit(1, 3000, None, Some(2000)), Some((1, 2000)));
    }

    // A tenth of a 1080 by 2400 phone screen: full-size PNGs took 6 s in a debug build.
    #[test]
    fn frame_refs_caps_a_tall_frame() {
        let out = std::env::temp_dir().join(format!("recdr-unit-{}-refs-tall", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let dir = RecordingDir::create(&out, 0).unwrap();
        let mut writer = dir.frame_writer(108, 240).unwrap();
        let rgba = [128, 128, 128, 255].repeat(108 * 240);
        writer
            .push(
                Frame {
                    width: 108,
                    height: 240,
                    rgba,
                },
                0,
            )
            .unwrap();
        let entries = writer.finish();

        let capped = frame_refs(&dir, &entries, Some(100), Some(200)).unwrap();
        let capped = &capped[0].path;
        assert!(capped.parent().unwrap().ends_with("samples"), "{capped:?}");
        assert_eq!(image::image_dimensions(capped).unwrap(), (90, 200));

        let wide = frame_refs(&dir, &entries, Some(100), None).unwrap();
        let wide = &wide[0].path;
        assert_eq!(image::image_dimensions(wide).unwrap(), (100, 222));
        // Two sizes, two files: neither call overwrites the other's sample.
        assert_ne!(capped, wide);
        assert_eq!(image::image_dimensions(capped).unwrap(), (90, 200));
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
