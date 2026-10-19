//! Expandable literal summary with children captured once by their owner.
use crate::{
    browser,
    components::native_text::{NativeText, TextKind},
    tools::types::{ToolContent, ToolRenderResult, ToolRenderState, ToolRenderView, ToolUiError},
};
use std::{cell::Cell, rc::Rc};
/// Captured owned child and its continuously retained state.
struct Child {
    /// Original captured content.
    recipe: ToolRenderResult,
    /// Row lifetime retained while collapsed.
    state: ToolRenderState,
}
/// Retained disclosure: summary and children keep independent native lifetimes.
pub struct ExpandableSection {
    /// Caller-authored literal summary.
    pub summary: String,
    /// Expansion applied on first render and reconnection.
    pub default_expanded: bool,
    /// Native placement owner.
    host: Rc<dyn browser::attachment_content::AttachmentHost>,
    /// Retained literal summary and weak activation route.
    header: NativeText,
    /// Live expansion state shared with native activation.
    expanded: Rc<Cell<bool>>,
    /// Whether the initial default has been applied.
    connected: bool,
    /// Original children, never recaptured from generated UI.
    children: Vec<Child>,
    /// Placement revocation independent of child ownership.
    child_scope: egui::Id,
    /// Primary press admitted within the visible header.
    pressed: Option<()>,
}
impl ExpandableSection {
    /// Capture the original owned children once.
    #[must_use]
    pub fn new(
        host: Rc<dyn browser::attachment_content::AttachmentHost>,
        children: Vec<Box<dyn ToolContent>>,
    ) -> Self {
        let expanded = Rc::new(Cell::new(false));
        let weak = Rc::downgrade(&expanded);
        let mut header = NativeText::new(Rc::clone(&host));
        header.set_action(Some(Rc::new(move || {
            if let Some(expanded) = weak.upgrade() {
                expanded.set(!expanded.get());
            }
        })));
        Self {
            summary: String::new(),
            default_expanded: false,
            host,
            header,
            expanded,
            connected: false,
            children: children
                .into_iter()
                .map(|content| Child {
                    recipe: ToolRenderResult {
                        content,
                        is_custom: false,
                    },
                    state: ToolRenderState::default(),
                })
                .collect(),
            child_scope: browser::attachment_content::identity(),
            pressed: None,
        }
    }
    /// Reapply the latest default without recapturing children.
    pub fn connected_callback(&mut self) {
        self.expanded.set(self.default_expanded);
        self.connected = true;
    }
    /// Draw the summary and only the currently visible children.
    /// # Errors
    /// Returns reached presentation or native child failures.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> Result<egui::Response, ToolUiError> {
        if !self.connected {
            self.connected_callback();
        }
        let header = self.draw_header(ui, view)?;
        let visible = header.rect.intersect(ui.clip_rect());
        let slots = if header.enabled() && visible.is_positive() {
            vec![((), visible, header.layer_id)]
        } else {
            vec![]
        };
        let events = ui.input(|input| input.events.clone());
        let pointer = !browser::attachment_content::reading::controls::pointer_actions(
            &events,
            &mut self.pressed,
            &slots,
            ui.ctx(),
        )
        .is_empty();
        if pointer || (header.clicked() && !header.clicked_by(egui::PointerButton::Primary)) {
            self.expanded.set(!self.expanded.get());
        }
        if self.expanded.get() {
            let _active = self.host.enter_placement_scope(self.child_scope);
            ui.add_space(8.);
            for child in &mut self.children {
                child.recipe.render(ui, view, &mut child.state)?;
            }
        } else {
            self.host.hide_placement_scope(self.child_scope);
        }
        Ok(header.on_hover_cursor(egui::CursorIcon::PointingHand))
    }
    /// Paint the chevron beside the retained literal summary.
    fn draw_header(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> Result<egui::Response, ToolUiError> {
        browser::attachment_content::style(ui, view.presentation)
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
        let row = ui.horizontal(|ui| -> Result<(), ToolUiError> {
            ui.spacing_mut().item_spacing.x = 8.;
            let node = browser::attachment_content::glyph_node(if self.expanded.get() {
                "ChevronDown"
            } else {
                "ChevronRight"
            })
            .map_err(ToolUiError::Control)?;
            let rect = ui.allocate_space(egui::vec2(16., 16.)).1;
            let role = if ui.rect_contains_pointer(ui.max_rect()) {
                "text"
            } else {
                "muted"
            };
            let color = browser::attachment_content::color(view.presentation, role)
                .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
            browser::attachment_content::paint_icon(
                ui,
                rect,
                &node,
                (crate::icons::IconSize::Sm, color, 0.),
            )
            .map_err(ToolUiError::Control)?;
            self.header
                .render(ui, (&self.summary, TextKind::Summary), view)?;
            Ok(())
        });
        row.inner?;
        let response = ui.interact(
            row.response.rect,
            self.header.id().with("toggle"),
            egui::Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), &self.summary)
        });
        #[cfg(target_arch = "wasm32")]
        self.header.set_hovered(response.hovered())?;
        browser::attachment_content::reading::controls::focus_border(
            ui,
            &response,
            view.presentation,
        )?;
        Ok(response)
    }
}
impl Drop for ExpandableSection {
    fn drop(&mut self) {
        self.host.hide_placement_scope(self.child_scope);
    }
}
