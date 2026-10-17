//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/sql.js`, version 11.9.0.
/// SHA-256: 656f537cd25c07ab6c027cc296568420542147f94f737e40a6eb0c94ca519654.
pub(super) const VIEWER: Language = Language {
    name: "sql",
    root: Some(ModeId(381)),
    aliases: &[],
    superset: None,
    fold: true,
};

/// Derived from `highlight.js/lib/languages/sql.js`, version 11.11.1.
/// SHA-256: a0cbca94463751a929701957153fd23552944f47c00da87605f2e0b1f27ff6fb.
pub(super) const BROWSER: Language = Language {
    name: "sql",
    root: Some(ModeId(531)),
    aliases: &[],
    superset: None,
    fold: true,
};
