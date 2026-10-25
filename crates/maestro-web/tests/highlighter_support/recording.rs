//! Recording syntax engine shared by native consumer tests.
use std::cell::{Cell, RefCell};
/// Controlled classifier retaining exact calls, not inferred language labels.
#[derive(Default)]
pub struct Highlighter {
    pub requests: RefCell<Vec<(String, String)>>,
    pub supports: RefCell<Vec<String>>,
    pub unsupported: Cell<bool>,
    pub fail: Cell<bool>,
}
impl maestro_theme::SyntaxHighlighter for Highlighter {
    fn supports_language(&self, language: &str) -> bool {
        self.supports.borrow_mut().push(language.into());
        !self.unsupported.get()
    }
    fn highlight(
        &self,
        code: &str,
        language: &str,
    ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
        self.requests
            .borrow_mut()
            .push((code.into(), language.into()));
        if self.fail.get() {
            return Err(std::io::Error::other("engine failure").into());
        }
        Ok(vec![maestro_theme::SyntaxSpan {
            range: 0..code.len(),
            color: None,
        }])
    }
}
