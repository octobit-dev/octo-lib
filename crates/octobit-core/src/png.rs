//! PNG files. Requires the `png` feature, which is on by default.
//!
//! Indexed PNG keeps palette indices as they are, so a buffer and its palette survive a
//! round trip unchanged. It is also the smallest form of a canvas snapshot. RGBA PNG holds
//! any image, and [`decode_rgba`] reads every kind of PNG.

use std::fmt;
use std::io::Cursor;

use ::png::{BitDepth, ColorType, Decoder, Encoder, Transformations};

use crate::{BufferError, IndexedBuffer, Palette, PaletteError, Rgba8, RgbaBuffer};

/// Largest image the decoder accepts, in pixels: 64 Mi, for example 8192x8192. Larger
/// images are rejected before their pixels are allocated.
const MAX_DECODED_PIXELS: usize = 64 * 1024 * 1024;

/// Errors from reading or writing PNG files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PngError {
    /// A pixel holds an index that the palette does not have.
    IndexOutOfPalette {
        /// Column of the pixel.
        x: u32,
        /// Row of the pixel.
        y: u32,
        /// The index stored in the pixel.
        index: u8,
    },
    /// The PNG does not use indexed color.
    NotIndexed,
    /// The image has more pixels than the decoder accepts (64 Mi).
    TooLarge {
        /// Width of the image.
        width: u32,
        /// Height of the image.
        height: u32,
    },
    /// The palette in the PNG is missing or invalid.
    Palette(PaletteError),
    /// The pixels do not form a valid buffer.
    Buffer(BufferError),
    /// The PNG codec failed, for example on malformed data. Holds its message.
    Codec(String),
}

impl fmt::Display for PngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexOutOfPalette { x, y, index } => {
                write!(f, "pixel ({x}, {y}) has index {index}, outside the palette")
            }
            Self::NotIndexed => write!(f, "the PNG does not use indexed color"),
            Self::TooLarge { width, height } => write!(
                f,
                "a {width}x{height} image has more than {MAX_DECODED_PIXELS} pixels"
            ),
            Self::Palette(error) => write!(f, "invalid PNG palette: {error}"),
            Self::Buffer(error) => write!(f, "invalid PNG pixels: {error}"),
            Self::Codec(message) => write!(f, "PNG codec error: {message}"),
        }
    }
}

