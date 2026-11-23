use serde_json::json;

use crate::model::ErrorInfo;

/// Every failure a user can see. The variant decides the JSON error code.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An error a detached recorder wrote to its state file, reported by `start` as it was.
    #[error("{}", .0.message)]
    Relayed(ErrorInfo),
    #[error("{0}")]
    Usage(String),
    #[error("no target matches {0}")]
    TargetNotFound(String),
    /// `kind` is "window" or "tab"; `candidates` are the matching items as printed.
    #[error("{} {kind}s match {query:?}; pass --{kind}-id", candidates.len())]
    AmbiguousTarget {
        query: String,
        kind: &'static str,
        candidates: Vec<serde_json::Value>,
    },
    #[error("window {0:?} is minimized")]
    TargetMinimized(String),
    #[error("the region must lie inside one monitor")]
    RegionOutOfBounds,
    #[error("ffmpeg was not found on PATH; it is needed for mp4")]
    FfmpegNotFound,
    #[error("the recorder did not start: {0}")]
    RecorderStartFailed(String),
    #[error("no recording {0:?} under the output root")]
    RecordingNotFound(String),
    #[error("recording {0:?} is not running")]
    RecordingNotActive(String),
    #[error("recording {0:?} has not finished; stop it or wait for it first")]
    RecordingNotFinished(String),
    #[error(
        "the recorder for {0:?} exited without a result; its log is in the recording directory"
    )]
    RecorderCrashed(String),
    #[error("{0} frames match; ask for at most 8")]
    TooManyFrames(usize),
    #[error("capture failed: {0}")]
    CaptureFailed(String),
    #[error("encoding failed: {0}")]
    EncodeFailed(String),
    #[error(
        "window, region and monitor capture need Windows or macOS; on this system recdr records browser tabs with --cdp"
    )]
    UnsupportedPlatform,
    #[error("could not connect to the browser: {0}")]
    CdpConnectFailed(String),
    #[error(
        "macOS has not given this program the Screen Recording permission. Open System Settings > Privacy & Security > Screen & System Audio Recording, switch on the program that started recdr (your terminal or editor), restart that program, and run recdr again."
    )]
    PermissionDenied,
    /// `recdr update` on a copy that Homebrew or cargo owns; the message names the command.
    #[error("{0}")]
    ManagedInstall(String),
    #[error("update failed: {0}")]
    UpdateFailed(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Stable identifier printed in the JSON error object. Agents match on this,
    /// so renaming one is a breaking change.
    pub fn code(&self) -> &str {
        match self {
            Error::Relayed(info) => &info.code,
            Error::Usage(_) => "usage",
            Error::TargetNotFound(_) => "target_not_found",
            Error::AmbiguousTarget { .. } => "ambiguous_target",
            Error::TargetMinimized(_) => "target_minimized",
            Error::RegionOutOfBounds => "region_out_of_bounds",
            Error::FfmpegNotFound => "ffmpeg_not_found",
            Error::RecorderStartFailed(_) => "recorder_start_failed",
            Error::RecordingNotFound(_) => "recording_not_found",
            Error::RecordingNotActive(_) => "recording_not_active",
            Error::RecordingNotFinished(_) => "recording_not_finished",
            Error::RecorderCrashed(_) => "recorder_crashed",
            Error::TooManyFrames(_) => "too_many_frames",
            Error::CaptureFailed(_) => "capture_failed",
            Error::EncodeFailed(_) => "encode_failed",
            Error::UnsupportedPlatform => "unsupported_platform",
            Error::CdpConnectFailed(_) => "cdp_connect_failed",
            Error::PermissionDenied => "permission_denied",
            Error::ManagedInstall(_) => "managed_install",
            Error::UpdateFailed(_) => "update_failed",
            Error::Io(_) => "io_error",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) => 2,
            _ => 1,
        }
    }

    /// The error as it is printed: code, message, and candidates for `ambiguous_target`.
    pub fn info(&self) -> ErrorInfo {
        match self {
            Error::Relayed(info) => info.clone(),
            _ => ErrorInfo {
                code: self.code().to_owned(),
                message: self.to_string(),
                candidates: match self {
                    Error::AmbiguousTarget { candidates, .. } => Some(candidates.clone()),
                    _ => None,
                },
            },
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({ "error": self.info() })
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Io(std::io::Error::other(e))
    }
}

