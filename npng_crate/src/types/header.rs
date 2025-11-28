use bincode::{Decode, Encode};
use crate::error::NPNGError;
use crate::types::metadata::Metadata;
use crate::ver::{VERSION_MAJOR, VERSION_METADATA, VERSION_MINOR};

#[repr(C)]
#[derive(Debug, Clone, Encode, Decode)]
pub struct Header {
    pub magic: [u8; 9], // [0x00, 0x4E, 0x00, 0x50, 0x00, 0x4E, 0x00, 0x47, 0x00] (utf-16 "NPNG")
    pub version_major: u16,
    pub version_minor: u16,
    pub version_metadata: String,
    pub reserved: [u8; 8], // reserved for future use
    pub alpha: bool,
    pub varint: bool,
    pub encoding_format: String,
    pub metadata: Metadata,
    pub del: [u8; 6], // [0xff; 6]
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
        if encoding_format.len() > 256 {
            encoding_format = encoding_format.split_at(255).0.to_string();
        }
        if metadata.created_in.len() > 512 {
            metadata.created_in = metadata.created_in.split_at(512).0.to_string();
        }
        if metadata.extra.len() > 512 {
            metadata.extra = metadata
                .extra
                .iter()
                .take(512)
                .map(|(k, v)| (k.clone().trim().to_string(), v.clone().trim().to_string()))
                .collect();
        }
        Ok(Header {
            magic: [0x00, 0x4E, 0x00, 0x50, 0x00, 0x4E, 0x00, 0x47, 0x00],
            version_major: VERSION_MAJOR,
            version_minor: VERSION_MINOR,
            version_metadata: VERSION_METADATA.to_string(),
            reserved: [0x00; 8], // reserved for future use
            alpha,
            varint,
            encoding_format: encoding_format.trim().to_string(),
            metadata,
            del: [0xff; 6],
        })
    }
}