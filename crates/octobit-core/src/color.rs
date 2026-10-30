/// An sRGB color with 8 bits per channel and straight (not premultiplied) alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba8 {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel: 0 is fully transparent, 255 is fully opaque.
    pub a: u8,
}

impl Rgba8 {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    /// Creates a color from its four channels.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a fully opaque color.
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}
