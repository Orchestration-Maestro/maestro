//! Native generic file artifact body and independent header in a real document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, select, view};
use crate::native_simple_artifacts_support::{Decoded, GuidanceExpected, corpus};
use crate::session_download_browser::Fixture;
use crate::session_header_support::{Event, Host};
use maestro_web::{
    ArtifactElement,
    dom::{DomContent, DomRoot, DomView},
    tools::artifacts::generic_artifact::GenericArtifact,
};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement, Node};

/// Localized page operands for `language`.
fn localized(language: &str) -> Rc<DomView> {
    let view = view();
    view.i18n.set_language(language).unwrap();
    view
}
/// A generic artifact holding the supplied authored fields.
fn artifact(filename: &str, content: &str, host: &Rc<Host>) -> GenericArtifact {
    let mut artifact = GenericArtifact::new(
        Rc::clone(host) as Rc<dyn maestro_web::browser::attachment_content::AttachmentHost>
    );
    artifact.set_filename(filename.to_owned());
    artifact.set_content(content.to_owned());
    artifact
}
/// The header control button of `root`.
fn button(root: &DomRoot) -> HtmlElement {
    select(&root.node, "button").unchecked_into()
}
/// The recorded guidance, glyph and filename are literal native text.
fn assert_guidance(root: &DomRoot, expected: &GuidanceExpected) {
    let root: &Element = root.node.unchecked_ref();
    assert_eq!(root.class_name(), "maestro-generic-artifact");
    let filename = select(&root.clone().into(), "div > div");
    let text = filename.first_child().expect("filename text node");
    assert_eq!(text.node_type(), Node::TEXT_NODE);
    assert_eq!(
        text.text_content().as_deref(),
        Some(expected.filename.as_str())
    );
    let guidance = select(&root.clone().into(), "p");
    let nodes = guidance.query_selector_all("span").unwrap();
    let spans: Vec<Element> = (0..nodes.length())
        .filter_map(|index| nodes.item(index))
        .filter(|node| node.unchecked_ref::<Element>().text_content().as_deref() != Some(" "))
        .map(wasm_bindgen::JsCast::unchecked_into)
        .collect();
    assert_eq!(spans.len(), 2);
    for (span, sentence) in spans.iter().zip(&expected.guidance) {
        assert_eq!(span.text_content().as_deref(), Some(sentence.as_str()));
    }
    assert!(select(&root.clone().into(), "svg path").has_attribute("d"));
}
/// Mount one generic body with the recorded fields and observe its visible output.
#[wasm_bindgen]
pub fn generic_artifact_dom_guidance_is_literal_and_localized() {
    let host = Rc::new(Host::default());
    for case in corpus().guidance {
        let view = localized(&case.input.language);
        let artifact = artifact(&case.input.filename, &case.input.content, &host);
        let root = artifact.mount(&view).unwrap();
        assert_guidance(&root, &case.expected);
        let header = artifact.get_header_buttons(&view).unwrap();
        assert_eq!(
            select(&header.node, "button").get_attribute("title"),
            case.expected.titles.first().cloned()
        );
        assert_host_silent(&host);
    }
    // The body only shows literal text: unreadable original bytes never reach it.
    let view = localized("en");
    let artifact = artifact("<literal>&.txt", "not base64!", &host);
    let root = artifact.mount(&view).unwrap();
    assert_eq!(
        select(&root.node, "div > div").text_content().as_deref(),
        Some("<literal>&.txt")
    );
    assert_host_silent(&host);
}
/// The native path never measures, reads, publishes or downloads through the retained host.
///
/// A dropped artifact still records its own removal, which is the retained surface
/// cleanup of the shared type and reaches no placement.
fn assert_host_silent(host: &Host) {
    assert!(host.downloads.borrow().is_empty());
    assert!(host.published.borrow().is_empty());
    assert!(host.admissions.borrow().is_empty());
    let events = host.events.borrow();
    assert!(
        events.iter().all(|event| matches!(event, Event::Remove(_))),
        "{events:?}"
    );
}
/// Mount the independent header of `artifact` and observe one saved download.
async fn saved(
    fixture: &Fixture,
    artifact: &GenericArtifact,
    view: &DomView,
    payload: (&str, &str, &[u8]),
) {
    let (name, mime, bytes) = payload;
    let root = artifact.get_header_buttons(view).unwrap();
    let control = button(&root);
    body().append_child(&root.node).unwrap();
    fixture.saved(&control, bytes, mime, name).await;
    body().remove_child(&root.node).unwrap();
}
/// Every recorded media type and payload reaches the header's byte download.
#[wasm_bindgen]
pub async fn generic_artifact_dom_header_preserves_complete_binary(
    import_module: &js_sys::Function,
) {
    let fixture = Fixture::import(import_module).await;
    let host = Rc::new(Host::default());
    let view = localized("en");
    let cases = corpus();
    for case in cases.mime {
        let artifact = artifact(&case.filename, "Zg==", &host);
        saved(
            &fixture,
            &artifact,
            &view,
            (&case.filename, &case.mime, &[102]),
        )
        .await;
    }
    for case in cases.decode {
        let artifact = artifact("a.zip", &case.content, &host);
        match case.expected {
            Decoded::Success { bytes } => {
                saved(
                    &fixture,
                    &artifact,
                    &view,
                    ("a.zip", "application/zip", &bytes),
                )
                .await;
            }
            Decoded::Error => assert!(
                artifact.get_header_buttons(&view).is_err(),
                "rejected payload {:?} produced a header",
                case.content
            ),
        }
    }
    assert_host_silent(&host);
}
/// Dropping a mounted root releases only its own native resources.
#[wasm_bindgen]
pub async fn generic_artifact_dom_roots_release_their_own_resources(
    import_module: &js_sys::Function,
) {
    let fixture = Fixture::import(import_module).await;
    let host = Rc::new(Host::default());
    let view = localized("en");
    let detached = artifact("a.zip", "Zg==", &host);
    let root = detached.get_header_buttons(&view).unwrap();
    let control = button(&root);
    let row = root.node.clone();
    body().append_child(&row).unwrap();
    drop(root);
    fixture.begin("detached");
    control.click();
    wasm_bindgen_futures::JsFuture::from(fixture.finish_none())
        .await
        .unwrap();
    body().remove_child(&row).unwrap();
    let first = artifact("a.zip", "Zg==", &host);
    let second = artifact("b.bin", "Zm8=", &host);
    let first_root = first.get_header_buttons(&view).unwrap();
    let second_root = second.get_header_buttons(&view).unwrap();
    let control = button(&second_root);
    body().append_child(&second_root.node).unwrap();
    drop(first_root);
    fixture
        .saved(&control, &[102, 111], "application/octet-stream", "b.bin")
        .await;
    body().remove_child(&second_root.node).unwrap();
    assert_host_silent(&host);
}
