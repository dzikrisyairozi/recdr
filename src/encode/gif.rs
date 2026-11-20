use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use gif::{Encoder, Frame, Repeat};

use super::{render, timeline};
use crate::error::{Error, Result};
use crate::model::Manifest;
use crate::store::RecordingDir;

/// Writes `recording.gif`, one GIF frame per timeline piece, looping forever.
pub fn encode(dir: &RecordingDir, manifest: &Manifest) -> Result<PathBuf> {
    let failed = |e: gif::EncodingError| Error::EncodeFailed(e.to_string());
    let (width, height) = (gif_side(manifest.width)?, gif_side(manifest.height)?);
    let path = dir.gif_path();
    let mut encoder =
        Encoder::new(BufWriter::new(File::create(&path)?), width, height, &[]).map_err(failed)?;
    encoder.set_repeat(Repeat::Infinite).map_err(failed)?;
    for piece in timeline(manifest) {
        let mut rgba = render(dir, &piece)?.into_raw();
        let mut frame = Frame::from_rgba_speed(width, height, &mut rgba, 10);
        for delay in delays(piece.duration_ms) {
            frame.delay = delay;
            encoder.write_frame(&frame).map_err(failed)?;
        }
    }
    Ok(path)
}

/// GIF delays for a picture shown `duration_ms`: hundredths of a second, at least 2
/// (browsers treat less as 10), and split where one would pass the u16 a frame holds.
fn delays(duration_ms: u64) -> Vec<u16> {
    let mut left = ((duration_ms + 5) / 10).max(2);
    let mut out = Vec::new();
    while left > 0 {
        let delay = left.min(u64::from(u16::MAX));
        out.push(delay as u16);
        left -= delay;
    }
    out
}

fn gif_side(pixels: u32) -> Result<u16> {
    u16::try_from(pixels)
        .map_err(|_| Error::EncodeFailed(format!("{pixels} pixels is too large for a GIF")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::one_frame;
    use crate::model::Click;

    fn frame_count(path: &std::path::Path) -> usize {
        let mut decoder = gif::DecodeOptions::new()
            .read_info(File::open(path).unwrap())
            .unwrap();
        let mut count = 0;
        while decoder.read_next_frame().unwrap().is_some() {
            count += 1;
        }
        count
    }

    #[test]
    fn a_long_still_picture_keeps_its_length() {
        assert_eq!(delays(40), [4]);
        assert_eq!(delays(3), [2]);
        let long = delays(900_000);
        assert_eq!(long.iter().map(|&d| u64::from(d)).sum::<u64>(), 90_000);
        assert!(long.len() == 2);
    }

    #[test]
    fn a_click_adds_frames_for_its_ring() {
        let click = Click {
            t_ms: 200,
            x: 10,
            y: 10,
            button: "left".into(),
        };
        let (dir, manifest) = one_frame("gif-click", vec![click]);
        assert_eq!(frame_count(&encode(&dir, &manifest).unwrap()), 3);
        let (dir, manifest) = one_frame("gif-plain", Vec::new());
        assert_eq!(frame_count(&encode(&dir, &manifest).unwrap()), 1);
    }
}
