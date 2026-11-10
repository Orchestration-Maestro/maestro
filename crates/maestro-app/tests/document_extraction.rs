//! Document extraction through the embedding interface.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_app::browser::tools::extraction::{ExtractDocumentParams, ExtractDocumentResult};
use maestro_app::{EXTRACT_DOCUMENT_TOOL, Fetch, HttpResponse, create_extract_document_tool};
use maestro_models::{Cancellation, DiagnosticErrorInfo, FetchError};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use support::document_extraction::{Effects, corpus};
use support::run_io as run;

/// Replay public real-loader rows with the selected constructor transport.
async fn rows(indices: &[usize]) {
    for (_, case) in corpus()
        .cases
        .into_iter()
        .enumerate()
        .filter(|(index, _)| indices.contains(index))
    {
        assert!(case.input.real_attachment);
        assert!(case.input.load_error.is_none() && case.input.attachment.is_null());
        let effects = Arc::new(Mutex::new(Effects::default()));
        let tool = create_extract_document_tool(Some(support::document_extraction::fetch(
            &case.input,
            &effects,
        )));
        tool.cors_proxy_url
            .write()
            .unwrap()
            .clone_from(&case.input.proxy);
        let execute = Arc::clone(&tool.tool.read().unwrap().execute);
        support::document_extraction::check(case, execute, effects).await;
    }
}
/// The real loader consumes text bytes rather than response MIME.
#[test]
fn extraction_tool_loads_text_bytes_without_header_mime() {
    run(async {
        rows(&[92, 93, 96]).await;
        presentation_bytes().await;
    });
}
/// Empty text and unsupported suffixes fail through the public factory.
#[test]
fn extraction_tool_rejects_unsupported_and_empty_attachments() {
    run(async {
        rows(&[94, 95]).await;
    });
}
/// Query and fragment are excluded before basename selection.
#[test]
fn extraction_tool_selects_filename_before_query_and_fragment() {
    run(async {
        rows(&[97, 98]).await;
    });
}

