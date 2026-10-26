//! Chosen, dropped and pasted files: limits, loading in order and per-file failures.
use super::{browser::Editor, browser::Listener, limit_megabytes};
use crate::{SharedAttachment, browser::attachment_content::browser::host_error, dom::report};
use maestro_app::browser::utils::attachment_utils::{AttachmentSource, load_attachment};
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::JsCast as _;
use wasm_bindgen_futures::JsFuture;
use web_sys::{ClipboardEvent, DragEvent, File, FileList};

/// Where a batch of files came from; pasted images word their alerts differently.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// The file chooser, reset after each batch.
    Picker,
    /// Dropped on the editor.
    Drop,
    /// Pasted images.
    Paste,
}
impl Editor {
    /// Register the chooser, paste and drag listeners.
    pub(super) fn bind_files(
        self: &Rc<Self>,
    ) -> Result<Vec<Listener>, crate::tools::types::ToolUiError> {
        Ok(vec![
            self.listen(self.picker.as_ref(), "change", |editor, _| {
                let files = list(editor.picker.files());
                editor.add_files(files, Origin::Picker);
            })?,
            self.listen(self.textarea.as_ref(), "paste", |editor, event| {
                editor.pasted(event.unchecked_ref());
            })?,
            self.listen(&self.root, "dragover", |editor, event| {
                event.prevent_default();
                event.stop_propagation();
                if !editor.dragging.get() {
                    editor.dragging.set(true);
                    report(editor.render());
                }
            })?,
            self.listen(&self.root, "dragleave", |editor, event| {
                editor.drag_left(event.unchecked_ref());
            })?,
            self.listen(&self.root, "drop", |editor, event| {
                event.prevent_default();
                event.stop_propagation();
                editor.dragging.set(false);
                report(editor.render());
                let drag: &DragEvent = event.unchecked_ref();
                let files = list(drag.data_transfer().and_then(|data| data.files()));
                editor.add_files(files, Origin::Drop);
            })?,
        ])
    }
    /// End the drag only when the pointer leaves the editor's box, not one of its children.
    fn drag_left(&self, event: &DragEvent) {
        event.prevent_default();
        event.stop_propagation();
        let bounds = self.root.get_bounding_client_rect();
        let (x, y) = (f64::from(event.client_x()), f64::from(event.client_y()));
        if x <= bounds.left() || x >= bounds.right() || y <= bounds.top() || y >= bounds.bottom() {
            self.dragging.set(false);
            report(self.render());
        }
    }
    /// Take pasted images as attachments; other pasted content keeps its native behavior.
    fn pasted(self: &Rc<Self>, event: &ClipboardEvent) {
        let Some(items) = event.clipboard_data().map(|data| data.items()) else {
            return;
        };
        let images: Vec<File> = (0..items.length())
            .filter_map(|index| items.get(index))
            .filter(|item| item.type_().starts_with("image/"))
            .filter_map(|item| item.get_as_file().ok().flatten())
            .collect();
        if !images.is_empty() {
            event.prevent_default();
            self.add_files(images, Origin::Paste);
        }
    }
    /// Admit a batch within the count limit, then load each file in order.
    fn add_files(self: &Rc<Self>, files: Vec<File>, origin: Origin) {
        if files.is_empty() {
            return;
        }
        let max_files = self.props.borrow().max_files;
        if files.len() + self.pending_files.get() + self.attachments.borrow().len() > max_files {
            self.alert(&format!("Maximum {max_files} files allowed"));
            if origin == Origin::Picker {
                self.picker.set_value("");
            }
            return;
        }
        self.pending_files
            .set(self.pending_files.get() + files.len());
        report(self.render());
        wasm_bindgen_futures::spawn_local(loaded(Rc::downgrade(self), files, origin));
    }
    /// Load one file, alerting a size or processing failure; `None` skips it.
    async fn load(&self, file: &File, origin: Origin) -> Option<SharedAttachment> {
        let max_size = self.props.borrow().max_file_size;
        let name = file.name();
        if num_traits::ToPrimitive::to_f64(&max_size).is_some_and(|limit| file.size() > limit) {
            let megabytes = limit_megabytes(max_size);
            self.alert(&match origin {
                Origin::Paste => format!("Image exceeds maximum size of {megabytes}MB"),
                Origin::Picker | Origin::Drop => {
                    format!("{name} exceeds maximum size of {megabytes}MB")
                }
            });
            return None;
        }
        match read(file).await {
            Ok(attachment) => Some(Arc::new(RwLock::new(attachment))),
            Err(error) => {
                let (context, alert) = match origin {
                    Origin::Paste => (
                        "Error processing pasted image:".to_owned(),
                        format!("Failed to process pasted image: {error}"),
                    ),
                    Origin::Picker | Origin::Drop => (
                        format!("Error processing {name}:"),
                        format!("Failed to process {name}: {error}"),
                    ),
                };
                web_sys::console::error_2(&context.into(), &error.into());
                self.alert(&alert);
                None
            }
        }
    }
    /// Show a platform alert.
    fn alert(&self, message: &str) {
        if let Some(window) = self.view.document.default_view() {
            let _ = window.alert_with_message(message);
        }
    }
}
/// Load `files` in order while the editor exists, then append the loaded ones and report them.
async fn loaded(editor: std::rc::Weak<Editor>, files: Vec<File>, origin: Origin) {
    let reserved = files.len();
    let mut added = Vec::new();
    for file in files {
        let Some(current) = editor.upgrade() else {
            return;
        };
        if let Some(attachment) = current.load(&file, origin).await {
            added.push(attachment);
        }
    }
    let Some(editor) = editor.upgrade() else {
        return;
    };
    editor
        .pending_files
        .set(editor.pending_files.get() - reserved);
    editor.attachments.borrow_mut().extend(added);
    report(editor.show_tiles());
    editor.files_changed();
    if origin == Origin::Picker {
        editor.picker.set_value("");
    }
    report(editor.render());
}
/// The original bytes of `file` as an attachment.
async fn read(
    file: &File,
) -> Result<maestro_app::browser::utils::attachment_utils::Attachment, String> {
    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|error| format!("{:?}", host_error(&error)))?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    let name = file.name();
    let mime = file.type_();
    let source = AttachmentSource::File {
        bytes: &bytes,
        name: &name,
        mime_type: &mime,
    };
    load_attachment(source, None, &maestro_app::default_fetch())
        .await
        .map_err(|error| error.to_string())
}
/// The files of a native list, in order.
fn list(files: Option<FileList>) -> Vec<File> {
    files.map_or_else(Vec::new, |files| {
        (0..files.length())
            .filter_map(|index| files.get(index))
            .collect()
    })
}
