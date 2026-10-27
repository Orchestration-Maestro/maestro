//! Word artifact bodies observed through the native document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, select, view};
use crate::docx_preview_browser::fetch_bytes;
use maestro_web::{
    dom::{DomContent, DomRoot, DomView},
    tools::artifacts::docx_artifact::DocxArtifact,
};
use serde::Deserialize;
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

/// One recorded document package and its visible content.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Package {
    /// Descriptive package name.
    pub(crate) name: String,
    /// Complete original bytes, encoded for the artifact setter.
    pub(crate) content: String,
    /// Ordered rendered page count.
    pages: u32,
    /// Visible page text, in page order.
    text: Vec<String>,
    /// Ordered text and authored destination of each link.
    links: Vec<Link>,
    /// Inline image count.
    images: u32,
    /// Table count.
    tables: u32,
    /// Math element count.
    math: u32,
    /// Whether embedded fonts are loaded.
    has_fonts: bool,
}
/// One link observed in the unmodified document.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    /// Literal link caption.
    text: String,
    /// Safe authored destination, absent for refused targets.
    href: Option<String>,
}
/// Read all seventeen document packages.
pub(crate) fn packages() -> Vec<Package> {
    serde_json::from_str(include_str!("fixtures/native_document_artifacts.json")).unwrap()
}
/// Apply source text to a fresh artifact.
fn artifact(content: &str) -> DocxArtifact {
    let mut artifact = DocxArtifact::new();
    artifact.set_content(content.to_owned());
    artifact
}
/// Real packages exercise all document options rather than inspecting a selector record.
#[wasm_bindgen]
pub fn docx_artifact_dom_uses_full_document_options(import_module: &js_sys::Function) {
    let view = view();
    let _host = crate::contributions_support::host(&view, import_module);
    let empty = DocxArtifact::default();
    assert_eq!(empty.filename, "");
    assert_eq!(empty.content(), "");
    assert!(
        empty
            .mount(&view)
            .unwrap()
            .node
            .text_content()
            .unwrap()
            .is_empty()
    );
    for case in packages() {
        let root = artifact(&case.content).mount(&view).unwrap();
        body().append_child(&root.node).unwrap();
        let node: &Element = root.node.unchecked_ref();
        assert_document(node, &case);
        body().remove_child(&root.node).unwrap();
    }
}

/// Native pages keep their document pixels, layout and safe authored links in either brand mode.
#[wasm_bindgen]
pub async fn docx_artifact_dom_styles_and_safe_targets(import_module: &js_sys::Function) {
    use crate::contributions_support::{host, page_module};
    let view = view();
    let _host = host(&view, import_module);
    let page = page_module(import_module).await;
    for mode in [
        maestro_theme::BrandMode::Dark,
        maestro_theme::BrandMode::Light,
    ] {
        for width in [240, 960] {
            let viewport = view.document.create_element("div").unwrap();
            viewport
                .set_attribute("style", &format!("width:{width}px;height:180px"))
                .unwrap();
            let presentation = view.pack.presentation(mode).unwrap();
            for role in ["background", "text", "error", "panel"] {
                viewport
                    .unchecked_ref::<HtmlElement>()
                    .style()
                    .set_property(
                        &format!("--maestro-color-{role}"),
                        presentation.colors[role],
                    )
                    .unwrap();
            }
            body().append_child(&viewport).unwrap();
            for case in packages() {
                assert_page_geometry(&view, &viewport, &case, &page, width).await;
            }
            viewport.remove();
        }
    }
}

