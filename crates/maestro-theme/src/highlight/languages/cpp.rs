//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/cpp.js`, version 11.9.0.
/// SHA-256: b3e4982500a1f44581e01f36fdce141d5419bd7dcee06dbf4323865afc505be0.
pub(super) const VIEWER: Language = Language {
    name: "cpp",
    root: Some(ModeId(24)),
    aliases: &["cc", "c++", "h++", "hpp", "hh", "hxx", "cxx"],
    superset: None,
    fold: false,
};
