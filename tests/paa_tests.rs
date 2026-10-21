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

#[test]
fn test_paa_argb4444_colors_not_swapped() {
    let mut img = RgbaImage::new(16, 16);
    // Pure red pixel
    img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    // Pure blue pixel
    img.put_pixel(1, 0, Rgba([0, 0, 255, 255]));

    let dynamic = DynamicImage::ImageRgba8(img);
    let paa = PaaImage::from_dynamic_image(&dynamic, PaXType::ARGB4, false).expect("ARGB4 creation failed");

    let mut buffer = Cursor::new(Vec::new());
    paa.write(&mut buffer).expect("write failed");
    buffer.set_position(0);

    let read_paa = PaaImage::read(&mut buffer).expect("read failed");
    let decoded = read_paa.to_dynamic_image(0).expect("decode failed").to_rgba8();

    let red_pixel = decoded.get_pixel(0, 0);
    assert!(red_pixel[0] >= 240, "Red channel must be high, got {}", red_pixel[0]);
    assert!(red_pixel[2] <= 16, "Blue channel must be low, got {}", red_pixel[2]);

    let blue_pixel = decoded.get_pixel(1, 0);
    assert!(blue_pixel[2] >= 240, "Blue channel must be high, got {}", blue_pixel[2]);
    assert!(blue_pixel[0] <= 16, "Red channel must be low, got {}", blue_pixel[0]);
}

#[test]
fn test_paa_graya_alpha_preserved() {
    let mut img = RgbaImage::new(16, 16);
    img.put_pixel(0, 0, Rgba([200, 200, 200, 100]));

    let dynamic = DynamicImage::ImageRgba8(img);
    let paa = PaaImage::from_dynamic_image(&dynamic, PaXType::GRAYA, false).expect("GRAYA creation failed");

    let mut buffer = Cursor::new(Vec::new());
    paa.write(&mut buffer).expect("write failed");
    buffer.set_position(0);

    let read_paa = PaaImage::read(&mut buffer).expect("read failed");
    let decoded = read_paa.to_dynamic_image(0).expect("decode failed").to_rgba8();

    let pixel = decoded.get_pixel(0, 0);
    assert_eq!(pixel[3], 100, "Alpha channel must be preserved in GRAYA (0x8080 AI88)");
    assert!((pixel[0] as i32 - 200).abs() <= 2, "Grayscale value should be preserved");
}

#[test]
fn test_rectangular_mipmap_generation() {
    let img = create_test_image(512, 16, false);
    let paa = PaaImage::from_dynamic_image(&img, PaXType::DXT1, true).expect("creation failed");

    // For 512x16, mipmaps should not stop at 256x4. It should continue:
    // Level 0: 512x16
    // Level 1: 256x8
    // Level 2: 128x4
    // Level 3: 64x4
    // Level 4: 32x4
    // Level 5: 16x4
    // Level 6: 8x4
    // Level 7: 4x4
    assert!(paa.maps.len() >= 8, "Expected at least 8 mipmap levels for 512x16, got {}", paa.maps.len());
    let last = paa.maps.last().unwrap();
    assert_eq!(last.width(PaXType::DXT1), 4);
    assert_eq!(last.height(), 4);
}

#[test]
fn test_uncompressed_non_dxt_roundtrip() {
    let img = create_test_image(16, 16, true);
    let mip = paa_viewer::paa::Mipmap::from_rgba_image(&img.to_rgba8(), PaXType::ARGB8, false).expect("mip creation");
    assert!(!mip.is_compressed(PaXType::ARGB8));

    let decoded = mip.to_rgba_image(PaXType::ARGB8).expect("uncompressed decode failed");
    assert_eq!(decoded.dimensions(), (16, 16));
    assert_eq!(decoded.get_pixel(0, 0), img.to_rgba8().get_pixel(0, 0));
}

#[test]
fn test_cli_resize_nan_inf() {
    assert!(paa_viewer::cli::parse_resize_str("nan").is_err());
    assert!(paa_viewer::cli::parse_resize_str("inf").is_err());
    assert!(paa_viewer::cli::parse_resize_str("-1.0").is_err());
    assert!(paa_viewer::cli::parse_resize_str("0").is_err());
    assert!(paa_viewer::cli::parse_resize_str("0.5").is_ok());
}

#[test]
fn test_next_power_of_two_extreme() {
    let p = next_power_of_two(2_147_483_649);
    assert!(p.is_power_of_two(), "next_power_of_two must always be a power of two");

    let near = nearest_power_of_two(2_147_483_649);
    assert!(near.is_power_of_two(), "nearest_power_of_two must always be a power of two");
}

#[test]
fn test_fitpad_zero_dimensions_no_panic() {
    let zero_img = DynamicImage::new_rgba8(0, 0);
    let res = resize_pot(&zero_img, PotMode::FitPad { target_w: 64, target_h: 64 }, ResizeFilter::Nearest);
    assert_eq!(res.dimensions(), (64, 64));
}

#[test]
fn test_lz77_empty_roundtrip() {
    let mut decomp = Vec::new();
    let res = paa_viewer::paa::lz77::decompress(&[], &mut decomp);
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), 0);
}

#[test]
fn test_clean_path_file_uri_decoding() {
    let uri = std::path::PathBuf::from("file:///home/user/My%20Textures/arma%20texture.paa");
    let clean = paa_viewer::gui::app::clean_path(uri);
    assert_eq!(clean, std::path::PathBuf::from("/home/user/My Textures/arma texture.paa"));
}

#[test]
fn test_appimage_executable_resolution() {
    // If APPIMAGE environment variable is set to a real path, resolve_executable_path should use it
    let temp_appimage = std::env::temp_dir().join("test_fake_appimage.AppImage");
    let _ = std::fs::write(&temp_appimage, b"fake");

    unsafe {
        std::env::set_var("APPIMAGE", &temp_appimage);
    }

    let resolved = paa_viewer::context_menu::ContextMenuManager::resolve_executable_path().unwrap();
    assert_eq!(resolved, temp_appimage);

    unsafe {
        std::env::remove_var("APPIMAGE");
    }
    let _ = std::fs::remove_file(&temp_appimage);
}
