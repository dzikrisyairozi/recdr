//! What an agent reads instead of watching: per-frame change, keyframes and a contact sheet.

pub mod diff;
pub mod select;
pub mod sheet;

use image::RgbaImage;

use crate::error::Result;
use crate::model::{DiffEntry, Manifest};
use crate::store::RecordingDir;

/// Compares every stored frame with the one before it and marks keyframes, then writes `diff.json`.
pub fn run(dir: &RecordingDir, manifest: &Manifest) -> Result<Vec<DiffEntry>> {
    let (width, height) = (manifest.width, manifest.height);
    let last = manifest.frames.len().saturating_sub(1);
    let mut entries = Vec::with_capacity(manifest.frames.len());
    let mut previous: Option<RgbaImage> = None;
    let mut keyframe: Option<RgbaImage> = None;
    for (i, frame) in manifest.frames.iter().enumerate() {
        let image = dir.load_frame(frame)?;
        let (changed_fraction, bbox) = match &previous {
            Some(prev) => diff::diff(prev, &image, width, height),
            None => (0.0, None),
        };
        let is_key = match &keyframe {
            None => true,
            Some(key) => {
                i == last || diff::diff(key, &image, width, height).0 >= diff::KEYFRAME_FRACTION
            }
        };
        entries.push(DiffEntry {
            t_ms: frame.t_ms,
            changed_fraction,
            bbox,
            keyframe: is_key,
        });
        if is_key {
            keyframe = Some(image.clone());
        }
        previous = Some(image);
    }
    dir.write_diff(&entries)?;
    Ok(entries)
}
