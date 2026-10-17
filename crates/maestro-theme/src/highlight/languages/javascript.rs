//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/javascript.js`, version 11.9.0.
/// SHA-256: b9871c93188ce7804b6b4fdb1154cc41d8cc21953619408e93643521ce0fe1d3.
pub(super) const VIEWER: Language = Language {
    name: "javascript",
    root: Some(ModeId(108)),
    aliases: &["js", "jsx", "mjs", "cjs"],
    superset: None,
    fold: false,
};

/// Derived from `highlight.js/lib/languages/javascript.js`, version 11.11.1.
/// SHA-256: 26621006809440240317c074a5e6681838b5be5c20fdcdee560e2d25448d85a9.
pub(super) const BROWSER: Language = Language {
    name: "javascript",
    root: Some(ModeId(487)),
    aliases: &["js", "jsx", "mjs", "cjs"],
    superset: None,
    fold: false,
};
