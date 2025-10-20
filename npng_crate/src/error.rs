use bincode::error::{DecodeError, EncodeError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NPNGError {
    #[error("Encoding failed: {0}")]
    EncodingError(#[from] EncodeError),

    #[error("Decoding failed: {0}")]
    DecodingError(#[from] DecodeError),

    #[error("{0}")]
    Error(String),

    #[error("Invalid header: {0}")]
    InvalidHeader(String),

    #[error("Invalid checksum: {0}")]
    InvalidChecksum(String),

    #[error("Compression error: {0}")]
    Compression(#[from] NPNGCompressingError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum NPNGCompressingError {
    #[error("Compression failed: {0}")]
    CompressingError(String),

    #[error("Decompression failed: {0}")]
    DecompressingError(String),
}
