//! The MP4 writer where no system encoder is used: raw RGBA frames piped to ffmpeg.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread::{self, JoinHandle};

use image::RgbaImage;

use crate::error::{Error, Result};

/// True when `ffmpeg` on PATH runs.
pub(super) fn available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub struct Writer {
    child: Child,
    stdin: ChildStdin,
    errors: JoinHandle<String>,
    /// Set by the first failed write. ffmpeg has exited then, and its stderr says why.
    broken: bool,
}

impl Writer {
    /// Starts a video of `width` by `height` (both even) at `fps` frames per second.
    /// ffmpeg keeps its own quality setting, so the bit rate is not used.
    pub fn create(path: &Path, width: u32, height: u32, fps: u32, _bit_rate: u32) -> Result<Self> {
        let mut child = Command::new("ffmpeg")
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-s",
            ])
            .arg(format!("{width}x{height}"))
            .arg("-r")
            .arg(fps.to_string())
            .args(["-i", "-"])
            .args([
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ])
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::EncodeFailed(format!("ffmpeg did not start: {e}")))?;

        // Read stderr on its own thread so a chatty ffmpeg cannot block on a full pipe
        // while we block writing its stdin.
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let errors = thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            text
        });
        let stdin = child.stdin.take().expect("stdin is piped");
        Ok(Self {
            child,
            stdin,
            errors,
            broken: false,
        })
    }

    /// Adds video frame `k`. Frames arrive in order, starting at 0.
    pub fn push(&mut self, _k: u64, image: &RgbaImage) -> Result<()> {
        if !self.broken && self.stdin.write_all(image.as_raw()).is_err() {
            self.broken = true;
        }
        Ok(())
    }

    /// Completes the file.
    pub fn finish(self) -> Result<()> {
        let Self {
            mut child,
            stdin,
            errors,
            ..
        } = self;
        drop(stdin);
        let status = child
            .wait()
            .map_err(|e| Error::EncodeFailed(e.to_string()))?;
        let errors = errors.join().unwrap_or_default();
        if !status.success() {
            return Err(Error::EncodeFailed(format!(
                "ffmpeg exited with {status}: {}",
                errors.trim()
            )));
        }
        Ok(())
    }
}
