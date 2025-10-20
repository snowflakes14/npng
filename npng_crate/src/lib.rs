#[allow(dead_code)]
#[allow(unused)]

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fmt::Display,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use crc32fast::Hasher;
use image::{GenericImageView, ImageBuffer, ImageReader, Pixel as TraitPx, Rgba};
use log::warn;

pub use crate::error::NPNGError;
use crate::{
    coding::{
        spawn_plain_decode_workers, spawn_plain_workers, spawn_zlib_decode_workers,
        spawn_zlib_workers, spawn_zstd_decode_workers, spawn_zstd_workers,
    },
    types::{CheckSum, EncoderVersion, Header, Img, Metadata, Pixel},
    utils::{check_image_size_f, deserialize, serialize},
    ver::{VERSION_MAJOR, VERSION_MINOR},
};

mod coding;
mod error;

#[cfg(feature = "tokio_async")]
pub mod tokio;

pub mod types;
mod utils;
mod ver;

#[derive(Debug, Clone)]
pub enum Encoding {
    Plain,    // no compressing (high file sze)
    Zlib(u8), // max - 9
    Zstd(u8), // max - 22
}

impl Default for Encoding {
    fn default() -> Self {
        Encoding::Zstd(16)
    }
}

impl Display for Encoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Encoding::Plain => f.write_str("plain"),
            Encoding::Zlib(_) => f.write_str("zlib"),
            Encoding::Zstd(_) => f.write_str("zstd"),
        }
    }
}

pub fn version() -> EncoderVersion {
    EncoderVersion {
        version_major: VERSION_MAJOR,
        version_minor: VERSION_MINOR,
    }
}

pub struct Config {
    pub save_alpha: bool,
    pub varint: bool,
    pub encoding: Encoding,
}

impl Config {
    pub fn new(save_alpha: bool, varint: bool, encoding: Encoding) -> Self {
        Self {
            save_alpha,
            varint,
            encoding,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            varint: false,
            save_alpha: true,
            encoding: Encoding::default(),
        }
    }
}

/// Encodes a vector of pixels along with metadata into NPNG bytes.
///
/// # Parameters
/// - `pixels` - Vector of pixels to encode (Vec<[`Pixel`]>).
/// - `metadata` - Image [`Metadata`].
/// - `config` - [`Config`] containing encoding options:
///
/// ## Config options
/// - `save_alpha` - Save alpha channel. Fully opaque pixels may be skipped.
/// - `varint` - Use variable-length integer encoding (varint).  
/// - `encoding` - Pixel encoding/compression method ([`Encoding`]):
///     - [`Encoding::Plain`] - Pixels are stored without compression.
///     - [`Encoding::Zlib`]  - Pixels are compressed using the zlib algorithm.
///     - [`Encoding::Zstd`]  - Pixels are compressed using the zstd algorithm.
///
/// # Returns
/// - `Ok(Vec<u8>)` - Encoded NPNG bytes.
/// - `Err(NPNGError)` - An error occurred during encoding.
pub fn encode_pixel_vec_with_metadata(
    pixels: Vec<Pixel>,
    mut metadata: Metadata,
    config: Config,
) -> Result<Vec<u8>, NPNGError> {
    let save_alpha = config.save_alpha;
    let varint = config.varint;
    let encoding = config.encoding;
    let mut hasher = Hasher::new();

    /* ===== Calculating image size ===== */
    let s = check_image_size_f(pixels.clone());
    metadata.width = s.0;
    metadata.height = s.1;

    /* ===== Check for duplicate coordinates === */
    {
        let mut seen = HashSet::new();
        for p in &pixels {
            if !seen.insert((p.x, p.y)) {
                return Err(NPNGError::Error(format!(
                    "Duplicate pixel coordinates found: ({}, {})",
                    p.x, p.y
                )));
            }
        }
    }

    /* ===== Prepare buffer for entire image ===== */
    let mut buf = Vec::new();

    /* ===== Encode header ===== */
    let header = Header::new(encoding.to_string(), metadata.clone(), save_alpha, varint)?;
    let ser_header = serialize(&header, true)?;
    if ser_header.len() > 10_000 {
        return Err(NPNGError::Error("Header is too long".to_string()));
    }
    buf.extend(ser_header);

    // ===== Encode pixels =====
    let pixels_encoded = match encoding {
        Encoding::Plain => spawn_plain_workers(pixels, save_alpha, varint)?,
        Encoding::Zlib(level) => spawn_zlib_workers(pixels, level as u32, save_alpha, varint)?,
        Encoding::Zstd(level) => spawn_zstd_workers(pixels, level as u32, save_alpha, varint)?,
    };

    /* ===== Calculate and encode CRC32 ===== */
    buf.extend(pixels_encoded);
    hasher.update(buf.as_slice());
    let crc32 = hasher.finalize();
    buf.extend(serialize(
        CheckSum {
            del: [
                0x00, 0x00, 0x00, 0x00, 0x43, 0x68, 0x65, 0x63, 0x6B, 0x53, 0x75, 0x6D, 0x00, 0x00,
                0x00, 0x00, // 00 00 00 00 CheckSum 00 00 00 00
            ],
            crc32,
        },
        false,
    )?);

    Ok(buf)
}

