//! Local demonstration styles resolved from the embedded brand pack.

use serde::Deserialize;
use std::{collections::BTreeMap, io, rc::Rc};

/// Foreground styling shared by demonstration components.
pub(super) type Color = Rc<dyn Fn(&str) -> String>;
/// Trusted asset projection; unrelated brand fields remain with their owner.
#[derive(Deserialize)]
struct Pack {
    /// Named RGB swatches.
    palette: BTreeMap<String, String>,
    /// Presentation modes.
    modes: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>,
}
/// Resolves a dark-mode foreground without a fallback palette.
pub(super) fn color(role: &str) -> io::Result<Color> {
    let pack: Pack = serde_json::from_str(include_str!("../../../../assets/brand/brand.json"))?;
    let swatch = pack
        .modes
        .get("dark")
        .and_then(|mode| mode.get("colors"))
        .and_then(|colors| colors.get(role))
        .and_then(|name| pack.palette.get(name))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing brand color role"))?;
    let rgb = swatch
        .strip_prefix('#')
        .filter(|hex| hex.len() == 6)
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid brand color"))?;
    let prefix = format!(
        "\x1b[38;2;{};{};{}m",
        rgb >> 16,
        (rgb >> 8) & 255,
        rgb & 255
    );
    Ok(Rc::new(move |text| decorate(text, &prefix, "\x1b[39m")))
}
/// Wraps existing text, reopening a decoration after nested resets.
pub(super) fn decorate(text: &str, open: &str, close: &str) -> String {
    format!("{open}{}{close}", text.replace(close, open))
}
