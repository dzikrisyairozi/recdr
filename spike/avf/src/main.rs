//! Throwaway: does AVAssetWriter write an H.264 MP4 with the right colors and row
//! order from Rust? Plan task 7 of tasks/5-native-mp4. Never merged.

#[allow(dead_code)]
#[path = "../../../tests/common/mp4.rs"]
mod mp4;

use image::RgbaImage;

fn quarters(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba(if x < width / 2 && y < height / 2 {
            [255, 0, 0, 255]
        } else if x >= width / 2 && y >= height / 2 {
            [0, 0, 255, 255]
        } else {
            [255, 255, 255, 255]
        })
    })
}

#[cfg(target_os = "macos")]
mod avf {
    use std::path::Path;
    use std::ptr::NonNull;
    use std::time::{Duration, Instant};

    use image::RgbaImage;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_av_foundation::{
        AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor,
        AVAssetWriterStatus, AVFileTypeMPEG4, AVMediaTypeVideo, AVVideoAverageBitRateKey,
        AVVideoCodecKey, AVVideoCodecTypeH264, AVVideoCompressionPropertiesKey, AVVideoHeightKey,
        AVVideoWidthKey,
    };
    use objc2_core_foundation::CFRetained;
    use objc2_core_media::CMTime;
    use objc2_core_video::{
        CVPixelBuffer, CVPixelBufferCreate, CVPixelBufferGetBaseAddress,
        CVPixelBufferGetBytesPerRow, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
        CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_32BGRA, kCVReturnSuccess,
    };
    use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

    pub struct Writer {
        writer: Retained<AVAssetWriter>,
        input: Retained<AVAssetWriterInput>,
        adaptor: Retained<AVAssetWriterInputPixelBufferAdaptor>,
        width: usize,
        height: usize,
        fps: i32,
        count: i64,
    }

    fn writer_error(writer: &AVAssetWriter, what: &str) -> String {
        // SAFETY: `error` is a plain property read on a live writer.
        match unsafe { writer.error() } {
            Some(error) => format!("{what}: {}", error.localizedDescription()),
            None => what.to_owned(),
        }
    }

