//! Word artifact bodies observed through the native document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, select, view};
use crate::word_view_browser::fetch_bytes;
use maestro_web::{
    page::{DomContent, DomRoot, DomView},
    tools::documents::docx_artifact::DocxArtifact,
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
    /// Whether the rendering declares embedded font faces.
    has_fonts: bool,
    /// Embedded family whose loaded face paints the document text.
    loaded_font: Option<String>,
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
/// Read all nineteen document packages.
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
    use crate::contributions_support::host;
    let view = view();
    let _host = host(&view, import_module);
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
            let mut scrolled = 0;
            for case in packages() {
                scrolled += assert_page_geometry(&view, &viewport, &case, width).await;
            }
            assert!(
                width > 240 || scrolled > 0,
                "a wide table scrolls in the narrow page"
            );
            viewport.remove();
        }
    }
}

/// Observe one mounted package in the supplied bounded viewport and count its scrolled tables.
async fn assert_page_geometry(
    view: &DomView,
    viewport: &Element,
    case: &Package,
    width: i32,
) -> u32 {
    let root = artifact(&case.content).mount(view).unwrap();
    viewport.append_child(&root.node).unwrap();
    let node: &Element = root.node.unchecked_ref();
    let sheet = select(node, "section.docx");
    if case.text.iter().any(|text| !text.is_empty()) {
        crate::support::reading_paint::observe("word-page").await;
    }
    assert!(
        (sheet.get_bounding_client_rect().width() - f64::from(width)).abs() < 1.0,
        "{}",
        case.name
    );
    assert_painted_page(&sheet, case);
    let scrolled = assert_contained_content(node, &sheet, (case, width)).await;
    assert_loaded_font(node, case).await;
    let text = crate::contributions_support::page::select_children(&sheet);
    if case.text.iter().any(|text| !text.is_empty()) {
        assert!(!text.is_empty(), "{} selectable text", case.name);
    }
    viewport.remove_child(&root.node).unwrap();
    scrolled
}

/// The page paints its own padding around measured body text.
///
/// A header or footer paragraph sits at the document's header offset, which the artifact's own
/// page padding replaces, so the padding surrounds the body text block rather than them.
fn assert_painted_page(sheet: &Element, case: &Package) {
    let page = sheet.get_bounding_client_rect();
    assert!(page.height() > 0.0, "{} paints its page", case.name);
    let text = sheet.query_selector("article p").unwrap();
    assert!(text.is_some(), "{} paints its body paragraph", case.name);
    let painted = text.unwrap().get_bounding_client_rect();
    assert!(
        painted.width() > 0.0 && painted.height() > 0.0,
        "{} paints its text",
        case.name
    );
    assert!(
        painted.left() - page.left() >= 30.0 && painted.top() - page.top() >= 30.0,
        "{}: the page paints its own padding around the body text",
        case.name
    );
}

/// Wrapped text, ready images and wide tables stay inside their painted page; returns the
/// number of tables that scrolled.
async fn assert_contained_content(
    node: &Element,
    sheet: &Element,
    (case, width): (&Package, i32),
) -> u32 {
    let page_width = sheet.get_bounding_client_rect().width();
    assert_wrapped_text(node, (page_width, case));
    assert_ready_images(node, (page_width, case)).await;
    assert_table_scrolling(node, (page_width, case, width))
}

/// Paragraphs and spans outside tables stay inside their painted page instead of overflowing
/// it; table text sits inside its table's own horizontal scroll box.
/// Inline spans report zero widths, so the observation is the paragraph's bounding box.
fn assert_wrapped_text(node: &Element, (page_width, case): (f64, &Package)) {
    for selector in ["p:not(table p)", "span:not(table span)"] {
        let painted = node.query_selector_all(selector).unwrap();
        for index in 0..painted.length() {
            let element: HtmlElement = painted.item(index).unwrap().unchecked_into();
            assert!(
                element.get_bounding_client_rect().width() <= page_width + 1.0,
                "{}: {selector} stays inside its page",
                case.name
            );
        }
    }
}

/// Every inline image's own resource is decoded and paints at a nonzero size inside its page.
async fn assert_ready_images(node: &Element, (page_width, case): (f64, &Package)) {
    if case.images == 0 {
        return;
    }
    let images = node.query_selector_all("img").unwrap();
    for index in 0..images.length() {
        let image: Element = images.item(index).unwrap().unchecked_into();
        let decode: js_sys::Function = js_sys::Reflect::get(&image, &"decode".into())
            .unwrap()
            .unchecked_into();
        let decoded = js_sys::Promise::from(decode.call0(&image).unwrap());
        let decoded = wasm_bindgen_futures::JsFuture::from(decoded).await;
        assert!(
            decoded.is_ok(),
            "{}: the image decodes ({decoded:?})",
            case.name
        );
        let complete = js_sys::Reflect::get(&image, &"complete".into()).unwrap();
        let width = js_sys::Reflect::get(&image, &"naturalWidth".into()).unwrap();
        assert!(
            complete.as_bool() == Some(true) && width.as_f64().unwrap_or(0.0) > 0.0,
            "{}: the image resource is ready",
            case.name
        );
        let painted = image.get_bounding_client_rect();
        assert!(
            painted.width() > 0.0 && painted.height() > 0.0,
            "{}: the image paints at a nonzero size",
            case.name
        );
        assert!(
            painted.width() <= page_width + 1.0,
            "{}: the image stays inside its page",
            case.name
        );
    }
}

