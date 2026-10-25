//! Embedded brand input for browser qualification.
use maestro_theme::{BrandPack, ThemeError};

/// Decode a checked-in pack with no filesystem access: a `signal.json` path selects the
/// second pack, any other path the shared one; `path` places the marks and labels errors.
///
/// # Errors
/// Returns the pack's decode failure.
pub fn pack(path: &str) -> Result<BrandPack, ThemeError> {
    let text = if path.ends_with("signal.json") {
        include_str!("../../../../assets/brand/packs/signal.json")
    } else {
        include_str!("../../../../assets/brand/brand.json")
    };
    BrandPack::parse(path, text)
}
