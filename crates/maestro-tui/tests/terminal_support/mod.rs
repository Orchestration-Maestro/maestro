//! Terminal helper fixture conversion.
use super::image_support::{number, protocol};
use serde_json::Value;
/// Fixture boolean or its source default.
pub fn enabled(value: &Value) -> bool {
    value.as_bool().unwrap_or(true)
}
/// Serialized dimension result.
pub fn dimension_output(
    value: Option<maestro_tui::images::terminal_image::ImageDimensions>,
) -> Value {
    value.map_or(
        Value::Null,
        |dims| serde_json::json!({"widthPx": dims.width_px, "heightPx": dims.height_px}),
    )
}
/// Capability record supplied by a fixture.
pub fn capabilities(value: &Value) -> maestro_tui::images::terminal_image::TerminalCapabilities {
    maestro_tui::images::terminal_image::TerminalCapabilities {
        images: protocol(&value["images"]),
        true_color: enabled(&value["trueColor"]),
        hyperlinks: enabled(&value["hyperlinks"]),
    }
}
/// Kitty options with source defaults.
pub fn kitty_options(value: &Value) -> maestro_tui::images::terminal_image::KittyOptions {
    if value.as_object().is_none_or(serde_json::Map::is_empty) {
        return maestro_tui::images::terminal_image::KittyOptions::default();
    }
    maestro_tui::images::terminal_image::KittyOptions {
        columns: number(&value["columns"]),
        rows: value["rows"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok()),
        image_id: number(&value["imageId"]),
        move_cursor: enabled(&value["moveCursor"]),
    }
}
/// Render options with source defaults.
pub fn render_options(value: &Value) -> maestro_tui::images::terminal_image::ImageRenderOptions {
    maestro_tui::images::terminal_image::ImageRenderOptions {
        max_width_cells: number(&value["maxWidthCells"]),
        image_id: number(&value["imageId"]),
        move_cursor: enabled(&value["moveCursor"]),
        preserve_aspect_ratio: enabled(&value["preserveAspectRatio"]),
    }
}
/// Render result with absent identifiers omitted.
pub fn render_output(result: Option<maestro_tui::images::terminal_image::RenderedImage>) -> Value {
    result.map_or(Value::Null, |result| {
        let mut output = serde_json::json!({"sequence": result.sequence, "rows": result.rows});
        if let Some(id) = result.image_id {
            output["imageId"] = serde_json::json!(id);
        }
        output
    })
}
/// Normalizes the corpus's explicit undefined marker to an absent result field.
pub fn render_expected(mut value: Value) -> Value {
    if value["imageId"].is_object()
        && let Some(object) = value.as_object_mut()
    {
        object.remove("imageId");
    }
    value
}
