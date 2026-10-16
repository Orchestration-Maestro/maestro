//! PDF artifact and attachment public behavior.
#![cfg(test)]
pub mod artifact_support;
pub mod attachment_support;
pub mod session_header_support;
pub mod support;
use base64::Engine as _;
use maestro_web::tools::artifacts::create_artifact_element;
use std::rc::Rc;

#[test]
fn pdf_factory_installs_only_the_pdf_specialization() {
    let host = Rc::new(session_header_support::Host::default());
    for name in ["a.pdf", "a.PDF", "a.unknown"] {
        let view = create_artifact_element(name, "", host.clone()).unwrap();
        assert_eq!(view.filename(), name);
    }
    for name in [
        "a.html", "a.svg", "a.md", "a.png", "a.docx", "a.xls", "a.txt",
    ] {
        assert!(create_artifact_element(name, "", host.clone()).is_err());
    }
}

#[test]
fn pdf_artifact_fields_start_empty_and_retain_owned_edits() {
    use maestro_web::{ArtifactElement, tools::artifacts::pdf_artifact::PdfArtifact};
    let host = Rc::new(session_header_support::Host::default());
    let mut view = PdfArtifact::new(host.clone());
    assert_eq!((view.filename(), view.content()), ("", ""));
    let frame = artifact_support::Frame::default();
    frame.draw(&mut view, false, vec![]).unwrap();
    frame.click(&mut view).unwrap();
    assert_eq!(host.downloads.borrow().last().unwrap().2, Vec::<u8>::new());
    view.set_filename(String::from("雪 <file>.notpdf"));
    view.set_content(encoded("synthetic-multipage.pdf"));
    assert!(frame.context.has_requested_repaint());
    frame.draw(&mut view, false, vec![]).unwrap();
    let reading = host.published.borrow().values().next().unwrap().0.clone();
    view.set_filename("literal filename".into());
    frame.draw(&mut view, false, vec![]).unwrap();
    assert!(Rc::ptr_eq(
        &reading,
        &host.published.borrow().values().next().unwrap().0
    ));
    assert_eq!(view.filename(), "literal filename");
    assert_eq!(view.content(), encoded("synthetic-multipage.pdf"));
    frame.click(&mut view).unwrap();
    assert_eq!(host.downloads.borrow().last().unwrap().1, "application/pdf");
}

#[test]
fn pdf_artifact_retains_visibility_without_modal_ownership_for_pages_and_errors() {
    let frame = artifact_support::Frame::default();
    let modal = Some(egui::Id::new("pdf-panel-modal"));
    for content in [encoded("synthetic-multipage.pdf"), "aGVsbG8=".into()] {
        let host = Rc::new(session_header_support::Host::default());
        let mut view = create_artifact_element("a.pdf", &content, host.clone()).unwrap();
        view.set_visibility(false, modal).unwrap();
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        assert!(host.published.borrow().is_empty());
        assert_eq!(
            host.retained.borrow().values().copied().collect::<Vec<_>>(),
            [(false, modal)]
        );
        view.set_visibility(true, modal).unwrap();
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        let (id, reading) = {
            let published = host.published.borrow();
            let (id, (reading, _)) = published.iter().next().unwrap();
            (*id, Rc::clone(reading))
        };
        assert_eq!(host.retained.borrow()[&id], (true, modal));
        assert_eq!(host.publication_modals.borrow()[&id], None);
        view.set_visibility(false, None).unwrap();
        let events = host.events.borrow().len();
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        assert_eq!(host.retained.borrow()[&id], (false, None));
        assert_eq!(host.events.borrow().len(), events);
        view.set_visibility(true, None).unwrap();
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        assert_eq!(host.retained.borrow()[&id], (true, None));
        assert_eq!(host.publication_modals.borrow()[&id], None);
        assert!(Rc::ptr_eq(&reading, &host.published.borrow()[&id].0));
    }
}

