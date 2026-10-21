use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};

use crate::paa::{PaaImage, PaaMetadata, PaXType};
use crate::resize::{resize_pot, scale_by_factor, PotMode, ResizeFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetFormat {
    Auto,
    Paa(Option<PaXType>),
    Png,
    Jpeg { quality: u8 },
    Tga,
    Bmp,
    WebP,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResizeOperation {
    None,
    Factor(f32),
    Step(i32),
    Pot(PotMode),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertOptions {
    pub target: TargetFormat,
    pub generate_mipmaps: bool,
    pub resize: ResizeOperation,
    pub filter: ResizeFilter,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            target: TargetFormat::Auto,
            generate_mipmaps: true,
            resize: ResizeOperation::None,
            filter: ResizeFilter::Lanczos3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertReport {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub original_dimensions: (u32, u32),
    pub final_dimensions: (u32, u32),
    pub input_size_bytes: u64,
    pub output_size_bytes: u64,
    pub target_format_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchReport {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub reports: Vec<ConvertReport>,
    pub errors: Vec<(PathBuf, String)>,
}

pub fn load_image(path: &Path) -> Result<(DynamicImage, Option<PaaMetadata>), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "paa" || ext == "pac" {
        let file = File::open(path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
        let mut reader = BufReader::new(file);
        let paa = PaaImage::read(&mut reader).map_err(|e| format!("Failed to parse PAA {}: {e}", path.display()))?;
        let meta = paa.metadata();
        let img = paa.to_dynamic_image(0).map_err(|e| format!("Failed to decode PAA mipmap: {e}"))?;
        Ok((img, Some(meta)))
    } else {
        let img = image::open(path).map_err(|e| format!("Failed to load image {}: {e}", path.display()))?;
        Ok((img, None))
    }
}

pub fn convert_image(
    input_path: &Path,
    output_path: &Path,
    options: &ConvertOptions,
) -> Result<ConvertReport, String> {
    let (mut img, _) = load_image(input_path)?;
    let orig_dims = (img.width(), img.height());

    let input_size = std::fs::metadata(input_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Apply resize operation if requested
    img = match options.resize {
        ResizeOperation::None => img,
        ResizeOperation::Factor(factor) => scale_by_factor(&img, factor, options.filter),
        ResizeOperation::Step(step) => crate::resize::scale_step(&img, step, options.filter),
        ResizeOperation::Pot(mode) => resize_pot(&img, mode, options.filter),
    };

    let final_dims = (img.width(), img.height());

    // Determine target format
    let ext = output_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let target = match options.target {
        TargetFormat::Auto => match ext.as_str() {
            "paa" | "pac" => TargetFormat::Paa(None),
            "png" => TargetFormat::Png,
            "jpg" | "jpeg" => TargetFormat::Jpeg { quality: 90 },
            "tga" => TargetFormat::Tga,
            "bmp" => TargetFormat::Bmp,
            "webp" => TargetFormat::WebP,
            _ => return Err(format!("Unknown or unsupported output extension for: {}", output_path.display())),
        },
        TargetFormat::Paa(pax) => {
            if ext != "paa" && ext != "pac" && !ext.is_empty() {
                match ext.as_str() {
                    "png" => TargetFormat::Png,
                    "jpg" | "jpeg" => TargetFormat::Jpeg { quality: 90 },
                    "tga" => TargetFormat::Tga,
                    "bmp" => TargetFormat::Bmp,
                    "webp" => TargetFormat::WebP,
                    _ => TargetFormat::Paa(pax),
                }
            } else {
                TargetFormat::Paa(pax)
            }
        }
        other => other,
    };

    let target_name = match target {
        TargetFormat::Paa(opt_pax) => {
            let pax = opt_pax.unwrap_or_else(|| {
                // Auto-detect best PAA format: DXT1 if opaque, DXT5 if alpha
                let has_alpha = img.to_rgba8().pixels().any(|p| p.0[3] < 255);
                if has_alpha {
                    PaXType::DXT5
                } else {
                    PaXType::DXT1
                }
            });

            let paa = PaaImage::from_dynamic_image(&img, pax, options.generate_mipmaps)
                .map_err(|e| format!("Failed to encode PAA: {e}"))?;

            let file = File::create(output_path)
                .map_err(|e| format!("Failed to create {}: {e}", output_path.display()))?;
            let mut writer = BufWriter::new(file);
            paa.write(&mut writer)
                .map_err(|e| format!("Failed to write PAA: {e}"))?;

            format!("PAA ({pax})")
        }
        TargetFormat::Png => {
            img.save_with_format(output_path, ImageFormat::Png)
                .map_err(|e| format!("Failed to save PNG: {e}"))?;
            "PNG".to_string()
        }
        TargetFormat::Jpeg { quality } => {
            let rgb = img.to_rgb8();
            let file = File::create(output_path)
                .map_err(|e| format!("Failed to create {}: {e}", output_path.display()))?;
            let mut writer = BufWriter::new(file);
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
            encoder
                .encode(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(|e| format!("Failed to save JPEG: {e}"))?;
            format!("JPEG (quality {quality})")
        }
        TargetFormat::Tga => {
            img.save_with_format(output_path, ImageFormat::Tga)
                .map_err(|e| format!("Failed to save TGA: {e}"))?;
            "TGA".to_string()
        }
        TargetFormat::Bmp => {
            img.save_with_format(output_path, ImageFormat::Bmp)
                .map_err(|e| format!("Failed to save BMP: {e}"))?;
            "BMP".to_string()
        }
        TargetFormat::WebP => {
            img.save_with_format(output_path, ImageFormat::WebP)
                .map_err(|e| format!("Failed to save WebP: {e}"))?;
            "WebP".to_string()
        }
        TargetFormat::Auto => unreachable!(),
    };

    let output_size = std::fs::metadata(output_path)
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(ConvertReport {
        input_path: input_path.to_path_buf(),
        output_path: output_path.to_path_buf(),
        original_dimensions: orig_dims,
        final_dimensions: final_dims,
        input_size_bytes: input_size,
        output_size_bytes: output_size,
        target_format_name: target_name,
    })
}

pub fn batch_convert(
    input_paths: &[PathBuf],
    output_dir: &Path,
    options: &ConvertOptions,
    target_extension: &str,
) -> BatchReport {
    let mut reports = Vec::new();
    let mut errors = Vec::new();

    std::fs::create_dir_all(output_dir).ok();

    let mut used_names = std::collections::HashSet::new();

    for path in input_paths {
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("image");

        let mut out_file_name = format!("{file_stem}.{target_extension}");
        let mut out_path = output_dir.join(&out_file_name);
        let mut counter = 1;
        while used_names.contains(&out_path) {
            out_file_name = format!("{file_stem}_{counter}.{target_extension}");
            out_path = output_dir.join(&out_file_name);
            counter += 1;
        }
        used_names.insert(out_path.clone());
        match convert_image(path, &out_path, options) {
            Ok(rep) => reports.push(rep),
            Err(err) => errors.push((path.clone(), err)),
        }
    }

    let succ = reports.len();
    let fail = errors.len();

    BatchReport {
        total: input_paths.len(),
        successful: succ,
        failed: fail,
        reports,
        errors,
    }
}
