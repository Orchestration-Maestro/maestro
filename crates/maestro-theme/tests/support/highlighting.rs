//! Shared controlled and native syntax painters.
use maestro_theme::{SyntaxHighlighter, SyntaxSpan, SyntectHighlighter};
use std::{cell::RefCell, error::Error, sync::OnceLock};

/// Supports fixed labels and answers every highlight with one scripted outcome.
pub struct Scripted {
    /// Labels reported as supported.
    languages: &'static [&'static str],
    /// Spans returned, or the message of the failure returned.
    outcome: Result<Vec<SyntaxSpan>, &'static str>,
    /// `supports:<label>` and `highlight:<label>` entries in call order.
    log: RefCell<Vec<String>>,
}

impl Scripted {
    /// Support `languages` and answer with `outcome`.
    #[must_use]
    pub fn new(
        languages: &'static [&'static str],
        outcome: Result<Vec<SyntaxSpan>, &'static str>,
    ) -> Self {
        Self {
            languages,
            outcome,
            log: RefCell::default(),
        }
    }

    /// Fail every highlight of a supported `javascript` label.
    #[must_use]
    pub fn failing() -> Self {
        Self::new(&["javascript"], Err("controlled engine failure"))
    }

    /// The decisions recorded so far.
    pub fn calls(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

impl SyntaxHighlighter for Scripted {
    fn supports_language(&self, language: &str) -> bool {
        self.log.borrow_mut().push(format!("supports:{language}"));
        self.languages.contains(&language)
    }

    fn highlight(
        &self,
        _code: &str,
        language: &str,
    ) -> Result<Vec<SyntaxSpan>, Box<dyn Error + Send + Sync>> {
        self.log.borrow_mut().push(format!("highlight:{language}"));
        self.outcome.clone().map_err(Into::into)
    }
}

/// The production highlighter, loaded once for the whole target.
///
/// # Panics
/// Panics if the bundled syntax set fails to load.
#[cfg(test)]
pub fn native() -> &'static SyntectHighlighter {
    static NATIVE: OnceLock<SyntectHighlighter> = OnceLock::new();
    NATIVE.get_or_init(|| SyntectHighlighter::new().unwrap())
}

/// Remove tags and decode the five HTML entities once.
#[must_use]
pub fn decode(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
}
