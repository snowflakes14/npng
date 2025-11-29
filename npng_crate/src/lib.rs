#[cfg(target_pointer_width = "32")]
compile_error!("32-bit system is not supported. Sorry"); // I don't want to support 32-bit platforms, sorry

extern crate std;

#[cfg(feature = "log")]
use log::warn;

use bytes::Bytes;
use crc32fast::Hasher;
use image::{GenericImageView, ImageBuffer, ImageReader, Pixel as TraitPx, Rgba};
use std::str::FromStr;
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
use crate::types::{CheckSum, SIZE};
use crate::ver::VERSION_METADATA;
use crate::{
    coding::{spawn_plain_decode_workers, spawn_plain_workers},
    utils::{check_image_size_f, deserialize, serialize},
    ver::{VERSION_MAJOR, VERSION_MINOR},
};

pub use crate::types::Img;
pub use crate::types::VersionMetadata;
pub use crate::types::EncoderVersion;

use crate::types::metadata::Metadata;
use crate::types::header::Header;
pub use crate::types::pixel::Pixel;

use crate::compression::CompressMap;

use crate::error::*;
use crate::types::MAX_PIXELS;

mod coding;

#[cfg(feature = "tokio_async")]
pub mod tokio;

mod utils;
mod ver;
pub mod types;
pub mod compression;
pub mod error;

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

pub trait IntoCompressMap: Send + Sync {
    fn into_compress_map(self) -> Result<CompressMap, NPNGError>;
}

impl IntoCompressMap for Encoding {
    fn into_compress_map(self) -> Result<CompressMap, NPNGError> {
        Ok(match self {
            Encoding::Plain => CompressMap::plain(),
            Encoding::Zstd(l) => CompressMap::zstd(l as u32),
            Encoding::Zlib(l) => CompressMap::zlib(l as u32),
        })
    }
}

impl IntoCompressMap for CompressMap {
    fn into_compress_map(self) -> Result<CompressMap, NPNGError> {
        Ok(self)
    }
}

impl<T: Into<String> + Sync + Send> IntoCompressMap for T {
    fn into_compress_map(self) -> Result<CompressMap, NPNGError> {
        let s = self.into();
        match s.to_lowercase().trim() {
            "default" => Ok(CompressMap::default()),
            "plain" => Ok(CompressMap::plain()),
            "none" => Ok(CompressMap::plain()),
            "zlib" => Ok(CompressMap::zlib(6)),
            "zstd" => Ok(CompressMap::zstd(16)),
            _ => Err(NPNGError::Error("Unknown compressing".to_string())),
        }
    }
}



#[derive(Debug, Clone)]
pub struct Config {
    pub save_alpha: bool,
    pub varint: bool,
}

impl Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "save_alpha={}\nvarint={}", self.save_alpha, self.varint)
    }
}

impl Config {
    pub fn new(save_alpha: bool, varint: bool) -> Self {
        Self { save_alpha, varint }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            varint: false,
            save_alpha: true,
        }
    }
}

pub fn version() -> EncoderVersion {
    EncoderVersion {
        version_major: VERSION_MAJOR,
        version_minor: VERSION_MINOR,
        version_metadata: VersionMetadata::from_str(VERSION_METADATA).unwrap(),
    }
}

