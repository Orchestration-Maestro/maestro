//! Transcript Markdown with highlighted code blocks, copy controls and math.
use super::code_block::{self, CodeBlockView, Prepared as Code};
use crate::components::code_block::ReadingUiError;
use crate::{
    browser::attachment_content::{
        AttachmentHost, AttachmentHostError, ReadingContent,
        reading::controlled::{ControlledReading, ReadingHeight},
    },
    components::copy_button::CopyButton,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

mod admission;
pub(super) mod browser;
#[cfg(target_arch = "wasm32")]
pub(crate) mod dom;
mod math;
mod render;
pub(crate) use render::export::prepare_export;
#[cfg(test)]
mod tests;

/// Code listings shared by native controls and the browser document.
pub(crate) struct Document {
    /// Listings in slot order.
    pub(crate) code: Vec<Code>,
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
    /// Browser-owned markup accompanying the portable listings.
    markup: browser::Markup,
    /// Mounted owner of this published factory, never a speculative replacement.
    mounted: Rc<RefCell<Weak<browser::Mounted>>>,
    /// Successfully published reasoning presentation.
    thinking: bool,
}

/// Native document width and padding policy.
enum DocumentLayout {
    /// Fill the supplied flow width without inner padding.
    Flow,
    /// Fill an artifact viewport with document padding.
    Padded,
}

/// Markdown content rendered as selectable native text with copy controls.
///
/// [`Default`] content mounts as native page content through
/// [`DomContent`](crate::dom::DomContent); [`MarkdownBlock::new`] draws on a canvas host.
pub struct MarkdownBlock {
    /// Markdown source.
    pub content: String,
    /// Present reasoning text muted, italic and small.
    pub is_thinking: bool,
    /// Show tag-like text literally instead of as markup.
    pub escape_html: bool,
    /// Native document width and padding policy.
    layout: DocumentLayout,
    /// Replaceable platform effects of the canvas drawing, absent for page content.
    host: Option<Rc<dyn AttachmentHost>>,
    /// Stable native surface identity.
    id: egui::Id,
    /// Last successfully published document.
    published: Option<Published>,
}
impl Default for MarkdownBlock {
    /// Empty, ordinary and escaped page content.
    fn default() -> Self {
        Self {
            content: String::new(),
            is_thinking: false,
            escape_html: true,
            layout: DocumentLayout::Flow,
            host: None,
            id: crate::browser::attachment_content::identity(),
            published: None,
        }
    }
}
impl MarkdownBlock {
    /// Retain the platform effects; content is empty, ordinary and escaped.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        let mut block = Self::default();
        block.host = Some(host);
        block
    }
    /// The canvas platform effects.
    fn host(&self) -> Result<&Rc<dyn AttachmentHost>, AttachmentHostError> {
        self.host
            .as_ref()
            .ok_or_else(|| AttachmentHostError::Host("Missing platform host".into()))
    }
    /// Retain artifact padding inside the shared scrollable document.
    pub(crate) fn for_artifact(host: Rc<dyn AttachmentHost>) -> Self {
        let mut block = Self::new(host);
        block.layout = DocumentLayout::Padded;
        block
    }
    /// Forward retained containing-owner admission to the native root.
    pub(crate) fn set_visibility(
        &self,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), AttachmentHostError> {
        self.host()?
            .set_retained_visibility(self.id, visible, modal)
    }
    /// Prepare the current source, replacing the published document only on success.
    fn prepare<F: ?Sized>(&self, view: CodeBlockView<'_, F>) -> Result<Published, ReadingUiError> {
        let host = self.host().map_err(ReadingUiError::Host)?;
        let mut prepared = render::prepare(&self.content, self.escape_html);
        let code = std::mem::take(&mut prepared.code)
            .into_iter()
            .map(|c| Code::new(view.highlighter, c.code, c.language))
            .collect::<Result<Vec<_>, _>>()?;
        let buttons = code
            .iter()
            .map(|_| CopyButton::new(Rc::clone(host), view.i18n))
            .collect::<Result<Vec<_>, _>>()?;
        let document = Rc::new(Document { code });
        let markup = browser::prepare(prepared, &document);
        let ids = buttons.iter().map(|b| b.id).collect();
        let mounted = Rc::default();
        Ok(Published {
            key: (self.content.clone(), self.escape_html),
            content: browser::content(
                &markup,
                ids,
                (
                    self.is_thinking,
                    matches!(self.layout, DocumentLayout::Padded),
                    &mounted,
                ),
                browser::Update::Replace,
            ),
            mounted,
            thinking: self.is_thinking,
            buttons,
            document,
            markup,
        })
    }
    /// Draw the document and its code controls for this frame.
    /// # Errors
    /// Returns localization, highlighting, glyph, presentation or native placement failures.
    pub fn render<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        view: CodeBlockView<'_, F>,
    ) -> Result<egui::Response, ReadingUiError> {
        let current = match &self.published {
            Some(p) if p.key.0 == self.content && p.key.1 == self.escape_html => {
                p.document.code.iter().try_fold(true, |current, code| {
                    let html = Code::highlight(view.highlighter, &code.text, &code.language)?;
                    Ok::<_, ReadingUiError>(current && html == code.html)
                })?
            }
            _ => false,
        };
        let mut candidate = (!current).then(|| self.prepare(view)).transpose()?;
        let host = Rc::clone(self.host().map_err(ReadingUiError::Host)?);
        let Some(target) = candidate.as_mut().or(self.published.as_mut()) else {
            return Err(ReadingUiError::Host(AttachmentHostError::Host(
                "Missing document".into(),
            )));
        };
        let sizes = sync_controls(target, ui, view)?;
        let mounted = Rc::default();
        let style = (target.thinking != self.is_thinking).then(|| {
            browser::content(
                &target.markup,
                target.buttons.iter().map(|button| button.id).collect(),
                (
                    self.is_thinking,
                    matches!(self.layout, DocumentLayout::Padded),
                    &mounted,
                ),
                browser::Update::Presentation,
            )
        });
        let layout = ControlledReading {
            intrinsic_width: false,
            height: ReadingHeight::Flow,
            host: host.as_ref(),
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
            let responses = code_block::controls(
                ui,
                &layout,
                published.buttons.iter_mut(),
                (view.i18n, view.presentation),
            )?;
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
) -> Result<Vec<(egui::Id, egui::Vec2)>, ReadingUiError> {
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
        if let Some(host) = &self.host {
            host.remove(self.id);
        }
    }
}
