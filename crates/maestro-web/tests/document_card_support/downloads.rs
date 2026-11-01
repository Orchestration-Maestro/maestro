//! Original returned files observed through native preview and byte-download controls.
#![cfg(all(test, target_arch = "wasm32"))]
use super::{DownloadExpectation, Row, browser::Card, file_subset, fixture};
use crate::contributions_support::{host, select, text, view};
use js_sys::Array;
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::{Element, HtmlElement};

/// Open the selected real file tile; the overlay is independently page-owned.
pub(super) fn open(card: &Card, index: usize) -> Element {
    let target = card
        .root()
        .query_selector_all("[data-open]")
        .unwrap()
        .item(u32::try_from(index).unwrap())
        .unwrap();
    target.unchecked_ref::<HtmlElement>().click();
    card.view
        .document
        .query_selector_all("attachment-overlay")
        .unwrap()
        .item(
            card.view
                .document
                .query_selector_all("attachment-overlay")
                .unwrap()
                .length()
                - 1,
        )
        .unwrap()
        .unchecked_into()
}
/// Close exactly this independently opened overlay.
pub(super) fn close(overlay: &Element) {
    select(overlay, "[data-close]")
        .unchecked_ref::<HtmlElement>()
        .click();
    assert!(!overlay.is_connected());
}
/// Browser decoding supplies original bytes independently from the Rust renderer.
fn bytes(content: &str) -> Result<Vec<u8>, JsValue> {
    web_sys::window().unwrap().atob(content).map(|binary| {
        binary
            .chars()
            .map(|c| u8::try_from(u32::from(c)).unwrap())
            .collect()
    })
}
/// Compare literal tile identity, previews and extracted text before downloading the original.
async fn check_file(
    card: &Card,
    expected: &DownloadExpectation,
    index: usize,
    downloads: &crate::session_download_browser::Fixture,
) {
    let target = card
        .root()
        .query_selector_all("[data-open]")
        .unwrap()
        .item(u32::try_from(index).unwrap())
        .unwrap();
    let target: &Element = target.unchecked_ref();
    assert_eq!(target.get_attribute("title").unwrap(), expected.file_name);
    let image = target.query_selector("img").unwrap();
    assert_eq!(image.is_some(), expected.preview.is_some());
    if let Some(preview) = &expected.preview {
        assert!(
            image
                .unwrap()
                .get_attribute("src")
                .unwrap()
                .ends_with(preview)
        );
    }
    let overlay = open(card, index);
    assert_eq!(
        text(&select(&overlay, ".maestro-attachment-name")),
        expected.file_name
    );
    if let Some(extracted) = expected.extracted_text.as_ref().filter(|s| !s.is_empty()) {
        assert_eq!(text(&select(&overlay, "pre")), *extracted);
    }
    if let Ok(bytes) = bytes(&expected.content) {
        // Original records keep authored MIME case; native Blob types are ASCII-lowercase.
        downloads
            .saved(
                select(&overlay, "[data-download]").unchecked_ref(),
                &bytes,
                &expected.mime_type.to_ascii_lowercase(),
                &expected.file_name,
            )
            .await;
    }
    close(&overlay);
}
/// Replay every returned file, including duplicate names and MIME defaults, through native controls.
pub async fn records(import: &js_sys::Function, rows: Vec<Row>) {
    let view = view();
    let _elements = host(&view, import);
    let downloads = crate::session_download_browser::Fixture::import(import).await;
    for row in rows {
        let card = Card::new(view.clone(), &row);
        let expected = &row.expected.as_ref().unwrap().attachments;
        assert_eq!(
            card.root()
                .query_selector_all("[data-open]")
                .unwrap()
                .length() as usize,
            expected.len()
        );
        for (index, expected) in expected.iter().enumerate() {
            check_file(&card, expected, index, &downloads).await;
        }
    }
}
/// Select the original complete file cases, without rebuilding their oracles.
pub fn all_files() -> Vec<Row> {
    fixture()
        .into_iter()
        .filter(|r| {
            r.id == "repl-files" || r.id == "repl-duplicate-files" || r.group == "fileViews"
        })
        .collect()
}
/// Malformed encoded files still open with their original name and reject downloading invalid bytes.
pub async fn malformed(import: &js_sys::Function) {
    let view = view();
    let _elements = host(&view, import);
    let downloads = crate::session_download_browser::Fixture::import(import).await;
    let native = crate::browser_fixture_support::FixtureModule::import(
        import,
        &wasm_bindgen::link_to!(module = "/tests/native_attachment_support.js"),
    )
    .await
    .unwrap();
    let row = file_subset(&[7, 8, 9, 11]);
    let card = Card::new(view, &row);
    for (index, file) in row
        .expected
        .as_ref()
        .unwrap()
        .attachments
        .iter()
        .enumerate()
    {
        let overlay = open(&card, index);
        downloads.begin("success");
        native.call("beginErrors", &Array::new()).unwrap();
        select(&overlay, "[data-download]")
            .unchecked_ref::<HtmlElement>()
            .click();
        let decoded = bytes(&file.content);
        native
            .call(
                "finishErrors",
                &Array::of1(&u32::from(decoded.is_err()).into()),
            )
            .unwrap();
        let finish = match decoded {
            Ok(bytes) => downloads.finish(&bytes, &file.mime_type, &file.file_name, true),
            Err(_) => downloads.finish_none(),
        };
        wasm_bindgen_futures::JsFuture::from(finish).await.unwrap();
        close(&overlay);
    }
}
/// Rebinding a returned tile never rewrites a preview that was already opened from it.
pub async fn changed_file_keeps_opened_original(import: &js_sys::Function) {
    let view = view();
    let _elements = host(&view, import);
    let downloads = crate::session_download_browser::Fixture::import(import).await;
    let mut row = file_subset(&[1]);
    let mut card = Card::new(view, &row);
    let original = open(&card, 0);
    row.input.result.as_mut().unwrap().details.as_mut().unwrap()["files"][0]["contentBase64"] =
        serde_json::json!("TmV3");
    card.show(&row);
    let changed = open(&card, 0);
    drop(card);
    assert!(original.is_connected() && changed.is_connected());
    for (overlay, content) in [(&original, "雪"), (&changed, "New")] {
        assert_eq!(text(&select(overlay, "pre")), content);
        downloads
            .saved(
                select(overlay, "[data-download]").unchecked_ref(),
                content.as_bytes(),
                "text/plain",
                "雪.txt",
            )
            .await;
        close(overlay);
    }
}
