//! Reads the codec, size and length of an MP4 file from its headers, with the standard
//! library only. Used by the integration tests and, through `#[path]`, by the unit tests
//! of `src/encode/mod.rs`.

use std::path::Path;

pub struct Mp4Facts {
    /// The four characters that name the video codec, `avc1` for H.264.
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
}

/// Reads the facts from an MP4 file's headers. Panics with the path when the file is not an MP4.
pub fn facts(path: &Path) -> Mp4Facts {
    let bytes =
        std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    parse(&bytes).unwrap_or_else(|| panic!("{} is not an MP4 file", path.display()))
}

/// The facts, or None when the bytes are not an MP4 with a video track.
fn parse(bytes: &[u8]) -> Option<Mp4Facts> {
    let top = all(bytes);
    if top.first()?.0 != *b"ftyp" {
        return None;
    }
    let moov = *boxes(bytes, b"moov").first()?;

    for trak in boxes(moov, b"trak") {
        let Some(mdia) = boxes(trak, b"mdia").first().copied() else {
            continue;
        };
        // hdlr: 4 bytes of version and flags, 4 pre-defined, then the handler type.
        let handler = boxes(mdia, b"hdlr")
            .first()
            .and_then(|hdlr| hdlr.get(8..12));
        if handler != Some(b"vide") {
            continue;
        }
        let Some(stsd) = [b"minf", b"stbl", b"stsd"]
            .iter()
            .try_fold(mdia, |content, name| boxes(content, name).first().copied())
        else {
            continue;
        };
        // The track's own length, in its mdhd; mvhd's is the longest track's.
        let mdhd = *boxes(mdia, b"mdhd").first()?;
        let (scale, duration) = match *mdhd.first()? {
            0 => (u32_at(mdhd, 12)?, u64::from(u32_at(mdhd, 16)?)),
            1 => (u32_at(mdhd, 20)?, u64_at(mdhd, 24)?),
            _ => return None,
        };
        if scale == 0 {
            return None;
        }
        let duration_ms = duration * 1000 / u64::from(scale);
        // 4 bytes of version and flags, 4 of entry count, then the first entry.
        let entry = stsd.get(8..)?;
        let width = u32::from(u16_at(entry, 32)?);
        if width == 0 {
            continue;
        }
        return Some(Mp4Facts {
            codec: String::from_utf8_lossy(entry.get(4..8)?).into_owned(),
            width,
            height: u32::from(u16_at(entry, 34)?),
            duration_ms,
        });
    }
    None
}

/// The content of every box named `name` directly inside `content`, in order.
fn boxes<'a>(content: &'a [u8], name: &[u8; 4]) -> Vec<&'a [u8]> {
    all(content)
        .into_iter()
        .filter(|(n, _)| n == name)
        .map(|(_, c)| c)
        .collect()
}

/// Every box directly inside `content`, as (name, content). Stops at a box whose size is
/// smaller than its header or runs past the end.
fn all(content: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut found = Vec::new();
    let mut at = 0;
    while let (Some(size), Some(name)) = (u32_at(content, at), content.get(at + 4..at + 8)) {
        let (size, header) = match size {
            1 => match u64_at(content, at + 8) {
                Some(size) => (size, 16),
                None => break,
            },
            size => (u64::from(size), 8),
        };
        let Ok(size) = usize::try_from(size) else {
            break;
        };
        if size < header {
            break;
        }
        let Some(inner) = content.get(at + header..at + size) else {
            break;
        };
        found.push((name.try_into().expect("four bytes"), inner));
        at += size;
    }
    found
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}
