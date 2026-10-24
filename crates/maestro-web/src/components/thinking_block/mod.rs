//! Retained reasoning disclosure with native text and Markdown.
use crate::browser::transition::{Transition, TransitionStyle};
use crate::{
    browser::attachment_content::{self, AttachmentHost},
    components::{
        markdown_block::MarkdownBlock,
        native_text::{NativeText, TextKind},
    },
    tools::types::{ToolRenderView, ToolUiError},
};
use std::{cell::Cell, rc::Rc};

#[cfg(target_arch = "wasm32")]
pub(crate) mod dom;

/// Independently retained collapsed reasoning disclosure.
///
/// [`Default`] content mounts as native page content through
/// [`DomContent`](crate::page::DomContent); [`ThinkingBlock::new`] draws on a canvas host.
#[derive(Default)]
pub struct ThinkingBlock {
    /// Original reasoning source, never trimmed for display.
    pub content: String,
    /// Whether the literal header shimmers.
    pub is_streaming: bool,
    /// Canvas drawing state, absent for page content.
    canvas: Option<Canvas>,
}
/// Disclosure state of the canvas drawing.
struct Canvas {
    /// Platform effects for newly expanded Markdown.
    host: Rc<dyn AttachmentHost>,
    /// Expansion shared only with weak native callbacks.
    expanded: Rc<Cell<bool>>,
    /// Literal header and its owned native subscription.
    header: NativeText,
    /// Body exists only while expanded.
    body: Option<MarkdownBlock>,
    /// Mounted color and rotation properties, independent of body lifetime.
    transitions: Option<(Transition<3>, Transition<1>)>,
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
            canvas: Some(Canvas {
                host,
                expanded,
                header,
                body: None,
                pressed: None,
                transitions: None,
            }),
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
        let canvas = self.canvas.as_mut().ok_or_else(|| {
            attachment_content::AttachmentHostError::Host("Missing platform host".into())
        })?;
        canvas.render(ui, view, (&self.content, self.is_streaming))
    }
}
impl Canvas {
    /// Draw the disclosure of `content`, shimmering while `streaming`.
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        (content, streaming): (&str, bool),
    ) -> Result<egui::Response, ToolUiError> {
        let header = self.draw_header(ui, view, streaming)?;
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
            content.clone_into(&mut body.content);
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
        streaming: bool,
    ) -> Result<egui::Response, ToolUiError> {
        let header = ui.horizontal(|ui| -> Result<egui::Rect, ToolUiError> {
            ui.spacing_mut().item_spacing.x = 8.;
            let rect = ui.allocate_space(egui::vec2(16., 16.)).1;
            self.header
                .render(ui, ("Thinking...", TextKind::Thinking(streaming)), view)?;
            Ok(rect)
        });
        let rect = header.inner?;
        let hovered = header.response.hovered();
        #[cfg(target_arch = "wasm32")]
        if !streaming {
            self.header.set_hovered(hovered)?;
        }
        self.paint_chevron(ui, rect, hovered, view)?;
        Ok(header.response)
    }
    /// Sample the existing property owner before painting either changing property.
    fn paint_chevron(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        hovered: bool,
        view: ToolRenderView<'_>,
    ) -> Result<(), ToolUiError> {
        let color =
            attachment_content::color(view.presentation, if hovered { "text" } else { "muted" })
                .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
        let [r, g, b, _] = color.to_array();
        let target = [r, g, b].map(f32::from);
        let rotation = [if self.expanded.get() {
            std::f32::consts::FRAC_PI_2
        } else {
            0.
        }];
        let transitions = self
            .transitions
            .get_or_insert_with(|| (Transition::new(target), Transition::new(rotation)));
        let now = ui.input(|input| input.time);
        let style = TransitionStyle {
            duration: 0.15,
            curve: [0.4, 0., 0.2, 1.],
        };
        let color = transitions.0.sample(now, target, style);
        let rotation = transitions.1.sample(now, rotation, style);
        if color.active || rotation.active {
            ui.ctx().request_repaint();
        }
        let [r, g, b] = color.value.map(attachment_content::channel);
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
                egui::Color32::from_rgb(r, g, b),
                rotation.value[0],
            ),
        )?;
        Ok(())
    }
}
