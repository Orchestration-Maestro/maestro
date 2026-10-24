//! Owned image accumulation, decoding and completion.

mod file;
mod png;
mod shared_memory;
#[cfg(test)]
mod tests;

use super::command::{Command, Compression, Display, Format, Medium, Quiet, Transmission};

/// Maximum decoded width or height.
const MAX_DIMENSION: u32 = 10_000;
/// Maximum direct, file, inflated or decoded-pixel byte length.
const MAX_SIZE: u64 = 400 * 1024 * 1024;

/// First-command metadata and data retained across transmission chunks.
#[derive(Debug)]
pub struct LoadingImage {
    /// Initial image metadata, updated by decoding.
    pub image: Image,
    /// Accumulated input or transformed pixels.
    pub data: Vec<u8>,
    /// Display parameters retained from the first command.
    pub display: Option<Display>,
    /// Response policy retained from the first command.
    pub quiet: Quiet,
}

/// Independently enabled local image media; direct data ignores these flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Permit regular files.
    pub file: bool,
    /// Permit admitted temporary files.
    pub temporary_file: bool,
    /// Permit supported native shared-memory objects.
    pub shared_memory: bool,
}
impl Limits {
    /// Enable every supported local medium.
    pub const ALL: Self = Self {
        file: true,
        temporary_file: true,
        shared_memory: true,
    };
    /// Disable all local media, retaining direct transmissions.
    pub const DIRECT: Self = Self {
        file: false,
        temporary_file: false,
        shared_memory: false,
    };
}

/// Image metadata with owned bytes; completion produces validated raw pixels.
#[derive(Debug, PartialEq, Eq)]
pub struct Image {
    /// Protocol image identifier.
    pub id: u32,
    /// Protocol image number.
    pub number: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel encoding.
    pub format: Format,
    /// Compression of the current bytes.
    pub compression: Compression,
    /// Owned image bytes.
    pub data: Vec<u8>,
    /// Storage-assigned change stamp; zero before storage.
    pub generation: u64,
    /// Whether execution assigned an identifier implicitly.
    pub implicit_id: bool,
}
impl Default for Image {
    fn default() -> Self {
        Self {
            id: 0,
            number: 0,
            width: 0,
            height: 0,
            format: Format::Rgb,
            compression: Compression::None,
            data: Vec::new(),
            generation: 0,
            implicit_id: false,
        }
    }
}

/// Image loading or completion failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    /// Invalid media, range, encoded image or pixel length.
    InvalidData,
    /// Invalid, incomplete or excessive inflated stream.
    DecompressionFailed,
    /// Width or height is zero at completion.
    DimensionsRequired,
    /// Width or height exceeds the image dimension limit.
    DimensionsTooLarge,
    /// The native path cannot be represented within its path limit.
    FilePathTooLong,
    /// Temporary file resolves outside the canonical temporary roots.
    TemporaryFileNotInTempDir,
    /// Temporary file path lacks the protocol name marker.
    TemporaryFileNotNamedCorrectly,
    /// Pixel format is unsupported by a consumer.
    UnsupportedFormat,
    /// Medium is disabled or unsupported on this platform.
    UnsupportedMedium,
    /// Pixel channel depth is unsupported by a consumer.
    UnsupportedDepth,
}
impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ImageError {}

impl LoadingImage {
    /// Own first-command metadata and load its initial bytes.
    ///
    /// Linux shared memory uses native names. Unlink is attempted for every
    /// successfully opened segment on return, including errors; disabled media are
    /// never opened.
    ///
    /// # Errors
    /// Returns an invalid command, disabled medium or media-read failure.
    pub fn init(cmd: &Command, limits: Limits) -> Result<Self, ImageError> {
        let t = cmd.transmission().ok_or(ImageError::InvalidData)?;
        let mut result = Self {
            image: Image {
                id: t.image_id,
                number: t.image_number,
                width: t.width,
                height: t.height,
                format: t.format,
                compression: t.compression,
                ..Image::default()
            },
            data: Vec::new(),
            display: cmd.display().copied(),
            quiet: cmd.quiet,
        };
        result.data = match t.medium {
            Medium::Direct => {
                result.add_data(&cmd.data)?;
                return Ok(result);
            }
            Medium::File if limits.file => file::read(&cmd.data, t)?,
            Medium::TemporaryFile if limits.temporary_file => file::read(&cmd.data, t)?,
            Medium::SharedMemory if limits.shared_memory => shared_memory::read(&cmd.data, t)?,
            _ => return Err(ImageError::UnsupportedMedium),
        };
        Ok(result)
    }
    /// Append direct bytes under the image-data limit.
    ///
    /// # Errors
    /// Returns `InvalidData` without appending if the limit would be exceeded.
    pub fn add_data(&mut self, data: &[u8]) -> Result<(), ImageError> {
        if u64::try_from(data.len()).unwrap_or(u64::MAX)
            > MAX_SIZE.saturating_sub(u64::try_from(self.data.len()).unwrap_or(u64::MAX))
        {
            return Err(ImageError::InvalidData);
        }
        self.data.extend_from_slice(data);
        Ok(())
    }
    /// Inflate, decode, validate and transfer pixels, resetting image/data only.
    ///
    /// # Errors
    /// Returns the first codec, dimension or raw-length failure.
    pub fn complete(&mut self) -> Result<Image, ImageError> {
        if self.image.compression == Compression::ZlibDeflate {
            self.decompress()?;
        }
        if self.image.format == Format::Png {
            let decoded = png::decode_png_rgba(&self.data)?;
            self.image.width = decoded.width;
            self.image.height = decoded.height;
            self.image.format = Format::Rgba;
            self.data = decoded.data;
        }
        if self.image.width == 0 || self.image.height == 0 {
            return Err(ImageError::DimensionsRequired);
        }
        if self.image.width > MAX_DIMENSION || self.image.height > MAX_DIMENSION {
            return Err(ImageError::DimensionsTooLarge);
        }
        let bpp =
            Transmission::format_bpp(self.image.format).ok_or(ImageError::UnsupportedFormat)?;
        let expected = u64::from(self.image.width) * u64::from(self.image.height) * u64::from(bpp);
        if u64::try_from(self.data.len()).unwrap_or(u64::MAX) != expected {
            return Err(ImageError::InvalidData);
        }
        let mut image = std::mem::take(&mut self.image);
        image.data = std::mem::take(&mut self.data);
        Ok(image)
    }
    /// Inflate one complete zlib stream, accepting bytes beyond its end.
    fn decompress(&mut self) -> Result<(), ImageError> {
        let mut decoder = flate2::Decompress::new(true);
        let mut data = Vec::new();
        let mut buffer = vec![0; 32 * 1024];
        loop {
            let before = (decoder.total_in(), decoder.total_out());
            let input = usize::try_from(before.0).map_err(|_| ImageError::DecompressionFailed)?;
            let status = decoder
                .decompress(
                    &self.data[input..],
                    &mut buffer,
                    flate2::FlushDecompress::None,
                )
                .map_err(|_| ImageError::DecompressionFailed)?;
            if decoder.total_out() > MAX_SIZE {
                return Err(ImageError::DecompressionFailed);
            }
            let count = usize::try_from(decoder.total_out() - before.1)
                .map_err(|_| ImageError::DecompressionFailed)?;
            data.extend_from_slice(&buffer[..count]);
            if status == flate2::Status::StreamEnd {
                break;
            }
            if before == (decoder.total_in(), decoder.total_out()) {
                return Err(ImageError::DecompressionFailed);
            }
        }
        self.data = data;
        self.image.compression = Compression::None;
        Ok(())
    }
}
