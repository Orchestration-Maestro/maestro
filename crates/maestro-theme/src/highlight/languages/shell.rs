//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/shell.js`, version 11.9.0.
/// SHA-256: e53d881fbe6c40616e2b4f1631212e32563dcdc15c15bd46626213c3bc152c30.
pub(super) const VIEWER: Language = Language {
    name: "shell",
    root: Some(ModeId(378)),
    aliases: &["console", "shellsession"],
    superset: None,
    fold: false,
};
