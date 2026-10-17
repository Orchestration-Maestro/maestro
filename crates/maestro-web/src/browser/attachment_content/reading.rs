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
    /// Actual canvas-control sizes, matched by identity.
    pub sizes: &'a [(egui::Id, egui::Vec2)],
    /// Containing foreground modal.
    pub modal: Option<egui::Id>,
    /// Current repaint owner.
    pub context: &'a egui::Context,
}
/// Live natural dimensions in canvas points.
pub struct ReadingLayout {
    /// Unclipped natural height.
    pub height: f32,
    /// Full native slot rectangles relative to the fixed frame in canvas points.
    pub controls: Vec<(egui::Id, egui::Rect)>,
}
/// Feature factory returning the whole retained native owner.
#[cfg(target_arch = "wasm32")]
pub type ReadingMount = dyn Fn(&web_sys::Document) -> Result<ReadingRoot, AttachmentUiError>;
/// Feature-owned native reading root.
#[cfg(target_arch = "wasm32")]
pub struct ReadingRoot {
    /// Feature-created native text root.
    pub element: web_sys::Element,
    /// Empty native slots retaining their caller identities.
    pub slots: Vec<(egui::Id, web_sys::Element)>,
    /// Mounted feature lifetime; factories must not retain this owner.
    pub owner: Option<Box<dyn std::any::Any>>,
}

/// Publish a full native body through the existing two-phase transaction.
pub(crate) fn present(
    host: &dyn super::AttachmentHost,
    ui: &egui::Ui,
    surface: (egui::Id, egui::Rect, Option<egui::Id>),
    reading: &Rc<ReadingContent>,
    presentation: &maestro_theme::BrandPresentation<'_>,
) -> Result<(), super::AttachmentUiError> {
    for phase in [ReadingPhase::Measure, ReadingPhase::Publish] {
        host.present(
            super::AttachmentSurface {
                id: surface.0,
                rect: surface.1,
                clip_rect: ui.clip_rect(),
                pixels_per_point: ui.ctx().pixels_per_point(),
                opacity: 1.,
                occluders: vec![],
                content: super::AttachmentContent::Reading(ReadingPlacement {
                    content: Rc::clone(reading),
                    phase,
                    sizes: &[],
                    modal: surface.2,
                    context: ui.ctx(),
                }),
            },
            presentation,
        )?;
    }
    Ok(())
}

pub(crate) mod controls;

pub(crate) mod controlled;
