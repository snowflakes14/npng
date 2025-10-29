use std::{
    collections::HashMap,
    io::{Cursor, Read, Write},
};

use bytes::{Bytes, BytesMut};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use zstd::zstd_safe::WriteBuf;

use crate::{NPNGError, error::NPNGCompressingError};

#[derive(Clone, Debug)]
pub struct CompressMap {
    decompressors:
        HashMap<String, fn(Bytes, Option<u32>) -> Result<BytesMut, NPNGCompressingError>>,
    compressor: (
        String,
        fn(Bytes, u32) -> Result<BytesMut, NPNGCompressingError>,
    ),
    level: u32, // compression level
}

impl Default for CompressMap {
    fn default() -> Self {
        Self::plain()
    }
}

impl CompressMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_level(&mut self, level: u32) {
        self.level = level;
    }

    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn encoder(&self) -> String {
        self.compressor.0.clone()
    }

    pub fn set_compressor(
        &mut self,
        name: String,
        compressor: fn(Bytes, u32) -> Result<BytesMut, NPNGCompressingError>,
    ) -> Result<(), NPNGError> {
        if name.is_empty() || !name.is_ascii() || name.len() > 255 {
            return Err(NPNGError::Error(
                "compressor name is incorrect (empty, non-ascii, or too long)".to_string(),
            ));
        }
        self.compressor = (name, compressor);
        Ok(())
    }

    pub fn add_decompressor(
        &mut self,
        name: String,
        decompressor: fn(Bytes, Option<u32>) -> Result<BytesMut, NPNGCompressingError>,
    ) -> Result<(), NPNGError> {
        if name.is_empty() || !name.is_ascii() || name.len() > 255 {
            return Err(NPNGError::Error(
                "decompressor name is incorrect (empty, non-ascii, or too long)".to_string(),
            ));
        }
        self.decompressors.insert(name, decompressor);
        Ok(())
    }

    pub(crate) fn compress(&self, data: Bytes) -> Result<(String, BytesMut), NPNGError> {
        let (name, func) = self.compressor.clone();
        let compressed = func(data, self.level)?;
        Ok((name.clone(), compressed))
    }

    pub(crate) fn decompress(
        &self,
        data: Bytes,
        decompressor: &str,
    ) -> Result<BytesMut, NPNGError> {
        let func = self
            .decompressors
            .get(decompressor)
            .copied()
            .unwrap_or(Self::__plain_decompress);
        Ok(func(
            data,
            if self.level > 0 {
                Some(self.level)
            } else {
                None
            },
        )?)
    }

    // ===== Built-in functions =====
    fn __plain_compress(data: Bytes, _level: u32) -> Result<BytesMut, NPNGCompressingError> {
        Ok(data.into())
    }

    fn __plain_decompress(
        data: Bytes,
        _level: Option<u32>,
    ) -> Result<BytesMut, NPNGCompressingError> {
        Ok(data.into())
    }

    fn __zstd_compress(data: Bytes, level: u32) -> Result<BytesMut, NPNGCompressingError> {
        spawn_zstd_compress(data, level)
            .map_err(|e| NPNGCompressingError::CompressingError(e.to_string()))
    }

    fn __zstd_decompress(
        data: Bytes,
        _level: Option<u32>,
    ) -> Result<BytesMut, NPNGCompressingError> {
        spawn_zstd_decompress(data)
            .map_err(|e| NPNGCompressingError::DecompressingError(e.to_string()))
    }

    fn __zlib_compress(data: Bytes, level: u32) -> Result<BytesMut, NPNGCompressingError> {
        spawn_zlib_compress(data, level)
            .map_err(|e| NPNGCompressingError::CompressingError(e.to_string()))
    }

    fn __zlib_decompress(
        data: Bytes,
        _level: Option<u32>,
    ) -> Result<BytesMut, NPNGCompressingError> {
        spawn_zlib_decompress(data)
            .map_err(|e| NPNGCompressingError::DecompressingError(e.to_string()))
    }

    fn __xor_encoder(data: Bytes, key: u32) -> Result<BytesMut, NPNGCompressingError> {
        let key_bytes = key.to_le_bytes();
        let key_len = key_bytes.len();

        let mut result: BytesMut = data.into();

        for (i, b) in result.iter_mut().enumerate() {
            *b ^= key_bytes[i % key_len];
        }

        Ok(result)
    }

    fn __xor_decoder(data: Bytes, key: Option<u32>) -> Result<BytesMut, NPNGCompressingError> {
        match key {
            Some(k) => {
                let key_bytes = k.to_le_bytes();
                let key_len = key_bytes.len();
                let mut result: BytesMut = data.into();
                for (i, b) in result.iter_mut().enumerate() {
                    *b ^= key_bytes[i % key_len];
                }
                Ok(result)
            }
            None => Err(NPNGCompressingError::DecompressingError(
                "Empty key".to_string(),
            )),
        }
    }

    // ===== Constructors =====
    pub fn zstd(level: u32) -> Self {
        let mut s = Self {
            decompressors: HashMap::new(),
            compressor: ("plain".to_string(), Self::__plain_compress),
            level: 0,
        };
        s.add_decompressor("zstd".to_string(), Self::__zstd_decompress)
            .unwrap();
        s.set_compressor("zstd".to_string(), Self::__zstd_compress)
            .unwrap();
        s.level = level;
        s
    }

    pub fn zlib(level: u32) -> Self {
        let mut s = Self {
            decompressors: HashMap::new(),
            compressor: ("plain".to_string(), Self::__plain_compress),
            level: 0,
        };
        s.add_decompressor("zlib".to_string(), Self::__zlib_decompress)
            .unwrap();
        s.set_compressor("zlib".to_string(), Self::__zlib_compress)
            .unwrap();
        s.level = level;
        s
    }

    pub fn add_zlib_decompress(&mut self) {
        let _ = self.add_decompressor("zlib".to_string(), Self::__zlib_decompress);
    }

    pub fn add_zstd_decompress(&mut self) {
        let _ = self.add_decompressor("zstd".to_string(), Self::__zstd_decompress);
    }

    pub fn set_zlib_compress(&mut self, level: u32) {
        self.set_level(level);
        let _ = self.set_compressor("zlib".to_string(), Self::__zlib_compress);
    }

    pub fn set_zstd_compress(&mut self, level: u32) {
        self.set_level(level);
        let _ = self.set_compressor("zstd".to_string(), Self::__zstd_compress);
    }

    pub fn set_plain_compress(&mut self) {
        self.set_level(0);
        let _ = self.set_compressor("plain".to_string(), Self::__plain_compress);
    }

    pub fn plain() -> Self {
        let mut s = Self {
            decompressors: HashMap::new(),
            compressor: ("plain".to_string(), Self::__plain_compress),
            level: 0,
        };
        let _ = s.add_decompressor("plain".to_string(), Self::__plain_decompress);
        s
    }

    pub fn set_xor_encoding(&mut self, key: u32) {
        self.set_level(key);
        self.set_compressor("xor".to_string(), Self::__xor_encoder)
            .unwrap()
    }

    pub fn add_xor_decoding(&mut self, key: u32) {
        self.set_level(key);
        self.add_decompressor("xor".to_string(), Self::__xor_decoder)
            .unwrap()
    }

    pub fn xor(key: u32) -> Self {
        let mut d = HashMap::new();
        d.insert("xor".to_string(), Self::__xor_decoder);

        let mut s = Self {
            level: key,
            decompressors: HashMap::new(),
            compressor: ("xor".to_string(), Self::__xor_encoder),
        };
        s.add_decompressor("xor".to_string(), Self::__xor_decoder)
            .unwrap();
        s
    }

    pub fn add_default_decompressors(&mut self) {
        self.add_zlib_decompress();
        self.add_zstd_decompress();
    }
}

