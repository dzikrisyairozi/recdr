//! The recording directory. This is the only module that knows its file names.

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, RgbaImage, imageops};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{DiffEntry, Frame, FrameEntry, Manifest, RecordingResult, State};
use crate::time;

const STATE: &str = "state.json";
const MANIFEST: &str = "manifest.json";
const RESULT: &str = "result.json";
const DIFF: &str = "diff.json";
const MARKS: &str = "marks.jsonl";
const STOP: &str = "stop";
const LOG: &str = "recorder.log";
const FRAMES: &str = "frames";
const SAMPLES: &str = "samples";
const CONTACT_SHEET: &str = "contact_sheet.png";
const GIF: &str = "recording.gif";
const MP4: &str = "recording.mp4";

pub struct RecordingDir {
    path: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct MarkLine {
    unix_ms: u64,
    label: String,
}

#[derive(Serialize, Deserialize)]
struct DiffFile {
    frames: Vec<DiffEntry>,
}

impl RecordingDir {
    /// Makes `<out>/<id>/` with a fresh id, and `<out>/.gitignore` so recordings
    /// are never committed by accident.
    pub fn create(out: &Path, now_unix_ms: u64) -> Result<Self> {
        fs::create_dir_all(out)?;
        let ignore = out.join(".gitignore");
        if !ignore.exists() {
            fs::write(&ignore, "*\n")?;
        }
        let stamp = time::utc_stamp(now_unix_ms);
        let mut suffix = ((now_unix_ms ^ u64::from(std::process::id())) & 0xffff) as u16;
        let mut tries = 0;
        loop {
            let path = out.join(format!("{stamp}-{suffix:04x}"));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::create_dir(path.join(FRAMES))?;
                    return Ok(Self { path });
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists && tries < 15 => {
                    tries += 1;
                    suffix = suffix.wrapping_add(1);
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub fn open(out: &Path, id: &str) -> Result<Self> {
        let path = out.join(id);
        if !path.join(STATE).is_file() {
            return Err(Error::RecordingNotFound(id.to_owned()));
        }
        Ok(Self { path })
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn list(out: &Path) -> Result<Vec<Self>> {
        let entries = match fs::read_dir(out) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut dirs = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.join(STATE).is_file() {
                dirs.push(Self { path });
            }
        }
        dirs.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(dirs)
    }

    pub fn id(&self) -> &str {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_state(&self) -> Result<State> {
        self.read_json(STATE)
    }

    pub fn write_state(&self, state: &State) -> Result<()> {
        self.write_json(STATE, state).map(|_| ())
    }

    pub fn read_manifest(&self) -> Result<Manifest> {
        self.read_json(MANIFEST)
    }

    pub fn write_manifest(&self, manifest: &Manifest) -> Result<PathBuf> {
        self.write_json(MANIFEST, manifest)
    }

    pub fn read_diff(&self) -> Result<Vec<DiffEntry>> {
        Ok(self.read_json::<DiffFile>(DIFF)?.frames)
    }

    pub fn write_diff(&self, entries: &[DiffEntry]) -> Result<PathBuf> {
        let file = DiffFile {
            frames: entries.to_vec(),
        };
        self.write_json(DIFF, &file)
    }

    pub fn read_result(&self) -> Result<Option<RecordingResult>> {
        match fs::read(self.path.join(RESULT)) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_result(&self, result: &RecordingResult) -> Result<()> {
        self.write_json(RESULT, result).map(|_| ())
    }

    pub fn request_stop(&self) -> Result<()> {
        File::create(self.path.join(STOP))?;
        Ok(())
    }

    pub fn stop_requested(&self) -> bool {
        self.path.join(STOP).exists()
    }

    pub fn append_mark(&self, unix_ms: u64, label: &str) -> Result<()> {
        let mut line = serde_json::to_string(&MarkLine {
            unix_ms,
            label: label.to_owned(),
        })?;
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path.join(MARKS))?;
        // One write per line, so concurrent `mark` calls never interleave inside a line.
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    pub fn read_marks(&self) -> Result<Vec<(u64, String)>> {
        let text = match fs::read_to_string(self.path.join(MARKS)) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let mark: MarkLine = serde_json::from_str(line)?;
                Ok((mark.unix_ms, mark.label))
            })
            .collect()
    }

    pub fn frame_writer(&self, width: u32, height: u32) -> Result<FrameWriter> {
        fs::create_dir_all(self.path.join(FRAMES))?;
        Ok(FrameWriter {
            dir: self.path.clone(),
            width,
            height,
            frames: Vec::new(),
            last_rgba: None,
            bytes: 0,
        })
    }

    pub fn frame_path(&self, entry: &FrameEntry) -> PathBuf {
        // `file` uses `/` so the manifest reads the same everywhere; join part by part
        // so the printed path uses the platform separator throughout.
        entry
            .file
            .split('/')
            .fold(self.path.clone(), |path, part| path.join(part))
    }

    pub fn load_frame(&self, entry: &FrameEntry) -> Result<RgbaImage> {
        Ok(image::open(self.frame_path(entry))?.to_rgba8())
    }

    pub fn sample_path(&self, entry: &FrameEntry, max_width: u32) -> Result<PathBuf> {
        let dir = self.path.join(SAMPLES);
        fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("{:06}-w{max_width}.png", entry.index)))
    }

    pub fn contact_sheet_path(&self) -> PathBuf {
        self.path.join(CONTACT_SHEET)
    }

    pub fn diff_path(&self) -> PathBuf {
        self.path.join(DIFF)
    }

    pub fn gif_path(&self) -> PathBuf {
        self.path.join(GIF)
    }

    pub fn mp4_path(&self) -> PathBuf {
        self.path.join(MP4)
    }

    pub fn log_file(&self) -> Result<File> {
        Ok(File::create(self.path.join(LOG))?)
    }

    pub fn last_log_line(&self) -> String {
        fs::read_to_string(self.path.join(LOG))
            .unwrap_or_default()
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim()
            .to_owned()
    }

    fn read_json<T: DeserializeOwned>(&self, name: &str) -> Result<T> {
        Ok(serde_json::from_slice(&fs::read(self.path.join(name))?)?)
    }

    /// Writes to a temporary name, then renames, so a process polling the file
    /// never reads half of it.
    fn write_json<T: Serialize>(&self, name: &str, value: &T) -> Result<PathBuf> {
        let path = self.path.join(name);
        let tmp = self.path.join(format!("{name}.tmp"));
        fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
        fs::rename(&tmp, &path)?;
        Ok(path)
    }
}

/// Stores frames on its own thread, so the recorder keeps reading its source while a
/// frame is encoded. A frame that arrives while another is still waiting replaces it,
/// and the replaced one counts as skipped: a slow disk or CPU lowers the frame rate.
pub struct WriterThread {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<Result<FrameWriter>>>,
}

struct Shared {
    mailbox: Mutex<Mailbox>,
    ready: Condvar,
    bytes: AtomicU64,
}

#[derive(Default)]
struct Mailbox {
    waiting: Option<(Frame, u64)>,
    done: bool,
    skipped: u64,
}

impl WriterThread {
    /// Hands the frame to the writer thread and returns at once.
    pub fn push(&mut self, frame: Frame, t_ms: u64) -> Result<()> {
        // The thread ends early only on an error; report it here.
        if self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return self.join().map(|_| ());
        }
        let mut mailbox = self.shared.mailbox.lock().expect("frame writer lock");
        if mailbox.waiting.replace((frame, t_ms)).is_some() {
            mailbox.skipped += 1;
        }
        self.shared.ready.notify_one();
        Ok(())
    }

