//! Complete-history downloads through the public operation and supplied host.
#![cfg(test)]
use crate::session_download_support;
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
        error
            .get_ref()
            .unwrap()
            .downcast_ref::<maestro_web::browser::attachment_content::AttachmentHostError>(),
        Some(maestro_web::browser::attachment_content::AttachmentHostError::BorrowConflict)
    ));
    assert_eq!(*host.downloads.borrow(), [row.expected]);
}

#[test]
fn session_preparation_failures_skip_download_and_keep_io_errors() {
    let row = cases("session_download_keeps_full_history_and_header_projection").remove(0);
    for (field, value, message) in [
        ("header", serde_json::json!(true), "nonobject header"),
        ("entries", serde_json::Value::Null, "missing entries array"),
        ("entries", serde_json::json!({}), "missing entries array"),
        (
            "header",
            serde_json::json!({"id": true}),
            "nonstring session identity",
        ),
    ] {
        let host = Host::default();
        let mut doc = document(&row.input_json, "");
        doc.data.insert(field.into(), value);
        let error = download_session_json(&doc, &host).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(error.to_string(), message);
        assert!(host.downloads.borrow().is_empty());
        download_session_json(&document(&row.input_json, ""), &host).unwrap();
        assert_eq!(
            host.downloads.borrow().as_slice(),
            std::slice::from_ref(&row.expected)
        );
    }
}
