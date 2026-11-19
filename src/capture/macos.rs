//! macOS desktop capture: the permission check, the window and monitor lists in pixels
//! (from CoreGraphics), and the recording source (ScreenCaptureKit).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TrySendError};
use std::time::Duration;

use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType,
};
use objc2_core_graphics::{
    CGDisplayBounds, CGDisplayCopyDisplayMode, CGDisplayMode, CGGetActiveDisplayList,
    CGMainDisplayID, CGPreflightScreenCaptureAccess, CGRectMakeWithDictionaryRepresentation,
    CGRequestScreenCaptureAccess, CGWindowListCopyWindowInfo, CGWindowListOption, kCGWindowBounds,
    kCGWindowIsOnscreen, kCGWindowLayer, kCGWindowName, kCGWindowNumber, kCGWindowOwnerName,
    kCGWindowOwnerPID,
};
use screencapturekit::cm::SCFrameStatus;
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;
use screencapturekit::stream::delegate_trait::StreamCallbacks;

use super::tap::ClickTap;
use super::{CaptureSource, Display, display_bounds, to_event, to_pixels, to_points};
use crate::error::{Error, Result};
use crate::model::{CaptureEvent, Frame, MonitorInfo, ResolvedKind, ResolvedTarget, WindowInfo};

pub fn list_windows() -> Result<Vec<WindowInfo>> {
    permitted()?;
    let displays: Vec<Display> = cg_displays().into_iter().map(|(_, d)| d).collect();
    let Some(list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, 0) else {
        return Err(Error::CaptureFailed(
            "macOS did not list the windows".into(),
        ));
    };
    // SAFETY: CGWindowListCopyWindowInfo returns an array of dictionaries whose keys
    // are strings.
    let list: CFRetained<CFArray<CFDictionary<CFString, CFType>>> =
        unsafe { CFRetained::cast_unchecked(list) };
    Ok(list
        .iter()
        .filter_map(|window| {
            let text = |key: &CFString| {
                let value = window.get(key)?.downcast::<CFString>().ok()?;
                Some(value.to_string())
            };
            let number = |key: &CFString| window.get(key)?.downcast::<CFNumber>().ok()?.as_i64();
            // SAFETY: the keys are statics CoreGraphics defines.
            let (id_key, layer_key, name_key, owner_key, pid_key, bounds_key, onscreen_key) = unsafe {
                (
                    kCGWindowNumber,
                    kCGWindowLayer,
                    kCGWindowName,
                    kCGWindowOwnerName,
                    kCGWindowOwnerPID,
                    kCGWindowBounds,
                    kCGWindowIsOnscreen,
                )
            };
            let title = text(name_key).unwrap_or_default();
            let process = text(owner_key).unwrap_or_default();
            let layer = number(layer_key)? as i32;
            if !listed(layer, &title, !process.is_empty()) {
                return None;
            }
            let bounds = window.get(bounds_key)?.downcast::<CFDictionary>().ok()?;
            let mut frame = objc2_core_foundation::CGRect::default();
            // SAFETY: `bounds` is the window's bounds dictionary and `frame` a valid
            // out pointer.
            if !unsafe { CGRectMakeWithDictionaryRepresentation(Some(&bounds), &mut frame) } {
                return None;
            }
            let on_screen = window
                .get(onscreen_key)
                .and_then(|v| v.downcast::<CFBoolean>().ok())
                .is_some_and(|v| v.as_bool());
            Some(WindowInfo {
                id: number(id_key)? as u64,
                title,
                process,
                pid: number(pid_key)? as u32,
                bounds: to_pixels(
                    &displays,
                    frame.origin.x,
                    frame.origin.y,
                    frame.size.width,
                    frame.size.height,
                ),
                minimized: !on_screen,
            })
        })
        .collect())
}

/// The window filter: an ordinary window (layer 0) with a title, owned by an application.
fn listed(layer: i32, title: &str, has_application: bool) -> bool {
    layer == 0 && !title.is_empty() && has_application
}

pub fn list_monitors() -> Result<Vec<MonitorInfo>> {
    permitted()?;
    let displays: Vec<Display> = cg_displays().into_iter().map(|(_, d)| d).collect();
    Ok(display_bounds(&displays)
        .into_iter()
        .zip(&displays)
        .enumerate()
        .map(|(index, (bounds, d))| MonitorInfo {
            index,
            bounds,
            primary: d.x == 0.0 && d.y == 0.0,
        })
        .collect())
}