/// Encodes an image file (e.g., PNG, JPG) into NPNG bytes.
///
/// # Parameters
/// - `input` - Path to the input image file.
/// - `metadata` - Image [`Metadata`].
/// - `config` - [`Config`] containing encoding options:
///
/// ## Config options
/// - `save_alpha` - Save alpha channel. Fully opaque pixels may be skipped to optimize size.
/// - `varint` - Use variable-length integer encoding (varint).  
/// - `encoding` - Pixel encoding/compression method ([`Encoding`]):
///     - [`Encoding::Plain`] - Pixels are stored without compression.
///     - [`Encoding::Zlib`]  - Pixels are compressed using the zlib algorithm.
///     - [`Encoding::Zstd`]  - Pixels are compressed using the zstd algorithm.
///
/// # Returns
/// - `Ok(Vec<u8>)` - Encoded NPNG bytes.
/// - `Err(NPNGError)` - An error occurred during encoding.
pub fn encode_image_to_npng_bytes<P: AsRef<OsStr>>(
    input: P,
    mut metadata: Metadata,
    config: Config,
) -> Result<Vec<u8>, NPNGError> {
    /* ===== Open Image ===== */
    let img = ImageReader::open(Path::new(&input))
        .map_err(|e| NPNGError::Error(format!("Failed to open image: {}", e)))?
        .with_guessed_format()
        .map_err(|e| NPNGError::Error(format!("Failed to guess image format: {}", e)))?
        .decode()
        .map_err(|e| NPNGError::Error(format!("Failed to decode image: {}", e)))?;

    /* Get image dimensions */
    let (width, height) = img.dimensions();

    let mut pixels = Vec::with_capacity(((width * height) + 60) as usize);

    // Iterate over each pixel in the image
    for (x, y, p) in img.pixels() {
        // Convert the pixel to RGBA format
        let rgba = p.to_rgba();

        // Pack RGBA channels into a single u32 value
        let color: u32 = ((rgba[0] as u32) << 24) // Red channel
            | ((rgba[1] as u32) << 16)           // Green channel
            | ((rgba[2] as u32) << 8)            // Blue channel
            | (rgba[3] as u32); // Alpha channel (ignored if !save_alpha)

        // Store the pixel data in the Pixels vector
        pixels.push(Pixel {
            x: x as u16,
            y: y as u16,
            color,
        });
    }

    metadata.width = width as u16;
    metadata.height = height as u16;

    encode_pixel_vec_with_metadata(pixels, metadata, config)
}

