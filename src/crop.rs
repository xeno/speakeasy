/// Pixel rectangle inside a square frame, used for both overlays and ffmpeg crops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl PixelRect {
    pub fn ffmpeg_crop(self) -> String {
        format!("crop={}:{}:{}:{}", self.w, self.h, self.x, self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SquareCrops {
    pub side: u32,
    pub horizontal: PixelRect,
    pub vertical: PixelRect,
}

pub fn even(n: u32) -> u32 {
    n & !1
}

/// Center-crop an RGB24 frame to a 1:1 square with even sides (h264-friendly).
#[cfg(test)]
pub fn center_square_rgb(rgb: &[u8], width: u32, height: u32) -> Option<(Vec<u8>, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let expected = width as usize * height as usize * 3;
    if rgb.len() < expected {
        return None;
    }
    let side = even(width.min(height));
    if side < 16 {
        return None;
    }
    let x0 = (width - side) / 2;
    let y0 = (height - side) / 2;
    let mut out = Vec::with_capacity(side as usize * side as usize * 3);
    for y in 0..side {
        let src = ((y0 + y) * width + x0) as usize * 3;
        out.extend_from_slice(&rgb[src..src + side as usize * 3]);
    }
    Some((out, side))
}

/// Center-crop to a square and scale into `out`. Set `mirror` to flip horizontally.
pub fn scale_square(
    rgb: &[u8],
    width: u32,
    height: u32,
    out_side: u32,
    mirror: bool,
    out: &mut Vec<u8>,
) -> Option<u32> {
    if width == 0 || height == 0 {
        return None;
    }
    let expected = width as usize * height as usize * 3;
    if rgb.len() < expected {
        return None;
    }
    let src_side = even(width.min(height));
    let out_side = even(out_side.min(src_side));
    if out_side < 16 {
        return None;
    }
    let x0 = (width - src_side) / 2;
    let y0 = (height - src_side) / 2;
    out.resize(out_side as usize * out_side as usize * 3, 0);
    for y in 0..out_side {
        let src_y = y0 + y * src_side / out_side;
        let src_row = (src_y * width) as usize * 3;
        let dst_row = (y * out_side) as usize * 3;
        for x in 0..out_side {
            let sample_x = if mirror { out_side - 1 - x } else { x };
            let src_x = x0 + sample_x * src_side / out_side;
            let src = src_row + src_x as usize * 3;
            let dst = dst_row + x as usize * 3;
            out[dst] = rgb[src];
            out[dst + 1] = rgb[src + 1];
            out[dst + 2] = rgb[src + 2];
        }
    }
    Some(out_side)
}

/// 16:9 landscape and 9:16 portrait crops centered in a square.
pub fn crops_for_square(side: u32) -> SquareCrops {
    let side = even(side.max(16));
    let band = even(((side as f64) * 9.0 / 16.0).round() as u32).max(2);
    let offset = even((side - band) / 2);
    SquareCrops {
        side,
        horizontal: PixelRect {
            x: 0,
            y: offset.min(side.saturating_sub(band)),
            w: side,
            h: band.min(side),
        },
        vertical: PixelRect {
            x: offset.min(side.saturating_sub(band)),
            y: 0,
            w: band.min(side),
            h: side,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_is_16_by_9() {
        let crops = crops_for_square(1080);
        let ratio = f64::from(crops.horizontal.w) / f64::from(crops.horizontal.h);
        assert!((ratio - 16.0 / 9.0).abs() < 0.02);
        assert_eq!(crops.horizontal.w % 2, 0);
        assert_eq!(crops.horizontal.h % 2, 0);
        assert!(crops.horizontal.y + crops.horizontal.h <= crops.side);
    }

    #[test]
    fn vertical_is_9_by_16() {
        let crops = crops_for_square(1080);
        let ratio = f64::from(crops.vertical.h) / f64::from(crops.vertical.w);
        assert!((ratio - 16.0 / 9.0).abs() < 0.02);
        assert_eq!(crops.vertical.w % 2, 0);
        assert_eq!(crops.vertical.h % 2, 0);
        assert!(crops.vertical.x + crops.vertical.w <= crops.side);
    }

    #[test]
    fn center_square_crops_landscape() {
        let width = 40;
        let height = 24;
        let rgb = vec![0_u8; (width * height * 3) as usize];
        let (square, side) = center_square_rgb(&rgb, width, height).unwrap();
        assert_eq!(side, 24);
        assert_eq!(square.len(), 24 * 24 * 3);
    }

    #[test]
    fn scale_square_mirrors_and_resizes() {
        let width = 40;
        let height = 24;
        let mut rgb = vec![0_u8; (width * height * 3) as usize];
        rgb[8 * 3] = 255;
        let mut out = Vec::new();
        let side = scale_square(&rgb, width, height, 16, true, &mut out).unwrap();
        assert_eq!(side, 16);
        assert_eq!(out.len(), 16 * 16 * 3);
        assert_eq!(out[15 * 3], 255);
    }
}
