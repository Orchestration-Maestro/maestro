//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/r.js`, version 11.9.0.
/// SHA-256: 5ab85530aa424f887d6f80abeefda8ebd460b65d5c1793f22f4c8f3aa99049c7.
pub(super) const VIEWER: Language = Language {
    name: "r",
    root: Some(ModeId(314)),
    aliases: &[],
    superset: None,
    fold: false,
};