/// This function encodes image (e.g png, jpg) to npng `Img`
/// # Img
/// - `Pixels`: `Vec` with `Pixels`: { x, y, color }
/// - [`EncoderVersion`]: Struct with versions (version_major, version_minor)
/// - `data`: [`Metadata`]
///
/// # Parameters
/// - `input` - input file path
/// - `metadata` - Image [`Metadata`]
///
/// # Returns
/// - `Ok(Img)` - [`Img`]
/// - `Err(NPNGError)` - Error
pub fn encode_image_to_npng_pixels<P: AsRef<OsStr>>(
    input: P,
    mut metadata: Metadata,
) -> Result<Img, NPNGError> {
    /* ===== Open Image ===== */
    let img = ImageReader::open(Path::new(&input))
        .map_err(|e| NPNGError::Error(format!("Failed to open image: {}", e)))?
        .with_guessed_format()
        .map_err(|e| NPNGError::Error(format!("Failed to guess image format: {}", e)))?
        .decode()
        .map_err(|e| NPNGError::Error(format!("Failed to decode image: {}", e)))?;

    /* ===== Get image dimensions ===== */
    let (width, height) = img.dimensions();

    /* ===== Create pixels buf ===== */
    let mut pixels = Vec::with_capacity((width * height) as usize);

    // Iterate over each pixel in the image
    for (x, y, p) in img.pixels() {
        // Convert the pixel to RGBA format
        let rgba = p.to_rgba();

        // Pack RGBA channels into a single u32 value
        let color: u32 = ((rgba[0] as u32) << 24) // Red channel
            | ((rgba[1] as u32) << 16)           // Green channel
            | ((rgba[2] as u32) << 8)            // Blue channel
            | (rgba[3] as u32); // Alpha channel (ignored if !save_alpha)

        // Store the pixel data in the Pixels vector
        pixels.push(Pixel {
            x: x as u16,
            y: y as u16,
            color,
        });
    }

    metadata.width = width as u16;
    metadata.height = height as u16;

    Ok(Img {
        pixels,
        encoder_version: EncoderVersion {
            version_major: VERSION_MAJOR,
            version_minor: VERSION_MINOR,
        },
        data: metadata,
    })
}

/// This function encodes pixel vec (with [`Metadata`]) to .npng image
///
/// # Parameters
/// - `output`: output file path
/// - [`Metadata`] - Image Metadata
/// - pixels: [`Pixel`] Vec
/// - overwrite: overwrite image or no
/// - [`Config`] - config
/// ## Config options
/// - `save_alpha` - Save alpha channel. Fully opaque pixels may be skipped to optimize size.
/// - `varint` - Use variable-length integer encoding (varint).  
/// - `encoding` - Pixel encoding/compression method ([`Encoding`]):
///     - [`Encoding::Plain`] - Pixels are stored without compression.
///     - [`Encoding::Zlib`]  - Pixels are compressed using the zlib algorithm.
///     - [`Encoding::Zstd`]  - Pixels are compressed using the zstd algorithm.
///
/// # Returns
/// - Ok(()) - success encoding
/// - Err(NPNGError) - Error
pub fn encode_pixel_vec_to_npng_image<O: AsRef<OsStr>>(
    output: O, // output file path
    metadata: Metadata,
    pixels: Vec<Pixel>,
    overwrite: bool,
    config: Config,
) -> Result<(), NPNGError> {
    let path = Path::new(&output);

    // Check if the file already exists
    if path.exists() && !overwrite {
        return Ok(()); // Exit early if the file exists and overwriting is not allowed
    }

    /* ===== Encode pixels ===== */
    let ser = encode_pixel_vec_with_metadata(pixels, metadata, config)?;

    /* ===== Save encoded pixels ===== */
    let mut file = File::options()
        .create(true)
        .write(true)
        .truncate(overwrite)
        .open(path)?;

    file.write_all(&ser)?;

    Ok(())
}

