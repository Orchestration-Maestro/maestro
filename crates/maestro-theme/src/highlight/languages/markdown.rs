//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/markdown.js`, version 11.9.0.
/// SHA-256: e2e2edca3d20ef414e4a6d52267c146c93aa4f5c87545b51f100780a5fd2f8ef.
pub(super) const VIEWER: Language = Language {
    name: "markdown",
    root: Some(ModeId(204)),
    aliases: &["md", "mkdown", "mkd"],
    superset: None,
    fold: false,
};
