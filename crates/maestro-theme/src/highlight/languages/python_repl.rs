//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/python-repl.js`, version 11.9.0.
/// SHA-256: 86fd43acce89a8e90d657efa4733263b301e10c80620e717e6137679a95b7b10.
pub(super) const VIEWER: Language = Language {
    name: "python-repl",
    root: Some(ModeId(310)),
    aliases: &["pycon"],
    superset: None,
    fold: false,
};
