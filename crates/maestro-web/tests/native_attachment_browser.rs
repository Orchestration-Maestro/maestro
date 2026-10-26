//! Native attachment controls exercised through their registered page elements.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, host, select, text, view};
use js_sys::{Array, Function, Reflect};
use maestro_web::dom::DomContent as _;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::{CustomEvent, Element, HtmlElement};
mod overlay;

/// Assign an unreflected element operand.
fn set(element: &Element, name: &str, value: &JsValue) {
    assert!(Reflect::set(element, &name.into(), value).unwrap());
}
/// Read the selected index from the actual element.
fn selected(element: &Element) -> JsValue {
    Reflect::get(element, &"selectedIndex".into()).unwrap()
}
/// Replace authored mode labels.
fn labels(element: &Element, names: &[&str]) {
    let values = names
        .iter()
        .map(|name| JsValue::from_str(name))
        .collect::<Array>();
    set(element, "modes", &values);
}
/// Click one native button.
fn click(element: &Element, index: u32) {
    element
        .query_selector_all("button")
        .unwrap()
        .item(index)
        .unwrap()
        .unchecked_into::<HtmlElement>()
        .click();
}
/// Fewer than two labels leave no visible controls or empty group chrome.
fn assert_empty(element: &Element) {
    assert_eq!(element.query_selector_all("button").unwrap().length(), 0);
    assert!(
        element
            .first_element_child()
            .unwrap()
            .get_bounding_client_rect()
            .width()
            .abs()
            < f64::EPSILON,
        "an empty mode group has no visible chrome"
    );
}
/// Authored label cardinality, literal text and unchanged button identity.
fn assert_labels(element: &Element) {
    labels(element, &[]);
    assert_empty(element);
    labels(element, &["Only"]);
    assert_empty(element);
    labels(element, &["File", "Text", "<&雪>"]);
    let buttons = element.query_selector_all("button").unwrap();
    assert_eq!(
        buttons.length(),
        3,
        "the registered element renders authored modes"
    );
    assert_eq!(buttons.item(2).unwrap().text_content().unwrap(), "<&雪>");
    set(element, "selectedIndex", &2.into());
    assert!(
        buttons
            .item(2)
            .unwrap()
            .unchecked_into::<Element>()
            .class_list()
            .contains("maestro-toggle-selected")
    );
    labels(element, &["File", "Text", "<&雪>"]);
    assert!(
        buttons.item(0).unwrap().is_same_node(
            element
                .query_selector("button")
                .unwrap()
                .as_ref()
                .map(AsRef::as_ref)
        )
    );
}
/// Record one changed selection and select a different mode during delivery.
fn recorder(
    element: &Element,
    seen: &Rc<RefCell<Vec<(f64, String)>>>,
) -> Closure<dyn Fn(CustomEvent)> {
    let (events, target) = (Rc::clone(seen), element.clone());
    Closure::<dyn Fn(CustomEvent)>::new(move |event: CustomEvent| {
        assert!(event.bubbles());
        assert_eq!(selected(&target), JsValue::from(1));
        let detail = event.detail();
        events.borrow_mut().push((
            Reflect::get(&detail, &"index".into())
                .unwrap()
                .as_f64()
                .unwrap(),
            Reflect::get(&detail, &"mode".into())
                .unwrap()
                .as_string()
                .unwrap(),
        ));
        set(&target, "selectedIndex", &0.into());
    })
}
/// Default selection and a nonzero selection retained through reconnection.
fn mounted(view: &maestro_web::dom::DomView) -> Element {
    let element = view.document.create_element("mode-toggle").unwrap();
    assert_eq!(selected(&element), JsValue::from(0));
    body().append_child(&element).unwrap();
    assert_eq!(element.query_selector_all("button").unwrap().length(), 2);
    element.remove();
    labels(&element, &["File", "Text", "Third"]);
    set(&element, "selectedIndex", &2.into());
    body().append_child(&element).unwrap();
    assert!(
        element
            .query_selector_all("button")
            .unwrap()
            .item(2)
            .unwrap()
            .unchecked_into::<Element>()
            .class_list()
            .contains("maestro-toggle-selected")
    );
    element
}
/// Native labels, selection validation and reentrant event delivery share one control owner.
#[wasm_bindgen]
pub fn native_mode_group_updates_labels_selection_and_reentrant_events(import_module: &Function) {
    let view = view();
    let _host = host(&view, import_module);
    let element = mounted(&view);
    assert_labels(&element);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let listener = recorder(&element, &seen);
    element
        .add_event_listener_with_callback("mode-change", listener.as_ref().unchecked_ref())
        .unwrap();
    let choose = Reflect::get(&element, &"setMode".into())
        .unwrap()
        .unchecked_into::<Function>();
    for rejected in [-1, 3] {
        choose.call1(&element, &rejected.into()).unwrap();
        assert_eq!(selected(&element), JsValue::from(2));
    }
    click(&element, 2);
    assert!(seen.borrow().is_empty());
    click(&element, 1);
    assert_eq!(&*seen.borrow(), &[(1., "Text".to_owned())]);
    assert_eq!(selected(&element), JsValue::from(0));
    assert!(
        element
            .query_selector("button")
            .unwrap()
            .unwrap()
            .class_list()
            .contains("maestro-toggle-selected")
    );
    element
        .remove_event_listener_with_callback("mode-change", listener.as_ref().unchecked_ref())
        .unwrap();
    labels(&element, &["Replaced", "Second"]);
    assert_eq!(element.query_selector_all("button").unwrap().length(), 2);
    assert_eq!(
        element
            .query_selector("button")
            .unwrap()
            .unwrap()
            .text_content()
            .unwrap(),
        "Replaced"
    );
    element.remove();
}

