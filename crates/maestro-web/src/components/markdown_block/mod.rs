//! Transcript Markdown with highlighted code blocks, copy controls and math.
use super::code_block::{self, CodeBlockView, Prepared as Code};
use crate::{
    browser::attachment_content::{
        AttachmentHost, AttachmentUiError, ReadingContent, reading::controlled::ControlledReading,
    },
    components::copy_button::CopyButton,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

mod admission;
pub(super) mod browser;
mod render;

/// Complete native content: document HTML and its code listings in slot order.
pub(crate) struct Document {
    /// Document markup with one empty slot per listing.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        expect(dead_code, reason = "read only by the browser mount")
    )]
    pub(crate) html: String,
    /// Listings in slot order.
    pub(crate) code: Vec<Code>,
    /// Generated-only listing attribute, absent from authored markup.
    #[cfg(target_arch = "wasm32")]
    pub(crate) slot_attribute: String,
}

/// The last successfully published document.
struct Published {
    /// Source and escaping the document was prepared from.
    key: (String, bool),
    /// Native factory for the retained root.
    content: Rc<ReadingContent>,
    /// Copy controls in listing order, with the document they belong to.
    buttons: Vec<CopyButton>,
    /// Native content, shared with the mount.
    document: Rc<Document>,
    /// Mounted owner of this published factory, never a speculative replacement.
    mounted: Rc<RefCell<Weak<browser::Mounted>>>,
    /// Successfully published reasoning presentation.
    thinking: bool,
}

/// Markdown content rendered as selectable native text with canvas copy controls.
pub struct MarkdownBlock {
    /// Markdown source.
    pub content: String,
    /// Present reasoning text muted, italic and small.
    pub is_thinking: bool,
    /// Show tag-like text literally instead of as markup.
    pub escape_html: bool,
    /// Replaceable platform effects.
    host: Rc<dyn AttachmentHost>,
    /// Stable native surface identity.
    id: egui::Id,
    /// Last successfully published document.
    published: Option<Published>,
}
impl MarkdownBlock {
    /// Retain the platform effects; content is empty, ordinary and escaped.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            content: String::new(),
            is_thinking: false,
            escape_html: true,
            host,
            id: crate::browser::attachment_content::identity(),
            published: None,
        }
    }
    /// Prepare the current source, replacing the published document only on success.
    fn prepare<F: ?Sized>(
        &self,
        view: CodeBlockView<'_, F>,
    ) -> Result<Published, AttachmentUiError> {
        let prepared = render::prepare(&self.content, self.escape_html);
        let code = prepared
            .code
            .into_iter()
            .map(|c| Code::new(view.highlighter, c.code, c.language))
            .collect::<Result<Vec<_>, _>>()?;
        let buttons = code
            .iter()
            .map(|_| CopyButton::new(Rc::clone(&self.host), view.i18n))
            .collect::<Result<Vec<_>, _>>()?;
        let document = Rc::new(Document {
            html: prepared.html,
            #[cfg(target_arch = "wasm32")]
            slot_attribute: prepared.slot_attribute,
            code,
        });
        let ids = buttons.iter().map(|b| b.id).collect();
        let mounted = Rc::default();
        Ok(Published {
            key: (self.content.clone(), self.escape_html),
            content: browser::content(
                &document,
                ids,
                (self.is_thinking, &mounted),
                browser::Update::Replace,
            ),
            mounted,
            thinking: self.is_thinking,
            buttons,
            document,
        })
    }
    /// Draw the document and its code controls for this frame.
    /// # Errors
    /// Returns localization, highlighting, glyph, presentation or native placement failures.
    pub fn render<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        view: CodeBlockView<'_, F>,
    ) -> Result<egui::Response, AttachmentUiError> {
        let current = match &self.published {
            Some(p) if p.key.0 == self.content && p.key.1 == self.escape_html => {
                p.document.code.iter().try_fold(true, |current, code| {
                    let html = Code::highlight(view.highlighter, &code.text, &code.language)?;
                    Ok::<_, AttachmentUiError>(current && html == code.html)
                })?
            }
            _ => false,
        };
        let mut candidate = (!current).then(|| self.prepare(view)).transpose()?;
        let Some(target) = candidate.as_mut().or(self.published.as_mut()) else {
            return Err(AttachmentUiError::Host("Missing document".into()));
        };
        let sizes = sync_controls(target, ui, view)?;
        let mounted = Rc::default();
        let style = (target.thinking != self.is_thinking).then(|| {
            browser::content(
                &target.document,
                target.buttons.iter().map(|button| button.id).collect(),
                (self.is_thinking, &mounted),
                browser::Update::Presentation,
            )
        });
        let layout = ControlledReading {
            host: self.host.as_ref(),
            id: self.id,
            content: style.as_ref().unwrap_or(&target.content),
            sizes: &sizes,
            modal: view.modal,
            presentation: view.presentation,
        }
        .draw(ui)?;
        if let Some(content) = style {
            target.content = content;
            target.mounted = mounted;
            target.thinking = self.is_thinking;
        }
        if candidate.is_some() {
            self.published = candidate;
        }
        if let Some(published) = self.published.as_mut() {
            let responses = code_block::controls(ui, &layout, published.buttons.iter_mut(), view)?;
            self.reveal_focused(&responses);
        }
        Ok(layout.response)
    }
    /// Scroll the native viewport to a control that just gained keyboard focus.
    fn reveal_focused(&self, responses: &[Option<egui::Response>]) {
        let Some(mounted) = self
            .published
            .as_ref()
            .and_then(|p| p.mounted.borrow().upgrade())
        else {
            return;
        };
        responses
            .iter()
            .enumerate()
            .filter(|(_, r)| r.as_ref().is_some_and(egui::Response::gained_focus))
            .for_each(|(index, _)| mounted.reveal(index));
    }
}
/// Give each copy control its listing text and localized title, and measure it.
fn sync_controls<F: ?Sized>(
    target: &mut Published,
    ui: &egui::Ui,
    view: CodeBlockView<'_, F>,
) -> Result<Vec<(egui::Id, egui::Vec2)>, AttachmentUiError> {
    let title = view.i18n.text("Copy code")?;
    target
        .buttons
        .iter_mut()
        .zip(&target.document.code)
        .map(|(button, code)| {
            button.text.clone_from(&code.text);
            button.title.clone_from(&title);
            button.show_text = true;
            Ok((button.id, button.size(ui, view.i18n)?))
        })
        .collect()
}
impl Drop for MarkdownBlock {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
