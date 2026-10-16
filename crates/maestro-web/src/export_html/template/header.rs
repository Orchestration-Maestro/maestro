//! Native offline session metadata.
use super::SessionDocument;
use maestro_app::is_truthy;
use serde_json::Value;
use std::io;
mod details;
/// Owned literal metadata passed to the native factory.
#[derive(Debug, PartialEq, Eq)]
struct Prepared {
    /// Session identity.
    title: String,
    /// Values in date, models, messages, calls, tokens and cost order.
    rows: [String; 6],
    /// Owned system prompt disclosure.
    prompt: Option<details::Prompt>,
    /// Ordered owned tool disclosures.
    tools: Vec<details::Tool>,
}
/// Prepare reached header operands without changing the raw document.
fn prepare(
    doc: &SessionDocument,
    date: &dyn Fn(&Value) -> io::Result<String>,
) -> io::Result<Prepared> {
    let header = doc.data.get("header").unwrap_or(&Value::Null);
    let id = if is_truthy(&header["id"]) {
        header["id"]
            .as_str()
            .ok_or_else(|| super::document::invalid("nonstring session identity"))?
    } else {
        "unknown"
    };
    let stats = &doc.global_stats;
    let cost = &stats.tokens.cost;
    Ok(Prepared {
        title: format!("Session: {id}"),
        rows: [
            if is_truthy(&header["timestamp"]) {
                date(&header["timestamp"])?
            } else {
                "unknown".into()
            },
            fallback(stats.models.join(", "), "unknown"),
            messages(stats),
            stats.tool_calls.to_string(),
            tokens(stats),
            format!(
                "${}",
                ryu_js::Buffer::new().format_to_fixed(
                    cost.input + cost.output + cost.cache_read + cost.cache_write,
                    3
                )
            ),
        ],
        prompt: details::prompt(doc.data.get("systemPrompt").unwrap_or(&Value::Null))?,
        tools: details::tools(doc.data.get("tools").unwrap_or(&Value::Null))?,
    })
}
/// Substitute only a genuinely empty display value.
fn fallback(text: String, empty: &str) -> String {
    if text.is_empty() { empty.into() } else { text }
}
/// Render the offline header's thousand and million boundaries.
fn format_tokens(count: f64) -> String {
    if count >= 1_000_000. || count.is_nan() {
        format!(
            "{}M",
            ryu_js::Buffer::new().format_to_fixed(count / 1_000_000., 1)
        )
    } else {
        crate::utils::format::format_token_count(count)
    }
}
#[cfg(test)]
mod tests;

use crate::browser::attachment_content::{self, AttachmentHost, ReadingContent};
use maestro_theme::BrandPresentation;
use std::rc::Rc;
/// Requested action; execution belongs to the embedding application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderAction {
    /// Toggle reasoning visibility.
    ToggleThinking,
    /// Toggle tool visibility.
    ToggleTools,
    /// Download the original session.
    DownloadSessionJson,
}
/// Retained native reading identity scoped to a replaceable host.
pub struct HeaderState {
    /// Native placement owner.
    host: Rc<dyn AttachmentHost>,
    /// Unique surface identity.
    id: egui::Id,
    /// Last successfully published preparation and factory.
    content: Option<(Rc<Prepared>, Rc<ReadingContent>)>,
    /// Action owning the primary press begun in its visible canvas slot.
    pressed: Option<HeaderAction>,
}
impl HeaderState {
    /// Retain the supplied host without selecting a platform adapter.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            id: attachment_content::identity(),
            content: None,
            pressed: None,
        }
    }
}
impl Drop for HeaderState {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
/// Synchronous style and date operands, never retained by native callbacks.
pub struct HeaderView<'a> {
    /// Caller-selected brand projection.
    pub presentation: &'a BrandPresentation<'a>,
    /// Native or embedding-supplied locale operation.
    pub localize_date: &'a dyn Fn(&Value) -> io::Result<String>,
}
/// Canvas labels, native tooltips and action order.
const CONTROLS: [(&str, &str, HeaderAction); 3] = [
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
];
/// Native metadata label order.
#[cfg(any(target_arch = "wasm32", test))]
const LABELS: [&str; 6] = [
    "Date:",
    "Models:",
    "Messages:",
    "Tool Calls:",
    "Tokens:",
    "Cost:",
];
/// Native keyboard help, separate from canvas action labels.
#[cfg(any(target_arch = "wasm32", test))]
const HELP: &str = "T toggle thinking · O toggle tools";
/// Render nonzero message categories in their display order.
fn messages(stats: &super::Stats) -> String {
    fallback(
        [
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
        .join(", "),
        "0",
    )
}
/// Render nonzero reported token components in display order.
fn tokens(stats: &super::Stats) -> String {
    fallback(
        [
            ("↑", stats.tokens.input),
            ("↓", stats.tokens.output),
            ("R", stats.tokens.cache_read),
            ("W", stats.tokens.cache_write),
        ]
        .into_iter()
        .filter(|(_, n)| *n != 0. && !n.is_nan())
        .map(|(prefix, n)| format!("{prefix}{}", format_tokens(n)))
        .collect::<Vec<_>>()
        .join(" "),
        "0",
    )
}
mod draw;
pub use draw::render_header;
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::localize_header_date;
