use std::fmt;

use crate::Rgba8;

/// The largest number of colors in a palette. Indexed pixels store the index as a `u8`.
pub const MAX_COLORS: usize = 256;

/// An ordered list of 1 to 256 colors. Indexed pixels refer to colors by position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    colors: Vec<Rgba8>,
}

/// Errors from creating a [`Palette`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteError {
    /// The list of colors is empty.
    Empty,
    /// The list has more than [`MAX_COLORS`] colors. Holds the actual count.
    TooManyColors(usize),
}

impl fmt::Display for PaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "palette has no colors"),
            Self::TooManyColors(count) => {
                write!(f, "palette has {count} colors, the limit is {MAX_COLORS}")
            }
        }
    }
}

impl std::error::Error for PaletteError {}

impl Palette {
    /// Creates a palette from a list of colors.
    ///
    /// # Errors
    ///
    /// Returns [`PaletteError::Empty`] for an empty list and
    /// [`PaletteError::TooManyColors`] for more than [`MAX_COLORS`] colors.
    pub fn new(colors: Vec<Rgba8>) -> Result<Self, PaletteError> {
        if colors.is_empty() {
            return Err(PaletteError::Empty);
        }
        if colors.len() > MAX_COLORS {
            return Err(PaletteError::TooManyColors(colors.len()));
        }
        Ok(Self { colors })
    }

    /// Returns the color at `index`, or `None` if the palette is shorter.
    pub fn get(&self, index: u8) -> Option<Rgba8> {
        self.colors.get(usize::from(index)).copied()
    }

    /// Returns the index of the first color equal to `color`, or `None` if the palette does not contain it.
    pub fn index_of(&self, color: Rgba8) -> Option<u8> {
        let position = self.colors.iter().position(|&c| c == color)?;
        u8::try_from(position).ok()
    }

    /// Returns `true` if the palette contains `color`.
    pub fn contains(&self, color: Rgba8) -> bool {
        self.colors.contains(&color)
    }

    /// Returns all colors in order.
    pub fn colors(&self) -> &[Rgba8] {
        &self.colors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgba8 = Rgba8::opaque(255, 0, 0);
    const GREEN: Rgba8 = Rgba8::opaque(0, 255, 0);

    #[test]
    fn rejects_empty_list() {
        assert_eq!(Palette::new(Vec::new()), Err(PaletteError::Empty));
    }

    #[test]
    fn rejects_more_than_256_colors() {
        assert_eq!(
            Palette::new(vec![RED; 257]),
            Err(PaletteError::TooManyColors(257))
        );
    }

    #[test]
    fn accepts_exactly_256_colors() {
        assert!(Palette::new(vec![RED; 256]).is_ok());
    }

    #[test]
    fn finds_colors_by_index_and_by_value() {
        let palette = Palette::new(vec![RED, GREEN]).unwrap();

        assert_eq!(palette.get(1), Some(GREEN));
        assert_eq!(palette.get(2), None);
        assert_eq!(palette.index_of(GREEN), Some(1));
        assert_eq!(palette.index_of(Rgba8::TRANSPARENT), None);
        assert!(palette.contains(RED));
    }
}
