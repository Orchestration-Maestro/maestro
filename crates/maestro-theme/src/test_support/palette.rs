//! Recorded theme preparation.
use crate::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeOptions};
use std::rc::Rc;
/// Synthetic palettes use the same key order as the shipped theme.
///
/// # Errors
/// Returns failures from the supplied values or collaborators.
pub fn palette(variant: &str) -> Result<Rc<Theme>, Box<dyn std::error::Error>> {
    if matches!(variant, "dark" | "light") {
        return Ok(Rc::new(crate::load_theme_from_path(
            &format!("{}/assets/theme/{variant}.json", env!("CARGO_MANIFEST_DIR")),
            Some(ColorMode::Truecolor),
        )?));
    }
    let doc: serde_json::Value =
        serde_json::from_str(include_str!("../../assets/theme/dark.json"))?;
    let mut fg = Vec::new();
    let mut bg = Vec::new();
    for (index, (key, _)) in doc["colors"]
        .as_object()
        .ok_or("theme colors are an object")?
        .iter()
        .enumerate()
    {
        let rgb = if variant == "a" {
            0x0020_4060 + index * 0x0002_0101
        } else {
            0x0080_6040 + index * 0x0001_0102
        };
        let color = ColorValue::String(format!("#{rgb:06x}"));
        if matches!(
            key.as_str(),
            "selectedBg"
                | "userMessageBg"
                | "customMessageBg"
                | "toolPendingBg"
                | "toolSuccessBg"
                | "toolErrorBg"
        ) {
            bg.push((ThemeBg::Named(key.clone()), color));
        } else {
            fg.push((ThemeColor::Named(key.clone()), color));
        }
    }
    Ok(Rc::new(Theme::new(
        fg,
        bg,
        ColorMode::Truecolor,
        ThemeOptions::default(),
    )?))
}