pub(crate) fn spawn_zlib_compress(uncompressed: Bytes, level: u32) -> Result<BytesMut, NPNGError> {
    if level > 9 {
        return Err(NPNGError::Error("Invalid compression level".to_string()));
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(level));
    encoder
        .write_all(uncompressed.as_slice())
        .map_err(|e| NPNGError::Error(format!("Zlib write failed: {}", e)))?;
    let compressed = encoder
        .finish()
        .map_err(|e| NPNGError::Error(format!("Zlib finish failed: {}", e)))?;

    Ok(BytesMut::from(compressed.as_slice()))
}

pub(crate) fn spawn_zlib_decompress(compressed: Bytes) -> Result<BytesMut, NPNGError> {
    let mut decoder = ZlibDecoder::new(Cursor::new(compressed));
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| NPNGError::Error(format!("Zlib decode failed: {}", e)))?;

    Ok(BytesMut::from(decompressed.as_slice()))
}

pub(crate) fn spawn_zstd_compress(uncompressed: Bytes, level: u32) -> Result<BytesMut, NPNGError> {
    if level > 22 {
        return Err(NPNGError::Error(
            "Unsupported compression level".to_string(),
        ));
    }

    let mut encoder = zstd::Encoder::new(Vec::new(), level as i32)?;
    encoder
        .write_all(uncompressed.as_slice())
        .map_err(|e| NPNGError::Error(format!("Zstd write failed: {}", e)))?;
    let compressed = encoder
        .finish()
        .map_err(|e| NPNGError::Error(format!("Zstd finish failed: {}", e)))?;

    Ok(BytesMut::from(compressed.as_slice()))
}

pub(crate) fn spawn_zstd_decompress(compressed: Bytes) -> Result<BytesMut, NPNGError> {
    let mut decoder = zstd::Decoder::new(Cursor::new(compressed))?;
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| NPNGError::Error(format!("Zstd decode failed: {}", e)))?;

    Ok(BytesMut::from(decompressed.as_slice()))
}
