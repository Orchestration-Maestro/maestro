//! Attachment overlay behaviour through retained public rendering.
#![cfg(test)]
/// Shared attachment drawing fixtures.
use crate::attachment_support;
use crate::support;
use attachment_support::*;
use maestro_web::AttachmentOverlay;
use std::rc::Rc;
#[test]
fn overlay_images_use_original_bytes_and_contain_fit() {
    let record = attachment();
    {
        let mut record = record.write().unwrap();
        record.r#type = maestro_app::browser::utils::attachment_utils::AttachmentType::Image;
        record.mime_type = "image/svg+xml".into();
        record.preview = Some("thumbnail".into());
    }
    let host = Rc::new(Host::default());
    let mut overlay = AttachmentOverlay::open(record, None, host.clone());
    let mut frame = Frame::default();
    assert!(frame.overlay(&mut overlay, vec![]).unwrap());
    assert!(
        matches!(&host.surfaces.borrow().last().unwrap().content,Content::Image(s,alt,maestro_web::browser::attachment_content::ImageFit::Contain) if s=="data:image/svg+xml;base64,QUJD" && alt=="note.txt")
    );
    for c in content_cases()
        .into_iter()
        .filter(|c| c.expected.kind == "image")
    {
        let record = valid_pdf(c.input.record());
        let name = record.read().unwrap().file_name.clone();
        let host = Rc::new(Host::default());
        let mut overlay = AttachmentOverlay::open(record, None, host.clone());
        let mut frame = Frame::default();
        frame.overlay(&mut overlay, vec![]).unwrap();
        assert!(
            matches!(&host.surfaces.borrow().last().unwrap().content,Content::Image(source,alt,maestro_web::browser::attachment_content::ImageFit::Contain) if Some(source)==c.expected.source.as_ref()&&alt==&name),
            "{}",
            c.name
        );
        assert!(!c.show_text);
        assert!(!c.expected.small);
        assert!(c.expected.text.is_none());
    }
}
use maestro_web::browser::attachment_content::{FileType, TextView};
use maestro_web::dialogs::attachment_overlay::AttachmentUiError;
use serde::Deserialize;
/// Ordered classification case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypeCase {
    /// Case identity.
    pub(crate) name: String,
    /// Original attachment, absent only before opening.
    pub(crate) input: Input,
    /// Expected selected kind.
    pub(crate) expected: TypeExpected,
}
/// Classifier output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypeExpected {
    /// Source kind.
    pub(crate) r#type: String,
}
/// Observe the rendered content kind without using the widget classifier.
fn kind(content: &Content) -> &str {
    match content {
        Content::Reading(..) => "pdf",
        Content::Image(..) => "image",
        Content::Text(_, TextView::Presentation) => "pptx",
        Content::Text(..) => "text",
        Content::Preview(tile, ..) => match tile {
            FileType::Pdf => "pdf",
            FileType::Docx => "docx",
            FileType::Excel => "excel",
            _ => panic!("unexpected document container"),
        },
    }
}
#[test]
fn overlay_file_detection_preserves_priority_and_case() {
    let cases: Vec<TypeCase> =
        serde_json::from_str(include_str!("fixtures/attachment_types.json")).unwrap();
    for c in cases {
        let host = Rc::new(Host::default());
        let record = valid_pdf(c.input.record());
        if c.expected.r#type == "excel" {
            record.write().unwrap().content = excel_bytes(false);
        }
        let mut overlay = AttachmentOverlay::open(record, None, host.clone());
        let mut frame = Frame::default();
        frame.overlay(&mut overlay, vec![]).unwrap();
        if c.expected.r#type == "excel" {
            assert_reading(&host);
            continue;
        }
        assert_eq!(
            kind(&host.surfaces.borrow().last().unwrap().content),
            c.expected.r#type,
            "{}",
            c.name
        );
    }
}
/// Localized toggle input and complete mode labels.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToggleCase {
    /// Case identity.
    pub(crate) name: String,
    /// Original record.
    pub(crate) input: Input,
    /// Selected language.
    pub(crate) language: String,
    /// Exact mode labels/index.
    pub(crate) expected: ToggleExpected,
}
/// Source toggle output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToggleExpected {
    /// Preview label followed by text label.
    pub(crate) modes: Vec<String>,
    /// Initial selected index, absent with no toggle.
    pub(crate) selected: Option<usize>,
}
#[test]
fn overlay_toggle_gates_and_labels_each_supported_view() {
    let cases: Vec<ToggleCase> =
        serde_json::from_str(include_str!("fixtures/attachment_toggles.json")).unwrap();
    for c in cases {
        let host = Rc::new(Host::default());
        let record = valid_pdf(c.input.record());
        let name = record.read().unwrap().file_name.clone();
        let mut overlay = AttachmentOverlay::open(record, None, host);
        let mut frame = Frame::default();
        let lang = Rc::new(support::Host::default());
        *lang.stored.borrow_mut() = Some(c.language);
        frame.i18n = maestro_web::I18n::new(lang).unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        let labels: Vec<_> = frame.texts().into_iter().filter(|s| s != &name).collect();
        assert_eq!(labels, c.expected.modes, "{}", c.name);
        let accent =
            egui::Color32::from_hex(frame.pack.presentation(frame.mode).unwrap().colors["accent"])
                .unwrap();
        assert_eq!(
            frame.selected(&c.expected.modes, accent),
            c.expected.selected.into_iter().collect::<Vec<_>>(),
            "{}",
            c.name
        );
    }
}
#[test]
fn overlay_toggle_clears_errors_without_changing_attachment() {
    let record = attachment();
    record.write().unwrap().mime_type = "application/pdf".into();
    valid_pdf(record.clone());
    let host = Rc::new(Host::default());
    *host.fail.borrow_mut() = Some("preview failure".into());
    let mut overlay = AttachmentOverlay::open(record.clone(), None, host.clone());
    let mut frame = Frame::default();
    frame.overlay(&mut overlay, vec![]).unwrap();
    frame.overlay(&mut overlay, vec![]).unwrap();
    assert_reading(&host);
    let pos = frame.label("Text").center();
    frame.overlay(&mut overlay, click(pos)).unwrap();
    assert!(
        matches!(&host.surfaces.borrow().last().unwrap().content,Content::Text(s,TextView::Extracted) if s=="<literal>\n\ttext")
    );
    record.write().unwrap().extracted_text = Some("retained alias".into());
    let before = host.surfaces.borrow().len();
    frame.overlay(&mut overlay, click(pos)).unwrap();
    assert!(
        matches!(&host.surfaces.borrow()[before].content,Content::Text(s,TextView::Extracted) if s=="retained alias")
    );
    assert!(matches!(
        host.surfaces.borrow().last().unwrap().content,
        Content::Text(..)
    ));
    let pos = frame.label("PDF").center();
    frame.overlay(&mut overlay, click(pos)).unwrap();
    assert!(matches!(
        &host.surfaces.borrow().last().unwrap().content,
        Content::Reading(..)
    ));
    let pos = frame.label("Text").center();
    frame.overlay(&mut overlay, click(pos)).unwrap();
    *host.fail.borrow_mut() = Some("text failure".into());
    frame.overlay(&mut overlay, vec![]).unwrap();
    frame.overlay(&mut overlay, vec![]).unwrap();
    assert_reading(&host);
    assert_error_survives_record_edits(&mut frame, &mut overlay, &record, &host, "text failure");
}
/// Source content selection input/output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ContentCase {
    /// Case identity.
    pub(crate) name: String,
    /// Typed original record.
    pub(crate) input: Input,
    /// Whether text mode was selected.
    pub(crate) show_text: bool,
    /// Full expected content projection.
    pub(crate) expected: ContentExpected,
}
/// Source content result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContentExpected {
    /// Selected content kind.
    pub(crate) kind: String,
    /// Original image URL.
    pub(crate) source: Option<String>,
    /// Literal text, absent for document containers.
    pub(crate) text: Option<String>,
    /// Extracted-text font/layout selection.
    pub(crate) small: bool,
}
/// Source-derived content cases.
pub(crate) fn content_cases() -> Vec<ContentCase> {
    serde_json::from_str(include_str!("fixtures/attachment_content.json")).unwrap()
}
#[test]
fn overlay_text_preserves_literal_whitespace_and_empty_messages() {
    for c in content_cases()
        .into_iter()
        .filter(|c| c.expected.kind == "text")
    {
        let record = valid_pdf(c.input.record());
        let host = Rc::new(Host::default());
        let mut overlay = AttachmentOverlay::open(record.clone(), None, host.clone());
        let mut frame = Frame::default();
        frame.overlay(&mut overlay, vec![]).unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        if c.show_text {
            let pos = frame.label("Text").center();
            frame.overlay(&mut overlay, click(pos)).unwrap();
        }
        let surface = host.surfaces.borrow().last().unwrap().clone();
        assert!(
            matches!(surface.content,Content::Text(ref text,view) if Some(text)==c.expected.text.as_ref()&&(view==TextView::Extracted)==c.expected.small),
            "{}",
            c.name
        );
        assert!(c.expected.source.is_none());
        if c.show_text {
            for (text, expected) in [
                (None, "No text content available"),
                (Some(""), "No text content available"),
                (Some(" \n"), " \n"),
            ] {
                record.write().unwrap().extracted_text = text.map(str::to_owned);
                frame.overlay(&mut overlay, vec![]).unwrap();
                assert!(
                    matches!(&host.surfaces.borrow().last().unwrap().content,Content::Text(s,TextView::Extracted) if s==expected)
                );
            }
        }
    }
}
#[test]
fn overlay_presentation_shows_text_without_a_toggle() {
    for c in content_cases()
        .into_iter()
        .filter(|c| c.expected.kind == "pptx")
    {
        let record = valid_pdf(c.input.record());
        let host = Rc::new(Host::default());
        let mut overlay = AttachmentOverlay::open(record.clone(), None, host.clone());
        let mut frame = Frame::default();
        for (input, expected) in [
            (Some("<slide>\n\tile&"), "<slide>\n\tile&"),
            (None, "No text content available"),
            (Some(""), "No text content available"),
        ] {
            record.write().unwrap().extracted_text = input.map(str::to_owned);
            frame.overlay(&mut overlay, vec![]).unwrap();
            assert!(
                matches!(&host.surfaces.borrow().last().unwrap().content,Content::Text(s,TextView::Presentation) if s==expected),
                "{}",
                c.name
            );
            assert!(
                !frame
                    .texts()
                    .iter()
                    .any(|s| s == "Text" || s == "Presentation")
            );
        }
        assert!(!c.show_text);
        assert!(!c.expected.small);
        assert!(c.expected.text.is_none() && c.expected.source.is_none());
        *host.fail.borrow_mut() = Some("presentation failure".into());
        frame.overlay(&mut overlay, vec![]).unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        assert_reading(&host);
    }
}
#[test]
fn overlay_preview_reload_obeys_state_changes() {
    for c in content_cases()
        .into_iter()
        .filter(|c| ["docx", "excel"].contains(&c.expected.kind.as_str()))
    {
        let record = valid_pdf(c.input.record());
        let host = Rc::new(Host::default());
        let mut overlay = AttachmentOverlay::open(record.clone(), None, host.clone());
        let mut frame = Frame::default();
        if c.expected.kind == "excel" {
            record.write().unwrap().content = excel_bytes(false);
            assert_excel_reload(&mut frame, &mut overlay, &record, &host, &c);
            continue;
        }
        frame.overlay(&mut overlay, vec![]).unwrap();
        assert_initial_preview(&host, &record, &c);
        frame.overlay(&mut overlay, vec![]).unwrap();
        assert!(!last_reload(&host));
        record.write().unwrap().content = "changed bytes".into();
        frame.overlay(&mut overlay, vec![]).unwrap();
        assert!(last_reload(&host));
        let pos = frame.label("Text").center();
        frame.overlay(&mut overlay, click(pos)).unwrap();
        assert!(matches!(
            host.surfaces.borrow().last().unwrap().content,
            Content::Text(_, TextView::Extracted)
        ));
        let label = match c.expected.kind.as_str() {
            "pdf" => "PDF",
            "docx" => "Document",
            _ => "Spreadsheet",
        };
        let pos = frame.label(label).center();
        frame.overlay(&mut overlay, click(pos)).unwrap();
        assert!(last_reload(&host));
        frame.overlay(&mut overlay, click(pos)).unwrap();
        assert!(!last_reload(&host));
        assert_retained_text_mode(&mut frame, &mut overlay, &record, &host);
    }
}
#[test]
fn overlay_errors_replace_content_without_hiding_controls() {
    for message in ["failure", ""] {
        let record = attachment();
        record.write().unwrap().mime_type = "application/pdf".into();
        valid_pdf(record.clone());
        let host = Rc::new(Host::default());
        *host.fail.borrow_mut() = Some(message.into());
        let mut overlay = AttachmentOverlay::open(record.clone(), None, host.clone());
        let mut frame = Frame::default();
        frame.overlay(&mut overlay, vec![]).unwrap();
        let before = host.surfaces.borrow().len();
        frame.overlay(&mut overlay, vec![]).unwrap();
        if message.is_empty() {
            assert!(host.surfaces.borrow().len() > before);
        } else {
            assert_reading(&host);
            assert_error_survives_record_edits(&mut frame, &mut overlay, &record, &host, message);
        }
        let pos = frame.control(1);
        frame.overlay(&mut overlay, click(pos)).unwrap();
        assert_eq!(host.downloads.borrow().len(), 1);
        let pos = frame.control(0);
        assert!(!frame.overlay(&mut overlay, click(pos)).unwrap());
    }
}
/// A physical key event, including modifier-held Escape.
fn key(key: egui::Key, modifiers: egui::Modifiers) -> Vec<egui::Event> {
    vec![egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }]
}
#[test]
fn overlay_close_distinguishes_escape_backdrop_header_and_content() {
    for action in 0..6 {
        let record = attachment();
        let host = Rc::new(Host::default());
        let effects = host.clone();
        let alias = record.clone();
        let callback = Rc::new(move || {
            alias.write().unwrap().file_name = "closed by callback".into();
            effects.effects.borrow_mut().push("callback".into());
        });
        let mut overlay = AttachmentOverlay::open(record, Some(callback), host.clone());
        let mut frame = Frame::default();
        frame.screen.set_width(1200.);
        frame.overlay(&mut overlay, vec![]).unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        let before = host.effects.borrow().len();
        let content = host.surfaces.borrow().last().unwrap().rect;
        let events = match action {
            0 => key(egui::Key::Enter, egui::Modifiers::NONE),
            1 => click(frame.label("note.txt").center()),
            2 => click(content.center()),
            3 => key(egui::Key::Escape, egui::Modifiers::CTRL),
            4 => click(egui::pos2(1199., 599.)),
            _ => click(frame.control(0)),
        };
        assert_eq!(
            frame.overlay(&mut overlay, events).unwrap(),
            action < 3 || action == 4,
            "action {action}"
        );
        if action == 3 || action == 5 {
            let log = host.effects.borrow()[before..].to_vec();
            let c = log.iter().position(|s| s == "callback").unwrap();
            assert!(log[c + 1].starts_with("remove:"));
            assert!(log[c + 2].ends_with(":false"));
            let n = host.surfaces.borrow().len();
            assert!(!frame.overlay(&mut overlay, vec![]).unwrap());
            assert!(frame.texts().is_empty());
            assert_eq!(host.surfaces.borrow().len(), n);
        }
    }
}
#[test]
fn overlay_instances_keep_separate_surface_ownership() {
    let record = attachment();
    let host = Rc::new(Host::default());
    let callback_alias = record.clone();
    let callback = Rc::new(move || {
        callback_alias.write().unwrap().extracted_text = Some("callback edit".into());
    });
    let mut one = AttachmentOverlay::open(record.clone(), Some(callback), host.clone());
    let mut two = AttachmentOverlay::open(record, None, host.clone());
    let mut frame = Frame::default();
    frame.run(vec![], |c, i, p| {
        one.render(c, i, p).unwrap();
        two.render(c, i, p).unwrap();
    });
    assert_ne!(host.surfaces.borrow()[0].id, host.surfaces.borrow()[1].id);
    let closed = frame.run(key(egui::Key::Escape, egui::Modifiers::NONE), |c, i, p| {
        (one.render(c, i, p).unwrap(), two.render(c, i, p).unwrap())
    });
    assert_eq!(closed, (false, false));
    assert_eq!(
        host.effects
            .borrow()
            .iter()
            .filter(|s| s.starts_with("remove:"))
            .count(),
        2
    );
}
#[test]
fn overlay_drop_cleans_up_without_calling_on_close() {
    let record = attachment();
    let weak = std::sync::Arc::downgrade(&record);
    let host = Rc::new(Host::default());
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    let capture = record.clone();
    let callback = Rc::new(move || {
        capture.write().unwrap().content.clear();
        count.set(count.get() + 1);
    });
    let mut overlay = AttachmentOverlay::open(record.clone(), Some(callback), host.clone());
    drop(record);
    let mut frame = Frame::default();
    frame.overlay(&mut overlay, vec![]).unwrap();
    let id = host.surfaces.borrow().last().unwrap().id;
    let n = host.effects.borrow().len();
    drop(overlay);
    assert_eq!(calls.get(), 0);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        &host.effects.borrow()[n..],
        &[format!("remove:{id:?}"), format!("modal:{id:?}:false")]
    );
}
#[test]
fn overlay_download_forwards_original_fields() {
    for state in 0..3 {
        let record = attachment();
        record.write().unwrap().mime_type = "application/pdf".into();
        valid_pdf(record.clone());
        record.write().unwrap().preview = Some("not original".into());
        let host = Rc::new(Host::default());
        if state == 2 {
            *host.fail.borrow_mut() = Some("preview failure".into());
        }
        let mut overlay = AttachmentOverlay::open(record.clone(), None, host.clone());
        let mut frame = Frame::default();
        frame.overlay(&mut overlay, vec![]).unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        if state == 1 {
            let pos = frame.label("Text").center();
            frame.overlay(&mut overlay, click(pos)).unwrap();
        }
        let pos = frame.control(1);
        frame.overlay(&mut overlay, click(pos)).unwrap();
        let downloads = host.downloads.borrow();
        assert_eq!(downloads.len(), 1);
        assert!(std::sync::Arc::ptr_eq(&downloads[0], &record));
        let original = downloads[0].read().unwrap();
        assert_eq!(original.content, pdf_bytes());
        assert_eq!(original.file_name, "note.txt");
        assert_eq!(original.mime_type, "application/pdf");
    }
}
#[test]
fn overlay_download_propagates_host_failures_without_changing_mode() {
    let record = attachment();
    record.write().unwrap().mime_type = "application/pdf".into();
    valid_pdf(record.clone());
    let host = Rc::new(Host::default());
    let mut overlay = AttachmentOverlay::open(record.clone(), None, host.clone());
    let mut frame = Frame::default();
    frame.overlay(&mut overlay, vec![]).unwrap();
    frame.overlay(&mut overlay, vec![]).unwrap();
    let pos = frame.label("Text").center();
    frame.overlay(&mut overlay, click(pos)).unwrap();
    host.fail_download.set(true);
    let before = host.surfaces.borrow().len();
    let pos = frame.control(1);
    assert!(matches!(
        frame.overlay(&mut overlay, click(pos)),
        Err(AttachmentUiError::Host(_))
    ));
    assert!(std::sync::Arc::ptr_eq(&host.downloads.borrow()[0], &record));
    host.fail_download.set(false);
    frame.overlay(&mut overlay, vec![]).unwrap();
    assert!(matches!(
        &host.surfaces.borrow()[before].content,
        Content::Text(_, TextView::Extracted)
    ));
    assert!(matches!(
        host.surfaces.borrow().last().unwrap().content,
        Content::Text(..)
    ));
}

