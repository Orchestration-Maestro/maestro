//! Native brand pack loading and terminal theme adapters.
//!
//! See `docs/theme.md` for native loading and `docs/brand.md` for pure projection.
mod mark;
mod projection;

use super::{ThemeError, ThemeOperations};
use maestro_brand::{BrandError, PackData};
pub use maestro_brand::{
    BrandFont, BrandGlyph, BrandMark, BrandMode, BrandPresentation, BrandType,
};
use maestro_path::{dirname, join};

/// A decoded brand pack: colors, fonts, type, spacing, mark and glyph data.
pub struct BrandPack {
    /// The decoded pack file, with mark paths joined to the pack's directory.
    data: PackData,
}

/// Read and decode the pack at `path`, reading nothing else.
///
/// Mark paths become `path`'s directory joined with the authored relative paths.
///
/// # Errors
/// Returns the read failure, `Invalid brand pack <path>: <cause>` for malformed text or
/// fields, or `Invalid brand measurement: <pointer>` for a type size, spacing or radius out of range.
pub fn load_brand_pack(
    path: &str,
    operations: &dyn ThemeOperations,
) -> Result<BrandPack, ThemeError> {
    let text = operations.read_to_string(path).map_err(ThemeError::io)?;
    let mut data = PackData::parse(&text).map_err(|error| match error {
        BrandError::Decode(cause) => {
            ThemeError::caused_by(format!("Invalid brand pack {path}: {cause}"), cause)
        }
        error => ThemeError::from(error),
    })?;
    let directory = dirname(path);
    let from_pack = |relative: &str| join(&[&directory, relative]);
    data.mark.template = from_pack(&data.mark.template);
    for variant in data.mark.variants.values_mut() {
        *variant = from_pack(variant);
    }
    Ok(BrandPack { data })
}

impl From<BrandError> for ThemeError {
    /// Retain the shared failure as the cause of a theme error.
    fn from(error: BrandError) -> Self {
        Self::caused_by(error.to_string(), error)
    }
}

pub(crate) use projection::syntax_css;
