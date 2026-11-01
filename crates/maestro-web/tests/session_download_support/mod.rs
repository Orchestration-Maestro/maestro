//! Controlled synchronous download boundary and source-derived fixture reader.
#![cfg(test)]
use base64::Engine as _;
use maestro_web::browser::attachment_content::*;
use maestro_web::export_html::template::{SessionDocument, load_document};
use std::{cell::RefCell, rc::Rc};

/// Exact supplied bytes and platform properties.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Download {
    pub text: String,
    pub mime: String,
    pub file_name: String,
}
/// One independent input and its observed output.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub name: String,
    pub test: String,
    pub input_json: String,
    pub expected: Download,
}
/// Read only the requested behavior group from the shared corpus.
pub fn cases(test: &str) -> Vec<Case> {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/session_downloads.json")).unwrap();
    assert_eq!(rows.len(), 20);
    let selected: Vec<_> = rows.into_iter().filter(|row| row.test == test).collect();
    let expected = match test {
        "session_download_keeps_full_history_and_header_projection" => 5,
        "session_download_uses_session_names_and_unicode_fallbacks" => 9,
        "session_download_keeps_compact_json_order_and_number_spelling" => 2,
        "session_download_does_not_add_a_final_newline" => 4,
        _ => panic!("unknown fixture group"),
    };
    assert_eq!(selected.len(), expected);
    selected
}
/// Preserve the literal input tokens at the public loader boundary.
pub fn document(input: &str, query: &str) -> SessionDocument {
    load_document(
        &base64::engine::general_purpose::STANDARD.encode(input),
        query,
        None,
    )
    .unwrap()
}
/// Recording host; unrelated effects must not be called.
#[derive(Default)]
pub struct Host {
    pub downloads: RefCell<Vec<Download>>,
    pub fail: bool,
}
impl AttachmentHost for Host {
    fn copy_to_clipboard(
        &self,
        _: &str,
        _: maestro_web::browser::attachment_content::ClipboardMode,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool>>> {
        unreachable!("unexpected clipboard operation")
    }

    fn download_bytes(
        &self,
        bytes: &[u8],
        name: &str,
        mime: &str,
    ) -> Result<(), AttachmentHostError> {
        self.downloads.borrow_mut().push(Download {
            text: String::from_utf8(bytes.to_vec()).unwrap(),
            mime: mime.into(),
            file_name: name.into(),
        });
        if self.fail {
            Err(AttachmentHostError::BorrowConflict)
        } else {
            Ok(())
        }
    }
    fn reading_layout(&self, _: egui::Id) -> Result<ReadingLayout, AttachmentHostError> {
        unreachable!()
    }
    fn present(
        &self,
        _: AttachmentSurface<'_>,
        _: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentHostError> {
        unreachable!()
    }
    fn set_retained_visibility(
        &self,
        _: egui::Id,
        _: bool,
        _: Option<egui::Id>,
    ) -> Result<(), AttachmentHostError> {
        Ok(())
    }
    fn set_overlay(&self, _: egui::Id, _: bool) -> Result<(), AttachmentHostError> {
        unreachable!()
    }
    fn hover_capable(&self) -> Result<bool, AttachmentHostError> {
        unreachable!()
    }
    fn remove(&self, _: egui::Id) {
        unreachable!()
    }
    fn timer(
        &self,
        _: i32,
        _: bool,
        _: Rc<dyn Fn()>,
    ) -> Result<maestro_web::browser::timer::Timer, AttachmentHostError> {
        unreachable!()
    }
    fn spawn_local(&self, _: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        unreachable!()
    }
    fn download(&self, _: maestro_web::SharedAttachment) -> Result<(), AttachmentHostError> {
        unreachable!()
    }
}
/// Replay each row once, asserting all independently recorded output properties.
pub fn replay(test: &str) {
    for row in cases(test) {
        let host = Host::default();
        maestro_web::export_html::template::download_session_json(
            &document(&row.input_json, ""),
            &host,
        )
        .unwrap();
        assert_eq!(*host.downloads.borrow(), [row.expected], "{}", row.name);
    }
}
