//! Unsupported highlighter for non-code view callers.

/// Unsupported engine for callers which do not render source code.
pub struct NoHighlight;
impl maestro_theme::SyntaxHighlighter for NoHighlight {
    fn supports_language(&self, _: &str) -> bool {
        false
    }
    fn highlight(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
        unreachable!()
    }
}
