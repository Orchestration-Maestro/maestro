//! Owned feature-created native reading admission.
#[cfg(target_arch = "wasm32")]
use super::AttachmentUiError;
use std::rc::Rc;
/// Native factory with no borrowed render operands.
pub struct ReadingContent {
    /// Mount into the actual canvas document.
    #[cfg(target_arch = "wasm32")]
    pub mount: Rc<ReadingMount>,
}
/// Explicit transaction phase; an empty viewport is still publishable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingPhase {
    /// Measure without replacing the published root.
    Measure,
    /// Publish complete placement.
    Publish,
}
/// Call-scoped geometry and owned reading content.
pub struct ReadingPlacement<'a> {
    /// Candidate native content identity.
    pub content: Rc<ReadingContent>,
    /// Current transaction phase.
    pub phase: ReadingPhase,
    /// Containing foreground modal.
    pub modal: Option<egui::Id>,
    /// Current repaint owner.
    pub context: &'a egui::Context,
}
/// Live natural dimensions in canvas points.
pub struct ReadingLayout {
    /// Unclipped natural height.
    pub height: f32,
}
/// Feature factory returning the whole retained native owner.
#[cfg(target_arch = "wasm32")]
pub type ReadingMount = dyn Fn(&web_sys::Document) -> Result<ReadingRoot, AttachmentUiError>;
/// Feature-owned native reading root.
#[cfg(target_arch = "wasm32")]
pub struct ReadingRoot {
    /// Feature-created native text root.
    pub element: web_sys::Element,
}
