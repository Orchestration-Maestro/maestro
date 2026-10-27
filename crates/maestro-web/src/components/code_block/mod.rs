//! Highlighted code with a language label and an original-text copy control.
mod ui_error;
use crate::{
    I18n,
    browser::attachment_content::{
        AttachmentHost, ReadingContent,
        reading::controlled::{ControlledLayout, ControlledReading, ReadingHeight},
    },
    components::copy_button::CopyButton,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
pub use ui_error::ReadingUiError;

pub(crate) mod browser;
#[cfg(target_arch = "wasm32")]
pub(crate) mod dom;

/// Borrowed operands of one frame, never retained by native callbacks.
pub struct CodeBlockView<'a, F: ?Sized> {
    /// Localization for the label and copy texts.
    pub i18n: &'a I18n<F>,
    /// Selected brand projection.
    pub presentation: &'a maestro_theme::BrandPresentation<'a>,
    /// Shared syntax engine.
    pub highlighter: &'a dyn maestro_theme::SyntaxHighlighter,
    /// Containing modal, never inferred from foreground state.
    pub modal: Option<egui::Id>,
}
impl<F: ?Sized> Clone for CodeBlockView<'_, F> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<F: ?Sized> Copy for CodeBlockView<'_, F> {}

/// Complete native content of one code block.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Prepared {
    /// Language label; empty displays as `plaintext`.
    pub(crate) language: String,
    /// Original text copied by the control.
    pub(crate) text: String,
    /// Escaped highlighted markup.
    pub(crate) html: String,
}
impl Prepared {
    /// Highlight `text`, admitting the language through the shared engine.
    pub(crate) fn new(
        highlighter: &dyn maestro_theme::SyntaxHighlighter,
        text: String,
        language: String,
    ) -> Result<Self, ReadingUiError> {
        let html = Self::highlight(highlighter, &text, &language)?;
        Ok(Self {
            language,
            text,
            html,
        })
    }
    /// Render the current engine's output without copying retained source operands.
    pub(crate) fn highlight(
        highlighter: &dyn maestro_theme::SyntaxHighlighter,
        text: &str,
        language: &str,
    ) -> Result<String, ReadingUiError> {
        maestro_theme::highlight_auto(
            highlighter,
            text,
            Some(language).filter(|l| !l.is_empty()),
            maestro_theme::HighlightProfile::CodeBlock,
        )
        .map_err(ReadingUiError::Highlight)
    }
    /// The displayed and class language.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn label(&self) -> &str {
        if self.language.is_empty() {
            "plaintext"
        } else {
            &self.language
        }
    }
}

/// Decode base64 UTF-8 text, keeping the whole input when it is not.
fn decode(input: &str) -> String {
    crate::utils::base64::decode(input)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| input.to_owned())
}

/// Paint the slot-owned copy controls at their measured native rectangles.
pub(crate) fn controls<'a, F: ?Sized>(
    ui: &mut egui::Ui,
    layout: &ControlledLayout,
    buttons: impl IntoIterator<Item = &'a mut CopyButton>,
    view: (&I18n<F>, &maestro_theme::BrandPresentation<'_>),
) -> Result<Vec<Option<egui::Response>>, ReadingUiError> {
    let clip = layout.response.rect.intersect(ui.clip_rect());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(clip));
    child.set_clip_rect(clip);
    buttons
        .into_iter()
        .map(|button| {
            let full = layout.controls.iter().find(|(id, _)| *id == button.id);
            full.map(|(_, full)| button.render_at(&mut child, *full, view.0, view.1))
                .transpose()
                .map_err(ReadingUiError::Control)
        })
        .collect()
}

/// A highlighted code listing with its language label and copy control.
pub struct CodeBlock {
    /// Original or base64-encoded code.
    pub code: String,
    /// Language label; empty displays as `plaintext`.
    pub language: String,
    /// Replaceable platform effects.
    host: Rc<dyn AttachmentHost>,
    /// Stable native surface identity.
    id: egui::Id,
    /// Current native content, read when a root is mounted.
    prepared: Rc<Prepared>,
    /// Mounted owner, referenced weakly so dropping the surface releases it.
    mounted: Rc<RefCell<Weak<browser::Mounted>>>,
    /// Candidate factory for the current preparation; successful publication retains the mounted root.
    content: Option<Rc<ReadingContent>>,
    /// Canvas copy control, retained across updates.
    copy: Option<CopyButton>,
}
impl CodeBlock {
    /// Retain the platform effects; code and language start empty.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            code: String::new(),
            language: String::new(),
            host,
            id: crate::browser::attachment_content::identity(),
            prepared: Rc::new(Prepared {
                language: String::new(),
                text: String::new(),
                html: String::new(),
            }),
            mounted: Rc::default(),
            content: None,
            copy: None,
        }
    }
    /// Draw the listing and its copy control for this frame.
    /// # Errors
    /// Returns localization, highlighting, glyph, presentation or native placement failures.
    pub fn render<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        view: CodeBlockView<'_, F>,
    ) -> Result<egui::Response, ReadingUiError> {
        let next = Prepared::new(view.highlighter, decode(&self.code), self.language.clone())?;
        let mut copy = match self.copy.take() {
            Some(copy) => copy,
            None => CopyButton::new(Rc::clone(&self.host), view.i18n)?,
        };
        copy.title = view.i18n.text("Copy code")?;
        copy.show_text = true;
        let size = copy.size(ui, view.i18n)?;
        let result = self.draw(ui, next, (copy.id, size), view);
        match result {
            Ok(layout) => {
                copy.text.clone_from(&self.prepared.text);
                let painted = controls(ui, &layout, [&mut copy], (view.i18n, view.presentation));
                self.copy = Some(copy);
                let gained = painted?.iter().flatten().any(egui::Response::gained_focus);
                if let Some(mounted) = self.mounted.borrow().upgrade().filter(|_| gained) {
                    mounted.reveal();
                }
                Ok(layout.response)
            }
            Err(error) => {
                self.copy = Some(copy);
                Err(error)
            }
        }
    }
    /// Publish current content through the shared measure and placement flow.
    fn draw<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        next: Prepared,
        control: (egui::Id, egui::Vec2),
        view: CodeBlockView<'_, F>,
    ) -> Result<ControlledLayout, ReadingUiError> {
        let changed = *self.prepared != next || self.content.is_none();
        let prepared = Rc::new(next);
        let mounted = Rc::default();
        let candidate = changed.then(|| browser::content(&prepared, &mounted, control.0));
        let content = candidate
            .as_ref()
            .or(self.content.as_ref())
            .ok_or_else(|| {
                ReadingUiError::Host(
                    crate::browser::attachment_content::AttachmentHostError::Host(
                        "Missing listing".into(),
                    ),
                )
            })?;
        let layout = ControlledReading {
            intrinsic_width: false,
            height: ReadingHeight::Flow,
            host: self.host.as_ref(),
            id: self.id,
            content,
            sizes: &[control],
            modal: view.modal,
            presentation: view.presentation,
        }
        .draw(ui)?;
        if let Some(content) = candidate {
            self.prepared = prepared;
            self.mounted = mounted;
            self.content = Some(content);
        }
        Ok(layout)
    }
}
impl Drop for CodeBlock {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
