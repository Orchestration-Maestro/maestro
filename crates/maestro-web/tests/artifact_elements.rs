//! Public generic artifact composition.
#![cfg(test)]
pub mod artifact_support;
pub mod session_header_support;
pub mod support;
use maestro_web::tools::artifacts::create_artifact_element;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
/// Recognized specialized files never silently become download-only views.
#[test]
fn generic_factory_never_replaces_recognized_views() {
    let host = Rc::new(session_header_support::Host::default());
    let view = create_artifact_element("a.md.zip", "Zg==", host.clone()).unwrap();
    assert_eq!(view.filename(), "a.md.zip");
    assert_eq!(view.content(), "Zg==");
    for name in [
        "a.html", "a.svg", "a.md", "a.png", "a.pdf", "a.docx", "a.xls", "a.txt",
    ] {
        assert!(create_artifact_element(name, "", host.clone()).is_err());
    }
}
use artifact_support::{Decoded, Frame, corpus};
use maestro_web::{ArtifactElement, tools::artifacts::generic_artifact::GenericArtifact};
use session_header_support::{Failure, Host};
/// The SDK root trait admits the concrete view through both drawing operations.
#[test]
fn artifact_element_and_local_caller_are_usable_by_embedders() {
    let host = Rc::new(Host::default());
    let mut view: Box<dyn ArtifactElement> =
        create_artifact_element("a.zip", "Zg==", host.clone()).unwrap();
    view.set_filename("b.zip".into());
    view.set_content("Zm8=".into());
    let frame = Frame::default();
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    frame.click(view.as_mut()).unwrap();
    assert_eq!(
        host.downloads.borrow().as_slice(),
        &[("b.zip".into(), "application/zip".into(), vec![102, 111])]
    );
}
/// Content edits preserve original fields and the admitted body factory.
#[test]
fn generic_content_edits_keep_filename_and_original_content() {
    let host = Rc::new(Host::default());
    let mut view = GenericArtifact::new(host.clone());
    assert_eq!((view.filename(), view.content()), ("", ""));
    view.set_filename("<script>雪.zip".into());
    view.set_content("not base64!".into());
    let frame = Frame::default();
    frame.draw(&mut view, false, vec![]).unwrap();
    let content = host.published.borrow().values().next().unwrap().0.clone();
    view.set_content(String::from("Zm8="));
    assert!(frame.context.has_requested_repaint());
    frame.draw(&mut view, false, vec![]).unwrap();
    assert!(Rc::ptr_eq(
        &content,
        &host.published.borrow().values().next().unwrap().0
    ));
    assert_eq!(view.filename(), "<script>雪.zip");
    assert_eq!(view.content(), "Zm8=");
}
/// Every media-type case reaches the actual header control.
#[test]
fn generic_download_mime_tracks_the_last_extension() {
    let frame = Frame::default();
    for case in corpus().mime {
        let host = Rc::new(Host::default());
        let mut view = GenericArtifact::new(host.clone());
        view.set_filename(case.input.filename.clone());
        view.set_content("Zg==".into());
        frame.click(&mut view).unwrap();
        assert_eq!(
            host.downloads.borrow().as_slice(),
            &[(case.input.filename, case.output, vec![102])]
        );
    }
}
/// Folded data URLs retain the complete decoded original bytes.
#[test]
fn generic_download_decodes_the_complete_original_payload() {
    let frame = Frame::default();
    for case in corpus().decode {
        let Decoded::Bytes { bytes } = case.output else {
            continue;
        };
        let host = Rc::new(Host::default());
        let mut view = create_artifact_element("a.zip", &case.input.content, host.clone()).unwrap();
        frame.click(view.as_mut()).unwrap();
        assert_eq!(
            host.downloads.borrow().as_slice(),
            &[("a.zip".into(), "application/zip".into(), bytes)]
        );
    }
}
/// Invalid complete payloads fail before any platform download effect.
#[test]
fn generic_download_rejects_invalid_payload_without_saving() {
    let frame = Frame::default();
    for case in corpus().decode {
        let Decoded::Error { error } = case.output else {
            continue;
        };
        assert_eq!(error, "InvalidCharacterError");
        let host = Rc::new(Host::default());
        let mut view = create_artifact_element("a.zip", &case.input.content, host.clone()).unwrap();
        assert!(matches!(
            frame.click(view.as_mut()),
            Err(maestro_web::browser::attachment_content::AttachmentUiError::Presentation(_))
        ));
        assert!(host.downloads.borrow().is_empty());
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        assert_eq!(host.published.borrow().len(), 1);
    }
}
/// A reached save failure keeps the body and retries the same original bytes.
#[test]
fn generic_download_failure_keeps_the_native_body() {
    let host = Rc::new(Host::default());
    let mut view = create_artifact_element("a.zip", "AAEC//79", host.clone()).unwrap();
    let frame = Frame::default();
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    let content = host.published.borrow().values().next().unwrap().0.clone();
    host.fail_download.set(true);
    assert!(matches!(
        frame.click(view.as_mut()),
        Err(maestro_web::browser::attachment_content::AttachmentUiError::Host(_))
    ));
    assert_eq!(view.content(), "AAEC//79");
    host.fail_download.set(false);
    frame.click(view.as_mut()).unwrap();
    assert_eq!(host.downloads.borrow().len(), 2);
    assert_eq!(host.downloads.borrow()[0], host.downloads.borrow()[1]);
    assert!(Rc::ptr_eq(
        &content,
        &host.published.borrow().values().next().unwrap().0
    ));
}
/// Failed admission/publication never replaces successful placement or factory.
#[test]
fn generic_reading_failures_keep_published_identity() {
    let host = Rc::new(Host::default());
    let mut view = create_artifact_element("a.zip", "", host.clone()).unwrap();
    let frame = Frame::default();
    host.failure.set(Some(Failure::Measure));
    assert!(frame.draw(view.as_mut(), false, vec![]).is_err());
    assert!(host.published.borrow().is_empty());
    host.failure.set(None);
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    let (id, (content, rect)) = host
        .published
        .borrow()
        .iter()
        .map(|(id, pair)| (*id, pair.clone()))
        .next()
        .unwrap();
    host.failure.set(Some(Failure::Publish));
    assert!(frame.draw(view.as_mut(), false, vec![]).is_err());
    assert_eq!(host.published.borrow()[&id].1, rect);
    assert!(Rc::ptr_eq(&host.published.borrow()[&id].0, &content));
    host.failure.set(None);
    frame.draw(view.as_mut(), false, vec![]).unwrap();
    assert!(Rc::ptr_eq(&host.published.borrow()[&id].0, &content));
    drop(view);
    assert!(host.published.borrow().is_empty());
    assert_eq!(
        host.events.borrow().last(),
        Some(&session_header_support::Event::Remove(id))
    );
}
/// Native reading receives caller-selected brand roles in both modes and packs.
#[test]
fn generic_styles_follow_supplied_brand_roles() {
    for file in ["brand.json", "packs/signal.json"] {
        let mut frame = Frame {
            pack: maestro_theme::load_brand_pack(
                &format!("{}/../../assets/brand/{file}", env!("CARGO_MANIFEST_DIR")),
                &maestro_theme::theme::NativeThemeOperations,
            )
            .unwrap(),
            ..Frame::default()
        };
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            frame.mode = mode;
            let host = Rc::new(Host::default());
            let mut view = create_artifact_element("a.zip", "bad!", host.clone()).unwrap();
            let output = frame.draw(view.as_mut(), false, vec![]).unwrap();
            assert_eq!(
                host.css.borrow().last().unwrap(),
                &frame.pack.presentation(mode).unwrap().css_properties()
            );
            let rect = host.published.borrow().values().next().unwrap().1;
            assert_eq!(rect.size(), egui::vec2(400., 300.));
            assert!(
                output
                    .shapes
                    .iter()
                    .all(|s| !matches!(s.shape, egui::Shape::Text(_) | egui::Shape::Mesh(_)))
            );
            assert!(host.downloads.borrow().is_empty());
        }
    }
}
/// Actual local tool calls update the retained real view and clicked downloads.
#[test]
fn local_artifact_tool_updates_the_real_generic_view() {
    use artifact_support::presentation::{Presentation, ready};
    use maestro_app::browser::tools::artifacts::LocalArtifactsPanel;
    use serde_json::json;
    let host = Rc::new(Host::default());
    let presentation = Rc::new(Presentation {
        host: host.clone(),
        view: RefCell::default(),
        created: Cell::default(),
        opened: Cell::default(),
        changed: Cell::default(),
    });
    let caller = LocalArtifactsPanel::new_local(presentation.clone());
    let tool = caller.tool();
    let frame = Frame::default();
    let mut identity = None;
    for (args, bytes) in edit_cases() {
        ready((tool.execute)("call".into(), args, None, None)).unwrap();
        let mut view = presentation.view.borrow_mut().take().unwrap();
        frame.draw(view.as_mut(), false, vec![]).unwrap();
        frame.click(view.as_mut()).unwrap();
        assert_eq!(
            host.downloads.borrow().last().unwrap(),
            &("a.md.zip".into(), "application/zip".into(), bytes)
        );
        let current = (&raw const *view).cast::<()>();
        assert_eq!(*identity.get_or_insert(current), current);
        *presentation.view.borrow_mut() = Some(view);
    }
    assert_eq!(presentation.created.get(), 1);
    caller.open_artifact("a.md.zip");
    assert_eq!(presentation.opened.get(), 1);
    assert_eq!(presentation.changed.get(), 3);
    drop(caller);
    let get = ready((tool.execute)(
        "get".into(),
        json!({"command":"get","filename":"a.md.zip"}),
        None,
        None,
    ))
    .unwrap();
    assert!(serde_json::to_string(&get).unwrap().contains("AAEC//79"));
    ready((tool.execute)(
        "delete".into(),
        json!({"command":"delete","filename":"a.md.zip"}),
        None,
        None,
    ))
    .unwrap();
    assert!(presentation.view.borrow().is_none());
    assert!(host.published.borrow().is_empty());
    let weak = Rc::downgrade(&presentation);
    drop(presentation);
    drop(tool);
    assert!(weak.upgrade().is_none());
}

/// Distinguishable application edits and their original-byte expectations.
fn edit_cases() -> [(serde_json::Value, Vec<u8>); 3] {
    use serde_json::json;
    [
        (
            json!({"command":"create","filename":"a.md.zip","content":"Zg=="}),
            vec![102],
        ),
        (
            json!({"command":"update","filename":"a.md.zip","old_str":"Zg==","new_str":"Zm8="}),
            vec![102, 111],
        ),
        (
            json!({"command":"rewrite","filename":"a.md.zip","content":"AAEC//79"}),
            vec![0, 1, 2, 255, 254, 253],
        ),
    ]
}
