//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/lua.js`, version 11.9.0.
/// SHA-256: 01c7996c4136063bd3ff159852f95beda637ef8fcd072e77b1850103ab29ba88.
pub(super) const VIEWER: Language = Language {
    name: "lua",
    root: Some(ModeId(192)),
    aliases: &[],
    superset: None,
    fold: false,
};
