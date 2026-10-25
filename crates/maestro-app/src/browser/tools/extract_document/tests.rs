//! Executable policy with a controlled attachment operation.
use super::*;
mod support {
    use crate as maestro_app;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/document_extraction.rs"
    ));
}
use crate::tests::runtime::run_io as run;
use std::sync::Mutex;
use support::{Effects, Input};

/// Build the same executable with controlled attachment records.
fn controlled(input: &Input, effects: &Arc<Mutex<Effects>>) -> ExtractDocumentTool {
    assert!(!input.real_attachment && input.phase.is_none());
    let loaded: Loaded = if input.attachment.is_null() {
        Loaded::default()
    } else {
        serde_json::from_value(input.attachment.clone()).unwrap()
    };
    let error = input.load_error.clone();
    let observations = Arc::clone(effects);
    build_tool(
        support::fetch(input, effects),
        Arc::new(move |bytes, name| {
            observations
                .lock()
                .unwrap()
                .loads
                .push(serde_json::json!({"fileName":name,"size":bytes.len()}));
            let loaded = loaded.clone();
            let error = error.clone();
            Box::pin(async move {
                if let Some(error) = error {
                    return Err(DiagnosticErrorInfo::error(error));
                }
                Ok(attachment(name, bytes.len(), loaded))
            })
        }),
    )
}
/// Replay a disjoint selection of corpus rows.
async fn rows(indices: &[usize]) {
    let corpus = support::corpus();
    assert_eq!(corpus.metadata.value()["name"], "extract_document");
    for (_, case) in corpus
        .cases
        .into_iter()
        .enumerate()
        .filter(|(index, _)| indices.contains(index))
    {
        let effects = Arc::new(Mutex::new(Effects::default()));
        let tool = controlled(&case.input, &effects);
        *tool.cors_proxy_url.write().unwrap() = case.input.proxy.clone();
        effects.lock().unwrap().proxy = Arc::downgrade(&tool.cors_proxy_url);
        let execute = Arc::clone(&tool.tool.read().unwrap().execute);
        support::check(case, execute, effects).await;
    }
}
/// Preflight failures dispatch no transport work.
#[test]
fn extraction_tool_rejects_cancelled_and_invalid_urls() {
    run(async {
        rows(&(0..8).collect::<Vec<_>>()).await;
    });
}
/// Status admission occurs before either header size or body reads.
#[test]
fn extraction_tool_checks_status_before_body() {
    run(async {
        rows(&(25..33).collect::<Vec<_>>()).await;
    });
}
/// Decimal declared sizes enforce the limit before reading.
#[test]
fn extraction_tool_checks_declared_size_before_body() {
    run(async {
        rows(&(33..50).collect::<Vec<_>>()).await;
    });
}
/// Complete body collection precedes the actual size check.
#[test]
fn extraction_tool_checks_downloaded_size_after_body() {
    run(async {
        rows(&[50, 51, 52, 53, 55]).await;
    });
}
/// Ordinary failures survive without loading or retrying.
#[test]
fn extraction_tool_propagates_other_download_failures() {
    run(async {
        rows(&[54]).await;
    });
}
/// CORS classification covers all download phases and permits only one retry.
#[test]
fn extraction_tool_retries_only_cors_failures() {
    run(async {
        rows(&[57, 59, 61, 63, 64, 65, 66, 67, 76, 77, 78, 79]).await;
    });
}
/// Current proxy setting and exact authored URI component encoding.
#[test]
fn extraction_tool_uses_current_proxy_and_exact_component_encoding() {
    run(async {
        rows(&[74, 75, 80]).await;
    });
}
/// Failed proxy attempts keep manual-attachment guidance and their cause.
#[test]
fn extraction_tool_reports_cors_guidance() {
    run(async {
        rows(&[56, 58, 60, 62, 68, 69, 70, 71, 72, 73]).await;
    });
}
/// Authored URL spelling and the exact arXiv prefix determine filenames.
#[test]
fn extraction_tool_preserves_authored_urls_and_file_names() {
    run(async {
        rows(&(8..25).collect::<Vec<_>>()).await;
    });
}
/// Canonical attachment MIME types project to the emitted format tags.
#[test]
fn extraction_tool_maps_all_attachment_formats() {
    run(async {
        rows(&(81..87).collect::<Vec<_>>()).await;
    });
}
/// Missing and empty text are rejected outside download retry policy.
#[test]
fn extraction_tool_rejects_unsupported_and_empty_attachments() {
    run(async {
        rows(&(87..91).collect::<Vec<_>>()).await;
    });
}
/// Text and details retain the loader-selected values.
#[test]
fn extraction_tool_retains_result_content_and_details() {
    run(async {
        rows(&[91]).await;
    });
}

