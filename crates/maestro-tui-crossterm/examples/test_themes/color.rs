//! Local demonstration styles resolved from the embedded brand pack.

use maestro_brand::{BrandMode, PackData, role_color};
use std::{io, rc::Rc};

/// Foreground styling shared by demonstration components.
pub(super) type Color = Rc<dyn Fn(&str) -> String>;
/// Captures an owned dark-mode foreground prefix for the requested brand role.
/// Brand admission is owned by the [brand library](../../../../docs/brand.md).
pub(super) fn color(role: &str) -> io::Result<Color> {
    let invalid_data = |error| io::Error::new(io::ErrorKind::InvalidData, error);
    let pack = PackData::parse(include_str!("../../../../assets/brand/brand.json"))
        .map_err(invalid_data)?;
    let presentation = pack.presentation(BrandMode::Dark).map_err(invalid_data)?;
    let swatch = role_color(&presentation.colors, role).map_err(invalid_data)?;
    let rgb = u32::from_str_radix(&swatch[1..], 16)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let prefix = format!(
        "\x1b[38;2;{};{};{}m",
        rgb >> 16,
        (rgb >> 8) & 255,
        rgb & 255
    );
    Ok(Rc::new(move |text| decorate(text, &prefix, "\x1b[39m")))
}
/// Wraps existing text, reopening a decoration after matching closing sequences.
pub(super) fn decorate(text: &str, open: &str, close: &str) -> String {
    format!("{open}{}{close}", text.replace(close, open))
}
