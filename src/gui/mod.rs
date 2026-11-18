//! The desktop app: a frame over what to record, a toolbar, and an editor for trimming
//! and cropping. `geometry`, `state` and `worker` hold every rule and name no window type.

use std::path::PathBuf;

use crate::error::{Error, Result};

pub mod geometry;
pub mod state;
pub mod worker;

/// Opens the app and returns when it is closed. Until the windows part exists, refused
/// on every system.
pub fn run(_out: PathBuf) -> Result<()> {
    Err(Error::UnsupportedPlatform)
}
