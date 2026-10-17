//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/makefile.js`, version 11.9.0.
/// SHA-256: 79b79a25dc7c233ef23c756507c49054902f094d6429adbc70629bc170db0ef2.
pub(super) const VIEWER: Language = Language {
    name: "makefile",
    root: Some(ModeId(198)),
    aliases: &["mk", "mak", "make"],
    superset: None,
    fold: false,
};
