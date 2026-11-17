//! Windows Graphics Capture backend and window and monitor enumeration.

use std::mem;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO};
use windows::Win32::UI::WindowsAndMessaging::{IsIconic, MONITORINFOF_PRIMARY};
use windows_capture::monitor::Monitor;
use windows_capture::window::Window;

use crate::error::{Error, Result};
use crate::model::{MonitorInfo, Rect, WindowInfo};

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
