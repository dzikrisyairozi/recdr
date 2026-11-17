//! Synthetic frame source, so the whole recording path runs in tests without a desktop.

use std::thread;
use std::time::{Duration, Instant};

use super::CaptureSource;
use crate::error::Result;
use crate::model::{CaptureEvent, FakeKind, Frame};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const INTERVAL: Duration = Duration::from_millis(33);
const SQUARE: u32 = 40;

pub struct FakeSource {
    kind: FakeKind,
    produced: u32,
    next_due: Instant,
}

impl FakeSource {
    pub fn new(kind: FakeKind) -> Self {
        Self {
            kind,
            produced: 0,
            next_due: Instant::now(),
        }
    }

    fn frame(&self) -> Frame {
        let mut rgba = [0, 0, 0, 255].repeat((WIDTH * HEIGHT) as usize);
        if self.kind != FakeKind::Static {
            let left = (self.produced * 8) % (WIDTH - SQUARE);
            for y in 100..100 + SQUARE {
                for x in left..left + SQUARE {
                    let at = ((y * WIDTH + x) * 4) as usize;
                    rgba[at..at + 3].copy_from_slice(&[255, 255, 255]);
                }
            }
        }
        Frame {
            width: WIDTH,
            height: HEIGHT,
            rgba,
        }
    }
}

impl CaptureSource for FakeSource {
    fn next_event(&mut self, timeout: Duration) -> Result<Option<CaptureEvent>> {
        if self.kind == FakeKind::Closing && self.produced >= 10 {
            return Ok(Some(CaptureEvent::Closed));
        }
        let wait = self.next_due.saturating_duration_since(Instant::now());
        if wait > timeout {
            thread::sleep(timeout);
            return Ok(None);
        }
        thread::sleep(wait);
        let frame = self.frame();
        self.produced += 1;
        self.next_due += INTERVAL;
        Ok(Some(CaptureEvent::Frame(frame)))
    }

    fn dropped(&self) -> u64 {
        0
    }
}