/// Shared document factories retain progressive pages and the current worksheet across frames.
#[wasm_bindgen]
pub fn shared_document_roots_preserve_progress_and_sheet_selection() {
    let document = crate::contributions_support::document();
    let canvas = document.create_element("canvas").unwrap();
    canvas.set_attribute("id", "canvas").unwrap();
    canvas.set_attribute("width", "800").unwrap();
    canvas.set_attribute("height", "600").unwrap();
    body().append_child(&canvas).unwrap();
    shared_pdf_pages(&document);
    shared_sheet_selection(&document);
    canvas.remove();
}
/// Existing artifact factories reuse the native page root while raster work progresses.
fn shared_pdf_pages(document: &web_sys::Document) {
    use crate::{pdf_views_browser, reading_browser_support::View};
    let mut pdf = View::new("shared.pdf", &pdf_views_browser::encoded(true));
    pdf.draw(false, vec![]);
    let root = document.query_selector(".maestro-pdf").unwrap().unwrap();
    let first = root.query_selector("img").unwrap().unwrap();
    assert_eq!(root.query_selector_all("img").unwrap().length(), 1);
    for count in 2..=3 {
        pdf.draw(false, vec![]);
        assert!(
            root.is_same_node(
                document
                    .query_selector(".maestro-pdf")
                    .unwrap()
                    .as_ref()
                    .map(AsRef::as_ref)
            )
        );
        assert!(
            first.is_same_node(
                root.query_selector("img")
                    .unwrap()
                    .as_ref()
                    .map(AsRef::as_ref)
            )
        );
        assert_eq!(root.query_selector_all("img").unwrap().length(), count);
    }
    drop(pdf);
}
/// The current selected sheet and both native bodies survive another frame.
fn shared_sheet_selection(document: &web_sys::Document) {
    use crate::{
        excel_views_browser,
        reading_browser_support::{View, click},
    };
    let mut excel = View::new("shared.xlsx", &excel_views_browser::encoded("two"));
    excel.draw(false, vec![]);
    let root = document.query_selector(".maestro-excel").unwrap().unwrap();
    let sheets = root.query_selector_all(".maestro-excel-sheet").unwrap();
    assert_eq!(sheets.length(), 2);
    let first = sheets.item(0).unwrap().unchecked_into::<Element>();
    let second = sheets.item(1).unwrap().unchecked_into::<Element>();
    assert_eq!(first.get_attribute("style").unwrap(), "display:block;");
    assert_eq!(second.get_attribute("style").unwrap(), "display:none;");
    let tab = root
        .query_selector_all(".maestro-excel-tab")
        .unwrap()
        .item(1)
        .unwrap()
        .unchecked_into::<Element>()
        .get_bounding_client_rect();
    let position = egui::pos2(
        num_traits::cast(tab.x() + tab.width() / 2.).unwrap(),
        num_traits::cast(tab.y() + tab.height() / 2.).unwrap(),
    );
    excel.draw(false, click(position));
    excel.draw(false, vec![]);
    assert!(
        root.is_same_node(
            document
                .query_selector(".maestro-excel")
                .unwrap()
                .as_ref()
                .map(AsRef::as_ref)
        )
    );
    assert!(
        first.is_same_node(
            root.query_selector(".maestro-excel-sheet")
                .unwrap()
                .as_ref()
                .map(AsRef::as_ref)
        )
    );
    assert_eq!(first.get_attribute("style").unwrap(), "display:none;");
    assert_eq!(second.get_attribute("style").unwrap(), "display:block;");
    drop(excel);
}

