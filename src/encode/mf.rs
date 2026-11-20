//! The MP4 writer on Windows: H.264 through Media Foundation, by way of
//! `windows_capture`'s encoder.

use std::path::Path;

use image::RgbaImage;
use windows_capture::encoder::{
    AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
    VideoSettingsSubType,
};

use crate::error::{Error, Result};

pub struct Writer {
    encoder: VideoEncoder,
    fps: u64,
    /// The last picture in the encoder's order, BGRA with the bottom row first.
    bgra: Vec<u8>,
}

fn failed(error: impl std::fmt::Display) -> Error {
    Error::EncodeFailed(error.to_string())
}

impl Writer {
    /// Starts a video of `width` by `height` (both even) at `fps` frames per second.
    pub fn create(path: &Path, width: u32, height: u32, fps: u32, bit_rate: u32) -> Result<Self> {
        let video = VideoSettingsBuilder::new(width, height)
            .sub_type(VideoSettingsSubType::H264)
            .bitrate(bit_rate)
            .frame_rate(fps);
        let encoder = VideoEncoder::new(
            video,
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            path,
        )
        .map_err(failed)?;
        Ok(Self {
            encoder,
            fps: u64::from(fps),
            bgra: Vec::new(),
        })
    }

    /// Adds video frame `k`. Frames arrive in order, starting at 0. `changed` is false
    /// when `image` is the picture of the frame before.
    pub fn push(&mut self, k: u64, image: &RgbaImage, changed: bool) -> Result<()> {
        if changed || self.bgra.is_empty() {
            // The encoder takes BGRA with the bottom row first.
            self.bgra = image::imageops::flip_vertical(image).into_raw();
            for pixel in self.bgra.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        // Timestamps are in units of 100 ns.
        let timestamp = (k * 10_000_000 / self.fps) as i64;
        self.encoder
            .send_frame_buffer(&self.bgra, timestamp)
            .map_err(failed)
    }

    /// Completes the file.
    pub fn finish(self) -> Result<()> {
        self.encoder.finish().map_err(failed)
    }
}
