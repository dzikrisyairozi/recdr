//! Turns raw user input, from either front end, into validated record options.

use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::model::{FakeKind, Format, RecordOptions, Rect, Target};
use crate::time::parse_time;

pub const DEFAULT_FPS: u32 = 15;
pub const DEFAULT_MAX_DURATION: &str = "5m";
pub const DEFAULT_MAX_SIZE: &str = "2GB";
pub const DEFAULT_FORMAT: &str = "gif";
pub const DEFAULT_OUT: &str = ".recdr";
pub const FAKE_SOURCE_ENV: &str = "RECDR_FAKE_SOURCE";

/// What a user asked for, before validation. Every field is optional so the CLI
/// and the MCP server can fill it from their own input shapes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RecordRequest {
    pub window_id: Option<u64>,
    pub window: Option<String>,
    pub process: Option<String>,
    pub region: Option<String>,
    pub monitor: Option<usize>,
    pub fps: Option<u32>,
    pub max_duration: Option<String>,
    pub max_size: Option<String>,
    pub format: Option<String>,
    pub no_cursor: bool,
    pub no_click_highlight: bool,
    pub out: Option<PathBuf>,
    pub duration: Option<String>,
    #[serde(skip)]
    pub command: Option<Vec<String>>,
}

impl RecordRequest {
    pub fn into_options(self) -> Result<RecordOptions> {
        let mut targets = Vec::new();
        if let Some(id) = self.window_id {
            targets.push(Target::WindowId(id));
        }
        if let Some(text) = self.window {
            targets.push(Target::WindowTitle(text));
        }
        if let Some(name) = self.process {
            targets.push(Target::Process(name));
        }
        if let Some(text) = self.region {
            targets.push(Target::Region(parse_region(&text)?));
        }
        if let Some(index) = self.monitor {
            targets.push(Target::Monitor(index));
        }
        if targets.len() != 1 {
            return Err(usage(
                "give exactly one of --window-id, --window, --process, --region, --monitor",
            ));
        }
        let mut target = targets.remove(0);
        if let Ok(value) = std::env::var(FAKE_SOURCE_ENV) {
            target = Target::Fake(fake_kind(&value)?);
        }

        let fps = self.fps.unwrap_or(DEFAULT_FPS);
        if !(1..=60).contains(&fps) {
            return Err(usage("--fps must be between 1 and 60"));
        }
        let max_duration_ms =
            parse_time(self.max_duration.as_deref().unwrap_or(DEFAULT_MAX_DURATION))?;
        let duration_ms = self.duration.as_deref().map(parse_time).transpose()?;
        if duration_ms.is_some_and(|d| d > max_duration_ms) {
            return Err(usage("--duration is longer than --max-duration"));
        }
        if duration_ms.is_some() && self.command.is_some() {
            return Err(usage("give --duration or a command after --, not both"));
        }

        Ok(RecordOptions {
            target,
            fps,
            max_duration_ms,
            max_size_bytes: parse_size(self.max_size.as_deref().unwrap_or(DEFAULT_MAX_SIZE))?,
            formats: parse_formats(self.format.as_deref().unwrap_or(DEFAULT_FORMAT))?,
            cursor: !self.no_cursor,
            clicks: !self.no_click_highlight,
            out: resolve_out(self.out)?,
            duration_ms,
            command: self.command,
        })
    }
}

fn usage(message: &str) -> Error {
    Error::Usage(message.to_owned())
}

fn fake_kind(value: &str) -> Result<FakeKind> {
    match value {
        "static" => Ok(FakeKind::Static),
        "moving" => Ok(FakeKind::Moving),
        "closing" => Ok(FakeKind::Closing),
        "broken" => Ok(FakeKind::Broken),
        other => Err(Error::Usage(format!(
            "{FAKE_SOURCE_ENV}={other:?} is not static, moving, closing or broken"
        ))),
    }
}

/// `500MB` or `2GB` to bytes, with 1 MB = 1,000,000 bytes.
pub fn parse_size(text: &str) -> Result<u64> {
    let bad = || {
        Error::Usage(format!(
            "{text:?} is not a size; use a whole number with MB or GB"
        ))
    };
    let (digits, factor) = if let Some(n) = text.strip_suffix("MB") {
        (n, 1_000_000)
    } else if let Some(n) = text.strip_suffix("GB") {
        (n, 1_000_000_000)
    } else {
        return Err(bad());
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    digits
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(factor))
        .ok_or_else(bad)
}

/// `x,y,w,h` in physical pixels.
pub fn parse_region(text: &str) -> Result<Rect> {
    let bad = || Error::Usage(format!("{text:?} is not a region; use x,y,width,height"));
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    let [x, y, w, h] = parts.as_slice() else {
        return Err(bad());
    };
    let rect = Rect {
        x: x.parse().map_err(|_| bad())?,
        y: y.parse().map_err(|_| bad())?,
        width: w.parse().map_err(|_| bad())?,
        height: h.parse().map_err(|_| bad())?,
    };
    if rect.width == 0 || rect.height == 0 {
        return Err(bad());
    }
    Ok(rect)
}

/// `gif`, `mp4` or both, comma separated. A repeated format counts once.
pub fn parse_formats(text: &str) -> Result<Vec<Format>> {
    let mut formats = Vec::new();
    for part in text.split(',').map(str::trim) {
        let format = match part {
            "gif" => Format::Gif,
            "mp4" => Format::Mp4,
            other => return Err(Error::Usage(format!("{other:?} is not gif or mp4"))),
        };
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    Ok(formats)
}

/// The output root, made absolute so a detached recorder and later commands agree on it.
pub fn resolve_out(out: Option<PathBuf>) -> Result<PathBuf> {
    let out = out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    Ok(std::path::absolute(out)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_units() {
        assert_eq!(parse_size("2GB").unwrap(), 2_000_000_000);
        assert_eq!(parse_size("1MB").unwrap(), 1_000_000);
        for text in ["2", "2KB", "-1MB", "MB"] {
            assert_eq!(parse_size(text).unwrap_err().code(), "usage", "{text}");
        }
    }

    #[test]
    fn parse_region_reads_four_numbers_and_rejects_zero_size() {
        assert_eq!(
            parse_region("10,20,640,480").unwrap(),
            Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480
            }
        );
        assert_eq!(parse_region("-1920,0,10,10").unwrap().x, -1920);
        for text in ["1,2,3", "1,2,0,4", "a,b,c,d"] {
            assert_eq!(parse_region(text).unwrap_err().code(), "usage", "{text}");
        }
    }

    #[test]
    fn parse_formats_both_and_deduplicated() {
        assert_eq!(
            parse_formats("gif,mp4").unwrap(),
            [Format::Gif, Format::Mp4]
        );
        assert_eq!(parse_formats("gif,gif").unwrap(), [Format::Gif]);
        assert_eq!(parse_formats("webm").unwrap_err().code(), "usage");
    }
}
