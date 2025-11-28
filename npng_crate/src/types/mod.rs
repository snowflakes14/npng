use std::str::FromStr;
use bincode::{Decode, Encode};
use crate::error::NPNGError;
use crate::Pixel;
use crate::types::metadata::Metadata;

pub mod metadata;
pub mod header;
pub mod pixel;

#[derive(Debug, Clone)]
pub struct EncoderVersion {
    pub version_major: u16,
    pub version_minor: u16,
    pub version_metadata: VersionMetadata,
}

#[derive(Debug, Clone)]
pub enum VersionMetadata {
    Experimental,
    Beta,
    Stable,
}

impl FromStr for VersionMetadata {
    type Err = NPNGError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "experimental" => Ok(VersionMetadata::Experimental),
            "beta" => Ok(VersionMetadata::Beta),
            "stable" => Ok(VersionMetadata::Stable),
            _ => Err(NPNGError::Error("Unknown version metadata".to_string())),
        }
    }
}

impl Into<String> for VersionMetadata {
    fn into(self) -> String {
        match self {
            VersionMetadata::Experimental => "experimental".to_string(),
            VersionMetadata::Beta => "beta".to_string(),
            VersionMetadata::Stable => "stable".to_string(),
        }
    }
}

impl EncoderVersion {
    pub fn version(&self) -> (u16, u16, VersionMetadata) {
        (
            self.version_major,
            self.version_minor,
            self.version_metadata.clone(),
        )
    }
    pub fn version_major(&self) -> u16 {
        self.version_major
    }
    pub fn version_minor(&self) -> u16 {
        self.version_minor
    }
    pub fn version_metadata(&self) -> VersionMetadata {
        self.version_metadata.clone()
    }
}

#[derive(Debug, Clone)]
pub struct Img {
    pub pixels: Vec<Pixel>,
    pub encoder_version: EncoderVersion,
    pub metadata: Metadata,
}

impl Img {
    pub fn pixels(&self) -> Vec<Pixel> {
        self.pixels.clone()
    }

    pub fn encoder_version(&self) -> EncoderVersion {
        self.encoder_version.clone()
    }

    pub fn metadata(&self) -> Metadata {
        self.metadata.clone()
    }

    pub fn as_ref(&self) -> &Img {
        self
    }

    pub fn pixels_ref(&self) -> &Vec<Pixel> {
        &self.pixels
    }

    pub fn encode_version_ref(&self) -> &EncoderVersion {
        &self.encoder_version
    }

    pub fn metadata_ref(&self) -> &Metadata {
        &self.metadata
    }
}

#[repr(C)]
#[derive(Encode, Decode, Clone, Debug)]
pub(crate) struct CheckSum {
    pub del: [u8; 16],
    pub crc32: u32,
}

pub(crate) const MAX_PIXELS: usize = SIZE * SIZE; // 4_294_967_296
pub(crate) const SIZE: usize = 65536;
