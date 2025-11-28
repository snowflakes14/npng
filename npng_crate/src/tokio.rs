extern crate __tk_rt_private as tokio; // use renamed tokio as tokio

use std::ffi::OsStr;

use tokio::task;

use crate::{
    Config, NPNGError,
    EncoderVersion,
    IntoCompressMap,
    decode_bytes_to_image, decode_bytes_to_pixel_vec, decode_npng_image_to_image,
    encode_image_to_npng_bytes, encode_image_to_npng_image, encode_image_to_npng_pixels,
    encode_pixel_vec_to_npng_image, encode_pixel_vec_with_metadata,
    types::{Img, metadata::Metadata, pixel::Pixel},
};

/// Encode pixels -> NPNG bytes (blocking) on a tokio thread.
pub fn encode_pixel_vec_tokio(
    pixels: Vec<Pixel>,
    metadata: Metadata,
    config: Config,
    compress_map: impl IntoCompressMap + 'static,
) -> task::JoinHandle<Result<Vec<u8>, NPNGError>> {
    task::spawn_blocking(move || {
        encode_pixel_vec_with_metadata(pixels, metadata, config, compress_map)
    })
}

/// Encode image file -> NPNG bytes (blocking) on a tokio thread.
pub fn encode_image_to_npng_bytes_tokio<P: AsRef<OsStr> + Send + 'static>(
    input: P,
    metadata: Metadata,
    config: Config,
    compress_map: impl IntoCompressMap + 'static,
) -> task::JoinHandle<Result<Vec<u8>, NPNGError>> {
    task::spawn_blocking(move || encode_image_to_npng_bytes(input, metadata, config, compress_map))
}

/// Decode image file -> Img (pixels+metadata) (blocking) on a tokio thread.
pub fn encode_image_to_npng_pixels_tokio<P: AsRef<OsStr> + Send + 'static>(
    input: P,
    metadata: Metadata,
) -> task::JoinHandle<Result<Img, NPNGError>> {
    task::spawn_blocking(move || encode_image_to_npng_pixels(input, metadata))
}

/// Write pixels -> .npng file (blocking) on a tokio thread.
pub fn encode_pixel_vec_to_npng_image_tokio<O: AsRef<OsStr> + Send + 'static>(
    output: O,
    metadata: Metadata,
    pixels: Vec<Pixel>,
    overwrite: bool,
    config: Config,
    compress_map: impl IntoCompressMap + 'static,
) -> task::JoinHandle<Result<(), NPNGError>> {
    task::spawn_blocking(move || {
        encode_pixel_vec_to_npng_image(output, metadata, pixels, overwrite, config, compress_map)
    })
}

/// Encode image file -> .npng file (blocking) on a tokio thread.
pub fn encode_image_to_npng_image_tokio<
    I: AsRef<OsStr> + Send + 'static,
    O: AsRef<OsStr> + Send + 'static,
>(
    input: I,
    output: O,
    metadata: Metadata,
    overwrite: bool,
    config: Config,
    compress_map: impl IntoCompressMap + 'static,
) -> task::JoinHandle<Result<(), NPNGError>> {
    task::spawn_blocking(move || {
        encode_image_to_npng_image(input, output, metadata, overwrite, config, compress_map)
    })
}

/// Decode NPNG bytes -> Img (blocking) on a tokio thread.
pub fn decode_bytes_to_pixel_vec_tokio(
    bytes: Vec<u8>,
    check_image_size: bool,
    ignore_checksum: bool,
    compress_map: impl IntoCompressMap + 'static,
) -> task::JoinHandle<Result<Img, NPNGError>> {
    task::spawn_blocking(move || {
        decode_bytes_to_pixel_vec(&bytes, check_image_size, ignore_checksum, compress_map)
    })
}

/// Decode NPNG bytes -> image file (blocking) on a tokio thread.
pub fn decode_bytes_to_image_tokio<O: AsRef<OsStr> + Send + 'static>(
    bytes: Vec<u8>,
    output: O,
    ignore_checksum: bool,
    compress_map: impl IntoCompressMap + 'static,
) -> task::JoinHandle<Result<(EncoderVersion, Metadata), NPNGError>> {
    task::spawn_blocking(move || {
        decode_bytes_to_image(&bytes, output, ignore_checksum, compress_map)
    })
}

/// Decode .npng file -> image file (blocking) on a tokio thread.
pub fn decode_npng_image_to_image_tokio<
    I: AsRef<OsStr> + Send + 'static,
    O: AsRef<OsStr> + Send + 'static,
>(
    input: I,
    output: O,
    ignore_checksum: bool,
    compress_map: impl IntoCompressMap + 'static,
) -> task::JoinHandle<Result<(EncoderVersion, Metadata), NPNGError>> {
    task::spawn_blocking(move || {
        decode_npng_image_to_image(input, output, ignore_checksum, compress_map)
    })
}
