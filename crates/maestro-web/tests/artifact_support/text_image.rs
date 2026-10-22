//! Strict source-run records and a caller-owned recording classifier.
use serde::Deserialize;
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
pub struct Bytes {
    pub bytes: Vec<u8>,
}
/// Complete native projection; each row belongs to one observation route.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Corpus {
    pub text_language: Vec<Case<Name, Option<String>>>,
    pub text_mime: Vec<Case<Name, String>>,
    pub text_header: Vec<Case<File, Header>>,
    pub image_mime: Vec<Case<Name, String>>,
    pub image_bytes: Vec<Case<Content, Bytes>>,
    pub image_invalid: Vec<Content>,
}
/// Read the committed projection with no ignored fields.
/// # Panics
/// Panics if the committed source recording is malformed or admits a browser-only
/// section.
#[must_use]
pub fn corpus() -> Corpus {
    use crate::support::{reject_sections, strict_record};
    use serde_json::Value;
    let source = include_str!("../fixtures/text_image_artifacts.json");
    let valid: Value = serde_json::from_str(source).unwrap();
    reject_sections::<Corpus>(&valid, &["textRender", "imageUrl", "imageRender"]);
    strict_record::<Corpus>(
        &valid,
        &[
            "textLanguage",
            "textMime",
            "textHeader",
            "imageMime",
            "imageBytes",
            "imageInvalid",
        ],
    );
    strict_record::<Case<Name, Option<String>>>(&valid["textLanguage"][0], &["input"]);
    strict_record::<Name>(&valid["textLanguage"][0]["input"], &["filename"]);
    strict_record::<Case<Name, String>>(&valid["textMime"][0], &["input", "output"]);
    strict_record::<Case<Name, String>>(&valid["imageMime"][0], &["input", "output"]);
    strict_record::<Case<File, Header>>(&valid["textHeader"][0], &["input", "output"]);
    strict_record::<File>(&valid["textHeader"][0]["input"], &["filename", "content"]);
    strict_record::<Header>(&valid["textHeader"][0]["output"], &["copy", "download"]);
    strict_record::<Copy>(
        &valid["textHeader"][0]["output"]["copy"],
        &["text", "title", "showText"],
    );
    strict_record::<Download>(
        &valid["textHeader"][0]["output"]["download"],
        &["filename", "mimeType", "title", "bytes"],
    );
    strict_record::<Case<Content, Bytes>>(&valid["imageBytes"][0], &["input", "output"]);
    strict_record::<Content>(&valid["imageBytes"][0]["input"], &["content"]);
    strict_record::<Bytes>(&valid["imageBytes"][0]["output"], &["bytes"]);
    strict_record::<Content>(&valid["imageInvalid"][0], &["content"]);
    serde_json::from_str(source).unwrap()
}
#[cfg(target_arch = "wasm32")]
#[path = "text_image_browser_records.rs"]
pub mod browser_records;
#[path = "../highlighter_support/recording.rs"]
pub mod recording;
pub use recording::Highlighter;
