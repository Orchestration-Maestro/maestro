//! The selected brand pack and its projections into themes, presentation data, CSS and marks.
//!
//! See `docs/theme.md` for the pack shape, the projections and their errors.
mod css;
mod data;
mod mark;
mod projection;

use super::{ThemeError, ThemeOperations};
use data::PackData;
pub use data::{BrandFont, BrandMark, BrandType};
use maestro_path::{dirname, join};
pub use projection::{BrandGlyph, BrandPresentation};

/// The two shipped appearances a pack supplies colors for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrandMode {
    /// The dark appearance.
    Dark,
    /// The light appearance.
    Light,
}

impl BrandMode {
    /// The key of this mode in a pack and the name of its shipped theme.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

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
/// fields, or `Invalid brand measurement: <pointer>` for a size or spacing out of range.
pub fn load_brand_pack(
    path: &str,
    operations: &dyn ThemeOperations,
) -> Result<BrandPack, ThemeError> {
    let text = operations.read_to_string(path).map_err(ThemeError::io)?;
    let mut data = PackData::parse(path, &text)?;
    data.check_measurements()?;
    let directory = dirname(path);
    let from_pack = |relative: &str| join(&[&directory, relative]);
    data.mark.template = from_pack(&data.mark.template);
    for variant in data.mark.variants.values_mut() {
        *variant = from_pack(variant);
    }
    Ok(BrandPack { data })
}
