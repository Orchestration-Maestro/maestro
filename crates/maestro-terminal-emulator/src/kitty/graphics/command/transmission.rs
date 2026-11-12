//! Image transmission records.

use super::ParseError;
/// Image transmission parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transmission {
    /// Format.
    pub format: Format,
    /// Medium.
    pub medium: Medium,
    /// Declared width in pixels.
    pub width: u32,
    /// Declared height in pixels.
    pub height: u32,
    /// Maximum selected local-medium byte count; zero uses its default.
    pub size: u32,
    /// Local-medium byte offset.
    pub offset: u32,
    /// Image id.
    pub image_id: u32,
    /// Image number.
    pub image_number: u32,
    /// Placement id.
    pub placement_id: u32,
    /// Compression.
    pub compression: Compression,
    /// Whether subsequent direct chunks follow this command.
    pub more_chunks: bool,
}

/// Pixel encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Format {
    /// Three eight-bit red, green and blue channels.
    Rgb,
    /// Four eight-bit red, green, blue and alpha channels.
    #[default]
    Rgba,
    /// Encoded PNG, decoded before raw-pixel validation.
    Png,
    /// Eight-bit grayscale and alpha channels.
    GrayAlpha,
    /// One eight-bit grayscale channel.
    Gray,
}

/// Image data location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Medium {
    /// Bytes carried directly in command payloads.
    #[default]
    Direct,
    /// A regular file retained after loading.
    File,
    /// An admitted temporary file removed after its read operation.
    TemporaryFile,
    /// A native named segment, unlinked after opening.
    SharedMemory,
}

/// Payload compression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Compression {
    /// Uncompressed input.
    #[default]
    None,
    /// Zlib-wrapped deflate input.
    ZlibDeflate,
}

impl Transmission {
    /// Project the selected control fields into their typed record.
    pub(super) fn parse(keys: &super::parser::Keys) -> Result<Self, ParseError> {
        let medium = match u8::try_from(keys.get(b't', u32::from(b'd')))
            .map_err(|_| ParseError::InvalidFormat)?
        {
            b'd' => Medium::Direct,
            b'f' => Medium::File,
            b't' => Medium::TemporaryFile,
            b's' => Medium::SharedMemory,
            _ => return Err(ParseError::InvalidFormat),
        };
        Ok(Self {
            width: keys.get(b's', 0),
            height: keys.get(b'v', 0),
            size: keys.get(b'S', 0),
            offset: keys.get(b'O', 0),
            image_id: keys.get(b'i', 0),
            image_number: keys.get(b'I', 0),
            placement_id: keys.get(b'p', 0),
            format: match keys.get(b'f', 32) {
                24 => Format::Rgb,
                32 => Format::Rgba,
                100 => Format::Png,
                _ => return Err(ParseError::InvalidFormat),
            },
            medium,
            compression: match keys.optional(b'o') {
                None => Compression::None,
                Some(value) if value == u32::from(b'z') => Compression::ZlibDeflate,
                _ => return Err(ParseError::InvalidFormat),
            },
            more_chunks: medium == Medium::Direct && keys.get(b'm', 0) > 0,
        })
    }
}

impl Transmission {
    /// Raw bytes per pixel; PNG must be decoded first.
    #[must_use]
    pub fn format_bpp(format: Format) -> Option<u8> {
        match format {
            Format::Rgb => Some(3),
            Format::Rgba => Some(4),
            Format::Gray => Some(1),
            Format::GrayAlpha => Some(2),
            Format::Png => None,
        }
    }
}