    /// Bytes of the PNG files stored so far.
    pub fn bytes(&self) -> u64 {
        self.shared.bytes.load(Ordering::Relaxed)
    }

    /// Stores the waiting frame, ends the thread, and returns the stored frames and
    /// the number skipped.
    pub fn finish(mut self) -> Result<(Vec<FrameEntry>, u64)> {
        self.close();
        let frames = self.join()?;
        let skipped = self
            .shared
            .mailbox
            .lock()
            .expect("frame writer lock")
            .skipped;
        Ok((frames.frames, skipped))
    }

    fn close(&self) {
        self.shared.mailbox.lock().expect("frame writer lock").done = true;
        self.shared.ready.notify_one();
    }

    fn join(&mut self) -> Result<FrameWriter> {
        let thread = self
            .thread
            .take()
            .ok_or_else(|| Error::Io(std::io::Error::other("the frame writer has ended")))?;
        thread
            .join()
            .map_err(|_| Error::Io(std::io::Error::other("the frame writer panicked")))?
    }
}

impl Drop for WriterThread {
    fn drop(&mut self) {
        self.close();
    }
}

fn store_frames(mut frames: FrameWriter, shared: &Shared) -> Result<FrameWriter> {
    loop {
        let next = {
            let mut mailbox = shared.mailbox.lock().expect("frame writer lock");
            loop {
                // The waiting frame is taken before `done` is looked at, so `finish`
                // stores the last frame.
                if let Some(next) = mailbox.waiting.take() {
                    break Some(next);
                }
                if mailbox.done {
                    break None;
                }
                mailbox = shared.ready.wait(mailbox).expect("frame writer lock");
            }
        };
        let Some((frame, t_ms)) = next else {
            return Ok(frames);
        };
        frames.push(frame, t_ms)?;
        shared.bytes.store(frames.bytes, Ordering::Relaxed);
    }
}

/// Writes changed frames as numbered PNG files, on the caller's thread.
pub struct FrameWriter {
    dir: PathBuf,
    width: u32,
    height: u32,
    frames: Vec<FrameEntry>,
    last_rgba: Option<Vec<u8>>,
    bytes: u64,
}

impl FrameWriter {
    /// Stores the frame unless it is identical to the last stored one. A frame of
    /// another size is letterboxed to the writer's size first.
    pub fn push(&mut self, frame: Frame, t_ms: u64) -> Result<bool> {
        let frame = letterbox(frame, self.width, self.height);
        if self.last_rgba.as_deref() == Some(frame.rgba.as_slice()) {
            return Ok(false);
        }
        let index = self.frames.len() as u32 + 1;
        let file = format!("{FRAMES}/{index:06}.png");
        let path = self.dir.join(&file);
        let mut out = BufWriter::new(File::create(&path)?);
        PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive)
            .write_image(
                &frame.rgba,
                frame.width,
                frame.height,
                ExtendedColorType::Rgba8,
            )?;
        out.flush()?;
        drop(out);
        self.bytes += fs::metadata(&path)?.len();
        self.frames.push(FrameEntry { index, t_ms, file });
        self.last_rgba = Some(frame.rgba);
        Ok(true)
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn finish(self) -> Vec<FrameEntry> {
        self.frames
    }

