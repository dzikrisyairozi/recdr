//! Win32 calls that are not screen capture.

use std::ffi::OsStr;
use std::fs::File;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use windows::Win32::Foundation::{
    CloseHandle, FILETIME, HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, STILL_ACTIVE,
    SetHandleInformation,
};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

use crate::error::Result;

const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Makes every rectangle this process sees physical pixels. A failure (already set,
/// or an old Windows build) is ignored: coordinates are then scaled, not wrong in kind.
pub fn set_dpi_aware() {
    // SAFETY: takes a constant context value and touches no memory we own.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

/// While the process with this id is running, the time it was started, in the
/// system's 100 ns units. Windows gives a dead process's id to new processes, so a
/// caller that knows the start time can tell its process from a later one.
pub fn process_started(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    // SAFETY: OpenProcess returns an owned handle that is closed below; the exit
    // code and the times are written into locals.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut code = 0u32;
        let running =
            GetExitCodeProcess(process, &mut code).is_ok() && code == STILL_ACTIVE.0 as u32;
        let (mut created, mut exited, mut kernel, mut user) = Default::default();
        let timed =
            GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user).is_ok();
        let _ = CloseHandle(process);
        let created: FILETIME = created;
        (running && timed)
            .then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }
}

/// Starts `exe` with no console and no inherited standard handles, its stderr going
/// to `stderr`, and returns its process id.
pub fn spawn_detached(exe: &Path, args: &[&OsStr], stderr: File) -> Result<u32> {
    // Without this, a caller reading our stdout to end of file would wait for the
    // child too, because the child would hold an inherited copy of the pipe.
    let handles = [
        std::io::stdin().as_raw_handle(),
        std::io::stdout().as_raw_handle(),
        std::io::stderr().as_raw_handle(),
    ];
    set_inherit(&handles, false);
    let spawned = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn();
    set_inherit(&handles, true);
    Ok(spawned?.id())
}

fn set_inherit(handles: &[std::os::windows::io::RawHandle], inherit: bool) {
    let flags = if inherit {
        HANDLE_FLAG_INHERIT
    } else {
        HANDLE_FLAGS(0)
    };
    for &raw in handles {
        let handle = HANDLE(raw);
        if handle.is_invalid() || raw.is_null() {
            continue;
        }
        // SAFETY: the handle is one of this process's standard handles, open for the
        // life of the process; only its inherit flag changes.
        let _ = unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, flags) };
    }
}
