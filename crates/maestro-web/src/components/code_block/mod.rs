//! Highlighted code with a language label and an original-text copy control.
use crate::{
    I18n,
    browser::attachment_content::{
        AttachmentHost, AttachmentUiError, ReadingContent,
        reading::controlled::{ControlledLayout, ControlledReading},
    },
    components::copy_button::CopyButton,
};
use base64::{
    Engine as _,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

pub(crate) mod browser;

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
    ) -> Result<Self, AttachmentUiError> {
        let html = maestro_theme::highlight_auto(
            highlighter,
            &text,
            Some(language.as_str()).filter(|l| !l.is_empty()),
            maestro_theme::HighlightProfile::CodeBlock,
        )
        .map_err(|e| AttachmentUiError::Presentation(e.to_string()))?;
        Ok(Self {
            language,
            text,
            html,
        })
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

/// Browser `atob` padding rules, with trailing bits admitted.
const BROWSER_BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);
/// Decode base64 UTF-8 text, keeping the whole input when it is not.
fn decode(input: &str) -> String {
    let compact: String = input
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\x0c' | '\r'))
        .collect();
    BROWSER_BASE64
        .decode(compact)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| input.to_owned())
}

/// Paint the slot-owned copy controls at their measured native rectangles.
pub(crate) fn controls<'a, F: ?Sized>(
    ui: &mut egui::Ui,
    layout: &ControlledLayout,
    buttons: impl IntoIterator<Item = &'a mut CopyButton>,
    view: CodeBlockView<'_, F>,
) -> Result<Vec<Option<egui::Response>>, AttachmentUiError> {
    let clip = layout.response.rect.intersect(ui.clip_rect());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(clip));
    child.set_clip_rect(clip);
    buttons
        .into_iter()
        .map(|button| {
            let full = layout.controls.iter().find(|(id, _)| *id == button.id);
            full.map(|(_, full)| button.render_at(&mut child, *full, view.i18n, view.presentation))
                .transpose()
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
    prepared: Rc<RefCell<Prepared>>,
    /// Mounted owner, referenced weakly so dropping the surface releases it.
    mounted: Rc<RefCell<Weak<browser::Mounted>>>,
    /// Native factory, created once and reused for the retained root.
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
            prepared: Rc::new(RefCell::new(Prepared {
                language: String::new(),
                text: String::new(),
                html: String::new(),
            })),
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
    ) -> Result<egui::Response, AttachmentUiError> {
        let next = Prepared::new(view.highlighter, decode(&self.code), self.language.clone())?;
        let mut copy = match self.copy.take() {
            Some(copy) => copy,
            None => CopyButton::new(Rc::clone(&self.host), view.i18n)?,
        };
        copy.text.clone_from(&next.text);
        copy.title = view.i18n.text("Copy code")?;
        copy.show_text = true;
        let size = copy.size(ui, view.i18n)?;
        let result = self.draw(ui, next, (copy.id, size), view);
        match result {
            Ok(layout) => {
                let painted = controls(ui, &layout, [&mut copy], view);
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
    ) -> Result<ControlledLayout, AttachmentUiError> {
        if *self.prepared.borrow() != next {
            *self.prepared.borrow_mut() = next;
            if let Some(mounted) = self.mounted.borrow().upgrade() {
                mounted.update(&self.prepared.borrow())?;
            }
        }
        let content = Rc::clone(
            self.content
                .get_or_insert_with(|| browser::content(&self.prepared, &self.mounted, control.0)),
        );
        ControlledReading {
            host: self.host.as_ref(),
            id: self.id,
            content: &content,
            sizes: &[control],
            modal: view.modal,
            presentation: view.presentation,
        }
        .draw(ui)
    }
}
impl Drop for CodeBlock {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
