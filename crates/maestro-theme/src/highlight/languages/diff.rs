//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/diff.js`, version 11.9.0.
/// SHA-256: bf381b1d5c9e9206b00873aabb3e5a235e70020859487dd77e50d5aa42d8b3a7.
pub(super) const VIEWER: Language = Language {
    name: "diff",
    root: Some(ModeId(76)),
    aliases: &["patch"],
    superset: None,
    fold: false,
};
