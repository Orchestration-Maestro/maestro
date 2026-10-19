//! Retained reasoning disclosure with native text and Markdown.
use crate::{
    browser::attachment_content::{self, AttachmentHost},
    components::{
        markdown_block::MarkdownBlock,
        native_text::{NativeText, TextKind},
    },
    tools::types::{ToolRenderView, ToolUiError},
};
use std::{cell::Cell, rc::Rc};

/// Independently retained collapsed reasoning disclosure.
pub struct ThinkingBlock {
    /// Original reasoning source, never trimmed for display.
    pub content: String,
    /// Whether the literal header shimmers.
    pub is_streaming: bool,
    /// Platform effects for newly expanded Markdown.
    host: Rc<dyn AttachmentHost>,
    /// Expansion shared only with weak native callbacks.
    expanded: Rc<Cell<bool>>,
    /// Literal header and its owned native subscription.
    header: NativeText,
    /// Body exists only while expanded.
    body: Option<MarkdownBlock>,
    /// Primary gesture admitted by the current clipped header.
    pressed: Option<()>,
}
impl ThinkingBlock {
    /// Bind a collapsed disclosure to the supplied platform.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        let expanded = Rc::new(Cell::new(false));
        let weak = Rc::downgrade(&expanded);
        let mut header = NativeText::new(Rc::clone(&host));
        header.set_action(Some(Rc::new(move || {
            if let Some(expanded) = weak.upgrade() {
                expanded.set(!expanded.get());
            }
        })));
        Self {
            content: String::new(),
            is_streaming: false,
            host,
            expanded,
            header,
            body: None,
            pressed: None,
        }
    }
    /// Draw the disclosure, retaining expansion across content and streaming updates.
    /// # Errors
    /// Returns reached native, glyph, localization or Markdown preparation failures.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> Result<egui::Response, ToolUiError> {
        let header = self.draw_header(ui, view)?;
        let response = header.on_hover_cursor(egui::CursorIcon::PointingHand);
        let visible = response.rect.intersect(ui.clip_rect());
        let slots = if response.enabled() && visible.is_positive() {
            vec![((), visible, response.layer_id)]
        } else {
            vec![]
        };
        let events = ui.input(|input| input.events.clone());
        if !attachment_content::reading::controls::pointer_actions(
            &events,
            &mut self.pressed,
            &slots,
            ui.ctx(),
        )
        .is_empty()
        {
            self.expanded.set(!self.expanded.get());
        }
        if self.expanded.get() {
            let body = self
                .body
                .get_or_insert_with(|| MarkdownBlock::new(Rc::clone(&self.host)));
            body.is_thinking = true;
            body.content.clone_from(&self.content);
            body.render(ui, super::messages::reading_view(view))
                .map_err(ToolUiError::reading)?;
        } else {
            self.body = None;
        }
        Ok(response)
    }
    /// Compose the disclosure glyph and its literal native text.
    fn draw_header(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> Result<egui::Response, ToolUiError> {
        let header = ui.horizontal(|ui| -> Result<egui::Rect, ToolUiError> {
            ui.spacing_mut().item_spacing.x = 8.;
            let rect = ui.allocate_space(egui::vec2(16., 16.)).1;
            self.header.render(
                ui,
                ("Thinking...", TextKind::Thinking(self.is_streaming)),
                view,
            )?;
            Ok(rect)
        });
        let rect = header.inner?;
        let hovered = header.response.hovered();
        #[cfg(target_arch = "wasm32")]
        if !self.is_streaming {
            self.header.set_hovered(hovered)?;
        }
        let icons =
            crate::icons::icons().map_err(|error| ToolUiError::Presentation(error.to_string()))?;
        let icon = icons
            .get("ChevronRight")
            .ok_or_else(|| ToolUiError::Presentation("Missing glyph: ChevronRight".into()))?;
        attachment_content::paint_icon(
            ui,
            rect,
            icon,
            (
                crate::icons::IconSize::Sm,
                attachment_content::color(
                    view.presentation,
                    if hovered { "text" } else { "muted" },
                )
                .map_err(|error| ToolUiError::Presentation(error.to_string()))?,
                if self.expanded.get() {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.
                },
            ),
        )?;
        Ok(header.response)
    }
}