/// This function encodes image (e.g png, jpg) to npng image
///
/// # Parameters
/// - `input`: Input image
/// - `output`: Output npng file path
/// - [`Metadata`] - Metadata
/// - `overwrite`: overwrite image or no
/// - config - [`Config`]
/// ## Config options
/// - `save_alpha` - Save alpha channel. Fully opaque pixels may be skipped to optimize size.
/// - `varint` - Use variable-length integer encoding (varint).  
/// - `encoding` - Pixel encoding/compression method ([`Encoding`]):
///     - [`Encoding::Plain`] - Pixels are stored without compression.
///     - [`Encoding::Zlib`]  - Pixels are compressed using the zlib algorithm.
///     - [`Encoding::Zstd`]  - Pixels are compressed using the zstd algorithm.
///
/// # Returns
/// - Ok(()) - success encoding
/// - Err(NPNGError) - Error
pub fn encode_image_to_npng_image<I: AsRef<OsStr>, O: AsRef<OsStr>>(
    input: I,
    output: O,
    metadata: Metadata,
    overwrite: bool,
    config: Config,
) -> Result<(), NPNGError> {
    let path = Path::new(&output);

    // Check if the file already exists
    if path.exists() && !overwrite {
        return Ok(()); // Exit early if the file exists and overwriting is not allowed
    }

    /* ===== Encode image into npng ===== */
    let ser = encode_image_to_npng_bytes(input, metadata, config)?;

    /* ===== Save encoded pixels ===== */
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(overwrite)
        .open(path)?;

    file.write_all(&ser)?;

    Ok(())
}

/// This function decodes bytes to [`Pixel`] Vec with metadata ([`Img`])
/// # Img
/// - `Pixels`: `Vec` with `Pixels`: { x, y, color }
/// - [`EncoderVersion`]: Struct with versions (version_major, version_minor)
/// - `data`: [`Metadata`]
///
/// # Parameters
/// - `bytes`: Slice with bytes
/// - `check_image_size`: check decoded bytes image size
/// - `ignore_checksum`: ignore crc32 (totally not recommended)
///
/// # Returns
/// - Ok(Img) - Success decoding
/// - Err(NPNGError) - Error
pub fn decode_bytes_to_pixel_vec(
    bytes: &[u8],
    check_image_size: bool,
    ignore_checksum: bool,
) -> Result<Img, NPNGError> {
    /* ===== Check header len ===== */
    if bytes.len() < 9 {
        return Err(NPNGError::InvalidHeader("Header is too short".to_string()));
    }

    // Split the header into magic bytes and the rest
    let magic_bytes = bytes.split_at(9);
    if magic_bytes.0 != [0x00, 0x4E, 0x00, 0x50, 0x00, 0x4E, 0x00, 0x47, 0x00] {
        return Err(NPNGError::InvalidHeader("Invalid magic bytes".to_string())); // Return err if magic bytes not . N . P . N . G .
    }

    /* ===== Get CRC32 Checksum stored in file ===== */
    let check_sum = {
        // Determine the starting index of the checksum section
        let checksum_start = bytes.len() - 20;

        // Extract the raw checksum bytes
        let raw_checksum = bytes[checksum_start..].to_vec();

        // Deserialize the checksum
        let checksum_struct: CheckSum = match deserialize(raw_checksum.to_owned(), false) {
            Ok(c) => c,
            Err(_) => {
                // Return error if checksum is bad
                return Err(NPNGError::InvalidChecksum("invalid checksum".to_string()));
            }
        };

        // Return the CRC32 value from the deserialized checksum
        checksum_struct
    }
    .crc32;
    let mut hasher = Hasher::new();

    let delimiter = [0xFF; 6]; // FF FF FF FF FF FF
    let header_end_pos = bytes
        .windows(delimiter.len())
        .position(|w| w == delimiter)
        .map(|pos| pos + delimiter.len());

    match header_end_pos {
        Some(end) => {
            let header = &bytes[..end]; // header including FF FF FF FF FF FF
            if header.len() > 8192 {
                return Err(NPNGError::InvalidHeader("Header is too long".to_string())); // Return Err if header is too long (>8KB or KiB idk)
            }
            let body = &bytes[end..bytes.len() - 20];

            hasher.update(header);
            hasher.update(body);
            let h = hasher.finalize();
            if check_sum != h && !ignore_checksum {
                return Err(NPNGError::InvalidChecksum("Image is corrupted".to_string())); // Return error if CRC32 does not match the CheckSum section
            }

            /* ===== Deserialize the header into a Header struct ===== */
            let header_decoded =
                deserialize::<Header>(header.to_vec(), true).map_err(|e: NPNGError| {
                    NPNGError::InvalidHeader(format!("Header decoding error: {}", e))
                })?;

            if header_decoded.version_major != VERSION_MAJOR {
                warn!("Image version differs from crate version");
            }
            let save_alpha = header_decoded.alpha;
            let varint = header_decoded.varint;
            let mut result = Img {
                pixels: Vec::new(), // Empty vec, filling after pixel decoding
                encoder_version: EncoderVersion {
                    version_minor: header_decoded.version_minor, //===========================================================================
                    version_major: header_decoded.version_major, //=== Construct a structure with versions
                                                                 //===========================================================================
                },
                data: header_decoded.metadata,
            };

            let format = header_decoded.encoding_format.clone();
            match format.as_str() {
                "plain" => {
                    let d = spawn_plain_decode_workers(body.to_vec(), save_alpha, varint)?;
                    result.pixels = d.clone();
                    if check_image_size {
                        let s = check_image_size_f(d);
                        result.data.width = s.0;
                        result.data.height = s.1;
                    }
                    Ok(result)
                }
                "deflate" => {
                    let d = spawn_zlib_decode_workers(body, save_alpha, varint)?;
                    result.pixels = d.clone();
                    if check_image_size {
                        let s = check_image_size_f(d);
                        result.data.width = s.0;
                        result.data.height = s.1;
                    }
                    Ok(result)
                }
                "zstd" => {
                    let d = spawn_zstd_decode_workers(body, save_alpha, varint)?;
                    result.pixels = d.clone();
                    if check_image_size {
                        let s = check_image_size_f(d);
                        result.data.width = s.0;
                        result.data.height = s.1;
                    }
                    Ok(result)
                }
                e => Err(NPNGError::Error(format!("Unknown encoding format: {e}"))),
            }
        }
        None => Err(NPNGError::Error("Invalid header".to_string())),
    }
}

