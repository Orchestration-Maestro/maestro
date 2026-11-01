//! Frame-driven owned PDF pages shared by attachment and artifact views.
use crate::browser::attachment_content::{
    AttachmentHost, AttachmentHostError, ReadingContent, reading,
};
use maestro_app::browser::utils::attachment_utils::pdf::{PdfDocument, PdfPage};
use std::{cell::RefCell, rc::Rc};
/// Completed raster operands and their retained native nodes.
#[derive(Default)]
struct Pages {
    /// Completed pages in document order.
    values: Vec<PdfPage>,
    /// Total pages determines separators while rendering progressively.
    total: usize,
    /// Mounted page root, never a document owner.
    #[cfg(target_arch = "wasm32")]
    root: Option<web_sys::Element>,
}
/// Document lifetime and cursor, separate from native placement.
#[derive(Default)]
pub(crate) struct Pdf {
    /// Live document exists only while pages remain.
    document: Option<PdfDocument>,
    /// Completed pages survive successful empty edits.
    pages: Rc<RefCell<Pages>>,
    /// Stable native factory.
    reading: Option<Rc<ReadingContent>>,
    /// Prevent repeated work within the same input frame.
    frame: Option<u64>,
}
impl Pdf {
    /// Cancel the owned document, optionally retaining completed pages.
    pub(crate) fn reset(&mut self, retain: bool) {
        self.document = None;
        self.frame = None;
        if !retain {
            self.pages = Rc::new(RefCell::new(Pages::default()));
            self.reading = None;
        }
    }
    /// Open once after caller byte selection.
    pub(crate) fn open(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        self.reset(false);
        let document = PdfDocument::new(bytes).map_err(|e| e.to_string())?;
        self.pages.borrow_mut().total = document.num_pages();
        self.document = Some(document);
        Ok(())
    }
    /// Complete at most one page per explicit frame, releasing finished work.
    pub(crate) fn advance(&mut self, context: &egui::Context) -> Result<(), String> {
        // Explicit begin/end passes do not advance the context frame counter.
        let frame = context
            .cumulative_pass_nr()
            .saturating_sub(u64::try_from(context.current_pass_index()).unwrap_or_default());
        if self.frame == Some(frame) {
            return Ok(());
        }
        self.frame = Some(frame);
        if self.next_page()? {
            context.request_repaint();
        }
        Ok(())
    }
    /// Complete one page, returning whether unfinished pages remain.
    pub(crate) fn next_page(&mut self) -> Result<bool, String> {
        let Some(document) = &self.document else {
            return Ok(false);
        };
        let number = self.pages.borrow().values.len() + 1;
        if number > document.num_pages() {
            self.document = None;
            return Ok(false);
        }
        match document.render_page(number) {
            Ok(page) => self.pages.borrow_mut().values.push(page),
            Err(error) => {
                self.document = None;
                return Err(error.to_string());
            }
        }
        let remains = number < document.num_pages();
        if !remains {
            self.document = None;
        }
        Ok(remains)
    }
    /// Admit stable page nodes without retaining drawing or engine operands.
    pub(crate) fn present(
        &mut self,
        host: &dyn AttachmentHost,
        ui: &egui::Ui,
        surface: (egui::Id, egui::Rect, Option<egui::Id>, bool),
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentHostError> {
        #[cfg(target_arch = "wasm32")]
        browser::sync(&self.pages.borrow())?;
        let content = self.reading(surface.3);
        reading::present(
            host,
            ui,
            (surface.0, surface.1, surface.2),
            &content,
            presentation,
        )
    }
    /// Mount completed pages directly in the caller's native document.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn mount_root(
        &self,
        view: &crate::dom::DomView,
        padded: bool,
    ) -> Result<crate::dom::DomRoot, crate::tools::types::ToolUiError> {
        Ok(crate::dom::DomRoot::new(browser::root(
            &view.document,
            &self.pages,
            padded,
        )?))
    }
    /// Append newly completed pages to the retained native root.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn sync_root(&self) -> Result<(), crate::tools::types::ToolUiError> {
        browser::sync(&self.pages.borrow()).map_err(Into::into)
    }
    /// Retain one factory for this completed-page collection.
    fn reading(&mut self, padded: bool) -> Rc<ReadingContent> {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = padded;
        if let Some(reading) = &self.reading {
            return Rc::clone(reading);
        }
        #[cfg(target_arch = "wasm32")]
        let pages = Rc::clone(&self.pages);
        let reading = Rc::new(ReadingContent {
            #[cfg(target_arch = "wasm32")]
            mount: Rc::new(move |document| browser::mount(document, &pages, padded)),
        });
        self.reading = Some(Rc::clone(&reading));
        reading
    }
}
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(test)]
mod tests;

/// Select a PDF failure detail without changing overlay suppression.
pub(crate) fn detail<F: ?Sized>(
    message: &str,
    i: &crate::I18n<F>,
) -> Result<String, crate::I18nError> {
    super::error::detail(message, i, "Failed to load PDF")
}
