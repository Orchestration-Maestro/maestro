//! Shared measure/readback/allocate/publish flow for mixed native reading content.
use super::{ReadingContent, ReadingPhase, ReadingPlacement};
use crate::browser::attachment_content::{
    AttachmentContent, AttachmentHost, AttachmentSurface, AttachmentUiError,
};
use std::rc::Rc;

/// Height intent supplied by the native surface's owner.
pub(crate) enum ReadingHeight {
    /// Intrinsic content can grow a provisional horizontal row.
    Flow,
    /// A caller-selected viewport remains bounded under every parent layout.
    Viewport,
}

/// Call-scoped operands for one native surface with canvas control slots.
pub(crate) struct ControlledReading<'a> {
    /// Native flow or explicitly bounded viewport.
    pub height: ReadingHeight,
    /// Existing platform transaction owner.
    pub host: &'a dyn AttachmentHost,
    /// Retained surface identity.
    pub id: egui::Id,
    /// Fully owned feature factory.
    pub content: &'a Rc<ReadingContent>,
    /// Measured full canvas-control sizes.
    pub sizes: &'a [(egui::Id, egui::Vec2)],
    /// Existing containing-modal identity, never inferred from foreground state.
    pub modal: Option<egui::Id>,
    /// Selected frame presentation.
    pub presentation: &'a maestro_theme::BrandPresentation<'a>,
}
/// Published surface response and full control rectangles in canvas points.
pub(crate) struct ControlledLayout {
    /// Allocated natural-height response.
    pub response: egui::Response,
    /// Unclipped native-measured slots translated to canvas coordinates.
    pub controls: Vec<(egui::Id, egui::Rect)>,
}
impl ControlledReading<'_> {
    /// Submit one explicit transaction phase through the existing host.
    fn place(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        phase: ReadingPhase,
        holes: Vec<egui::Rect>,
    ) -> Result<(), AttachmentUiError> {
        self.host.present(
            AttachmentSurface {
                id: self.id,
                rect,
                clip_rect: ui.clip_rect(),
                pixels_per_point: ui.ctx().pixels_per_point(),
                opacity: 1.,
                occluders: holes,
                content: AttachmentContent::Reading(ReadingPlacement {
                    content: Rc::clone(self.content),
                    phase,
                    sizes: self.sizes,
                    modal: self.modal,
                    context: ui.ctx(),
                }),
            },
            self.presentation,
        )
    }
    /// Use one measured snapshot for allocation, publication and controls.
    pub(crate) fn draw(&self, ui: &mut egui::Ui) -> Result<ControlledLayout, AttachmentUiError> {
        let mut available = ui.available_size_before_wrap().max(egui::Vec2::ZERO);
        if matches!(self.height, ReadingHeight::Flow)
            && ui
                .stack()
                .iter()
                .any(|s| s.layout_direction.is_horizontal())
        {
            available.y = (ui.clip_rect().bottom() - ui.cursor().top()).max(0.);
        }
        self.place(
            ui,
            egui::Rect::from_min_size(ui.next_widget_position(), available),
            ReadingPhase::Measure,
            vec![],
        )?;
        let layout = self.host.reading_layout(self.id)?;
        let (id, rect) = ui.allocate_space(egui::vec2(
            available.x,
            layout.height.min(available.y).max(0.),
        ));
        let controls: Vec<_> = layout
            .controls
            .into_iter()
            .map(|(id, r)| (id, r.translate(rect.min.to_vec2())))
            .collect();
        self.place(
            ui,
            rect,
            ReadingPhase::Publish,
            controls.iter().map(|(_, r)| *r).collect(),
        )?;
        Ok(ControlledLayout {
            response: ui.interact(rect, id, egui::Sense::hover()),
            controls,
        })
    }
}
