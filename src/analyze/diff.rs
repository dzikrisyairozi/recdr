use crate::model::Rect;

/// A channel must move by more than this to count; below it is compression and cursor noise.
pub const CHANNEL_THRESHOLD: u8 = 16;
/// A frame is a keyframe when this share of pixels changed since the last keyframe.
pub const KEYFRAME_FRACTION: f32 = 0.005;

/// The share of pixels that changed between two RGBA buffers, and the box around them.
pub fn diff(a: &[u8], b: &[u8], width: u32, height: u32) -> (f32, Option<Rect>) {
    let mut changed = 0u64;
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    let (a, b) = (a.as_chunks::<4>().0, b.as_chunks::<4>().0);
    for (i, (pa, pb)) in a.iter().zip(b).enumerate() {
        if (0..3).any(|c| pa[c].abs_diff(pb[c]) > CHANNEL_THRESHOLD) {
            let (x, y) = (i as u32 % width, i as u32 / width);
            changed += 1;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    let total = u64::from(width) * u64::from(height);
    if changed == 0 || total == 0 {
        return (0.0, None);
    }
    let bbox = Rect {
        x: x0 as i32,
        y: y0 as i32,
        width: x1 - x0 + 1,
        height: y1 - y0 + 1,
    };
    (changed as f32 / total as f32, Some(bbox))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn black(width: u32, height: u32) -> Vec<u8> {
        [0, 0, 0, 255].repeat((width * height) as usize)
    }

    #[test]
    fn equal_buffers_have_no_change() {
        assert_eq!(diff(&black(4, 4), &black(4, 4), 4, 4), (0.0, None));
    }

    #[test]
    fn a_white_square_is_found() {
        let a = black(100, 100);
        let mut b = black(100, 100);
        for y in 30..40 {
            for x in 20..30 {
                let at = (y * 100 + x) * 4;
                b[at..at + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        let rect = Rect {
            x: 20,
            y: 30,
            width: 10,
            height: 10,
        };
        assert_eq!(diff(&a, &b, 100, 100), (0.01, Some(rect)));
    }

    #[test]
    fn sixteen_is_noise_and_seventeen_is_change() {
        let a = black(1, 1);
        assert_eq!(diff(&a, &[16, 0, 0, 255], 1, 1).0, 0.0);
        assert_eq!(diff(&a, &[0, 17, 0, 255], 1, 1).0, 1.0);
    }
}
