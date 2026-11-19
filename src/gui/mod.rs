//! The desktop app: a frame over what to record, a toolbar, and an editor for trimming
//! and cropping. `geometry`, `state` and `worker` hold every rule and name no window type;
//! `app`, `overlay` and `editor` only draw `state::App` and turn input into events.

use std::path::PathBuf;

use crate::error::Result;

pub mod geometry;
pub mod state;
pub mod worker;

#[cfg(any(windows, target_os = "macos"))]
mod app;
#[cfg(any(windows, target_os = "macos"))]
mod editor;
#[cfg(any(windows, target_os = "macos"))]
mod overlay;

/// Opens the app and returns when it is closed.
#[cfg(any(windows, target_os = "macos"))]
pub fn run(out: PathBuf) -> Result<()> {
    app::run(out)
}

/// Opens the app and returns when it is closed. There is no app on this system.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn run(_out: PathBuf) -> Result<()> {
    Err(crate::error::Error::UnsupportedPlatform)
}
