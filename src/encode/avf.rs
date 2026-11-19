//! The MP4 writer on macOS: H.264 through `AVAssetWriter`, which encodes with
//! VideoToolbox and writes the container.

use std::path::Path;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use image::RgbaImage;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVAssetWriterStatus,
    AVFileTypeMPEG4, AVMediaTypeVideo, AVVideoAverageBitRateKey, AVVideoCodecKey,
    AVVideoCodecTypeH264, AVVideoCompressionPropertiesKey, AVVideoHeightKey, AVVideoWidthKey,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::CMTime;
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferCreate, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    kCVPixelFormatType_32BGRA, kCVReturnSuccess,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use crate::error::{Error, Result};

pub struct Writer {
    writer: Retained<AVAssetWriter>,
    input: Retained<AVAssetWriterInput>,
    adaptor: Retained<AVAssetWriterInputPixelBufferAdaptor>,
    width: usize,
    height: usize,
    fps: i32,
    /// Frames appended so far; the session ends at this many frame times.
    count: i64,
}

fn failed(message: impl Into<String>) -> Error {
    Error::EncodeFailed(message.into())
}

/// `what`, followed by the writer's own error text when it has one.
fn writer_error(writer: &AVAssetWriter, what: &str) -> Error {
    // SAFETY: `error` is a property read on a live writer.
    match unsafe { writer.error() } {
        Some(error) => failed(format!("{what}: {}", error.localizedDescription())),
        None => failed(what),
    }
}

