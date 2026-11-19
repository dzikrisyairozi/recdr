//! One recording, from the first frame to `result.json`.

use std::process::Child;
use std::time::{Duration, Instant};

use crate::analyze;
use crate::capture;
use crate::encode;
use crate::error::{Error, Result};
use crate::model::{
    CaptureEvent, Click, Format, FrameSelection, Manifest, Mark, RecordingResult, ResolvedKind,
    State, Status, StopReason,
};
use crate::platform;
use crate::store::RecordingDir;
use crate::time;

const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(50);

/// Runs the recording described by the directory's `state.json` until a stop
/// condition holds, then writes every output.
pub fn run(dir: &RecordingDir) -> Result<RecordingResult> {
    let mut state = dir.read_state()?;
    let (manifest, command_exit_code) = capture(dir, &mut state)?;
    finalize(dir, &mut state, manifest, command_exit_code)
}

fn capture(dir: &RecordingDir, state: &mut State) -> Result<(Manifest, Option<i32>)> {
    let options = state.options.clone();
    let mut source = capture::open(&state.resolved, options.fps, options.cursor, options.clicks)?;

    let waited = Instant::now();
    let first = loop {
        match source.next_event(Duration::from_millis(100))? {
            Some(CaptureEvent::Frame(frame)) => break frame,
            Some(CaptureEvent::Closed) => {
                return Err(Error::CaptureFailed(
                    "the target closed before the first frame".into(),
                ));
            }
            None if waited.elapsed() >= FIRST_FRAME_TIMEOUT => {
                return Err(Error::CaptureFailed(
                    "no frame arrived within 5 seconds".into(),
                ));
            }
            Some(CaptureEvent::Click { .. }) | None => {}
        }
    };

    // Frames, marks and clicks all count from this one moment.
    let start = Instant::now();
    state.started_at_unix_ms = time::now_unix_ms();
    state.status = Status::Recording;
    state.pid = std::process::id();
    state.width = first.width;
    state.height = first.height;
    dir.write_state(state)?;

    let mut writer = dir.frame_writer(first.width, first.height)?;
    writer.push(first, 0)?;
    // Later frames are stored on the writer's own thread, so this loop keeps reading
    // its source, timing clicks and checking for the end while a frame is encoded.
    let mut writer = writer.into_thread()?;

    let mut child = match &options.command {
        Some(command) => Some(platform::spawn_command(command)?),
        None => None,
    };
    let mut command_exit_code = None;
    let mut raw_clicks = Vec::new();

    let interval_ms = u64::from(1000 / options.fps);
    // At most one frame per 1/fps slot of the timeline. Counting slots, not the time since
    // the last stored frame, keeps the rate when frames arrive late: a late frame does not
    // push every later slot back.
    let mut last_slot = 0;
    let (stop_reason, duration_ms) = loop {
        let event = source.next_event(POLL)?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let mut closed = false;
        match event {
            // A paced source sends its next frame only after this one is handed on, so
            // it waits for the writer rather than lose a frame (cdp-capture criterion 4).
            Some(CaptureEvent::Frame(frame)) if source.paced() => {
                writer.push_waiting(frame, elapsed_ms)?;
                last_slot = elapsed_ms / interval_ms;
            }
            Some(CaptureEvent::Frame(frame)) if elapsed_ms / interval_ms > last_slot => {
                writer.push(frame, elapsed_ms)?;
                last_slot = elapsed_ms / interval_ms;
            }
            Some(CaptureEvent::Frame(_)) | None => {}
            Some(CaptureEvent::Click {
                unix_ms,
                x,
                y,
                button,
            }) => raw_clicks.push((unix_ms, x, y, button)),
            Some(CaptureEvent::Closed) => closed = true,
        }
        let reason = if closed {
            Some(if matches!(state.resolved.kind, ResolvedKind::Tab(_)) {
                StopReason::TabClosed
            } else {
                StopReason::WindowClosed
            })
        } else if dir.stop_requested() {
            Some(StopReason::Stopped)
        } else if let Some(status) = child.as_mut().map(Child::try_wait).transpose()?.flatten() {
            command_exit_code = status.code();
            Some(StopReason::CommandExited)
        } else if options.duration_ms.is_some_and(|d| elapsed_ms >= d) {
            Some(StopReason::DurationReached)
        } else if elapsed_ms >= options.max_duration_ms {
            Some(StopReason::CapReached)
        } else if writer.bytes() >= options.max_size_bytes {
            Some(StopReason::SizeCapReached)
        } else {
            None
        };
        if let Some(reason) = reason {
            break (reason, elapsed_ms);
        }
    };

    let (frames, skipped) = writer.finish()?;
    let dropped_frames = source.dropped() + skipped;
    drop(source);
    if let Some(mut child) = child
        && stop_reason != StopReason::CommandExited
    {
        platform::kill_tree(&mut child);
    }
    let clicks = clicks(raw_clicks, state.started_at_unix_ms, duration_ms);

    let marks = dir
        .read_marks()?
        .into_iter()
        .filter_map(|(unix_ms, label)| {
            let t_ms = unix_ms.checked_sub(state.started_at_unix_ms)?;
            (t_ms <= duration_ms).then_some(Mark { t_ms, label })
        })
        .collect();

    let manifest = Manifest {
        id: state.id.clone(),
        width: state.width,
        height: state.height,
        fps: options.fps,
        started_at_unix_ms: state.started_at_unix_ms,
        duration_ms,
        frames,
        marks,
        clicks,
        stop_reason,
        dropped_frames,
    };
    Ok((manifest, command_exit_code))
}