/// This function decodes npng bytes to image (e.g png, jpg)
///
/// # Parameters
/// - `bytes`: Slice with bytes
/// - `output`: Output file path
/// - `ignore_checksum`: ignore crc32 (totally not recommended)
///
/// # Returns
/// - `Ok((EncoderVersion, Metadata))` - tuple with [`EncoderVersion`] and [`Metadata`]
/// - `Err(NPNGError)` - Error
pub fn decode_bytes_to_image<O: AsRef<OsStr>>(
    bytes: &[u8],
    output: O,
    ignore_checksum: bool,
) -> Result<(EncoderVersion, Metadata), NPNGError> {
    let img = decode_bytes_to_pixel_vec(bytes, true, ignore_checksum)?;
    let metadata = img.data.clone();
    let version = img.encoder_version.clone();

    let width = img.data.width as u32;
    let height = img.data.height as u32;

    let mut buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);

    // === Adding Pixels ===
    for pixel in &img.pixels {
        let x = pixel.x as u32;
        let y = pixel.y as u32;

        let r = ((pixel.color >> 24) & 0xFF) as u8;
        let g = ((pixel.color >> 16) & 0xFF) as u8;
        let b = ((pixel.color >> 8) & 0xFF) as u8;
        let a = (pixel.color & 0xFF) as u8;

        buffer.put_pixel(x, y, Rgba([r, g, b, a]));
    }

    // === Saving Image ===
    let path = Path::new(&output);
    buffer
        .save(path)
        .map_err(|e| NPNGError::Error(format!("Failed to save image: {}", e)))?;

    Ok((version, metadata))
}

/// This function decodes npng file to image (e.g png, jpg)
///
/// # Parameters
/// - `input` - input file path
/// - `output` - output file path
/// - `ignore_checksum`: ignore crc32 (totally not recommended)
///
/// # Returns
/// - `Ok((EncoderVersion, Metadata))` - tuple with [`EncoderVersion`] and [`Metadata`]
/// - `Err(NPNGError)` - Error
pub fn decode_npng_image_to_image<I: AsRef<OsStr>, O: AsRef<OsStr>>(
    input: I,
    output: O,
    ignore_checksum: bool,
) -> Result<(EncoderVersion, Metadata), NPNGError> {
    let buf = std::fs::read(Path::new(input.as_ref()))?;
    decode_bytes_to_image(&buf, output, ignore_checksum)
}

