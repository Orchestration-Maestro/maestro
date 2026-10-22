//! Strict browser-owned source and image recording records.
use super::{BROWSER_SECTIONS, Case, File, NATIVE_SECTIONS};
use serde::Deserialize;

/// Rendered source body and the language class selected for it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    /// Selected language class, absent when no classifier matched the name.
    pub class: Option<String>,
    /// Rendered body text, kept literal.
    pub text: String,
}

/// Mounted image source with its accessible text and pack fallback.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    /// Mounted source, including raw base64 framing.
    pub src: String,
    /// Accessible text kept from the authored filename.
    pub alt: String,
    /// Pack fallback substituted when the source fails to load.
    pub fallback: String,
}

/// Complete browser projection; every field has a test consumer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Corpus {
    /// Rendered source bodies and their selected language classes.
    pub text_render: Vec<Case<File, Text>>,
    /// Raw base64 framing and literal authored data URLs.
    pub image_url: Vec<Case<File, String>>,
    /// Mounted source and the failure fallback for each image.
    pub image_render: Vec<Case<File, Image>>,
}

/// Decode the committed browser recording with no ignored fields.
/// # Panics
/// Panics if the committed recording is malformed, admits a native-only section,
/// or holds a section count other than the one the browser recording has.
#[must_use]
pub fn corpus() -> Corpus {
    use crate::support::{reject_sections, strict_record};
    use serde_json::Value;
    let source = include_str!("../fixtures/text_image_artifacts_browser.json");
    let valid: Value = serde_json::from_str(source).unwrap();
    reject_sections::<Corpus>(&valid, &NATIVE_SECTIONS);
    strict_record::<Corpus>(&valid, &BROWSER_SECTIONS);
    strict_record::<Case<File, Text>>(&valid["textRender"][0], &["input", "output"]);
    strict_record::<File>(&valid["textRender"][0]["input"], &["filename", "content"]);
    strict_record::<Text>(&valid["textRender"][0]["output"], &["text"]);
    strict_record::<Case<File, String>>(&valid["imageUrl"][0], &["input", "output"]);
    strict_record::<Case<File, Image>>(&valid["imageRender"][0], &["input", "output"]);
    strict_record::<Image>(
        &valid["imageRender"][0]["output"],
        &["src", "alt", "fallback"],
    );
    let corpus: Corpus = serde_json::from_str(source).unwrap();
    assert_eq!(corpus.text_render.len(), 10);
    assert_eq!(corpus.image_url.len(), 10);
    assert_eq!(corpus.image_render.len(), 2);
    corpus
}
