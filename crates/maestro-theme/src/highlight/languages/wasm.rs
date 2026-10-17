//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/wasm.js`, version 11.9.0.
/// SHA-256: 08e0dacdff2f21c5928e934c2e70e9ca34c6d6345856951d344e4a6b772ee9bb.
pub(super) const VIEWER: Language = Language {
    name: "wasm",
    root: Some(ModeId(443)),
    aliases: &[],
    superset: None,
    fold: false,
};
