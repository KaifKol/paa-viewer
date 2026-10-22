use std::io::{Read, Seek, SeekFrom, Write};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use image::{DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};

use super::error::PaaError;
use super::mipmap::{Mipmap, MipmapInfo};
use super::pax::PaXType;
use super::taggs::{TagInfo, Taggs};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaaMetadata {
    pub format: PaXType,
    pub format_name: String,
    pub width: u32,
    pub height: u32,
    pub is_power_of_two: bool,
    pub aspect_ratio: String,
    pub aspect_ratio_num: f32,
    pub mipmap_count: usize,
    pub mipmaps: Vec<MipmapInfo>,
    pub average_color: Option<[u8; 4]>,
    pub max_color: Option<[u8; 4]>,
    pub has_alpha: bool,
    pub total_data_size: usize,
    pub tags: Vec<TagInfo>,
}

#[derive(Debug, Clone)]
pub struct PaaImage {
    pub format: PaXType,
    pub taggs: Taggs,
    pub maps: Vec<Mipmap>,
}

impl PaaImage {
    pub fn new(format: PaXType) -> Self {
        Self {
            format,
            taggs: Taggs::new(),
            maps: Vec::new(),
        }
    }

    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, PaaError> {
        let format = PaXType::from_stream(reader)?;
        let taggs = Taggs::read(reader)?;
        let mut paa = Self {
            format,
            taggs,
            maps: Vec::new(),
        };

        if let Some(offsets) = paa.taggs.get_offsets() {
            for &offset in &offsets {
                if offset != 0 {
                    reader.seek(SeekFrom::Start(u64::from(offset)))?;
                    let mip = Mipmap::from_stream(paa.format, reader)?;
                    paa.maps.push(mip);
                }
            }
        }

        // Fallback: If no offsets found in SFFO tag, try reading sequential mipmaps
        if paa.maps.is_empty() {
            // Read past the 2-byte palette index if present
            let _ = reader.read_u16::<LittleEndian>();
            while let Ok(mip) = Mipmap::from_stream(paa.format, reader) {
                let is_end = mip.width(paa.format) <= 1 && mip.height() <= 1;
                paa.maps.push(mip);
                if is_end {
                    break;
                }
            }
        }

        if paa.maps.is_empty() {
            return Err(PaaError::CorruptedData("No mipmaps found in PAA file".into()));
        }

        Ok(paa)
    }

    pub fn write<W: Write + Seek>(&self, writer: &mut W) -> Result<(), PaaError> {
        writer.write_all(&self.format.as_bytes())?;

        // Write all tags except SFFO (offset table will be calculated and written)
        for (name, data) in &self.taggs.entries {
            if name == "SFFO" || name == "OFFS" {
                continue;
            }
            writer.write_all(super::taggs::SIGNATURE_TAGG)?;
            let name_bytes = name.as_bytes();
            if name_bytes.len() != 4 {
                return Err(PaaError::EncodingError(format!("Invalid tag name length: {name}")));
            }
            writer.write_all(name_bytes)?;
            writer.write_u32::<LittleEndian>(data.len() as u32)?;
            writer.write_all(data)?;
        }

        // Calculate offset where SFFO begins
        // Format (2) + Tags (each is 4 + 4 + 4 + len) + SFFO header (4 + 4 + 4 + 64) + palette (2)
        let offset_count = self.maps.len().max(16);
        let sffo_tag_len = offset_count * 4;
        writer.write_all(super::taggs::SIGNATURE_TAGG)?;
        writer.write_all(b"SFFO")?;
        writer.write_u32::<LittleEndian>(sffo_tag_len as u32)?;

        let sffo_offsets_pos = writer.stream_position()?;
        let mut calculated_offsets = Vec::with_capacity(offset_count);

        let mut next_offset = sffo_offsets_pos + (sffo_tag_len as u64) + 2;

        for mip in &self.maps {
            calculated_offsets.push(next_offset as u32);
            // Each mipmap in stream has 2 bytes width + 2 bytes height + 3 bytes length + data
            next_offset += 2 + 2 + 3 + (mip.data.len() as u64);
        }

        while calculated_offsets.len() < offset_count {
            calculated_offsets.push(0);
        }

        for &offset in &calculated_offsets {
            writer.write_u32::<LittleEndian>(offset)?;
        }

        // Palette index (0 for non-indexed)
        writer.write_u16::<LittleEndian>(0)?;

        // Write actual mipmaps
        for mip in &self.maps {
            mip.write(writer)?;
        }

        // Trailing bytes (end of file marker: 7 bytes of zeros: 2B width, 2B height, 3B size)
        writer.write_all(&[0u8; 7])?;

        Ok(())
    }