/// The active displays in CoreGraphics' order, with their ids. The lists come from
/// CoreGraphics, not ScreenCaptureKit: a process that has asked ScreenCaptureKit for
/// its shareable content and stays alive (`start`, the MCP server) keeps the recorder
/// it starts from ever getting its stream started (tasks/4-macos-capture, Task 8).
fn cg_displays() -> Vec<(u32, Display)> {
    let mut ids = [0u32; 32];
    let mut count = 0u32;
    // SAFETY: `ids` holds 32 entries and `count` is a valid out pointer.
    let status = unsafe { CGGetActiveDisplayList(32, ids.as_mut_ptr(), &mut count) };
    if status.0 != 0 {
        return Vec::new();
    }
    ids[..count as usize]
        .iter()
        .map(|&id| {
            let frame = CGDisplayBounds(id);
            let display = Display {
                x: frame.origin.x,
                y: frame.origin.y,
                width: frame.size.width,
                height: frame.size.height,
                scale: display_scale(id),
            };
            (id, display)
        })
        .collect()
}

/// Pixels per point of the display's current mode.
// ponytail: a display without a mode counts as 1x; its frames would then be smaller
// than the screen, which the monitor test would show.
fn display_scale(id: u32) -> f64 {
    let Some(mode) = CGDisplayCopyDisplayMode(id) else {
        return 1.0;
    };
    let points = CGDisplayMode::width(Some(&mode));
    if points == 0 {
        return 1.0;
    }
    CGDisplayMode::pixel_width(Some(&mode)) as f64 / points as f64
}

/// What the stream hands the recorder: a complete frame, or the stop notice.
enum Item {
    Frame(Frame),
    Stopped(String),
}

pub struct SckSource {
    stream: SCStream,
    items: Receiver<Item>,
    dropped: Arc<AtomicU64>,
    window: bool,
    tap: Option<ClickTap>,
    /// The captured area's top-left corner in points, its size in pixels, and pixels
    /// per point, for the click filter.
    corner: (f64, f64),
    size: (u32, u32),
    scale: f64,
}

