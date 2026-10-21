use std::fmt;
use std::io::Read;
use serde::{Deserialize, Serialize};
use texpresso::{COLOUR_WEIGHTS_PERCEPTUAL, Format, Params};

use super::error::PaaError;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaXType {
    DXT1,
    DXT2,
    DXT3,
    DXT4,
    DXT5,
    ARGB4,
    ARGB5,
    ARGB8,
    GRAYA,
}

impl PaXType {
    pub fn from_stream<R: Read>(stream: &mut R) -> Result<Self, PaaError> {
        let mut bytes = [0u8; 2];
        stream.read_exact(&mut bytes)?;
        Self::from_bytes(bytes).ok_or(PaaError::InvalidMagic(bytes))
    }

    pub const fn from_bytes(bytes: [u8; 2]) -> Option<Self> {
        match bytes {
            [1, 255] => Some(Self::DXT1),    // 0x01FF
            [2, 255] => Some(Self::DXT2),    // 0x02FF
            [3, 255] => Some(Self::DXT3),    // 0x03FF
            [4, 255] => Some(Self::DXT4),    // 0x04FF
            [5, 255] => Some(Self::DXT5),    // 0x05FF
            [68, 68] => Some(Self::ARGB4),   // 0x4444 (ARGB4444)
            [85, 21] => Some(Self::ARGB5),   // 0x1555 (ARGB1555)
            [136, 136] => Some(Self::ARGB8), // 0x8888 (ARGB8888)
            [128, 128] => Some(Self::GRAYA), // 0x8080 (GRAYA)
            _ => None,
        }
    }

    pub const fn as_bytes(&self) -> [u8; 2] {
        match self {
            Self::DXT1 => [1, 255],
            Self::DXT2 => [2, 255],
            Self::DXT3 => [3, 255],
            Self::DXT4 => [4, 255],
            Self::DXT5 => [5, 255],
            Self::ARGB4 => [68, 68],
            Self::ARGB5 => [85, 21],
            Self::ARGB8 => [136, 136],
            Self::GRAYA => [128, 128],
        }
    }

    pub const fn is_dxt(&self) -> bool {
        matches!(
            self,
            Self::DXT1 | Self::DXT2 | Self::DXT3 | Self::DXT4 | Self::DXT5
        )
    }

    pub fn image_size(&self, width: usize, height: usize) -> usize {
        match *self {
            Self::DXT1 => {
                let format = Format::Bc1;
                format.compressed_size(width, height)
            }
            Self::DXT2 | Self::DXT3 => {
                let format = Format::Bc2;
                format.compressed_size(width, height)
            }
            Self::DXT4 | Self::DXT5 => {
                let format = Format::Bc3;
                format.compressed_size(width, height)
            }
            Self::ARGB5 | Self::ARGB4 => width * height * 2,
            Self::ARGB8 => width * height * 4,
            Self::GRAYA => width * height * 2,
        }
    }

    pub fn compress(&self, rgba_data: &[u8], width: usize, height: usize, output: &mut [u8]) -> Result<(), PaaError> {
        match *self {
            Self::DXT1 => {
                Format::Bc1.compress(
                    rgba_data,
                    width,
                    height,
                    Params {
                        algorithm: texpresso::Algorithm::IterativeClusterFit,
                        weights: COLOUR_WEIGHTS_PERCEPTUAL,
                        weigh_colour_by_alpha: true,
                    },
                    output,
                );
                Ok(())
            }
            Self::DXT3 => {
                Format::Bc2.compress(
                    rgba_data,
                    width,
                    height,
                    Params {
                        algorithm: texpresso::Algorithm::IterativeClusterFit,
                        weights: COLOUR_WEIGHTS_PERCEPTUAL,
                        weigh_colour_by_alpha: true,
                    },
                    output,
                );
                Ok(())
            }
            Self::DXT5 => {
                Format::Bc3.compress(
                    rgba_data,
                    width,
                    height,
                    Params {
                        algorithm: texpresso::Algorithm::IterativeClusterFit,
                        weights: COLOUR_WEIGHTS_PERCEPTUAL,
                        weigh_colour_by_alpha: true,
                    },
                    output,
                );
                Ok(())
            }
            Self::DXT2 | Self::DXT4 => {
                Err(PaaError::UnsupportedFormat(format!("{self:?} compression not supported")))
            }
            Self::ARGB5 => {
                let max_pixels = (width * height).min((rgba_data.len() / 4).min(output.len() / 2));
                for i in 0..max_pixels {
                    let offset = i * 2;
                    let r = u16::from(rgba_data[i * 4] >> 3);
                    let g = u16::from(rgba_data[i * 4 + 1] >> 3);
                    let b = u16::from(rgba_data[i * 4 + 2] >> 3);
                    let a = u16::from(rgba_data[i * 4 + 3] >= 128);
                    let pixel = (a << 15) | (r << 10) | (g << 5) | b;
                    let bytes = pixel.to_le_bytes();
                    output[offset] = bytes[0];
                    output[offset + 1] = bytes[1];
                }
                Ok(())
            }
            Self::ARGB4 => {
                let max_pixels = (width * height).min((rgba_data.len() / 4).min(output.len() / 2));
                for i in 0..max_pixels {
                    let offset = i * 2;
                    let r = u16::from(rgba_data[i * 4] >> 4);
                    let g = u16::from(rgba_data[i * 4 + 1] >> 4);
                    let b = u16::from(rgba_data[i * 4 + 2] >> 4);
                    let a = u16::from(rgba_data[i * 4 + 3] >> 4);
                    let pixel = (a << 12) | (r << 8) | (g << 4) | b;
                    let bytes = pixel.to_le_bytes();
                    output[offset] = bytes[0];
                    output[offset + 1] = bytes[1];
                }
                Ok(())
            }
            Self::ARGB8 => {
                let max_pixels = (width * height).min((rgba_data.len() / 4).min(output.len() / 4));
                for i in 0..max_pixels {
                    let offset = i * 4;
                    output[offset] = rgba_data[i * 4 + 2];     // B
                    output[offset + 1] = rgba_data[i * 4 + 1]; // G
                    output[offset + 2] = rgba_data[i * 4];     // R
                    output[offset + 3] = rgba_data[i * 4 + 3]; // A
                }
                Ok(())
            }
            Self::GRAYA => {
                let max_pixels = (width * height).min((rgba_data.len() / 4).min(output.len() / 2));
                for i in 0..max_pixels {
                    let offset = i * 2;
                    let r = f32::from(rgba_data[i * 4]);
                    let g = f32::from(rgba_data[i * 4 + 1]);
                    let b = f32::from(rgba_data[i * 4 + 2]);
                    let gray = (0.2126 * r + 0.7152 * g + 0.0722 * b).round() as u8;
                    let a = rgba_data[i * 4 + 3];
                    output[offset] = gray;
                    output[offset + 1] = a;
                }
                Ok(())
            }
        }
    }

