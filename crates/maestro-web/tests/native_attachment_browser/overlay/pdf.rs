//! Original PDF admission and replacement through the native overlay.
use super::{close, control, opened, refresh};
use crate::contributions_support::{frames, select, text};
use maestro_web::{SharedAttachment, dom::DomView};
use std::{rc::Rc, sync::Arc};
use wasm_bindgen::JsValue;

/// Original multipage input, with the raw attachment byte domain.
fn record() -> SharedAttachment {
    let record = crate::attachment_support::attachment();
    {
        let mut attachment = record.write().unwrap();
        attachment.mime_type = "application/pdf".into();
        attachment.content = crate::pdf_views_browser::encoded(true);
    }
    record
}

/// Raw overlay content accepts complete base64, never an artifact data-URL prefix.
pub(in super::super) fn raw_admission(view: &Rc<DomView>) {
    for (input, success) in [
        (crate::pdf_views_browser::encoded(true), true),
        (
            format!(
                "data:application/pdf;base64,{}",
                crate::pdf_views_browser::encoded(true)
            ),
            false,
        ),
        ("hello!".into(), false),
    ] {
        let record = record();
        record.write().unwrap().content = input;
        let (_, root) = opened(view, record);
        assert_eq!(
            root.query_selector(".maestro-pdf").unwrap().is_some(),
            success
        );
        assert_eq!(
            root.query_selector(".maestro-file-error")
                .unwrap()
                .is_some(),
            !success
        );
        assert!(root.query_selector("pre").unwrap().is_none());
        assert!(root.query_selector("[data-download]").unwrap().is_some());
        assert!(root.query_selector("[data-close]").unwrap().is_some());
        close(&root);
    }
}

/// Equal inputs retain pages; replacement and mode changes cancel old progressive work.
pub(in super::super) async fn live_replacement(view: &Rc<DomView>, page: &JsValue) {
    let record = record();
    let (overlay, root) = opened(view, Arc::clone(&record));
    frames(page, 0).await;
    let old = select(root.as_ref(), ".maestro-pdf");
    let first = select(root.as_ref(), ".maestro-pdf img");
    assert_eq!(old.query_selector_all("img").unwrap().length(), 1);
    refresh(&overlay, view, &root);
    assert!(first.is_same_node(Some(&select(root.as_ref(), ".maestro-pdf img"))));
    record.write().unwrap().content = crate::pdf_views_browser::encoded(false);
    refresh(&overlay, view, &root);
    assert!(!old.is_connected());
    frames(page, 3).await;
    assert_eq!(old.query_selector_all("img").unwrap().length(), 1);
    assert!(first.is_same_node(Some(&select(old.as_ref(), "img"))));
    assert_eq!(
        root.query_selector_all(".maestro-pdf img")
            .unwrap()
            .length(),
        1
    );
    let replacement = select(root.as_ref(), ".maestro-pdf");
    control(&root, ".maestro-toggle button:nth-child(2)");
    record.write().unwrap().extracted_text = Some("live alias".into());
    refresh(&overlay, view, &root);
    assert_eq!(text(&select(root.as_ref(), "pre")), "live alias");
    control(&root, ".maestro-toggle button:nth-child(1)");
    assert!(!replacement.is_same_node(Some(&select(root.as_ref(), ".maestro-pdf"))));
    let pending = select(root.as_ref(), ".maestro-pdf");
    record.write().unwrap().mime_type = "text/plain".into();
    refresh(&overlay, view, &root);
    frames(page, 3).await;
    assert_eq!(pending.query_selector_all("img").unwrap().length(), 0);
    assert_eq!(text(&select(root.as_ref(), "pre")), "live alias");
    record.write().unwrap().mime_type = "application/pdf".into();
    refresh(&overlay, view, &root);
    assert!(root.query_selector(".maestro-pdf").unwrap().is_some());
    close(&root);
}
