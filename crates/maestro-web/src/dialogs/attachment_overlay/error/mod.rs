//! One literal native error owner for attachment and artifact bodies.
use crate::browser::attachment_content::{
    AttachmentHost, AttachmentHostError, ReadingContent, reading,
};
use std::{cell::RefCell, rc::Rc};
/// Current literal text and its stable native nodes.
#[derive(Default)]
pub(super) struct Text {
    /// Localized heading followed by the already-selected diagnostic.
    pub(super) values: [String; 2],
    /// Retained text nodes in the actual document.
    #[cfg(target_arch = "wasm32")]
    pub(super) nodes: Option<[web_sys::Element; 2]>,
}
/// Stable native failure content, independent of caller error policy.
#[derive(Default)]
pub(crate) struct Error {
    /// Owned literal strings only.
    text: Rc<RefCell<Text>>,
    /// Successfully admitted native factory.
    reading: Option<Rc<ReadingContent>>,
}
impl Error {
    /// Place caller-selected heading/detail with unchanged native identity.
    pub(crate) fn present(
        &mut self,
        host: &dyn AttachmentHost,
        ui: &egui::Ui,
        surface: (egui::Id, Option<egui::Id>, bool),
        operands: ([String; 2], &maestro_theme::BrandPresentation<'_>),
    ) -> Result<egui::Rect, AttachmentHostError> {
        self.text.borrow_mut().values = operands.0;
        #[cfg(target_arch = "wasm32")]
        {
            let text = self.text.borrow();
            crate::browser::attachment_content::browser::sync_text(
                text.nodes.as_ref(),
                &text.values,
            );
        }
        let reading = self.reading.as_ref().map_or_else(
            || {
                #[cfg(target_arch = "wasm32")]
                let text = Rc::clone(&self.text);
                Rc::new(ReadingContent {
                    #[cfg(target_arch = "wasm32")]
                    mount: Rc::new(move |document| browser::mount(document, &text, surface.2)),
                })
            },
            Rc::clone,
        );
        let rect = ui.available_rect_before_wrap();
        reading::present(host, ui, (surface.0, rect, surface.1), &reading, operands.1)?;
        self.reading = Some(reading);
        Ok(rect)
    }
}
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub(crate) use browser::nodes;
#[cfg(target_arch = "wasm32")]
pub(super) use browser::root;

/// Preserve literal diagnostics, using the fallback only for absence.
pub(super) fn failure_detail(message: Option<&str>, fallback: String) -> String {
    message
        .filter(|s| !s.is_empty())
        .map_or(fallback, str::to_owned)
}
/// Select a localized document fallback without changing host-error policy.
pub(crate) fn detail<F: ?Sized>(
    message: &str,
    i: &crate::I18n<F>,
    fallback: &str,
) -> Result<String, crate::I18nError> {
    Ok(failure_detail(Some(message), i.text(fallback)?))
}
