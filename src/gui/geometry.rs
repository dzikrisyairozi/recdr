//! Arithmetic on the frame, the toolbar, crops and the timeline. Every length is in
//! pixels unless its name says points. A `Rect` named `inside` is the recorded area:
//! the frame without its border.

use crate::model::{MonitorInfo, Rect};

/// Drawn width of the outline.
pub const BORDER: u32 = 2;
/// Thickness of a border window, so the mouse can catch it.
pub const GRIP: u32 = 8;
/// Length of the corner zone at each end of an edge.
pub const CORNER: u32 = 16;
pub const MIN_INSIDE: u32 = 32;
pub const TOOLBAR_POINTS: (f64, f64) = (560.0, 40.0);
pub const START_INSIDE: (u32, u32) = (640, 480);
pub const MIN_KEPT_MS: u64 = 100;

pub fn to_pixels(points: f64, scale: f64) -> i32 {
    (points * scale).round() as i32
}

pub fn to_points(pixels: f64, scale: f64) -> f64 {
    pixels / scale
}

pub fn toolbar_size(scale: f64) -> (u32, u32) {
    (
        to_pixels(TOOLBAR_POINTS.0, scale) as u32,
        to_pixels(TOOLBAR_POINTS.1, scale) as u32,
    )
}

/// The monitor that holds the rectangle's center, or the first monitor.
pub fn monitor_of<'a>(rect: &Rect, monitors: &'a [MonitorInfo]) -> Option<&'a MonitorInfo> {
    let cx = i64::from(rect.x) + i64::from(rect.width) / 2;
    let cy = i64::from(rect.y) + i64::from(rect.height) / 2;
    monitors
        .iter()
        .find(|m| {
            let b = &m.bounds;
            (i64::from(b.x)..i64::from(b.x) + i64::from(b.width)).contains(&cx)
                && (i64::from(b.y)..i64::from(b.y) + i64::from(b.height)).contains(&cy)
        })
        .or(monitors.first())
}

/// The frame a new app starts with: START_INSIDE centered on the primary monitor.
pub fn initial(monitors: &[MonitorInfo]) -> Rect {
    let (width, height) = START_INSIDE;
    let Some(m) = monitors.iter().find(|m| m.primary).or(monitors.first()) else {
        return Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
    };
    let b = &m.bounds;
    Rect {
        x: b.x + (b.width as i32 - width as i32) / 2,
        y: b.y + (b.height as i32 - height as i32) / 2,
        width,
        height,
    }
}

/// `value` held between `low` and `high`; `low` wins when the two cross, so a monitor
/// too small for the limits does not panic.
fn hold(value: i64, low: i64, high: i64) -> i64 {
    value.min(high).max(low)
}

/// `inside` made to fit: at least MIN_INSIDE each way, at most the monitor less the
/// grips (and less the toolbar's height), and moved wholly onto the monitor of its center.
pub fn keep_inside(inside: Rect, monitors: &[MonitorInfo], toolbar: (u32, u32)) -> Rect {
    let Some(monitor) = monitor_of(&inside, monitors) else {
        return inside;
    };
    let m = &monitor.bounds;
    let (mx, my, mw, mh) = (
        i64::from(m.x),
        i64::from(m.y),
        i64::from(m.width),
        i64::from(m.height),
    );
    let (grip, bar, min) = (i64::from(GRIP), i64::from(toolbar.1), i64::from(MIN_INSIDE));
    let width = hold(i64::from(inside.width), min, mw - 2 * grip);
    let height = hold(i64::from(inside.height), min, mh - 2 * grip - bar);
    let x = hold(i64::from(inside.x), mx + grip, mx + mw - grip - width);
    let mut y = hold(i64::from(inside.y), my + grip, my + mh - grip - height);
    let fitted = |y: i64| Rect {
        x: x as i32,
        y: y as i32,
        width: width as u32,
        height: height as u32,
    };
    if toolbar_place(&fitted(y), m, toolbar) == Place::Inside {
        y = my + mh - grip - bar - height;
    }
    fitted(y)
}

/// Where the toolbar goes: under the frame, above it, or inside its bottom edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Under,
    Above,
    Inside,
}