/// Encodes a vector of pixels with metadata into NPNG bytes.
///
/// # Parameters
/// - `pixels` - Vector of pixels to encode (`Vec<Pixel>`).
/// - `metadata` - Image metadata [`Metadata`]. The width and height will be updated
///   automatically based on the pixels.
/// - `config` - Encoding options [`Config`]:
///     - `save_alpha` - Whether to include the alpha channel in the output. Fully opaque
///       pixels don't saving
///     - `varint` - Whether to use variable-length integer encoding for pixel data.
/// - `compress_map` - Compression map
///
/// # Behavior
/// 1. Checks the image size from the pixels and updates `metadata.width` and `metadata.height`.
/// 2. Ensures there are no duplicate pixel coordinates; returns an error if duplicates exist.
/// 3. Prepares a buffer, encodes the header, and checks its size.
/// 4. Encodes pixels using plain workers, applying `save_alpha` and `varint` options.
/// 5. Compresses the pixel data using [`CompressMap`].
/// 6. Calculates and appends a CRC32 checksum for integrity verification.
///
/// # Returns
/// - `Ok(Vec<u8>)` - Encoded NPNG bytes ready for storage or transmission.
/// - `Err(NPNGError)` - If encoding fails, duplicate pixels are found, or the header is too long.
pub fn encode_pixel_vec_with_metadata<C: IntoCompressMap>(
    pixels: Vec<Pixel>,
    mut metadata: Metadata,
    config: Config,
    compress_map: C,
) -> Result<Vec<u8>, NPNGError> {
    if pixels.len() > MAX_PIXELS {
        return Err(NPNGError::Error(format!(
            "Too many pixels ({}), maximum supported is {}",
            pixels.len(),
            MAX_PIXELS
        )));
    }
    let compress_map = compress_map.into_compress_map()?;
    let save_alpha = config.save_alpha;
    let varint = config.varint;
    let mut hasher = Hasher::new();

    /* ===== Calculating image size ===== */
    let s = check_image_size_f(pixels.clone());
    metadata.width = s.0;
    metadata.height = s.1;

    /* ===== Check for duplicate coordinates === */
    {
        let mut bitmap = vec![0u8; (MAX_PIXELS) / 8]; // 512 MB

        for p in &pixels {
            let idx = (p.y as usize) * SIZE + (p.x as usize);
            let byte = idx / 8;
            let bit = idx % 8;
            let mask = 1 << bit;
            if bitmap[byte] & mask != 0 {
                return Err(NPNGError::DuplicatePixel(p.x, p.y));
            }
            bitmap[byte] |= mask;
        }
    }

    /* ===== Prepare buffer for entire image ===== */
    let mut buf = Vec::new();

    /* ===== Encode header ===== */
    let encoder = compress_map.encoder();
    let header = Header::new(encoder, metadata.clone(), save_alpha, varint)?;
    let ser_header = serialize(&header, true)?;
    if ser_header.len() > 10_000 {
        return Err(NPNGError::Error("Header is too long".to_string()));
    }
    buf.extend(ser_header);

    // ===== Encode pixels =====
    let pixels_encoded = spawn_plain_workers(pixels, config.save_alpha, config.varint)?;
    let pixels_encoded = compress_map.compress(pixels_encoded.into())?;

    /* ===== Calculate and encode CRC32 ===== */
    buf.extend(pixels_encoded.1);
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
/// - `metadata` - Image metadata (`Metadata`). The width and height will be updated
///   based on the actual image dimensions.
/// - `config` - Encoding options (`Config`):
///     - `save_alpha` - Whether to include the alpha channel in the output. Fully opaque
///       pixels are not saved
///     - `varint` - Use variable-length integer encoding for pixel data.
/// - `compress_map` - Compression context used for encoding pixel data and header.
///
/// # Behavior
/// 1. Opens the image file and decodes it into pixels.
/// 2. Converts each pixel to RGBA and packs it into a `Pixel` structure.
/// 3. Updates `metadata.width` and `metadata.height` to match the image.
/// 4. Calls `encode_pixel_vec_with_metadata` to encode pixels, applying the `config` options
///    and compression.
///
/// # Returns
/// - `Ok(Vec<u8>)` - Encoded NPNG bytes ready for storage or transmission.
/// - `Err(NPNGError)` - If opening, decoding, or encoding the image fails.
pub fn encode_image_to_npng_bytes<P: AsRef<OsStr>, C: IntoCompressMap>(
    input: P,
    mut metadata: Metadata,
    config: Config,
    compress_map: C,
) -> Result<Vec<u8>, NPNGError> {
    let compress_map = compress_map.into_compress_map()?;
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


    encode_pixel_vec_with_metadata(pixels, metadata, config, compress_map)
}

/// Encodes an image file (e.g., PNG, JPG) into an NPNG `Img` structure.
///
/// # Img Structure
/// - `pixels` - `Vec<Pixel>` containing all pixels: `{ x, y, color }`
/// - `encoder_version` - [`EncoderVersion`] struct with `version_major` and `version_minor`
/// - `data` - [`Metadata`] containing image dimensions and other info
///
/// # Parameters
/// - `input` - Path to the input image file.
/// - `metadata` - Image [`Metadata`]. The width and height will be updated to match the image.
///
/// # Behavior
/// 1. Opens and decodes the image file.
/// 2. Converts each pixel to RGBA and packs it into a `Pixel` structure.
/// 3. Updates `metadata.width` and `metadata.height`.
/// 4. Returns an `Img` containing all pixels, encoder version, and metadata.
///
/// # Returns
/// - `Ok(Img)` - Encoded image as an `Img` structure ready for further processing or encoding.
/// - `Err(NPNGError)` - If opening, decoding, or processing the image fails.
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
            version_metadata: VersionMetadata::from_str(VERSION_METADATA)?,
        },
        metadata: metadata,
    })
}

