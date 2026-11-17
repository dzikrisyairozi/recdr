//! The operations a user can ask for. Both front ends call these and nothing below.

use crate::capture;
use crate::error::Result;
use crate::model::{MonitorList, WindowList};

pub fn list_windows() -> Result<WindowList> {
    Ok(WindowList {
        windows: capture::list_windows()?,
    })
}

pub fn list_monitors() -> Result<MonitorList> {
    Ok(MonitorList {
        monitors: capture::list_monitors()?,
    })
}
