//! Reader-owned artifact observations.
use serde::Deserialize;

/// Recorded inputs and expectations; every field has a test consumer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Localized toggle sequences.
    pub toggles: Vec<Toggle>,
    /// Localized copy and download titles per artifact family.
    pub headers: Vec<Header>,
    /// Locale-independent text and header/source/image observations.
    pub content: Vec<Case>,
}
/// Localized selection and transition sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Toggle {
    /// Caller locale.
    pub language: String,
    /// Localized accessible labels.
    pub titles: [String; 2],
    /// Selection after each action.
    pub states: Vec<[bool; 2]>,
    /// Changed selections only.
    pub events: Vec<String>,
    /// Programmatically configured first frame.
    pub configured: [bool; 2],
}
/// Localized header titles shared by every case of one artifact family.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    /// Caller locale.
    pub language: String,
    /// Artifact family.
    pub kind: String,
    /// Ordered copy and download titles.
    pub titles: [String; 2],
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
    /// Download MIME.
    pub mime: String,
    /// Original download bytes.
    pub bytes: Vec<u8>,
}
