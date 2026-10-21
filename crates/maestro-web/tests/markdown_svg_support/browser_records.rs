//! Reader-owned artifact observations.
use serde::Deserialize;

/// Recorded inputs and expectations; every field has a test consumer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Locale-independent text and header/source/image observations.
    pub content: Vec<Case>,
    /// Same-object resource operations.
    pub lifecycle: Lifecycle,
}
/// One literal artifact observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Artifact family.
    pub kind: String,
    /// Authored filename.
    pub filename: String,
    /// Original literal content.
    pub content: String,
    /// Edited filename independent of language.
    pub renamed_filename: String,
    /// Replacement literal source.
    pub edited_content: String,
    /// Fixed native syntax class.
    pub language_class: String,
    /// Native SVG load/error, absent for no image or Markdown.
    pub outcome: Option<String>,
}
/// Literal SVG operations and exact URL event sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lifecycle {
    /// Initial, replaced, cleared and whitespace source.
    pub contents: [String; 4],
    /// Two independently retained documents.
    pub independent: [String; 2],
    /// Ordered create/revoke observations, including resource identity.
    pub events: Vec<UrlEvent>,
}
/// Native URL allocation or release.
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct UrlEvent {
    /// Resource operation.
    pub op: String,
    /// Stable per-run resource identity.
    pub id: u32,
    /// Created Blob MIME, absent for release.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    /// Created Blob byte size, absent for release.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
}
