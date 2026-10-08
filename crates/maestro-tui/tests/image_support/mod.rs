//! Shared image fixture inputs.
use serde_json::Value;

/// Recorded cases owned by one behavior test.
pub fn cases(name: &str) -> Vec<Value> {
    let parsed = serde_json::from_str::<Vec<Value>>(include_str!("../fixtures/image_cases.json"));
    assert!(parsed.is_ok(), "invalid image corpus: {parsed:?}");
    let cases: Vec<_> = parsed
        .unwrap_or_default()
        .into_iter()
        .filter(|case| case["test"] == name)
        .collect();
    assert!(!cases.is_empty(), "no cases for {name}");
    cases
}
/// Integer field supplied by a fixture.
pub fn number(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|number| u32::try_from(number).ok())
}
/// Pixel geometry from a fixture record.
pub fn dimensions(value: &Value) -> maestro_tui::images::terminal_image::ImageDimensions {
    let width = number(&value["widthPx"]);
    let height = number(&value["heightPx"]);
    assert!(
        width.is_some() && height.is_some(),
        "invalid geometry {value}"
    );
    maestro_tui::images::terminal_image::ImageDimensions {
        width_px: width.unwrap_or_default(),
        height_px: height.unwrap_or_default(),
    }
}
/// Cell geometry from a fixture record.
pub fn cells(value: &Value) -> maestro_tui::images::terminal_image::CellDimensions {
    let dims = dimensions(value);
    maestro_tui::images::terminal_image::CellDimensions {
        width_px: dims.width_px,
        height_px: dims.height_px,
    }
}
/// Capability protocol from its fixture spelling.
pub fn protocol(value: &Value) -> Option<maestro_tui::images::terminal_image::ImageProtocol> {
    use maestro_tui::images::terminal_image::ImageProtocol;
    match value.as_str() {
        Some("kitty") => Some(ImageProtocol::Kitty),
        Some("iterm2") => Some(ImageProtocol::Iterm2),
        _ => None,
    }
}
/// Capability result in the corpus format.
pub fn capability_output(caps: maestro_tui::images::terminal_image::TerminalCapabilities) -> Value {
    let images = caps.images.map(|value| match value {
        maestro_tui::images::terminal_image::ImageProtocol::Kitty => "kitty",
        maestro_tui::images::terminal_image::ImageProtocol::Iterm2 => "iterm2",
    });
    serde_json::json!({"images": images, "trueColor": caps.true_color, "hyperlinks": caps.hyperlinks})
}
/// Corpus representation of an optional image identifier.
pub fn image_id(id: Option<u32>) -> Value {
    id.map_or(serde_json::json!({"undefined": true}), |id| {
        serde_json::json!(id)
    })
}