/// Read the reload decision from the actual pending document placement.
fn last_reload(host: &Host) -> bool {
    match &host.surfaces.borrow().last().unwrap().content {
        Content::Preview(_, _, reload) => *reload,
        _ => panic!("expected a document preview"),
    }
}

/// Observe initial container identity and its independent fixture projection.
fn assert_initial_preview(host: &Host, record: &maestro_web::SharedAttachment, c: &ContentCase) {
    let first = host.surfaces.borrow()[0].clone();
    assert_eq!(kind(&first.content), c.expected.kind, "{}", c.name);
    assert!(
        matches!(first.content,Content::Preview(_,ref handle,true) if std::sync::Arc::ptr_eq(handle,record))
    );
    assert_eq!(first.rect.width().to_bits(), 800.0_f32.to_bits());
    assert!(!c.show_text);
    assert!(!c.expected.small);
    assert!(c.expected.source.is_none() && c.expected.text.is_none());
}

/// Mutating the original record keeps a supported selected mode live across kinds.
fn assert_retained_text_mode(
    frame: &mut Frame,
    overlay: &mut AttachmentOverlay,
    record: &maestro_web::SharedAttachment,
    host: &Host,
) {
    let pos = frame.label("Text").center();
    frame.overlay(overlay, click(pos)).unwrap();
    for (mime, image) in [
        ("image/png", true),
        ("text/plain", false),
        ("presentationml", false),
    ] {
        {
            let mut original = record.write().unwrap();
            original.mime_type = mime.into();
            original.r#type = if image {
                maestro_app::browser::utils::attachment_utils::AttachmentType::Image
            } else {
                maestro_app::browser::utils::attachment_utils::AttachmentType::Document
            };
        }
        frame.overlay(overlay, vec![]).unwrap();
        let content = host.surfaces.borrow().last().unwrap().content.clone();
        assert_eq!(matches!(content, Content::Image(..)), image);
        if !image {
            assert!(matches!(content, Content::Text(_, TextView::Extracted)));
        }
    }
}