/// Encodes a vector of pixels (with [`Metadata`]) into a `.npng` image file.
///
/// # Parameters
/// - `output` - Path to the output `.npng` file.
/// - `metadata` - Image [`Metadata`] containing width, height, and other info.
/// - `pixels` - Vector of [`Pixel`] structures representing the image pixels.
/// - `overwrite` - If `true`, an existing file at `output` will be overwritten; otherwise, the function exits early.
/// - `config` - [`Config`] containing encoding options:
///     - `save_alpha` - Whether to save the alpha channel. Fully opaque pixels may be skipped to reduce file size.
///     - `varint` - Use variable-length integer encoding for pixel data.
///     - `encoding` - Pixel compression method ([`Encoding`]):
///         - [`Encoding::Plain`] - No compression.
///         - [`Encoding::Zlib`]  - Compress using zlib.
///         - [`Encoding::Zstd`]  - Compress using zstd.
/// - `compress_map` - Compression context used for encoding the pixel data and header.
///
/// # Behavior
/// 1. Checks if the output file exists and respects the `overwrite` flag.
/// 2. Encodes the pixel vector with `metadata` using `encode_pixel_vec_with_metadata`, applying `config` options and compression.
/// 3. Writes the resulting NPNG bytes to the output file.
///
/// # Returns
/// - `Ok(())` - Image successfully encoded and saved.
/// - `Err(NPNGError)` - If encoding fails or writing to the file fails.

pub fn encode_pixel_vec_to_npng_image<O: AsRef<OsStr>, C: IntoCompressMap>(
    output: O, // output file path
    metadata: Metadata,
    pixels: Vec<Pixel>,
    overwrite: bool,
    config: Config,
    compress_map: C,
) -> Result<(), NPNGError> {
    let compress_map = compress_map.into_compress_map()?;

    let path = Path::new(&output);

    // Check if the file already exists
    if path.exists() && !overwrite {
        return Ok(()); // Exit early if the file exists and overwriting is not allowed
    }

    /* ===== Encode pixels ===== */
    let ser = encode_pixel_vec_with_metadata(pixels, metadata, config, compress_map)?;

    /* ===== Save encoded pixels ===== */
    let mut file = File::options()
        .create(true)
        .write(true)
        .truncate(overwrite)
        .open(path)?;

    file.write_all(&ser)?;

    Ok(())
}

/// Encodes an image file (e.g., PNG, JPG) into an NPNG image file.
///
/// # Parameters
/// - `input` - Path to the input image file.
/// - `output` - Path to the output `.npng` file.
/// - `metadata` - Image [`Metadata`] containing width, height, and other info.
/// - `overwrite` - If `true`, an existing output file will be overwritten; otherwise, the function exits early.
/// - `config` - [`Config`] containing encoding options:
///     - `save_alpha` - Whether to save the alpha channel. Fully opaque pixels may be skipped to reduce file size.
///     - `varint` - Use variable-length integer encoding for pixel data.
/// - `compress_map` - Compression context used for encoding pixel data and header.
///
/// # Behavior
/// 1. Checks if the output file exists and respects the `overwrite` flag.
/// 2. Reads and decodes the input image, converts it to pixels, and encodes it into NPNG bytes.
/// 3. Writes the encoded NPNG bytes to the specified output file.
///
/// # Returns
/// - `Ok(())` - Image successfully encoded and saved.
/// - `Err(NPNGError)` - If reading, decoding, encoding, or writing fails.
pub fn encode_image_to_npng_image<I: AsRef<OsStr>, O: AsRef<OsStr>, C: IntoCompressMap>(
    input: I,
    output: O,
    metadata: Metadata,
    overwrite: bool,
    config: Config,
    compress_map: C,
) -> Result<(), NPNGError> {
    let compress_map = compress_map.into_compress_map()?;

    let path = Path::new(&output);

    // Check if the file already exists
    if path.exists() && !overwrite {
        return Ok(()); // Exit early if the file exists and overwriting is not allowed
    }

    /* ===== Encode image into npng ===== */
    let ser = encode_image_to_npng_bytes(input, metadata, config, compress_map)?;

    /* ===== Save encoded pixels ===== */
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(overwrite)
        .open(path)?;

    file.write_all(&ser)?;

    Ok(())
}