/// Open one overlay and observe its actual already-appended native root.
fn opened(
    view: &Rc<maestro_web::dom::DomView>,
    record: maestro_web::SharedAttachment,
) -> (maestro_web::AttachmentOverlay, Element) {
    let overlay = maestro_web::AttachmentOverlay::open(view, record, None).unwrap();
    let mounted = overlay.mount(view).unwrap();
    let element: Element = mounted.node.unchecked_into();
    assert!(element.is_connected());
    (overlay, element)
}
/// Activate a named native control.
fn control(root: &Element, selector: &str) {
    root.query_selector(selector)
        .unwrap()
        .unwrap()
        .unchecked_into::<HtmlElement>()
        .click();
}
/// Close the actual element through its visible control.
fn close(root: &Element) {
    control(root, "[data-close]");
    assert!(!root.is_connected());
}
/// Supply valid existing format fixtures without changing classification operands.
fn valid_document(record: &maestro_web::SharedAttachment, kind: &str) {
    use base64::Engine as _;
    let content = match kind {
        "pdf" => Some(crate::pdf_views_browser::encoded(true)),
        "excel" => Some(crate::excel_views_browser::encoded("two")),
        "docx" => Some(
            base64::engine::general_purpose::STANDARD.encode(include_bytes!(
                "../src/docx_preview/fixtures/docx/wrapper-default.docx"
            )),
        ),
        _ => None,
    };
    if let Some(content) = content {
        record.write().unwrap().content = content;
    }
}
/// Observe a content branch through its rendered native body, not a classifier hook.
fn assert_kind(root: &Element, kind: &str, name: &str) {
    let selector = match kind {
        "pdf" => ".maestro-pdf",
        "excel" => ".maestro-excel",
        "docx" => ".maestro-docx-container",
        "pptx" => ".maestro-attachment-presentation",
        "image" => ".maestro-attachment-image",
        "text" => "pre",
        _ => panic!("unknown fixture kind"),
    };
    assert!(
        root.query_selector(selector).unwrap().is_some(),
        "{name}: {kind}"
    );
}
/// All classification and localized mode fixtures select the original first-match branch.
fn classification_and_labels(view: &Rc<maestro_web::dom::DomView>) {
    use crate::attachment_overlay::{ToggleCase, TypeCase};
    let types: Vec<TypeCase> =
        serde_json::from_str(include_str!("fixtures/attachment_types.json")).unwrap();
    assert_eq!(types.len(), 16);
    for case in types {
        let record = case.input.record();
        valid_document(&record, &case.expected.r#type);
        let (_, root) = opened(view, record);
        assert_kind(&root, &case.expected.r#type, &case.name);
        close(&root);
    }
    let toggles: Vec<ToggleCase> =
        serde_json::from_str(include_str!("fixtures/attachment_toggles.json")).unwrap();
    assert_eq!(toggles.len(), 48);
    for case in toggles {
        let localized = crate::contributions_support::localized_view(view, &case.language);
        let (_, root) = opened(&localized, case.input.record());
        let buttons = root.query_selector_all(".maestro-toggle button").unwrap();
        let labels = (0..buttons.length())
            .map(|i| buttons.item(i).unwrap().text_content().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(labels, case.expected.modes, "{}", case.name);
        let selected = (0..buttons.length()).find(|&i| {
            buttons
                .item(i)
                .unwrap()
                .unchecked_into::<Element>()
                .class_list()
                .contains("maestro-toggle-selected")
        });
        assert_eq!(selected.map(|i| i as usize), case.expected.selected);
        close(&root);
    }
}
/// Content cases preserve literal text, original image bytes and extracted-text sizing.
fn content_cases(view: &Rc<maestro_web::dom::DomView>) {
    let cases = crate::attachment_overlay::content_cases();
    assert_eq!(cases.len(), 26);
    for case in cases {
        let record = case.input.record();
        valid_document(&record, &case.expected.kind);
        let filename = record.read().unwrap().file_name.clone();
        let (_, root) = opened(view, record);
        if case.show_text && case.expected.small {
            control(&root, ".maestro-toggle button:nth-child(2)");
        }
        assert_kind(&root, &case.expected.kind, &case.name);
        if let Some(text) = case.expected.text {
            let pre = root.query_selector("pre").unwrap().unwrap();
            assert_eq!(pre.text_content().unwrap(), text, "{}", case.name);
            assert_eq!(
                root.query_selector(".maestro-attachment-extracted")
                    .unwrap()
                    .is_some(),
                case.expected.small
            );
        }
        if let Some(source) = case.expected.source {
            let image = root.query_selector("img").unwrap().unwrap();
            assert_eq!(image.get_attribute("src").unwrap(), source);
            assert_eq!(image.get_attribute("alt").unwrap(), filename);
            assert_eq!(
                web_sys::window()
                    .unwrap()
                    .get_computed_style(&image)
                    .unwrap()
                    .unwrap()
                    .get_property_value("object-fit")
                    .unwrap(),
                "contain"
            );
        }
        close(&root);
    }
}
/// A visible file failure stays selected until the existing mode control changes it.
fn error_modes(view: &Rc<maestro_web::dom::DomView>) {
    let record = crate::attachment_support::attachment();
    {
        let mut a = record.write().unwrap();
        a.mime_type = "application/pdf".into();
        a.content = "invalid!".into();
        a.extracted_text = Some("<literal text>".into());
    }
    let (overlay, root) = opened(view, std::sync::Arc::clone(&record));
    assert!(
        root.query_selector(".maestro-file-error")
            .unwrap()
            .is_some()
    );
    assert!(root.query_selector("pre").unwrap().is_none());
    control(&root, ".maestro-toggle button:nth-child(2)");
    assert_eq!(
        root.query_selector("pre")
            .unwrap()
            .unwrap()
            .text_content()
            .unwrap(),
        "<literal text>"
    );
    assert!(
        root.query_selector(".maestro-file-error")
            .unwrap()
            .is_none()
    );
    control(&root, ".maestro-toggle button:nth-child(1)");
    let error = root.query_selector(".maestro-file-error").unwrap().unwrap();
    overlay::preserve_error(&overlay, view, &root, &record);
    record.write().unwrap().content = crate::pdf_views_browser::encoded(true);
    let mounted = overlay.mount(view).unwrap();
    overlay.update(&mounted, view).unwrap();
    assert!(
        error.is_same_node(
            root.query_selector(".maestro-file-error")
                .unwrap()
                .as_ref()
                .map(AsRef::as_ref)
        )
    );
    control(&root, ".maestro-toggle button:nth-child(2)");
    control(&root, ".maestro-toggle button:nth-child(1)");
    assert!(
        root.query_selector(".maestro-file-error")
            .unwrap()
            .is_none()
    );
    assert!(root.query_selector(".maestro-pdf").unwrap().is_some());
    close(&root);
}
/// Fixed chrome, classification priority, modes and literal localized fallbacks.
#[wasm_bindgen]
pub async fn overlay_dom_fixed_chrome_modes_text_and_locales() {
    let view = view();
    classification_and_labels(&view);
    content_cases(&view);
    overlay::live_text(&view);
    overlay::live_modes(&view);
    overlay::borrow_failure(&view);
    let unset = view.document.create_element("attachment-overlay").unwrap();
    body().append_child(&unset).unwrap();
    assert!(unset.first_element_child().is_none());
    unset.remove();
    error_modes(&view);
    let (_, root) = opened(&view, crate::attachment_support::attachment());
    let window = web_sys::window().unwrap();
    let backdrop = root.first_element_child().unwrap();
    let css = window.get_computed_style(&backdrop).unwrap().unwrap();
    assert_eq!(css.get_property_value("position").unwrap(), "fixed");
    assert_eq!(css.get_property_value("flex-direction").unwrap(), "column");
    let header = root.query_selector("header").unwrap().unwrap();
    let body = root
        .query_selector(".maestro-attachment-body")
        .unwrap()
        .unwrap();
    assert_eq!(
        window
            .get_computed_style(&header)
            .unwrap()
            .unwrap()
            .get_property_value("flex-shrink")
            .unwrap(),
        "0"
    );
    assert_eq!(
        window
            .get_computed_style(&body)
            .unwrap()
            .unwrap()
            .get_property_value("overflow-y")
            .unwrap(),
        "auto"
    );
    overlay::chrome_layout(&root);
    close(&root);
    control_names(&view).await;
}
/// The icon-only download and close controls carry the selected localization's names.
async fn control_names(view: &Rc<maestro_web::dom::DomView>) {
    for (language, names) in [
        ("en", ["Download", "Close"]),
        ("de", ["Herunterladen", "Schließen"]),
    ] {
        let localized = crate::contributions_support::localized_view(view, language);
        let (_, root) = opened(&localized, crate::attachment_support::attachment());
        let tree =
            crate::contributions_support::driver("driverAria", ".maestro-attachment-actions")
                .await
                .as_string()
                .unwrap();
        for name in names {
            assert!(tree.contains(&format!("button \"{name}\"")), "{tree}");
        }
        close(&root);
    }
}

/// Progressive native pages preserve identity and release finished documents.
async fn overlay_pdf(view: &Rc<maestro_web::dom::DomView>, page: &JsValue) {
    let record = crate::attachment_support::attachment();
    {
        let mut a = record.write().unwrap();
        a.mime_type = "application/pdf".into();
        a.content = crate::pdf_views_browser::encoded(true);
    }
    let (_, root) = opened(view, record);
    crate::contributions_support::frames(page, 0).await;
    let first = root.query_selector(".maestro-pdf img").unwrap().unwrap();
    assert_eq!(
        root.query_selector_all(".maestro-pdf img")
            .unwrap()
            .length(),
        1
    );
    crate::contributions_support::frames(page, 2).await;
    let pages = root.query_selector_all(".maestro-pdf img").unwrap();
    assert_eq!(pages.length(), 3);
    assert!(first.is_same_node(pages.item(0).as_ref()));
    assert_eq!(root.query_selector_all("hr").unwrap().length(), 2);
    let sources = (0..3)
        .map(|i| {
            pages
                .item(i)
                .unwrap()
                .unchecked_into::<Element>()
                .get_attribute("src")
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_ne!(sources[0], sources[1]);
    assert_eq!(sources[0], sources[2]);
    close(&root);
}
/// Sheet buttons change only visibility, retaining native bodies and scroll positions.
fn overlay_sheets(view: &Rc<maestro_web::dom::DomView>) {
    for name in ["two", "scroll"] {
        let record = crate::attachment_support::attachment();
        {
            let mut a = record.write().unwrap();
            a.file_name = "book.xlsx".into();
            a.content = crate::excel_views_browser::encoded(name);
        }
        let (_, root) = opened(view, record);
        if name == "scroll" {
            root.first_element_child()
                .unwrap()
                .set_attribute("style", "width:240px;height:140px")
                .unwrap();
        }
        let sheets = root.query_selector_all(".maestro-excel-sheet").unwrap();
        assert!(sheets.length() >= 2);
        let first: HtmlElement = sheets.item(0).unwrap().unchecked_into();
        let second: HtmlElement = sheets.item(1).unwrap().unchecked_into();
        assert_eq!(
            first.style().get_property_value("display").unwrap(),
            "block"
        );
        assert_eq!(
            second.style().get_property_value("display").unwrap(),
            "none"
        );
        first.set_scroll_top(21);
        first.set_scroll_left(150);
        let horizontal = first.scroll_left();
        let scroll = first.scroll_top();
        if name == "scroll" {
            assert!(scroll > 0 && horizontal > 0);
        }
        control(&root, ".maestro-excel-tab:nth-child(2)");
        assert_eq!(first.style().get_property_value("display").unwrap(), "none");
        assert_eq!(
            second.style().get_property_value("display").unwrap(),
            "block"
        );
        assert!(
            second.is_same_node(
                root.query_selector_all(".maestro-excel-sheet")
                    .unwrap()
                    .item(1)
                    .as_ref()
            )
        );
        control(&root, ".maestro-excel-tab:nth-child(1)");
        assert_eq!(first.scroll_top(), scroll);
        assert_eq!(first.scroll_left(), horizontal);
        close(&root);
    }
}
/// Each native Word body keeps only its own live image resources until close.
async fn overlay_word_resources(view: &Rc<maestro_web::dom::DomView>) {
    use crate::docx_preview_browser::fetch_bytes;
    let case = crate::native_document_artifacts_browser::packages()
        .into_iter()
        .find(|case| case.name == "image-inline")
        .unwrap();
    let record = crate::attachment_support::attachment();
    {
        let mut a = record.write().unwrap();
        a.mime_type =
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into();
        a.content = case.content;
    }
    let (_, first) = opened(view, std::sync::Arc::clone(&record));
    let (_, second) = opened(view, record);
    let first_url = first
        .query_selector("img")
        .unwrap()
        .unwrap()
        .get_attribute("src")
        .unwrap();
    let second_url = second
        .query_selector("img")
        .unwrap()
        .unwrap()
        .get_attribute("src")
        .unwrap();
    assert_ne!(first_url, second_url);
    assert!(!fetch_bytes(&first_url).await.is_empty());
    assert!(!fetch_bytes(&second_url).await.is_empty());
    close(&first);
    assert!(fetch_bytes(&first_url).await.is_empty());
    assert!(!fetch_bytes(&second_url).await.is_empty());
    close(&second);
    assert!(fetch_bytes(&second_url).await.is_empty());
}
/// Real format fixtures run through the overlay's shared document owners.
#[wasm_bindgen]
pub async fn overlay_dom_documents_keep_resources_and_sheet_state(import_module: &Function) {
    use base64::Engine as _;
    let view = view();
    let page = crate::contributions_support::page_module(import_module).await;
    overlay_pdf(&view, &page).await;
    overlay::pdf::raw_admission(&view);
    overlay::pdf::live_replacement(&view, &page).await;
    overlay_sheets(&view);
    overlay::sheet_replacement(&view);
    overlay::sheet_failure(&view);
    overlay_word_resources(&view).await;
    for case in crate::native_document_artifacts_browser::packages() {
        let record = crate::attachment_support::attachment();
        {
            let mut a = record.write().unwrap();
            a.mime_type =
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into();
            a.content.clone_from(&case.content);
        }
        let (_, root) = opened(&view, record);
        let document = root
            .query_selector(".maestro-docx-container")
            .unwrap()
            .unwrap();
        crate::native_document_artifacts_browser::assert_document(&document, &case);
        close(&root);
    }
    let record = crate::attachment_support::attachment();
    {
        let mut a = record.write().unwrap();
        a.file_name = "single.xlsx".into();
        a.content = base64::engine::general_purpose::STANDARD.encode(include_bytes!(
            "../../maestro-app/tests/fixtures/spreadsheet_html/rich.xlsx"
        ));
    }
    let (_, root) = opened(&view, record);
    assert!(
        root.query_selector(".maestro-excel-single table")
            .unwrap()
            .is_some()
    );
    assert!(
        root.query_selector(".maestro-excel-tabs")
            .unwrap()
            .is_none()
    );
    close(&root);
}
/// Exercise every close route while document work is still pending.
async fn close_routes(
    view: &Rc<maestro_web::dom::DomView>,
    page: &JsValue,
    native: &crate::browser_fixture_support::FixtureModule,
    calls: &Rc<std::cell::Cell<u32>>,
) {
    for route in ["[data-close]", ".maestro-attachment-overlay", "Escape"] {
        let record = crate::attachment_support::attachment();
        {
            let mut a = record.write().unwrap();
            a.mime_type = "application/pdf".into();
            a.content = crate::pdf_views_browser::encoded(true);
            a.extracted_text = Some("literal text".into());
        }
        let seen = Rc::clone(calls);
        let overlay = maestro_web::AttachmentOverlay::open(
            view,
            record,
            Some(Rc::new(move || seen.set(seen.get() + 1))),
        )
        .unwrap();
        let root: Element = overlay.mount(view).unwrap().node.unchecked_into();
        control(&root, "header");
        control(&root, ".maestro-attachment-body");
        assert!(root.is_connected());
        assert!(root.query_selector(".maestro-pdf").unwrap().is_some());
        control(&root, ".maestro-toggle button:nth-child(2)");
        assert_eq!(
            root.query_selector("pre")
                .unwrap()
                .unwrap()
                .text_content()
                .unwrap(),
            "literal text"
        );
        control(&root, ".maestro-toggle button:nth-child(1)");
        let old_body = root.query_selector(".maestro-pdf").unwrap().unwrap();
        let count = calls.get();
        if route == "Escape" {
            native
                .call("pressKey", &Array::of1(&"Escape".into()))
                .unwrap();
        } else {
            control(&root, route);
        }
        assert!(!root.is_connected());
        assert_eq!(calls.get(), count + 1);
        control(&root, "[data-close]");
        assert_eq!(calls.get(), count + 1);
        crate::contributions_support::frames(page, 3).await;
        assert_eq!(old_body.query_selector_all("img").unwrap().length(), 0);
        drop(overlay);
    }
}
/// Click routing, cancellation, independent reopen and one-shot notification.
#[wasm_bindgen]
pub async fn overlay_dom_close_reopen_and_pending_document_release(import_module: &Function) {
    use std::cell::Cell;
    let view = view();
    let page = crate::contributions_support::page_module(import_module).await;
    let native = crate::browser_fixture_support::FixtureModule::import(
        import_module,
        &wasm_bindgen::link_to!(module = "/tests/native_attachment_support.js"),
    )
    .await
    .unwrap();
    let calls = Rc::new(Cell::new(0));
    native.call("beginKeys", &Array::new()).unwrap();
    close_routes(&view, &page, &native, &calls).await;
    native.call("finishKeys", &Array::new()).unwrap();
    assert_eq!(Rc::strong_count(&calls), 1);
    assert_eq!(calls.get(), 3);
    overlay::close_callback(&view);
    overlay::keyboard_order(&view).await;
    crate::session_header_browser::attachment_layer_intercepts_header_controls(&view).await;
    let (_, first) = opened(&view, crate::attachment_support::attachment());
    let (_, second) = opened(&view, crate::attachment_support::attachment());
    close(&first);
    assert!(second.is_connected());
    close(&second);
    let (_, first) = opened(&view, crate::attachment_support::attachment());
    let (_, second) = opened(&view, crate::attachment_support::attachment());
    native
        .call("pressKey", &Array::of1(&"Escape".into()))
        .unwrap();
    assert!(!first.is_connected() && !second.is_connected());
}

/// Exercise original-byte downloads and each reached native refusal independently.
async fn download_modes(
    view: &Rc<maestro_web::dom::DomView>,
    download: &crate::session_download_browser::Fixture,
    native: &crate::browser_fixture_support::FixtureModule,
) {
    for mode in ["ok", "create", "append", "removed"] {
        let record = crate::attachment_support::attachment();
        {
            let mut a = record.write().unwrap();
            a.content = "QUJD".into();
            a.file_name = "literal<&>.bin".into();
            a.mime_type = "application/octet-stream".into();
        }
        let (_, root) = opened(view, std::sync::Arc::clone(&record));
        record.write().unwrap().content = "\t AAH/ \n".into();
        let pre = root.query_selector("pre").unwrap().unwrap();
        download.begin(mode);
        native.call("beginErrors", &Array::new()).unwrap();
        control(&root, "[data-download]");
        let success = matches!(mode, "ok" | "removed");
        native
            .call("finishErrors", &Array::of1(&u32::from(!success).into()))
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(download.finish(
            &[0, 1, 255],
            "application/octet-stream",
            "literal<&>.bin",
            success,
        ))
        .await
        .unwrap();
        assert!(
            pre.is_same_node(
                root.query_selector("pre")
                    .unwrap()
                    .as_ref()
                    .map(AsRef::as_ref)
            )
        );
        assert!(root.is_connected());
        close(&root);
    }
}
/// Original bytes, native refusal propagation and synchronous overlay append failure.
#[wasm_bindgen]
pub async fn overlay_dom_downloads_original_bytes_and_reports_failures(import_module: &Function) {
    let view = view();
    let download = crate::session_download_browser::Fixture::import(import_module).await;
    let native = crate::browser_fixture_support::FixtureModule::import(
        import_module,
        &wasm_bindgen::link_to!(module = "/tests/native_attachment_support.js"),
    )
    .await
    .unwrap();
    download_modes(&view, &download, &native).await;
    let record = crate::attachment_support::attachment();
    record.write().unwrap().content = "invalid!".into();
    let (_, root) = opened(&view, record);
    download.begin("ok");
    native.call("beginErrors", &Array::new()).unwrap();
    control(&root, "[data-download]");
    native.call("finishErrors", &Array::of1(&1.into())).unwrap();
    wasm_bindgen_futures::JsFuture::from(download.finish_none())
        .await
        .unwrap();
    assert!(root.query_selector("pre").unwrap().is_some());
    close(&root);
    refused_overlay_open(&view, &download, &native).await;
    let (_, root) = opened(&view, crate::attachment_support::attachment());
    close(&root);
}

/// Overlay construction and publication failures return on the synchronous opening route.
async fn refused_overlay_open(
    view: &Rc<maestro_web::dom::DomView>,
    download: &crate::session_download_browser::Fixture,
    native: &crate::browser_fixture_support::FixtureModule,
) {
    let restore: Function = native
        .call("refuseCreation", &Array::new())
        .unwrap()
        .unchecked_into();
    let error =
        maestro_web::AttachmentOverlay::open(view, crate::attachment_support::attachment(), None)
            .err()
            .unwrap();
    restore.call0(&JsValue::UNDEFINED).unwrap();
    assert!(error.to_string().contains("controlled overlay creation"));
    for mode in ["append", "body"] {
        download.begin(mode);
        let error = maestro_web::AttachmentOverlay::open(
            view,
            crate::attachment_support::attachment(),
            None,
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains(if mode == "append" {
            "controlled append"
        } else {
            "Missing document body"
        }));
        wasm_bindgen_futures::JsFuture::from(download.finish_none())
            .await
            .unwrap();
        assert_eq!(
            view.document
                .query_selector_all("attachment-overlay")
                .unwrap()
                .length(),
            0
        );
    }
}

/// Every thumbnail branch preserves the original URL and literal filename.
#[wasm_bindgen]
pub async fn tile_dom_preview_fallback_badge_and_utf16_names(import_module: &Function) {
    let view = view();
    crate::attachment_tiles::native_cases(&view);
    crate::attachment_surfaces::attachment_surfaces_keep_fractional_rects_and_owned_content(&view);
    crate::attachment_surfaces::attachment_styles_follow_caller_brand_pack(&view);
    tile_hover(&view, import_module).await;
    crate::attachment_tiles::tile_badge_keeps_image_pointer_route(&view, import_module).await;
}

/// Removal uses the latest callback and never reaches the opening listener.
#[wasm_bindgen]
pub async fn tile_dom_remove_does_not_open_and_uses_current_callback() {
    use maestro_web::components::attachment_tile::AttachmentTile;
    let view = view();
    crate::attachment_surfaces::attachment_tiles_replace_transactionally_and_release_listeners(
        &view,
    );
    let mut tile = AttachmentTile::from(crate::attachment_support::attachment());
    let root = tile.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    let node: Element = root.node.clone().unchecked_into();
    assert!(node.query_selector("[data-remove]").unwrap().is_none());
    tile.show_delete = true;
    tile.update(&root, &view).unwrap();
    control(&node, "[data-remove]");
    let calls = Rc::new(std::cell::Cell::new(0));
    let old = Rc::clone(&calls);
    tile.on_delete = Some(Rc::new(move || old.set(old.get() + 100)));
    tile.update(&root, &view).unwrap();
    let current = Rc::clone(&calls);
    tile.on_delete = Some(Rc::new(move || current.set(current.get() + 1)));
    tile.update(&root, &view).unwrap();
    let remove: HtmlElement = node
        .query_selector("[data-remove]")
        .unwrap()
        .unwrap()
        .unchecked_into();
    assert_eq!(
        crate::contributions_support::computed(remove.as_ref(), "opacity"),
        "1"
    );
    remove.focus().unwrap();
    assert!(
        view.document
            .active_element()
            .unwrap()
            .is_same_node(Some(&remove))
    );
    crate::contributions_support::driver("driverPress", "Enter").await;
    assert_eq!(calls.get(), 1);
    crate::contributions_support::driver("driverPress", "Space").await;
    assert_eq!(calls.get(), 2);
    tile.on_delete = None;
    tile.update(&root, &view).unwrap();
    control(&node, "[data-remove]");
    assert_eq!(calls.get(), 2);
    tile.show_delete = false;
    tile.update(&root, &view).unwrap();
    assert!(node.query_selector("[data-remove]").unwrap().is_none());
    assert_eq!(
        view.document
            .query_selector_all("attachment-overlay")
            .unwrap()
            .length(),
        0
    );
    node.remove();
    opener_keyboard(&view).await;
}
/// The opener is a native button named by the filename: Tab reaches it before removal, Enter and
/// Space each open the current record, and it keeps focus while an update changes its thumbnail.
async fn opener_keyboard(view: &Rc<maestro_web::dom::DomView>) {
    use crate::contributions_support::{driver, focused};
    let record = crate::attachment_support::attachment();
    let mut tile = maestro_web::components::attachment_tile::AttachmentTile::from(
        std::sync::Arc::clone(&record),
    );
    tile.show_delete = true;
    let before: HtmlElement = view
        .document
        .create_element("button")
        .unwrap()
        .unchecked_into();
    body().append_child(&before).unwrap();
    let root = tile.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let node: Element = root.node.clone().unchecked_into();
    let opener = select(&node, "[data-open]");
    let names = driver("driverAria", ".maestro-attachment-tile")
        .await
        .as_string()
        .unwrap();
    for name in ["note.txt", "Remove"] {
        assert!(names.contains(&format!("button \"{name}\"")), "{names}");
    }
    before.focus().unwrap();
    driver("driverPress", "Tab").await;
    assert!(focused(&opener));
    keyboard_opens(view).await;
    record.write().unwrap().preview = Some("AA==".into());
    tile.update(&root, view).unwrap();
    assert!(select(&node, "[data-open]").is_same_node(Some(&opener)));
    assert!(node.query_selector("[data-open] img").unwrap().is_some());
    assert!(focused(&opener), "an update keeps focus on the opener");
    driver("driverPress", "Tab").await;
    assert!(focused(&select(&node, "[data-remove]")));
    node.remove();
    before.remove();
}
/// Enter and Space on the focused opener each open the current record once.
async fn keyboard_opens(view: &maestro_web::dom::DomView) {
    for key in ["Enter", "Space"] {
        crate::contributions_support::driver("driverPress", key).await;
        let overlay = view.document.query_selector("attachment-overlay").unwrap();
        let overlay = overlay.unwrap_or_else(|| panic!("{key} opens the record"));
        assert_eq!(
            text(&select(&overlay, ".maestro-attachment-name")),
            "note.txt"
        );
        close(&overlay);
    }
}

/// Opened overlays retain the original record after their originating tile is gone.
#[wasm_bindgen]
pub fn tile_dom_opened_overlays_outlive_tiles() {
    use maestro_web::components::attachment_tile::AttachmentTile;
    let view = view();
    let attachment = crate::attachment_support::attachment();
    attachment.write().unwrap().mime_type = "image/png".into();
    attachment.write().unwrap().r#type =
        maestro_app::browser::utils::attachment_utils::AttachmentType::Image;
    attachment.write().unwrap().content = "YQ==".into();
    attachment.write().unwrap().preview = None;
    let mut tile = AttachmentTile::from(std::sync::Arc::clone(&attachment));
    let root = tile.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    let node: Element = root.node.clone().unchecked_into();
    attachment.write().unwrap().content = "Zmlyc3Q=".into();
    control(&node, "[data-open]");
    let first = view
        .document
        .query_selector("attachment-overlay")
        .unwrap()
        .unwrap();
    assert_eq!(
        select(first.as_ref(), "img").get_attribute("src").unwrap(),
        "data:image/png;base64,Zmlyc3Q="
    );
    tile.attachment = crate::attachment_support::attachment();
    let attachment = std::sync::Arc::clone(&tile.attachment);
    attachment.write().unwrap().mime_type = "image/png".into();
    attachment.write().unwrap().r#type =
        maestro_app::browser::utils::attachment_utils::AttachmentType::Image;
    attachment.write().unwrap().content = "c2Vjb25k".into();
    attachment.write().unwrap().file_name = "changed.txt".into();
    attachment.write().unwrap().preview = Some("PNG".into());
    tile.update(&root, &view).unwrap();
    control(&node, "[data-open]");
    let second = view
        .document
        .query_selector_all("attachment-overlay")
        .unwrap()
        .item(1)
        .unwrap()
        .unchecked_into::<Element>();
    assert_eq!(
        select(second.as_ref(), "img").get_attribute("src").unwrap(),
        "data:image/png;base64,c2Vjb25k"
    );
    assert_eq!(
        text(&select(second.as_ref(), ".maestro-attachment-name")),
        "changed.txt"
    );
    node.remove();
    drop((tile, root));
    assert!(first.is_connected());
    assert!(second.is_connected());
    close(&first);
    assert!(second.is_connected());
    close(&second);
    crate::attachment_surfaces::attachment_modal_hides_and_restores_existing_surfaces(&view);
    crate::attachment_tiles::user_attachment_tiles_wrap_as_single_fixed_items(&view);
}

/// Hover-capable paint and focused removal remain visible through native pointer input.
async fn tile_hover(view: &maestro_web::dom::DomView, import_module: &Function) {
    use crate::contributions_support::{call, computed, driver, page_module};
    let page = page_module(import_module).await;
    let mut tile = maestro_web::components::attachment_tile::AttachmentTile::from(
        crate::attachment_support::attachment(),
    );
    tile.show_delete = true;
    let root = tile.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let remove = select(&root.node, "[data-remove]");
    assert_eq!(computed(&remove, "opacity"), "0");
    let rect = select(&root.node, "[data-open]").get_bounding_client_rect();
    driver(
        "driverPointer",
        &format!("[{}, {}, false]", rect.x() + 10., rect.y() + 10.),
    )
    .await;
    call(&page, "finish", &Array::of1(&remove)).await;
    assert_eq!(computed(&remove, "opacity"), "1");
    driver("driverPointer", "[700, 500, false]").await;
    call(&page, "finish", &Array::of1(&remove)).await;
    remove.unchecked_ref::<HtmlElement>().focus().unwrap();
    call(&page, "finish", &Array::of1(&remove)).await;
    assert_eq!(computed(&remove, "opacity"), "1");
    root.node.unchecked_ref::<Element>().remove();
}
