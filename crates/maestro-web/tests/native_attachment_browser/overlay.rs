//! Live attachment edits observed through the mounted native overlay.
use super::{close, control, opened};
use crate::contributions_support::{driver, select, text};
use maestro_web::{
    AttachmentOverlay,
    dom::{DomContent as _, DomView},
};
use std::{cell::Cell, rc::Rc, sync::Arc};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement};
pub(super) mod pdf;

/// Publish current original fields without replacing the containing overlay.
fn refresh(overlay: &AttachmentOverlay, view: &DomView, root: &Element) {
    let mounted = overlay.mount(view).unwrap();
    assert!(mounted.node.is_same_node(Some(root)));
    overlay.update(&mounted, view).unwrap();
}

/// Plain and presentation text stay literal, including absent, empty and whitespace edits.
pub(super) fn live_text(view: &Rc<DomView>) {
    let record = crate::attachment_support::attachment();
    let (overlay, root) = opened(view, Arc::clone(&record));
    for (input, expected) in [
        (None, "No content available"),
        (Some(""), "No content available"),
        (Some(" \n"), " \n"),
        (Some("<literal>\n\t&"), "<literal>\n\t&"),
    ] {
        record.write().unwrap().extracted_text = input.map(str::to_owned);
        refresh(&overlay, view, &root);
        assert_eq!(text(&select(root.as_ref(), "pre")), expected);
    }
    {
        let mut original = record.write().unwrap();
        original.file_name = "<slides>.PPTX".into();
        original.extracted_text = None;
    }
    refresh(&overlay, view, &root);
    assert_eq!(
        text(&select(root.as_ref(), ".maestro-attachment-name")),
        "<slides>.PPTX"
    );
    for (input, expected) in [
        (Some("<slide>\n\tile&"), "<slide>\n\tile&"),
        (None, "No text content available"),
        (Some(""), "No text content available"),
    ] {
        record.write().unwrap().extracted_text = input.map(str::to_owned);
        refresh(&overlay, view, &root);
        assert_eq!(
            text(&select(
                root.as_ref(),
                ".maestro-attachment-presentation pre"
            )),
            expected
        );
    }
    assert!(
        root.query_selector(".maestro-toggle button")
            .unwrap()
            .is_none()
    );
    close(&root);
}

/// Text selection remains live across file kinds; image priority still wins.
pub(super) fn live_modes(view: &Rc<DomView>) {
    let record = crate::attachment_support::attachment();
    {
        let mut original = record.write().unwrap();
        original.file_name = "book.xlsx".into();
        original.content = crate::excel_views_browser::encoded("two");
    }
    let (overlay, root) = opened(view, Arc::clone(&record));
    let sheet = select(root.as_ref(), ".maestro-excel");
    let button = select(root.as_ref(), ".maestro-toggle button");
    refresh(&overlay, view, &root);
    assert!(sheet.is_same_node(Some(&select(root.as_ref(), ".maestro-excel"))));
    assert!(button.is_same_node(Some(&select(root.as_ref(), ".maestro-toggle button"))));
    control(&root, ".maestro-toggle button:nth-child(2)");
    for (input, expected) in [
        (None, "No text content available"),
        (Some(""), "No text content available"),
        (Some(" \n"), " \n"),
    ] {
        record.write().unwrap().extracted_text = input.map(str::to_owned);
        refresh(&overlay, view, &root);
        assert_eq!(
            text(&select(root.as_ref(), ".maestro-attachment-extracted pre")),
            expected
        );
    }
    for (mime, image) in [
        ("image/png", true),
        ("text/plain", false),
        ("presentationml", false),
    ] {
        {
            let mut original = record.write().unwrap();
            original.file_name = "changed".into();
            original.mime_type = mime.into();
            original.r#type = if image {
                maestro_app::browser::utils::attachment_utils::AttachmentType::Image
            } else {
                maestro_app::browser::utils::attachment_utils::AttachmentType::Document
            };
        }
        refresh(&overlay, view, &root);
        assert_eq!(root.query_selector("img").unwrap().is_some(), image);
        if !image {
            assert_eq!(
                text(&select(root.as_ref(), ".maestro-attachment-extracted pre")),
                " \n"
            );
        }
    }
    close(&root);
}

/// A replaced spreadsheet drops its old native body; unrelated updates keep the selected sheet.
pub(super) fn sheet_replacement(view: &Rc<DomView>) {
    use base64::Engine as _;
    let record = crate::attachment_support::attachment();
    {
        let mut original = record.write().unwrap();
        original.file_name = "book.xlsx".into();
        original.content = crate::excel_views_browser::encoded("two");
    }
    let (overlay, root) = opened(view, Arc::clone(&record));
    control(&root, ".maestro-excel-tab:nth-child(2)");
    let second = root
        .query_selector_all(".maestro-excel-sheet")
        .unwrap()
        .item(1)
        .unwrap()
        .unchecked_into::<Element>();
    refresh(&overlay, view, &root);
    assert!(
        second.is_same_node(
            root.query_selector_all(".maestro-excel-sheet")
                .unwrap()
                .item(1)
                .as_ref()
        )
    );
    assert_eq!(
        crate::contributions_support::computed(&second, "display"),
        "block"
    );
    let old = select(root.as_ref(), ".maestro-excel");
    record.write().unwrap().content = base64::engine::general_purpose::STANDARD.encode(
        include_bytes!("../../../maestro-app/tests/fixtures/spreadsheet_html/one-sheet.xlsx"),
    );
    refresh(&overlay, view, &root);
    assert!(!old.is_connected());
    assert!(
        root.query_selector(".maestro-excel-single table")
            .unwrap()
            .is_some()
    );
    control(&root, ".maestro-toggle button:nth-child(2)");
    control(&root, ".maestro-toggle button:nth-child(1)");
    assert!(
        root.query_selector(".maestro-excel-single table")
            .unwrap()
            .is_some()
    );
    close(&root);
}

