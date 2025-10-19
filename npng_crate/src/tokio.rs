
extern crate __tk_rt_private as tokio; // use renamed tokio as tokio

use std::ffi::OsStr;

use image::{ImageBuffer, Rgba};
use tokio::task;

use crate::{
    Config, Encoding, NPNGError, decode_bytes_to_image, decode_bytes_to_pixel_vec,
    decode_npng_image_to_image, encode_image_to_bytes, encode_image_to_npng_image,
    encode_image_to_pixels, encode_pixel_vec_to_npng_image, encode_pixel_vec_with_metadata,
    img_to_image_buffer, save_img_to_image_file,
    types::{EncoderVersion, Img, Metadata, Pixel},
};

// Tokio async wrappers for functions

pub fn encode_pixel_vec_tokio(
    pixels: Vec<Pixel>,
    encoding: Encoding,
    metadata: Metadata,
    config: Config,
) -> task::JoinHandle<Result<Vec<u8>, NPNGError>> {
    task::spawn_blocking(move || encode_pixel_vec_with_metadata(pixels, encoding, metadata, config))
}

pub fn encode_image_to_bytes_tokio<P: AsRef<OsStr> + Send + 'static>(
    input: P,
    encoding: Encoding,
    metadata: Metadata,
    config: Config,
) -> task::JoinHandle<Result<Vec<u8>, NPNGError>> {
    task::spawn_blocking(move || encode_image_to_bytes(input, encoding, metadata, config))
}

pub fn encode_image_to_pixels_tokio<P: AsRef<OsStr> + Send + 'static>(
    input: P,
    metadata: Metadata,
) -> task::JoinHandle<Result<Img, NPNGError>> {
    task::spawn_blocking(move || encode_image_to_pixels(input, metadata))
}

pub fn encode_pixel_vec_to_npng_image_tokio<O: AsRef<OsStr> + Send + 'static>(
    output: O,
    encoding: Encoding,
    metadata: Metadata,
    pixels: Vec<Pixel>,
    overwrite: bool,
    config: Config,
) -> task::JoinHandle<Result<(), NPNGError>> {
    task::spawn_blocking(move || {
        encode_pixel_vec_to_npng_image(output, encoding, metadata, pixels, overwrite, config)
    })
}

pub fn encode_image_to_npng_image_tokio<
    I: AsRef<OsStr> + Send + 'static,
    O: AsRef<OsStr> + Send + 'static,
>(
    input: I,
    output: O,
    encoding: Encoding,
    metadata: Metadata,
    overwrite: bool,
    config: Config,
) -> task::JoinHandle<Result<(), NPNGError>> {
    task::spawn_blocking(move || {
        encode_image_to_npng_image(input, output, encoding, metadata, overwrite, config)
    })
}

pub fn decode_bytes_to_pixel_vec_tokio(
    bytes: Vec<u8>,
    check_image_size: bool,
    warnings: bool,
    ignore_checksum: bool,
) -> task::JoinHandle<Result<Img, NPNGError>> {
    task::spawn_blocking(move || {
        decode_bytes_to_pixel_vec(&bytes, check_image_size, warnings, ignore_checksum)
    })
}

pub fn decode_bytes_to_image_tokio<O: AsRef<OsStr> + Send + 'static>(
    bytes: Vec<u8>,
    output: O,
    warnings: bool,
    ignore_checksum: bool,
) -> task::JoinHandle<Result<(EncoderVersion, Metadata), NPNGError>> {
    task::spawn_blocking(move || decode_bytes_to_image(&bytes, output, warnings, ignore_checksum))
}

pub fn decode_npng_image_to_image_tokio<
    I: AsRef<OsStr> + Send + 'static,
    O: AsRef<OsStr> + Send + 'static,
>(
    input: I,
    output: O,
    warnings: bool,
    ignore_checksum: bool,
) -> task::JoinHandle<Result<(EncoderVersion, Metadata), NPNGError>> {
    task::spawn_blocking(move || {
        decode_npng_image_to_image(input, output, warnings, ignore_checksum)
    })
}
