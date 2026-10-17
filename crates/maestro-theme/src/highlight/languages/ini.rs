//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/ini.js`, version 11.9.0.
/// SHA-256: 21eee54329761c77fcf8ef38b42497f602992ef468ea7b578c89fd212cbcdb46.
pub(super) const VIEWER: Language = Language {
    name: "ini",
    root: Some(ModeId(87)),
    aliases: &["toml"],
    superset: None,
    fold: true,
};
