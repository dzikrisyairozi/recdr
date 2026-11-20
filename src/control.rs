//! The operations a user can ask for. Both front ends call these and nothing below.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::analyze;
use crate::capture;
use crate::encode;
use crate::error::{Error, Result};
use crate::model::{
    Format, FrameList, FrameSelection, MarkAck, MonitorList, RecordOptions, RecordingResult,
    StartInfo, State, Status, StatusEntry, StatusList, Target, WindowList,
};
use crate::platform;
use crate::recorder;
use crate::store::RecordingDir;
use crate::time;

const POLL: Duration = Duration::from_millis(50);
const START_TIMEOUT: Duration = Duration::from_secs(5);

pub fn list_windows() -> Result<WindowList> {
    Ok(WindowList {
        windows: capture::list_windows()?,
    })
}

pub fn list_monitors() -> Result<MonitorList> {
    Ok(MonitorList {
        monitors: capture::list_monitors()?,
    })
}

/// Records in this process and blocks until the recording ends.
pub fn record(options: RecordOptions) -> Result<RecordingResult> {
    if options.duration_ms.is_none() && options.command.is_none() {
        return Err(Error::Usage(
            "record needs --duration or a command after --".into(),
        ));
    }
    let dir = prepare(options)?;
    recorder::run(&dir)
}

/// Starts a recorder in its own process and returns once it has its first frame.
pub fn start(options: RecordOptions) -> Result<StartInfo> {
    if options.command.is_some() {
        return Err(Error::Usage("start does not take a command".into()));
    }
    let dir = prepare(options)?;
    let exe = std::env::current_exe()?;
    let pid = platform::spawn_detached(
        &exe,
        &[OsStr::new("_recorder"), dir.path().as_os_str()],
        dir.log_file()?,
    )?;
    let begun = Instant::now();
    loop {
        // A read can race the recorder's rename; the next poll sees the new file.
        if let Ok(state) = dir.read_state()
            && matches!(state.status, Status::Recording | Status::Finished)
        {
            return Ok(StartInfo {
                id: state.id,
                dir: dir.path().to_path_buf(),
                width: state.width,
                height: state.height,
            });
        }
        if !platform::process_alive(pid) || begun.elapsed() >= START_TIMEOUT {
            return Err(Error::RecorderStartFailed(dir.last_log_line()));
        }
        thread::sleep(POLL);
    }
}

/// Blocks until a detached recording has finished on its own.
pub fn wait(out: &Path, id: &str) -> Result<RecordingResult> {
    await_result(&RecordingDir::open(out, id)?)
}

/// Asks a detached recording to stop and returns its result. Stopping a finished
/// recording returns the result it already has.
pub fn stop(out: &Path, id: &str) -> Result<RecordingResult> {
    let dir = RecordingDir::open(out, id)?;
    if let Some(result) = dir.read_result()? {
        return Ok(result);
    }
    dir.request_stop()?;
    await_result(&dir)
}

/// Labels the current moment of a running recording.
pub fn mark(out: &Path, id: &str, label: &str) -> Result<MarkAck> {
    let dir = RecordingDir::open(out, id)?;
    if derived_status(&dir)? != Status::Recording {
        return Err(Error::RecordingNotActive(id.to_owned()));
    }
    dir.append_mark(time::now_unix_ms(), label)?;
    Ok(MarkAck {
        id: id.to_owned(),
        label: label.to_owned(),
    })
}

/// One recording, or every recording under `out`.
pub fn status(out: &Path, id: Option<&str>) -> Result<StatusList> {
    let dirs = match id {
        Some(id) => vec![RecordingDir::open(out, id)?],
        None => RecordingDir::list(out)?,
    };
    let mut recordings = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let state = dir.read_state()?;
        let elapsed_ms = match dir.read_result()? {
            Some(result) => result.duration_ms,
            None => time::now_unix_ms().saturating_sub(state.started_at_unix_ms),
        };
        recordings.push(StatusEntry {
            id: state.id,
            dir: dir.path().to_path_buf(),
            status: derived_status(&dir)?,
            started_at: state.started_at_unix_ms,
            elapsed_ms,
        });
    }
    Ok(StatusList { recordings })
}

/// Body of the hidden `_recorder <dir>` command that `start` spawns.
pub fn run_recorder(dir: PathBuf) -> Result<()> {
    recorder::run(&RecordingDir::at(dir))
        .map(|_| ())
        .inspect_err(|e| {
            // stderr is the recording's log file; `start` reports its last line.
            eprintln!("{e}");
        })
}

/// The status a reader should believe: `Finished` once the result exists, `Crashed`
/// when the recorder process is gone without one. Never `Starting`.
fn derived_status(dir: &RecordingDir) -> Result<Status> {
    if dir.read_result()?.is_some() {
        return Ok(Status::Finished);
    }
    let state = dir.read_state()?;
    let young = time::now_unix_ms().saturating_sub(state.started_at_unix_ms)
        < START_TIMEOUT.as_millis() as u64;
    if (state.status == Status::Starting && young) || platform::process_alive(state.pid) {
        Ok(Status::Recording)
    } else {
        Ok(Status::Crashed)
    }
}

fn await_result(dir: &RecordingDir) -> Result<RecordingResult> {
    loop {
        if let Some(result) = dir.read_result()? {
            return Ok(result);
        }
        if derived_status(dir)? == Status::Crashed {
            return Err(Error::RecorderCrashed(dir.id().to_owned()));
        }
        thread::sleep(POLL);
    }
}

/// Stored frames of a finished recording, picked by time, interval or keyframe.
/// More than `max_count` picked is `TooManyFrames`, before any sample is written.
pub fn frames(
    out: &Path,
    id: &str,
    selection: &FrameSelection,
    max_width: Option<u32>,
    max_count: Option<usize>,
) -> Result<FrameList> {
    let dir = RecordingDir::open(out, id)?;
    if dir.read_result()?.is_none() {
        return Err(Error::RecordingNotFound(id.to_owned()));
    }
    let manifest = dir.read_manifest()?;
    let diffs = dir.read_diff()?;
    let entries = analyze::select::select(&manifest, &diffs, selection);
    if let Some(max) = max_count
        && entries.len() > max
    {
        return Err(Error::TooManyFrames(entries.len()));
    }
    Ok(FrameList {
        frames: analyze::select::frame_refs(&dir, &entries, max_width)?,
    })
}

/// Resolves the target and writes the initial state. Nothing is created on disk
/// when the target cannot be resolved.
fn prepare(options: RecordOptions) -> Result<RecordingDir> {
    if options.formats.contains(&Format::Mp4) && !encode::ffmpeg::available() {
        return Err(Error::FfmpegNotFound);
    }
    let (windows, monitors) = match &options.target {
        Target::WindowId(_) | Target::WindowTitle(_) | Target::Process(_) => {
            (capture::list_windows()?, Vec::new())
        }
        Target::Region(_) | Target::Monitor(_) => (Vec::new(), capture::list_monitors()?),
        Target::Fake(_) => (Vec::new(), Vec::new()),
    };
    let resolved = capture::resolve(&options.target, &windows, &monitors)?;
    let now = time::now_unix_ms();
    let dir = RecordingDir::create(&options.out, now)?;
    dir.write_state(&State {
        id: dir.id().to_owned(),
        status: Status::Starting,
        pid: 0,
        started_at_unix_ms: now,
        width: 0,
        height: 0,
        resolved,
        options,
    })?;
    Ok(dir)
}
