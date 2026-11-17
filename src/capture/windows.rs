//! Windows Graphics Capture backend and window and monitor enumeration.

use std::ffi::c_void;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO};
use windows::Win32::UI::WindowsAndMessaging::{IsIconic, MONITORINFOF_PRIMARY};
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame as WgcFrame;
use windows_capture::graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl};
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

use super::CaptureSource;
use crate::error::{Error, Result};
use crate::model::{
    CaptureEvent, Frame, MonitorInfo, Rect, ResolvedKind, ResolvedTarget, WindowInfo,
};

fn rect(r: RECT) -> Rect {
    Rect {
        x: r.left,
        y: r.top,
        width: (r.right - r.left).max(0) as u32,
        height: (r.bottom - r.top).max(0) as u32,
    }
}

/// Visible top-level windows. One window that cannot be read is skipped or listed
/// with blanks; it never fails the whole list.
pub fn list_windows() -> Result<Vec<WindowInfo>> {
    let windows = Window::enumerate().map_err(|e| Error::CaptureFailed(e.to_string()))?;
    let mut out = Vec::new();
    for window in windows {
        let (Ok(title), Ok(bounds)) = (window.title(), window.rect()) else {
            continue;
        };
        if title.is_empty() {
            continue;
        }
        let hwnd = window.as_raw_hwnd();
        // SAFETY: the handle came from EnumWindows; IsIconic only reads window state
        // and returns false for a handle that has since been destroyed.
        let minimized = unsafe { IsIconic(HWND(hwnd)) }.as_bool();
        out.push(WindowInfo {
            id: hwnd as usize as u64,
            title,
            process: window.process_name().unwrap_or_default(),
            pid: window.process_id().unwrap_or(0),
            bounds: rect(bounds),
            minimized,
        });
    }
    Ok(out)
}

