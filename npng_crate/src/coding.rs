/// `coding.rs` - internal functions for encoding and decoding
use std::sync::Arc;

use bincode::config::{legacy, standard};
use bytes::BytesMut;
use rayon::prelude::*;
use npng_core::error::NPNGError;
use npng_core::{Pixel, RGBPixel};
use crate::{
    utils::encode_pixel,
};

pub(crate) fn spawn_plain_workers(
    pixels: Vec<Pixel>,
    save_alpha: bool,
    varint: bool,
) -> Result<BytesMut, NPNGError> {
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
    let mut buf = BytesMut::new();
    for (_, encoded_pixel) in results {
        buf.extend(encoded_pixel);
    }

    Ok(buf)
}

pub(crate) fn spawn_plain_decode_workers(
    encoded_bytes: BytesMut,
    save_alpha: bool,
    varint: bool,
) -> Result<Vec<Pixel>, NPNGError> {
    let data_arc = Arc::new(encoded_bytes);

    let data_len = data_arc.len();
    let mut cursor = 0usize;

    let mut pixels = Vec::new();

    while cursor < data_len {
        let slice = &data_arc[cursor..];

        let (pixel, len) = if !varint {
            if save_alpha {
                bincode::decode_from_slice::<Pixel, _>(slice, legacy())?
            } else {
                let (rgb, len) = bincode::decode_from_slice::<RGBPixel, _>(slice, legacy())?;
                (Pixel::from(rgb), len)
            }
        } else {
            if save_alpha {
                bincode::decode_from_slice::<Pixel, _>(slice, standard())?
            } else {
                let (rgb, len) = bincode::decode_from_slice::<RGBPixel, _>(slice, standard())?;
                (Pixel::from(rgb), len)
            }
        };

        pixels.push(pixel);

        cursor += len;
    }

    Ok(pixels)
}
