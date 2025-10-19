use bincode::error::{DecodeError, EncodeError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NPNGError {
    #[error("encoding error {0}")]
    EncodingError(#[from] EncodeError),
    #[error("decoding error {0}")]
    DecodingError(#[from] DecodeError),
    #[error("error: {0}")]
    Error(String),
    #[error("{0}")]
    InvalidHeader(String),
    #[error("{0}")]
    InvalidChecksum(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
