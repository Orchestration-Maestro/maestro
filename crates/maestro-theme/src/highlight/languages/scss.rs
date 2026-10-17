//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/scss.js`, version 11.9.0.
/// SHA-256: d9fefc37806b4dfa6eb5fc370e78da83736ab71bf7dbe4e3bf77428ccb27f42f.
pub(super) const VIEWER: Language = Language {
    name: "scss",
    root: Some(ModeId(373)),
    aliases: &[],
    superset: None,
    fold: true,
};