pub fn toolbar_place(inside: &Rect, monitor: &Rect, toolbar: (u32, u32)) -> Place {
    let (grip, bar) = (i64::from(GRIP), i64::from(toolbar.1));
    let bottom = i64::from(inside.y) + i64::from(inside.height);
    if bottom + grip + bar <= i64::from(monitor.y) + i64::from(monitor.height) {
        Place::Under
    } else if i64::from(inside.y) - grip - bar >= i64::from(monitor.y) {
        Place::Above
    } else {
        Place::Inside
    }
}

pub fn toolbar_rect(inside: &Rect, monitor: &Rect, toolbar: (u32, u32)) -> Rect {
    let (grip, bar) = (GRIP as i32, toolbar.1 as i32);
    let y = match toolbar_place(inside, monitor, toolbar) {
        Place::Under => inside.y + inside.height as i32 + grip,
        Place::Above => inside.y - grip - bar,
        Place::Inside => inside.y + inside.height as i32 - bar,
    };
    let right = i64::from(monitor.x) + i64::from(monitor.width) - i64::from(toolbar.0);
    Rect {
        x: hold(i64::from(inside.x), i64::from(monitor.x), right) as i32,
        y,
        width: toolbar.0,
        height: toolbar.1,
    }
}

/// The four border windows: top, bottom, left, right.
pub fn borders(inside: &Rect) -> [Rect; 4] {
    let grip = GRIP as i32;
    let across = inside.width + 2 * GRIP;
    [
        Rect {
            x: inside.x - grip,
            y: inside.y - grip,
            width: across,
            height: GRIP,
        },
        Rect {
            x: inside.x - grip,
            y: inside.y + inside.height as i32,
            width: across,
            height: GRIP,
        },
        Rect {
            x: inside.x - grip,
            y: inside.y,
            width: GRIP,
            height: inside.height,
        },
        Rect {
            x: inside.x + inside.width as i32,
            y: inside.y,
            width: GRIP,
            height: inside.height,
        },
    ]
}

/// An edge or a corner of the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Which edge or corner a press at `along` pixels from the start of border window `side`
/// takes. `side` is the index in `borders`.
pub fn edge_at(side: usize, along: u32, length: u32) -> Edge {
    match side {
        0 | 1 => {
            let end = CORNER + GRIP;
            let top = side == 0;
            if along < end {
                if top { Edge::TopLeft } else { Edge::BottomLeft }
            } else if along >= length.saturating_sub(end) {
                if top {
                    Edge::TopRight
                } else {
                    Edge::BottomRight
                }
            } else if top {
                Edge::Top
            } else {
                Edge::Bottom
            }
        }
        _ => {
            let left = side == 2;
            if along < CORNER {
                if left { Edge::TopLeft } else { Edge::TopRight }
            } else if along >= length.saturating_sub(CORNER) {
                if left {
                    Edge::BottomLeft
                } else {
                    Edge::BottomRight
                }
            } else if left {
                Edge::Left
            } else {
                Edge::Right
            }
        }
    }
}

/// `inside` after the edge was dragged by (dx, dy) pixels. A size under MIN_INSIDE is
/// set to MIN_INSIDE with the opposite edge kept where it was.
pub fn resized(inside: &Rect, edge: Edge, dx: i32, dy: i32) -> Rect {
    use Edge::*;
    let min = i64::from(MIN_INSIDE);
    let (mut left, mut top) = (i64::from(inside.x), i64::from(inside.y));
    let (mut right, mut bottom) = (
        left + i64::from(inside.width),
        top + i64::from(inside.height),
    );
    let (dx, dy) = (i64::from(dx), i64::from(dy));
    if matches!(edge, Left | TopLeft | BottomLeft) {
        left = (left + dx).min(right - min);
    }
    if matches!(edge, Right | TopRight | BottomRight) {
        right = (right + dx).max(left + min);
    }
    if matches!(edge, Top | TopLeft | TopRight) {
        top = (top + dy).min(bottom - min);
    }
    if matches!(edge, Bottom | BottomLeft | BottomRight) {
        bottom = (bottom + dy).max(top + min);
    }
    Rect {
        x: left as i32,
        y: top as i32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    }
}