impl From<image::ImageError> for Error {
    fn from(e: image::ImageError) -> Self {
        Error::Io(std::io::Error::other(e))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_target_lists_candidates_inside_error() {
        use crate::model::{Rect, WindowInfo};
        let window = |id| {
            json!(WindowInfo {
                id,
                title: "t".into(),
                process: String::new(),
                pid: 0,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                minimized: false,
            })
        };
        let error = Error::AmbiguousTarget {
            query: "t".into(),
            kind: "window",
            candidates: vec![window(1), window(2)],
        };
        assert_eq!(error.to_string(), "2 windows match \"t\"; pass --window-id");
        let json = error.to_json();
        assert_eq!(json["error"]["code"], "ambiguous_target");
        assert_eq!(json["error"]["candidates"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn relayed_error_prints_as_it_was_stored() {
        let info = ErrorInfo {
            code: "ambiguous_target".into(),
            message: "2 windows match \"t\"; pass --window-id".into(),
            candidates: Some(vec![json!({"id": 1}), json!({"id": 2})]),
        };
        let error = Error::Relayed(info.clone());
        assert_eq!(error.to_json(), json!({ "error": info }));
        assert_eq!(error.code(), "ambiguous_target");
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn every_variant_has_the_code_from_the_spec() {
        let all = [
            (
                Error::Relayed(ErrorInfo {
                    code: "capture_failed".into(),
                    message: String::new(),
                    candidates: None,
                }),
                "capture_failed",
            ),
            (Error::Usage(String::new()), "usage"),
            (Error::TargetNotFound(String::new()), "target_not_found"),
            (
                Error::AmbiguousTarget {
                    query: String::new(),
                    kind: "window",
                    candidates: Vec::new(),
                },
                "ambiguous_target",
            ),
            (Error::TargetMinimized(String::new()), "target_minimized"),
            (Error::RegionOutOfBounds, "region_out_of_bounds"),
            (Error::FfmpegNotFound, "ffmpeg_not_found"),
            (
                Error::RecorderStartFailed(String::new()),
                "recorder_start_failed",
            ),
            (
                Error::RecordingNotFound(String::new()),
                "recording_not_found",
            ),
            (
                Error::RecordingNotActive(String::new()),
                "recording_not_active",
            ),
            (
                Error::RecordingNotFinished(String::new()),
                "recording_not_finished",
            ),
            (Error::RecorderCrashed(String::new()), "recorder_crashed"),
            (Error::TooManyFrames(9), "too_many_frames"),
            (Error::CaptureFailed(String::new()), "capture_failed"),
            (Error::EncodeFailed(String::new()), "encode_failed"),
            (Error::UnsupportedPlatform, "unsupported_platform"),
            (Error::CdpConnectFailed(String::new()), "cdp_connect_failed"),
            (Error::PermissionDenied, "permission_denied"),
            (Error::ManagedInstall(String::new()), "managed_install"),
            (Error::UpdateFailed(String::new()), "update_failed"),
            (Error::Io(std::io::Error::other("x")), "io_error"),
        ];
        assert_eq!(all.len(), 21);
        for (error, code) in &all {
            assert_eq!(error.code(), *code);
            let expected_exit = if *code == "usage" { 2 } else { 1 };
            assert_eq!(error.exit_code(), expected_exit, "{code}");
            assert_eq!(error.to_json()["error"]["code"], *code);
        }
    }
}
