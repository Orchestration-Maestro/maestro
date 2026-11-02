//! Authored observations of retained overlay capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionResult, OverlayAnchor, OverlayMargin, OverlayMarginValue, OverlayOptions,
    OverlayOptionsHandle, SizeValue,
};

/// Keeps one options owner while replacing every placement field.
pub fn options() -> ExtensionResult<String> {
    let first = OverlayOptions {
        width: Some(SizeValue::Cells(19)),
        min_width: Some(7),
        max_height: Some(SizeValue::Percentage("62.5%".into())),
        anchor: Some(OverlayAnchor::BottomRight),
        offset_x: Some(-2),
        offset_y: Some(3),
        row: Some(SizeValue::Percentage("25%".into())),
        col: Some(SizeValue::Cells(4)),
        margin: Some(OverlayMarginValue::Sides(OverlayMargin {
            top: Some(1),
            right: Some(2),
            bottom: Some(3),
            left: Some(-4),
        })),
        non_capturing: Some(false),
    };
    let options = OverlayOptionsHandle::new(first.clone())?;
    let alias = options.clone();
    assert_eq!(options.get()?, first);
    let second = OverlayOptions {
        width: Some(SizeValue::Percentage("75%".into())),
        margin: Some(OverlayMarginValue::Uniform(2)),
        non_capturing: Some(true),
        ..OverlayOptions::default()
    };
    alias.set(second.clone())?;
    assert_eq!(options.get()?, second);
    options.set(OverlayOptions::default())?;
    assert_eq!(alias.get()?, OverlayOptions::default());
    Ok("overlay options shared".into())
}
