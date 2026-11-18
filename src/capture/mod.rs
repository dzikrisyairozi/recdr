//! Listing windows and monitors, resolving a target, producing frames.

mod cdp;
mod fake;
#[cfg(windows)]
mod hook;
#[cfg(windows)]
mod windows;

use std::time::Duration;

use crate::error::{Error, Result};
use crate::model::{
    CaptureEvent, Endpoint, FakeKind, MonitorInfo, Rect, ResolvedKind, ResolvedTarget, TabInfo,
    Target, WindowInfo,
};

pub trait CaptureSource {
    /// Ok(None) when nothing arrived within `timeout`.
    fn next_event(&mut self, timeout: Duration) -> Result<Option<CaptureEvent>>;
    /// Frames the source discarded because the consumer was behind.
    fn dropped(&self) -> u64;
}

/// `clicks` asks the source to report presses; the synthetic source ignores it.
pub fn open(
    resolved: &ResolvedTarget,
    fps: u32,
    cursor: bool,
    clicks: bool,
) -> Result<Box<dyn CaptureSource>> {
    match resolved.kind {
        ResolvedKind::Fake(FakeKind::Broken) => Err(Error::CaptureFailed(
            "the synthetic source is broken".into(),
        )),
        ResolvedKind::Fake(kind) => Ok(Box::new(fake::FakeSource::new(kind))),
        ResolvedKind::Window(_) | ResolvedKind::Monitor { .. } => {
            desktop_source(resolved, fps, cursor, clicks)
        }
    }
}

#[cfg(windows)]
fn desktop_source(
    resolved: &ResolvedTarget,
    fps: u32,
    cursor: bool,
    clicks: bool,
) -> Result<Box<dyn CaptureSource>> {
    Ok(Box::new(windows::WgcSource::open(
        resolved, fps, cursor, clicks,
    )?))
}

#[cfg(not(windows))]
fn desktop_source(
    _resolved: &ResolvedTarget,
    _fps: u32,
    _cursor: bool,
    _clicks: bool,
) -> Result<Box<dyn CaptureSource>> {
    Err(Error::UnsupportedPlatform)
}

/// The tabs of the browser at `endpoint`.
pub fn list_tabs(endpoint: &Endpoint) -> Result<Vec<TabInfo>> {
    cdp::list_tabs(endpoint)
}

#[cfg(windows)]
pub fn list_windows() -> Result<Vec<WindowInfo>> {
    windows::list_windows()
}

#[cfg(not(windows))]
pub fn list_windows() -> Result<Vec<WindowInfo>> {
    Err(Error::UnsupportedPlatform)
}

#[cfg(windows)]
pub fn list_monitors() -> Result<Vec<MonitorInfo>> {
    windows::list_monitors()
}

#[cfg(not(windows))]
pub fn list_monitors() -> Result<Vec<MonitorInfo>> {
    Err(Error::UnsupportedPlatform)
}

/// Binds a target to one window or monitor from the given lists.
pub fn resolve(
    target: &Target,
    windows: &[WindowInfo],
    monitors: &[MonitorInfo],
) -> Result<ResolvedTarget> {
    match target {
        Target::WindowId(id) => {
            let window = windows
                .iter()
                .find(|w| w.id == *id)
                .ok_or_else(|| Error::TargetNotFound(format!("window id {id}")))?;
            window_target(window)
        }
        Target::WindowTitle(text) => {
            let needle = text.to_lowercase();
            let found = windows
                .iter()
                .filter(|w| w.title.to_lowercase().contains(&needle));
            single(found, text, format!("window {text:?}"))
        }
        Target::Process(name) => {
            let needle = name.to_lowercase();
            let found = windows
                .iter()
                .filter(|w| w.process.to_lowercase() == needle);
            single(found, name, format!("process {name:?}"))
        }
        Target::Region(region) => {
            let monitor = monitors
                .iter()
                .find(|m| contains(&m.bounds, region))
                .ok_or(Error::RegionOutOfBounds)?;
            Ok(ResolvedTarget {
                kind: ResolvedKind::Monitor {
                    index: monitor.index,
                    crop: Some(Rect {
                        x: region.x - monitor.bounds.x,
                        y: region.y - monitor.bounds.y,
                        ..*region
                    }),
                },
                origin: (region.x, region.y),
            })
        }
        Target::Monitor(index) => {
            let monitor = monitors
                .iter()
                .find(|m| m.index == *index)
                .ok_or_else(|| Error::TargetNotFound(format!("monitor {index}")))?;
            Ok(ResolvedTarget {
                kind: ResolvedKind::Monitor {
                    index: *index,
                    crop: None,
                },
                origin: (monitor.bounds.x, monitor.bounds.y),
            })
        }
        Target::Fake(kind) => Ok(ResolvedTarget {
            kind: ResolvedKind::Fake(*kind),
            origin: (0, 0),
        }),
    }
}

