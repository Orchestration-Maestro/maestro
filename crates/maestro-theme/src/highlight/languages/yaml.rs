//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/yaml.js`, version 11.9.0.
/// SHA-256: 30838a97519b2bcd02ba4886b2f9e6fec0311b8ecfb1abd989af949633919732.
pub(super) const VIEWER: Language = Language {
    name: "yaml",
    root: Some(ModeId(475)),
    aliases: &["yml"],
    superset: None,
    fold: true,
};