/// Public records, ordered schema and independent settings retain their declarations.
#[test]
fn extraction_tool_declares_schema_description_and_owned_settings() {
    run(extraction_tool_declares_schema_description_and_owned_settings_case());
}
/// Controlled execution for the synchronous test runtime.
async fn extraction_tool_declares_schema_description_and_owned_settings_case() {
    let metadata = corpus().metadata.value();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    let fetch: Fetch = Arc::new(move |request| {
        seen.lock().unwrap().push(request.url);
        Box::pin(async {
            Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: [].into(),
                body: Box::pin(futures_util::stream::iter([Ok(b"selected".to_vec())])),
            })
        })
    });
    let first = create_extract_document_tool(Some(fetch));
    let second = create_extract_document_tool(None);
    let execute = check_declaration(&first, &second, &metadata);
    let params = ExtractDocumentParams {
        url: "https://example.test/selected.txt".into(),
    };
    let mut args = serde_json::to_value(params).unwrap();
    args["callerMember"] = json!(true);
    let result = execute(String::new(), args, None, None).await.unwrap();
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        ["https://example.test/selected.txt"]
    );
    assert_eq!(
        result.details.unwrap(),
        serde_json::to_value(ExtractDocumentResult {
            extracted_text: "selected".into(),
            format: "unknown".into(),
            file_name: "selected.txt".into(),
            size: 8
        })
        .unwrap()
    );
}
/// Only this transaction edits default metadata, restoring it before returning.
#[test]
fn extraction_tool_default_instance_is_shared() {
    let metadata = corpus().metadata.value();
    let agent = maestro_agent::Agent::<std::convert::Infallible>::new(
        maestro_agent::AgentOptions::default(),
    );
    agent
        .state()
        .write()
        .unwrap()
        .set_tools(vec![Arc::clone(&EXTRACT_DOCUMENT_TOOL.tool)]);
    let tools = Arc::clone(agent.state().read().unwrap().tools());
    let registered = Arc::clone(&tools.read().unwrap()[0]);
    assert_eq!(
        Arc::ptr_eq(&registered, &EXTRACT_DOCUMENT_TOOL.tool),
        metadata["defaultIsShared"]
    );
    assert_eq!(
        EXTRACT_DOCUMENT_TOOL
            .cors_proxy_url
            .read()
            .unwrap()
            .is_none(),
        metadata["defaultIsPreconstructed"]
    );
    let original = std::mem::replace(
        &mut registered.write().unwrap().label,
        "Registered document".into(),
    );
    let observed = EXTRACT_DOCUMENT_TOOL.tool.read().unwrap().label.clone();
    registered.write().unwrap().label = original;
    assert_eq!(
        observed == "Registered document",
        metadata["defaultRetainsMutableRegistrationAlias"]
    );
}
/// Strict retained-registration oracle.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Ownership {
    /// Recorded size belongs to the controlled attachment loader.
    metadata: OwnershipMetadata,
    /// Ordered authored request targets.
    requests: Vec<Value>,
    /// Full extracted result.
    result: Value,
}
/// Qualification of the retained oracle's controlled-loader result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct OwnershipMetadata {
    /// The fixture's size is not a real-text byte-length expectation.
    size_is_seam_only: bool,
}
/// Execution and settings remain owned without the construction wrapper.
#[test]
fn extraction_tool_settings_survive_registration_and_release() {
    run(extraction_tool_settings_survive_registration_and_release_case());
}
/// Controlled execution for the synchronous test runtime.
async fn extraction_tool_settings_survive_registration_and_release_case() {
    let expected: Ownership =
        serde_json::from_str(include_str!("fixtures/document_extraction/ownership.json")).unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    let fetch: Fetch = Arc::new(move |request| {
        let direct = {
            let mut seen = seen.lock().unwrap();
            let direct = seen.is_empty();
            seen.push(json!({"url":request.url}));
            direct
        };
        Box::pin(async move {
            if direct {
                return Err(FetchError::Connection(DiagnosticErrorInfo::error(
                    "CORS denied",
                )));
            }
            Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: [].into(),
                body: Box::pin(futures_util::stream::iter([Ok(b"seam text".to_vec())])),
            })
        })
    });
    let wrapper = create_extract_document_tool(Some(fetch));
    let execute = Arc::clone(&wrapper.tool.read().unwrap().execute);
    let slot = Arc::clone(&wrapper.cors_proxy_url);
    let weak_tool = Arc::downgrade(&wrapper.tool);
    drop(wrapper);
    assert!(weak_tool.upgrade().is_none());
    *slot.write().unwrap() = Some("https://retained.test/?".into());
    let result = execute(
        String::new(),
        json!({"url":"https://example.test/retained.txt"}),
        Some(Cancellation::new()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(*requests.lock().unwrap(), expected.requests);
    check_ownership_result(result, expected);
    drop(execute);
    assert_eq!(Arc::strong_count(&slot), 1);
}

/// Marks release of pending download work.
struct Released(Arc<std::sync::atomic::AtomicBool>);
impl Drop for Released {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
/// Abort at an acknowledged pending phase and assert immediate settlement.
async fn cancel(case: support::document_extraction::Case) {
    use futures_util::FutureExt;
    let phase = case.input.phase.as_deref().unwrap();
    let proxy = phase == "proxy_body";
    let effects = Arc::new(Mutex::new(Effects::default()));
    let released = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (fetch, ready) = pending_fetch(&effects, &released, phase);
    let tool = create_extract_document_tool(Some(fetch));
    if proxy {
        *tool.cors_proxy_url.write().unwrap() = Some("https://proxy.test/?".into());
    }
    let execute = Arc::clone(&tool.tool.read().unwrap().execute);
    let signal = Cancellation::new();
    let work = execute(
        String::new(),
        json!({"url":case.input.url}),
        Some(signal.clone()),
        None,
    );
    let mut work = std::pin::pin!(work);
    let mut ready = std::pin::pin!(ready);
    tokio::select! {result=&mut work=>panic!("settled before entry: {}",result.is_ok()),ack=&mut ready=>ack.unwrap()};
    signal.abort();
    let result = work
        .as_mut()
        .now_or_never()
        .expect("download remained pending after abort");
    let actual = support::document_extraction::outcome(result);
    let actual = support::document_extraction::transcript(actual, &effects.lock().unwrap());
    assert_eq!(actual, case.expected, "{}", case.name);
    assert!(released.load(std::sync::atomic::Ordering::SeqCst));
}
/// Header and both body phases share the download cancellation owner.
#[test]
fn extraction_tool_cancels_pending_downloads() {
    run(extraction_tool_cancels_pending_downloads_case());
}
/// Controlled execution for the synchronous test runtime.
async fn extraction_tool_cancels_pending_downloads_case() {
    for case in corpus().cases.into_iter().skip(99).take(3) {
        cancel(case).await;
    }
}

/// Native HTTP observations, excluding the private loader call trace.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Native {
    /// Complete server request transcript.
    received: Vec<Value>,
    /// Per-request outcomes.
    cases: Vec<NativeCase>,
}
/// One native request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeCase {
    /// Authored request path.
    path: String,
    /// Complete tool result or error.
    expected: Value,
}
/// The default HTTP adapter reaches the real byte loader and preserves reason text.
#[test]
fn extraction_tool_http_adapter_downloads_controlled_text() {
    run(extraction_tool_http_adapter_downloads_controlled_text_case());
}
/// Controlled execution for the synchronous test runtime.
async fn extraction_tool_http_adapter_downloads_controlled_text_case() {
    use std::io::{Read, Write};
    let expected: Native = serde_json::from_str(include_str!(
        "fixtures/document_extraction/native-http.json"
    ))
    .unwrap();
    let server = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = server.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let mut received = Vec::new();
        for (index, connection) in server.incoming().take(2).enumerate() {
            let mut connection = connection.unwrap();
            let mut bytes = Vec::new();
            let mut byte = [0];
            while !bytes.ends_with(b"\r\n\r\n") {
                connection.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
            }
            let text = String::from_utf8(bytes).unwrap();
            let mut first = text.lines().next().unwrap().split_whitespace();
            received.push(json!({"method":first.next().unwrap(),"path":first.next().unwrap()}));
            let (status, body) = if index == 0 {
                ("200 OK", "native résumé\ntext")
            } else {
                ("403 Controlled denial", "")
            };
            write!(connection,"HTTP/1.1 {status}\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
        received
    });
    let tool = create_extract_document_tool(None);
    let execute = Arc::clone(&tool.tool.read().unwrap().execute);
    let mut observations = Vec::new();
    for case in &expected.cases {
        observations.push(support::document_extraction::outcome(
            execute(
                String::new(),
                json!({"url":format!("http://{address}{}",case.path)}),
                None,
                None,
            )
            .await,
        ));
    }
    let received = thread.join().unwrap();
    assert_eq!(received, expected.received);
    for (case, actual) in expected.cases.into_iter().zip(observations) {
        assert_eq!(actual, case.expected);
    }
}

