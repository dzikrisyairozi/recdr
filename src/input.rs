//! Mouse clicks, through a low-level mouse hook. Keyboard input is never hooked.

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, HC_ACTION, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW,
    PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_MOUSE_LL, WM_LBUTTONDOWN,
    WM_MBUTTONDOWN, WM_QUIT, WM_RBUTTONDOWN,
};

use crate::error::{Error, Result};
use crate::model::Click;
use crate::time;

type RawClick = (u64, i32, i32, &'static str);

/// The hook procedure has no user data pointer, so it reaches the recorder through this.
static SENDER: Mutex<Option<Sender<RawClick>>> = Mutex::new(None);

pub struct ClickHook {
    thread: JoinHandle<()>,
    thread_id: u32,
    clicks: Receiver<RawClick>,
}

impl ClickHook {
    pub fn start() -> Result<Self> {
        let (sender, clicks) = mpsc::channel();
        {
            let mut slot = SENDER.lock().unwrap_or_else(|e| e.into_inner());
            if slot.is_some() {
                return Err(Error::CaptureFailed(
                    "a click hook is already running in this process".into(),
                ));
            }
            *slot = Some(sender);
        }
        let (ready_tx, ready) = mpsc::channel();
        let thread = thread::spawn(move || hook_thread(&ready_tx));
        match ready.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                thread,
                thread_id,
                clicks,
            }),
            Ok(Err(message)) => {
                let _ = thread.join();
                clear_sender();
                Err(Error::CaptureFailed(message))
            }
            Err(_) => {
                clear_sender();
                Err(Error::CaptureFailed("the click hook thread ended".into()))
            }
        }
    }

    /// Stops the hook and returns the clicks inside the captured rectangle, relative
    /// to its top-left corner and timed from `started_at_unix_ms`.
    pub fn finish(
        self,
        origin: (i32, i32),
        width: u32,
        height: u32,
        started_at_unix_ms: u64,
    ) -> Vec<Click> {
        // SAFETY: posts a message to a thread id this hook owns; no memory is shared.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        let _ = self.thread.join();
        clear_sender();
        let (x0, y0) = origin;
        self.clicks
            .try_iter()
            .filter_map(|(unix_ms, x, y, button)| {
                let (dx, dy) = (x - x0, y - y0);
                let inside = (0..width as i32).contains(&dx) && (0..height as i32).contains(&dy);
                inside.then(|| Click {
                    t_ms: unix_ms.saturating_sub(started_at_unix_ms),
                    x: dx,
                    y: dy,
                    button: button.to_owned(),
                })
            })
            .collect()
    }
}

fn clear_sender() {
    *SENDER.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Installs the hook and pumps messages until WM_QUIT; the hook only runs while this
/// thread is in GetMessageW.
fn hook_thread(ready: &Sender<std::result::Result<u32, String>>) {
    // SAFETY: plain Win32 calls on this thread's own message queue; `msg` is a local
    // the calls write into, and the hook is removed before the thread ends.
    unsafe {
        let mut msg = MSG::default();
        // Creates the message queue so WM_QUIT from `finish` cannot be lost.
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        let hook = match SetWindowsHookExW(WH_MOUSE_LL, Some(on_mouse), None, 0) {
            Ok(hook) => hook,
            Err(e) => {
                let _ = ready.send(Err(format!("the click hook failed: {e}")));
                return;
            }
        };
        let _ = ready.send(Ok(GetCurrentThreadId()));
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {}
        let _ = UnhookWindowsHookEx(hook);
    }
}

unsafe extern "system" fn on_mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let button = match wparam.0 as u32 {
            WM_LBUTTONDOWN => Some("left"),
            WM_RBUTTONDOWN => Some("right"),
            WM_MBUTTONDOWN => Some("middle"),
            _ => None,
        };
        if let Some(button) = button {
            // SAFETY: for WH_MOUSE_LL with HC_ACTION, lparam points to an MSLLHOOKSTRUCT
            // that is valid for the duration of this call.
            let point = unsafe { (*(lparam.0 as *const MSLLHOOKSTRUCT)).pt };
            if let Some(sender) = SENDER.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                let _ = sender.send((time::now_unix_ms(), point.x, point.y, button));
            }
        }
    }
    // SAFETY: passes the hook arguments on unchanged, as the hook chain requires.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs a desktop session"]
    fn only_one_hook_at_a_time() {
        let first = ClickHook::start().unwrap();
        assert_eq!(ClickHook::start().err().unwrap().code(), "capture_failed");
        assert!(first.finish((0, 0), 1, 1, 0).is_empty());
        ClickHook::start().unwrap().finish((0, 0), 1, 1, 0);
    }
}
