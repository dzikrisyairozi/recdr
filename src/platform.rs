//! Win32 calls that are not screen capture.

use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

/// Makes every rectangle this process sees physical pixels. A failure (already set,
/// or an old Windows build) is ignored: coordinates are then scaled, not wrong in kind.
pub fn set_dpi_aware() {
    // SAFETY: takes a constant context value and touches no memory we own.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}
