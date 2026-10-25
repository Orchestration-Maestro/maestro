//! Text and image views through the public artifact boundary.
#![cfg(test)]
use crate::artifact_support;
use crate::session_header_support;
use crate::text_image_support;
use artifact_support::Frame;
use maestro_web::{ArtifactElement, tools::artifacts::create_artifact_element};
use session_header_support::Host;
use std::rc::Rc;

/// Recognized text and images install usable retained bodies.
#[test]
fn artifact_factory_installs_text_and_image_views() {
    let host = Rc::new(Host::default());
    let frame = Frame::default();
    for name in ["a.txt", "a.png"] {
        let mut view = create_artifact_element(name, "", host.clone()).unwrap();
        assert_eq!((view.filename(), view.content()), (name, ""));
        frame.draw(view.as_mut(), false, vec![]).unwrap();
    }
    for mut view in [
        Box::new(maestro_web::TextArtifact::new(host.clone())) as Box<dyn ArtifactElement>,
        Box::new(maestro_web::ImageArtifact::new(host.clone())),
    ] {
        frame.draw(view.as_mut(), false, vec![]).unwrap();
    }
    for name in ["a.html", "a.docx"] {
        assert!(create_artifact_element(name, "", host.clone()).is_err());
    }
}
use crate::effects_support;
use maestro_web::{ImageArtifact, TextArtifact};
use text_image_support::{Highlighter, corpus};

