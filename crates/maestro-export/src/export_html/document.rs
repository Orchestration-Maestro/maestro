//! Ordered bundle observations and literal original-slot composition.
use super::{ExportAsset, ExportError, ExportOperations, colors::backgrounds};
use base64::{Engine, engine::general_purpose::STANDARD};
use maestro_json::{canonical_entries, compact_json};
use serde_json::Value;

/// Generate document text from the supplied bundle and two theme observations.
pub(super) fn generate(
    data: &Value,
    theme: Option<&str>,
    operations: &dyn ExportOperations,
) -> Result<String, ExportError> {
    let template = operations.read_asset(ExportAsset::Template)?;
    let css = operations.read_asset(ExportAsset::Css)?;
    let script = operations.read_asset(ExportAsset::JavaScript)?;
    let markdown = operations.read_asset(ExportAsset::Markdown)?;
    let highlighting = operations.read_asset(ExportAsset::Highlight)?;
    let colors = operations.get_resolved_theme_colors(theme)?;
    let derived = backgrounds(&colors, operations.get_theme_export_colors(theme));
    let mut vars: Vec<String> =
        canonical_entries(colors.iter().map(|(key, value)| (key.as_str(), value)))
            .into_iter()
            .map(|(key, value)| format!("--{key}: {value};"))
            .collect();
    vars.extend([
        format!("--exportPageBg: {};", derived.page),
        format!("--exportCardBg: {};", derived.card),
        format!("--exportInfoBg: {};", derived.info),
    ]);
    let colors = operations.get_resolved_theme_colors(theme)?;
    let derived = backgrounds(&colors, operations.get_theme_export_colors(theme));
    let payload = STANDARD.encode(compact_json(data)?.as_bytes());
    let css = insert(
        &css,
        &[
            ("{{THEME_VARS}}", &vars.join("\n      ")),
            ("{{BODY_BG}}", &derived.page),
            ("{{CONTAINER_BG}}", &derived.card),
            ("{{INFO_BG}}", &derived.info),
        ],
    );
    Ok(insert(
        &template,
        &[
            ("{{CSS}}", &css),
            ("{{JS}}", &script),
            ("{{SESSION_DATA}}", &payload),
            ("{{MARKED_JS}}", &markdown),
            ("{{HIGHLIGHT_JS}}", &highlighting),
        ],
    ))
}
/// Replace each first original slot, never scanning inserted content.
fn insert(source: &str, replacements: &[(&str, &str)]) -> String {
    let mut positions: Vec<_> = replacements
        .iter()
        .filter_map(|&(slot, value)| source.find(slot).map(|start| (start, slot.len(), value)))
        .collect();
    positions.sort_unstable_by_key(|&(start, _, _)| start);
    let mut result = String::new();
    let mut cursor = 0;
    for (start, length, value) in positions {
        result.push_str(&source[cursor..start]);
        result.push_str(value);
        cursor = start + length;
    }
    result.push_str(&source[cursor..]);
    result
}