    /// Moves the writer to its own thread for the frames still to come.
    pub fn into_thread(self) -> Result<WriterThread> {
        let shared = Arc::new(Shared {
            mailbox: Mutex::new(Mailbox::default()),
            ready: Condvar::new(),
            bytes: AtomicU64::new(self.bytes),
        });
        let thread_shared = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("frame-writer".into())
            .spawn(move || store_frames(self, &thread_shared))?;
        Ok(WriterThread {
            shared,
            thread: Some(thread),
        })
    }
}

/// Scales the frame to fit `width` x `height`, keeping its aspect ratio, centred on black.
pub fn letterbox(frame: Frame, width: u32, height: u32) -> Frame {
    if frame.width == width && frame.height == height {
        return frame;
    }
    let mut canvas = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 255]));
    if let Some(source) = RgbaImage::from_raw(frame.width, frame.height, frame.rgba)
        && frame.width > 0
        && frame.height > 0
    {
        let scale = f64::min(
            f64::from(width) / f64::from(frame.width),
            f64::from(height) / f64::from(frame.height),
        );
        let w = ((f64::from(frame.width) * scale).round() as u32).clamp(1, width);
        let h = ((f64::from(frame.height) * scale).round() as u32).clamp(1, height);
        let scaled = imageops::resize(&source, w, h, imageops::FilterType::Triangle);
        imageops::replace(
            &mut canvas,
            &scaled,
            i64::from((width - w) / 2),
            i64::from((height - h) / 2),
        );
    }
    Frame {
        width,
        height,
        rgba: canvas.into_raw(),
    }
}

