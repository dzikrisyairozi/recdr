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
        // GIF delays are in hundredths of a second; browsers treat under 2 as 10.
        frame.delay = ((piece.duration_ms + 5) / 10).max(2) as u16;
        encoder.write_frame(&frame).map_err(failed)?;
    }
    Ok(path)
}

fn gif_side(pixels: u32) -> Result<u16> {
    u16::try_from(pixels)
        .map_err(|_| Error::EncodeFailed(format!("{pixels} pixels is too large for a GIF")))
}