fn single<'a>(
    mut found: impl Iterator<Item = &'a WindowInfo>,
    query: &str,
    described: String,
) -> Result<ResolvedTarget> {
    let first = found.next().ok_or(Error::TargetNotFound(described))?;
    let rest: Vec<&WindowInfo> = found.collect();
    if !rest.is_empty() {
        let candidates = std::iter::once(first).chain(rest).cloned().collect();
        return Err(Error::AmbiguousTarget {
            query: query.to_owned(),
            candidates,
        });
    }
    window_target(first)
}

fn window_target(window: &WindowInfo) -> Result<ResolvedTarget> {
    if window.minimized {
        return Err(Error::TargetMinimized(window.title.clone()));
    }
    Ok(ResolvedTarget {
        kind: ResolvedKind::Window(window.id),
        origin: (window.bounds.x, window.bounds.y),
    })
}

fn contains(outer: &Rect, inner: &Rect) -> bool {
    let right = |r: &Rect| i64::from(r.x) + i64::from(r.width);
    let bottom = |r: &Rect| i64::from(r.y) + i64::from(r.height);
    inner.x >= outer.x
        && inner.y >= outer.y
        && right(inner) <= right(outer)
        && bottom(inner) <= bottom(outer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn window(id: u64, title: &str, process: &str, minimized: bool) -> WindowInfo {
        WindowInfo {
            id,
            title: title.into(),
            process: process.into(),
            pid: id as u32,
            bounds: rect(10 * id as i32, 20, 300, 200),
            minimized,
        }
    }

    fn windows() -> Vec<WindowInfo> {
        vec![
            window(1, "Docs - Chrome", "chrome.exe", false),
            window(2, "Issues - Chrome", "chrome.exe", false),
            window(3, "Untitled - Notepad", "Notepad.exe", true),
        ]
    }

    fn monitors() -> Vec<MonitorInfo> {
        vec![
            MonitorInfo {
                index: 0,
                bounds: rect(0, 0, 1920, 1080),
                primary: true,
            },
            MonitorInfo {
                index: 1,
                bounds: rect(1920, 0, 1280, 720),
                primary: false,
            },
        ]
    }

    fn resolve_with(target: Target) -> Result<ResolvedTarget> {
        resolve(&target, &windows(), &monitors())
    }

    #[test]
    fn window_id_hit_and_miss() {
        let hit = resolve_with(Target::WindowId(2)).unwrap();
        assert_eq!(hit.kind, ResolvedKind::Window(2));
        assert_eq!(hit.origin, (20, 20));
        let miss = resolve_with(Target::WindowId(9)).unwrap_err();
        assert_eq!(miss.code(), "target_not_found");
    }

    #[test]
    fn window_title_ignores_case() {
        let hit = resolve_with(Target::WindowTitle("DOCS".into())).unwrap();
        assert_eq!(hit.kind, ResolvedKind::Window(1));
    }

    #[test]
    fn title_matching_two_windows_is_ambiguous_and_lists_both() {
        match resolve_with(Target::WindowTitle("chrome".into())) {
            Err(Error::AmbiguousTarget { candidates, .. }) => {
                let ids: Vec<u64> = candidates.iter().map(|w| w.id).collect();
                assert_eq!(ids, [1, 2]);
            }
            other => panic!("expected AmbiguousTarget, got {other:?}"),
        }
    }

    #[test]
    fn process_matches_ignoring_case() {
        let err = resolve_with(Target::Process("notepad.EXE".into())).unwrap_err();
        // Found, but minimized: proves the case-insensitive match happened.
        assert_eq!(err.code(), "target_minimized");
    }

    #[test]
    fn minimized_window_is_rejected() {
        let err = resolve_with(Target::WindowId(3)).unwrap_err();
        assert_eq!(err.code(), "target_minimized");
    }

    #[test]
    fn region_on_second_monitor_is_cropped_relative_to_it() {
        let hit = resolve_with(Target::Region(rect(2000, 100, 640, 480))).unwrap();
        assert_eq!(
            hit.kind,
            ResolvedKind::Monitor {
                index: 1,
                crop: Some(rect(80, 100, 640, 480)),
            }
        );
        assert_eq!(hit.origin, (2000, 100));
    }

    #[test]
    fn region_crossing_monitors_is_out_of_bounds() {
        let err = resolve_with(Target::Region(rect(1800, 0, 400, 400))).unwrap_err();
        assert_eq!(err.code(), "region_out_of_bounds");
    }

    #[test]
    fn unknown_monitor_is_not_found() {
        let err = resolve_with(Target::Monitor(5)).unwrap_err();
        assert_eq!(err.code(), "target_not_found");
    }
}
