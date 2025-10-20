/// `coding.rs` - internal functions for encoding and decoding
use std::{
    io::{Cursor, Read, Write},
    sync::Arc,
};

use bincode::config::{legacy, standard};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use rayon::prelude::*;

use crate::{
    error::NPNGError,
    types::{Pixel, RGBPixel},
    utils::{decode_pixel, encode_pixel},
};

pub(crate) fn spawn_plain_workers(
    pixels: Vec<Pixel>,
    save_alpha: bool,
    varint: bool,
) -> Result<Vec<u8>, NPNGError> {
    // 1. Encode pixels in parallel with their indices
    let mut results: Vec<(usize, Vec<u8>)> = pixels
        .clone()
        .into_par_iter()
        .enumerate()
        .map(|(i, pixel)| {
            let encoded = encode_pixel(pixel, save_alpha, varint)?;
            Ok((i, encoded))
        })
        .collect::<Result<Vec<_>, NPNGError>>()?;

    // 2. Sort the results to restore the original order
    results.sort_by_key(|(i, _)| *i);

    // 3. Flatten all encoded pixels into a single buffer
    let mut buf = Vec::new();
    for (_, encoded_pixel) in results {
        buf.extend(encoded_pixel);
    }

    Ok(buf)
}

pub(crate) fn spawn_plain_decode_workers(
    encoded_bytes: Vec<u8>,
    save_alpha: bool,
    varint: bool,
) -> Result<Vec<Pixel>, NPNGError> {
    let mut offsets: Vec<(usize, usize)> = Vec::new();
    let mut cursor = 0usize;
    let mut result;
    while cursor < encoded_bytes.len() {
        if !varint {
            result = if save_alpha {
                // Pixel with alpha
                bincode::decode_from_slice::<Pixel, _>(&encoded_bytes[cursor..], legacy())
                    .map(|(_p, len)| len)
            } else {
                // RGBPixel without alpha
                bincode::decode_from_slice::<RGBPixel, _>(&encoded_bytes[cursor..], legacy())
                    .map(|(_p, len)| len)
            };
        } else {
            result = if save_alpha {
                // Pixel with alpha
                bincode::decode_from_slice::<Pixel, _>(&encoded_bytes[cursor..], standard())
                    .map(|(_p, len)| len)
            } else {
                // RGBPixel without alpha
                bincode::decode_from_slice::<RGBPixel, _>(&encoded_bytes[cursor..], standard())
                    .map(|(_p, len)| len)
            };
        }

        match result {
            Ok(len) => {
                offsets.push((cursor, len));
                cursor += len;
            }
            Err(e) => {
                return Err(NPNGError::Error(format!(
                    "failed to split pixel at offset {}: {}",
                    cursor, e
                )));
            }
        }
    }

    let data_arc = Arc::new(encoded_bytes);

    /* Decoding Pixels */
    let mut pixels: Vec<(usize, Pixel)> = offsets
        .into_par_iter()
        .enumerate()
        .map(|(i, (start, len))| {
            let data_slice: &[u8] = &data_arc[start..start + len];
            let pixel = decode_pixel(data_slice, save_alpha, varint).map_err(|e| {
                NPNGError::Error(format!("Failed to decode pixel at index {}: {}", i, e))
            })?;
            Ok((i, pixel))
        })
        .collect::<Result<Vec<_>, NPNGError>>()?;

    pixels.sort_by_key(|(i, _)| *i);

    let result: Vec<Pixel> = pixels.into_iter().map(|(_, p)| p).collect();

    Ok(result)
}

pub(crate) fn spawn_zlib_compress(uncompressed: &[u8], level: u32) -> Result<Vec<u8>, NPNGError> {
    if level > 9 {
        return Err(NPNGError::Error("Invalid compression level".to_string()));
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(level));
    encoder
        .write_all(uncompressed)
        .map_err(|e| NPNGError::Error(format!("Zlib write failed: {}", e)))?;
    let compressed = encoder
        .finish()
        .map_err(|e| NPNGError::Error(format!("Zlib finish failed: {}", e)))?;

    Ok(compressed)
}

pub(crate) fn spawn_zlib_decompress(compressed: &[u8]) -> Result<Vec<u8>, NPNGError> {
    let mut decoder = ZlibDecoder::new(Cursor::new(compressed));
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| NPNGError::Error(format!("Zlib decode failed: {}", e)))?;

    Ok(decompressed)
}

pub(crate) fn spawn_zstd_compress(uncompressed: &[u8], level: u32) -> Result<Vec<u8>, NPNGError> {
    if level > 22 {
        return Err(NPNGError::Error(
            "Unsupported compression level".to_string(),
        ));
    }

    let mut encoder = zstd::Encoder::new(Vec::new(), level as i32)?;
    encoder
        .write_all(uncompressed)
        .map_err(|e| NPNGError::Error(format!("Zstd write failed: {}", e)))?;
    let compressed = encoder
        .finish()
        .map_err(|e| NPNGError::Error(format!("Zstd finish failed: {}", e)))?;

    Ok(compressed)
}

pub(crate) fn spawn_zstd_decompress(compressed: &[u8]) -> Result<Vec<u8>, NPNGError> {
    let mut decoder = zstd::Decoder::new(Cursor::new(compressed))?;
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| NPNGError::Error(format!("Zstd decode failed: {}", e)))?;

    Ok(decompressed)
}
