//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/kotlin.js`, version 11.9.0.
/// SHA-256: 9d52051b1e7751f435233e3f9ca394672fef354f48c7ca93f224a20348eed3db.
pub(super) const VIEWER: Language = Language {
    name: "kotlin",
    root: Some(ModeId(147)),
    aliases: &["kt", "kts"],
    superset: None,
    fold: false,
};
