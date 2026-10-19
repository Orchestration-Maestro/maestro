//! PNG transformation into owned RGBA pixels.
use super::super::command::Format;
use super::{Image, ImageError, MAX_SIZE};
use std::io::Cursor;

/// Decode expanded eight-bit channels, preserving PNG geometry.
pub(super) fn decode_png_rgba(bytes: &[u8]) -> Result<Image, ImageError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|_| ImageError::InvalidData)?;
    // The header sizes the output buffer, and no frame exceeds its canvas: check the
    // four-byte RGBA pixels before allocating.
    let (width, height) = reader.info().size();
    if u64::from(width) * u64::from(height) > MAX_SIZE / 4 {
        return Err(ImageError::InvalidData);
    }
    let mut data = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut data)
        .map_err(|_| ImageError::InvalidData)?;
    data.truncate(info.buffer_size());
    if info.bit_depth != png::BitDepth::Eight {
        return Err(ImageError::InvalidData);
    }
    let data = match info.color_type {
        png::ColorType::Rgba => data,
        png::ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect(),
        png::ColorType::Grayscale => data
            .into_iter()
            .flat_map(|gray| [gray, gray, gray, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|ga| [ga[0], ga[0], ga[0], ga[1]])
            .collect(),
        png::ColorType::Indexed => return Err(ImageError::InvalidData),
    };
    Ok(Image {
        width: info.width,
        height: info.height,
        format: Format::Rgba,
        data,
        ..Image::default()
    })
}
