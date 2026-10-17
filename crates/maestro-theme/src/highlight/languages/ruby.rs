//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/ruby.js`, version 11.9.0.
/// SHA-256: d16fe1c7cb263dc2167c819104bb50311f4a4e10a1368ad5e682188434d0b728.
pub(super) const VIEWER: Language = Language {
    name: "ruby",
    root: Some(ModeId(331)),
    aliases: &["rb", "gemspec", "podspec", "thor", "irb"],
    superset: None,
    fold: false,
};