/// Validate declaration and independent slots without retaining a guard across execution.
fn check_declaration(
    first: &maestro_app::browser::tools::extraction::ExtractDocumentTool,
    second: &maestro_app::browser::tools::extraction::ExtractDocumentTool,
    metadata: &Value,
) -> maestro_agent::ExecuteTool {
    let definition = first.tool.read().unwrap();
    assert_eq!(definition.label, metadata["label"]);
    assert_eq!(definition.definition.name, metadata["name"]);
    assert_eq!(definition.definition.description, metadata["description"]);
    assert_eq!(
        serde_json::to_string(&definition.definition.parameters).unwrap(),
        serde_json::to_string(&metadata["parameters"]).unwrap()
    );
    assert!(definition.prepare_arguments.is_none() && definition.execution_mode.is_none());
    let execute = Arc::clone(&definition.execute);
    assert_eq!(
        first.cors_proxy_url.read().unwrap().is_none(),
        metadata["proxyInitiallyAbsent"]
    );
    assert_eq!(
        !Arc::ptr_eq(&first.tool, &second.tool),
        metadata["factoriesAreDistinct"]
    );
    assert_eq!(
        !Arc::ptr_eq(&first.cors_proxy_url, &second.cors_proxy_url),
        metadata["factorySettingsIndependent"]
    );
    execute
}

