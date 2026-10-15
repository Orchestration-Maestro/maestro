//! Complete-history downloads through the public operation and supplied host.
#![cfg(test)]
mod session_download_support;
use maestro_web::export_html::template::download_session_json;
use session_download_support::{Host, cases, document, replay};

#[test]
fn session_download_keeps_full_history_and_header_projection() {
    for row in cases("session_download_keeps_full_history_and_header_projection") {
        let host = Host::default();
        let mut doc = document(&row.input_json, "leafId=left");
        download_session_json(&doc, &host).unwrap();
        assert_eq!(
            host.downloads.take().as_slice(),
            std::slice::from_ref(&row.expected),
            "{}",
            row.name
        );
        if row.name == "all-branches-and-repeated-records" {
            doc.leaf_id = maestro_app::Presence::Present(serde_json::json!("right"));
            download_session_json(&doc, &host).unwrap();
            assert_eq!(
                host.downloads.take().as_slice(),
                std::slice::from_ref(&row.expected)
            );
        }
    }
}

#[test]
fn session_download_uses_session_names_and_unicode_fallbacks() {
    replay("session_download_uses_session_names_and_unicode_fallbacks");
}

#[test]
fn session_download_keeps_compact_json_order_and_number_spelling() {
    replay("session_download_keeps_compact_json_order_and_number_spelling");
}

#[test]
fn session_download_does_not_add_a_final_newline() {
    replay("session_download_does_not_add_a_final_newline");
}

#[test]
fn session_download_returns_the_supplied_host_failure() {
    let row = cases("session_download_keeps_full_history_and_header_projection").remove(0);
    let host = Host {
        fail: true,
        ..Host::default()
    };
    let error = download_session_json(&document(&row.input_json, ""), &host).unwrap_err();
    assert!(matches!(
        error,
        maestro_web::browser::attachment_content::AttachmentUiError::BorrowConflict
    ));
    assert_eq!(*host.downloads.borrow(), [row.expected]);
}
