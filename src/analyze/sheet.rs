use std::path::PathBuf;

use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

use crate::draw;
use crate::error::Result;
use crate::model::{DiffEntry, MAX_IMAGE_EDGE, Manifest};
use crate::store::RecordingDir;

pub const MAX_TILES: usize = 12;
const TILE_MAX_WIDTH: u32 = 480;
const STRIP: u32 = 20;
const GAP: u32 = 4;

/// Up to `max` items spread evenly, always keeping the first and the last.
pub fn pick_tiles(keyframes: &[usize], max: usize) -> Vec<usize> {
    let n = keyframes.len();
    if n <= max {
        return keyframes.to_vec();
    }
    (0..max)
        .map(|i| keyframes[((i * (n - 1)) as f64 / (max - 1) as f64).round() as usize])
        .collect()
}

/// One image of the keyframes in a grid, each labelled with its time and the marks
/// that fall before the next tile.
pub fn contact_sheet(
    dir: &RecordingDir,
    manifest: &Manifest,
    diffs: &[DiffEntry],
) -> Result<PathBuf> {
    let keyframes: Vec<usize> = (0..manifest.frames.len())
        .filter(|&i| diffs.get(i).is_some_and(|d| d.keyframe))
        .collect();
    let tiles = pick_tiles(&keyframes, MAX_TILES);
    let count = tiles.len().max(1) as u32;
    let columns = (f64::from(count).sqrt().ceil() as u32).max(1);
    let rows = count.div_ceil(columns);
    // The tallest tile that keeps every row inside MAX_IMAGE_EDGE, as a width.
    // Width needs no check: 12 tiles make at most 4 columns of 480 px, 1940 px in all.
    let tallest = ((MAX_IMAGE_EDGE - GAP) / rows).saturating_sub(STRIP + GAP);
    let by_rows = (u64::from(tallest) * u64::from(manifest.width.max(1))
        / u64::from(manifest.height.max(1))) as u32;
    let tile_width = manifest.width.clamp(1, TILE_MAX_WIDTH).min(by_rows).max(1);
    let tile_height = (u64::from(manifest.height) * u64::from(tile_width)
        / u64::from(manifest.width.max(1)))
    .max(1) as u32;
    let mut sheet = RgbaImage::from_pixel(
        GAP + columns * (tile_width + GAP),
        GAP + rows * (tile_height + STRIP + GAP),
        Rgba([32, 32, 32, 255]),
    );

    for (n, &frame_index) in tiles.iter().enumerate() {
        let entry = &manifest.frames[frame_index];
        let start = entry.t_ms;
        let end = tiles
            .get(n + 1)
            .map_or(manifest.duration_ms, |&next| manifest.frames[next].t_ms);
        let left = GAP + (n as u32 % columns) * (tile_width + GAP);
        let top = GAP + (n as u32 / columns) * (tile_height + STRIP + GAP);

        let frame = dir.load_frame(entry)?;
        let tile = imageops::resize(&frame, tile_width, tile_height, FilterType::Triangle);
        imageops::replace(&mut sheet, &tile, i64::from(left), i64::from(top));
        for y in top + tile_height..top + tile_height + STRIP {
            for x in left..left + tile_width {
                sheet.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        // The last tile's span includes `duration_ms` itself, so a mark at the very end shows.
        let in_span = |t: u64| start <= t && (t < end || (n + 1 == tiles.len() && t <= end));
        let labels: Vec<&str> = manifest
            .marks
            .iter()
            .filter(|m| in_span(m.t_ms))
            .map(|m| m.label.as_str())
            .collect();
        let mut label = format!("{:.1}s", start as f64 / 1000.0);
        if !labels.is_empty() {
            label.push(' ');
            label.push_str(&labels.join(", "));
        }
        let fits = (tile_width.saturating_sub(8) / 16) as usize;
        let label: String = label.chars().take(fits).collect();
        draw::text(&mut sheet, left + 4, top + tile_height + 2, &label, 2);
        for click in manifest.clicks.iter().filter(|c| in_span(c.t_ms)) {
            let scale = |v: i32, side: u32, full: u32| v * side as i32 / full.max(1) as i32;
            draw::ring(
                &mut sheet,
                left as i32 + scale(click.x, tile_width, manifest.width),
                top as i32 + scale(click.y, tile_height, manifest.height),
                10,
            );
        }
    }

    let path = dir.contact_sheet_path();
    sheet.save(&path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Frame, Mark, StopReason};

    #[test]
    fn pick_tiles_keeps_small_lists_and_spreads_long_ones() {
        let five: Vec<usize> = (0..5).collect();
        assert_eq!(pick_tiles(&five, MAX_TILES), five);
        let thirty: Vec<usize> = (0..30).collect();
        let picked = pick_tiles(&thirty, MAX_TILES);
        assert_eq!(picked.len(), 12);
        assert_eq!(picked[0], 0);
        assert_eq!(picked[11], 29);
    }

    /// A sheet of `count` keyframes of `width` by `height`, 100 ms apart, each its own shade.
    fn sheet_of(width: u32, height: u32, count: u64, marks: Vec<Mark>, name: &str) -> RgbaImage {
        let out = std::env::temp_dir().join(format!("recdr-unit-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let dir = RecordingDir::create(&out, 0).unwrap();
        let mut writer = dir.frame_writer(width, height).unwrap();
        for i in 0..count {
            let shade = (i * 20) as u8;
            let rgba = [shade, shade, shade, 255].repeat((width * height) as usize);
            writer
                .push(
                    Frame {
                        width,
                        height,
                        rgba,
                    },
                    i * 100,
                )
                .unwrap();
        }
        let manifest = Manifest {
            id: String::new(),
            width,
            height,
            fps: 15,
            started_at_unix_ms: 0,
            duration_ms: count * 100,
            frames: writer.finish(),
            marks,
            clicks: Vec::new(),
            stop_reason: StopReason::Stopped,
            dropped_frames: 0,
        };
        let diffs: Vec<DiffEntry> = (0..count)
            .map(|i| DiffEntry {
                t_ms: i * 100,
                changed_fraction: 0.0,
                bbox: None,
                keyframe: true,
            })
            .collect();
        let path = contact_sheet(&dir, &manifest, &diffs).unwrap();
        image::open(path).unwrap().to_rgba8()
    }

    fn sheet_with(marks: Vec<Mark>, name: &str) -> RgbaImage {
        sheet_of(200, 48, 2, marks, name)
    }

    #[test]
    fn a_portrait_sheet_fits_in_2000() {
        let sheet = sheet_of(1080, 2400, 12, Vec::new(), "sheet-portrait");
        assert_eq!(sheet.dimensions(), (1172, 1996));
    }

    #[test]
    fn a_landscape_sheet_keeps_its_size() {
        let sheet = sheet_of(1920, 1080, 12, Vec::new(), "sheet-landscape");
        assert_eq!(sheet.dimensions(), (1940, 886));
    }

    #[test]
    fn a_mark_shows_in_the_first_tile_strip() {
        let plain = sheet_with(Vec::new(), "sheet-plain");
        let marked = sheet_with(
            vec![Mark {
                t_ms: 0,
                label: "go".into(),
            }],
            "sheet-marked",
        );
        assert_eq!(plain.dimensions(), marked.dimensions());
        // First tile: x from 4 to 204, strip from y 52 to 72.
        let strip_differs =
            (52..72).any(|y| (4..204).any(|x| plain.get_pixel(x, y) != marked.get_pixel(x, y)));
        assert!(strip_differs);
    }
}
