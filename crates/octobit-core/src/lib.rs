//! Core types of octo-lib: colors, palettes and pixel buffers.
//!
//! A canvas is an [`IndexedBuffer`] of palette indices. A snapshot is its pixels plus its
//! size, and [`IndexedBuffer::diff`] lists the pixels that changed between two snapshots.
//!
//! ```
//! use octobit_core::{IndexedBuffer, Palette, PixelChange, Rgba8};
//!
//! let white = Rgba8::opaque(255, 255, 255);
//! let black = Rgba8::opaque(0, 0, 0);
//! let palette = Palette::new(vec![white, black])?;
//!
//! let mut canvas = IndexedBuffer::new(16, 16, 0)?;
//! let before = canvas.clone();
//!
//! // A placed pixel arrives as a color and is stored as its palette index.
//! let index = palette.index_of(black).expect("black is in the palette");
//! canvas.set(3, 4, index)?;
//!
//! assert_eq!(before.diff(&canvas)?, vec![PixelChange { x: 3, y: 4, index: 1 }]);
//!
//! // Restore a snapshot from its raw bytes.
//! let restored = IndexedBuffer::from_pixels(16, 16, canvas.pixels().to_vec())?;
//! assert_eq!(restored, canvas);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod color;
mod indexed;
mod palette;

pub use color::Rgba8;
pub use indexed::{BufferError, IndexedBuffer, PixelChange};
pub use palette::{MAX_COLORS, Palette, PaletteError};
