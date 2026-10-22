use std::io::{Read, Write};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::error::PaaError;

pub const SIGNATURE_TAGG: &[u8; 4] = b"GGAT"; // "TAGG" in little-endian

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    pub raw_name: String,
    pub display_name: String,
    pub description: String,
    pub length: usize,
    pub data_hex: String,
    pub color_preview: Option<[u8; 4]>,
}

#[derive(Debug, Clone, Default)]
pub struct Taggs {
    pub entries: IndexMap<String, Vec<u8>>,
}

impl Taggs {
    pub fn new() -> Self {
        Self {
            entries: IndexMap::new(),
        }
    }

    pub fn read<R: Read + std::io::Seek>(reader: &mut R) -> Result<Self, PaaError> {
        let mut taggs = Self::new();
        loop {
            let mut sig = [0u8; 4];
            let n = match reader.read_exact(&mut sig) {
                Ok(()) => 4,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => 0,
                Err(e) => return Err(PaaError::Io(e)),
            };

            if n < 4 {
                break;
            }
            if &sig != SIGNATURE_TAGG {
                // Reached end of tags; seek back the 4 bytes read so palette/mipmap read works
                reader.seek(std::io::SeekFrom::Current(-4))?;
                break;
            }
            let mut name_bytes = [0u8; 4];
            reader.read_exact(&mut name_bytes)?;
            let name = std::str::from_utf8(&name_bytes)
                .map_err(|_| PaaError::InvalidTagName)?
                .to_string();

            let length = reader.read_u32::<LittleEndian>()? as usize;
            if length > 16 * 1024 * 1024 {
                return Err(PaaError::CorruptedData(format!("Tag length exceeds 16MB limit: {length} bytes")));
            }
            let mut data = vec![0u8; length];
            reader.read_exact(&mut data)?;

            taggs.entries.insert(name, data);
        }
        Ok(taggs)
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), PaaError> {
        for (name, data) in &self.entries {
            writer.write_all(SIGNATURE_TAGG)?;
            let name_bytes = name.as_bytes();
            if name_bytes.len() != 4 {
                return Err(PaaError::EncodingError(format!("Invalid tag name length: {name}")));
            }
            writer.write_all(name_bytes)?;
            writer.write_u32::<LittleEndian>(data.len() as u32)?;
            writer.write_all(data)?;
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Vec<u8>> {
        self.entries.get(name)
    }

    pub fn get_average_color(&self) -> Option<[u8; 4]> {
        // Tag "CGVA" or "AVGC"
        let data = self.entries.get("CGVA").or_else(|| self.entries.get("AVGC"))?;
        if data.len() >= 4 {
            Some([data[2], data[1], data[0], data[3]])
        } else {
            None
        }
    }

    pub fn get_max_color(&self) -> Option<[u8; 4]> {
        // Tag "CXAM" or "MAXC"
        let data = self.entries.get("CXAM").or_else(|| self.entries.get("MAXC"))?;
        if data.len() >= 4 {
            Some([data[2], data[1], data[0], data[3]])
        } else {
            None
        }
    }

    pub fn has_alpha(&self) -> bool {
        // Tag "GALF" or "FLAG"
        if let Some(data) = self.entries.get("GALF").or_else(|| self.entries.get("FLAG")) {
            if !data.is_empty() {
                return data[0] != 0;
            }
        }
        false
    }

    pub fn get_offsets(&self) -> Option<Vec<u32>> {
        // Tag "SFFO" or "OFFS"
        let data = self.entries.get("SFFO").or_else(|| self.entries.get("OFFS"))?;
        let count = data.len() / 4;
        let mut offsets = Vec::with_capacity(count);
        for i in 0..count {
            let offset = u32::from_le_bytes([
                data[i * 4],
                data[i * 4 + 1],
                data[i * 4 + 2],
                data[i * 4 + 3],
            ]);
            offsets.push(offset);
        }
        Some(offsets)
    }

    pub fn get_tag_infos(&self) -> Vec<TagInfo> {
        self.entries
            .iter()
            .map(|(raw_name, data)| {
                let (display_name, desc, color) = match raw_name.as_str() {
                    "CGVA" | "AVGC" => {
                        let col = if data.len() >= 4 {
                            Some([data[2], data[1], data[0], data[3]])
                        } else {
                            None
                        };
                        (
                            "Average Color (AVGC)".to_string(),
                            "Average RGBA color value of texture".to_string(),
                            col,
                        )
                    }
                    "CXAM" | "MAXC" => {
                        let col = if data.len() >= 4 {
                            Some([data[2], data[1], data[0], data[3]])
                        } else {
                            None
                        };
                        (
                            "Max Color (MAXC)".to_string(),
                            "Maximum RGBA color channel values".to_string(),
                            col,
                        )
                    }
                    "GALF" | "FLAG" => {
                        let is_alpha = !data.is_empty() && data[0] != 0;
                        (
                            "Alpha Flag (FLAG)".to_string(),
                            format!("Transparency flag (value: {is_alpha})"),
                            None,
                        )
                    }
                    "SFFO" | "OFFS" => (
                        "Mipmap Offsets (OFFS)".to_string(),
                        format!("Offset table for {} mipmap levels", data.len() / 4),
                        None,
                    ),
                    "ZIWS" | "SWIZ" => (
                        "Swizzle (SWIZ)".to_string(),
                        "Texture channel swizzle parameters".to_string(),
                        None,
                    ),
                    _ => (
                        raw_name.clone(),
                        format!("Custom or unknown tag ({raw_name})"),
                        None,
                    ),
                };

                let hex = if data.len() <= 32 {
                    data.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
                } else {
                    let head = data[..16].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
                    format!("{head} ... ({} bytes total)", data.len())
                };

                TagInfo {
                    raw_name: raw_name.clone(),
                    display_name,
                    description: desc,
                    length: data.len(),
                    data_hex: hex,
                    color_preview: color,
                }
            })
            .collect()
    }
}
