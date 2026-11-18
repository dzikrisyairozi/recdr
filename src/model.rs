//! Plain data types shared by every layer. No logic lives here.

use std::path::PathBuf;

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

/// One captured image, RGBA8, rows packed with no padding.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum CaptureEvent {
    Frame(Frame),
    /// `x` and `y` are frame pixels relative to the captured area; `button` is
    /// "left", "middle" or "right".
    Click {
        unix_ms: u64,
        x: i32,
        y: i32,
        button: &'static str,
    },
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Gif,
    Mp4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Stopped,
    DurationReached,
    CapReached,
    SizeCapReached,
    CommandExited,
    WindowClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Starting,
    Recording,
    Finished,
    Crashed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordOptions {
    pub target: Target,
    pub fps: u32,
    pub max_duration_ms: u64,
    pub max_size_bytes: u64,
    pub formats: Vec<Format>,
    pub cursor: bool,
    pub clicks: bool,
    pub out: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub command: Option<Vec<String>>,
}

/// Contents of `state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub id: String,
    pub status: Status,
    pub pid: u32,
    pub started_at_unix_ms: u64,
    pub width: u32,
    pub height: u32,
    pub resolved: ResolvedTarget,
    pub options: RecordOptions,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<ErrorInfo>,
}

/// An error in the shape it is printed, so a detached recorder can hand it to `start`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub candidates: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameEntry {
    pub index: u32,
    pub t_ms: u64,
    /// Relative to the recording directory.
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub t_ms: u64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Click {
    pub t_ms: u64,
    pub x: i32,
    pub y: i32,
    /// "left", "right" or "middle".
    pub button: String,
}

/// Contents of `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub started_at_unix_ms: u64,
    pub duration_ms: u64,
    pub frames: Vec<FrameEntry>,
    pub marks: Vec<Mark>,
    pub clicks: Vec<Click>,
    pub stop_reason: StopReason,
    pub dropped_frames: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffEntry {
    pub t_ms: u64,
    pub changed_fraction: f32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bbox: Option<Rect>,
    pub keyframe: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameRef {
    pub t_ms: u64,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum FrameSelection {
    At(Vec<u64>),
    Every(u64),
    Keyframes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartInfo {
    pub id: String,
    pub dir: PathBuf,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEntry {
    pub id: String,
    pub dir: PathBuf,
    pub status: Status,
    pub started_at: u64,
    pub elapsed_ms: u64,
}

/// Printed by `stop` and `record`, and saved as `result.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingResult {
    pub id: String,
    pub dir: PathBuf,
    pub stop_reason: StopReason,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub command_exit_code: Option<i32>,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub frames_stored: u32,
    pub dropped_frames: u64,
    pub keyframes: Vec<FrameRef>,
    pub marks: Vec<Mark>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gif: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mp4: Option<PathBuf>,
    pub contact_sheet: PathBuf,
    pub diff: PathBuf,
    pub manifest: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusList {
    pub recordings: Vec<StatusEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameList {
    pub frames: Vec<FrameRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkAck {
    pub id: String,
    pub label: String,
}