use serde::Deserialize;
/// Controlled attachment overrides.
#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Loaded {
    /// Optional extracted text, preserving null versus absence.
    #[serde(default)]
    pub extracted_text: maestro_session::Presence<String>,
    /// Attachment-selected filename.
    pub file_name: Option<String>,
    /// Attachment-selected MIME.
    pub mime_type: Option<String>,
    /// Attachment-selected size.
    pub size: Option<usize>,
}
/// Extraction is not rechecked against cancellation once bytes have been collected.
#[test]
fn extraction_tool_finishes_attachment_load_after_download_cancellation() {
    run(extraction_tool_finishes_attachment_load_after_download_cancellation_case());
}
/// Controlled execution for the synchronous test runtime.
async fn extraction_tool_finishes_attachment_load_after_download_cancellation_case() {
    use futures_util::FutureExt;
    let case = support::corpus().cases.pop().unwrap();
    assert_eq!(case.input.phase.as_deref(), Some("attachment"));
    let effects = Arc::new(Mutex::new(Effects::default()));
    let observed = Arc::clone(&effects);
    let ((entered, ready), (release, released)) = (
        tokio::sync::oneshot::channel(),
        tokio::sync::oneshot::channel(),
    );
    let channels = Mutex::new(Some((entered, released)));
    let tool = build_tool(
        support::fetch(&case.input, &effects),
        Arc::new(move |bytes, name| {
            observed
                .lock()
                .unwrap()
                .loads
                .push(serde_json::json!({"fileName":name,"size":bytes.len()}));
            let (entered, released) = channels.lock().unwrap().take().unwrap();
            Box::pin(async move {
                entered.send(()).unwrap();
                released.await.unwrap();
                Ok(attachment(
                    name,
                    bytes.len(),
                    Loaded {
                        extracted_text: maestro_session::Presence::Present("completed load".into()),
                        ..Loaded::default()
                    },
                ))
            })
        }),
    );
    let execute = Arc::clone(&tool.tool.read().unwrap().execute);
    let signal = Cancellation::new();
    let work = execute(
        String::new(),
        serde_json::json!({"url":case.input.url}),
        Some(signal.clone()),
        None,
    );
    let mut work = std::pin::pin!(work);
    tokio::select! {result=&mut work=>panic!("settled before loading: {}",result.is_ok()),ack=ready=>ack.unwrap()};
    signal.abort();
    assert!(work.as_mut().now_or_never().is_none());
    release.send(()).unwrap();
    let actual = support::outcome(work.await);
    let actual = support::transcript(actual, &effects.lock().unwrap());
    assert_eq!(actual, case.expected);
}

/// Build one controlled attachment using only explicit loader overrides.
fn attachment(name: String, size: usize, loaded: Loaded) -> Attachment {
    Attachment {
        id: String::new(),
        r#type: crate::browser::utils::attachment_utils::AttachmentType::Document,
        file_name: loaded.file_name.unwrap_or(name),
        mime_type: loaded.mime_type.unwrap_or_else(|| "text/plain".into()),
        size: loaded.size.unwrap_or(size),
        content: String::new(),
        extracted_text: match loaded.extracted_text {
            maestro_session::Presence::Missing => Some("seam text".into()),
            maestro_session::Presence::Null => None,
            maestro_session::Presence::Present(text) => Some(text),
        },
        preview: None,
    }
}
