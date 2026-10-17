//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/go.js`, version 11.9.0.
/// SHA-256: 3af4db7f9c31f1479c88e812c175e59364873aa63fc186cbc4f746be66b7c3f0.
pub(super) const VIEWER: Language = Language {
    name: "go",
    root: Some(ModeId(79)),
    aliases: &["golang"],
    superset: None,
    fold: false,
};
