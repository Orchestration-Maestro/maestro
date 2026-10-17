//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/python.js`, version 11.9.0.
/// SHA-256: 0fe267655b562c91efbad90dfbbb62f6575a8771832e51532b99ac0a47215b0d.
pub(super) const VIEWER: Language = Language {
    name: "python",
    root: Some(ModeId(283)),
    aliases: &["py", "gyp", "ipython"],
    superset: None,
    fold: false,
};

/// Derived from `highlight.js/lib/languages/python.js`, version 11.11.1.
/// SHA-256: 6fa85c05f6852e7f761b5c0cfdb3f75992704012bbde67197db2c22286fcd072.
pub(super) const BROWSER: Language = Language {
    name: "python",
    root: Some(ModeId(527)),
    aliases: &["py", "gyp", "ipython"],
    superset: None,
    fold: false,
};