/// Language requests carry the exact label and complete original source.
#[test]
fn text_code_files_request_their_mapped_language() {
    let engine = Rc::new(Highlighter::default());
    let frame = Frame {
        highlighter: engine.clone(),
        ..Frame::default()
    };
    for case in corpus().text_language {
        let Some(language) = case.output else {
            continue;
        };
        let mut view = TextArtifact::new(Rc::new(Host::default()));
        view.set_filename(case.input.filename);
        view.set_content("source <>& é".into());
        let start = engine.requests.borrow().len();
        frame.draw(&mut view, false, vec![]).unwrap();
        assert_eq!(
            &engine.requests.borrow()[start..],
            &[("source <>& é".into(), language)]
        );
    }
}
/// Non-code filenames never consult the classifier.
#[test]
fn text_other_files_render_without_highlighting() {
    let engine = Rc::new(Highlighter::default());
    let frame = Frame {
        highlighter: engine.clone(),
        ..Frame::default()
    };
    for case in corpus().text_language {
        if case.output.is_some() {
            continue;
        }
        let mut view = TextArtifact::new(Rc::new(Host::default()));
        view.set_filename(case.input.filename);
        frame.draw(&mut view, false, vec![]).unwrap();
    }
    assert!(engine.supports.borrow().is_empty());
    assert!(engine.requests.borrow().is_empty());
}
/// Idle redraws retain prepared output; edits alone trigger classification.
#[test]
fn text_highlighting_waits_for_edits() {
    let engine = Rc::new(Highlighter::default());
    let frame = Frame {
        highlighter: engine.clone(),
        ..Frame::default()
    };
    let mut view = TextArtifact::new(Rc::new(Host::default()));
    view.set_filename("a.js".into());
    view.set_content("one".into());
    for _ in 0..3 {
        frame.draw(&mut view, false, vec![]).unwrap();
    }
    assert_eq!(
        engine.requests.borrow().as_slice(),
        &[("one".into(), "javascript".into())]
    );
    assert!(!frame.context.has_requested_repaint());
    view.set_content("two".into());
    assert!(frame.context.has_requested_repaint());
    frame.draw(&mut view, false, vec![]).unwrap();
    view.set_filename("notes.txt".into());
    frame.draw(&mut view, false, vec![]).unwrap();
    view.set_filename("x.rb".into());
    frame.draw(&mut view, false, vec![]).unwrap();
    assert_eq!(
        &engine.requests.borrow()[1..],
        &[
            ("two".into(), "javascript".into()),
            ("two".into(), "ruby".into())
        ]
    );
}
/// Unsupported labels and engine failures never hide the source body.
#[test]
fn text_body_never_reports_highlighter_failures() {
    for unsupported in [false, true] {
        let engine = Rc::new(Highlighter::default());
        engine.unsupported.set(unsupported);
        engine.fail.set(true);
        let frame = Frame {
            highlighter: engine.clone(),
            ..Frame::default()
        };
        let mut view = TextArtifact::new(Rc::new(Host::default()));
        view.set_filename("a.sass".into());
        view.set_content("$c: red".into());
        for _ in 0..2 {
            frame.draw(&mut view, false, vec![]).unwrap();
        }
        assert_eq!(engine.supports.borrow().len(), 1);
        assert_eq!(engine.requests.borrow().len(), usize::from(!unsupported));
    }
}
/// Copy and download consume literal values through distinct controls.
#[test]
fn text_header_copies_and_downloads_original_text() {
    for (index, case) in corpus().text_header.into_iter().enumerate() {
        let frame = Frame::default();
        let host = Rc::new(effects_support::Host::default());
        let mut view = TextArtifact::new(host.clone());
        view.set_filename(case.input.filename);
        view.set_content(case.input.content);
        let output = frame.activate_header(&mut view, 0, 2);
        let names: Vec<_> = output
            .platform_output
            .events
            .iter()
            .filter_map(|event| match event {
                egui::output::OutputEvent::Clicked(info) => info.label.as_deref(),
                _ => None,
            })
            .collect();
        assert_eq!(names, [case.output.copy.title.as_str()]);
        assert_eq!(host.copies.borrow().as_slice(), &[case.output.copy.text]);
        assert!(!case.output.copy.show_text);
        if [1, 2, 6].contains(&index) {
            continue;
        }
        let output = frame.activate_header(&mut view, 1, 2);
        assert!(output.platform_output.events.iter().any(|event| matches!(event, egui::output::OutputEvent::Clicked(info) if info.label.as_deref() == Some(&case.output.download.title))));
        assert_eq!(
            host.downloads.borrow().as_slice(),
            &[(
                case.output.download.filename,
                case.output.download.mime_type,
                case.output.download.bytes
            )]
        );
    }
}
/// Base64-looking text downloads as UTF-8 rather than decoded binary.
#[test]
fn text_download_saves_literal_text_bytes() {
    for (index, case) in corpus().text_header.into_iter().enumerate() {
        if ![1, 2, 6].contains(&index) {
            continue;
        }
        let frame = Frame::default();
        let host = Rc::new(Host::default());
        let mut view = TextArtifact::new(host.clone());
        view.set_filename(case.input.filename);
        view.set_content(case.input.content);
        let output = frame.activate_header(&mut view, 1, 2);
        assert!(output.platform_output.events.iter().any(|event| matches!(event, egui::output::OutputEvent::Clicked(info) if info.label.as_deref() == Some(&case.output.download.title))));
        assert_eq!(
            host.downloads.borrow().as_slice(),
            &[(
                case.output.download.filename,
                case.output.download.mime_type,
                case.output.download.bytes
            )]
        );
    }
}
/// Text MIME follows the last literal extension.
#[test]
fn text_download_mime_tracks_the_last_extension() {
    for case in corpus().text_mime {
        let frame = Frame::default();
        let host = Rc::new(Host::default());
        let mut view = TextArtifact::new(host.clone());
        view.set_filename(case.input.filename.clone());
        frame.activate_header(&mut view, 1, 2);
        assert_eq!(
            host.downloads.borrow().as_slice(),
            &[(case.input.filename, case.output, vec![])]
        );
    }
}
/// The same copy owner keeps its single two-second timer across header redraws.
#[test]
fn text_copy_feedback_survives_redraws() {
    let host = Rc::new(effects_support::Host::default());
    let mut view = TextArtifact::new(host.clone());
    let frame = Frame::default();
    view.set_content("before".into());
    frame.activate_header(&mut view, 0, 2);
    host.results.borrow()[0].set(Some(true));
    host.run_work();
    for _ in 0..2 {
        let output = frame.draw(&mut view, true, vec![]).unwrap();
        assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains("Copied!"))));
    }
    assert_eq!((host.live(), host.admissions.get()), (1, 1));
    host.advance(1999);
    assert_eq!(host.live(), 1);
    host.advance(2000);
    assert_eq!(host.live(), 0);
    view.set_content("after".into());
    frame.activate_header(&mut view, 0, 2);
    assert_eq!(host.copies.borrow().as_slice(), &["before", "after"]);
}
/// Every successful payload reaches the original-byte download boundary.
#[test]
fn image_download_saves_decoded_bytes() {
    for case in corpus().image_bytes {
        let host = Rc::new(Host::default());
        let mut view = ImageArtifact::new(host.clone());
        view.set_filename("a.png".into());
        view.set_content(case.input.content);
        Frame::default().click(&mut view).unwrap();
        assert_eq!(
            host.downloads.borrow().as_slice(),
            &[("a.png".into(), "image/png".into(), case.output.bytes)]
        );
    }
}
/// Malformed downloads fail before saving while the independent body still mounts.
#[test]
fn image_download_rejects_invalid_base64() {
    for case in corpus().image_invalid {
        let host = Rc::new(Host::default());
        let mut view = ImageArtifact::new(host.clone());
        view.set_content(case.content);
        let frame = Frame::default();
        assert!(matches!(
            frame.draw(&mut view, true, vec![]),
            Err(maestro_web::tools::artifacts::ArtifactUiError::Presentation(_))
        ));
        assert!(host.downloads.borrow().is_empty());
        frame.draw(&mut view, false, vec![]).unwrap();
        assert_eq!(host.published.borrow().len(), 1);
    }
}
/// Image MIME follows the final filename segment, independent of payload.
#[test]
fn image_download_mime_tracks_the_last_extension() {
    for case in corpus().image_mime {
        let host = Rc::new(Host::default());
        let mut view = ImageArtifact::new(host.clone());
        view.set_filename(case.input.filename.clone());
        view.set_content("Zg==".into());
        Frame::default().click(&mut view).unwrap();
        assert_eq!(
            host.downloads.borrow().as_slice(),
            &[(case.input.filename, case.output, vec![102])]
        );
    }
}
/// Each view keeps its factory on edits and revokes only its own surface on drop.
#[test]
fn text_and_image_views_release_native_surfaces() {
    let host = Rc::new(Host::default());
    let frame = Frame::default();
    let mut neighbor = create_artifact_element("neighbor.zip", "", host.clone()).unwrap();
    frame.draw(neighbor.as_mut(), false, vec![]).unwrap();
    for mut view in [
        Box::new(TextArtifact::new(host.clone())) as Box<dyn ArtifactElement>,
        Box::new(ImageArtifact::new(host.clone())),
    ] {
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        // Select this view by its new admission, not hash-map iteration order.
        let content = host.admissions.borrow().last().unwrap().clone();
        let id = *host
            .published
            .borrow()
            .iter()
            .find(|(_, (candidate, _))| Rc::ptr_eq(candidate, &content))
            .unwrap()
            .0;
        view.set_visibility(false, Some(egui::Id::new("modal")))
            .unwrap();
        assert_eq!(
            host.retained.borrow()[&id],
            (false, Some(egui::Id::new("modal")))
        );
        view.set_content("changed".into());
        assert!(frame.context.has_requested_repaint());
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        assert!(Rc::ptr_eq(&content, &host.published.borrow()[&id].0));
        drop(view);
        assert!(!host.published.borrow().contains_key(&id));
        assert_eq!(host.published.borrow().len(), 1);
        assert_eq!(
            host.events
                .borrow()
                .iter()
                .filter(|event| **event == session_header_support::Event::Remove(id))
                .count(),
            1
        );
    }
}
