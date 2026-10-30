//! Core types of octo-lib: colors, palettes and pixel buffers.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod color;
mod palette;

pub use color::Rgba8;
pub use palette::{MAX_COLORS, Palette, PaletteError};
