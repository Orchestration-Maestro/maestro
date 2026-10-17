//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/typescript.js`, version 11.9.0.
/// SHA-256: 9f9ff86b80b63b8623973eb4cdacc9ce26871a409139e4800a7a4bf9d4440956.
pub(super) const VIEWER: Language = Language {
    name: "typescript",
    root: Some(ModeId(416)),
    aliases: &["ts", "tsx", "mts", "cts"],
    superset: None,
    fold: false,
};

/// Derived from `highlight.js/lib/languages/typescript.js`, version 11.11.1.
/// SHA-256: ba351fb35bd479c4228d539ee4f4772286623f2be4134bc80f9e10285451790b.
pub(super) const BROWSER: Language = Language {
    name: "typescript",
    root: Some(ModeId(507)),
    aliases: &["ts", "tsx", "mts", "cts"],
    superset: None,
    fold: false,
};