pub fn decode_npng_file_to_pixels<I: AsRef<OsStr>>(
    input: I,
    check_image_size: bool,
    ignore_checksum: bool,
) -> Result<Img, NPNGError> {
    let buf = std::fs::read(Path::new(input.as_ref()))?;
    decode_bytes_to_pixel_vec(&buf, check_image_size, ignore_checksum)
}

pub fn decode_npng_bytes_to_image_buffer(
    bytes: &[u8],
    ignore_checksum: bool,
) -> Result<(ImageBuffer<Rgba<u8>, Vec<u8>>, Metadata), NPNGError> {
    let img = decode_bytes_to_pixel_vec(bytes, true, ignore_checksum)?;

    let width = img.data.width as u32;
    let height = img.data.height as u32;

    let mut buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);

    for pixel in &img.pixels {
        let x = pixel.x as u32;
        let y = pixel.y as u32;

        let r = ((pixel.color >> 24) & 0xFF) as u8;
        let g = ((pixel.color >> 16) & 0xFF) as u8;
        let b = ((pixel.color >> 8) & 0xFF) as u8;
        let a = (pixel.color & 0xFF) as u8;

        buffer.put_pixel(x, y, Rgba([r, g, b, a]));
    }

    Ok((buffer, img.data))
}

pub fn decode_npng_file_to_rgba_vec<I: AsRef<OsStr>>(
    input: I,
    ignore_checksum: bool,
) -> Result<(Vec<u8>, u32, u32), NPNGError> {
    let (buffer, _) = decode_npng_bytes_to_image_buffer(
        &std::fs::read(Path::new(input.as_ref()))?,
        ignore_checksum,
    )?;
    let width = buffer.width();
    let height = buffer.height();

    // Convert ImageBuffer into Vec<u8>
    let raw = buffer.into_raw();
    Ok((raw, width, height))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, path::Path};

    use image::io::Reader as ImageReader;

    use super::*;

    fn require_in_png() {
        let p = Path::new("in.png");
        assert!(p.exists(), "in.png not found");
    }

    #[test]
    fn test_encode_image_to_npng_image_creates_file() {
        require_in_png();

        let metadata = Metadata::new_str("TEST", HashMap::new());

        let out_path = "out.npng";

        if Path::new(out_path).exists() {
            let _ = fs::remove_file(out_path);
        }

        encode_image_to_npng_image("in.png", out_path, metadata, true, Config::default())
            .expect("encode_image_to_npng_image failed.");

        let md = fs::metadata(out_path).expect("cannot read out.npng");
        assert!(md.len() > 0, "out.npng is empty");

        let _ = fs::remove_file(out_path);
    }

    #[test]
    fn test_encode_bytes_and_decode_bytes_roundtrip() {
        require_in_png();

        let reader = ImageReader::open("in.png")
            .expect("cannot open in.png")
            .with_guessed_format()
            .expect("cannot guess format");
        let img = reader.decode().expect("cannot decode in.png");

        let metadata = Metadata::new_str("TEST", HashMap::new());

        let bytes = encode_image_to_npng_bytes("in.png", metadata, Config::default())
            .expect("encode_image_to_npng_bytes failed");

        assert!(!bytes.is_empty(), "Encoded bytes are empty");

        let out_decoded = "decoded.png";
        if Path::new(out_decoded).exists() {
            let _ = fs::remove_file(out_decoded);
        }

        let (version, decoded_meta) = decode_bytes_to_image(&bytes, out_decoded, false)
            .expect("decode_bytes_to_image failed");

        let md = fs::metadata(out_decoded).expect("cannot read decoded.png");
        assert!(md.len() > 0, "decoded.png is empty");

        assert!(
            version.version_major <= version.version_major,
            "Invalid version"
        );

        let _ = fs::remove_file(out_decoded);
    }
}
