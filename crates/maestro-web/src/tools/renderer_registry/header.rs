//! Shared status icons, rich captions and collapsible header controls.
use super::ToolHeaderState;
use crate::tools::types::ToolUiError;
use crate::{
    browser::attachment_content::{self},
    icons::{IconNode, IconSize},
    tools::types::{ToolContent, ToolContentOutput, ToolRenderState, ToolRenderView},
};
use maestro_app::ToolRendererState;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Per-header nested action admission, shared only with its rich caption.
#[derive(Default)]
pub(crate) struct InlineActions {
    /// Bounds consumed by the current nested action.
    pub(crate) excluded: Cell<Option<egui::Rect>>,
    /// The containing disclosure's single native activation route.
    pub(crate) activation: RefCell<Option<Rc<dyn Fn()>>>,
}
impl InlineActions {
    /// Weak native activation route, with no callback invoked under a borrow.
    pub(crate) fn action(owner: &Rc<Self>) -> Rc<dyn Fn()> {
        let weak = Rc::downgrade(owner);
        Rc::new(move || {
            let action = weak
                .upgrade()
                .and_then(|owner| owner.activation.borrow().clone());
            if let Some(action) = action {
                action();
            }
        })
    }
}
/// Compose a rich card caption with the existing header action owner.
pub(crate) fn render_inline_header(
    state: ToolHeaderState,
    tool_icon: IconNode,
    text: ToolHeaderText,
    expanded: Option<Rc<Cell<bool>>>,
    actions: Rc<InlineActions>,
) -> Box<dyn ToolContent> {
    Box::new(Header {
        state,
        tool_icon,
        text,
        expanded,
        actions: Some(actions),
    })
}

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
        actions: None,
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
        actions: None,
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
    /// Optional nested rich-caption action admission.
    actions: Option<Rc<InlineActions>>,
}
/// State of one continuously mounted header.
#[derive(Default)]
struct Retained {
    /// Plain template identity; collapsible headers share one template across statuses.
    plain_state: Option<ToolHeaderState>,
    /// Stable button identity for this retained header state.
    button_id: Option<egui::Id>,
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
    ) -> ToolContentOutput {
        ToolContentOutput::retained(state, |retained: &mut Retained, overlays| {
            let plain_state = self.expanded.is_none().then_some(self.state);
            if plain_state.is_some()
                && retained.plain_state.is_some()
                && retained.plain_state != plain_state
            {
                *retained = Retained::default();
            }
            retained.plain_state = plain_state;
            let time = ui.input(|input| input.time);
            let angle = if self.state == ToolHeaderState::Inprogress {
                let start = *retained.pending_since.get_or_insert(time);
                ui.ctx().request_repaint();
                attachment_content::points((time - start).rem_euclid(1.) * std::f64::consts::TAU)
            } else {
                retained.pending_since = None;
                0.
            };
            self.draw(ui, view, retained, (angle, overlays))
        })
    }
}
impl Header {
    /// Resolve inherited caption foreground after the complete header has been measured.
    fn draw(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
        paint: (f32, &mut Vec<crate::AttachmentOverlay>),
    ) -> Result<egui::Response, ToolUiError> {
        #[cfg(target_arch = "wasm32")]
        let captions = RefCell::new(Vec::new());
        #[cfg(target_arch = "wasm32")]
        let view = ToolRenderView {
            captions: Some(&captions),
            ..view
        };
        let response = self.draw_row(ui, view, retained, paint)?;
        #[cfg(target_arch = "wasm32")]
        for node in captions.into_inner() {
            NativeText::set_target_hovered(&node, self.expanded.is_some() && response.hovered())?;
        }
        Ok(response)
    }
    /// Compose icon order, caption and optional trailing control.
    fn draw_row(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
        (angle, overlays): (f32, &mut Vec<crate::AttachmentOverlay>),
    ) -> Result<egui::Response, ToolUiError> {
        let collapsible = self.expanded.is_some();
        if let Some(actions) = &self.actions {
            actions.excluded.set(None);
        }
        let row = self.row(ui, view, retained, (angle, overlays))?;
        let response = if collapsible {
            let id = *retained
                .button_id
                .get_or_insert_with(attachment_content::identity);
            ui.interact(row.rect, id, egui::Sense::click())
        } else {
            row
        };
        if let Some(expanded) = &self.expanded {
            let visible = response.rect.intersect(ui.clip_rect());
            let pointer = self.pointer(ui, retained, &response);
            if let Some(actions) = &self.actions {
                let next = crate::components::expandable_section::toggle_action(
                    &response, visible, expanded,
                );
                let old = actions.activation.replace(Some(next));
                drop(old);
            }
            if pointer || (response.clicked() && !response.clicked_by(egui::PointerButton::Primary))
            {
                crate::components::expandable_section::activate_toggle(
                    &response, visible, expanded,
                );
            }
            if let Some(caption) = &mut retained.caption {
                caption.set_action(Some(crate::components::expandable_section::toggle_action(
                    &response, visible, expanded,
                )));
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
    /// Reuse shared two-endpoint admission outside nested rich action bounds.
    fn pointer(&self, ui: &egui::Ui, retained: &mut Retained, response: &egui::Response) -> bool {
        let visible = response.rect.intersect(ui.clip_rect());
        let slots = if response.enabled() && visible.is_positive() {
            vec![((), visible, response.layer_id)]
        } else {
            vec![]
        };
        let mut pointer = false;
        for event in ui.input(|input| input.events.clone()) {
            let excluded = matches!(&event, egui::Event::PointerButton { pos, .. }
                if self.actions.as_ref().and_then(|a| a.excluded.get()).is_some_and(|rect| rect.contains(*pos)));
            if excluded {
                retained.pressed = None;
                continue;
            }
            pointer |= !attachment_content::reading::controls::pointer_actions(
                &[event],
                &mut retained.pressed,
                &slots,
                ui.ctx(),
            )
            .is_empty();
        }
        pointer
    }
    /// Keep literal hover color and keyboard focus paint inside the caller's clip.
    fn decorate(
        &self,
        ui: &egui::Ui,
        view: ToolRenderView<'_>,
        retained: &Retained,
        response: &egui::Response,
    ) -> Result<(), ToolUiError> {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = retained;
        #[cfg(target_arch = "wasm32")]
        if let Some(caption) = &retained.caption {
            caption.set_hovered(self.expanded.is_some() && response.hovered())?;
        }
        if self.expanded.is_some() {
            attachment_content::reading::controls::focus_border(ui, response, view.presentation)?;
        }
        Ok(())
    }
    /// Paint ordered geometry and the literal or rich caption.
    fn row(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
        (angle, overlays): (f32, &mut Vec<crate::AttachmentOverlay>),
    ) -> Result<egui::Response, ToolUiError> {
        attachment_content::style(ui, view.presentation)
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
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
            |ui| -> Result<(), ToolUiError> {
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
                        attachment_content::color(view.presentation, role)
                            .map_err(|error| ToolUiError::Presentation(error.to_string()))?,
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
                        self.caption(ui, view, retained, overlays)
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
        overlays: &mut Vec<crate::AttachmentOverlay>,
    ) -> Result<egui::Response, ToolUiError> {
        match &self.text {
            ToolHeaderText::Text(text) => {
                retained.rich = None;
                let caption = retained
                    .caption
                    .get_or_insert_with(|| NativeText::new(Rc::clone(view.host)));
                let kind = if self.expanded.is_some() {
                    TextKind::Summary
                } else {
                    caption.set_action(None);
                    TextKind::Caption
                };
                caption.render(ui, (text, kind), view)
            }
            ToolHeaderText::Content(content) => {
                retained.caption = None;
                retained
                    .rich
                    .get_or_insert_default()
                    .draw(content.as_ref(), ui, view)
                    .collect(overlays)
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
) -> Result<(), ToolUiError> {
    let node = attachment_content::glyph_node(name).map_err(ToolUiError::Control)?;
    let rect = ui.allocate_space(egui::vec2(16., 16.)).1;
    attachment_content::paint_icon(
        ui,
        rect,
        &node,
        (
            IconSize::Sm,
            attachment_content::color(
                view.presentation,
                if name == "Loader" { "text" } else { "muted" },
            )
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?,
            angle,
        ),
    )
    .map_err(ToolUiError::Control)
}

/// Centre the already painted leading glyphs without moving native content or its clip.
pub(crate) fn center_leading(
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