pub fn encode_img_to_npng_bytes<C: IntoCompressMap>(
    img: Img,
    config: Config,
    compress_map: C,
) -> Result<Vec<u8>, NPNGError> {
    encode_pixel_vec_with_metadata(img.pixels, img.metadata, config, compress_map)
}

/// Decodes NPNG bytes into a vector of [`Pixel`]s along with metadata (`Img`).
///
/// # Img Structure
/// - `pixels` - `Vec<Pixel>` containing all pixels: `{ x, y, color }`
/// - `encoder_version` - [`EncoderVersion`] struct with `version_major` and `version_minor`
/// - `data` - [`Metadata`] containing image dimensions and other info
///
/// # Parameters
/// - `bytes` - Slice of bytes representing the encoded NPNG image.
/// - `check_image_size` - If `true`, the function will recalculate and validate the image dimensions after decoding.
/// - `ignore_checksum` - If `true`, CRC32 checksum verification will be skipped (not recommended).
/// - `compress_map` - Compression context used to decompress the pixel data and header.
///
/// # Behavior
/// 1. Checks the header length and verifies magic bytes to ensure it is a valid NPNG file.
/// 2. Extracts and optionally verifies the CRC32 checksum.
/// 3. Locates the end of the header and deserializes it into a `Header` struct.
/// 4. Checks version compatibility and reads header flags (`alpha` and `varint`).
/// 5. Decompresses the pixel data using `compress_map` and decodes pixels into a `Vec<Pixel>`.
/// 6. Updates `metadata.width` and `metadata.height` if `check_image_size` is `true`.
///
/// # Returns
/// - `Ok(Img)` - Successfully decoded image as an `Img` structure.
/// - `Err(NPNGError)` - If the header is invalid, checksum fails, decompression fails, or pixel decoding fails.
pub fn decode_bytes_to_pixel_vec<C: IntoCompressMap>(
    bytes: &[u8],
    check_image_size: bool,
    ignore_checksum: bool,
    compress_map: C,
) -> Result<Img, NPNGError> {
    let compress_map = compress_map.into_compress_map()?;

    /* ===== Check header len ===== */
    if bytes.len() < 9 {
        return Err(NPNGError::InvalidHeader("Header is too short".to_string()));
    }

    // Split the header into magic bytes and the rest
    let magic_bytes = bytes.split_at(9);
    if magic_bytes.0 != [0x00, 0x4E, 0x00, 0x50, 0x00, 0x4E, 0x00, 0x47, 0x00] {
        return Err(NPNGError::InvalidHeader("Invalid magic bytes".to_string())); // Return err if magic bytes not .. N .. P .. N .. G ..
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
                return Err(NPNGError::InvalidChecksum("broken checksum section".to_string()));
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
            let header = &bytes[..end]; // header including delimiter
            if header.len() > 10_000 {
                return Err(NPNGError::InvalidHeader("Header is too long".to_string())); // Return Err if header is too long (>10KB)
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
                #[cfg(feature = "log")]
                warn!("Image version differs from crate version");
                #[cfg(not(feature = "log"))]
                return Err(NPNGError::Error("Image version differs from crate version".to_string()));
            }
            let save_alpha = header_decoded.alpha;
            let varint = header_decoded.varint;
            let mut result = Img {
                pixels: Vec::new(), // Empty vec, filling after pixel decoding
                encoder_version: EncoderVersion {
                    version_minor: header_decoded.version_minor, //==============================================
                    version_major: header_decoded.version_major, //=== Construct a structure with versions
                    version_metadata: VersionMetadata::from_str( //================================================
                        header_decoded.version_metadata.as_str(),
                    )?,
                },
                metadata: header_decoded.metadata,
            };

            let format = header_decoded.encoding_format.clone();
            let uncompressed =
                compress_map.decompress(Bytes::copy_from_slice(body), format.as_str())?;
            let decoded = spawn_plain_decode_workers(uncompressed, save_alpha, varint)?;
            if decoded.len() > MAX_PIXELS {
                return Err(NPNGError::Error("Pixel vec is too long".to_string()));
            }
            /* ===== Check for duplicate coordinates === */
            {
                let mut bitmap = vec![0u8; (MAX_PIXELS) / 8]; // 512 MB

                for p in &decoded {
                    let idx = (p.y as usize) * SIZE + (p.x as usize);
                    let byte = idx / 8;
                    let bit = idx % 8;
                    let mask = 1 << bit;
                    if bitmap[byte] & mask != 0 {
                        return Err(NPNGError::DuplicatePixel(p.x, p.y));
                    }
                    bitmap[byte] |= mask;
                }
            }

            if check_image_size {
                let real_size = check_image_size_f(decoded.clone());
                result.metadata.width = real_size.0;
                result.metadata.height = real_size.1;
            }

            result.pixels = decoded;

            Ok(result)
        }
        None => Err(NPNGError::Error("Invalid header".to_string())),
    }
}

