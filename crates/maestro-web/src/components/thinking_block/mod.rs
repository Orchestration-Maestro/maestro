//! Retained reasoning disclosure with native text and Markdown.
#[cfg(target_arch = "wasm32")]
pub(crate) mod dom;

/// Independently retained collapsed reasoning disclosure, mounted as page content through
/// [`DomContent`](crate::dom::DomContent).
#[derive(Default)]
pub struct ThinkingBlock {
    /// Original reasoning source, never trimmed for display.
    pub content: String,
    /// Whether the literal header shimmers.
    pub is_streaming: bool,
}
