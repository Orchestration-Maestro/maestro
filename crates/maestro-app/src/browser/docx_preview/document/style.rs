//! Owned style definitions before inheritance resolution.
use super::{common::CommonProperties, paragraph::ParagraphProperties};
/// One named style or document-default definition.
#[derive(Clone, Debug, Default)]
pub struct IDomStyle {
    /// Raw style identifier.
    pub id: Option<String>,
    /// Raw display name.
    pub name: Option<String>,
    /// CSS element target for a supported style kind.
    pub target: Option<String>,
    /// Raw inherited style reference.
    pub based_on: Option<String>,
    /// Ordered CSS declarations.
    pub styles: Vec<IDomSubStyle>,
    /// Encountered nullable default flag.
    pub is_default: Option<Option<bool>>,
    /// Encountered nullable linked style reference.
    pub linked: Option<Option<String>>,
    /// Encountered nullable next style reference.
    pub next: Option<Option<String>>,
    /// Comma-separated aliases, retaining empty pieces.
    pub aliases: Option<Vec<String>>,
    /// Raw metadata from the last paragraph property group.
    pub paragraph_props: Option<ParagraphProperties>,
    /// Common metadata from the last run property group.
    pub run_props: Option<CommonProperties>,
}
/// One CSS target, optional modifier and ordered declaration map.
#[derive(Clone, Debug, Default)]
pub struct IDomSubStyle {
    /// CSS target selector.
    pub target: String,
    /// Conditional table region modifier.
    pub r#mod: Option<String>,
    /// Nullable CSS values in insertion order.
    pub values: indexmap::IndexMap<String, Option<String>>,
}