impl SckSource {
    pub fn open(resolved: &ResolvedTarget, fps: u32, cursor: bool, clicks: bool) -> Result<Self> {
        permitted()?;
        let content = content()?;
        let failed = |e: SCError| Error::CaptureFailed(e.to_string());
        let (filter, size, source, corner, scale) = match &resolved.kind {
            ResolvedKind::Monitor { index, crop } => {
                let all = cg_displays();
                let not_found = || Error::TargetNotFound(format!("monitor {index}"));
                let (id, info) = *all.get(*index).ok_or_else(not_found)?;
                let displays: Vec<Display> = all.iter().map(|(_, d)| *d).collect();
                let bounds = display_bounds(&displays)[*index];
                let display = content
                    .displays()
                    .into_iter()
                    .find(|d| d.display_id() == id)
                    .ok_or_else(not_found)?;
                let filter = SCContentFilter::create()
                    .with_display(&display)
                    .with_excluding_windows(&[])
                    .build()
                    .map_err(failed)?;
                match crop {
                    Some(crop) => {
                        let (x, y, w, h) = to_points(crop, info.scale);
                        let source = CGRect::new(x, y, w, h);
                        let corner = (info.x + x, info.y + y);
                        let size = (crop.width, crop.height);
                        (filter, size, Some(source), corner, info.scale)
                    }
                    None => {
                        let size = (bounds.width, bounds.height);
                        (filter, size, None, (info.x, info.y), info.scale)
                    }
                }
            }
            ResolvedKind::Window(id) => {
                let window = content
                    .windows()
                    .into_iter()
                    .find(|w| u64::from(w.window_id()) == *id)
                    .ok_or_else(|| Error::TargetNotFound(format!("window id {id}")))?;
                // A window filter aborts the process in CGS_REQUIRE_INIT unless
                // CoreGraphics has connected to the window server; this connects it.
                CGMainDisplayID();
                let filter = SCContentFilter::create()
                    .with_window(&window)
                    .build()
                    .map_err(failed)?;
                let (rect, scale) = (filter.content_rect(), f64::from(filter.point_pixel_scale()));
                let size = (
                    (rect.size.width * scale).round() as u32,
                    (rect.size.height * scale).round() as u32,
                );
                (filter, size, None, (rect.origin.x, rect.origin.y), scale)
            }
            ResolvedKind::Fake(_) | ResolvedKind::Tab(_) => {
                return Err(Error::CaptureFailed(
                    "this target has no ScreenCaptureKit source".into(),
                ));
            }
        };
        let mut config = SCStreamConfiguration::new()
            .with_width(size.0)
            .with_height(size.1)
            .with_minimum_frame_interval(&CMTime::new(1, fps as i32))
            .with_shows_cursor(cursor)
            .with_pixel_format(PixelFormat::BGRA)
            .with_scales_to_fit(true);
        if let Some(source) = source {
            config = config.with_source_rect(source);
        }

        let (tx, items) = mpsc::sync_channel::<Item>(2);
        let dropped = Arc::new(AtomicU64::new(0));
        let stop_tx = tx.clone();
        // The blocking send: the stop notice is never the one discarded.
        let callbacks = StreamCallbacks::new().on_error(move |e: SCError| {
            let _ = stop_tx.send(Item::Stopped(e.to_string()));
        });
        let mut stream =
            SCStream::new_with_delegate(&filter, &config, callbacks).map_err(failed)?;
        let counter = Arc::clone(&dropped);
        let handler = move |sample: CMSampleBuffer, kind: SCStreamOutputType| {
            if !matches!(kind, SCStreamOutputType::Screen)
                || sample.frame_status() != Some(SCFrameStatus::Complete)
            {
                return;
            }
            let Some(frame) = to_frame(&sample) else {
                return;
            };
            if let Err(TrySendError::Full(_)) = tx.try_send(Item::Frame(frame)) {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        };
        stream
            .add_output_handler(handler, SCStreamOutputType::Screen)
            .map_err(failed)?;
        if let Err(e) = stream.start_capture() {
            // Decision 7: a refused stream is told apart from a broken one by asking again.
            permitted()?;
            return Err(failed(e));
        }
        Ok(Self {
            stream,
            items,
            dropped,
            window: matches!(resolved.kind, ResolvedKind::Window(_)),
            tap: if clicks { ClickTap::start() } else { None },
            corner,
            size,
            scale,
        })
    }
}

impl CaptureSource for SckSource {
    fn next_event(&mut self, timeout: Duration) -> Result<Option<CaptureEvent>> {
        while let Some(press) = self.tap.as_ref().and_then(ClickTap::try_next) {
            if let Some(click) = to_event(press, self.corner, self.size, self.scale) {
                return Ok(Some(click));
            }
        }
        match self.items.recv_timeout(timeout) {
            Ok(Item::Frame(frame)) => Ok(Some(CaptureEvent::Frame(frame))),
            Ok(Item::Stopped(_)) if self.window => Ok(Some(CaptureEvent::Closed)),
            Ok(Item::Stopped(message)) => Err(Error::CaptureFailed(format!(
                "macOS stopped the stream: {message}"
            ))),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                Err(Error::CaptureFailed("the capture stream ended".into()))
            }
        }
    }

    fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl Drop for SckSource {
    fn drop(&mut self) {
        let _ = self.stream.stop_capture();
    }
}

/// A BGRA sample to an RGBA frame with packed rows.
fn to_frame(sample: &CMSampleBuffer) -> Option<Frame> {
    let buffer = sample.pixel_buffer()?;
    let guard = buffer.lock(CVPixelBufferLockFlags::READ_ONLY).ok()?;
    let (width, height, stride) = (guard.width(), guard.height(), guard.bytes_per_row());
    // SAFETY: the guard keeps the buffer locked for reading while the slice is used,
    // and the slice is dropped before the guard.
    let bytes = unsafe { guard.as_slice() }?;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in bytes.chunks(stride).take(height) {
        for [b, g, r, a] in row[..width * 4].as_chunks::<4>().0 {
            rgba.extend_from_slice(&[*r, *g, *b, *a]);
        }
    }
    Some(Frame {
        width: width as u32,
        height: height as u32,
        rgba,
    })
}

/// Ok when macOS lets this program read the screen. Otherwise asks macOS to show its
/// dialog and returns `permission_denied`.
fn permitted() -> Result<()> {
    if CGPreflightScreenCaptureAccess() {
        return Ok(());
    }
    CGRequestScreenCaptureAccess();
    Err(Error::PermissionDenied)
}

fn content() -> Result<SCShareableContent> {
    SCShareableContent::get()
        .map_err(|e| Error::CaptureFailed(format!("macOS did not list the screen: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_wants_layer_0_a_title_and_an_application() {
        assert!(listed(0, "anim", true));
        assert!(!listed(25, "anim", true));
        assert!(!listed(0, "", true));
        assert!(!listed(0, "anim", false));
    }
}
