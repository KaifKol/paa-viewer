use std::path::PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::convert::{ConvertOptions, ResizeOperation, TargetFormat};
use crate::paa::PaXType;
use crate::resize::{PotMode, ResizeFilter};

#[derive(Debug, Parser)]
#[command(
    name = "paa-viewer",
    author = "omp",
    version = "0.1.0",
    about = "Bohemia Interactive PAA texture viewer, converter and power-of-two resizer"
)]
pub struct Cli {
    #[arg(help = "Optional image file to open in GUI mode directly", global = false)]
    pub file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Launch the desktop graphical interface (default)")]
    Gui {
        #[arg(help = "Image or PAA file to open")]
        file: Option<PathBuf>,
    },

    #[command(about = "Display detailed technical information and metadata for a PAA file")]
    Info {
        #[arg(help = "Path to PAA file")]
        file: PathBuf,

        #[arg(long, help = "Output metadata formatted as JSON")]
        json: bool,
    },

    #[command(about = "Convert single image between PAA and PNG, JPG, TGA, BMP, WebP with optional resize")]
    Convert {
        #[arg(help = "Source image path")]
        input: PathBuf,

        #[arg(help = "Optional destination path (defaults to switching between .paa and .png)")]
        output: Option<PathBuf>,

        #[arg(long, help = "Target PAA format: dxt1, dxt3, dxt5, argb8888, argb1555, argb4444, graya")]
        format: Option<PaaFormatArg>,

        #[arg(
            long,
            help = "Resize option: multiplier (e.g. '0.5', '2.0'), POT mode ('pot-nearest', 'pot-next', 'pot-prev'), or exact dimensions ('512x512', 'stretch:1024x1024', 'fit:1024x1024')"
        )]
        resize: Option<String>,

        #[arg(long, default_value = "lanczos", help = "Scaling filter: lanczos, triangle, nearest")]
        filter: FilterArg,

        #[arg(long, help = "Disable automatic mipmap generation for PAA")]
        no_mipmaps: bool,
    },

    #[command(about = "Batch convert a folder of images or PAA files")]
    Batch {
        #[arg(help = "Directory containing source images")]
        input_dir: PathBuf,

        #[arg(long, help = "Destination directory")]
        out_dir: PathBuf,

        #[arg(long, help = "Target extension/format: paa, png, jpg, tga, bmp, webp")]
        to: String,

        #[arg(long, help = "Target PAA format (if converting to PAA): dxt1, dxt3, dxt5, argb8888, etc.")]
        format: Option<PaaFormatArg>,

        #[arg(long, help = "Resize option (e.g. '0.5', '2.0', 'pot-nearest', 'pot-next')")]
        resize: Option<String>,

        #[arg(long, default_value = "lanczos", help = "Scaling filter")]
        filter: FilterArg,

        #[arg(long, help = "Disable mipmaps generation")]
        no_mipmaps: bool,

        #[arg(long, help = "Process subdirectories recursively")]
        recursive: bool,
    },
    #[command(about = "Manage system context menu and file association integration")]
    ContextMenu {
        #[command(subcommand)]
        action: ContextMenuAction,
    },
}
#[derive(Debug, Subcommand)]
pub enum ContextMenuAction {
    #[command(about = "Register PAA Viewer in system context menus and file associations")]
    Install,

    #[command(about = "Unregister PAA Viewer from system context menus")]
    Uninstall,

    #[command(about = "Check if PAA Viewer is registered in system context menus")]
    Status,

    #[command(about = "Export Windows registry file (.reg) for manual import")]
    ExportReg {
        #[arg(help = "Output path for .reg file", default_value = "paa-viewer-context-menu.reg")]
        output: PathBuf,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum PaaFormatArg {
    Dxt1,
    Dxt3,
    Dxt5,
    Argb8888,
    Argb1555,
    Argb4444,
    Graya,
}

impl From<PaaFormatArg> for PaXType {
    fn from(arg: PaaFormatArg) -> Self {
        match arg {
            PaaFormatArg::Dxt1 => PaXType::DXT1,
            PaaFormatArg::Dxt3 => PaXType::DXT3,
            PaaFormatArg::Dxt5 => PaXType::DXT5,
            PaaFormatArg::Argb8888 => PaXType::ARGB8,
            PaaFormatArg::Argb1555 => PaXType::ARGB5,
            PaaFormatArg::Argb4444 => PaXType::ARGB4,
            PaaFormatArg::Graya => PaXType::GRAYA,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum FilterArg {
    Lanczos,
    Triangle,
    Nearest,
}

impl From<FilterArg> for ResizeFilter {
    fn from(f: FilterArg) -> Self {
        match f {
            FilterArg::Lanczos => ResizeFilter::Lanczos3,
            FilterArg::Triangle => ResizeFilter::Triangle,
            FilterArg::Nearest => ResizeFilter::Nearest,
        }
    }
}

pub fn parse_resize_str(s: &str) -> Result<ResizeOperation, String> {
    let lower = s.trim().to_ascii_lowercase();

    match lower.as_str() {
        "pot-nearest" | "pot_nearest" | "pot" => Ok(ResizeOperation::Pot(PotMode::Nearest)),
        "pot-next" | "pot_next" => Ok(ResizeOperation::Pot(PotMode::Next)),
        "pot-prev" | "pot_prev" => Ok(ResizeOperation::Pot(PotMode::Prev)),
        _ => {
            if let Ok(factor) = lower.parse::<f32>() {
                if !factor.is_finite() || factor <= 0.0 {
                    return Err("Scale factor must be a finite number greater than zero".into());
                }
                return Ok(ResizeOperation::Factor(factor));
            }

            if let Some(dim_str) = lower.strip_prefix("stretch:") {
                let (w, h) = parse_dims(dim_str)?;
                return Ok(ResizeOperation::Pot(PotMode::Stretch { target_w: w, target_h: h }));
            }

            if let Some(dim_str) = lower.strip_prefix("fit:") {
                let (w, h) = parse_dims(dim_str)?;
                return Ok(ResizeOperation::Pot(PotMode::FitPad { target_w: w, target_h: h }));
            }

            if lower.contains('x') {
                let (w, h) = parse_dims(&lower)?;
                return Ok(ResizeOperation::Pot(PotMode::Stretch { target_w: w, target_h: h }));
            }

            Err(format!(
                "Unrecognized resize option '{s}'. Use '0.5', '2.0', 'pot-nearest', 'pot-next', 'pot-prev', or '512x512'"
            ))
        }
    }
}

fn parse_dims(s: &str) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return Err(format!("Dimensions must be formatted as WIDTHxHEIGHT (got '{s}')"));
    }
    let w = parts[0]
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("Invalid width '{}'", parts[0]))?;
    let h = parts[1]
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("Invalid height '{}'", parts[1]))?;

    if w == 0 || h == 0 {
        return Err("Dimensions must be greater than zero".into());
    }
    Ok((w, h))
}

pub fn build_convert_options(
    target_format: Option<PaaFormatArg>,
    resize_str: Option<&str>,
    filter: FilterArg,
    no_mipmaps: bool,
) -> Result<ConvertOptions, String> {
    let resize_op = if let Some(r) = resize_str {
        parse_resize_str(r)?
    } else {
        ResizeOperation::None
    };

    let target = match target_format {
        Some(f) => TargetFormat::Paa(Some(f.into())),
        None => TargetFormat::Auto,
    };

    Ok(ConvertOptions {
        target,
        generate_mipmaps: !no_mipmaps,
        resize: resize_op,
        filter: filter.into(),
    })
}
