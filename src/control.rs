//! The operations a user can ask for. Both front ends call these and nothing below.

use crate::capture;
use crate::error::{Error, Result};
use crate::model::{
    MonitorList, RecordOptions, RecordingResult, State, Status, Target, WindowList,
};
use crate::recorder;
use crate::store::RecordingDir;
use crate::time;

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

/// Resolves the target and writes the initial state. Nothing is created on disk
/// when the target cannot be resolved.
fn prepare(options: RecordOptions) -> Result<RecordingDir> {
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
