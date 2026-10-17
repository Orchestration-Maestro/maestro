//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/swift.js`, version 11.9.0.
/// SHA-256: 8c56ac64d57574c515bf48faffb84f7924b1107ad5d4c8de45ba8ff004201dbb.
pub(super) const VIEWER: Language = Language {
    name: "swift",
    root: Some(ModeId(386)),
    aliases: &[],
    superset: None,
    fold: false,
};
