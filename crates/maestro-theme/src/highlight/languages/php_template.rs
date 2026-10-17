//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/php-template.js`, version 11.9.0.
/// SHA-256: 25d6be3facd42cdfec6b4b9785d61edcce5ec17bbce706cba3c131b10b3d4027.
pub(super) const VIEWER: Language = Language {
    name: "php-template",
    root: Some(ModeId(278)),
    aliases: &[],
    superset: None,
    fold: false,
};
