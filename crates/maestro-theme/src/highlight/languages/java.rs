//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/java.js`, version 11.9.0.
/// SHA-256: 864aebc9b95f1c509ecb9f6ee72cc74dff1a8391d803b0b0a21896e3a0f20f2d.
pub(super) const VIEWER: Language = Language {
    name: "java",
    root: Some(ModeId(96)),
    aliases: &["jsp"],
    superset: None,
    fold: false,
};
