//! Process calls on Linux and macOS.

use std::ffi::OsStr;
use std::fs::File;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::error::{Error, Result};

/// Nothing to do: only Windows scales coordinates for processes that do not ask.
pub fn set_dpi_aware() {}

/// While the process with this id is running, the time it was started (0 where the
/// system does not say). A dead process's id is given to new processes, so a caller
/// that knows the start time can tell its process from a later one.
pub fn process_started(pid: u32) -> Option<u64> {
    process_alive(pid).then(|| super::start_time(pid).unwrap_or(0))
}

/// True while the process with this id is running.
fn process_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid == 0 {
        return false;
    }
    // A detached recorder that has exited stays a zombie while the process that
    // started it is alive (the MCP server, for example), and kill(pid, 0) succeeds on
    // a zombie. Reaping it first tells the two apart.
    // SAFETY: waitpid with a null status pointer and WNOHANG only reaps an exited
    // child of ours; for any other pid it fails and changes nothing.
    if unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) } == pid {
        return false;
    }
    // SAFETY: signal 0 only checks that the process exists; nothing is delivered.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Starts `exe` in its own session with no terminal, its stderr going to `stderr`,
/// and returns its process id.
pub fn spawn_detached(exe: &Path, args: &[&OsStr], stderr: File) -> Result<u32> {
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr));
    // SAFETY: setsid is async-signal-safe and touches no memory of the parent.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(command.spawn()?.id())
}

/// Starts the wrapped command with its stdout going to our stderr. No shell: the
/// first word is the program.
pub fn spawn_command(command: &[String]) -> Result<Child> {
    let Some((program, args)) = command.split_first() else {
        return Err(Error::Usage("the command is empty".into()));
    };
    // Its own process group, so kill_tree can end everything it started.
    Ok(Command::new(program)
        .args(args)
        .stdout(Stdio::from(std::io::stderr()))
        .process_group(0)
        .spawn()?)
}

/// Ends the command and every process it started, then waits for it.
pub fn kill_tree(child: &mut Child) {
    if let Ok(pid) = libc::pid_t::try_from(child.id()) {
        // SAFETY: sends a signal to the process group spawn_command created; no
        // memory is shared.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let _ = child.wait();
}
