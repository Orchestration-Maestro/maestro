//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/objectivec.js`, version 11.9.0.
/// SHA-256: 3a410538d1040627da9f664fc109cd5efa8fef655dffc99612d83e2b5af3548b.
pub(super) const VIEWER: Language = Language {
    name: "objectivec",
    root: Some(ModeId(231)),
    aliases: &["mm", "objc", "obj-c", "obj-c++", "objective-c++"],
    superset: None,
    fold: false,
};
