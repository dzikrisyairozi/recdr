//! Plain data types shared by every layer. No logic lives here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// The Win32 window handle as a number.
    pub id: u64,
    pub title: String,
    /// Executable name; empty when the process cannot be opened (elevated processes).
    pub process: String,
    pub pid: u32,
    pub bounds: Rect,
    pub minimized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Zero-based position in the system's monitor enumeration.
    pub index: usize,
    pub bounds: Rect,
    pub primary: bool,
}

/// Synthetic frame sources selected with `RECDR_FAKE_SOURCE` in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FakeKind {
    Static,
    Moving,
    Closing,
    Broken,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    WindowId(u64),
    WindowTitle(String),
    Process(String),
    Region(Rect),
    Monitor(usize),
    Fake(FakeKind),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedKind {
    Window(u64),
    /// `crop` is relative to the monitor's top-left corner.
    Monitor {
        index: usize,
        crop: Option<Rect>,
    },
    Fake(FakeKind),
}

/// A target bound to one window or monitor. The recording's size is the size of
/// its first frame, so no size is stored here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedTarget {
    pub kind: ResolvedKind,
    /// Top-left corner of the captured area in virtual-screen coordinates.
    pub origin: (i32, i32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowList {
    pub windows: Vec<WindowInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorList {
    pub monitors: Vec<MonitorInfo>,
}
