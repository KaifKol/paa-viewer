use std::fmt;

#[derive(Debug)]
pub enum PaaError {
    Io(std::io::Error),
    InvalidMagic([u8; 2]),
    InvalidTagName,
    InvalidDimensions(u32, u32),
    UnsupportedFormat(String),
    DecompressionFailed(String),
    CompressionFailed(String),
    CorruptedData(String),
    EncodingError(String),
}

impl fmt::Display for PaaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidMagic(m) => write!(f, "Invalid PAA magic bytes: [{:#04x}, {:#04x}]", m[0], m[1]),
            Self::InvalidTagName => write!(f, "Invalid tag name in PAA header"),
            Self::InvalidDimensions(w, h) => write!(f, "Invalid texture dimensions: {w}x{h}"),
            Self::UnsupportedFormat(fmt) => write!(f, "Unsupported PAA format: {fmt}"),
            Self::DecompressionFailed(msg) => write!(f, "Decompression failed: {msg}"),
            Self::CompressionFailed(msg) => write!(f, "Compression failed: {msg}"),
            Self::CorruptedData(msg) => write!(f, "Corrupted PAA data: {msg}"),
            Self::EncodingError(msg) => write!(f, "Encoding error: {msg}"),
        }
    }
}

impl std::error::Error for PaaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PaaError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
