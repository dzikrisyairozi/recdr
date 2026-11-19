//! macOS desktop capture with ScreenCaptureKit: the permission check, the window and
//! monitor lists in pixels, and the recording source.

use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use screencapturekit::prelude::*;

use super::{Display, display_bounds, to_pixels};
use crate::error::{Error, Result};
use crate::model::{MonitorInfo, WindowInfo};

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
