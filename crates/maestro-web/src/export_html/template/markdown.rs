//! Selectable Markdown for offline session documents.
use maestro_theme::SyntaxHighlighter;

/// Render session Markdown without chat-only formulas or code controls.
#[must_use]
pub fn safe_marked_parse(text: &str, highlighter: &dyn SyntaxHighlighter) -> String {
    crate::components::markdown_block::prepare_export(text, highlighter)
}
