//! Offline document header widgets.
#[cfg(target_arch = "wasm32")]
mod browser;
use super::{SessionDocument, document::invalid};
use crate::browser::attachment_content::{color, style};
#[cfg(target_arch = "wasm32")]
pub use browser::localize_header_date;
use maestro_app::is_truthy;
use maestro_theme::BrandPresentation;
use serde_json::Value;
use std::{collections::BTreeSet, io};

/// Render a count with thousand and million suffixes.
#[must_use]
pub fn format_tokens(count: f64) -> String {
    if count < 1_000_000.0 {
        crate::utils::format::format_token_count(count)
    } else {
        format!(
            "{}M",
            ryu_js::Buffer::new().format_to_fixed(count / 1_000_000.0, 1)
        )
    }
}

/// An activated header control, to be handled by the caller.
#[derive(Debug, PartialEq, Eq)]
pub enum HeaderAction {
    /// Request thinking visibility changes.
    ToggleThinking,
    /// Request tool-result visibility changes.
    ToggleTools,
    /// Request the original session download.
    DownloadSessionJson,
}
/// Expansion retained by the caller for one displayed document.
#[derive(Default)]
pub struct HeaderState {
    /// Whether the complete prompt is visible.
    pub(super) prompt: bool,
    /// Expanded tool positions, independent of tool names.
    pub(super) tools: BTreeSet<usize>,
}
/// Borrowed presentation and native date effect for one draw.
#[derive(Clone, Copy)]
pub struct HeaderView<'a> {
    /// Caller-selected brand projection.
    pub presentation: &'a BrandPresentation<'a>,
    /// Localized date operation, called only for a truthy timestamp.
    pub localize_date: &'a dyn Fn(&Value) -> io::Result<String>,
}
/// Draw the offline document header and return activated control requests.
/// # Errors
/// Propagates presentation, date and reached text/schema admission failures.
pub fn render_header(
    ui: &mut egui::Ui,
    document: &SessionDocument,
    state: &mut HeaderState,
    view: HeaderView<'_>,
) -> io::Result<Vec<HeaderAction>> {
    ui.scope(|ui| {
        let p = view.presentation;
        style(ui, p)?;
        let data = &document.data;
        let header = &data.get("header").unwrap_or(&Value::Null);
        text(
            ui,
            p,
            format!("Session: {}", fallback(literal(&header["id"])?, "unknown")),
            "accent",
        )?;
        text(ui, p, "T toggle thinking · O toggle tools", "muted")?;
        let mut actions = Vec::new();
        for (label, hint, action) in [
            (
                "Toggle thinking",
                "Toggle thinking (T)",
                HeaderAction::ToggleThinking,
            ),
            (
                "Toggle tools",
                "Toggle tools (O)",
                HeaderAction::ToggleTools,
            ),
            (
                "↓ JSONL",
                "Download session as JSONL",
                HeaderAction::DownloadSessionJson,
            ),
        ] {
            if ui.button(label).on_hover_text(hint).clicked() {
                actions.push(action);
            }
        }
        let timestamp = &header["timestamp"];
        let localized = if is_truthy(timestamp) {
            (view.localize_date)(timestamp)?
        } else {
            "unknown".into()
        };
        for (label, value) in metadata(&document.global_stats, localized) {
            text(ui, p, label, "muted")?;
            text(ui, p, &value, "text")?;
        }
        super::details::render_details(ui, data, state, p)?;
        Ok(actions)
    })
    .inner
}
/// Resolve optional literal text without adding general value coercion.
pub(super) fn literal(value: &Value) -> io::Result<&str> {
    if !is_truthy(value) {
        return Ok("");
    }
    value
        .as_str()
        .ok_or_else(|| invalid("nonstring header text"))
}
/// Substitute only for empty display text.
fn fallback<'a>(text: &'a str, default: &'a str) -> &'a str {
    if text.is_empty() { default } else { text }
}
/// Paint selectable literal text and report only completed plain clicks.
pub(super) fn text(
    ui: &mut egui::Ui,
    p: &BrandPresentation<'_>,
    value: impl Into<egui::RichText>,
    role: &str,
) -> io::Result<bool> {
    let response = ui.add(
        egui::Label::new(value.into().color(color(p, role)?))
            .selectable(true)
            .sense(egui::Sense::click_and_drag()),
    );
    Ok(response.clicked() && !response.double_clicked() && !response.triple_clicked())
}

/// Format the six rows from the preparation snapshot, never the raw history.
fn metadata(stats: &super::Stats, date: String) -> [(&'static str, String); 6] {
    let messages = [
        (stats.user_messages, "user"),
        (stats.assistant_messages, "assistant"),
        (stats.tool_results, "tool results"),
        (stats.custom_messages, "custom"),
        (stats.compactions, "compactions"),
        (stats.branch_summaries, "branch summaries"),
    ]
    .into_iter()
    .filter(|(n, _)| *n != 0)
    .map(|(n, label)| format!("{n} {label}"))
    .collect::<Vec<_>>()
    .join(", ");
    let tokens = &stats.tokens;
    let counts = [
        ("↑", tokens.input),
        ("↓", tokens.output),
        ("R", tokens.cache_read),
        ("W", tokens.cache_write),
    ]
    .into_iter()
    .filter(|(_, n)| *n != 0.0 && !n.is_nan())
    .map(|(label, n)| format!("{label}{}", format_tokens(n)))
    .collect::<Vec<_>>()
    .join(" ");
    let cost = &tokens.cost;
    let total = cost.input + cost.output + cost.cache_read + cost.cache_write;
    [
        ("Date:", date),
        (
            "Models:",
            fallback(&stats.models.join(", "), "unknown").into(),
        ),
        ("Messages:", fallback(&messages, "0").into()),
        ("Tool Calls:", stats.tool_calls.to_string()),
        ("Tokens:", fallback(&counts, "0").into()),
        (
            "Cost:",
            format!("${}", ryu_js::Buffer::new().format_to_fixed(total, 3)),
        ),
    ]
}