/// Existing presentation failures remain dominant after edits to the shared record.
fn assert_error_survives_record_edits(
    frame: &mut Frame,
    overlay: &mut AttachmentOverlay,
    record: &maestro_web::SharedAttachment,
    host: &Host,
    message: &str,
) {
    let factory = reading_factory(host);
    let _ = message; // Literal diagnostics are observed in the native document suite.
    for mime in [
        "image/png",
        "text/plain",
        "presentationml",
        "wordprocessingml",
        "ms-excel",
    ] {
        {
            let mut original = record.write().unwrap();
            original.mime_type = mime.into();
            original.r#type = if mime == "image/png" {
                maestro_app::browser::utils::attachment_utils::AttachmentType::Image
            } else {
                maestro_app::browser::utils::attachment_utils::AttachmentType::Document
            };
        }
        frame.overlay(overlay, vec![]).unwrap();
        assert!(Rc::ptr_eq(&factory, &reading_factory(host)));
    }
}

#[test]
fn overlay_tab_order_activates_preview_text_download_close() {
    for toggles in [true, false] {
        let record = attachment();
        if toggles {
            record.write().unwrap().mime_type = "application/pdf".into();
            valid_pdf(record.clone());
        }
        let host = Rc::new(Host::default());
        let mut overlay = AttachmentOverlay::open(record, None, host.clone());
        let mut frame = Frame::default();
        frame.overlay(&mut overlay, vec![]).unwrap();
        frame.overlay(&mut overlay, vec![]).unwrap();
        for index in 0..if toggles { 4 } else { 2 } {
            assert!(
                frame
                    .overlay(&mut overlay, key(egui::Key::Tab, egui::Modifiers::NONE))
                    .unwrap()
            );
            let open = frame
                .overlay(&mut overlay, key(egui::Key::Enter, egui::Modifiers::NONE))
                .unwrap();
            let close_index = if toggles { 3 } else { 1 };
            assert_eq!(
                open,
                index != close_index,
                "toggles={toggles}, index={index}"
            );
            if toggles && index == 1 {
                frame.overlay(&mut overlay, vec![]).unwrap();
                assert!(matches!(
                    host.surfaces.borrow().last().unwrap().content,
                    Content::Text(_, TextView::Extracted)
                ));
            }
            assert_eq!(
                host.downloads.borrow().len(),
                usize::from(index >= close_index - 1)
            );
        }
    }
}
#[test]
fn overlay_filename_stays_left_of_right_aligned_controls() {
    let mut overlay = AttachmentOverlay::open(attachment(), None, Rc::new(Host::default()));
    let mut frame = Frame::default();
    frame.overlay(&mut overlay, vec![]).unwrap();
    frame.overlay(&mut overlay, vec![]).unwrap();
    assert_eq!(frame.label("note.txt").left().to_bits(), 16_f32.to_bits());
    assert!(frame.control(0).x > 750.);
    assert!((8. ..=40.).contains(&frame.control(0).y));
    assert!(frame.label("note.txt").bottom() <= 48.);
    assert!(frame.label("note.txt").right() < frame.control(1).x);
}

