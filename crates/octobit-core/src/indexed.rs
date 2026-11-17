use std::fmt;

// Coordinates are `u32` and offsets are `usize`. The casts between them are lossless on
// 32-bit and 64-bit targets, the only ones octo-lib supports.
const _: () = assert!(usize::BITS >= 32);

/// A grid of palette indices, one byte per pixel.
///
/// Pixels are stored row by row, starting at the top left corner. The buffer does not hold
/// a palette: callers check indices against their own [`Palette`](crate::Palette).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedBuffer {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// One pixel set to a palette index. [`IndexedBuffer::diff`] returns a list of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelChange {
    /// Column, counted from the left edge.
    pub x: u32,
    /// Row, counted from the top edge.
    pub y: u32,
    /// The new palette index.
    pub index: u8,
}

/// Errors from creating or changing an [`IndexedBuffer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
    /// The width or the height is zero.
    ZeroSize,
    /// The pixels do not fit in memory.
    TooLarge {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
    },
    /// The pixel data does not hold exactly `width * height` bytes.
    LengthMismatch {
        /// `width * height`.
        expected: usize,
        /// The length of the pixel data.
        actual: usize,
    },
    /// The pixel lies outside the buffer.
    OutOfBounds {
        /// Column of the pixel.
        x: u32,
        /// Row of the pixel.
        y: u32,
    },
    /// Two buffers that must have the same size do not.
    SizeMismatch,
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSize => write!(f, "buffer width and height must be at least 1"),
            Self::TooLarge { width, height } => {
                write!(f, "a {width}x{height} buffer does not fit in memory")
            }
            Self::LengthMismatch { expected, actual } => {
                write!(f, "expected {expected} pixels, got {actual}")
            }
            Self::OutOfBounds { x, y } => write!(f, "pixel ({x}, {y}) is outside the buffer"),
            Self::SizeMismatch => write!(f, "buffers have different sizes"),
        }
    }
}

impl std::error::Error for BufferError {}

impl IndexedBuffer {
    /// Creates a buffer with every pixel set to `fill`.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::ZeroSize`] if `width` or `height` is zero and
    /// [`BufferError::TooLarge`] if the pixels do not fit in memory.
    pub fn new(width: u32, height: u32, fill: u8) -> Result<Self, BufferError> {
        let len = pixel_count(width, height)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(len)
            .map_err(|_| BufferError::TooLarge { width, height })?;
        pixels.resize(len, fill);
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Creates a buffer from pixels stored row by row, for example ones saved from
    /// [`pixels`](Self::pixels).
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::ZeroSize`] if `width` or `height` is zero,
    /// [`BufferError::TooLarge`] if `width * height` does not fit in `usize` and
    /// [`BufferError::LengthMismatch`] if `pixels` does not hold exactly `width * height` bytes.
    pub fn from_pixels(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, BufferError> {
        let expected = pixel_count(width, height)?;
        if pixels.len() != expected {
            return Err(BufferError::LengthMismatch {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
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

    /// Returns all pixels row by row, starting at the top left corner.
    ///
    /// Together with the width and height this is a full snapshot of the buffer.
    /// [`from_pixels`](Self::from_pixels) restores it.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Returns the palette index at (`x`, `y`), or `None` outside the buffer.
    pub fn get(&self, x: u32, y: u32) -> Option<u8> {
        self.offset(x, y).and_then(|i| self.pixels.get(i).copied())
    }

    /// Sets the pixel at (`x`, `y`) to `index`.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::OutOfBounds`] if the pixel lies outside the buffer.
    pub fn set(&mut self, x: u32, y: u32, index: u8) -> Result<(), BufferError> {
        let pixel = self
            .offset(x, y)
            .and_then(|i| self.pixels.get_mut(i))
            .ok_or(BufferError::OutOfBounds { x, y })?;
        *pixel = index;
        Ok(())
    }

    /// Returns the pixels that differ in `newer`, in row order.
    ///
    /// Applying the result to this buffer with [`apply`](Self::apply) gives `newer`.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::SizeMismatch`] if the buffers have different sizes.
    pub fn diff(&self, newer: &Self) -> Result<Vec<PixelChange>, BufferError> {
        if (self.width, self.height) != (newer.width, newer.height) {
            return Err(BufferError::SizeMismatch);
        }
        let mut changes = Vec::new();
        for ((old_row, new_row), y) in self.rows().zip(newer.rows()).zip(0..) {
            for ((&old, &new), x) in old_row.iter().zip(new_row).zip(0..) {
                if old != new {
                    changes.push(PixelChange { x, y, index: new });
                }
            }
        }
        Ok(changes)
    }

    /// Sets every pixel listed in `changes`, in order, so a later change to the same pixel wins.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::OutOfBounds`] for the first change outside the buffer. The buffer
    /// stays unchanged in that case.
    pub fn apply(&mut self, changes: &[PixelChange]) -> Result<(), BufferError> {
        if let Some(c) = changes.iter().find(|c| self.offset(c.x, c.y).is_none()) {
            return Err(BufferError::OutOfBounds { x: c.x, y: c.y });
        }
        for c in changes {
            self.set(c.x, c.y, c.index)?;
        }
        Ok(())
    }

