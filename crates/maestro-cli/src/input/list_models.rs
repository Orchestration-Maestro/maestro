//! The available-model table with its optional fuzzy search.

use super::pad_column;
use maestro_app::{Model, ModelInput, ModelRegistry, format_no_models_available_message};
use maestro_theme::{Collator, Theme};
use maestro_tui::fuzzy_filter;
use std::cmp::Ordering;
use std::error::Error;
use std::io::{self, Write};

/// One column of the prepared table.
const COLUMNS: [&str; 6] = [
    "provider", "model", "context", "max-out", "thinking", "images",
];

/// The two-space gap between table columns.
const GAP: &str = "  ";

/// Supplied observations for one model listing.
#[derive(Clone, Copy)]
pub struct ListModelsOptions<'a> {
    /// Retained search pattern; no pattern or an empty one lists every model.
    pub search_pattern: Option<&'a str>,
    /// Reads the documentation root of the empty-list guidance.
    pub docs_path: &'a dyn Fn() -> io::Result<String>,
    /// Prepared locale comparator for provider and model names.
    pub collator: &'a Collator,
    /// Whether diagnostics are styled.
    pub styles_enabled: bool,
}

/// One prepared row, owning the text its columns print.
#[derive(Clone)]
struct Row {
    /// Provider name.
    provider: String,
    /// Model identifier.
    model: String,
    /// Formatted context capacity.
    context: String,
    /// Formatted output capacity.
    max_out: String,
    /// Whether reasoning is supported.
    thinking: &'static str,
    /// Whether images are accepted.
    images: &'static str,
}

/// Print every available model, optionally filtered by `search_pattern`.
///
/// Load diagnostics are written to `diagnostics` and the table to `output`.
///
/// # Errors
/// Returns registry, documentation-root or writing failures.
pub fn list_models(
    registry: &ModelRegistry,
    options: ListModelsOptions<'_>,
    output: &mut dyn Write,
    diagnostics: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    if let Some(error) = registry.get_error() {
        let warning = format!("Warning: errors loading models.json:\n{error}");
        writeln!(
            diagnostics,
            "{}",
            Theme::yellow(&warning, options.styles_enabled)
        )?;
    }
    let available = registry.get_available()?;
    if available.is_empty() {
        let docs_path = (options.docs_path)()?;
        writeln!(output, "{}", format_no_models_available_message(&docs_path))?;
        return Ok(());
    }
    let rows: Vec<Row> = available
        .iter()
        .filter_map(|handle| handle.read().ok().map(|model| row(&model)))
        .collect();
    let mut rows = search(&rows, options.search_pattern);
    if rows.is_empty() {
        let pattern = options.search_pattern.unwrap_or_default();
        writeln!(output, "No models matching \"{pattern}\"")?;
        return Ok(());
    }
    rows.sort_by(|left, right| compare(left, right, options.collator));
    write_table(&rows, output)?;
    Ok(())
}

/// Apply the authored search pattern, keeping every row for an absent or empty one.
fn search<'a>(rows: &'a [Row], pattern: Option<&str>) -> Vec<&'a Row> {
    let Some(pattern) = pattern.filter(|pattern| !pattern.is_empty()) else {
        return rows.iter().collect();
    };
    fuzzy_filter(rows, pattern, |row| {
        format!("{} {}", row.provider, row.model)
    })
}

/// Copy the columns the table prints from one descriptor.
fn row(model: &Model) -> Row {
    Row {
        provider: model.provider.clone(),
        model: model.id.clone(),
        context: format_token_count(model.context_window),
        max_out: format_token_count(model.max_tokens),
        thinking: yes_no(model.reasoning),
        images: yes_no(model.input.contains(&ModelInput::Image)),
    }
}

/// Spell a capability as the table's own word.
fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// Order rows by provider, then by model, under the prepared locale.
fn compare(left: &Row, right: &Row, collator: &Collator) -> Ordering {
    match collator.compare(&left.provider, &right.provider) {
        Ordering::Equal => collator.compare(&left.model, &right.model),
        order => order,
    }
}

/// Format a token capacity as a human-readable count.
fn format_token_count(count: f64) -> String {
    for (unit, suffix) in [(1_000_000.0, "M"), (1_000.0, "K")] {
        if count >= unit {
            let scaled = count / unit;
            let text = if scaled.fract() == 0.0 {
                format!("{scaled}")
            } else {
                ryu_js::Buffer::new().format_to_fixed(scaled, 1).to_owned()
            };
            return format!("{text}{suffix}");
        }
    }
    if count == 0.0 {
        "0".to_owned()
    } else {
        format!("{count}")
    }
}

/// The printed cells of one row, in column order.
fn cells(row: &Row) -> [&str; 6] {
    [
        &row.provider,
        &row.model,
        &row.context,
        &row.max_out,
        row.thinking,
        row.images,
    ]
}

/// Print the header and every row with their measured columns.
fn write_table(rows: &[&Row], output: &mut dyn Write) -> io::Result<()> {
    let mut widths = COLUMNS.map(|header| header.encode_utf16().count());
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(cells(row)) {
            *width = (*width).max(cell.encode_utf16().count());
        }
    }
    writeln!(output, "{}", line(&COLUMNS, &widths))?;
    for row in rows {
        writeln!(output, "{}", line(&cells(row), &widths))?;
    }
    Ok(())
}

/// Pad every cell to its column and join them with the column gap.
fn line(cells: &[&str; 6], widths: &[usize; 6]) -> String {
    cells
        .iter()
        .zip(widths)
        .map(|(cell, width)| pad_column(cell, *width))
        .collect::<Vec<String>>()
        .join(GAP)
}
