//! Browser observations of host-free artifact bodies and their independent controls.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, host, select, view};
use maestro_web::tools::artifacts::excel_artifact::ExcelArtifact;
use maestro_web::{
    dom::{DomContent, DomRoot, DomView},
    tools::artifacts::{ArtifactElement, TextArtifact},
};
use wasm_bindgen::{JsCast as _, JsValue, prelude::wasm_bindgen};

/// Source classification, literal text and equal updates preserve the mounted listing.
#[wasm_bindgen]
pub async fn text_artifact_dom_classifies_source_and_keeps_literal_text(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let mut artifact = TextArtifact::default();
    artifact.set_filename("sample.TS".into());
    artifact.set_content("const text = '<literal>雪';".into());
    let root = artifact.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    let code = select(&root.node, "code");
    assert!(code.class_name().contains("language-typescript"));
    assert_eq!(code.text_content().unwrap(), "const text = '<literal>雪';");
    let text = code.first_child().unwrap();
    artifact.update(&root, &view).unwrap();
    assert!(code.first_child().unwrap().is_same_node(Some(&text)));
    let header = artifact.get_header_buttons(&view).unwrap();
    body().append_child(&header.node).unwrap();
    assert_eq!(
        select(&header.node, "copy-button")
            .get_attribute("text")
            .as_deref(),
        Some(artifact.content())
    );
    let fixture = crate::session_download_browser::Fixture::import(&import).await;
    fixture
        .saved(
            &select(&header.node, "button[title='Download']").unchecked_into(),
            artifact.content().as_bytes(),
            "text/plain",
            "sample.TS",
        )
        .await;
    artifact.set_filename("sample.unknown".into());
    artifact.set_content("<img src=x onerror=alert(1)>".into());
    artifact.update(&root, &view).unwrap();
    assert!(select(&root.node, "code").is_same_node(Some(&code)));
    assert_eq!(
        code.text_content().as_deref(),
        Some("<img src=x onerror=alert(1)>")
    );
    assert!(code.query_selector("img").unwrap().is_none());
    root.node.unchecked_ref::<web_sys::Element>().remove();
    header.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Images keep literal metadata, recover from a failed source and download original bytes.
#[wasm_bindgen]
pub async fn image_artifact_dom_source_mime_and_fallback(import: js_sys::Function) {
    use maestro_web::ImageArtifact;
    let view = view();
    let _host = host(&view, &import);
    let mut artifact = ImageArtifact::default();
    artifact.set_filename("<literal>.GIF".into());
    artifact.set_content("AAH/".into());
    let root = artifact.mount(&view).unwrap();
    let image = select(&root.node, "img");
    assert_eq!(
        image.get_attribute("src").as_deref(),
        Some("data:image/gif;base64,AAH/")
    );
    assert_eq!(image.get_attribute("alt").as_deref(), Some("<literal>.GIF"));
    image
        .dispatch_event(&web_sys::Event::new("error").unwrap())
        .unwrap();
    assert!(image.get_attribute("src").unwrap().contains("Image Error"));
    artifact.update(&root, &view).unwrap();
    assert!(image.get_attribute("src").unwrap().contains("Image Error"));
    artifact.set_content(
        "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=".into(),
    );
    artifact.update(&root, &view).unwrap();
    assert!(select(&root.node, "img").is_same_node(Some(&image)));
    assert_eq!(
        image.get_attribute("src").as_deref(),
        Some("data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=")
    );
    let header = artifact.get_header_buttons(&view).unwrap();
    body().append_child(&header.node).unwrap();
    let fixture = crate::session_download_browser::Fixture::import(&import).await;
    fixture
        .saved(
            &select(&header.node, "button").unchecked_into(),
            br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#,
            "image/gif",
            "<literal>.GIF",
        )
        .await;
    header.node.unchecked_ref::<web_sys::Element>().remove();
    drop(root);
    image
        .dispatch_event(&web_sys::Event::new("error").unwrap())
        .unwrap();
    assert_eq!(
        image.get_attribute("src").as_deref(),
        Some("data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=")
    );
}

/// Markdown uses the shared document renderer and keeps source and preview roots across mode changes.
#[wasm_bindgen]
pub async fn markdown_artifact_dom_preview_uses_shared_math_and_links(import: js_sys::Function) {
    use maestro_web::tools::artifacts::MarkdownArtifact;
    let view = view();
    let _host = host(&view, &import);
    let mut artifact = MarkdownArtifact::default();
    artifact.set_filename("literal.md".into());
    artifact.set_content(
        "# Heading\n\n[link](https://example.com)\n\n$1+2$\n\n```rust\nlet x = 1;\n```".into(),
    );
    let root = artifact.mount(&view).unwrap();
    let header = artifact.get_header_buttons(&view).unwrap();
    body().append_child(&root.node).unwrap();
    body().append_child(&header.node).unwrap();
    assert_eq!(
        select(&root.node, "h1").text_content().as_deref(),
        Some("Heading")
    );
    assert_eq!(
        select(&root.node, "a").get_attribute("href").as_deref(),
        Some("https://example.com")
    );
    assert!(select(&root.node, ".katex").child_element_count() > 0);
    let source = select(&root.node, ".maestro-artifact-source");
    let preview = select(&root.node, ".maestro-artifact-preview");
    assert!(source.has_attribute("hidden"));
    select(&header.node, "button[title='Code']")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    assert!(!source.has_attribute("hidden"));
    assert!(preview.has_attribute("hidden"));
    assert_eq!(
        select(&source.clone().into(), "code")
            .text_content()
            .as_deref(),
        Some(artifact.content())
    );
    let heading = select(&preview.clone().into(), "h1");
    select(&header.node, "button[title='Preview']")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    artifact.update(&root, &view).unwrap();
    assert!(select(&preview.into(), "h1").is_same_node(Some(&heading)));
    assert_eq!(
        select(&header.node, "copy-button")
            .get_attribute("title")
            .as_deref(),
        Some("Copy Markdown")
    );
    let fixture = crate::session_download_browser::Fixture::import(&import).await;
    fixture
        .saved(
            &select(&header.node, "button[title='Download Markdown']").unchecked_into(),
            artifact.content().as_bytes(),
            "text/markdown",
            "literal.md",
        )
        .await;
    root.node.unchecked_ref::<web_sys::Element>().remove();
    header.node.unchecked_ref::<web_sys::Element>().remove();
}

/// SVG preview is an inert image resource, released on disconnect and restored on reconnect.
#[wasm_bindgen]
pub async fn svg_artifact_dom_urls_follow_edits_and_connection(import: js_sys::Function) {
    use maestro_web::tools::artifacts::SvgArtifact;
    let view = view();
    let _host = host(&view, &import);
    let mut artifact = SvgArtifact::default();
    artifact.set_filename("literal.svg".into());
    artifact.set_content("<svg xmlns='http://www.w3.org/2000/svg'><script>parent.svgExecuted=true</script><rect width='4' height='4'/></svg>".into());
    let root = artifact.mount(&view).unwrap();
    assert_eq!(root.node.node_name(), "SVG-ARTIFACT");
    body().append_child(&root.node).unwrap();
    let image = select(&root.node, "img");
    let first = image.get_attribute("src").unwrap();
    assert!(first.starts_with("blob:"));
    assert!(
        js_sys::Reflect::get(&web_sys::window().unwrap(), &"svgExecuted".into())
            .unwrap()
            .is_undefined()
    );
    artifact.update(&root, &view).unwrap();
    assert_eq!(image.get_attribute("src").as_deref(), Some(first.as_str()));
    root.node.unchecked_ref::<web_sys::Element>().remove();
    assert!(image.get_attribute("src").is_none());
    body().append_child(&root.node).unwrap();
    assert_ne!(image.get_attribute("src").as_deref(), Some(first.as_str()));
    let header = artifact.get_header_buttons(&view).unwrap();
    body().append_child(&header.node).unwrap();
    select(&header.node, "button[title='Code']")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    assert_eq!(
        select(&root.node, ".maestro-artifact-source code")
            .text_content()
            .as_deref(),
        Some(artifact.content())
    );
    assert!(select(&root.node, ".maestro-artifact-preview").has_attribute("hidden"));
    let fixture = crate::session_download_browser::Fixture::import(&import).await;
    fixture
        .saved(
            &select(&header.node, "button[title='Download SVG']").unchecked_into(),
            artifact.content().as_bytes(),
            "image/svg+xml",
            "literal.svg",
        )
        .await;
    root.node.unchecked_ref::<web_sys::Element>().remove();
    header.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Native workbook tabs retain their table roots, scroll and original download independently.
#[wasm_bindgen]
pub async fn excel_artifact_dom_tabs_keep_sheet_order_and_scroll(import: js_sys::Function) {
    use base64::Engine as _;
    use maestro_web::tools::artifacts::excel_artifact::ExcelArtifact;
    let view = view();
    let _host = host(&view, &import);
    let bytes = include_bytes!("fixtures/excel/scroll.xlsx");
    let mut artifact = ExcelArtifact::default();
    artifact.set_filename("book.xlsx".into());
    artifact.set_content(base64::engine::general_purpose::STANDARD.encode(bytes));
    let root = artifact.mount(&view).unwrap();
    let frame = view.document.create_element("div").unwrap();
    frame
        .set_attribute("style", "height:180px;width:260px")
        .unwrap();
    body().append_child(&frame).unwrap();
    frame.append_child(&root.node).unwrap();
    assert_workbook_tabs(&artifact, &root, &view);
    let header = artifact.get_header_buttons(&view).unwrap();
    body().append_child(&header.node).unwrap();
    let fixture = crate::session_download_browser::Fixture::import(&import).await;
    fixture
        .saved(
            &select(&header.node, "button").unchecked_into(),
            bytes,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "book.xlsx",
        )
        .await;
    assert_workbook_recovery(&mut artifact, &root, &view, bytes);
    frame.remove();
    header.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Workbook sheet activation keeps table identity, scroll and the current accent role.
fn assert_workbook_tabs(artifact: &ExcelArtifact, root: &DomRoot, view: &DomView) {
    assert!(
        root.node
            .unchecked_ref::<web_sys::Element>()
            .get_bounding_client_rect()
            .height()
            <= 180.5,
        "the native workbook must fit its supplied flow height"
    );
    let tabs = select(&root.node, ".maestro-excel-tabs");
    assert_eq!(tabs.text_content().as_deref(), Some("ZuluAlpha 雪"));
    let first: web_sys::HtmlElement = select(&root.node, ".maestro-excel-sheet").unchecked_into();
    let palette = view.pack.presentation(view.mode).unwrap();
    assert_eq!(
        crate::contributions_support::computed(&tabs.first_element_child().unwrap(), "color"),
        crate::contributions_support::rgb(palette.colors["accent"])
    );
    first.set_scroll_top(100);
    let offset = first.scroll_top();
    assert!(offset > 0);
    assert!(
        select(&root.node, "td").get_bounding_client_rect().top()
            >= first.get_bounding_client_rect().top() - 1.
    );
    tabs.last_element_child()
        .unwrap()
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    assert_eq!(first.style().get_property_value("display").unwrap(), "none");
    tabs.first_element_child()
        .unwrap()
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    artifact.update(root, view).unwrap();
    assert!(select(&root.node, ".maestro-excel-sheet").is_same_node(Some(&first)));
    assert_eq!(first.scroll_top(), offset);
}
/// Invalid and replacement workbooks recover through the shared reader and link policy.
fn assert_workbook_recovery(
    artifact: &mut ExcelArtifact,
    root: &DomRoot,
    view: &DomView,
    bytes: &[u8],
) {
    use base64::Engine as _;
    artifact.set_content("AA==".into());
    artifact.update(root, view).unwrap();
    assert!(
        root.node
            .text_content()
            .unwrap()
            .contains("Error loading spreadsheet")
    );
    artifact.set_content(base64::engine::general_purpose::STANDARD.encode(bytes));
    artifact.update(root, view).unwrap();
    assert!(
        select(&root.node, "table")
            .query_selector("td")
            .unwrap()
            .is_some()
    );
    let linked =
        include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/active-links.xlsx");
    artifact.set_content(base64::engine::general_purpose::STANDARD.encode(linked));
    artifact.update(root, view).unwrap();
    assert_eq!(
        select(&root.node, "a").get_attribute("href").as_deref(),
        Some("data:text/plain,hello")
    );
    assert!(
        root.node
            .unchecked_ref::<web_sys::Element>()
            .query_selector("a[href^='javascript:']")
            .unwrap()
            .is_none()
    );
}

/// PDF pages use the shared renderer, preserve completed roots and recover after an invalid document.
#[wasm_bindgen]
pub async fn pdf_artifact_dom_renders_all_scaled_pages(import: js_sys::Function) {
    use crate::contributions_support::{page_module, until};
    use base64::Engine as _;
    use maestro_web::tools::artifacts::pdf_artifact::PdfArtifact;
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let bytes = include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-multipage.pdf");
    let mut artifact = PdfArtifact::default();
    artifact.set_filename("pages.pdf".into());
    artifact.set_content(base64::engine::general_purpose::STANDARD.encode(bytes));
    let root = artifact.mount(&view).unwrap();
    assert_eq!(root.node.node_name(), "PDF-ARTIFACT");
    body().append_child(&root.node).unwrap();
    let element: &web_sys::Element = root.node.unchecked_ref();
    until(&page, &root.node, || {
        element.query_selector_all("img").unwrap().length() == 3
    })
    .await;
    assert_eq!(element.query_selector_all("hr").unwrap().length(), 2);
    let first = select(&root.node, "img");
    assert_eq!(first.get_attribute("width").as_deref(), Some("300"));
    assert_eq!(first.get_attribute("height").as_deref(), Some("150"));
    artifact.update(&root, &view).unwrap();
    assert!(select(&root.node, "img").is_same_node(Some(&first)));
    element.remove();
    body().append_child(&root.node).unwrap();
    assert!(select(&root.node, "img").is_same_node(Some(&first)));
    let header = artifact.get_header_buttons(&view).unwrap();
    body().append_child(&header.node).unwrap();
    let fixture = crate::session_download_browser::Fixture::import(&import).await;
    fixture
        .saved(
            &select(&header.node, "button").unchecked_into(),
            bytes,
            "application/pdf",
            "pages.pdf",
        )
        .await;
    artifact.set_content("AA==".into());
    artifact.update(&root, &view).unwrap();
    assert!(
        root.node
            .text_content()
            .unwrap()
            .contains("Error loading PDF")
    );
    artifact.set_content(base64::engine::general_purpose::STANDARD.encode(bytes));
    artifact.update(&root, &view).unwrap();
    until(&page, &root.node, || {
        element.query_selector_all("img").unwrap().length() == 3
    })
    .await;
    element.remove();
    header.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Bare source tags accept properties before connection and publish their real format bodies.
#[wasm_bindgen]
pub async fn artifact_tags_mount_their_owned_bodies(import: js_sys::Function) {
    use crate::contributions_support::page_module;
    use base64::Engine as _;
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let encoded = base64::engine::general_purpose::STANDARD.encode(include_bytes!(
        "../../maestro-app/tests/fixtures/pdf/synthetic-multipage.pdf"
    ));
    for (tag, content, selector) in [
        ("text-artifact", "<literal>", "code"),
        ("generic-artifact", "AAH/", "p"),
        (
            "image-artifact",
            "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=",
            "img",
        ),
        ("markdown-artifact", "# native", "h1"),
        (
            "svg-artifact",
            "<svg xmlns='http://www.w3.org/2000/svg'/>",
            "img",
        ),
        ("pdf-artifact", encoded.as_str(), "img"),
        ("excel-artifact", "AA==", ".maestro-file-error"),
        ("docx-artifact", "AA==", ".maestro-file-error"),
        ("html-artifact", "<p>source-tag</p>", "iframe"),
    ] {
        assert_source_tag_fields(&view, &page, tag, content, selector).await;
    }
    assert!(
        !web_sys::window()
            .unwrap()
            .custom_elements()
            .get("artifact-console")
            .is_undefined()
    );
    let console = view.document.create_element("artifact-console").unwrap();
    let logs = js_sys::JSON::parse(r#"[{"type":"error","text":"<literal>"}]"#).unwrap();
    js_sys::Reflect::set(&console, &"logs".into(), &logs).unwrap();
    body().append_child(&console).unwrap();
    assert!(console.text_content().unwrap().contains("1 error"));
    console.remove();
}

/// Native source properties are declared accessors and reach the connected format body.
async fn assert_source_tag_fields(
    view: &DomView,
    page: &JsValue,
    tag: &str,
    content: &str,
    selector: &str,
) {
    use crate::contributions_support::until;
    assert!(
        !web_sys::window()
            .unwrap()
            .custom_elements()
            .get(tag)
            .is_undefined(),
        "{tag} must be registered"
    );
    let node = view.document.create_element(tag).unwrap();
    let descriptor = js_sys::Object::get_own_property_descriptor(
        node.unchecked_ref::<js_sys::Object>(),
        &wasm_bindgen::JsValue::from_str("content"),
    );
    assert!(
        js_sys::Reflect::get(&descriptor, &"set".into())
            .unwrap()
            .is_function(),
        "{tag} requires a source setter, not an expando"
    );
    js_sys::Reflect::set(&node, &"filename".into(), &"literal.name".into()).unwrap();
    js_sys::Reflect::set(&node, &"content".into(), &content.into()).unwrap();
    assert!(!node.has_attribute("content"));
    assert_eq!(
        js_sys::Reflect::get(&node, &"content".into())
            .unwrap()
            .as_string()
            .as_deref(),
        Some(content)
    );
    body().append_child(&node).unwrap();
    until(page, &node.clone().into(), || {
        node.query_selector(selector).unwrap().is_some()
    })
    .await;
    assert_source_tag_presentation(tag, &node);
    node.remove();
}
/// Format-specific native presentation uses the literal source and filename without replacing resources.
fn assert_source_tag_presentation(tag: &str, node: &web_sys::Element) {
    if matches!(
        tag,
        "generic-artifact" | "image-artifact" | "pdf-artifact" | "excel-artifact" | "docx-artifact"
    ) {
        assert_eq!(
            crate::contributions_support::computed(node, "display"),
            "block",
            "{tag} must own its connected block presentation"
        );
    }
    if tag == "svg-artifact" {
        let image = select(&node.clone().into(), "img");
        let url = image.get_attribute("src");
        js_sys::Reflect::set(node, &"filename".into(), &"renamed.svg".into()).unwrap();
        assert_eq!(image.get_attribute("alt").as_deref(), Some("renamed.svg"));
        assert_eq!(
            image.get_attribute("src"),
            url,
            "a filename edit must not replace SVG bytes"
        );
    }
    if tag == "html-artifact" {
        assert!(
            select(&node.clone().into(), "iframe")
                .get_attribute("srcdoc")
                .unwrap()
                .contains("source-tag")
        );
    }
    if tag == "text-artifact" {
        assert_eq!(
            select(&node.clone().into(), "code")
                .text_content()
                .as_deref(),
            Some("<literal>")
        );
    }
}

/// The native panel consumes application commands while retaining one live view per literal filename.
#[wasm_bindgen]
pub async fn artifacts_panel_dom_retains_views_through_application_commands(
    import: js_sys::Function,
) {
    use maestro_web::ArtifactsPanel;
    let view = view();
    let _host = host(&view, &import);
    let panel = ArtifactsPanel::default();
    let caller = panel.application(&view);
    let root = panel.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    for (filename, content) in [("literal\"[x].txt", "first"), ("notes.md", "# notes")] {
        (caller.tool().execute)(
            "create".into(),
            serde_json::json!({"command":"create", "filename":filename, "content":content}),
            None,
            None,
        )
        .await
        .unwrap();
    }
    assert_eq!(
        select(&root.node, ".maestro-artifact-tabs").child_element_count(),
        2
    );
    let text = select(&root.node, "text-artifact");
    assert!(text.has_attribute("hidden"));
    let tab = select(&root.node, ".maestro-artifact-tabs button");
    assert_eq!(tab.get_attribute("aria-pressed").as_deref(), Some("false"));
    tab.unchecked_ref::<web_sys::HtmlElement>().click();
    assert_eq!(tab.get_attribute("aria-pressed").as_deref(), Some("true"));
    assert_eq!(
        caller.active_filename().as_deref(),
        Some("literal\"[x].txt")
    );
    assert!(!text.has_attribute("hidden"));
    (caller.tool().execute)("rewrite".into(), serde_json::json!({"command":"rewrite", "filename":"literal\"[x].txt", "content":"second <literal>"}), None, None).await.unwrap();
    assert!(select(&root.node, "text-artifact").is_same_node(Some(&text)));
    assert_eq!(
        select(&text.clone().into(), "code")
            .text_content()
            .as_deref(),
        Some("second <literal>")
    );
    assert_panel_close_and_reconnect(&panel, &root, &text);
    assert_panel_removal_and_root_release(&panel, root, &caller, &view).await;
}

/// Deleted files and released roots stay absent after later commands or native reconnection.
async fn assert_panel_removal_and_root_release(
    panel: &maestro_web::ArtifactsPanel,
    root: DomRoot,
    caller: &maestro_app::browser::tools::artifacts::LocalArtifactsPanel,
    view: &DomView,
) {
    (caller.tool().execute)(
        "delete".into(),
        serde_json::json!({"command":"delete", "filename":"literal\"[x].txt"}),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(
        root.node
            .unchecked_ref::<web_sys::Element>()
            .query_selector("text-artifact")
            .unwrap()
            .is_none()
    );
    assert_eq!(caller.active_filename().as_deref(), Some("notes.md"));
    let released: web_sys::Element = root.node.clone().unchecked_into();
    drop(root);
    assert_eq!(released.child_element_count(), 0);
    (caller.tool().execute)("rewrite".into(), serde_json::json!({"command":"rewrite", "filename":"notes.md", "content":"# after release"}), None, None).await.unwrap();
    assert_eq!(
        released.child_element_count(),
        0,
        "commands cannot repaint a root whose owner was dropped"
    );
    released.remove();
    body().append_child(&released).unwrap();
    assert_eq!(
        released.child_element_count(),
        0,
        "reconnection cannot revive a released typed root"
    );
    let fresh = panel.mount(view).unwrap();
    body().append_child(&fresh.node).unwrap();
    assert_eq!(
        select(&fresh.node, "markdown-artifact h1")
            .text_content()
            .as_deref(),
        Some("after release")
    );
    released.remove();
    assert!(select(&fresh.node, "markdown-artifact").is_connected());
    fresh.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Collapse, close and reconnection keep the selected native element and notify only once.
fn assert_panel_close_and_reconnect(
    panel: &maestro_web::ArtifactsPanel,
    root: &DomRoot,
    text: &web_sys::Element,
) {
    use std::{cell::Cell, rc::Rc};
    panel.set_collapsed(true);
    assert!(
        root.node
            .unchecked_ref::<web_sys::Element>()
            .has_attribute("hidden")
    );
    panel.set_collapsed(false);
    let closed = Rc::new(Cell::new(0));
    let count = Rc::clone(&closed);
    panel.set_on_close(Some(Rc::new(move || count.set(count.get() + 1))));
    select(&root.node, "button[title='Close artifacts']")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    assert_eq!(closed.get(), 1);
    root.node.unchecked_ref::<web_sys::Element>().remove();
    body().append_child(&root.node).unwrap();
    assert!(select(&root.node, "text-artifact").is_same_node(Some(text)));
}

/// Independent source bodies own bounded native scrolling and preserve focus and selection on equal updates.
#[wasm_bindgen]
pub async fn artifact_bodies_dom_keep_native_scroll_selection_and_focus(import: js_sys::Function) {
    use crate::contributions_support::{call, computed, page_module, rgb, selection};
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let mut artifact = TextArtifact::default();
    artifact.set_filename("source.txt".into());
    artifact.set_content("literal line 雪\n".repeat(80));
    let wrapper = view.document.create_element("div").unwrap();
    wrapper
        .set_attribute("style", "height:180px;width:260px")
        .unwrap();
    body().append_child(&wrapper).unwrap();
    let root = artifact.mount(&view).unwrap();
    wrapper.append_child(&root.node).unwrap();
    let viewport: &web_sys::HtmlElement = root.node.unchecked_ref();
    assert!(
        viewport.get_bounding_client_rect().height() <= 180.5,
        "the native body must fit its supplied flow height"
    );
    viewport.set_scroll_top(60);
    let offset = viewport.scroll_top();
    assert!(offset > 0, "the independent body must own its scrolling");
    let header = artifact.get_header_buttons(&view).unwrap();
    body().append_child(&header.node).unwrap();
    let button: web_sys::HtmlElement =
        select(&header.node, "button[title='Download']").unchecked_into();
    button.focus().unwrap();
    assert!(button.get_bounding_client_rect().width() > 0.);
    let code = select(&root.node, "code");
    call(&page, "select", &js_sys::Array::of1(&code)).await;
    let selected = selection(&page, &code).await;
    artifact.update(&root, &view).unwrap();
    assert_eq!(selection(&page, &code).await, selected);
    assert_eq!(viewport.scroll_top(), offset);
    assert!(
        view.document
            .active_element()
            .unwrap()
            .is_same_node(Some(&button))
    );
    for mode in [
        maestro_theme::BrandMode::Dark,
        maestro_theme::BrandMode::Light,
    ] {
        let palette = view.pack.presentation(mode).unwrap();
        wrapper
            .unchecked_ref::<web_sys::HtmlElement>()
            .style()
            .set_property("--maestro-color-text", palette.colors["text"])
            .unwrap();
        assert_eq!(computed(&code, "color"), rgb(palette.colors["text"]));
    }
    wrapper.remove();
    header.node.unchecked_ref::<web_sys::Element>().remove();
}

/// A panel reload publishes current sandbox operands to the existing HTML toolbar as well as its frame.
#[wasm_bindgen]
pub async fn artifacts_panel_dom_reload_uses_current_sandbox_configuration(
    import: js_sys::Function,
) {
    use maestro_app::SharedHandle;
    use maestro_web::ArtifactsPanel;
    use std::{cell::Cell, rc::Rc};
    let view = view();
    let _host = host(&view, &import);
    let panel = ArtifactsPanel::default();
    let old = Rc::new(Cell::new(0));
    let current = Rc::new(Cell::new(0));
    let count = Rc::clone(&old);
    panel.set_sandbox_url_provider(Some(SharedHandle::new(move || {
        count.set(count.get() + 1);
        Ok(format!(
            "{}/sandbox.html",
            crate::contributions_support::origin()
        ))
    })));
    let caller = panel.application(&view);
    let root = panel.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    (caller.tool().execute)("html".into(), serde_json::json!({"command":"create", "filename":"page.html", "content":"<p>original</p>"}), None, None).await.unwrap();
    assert!(old.get() > 0);
    let count = Rc::clone(&current);
    panel.set_sandbox_url_provider(Some(SharedHandle::new(move || {
        count.set(count.get() + 1);
        Ok(format!(
            "{}/sandbox.html",
            crate::contributions_support::origin()
        ))
    })));
    (caller.tool().execute)(
        "text".into(),
        serde_json::json!({"command":"create", "filename":"data.txt", "content":"new"}),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(current.get() > 0);
    caller.show_artifact("page.html");
    let before_old = old.get();
    let before_current = current.get();
    select(&root.node, "button[title='Reload HTML']")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    assert_eq!(
        old.get(),
        before_old,
        "a retained toolbar must not reinstall the previous sandbox address provider"
    );
    assert_eq!(current.get(), before_current + 1);
    root.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Every panel-created source element exposes the literal fields of its current native controller.
#[wasm_bindgen]
pub async fn artifacts_panel_dom_source_properties_follow_their_controller(
    import: js_sys::Function,
) {
    use maestro_web::ArtifactsPanel;
    use std::{cell::Cell, rc::Rc};
    let view = view();
    let _host = host(&view, &import);
    let panel = ArtifactsPanel::default();
    let opened = Rc::new(Cell::new(0));
    let count = Rc::clone(&opened);
    panel.set_on_open(Some(Rc::new(move || count.set(count.get() + 1))));
    let caller = panel.application(&view);
    let root = panel.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    for (tag, name, source) in [
        ("text-artifact", "literal.txt", "<literal>"),
        ("generic-artifact", "literal.zip", "AA=="),
        (
            "image-artifact",
            "literal.png",
            "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=",
        ),
        ("markdown-artifact", "literal.md", "# literal"),
        (
            "svg-artifact",
            "literal.svg",
            "<svg xmlns='http://www.w3.org/2000/svg'/>",
        ),
        ("pdf-artifact", "literal.pdf", "AA=="),
        ("excel-artifact", "literal.xlsx", "AA=="),
        ("docx-artifact", "literal.docx", "AA=="),
        ("html-artifact", "literal.html", "<p>literal</p>"),
    ] {
        let args = serde_json::json!({"command":"create", "filename":name, "content":source});
        let params: maestro_web::ArtifactsParams = serde_json::from_value(args.clone()).unwrap();
        assert_eq!(params.filename, name);
        (caller.tool().execute)("create".into(), args, None, None)
            .await
            .unwrap();
        assert_panel_source_record(&caller, &root, tag, name, source);
        if matches!(tag, "html-artifact" | "svg-artifact" | "pdf-artifact") {
            assert_panel_property_edit(&panel, &caller, &root, (name, tag), &view);
        }
    }
    caller.open_artifact("literal.txt");
    assert_eq!(opened.get(), 1);
    root.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Native property edits survive panel refreshes and reach SDK lookup and new headers.
fn assert_panel_property_edit(
    panel: &maestro_web::ArtifactsPanel,
    caller: &maestro_app::browser::tools::artifacts::LocalArtifactsPanel,
    root: &DomRoot,
    (name, tag): (&str, &str),
    view: &DomView,
) {
    let edited = match tag {
        "html-artifact" => "<p>edited</p>",
        "svg-artifact" => "<svg xmlns='http://www.w3.org/2000/svg'><title>edited</title></svg>",
        _ => "AQ==",
    };
    let element = select(&root.node, tag);
    js_sys::Reflect::set(&element, &"content".into(), &edited.into()).unwrap();
    let live = panel.get_native_artifact_element(name).unwrap();
    assert_eq!(live.borrow().content(), edited, "{tag} SDK source");
    caller.show_artifact("literal.txt");
    caller.show_artifact(name);
    assert!(select(&root.node, tag).is_same_node(Some(&element)));
    assert_eq!(
        js_sys::Reflect::get(&element, &"content".into())
            .unwrap()
            .as_string()
            .as_deref(),
        Some(edited),
        "{tag} source after tab changes"
    );
    let header = live.borrow().get_header_buttons(view).unwrap();
    if tag != "pdf-artifact" {
        assert_eq!(
            select(&header.node, "copy-button")
                .get_attribute("text")
                .as_deref(),
            Some(edited)
        );
    }
}

/// Native element properties and application records retain the same source bytes.
fn assert_panel_source_record(
    caller: &maestro_app::browser::tools::artifacts::LocalArtifactsPanel,
    root: &DomRoot,
    tag: &str,
    name: &str,
    source: &str,
) {
    let element = select(&root.node, tag);
    assert_eq!(
        js_sys::Reflect::get(&element, &"filename".into())
            .unwrap()
            .as_string()
            .as_deref(),
        Some(name),
        "{tag} must expose its actual filename"
    );
    assert_eq!(
        js_sys::Reflect::get(&element, &"content".into())
            .unwrap()
            .as_string()
            .as_deref(),
        Some(source)
    );
    let records = caller.artifacts();
    let records = records.read().unwrap();
    let record = records[name].read().unwrap();
    let record: &maestro_web::Artifact = &record;
    assert_eq!(record.content, source);
}

/// Correcting an invalid original payload clears its panel diagnostic and restores its controls.
#[wasm_bindgen]
pub async fn artifacts_panel_dom_errors_clear_after_a_successful_rewrite(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let panel = maestro_web::ArtifactsPanel::default();
    let caller = panel.application(&view);
    let root = panel.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    (caller.tool().execute)(
        "bad".into(),
        serde_json::json!({"command":"create", "filename":"file.zip", "content":"bad!!!"}),
        None,
        None,
    )
    .await
    .unwrap();
    let diagnostic = select(&root.node, ".maestro-artifact-panel-error");
    assert!(!diagnostic.text_content().unwrap().is_empty());
    (caller.tool().execute)(
        "fixed".into(),
        serde_json::json!({"command":"rewrite", "filename":"file.zip", "content":"AA=="}),
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        diagnostic.text_content().as_deref(),
        Some(""),
        "a successful rewrite must not retain an obsolete payload error"
    );
    assert!(
        select(&root.node, "button[title='Download']")
            .get_bounding_client_rect()
            .width()
            > 0.
    );
    assert_html_rewrite_recovery(&panel, &caller, &root, &import).await;
    root.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Failed source execution still invalidates cached copy and download operands.
async fn assert_html_rewrite_recovery(
    panel: &maestro_web::ArtifactsPanel,
    caller: &maestro_app::browser::tools::artifacts::LocalArtifactsPanel,
    root: &DomRoot,
    import: &js_sys::Function,
) {
    use maestro_app::{DiagnosticErrorInfo, SharedHandle};
    for (command, source, fail) in [
        ("create", "<p>before</p>", false),
        ("rewrite", "<p>after</p>", true),
        ("rewrite", "<p>after</p>", false),
    ] {
        panel.set_sandbox_url_provider(if fail {
            Some(SharedHandle::new(|| {
                Err(DiagnosticErrorInfo::error("address failed"))
            }))
        } else {
            None
        });
        (caller.tool().execute)(
            "write".into(),
            serde_json::json!({"command":command, "filename":"page.html", "content":source}),
            None,
            None,
        )
        .await
        .unwrap();
    }
    assert_eq!(
        panel
            .get_native_artifact_element("page.html")
            .unwrap()
            .borrow()
            .content(),
        "<p>after</p>"
    );
    let header = select(&root.node, ".maestro-artifact-active-header");
    assert_eq!(
        select(&header.clone().into(), "copy-button")
            .get_attribute("text")
            .as_deref(),
        Some("<p>after</p>")
    );
    let expected = standalone_panel_html(panel);
    let fixture = crate::session_download_browser::Fixture::import(import).await;
    fixture
        .saved(
            &select(&header.into(), "button[title='Download HTML']").unchecked_into(),
            expected.as_bytes(),
            "text/html",
            "page.html",
        )
        .await;
}

/// Prepare the current HTML source through its live sandbox for byte-exact download checks.
fn standalone_panel_html(panel: &maestro_web::ArtifactsPanel) -> String {
    let live = panel.get_native_artifact_element("page.html").unwrap();
    let artifact = live.borrow();
    let maestro_web::NativeArtifact::Html(html) = &*artifact else {
        unreachable!()
    };
    html.sandbox_iframe_ref
        .borrow()
        .as_ref()
        .unwrap()
        .prepare_html_document(
            "artifact-page.html",
            "<p>after</p>",
            &html.runtime_providers,
            Some(
                maestro_web::components::sandboxed_iframe::PrepareHtmlOptions {
                    is_html_artifact: true,
                    is_standalone: true,
                },
            ),
        )
        .unwrap()
}

/// Nested panels retain their own localization operands and do not consume another panel's tab events.
#[wasm_bindgen]
pub async fn artifacts_panel_dom_instances_keep_their_own_operands(import: js_sys::Function) {
    use crate::contributions_support::localized_view;
    let english = view();
    let _host = host(&english, &import);
    let german = localized_view(&english, "de");
    let outer = maestro_web::ArtifactsPanel::default();
    let inner = maestro_web::ArtifactsPanel::default();
    let outer_app = outer.application(&english);
    let inner_app = inner.application(&german);
    let outer_root = outer.mount(&english).unwrap();
    let inner_root = inner.mount(&german).unwrap();
    body().append_child(&outer_root.node).unwrap();
    outer_root.node.append_child(&inner_root.node).unwrap();
    for (caller, filename) in [(&outer_app, "outer.zip"), (&inner_app, "inner.zip")] {
        (caller.tool().execute)(
            "create".into(),
            serde_json::json!({"command":"create", "filename":filename, "content":"AA=="}),
            None,
            None,
        )
        .await
        .unwrap();
    }
    assert!(
        inner_root
            .node
            .text_content()
            .unwrap()
            .contains("Vorschau für diesen Dateityp nicht verfügbar."),
        "a new source body must use its creating panel's localization, not the registry's first view"
    );
    inner.update(&inner_root, &english).unwrap();
    assert_eq!(
        select(&inner_root.node, "button[data-close-artifacts]")
            .get_attribute("title")
            .as_deref(),
        Some("Close artifacts")
    );
    let generic = select(&inner_root.node, "generic-artifact");
    js_sys::Reflect::set(&generic, &"filename".into(), &"renamed.zip".into()).unwrap();
    assert!(
        generic
            .text_content()
            .unwrap()
            .contains("Preview not available for this file type."),
        "source property updates must use the latest bound page operands"
    );
    select(&inner_root.node, ".maestro-artifact-tabs button")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    assert_eq!(inner_app.active_filename().as_deref(), Some("inner.zip"));
    assert_eq!(
        outer_app.active_filename().as_deref(),
        Some("outer.zip"),
        "a nested panel's tab must not select a filename in its ancestor"
    );
    outer_root.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Switching placed panel instances switches the application owner rather than repainting the old session's files.
#[wasm_bindgen]
pub async fn artifacts_panel_dom_placements_switch_the_application_owner(import: js_sys::Function) {
    use maestro_web::dom::MountedContent;
    let view = view();
    let _host = host(&view, &import);
    let first = maestro_web::ArtifactsPanel::default();
    let second = maestro_web::ArtifactsPanel::default();
    let first_app = first.application(&view);
    let second_app = second.application(&view);
    for (caller, name, source) in [
        (&first_app, "first.txt", "first session"),
        (&second_app, "second.txt", "second session"),
    ] {
        (caller.tool().execute)(
            "create".into(),
            serde_json::json!({"command":"create", "filename":name, "content":source}),
            None,
            None,
        )
        .await
        .unwrap();
    }
    let parent = view.document.create_element("div").unwrap();
    body().append_child(&parent).unwrap();
    let mut placed = MountedContent::default();
    placed.show(&first, parent.as_ref(), &view).unwrap();
    assert_eq!(
        select(placed.root().unwrap(), "code")
            .text_content()
            .as_deref(),
        Some("first session")
    );
    placed.show(&second, parent.as_ref(), &view).unwrap();
    assert_eq!(
        select(placed.root().unwrap(), "code")
            .text_content()
            .as_deref(),
        Some("second session")
    );
    (first_app.tool().execute)("rewrite".into(), serde_json::json!({"command":"rewrite", "filename":"first.txt", "content":"old session changed"}), None, None).await.unwrap();
    assert_eq!(
        select(placed.root().unwrap(), "code")
            .text_content()
            .as_deref(),
        Some("second session")
    );
    placed.clear();
    parent.remove();
}

/// A retained header selects the currently bound document controller, not a discarded controller's mode.
#[wasm_bindgen]
pub fn artifact_dom_mode_changes_follow_the_bound_controller(import: &js_sys::Function) {
    use maestro_web::tools::artifacts::{MarkdownArtifact, SvgArtifact};
    let view = view();
    let _host = host(&view, import);
    for header_first in [false, true] {
        let mut first = MarkdownArtifact::default();
        first.set_filename("first.md".into());
        first.set_content("# first".into());
        let early_header = header_first.then(|| first.get_header_buttons(&view).unwrap());
        let root = first.mount(&view).unwrap();
        let header = early_header.unwrap_or_else(|| first.get_header_buttons(&view).unwrap());
        assert_rebound_modes::<MarkdownArtifact>(&root, &header, &view);
        drop(root);
        assert_remounted_modes(&first, &header, &view);

        let mut first = SvgArtifact::default();
        first.set_filename("first.svg".into());
        first.set_content("<svg xmlns=\"http://www.w3.org/2000/svg\"/>".into());
        let early_header = header_first.then(|| first.get_header_buttons(&view).unwrap());
        let root = first.mount(&view).unwrap();
        let header = early_header.unwrap_or_else(|| first.get_header_buttons(&view).unwrap());
        assert_rebound_modes::<SvgArtifact>(&root, &header, &view);
        drop(root);
        assert_remounted_modes(&first, &header, &view);
    }
}

/// Headers made before or after their body follow both preview and code selections.
fn assert_rebound_modes<T: ArtifactElement + DomContent + Default>(
    root: &DomRoot,
    header: &DomRoot,
    view: &DomView,
) {
    body().append_child(&root.node).unwrap();
    body().append_child(&header.node).unwrap();
    let mut current = T::default();
    current.set_filename("current.md".into());
    current.set_content("# current".into());
    current.update(root, view).unwrap();
    select(&header.node, "button[title='Code']")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    let source = select(&root.node, ".maestro-artifact-source");
    assert!(!source.has_attribute("hidden"));
    assert_eq!(
        select(&source.clone().into(), "code")
            .text_content()
            .as_deref(),
        Some("# current")
    );
    current.update(root, view).unwrap();
    assert!(
        !source.has_attribute("hidden"),
        "an unchanged update must preserve the current controller's user selection"
    );
    let mut rebound = T::default();
    rebound.set_content("# rebound".into());
    rebound.update(root, view).unwrap();
    assert!(source.has_attribute("hidden"));
    assert!(
        !select(&header.node, "button[title='Code']")
            .class_name()
            .contains("maestro-toggle-selected")
    );
    select(&header.node, "button[title='Code']")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    assert!(
        !source.has_attribute("hidden"),
        "a formerly selected Code button must select the rebound controller"
    );
    rebound.update(root, view).unwrap();
    assert!(!source.has_attribute("hidden"));
    root.node.unchecked_ref::<web_sys::Element>().remove();
    header.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Reusing a header after releasing its rebound body restores the original controller's selection.
fn assert_remounted_modes(first: &dyn DomContent, header: &DomRoot, view: &DomView) {
    let root = first.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    body().append_child(&header.node).unwrap();
    let source = select(&root.node, ".maestro-artifact-source");
    assert!(source.has_attribute("hidden"));
    let code = select(&header.node, "button[title='Code']");
    assert!(
        !code.class_name().contains("maestro-toggle-selected"),
        "a retained header must show the remounted controller's preview selection"
    );
    code.unchecked_into::<web_sys::HtmlElement>().click();
    assert!(
        !source.has_attribute("hidden"),
        "a retained Code button must select the remounted controller"
    );
    root.node.unchecked_ref::<web_sys::Element>().remove();
    header.node.unchecked_ref::<web_sys::Element>().remove();
}

/// SDK lookup returns the live native owner whose independent header controls the already-mounted body.
#[wasm_bindgen]
pub async fn artifacts_panel_dom_exposes_the_live_native_element(import: js_sys::Function) {
    use maestro_web::NativeArtifact;
    use std::rc::Rc;
    let view = view();
    let _host = host(&view, &import);
    let panel = maestro_web::ArtifactsPanel::default();
    assert!(panel.get_native_artifact_element("notes.md").is_none());
    let caller = panel.application(&view);
    (caller.tool().execute)(
        "create".into(),
        serde_json::json!({"command":"create", "filename":"notes.md", "content":"# original"}),
        None,
        None,
    )
    .await
    .unwrap();
    let root = panel.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    let live = panel.get_native_artifact_element("notes.md").unwrap();
    assert!(matches!(&*live.borrow(), NativeArtifact::Markdown(_)));
    assert_eq!(live.borrow().filename(), "notes.md");
    let header = live.borrow().get_header_buttons(&view).unwrap();
    body().append_child(&header.node).unwrap();
    select(&header.node, "button[title='Code']")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
    let source = select(&root.node, ".maestro-artifact-source");
    assert!(!source.has_attribute("hidden"));
    (caller.tool().execute)(
        "rewrite".into(),
        serde_json::json!({"command":"rewrite", "filename":"notes.md", "content":"# updated"}),
        None,
        None,
    )
    .await
    .unwrap();
    let updated = panel.get_native_artifact_element("notes.md").unwrap();
    assert!(Rc::ptr_eq(&live, &updated));
    assert_eq!(updated.borrow().content(), "# updated");
    assert!(!source.has_attribute("hidden"));
    assert_eq!(
        select(&source.into(), "code").text_content().as_deref(),
        Some("# updated")
    );
    (caller.tool().execute)(
        "delete".into(),
        serde_json::json!({"command":"delete", "filename":"notes.md"}),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(panel.get_native_artifact_element("notes.md").is_none());
    root.node.unchecked_ref::<web_sys::Element>().remove();
    header.node.unchecked_ref::<web_sys::Element>().remove();
}
