//! User text with caller-owned attachment handles.
use super::reading_view;
use crate::{
    Attachment, AttachmentOverlay, AttachmentTile,
    browser::attachment_content::{
        self, AttachmentContent, AttachmentHost, AttachmentSurface, ReadingContent, ReadingPhase,
        ReadingPlacement,
    },
    components::markdown_block::MarkdownBlock,
    tools::types::{ToolContentOutput, ToolRenderView, ToolUiError},
};
use maestro_app::{UserBlock, UserContent};
use std::{cell::RefCell, rc::Rc};

/// Borrow existing user content without creating another storage record.
#[derive(Clone, Copy)]
pub enum UserMessageInput<'a> {
    /// Ordinary model-facing user record, without attachment tiles.
    User(&'a maestro_app::UserMessage),
    /// Browser content with its original optional attachment handles.
    WithAttachments {
        /// Text or ordered text/image blocks.
        content: &'a UserContent,
        /// Independently owned attachments, in display order.
        attachments: Option<&'a [Rc<RefCell<Attachment>>]>,
    },
}
/// One retained user row, its Markdown and positional tiles.
pub struct UserMessage {
    /// Shared platform effects.
    host: Rc<dyn AttachmentHost>,
    /// Retained first-text reading owner.
    text: MarkdownBlock,
    /// Tiles retained by emitted position.
    tiles: Vec<AttachmentTile>,
    /// Fixed containing decoration, independent of each child's scrolling viewport.
    decoration: Rc<ReadingContent>,
    /// Stable placement lifetime for the containing decoration.
    decoration_id: egui::Id,
}
impl UserMessage {
    /// Bind the row to the supplied platform.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            decoration: decoration(),
            decoration_id: attachment_content::identity(),
            text: MarkdownBlock::for_user(Rc::clone(&host)),
            host,
            tiles: Vec::new(),
        }
    }
    /// Render first text and attachment tiles, returning independent opened overlays.
    /// # Errors
    /// Returns reached reading, attachment or presentation failures.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        message: UserMessageInput<'_>,
        view: ToolRenderView<'_>,
    ) -> ToolContentOutput {
        let mut overlays = Vec::new();
        let response = self.render_body(ui, message, view, &mut overlays);
        ToolContentOutput { response, overlays }
    }
    /// Draw the row while transferring opened owners before any later error.
    fn render_body(
        &mut self,
        ui: &mut egui::Ui,
        message: UserMessageInput<'_>,
        view: ToolRenderView<'_>,
        overlays: &mut Vec<AttachmentOverlay>,
    ) -> Result<egui::Response, ToolUiError> {
        let (content, attachments) = match message {
            UserMessageInput::User(message) => (&message.content, &[][..]),
            UserMessageInput::WithAttachments {
                content,
                attachments,
            } => (content, attachments.unwrap_or_default()),
        };
        let source = match content {
            UserContent::Text(text) => text.as_str(),
            UserContent::Blocks(blocks) => blocks
                .iter()
                .find_map(|block| match block {
                    UserBlock::Text(text) => Some(text.text.as_str()),
                    UserBlock::Image(_) => None,
                })
                .unwrap_or_default(),
        };
        self.text.content.clear();
        self.text.content.push_str(source);
        self.tiles.truncate(attachments.len());
        for (index, attachment) in attachments.iter().enumerate() {
            AttachmentTile::retain_at(&mut self.tiles, index, Rc::clone(attachment), &self.host);
        }
        self.place(
            ui,
            ui.available_rect_before_wrap(),
            ReadingPhase::Measure,
            view,
        )?;
        let border = self.host.reading_layout(self.decoration_id)?.border;
        let mut frame = egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(16, 8))
            .outer_margin(egui::Margin::symmetric(16, 0))
            .begin(ui);
        let mut content = frame.content_ui.new_child(
            egui::UiBuilder::new().max_rect(frame.content_ui.available_rect_before_wrap() - border),
        );
        self.draw(&mut content, view, overlays)?;
        frame
            .content_ui
            .allocate_rect(content.min_rect() + border, egui::Sense::hover());
        let response = frame.end(ui);
        self.place(
            ui,
            response.rect.shrink2(egui::vec2(16., 0.)),
            ReadingPhase::Publish,
            view,
        )?;
        Ok(response)
    }
    /// Measure before children, then publish the same candidate so paint stays behind text.
    fn place(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        phase: ReadingPhase,
        view: ToolRenderView<'_>,
    ) -> Result<(), ToolUiError> {
        self.host.present(
            AttachmentSurface {
                id: self.decoration_id,
                rect,
                clip_rect: ui.clip_rect(),
                pixels_per_point: ui.ctx().pixels_per_point(),
                opacity: 1.,
                occluders: vec![],
                content: AttachmentContent::Reading(ReadingPlacement {
                    intrinsic_width: false,
                    content: Rc::clone(&self.decoration),
                    phase,
                    sizes: &[],
                    modal: view.modal,
                    context: ui.ctx(),
                }),
            },
            view.presentation,
        )?;
        Ok(())
    }
    /// Draw children without retaining frame operands or attachment record borrows.
    fn draw(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        overlays: &mut Vec<AttachmentOverlay>,
    ) -> Result<(), ToolUiError> {
        self.text
            .render(ui, reading_view(view))
            .map_err(ToolUiError::reading)?;
        if self.tiles.is_empty() {
            return Ok(());
        }
        ui.add_space(12.);
        ui.horizontal_wrapped(|ui| -> Result<(), ToolUiError> {
            ui.spacing_mut().item_spacing = egui::vec2(8., 8.);
            for tile in &mut self.tiles {
                overlays.extend(
                    tile.render(ui, view.i18n, view.presentation)
                        .map_err(ToolUiError::Attachment)?,
                );
            }
            Ok(())
        })
        .inner
    }
}

impl Drop for UserMessage {
    fn drop(&mut self) {
        self.host.remove(self.decoration_id);
    }
}
/// Mount containing paint through the existing native frame rather than a scrolling text body.
fn decoration() -> Rc<ReadingContent> {
    Rc::new(ReadingContent {
        #[cfg(target_arch = "wasm32")]
        mount: Rc::new(|document| {
            use attachment_content::browser::{append, element};
            let decoration = element(document, "div", "maestro-user-decoration")?;
            let style = element(document, "style", "")?;
            style.set_text_content(Some(include_str!("../messages.css")));
            append(&decoration, &style)?;
            Ok(attachment_content::ReadingRoot {
                frame_style: "border-radius:0.75rem;backdrop-filter:blur(10px);",
                decoration: Some(decoration),
                element: element(document, "div", "")?,
                slots: vec![],
                owner: None,
                commit: None,
            })
        }),
    })
}
