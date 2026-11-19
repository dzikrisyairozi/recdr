//! macOS desktop capture with ScreenCaptureKit: the permission check, the window and
//! monitor lists in pixels, and the recording source.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TrySendError};
use std::time::Duration;

use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use screencapturekit::cm::SCFrameStatus;
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;
use screencapturekit::stream::delegate_trait::StreamCallbacks;

use super::{CaptureSource, Display, display_bounds, to_pixels, to_points};
use crate::error::{Error, Result};
use crate::model::{CaptureEvent, Frame, MonitorInfo, ResolvedKind, ResolvedTarget, WindowInfo};

pub fn list_windows() -> Result<Vec<WindowInfo>> {
    permitted()?;
    let content = content()?;
    let displays = displays(&content);
    Ok(content
        .windows()
        .iter()
        .filter_map(|w| {
            let app = w.owning_application();
            let title = w.title().unwrap_or_default();
            if !listed(w.window_layer(), &title, app.is_some()) {
                return None;
            }
            let app = app?;
            let f = w.frame();
            Some(WindowInfo {
                id: u64::from(w.window_id()),
                title,
                process: app.application_name(),
                pid: app.process_id() as u32,
                bounds: to_pixels(
                    &displays,
                    f.origin.x,
                    f.origin.y,
                    f.size.width,
                    f.size.height,
                ),
                minimized: !w.is_on_screen(),
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
    let displays = displays(&content()?);
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
}

impl SckSource {
    pub fn open(resolved: &ResolvedTarget, fps: u32, cursor: bool, _clicks: bool) -> Result<Self> {
        permitted()?;
        let content = content()?;
        let failed = |e: SCError| Error::CaptureFailed(e.to_string());
        let (filter, size, source) = match &resolved.kind {
            ResolvedKind::Monitor { index, crop } => {
                let all = content.displays();
                let display = all
                    .get(*index)
                    .ok_or_else(|| Error::TargetNotFound(format!("monitor {index}")))?;
                let displays = displays(&content);
                let (info, bounds) = (displays[*index], display_bounds(&displays)[*index]);
                let filter = SCContentFilter::create()
                    .with_display(display)
                    .with_excluding_windows(&[])
                    .build()
                    .map_err(failed)?;
                match crop {
                    Some(crop) => {
                        let (x, y, w, h) = to_points(crop, info.scale);
                        let source = CGRect::new(x, y, w, h);
                        (filter, (crop.width, crop.height), Some(source))
                    }
                    None => (filter, (bounds.width, bounds.height), None),
                }
            }
            ResolvedKind::Window(_) => {
                return Err(Error::CaptureFailed(
                    "window targets are not built yet on macOS".into(),
                ));
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
        })
    }
}

impl CaptureSource for SckSource {
    fn next_event(&mut self, timeout: Duration) -> Result<Option<CaptureEvent>> {
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

/// The displays ScreenCaptureKit reports, in its order.
fn displays(content: &SCShareableContent) -> Vec<Display> {
    content
        .displays()
        .iter()
        .map(|d| {
            let frame = d.frame();
            Display {
                x: frame.origin.x,
                y: frame.origin.y,
                width: frame.size.width,
                height: frame.size.height,
                scale: display_scale(d),
            }
        })
        .collect()
}

/// `point_pixel_scale` of a filter made for the display.
// ponytail: a display whose filter cannot be built counts as 1x; its frames would then
// be smaller than the screen, which the monitor test would show.
fn display_scale(display: &SCDisplay) -> f64 {
    SCContentFilter::create()
        .with_display(display)
        .with_excluding_windows(&[])
        .build()
        .map(|f| f64::from(f.point_pixel_scale()))
        .unwrap_or(1.0)
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
