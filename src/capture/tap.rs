//! The mouse event tap: a listen-only CGEventTap on its own thread and run loop, which
//! hands mouse-button presses to the recording.

use std::cell::OnceCell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use objc2_core_foundation::{CFMachPort, CFRetained, CFRunLoop, kCFRunLoopCommonModes};
use objc2_core_graphics::{
    CGEvent, CGEventMask, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType, CGPreflightListenEventAccess, CGRequestListenEventAccess,
};

use super::Press;
use crate::time;

/// What the recording prints when macOS does not allow the tap.
const WARNING: &str = "clicks are not recorded: macOS has not given this program the Input Monitoring permission (System Settings > Privacy & Security > Input Monitoring)";

/// The tap thread's run loop, kept to stop it from the thread that drops the tap.
struct RunLoop(CFRetained<CFRunLoop>);
// SAFETY: the run loop is only used for CFRunLoopStop, which Core Foundation allows from
// any thread; the binding's type is not Send.
unsafe impl Send for RunLoop {}

/// What the callback reads through its `user_info` pointer.
struct State {
    presses: Sender<Press>,
    port: OnceCell<CFRetained<CFMachPort>>,
}

pub struct ClickTap {
    presses: Receiver<Press>,
    run_loop: RunLoop,
    thread: Option<JoinHandle<()>>,
}

impl ClickTap {
    /// None, after printing `WARNING` to stderr, when macOS does not allow the tap.
    pub fn start() -> Option<Self> {
        // On some versions macOS creates a tap for a program without the permission and
        // sends it nothing; a recording must not report clicks as recorded then.
        let allowed = CGPreflightListenEventAccess() || CGRequestListenEventAccess();
        if !allowed {
            eprintln!("{WARNING}");
            return None;
        }
        let (presses_tx, presses) = mpsc::channel();
        let (report_tx, report) = mpsc::sync_channel::<Option<RunLoop>>(1);
        let thread = std::thread::spawn(move || run(presses_tx, report_tx));
        let Some(run_loop) = report.recv().ok().flatten() else {
            let _ = thread.join();
            eprintln!("{WARNING}");
            return None;
        };
        Some(Self {
            presses,
            run_loop,
            thread: Some(thread),
        })
    }

    /// The next press the tap has seen, without waiting.
    pub fn try_next(&self) -> Option<Press> {
        self.presses.try_recv().ok()
    }
}

impl Drop for ClickTap {
    fn drop(&mut self) {
        self.run_loop.0.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The tap's thread: creates the tap, reports its run loop (or None), runs it.
fn run(presses: Sender<Press>, report: mpsc::SyncSender<Option<RunLoop>>) {
    let state = Box::into_raw(Box::new(State {
        presses,
        port: OnceCell::new(),
    }));
    // SAFETY: `callback` has the CGEventTapCallBack signature, and `state` stays valid
    // until the box is rebuilt below, after the run loop has returned.
    let port = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            mask(),
            Some(callback),
            state.cast(),
        )
    };
    let source = port
        .as_ref()
        .and_then(|port| CFMachPort::new_run_loop_source(None, Some(port), 0));
    let (Some(port), Some(source), Some(run_loop)) = (port, source, CFRunLoop::current()) else {
        let _ = report.send(None);
        // SAFETY: from Box::into_raw above; no tap uses it.
        drop(unsafe { Box::from_raw(state) });
        return;
    };
    // SAFETY: only this thread uses `state`, and the callback runs on this thread.
    let _ = unsafe { &*state }.port.set(port);
    // SAFETY: kCFRunLoopCommonModes is a constant Core Foundation defines.
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
    let _ = report.send(Some(RunLoop(run_loop)));
    CFRunLoop::run();
    // SAFETY: the run loop has returned, so the callback no longer runs; the port is
    // dropped with the box.
    drop(unsafe { Box::from_raw(state) });
}

unsafe extern "C-unwind" fn callback(
    _proxy: CGEventTapProxy,
    kind: CGEventType,
    event: NonNull<CGEvent>,
    user_info: *mut c_void,
) -> *mut CGEvent {
    // SAFETY: `user_info` is the State box `run` keeps alive while the run loop runs.
    let state = unsafe { &*(user_info as *const State) };
    let button = match kind {
        CGEventType::LeftMouseDown => Some("left"),
        CGEventType::RightMouseDown => Some("right"),
        CGEventType::OtherMouseDown => Some("middle"),
        _ => None,
    };
    if let Some(button) = button {
        // SAFETY: the event is valid for the duration of the callback.
        let at = CGEvent::location(Some(unsafe { event.as_ref() }));
        let _ = state
            .presses
            .send((time::now_unix_ms(), at.x, at.y, button));
    } else if kind == CGEventType::TapDisabledByTimeout
        && let Some(port) = state.port.get()
    {
        CGEvent::tap_enable(port, true);
    }
    event.as_ptr()
}

/// The tap's event mask: left, right and other mouse-button presses.
fn mask() -> CGEventMask {
    [
        CGEventType::LeftMouseDown,
        CGEventType::RightMouseDown,
        CGEventType::OtherMouseDown,
    ]
    .iter()
    .fold(0, |mask, kind| mask | (1 << kind.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn mask_is_the_three_presses() {
        assert_eq!(mask(), 0x0200_000A);
    }

    #[test]
    fn start_then_drop_returns() {
        let Some(tap) = ClickTap::start() else {
            return;
        };
        let (done_tx, done) = mpsc::channel();
        std::thread::spawn(move || {
            drop(tap);
            let _ = done_tx.send(());
        });
        assert!(
            done.recv_timeout(Duration::from_secs(2)).is_ok(),
            "drop did not return"
        );
    }
}
