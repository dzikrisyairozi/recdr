//! Listing windows and monitors, resolving a target, producing frames.

mod cdp;
mod fake;
#[cfg(windows)]
mod hook;
#[cfg(windows)]
mod windows;

use std::time::Duration;

use serde::Serialize;

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
    /// True when the source hands out frames no faster than the requested rate, so the
    /// recorder stores every frame it gets.
    fn paced(&self) -> bool {
        false
    }
}

/// A mouse press as the system reports it: wall-clock milliseconds, x and y in the
/// system's screen units (pixels on Windows, points on macOS), button.
pub type Press = (u64, f64, f64, &'static str);

/// A press to a click event relative to the captured area in pixels, or None when it
/// is outside. `origin` is the area's top-left corner in the press's units, `size` is
/// the area in pixels, and `scale` is pixels per unit.
pub fn to_event(
    press: Press,
    origin: (f64, f64),
    size: (u32, u32),
    scale: f64,
) -> Option<CaptureEvent> {
    let (unix_ms, x, y, button) = press;
    let x = ((x - origin.0) * scale).floor();
    let y = ((y - origin.1) * scale).floor();
    let inside = (0.0..f64::from(size.0)).contains(&x) && (0.0..f64::from(size.1)).contains(&y);
    inside.then_some(CaptureEvent::Click {
        unix_ms,
        x: x as i32,
        y: y as i32,
        button,
    })
}

/// A display as macOS reports it: position and size in points, and pixels per point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

/// The `bounds` of each display in pixels, in the same order. Positions are scaled by
/// the largest scale so that displays of different scales do not overlap.
pub fn display_bounds(displays: &[Display]) -> Vec<Rect> {
    let s = displays.iter().map(|d| d.scale).fold(0.0, f64::max);
    displays
        .iter()
        .map(|d| Rect {
            x: (d.x * s).round() as i32,
            y: (d.y * s).round() as i32,
            width: (d.width * d.scale).round() as u32,
            height: (d.height * d.scale).round() as u32,
        })
        .collect()
}

/// A rectangle in points to pixels, on the display that holds its center (the first
/// display when none does).
pub fn to_pixels(displays: &[Display], x: f64, y: f64, width: f64, height: f64) -> Rect {
    let (cx, cy) = (x + width / 2.0, y + height / 2.0);
    let i = displays
        .iter()
        .position(|d| d.x <= cx && cx < d.x + d.width && d.y <= cy && cy < d.y + d.height)
        .unwrap_or(0);
    let (Some(&d), Some(&b)) = (displays.get(i), display_bounds(displays).get(i)) else {
        return Rect {
            x: x.round() as i32,
            y: y.round() as i32,
            width: width.round() as u32,
            height: height.round() as u32,
        };
    };
    Rect {
        x: b.x + ((x - d.x) * d.scale).round() as i32,
        y: b.y + ((y - d.y) * d.scale).round() as i32,
        width: (width * d.scale).round() as u32,
        height: (height * d.scale).round() as u32,
    }
}

/// A rectangle in pixels, relative to a display's corner, to points relative to the same corner.
pub fn to_points(crop: &Rect, scale: f64) -> (f64, f64, f64, f64) {
    (
        f64::from(crop.x) / scale,
        f64::from(crop.y) / scale,
        f64::from(crop.width) / scale,
        f64::from(crop.height) / scale,
    )
}

/// `clicks` asks the source to report presses; the synthetic source ignores it.
pub fn open(
    resolved: &ResolvedTarget,
    fps: u32,
    cursor: bool,
    clicks: bool,
) -> Result<Box<dyn CaptureSource>> {
    match &resolved.kind {
        ResolvedKind::Fake(FakeKind::Broken) => Err(Error::CaptureFailed(
            "the synthetic source is broken".into(),
        )),
        ResolvedKind::Fake(kind) => Ok(Box::new(fake::FakeSource::new(*kind))),
        ResolvedKind::Tab(tab) => Ok(Box::new(cdp::CdpSource::open(tab, fps, clicks)?)),
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
            window_target(single(found, text, "window", format!("window {text:?}"))?)
        }
        Target::Process(name) => {
            let needle = name.to_lowercase();
            let found = windows
                .iter()
                .filter(|w| w.process.to_lowercase() == needle);
            window_target(single(found, name, "window", format!("process {name:?}"))?)
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
        Target::Tab(tab) => Ok(ResolvedTarget {
            kind: ResolvedKind::Tab(tab.clone()),
            origin: (0, 0),
        }),
        Target::Fake(kind) => Ok(ResolvedTarget {
            kind: ResolvedKind::Fake(*kind),
            origin: (0, 0),
        }),
    }
}

/// The one item of `found`, or `target_not_found` for none and `ambiguous_target` for several.
fn single<'a, T: Serialize>(
    mut found: impl Iterator<Item = &'a T>,
    query: &str,
    kind: &'static str,
    described: String,
) -> Result<&'a T> {
    let first = found.next().ok_or(Error::TargetNotFound(described))?;
    let rest: Vec<&T> = found.collect();
    if !rest.is_empty() {
        let candidates = std::iter::once(first)
            .chain(rest)
            .map(|item| serde_json::json!(item))
            .collect();
        return Err(Error::AmbiguousTarget {
            query: query.to_owned(),
            kind,
            candidates,
        });
    }
    Ok(first)
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
    fn tab_target_travels_unresolved() {
        let tab = crate::model::TabTarget {
            cdp: Endpoint::Port(9222),
            query: crate::model::TabQuery::Text("docs".into()),
        };
        let resolved = resolve_with(Target::Tab(tab.clone())).unwrap();
        assert_eq!(resolved.kind, ResolvedKind::Tab(tab));
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
            Err(error @ Error::AmbiguousTarget { .. }) => {
                assert_eq!(
                    error.to_string(),
                    "2 windows match \"chrome\"; pass --window-id"
                );
                let Error::AmbiguousTarget { candidates, .. } = error else {
                    unreachable!()
                };
                let ids: Vec<Option<u64>> = candidates.iter().map(|c| c["id"].as_u64()).collect();
                assert_eq!(ids, [Some(1), Some(2)]);
            }
            other => panic!("expected AmbiguousTarget, got {other:?}"),
        }
    }

    fn tab(id: &str, title: &str) -> TabInfo {
        TabInfo {
            id: id.into(),
            title: title.into(),
            url: format!("http://example.test/{id}"),
        }
    }

    #[test]
    fn single_serves_tabs_too() {
        let tabs = [tab("A1", "Docs"), tab("B2", "Docs too")];
        let pick = |query: &str| {
            let needle = query.to_lowercase();
            let found = tabs
                .iter()
                .filter(|t| t.title.to_lowercase().contains(&needle));
            single(found, query, "tab", format!("tab {query:?}"))
        };

        let error = pick("docs").unwrap_err();
        assert_eq!(error.code(), "ambiguous_target");
        assert_eq!(error.to_string(), "2 tabs match \"docs\"; pass --tab-id");
        let candidates = error.info().candidates.unwrap();
        assert_eq!(candidates.len(), 2);
        for candidate in &candidates {
            let mut keys: Vec<&String> = candidate.as_object().unwrap().keys().collect();
            keys.sort();
            assert_eq!(keys, ["id", "title", "url"]);
        }

        assert_eq!(pick("too").unwrap().id, "B2");

        let missing = pick("mail").unwrap_err();
        assert_eq!(missing.code(), "target_not_found");
        assert_eq!(missing.to_string(), "no target matches tab \"mail\"");
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

    fn click_at(event: Option<CaptureEvent>) -> Option<(i32, i32)> {
        match event {
            Some(CaptureEvent::Click {
                unix_ms,
                x,
                y,
                button,
            }) => {
                assert_eq!((unix_ms, button), (7, "left"));
                Some((x, y))
            }
            None => None,
            Some(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn to_event_keeps_presses_inside_the_area_at_scale_1() {
        let at = |x: f64, y: f64| to_event((7, x, y, "left"), (100.0, 200.0), (50, 50), 1.0);
        assert_eq!(click_at(at(110.0, 220.0)), Some((10, 20)));
        assert_eq!(click_at(at(100.0, 200.0)), Some((0, 0)));
        assert_eq!(click_at(at(149.0, 249.0)), Some((49, 49)));
        for (x, y) in [(99.0, 220.0), (150.0, 220.0), (110.0, 250.0)] {
            assert_eq!(click_at(at(x, y)), None, "({x}, {y})");
        }
    }

    #[test]
    fn to_event_scales_points_to_pixels() {
        let at = |x: f64, y: f64| to_event((7, x, y, "left"), (100.0, 200.0), (100, 100), 2.0);
        assert_eq!(click_at(at(110.5, 220.0)), Some((21, 40)));
        assert_eq!(click_at(at(150.0, 220.0)), None);
    }

    fn display(x: f64, y: f64, width: f64, height: f64, scale: f64) -> Display {
        Display {
            x,
            y,
            width,
            height,
            scale,
        }
    }

    /// A 2x laptop display with a 1x display to its right.
    fn retina_then_plain() -> Vec<Display> {
        vec![
            display(0.0, 0.0, 1512.0, 982.0, 2.0),
            display(1512.0, 0.0, 1920.0, 1080.0, 1.0),
        ]
    }

    /// A 1x display with a 2x display to its right.
    fn plain_then_retina() -> Vec<Display> {
        vec![
            display(0.0, 0.0, 1920.0, 1080.0, 1.0),
            display(1920.0, 0.0, 1512.0, 982.0, 2.0),
        ]
    }

    #[test]
    fn one_retina_display_is_in_pixels() {
        let displays = [display(0.0, 0.0, 1512.0, 982.0, 2.0)];
        assert_eq!(display_bounds(&displays), [rect(0, 0, 3024, 1964)]);
    }

    #[test]
    fn display_bounds_do_not_overlap_either_way_round() {
        assert_eq!(
            display_bounds(&retina_then_plain()),
            [rect(0, 0, 3024, 1964), rect(3024, 0, 1920, 1080)]
        );
        assert_eq!(
            display_bounds(&plain_then_retina()),
            [rect(0, 0, 1920, 1080), rect(3840, 0, 3024, 1964)]
        );
    }

    #[test]
    fn a_rectangle_inside_each_display_goes_to_pixels_and_back() {
        for displays in [retina_then_plain(), plain_then_retina()] {
            for (d, b) in displays.iter().zip(display_bounds(&displays)) {
                let r = to_pixels(&displays, d.x + 10.0, d.y + 10.0, 100.0, 50.0);
                assert!(contains(&b, &r), "{r:?} is not inside {b:?}");
                let crop = Rect {
                    x: r.x - b.x,
                    y: r.y - b.y,
                    ..r
                };
                assert_eq!(to_points(&crop, d.scale), (10.0, 10.0, 100.0, 50.0));
            }
        }
    }

    #[test]
    fn a_rectangle_on_no_display_is_placed_on_the_first() {
        let r = to_pixels(&retina_then_plain(), -500.0, -500.0, 100.0, 50.0);
        assert_eq!(r, rect(-1000, -1000, 200, 100));
    }

    #[test]
    fn a_center_on_a_shared_edge_belongs_to_the_display_that_starts_there() {
        // Center x is 1512, where the 1x display starts.
        let r = to_pixels(&retina_then_plain(), 1462.0, 10.0, 100.0, 50.0);
        assert_eq!(r, rect(2974, 10, 100, 50));
    }

    #[test]
    fn with_no_displays_points_are_rounded_as_they_are() {
        assert_eq!(to_pixels(&[], 1.4, 2.6, 10.5, 3.2), rect(1, 3, 11, 3));
    }

    #[test]
    fn unknown_monitor_is_not_found() {
        let err = resolve_with(Target::Monitor(5)).unwrap_err();
        assert_eq!(err.code(), "target_not_found");
    }
}
