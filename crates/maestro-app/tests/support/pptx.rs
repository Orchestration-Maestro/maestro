//! Presentation fixture replay through the public loader.
#![cfg(test)]
use super::attachment_observation::observed;
use maestro_app::browser::utils::attachment_utils::{
    AttachmentError, AttachmentSource, load_attachment,
};
use maestro_models::{Fetch, FetchError, HttpResponse};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

/// Complete recorded presentation observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Ordered cases.
    rows: Vec<Row>,
}
/// One named public observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Selecting witness.
    test: String,
    /// Diagnostic case label.
    case: String,
    /// Supplied source.
    input: Input,
    /// Full expected record or error projection.
    expected: Value,
}
/// Recorded source operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Original package path.
    file: String,
    /// Platform source kind.
    source: String,
    /// Original filename.
    name: Option<String>,
    /// Supplied media type.
    mime: Option<String>,
    /// Filename override.
    r#override: Option<String>,
    /// Literal request URL.
    url: Option<String>,
    /// Scripted earlier transport failure.
    transport: Option<String>,
}
/// Replay every row owned by one witness.
pub fn suite(test: &str) {
    let corpus: Corpus = serde_json::from_str(include_str!("../fixtures/pptx/pptx.json")).unwrap();
    let rows: Vec<_> = corpus
        .rows
        .into_iter()
        .filter(|row| row.test == test)
        .collect();
    assert!(!rows.is_empty());
    for row in rows {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/pptx")
                .join(&row.input.file),
        )
        .unwrap();
        let fetch = transport(&row.input, &bytes);
        let source = match row.input.source.as_str() {
            "file" => AttachmentSource::File {
                bytes: &bytes,
                name: row.input.name.as_deref().unwrap(),
                mime_type: row.input.mime.as_deref().unwrap(),
            },
            "blob" => AttachmentSource::Blob {
                bytes: &bytes,
                mime_type: row.input.mime.as_deref().unwrap(),
            },
            "buffer" => AttachmentSource::Buffer(&bytes),
            "url" => AttachmentSource::Url(row.input.url.as_deref().unwrap()),
            _ => panic!("unexpected source"),
        };
        let result = super::run(load_attachment(
            source,
            row.input.r#override.as_deref(),
            &fetch,
        ));
        let actual = match result {
            Err(AttachmentError::Fetch(_)) => json!({"error_kind":"Fetch"}),
            Err(AttachmentError::HttpStatus(_)) => json!({"error_kind":"HttpStatus"}),
            result => observed(result),
        };
        assert_eq!(actual, row.expected, "case {}", row.case);
    }
}
/// Supply controlled transport outcomes below application response handling.
fn transport(input: &Input, bytes: &[u8]) -> Fetch {
    let bytes = bytes.to_vec();
    let mime = input.mime.clone().unwrap_or_default();
    let url = input.url.clone();
    let failure = input.transport.clone();
    Arc::new(move |request| {
        assert_eq!(Some(request.url), url);
        assert_eq!(request.method, "GET");
        assert!(request.headers.is_empty() && request.body.is_empty() && request.signal.is_none());
        let response = if failure.as_deref() == Some("connection") {
            Err(FetchError::Connection(
                maestro_models::DiagnosticErrorInfo::error("fetch failed"),
            ))
        } else {
            let failed = failure.as_deref() == Some("status");
            let bytes = bytes.clone();
            Ok(HttpResponse {
                status: if failed { 404 } else { 200 },
                status_text: String::new(),
                headers: [("content-type".into(), mime.clone())].into(),
                body: Box::pin(futures_util::stream::once(async move {
                    assert!(!failed, "rejected response body was consumed");
                    Ok(bytes)
                })),
            })
        };
        Box::pin(async move { response })
    })
}
