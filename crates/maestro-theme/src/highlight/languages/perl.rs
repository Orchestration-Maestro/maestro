//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/perl.js`, version 11.9.0.
/// SHA-256: 299120f01f5e861507fe59b7f5c4ed144114c55ea04f1b6b00fe69d64713c47c.
pub(super) const VIEWER: Language = Language {
    name: "perl",
    root: Some(ModeId(234)),
    aliases: &["pl", "pm"],
    superset: None,
    fold: false,
};
