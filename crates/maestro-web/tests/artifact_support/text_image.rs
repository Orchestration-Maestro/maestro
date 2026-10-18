//! Strict source-run records and a caller-owned recording classifier.
use serde::Deserialize;
use std::cell::{Cell, RefCell};
/// Recorded input and observable output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case<I, O> {
    pub input: I,
    pub output: O,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Name {
    pub filename: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub content: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub filename: String,
    pub content: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Copy {
    pub text: String,
    pub title: String,
    pub show_text: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Download {
    pub filename: String,
    pub mime_type: String,
    pub title: String,
    pub bytes: Vec<u8>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub copy: Copy,
    pub download: Download,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub class: Option<String>,
    pub text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bytes {
    pub bytes: Vec<u8>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub src: String,
    pub alt: String,
    pub fallback: String,
}
/// Complete projected recording; each row belongs to one observation route.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Corpus {
    pub text_language: Vec<Case<Name, Option<String>>>,
    pub text_mime: Vec<Case<Name, String>>,
    pub text_header: Vec<Case<File, Header>>,
    pub text_render: Vec<Case<File, Text>>,
    pub image_mime: Vec<Case<Name, String>>,
    pub image_url: Vec<Case<File, String>>,
    pub image_bytes: Vec<Case<Content, Bytes>>,
    pub image_invalid: Vec<Content>,
    pub image_render: Vec<Case<File, Image>>,
}
/// Read the committed projection with no ignored fields.
/// # Panics
/// Panics if the committed source recording is malformed.
#[must_use]
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/text_image_artifacts.json")).unwrap()
}
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