fn encoded(name: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(
        std::fs::read(format!(
            "{}/../maestro-app/tests/fixtures/pdf/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
}

#[test]
fn pdf_preview_advances_once_per_frame_without_reopening() {
    use attachment_support::{Content, Frame, Host, attachment};
    use maestro_web::browser::attachment_content::ReadingPhase;
    let record = attachment();
    record.borrow_mut().mime_type = "application/pdf".into();
    record.borrow_mut().content = encoded("synthetic-multipage.pdf");
    let host = Rc::new(Host::default());
    let mut overlay = maestro_web::AttachmentOverlay::open(record, None, host.clone());
    let mut frame = Frame::default();
    frame.overlay(&mut overlay, vec![]).unwrap();
    let Content::Reading(first, ReadingPhase::Publish) =
        host.surfaces.borrow().last().unwrap().content.clone()
    else {
        panic!("PDF not admitted as reading content")
    };

    assert!(frame.ctx.has_requested_repaint());
    for _ in 0..4 {
        frame.overlay(&mut overlay, vec![]).unwrap();
        assert!(
            matches!(&host.surfaces.borrow().last().unwrap().content,Content::Reading(current,ReadingPhase::Publish) if Rc::ptr_eq(current,&first))
        );
    }
}

pub mod pdf_support;
#[test]
fn pdf_download_and_render_share_complete_original_bytes() {
    use pdf_support::Outcome;
    let frame = artifact_support::Frame::default();
    for row in pdf_support::rows() {
        let host = Rc::new(session_header_support::Host::default());
        let mut view = create_artifact_element("a.pdf", &row.input.content, host.clone()).unwrap();
        match row.artifact {
            Outcome::Success { bytes } => {
                frame.click(view.as_mut()).unwrap();
                assert_eq!(host.downloads.borrow().last().unwrap().2, bytes);
                frame.draw(view.as_mut(), false, vec![]).unwrap();
            }
            Outcome::Error => {
                assert!(frame.click(view.as_mut()).is_err());
                assert!(host.downloads.borrow().is_empty());
            }
        }
        let _ = row.overlay; // Raw outcomes are consumed by the decoder unit witness.
    }
}

#[test]
fn pdf_overlay_keeps_raw_base64_admission() {
    use attachment_support::{Content, Frame, Host, attachment};
    for (input, success) in [
        (encoded("synthetic-multipage.pdf"), true),
        (
            format!(
                "data:application/pdf;base64,{}",
                encoded("synthetic-multipage.pdf")
            ),
            false,
        ),
        ("hello!".into(), false),
    ] {
        let record = attachment();
        record.borrow_mut().mime_type = "application/pdf".into();
        record.borrow_mut().content = input;
        let host = Rc::new(Host::default());
        let mut overlay = maestro_web::AttachmentOverlay::open(record, None, host.clone());
        let mut frame = Frame::default();
        frame.overlay(&mut overlay, vec![]).unwrap();
        assert_eq!(!host.surfaces.borrow().is_empty(), success);
        frame.overlay(&mut overlay, vec![]).unwrap();
        assert!(matches!(
            host.surfaces.borrow().last().unwrap().content,
            Content::Reading(..)
        ));
        assert!(
            !host
                .surfaces
                .borrow()
                .iter()
                .any(|s| matches!(s.content, Content::Text(..)))
        );
    }
}

#[test]
fn pdf_download_preserves_name_mime_and_save_failures() {
    let frame = artifact_support::Frame::default();
    let host = Rc::new(session_header_support::Host::default());
    let content = encoded("synthetic-wide.pdf");
    let mut view =
        create_artifact_element("雪 <file> \"quoted\".pdf", &content, host.clone()).unwrap();
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    let reading = host.published.borrow().values().next().unwrap().0.clone();
    host.fail_download.set(true);
    assert!(matches!(
        frame.click(view.as_mut()),
        Err(maestro_web::browser::attachment_content::AttachmentUiError::Host(_))
    ));
    host.fail_download.set(false);
    frame.click(view.as_mut()).unwrap();
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    assert!(Rc::ptr_eq(
        &reading,
        &host.published.borrow().values().next().unwrap().0
    ));
    assert_eq!(host.downloads.borrow()[0], host.downloads.borrow()[1]);
    assert_eq!(host.downloads.borrow()[1].0, "雪 <file> \"quoted\".pdf");
    assert_eq!(host.downloads.borrow()[1].1, "application/pdf");
    assert_eq!(
        host.downloads.borrow()[1].2,
        include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-wide.pdf")
    );
}

#[test]
fn pdf_edits_cancel_old_pages_before_replacement() {
    let frame = artifact_support::Frame::default();
    let host = Rc::new(session_header_support::Host::default());
    let mut view =
        create_artifact_element("a.pdf", &encoded("synthetic-multipage.pdf"), host.clone())
            .unwrap();
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    let initial = host.published.borrow().values().next().unwrap().0.clone();
    view.set_content(encoded("synthetic-tall.pdf"));
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    let replaced = host.published.borrow().values().next().unwrap().0.clone();
    assert!(!Rc::ptr_eq(&initial, &replaced));
    view.set_content(String::new());
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    assert!(Rc::ptr_eq(
        &replaced,
        &host.published.borrow().values().next().unwrap().0
    ));
    view.set_content("hello!".into());
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    let failed = host.published.borrow().values().next().unwrap().0.clone();
    assert!(!Rc::ptr_eq(&failed, &replaced));
    view.set_content(String::new());
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    assert!(!Rc::ptr_eq(
        &failed,
        &host.published.borrow().values().next().unwrap().0
    ));
    view.set_content(encoded("synthetic-wide.pdf"));
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    frame.click(view.as_mut()).unwrap();
    assert_eq!(
        host.downloads.borrow().last().unwrap().2,
        include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-wide.pdf")
    );
}

#[test]
fn pdf_text_toggle_and_close_cancel_unfinished_pages() {
    use attachment_support::{Content, Frame, Host, attachment, click};
    let record = attachment();
    record.borrow_mut().mime_type = "application/pdf".into();
    record.borrow_mut().content = encoded("synthetic-multipage.pdf");
    let host = Rc::new(Host::default());
    let effects = host.clone();
    let mut overlay = maestro_web::AttachmentOverlay::open(
        record.clone(),
        Some(Rc::new(move || {
            effects.effects.borrow_mut().push("callback".into());
        })),
        host.clone(),
    );
    let mut frame = Frame::default();
    frame.overlay(&mut overlay, vec![]).unwrap();
    let first = match &host.surfaces.borrow().last().unwrap().content {
        Content::Reading(r, _) => r.clone(),
        _ => panic!("missing pages"),
    };
    frame.overlay(&mut overlay, vec![]).unwrap();
    let pos = frame.label("Text").center();
    frame.overlay(&mut overlay, click(pos)).unwrap();
    record.borrow_mut().extracted_text = Some("live alias".into());
    frame.overlay(&mut overlay, vec![]).unwrap();
    assert!(
        matches!(&host.surfaces.borrow().last().unwrap().content,Content::Text(t,_) if t=="live alias")
    );
    let pos = frame.label("PDF").center();
    frame.overlay(&mut overlay, click(pos)).unwrap();
    assert!(
        matches!(&host.surfaces.borrow().last().unwrap().content,Content::Reading(r,_) if !Rc::ptr_eq(r,&first))
    );
    record.borrow_mut().mime_type = "text/plain".into();
    frame.overlay(&mut overlay, vec![]).unwrap();
    record.borrow_mut().mime_type = "application/pdf".into();
    frame.overlay(&mut overlay, vec![]).unwrap();
    let before = host.surfaces.borrow().len();
    frame
        .overlay(
            &mut overlay,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        )
        .unwrap();
    assert!(!frame.overlay(&mut overlay, vec![]).unwrap());
    assert_eq!(host.surfaces.borrow().len(), before);
    let effects = host.effects.borrow();
    let n = effects.iter().position(|e| e == "callback").unwrap();
    assert!(effects[n + 1].starts_with("remove:"));
}

#[test]
fn pdf_load_failures_keep_controls_and_recover_on_authorized_change() {
    use attachment_support::{Content, Frame, Host, attachment, click};
    let record = attachment();
    record.borrow_mut().mime_type = "application/pdf".into();
    record.borrow_mut().content = encoded("synthetic-wide.pdf");
    let host = Rc::new(Host::default());
    *host.fail.borrow_mut() = Some("preview failure".into());
    let mut overlay = maestro_web::AttachmentOverlay::open(record.clone(), None, host.clone());
    let mut frame = Frame::default();
    frame.overlay(&mut overlay, vec![]).unwrap();
    frame.overlay(&mut overlay, vec![]).unwrap();
    let count = host
        .surfaces
        .borrow()
        .iter()
        .filter(|s| !matches!(s.content, Content::Reading(..)))
        .count();
    record.borrow_mut().content = encoded("synthetic-tall.pdf");
    frame.overlay(&mut overlay, vec![]).unwrap();
    assert_eq!(
        host.surfaces
            .borrow()
            .iter()
            .filter(|s| !matches!(s.content, Content::Reading(..)))
            .count(),
        count
    );
    let error = match &host.surfaces.borrow().last().unwrap().content {
        Content::Reading(r, _) => r.clone(),
        _ => panic!("missing error surface"),
    };
    let pos = frame.label("Text").center();
    frame.overlay(&mut overlay, click(pos)).unwrap();
    assert!(matches!(
        host.surfaces.borrow().last().unwrap().content,
        Content::Text(..)
    ));
    let pos = frame.label("PDF").center();
    frame.overlay(&mut overlay, click(pos)).unwrap();
    assert!(
        matches!(&host.surfaces.borrow().last().unwrap().content,Content::Reading(r,_) if !Rc::ptr_eq(r,&error))
    );
}

#[test]
fn pdf_reading_publication_failures_preserve_owned_surface() {
    use session_header_support::Failure;
    let host = Rc::new(session_header_support::Host::default());
    let frame = artifact_support::Frame::default();
    let mut view =
        create_artifact_element("a.pdf", &encoded("synthetic-multipage.pdf"), host.clone())
            .unwrap();
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    let (id, (factory, rect)) = host
        .published
        .borrow()
        .iter()
        .map(|(id, value)| (*id, value.clone()))
        .next()
        .unwrap();
    for phase in [Failure::Measure, Failure::Publish] {
        view.set_content(encoded("synthetic-tall.pdf"));
        host.failure.set(Some(phase));
        assert!(frame.draw(view.as_mut(), false, vec![]).is_err());
        assert_eq!(host.published.borrow()[&id].1, rect);
        assert!(Rc::ptr_eq(&factory, &host.published.borrow()[&id].0));
        host.failure.set(None);
    }
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    assert!(!Rc::ptr_eq(&factory, &host.published.borrow()[&id].0));
    drop(view);
    assert!(host.published.borrow().is_empty());
}

#[test]
fn pdf_local_application_tool_updates_the_real_pdf_view() {
    use artifact_support::presentation::{Presentation, ready};
    use maestro_app::browser::tools::artifacts::LocalArtifactsPanel;
    use serde_json::json;
    use std::cell::{Cell, RefCell};
    let host = Rc::new(session_header_support::Host::default());
    let presentation = Rc::new(Presentation {
        panel: None,
        host: host.clone(),
        view: RefCell::default(),
        created: Cell::default(),
        opened: Cell::default(),
        changed: Cell::default(),
    });
    let caller = LocalArtifactsPanel::new_local(presentation.clone());
    let tool = caller.tool();
    let frame = artifact_support::Frame::default();
    let mut identity = None;
    for (args, content, bytes) in pdf_tool_edits() {
        ready((tool.execute)("call".into(), args, None, None)).unwrap();
        let mut view = presentation.view.borrow_mut().take().unwrap();
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        frame.click(view.as_mut()).unwrap();
        assert_eq!(view.content(), content);
        let current = (&raw const *view).cast::<()>();
        assert_eq!(*identity.get_or_insert(current), current);
        assert_eq!(host.downloads.borrow().last().unwrap().2, bytes);
        *presentation.view.borrow_mut() = Some(view);
    }
    assert_eq!(presentation.created.get(), 1);
    ready((tool.execute)(
        "delete".into(),
        json!({"command":"delete","filename":"a.pdf"}),
        None,
        None,
    ))
    .unwrap();
    assert!(presentation.view.borrow().is_none());
    assert!(host.published.borrow().is_empty());
    drop(caller);
    let weak = Rc::downgrade(&presentation);
    drop(presentation);
    drop(tool);
    assert!(weak.upgrade().is_none());
}

/// Real create, replacement and appended-content commands with original byte expectations.
fn pdf_tool_edits() -> [(serde_json::Value, String, &'static [u8]); 3] {
    use serde_json::json;
    let original = encoded("synthetic-multipage.pdf");
    let replacement = encoded("synthetic-tall.pdf");
    let appended = format!("{replacement}\n");
    [
        (
            json!({"command":"create","filename":"a.pdf","content":original}),
            original,
            include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-multipage.pdf")
                .as_slice(),
        ),
        (
            json!({"command":"rewrite","filename":"a.pdf","content":replacement}),
            replacement.clone(),
            include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-tall.pdf").as_slice(),
        ),
        (
            json!({"command":"update","filename":"a.pdf","old_str":replacement,"new_str":appended}),
            appended,
            include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-tall.pdf").as_slice(),
        ),
    ]
}
