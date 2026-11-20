//! Writes a GIF or an MP4 from part of a finished recording.

use crate::encode;
use crate::error::{Error, Result};
use crate::model::{Click, ExportOptions, ExportResult, Format, FrameEntry, Manifest, Rect};
use crate::store::{self, RecordingDir};

/// Checks the options against the recording and writes the file. The file is written
/// next to its target under a `.part` name and renamed over it only when the encoder
/// succeeds, so a failed export leaves an existing target as it was.
pub fn run(dir: &RecordingDir, options: &ExportOptions) -> Result<ExportResult> {
    if dir.read_result()?.is_none() {
        return Err(Error::RecordingNotFound(dir.id().to_owned()));
    }
    let manifest = dir.read_manifest()?;
    let from = options.from_ms.unwrap_or(0);
    let until = options.until_ms.unwrap_or(manifest.duration_ms);
    if !(from < until && until <= manifest.duration_ms) {
        return Err(Error::Usage(format!(
            "--from must be before --until, and both inside the recording (0 to {} ms)",
            manifest.duration_ms
        )));
    }
    let crop = options.crop.as_ref();
    if let Some(r) = crop {
        let fits = r.x >= 0
            && r.y >= 0
            && r.width >= 2
            && r.height >= 2
            && i64::from(r.x) + i64::from(r.width) <= i64::from(manifest.width)
            && i64::from(r.y) + i64::from(r.height) <= i64::from(manifest.height);
        if !fits {
            return Err(Error::Usage(format!(
                "--crop must lie inside the {}x{} picture and be at least 2x2",
                manifest.width, manifest.height
            )));
        }
    }
    // The recording's files are recdr's own: replacing recording.gif or a frame would
    // change the recording that every later call reads. A missing parent is left for
    // the write to report.
    let parent = options.file.parent().filter(|p| !p.as_os_str().is_empty());
    let parent = parent.unwrap_or(std::path::Path::new("."));
    if let (Ok(parent), Ok(recording)) = (parent.canonicalize(), dir.path().canonicalize())
        && parent.starts_with(&recording)
    {
        return Err(Error::Usage(format!(
            "--file is inside the recording directory {}; write the export somewhere else",
            dir.path().display()
        )));
    }
    if options.format == Format::Mp4 {
        encode::check_mp4()?;
    }

    let part = kept(&manifest, from, until, crop);
    let ext = match options.format {
        Format::Gif => "gif",
        Format::Mp4 => "mp4",
    };
    let temp = options.file.with_extension(format!("part.{ext}"));
    let written = match options.format {
        Format::Gif => encode::gif::encode(dir, &part, crop, &temp),
        Format::Mp4 => encode::mp4(dir, &part, crop, &temp),
    };
    let remove_temp = || {
        if temp.is_file() {
            let _ = std::fs::remove_file(&temp);
        }
    };
    let frames = written.map_err(|error| {
        remove_temp();
        match error {
            Error::Io(e) => Error::EncodeFailed(e.to_string()),
            other => other,
        }
    })?;
    std::fs::rename(&temp, &options.file).map_err(|e| {
        remove_temp();
        Error::EncodeFailed(e.to_string())
    })?;

    let (width, height) = match options.format {
        Format::Gif => (part.width, part.height),
        Format::Mp4 => encode::video_size(part.width, part.height),
    };
    Ok(ExportResult {
        path: options.file.clone(),
        width,
        height,
        duration_ms: part.duration_ms,
        frames,
    })
}