/// Install controlled pending transport work with an explicit entry acknowledgment.
fn pending_fetch(
    effects: &Arc<Mutex<Effects>>,
    released: &Arc<std::sync::atomic::AtomicBool>,
    phase: &str,
) -> (Fetch, tokio::sync::oneshot::Receiver<()>) {
    let proxy = phase == "proxy_body";
    let headers = phase == "headers";
    let observed = Arc::clone(effects);
    let release = Arc::clone(released);
    let (entered, ready) = tokio::sync::oneshot::channel();
    let entered = Arc::new(Mutex::new(Some(entered)));
    let fetch: Fetch = Arc::new(move |request| {
        let direct = {
            let mut observed = observed.lock().unwrap();
            let direct = observed.requests.is_empty();
            observed
                .requests
                .push((request.url, request.signal.unwrap()));
            direct
        };
        let entered = if proxy && direct {
            None
        } else {
            entered.lock().unwrap().take()
        };
        let release = Arc::clone(&release);
        let observed = Arc::clone(&observed);
        Box::pin(async move {
            if proxy && direct {
                return Err(FetchError::Connection(DiagnosticErrorInfo::error(
                    "CORS denied",
                )));
            }
            let body = futures_util::stream::once(pending_body(entered, release));
            if headers {
                let _ = futures_util::StreamExt::into_future(Box::pin(body)).await;
                unreachable!();
            }
            observed
                .lock()
                .unwrap()
                .bodies
                .push(if proxy { "proxy" } else { "direct" }.into());
            Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: [].into(),
                body: Box::pin(body),
            })
        })
    });
    (fetch, ready)
}
/// Keep a drop witness alive while download work remains pending.
async fn pending_body(
    entered: Option<tokio::sync::oneshot::Sender<()>>,
    release: Arc<std::sync::atomic::AtomicBool>,
) -> Result<Vec<u8>, FetchError> {
    let _released = Released(release);
    entered.unwrap().send(()).unwrap();
    futures_util::future::pending().await
}

/// Landed presentation loading is reached through the document executable.
async fn presentation_bytes() {
    let fetch: Fetch = Arc::new(|request| {
        assert_eq!(request.url, "https://example.test/deck.pptx");
        Box::pin(async {
            Ok(HttpResponse {
                status: 200,
                status_text: "OK".into(),
                headers: [("content-type".into(), "text/plain".into())].into(),
                body: Box::pin(futures_util::stream::iter([Ok(include_bytes!(
                    "fixtures/pptx/packages/01.pptx"
                )
                .to_vec())])),
            })
        })
    });
    let tool = create_extract_document_tool(Some(fetch));
    let execute = Arc::clone(&tool.tool.read().unwrap().execute);
    let result = execute(
        String::new(),
        json!({"url":"https://example.test/deck.pptx"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = "<pptx filename=\"deck.pptx\">\n<slide number=\"1\">\nhello\n</slide>\n</pptx>";
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({"content":[{"type":"text","text":text}],"details":{"extractedText":text,"format":"pptx","fileName":"deck.pptx","size":251}})
    );
}

/// Compare real-loader ownership output without treating seam size as byte policy.
fn check_ownership_result(result: maestro_agent::AgentToolResult, expected: Ownership) {
    let mut actual = serde_json::to_value(result).unwrap();
    let actual_size = actual["details"]
        .as_object_mut()
        .unwrap()
        .shift_remove("size")
        .unwrap();
    assert_eq!(actual_size, 9);
    assert!(expected.metadata.size_is_seam_only);
    let mut expected_result = expected.result;
    let seam_size = expected_result["details"]
        .as_object_mut()
        .unwrap()
        .shift_remove("size")
        .unwrap();
    assert_eq!(seam_size, 17);
    assert_eq!(actual, expected_result);
}