    pub fn decompress(&self, data: &[u8], width: usize, height: usize, output: &mut [u8]) -> Result<(), PaaError> {
        match *self {
            Self::DXT1 => {
                Format::Bc1.decompress(data, width, height, output);
                Ok(())
            }
            Self::DXT3 => {
                Format::Bc2.decompress(data, width, height, output);
                Ok(())
            }
            Self::DXT5 => {
                Format::Bc3.decompress(data, width, height, output);
                Ok(())
            }
            Self::DXT2 | Self::DXT4 => {
                Err(PaaError::UnsupportedFormat(format!("{self:?} decompression not supported")))
            }
            Self::ARGB5 => {
                let pixel_count = width * height;
                for i in 0..pixel_count {
                    let offset = i * 2;
                    if offset + 1 < data.len() && i * 4 + 3 < output.len() {
                        let pixel = u16::from_le_bytes([data[offset], data[offset + 1]]);
                        let r = ((pixel >> 10) & 0x1F) as u8;
                        let g = ((pixel >> 5) & 0x1F) as u8;
                        let b = (pixel & 0x1F) as u8;
                        let a = if (pixel & 0x8000) != 0 { 255 } else { 0 };

                        output[i * 4] = (r << 3) | (r >> 2);
                        output[i * 4 + 1] = (g << 3) | (g >> 2);
                        output[i * 4 + 2] = (b << 3) | (b >> 2);
                        output[i * 4 + 3] = a;
                    }
                }
                Ok(())
            }
            Self::ARGB4 => {
                let pixel_count = width * height;
                for i in 0..pixel_count {
                    let offset = i * 2;
                    if offset + 1 < data.len() && i * 4 + 3 < output.len() {
                        let pixel = u16::from_le_bytes([data[offset], data[offset + 1]]);
                        let r = ((pixel >> 8) & 0x0F) as u8;
                        let g = ((pixel >> 4) & 0x0F) as u8;
                        let b = (pixel & 0x0F) as u8;
                        let a = ((pixel >> 12) & 0x0F) as u8;

                        output[i * 4] = (r << 4) | r;
                        output[i * 4 + 1] = (g << 4) | g;
                        output[i * 4 + 2] = (b << 4) | b;
                        output[i * 4 + 3] = (a << 4) | a;
                    }
                }
                Ok(())
            }
            Self::ARGB8 => {
                let pixel_count = width * height;
                for i in 0..pixel_count {
                    let offset = i * 4;
                    if offset + 3 < data.len() && i * 4 + 3 < output.len() {
                        output[i * 4] = data[offset + 2];     // R
                        output[i * 4 + 1] = data[offset + 1]; // G
                        output[i * 4 + 2] = data[offset];     // B
                        output[i * 4 + 3] = data[offset + 3]; // A
                    }
                }
                Ok(())
            }
            Self::GRAYA => {
                let pixel_count = width * height;
                for i in 0..pixel_count {
                    let offset = i * 2;
                    if offset + 1 < data.len() && i * 4 + 3 < output.len() {
                        let gray = data[offset];
                        let alpha = data[offset + 1];
                        output[i * 4] = gray;
                        output[i * 4 + 1] = gray;
                        output[i * 4 + 2] = gray;
                        output[i * 4 + 3] = alpha;
                    }
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for PaXType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DXT1 => write!(f, "DXT1"),
            Self::DXT2 => write!(f, "DXT2"),
            Self::DXT3 => write!(f, "DXT3"),
            Self::DXT4 => write!(f, "DXT4"),
            Self::DXT5 => write!(f, "DXT5"),
            Self::ARGB4 => write!(f, "ARGB4444"),
            Self::ARGB5 => write!(f, "ARGB1555"),
            Self::ARGB8 => write!(f, "ARGB8888"),
            Self::GRAYA => write!(f, "GRAYA"),
        }
    }
}