/// Observe one mounted package in the supplied bounded viewport.
async fn assert_page_geometry(
    view: &DomView,
    viewport: &Element,
    case: &Package,
    page: &wasm_bindgen::JsValue,
    width: i32,
) {
    use crate::contributions_support::{call, computed};
    let root = artifact(&case.content).mount(view).unwrap();
    viewport.append_child(&root.node).unwrap();
    let node: &Element = root.node.unchecked_ref();
    let sheet = select(node, "section.docx");
    assert_eq!(computed(&sheet, "background-color"), "rgb(255, 255, 255)");
    assert_eq!(computed(&sheet, "color"), "rgb(0, 0, 0)");
    assert_eq!(computed(&sheet, "padding-top"), "32px");
    assert_eq!(computed(&sheet, "box-shadow"), "none");
    assert!(
        (sheet.get_bounding_client_rect().width() - f64::from(width)).abs() < 1.0,
        "{}",
        case.name
    );
    assert_eq!(computed(&sheet, "overflow-x"), "auto");
    assert_contained_content(node, &sheet, case);
    let text = call(page, "select", &js_sys::Array::of1(&sheet))
        .await
        .as_string()
        .unwrap();
    if case.text.iter().any(|text| !text.is_empty()) {
        assert!(!text.is_empty(), "{} selectable text", case.name);
    }
    viewport.remove_child(&root.node).unwrap();
}

/// Wrapping text, wide tables and inline images stay within their document page.
fn assert_contained_content(node: &Element, sheet: &Element, case: &Package) {
    use crate::contributions_support::computed;
    for selector in ["p", "span"] {
        if let Some(text) = node.query_selector(selector).unwrap() {
            assert_eq!(computed(&text, "overflow-wrap"), "break-word");
        }
    }
    if case.images > 0 {
        let image = select(node, "img");
        assert!(
            image.get_bounding_client_rect().width() <= sheet.get_bounding_client_rect().width()
        );
    }
    if case.tables > 0 {
        assert_eq!(computed(&select(node, "table"), "display"), "block");
        assert_eq!(computed(&select(node, "table"), "overflow-x"), "auto");
    }
}

/// Decode and render failures remain independent of the original-byte header.
#[wasm_bindgen]
pub async fn docx_artifact_dom_errors_retry_and_keep_download(import_module: &js_sys::Function) {
    use crate::contributions_support::host;
    use crate::native_simple_artifacts_support::corpus;
    use crate::session_download_browser::Fixture;
    let view = view();
    let _host = host(&view, import_module);
    let fixture = Fixture::import(import_module).await;
    for case in corpus().decode {
        let mut artifact = artifact(&case.content);
        artifact.filename = "literal$&雪.docx".to_owned();
        let root = artifact.mount(&view).unwrap();
        assert_download(&artifact, &view, &fixture, &case.expected).await;
        if !case.content.is_empty() {
            assert_failed_retry(&mut artifact, &root, &view);
        }
    }
    let failed = artifact("Zg==");
    let bad = failed.mount(&view).unwrap();
    for (language, heading, title) in [
        ("en", "Error loading document", "Download"),
        ("de", "Fehler beim Laden des Dokuments", "Herunterladen"),
    ] {
        view.i18n.set_language(language).unwrap();
        failed.update(&bad, &view).unwrap();
        assert_eq!(
            select(bad.node.unchecked_ref(), "h3")
                .text_content()
                .as_deref(),
            Some(heading)
        );
        let header = artifact("Zg==").get_header_buttons(&view).unwrap();
        assert_eq!(
            select(header.node.unchecked_ref(), "button")
                .get_attribute("title")
                .as_deref(),
            Some(title)
        );
    }
}

/// Verify the independent header's decoded bytes or its refusal without native effects.
async fn assert_download(
    artifact: &DocxArtifact,
    view: &DomView,
    fixture: &crate::session_download_browser::Fixture,
    expected: &crate::native_simple_artifacts_support::Decoded,
) {
    use crate::native_simple_artifacts_support::Decoded;
    match expected {
        Decoded::Success { bytes } => {
            let header = artifact.get_header_buttons(view).unwrap();
            body().append_child(&header.node).unwrap();
            let control: HtmlElement =
                select(header.node.unchecked_ref(), "button").unchecked_into();
            fixture
                .saved(
                    &control,
                    bytes,
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                    &artifact.filename,
                )
                .await;
            body().remove_child(&header.node).unwrap();
        }
        Decoded::Error => {
            fixture.begin("success");
            assert!(artifact.get_header_buttons(view).is_err());
            wasm_bindgen_futures::JsFuture::from(fixture.finish_none())
                .await
                .unwrap();
        }
    }
}

