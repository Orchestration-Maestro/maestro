//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/csharp.js`, version 11.9.0.
/// SHA-256: 7731c269b20deb0ea8b4af1982e1d0b12b008a27ba17b17408f172cde8a7d0cc.
pub(super) const VIEWER: Language = Language {
    name: "csharp",
    root: Some(ModeId(37)),
    aliases: &["cs", "c#"],
    superset: None,
    fold: false,
};
