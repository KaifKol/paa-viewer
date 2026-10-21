use image::imageops::{self, FilterType};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResizeFilter {
    Lanczos3,
    Triangle,
    Nearest,
}

impl From<ResizeFilter> for FilterType {
    fn from(f: ResizeFilter) -> Self {
        match f {
            ResizeFilter::Lanczos3 => FilterType::Lanczos3,
            ResizeFilter::Triangle => FilterType::Triangle,
            ResizeFilter::Nearest => FilterType::Nearest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PotMode {
    Nearest,
    Next,
    Prev,
    Stretch { target_w: u32, target_h: u32 },
    FitPad { target_w: u32, target_h: u32 },
}

pub fn is_power_of_two(n: u32) -> bool {
    n > 0 && (n & (n - 1)) == 0
}

pub fn nearest_power_of_two(n: u32) -> u32 {
    if n <= 1 {
        return 1;
    }
    let next = next_power_of_two(n);
    let prev = (next / 2).max(1);
    if next.abs_diff(n) < n.abs_diff(prev) {
        next
    } else {
        prev
    }
}

pub fn next_power_of_two(n: u32) -> u32 {
    if n <= 1 {
        return 1;
    }
    n.checked_next_power_of_two().unwrap_or(1 << 31)
}

pub fn prev_power_of_two(n: u32) -> u32 {
    if n <= 1 {
        return 1;
    }
    1u32 << (31 - n.leading_zeros())
}

pub fn scale_by_factor(img: &DynamicImage, factor: f32, filter: ResizeFilter) -> DynamicImage {
    let (w, h) = img.dimensions();
    let target_w = ((w as f32 * factor).round() as u32).max(1);
    let target_h = ((h as f32 * factor).round() as u32).max(1);

    let rgba = img.to_rgba8();
    let resized = imageops::resize(&rgba, target_w, target_h, filter.into());
    DynamicImage::ImageRgba8(resized)
}

pub fn scale_step(img: &DynamicImage, step: i32, filter: ResizeFilter) -> DynamicImage {
    let factor = 2.0f32.powi(step);
    scale_by_factor(img, factor, filter)
}

pub fn resize_pot(img: &DynamicImage, mode: PotMode, filter: ResizeFilter) -> DynamicImage {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();

    match mode {
        PotMode::Nearest => {
            let target_w = nearest_power_of_two(w);
            let target_h = nearest_power_of_two(h);
            let resized = imageops::resize(&rgba, target_w, target_h, filter.into());
            DynamicImage::ImageRgba8(resized)
        }
        PotMode::Next => {
            let target_w = next_power_of_two(w);
            let target_h = next_power_of_two(h);
            let resized = imageops::resize(&rgba, target_w, target_h, filter.into());
            DynamicImage::ImageRgba8(resized)
        }
        PotMode::Prev => {
            let target_w = prev_power_of_two(w);
            let target_h = prev_power_of_two(h);
            let resized = imageops::resize(&rgba, target_w, target_h, filter.into());
            DynamicImage::ImageRgba8(resized)
        }
        PotMode::Stretch { target_w, target_h } => {
            let tw = target_w.max(1);
            let th = target_h.max(1);
            let resized = imageops::resize(&rgba, tw, th, filter.into());
            DynamicImage::ImageRgba8(resized)
        }
        PotMode::FitPad { target_w, target_h } => {
            let tw = target_w.max(1);
            let th = target_h.max(1);

            if w == 0 || h == 0 {
                let out = RgbaImage::from_pixel(tw, th, Rgba([0, 0, 0, 0]));
                return DynamicImage::ImageRgba8(out);
            }

            let scale_x = tw as f32 / w as f32;
            let scale_y = th as f32 / h as f32;
            let scale = scale_x.min(scale_y);
            let scaled_w = ((w as f32 * scale).round() as u32).clamp(1, tw);
            let scaled_h = ((h as f32 * scale).round() as u32).clamp(1, th);

            let scaled = imageops::resize(&rgba, scaled_w, scaled_h, filter.into());

            let mut out = RgbaImage::from_pixel(tw, th, Rgba([0, 0, 0, 0]));
            let offset_x = (tw - scaled_w) / 2;
            let offset_y = (th - scaled_h) / 2;

            for y in 0..scaled_h {
                for x in 0..scaled_w {
                    out.put_pixel(offset_x + x, offset_y + y, *scaled.get_pixel(x, y));
                }
            }

            DynamicImage::ImageRgba8(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pot_calculations() {
        assert!(is_power_of_two(1));
        assert!(is_power_of_two(2));
        assert!(is_power_of_two(512));
        assert!(!is_power_of_two(500));
        assert!(!is_power_of_two(0));

        assert_eq!(nearest_power_of_two(500), 512);
        assert_eq!(nearest_power_of_two(300), 256);
        assert_eq!(nearest_power_of_two(700), 512); // 700 - 512 = 188, 1024 - 700 = 324 -> 512

        assert_eq!(next_power_of_two(500), 512);
        assert_eq!(next_power_of_two(512), 512);
        assert_eq!(next_power_of_two(513), 1024);

        assert_eq!(prev_power_of_two(500), 256);
        assert_eq!(prev_power_of_two(512), 512);
    }

    #[test]
    fn test_scaling_by_factor() {
        let img = DynamicImage::new_rgba8(100, 100);
        let scaled_half = scale_by_factor(&img, 0.5, ResizeFilter::Triangle);
        assert_eq!(scaled_half.dimensions(), (50, 50));

        let scaled_double = scale_by_factor(&img, 2.0, ResizeFilter::Triangle);
        assert_eq!(scaled_double.dimensions(), (200, 200));
    }
}
