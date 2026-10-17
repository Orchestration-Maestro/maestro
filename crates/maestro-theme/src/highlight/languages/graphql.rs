//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/graphql.js`, version 11.9.0.
/// SHA-256: 0d76e2e8e0a7d8447d4e4512c16dfee9dd7f6910a7979fc0b1e591426ff51180.
pub(super) const VIEWER: Language = Language {
    name: "graphql",
    root: Some(ModeId(83)),
    aliases: &["gql"],
    superset: None,
    fold: true,
};