/// Equal failure retries keep the diagnostic, and empty/recovery assignments replace it.
fn assert_failed_retry(artifact: &mut DocxArtifact, root: &DomRoot, view: &DomView) {
    let node: &Element = root.node.unchecked_ref();
    assert_eq!(
        select(node, "h3").text_content().as_deref(),
        Some("Error loading document")
    );
    let previous = select(node, "p");
    let diagnostic = previous.text_content().unwrap();
    assert!(!diagnostic.is_empty());
    artifact.set_content(artifact.content().to_owned());
    artifact.update(root, view).unwrap();
    let retried = select(node, "p");
    assert!(
        !retried.is_same_node(Some(&previous)),
        "equal failed assignment retries"
    );
    assert_eq!(retried.text_content().as_deref(), Some(diagnostic.as_str()));
    artifact.set_content(String::new());
    artifact.update(root, view).unwrap();
    assert!(node.text_content().unwrap().is_empty());
    artifact.set_content(packages().remove(0).content);
    artifact.update(root, view).unwrap();
    assert_eq!(node.query_selector_all("section.docx").unwrap().length(), 1);
}

/// Successful equal/empty updates preserve pages; replacement and drop release only owned resources.
#[wasm_bindgen]
pub async fn docx_artifact_dom_retains_and_releases_resources(import_module: &js_sys::Function) {
    use crate::contributions_support::{host, page_module};
    let view = view();
    let _host = host(&view, import_module);
    let page = page_module(import_module).await;
    let cases = packages();
    let source = cases
        .iter()
        .find(|case| case.name == "image-inline")
        .unwrap();
    let mut artifact = artifact(&source.content);
    let root = artifact.mount(&view).unwrap();
    let sibling = artifact.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    body().append_child(&sibling.node).unwrap();
    let node: &Element = root.node.unchecked_ref();
    node.set_attribute("style", "width:240px;height:80px")
        .unwrap();
    let sibling_url = select(sibling.node.unchecked_ref(), "img")
        .get_attribute("src")
        .unwrap();
    let (sheet, url) =
        assert_preserved_pages(&mut artifact, &root, &page, &source.content, &view).await;
    artifact.set_content("!".to_owned());
    artifact.update(&root, &view).unwrap();
    assert!(node.query_selector("section.docx").unwrap().is_none());
    assert!(fetch_bytes(&url).await.is_empty());
    artifact.set_content(String::new());
    artifact.update(&root, &view).unwrap();
    assert!(node.text_content().unwrap().is_empty());
    artifact.set_content(source.content.clone());
    artifact.update(&root, &view).unwrap();
    assert!(!select(node, "section.docx").is_same_node(Some(&sheet)));
    let recovered = select(node, "img").get_attribute("src").unwrap();
    drop(artifact);
    assert!(!fetch_bytes(&recovered).await.is_empty());
    assert_released_documents(root, sibling, &recovered, &sibling_url).await;
    assert_cancelled_shapes(&view, &page).await;
}

/// Releasing each document revokes only its own live image URL.
async fn assert_released_documents(
    root: DomRoot,
    sibling: DomRoot,
    recovered: &str,
    sibling_url: &str,
) {
    body().remove_child(&root.node).unwrap();
    drop(root);
    assert!(fetch_bytes(recovered).await.is_empty());
    assert!(!fetch_bytes(sibling_url).await.is_empty());
    body().remove_child(&sibling.node).unwrap();
    drop(sibling);
    assert!(fetch_bytes(sibling_url).await.is_empty());
}

