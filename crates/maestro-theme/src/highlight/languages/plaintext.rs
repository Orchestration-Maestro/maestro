//! Immutable language descriptors.
use super::Language;

/// Derived from `highlight.js/lib/languages/plaintext.js`, version 11.9.0.
/// SHA-256: 74a42662f49d0093dee501a4e46db9f36c1f8752bff4f86bdcb26c88362854bd.
pub(super) const VIEWER: Language = Language {
    name: "plaintext",
    root: None,
    aliases: &["text", "txt"],
    superset: None,
    fold: false,
};
