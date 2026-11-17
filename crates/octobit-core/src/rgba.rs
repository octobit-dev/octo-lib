use crate::indexed::pixel_count;
use crate::{BufferError, Rgba8};

/// A grid of colors, four bytes per pixel in the order red, green, blue, alpha.
///
/// Pixels are stored row by row, starting at the top left corner, with straight (not
/// premultiplied) alpha. This is the layout of 8-bit RGBA PNG and of the browser's `ImageData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaBuffer {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

impl RgbaBuffer {
    /// Creates a buffer with every pixel set to `fill`.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::ZeroSize`] if `width` or `height` is zero and
    /// [`BufferError::TooLarge`] if the pixels do not fit in memory.
    pub fn new(width: u32, height: u32, fill: Rgba8) -> Result<Self, BufferError> {
        let len = byte_count(width, height)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| BufferError::TooLarge { width, height })?;
        let Rgba8 { r, g, b, a } = fill;
        bytes.extend(std::iter::repeat_n([r, g, b, a], len / 4).flatten());
        Ok(Self {
            width,
            height,
            bytes,
        })
    }

    /// Creates a buffer from RGBA bytes stored row by row, for example ones from
    /// [`as_bytes`](Self::as_bytes).
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::ZeroSize`] if `width` or `height` is zero,
    /// [`BufferError::TooLarge`] if `width * height * 4` does not fit in `usize` and
    /// [`BufferError::LengthMismatch`] if `bytes` does not hold exactly `width * height * 4` bytes.
    pub fn from_bytes(width: u32, height: u32, bytes: Vec<u8>) -> Result<Self, BufferError> {
        let expected = byte_count(width, height)?;
        if bytes.len() != expected {
            return Err(BufferError::LengthMismatch {
                expected,
                actual: bytes.len(),
            });
        }
        Ok(Self {
            width,
            height,
            bytes,
        })
    }

    /// Returns the width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns all pixels as RGBA bytes, row by row, starting at the top left corner.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the color at (`x`, `y`), or `None` outside the buffer.
    pub fn get(&self, x: u32, y: u32) -> Option<Rgba8> {
        let i = self.offset(x, y)?;
        let [r, g, b, a] = <[u8; 4]>::try_from(self.bytes.get(i..i + 4)?).ok()?;
        Some(Rgba8::new(r, g, b, a))
    }

    /// Sets the pixel at (`x`, `y`) to `color`.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::OutOfBounds`] if the pixel lies outside the buffer.
    pub fn set(&mut self, x: u32, y: u32, color: Rgba8) -> Result<(), BufferError> {
        let pixel = self
            .offset(x, y)
            .and_then(|i| self.bytes.get_mut(i..i + 4))
            .ok_or(BufferError::OutOfBounds { x, y })?;
        pixel.copy_from_slice(&[color.r, color.g, color.b, color.a]);
        Ok(())
    }

    fn offset(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y < self.height)
            .then(|| (y as usize * self.width as usize + x as usize) * 4)
    }
}

fn byte_count(width: u32, height: u32) -> Result<usize, BufferError> {
    pixel_count(width, height)?
        .checked_mul(4)
        .ok_or(BufferError::TooLarge { width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgba8 = Rgba8::opaque(255, 0, 0);

    #[test]
    fn rejects_zero_width_or_height() {
        assert_eq!(RgbaBuffer::new(0, 4, RED), Err(BufferError::ZeroSize));
        assert_eq!(RgbaBuffer::new(4, 0, RED), Err(BufferError::ZeroSize));
    }

    #[test]
    fn rejects_buffers_that_do_not_fit_in_memory() {
        assert_eq!(
            RgbaBuffer::new(u32::MAX, u32::MAX, RED),
            Err(BufferError::TooLarge {
                width: u32::MAX,
                height: u32::MAX
            })
        );
    }

    #[test]
    fn rejects_byte_data_of_the_wrong_length() {
        assert_eq!(
            RgbaBuffer::from_bytes(2, 2, vec![0; 15]),
            Err(BufferError::LengthMismatch {
                expected: 16,
                actual: 15
            })
        );
    }

    #[test]
    fn fills_every_pixel() {
        let buffer = RgbaBuffer::new(2, 1, Rgba8::new(1, 2, 3, 4)).unwrap();

        assert_eq!(buffer.as_bytes(), &[1, 2, 3, 4, 1, 2, 3, 4]);
    }

    #[test]
    fn stores_pixels_row_by_row() {
        let bytes = (0..16).collect();
        let buffer = RgbaBuffer::from_bytes(2, 2, bytes).unwrap();

        assert_eq!(buffer.get(1, 0), Some(Rgba8::new(4, 5, 6, 7)));
        assert_eq!(buffer.get(0, 1), Some(Rgba8::new(8, 9, 10, 11)));
        assert_eq!(buffer.get(2, 0), None);
        assert_eq!(buffer.get(0, 2), None);
    }

    #[test]
    fn sets_pixels_inside_the_buffer_only() {
        let mut buffer = RgbaBuffer::new(2, 1, Rgba8::TRANSPARENT).unwrap();

        assert_eq!(buffer.set(1, 0, RED), Ok(()));
        assert_eq!(buffer.as_bytes(), &[0, 0, 0, 0, 255, 0, 0, 255]);
        assert_eq!(
            buffer.set(0, 1, RED),
            Err(BufferError::OutOfBounds { x: 0, y: 1 })
        );
    }
}