/// The manifest of the part of a recording between two times, cut to `crop`.
pub fn kept(manifest: &Manifest, from_ms: u64, until_ms: u64, crop: Option<&Rect>) -> Manifest {
    let moved = |entry: &FrameEntry, t_ms| FrameEntry {
        t_ms,
        ..entry.clone()
    };
    let first = store::frame_at(&manifest.frames, from_ms).map(|entry| moved(entry, 0));
    let rest = manifest
        .frames
        .iter()
        .filter(|f| from_ms < f.t_ms && f.t_ms < until_ms)
        .map(|f| moved(f, f.t_ms - from_ms));
    let inside = |c: &Click| {
        crop.is_none_or(|r| {
            (r.x..r.x + r.width as i32).contains(&c.x)
                && (r.y..r.y + r.height as i32).contains(&c.y)
        })
    };
    let (dx, dy) = crop.map_or((0, 0), |r| (r.x, r.y));
    let clicks = manifest
        .clicks
        .iter()
        .filter(|c| (from_ms..until_ms).contains(&c.t_ms) && inside(c))
        .map(|c| Click {
            t_ms: c.t_ms - from_ms,
            x: c.x - dx,
            y: c.y - dy,
            button: c.button.clone(),
        })
        .collect();
    let (width, height) = crop.map_or((manifest.width, manifest.height), |r| (r.width, r.height));
    Manifest {
        frames: first.into_iter().chain(rest).collect(),
        clicks,
        marks: Vec::new(),
        width,
        height,
        duration_ms: until_ms - from_ms,
        ..manifest.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mark, StopReason};

    fn manifest(clicks: Vec<Click>) -> Manifest {
        Manifest {
            id: "x".into(),
            width: 100,
            height: 100,
            fps: 15,
            started_at_unix_ms: 7,
            duration_ms: 3000,
            frames: [0, 500, 1000, 1500, 2500]
                .iter()
                .enumerate()
                .map(|(i, &t_ms)| FrameEntry {
                    index: i as u32 + 1,
                    t_ms,
                    file: format!("frames/{t_ms}.png"),
                })
                .collect(),
            marks: vec![Mark {
                t_ms: 1300,
                label: "m".into(),
            }],
            clicks,
            stop_reason: StopReason::Stopped,
            dropped_frames: 2,
        }
    }

    fn click(t_ms: u64, x: i32, y: i32) -> Click {
        Click {
            t_ms,
            x,
            y,
            button: "left".into(),
        }
    }

    fn times_and_files(m: &Manifest) -> Vec<(u64, String)> {
        m.frames.iter().map(|f| (f.t_ms, f.file.clone())).collect()
    }

    #[test]
    fn a_trim_starts_at_the_frame_on_screen() {
        let part = kept(&manifest(Vec::new()), 1200, 2600, None);
        assert_eq!(
            times_and_files(&part),
            [
                (0, "frames/1000.png".to_owned()),
                (300, "frames/1500.png".to_owned()),
                (1300, "frames/2500.png".to_owned()),
            ]
        );
        assert_eq!(part.frames[0].index, 3);
        assert_eq!(part.duration_ms, 1400);
        assert!(part.marks.is_empty());
    }

    #[test]
    fn the_whole_recording_is_the_same_but_marks() {
        let m = manifest(vec![click(1300, 60, 50)]);
        let part = kept(&m, 0, 3000, None);
        assert_eq!(part.frames, m.frames);
        assert_eq!(part.clicks, m.clicks);
        assert_eq!(
            (part.width, part.height, part.duration_ms),
            (100, 100, 3000)
        );
        assert_eq!(
            (
                part.id.as_str(),
                part.fps,
                part.started_at_unix_ms,
                part.dropped_frames
            ),
            ("x", 15, 7, 2)
        );
        assert!(part.marks.is_empty());
    }

    #[test]
    fn a_crop_keeps_and_moves_the_clicks_inside_it() {
        let m = manifest(vec![
            click(1300, 60, 50),
            click(1300, 5, 5),
            click(1100, 60, 50),
        ]);
        let crop = Rect {
            x: 10,
            y: 20,
            width: 80,
            height: 60,
        };
        let part = kept(&m, 1200, 2600, Some(&crop));
        assert_eq!(part.clicks, [click(100, 50, 30)]);
        assert_eq!((part.width, part.height), (80, 60));
    }

    #[test]
    fn a_click_at_from_is_kept_and_one_at_until_is_not() {
        let m = manifest(vec![click(1200, 1, 1), click(2600, 2, 2)]);
        let part = kept(&m, 1200, 2600, None);
        assert_eq!(part.clicks, [click(0, 1, 1)]);
    }

    #[test]
    fn no_frames_stay_no_frames() {
        let mut m = manifest(Vec::new());
        m.frames.clear();
        assert!(kept(&m, 100, 200, None).frames.is_empty());
    }
}
