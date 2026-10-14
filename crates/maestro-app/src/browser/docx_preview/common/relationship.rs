//! Open relationship records.
/// A relationship retaining missing attributes and authored target text.
#[derive(Debug)]
pub struct Relationship {
    /// Case-sensitive relationship identity.
    pub id: Option<String>,
    /// Open relationship-type URI.
    pub r#type: Option<String>,
    /// Authored target.
    pub target: Option<String>,
    /// Optional target mode.
    pub target_mode: Option<String>,
}

impl Relationship {
    /// Find the first matching ID without skipping a missing target.
    pub(crate) fn find_target<'a>(rels: Option<&'a [Self]>, id: &str) -> Option<&'a str> {
        rels?
            .iter()
            .find(|r| r.id.as_deref() == Some(id))?
            .target
            .as_deref()
    }
}
