//! Strict source observations shared by native and actual-browser witnesses.
#![cfg(test)]
use serde::Deserialize;
/// Consumed oracle inputs and expectations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Localized toggle sequences.
    pub toggles: Vec<Toggle>,
    /// Original text and header/source/image observations.
    pub content: Vec<Case>,
    /// Same-object resource operations.
    pub lifecycle: Lifecycle,
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
/// One literal artifact observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Artifact family.
    pub kind: String,
    /// Caller locale.
    pub language: String,
    /// Authored filename.
    pub filename: String,
    /// Original literal content.
    pub content: String,
    /// Edited filename independent of language.
    pub renamed_filename: String,
    /// Replacement literal source.
    pub edited_content: String,
    /// Ordered copy and download titles.
    pub titles: [String; 2],
    /// Download MIME.
    pub mime: String,
    /// Original download bytes.
    pub bytes: Vec<u8>,
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
/// Decode every committed field strictly.
/// # Panics
/// Panics if the committed recording has invalid fields.
#[must_use]
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/markdown_svg_artifacts.json")).unwrap()
}
/// Supply a locale through the real dictionary owner.
/// # Panics
/// Panics if the controlled language host fails.
pub fn locale(i18n: &maestro_web::I18n, language: &str) {
    i18n.set_language(language).unwrap();
}
/// Complete admitted pointer activation.
#[must_use]
pub fn click(position: egui::Pos2) -> Vec<egui::Event> {
    crate::session_header_support::pointer(position, true)
        .into_iter()
        .chain(crate::session_header_support::pointer(position, false))
        .collect()
}

/// One complete keyboard activation without repeat or a modifier chord.
#[must_use]
pub fn key(key: egui::Key) -> Vec<egui::Event> {
    [true, false]
        .into_iter()
        .map(|pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        })
        .collect()
}