/// The stored frame that was on screen at `t_ms`.
pub fn frame_at(frames: &[FrameEntry], t_ms: u64) -> Option<&FrameEntry> {
    frames
        .iter()
        .rev()
        .find(|f| f.t_ms <= t_ms)
        .or_else(|| frames.first())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FakeKind, RecordOptions, ResolvedKind, ResolvedTarget, Status, Target};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("recdr-unit-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Frame {
        Frame {
            width,
            height,
            rgba: rgba.repeat((width * height) as usize),
        }
    }

    fn state(id: &str) -> State {
        State {
            id: id.into(),
            status: Status::Starting,
            pid: 0,
            started_at_unix_ms: 5,
            width: 0,
            height: 0,
            resolved: ResolvedTarget {
                kind: ResolvedKind::Fake(FakeKind::Moving),
                origin: (0, 0),
            },
            options: RecordOptions {
                target: Target::Fake(FakeKind::Moving),
                fps: 15,
                max_duration_ms: 1000,
                max_size_bytes: 1000,
                formats: Vec::new(),
                cursor: true,
                clicks: true,
                out: PathBuf::new(),
                duration_ms: None,
                command: None,
            },
            error: None,
        }
    }

    #[test]
    fn create_twice_gives_two_ids_and_a_gitignore() {
        let out = temp("create");
        let a = RecordingDir::create(&out, 1_795_012_502_000).unwrap();
        let b = RecordingDir::create(&out, 1_795_012_502_000).unwrap();
        assert_ne!(a.id(), b.id());
        assert!(a.id().starts_with("20261118-143502-"));
        assert!(b.id().starts_with("20261118-143502-"));
        assert_eq!(fs::read_to_string(out.join(".gitignore")).unwrap(), "*\n");
        assert!(a.path().join(FRAMES).is_dir());
    }

    #[test]
    fn open_unknown_id_is_not_found_and_list_of_missing_out_is_empty() {
        let out = temp("open");
        assert_eq!(
            RecordingDir::open(&out, "nope").err().unwrap().code(),
            "recording_not_found"
        );
        assert!(RecordingDir::list(&out).unwrap().is_empty());
    }

    #[test]
    fn state_round_trips_without_leaving_a_temp_file() {
        let out = temp("state");
        let dir = RecordingDir::create(&out, 0).unwrap();
        dir.write_state(&state(dir.id())).unwrap();
        assert_eq!(dir.read_state().unwrap().started_at_unix_ms, 5);
        assert!(!dir.path().join("state.json.tmp").exists());
        assert!(dir.read_result().unwrap().is_none());
        let reopened = RecordingDir::open(&out, dir.id()).unwrap();
        assert_eq!(reopened.id(), dir.id());
        assert_eq!(RecordingDir::list(&out).unwrap().len(), 1);
    }

    #[test]
    fn stop_and_marks() {
        let out = temp("marks");
        let dir = RecordingDir::create(&out, 0).unwrap();
        assert!(!dir.stop_requested());
        dir.request_stop().unwrap();
        assert!(dir.stop_requested());
        dir.append_mark(10, "one").unwrap();
        dir.append_mark(20, "two, with \"quotes\"").unwrap();
        assert_eq!(
            dir.read_marks().unwrap(),
            [(10, "one".into()), (20, "two, with \"quotes\"".into())]
        );
    }

    #[test]
    fn frame_writer_skips_repeats_and_counts_bytes() {
        let out = temp("writer");
        let dir = RecordingDir::create(&out, 0).unwrap();
        let mut writer = dir.frame_writer(4, 4).unwrap();
        assert!(writer.push(solid(4, 4, [0, 0, 0, 255]), 0).unwrap());
        assert!(!writer.push(solid(4, 4, [0, 0, 0, 255]), 10).unwrap());
        assert!(writer.push(solid(4, 4, [255, 0, 0, 255]), 20).unwrap());
        let bytes = writer.bytes();
        let frames = writer.finish();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].t_ms, 0);
        assert_eq!(frames[1].t_ms, 20);
        let on_disk: u64 = frames
            .iter()
            .map(|f| fs::metadata(dir.frame_path(f)).unwrap().len())
            .sum();
        assert_eq!(bytes, on_disk);
    }

    #[test]
    fn frame_of_another_size_is_stored_at_writer_size() {
        let out = temp("resize");
        let dir = RecordingDir::create(&out, 0).unwrap();
        let mut writer = dir.frame_writer(4, 4).unwrap();
        writer.push(solid(8, 4, [255, 0, 0, 255]), 0).unwrap();
        let frames = writer.finish();
        let image = dir.load_frame(&frames[0]).unwrap();
        assert_eq!(image.dimensions(), (4, 4));
    }

    #[test]
    fn writer_thread_keeps_the_last_frame_and_counts_the_skipped() {
        let out = temp("writer-thread");
        let dir = RecordingDir::create(&out, 0).unwrap();
        let mut first = dir.frame_writer(64, 64).unwrap();
        first.push(solid(64, 64, [0, 0, 0, 255]), 0).unwrap();
        let mut writer = first.into_thread().unwrap();
        for i in 1..=20u8 {
            writer
                .push(solid(64, 64, [i, 0, 0, 255]), u64::from(i))
                .unwrap();
        }
        let (frames, skipped) = writer.finish().unwrap();
        assert_eq!(frames.first().unwrap().t_ms, 0);
        assert_eq!(frames.last().unwrap().t_ms, 20);
        // Every pushed frame differs from the one before, so each is stored or skipped.
        assert_eq!(frames.len() as u64 - 1 + skipped, 20);
    }

    #[test]
    fn letterbox_centres_on_black() {
        let boxed = letterbox(solid(200, 100, [255, 0, 0, 255]), 100, 100);
        let image = RgbaImage::from_raw(100, 100, boxed.rgba).unwrap();
        assert_eq!(image.get_pixel(50, 50).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn frame_at_picks_the_frame_on_screen() {
        let entry = |t_ms| FrameEntry {
            index: 0,
            t_ms,
            file: String::new(),
        };
        let frames = [entry(100), entry(200), entry(300)];
        assert_eq!(frame_at(&frames, 50).unwrap().t_ms, 100);
        assert_eq!(frame_at(&frames, 200).unwrap().t_ms, 200);
        assert_eq!(frame_at(&frames, 250).unwrap().t_ms, 200);
        assert_eq!(frame_at(&frames, 999).unwrap().t_ms, 300);
        assert!(frame_at(&[], 0).is_none());
    }
}
