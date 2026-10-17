//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/css.js`, version 11.9.0.
/// SHA-256: 92c0e16ca484d6a9d8aed422266e1afb88d3fbee03807d9578330429707ab3a0.
pub(super) const VIEWER: Language = Language {
    name: "css",
    root: Some(ModeId(63)),
    aliases: &[],
    superset: None,
    fold: true,
};

/// Derived from `highlight.js/lib/languages/css.js`, version 11.11.1.
/// SHA-256: 2787cf1b1f53248434ea67921acc2ffdeb0192c0b95b84003f57b8c711c87040.
pub(super) const BROWSER: Language = Language {
    name: "css",
    root: Some(ModeId(528)),
    aliases: &[],
    superset: None,
    fold: true,
};