/// A failed workbook keeps its error until text/file selection loads the current original bytes.
pub(super) fn sheet_failure(view: &Rc<DomView>) {
    use base64::Engine as _;
    let record = crate::attachment_support::attachment();
    record.write().unwrap().mime_type = "application/vnd.ms-excel".into();
    record.write().unwrap().content = "invalid!".into();
    let (overlay, root) = opened(view, Arc::clone(&record));
    let error = select(root.as_ref(), ".maestro-file-error");
    record.write().unwrap().content = base64::engine::general_purpose::STANDARD.encode(
        include_bytes!("../../../maestro-app/tests/fixtures/spreadsheet_html/one-sheet.xlsx"),
    );
    refresh(&overlay, view, &root);
    assert!(error.is_same_node(Some(&select(root.as_ref(), ".maestro-file-error"))));
    control(&root, ".maestro-toggle button:nth-child(2)");
    assert_eq!(text(&select(root.as_ref(), "pre")), "<literal>\n\ttext");
    control(&root, ".maestro-toggle button:nth-child(1)");
    assert!(
        root.query_selector(".maestro-excel-single table")
            .unwrap()
            .is_some()
    );
    close(&root);
}

/// A visible failure stays selected after edits to every supported file family.
pub(super) fn preserve_error(
    overlay: &AttachmentOverlay,
    view: &DomView,
    root: &Element,
    record: &maestro_web::SharedAttachment,
) {
    let error = select(root.as_ref(), ".maestro-file-error");
    for mime in [
        "image/png",
        "text/plain",
        "presentationml",
        "wordprocessingml",
        "ms-excel",
        "application/pdf",
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
        refresh(overlay, view, root);
        assert!(error.is_same_node(Some(&select(root.as_ref(), ".maestro-file-error"))));
        assert!(root.query_selector("[data-download]").unwrap().is_some());
    }
}

/// A failed borrow preserves published content and can be retried after releasing the writer.
pub(super) fn borrow_failure(view: &Rc<DomView>) {
    let record = crate::attachment_support::attachment();
    let (overlay, root) = opened(view, Arc::clone(&record));
    let pre = select(root.as_ref(), "pre");
    let mounted = overlay.mount(view).unwrap();
    let mut writer = record.write().unwrap();
    writer.extracted_text = Some("after writer".into());
    assert!(overlay.update(&mounted, view).is_err());
    assert!(pre.is_same_node(Some(&select(root.as_ref(), "pre"))));
    assert_eq!(text(&pre), "<literal>\n\ttext");
    drop(writer);
    refresh(&overlay, view, &root);
    assert_eq!(text(&select(root.as_ref(), "pre")), "after writer");
    close(&root);
}

/// The native filename stays left of the right-aligned actions, above the scrolling body.
pub(super) fn chrome_layout(root: &Element) {
    let header = select(root.as_ref(), "header").get_bounding_client_rect();
    let name = select(root.as_ref(), ".maestro-attachment-name").get_bounding_client_rect();
    let download = select(root.as_ref(), "[data-download]").get_bounding_client_rect();
    let close = select(root.as_ref(), "[data-close]").get_bounding_client_rect();
    let body = select(root.as_ref(), ".maestro-attachment-body").get_bounding_client_rect();
    assert!(name.left() < name.right() && name.right() <= download.left());
    assert!(download.right() <= close.left() && close.right() <= header.right());
    assert!(name.top() >= header.top() && name.bottom() <= header.bottom());
    assert!(body.top() >= header.bottom() && body.height() > 0.);
}

/// Native tab traversal reaches modes, download and close in their visual order.
pub(super) async fn keyboard_order(view: &Rc<DomView>) {
    let record = crate::attachment_support::attachment();
    record.write().unwrap().mime_type = "application/pdf".into();
    record.write().unwrap().content = crate::pdf_views_browser::encoded(true);
    let (_, root) = opened(view, record);
    let first = select(root.as_ref(), ".maestro-toggle button").unchecked_into::<HtmlElement>();
    first.focus().unwrap();
    for selector in [
        ".maestro-toggle button:nth-child(2)",
        "[data-download]",
        "[data-close]",
    ] {
        driver("driverPress", "Tab").await;
        assert!(
            view.document
                .active_element()
                .unwrap()
                .is_same_node(Some(&select(root.as_ref(), selector)))
        );
    }
    driver("driverPress", "Enter").await;
    assert!(!root.is_connected());
}

/// Close releases pending work before invoking an unlocked, one-shot callback.
pub(super) fn close_callback(view: &Rc<DomView>) {
    let record = crate::attachment_support::attachment();
    let calls = Rc::new(Cell::new(0));
    let capture = Arc::clone(&record);
    let seen = Rc::clone(&calls);
    let document = view.document.clone();
    let overlay = AttachmentOverlay::open(
        view,
        Arc::clone(&record),
        Some(Rc::new(move || {
            assert!(
                document
                    .query_selector("attachment-overlay")
                    .unwrap()
                    .is_some()
            );
            capture.try_write().unwrap().file_name = "callback edit".into();
            seen.set(seen.get() + 1);
        })),
    )
    .unwrap();
    let root: Element = overlay.mount(view).unwrap().node.unchecked_into();
    drop(overlay);
    assert!(root.is_connected());
    assert_eq!(calls.get(), 0);
    close(&root);
    assert_eq!(calls.get(), 1);
    assert_eq!(record.read().unwrap().file_name, "callback edit");
    control(&root, "[data-close]");
    assert_eq!(calls.get(), 1);
}
