use bincode::{
    Decode, Encode,
    config::{legacy, standard as std_config},
};

use crate::{
    NPNGError,
    types::{Pixel, RGBPixel},
};

/// Serialize a value into a byte vector. (bincode wrapper)
///
/// # Parameters
/// - `data`: The value to serialize (must implement the `Encode` trait)
/// - `standard`: Whether to use the standard encoding (varint)
///
/// # Returns
/// `Result<Vec<u8>, NPNGError>`
/// - `Ok(Vec<u8>)`: Serialized bytes
/// - `Err(NPNGError)`: Serialization error
pub(crate) fn serialize<T: Encode>(data: T, standard: bool) -> Result<Vec<u8>, NPNGError> {
    if standard {
        return Ok(bincode::encode_to_vec(data, std_config())?);
    }
    Ok(bincode::encode_to_vec(data, legacy())?)
}

/// Deserialize a byte vector into a value.
///
/// # Parameters
/// - `data`: Bytes to deserialize
/// - `standard`: Whether to use the standard encoding (varint)
///
/// # Returns
/// `Result<O, NPNGError>`
/// - `Ok(O)`: Deserialized object (must implement the `Decode` trait)
/// - `Err(NPNGError)`: Deserialization error
pub(crate) fn deserialize<O: Decode<()>>(data: Vec<u8>, standard: bool) -> Result<O, NPNGError> {
    if standard {
        return Ok(bincode::decode_from_slice(data.as_slice(), std_config())?.0);
    }
    Ok(bincode::decode_from_slice(data.as_slice(), legacy())?.0)
}

/// Encodes a Pixel into a byte vector.
///
/// This function can encode either a full `Pixel` with alpha channel
/// or an `RGBPixel` without alpha, depending on the `save_alpha` flag.
/// Fully transparent pixels (alpha = 0x00) are skipped and return an empty vector.
///
/// # Parameters
/// - `d`: The `Pixel` to encode.
/// - `save_alpha`: If `true`, encode the full `Pixel` including alpha.
///                 If `false`, encode only the RGB channels.
///
/// # Returns
/// - `Ok(Vec<u8>)`: The serialized pixel data.
/// - `Err(NPNGError)`: If serialization fails.
///
/// # Example
/// ```rust
/// let encoded = encode_pixel(pixel, true)?;
/// ```
pub(crate) fn encode_pixel(
    mut d: Pixel,
    save_alpha: bool,
    varint: bool,
) -> Result<Vec<u8>, NPNGError> {
    let color = d.color;

    // Fully transparent pixel - nothing to save
    if (color & 0xFF) == 0x00 {
        return Ok(Vec::new());
    }

    if !save_alpha {
        // Encode as RGBPixel (without alpha)
        let rgb_pixel = RGBPixel {
            x: d.x,
            y: d.y,
            color: [
                ((color >> 24) & 0xFF) as u8, // Red
                ((color >> 16) & 0xFF) as u8, // Green
                ((color >> 8) & 0xFF) as u8,  // Blue
            ],
        };

        // Serialize RGBPixel and return
        let s = serialize(rgb_pixel, varint)?;
        return Ok(s);
    }

    // encode full Pixel with alpha
    d.color = color;
    let s = serialize(d, varint)?;
    Ok(s)
}

/// Decodes a pixel from a byte slice.
///
/// This function can decode either a full `Pixel` with alpha channel
/// or an `RGBPixel` without alpha, depending on the `save_alpha` flag.
///
/// # Parameters
/// - `data`: The byte slice containing the serialized pixel data.
/// - `save_alpha`: If `true`, decode as a full `Pixel` (including alpha).
///                 If `false`, decode as `RGBPixel` and set alpha to 0xFF.
///
/// # Returns
/// - `Ok(Pixel)`: The decoded pixel with proper color and alpha channel.
/// - `Err(NPNGError)`: If deserialization fails.
///
/// # Example
/// ```rust
/// let pixel = decode_pixel(&data, true)?;
/// ```
pub(crate) fn decode_pixel(
    data: &[u8],
    save_alpha: bool,
    varint: bool,
) -> Result<Pixel, NPNGError> {
    if save_alpha {
        // Decode a full Pixel with alpha channel preserved
        let pixel: Pixel = deserialize(data.to_vec(), varint)?;
        Ok(pixel)
    } else {
        // Decode as RGBPixel (without alpha) and construct a full Pixel
        // by setting alpha to 0xFF (fully opaque)
        let rgb_pixel: RGBPixel = deserialize(data.to_vec(), varint)?;
        let pixel = Pixel {
            x: rgb_pixel.x,
            y: rgb_pixel.y,
            color: ((rgb_pixel.color[0] as u32) << 24)  // Red
                | ((rgb_pixel.color[1] as u32) << 16)   // Green
                | ((rgb_pixel.color[2] as u32) << 8)    // Blue
                | 0xFF, // Alpha (fully opaque)
        };
        Ok(pixel)
    }
}

/// Get npng image size
/// # Returns
/// tuple with `(width, height)`
pub(crate) fn check_image_size_f(pixels: Vec<Pixel>) -> (u16, u16) {
    let width = pixels.iter().map(|p| p.x).max().unwrap_or(0) + 1;
    let height = pixels.iter().map(|p| p.y).max().unwrap_or(0) + 1;
    (width, height)
}
