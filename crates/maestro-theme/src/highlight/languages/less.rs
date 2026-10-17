//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/less.js`, version 11.9.0.
/// SHA-256: 67e8013a6e7326b211380cf2947fc7c8c88ff42e3308f1f262d87b72b693523e.
pub(super) const VIEWER: Language = Language {
    name: "less",
    root: Some(ModeId(168)),
    aliases: &[],
    superset: None,
    fold: true,
};