    fn offset(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y < self.height).then(|| y as usize * self.width as usize + x as usize)
    }

    fn rows(&self) -> std::slice::ChunksExact<'_, u8> {
        self.pixels.chunks_exact(self.width as usize)
    }
}

fn pixel_count(width: u32, height: u32) -> Result<usize, BufferError> {
    if width == 0 || height == 0 {
        return Err(BufferError::ZeroSize);
    }
    (width as usize)
        .checked_mul(height as usize)
        .ok_or(BufferError::TooLarge { width, height })
}

#[cfg(test)]
mod tests {
    use proptest::collection::vec;
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn rejects_zero_width_or_height() {
        assert_eq!(IndexedBuffer::new(0, 4, 0), Err(BufferError::ZeroSize));
        assert_eq!(IndexedBuffer::new(4, 0, 0), Err(BufferError::ZeroSize));
    }

    #[test]
    fn rejects_buffers_that_do_not_fit_in_memory() {
        assert_eq!(
            IndexedBuffer::new(u32::MAX, u32::MAX, 0),
            Err(BufferError::TooLarge {
                width: u32::MAX,
                height: u32::MAX
            })
        );
    }

    #[test]
    fn rejects_pixel_data_of_the_wrong_length() {
        assert_eq!(
            IndexedBuffer::from_pixels(2, 2, vec![0; 3]),
            Err(BufferError::LengthMismatch {
                expected: 4,
                actual: 3
            })
        );
    }

    #[test]
    fn stores_pixels_row_by_row() {
        let buffer = IndexedBuffer::from_pixels(2, 2, vec![0, 1, 2, 3]).unwrap();

        assert_eq!(buffer.get(1, 0), Some(1));
        assert_eq!(buffer.get(0, 1), Some(2));
        assert_eq!(buffer.get(2, 0), None);
        assert_eq!(buffer.get(0, 2), None);
    }

    #[test]
    fn sets_pixels_inside_the_buffer_only() {
        let mut buffer = IndexedBuffer::new(3, 2, 0).unwrap();

        assert_eq!(buffer.set(2, 1, 7), Ok(()));
        assert_eq!(buffer.pixels(), &[0, 0, 0, 0, 0, 7]);
        assert_eq!(
            buffer.set(3, 0, 7),
            Err(BufferError::OutOfBounds { x: 3, y: 0 })
        );
    }

    #[test]
    fn restores_a_snapshot_from_its_pixels() {
        let mut buffer = IndexedBuffer::new(3, 2, 1).unwrap();
        buffer.set(1, 1, 5).unwrap();

        let restored =
            IndexedBuffer::from_pixels(buffer.width(), buffer.height(), buffer.pixels().to_vec());

        assert_eq!(restored, Ok(buffer));
    }

    #[test]
    fn diff_lists_changed_pixels_in_row_order() {
        let old = IndexedBuffer::new(3, 2, 0).unwrap();
        let mut newer = old.clone();
        newer.set(0, 1, 4).unwrap();
        newer.set(2, 0, 9).unwrap();

        assert_eq!(
            old.diff(&newer),
            Ok(vec![
                PixelChange {
                    x: 2,
                    y: 0,
                    index: 9
                },
                PixelChange {
                    x: 0,
                    y: 1,
                    index: 4
                },
            ])
        );
        assert_eq!(old.diff(&old), Ok(Vec::new()));
    }

    #[test]
    fn diff_rejects_buffers_of_different_sizes() {
        let a = IndexedBuffer::new(2, 3, 0).unwrap();
        let b = IndexedBuffer::new(3, 2, 0).unwrap();

        assert_eq!(a.diff(&b), Err(BufferError::SizeMismatch));
    }

    #[test]
    fn apply_leaves_the_buffer_unchanged_on_error() {
        let mut buffer = IndexedBuffer::new(2, 2, 0).unwrap();
        let changes = [
            PixelChange {
                x: 0,
                y: 0,
                index: 1,
            },
            PixelChange {
                x: 5,
                y: 0,
                index: 1,
            },
        ];

        assert_eq!(
            buffer.apply(&changes),
            Err(BufferError::OutOfBounds { x: 5, y: 0 })
        );
        assert_eq!(buffer.pixels(), &[0, 0, 0, 0]);
    }

    fn two_buffers_of_one_size() -> impl Strategy<Value = (IndexedBuffer, IndexedBuffer)> {
        (1u32..16, 1u32..16).prop_flat_map(|(width, height)| {
            let len = (width * height) as usize;
            (vec(0u8..4, len), vec(0u8..4, len)).prop_map(move |(a, b)| {
                (
                    IndexedBuffer::from_pixels(width, height, a).unwrap(),
                    IndexedBuffer::from_pixels(width, height, b).unwrap(),
                )
            })
        })
    }

    proptest! {
        #[test]
        fn applying_a_diff_gives_the_newer_buffer((old, newer) in two_buffers_of_one_size()) {
            let mut patched = old.clone();
            patched.apply(&old.diff(&newer).unwrap()).unwrap();
            prop_assert_eq!(patched, newer);
        }
    }
}
