//! One retained workbook projection for attachment and artifact consumers.
use super::error::Text;
use crate::browser::attachment_content::{AttachmentHost, AttachmentHostError, ReadingContent};
use maestro_app::browser::ssf::{Ssf, read, sheet_to_html};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
/// Prepared content for one successfully decoded or failed worksheet.
type SheetContent = Result<String, Rc<RefCell<Text>>>;
/// Prepared sheet names and native content in workbook order.
#[derive(Default)]
struct Sheets {
    /// The reader's original order, never lexical order.
    values: Vec<(String, SheetContent)>,
    /// Mounted sheet bodies, retained independently of selection.
    #[cfg(target_arch = "wasm32")]
    nodes: RefCell<Vec<web_sys::Element>>,
    /// Bounded native root, independent of natural-height registry measurement.
    #[cfg(target_arch = "wasm32")]
    root: RefCell<Option<web_sys::Element>>,
    /// Last surface height in unscaled canvas points.
    #[cfg(target_arch = "wasm32")]
    height: std::cell::Cell<f32>,
}
/// Selected table and its stable native factory.
#[derive(Default)]
pub(crate) struct Excel {
    /// Prepared owned content shared with the native mount.
    sheets: Rc<Sheets>,
    /// Current workbook-order index.
    selected: Rc<Cell<usize>>,
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
                .map(|(name, sheet)| {
                    let content = sheet
                        .map(|mut sheet| sheet_to_html(&mut sheet, &format!("sheet-{name}"), &ssf))
                        .map_err(|error| {
                            Rc::new(RefCell::new(Text {
                                values: [String::new(), error.to_string()],
                                #[cfg(target_arch = "wasm32")]
                                nodes: None,
                            }))
                        });
                    (name, content)
                })
                .collect(),
            #[cfg(target_arch = "wasm32")]
            nodes: RefCell::default(),
            #[cfg(target_arch = "wasm32")]
            root: RefCell::default(),
            #[cfg(target_arch = "wasm32")]
            height: std::cell::Cell::new(0.),
        });
        Ok(())
    }
    /// Mount native table bodies and their retained tab subscriptions.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn mount_root(
        &self,
        view: &crate::dom::DomView,
        padded: bool,
    ) -> Result<crate::dom::DomRoot, crate::tools::types::ToolUiError> {
        for (_, content) in &self.sheets.values {
            if let Err(text) = content {
                text.borrow_mut().values[0] = view.i18n.text("Error loading spreadsheet")?;
            }
        }
        browser::native(view, &self.sheets, &self.selected, padded)
    }
    /// Admit tables and draw the canvas controls in native measured slots.
    pub(crate) fn present(
        &mut self,
        host: &dyn AttachmentHost,
        ui: &mut egui::Ui,
        surface: (egui::Id, egui::Rect, Option<egui::Id>, bool),
        operands: (&maestro_theme::BrandPresentation<'_>, &str),
    ) -> Result<(), AttachmentHostError> {
        for (_, content) in &self.sheets.values {
            if let Err(text) = content {
                operands.1.clone_into(&mut text.borrow_mut().values[0]);
                #[cfg(target_arch = "wasm32")]
                let text = text.borrow();
                #[cfg(target_arch = "wasm32")]
                crate::browser::attachment_content::browser::sync_text(
                    text.nodes.as_ref(),
                    &text.values,
                );
            }
        }
        #[cfg(target_arch = "wasm32")]
        browser::bound(&self.sheets, surface.1.height())?;
        if self.reading.is_none() {
            #[cfg(target_arch = "wasm32")]
            let sheets = Rc::clone(&self.sheets);
            #[cfg(target_arch = "wasm32")]
            let selected = Rc::clone(&self.selected);
            self.reading = Some(Rc::new(ReadingContent {
                #[cfg(target_arch = "wasm32")]
                mount: Rc::new(move |document| {
                    browser::mount(document, &sheets, surface, selected.get())
                }),
            }));
        }
        controls::present(self, host, ui, surface, operands.0)?;
        #[cfg(target_arch = "wasm32")]
        browser::select(&self.sheets, self.selected.get())?;
        Ok(())
    }
}
#[cfg(target_arch = "wasm32")]
mod browser;
mod controls;