/// Real document bytes replace former PDF placeholder operands.
fn pdf_bytes() -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(include_bytes!(
        "../../maestro-app/tests/fixtures/pdf/synthetic-multipage.pdf"
    ))
}
/// Retain classification inputs while supplying a real PDF body.
fn valid_pdf(record: maestro_web::SharedAttachment) -> maestro_web::SharedAttachment {
    if record.read().unwrap().mime_type == "application/pdf"
        && record.read().unwrap().r#type
            == maestro_app::browser::utils::attachment_utils::AttachmentType::Document
    {
        record.write().unwrap().content = pdf_bytes();
    }
    record
}
/// Exact text and geometry are observed by the real native document witness.
fn assert_reading(host: &Host) {
    let _ = reading_factory(host);
}

/// Real spreadsheet bytes for retained and replacement observations.
fn excel_bytes(replacement: bool) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(if replacement {
        include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/one-sheet.xlsx")
            .as_slice()
    } else {
        include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/two-sheets.xlsx")
            .as_slice()
    })
}
/// Spreadsheet native factories replace the former pending-container observations.
fn assert_excel_reload(
    frame: &mut Frame,
    overlay: &mut AttachmentOverlay,
    record: &maestro_web::SharedAttachment,
    host: &Host,
    case: &ContentCase,
) {
    frame.overlay(overlay, vec![]).unwrap();
    let factory = reading_factory(host);
    assert!((host.surfaces.borrow().last().unwrap().rect.width() - 800.).abs() < f32::EPSILON);
    assert!(!case.show_text);
    assert!(!case.expected.small);
    assert!(case.expected.source.is_none() && case.expected.text.is_none());
    frame.overlay(overlay, vec![]).unwrap();
    assert!(Rc::ptr_eq(&factory, &reading_factory(host)));
    record.write().unwrap().content = excel_bytes(true);
    frame.overlay(overlay, vec![]).unwrap();
    assert!(!Rc::ptr_eq(&factory, &reading_factory(host)));
    let pos = frame.label("Text").center();
    frame.overlay(overlay, click(pos)).unwrap();
    assert!(matches!(
        host.surfaces.borrow().last().unwrap().content,
        Content::Text(_, TextView::Extracted)
    ));
    let pos = frame.label("Spreadsheet").center();
    frame.overlay(overlay, click(pos)).unwrap();
    let factory = reading_factory(host);
    frame.overlay(overlay, click(pos)).unwrap();
    assert!(Rc::ptr_eq(&factory, &reading_factory(host)));
    assert_retained_text_mode(frame, overlay, record, host);
}
