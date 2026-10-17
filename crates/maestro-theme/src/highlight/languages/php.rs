//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/php.js`, version 11.9.0.
/// SHA-256: e5e710da392783bdaa2714024e504dc83c99e32773a020daf46f3751424f6bee.
pub(super) const VIEWER: Language = Language {
    name: "php",
    root: Some(ModeId(253)),
    aliases: &[],
    superset: None,
    fold: false,
};
