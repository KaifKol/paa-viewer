use std::io::Cursor;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use paa_viewer::convert::{convert_image, ConvertOptions, ResizeOperation, TargetFormat};
use paa_viewer::paa::{PaaImage, PaXType};
use paa_viewer::resize::{nearest_power_of_two, next_power_of_two, resize_pot, scale_by_factor, scale_step, PotMode, ResizeFilter};

fn create_test_image(width: u32, height: u32, with_alpha: bool) -> DynamicImage {
    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let r = ((x * 255) / width.max(1)) as u8;
            let g = ((y * 255) / height.max(1)) as u8;
            let b = (((x + y) * 255) / (width + height).max(1)) as u8;
            let a = if with_alpha {
                if (x + y) % 2 == 0 { 128 } else { 255 }
            } else {
                255
            };
            img.put_pixel(x, y, Rgba([r, g, b, a]));
        }
    }
    DynamicImage::ImageRgba8(img)
}

#[test]
fn test_paa_dxt1_roundtrip() {
    let original = create_test_image(128, 128, false);
    let paa = PaaImage::from_dynamic_image(&original, PaXType::DXT1, true).expect("PAA creation failed");

    assert_eq!(paa.format, PaXType::DXT1);
    assert!(!paa.maps.is_empty());
    assert_eq!(paa.maps[0].width(PaXType::DXT1), 128);
    assert_eq!(paa.maps[0].height(), 128);

    let mut buffer = Cursor::new(Vec::new());
    paa.write(&mut buffer).expect("PAA write failed");

    buffer.set_position(0);
    let decoded_paa = PaaImage::read(&mut buffer).expect("PAA read failed");
    assert_eq!(decoded_paa.format, PaXType::DXT1);
    assert_eq!(decoded_paa.maps.len(), paa.maps.len());

    let decoded_img = decoded_paa.to_dynamic_image(0).expect("to_dynamic_image failed");
    assert_eq!(decoded_img.dimensions(), (128, 128));
}

#[test]
fn test_paa_dxt5_roundtrip_with_alpha() {
    let original = create_test_image(256, 128, true);
    let paa = PaaImage::from_dynamic_image(&original, PaXType::DXT5, true).expect("PAA creation failed");

    assert_eq!(paa.format, PaXType::DXT5);
    assert!(paa.taggs.has_alpha());

    let mut buffer = Cursor::new(Vec::new());
    paa.write(&mut buffer).expect("PAA write failed");

    buffer.set_position(0);
    let decoded_paa = PaaImage::read(&mut buffer).expect("PAA read failed");
    assert_eq!(decoded_paa.format, PaXType::DXT5);

    let decoded_img = decoded_paa.to_dynamic_image(0).expect("to_dynamic_image failed");
    assert_eq!(decoded_img.dimensions(), (256, 128));
}

#[test]
fn test_paa_argb8888_roundtrip() {
    let original = create_test_image(64, 64, true);
    let paa = PaaImage::from_dynamic_image(&original, PaXType::ARGB8, false).expect("PAA creation failed");

    let mut buffer = Cursor::new(Vec::new());
    paa.write(&mut buffer).expect("PAA write failed");

    buffer.set_position(0);
    let decoded_paa = PaaImage::read(&mut buffer).expect("PAA read failed");
    assert_eq!(decoded_paa.format, PaXType::ARGB8);

    let decoded_img = decoded_paa.to_dynamic_image(0).expect("to_dynamic_image failed");
    assert_eq!(decoded_img.dimensions(), (64, 64));

    // For uncompressed ARGB8888, colors should match perfectly
    let orig_rgba = original.to_rgba8();
    let dec_rgba = decoded_img.to_rgba8();
    for y in 0..64 {
        for x in 0..64 {
            assert_eq!(orig_rgba.get_pixel(x, y), dec_rgba.get_pixel(x, y));
        }
    }
}

#[test]
fn test_paa_argb1555_roundtrip() {
    let original = create_test_image(32, 32, true);
    let paa = PaaImage::from_dynamic_image(&original, PaXType::ARGB5, false).expect("PAA creation failed");

    let mut buffer = Cursor::new(Vec::new());
    paa.write(&mut buffer).expect("PAA write failed");

    buffer.set_position(0);
    let decoded_paa = PaaImage::read(&mut buffer).expect("PAA read failed");
    assert_eq!(decoded_paa.format, PaXType::ARGB5);
    let decoded_img = decoded_paa.to_dynamic_image(0).expect("to_dynamic_image failed");
    assert_eq!(decoded_img.dimensions(), (32, 32));
}

#[test]
fn test_paa_argb4444_roundtrip() {
    let original = create_test_image(32, 32, true);
    let paa = PaaImage::from_dynamic_image(&original, PaXType::ARGB4, false).expect("PAA creation failed");

    let mut buffer = Cursor::new(Vec::new());
    paa.write(&mut buffer).expect("PAA write failed");

    buffer.set_position(0);
    let decoded_paa = PaaImage::read(&mut buffer).expect("PAA read failed");
    assert_eq!(decoded_paa.format, PaXType::ARGB4);
    let decoded_img = decoded_paa.to_dynamic_image(0).expect("to_dynamic_image failed");
    assert_eq!(decoded_img.dimensions(), (32, 32));
}