/// Decodes NPNG bytes into a standard image file (e.g., PNG, JPG) and saves it.
///
/// # Parameters
/// - `bytes` - Slice of bytes representing the encoded NPNG image.
/// - `output` - Path to the output image file.
/// - `ignore_checksum` - If `true`, CRC32 checksum verification will be skipped (not recommended).
/// - `compress_map` - Compression context used to decompress the pixel data and header.
///
/// # Behavior
/// 1. Decodes NPNG bytes into pixels and metadata using `decode_bytes_to_pixel_vec`.
/// 2. Creates an `ImageBuffer` and populates it with decoded RGBA pixel data.
/// 3. Saves the buffer to the specified output file path.
///
/// # Returns
/// - `Ok((EncoderVersion, Metadata))` - Tuple containing the encoder version and image metadata.
/// - `Err(NPNGError)` - If decoding fails or saving the image fails.
pub fn decode_bytes_to_image<O: AsRef<OsStr>, C: IntoCompressMap>(
    bytes: &[u8],
    output: O,
    ignore_checksum: bool,
    compress_map: C,
) -> Result<(EncoderVersion, Metadata), NPNGError> {
    let compress_map = compress_map.into_compress_map()?;

    let img = decode_bytes_to_pixel_vec(bytes, true, ignore_checksum, compress_map)?;
    let metadata = img.metadata.clone();
    let version = img.encoder_version.clone();

    let width = img.metadata.width as u32;
    let height = img.metadata.height as u32;

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

/// Decodes an NPNG file into a standard image file (e.g., PNG, JPG) and saves it.
///
/// # Parameters
/// - `input` - Path to the input `.npng` file.
/// - `output` - Path to the output image file.
/// - `ignore_checksum` - If `true`, CRC32 checksum verification will be skipped (not recommended).
/// - `compress_map` - Compression context used to decompress the pixel data and header.
///
/// # Behavior
/// 1. Reads the NPNG file from the specified input path.
/// 2. Decodes the bytes into pixels and metadata using `decode_bytes_to_image`.
/// 3. Saves the resulting image to the specified output path.
///
/// # Returns
/// - `Ok((EncoderVersion, Metadata))` - Tuple containing the encoder version and image metadata.
/// - `Err(NPNGError)` - If reading, decoding, or saving the image fails.
pub fn decode_npng_image_to_image<I: AsRef<OsStr>, O: AsRef<OsStr>, C: IntoCompressMap>(
    input: I,
    output: O,
    ignore_checksum: bool,
    compress_map: C,
) -> Result<(EncoderVersion, Metadata), NPNGError> {
    let compress_map = compress_map.into_compress_map()?;

    let buf = std::fs::read(Path::new(input.as_ref()))?;
    decode_bytes_to_image(&buf, output, ignore_checksum, compress_map)
}

/// Decodes an NPNG file into a vector of [`Pixel`]s and metadata ([`Img`]).
///
/// # Parameters
/// - `input` - Path to the input `.npng` file.
/// - `check_image_size` - If `true`, the function will recalculate and validate the image dimensions after decoding.
/// - `ignore_checksum` - If `true`, CRC32 checksum verification will be skipped (not recommended).
/// - `compress_map` - Compression context used to decompress the pixel data and header.
///
/// # Behavior
/// 1. Reads the NPNG file from the specified path.
/// 2. Decodes the bytes into pixels and metadata using `decode_bytes_to_pixel_vec`, applying checksum verification and image size checks according to the parameters.
///
/// # Returns
/// - `Ok(Img)` - Successfully decoded image as an `Img` structure containing pixels and metadata.
/// - `Err(NPNGError)` - If reading the file, decoding, or decompression fails.
pub fn decode_npng_file_to_pixels<I: AsRef<OsStr>, C: IntoCompressMap>(
    input: I,
    check_image_size: bool,
    ignore_checksum: bool,
    compress_map: C,
) -> Result<Img, NPNGError> {
    let compress_map = compress_map.into_compress_map()?;

    let buf = std::fs::read(Path::new(input.as_ref()))?;
    decode_bytes_to_pixel_vec(&buf, check_image_size, ignore_checksum, compress_map)
}

/// Decodes NPNG bytes into an `ImageBuffer` and returns the image metadata.
///
/// # Parameters
/// - `bytes` - Slice of bytes representing the encoded NPNG image.
/// - `ignore_checksum` - If `true`, CRC32 checksum verification will be skipped (not recommended).
/// - `compress_map` - Compression context used to decompress the pixel data and header.
///
/// # Behavior
/// 1. Decodes the NPNG bytes into pixels and metadata using `decode_bytes_to_pixel_vec`.
/// 2. Creates an `ImageBuffer<Rgba<u8>, Vec<u8>>` and populates it with decoded pixel data.
/// 3. Returns the image buffer along with the metadata.
///
/// # Returns
/// - `Ok((ImageBuffer<Rgba<u8>, Vec<u8>>, Metadata))` - Decoded image buffer and metadata.
/// - `Err(NPNGError)` - If decoding or decompression fails.
pub fn decode_npng_bytes_to_image_buffer<C: IntoCompressMap>(
    bytes: &[u8],
    ignore_checksum: bool,
    compress_map: C,
) -> Result<(ImageBuffer<Rgba<u8>, Vec<u8>>, Metadata), NPNGError> {
    let compress_map = compress_map.into_compress_map()?;

    let img = decode_bytes_to_pixel_vec(bytes, true, ignore_checksum, compress_map)?;

    let width = img.metadata.width as u32;
    let height = img.metadata.height as u32;

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

    Ok((buffer, img.metadata))
}

/// Decodes an NPNG file into a raw RGBA byte vector along with image dimensions.
///
/// # Parameters
/// - `input` - Path to the input `.npng` file.
/// - `ignore_checksum` - If `true`, CRC32 checksum verification will be skipped (not recommended).
/// - `compress_map` - Compression context used to decompress the pixel data and header.
///
/// # Behavior
/// 1. Reads the NPNG file from the specified path.
/// 2. Decodes the bytes into an `ImageBuffer` using `decode_npng_bytes_to_image_buffer`.
/// 3. Converts the `ImageBuffer` into a flat `Vec<u8>` in RGBA order.
/// 4. Returns the raw byte vector along with the width and height of the image.
///
/// # Returns
/// - `Ok((Vec<u8>, u32, u32))` - Raw RGBA bytes, width, and height of the decoded image.
/// - `Err(NPNGError)` - If reading, decoding, or decompression fails.
pub fn decode_npng_file_to_rgba_vec<I: AsRef<OsStr>, C: IntoCompressMap>(
    input: I,
    ignore_checksum: bool,
    compress_map: C,
) -> Result<(Vec<u8>, u32, u32), NPNGError> {
    let compress_map = compress_map.into_compress_map()?;

    let (buffer, _) = decode_npng_bytes_to_image_buffer(
        &std::fs::read(Path::new(input.as_ref()))?,
        ignore_checksum,
        compress_map,
    )?;
    let width = buffer.width();
    let height = buffer.height();

    // Convert ImageBuffer into Vec<u8>
    let raw = buffer.into_raw();
    Ok((raw, width, height))
}