impl std::error::Error for PngError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Palette(error) => Some(error),
            Self::Buffer(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PaletteError> for PngError {
    fn from(error: PaletteError) -> Self {
        Self::Palette(error)
    }
}

impl From<BufferError> for PngError {
    fn from(error: BufferError) -> Self {
        Self::Buffer(error)
    }
}

fn codec(error: impl fmt::Display) -> PngError {
    PngError::Codec(error.to_string())
}

/// Encodes a buffer and its palette as an indexed PNG.
///
/// The PNG uses the smallest bit depth that fits the palette: 1 bit for up to 2 colors,
/// 2 bits for up to 4, 4 bits for up to 16 and 8 bits above that. Transparent palette
/// colors are stored in a `tRNS` chunk.
///
/// # Errors
///
/// Returns [`PngError::IndexOutOfPalette`] for the first pixel whose index the palette does
/// not have, and [`PngError::Codec`] if the PNG encoder fails.
pub fn encode_indexed(buffer: &IndexedBuffer, palette: &Palette) -> Result<Vec<u8>, PngError> {
    let colors = palette.colors();
    if let Some(error) = first_index_outside(buffer, colors.len()) {
        return Err(error);
    }

    let depth = bit_depth(colors.len());
    let rgb: Vec<u8> = colors.iter().flat_map(|c| [c.r, c.g, c.b]).collect();
    let mut alpha: Vec<u8> = colors.iter().map(|c| c.a).collect();
    while alpha.last() == Some(&255) {
        alpha.pop();
    }

    let mut png = Vec::new();
    let mut encoder = Encoder::new(&mut png, buffer.width(), buffer.height());
    encoder.set_color(ColorType::Indexed);
    encoder.set_depth(depth);
    encoder.set_palette(rgb);
    if !alpha.is_empty() {
        encoder.set_trns(alpha);
    }
    let mut writer = encoder.write_header().map_err(codec)?;
    writer
        .write_image_data(&pack(buffer, depth))
        .map_err(codec)?;
    writer.finish().map_err(codec)?;
    Ok(png)
}

/// Decodes an indexed PNG into a buffer and its palette, with indices exactly as stored.
///
/// Accepts indexed PNG of every bit depth: 1, 2, 4 and 8 bits per pixel.
///
/// # Errors
///
/// Returns [`PngError::NotIndexed`] for other color types, [`PngError::TooLarge`] for
/// images with more than 64 Mi pixels, [`PngError::Palette`] for a missing or invalid
/// palette, [`PngError::IndexOutOfPalette`] for a pixel outside the palette and
/// [`PngError::Codec`] for malformed data.
pub fn decode_indexed(png: &[u8]) -> Result<(IndexedBuffer, Palette), PngError> {
    let mut reader = Decoder::new(Cursor::new(png)).read_info().map_err(codec)?;
    let info = reader.info();
    if info.color_type != ColorType::Indexed {
        return Err(PngError::NotIndexed);
    }
    let (width, height, depth) = (info.width, info.height, info.bit_depth);
    let palette = palette_from_chunks(info.palette.as_deref(), info.trns.as_deref())?;

    let pixel_count = decoded_pixel_count(width, height)?;
    let raw_len = reader
        .output_buffer_size()
        .ok_or(PngError::TooLarge { width, height })?;
    let mut raw = vec![0; raw_len];
    reader.next_frame(&mut raw).map_err(codec)?;

    let pixels = unpack(&raw, width, pixel_count, depth);
    let buffer = IndexedBuffer::from_pixels(width, height, pixels)?;
    if let Some(error) = first_index_outside(&buffer, palette.colors().len()) {
        return Err(error);
    }
    Ok((buffer, palette))
}

/// Encodes a buffer as an 8-bit RGBA PNG.
///
/// # Errors
///
/// Returns [`PngError::Codec`] if the PNG encoder fails.
pub fn encode_rgba(buffer: &RgbaBuffer) -> Result<Vec<u8>, PngError> {
    let mut png = Vec::new();
    let mut encoder = Encoder::new(&mut png, buffer.width(), buffer.height());
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(codec)?;
    writer.write_image_data(buffer.as_bytes()).map_err(codec)?;
    writer.finish().map_err(codec)?;
    Ok(png)
}

/// Decodes a PNG of any color type and bit depth into an RGBA buffer.
///
/// Indexed and grayscale images are expanded, 16-bit samples keep their high byte and images
/// without alpha become opaque. Colors are taken as stored: gamma and color profile chunks
/// are ignored.
///
/// # Errors
///
/// Returns [`PngError::TooLarge`] for images with more than 64 Mi pixels and
/// [`PngError::Codec`] for malformed data.
pub fn decode_rgba(png: &[u8]) -> Result<RgbaBuffer, PngError> {
    let mut decoder = Decoder::new(Cursor::new(png));
    // ALPHA expands indexed, grayscale and RGB images and adds an alpha channel, so the
    // output is 8-bit RGBA or 8-bit grayscale with alpha.
    decoder.set_transformations(Transformations::ALPHA | Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(codec)?;
    let (width, height) = (reader.info().width, reader.info().height);
    decoded_pixel_count(width, height)?;

    let raw_len = reader
        .output_buffer_size()
        .ok_or(PngError::TooLarge { width, height })?;
    let mut raw = vec![0; raw_len];
    reader.next_frame(&mut raw).map_err(codec)?;

    let bytes = match reader.output_color_type() {
        (ColorType::Rgba, BitDepth::Eight) => raw,
        (ColorType::GrayscaleAlpha, BitDepth::Eight) => raw
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[gray, alpha]| [gray, gray, gray, alpha])
            .collect(),
        other => return Err(codec(format!("unexpected decoder output {other:?}"))),
    };
    Ok(RgbaBuffer::from_bytes(width, height, bytes)?)
}

fn decoded_pixel_count(width: u32, height: u32) -> Result<usize, PngError> {
    (width as usize)
        .checked_mul(height as usize)
        .filter(|&count| count <= MAX_DECODED_PIXELS)
        .ok_or(PngError::TooLarge { width, height })
}

fn palette_from_chunks(rgb: Option<&[u8]>, alpha: Option<&[u8]>) -> Result<Palette, PngError> {
    let alpha = alpha.unwrap_or_default();
    let (rgb, _) = rgb.unwrap_or_default().as_chunks::<3>();
    let colors = rgb
        .iter()
        .enumerate()
        .map(|(i, &[r, g, b])| Rgba8::new(r, g, b, alpha.get(i).copied().unwrap_or(255)))
        .collect();
    Ok(Palette::new(colors)?)
}

fn first_index_outside(buffer: &IndexedBuffer, palette_len: usize) -> Option<PngError> {
    let width = buffer.width() as usize;
    let (position, &index) = buffer
        .pixels()
        .iter()
        .enumerate()
        .find(|&(_, &index)| usize::from(index) >= palette_len)?;
    Some(PngError::IndexOutOfPalette {
        x: (position % width) as u32,
        y: (position / width) as u32,
        index,
    })
}

fn bit_depth(palette_len: usize) -> BitDepth {
    match palette_len {
        0..=2 => BitDepth::One,
        3..=4 => BitDepth::Two,
        5..=16 => BitDepth::Four,
        _ => BitDepth::Eight,
    }
}

/// Packs indices into PNG rows: several pixels per byte below 8 bits, the leftmost pixel in
/// the highest bits, each row padded to a whole byte.
fn pack(buffer: &IndexedBuffer, depth: BitDepth) -> Vec<u8> {
    let bits = depth as usize;
    if bits == 8 {
        return buffer.pixels().to_vec();
    }
    let per_byte = 8 / bits;
    let width = buffer.width() as usize;
    let row_len = width.div_ceil(per_byte);
    let mut packed = vec![0; row_len * buffer.height() as usize];
    for (row, out) in buffer
        .pixels()
        .chunks_exact(width)
        .zip(packed.chunks_exact_mut(row_len))
    {
        for (x, &index) in row.iter().enumerate() {
            let shift = 8 - bits * (x % per_byte + 1);
            out[x / per_byte] |= index << shift;
        }
    }
    packed
}

/// Reverses [`pack`]: one index per byte from packed PNG rows.
fn unpack(raw: &[u8], width: u32, pixel_count: usize, depth: BitDepth) -> Vec<u8> {
    let bits = depth as usize;
    if bits == 8 {
        return raw.to_vec();
    }
    let per_byte = 8 / bits;
    let mask = (1u8 << bits) - 1;
    let width = width as usize;
    let mut pixels = Vec::with_capacity(pixel_count);
    for row in raw.chunks_exact(width.div_ceil(per_byte)) {
        pixels.extend((0..width).map(|x| {
            let shift = 8 - bits * (x % per_byte + 1);
            (row[x / per_byte] >> shift) & mask
        }));
    }
    pixels
}

#[cfg(test)]
mod tests {
    use proptest::collection::vec;
    use proptest::prelude::*;

    use super::*;

    fn raw_png(
        size: (u32, u32),
        color: ColorType,
        depth: BitDepth,
        chunks: (&[u8], &[u8]),
        data: &[u8],
    ) -> Vec<u8> {
        let (palette, trns) = chunks;
        let mut png = Vec::new();
        let mut encoder = Encoder::new(&mut png, size.0, size.1);
        encoder.set_color(color);
        encoder.set_depth(depth);
        if !palette.is_empty() {
            encoder.set_palette(palette.to_vec());
        }
        if !trns.is_empty() {
            encoder.set_trns(trns.to_vec());
        }
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
        writer.finish().unwrap();
        png
    }

    fn png_info(png: &[u8]) -> (BitDepth, bool) {
        let reader = Decoder::new(Cursor::new(png)).read_info().unwrap();
        (reader.info().bit_depth, reader.info().trns.is_some())
    }

    fn palette_of(len: usize) -> Palette {
        Palette::new((0..len).map(|i| Rgba8::opaque(i as u8, 0, 0)).collect()).unwrap()
    }

    #[test]
    fn uses_the_smallest_bit_depth_for_the_palette() {
        let buffer = IndexedBuffer::new(5, 3, 0).unwrap();

        for (len, depth) in [
            (2, BitDepth::One),
            (4, BitDepth::Two),
            (16, BitDepth::Four),
            (17, BitDepth::Eight),
            (256, BitDepth::Eight),
        ] {
            let png = encode_indexed(&buffer, &palette_of(len)).unwrap();
            assert_eq!(png_info(&png).0, depth, "palette of {len} colors");
        }
    }

    #[test]
    fn writes_transparency_only_when_needed() {
        let buffer = IndexedBuffer::new(2, 2, 0).unwrap();
        let opaque = palette_of(3);
        let transparent = Palette::new(vec![Rgba8::TRANSPARENT, Rgba8::opaque(1, 2, 3)]).unwrap();

        assert!(!png_info(&encode_indexed(&buffer, &opaque).unwrap()).1);
        assert!(png_info(&encode_indexed(&buffer, &transparent).unwrap()).1);
    }

    #[test]
    fn encode_rejects_indices_outside_the_palette() {
        let mut buffer = IndexedBuffer::new(3, 2, 0).unwrap();
        buffer.set(1, 1, 2).unwrap();

        assert_eq!(
            encode_indexed(&buffer, &palette_of(2)),
            Err(PngError::IndexOutOfPalette {
                x: 1,
                y: 1,
                index: 2
            })
        );
    }

    #[test]
    fn decode_rejects_indices_outside_the_palette() {
        let palette = [0, 0, 0, 255, 255, 255];
        let png = raw_png(
            (2, 1),
            ColorType::Indexed,
            BitDepth::Eight,
            (&palette, &[]),
            &[1, 5],
        );

        assert_eq!(
            decode_indexed(&png),
            Err(PngError::IndexOutOfPalette {
                x: 1,
                y: 0,
                index: 5
            })
        );
    }

    #[test]
    fn decode_rejects_other_color_types() {
        let png = raw_png(
            (1, 1),
            ColorType::Rgb,
            BitDepth::Eight,
            (&[], &[]),
            &[1, 2, 3],
        );

        assert_eq!(decode_indexed(&png), Err(PngError::NotIndexed));
    }

    #[test]
    fn decode_rejects_malformed_data() {
        assert!(matches!(
            decode_indexed(b"not a png"),
            Err(PngError::Codec(_))
        ));
    }

    #[test]
    fn decode_rejects_images_above_the_pixel_limit() {
        assert_eq!(decoded_pixel_count(8192, 8192), Ok(8192 * 8192));
        assert_eq!(
            decoded_pixel_count(8192, 8193),
            Err(PngError::TooLarge {
                width: 8192,
                height: 8193
            })
        );
    }

    #[test]
    fn encodes_rgba_as_8_bit_rgba() {
        let buffer = RgbaBuffer::new(3, 2, Rgba8::new(1, 2, 3, 4)).unwrap();
        let png = encode_rgba(&buffer).unwrap();
        let reader = Decoder::new(Cursor::new(&png[..])).read_info().unwrap();

        assert_eq!(
            (reader.info().color_type, reader.info().bit_depth),
            (ColorType::Rgba, BitDepth::Eight)
        );
    }

    #[test]
    fn decodes_every_color_type_to_rgba() {
        let none: (&[u8], &[u8]) = (&[], &[]);
        // Color type, bit depth, palette and tRNS, image data, expected RGBA of 2x1 pixels.
        type Case<'a> = (ColorType, BitDepth, (&'a [u8], &'a [u8]), &'a [u8], [u8; 8]);
        let cases: [Case; 6] = [
            (
                ColorType::Grayscale,
                BitDepth::One,
                none,
                &[0b1000_0000],
                [255, 255, 255, 255, 0, 0, 0, 255],
            ),
            (
                ColorType::Grayscale,
                BitDepth::Eight,
                none,
                &[10, 20],
                [10, 10, 10, 255, 20, 20, 20, 255],
            ),
            (
                ColorType::GrayscaleAlpha,
                BitDepth::Eight,
                none,
                &[10, 0, 20, 128],
                [10, 10, 10, 0, 20, 20, 20, 128],
            ),
            (
                ColorType::Rgb,
                BitDepth::Sixteen,
                none,
                &[1, 99, 2, 99, 3, 99, 4, 99, 5, 99, 6, 99],
                [1, 2, 3, 255, 4, 5, 6, 255],
            ),
            (
                ColorType::Rgba,
                BitDepth::Eight,
                none,
                &[1, 2, 3, 4, 5, 6, 7, 8],
                [1, 2, 3, 4, 5, 6, 7, 8],
            ),
            (
                ColorType::Indexed,
                BitDepth::Two,
                (&[1, 2, 3, 4, 5, 6], &[0]),
                &[0b0001_0000],
                [1, 2, 3, 0, 4, 5, 6, 255],
            ),
        ];

        for (color, depth, chunks, data, expected) in cases {
            let png = raw_png((2, 1), color, depth, chunks, data);
            let buffer = decode_rgba(&png).unwrap();
            assert_eq!(buffer.as_bytes(), &expected, "{color:?} at {depth:?}");
        }
    }

    fn rgba_buffer() -> impl Strategy<Value = RgbaBuffer> {
        (1u32..20, 1u32..20).prop_flat_map(|(width, height)| {
            vec(any::<u8>(), (width * height * 4) as usize)
                .prop_map(move |bytes| RgbaBuffer::from_bytes(width, height, bytes).unwrap())
        })
    }

    fn buffer_and_palette() -> impl Strategy<Value = (IndexedBuffer, Palette)> {
        let colors = vec(any::<[u8; 4]>(), 1..=256);
        (1u32..20, 1u32..20, colors).prop_flat_map(|(width, height, colors)| {
            let len = (width * height) as usize;
            let max_index = (colors.len() - 1) as u8;
            vec(0..=max_index, len).prop_map(move |pixels| {
                let colors = colors.iter().map(|&[r, g, b, a]| Rgba8::new(r, g, b, a));
                (
                    IndexedBuffer::from_pixels(width, height, pixels).unwrap(),
                    Palette::new(colors.collect()).unwrap(),
                )
            })
        })
    }

    proptest! {
        #[test]
        fn round_trip_keeps_buffer_and_palette((buffer, palette) in buffer_and_palette()) {
            let png = encode_indexed(&buffer, &palette).unwrap();
            prop_assert_eq!(decode_indexed(&png), Ok((buffer, palette)));
        }

        #[test]
        fn rgba_round_trip_keeps_every_byte(buffer in rgba_buffer()) {
            let png = encode_rgba(&buffer).unwrap();
            prop_assert_eq!(decode_rgba(&png), Ok(buffer));
        }
    }
}