/// A constant AVFoundation exports, or an error naming it when this system lacks it.
fn constant<T: ?Sized>(value: Option<&'static T>, name: &str) -> Result<&'static T> {
    value.ok_or_else(|| failed(format!("AVFoundation has no {name}")))
}

impl Writer {
    /// Starts a video of `width` by `height` (both even) at `fps` frames per second.
    pub fn create(path: &Path, width: u32, height: u32, fps: u32, bit_rate: u32) -> Result<Self> {
        // AVAssetWriter refuses to replace a file. A directory is left for it to refuse.
        if path.is_file() {
            std::fs::remove_file(path).map_err(|e| failed(e.to_string()))?;
        }
        let url = NSURL::from_file_path(path)
            .ok_or_else(|| failed(format!("{} is not a file path", path.display())))?;
        // SAFETY: each is an immutable constant AVFoundation exports; reading it is a load.
        let (file_type, media, codec_key, h264, width_key, height_key, compression_key, rate_key) = unsafe {
            (
                constant(AVFileTypeMPEG4, "AVFileTypeMPEG4")?,
                constant(AVMediaTypeVideo, "AVMediaTypeVideo")?,
                constant(AVVideoCodecKey, "AVVideoCodecKey")?,
                constant(AVVideoCodecTypeH264, "AVVideoCodecTypeH264")?,
                constant(AVVideoWidthKey, "AVVideoWidthKey")?,
                constant(AVVideoHeightKey, "AVVideoHeightKey")?,
                constant(
                    AVVideoCompressionPropertiesKey,
                    "AVVideoCompressionPropertiesKey",
                )?,
                constant(AVVideoAverageBitRateKey, "AVVideoAverageBitRateKey")?,
            )
        };
        // SAFETY: a file URL and a file type constant, as the method expects.
        let writer = unsafe { AVAssetWriter::assetWriterWithURL_fileType_error(&url, file_type) }
            .map_err(|e| failed(e.localizedDescription().to_string()))?;

        let rate = NSNumber::new_u32(bit_rate);
        let rate: &AnyObject = &rate;
        let compression = NSDictionary::<NSString, AnyObject>::from_slices(&[rate_key], &[rate]);
        let (w, h) = (NSNumber::new_u32(width), NSNumber::new_u32(height));
        let values: [&AnyObject; 4] = [h264, &w, &h, &compression];
        let settings = NSDictionary::<NSString, AnyObject>::from_slices(
            &[codec_key, width_key, height_key, compression_key],
            &values,
        );
        // SAFETY: the dictionary holds the keys and value types AVVideoSettings.h names.
        let input = unsafe {
            AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(media, Some(&settings))
        };
        // SAFETY: a property set before writing starts.
        unsafe { input.setExpectsMediaDataInRealTime(false) };
        // SAFETY: no attributes, because each frame's buffer comes from CVPixelBufferCreate.
        let adaptor = unsafe {
            AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(&input, None)
        };
        // SAFETY: the input was made above, and writing has not started yet.
        unsafe {
            if !writer.canAddInput(&input) {
                return Err(writer_error(&writer, "the writer refused the video input"));
            }
            writer.addInput(&input);
            if !writer.startWriting() {
                return Err(writer_error(&writer, "writing did not start"));
            }
            writer.startSessionAtSourceTime(CMTime::new(0, fps as i32));
        }
        Ok(Self {
            writer,
            input,
            adaptor,
            width: width as usize,
            height: height as usize,
            fps: fps as i32,
            count: 0,
        })
    }

    /// Adds video frame `k`. Frames arrive in order, starting at 0.
    pub fn push(&mut self, k: u64, image: &RgbaImage) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        // SAFETY: a property read.
        while !unsafe { self.input.isReadyForMoreMediaData() } {
            if Instant::now() > deadline {
                return Err(failed("the encoder did not take a frame within 10 seconds"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        let mut out: *mut CVPixelBuffer = std::ptr::null_mut();
        // SAFETY: `out` is a valid place for the pointer to the created buffer.
        let status = unsafe {
            CVPixelBufferCreate(
                None,
                self.width,
                self.height,
                kCVPixelFormatType_32BGRA,
                None,
                NonNull::from(&mut out),
            )
        };
        let Some(out) = NonNull::new(out).filter(|_| status == kCVReturnSuccess) else {
            return Err(failed(format!("CVPixelBufferCreate returned {status}")));
        };
        // SAFETY: a Create function hands over a buffer we own; CFRetained releases it.
        let buffer: CFRetained<CVPixelBuffer> = unsafe { CFRetained::from_raw(out) };

        let row_bytes = self.width * 4;
        // SAFETY: the buffer is locked while its memory is written. It is `width` by
        // `height` BGRA, so each of the `height` rows starts `bytes_per_row` after the
        // last and holds at least `width * 4` bytes, which is all that is written.
        unsafe {
            if CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags(0)) != kCVReturnSuccess
            {
                return Err(failed("a pixel buffer could not be locked"));
            }
            let base = CVPixelBufferGetBaseAddress(&buffer).cast::<u8>();
            let stride = CVPixelBufferGetBytesPerRow(&buffer);
            for (y, row) in image
                .as_raw()
                .chunks_exact(row_bytes)
                .take(self.height)
                .enumerate()
            {
                let dst = std::slice::from_raw_parts_mut(base.add(y * stride), row_bytes);
                for (d, s) in dst
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(row.as_chunks::<4>().0)
                {
                    *d = [s[2], s[1], s[0], s[3]];
                }
            }
            CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
        }

        // SAFETY: the input is ready, and the buffer is a filled BGRA picture of the video's size.
        let appended = unsafe {
            self.adaptor
                .appendPixelBuffer_withPresentationTime(&buffer, CMTime::new(k as i64, self.fps))
        };
        if !appended {
            return Err(writer_error(&self.writer, "a frame was not appended"));
        }
        self.count += 1;
        Ok(())
    }

    /// Completes the file. `finishWriting` blocks until the file is closed. It runs on
    /// a thread of its own: on the main thread (`record` encodes there) AVFoundation
    /// prints a warning to stderr, where recdr's JSON errors go.
    pub fn finish(self) -> Result<()> {
        struct Writer<'a>(&'a AVAssetWriter);
        // SAFETY: the writer is used by one thread at a time; this thread waits below.
        unsafe impl Send for Writer<'_> {}
        // SAFETY: called once, after the last append; the appending thread waits.
        unsafe {
            self.input.markAsFinished();
            self.writer
                .endSessionAtSourceTime(CMTime::new(self.count, self.fps));
        }
        let writer = Writer(&self.writer);
        let finished = std::thread::scope(|scope| {
            scope
                .spawn(move || {
                    let writer = writer;
                    // SAFETY: as above.
                    #[allow(deprecated)]
                    unsafe {
                        writer.0.finishWriting()
                    }
                })
                .join()
                .unwrap_or(false)
        });
        // SAFETY: a property read.
        if !finished || unsafe { self.writer.status() } != AVAssetWriterStatus::Completed {
            return Err(writer_error(&self.writer, "the file was not finished"));
        }
        Ok(())
    }
}
