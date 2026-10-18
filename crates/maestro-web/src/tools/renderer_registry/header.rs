//! Shared status icons, rich captions and collapsible header controls.
use super::ToolHeaderState;
use crate::{
    browser::attachment_content::{self, AttachmentUiError},
    icons::{IconNode, IconSize},
    tools::types::{ToolContent, ToolRenderState, ToolRenderView},
};
use maestro_app::ToolRendererState;
use std::{cell::Cell, rc::Rc};
pub(crate) mod browser;
use browser::{NativeText, TextKind};

/// Literal text or independently executable rich caption content.
pub enum ToolHeaderText {
    /// Literal caption, never localized by the header.
    Text(String),
    /// Owned rich child with its own retained state.
    Content(Box<dyn ToolContent>),
}
/// Build a plain status header with a trailing pending loader.
#[must_use]
pub fn render_header(
    state: ToolHeaderState,
    tool_icon: IconNode,
    text: ToolHeaderText,
) -> Box<dyn ToolContent> {
    Box::new(Header {
        state,
        tool_icon,
        text,
        expanded: None,
    })
}
/// Build a toggle header sharing the caller's expansion handle.
#[must_use]
pub fn render_collapsible_header(
    state: ToolHeaderState,
    tool_icon: IconNode,
    text: ToolHeaderText,
    expanded: Rc<Cell<bool>>,
) -> Box<dyn ToolContent> {
    Box::new(Header {
        state,
        tool_icon,
        text,
        expanded: Some(expanded),
    })
}
/// Immutable recipe; animation and mounted content belong to each rendered row.
struct Header {
    /// Current result status.
    state: ToolHeaderState,
    /// Caller-selected geometry.
    tool_icon: IconNode,
    /// Literal or rich caption.
    text: ToolHeaderText,
    /// Optional caller-owned toggle state.
    expanded: Option<Rc<Cell<bool>>>,
}
/// State of one continuously mounted header.
#[derive(Default)]
struct Retained {
    /// Start of the current pending interval.
    pending_since: Option<f64>,
    /// Literal-caption native lifetime.
    caption: Option<NativeText>,
    /// Independent rich-child lifetime.
    rich: Option<ToolRenderState>,
    /// Visible primary-pointer press admitted in this header.
    pressed: Option<()>,
}
impl ToolContent for Header {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &ToolRendererState,
    ) -> Result<egui::Response, AttachmentUiError> {
        let slot = state.state::<Retained>();
        let mut retained = std::mem::take(&mut *slot.borrow_mut());
        let time = ui.input(|input| input.time);
        let angle = if self.state == ToolHeaderState::Inprogress {
            let start = *retained.pending_since.get_or_insert(time);
            ui.ctx().request_repaint();
            attachment_content::points((time - start).rem_euclid(1.) * std::f64::consts::TAU)
        } else {
            retained.pending_since = None;
            0.
        };
        let result = self.draw(ui, view, &mut retained, angle);
        let old = slot.replace(retained);
        drop(old);
        result
    }
}
impl Header {
    /// Compose icon order, caption and optional trailing control.
    fn draw(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
        angle: f32,
    ) -> Result<egui::Response, AttachmentUiError> {
        let collapsible = self.expanded.is_some();
        let row = self.row(ui, view, retained, angle)?;
        let response = if collapsible {
            ui.interact(row.rect, row.id.with("toggle"), egui::Sense::click())
        } else {
            row
        };
        if let Some(expanded) = &self.expanded {
            let visible = response.rect.intersect(ui.clip_rect());
            let slots = if response.enabled() && visible.is_positive() {
                vec![((), visible, response.layer_id)]
            } else {
                vec![]
            };
            let events = ui.input(|input| input.events.clone());
            let pointer = !attachment_content::reading::controls::pointer_actions(
                &events,
                &mut retained.pressed,
                &slots,
                ui.ctx(),
            )
            .is_empty();
            if pointer || (response.clicked() && !response.clicked_by(egui::PointerButton::Primary))
            {
                expanded.set(!expanded.get());
            }
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    response.enabled(),
                    match &self.text {
                        ToolHeaderText::Text(text) => text,
                        ToolHeaderText::Content(_) => "",
                    },
                )
            });
        }
        self.decorate(ui, view, retained, &response)?;
        if collapsible {
            return Ok(response.on_hover_cursor(egui::CursorIcon::PointingHand));
        }
        Ok(response)
    }
    /// Keep literal hover color and keyboard focus paint inside the caller's clip.
    fn decorate(
        &self,
        ui: &egui::Ui,
        view: ToolRenderView<'_>,
        retained: &Retained,
        response: &egui::Response,
    ) -> Result<(), AttachmentUiError> {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = retained;
        #[cfg(target_arch = "wasm32")]
        if let Some(caption) = &retained.caption {
            caption.set_hovered(self.expanded.is_some() && response.hovered())?;
        }
        if self.expanded.is_some() && response.has_focus() {
            ui.painter().rect_stroke(
                response.rect,
                0.,
                egui::Stroke::new(1., attachment_content::color(view.presentation, "accent")?),
                egui::StrokeKind::Inside,
            );
        }
        Ok(())
    }
    /// Paint ordered geometry and the literal or rich caption.
    fn row(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
        angle: f32,
    ) -> Result<egui::Response, AttachmentUiError> {
        attachment_content::style(ui, view.presentation)?;
        let collapsible = self.expanded.is_some();
        let pending = self.state == ToolHeaderState::Inprogress;
        let role = match self.state {
            ToolHeaderState::Inprogress => "text",
            ToolHeaderState::Complete => "success",
            ToolHeaderState::Error => "error",
        };
        let mut leading = None;
        let row = ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 16.),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| -> Result<(), AttachmentUiError> {
                let first = ui.ctx().graphics_mut(|g| g.entry(ui.layer_id()).next_idx());
                ui.spacing_mut().item_spacing.x = 8.;
                if pending && collapsible {
                    catalog_icon(ui, view, "Loader", angle)?;
                }
                let rect = ui.allocate_space(egui::vec2(16., 16.)).1;
                attachment_content::paint_icon(
                    ui,
                    rect,
                    &self.tool_icon,
                    (
                        IconSize::Sm,
                        attachment_content::color(view.presentation, role)?,
                        0.,
                    ),
                )?;
                let last = ui.ctx().graphics_mut(|g| g.entry(ui.layer_id()).next_idx());
                leading = Some((first, last, rect.center().y));
                let tail = if pending || collapsible { 24. } else { 0. };
                let width = (ui.available_width() - tail).max(0.);
                ui.allocate_ui_with_layout(
                    egui::vec2(width, 16.),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_height(16.);
                        self.caption(ui, view, retained)
                    },
                )
                .inner?;
                if let Some((name, rotation)) = self.trailing(angle) {
                    catalog_icon(ui, view, name, rotation)?;
                }
                Ok(())
            },
        );
        row.inner?;
        center_leading(ui, leading, row.response.rect.center().y);
        Ok(row.response)
    }
    /// Select the current expansion glyph after drawing caller-owned caption controls.
    fn trailing(&self, angle: f32) -> Option<(&'static str, f32)> {
        if let Some(expanded) = &self.expanded {
            Some((
                if expanded.get() {
                    "ChevronUp"
                } else {
                    "ChevronsUpDown"
                },
                0.,
            ))
        } else if self.state == ToolHeaderState::Inprogress {
            Some(("Loader", angle))
        } else {
            None
        }
    }
    /// Draw rich content without holding the row's state borrow.
    fn caption(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
    ) -> Result<egui::Response, AttachmentUiError> {
        match &self.text {
            ToolHeaderText::Text(text) => {
                retained.rich = None;
                retained
                    .caption
                    .get_or_insert_with(|| NativeText::new(Rc::clone(view.host)))
                    .render(ui, (text, TextKind::Caption), view)
            }
            ToolHeaderText::Content(content) => {
                retained.caption = None;
                retained
                    .rich
                    .get_or_insert_default()
                    .draw(content.as_ref(), ui, view)
            }
        }
    }
}
/// Paint one sixteen-point catalog icon using the shared rasterizer.
fn catalog_icon(
    ui: &mut egui::Ui,
    view: ToolRenderView<'_>,
    name: &str,
    angle: f32,
) -> Result<(), AttachmentUiError> {
    let catalog =
        crate::icons::icons().map_err(|error| AttachmentUiError::Host(error.to_string()))?;
    let node = catalog
        .get(name)
        .ok_or_else(|| AttachmentUiError::Host(format!("Missing glyph: {name}")))?;
    let rect = ui.allocate_space(egui::vec2(16., 16.)).1;
    attachment_content::paint_icon(
        ui,
        rect,
        node,
        (
            IconSize::Sm,
            attachment_content::color(
                view.presentation,
                if name == "Loader" { "text" } else { "muted" },
            )?,
            angle,
        ),
    )
}

/// Centre the already painted leading glyphs without moving native content or its clip.
fn center_leading(
    ui: &egui::Ui,
    leading: Option<(egui::layers::ShapeIdx, egui::layers::ShapeIdx, f32)>,
    center: f32,
) {
    if let Some((first, last, origin)) = leading {
        let shift = egui::emath::TSTransform::from_translation(egui::vec2(0., center - origin));
        ui.ctx().graphics_mut(|g| {
            let list = g.entry(ui.layer_id());
            for n in first.0..last.0 {
                list.mutate_shape(egui::layers::ShapeIdx(n), |s| s.shape.transform(shift));
            }
        });
    }
}
