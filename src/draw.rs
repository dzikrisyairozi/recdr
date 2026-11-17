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

#[cfg(test)]
mod tests {
    use super::*;

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
