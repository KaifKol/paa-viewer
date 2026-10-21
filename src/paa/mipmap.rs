use std::io::{Read, Seek, Write};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use image::{DynamicImage, EncodableLayout, RgbaImage};
use serde::{Deserialize, Serialize};

use super::error::PaaError;
use super::lzo;
use super::lz77;
use super::pax::PaXType;

pub const COMPRESS_THRESHOLD: u16 = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MipmapInfo {
    pub level: usize,
    pub width: u32,
    pub height: u32,
    pub is_compressed: bool,
    pub data_size: usize,
    pub uncompressed_size: usize,
    pub compression_ratio: f32,
    pub file_offset: u64,
}

#[derive(Debug, Clone)]
pub struct Mipmap {
    pub raw_width: u16,
    pub height: u16,
    pub data: Vec<u8>,
    pub file_offset: u64,
}

impl Mipmap {
    pub fn new(raw_width: u16, height: u16, data: Vec<u8>, file_offset: u64) -> Self {
        Self {
            raw_width,
            height,
            data,
            file_offset,
        }
    }

    pub fn width(&self, format: PaXType) -> u32 {
        if format.is_dxt() && (self.raw_width & 0x8000) != 0 {
            u32::from(self.raw_width & 0x7FFF)
        } else {
            u32::from(self.raw_width)
        }
    }

    pub fn height(&self) -> u32 {
        u32::from(self.height)
    }

    pub fn is_compressed(&self, format: PaXType) -> bool {
        if format.is_dxt() {
            (self.raw_width & 0x8000) != 0
        } else {
            self.data.len() != self.uncompressed_size(format)
        }
    }

    pub fn uncompressed_size(&self, format: PaXType) -> usize {
        let w = self.width(format) as usize;
        let h = self.height as usize;
        format.image_size(w, h)
    }

    pub fn info(&self, level: usize, format: PaXType) -> MipmapInfo {
        let uncomp = self.uncompressed_size(format);
        let comp = self.data.len();
        let ratio = if uncomp > 0 {
            comp as f32 / uncomp as f32
        } else {
            1.0
        };

        MipmapInfo {
            level,
            width: self.width(format),
            height: self.height(),
            is_compressed: self.is_compressed(format),
            data_size: comp,
            uncompressed_size: uncomp,
            compression_ratio: ratio,
            file_offset: self.file_offset,
        }
    }

    pub fn from_stream<R: Read + Seek>(
        format: PaXType,
        reader: &mut R,
    ) -> Result<Self, PaaError> {
        let offset = reader.stream_position().unwrap_or(0);
        let raw_width = reader.read_u16::<LittleEndian>()?;
        let height = reader.read_u16::<LittleEndian>()?;
        let length = reader.read_u24::<LittleEndian>()? as usize;

        let mut data = vec![0u8; length];
        reader.read_exact(&mut data)?;

        let mip = Self {
            raw_width,
            height,
            data,
            file_offset: offset,
        };

        let w = mip.width(format);
        let h = mip.height();
        if w == 0 || h == 0 {
            return Err(PaaError::InvalidDimensions(w, h));
        }

        Ok(mip)
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), PaaError> {
        writer.write_u16::<LittleEndian>(self.raw_width)?;
        writer.write_u16::<LittleEndian>(self.height)?;
        if self.data.len() > 0x00FF_FFFF {
            return Err(PaaError::EncodingError("Mipmap data exceeds 24-bit limit".into()));
        }
        writer.write_u24::<LittleEndian>(self.data.len() as u32)?;
        writer.write_all(&self.data)?;
        Ok(())
    }

    pub fn from_rgba_image(
        image: &RgbaImage,
        format: PaXType,
        should_compress: bool,
    ) -> Result<Self, PaaError> {
        let (width, height) = image.dimensions();
        if width > u16::MAX as u32 || height > u16::MAX as u32 {
            return Err(PaaError::InvalidDimensions(width, height));
        }

        let uncompressed_size = format.image_size(width as usize, height as usize);
        let mut raw_encoded = vec![0u8; uncompressed_size];
        format.compress(image.as_bytes(), width as usize, height as usize, &mut raw_encoded)?;

        let dxt_lzo = format.is_dxt()
            && should_compress
            && (width >= u32::from(COMPRESS_THRESHOLD) || height >= u32::from(COMPRESS_THRESHOLD));

        let stored_width = (width as u16) | if dxt_lzo { 0x8000 } else { 0 };

        let data = if dxt_lzo {
            let mut lzo_out = Vec::new();
            lzo::compress(&raw_encoded, &mut lzo_out)?;
            lzo_out
        } else if !format.is_dxt() && should_compress {
            let mut lz77_out = vec![0u8; raw_encoded.len() * 2 + 64];
            let compressed_len = lz77::compress(&raw_encoded, &mut lz77_out)?;
            lz77_out.truncate(compressed_len);
            lz77_out
        } else {
            raw_encoded
        };

        Ok(Self {
            raw_width: stored_width,
            height: height as u16,
            data,
            file_offset: 0,
        })
    }

    pub fn to_rgba_image(&self, format: PaXType) -> Result<RgbaImage, PaaError> {
        let actual_width = self.width(format) as usize;
        let actual_height = self.height as usize;
        let expected_size = format.image_size(actual_width, actual_height);

        let mut pixel_data = vec![0u8; expected_size];

        if format.is_dxt() {
            if (self.raw_width & 0x8000) != 0 {
                lzo::decompress_to_slice(&self.data, &mut pixel_data)?;
            } else {
                if self.data.len() < expected_size {
                    return Err(PaaError::CorruptedData(format!(
                        "Mipmap data truncated: got {} bytes, expected {expected_size}",
                        self.data.len()
                    )));
                }
                pixel_data.copy_from_slice(&self.data[..expected_size]);
            }
        } else if self.data.len() == expected_size {
            pixel_data.copy_from_slice(&self.data);
        } else {
            lz77::decompress(&self.data, &mut pixel_data)?;
        }

        let buffer_len = actual_width
            .checked_mul(actual_height)
            .and_then(|p| p.checked_mul(4))
            .ok_or(PaaError::InvalidDimensions(actual_width as u32, actual_height as u32))?;
        let mut rgba_buffer = vec![0u8; buffer_len];
        format.decompress(&pixel_data, actual_width, actual_height, &mut rgba_buffer)?;
        RgbaImage::from_raw(actual_width as u32, actual_height as u32, rgba_buffer)
            .ok_or_else(|| PaaError::CorruptedData("Failed to construct RGBA image from buffer".into()))
    }

    pub fn to_dynamic_image(&self, format: PaXType) -> Result<DynamicImage, PaaError> {
        let rgba = self.to_rgba_image(format)?;
        Ok(DynamicImage::ImageRgba8(rgba))
    }
}
