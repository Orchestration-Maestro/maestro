//! Shared typed syntax failure and its retained cause identity.
#![cfg(test)]

use std::sync::Arc;

/// Caller-owned syntax cause with identity independent of its diagnostic text.
#[derive(Debug)]
pub struct Cause(pub Arc<()>);
impl std::fmt::Display for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("original syntax cause")
    }
}
impl std::error::Error for Cause {}
/// Engine returning the supplied cause through the normal highlighting boundary.
pub struct FailingEngine(pub Arc<()>);
impl maestro_theme::SyntaxHighlighter for FailingEngine {
    fn supports_language(&self, _: &str) -> bool {
        true
    }
    fn highlight(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
        Err(Box::new(Cause(self.0.clone())))
    }
}
