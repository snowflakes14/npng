use bincode::{Decode, Encode};
use crate::utils::set_byte;

#[derive(Debug, Clone, Encode, Decode)]
pub struct Pixel {
    pub x: u16,
    pub y: u16,
    pub color: u32, //rgba
}

impl Pixel {
    pub fn new(x: u16, y: u16, color: u32) -> Self {
        Pixel { x, y, color }
    }
}

/// Pixel without alpha channel
#[derive(Debug, Clone, Encode, Decode)]
pub(crate) struct RGBPixel {
    pub x: u16,
    pub y: u16,
    pub color: [u8; 3],
}

impl From<RGBPixel> for Pixel {
    fn from(rgb: RGBPixel) -> Self {
        let col: u32 = set_byte(0, 3, rgb.color[0]);
        let col: u32 = set_byte(col, 2, rgb.color[1]);
        let col: u32 = set_byte(col, 1, rgb.color[2]);
        let col: u32 = set_byte(col, 0, 0xFF);
        Self {
            x: rgb.x,
            y: rgb.y,
            color: col,
        }
    }
}
