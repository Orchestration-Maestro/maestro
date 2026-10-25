//! Native file selection, shared document roots and cancellable page progression.
use super::{Action, State};
use crate::{
    browser::{
        attachment_content::{FileType, browser::sync_text},
        timer,
    },
    components::mode_toggle::{Mode, ModeGroup, Options},
    dialogs::attachment_overlay::{content::file_type, error, excel::Excel, pdf::Pdf},
    dom::DomRoot,
    tools::types::ToolUiError,
};
use std::rc::Rc;
use web_sys::HtmlElement;
/// Document resources retained while the body is displayed.
pub(super) struct Body {
    /// Native placement and its child resource owners.
    root: DomRoot,
    /// Progressive PDF document, absent for other formats.
    pdf: Option<Pdf>,
}
impl State {
    /// Construct the shared mode controls only for a supported document with nonempty text.
    pub(super) fn mode_group(self: &Rc<Self>) -> Result<Option<ModeGroup>, ToolUiError> {
        let label = {
            let attachment = self
                .attachment
                .try_read()
                .map_err(|_| ToolUiError::Presentation("Attachment borrow conflict".into()))?;
            if attachment
                .extracted_text
                .as_ref()
                .is_none_or(String::is_empty)
            {
                return Ok(None);
            }
            match file_type(&attachment) {
                FileType::Pdf => "PDF",
                FileType::Docx => "Document",
                FileType::Excel => "Spreadsheet",
                _ => return Ok(None),
            }
        };
        let read = Rc::clone(&self.extracted);
        let write = Rc::clone(&self.extracted);
        let options = Options {
            modes: [label, "Text"]
                .into_iter()
                .map(|label| {
                    Ok(Mode {
                        label: self.view.i18n.text(label)?,
                        icon: None,
                    })
                })
                .collect::<Result<_, ToolUiError>>()?,
            read: Rc::new(move || f64::from(u8::from(read.get()))),
            write: Rc::new(move |index| write.set(index.to_bits() == 1_f64.to_bits())),
            detail: |index, _| index.into(),
        };
        let modes = ModeGroup::new(&self.view, options, None)?;
        self.bind(modes.node.clone().into(), "mode-change", Action::Mode)?;
        Ok(Some(modes))
    }
    /// Replace file/text content and cancel any prior document before starting new work.
    pub(super) fn show_content(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let frame = self.frame.borrow_mut().take();
        let body = self.body.borrow_mut().take();
        drop((frame, body));
        let container = self.container()?;
        container.set_text_content(None);
        match self.prepare() {
            Ok(body) => {
                container
                    .append_child(&body.root.node)
                    .map_err(|error| ToolUiError::native(&error))?;
                let pending = body.pdf.is_some();
                *self.body.borrow_mut() = Some(body);
                if pending {
                    self.schedule()?;
                }
                Ok(())
            }
            Err(error) => self.show_error(&error),
        }
    }
    /// Select the content branch from the original record, releasing its lock before rendering.
    fn prepare(&self) -> Result<Body, ToolUiError> {
        let (kind, name, mime, content, text) = {
            let attachment = self
                .attachment
                .try_read()
                .map_err(|_| ToolUiError::Presentation("Attachment borrow conflict".into()))?;
            (
                file_type(&attachment),
                attachment.file_name.clone(),
                attachment.mime_type.clone(),
                attachment.content.clone(),
                attachment.extracted_text.clone(),
            )
        };
        if self.extracted.get() && kind != FileType::Image
            || matches!(kind, FileType::Text | FileType::Pptx)
        {
            let fallback = if kind == FileType::Text && !self.extracted.get() {
                "No content available"
            } else {
                "No text content available"
            };
            let root = self.element(
                "div",
                if self.extracted.get() {
                    "maestro-attachment-text maestro-attachment-extracted"
                } else if kind == FileType::Pptx {
                    "maestro-attachment-text maestro-attachment-presentation"
                } else {
                    "maestro-attachment-text"
                },
            )?;
            let pre = self.element("pre", "")?;
            let text = text
                .filter(|text| !text.is_empty())
                .map_or_else(|| self.view.i18n.text(fallback), Ok)?;
            pre.set_text_content(Some(&text));
            super::append(&root, &pre)?;
            return Ok(Body {
                root: DomRoot::new(root),
                pdf: None,
            });
        }
        if kind == FileType::Image {
            let root = self.element("img", "maestro-attachment-image")?;
            for (key, value) in [
                ("src", format!("data:{mime};base64,{content}")),
                ("alt", name),
            ] {
                root.set_attribute(key, &value)
                    .map_err(|error| ToolUiError::native(&error))?;
            }
            return Ok(Body {
                root: DomRoot::new(root),
                pdf: None,
            });
        }
        let bytes = crate::utils::base64::decode(&content)
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
        self.document(kind, bytes)
    }
    /// Build a format's shared native body, retaining its actual resources.
    fn document(&self, kind: FileType, bytes: Vec<u8>) -> Result<Body, ToolUiError> {
        let root;
        let mut pdf = None;
        match kind {
            FileType::Pdf => {
                let mut document = Pdf::default();
                document.open(bytes).map_err(ToolUiError::Presentation)?;
                root = document.mount_root(&self.view, false)?;
                pdf = Some(document);
            }
            FileType::Excel => {
                let mut document = Excel::default();
                document.open(&bytes).map_err(ToolUiError::Presentation)?;
                root = document.mount_root(&self.view, false)?;
            }
            FileType::Docx => {
                let wrapper =
                    self.element("div", "maestro-docx-container maestro-attachment-document")?;
                let options = crate::docx_preview::Options {
                    page_size: crate::docx_preview::PageSizeOptions {
                        ignore_width: true,
                        ignore_height: false,
                    },
                    ..crate::docx_preview::Options::default()
                };
                let rendered = crate::docx_preview::render_async(&bytes, &wrapper, None, options)?;
                root = DomRoot {
                    node: wrapper.into(),
                    owner: Some(Box::new(rendered)),
                };
            }
            _ => return Err(ToolUiError::Presentation("Missing document format".into())),
        }
        Ok(Body { root, pdf })
    }
    /// Find only this instance's current scrolling body.
    fn container(&self) -> Result<HtmlElement, ToolUiError> {
        use wasm_bindgen::JsCast as _;
        self.root
            .borrow()
            .as_ref()
            .and_then(crate::dom::live)
            .and_then(|root| {
                root.query_selector(".maestro-attachment-body")
                    .ok()
                    .flatten()
            })
            .ok_or_else(|| ToolUiError::Presentation("Missing attachment body".into()))?
            .dyn_into()
            .map_err(|error| ToolUiError::native(&error))
    }
    /// Show a literal error until an explicit file/text selection clears it.
    fn show_error(&self, error: &ToolUiError) -> Result<(), ToolUiError> {
        let message = error.to_string();
        let detail = if message.is_empty() {
            let kind = {
                let attachment = self
                    .attachment
                    .try_read()
                    .map_err(|_| ToolUiError::Presentation("Attachment borrow conflict".into()))?;
                file_type(&attachment)
            };
            self.view.i18n.text(match kind {
                FileType::Pdf => "Failed to load PDF",
                FileType::Docx => "Failed to load document",
                FileType::Excel => "Failed to load spreadsheet",
                _ => "Failed to display text content",
            })?
        } else {
            message
        };
        let (root, nodes) = error::nodes(&self.view.document, false)?;
        sync_text(
            Some(&nodes),
            &[self.view.i18n.text("Error loading file")?, detail],
        );
        let container = self.container()?;
        container.set_text_content(None);
        container
            .append_child(&root)
            .map_err(|error| ToolUiError::native(&error))?;
        Ok(())
    }
    /// Use the existing cancel-on-drop frame registration for progressive raster work.
    fn schedule(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let window = self
            .view
            .document
            .default_view()
            .ok_or_else(|| ToolUiError::Presentation("Missing window".into()))?;
        let weak = Rc::downgrade(self);
        let frame = timer::frame(
            window,
            Rc::new(move || {
                let Some(state) = weak.upgrade().filter(|state| !state.closed.get()) else {
                    return;
                };
                let Err(error) = state.advance() else {
                    return;
                };
                let previous = state.body.borrow_mut().take();
                drop(previous);
                // A frame has no return channel; content errors belong to its visible panel.
                let _ = state.show_error(&error);
            }),
        )
        .map_err(|error| ToolUiError::native(&error))?;
        let previous = self.frame.replace(Some(frame));
        drop(previous);
        Ok(())
    }
    /// Complete one page and retain completed native nodes before scheduling the next.
    fn advance(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let remains = {
            let mut body = self.body.borrow_mut();
            if let Some(pdf) = body.as_mut().and_then(|body| body.pdf.as_mut()) {
                let remains = pdf.next_page().map_err(ToolUiError::Presentation)?;
                pdf.sync_root()?;
                remains
            } else {
                false
            }
        };
        if remains {
            self.schedule()?;
        }
        Ok(())
    }
}