/// Equal, renamed and empty-success updates retain selection, scroll, nodes and live image bytes.
async fn assert_preserved_pages(
    artifact: &mut DocxArtifact,
    root: &DomRoot,
    page: &wasm_bindgen::JsValue,
    source: &str,
    view: &DomView,
) -> (Element, String) {
    use crate::contributions_support::{call, selection};
    let node: &Element = root.node.unchecked_ref();
    let sheet = select(node, "section.docx");
    let url = select(node, "img").get_attribute("src").unwrap();
    let selected = call(page, "select", &js_sys::Array::of1(&sheet))
        .await
        .as_string()
        .unwrap();
    node.unchecked_ref::<HtmlElement>().set_scroll_top(10);
    let scroll = node.unchecked_ref::<HtmlElement>().scroll_top();
    assert!(scroll > 0);
    for operation in ["equal", "rename", "empty-success"] {
        match operation {
            "equal" => artifact.set_content(source.to_owned()),
            "rename" => artifact.filename = "later.docx".to_owned(),
            _ => artifact.set_content(String::new()),
        }
        artifact.update(root, view).unwrap();
        assert!(select(node, "section.docx").is_same_node(Some(&sheet)));
        assert_eq!(node.unchecked_ref::<HtmlElement>().scroll_top(), scroll);
        assert_eq!(selection(page, &sheet).await, (selected.clone(), true));
        assert!(!fetch_bytes(&url).await.is_empty());
    }
    (sheet, url)
}

/// Dropping one document cancels its scheduled shape sizing while its sibling still completes.
async fn assert_cancelled_shapes(view: &maestro_web::dom::DomView, page: &wasm_bindgen::JsValue) {
    use crate::contributions_support::frames;
    let cases = packages();
    let shapes = cases.iter().find(|case| case.name == "vml-shapes").unwrap();
    let cancelled = artifact(&shapes.content).mount(view).unwrap();
    let live = artifact(&shapes.content).mount(view).unwrap();
    body().append_child(&cancelled.node).unwrap();
    body().append_child(&live.node).unwrap();
    let stopped = select(cancelled.node.unchecked_ref(), "svg");
    let running = select(live.node.unchecked_ref(), "svg");
    let before = stopped.get_attribute("width");
    drop(cancelled);
    frames(page, 2).await;
    assert_eq!(stopped.get_attribute("width"), before);
    assert_ne!(running.get_attribute("width"), before);
    stopped
        .closest(".maestro-docx-artifact")
        .unwrap()
        .unwrap()
        .remove();
    body().remove_child(&live.node).unwrap();
}

/// Observe the same rendered-document contract through each public native consumer.
pub(crate) fn assert_document(node: &Element, case: &Package) {
    let pages = node.query_selector_all("section.docx").unwrap();
    assert_eq!(pages.length(), case.pages, "{}", case.name);
    let text: Vec<String> = (0..pages.length())
        .map(|index| {
            let page: HtmlElement = pages.item(index).unwrap().unchecked_into();
            page.inner_text()
        })
        .collect();
    assert_eq!(text, case.text, "{}", case.name);
    for (selector, count) in [
        ("img", case.images),
        ("table", case.tables),
        ("math", case.math),
    ] {
        assert_eq!(
            node.query_selector_all(selector).unwrap().length(),
            count,
            "{} {selector}",
            case.name
        );
    }
    let links = node.query_selector_all("a").unwrap();
    assert_eq!(usize::try_from(links.length()).unwrap(), case.links.len());
    for (index, expected) in case.links.iter().enumerate() {
        let link: Element = links
            .item(u32::try_from(index).unwrap())
            .unwrap()
            .unchecked_into();
        assert_eq!(link.text_content().as_deref(), Some(expected.text.as_str()));
        assert_eq!(link.get_attribute("href"), expected.href);
    }
    assert_eq!(
        node.text_content().unwrap().contains("@font-face"),
        case.has_fonts,
        "{}",
        case.name
    );
}