/// Presses timed from the first frame, keeping those inside the recording, the rule
/// marks follow.
fn clicks(
    raw: Vec<(u64, i32, i32, &'static str)>,
    started_at_unix_ms: u64,
    duration_ms: u64,
) -> Vec<Click> {
    raw.into_iter()
        .filter_map(|(unix_ms, x, y, button)| {
            let t_ms = unix_ms.checked_sub(started_at_unix_ms)?;
            (t_ms <= duration_ms).then(|| Click {
                t_ms,
                x,
                y,
                button: button.to_owned(),
            })
        })
        .collect()
}

fn finalize(
    dir: &RecordingDir,
    state: &mut State,
    manifest: Manifest,
    command_exit_code: Option<i32>,
) -> Result<RecordingResult> {
    let manifest_path = dir.write_manifest(&manifest)?;
    let diffs = analyze::run(dir, &manifest)?;
    let contact_sheet = analyze::sheet::contact_sheet(dir, &manifest, &diffs)?;
    let keyframes = analyze::select::select(&manifest, &diffs, &FrameSelection::Keyframes);
    let keyframes = analyze::select::frame_refs(dir, &keyframes, None)?;
    let gif = if state.options.formats.contains(&Format::Gif) {
        Some(encode::gif::encode(dir, &manifest)?)
    } else {
        None
    };
    let mp4 = if state.options.formats.contains(&Format::Mp4) {
        Some(encode::mp4(dir, &manifest)?)
    } else {
        None
    };
    let result = RecordingResult {
        id: manifest.id.clone(),
        dir: dir.path().to_path_buf(),
        stop_reason: manifest.stop_reason,
        command_exit_code,
        duration_ms: manifest.duration_ms,
        width: manifest.width,
        height: manifest.height,
        frames_stored: manifest.frames.len() as u32,
        dropped_frames: manifest.dropped_frames,
        keyframes,
        marks: manifest.marks.clone(),
        gif,
        mp4,
        contact_sheet,
        diff: dir.diff_path(),
        manifest: manifest_path,
    };
    dir.write_result(&result)?;
    state.status = Status::Finished;
    dir.write_state(state)?;
    Ok(result)
}

/// Exposed for tests of the stop conditions that are slow or awkward to reach from the CLI.
#[cfg(test)]
pub(crate) fn prepared(options: crate::model::RecordOptions, name: &str) -> RecordingDir {
    use crate::model::{FakeKind, ResolvedKind, ResolvedTarget};
    let out = std::env::temp_dir().join(format!("recdr-unit-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let dir = RecordingDir::create(&out, 0).unwrap();
    dir.write_state(&State {
        id: dir.id().to_owned(),
        status: Status::Starting,
        pid: 0,
        started_at_unix_ms: 0,
        width: 0,
        height: 0,
        resolved: ResolvedTarget {
            kind: ResolvedKind::Fake(FakeKind::Moving),
            origin: (0, 0),
        },
        options,
        error: None,
    })
    .unwrap();
    dir
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FakeKind, RecordOptions, Target};

    fn options() -> RecordOptions {
        RecordOptions {
            target: Target::Fake(FakeKind::Moving),
            fps: 15,
            max_duration_ms: 10_000,
            max_size_bytes: 2_000_000_000,
            formats: Vec::new(),
            cursor: true,
            clicks: false,
            out: std::path::PathBuf::new(),
            duration_ms: None,
            command: None,
        }
    }

    #[test]
    fn clicks_are_timed_from_the_first_frame() {
        let raw = vec![
            (900, 1, 1, "left"),
            (1200, 2, 3, "right"),
            (1600, 4, 4, "left"),
        ];
        let kept = clicks(raw, 1000, 500);
        assert_eq!(
            kept,
            vec![Click {
                t_ms: 200,
                x: 2,
                y: 3,
                button: "right".into(),
            }]
        );
    }

    #[test]
    fn size_cap_stops_the_recording() {
        let dir = prepared(
            RecordOptions {
                max_size_bytes: 1,
                ..options()
            },
            "size-cap",
        );
        assert_eq!(run(&dir).unwrap().stop_reason, StopReason::SizeCapReached);
    }

    #[test]
    fn time_cap_stops_a_recording_with_no_duration() {
        let dir = prepared(
            RecordOptions {
                max_duration_ms: 300,
                ..options()
            },
            "time-cap",
        );
        let result = run(&dir).unwrap();
        assert_eq!(result.stop_reason, StopReason::CapReached);
        assert_eq!(dir.read_state().unwrap().status, Status::Finished);
    }
}