#[test]
fn test_paa_graya_roundtrip() {
    let original = create_test_image(32, 32, false);
    let paa = PaaImage::from_dynamic_image(&original, PaXType::GRAYA, false).expect("PAA creation failed");

    let mut buffer = Cursor::new(Vec::new());
    paa.write(&mut buffer).expect("PAA write failed");

    buffer.set_position(0);
    let decoded_paa = PaaImage::read(&mut buffer).expect("PAA read failed");
    assert_eq!(decoded_paa.format, PaXType::GRAYA);
    let decoded_img = decoded_paa.to_dynamic_image(0).expect("to_dynamic_image failed");
    assert_eq!(decoded_img.dimensions(), (32, 32));
}

#[test]
fn test_metadata_and_tags() {
    let original = create_test_image(512, 256, true);
    let paa = PaaImage::from_dynamic_image(&original, PaXType::DXT5, true).expect("PAA creation failed");
    let meta = paa.metadata();

    assert_eq!(meta.width, 512);
    assert_eq!(meta.height, 256);
    assert!(meta.is_power_of_two);
    assert_eq!(meta.aspect_ratio, "2:1");
    assert_eq!(meta.aspect_ratio_num, 2.0);
    assert!(meta.has_alpha);
    assert!(meta.average_color.is_some());
    assert!(meta.max_color.is_some());
    assert!(meta.mipmap_count > 1);

    // Mipmap hierarchy check
    assert_eq!(meta.mipmaps[0].width, 512);
    assert_eq!(meta.mipmaps[0].height, 256);
    assert_eq!(meta.mipmaps[1].width, 256);
    assert_eq!(meta.mipmaps[1].height, 128);
}

#[test]
fn test_resize_operations() {
    let img = create_test_image(500, 300, false);

    // Scaling by factor
    let half = scale_by_factor(&img, 0.5, ResizeFilter::Lanczos3);
    assert_eq!(half.dimensions(), (250, 150));

    let step_double = scale_step(&img, 1, ResizeFilter::Lanczos3);
    assert_eq!(step_double.dimensions(), (1000, 600));

    // Power of two nearest
    let pot_near = resize_pot(&img, PotMode::Nearest, ResizeFilter::Lanczos3);
    assert_eq!(pot_near.dimensions(), (nearest_power_of_two(500), nearest_power_of_two(300)));
    assert_eq!(pot_near.dimensions(), (512, 256));

    // Power of two next
    let pot_next = resize_pot(&img, PotMode::Next, ResizeFilter::Lanczos3);
    assert_eq!(pot_next.dimensions(), (next_power_of_two(500), next_power_of_two(300)));
    assert_eq!(pot_next.dimensions(), (512, 512));

    // Stretch to exact dimensions
    let pot_stretch = resize_pot(&img, PotMode::Stretch { target_w: 1024, target_h: 512 }, ResizeFilter::Lanczos3);
    assert_eq!(pot_stretch.dimensions(), (1024, 512));

    // Fit & Pad
    let pot_fit = resize_pot(&img, PotMode::FitPad { target_w: 1024, target_h: 1024 }, ResizeFilter::Lanczos3);
    assert_eq!(pot_fit.dimensions(), (1024, 1024));
}

#[test]
fn test_convert_pipeline_png_to_paa_and_back() {
    let temp_dir = std::env::temp_dir().join("paa_viewer_test");
    std::fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

    let src_png = temp_dir.join("test_src.png");
    let out_paa = temp_dir.join("test_out.paa");
    let back_png = temp_dir.join("test_back.png");

    let img = create_test_image(256, 256, true);
    img.save(&src_png).expect("failed to save test png");

    // Convert PNG -> PAA with DXT5 and 0.5x scaling
    let opts = ConvertOptions {
        target: TargetFormat::Paa(Some(PaXType::DXT5)),
        generate_mipmaps: true,
        resize: ResizeOperation::Factor(0.5),
        filter: ResizeFilter::Lanczos3,
    };

    let rep = convert_image(&src_png, &out_paa, &opts).expect("PNG -> PAA failed");
    assert_eq!(rep.original_dimensions, (256, 256));
    assert_eq!(rep.final_dimensions, (128, 128));
    assert!(out_paa.exists());

    // Convert PAA -> PNG
    let back_opts = ConvertOptions {
        target: TargetFormat::Png,
        generate_mipmaps: false,
        resize: ResizeOperation::None,
        filter: ResizeFilter::Lanczos3,
    };

    let rep_back = convert_image(&out_paa, &back_png, &back_opts).expect("PAA -> PNG failed");
    assert_eq!(rep_back.final_dimensions, (128, 128));
    assert!(back_png.exists());

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}
