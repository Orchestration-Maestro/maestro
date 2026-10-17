//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/vbnet.js`, version 11.9.0.
/// SHA-256: b1026b16f10af73dfb34f174726fad857fd8d9b15ff9f9b70d410174cb805382.
pub(super) const VIEWER: Language = Language {
    name: "vbnet",
    root: Some(ModeId(438)),
    aliases: &["vb"],
    superset: None,
    fold: true,
};
