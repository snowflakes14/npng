# Npng

Npng — an educational raster image storage format.  
Implemented in Rust. The official library (lib) will be published later.

------------------------------------------------------------

## 📘 Format Description

Npng is a binary image format with support for transparency and arbitrary shapes.

------------------------------------------------------------

### 🔹 Features

1. **Alpha Channel**
    - Optional. Fully transparent pixels are not stored.
    - Partial transparency is available when the `alpha` flag is enabled.

2. **Image Shape**
    - Each pixel has (x, y) coordinates, allowing storage of images with arbitrary shapes.

3. **Compression**
    - Officially supported formats: Plain (no compression), Zlib, Zstd.

4. **Integrity**
    - Data verification via CRC32.

5. **Encoding**
    - Uses Little Endian.
    - Varint support is possible (not recommended).

------------------------------------------------------------

## ⚙️ Structures

**Header** — contains version information, flags, and encoding format.

```rust
pub struct Header {
    pub header: [u8; 9], // signature ([0x00, 0x4E, 0x00, 0x50, 0x00, 0x4E, 0x00, 0x47, 0x00])
    pub version_major: u8,
    pub version_minor: u16,
    pub del: [u8; 4],
    pub alpha: bool,
    pub varint: bool,
    pub reserved: [u8; 8], // reserved for future use
    pub encoding_format: String, // "Plain", "Zlib", "Zstd"
    pub metadata: Metadata,
    pub del: [u8; 6],
}
```

------------------------------------------------------------

**Metadata** — main image information.

```rust
pub struct Metadata {
    pub created_in: String,
    pub width: u16,
    pub height: u16,
    pub extra: HashMap<String, String>,
}
```

------------------------------------------------------------

**Pixel** — pixel coordinates and color.

```rust
pub struct Pixel {
    pub x: u16,
    pub y: u16,
    pub color: u32, // RGBA
}
```

```rust
pub struct RGBPixel {
    pub x: u16,
    pub y: u16,
    pub color: [u8; 3], // RGB
}
```

-------------------------------------------------------------

**CheckSum** — data integrity verification.

```rust
pub struct CheckSum {
    pub delimiter: [u8; 16],
    pub crc32: u32,
}
```

------------------------------------------------------------

This is an educational project — please don’t judge too harshly.

by snowflakes14 (c) ♥



P.S. Sorry for my bad english
