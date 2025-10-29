use std::collections::HashMap;

use bincode::{Decode, Encode};

use crate::{
    Encoding, NPNGError,
    compress::CompressMap,
    utils::set_byte,
    ver::{VERSION_MAJOR, VERSION_MINOR},
};

#[repr(C)]
#[derive(Debug, Clone, Encode, Decode)]
pub struct Header {
    pub header: [u8; 9], // [0x00, 0x4E, 0x00, 0x50, 0x00, 0x4E, 0x00, 0x47, 0x00]
    pub version_major: u16,
    pub version_minor: u16,
    pub del_h: [u8; 4], // [0x00, 0x00, 0x00, 0x00]
    pub alpha: bool,
    pub varint: bool,
    pub reserved: [u8; 8], // reserved for future use
    pub encoding_format: String,
    pub metadata: Metadata,
    pub del: [u8; 6], // [0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
}

impl Header {
    pub fn new(
        mut encoding_format: String,
        mut metadata: Metadata,
        alpha: bool,
        varint: bool,
    ) -> Result<Self, NPNGError> {
        if encoding_format.is_empty() {
            return Err(NPNGError::Error("encoding format is empty".to_string()));
        }
        if !encoding_format.is_ascii() {
            return Err(NPNGError::Error(
                "encoding format must be ascii".to_string(),
            ));
        }
        if !metadata.created_in.is_ascii() {
            return Err(NPNGError::Error(
                "metadata.created_in must be ascii".to_string(),
            ));
        }
        if encoding_format.len() > 255 {
            encoding_format = encoding_format.split_at(255).0.to_string();
        }
        if metadata.created_in.len() > 255 {
            metadata.created_in = metadata.created_in.split_at(255).0.to_string();
        }
        if metadata.extra.len() > 512 {
            metadata.extra = metadata
                .extra
                .iter()
                .take(512)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
        }
        Ok(Header {
            header: [0x00, 0x4E, 0x00, 0x50, 0x00, 0x4E, 0x00, 0x47, 0x00],
            del_h: [0x00; 4],
            version_major: VERSION_MAJOR,
            version_minor: VERSION_MINOR,
            alpha,
            varint,
            reserved: [1; 8], // reserved for future use
            encoding_format,
            metadata,
            del: [0xff; 6],
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, Encode, Decode)]
pub struct Metadata {
    pub created_in: String,
    pub width: u16,
    pub height: u16,
    pub extra: HashMap<String, String>,
}

impl Metadata {
    pub fn new(created_in: String, extra: HashMap<String, String>) -> Self {
        Metadata {
            created_in,
            width: 0,
            height: 0,
            extra,
        }
    }
    pub fn new_str(created_in: &str, extra: HashMap<&str, &str>) -> Self {
        Metadata {
            created_in: String::from(created_in),
            width: 0,
            height: 0,
            extra: extra
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Encode, Decode)]
pub struct Pixel {
    pub x: u16,
    pub y: u16,
    pub color: u32,
}

impl From<RGBPixel> for Pixel {
    fn from(rgb: RGBPixel) -> Self {
        let col: u32 = set_byte(0, 0, rgb.color[0]);
        let col: u32 = set_byte(col, 1, rgb.color[1]);
        let col: u32 = set_byte(col, 2, rgb.color[2]);
        let col: u32 = set_byte(col, 3, 0xFF);
        Self {
            x: rgb.x,
            y: rgb.y,
            color: col,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Encode, Decode)]
pub struct RGBPixel {
    pub x: u16,
    pub y: u16,
    pub color: [u8; 3],
}

impl Pixel {
    pub fn new(x: u16, y: u16, color: u32) -> Self {
        Pixel { x, y, color }
    }
}

#[derive(Debug, Clone)]
pub struct EncoderVersion {
    pub(crate) version_major: u16,
    pub(crate) version_minor: u16,
}

impl EncoderVersion {
    pub fn version(&self) -> (u16, u16) {
        (self.version_major, self.version_minor)
    }
    pub fn version_major(&self) -> u16 {
        self.version_major
    }
    pub fn version_minor(&self) -> u16 {
        self.version_minor
    }
}

#[derive(Debug, Clone)]
pub struct Img {
    pub pixels: Vec<Pixel>,
    pub encoder_version: EncoderVersion,
    pub data: Metadata,
}

#[repr(C)]
#[derive(Encode, Decode, Clone, Debug)]
pub struct CheckSum {
    pub(crate) del: [u8; 16],
    pub(crate) crc32: u32,
}

pub trait IntoCompressMap: Send + Sync {
    fn into_compress_map(self) -> Result<CompressMap, NPNGError>;
}

impl IntoCompressMap for Encoding {
    fn into_compress_map(self) -> Result<CompressMap, NPNGError> {
        Ok(match self {
            Encoding::Plain => CompressMap::plain(),
            Encoding::Zstd(l) => CompressMap::zstd(l as u32),
            Encoding::Zlib(l) => CompressMap::zlib(l as u32),
        })
    }
}

impl IntoCompressMap for CompressMap {
    fn into_compress_map(self) -> Result<CompressMap, NPNGError> {
        Ok(self)
    }
}