/// Monitors in enumeration order. `index` is the position in this list, which is
/// what `Monitor::from_index(index + 1)` opens.
pub fn list_monitors() -> Result<Vec<MonitorInfo>> {
    let monitors = Monitor::enumerate().map_err(|e| Error::CaptureFailed(e.to_string()))?;
    let mut out = Vec::new();
    for (index, monitor) in monitors.iter().enumerate() {
        let mut info = MONITORINFO {
            cbSize: mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let handle = HMONITOR(monitor.as_raw_hmonitor());
        // SAFETY: `info` is a properly sized MONITORINFO on our stack and the handle
        // came from EnumDisplayMonitors.
        if !unsafe { GetMonitorInfoW(handle, &mut info) }.as_bool() {
            return Err(Error::CaptureFailed(format!(
                "could not read monitor {index}"
            )));
        }
        out.push(MonitorInfo {
            index,
            bounds: rect(info.rcMonitor),
            primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }
    Ok(out)
}

/// Flags handed to the capture thread.
struct HandlerFlags {
    events: SyncSender<CaptureEvent>,
    dropped: Arc<AtomicU64>,
    crop: Option<Rect>,
}

struct Handler {
    flags: HandlerFlags,
    scratch: Vec<u8>,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = HandlerFlags;
    type Error = String;

    fn new(ctx: Context<Self::Flags>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            flags: ctx.flags,
            scratch: Vec::new(),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut WgcFrame,
        _control: InternalCaptureControl,
    ) -> std::result::Result<(), Self::Error> {
        let (frame_width, frame_height) = (frame.width(), frame.height());
        let buffer = match self.flags.crop {
            Some(crop) => {
                // A monitor texture can be smaller than the crop asked for (DPI or a
                // mode change); never read past its edge.
                let x = (crop.x.max(0) as u32).min(frame_width.saturating_sub(1));
                let y = (crop.y.max(0) as u32).min(frame_height.saturating_sub(1));
                let end_x = (x + crop.width).min(frame_width);
                let end_y = (y + crop.height).min(frame_height);
                frame.buffer_crop(x, y, end_x, end_y)
            }
            None => frame.buffer(),
        }
        .map_err(|e| e.to_string())?;
        let (width, height) = (buffer.width(), buffer.height());
        let rgba = buffer.as_nopadding_buffer(&mut self.scratch).to_vec();
        let event = CaptureEvent::Frame(Frame {
            width,
            height,
            rgba,
        });
        match self.flags.events.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.flags.dropped.fetch_add(1, Ordering::Relaxed);
            }
            // The recorder has stopped listening and is about to stop this capture.
            Err(TrySendError::Disconnected(_)) => {}
        }
        Ok(())
    }

    fn on_closed(&mut self) -> std::result::Result<(), Self::Error> {
        // Blocking send: losing this event would leave the recording running.
        let _ = self.flags.events.send(CaptureEvent::Closed);
        Ok(())
    }
}

/// A running WGC capture of one window or monitor.
pub struct WgcSource {
    events: Receiver<CaptureEvent>,
    control: Option<CaptureControl<Handler, String>>,
    dropped: Arc<AtomicU64>,
}

impl WgcSource {
    pub fn open(resolved: &ResolvedTarget, fps: u32, cursor: bool) -> Result<Self> {
        let (events_tx, events) = mpsc::sync_channel(8);
        let dropped = Arc::new(AtomicU64::new(0));
        let failed = |e: &dyn std::fmt::Display| Error::CaptureFailed(e.to_string());

        let cursor = if cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        };
        // Windows 10 cannot turn the yellow border off or limit the frame rate, and
        // asking anyway fails the capture, so ask only where it is supported.
        let border = if GraphicsCaptureApi::is_border_settings_supported().unwrap_or(false) {
            DrawBorderSettings::WithoutBorder
        } else {
            DrawBorderSettings::Default
        };
        let interval =
            if GraphicsCaptureApi::is_minimum_update_interval_supported().unwrap_or(false) {
                MinimumUpdateIntervalSettings::Custom(Duration::from_millis(u64::from(1000 / fps)))
            } else {
                MinimumUpdateIntervalSettings::Default
            };
        let flags = |crop| HandlerFlags {
            events: events_tx.clone(),
            dropped: Arc::clone(&dropped),
            crop,
        };

        let control = match resolved.kind {
            ResolvedKind::Window(id) => {
                let window = Window::from_raw_hwnd(id as usize as *mut c_void);
                let settings = Settings::new(
                    window,
                    cursor,
                    border,
                    SecondaryWindowSettings::Default,
                    interval,
                    DirtyRegionSettings::Default,
                    ColorFormat::Rgba8,
                    flags(None),
                );
                Handler::start_free_threaded(settings).map_err(|e| failed(&e))?
            }
            ResolvedKind::Monitor { index, crop } => {
                let monitor = Monitor::from_index(index + 1).map_err(|e| failed(&e))?;
                let settings = Settings::new(
                    monitor,
                    cursor,
                    border,
                    SecondaryWindowSettings::Default,
                    interval,
                    DirtyRegionSettings::Default,
                    ColorFormat::Rgba8,
                    flags(crop),
                );
                Handler::start_free_threaded(settings).map_err(|e| failed(&e))?
            }
            ResolvedKind::Fake(_) => {
                return Err(Error::CaptureFailed(
                    "a synthetic target has no WGC source".into(),
                ));
            }
        };
        Ok(Self {
            events,
            control: Some(control),
            dropped,
        })
    }
}

impl CaptureSource for WgcSource {
    fn next_event(&mut self, timeout: Duration) -> Result<Option<CaptureEvent>> {
        match self.events.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                Err(Error::CaptureFailed("the capture thread ended".into()))
            }
        }
    }

    fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl Drop for WgcSource {
    fn drop(&mut self) {
        // `stop` consumes the control, hence the Option.
        if let Some(control) = self.control.take() {
            let _ = control.stop();
        }
    }
}