    impl Writer {
        pub fn create(
            path: &Path,
            width: u32,
            height: u32,
            fps: u32,
            bit_rate: u32,
        ) -> Result<Self, String> {
            if path.is_file() {
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
            }
            let url = NSURL::from_file_path(path).ok_or("not a file path")?;
            // SAFETY: the statics are constants AVFoundation defines; reading them is a load.
            let (file_type, media, codec, h264, width_key, height_key, compression_key, rate_key) = unsafe {
                (
                    AVFileTypeMPEG4.ok_or("no AVFileTypeMPEG4")?,
                    AVMediaTypeVideo.ok_or("no AVMediaTypeVideo")?,
                    AVVideoCodecKey.ok_or("no AVVideoCodecKey")?,
                    AVVideoCodecTypeH264.ok_or("no AVVideoCodecTypeH264")?,
                    AVVideoWidthKey.ok_or("no AVVideoWidthKey")?,
                    AVVideoHeightKey.ok_or("no AVVideoHeightKey")?,
                    AVVideoCompressionPropertiesKey.ok_or("no AVVideoCompressionPropertiesKey")?,
                    AVVideoAverageBitRateKey.ok_or("no AVVideoAverageBitRateKey")?,
                )
            };
            // SAFETY: a file URL and a file type constant, as the method expects.
            let writer = unsafe { AVAssetWriter::assetWriterWithURL_fileType_error(&url, file_type) }
                .map_err(|e| e.localizedDescription().to_string())?;

            let rate = NSNumber::new_u32(bit_rate);
            let rate: &AnyObject = &rate;
            let compression = NSDictionary::<NSString, AnyObject>::from_slices(&[rate_key], &[rate]);
            let (w, h) = (NSNumber::new_u32(width), NSNumber::new_u32(height));
            let values: [&AnyObject; 4] = [h264, &w, &h, &compression];
            let settings = NSDictionary::<NSString, AnyObject>::from_slices(
                &[codec, width_key, height_key, compression_key],
                &values,
            );
            // SAFETY: the settings dictionary holds the keys and value types AVVideoSettings.h names.
            let input = unsafe {
                AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(media, Some(&settings))
            };
            // SAFETY: plain property set before writing starts.
            unsafe { input.setExpectsMediaDataInRealTime(false) };
            // SAFETY: no attributes; buffers are made with CVPixelBufferCreate.
            let adaptor = unsafe {
                AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(&input, None)
            };
            // SAFETY: the input was made above and writing has not started.
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

        pub fn push(&mut self, k: u64, image: &RgbaImage) -> Result<(), String> {
            let deadline = Instant::now() + Duration::from_secs(10);
            // SAFETY: plain property read.
            while !unsafe { self.input.isReadyForMoreMediaData() } {
                if Instant::now() > deadline {
                    return Err("the encoder did not take a frame within 10 seconds".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let mut out: *mut CVPixelBuffer = std::ptr::null_mut();
            // SAFETY: `out` is a valid place for the created buffer.
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
            let out = NonNull::new(out).filter(|_| status == kCVReturnSuccess);
            let Some(out) = out else {
                return Err(format!("CVPixelBufferCreate returned {status}"));
            };
            // SAFETY: a Create function returns a buffer we own, which CFRetained releases.
            let buffer: CFRetained<CVPixelBuffer> = unsafe { CFRetained::from_raw(out) };
            // SAFETY: the buffer is locked while its memory is written, and every row
            // written is inside the `bytes_per_row * height` bytes the buffer holds.
            unsafe {
                if CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags(0)) != kCVReturnSuccess {
                    return Err("could not lock a pixel buffer".into());
                }
                let base = CVPixelBufferGetBaseAddress(&buffer).cast::<u8>();
                let stride = CVPixelBufferGetBytesPerRow(&buffer);
                for (y, row) in image.as_raw().chunks_exact(self.width * 4).enumerate() {
                    let dst = std::slice::from_raw_parts_mut(base.add(y * stride), self.width * 4);
                    for (d, s) in dst.chunks_exact_mut(4).zip(row.chunks_exact(4)) {
                        d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
                    }
                }
                CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
            }
            // SAFETY: the input is ready, and the buffer is a filled BGRA buffer of the video's size.
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

        pub fn finish(self) -> Result<(), String> {
            // SAFETY: called once, after the last append, on this thread.
            let finished = unsafe {
                self.input.markAsFinished();
                self.writer
                    .endSessionAtSourceTime(CMTime::new(self.count, self.fps));
                #[allow(deprecated)]
                self.writer.finishWriting()
            };
            // SAFETY: plain property read.
            if !finished || unsafe { self.writer.status() } != AVAssetWriterStatus::Completed {
                return Err(writer_error(&self.writer, "the file was not finished"));
            }
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn main() {
    let path = std::env::current_dir().unwrap().join("out.mp4");
    let picture = quarters(320, 240);
    let begun = std::time::Instant::now();
    let mut writer = avf::Writer::create(&path, 320, 240, 10, 1_000_000).unwrap();
    for k in 0..10 {
        writer.push(k, &picture).unwrap();
    }
    writer.finish().unwrap();
    println!("wrote {} in {:?}", path.display(), begun.elapsed());

    let facts = mp4::facts(&path);
    println!(
        "codec {}, {} by {}, {} ms",
        facts.codec, facts.width, facts.height, facts.duration_ms
    );
    let png = path.with_extension("png");
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(&path)
        .args(["-frames:v", "1"])
        .arg(&png)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg could not decode the file");
    let frame = image::open(&png).unwrap().to_rgba8();
    let red = frame.get_pixel(40, 30).0;
    let blue = frame.get_pixel(280, 210).0;
    println!("pixel (40, 30) {red:?}, pixel (280, 210) {blue:?}");

    let h264 = ["avc1", "avc3"].contains(&facts.codec.as_str());
    let size = (facts.width, facts.height) == (320, 240);
    let is_red = red[0] > 180 && red[1] < 90 && red[2] < 90;
    let is_blue = blue[2] > 180 && blue[0] < 90 && blue[1] < 90;
    println!("h264 {h264}, size {size}, red {is_red}, blue {is_blue}");
    if !(h264 && size && is_red && is_blue) {
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    let _ = quarters(2, 2);
    eprintln!("this program runs on macOS only");
    std::process::exit(1);
}