    pub fn from_dynamic_image(
        image: &DynamicImage,
        format: PaXType,
        generate_mipmaps: bool,
    ) -> Result<Self, PaaError> {
        let (width, height) = image.dimensions();
        if width == 0 || height == 0 {
            return Err(PaaError::InvalidDimensions(width, height));
        }

        let rgba_image = image.to_rgba8();

        // Calculate Average Color
        let (sum_r, sum_g, sum_b, sum_a) = rgba_image.pixels().fold(
            (0u64, 0u64, 0u64, 0u64),
            |(acc_r, acc_g, acc_b, acc_a), p| {
                (
                    acc_r + u64::from(p.0[0]),
                    acc_g + u64::from(p.0[1]),
                    acc_b + u64::from(p.0[2]),
                    acc_a + u64::from(p.0[3]),
                )
            },
        );

        let total_pixels = (width as u64) * (height as u64);
        let avg_color = [
            (sum_r / total_pixels) as u8,
            (sum_g / total_pixels) as u8,
            (sum_b / total_pixels) as u8,
            (sum_a / total_pixels) as u8,
        ];

        // Calculate Max Color & Transparency
        let mut max_color = [0u8; 4];
        let mut has_transparency = false;
        for p in rgba_image.pixels() {
            max_color[0] = max_color[0].max(p.0[0]);
            max_color[1] = max_color[1].max(p.0[1]);
            max_color[2] = max_color[2].max(p.0[2]);
            max_color[3] = max_color[3].max(p.0[3]);
            if p.0[3] < 255 {
                has_transparency = true;
            }
        }

        let mut paa = Self::new(format);

        // Insert tags
        paa.taggs.entries.insert("CGVA".to_string(), vec![avg_color[2], avg_color[1], avg_color[0], avg_color[3]]);
        paa.taggs.entries.insert("CXAM".to_string(), vec![255, 255, 255, 255]); // Arma standard
        if has_transparency {
            paa.taggs.entries.insert("GALF".to_string(), vec![1, 0, 0, 0]);
        }

        // Mipmaps generation
        let mut current_img = rgba_image;
        let mut current_w = width;
        let mut current_h = height;

        let base_mip = Mipmap::from_rgba_image(&current_img, format, true)?;
        paa.maps.push(base_mip);

        if generate_mipmaps {
            // DXT blocks are at least 4x4, but a narrow source dimension must
            // never grow while generating the mipmap chain.
            let min_w = if format.is_dxt() { width.min(4) } else { 1 };
            let min_h = if format.is_dxt() { height.min(4) } else { 1 };
            while (current_w > min_w || current_h > min_h) && paa.maps.len() < 16 {
                current_w = (current_w / 2).max(min_w);
                current_h = (current_h / 2).max(min_h);
                current_img = image::imageops::resize(
                    &current_img,
                    current_w,
                    current_h,
                    image::imageops::FilterType::Lanczos3,
                );

                let mip = Mipmap::from_rgba_image(&current_img, format, true)?;
                paa.maps.push(mip);
            }
        }

        Ok(paa)
    }

    pub fn to_dynamic_image(&self, level: usize) -> Result<DynamicImage, PaaError> {
        let mip = self.maps.get(level).ok_or_else(|| {
            PaaError::CorruptedData(format!("Requested mipmap level {level} out of bounds (count: {})", self.maps.len()))
        })?;
        mip.to_dynamic_image(self.format)
    }

    pub fn metadata(&self) -> PaaMetadata {
        let (w, h) = if let Some(first) = self.maps.first() {
            (first.width(self.format), first.height())
        } else {
            (0, 0)
        };

        let is_pot = is_power_of_two(w) && is_power_of_two(h);
        let ratio_num = if h > 0 { w as f32 / h as f32 } else { 1.0 };
        let ratio_str = calculate_aspect_ratio_str(w, h);

        let mip_infos = self
            .maps
            .iter()
            .enumerate()
            .map(|(i, m)| m.info(i, self.format))
            .collect();

        let total_size = self.maps.iter().map(|m| m.data.len()).sum();

        PaaMetadata {
            format: self.format,
            format_name: self.format.to_string(),
            width: w,
            height: h,
            is_power_of_two: is_pot,
            aspect_ratio: ratio_str,
            aspect_ratio_num: ratio_num,
            mipmap_count: self.maps.len(),
            mipmaps: mip_infos,
            average_color: self.taggs.get_average_color(),
            max_color: self.taggs.get_max_color(),
            has_alpha: self.taggs.has_alpha(),
            total_data_size: total_size,
            tags: self.taggs.get_tag_infos(),
        }
    }
}

pub fn is_power_of_two(n: u32) -> bool {
    n > 0 && (n & (n - 1)) == 0
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

pub fn calculate_aspect_ratio_str(width: u32, height: u32) -> String {
    if width == 0 || height == 0 {
        return "0:0".to_string();
    }
    let divisor = gcd(width, height);
    let rx = width / divisor;
    let ry = height / divisor;
    if rx <= 64 && ry <= 64 {
        format!("{rx}:{ry}")
    } else {
        format!("{:.2}:1", width as f32 / height as f32)
    }
}
