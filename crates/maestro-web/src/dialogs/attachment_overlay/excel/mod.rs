//! One retained workbook projection for attachment and artifact consumers.
use crate::browser::attachment_content::{AttachmentHost, AttachmentUiError, ReadingContent};
use maestro_app::browser::ssf::{Ssf, read, sheet_to_html};
use std::rc::Rc;
/// Prepared sheet names and literal table fragments in workbook order.
#[derive(Default)]
struct Sheets {
    /// The reader's original order, never lexical order.
    values: Vec<(String, String)>,
    /// Mounted sheet bodies, retained independently of selection.
    #[cfg(target_arch = "wasm32")]
    nodes: std::cell::RefCell<Vec<web_sys::Element>>,
    /// Bounded native root, independent of natural-height registry measurement.
    #[cfg(target_arch = "wasm32")]
    root: std::cell::RefCell<Option<web_sys::Element>>,
    /// Last surface height in unscaled canvas points.
    #[cfg(target_arch = "wasm32")]
    height: std::cell::Cell<f32>,
}
/// Selected table and its stable native factory.
#[derive(Default)]
pub(crate) struct Excel {
    /// Prepared owned markup shared with the native mount.
    sheets: Rc<Sheets>,
    /// Current workbook-order index.
    selected: usize,
    /// One factory for one successfully prepared workbook.
    reading: Option<Rc<ReadingContent>>,
    /// Primary gesture admitted at its pressed endpoint.
    pressed: Option<usize>,
}
impl Excel {
    /// Release replaced content while optionally retaining a successful body.
    pub(crate) fn reset(&mut self, retain: bool) {
        self.pressed = None;
        if !retain {
            *self = Self::default();
        }
    }
    /// Read and prepare each worksheet once on the caller's repaint frame.
    pub(crate) fn open(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.reset(false);
        let mut ssf = Ssf::default();
        let workbook = read(bytes, "spreadsheet", &mut ssf).map_err(|e| e.to_string())?;
        if workbook.sheets.is_empty() {
            return Err(String::new());
        }
        self.sheets = Rc::new(Sheets {
            values: workbook
                .sheets
                .into_iter()
                .map(|(name, mut sheet)| {
                    let html = sheet_to_html(&mut sheet, &format!("sheet-{name}"), &ssf);
                    (name, html)
                })
                .collect(),
            #[cfg(target_arch = "wasm32")]
            nodes: std::cell::RefCell::default(),
            #[cfg(target_arch = "wasm32")]
            root: std::cell::RefCell::default(),
            #[cfg(target_arch = "wasm32")]
            height: std::cell::Cell::new(0.),
        });
        Ok(())
    }
    /// Admit tables and draw the canvas controls in native measured slots.
    pub(crate) fn present(
        &mut self,
        host: &dyn AttachmentHost,
        ui: &mut egui::Ui,
        surface: (egui::Id, egui::Rect, Option<egui::Id>, bool),
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        #[cfg(target_arch = "wasm32")]
        browser::bound(&self.sheets, surface.1.height())?;
        if self.reading.is_none() {
            #[cfg(target_arch = "wasm32")]
            let sheets = Rc::clone(&self.sheets);
            self.reading = Some(Rc::new(ReadingContent {
                #[cfg(target_arch = "wasm32")]
                mount: Rc::new(move |document| browser::mount(document, &sheets, surface)),
            }));
        }
        controls::present(self, host, ui, surface, presentation)?;
        #[cfg(target_arch = "wasm32")]
        browser::select(&self.sheets, self.selected)?;
        Ok(())
    }
}
#[cfg(target_arch = "wasm32")]
mod browser;
mod controls;
