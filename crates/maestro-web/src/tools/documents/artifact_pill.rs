//! Literal artifact filenames that open the captured panel.
#[cfg(target_arch = "wasm32")]
use crate::page::PreparedContent;
use crate::tools::browser::documents::LocalArtifactsPanel;
use std::rc::Rc;

/// Owned display and identity; malformed display values have no artifact identity.
#[derive(Clone)]
pub(in crate::tools) struct Pill {
    /// Literal display-only text.
    pub(in crate::tools) text: String,
    /// Authored string identity, never derived from formatted display text.
    pub(in crate::tools) filename: Option<String>,
    /// Panel selected when the recipe was prepared.
    pub(in crate::tools) panel: Option<Rc<LocalArtifactsPanel>>,
}
/// Prepare a selectable literal filename, capturing the supplied panel handle.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn artifact_pill(
    filename: String,
    artifacts_panel: Option<Rc<LocalArtifactsPanel>>,
) -> PreparedContent {
    Box::new(Pill {
        text: filename.clone(),
        filename: Some(filename),
        panel: artifacts_panel,
    })
}
#[cfg(target_arch = "wasm32")]
pub(in crate::tools) mod dom;
