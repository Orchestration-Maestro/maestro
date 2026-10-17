//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/rust.js`, version 11.9.0.
/// SHA-256: 01d5ef11928daf3fc2dfaf7fbe4d0aefbce268bae0ecd3b7f65fe3bd28ed877d.
pub(super) const VIEWER: Language = Language {
    name: "rust",
    root: Some(ModeId(368)),
    aliases: &["rs"],
    superset: None,
    fold: false,
};
