//! Immutable language descriptors.
use super::Language;

/// Derived from `highlight.js/lib/languages/c.js`, version 11.9.0.
/// SHA-256: aa075c2937cbf9d17061ead5194f6e2fb501a9efbeffd1468da064d6b2d99df3.
pub(super) const VIEWER: Language = Language {
    name: "c",
    root: None,
    aliases: &["h"],
    superset: None,
    fold: false,
};
