use image::RgbaImage;
use std::io::{Read, Seek};
use std::path::Path;

use crate::paa::{PaaError, PaaImage};

/// Computes output dimensions constrained to a square bounding box of `max_size`
/// while strictly preserving aspect ratio and never upscaling.
pub fn calculate_thumbnail_dimensions(
    width: u32,
    height: u32,
    max_size: u32,
) -> Result<(u32, u32), PaaError> {
    if max_size == 0 {
        return Err(PaaError::InvalidDimensions(0, 0));
    }
    if width == 0 || height == 0 {
        return Err(PaaError::InvalidDimensions(width, height));
    }
    if width <= max_size && height <= max_size {
        return Ok((width, height));
    }

    let scale = (max_size as f64 / width as f64).min(max_size as f64 / height as f64);
    let target_w = ((width as f64 * scale).round() as u32).clamp(1, max_size);
    let target_h = ((height as f64 * scale).round() as u32).clamp(1, max_size);

    Ok((target_w, target_h))
}

/// Renders mipmap 0 from a PAA stream as an RGBA image, resized to fit within `max_size`
/// bounding box using Lanczos3 filtering without upscaling.
pub fn render_paa_thumbnail<R: Read + Seek>(
    reader: &mut R,
    max_size: u32,
) -> Result<RgbaImage, PaaError> {
    if max_size == 0 {
        return Err(PaaError::InvalidDimensions(0, 0));
    }

    let paa = PaaImage::read(reader)?;
    let mip0 = paa
        .maps
        .first()
        .ok_or_else(|| PaaError::CorruptedData("No mipmaps found in PAA file".into()))?;

    let orig_w = mip0.width(paa.format);
    let orig_h = mip0.height();
    if orig_w == 0 || orig_h == 0 {
        return Err(PaaError::InvalidDimensions(orig_w, orig_h));
    }

    let (target_w, target_h) = calculate_thumbnail_dimensions(orig_w, orig_h, max_size)?;
    let base_rgba = mip0.to_rgba_image(paa.format)?;

    if target_w == orig_w && target_h == orig_h {
        Ok(base_rgba)
    } else {
        Ok(image::imageops::resize(
            &base_rgba,
            target_w,
            target_h,
            image::imageops::FilterType::Lanczos3,
        ))
    }
}