/// The crop a drag on the preview means, in picture pixels, or None when it is under 2 by 2.
pub fn crop_of(
    a: (f32, f32),
    b: (f32, f32),
    shown: (f32, f32),
    picture: (u32, u32),
) -> Option<Rect> {
    if shown.0 <= 0.0 || shown.1 <= 0.0 {
        return None;
    }
    let to_picture = |v: f32, shown: f32, side: u32| {
        let pixels = (v.clamp(0.0, shown) * side as f32 / shown).round();
        (pixels as u32).min(side)
    };
    let (x0, x1) = (
        to_picture(a.0, shown.0, picture.0),
        to_picture(b.0, shown.0, picture.0),
    );
    let (y0, y1) = (
        to_picture(a.1, shown.1, picture.1),
        to_picture(b.1, shown.1, picture.1),
    );
    let (left, right) = (x0.min(x1), x0.max(x1));
    let (top, bottom) = (y0.min(y1), y0.max(y1));
    (right - left >= 2 && bottom - top >= 2).then_some(Rect {
        x: left as i32,
        y: top as i32,
        width: right - left,
        height: bottom - top,
    })
}

/// The time a position on the timeline means.
pub fn time_at(x: f32, width: f32, duration_ms: u64) -> u64 {
    if width <= 0.0 {
        return 0;
    }
    (f64::from((x / width).clamp(0.0, 1.0)) * duration_ms as f64).round() as u64
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

    fn monitor(index: usize, bounds: Rect) -> MonitorInfo {
        MonitorInfo {
            index,
            bounds,
            primary: index == 0,
        }
    }

    fn one() -> Vec<MonitorInfo> {
        vec![monitor(0, rect(0, 0, 1920, 1080))]
    }

    const BAR: (u32, u32) = (560, 40);

    #[test]
    fn points_and_pixels() {
        for (scale, pixels) in [(1.0, 100), (1.5, 150), (2.0, 200)] {
            assert_eq!(to_pixels(100.0, scale), pixels);
            assert_eq!(to_points(f64::from(pixels), scale), 100.0);
        }
        assert_eq!(toolbar_size(1.0), BAR);
        assert_eq!(toolbar_size(1.5), (840, 60));
    }

    #[test]
    fn the_first_frame_is_centered_on_the_primary_monitor() {
        assert_eq!(initial(&one()), rect(640, 300, 640, 480));
        assert_eq!(initial(&[]), rect(0, 0, 640, 480));
    }

    #[test]
    fn keep_inside_limits_the_size() {
        let fitted = keep_inside(rect(100, 100, 10, 10), &one(), BAR);
        assert_eq!((fitted.width, fitted.height), (32, 32));
        assert_eq!(
            keep_inside(rect(0, 0, 5000, 5000), &one(), BAR),
            rect(8, 8, 1904, 1024)
        );
    }

    #[test]
    fn keep_inside_puts_the_frame_on_the_monitor_of_its_center() {
        let two = vec![
            monitor(0, rect(0, 0, 1920, 1080)),
            monitor(1, rect(1920, 0, 1920, 1080)),
        ];
        assert_eq!(
            keep_inside(rect(1500, 500, 640, 480), &two, BAR),
            rect(1272, 500, 640, 480)
        );
        assert_eq!(
            keep_inside(rect(1700, 500, 640, 480), &two, BAR),
            rect(1928, 500, 640, 480)
        );
    }

    #[test]
    fn keep_inside_leaves_room_for_the_toolbar() {
        let fitted = keep_inside(rect(100, 30, 640, 1024), &one(), BAR);
        assert_eq!(fitted, rect(100, 8, 640, 1024));
        let screen = rect(0, 0, 1920, 1080);
        assert_eq!(toolbar_place(&fitted, &screen, BAR), Place::Under);

        // A seeded generator of rectangles all over and around the monitor.
        let mut seed: u64 = 7;
        let mut next = |limit: u64| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % limit
        };
        for _ in 0..200 {
            let r = rect(
                next(4000) as i32 - 1000,
                next(3000) as i32 - 1000,
                next(3000) as u32,
                next(2000) as u32,
            );
            let fitted = keep_inside(r, &one(), BAR);
            assert_ne!(toolbar_place(&fitted, &screen, BAR), Place::Inside, "{r:?}");
        }
    }

    #[test]
    fn toolbar_goes_under_above_or_inside() {
        let screen = rect(0, 0, 1920, 1080);
        assert_eq!(
            toolbar_place(&rect(100, 100, 640, 480), &screen, BAR),
            Place::Under
        );
        assert_eq!(
            toolbar_place(&rect(100, 500, 640, 560), &screen, BAR),
            Place::Above
        );
        assert_eq!(
            toolbar_place(&rect(0, 0, 1920, 1080), &screen, BAR),
            Place::Inside
        );
        assert_eq!(
            toolbar_rect(&rect(0, 0, 1920, 1080), &screen, BAR),
            rect(0, 1040, 560, 40)
        );
        assert_eq!(
            toolbar_rect(&rect(1800, 100, 100, 100), &screen, BAR).x,
            1360
        );
    }

    #[test]
    fn the_borders_surround_the_inside_without_overlap() {
        let inside = rect(100, 100, 200, 100);
        let mut all = borders(&inside).to_vec();
        all.push(inside);
        let area: u64 = all
            .iter()
            .map(|r| u64::from(r.width) * u64::from(r.height))
            .sum();
        assert_eq!(area, 216 * 116);
        let covers = |x: i32, y: i32| {
            all.iter()
                .filter(|r| {
                    (r.x..r.x + r.width as i32).contains(&x)
                        && (r.y..r.y + r.height as i32).contains(&y)
                })
                .count()
        };
        for y in 92..92 + 116 {
            for x in 92..92 + 216 {
                assert_eq!(covers(x, y), 1, "({x}, {y})");
            }
        }
    }

    #[test]
    fn edges_and_corners() {
        assert_eq!(edge_at(0, 5, 216), Edge::TopLeft);
        assert_eq!(edge_at(0, 100, 216), Edge::Top);
        assert_eq!(edge_at(0, 200, 216), Edge::TopRight);
        assert_eq!(edge_at(1, 100, 216), Edge::Bottom);
        assert_eq!(edge_at(2, 50, 100), Edge::Left);
        assert_eq!(edge_at(3, 95, 100), Edge::BottomRight);
    }

    #[test]
    fn resizing_keeps_the_opposite_edge() {
        let inside = rect(100, 100, 200, 100);
        assert_eq!(
            resized(&inside, Edge::Left, 50, 0),
            rect(150, 100, 150, 100)
        );
        assert_eq!(
            resized(&inside, Edge::Left, 190, 0),
            rect(268, 100, 32, 100)
        );
        assert_eq!(
            resized(&inside, Edge::BottomRight, 10, -90),
            rect(100, 100, 210, 32)
        );
        assert_eq!(resized(&inside, Edge::Top, 99, 7), rect(100, 107, 200, 93));
    }

    #[test]
    fn a_drag_on_the_preview_is_a_crop_in_picture_pixels() {
        let crop = crop_of((10.0, 10.0), (110.0, 60.0), (320.0, 240.0), (640, 480));
        assert_eq!(crop, Some(rect(20, 20, 200, 100)));
        assert_eq!(
            crop_of((110.0, 60.0), (10.0, 10.0), (320.0, 240.0), (640, 480)),
            crop
        );
        assert_eq!(
            crop_of((10.0, 10.0), (10.5, 10.5), (320.0, 240.0), (640, 480)),
            None
        );
        assert_eq!(
            crop_of((-5.0, -5.0), (400.0, 300.0), (320.0, 240.0), (640, 480)),
            Some(rect(0, 0, 640, 480))
        );
    }

    #[test]
    fn a_place_on_the_timeline_is_a_time() {
        assert_eq!(time_at(50.0, 200.0, 3000), 750);
        assert_eq!(time_at(-3.0, 200.0, 3000), 0);
        assert_eq!(time_at(250.0, 200.0, 3000), 3000);
        assert_eq!(time_at(1.0, 0.0, 3000), 0);
    }
}
