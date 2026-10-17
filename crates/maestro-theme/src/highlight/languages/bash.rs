//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/bash.js`, version 11.9.0.
/// SHA-256: b45df957f872a969cf5c7aeb61470bea549f96614baf458cafcf877466f74bc8.
pub(super) const VIEWER: Language = Language {
    name: "bash",
    root: Some(ModeId(0)),
    aliases: &["sh"],
    superset: None,
    fold: false,
};

/// Derived from `highlight.js/lib/languages/bash.js`, version 11.11.1.
/// SHA-256: 134f86c16781d91ee183e4c5f479449e1c0d6b256f2f1db080fdd40252ac2b1c.
pub(super) const BROWSER: Language = Language {
    name: "bash",
    root: Some(ModeId(529)),
    aliases: &["sh", "zsh"],
    superset: None,
    fold: false,
};
