//! Attachment loading through the public application boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_app::browser::utils::attachment_utils::{
    AttachmentError, AttachmentSource, load_attachment,
};
use maestro_models::{Fetch, FetchError, HttpResponse};
use serde::Deserialize;
use serde_json::json;
use std::sync::{Arc, Mutex};
use support::{
    attachment_observation::observed,
    attachments::{corpus, equal},
    run,
};

/// Already-read platform source metadata.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Optional repeated suite selector.
    suite: Option<String>,
    /// Source kind.
    source: String,
    /// File name.
    name: Option<String>,
    /// Platform media type.
    mime: Option<String>,
    /// Filename override.
    r#override: Option<String>,
    /// Original byte data.
    bytes: Option<Vec<u8>>,
}
/// Replay one local source suite.
fn local_suite(suite: &str) {
    for row in corpus().into_iter().filter(|row| row.suite == suite) {
        let input: Input = serde_json::from_value(row.input).unwrap();
        if let Some(repeated) = &input.suite {
            assert_eq!(repeated, suite);
        }
        let bytes = input
            .bytes
            .unwrap_or_else(|| "résumé\n\t尾".as_bytes().to_vec());
        let mime = input.mime.unwrap_or_default().to_ascii_lowercase();
        let source = match input.source.as_str() {
            "file" => AttachmentSource::File {
                bytes: &bytes,
                name: input.name.as_deref().unwrap_or("note.txt"),
                mime_type: &mime,
            },
            "blob" => AttachmentSource::Blob {
                bytes: &bytes,
                mime_type: &mime,
            },
            "buffer" => AttachmentSource::Buffer(&bytes),
            _ => panic!("unexpected source"),
        };
        let fetch: Fetch = Arc::new(|_| panic!("local source fetched"));
        equal(
            &observed(run(load_attachment(
                source,
                input.r#override.as_deref(),
                &fetch,
            ))),
            &row.expected,
        );
    }
}
#[test]
fn attachment_sources_keep_filename_precedence() {
    local_suite("source_names");
}
#[test]
fn image_attachments_keep_original_bytes_and_preview() {
    local_suite("image_bytes");
}
#[test]
fn text_attachments_follow_mime_and_suffix_precedence() {
    local_suite("text_recognition");
}
#[test]
fn text_attachments_decode_without_trimming() {
    local_suite("utf8");
}
#[test]
fn attachment_ids_keep_name_time_and_random_suffix() {
    let fetch: Fetch = Arc::new(|_| panic!("local source fetched"));
    let attachment = run(load_attachment(
        AttachmentSource::Buffer(b""),
        Some("a_b.txt"),
        &fetch,
    ))
    .unwrap();
    assert_eq!(
        observed(Ok(attachment))["attachment"]["id"],
        "a_b.txt_<generated>"
    );
}
#[test]
fn attachment_serialization_keeps_optional_and_empty_fields() {
    for (mime, expected) in [
        ("image/png", json!({"preview":""})),
        ("text/plain", json!({"extractedText":""})),
    ] {
        let fetch: Fetch = Arc::new(|_| panic!("local source fetched"));
        let attachment = run(load_attachment(
            AttachmentSource::Blob {
                bytes: b"",
                mime_type: mime,
            },
            None,
            &fetch,
        ))
        .unwrap();
        let value = serde_json::to_value(attachment).unwrap();
        for (key, expected) in expected.as_object().unwrap() {
            assert_eq!(&value[key], expected);
        }
        assert_eq!(value.get("preview").is_some(), mime.starts_with("image/"));
        assert_eq!(
            value.get("extractedText").is_some(),
            mime.starts_with("text/")
        );
    }
}
#[test]
fn document_formats_do_not_fall_through_to_images_or_text() {
    let bytes = include_bytes!("fixtures/pptx/packages/01.pptx");
    for (name, mime) in [
        (
            "a.pdf",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ),
        ("a.pptx", "application/pdf"),
    ] {
        let fetch: Fetch = Arc::new(|_| panic!("local source fetched"));
        let result = run(load_attachment(
            AttachmentSource::File {
                bytes,
                name,
                mime_type: mime,
            },
            None,
            &fetch,
        ));
        assert_document_failure(&result, name, mime);
    }
    for (name, mime) in [
        ("a.PDF", "image/png"),
        ("a.XLSX", "text/plain"),
        ("a.xls", "image/png"),
        ("plain.txt", "application/pdf"),
        ("plain.txt", "application/vnd.ms-excel"),
        (
            "plain.txt",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    ] {
        let fetch: Fetch = Arc::new(|_| panic!("local source fetched"));
        let result = run(load_attachment(
            AttachmentSource::File {
                bytes: b"x",
                name,
                mime_type: mime,
            },
            None,
            &fetch,
        ));
        assert_document_failure(&result, name, mime);
    }
}

/// Preserve format precedence without falling through to text or images.
fn assert_document_failure(
    result: &Result<maestro_app::browser::utils::attachment_utils::Attachment, AttachmentError>,
    name: &str,
    mime: &str,
) {
    if mime == "application/pdf" || name.to_ascii_lowercase().ends_with(".pdf") {
        assert!(matches!(result, Err(AttachmentError::Pdf(_))));
    } else if [".xlsx", ".xls"]
        .iter()
        .any(|suffix| name.to_ascii_lowercase().ends_with(suffix))
        || [
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.ms-excel",
        ]
        .contains(&mime)
    {
        assert!(matches!(result, Err(AttachmentError::Excel(_))));
    } else {
        assert!(matches!(result, Err(AttachmentError::Unsupported(_))));
    }
}

/// Original literal URL input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UrlInput {
    /// URL suffix.
    path: String,
    /// Optional filename.
    r#override: Option<String>,
}
/// Script the transport-owned result, recording the application request.
fn url_suite(select: impl Fn(&str) -> bool) {
    for row in corpus().into_iter().filter(|row| row.suite == "urls") {
        let input: UrlInput = serde_json::from_value(row.input).unwrap();
        if !select(&input.path) {
            continue;
        }
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let path = input.path.clone();
        let fetch: Fetch = Arc::new(move |request| {
            captured.lock().unwrap().push(request);
            let response = scripted_response(&path);
            Box::pin(async move { response })
        });
        let url = format!("http://example.invalid{}", input.path);
        let result = observed(run(load_attachment(
            AttachmentSource::Url(&url),
            input.r#override.as_deref(),
            &fetch,
        )));
        let expected = row.expected;
        equal(&result, &expected);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].url, url);
        assert!(
            requests[0].headers.is_empty()
                && requests[0].body.is_empty()
                && requests[0].signal.is_none()
        );
    }
}
/// Model-owned transport outcomes for the recorded source server.
fn scripted_response(path: &str) -> Result<HttpResponse, FetchError> {
    if path == "/connection.txt" {
        return Err(FetchError::Connection(
            maestro_models::DiagnosticErrorInfo::error("fetch failed"),
        ));
    }
    let status = if path == "/status-before-body.txt" {
        404
    } else {
        path.split("status=")
            .nth(1)
            .and_then(|tail| tail.split('&').next())
            .map_or(200, |value| value.parse().unwrap())
    };
    let mime = if path == "/trailing/" {
        "text/plain"
    } else {
        path.split("mime=").nth(1).unwrap_or("")
    };
    let body = if path == "/body.txt" || path == "/status-before-body.txt" {
        Err(FetchError::Connection(
            maestro_models::DiagnosticErrorInfo::error("terminated"),
        ))
    } else {
        Ok(if status == 204 {
            vec![]
        } else {
            "body résumé".as_bytes().to_vec()
        })
    };
    Ok(HttpResponse {
        status,
        status_text: String::new(),
        headers: [("content-type".into(), mime.into())].into(),
        body: Box::pin(futures_util::stream::iter([body])),
    })
}
#[test]
fn url_attachments_keep_request_and_response_metadata() {
    url_suite(|path| {
        !path.contains("status=300")
            && !path.contains("status=404")
            && !path.contains("status=500")
            && !path.contains("body.txt")
            && !path.contains("connection.txt")
    });
}
#[test]
fn url_transport_and_body_failures_propagate() {
    url_suite(|path| path == "/body.txt" || path == "/connection.txt");
}
#[test]
fn url_status_failure_precedes_body_read() {
    use futures_util::stream::poll_fn;
    for status in [300, 404, 500] {
        let fetch: Fetch = Arc::new(move |_| {
            Box::pin(async move {
                Ok(HttpResponse {
                    status,
                    status_text: String::new(),
                    headers: [].into(),
                    body: Box::pin(poll_fn(|_| panic!("non-success body polled"))),
                })
            })
        });
        assert!(
            matches!(run(load_attachment(AttachmentSource::Url("http://example.invalid/status.txt"),None,&fetch)),Err(AttachmentError::HttpStatus(value)) if value == status)
        );
    }
    url_suite(|path| {
        path.contains("status=300")
            || path.contains("status=404")
            || path.contains("status=500")
            || path == "/status-before-body.txt"
    });
}
