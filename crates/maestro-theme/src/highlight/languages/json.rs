//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/json.js`, version 11.9.0.
/// SHA-256: 43cfdc9ec3fdbd55ab1d9aac51bfb0350de89c23ce316be6f3bb3d637965b643.
pub(super) const VIEWER: Language = Language {
    name: "json",
    root: Some(ModeId(144)),
    aliases: &[],
    superset: None,
    fold: false,
};

/// Derived from `highlight.js/lib/languages/json.js`, version 11.11.1.
/// SHA-256: 6923a3613029162eedb83f67fad8451183ce5df423a9aa37878b0efb8c54d6d1.
pub(super) const BROWSER: Language = Language {
    name: "json",
    root: Some(ModeId(144)),
    aliases: &["jsonc"],
    superset: None,
    fold: false,
};
