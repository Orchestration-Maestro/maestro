//! Owned list definitions and concrete numbering instances.
use super::{CommonProperties, ParagraphProperties};
mod numbering_part;
pub(crate) mod parser;
pub use numbering_part::NumberingPart;
/// Concrete numbering reference.
#[derive(Clone, Debug, Default)]
pub struct Numbering {
    /// Nullable concrete identifier.
    pub id: Option<String>,
    /// Encountered abstract identifier.
    pub abstract_id: Option<Option<String>>,
    /// Overrides in source order.
    pub overrides: Vec<NumberingLevelOverride>,
}
/// One ordered level override.
#[derive(Clone, Debug, Default)]
pub struct NumberingLevelOverride {
    /// Selected level.
    pub level: Option<f64>,
    /// Encountered start override.
    pub start: Option<Option<f64>>,
    /// Last replacement level.
    pub numbering_level: Option<NumberingLevel>,
}
/// Abstract list definition.
#[derive(Clone, Debug, Default)]
pub struct AbstractNumbering {
    /// Nullable definition identifier.
    pub id: Option<String>,
    /// Encountered name.
    pub name: Option<Option<String>>,
    /// Encountered multilevel type.
    pub multi_level_type: Option<Option<String>>,
    /// Levels in source order.
    pub levels: Vec<NumberingLevel>,
    /// Encountered numbering style link.
    pub numbering_style_link: Option<Option<String>>,
    /// Encountered style link.
    pub style_link: Option<Option<String>>,
}
/// Raw numbering level metadata.
#[derive(Clone, Debug, Default)]
pub struct NumberingLevel {
    /// Nullable numeric level.
    pub level: Option<f64>,
    /// Encountered textual start.
    pub start: Option<Option<String>>,
    /// Encountered numeric restart.
    pub restart: Option<Option<f64>>,
    /// Encountered open format.
    pub format: Option<Option<String>>,
    /// Encountered level text.
    pub text: Option<Option<String>>,
    /// Encountered justification.
    pub justification: Option<Option<String>>,
    /// Encountered raw picture identifier.
    pub bullet_picture_id: Option<Option<String>>,
    /// Encountered paragraph style.
    pub paragraph_style: Option<Option<String>>,
    /// Last raw paragraph group.
    pub paragraph_props: Option<ParagraphProperties>,
    /// Last raw run group.
    pub run_props: Option<CommonProperties>,
}
/// Complete immediate picture chain.
#[derive(Clone, Debug)]
pub struct NumberingBulletPicture {
    /// Raw picture identifier.
    pub id: Option<String>,
    /// Image relationship identifier.
    pub reference_id: Option<String>,
    /// Shape style.
    pub style: Option<String>,
}
