//! Pixel drawing on frames: labels and click rings.

use font8x8::{BASIC_FONTS, UnicodeFonts};
use image::{Rgba, RgbaImage};

/// Draws `text` in white 8x8 glyphs, each bit a `scale` x `scale` square.
pub fn text(image: &mut RgbaImage, x: u32, y: u32, text: &str, scale: u32) {
    let white = Rgba([255, 255, 255, 255]);
    for (i, c) in text.chars().enumerate() {
        let glyph = BASIC_FONTS
            .get(c)
            .or_else(|| BASIC_FONTS.get('?'))
            .unwrap_or_default();
        let left = x + i as u32 * 8 * scale;
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = left + col * scale + dx;
                        let py = y + row as u32 * scale + dy;
                        if px < image.width() && py < image.height() {
                            image.put_pixel(px, py, white);
                        }
                    }
                }
            }
        }
    }
}

/// A red circle outline 3 pixels thick, used to show where a click happened.
pub fn ring(image: &mut RgbaImage, cx: i32, cy: i32, radius: i32) {
    let red = Rgba([255, 64, 64, 255]);
    let (outer, inner) = (radius * radius, (radius - 3).max(0).pow(2));
    for y in cy - radius..=cy + radius {
        for x in cx - radius..=cx + radius {
            let d = (x - cx).pow(2) + (y - cy).pow(2);
            let on_image =
                x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height();
            if on_image && inner <= d && d <= outer {
                image.put_pixel(x as u32, y as u32, red);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_is_an_outline() {
        let mut image = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 255]));
        ring(&mut image, 32, 32, 14);
        assert_eq!(image.get_pixel(32, 32).0, [0, 0, 0, 255]);
        assert_eq!(image.get_pixel(45, 32).0, [255, 64, 64, 255]);
    }

    #[test]
    fn text_stays_inside_its_cell() {
        let mut image = RgbaImage::from_pixel(32, 32, Rgba([0, 0, 0, 255]));
        text(&mut image, 0, 0, "A", 2);
        let white = |x, y| image.get_pixel(x, y).0 == [255, 255, 255, 255];
        assert!((0..16).any(|y| (0..16).any(|x| white(x, y))));
        for y in 0..32 {
            for x in 0..32 {
                if x >= 16 || y >= 16 {
                    assert!(!white(x, y), "{x},{y}");
                }
            }
        }
    }
}
