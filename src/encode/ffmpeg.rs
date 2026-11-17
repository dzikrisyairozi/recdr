use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

use super::{render, timeline};
use crate::error::{Error, Result};
use crate::model::Manifest;
use crate::store::RecordingDir;

/// True when `ffmpeg` on PATH runs.
pub fn available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Writes `recording.mp4` (H.264) by piping raw RGBA frames at the recording's fps to ffmpeg.
pub fn encode(dir: &RecordingDir, manifest: &Manifest) -> Result<PathBuf> {
    let failed = |message: String| Error::EncodeFailed(message);
    let path = dir.mp4_path();
    let fps = u64::from(manifest.fps.max(1));
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
        .arg(format!("{}x{}", manifest.width, manifest.height))
        .arg("-r")
        .arg(fps.to_string())
        .args(["-i", "-", "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2"])
        .args([
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| failed(format!("ffmpeg did not start: {e}")))?;

    // Read stderr on its own thread so a chatty ffmpeg cannot block on a full pipe
    // while we block writing its stdin.
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let errors = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let pieces = timeline(manifest);
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let frames = (manifest.duration_ms * fps).div_ceil(1000);
    let mut shown: Option<(usize, Vec<u8>)> = None;
    for k in 0..frames {
        let t_ms = k * 1000 / fps;
        let Some(at) = pieces
            .iter()
            .rposition(|p| p.t_ms <= t_ms)
            .or((!pieces.is_empty()).then_some(0))
        else {
            break;
        };
        if shown.as_ref().is_none_or(|(i, _)| *i != at) {
            shown = Some((at, render(dir, &pieces[at])?.into_raw()));
        }
        let rgba = &shown.as_ref().expect("set above").1;
        // A write error means ffmpeg exited; its stderr says why.
        if stdin.write_all(rgba).is_err() {
            break;
        }
    }
    drop(stdin);

    let status = child.wait()?;
    let errors = errors.join().unwrap_or_default();
    if !status.success() {
        return Err(failed(format!(
            "ffmpeg exited with {status}: {}",
            errors.trim()
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::one_frame;

    #[test]
    fn a_zero_size_recording_fails_to_encode() {
        if !available() {
            println!("skipped: ffmpeg or ffprobe not on PATH");
            return;
        }
        let (dir, mut manifest) = one_frame("ffmpeg-zero", Vec::new());
        manifest.width = 0;
        manifest.height = 0;
        assert_eq!(encode(&dir, &manifest).unwrap_err().code(), "encode_failed");
    }
}