/// Reads a PAA file from `input`, decodes mipmap 0, resizes it to fit `max_size`,
/// and saves it as a PNG image to `output` regardless of output file extension.
pub fn write_paa_thumbnail(input: &Path, output: &Path, max_size: u32) -> Result<(), String> {
    if max_size == 0 {
        return Err("Thumbnail max size cannot be 0".to_string());
    }

    let file = std::fs::File::open(input)
        .map_err(|e| format!("Failed to open input file '{}': {e}", input.display()))?;
    let mut reader = std::io::BufReader::new(file);

    let thumbnail = render_paa_thumbnail(&mut reader, max_size)
        .map_err(|e| format!("Failed to render thumbnail from '{}': {e}", input.display()))?;

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create output directory '{}': {e}",
                    parent.display()
                )
            })?;
        }
    }

    image::DynamicImage::ImageRgba8(thumbnail)
        .save_with_format(output, image::ImageFormat::Png)
        .map_err(|e| {
            format!(
                "Failed to save PNG thumbnail to '{}': {e}",
                output.display()
            )
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paa::{PaXType, PaaImage};
    use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn make_test_paa(width: u32, height: u32) -> Vec<u8> {
        let mut img = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let r = ((x * 255) / width.max(1)) as u8;
                let g = ((y * 255) / height.max(1)) as u8;
                let b = 128u8;
                let a = 255u8;
                img.put_pixel(x, y, Rgba([r, g, b, a]));
            }
        }
        let dynamic = DynamicImage::ImageRgba8(img);
        let paa = PaaImage::from_dynamic_image(&dynamic, PaXType::DXT1, true)
            .expect("failed to create PAA from test image");
        let mut buffer = Vec::new();
        paa.write(&mut Cursor::new(&mut buffer))
            .expect("failed to write test PAA to buffer");
        buffer
    }

    #[test]
    fn test_calculate_thumbnail_dimensions_bounding() {
        let (w, h) = calculate_thumbnail_dimensions(1024, 768, 256).unwrap();
        assert!(w <= 256);
        assert!(h <= 256);
        assert_eq!(w, 256);
        assert_eq!(h, 192);

        let (w, h) = calculate_thumbnail_dimensions(768, 1024, 256).unwrap();
        assert!(w <= 256);
        assert!(h <= 256);
        assert_eq!(w, 192);
        assert_eq!(h, 256);

        let (w, h) = calculate_thumbnail_dimensions(500, 500, 256).unwrap();
        assert_eq!(w, 256);
        assert_eq!(h, 256);
    }

    #[test]
    fn test_calculate_thumbnail_dimensions_aspect_preservation() {
        let (w, h) = calculate_thumbnail_dimensions(1920, 1080, 256).unwrap();
        assert_eq!(w, 256);
        assert_eq!(h, 144);
        let orig_ratio = 1920.0 / 1080.0;
        let thumb_ratio = w as f64 / h as f64;
        assert!((orig_ratio - thumb_ratio).abs() < 0.01);

        let (w, h) = calculate_thumbnail_dimensions(800, 200, 100).unwrap();
        assert_eq!(w, 100);
        assert_eq!(h, 25);
        assert_eq!(w / h, 800 / 200);

        let (w, h) = calculate_thumbnail_dimensions(200, 800, 100).unwrap();
        assert_eq!(w, 25);
        assert_eq!(h, 100);
    }

    #[test]
    fn test_calculate_thumbnail_dimensions_no_upscaling() {
        let (w, h) = calculate_thumbnail_dimensions(64, 64, 256).unwrap();
        assert_eq!(w, 64);
        assert_eq!(h, 64);

        let (w, h) = calculate_thumbnail_dimensions(120, 80, 256).unwrap();
        assert_eq!(w, 120);
        assert_eq!(h, 80);

        let (w, h) = calculate_thumbnail_dimensions(256, 256, 256).unwrap();
        assert_eq!(w, 256);
        assert_eq!(h, 256);
    }

    #[test]
    fn test_zero_size_rejection() {
        assert!(matches!(
            calculate_thumbnail_dimensions(128, 128, 0),
            Err(PaaError::InvalidDimensions(0, 0))
        ));

        let mut empty = Cursor::new(Vec::new());
        assert!(matches!(
            render_paa_thumbnail(&mut empty, 0),
            Err(PaaError::InvalidDimensions(0, 0))
        ));

        let temp_dir = std::env::temp_dir().join("paa_thumb_zero_test");
        let dummy_in = temp_dir.join("dummy.paa");
        let dummy_out = temp_dir.join("dummy.png");
        let res = write_paa_thumbnail(&dummy_in, &dummy_out, 0);
        assert!(res.is_err());
    }

    #[test]
    fn test_render_paa_thumbnail_end_to_end() {
        let paa_data = make_test_paa(128, 64);

        // Resize down to fit max_size = 32 -> should be 32x16
        let mut cursor = Cursor::new(&paa_data);
        let thumb = render_paa_thumbnail(&mut cursor, 32).expect("render thumbnail failed");
        assert_eq!(thumb.dimensions(), (32, 16));

        // When max_size is larger than original -> should not upscale (128x64)
        let mut cursor = Cursor::new(&paa_data);
        let thumb_no_upscale =
            render_paa_thumbnail(&mut cursor, 512).expect("render thumbnail failed");
        assert_eq!(thumb_no_upscale.dimensions(), (128, 64));
    }

    #[test]
    fn test_write_paa_thumbnail_png_output_regardless_of_suffix() {
        let paa_data = make_test_paa(64, 64);

        let temp_dir = std::env::temp_dir().join(format!("paa_thumb_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        let input_path = temp_dir.join("test_input.paa");
        std::fs::write(&input_path, &paa_data).expect("write temp paa");

        // Use arbitrary non-PNG extensions and extensionless paths
        let non_png_outputs = [
            temp_dir.join("thumb.custom_ext"),
            temp_dir.join("thumb.jpg"),
            temp_dir.join("thumb_no_ext"),
        ];

        for out_path in &non_png_outputs {
            write_paa_thumbnail(&input_path, out_path, 32).expect("write_paa_thumbnail failed");

            let bytes = std::fs::read(out_path).expect("read output thumbnail");
            // Standard PNG magic header bytes: 0x89 'P' 'N' 'G' 0x0D 0x0A 0x1A 0x0A
            assert_eq!(
                &bytes[..8],
                b"\x89PNG\r\n\x1a\n",
                "Output file must be encoded as PNG regardless of suffix"
            );

            // Verify it parses as PNG
            let format = image::guess_format(&bytes).expect("guess format");
            assert_eq!(format, ImageFormat::Png);

            let decoded = image::load_from_memory(&bytes).expect("load decoded png");
            assert_eq!(decoded.dimensions(), (32, 32));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