/// A table stays inside its page; in the narrow page, a table whose grid is wider than its box
/// scrolls that grid horizontally. Returns the number of tables that scrolled.
///
/// Authored grid column widths are preferred widths, so a grid of short cells shrinks into the
/// narrow page; only cells that cannot wrap make a grid wider than its box.
fn assert_table_scrolling(node: &Element, (page_width, case, width): (f64, &Package, i32)) -> u32 {
    let mut scrolled = 0;
    if case.tables == 0 {
        return scrolled;
    }
    let tables = node.query_selector_all("table").unwrap();
    for index in 0..tables.length() {
        let table: HtmlElement = tables.item(index).unwrap().unchecked_into();
        assert!(
            table.get_bounding_client_rect().width() <= page_width + 1.0,
            "{}: the table stays inside its page",
            case.name
        );
        if width <= 240 && table.scroll_width() > table.client_width() {
            table.set_scroll_left(table.scroll_width());
            assert!(
                table.scroll_left() > 0,
                "{}: the wide table scrolls horizontally",
                case.name
            );
            scrolled += 1;
        }
    }
    scrolled
}

/// The document's embedded face is a loaded resource that changes the measured width of the
/// text it paints, so a face that fails to load or a run that falls back cannot pass.
async fn assert_loaded_font(node: &Element, case: &Package) {
    let Some(family) = &case.loaded_font else {
        return;
    };
    let fonts = js_sys::Reflect::get(&crate::contributions_support::document(), &"fonts".into());
    let faces: Vec<_> = js_sys::Array::from(&fonts.unwrap())
        .iter()
        .filter(|face| {
            js_sys::Reflect::get(face, &"family".into())
                .unwrap()
                .as_string()
                .is_some_and(|name| name.trim_matches(['"', '\'']) == family)
        })
        .collect();
    assert!(!faces.is_empty(), "{}: the face is declared", case.name);
    for face in faces {
        let load: js_sys::Function = js_sys::Reflect::get(&face, &"load".into())
            .unwrap()
            .unchecked_into();
        let loaded = js_sys::Promise::from(load.call0(&face).unwrap());
        let loaded = wasm_bindgen_futures::JsFuture::from(loaded).await;
        assert!(loaded.is_ok(), "{}: the face loads ({loaded:?})", case.name);
    }
    let run: HtmlElement = select(node, "article span").unchecked_into();
    let painted = run.get_bounding_client_rect().width();
    let fallback: HtmlElement = run.clone_node_with_deep(true).unwrap().unchecked_into();
    fallback
        .style()
        .set_property("font-family", "maestro-absent-family")
        .unwrap();
    run.after_with_node_1(&fallback).unwrap();
    let unloaded = fallback.get_bounding_client_rect().width();
    fallback.remove();
    assert!(
        (painted - unloaded).abs() > 1.0,
        "{}: the loaded face changes the text width ({painted} and {unloaded})",
        case.name
    );
}

/// Decode and render failures remain independent of the original-byte header.
#[wasm_bindgen]
pub async fn docx_artifact_dom_errors_retry_and_keep_download(import_module: &js_sys::Function) {
    use crate::contributions_support::host;
    use crate::native_simple_artifacts_support::corpus;
    use crate::session_download_browser::Fixture;
    let view = view();
    let _host = host(&view, import_module);
    let fixture = Fixture::default();
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
            fixture.finish_none();
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
    use crate::contributions_support::host;
    let view = view();
    let _host = host(&view, import_module);
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
    let (sheet, url) = assert_preserved_pages(&mut artifact, &root, &source.content, &view).await;
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
    assert_cancelled_shapes(&view).await;
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
    source: &str,
    view: &DomView,
) -> (Element, String) {
    let node: &Element = root.node.unchecked_ref();
    let sheet = select(node, "section.docx");
    let url = select(node, "img").get_attribute("src").unwrap();
    let selected = crate::contributions_support::page::select_children(&sheet);
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
        assert_eq!(
            crate::contributions_support::page::selection(&sheet),
            (selected.clone(), true)
        );
        assert!(!fetch_bytes(&url).await.is_empty());
    }
    (sheet, url)
}

/// Dropping one document cancels its scheduled shape sizing while its sibling still completes.
async fn assert_cancelled_shapes(view: &maestro_web::page::DomView) {
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
    frames(2).await;
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
