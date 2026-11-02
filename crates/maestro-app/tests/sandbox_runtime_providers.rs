//! Retained sandbox provider behavior through the application contracts.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support::run;
use maestro_app::browser::components::sandbox::{
    console_runtime_provider::ConsoleRuntimeProvider,
    sandbox_runtime_provider::SandboxRuntimeProvider,
};

#[test]
fn console_provider_defaults_and_runtime() {
    let provider = ConsoleRuntimeProvider::<serde_json::Value>::default();
    assert!(provider.get_data().unwrap().is_empty());
    assert!(provider.get_description().unwrap().is_empty());
    assert!(provider.get_logs().read().unwrap().is_empty());
    assert!(!provider.is_completed());
    assert!(provider.get_completion_error().is_none());
    assert!(
        provider
            .get_runtime()
            .unwrap()
            .starts_with("(_sandboxId) => {")
    );
    assert_eq!(
        provider.get_runtime().unwrap(),
        ConsoleRuntimeProvider::<serde_json::Value>::new()
            .get_runtime()
            .unwrap()
    );
}

use maestro_app::browser::components::sandbox::console_runtime_provider::ConsoleLogType;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Drive one public provider message to settlement.
fn send(provider: &dyn SandboxRuntimeProvider, message: Value) -> Vec<Value> {
    run(exchange(provider, message)).unwrap()
}
/// Await one complete message delivery and retain every ordered reply.
async fn exchange(
    provider: &dyn SandboxRuntimeProvider,
    message: Value,
) -> Result<Vec<Value>, DiagnosticErrorInfo> {
    let replies = Arc::new(Mutex::new(Vec::new()));
    let output = Arc::clone(&replies);
    provider
        .handle_message(
            message,
            Arc::new(move |value| {
                output.lock().unwrap().push(value);
                Ok(())
            }),
        )
        .await?;
    Ok(Arc::try_unwrap(replies).unwrap().into_inner().unwrap())
}

#[test]
fn console_messages_classify_and_acknowledge() {
    let provider = ConsoleRuntimeProvider::new();
    let methods = [
        json!("log"),
        json!("warn"),
        json!("error"),
        json!("info"),
        json!("trace"),
        Value::Null,
        json!(2),
        json!(false),
        json!({}),
        json!([]),
    ];
    let fields = [
        Value::Null,
        json!(false),
        json!(2.5),
        json!("text"),
        json!({"x":1}),
        json!([1, 2]),
    ];
    for method in &methods {
        for raw in &fields {
            assert_eq!(
                send(
                    &provider,
                    json!({"type":"console", "method":method, "text":raw, "args":raw})
                ),
                [json!({"success":true})]
            );
        }
    }
    assert_eq!(
        send(&provider, json!({"type":"console"})),
        [json!({"success":true})]
    );
    let logs = provider.get_logs();
    let logs = logs.read().unwrap();
    assert_eq!(logs.len(), methods.len() * fields.len() + 1);
    for (index, log) in logs[..logs.len() - 1].iter().enumerate() {
        let level = match index / fields.len() {
            1 => ConsoleLogType::Warn,
            2 => ConsoleLogType::Error,
            3 => ConsoleLogType::Info,
            _ => ConsoleLogType::Log,
        };
        assert_eq!(log.r#type, level);
        assert_eq!(log.text, fields[index % fields.len()]);
        assert_eq!(log.args.as_ref(), Some(&fields[index % fields.len()]));
    }
    assert_eq!(logs.last().unwrap().text, Value::Null);
    assert!(logs.last().unwrap().args.is_none());
}

#[test]
fn console_collections_keep_aliases_and_reset_identity() {
    let provider = ConsoleRuntimeProvider::new();
    let original = provider.get_logs();
    send(
        &provider,
        json!({"type":"console", "text":"first", "args":[{"n":1}]}),
    );
    assert!(Arc::ptr_eq(&original, &provider.get_logs()));
    original.write().unwrap()[0].args.as_mut().unwrap()[0]["n"] = json!(2);
    assert_eq!(
        provider.get_logs().read().unwrap()[0].args,
        Some(json!([{"n":2}]))
    );
    provider.reset();
    assert!(!Arc::ptr_eq(&original, &provider.get_logs()));
    assert!(provider.get_logs().read().unwrap().is_empty());
    assert_eq!(original.read().unwrap()[0].text, "first");
}

#[test]
fn console_lifecycle_getters_follow_settlement() {
    let provider = ConsoleRuntimeProvider::new();
    send(&provider, json!({"type":"other", "error":{}}));
    assert!(!provider.is_completed());
    assert!(send(&provider, json!({"type":"execution-complete"})).is_empty());
    assert!(provider.is_completed());
    assert!(provider.get_completion_error().is_none());
    for text in ["failed", "again"] {
        assert!(send(&provider, json!({"type":"execution-error", "error":{"message":text, "stack":"controlled stack"}})).is_empty());
        assert_console_failure(&provider, text, "controlled stack");
    }
    send(&provider, json!({"type":"other"}));
    assert_eq!(provider.get_completion_error().unwrap().message, "again");
    send(&provider, json!({"type":"execution-complete"}));
    assert!(provider.get_completion_error().is_none());
    send(
        &provider,
        json!({"type":"execution-error", "error":{"message":1,"stack":null}}),
    );
    assert_eq!(provider.get_completion_error().unwrap().message, "");
    assert_eq!(provider.get_completion_error().unwrap().stack, "");
    provider.reset();
    assert!(!provider.is_completed());
    assert!(provider.get_completion_error().is_none());
}

use maestro_app::browser::components::sandbox::file_download_runtime_provider::FileDownloadRuntimeProvider;

#[test]
fn download_provider_defaults_and_runtime() {
    let provider = FileDownloadRuntimeProvider::<Value>::default();
    assert!(provider.get_data().unwrap().is_empty());
    assert!(provider.get_files().read().unwrap().is_empty());
    assert_eq!(
        provider.get_description().unwrap(),
        "returnDownloadableFile(filename, content, mimeType?) - Create downloadable file for user (one-time download, not accessible later)"
    );
    assert!(
        provider
            .get_runtime()
            .unwrap()
            .starts_with("(_sandboxId) => {")
    );
    assert_eq!(
        provider.get_runtime().unwrap(),
        FileDownloadRuntimeProvider::<Value>::new()
            .get_runtime()
            .unwrap()
    );
}

#[test]
fn download_messages_collect_raw_content_and_acknowledge() {
    let provider = FileDownloadRuntimeProvider::new();
    let values = [
        json!("plain"),
        Value::Null,
        json!(false),
        json!(3.25),
        json!({"n":1}),
        json!([0, 255, 128]),
    ];
    for value in &values {
        for _ in 0..2 {
            assert_eq!(
                send(
                    &provider,
                    json!({"type":"file-returned", "fileName":value, "mimeType":value, "content":value})
                ),
                [json!({"success":true})]
            );
        }
    }
    send(&provider, json!({"type":"file-returned"}));
    let files = provider.get_files();
    let files = files.read().unwrap();
    assert_eq!(files.len(), values.len() * 2 + 1);
    for (index, file) in files[..files.len() - 1].iter().enumerate() {
        assert_eq!(file.file_name, values[index / 2]);
        assert_eq!(file.mime_type, values[index / 2]);
        assert_eq!(file.content, values[index / 2]);
    }
    assert_eq!(files.last().unwrap().content, Value::Null);
    assert_eq!(files.last().unwrap().file_name, Value::Null);
    assert_eq!(files.last().unwrap().mime_type, Value::Null);
    drop(files);
    send(
        &provider,
        json!({"type":"file-returned", "fileName":"report.csv", "mimeType":"text/csv", "content":"a,b"}),
    );
    let files = provider.get_files();
    let files = files.read().unwrap();
    let report = files.last().unwrap();
    assert_eq!(report.file_name, json!("report.csv"));
    assert_eq!(report.mime_type, json!("text/csv"));
}

#[test]
fn download_collections_keep_aliases_and_reset_identity() {
    let provider = FileDownloadRuntimeProvider::new();
    let original = provider.get_files();
    send(
        &provider,
        json!({"type":"file-returned", "content":{"bytes":[1,2]}}),
    );
    assert!(Arc::ptr_eq(&original, &provider.get_files()));
    original.write().unwrap()[0].content["bytes"][0] = json!(255);
    assert_eq!(
        provider.get_files().read().unwrap()[0].content,
        json!({"bytes":[255,2]})
    );
    provider.reset();
    assert!(!Arc::ptr_eq(&original, &provider.get_files()));
    assert!(provider.get_files().read().unwrap().is_empty());
    assert_eq!(
        original.read().unwrap()[0].content,
        json!({"bytes":[255,2]})
    );
}

#[test]
fn provider_response_failure_keeps_prior_collection() {
    let console = Arc::new(ConsoleRuntimeProvider::new());
    let files = Arc::new(FileDownloadRuntimeProvider::new());
    for (provider, message) in [
        (
            console.as_ref() as &dyn SandboxRuntimeProvider,
            json!({"type":"console", "text":"kept"}),
        ),
        (
            files.as_ref() as &dyn SandboxRuntimeProvider,
            json!({"type":"file-returned", "content":"kept"}),
        ),
    ] {
        assert_immediate_failure(provider, message);
    }
    assert_eq!(console.get_logs().read().unwrap()[0].text, "kept");
    assert_eq!(files.get_files().read().unwrap()[0].content, "kept");
    let observer = Arc::clone(&console);
    let prior = console.get_logs();
    run(console.handle_message(
        json!({"type":"console"}),
        Arc::new(move |_| {
            assert_eq!(observer.get_logs().read().unwrap().len(), 2);
            observer.reset();
            Ok(())
        }),
    ))
    .unwrap();
    assert_eq!(prior.read().unwrap().len(), 2);
    assert!(console.get_logs().read().unwrap().is_empty());
    let observer = Arc::clone(&files);
    let prior = files.get_files();
    run(files.handle_message(
        json!({"type":"file-returned"}),
        Arc::new(move |_| {
            assert_eq!(observer.get_files().read().unwrap().len(), 2);
            observer.reset();
            Ok(())
        }),
    ))
    .unwrap();
    assert_eq!(prior.read().unwrap().len(), 2);
    assert!(files.get_files().read().unwrap().is_empty());
}

use maestro_app::browser::components::sandbox::attachments_runtime_provider::AttachmentsRuntimeProvider;
use maestro_app::browser::utils::attachment_utils::{Attachment, AttachmentType};
use std::sync::RwLock;

/// Source attachments with duplicate identifiers and distinct selected fields.
fn attachments() -> Arc<RwLock<Vec<maestro_app::browser::utils::attachment_utils::SharedAttachment>>>
{
    Arc::new(RwLock::new(
        vec![
            Attachment {
                id: "same".into(),
                r#type: AttachmentType::Image,
                file_name: "image.png".into(),
                mime_type: "image/png".into(),
                size: 4,
                content: "AAH/gA==".into(),
                extracted_text: None,
                preview: Some("preview".into()),
            },
            Attachment {
                id: "same".into(),
                r#type: AttachmentType::Document,
                file_name: "text.txt".into(),
                mime_type: "text/plain".into(),
                size: 0,
                content: String::new(),
                extracted_text: Some(String::new()),
                preview: None,
            },
        ]
        .into_iter()
        .map(|value| Arc::new(RwLock::new(value)))
        .collect(),
    ))
}

#[test]
fn attachments_project_only_runtime_fields() {
    let empty = AttachmentsRuntimeProvider::<Value>::new(Arc::default());
    assert_eq!(
        empty.get_data().unwrap().get("attachments"),
        Some(&json!([]))
    );
    let provider = AttachmentsRuntimeProvider::<Value>::new(attachments());
    let data = provider.get_data().unwrap();
    assert_eq!(data.keys().collect::<Vec<_>>(), ["attachments"]);
    assert_eq!(
        data["attachments"],
        json!([
            {"id":"same", "fileName":"image.png", "mimeType":"image/png", "size":4, "content":"AAH/gA=="},
            {"id":"same", "fileName":"text.txt", "mimeType":"text/plain", "size":0, "content":"", "extractedText":""}
        ])
    );
    assert_eq!(
        data["attachments"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["id", "fileName", "mimeType", "size", "content"]
    );
    assert_eq!(
        data["attachments"][1]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "id",
            "fileName",
            "mimeType",
            "size",
            "content",
            "extractedText"
        ]
    );
}

#[test]
fn attachment_owner_reads_live_input_and_returns_snapshots() {
    use maestro_app::browser::tools::prompts::ATTACHMENTS_RUNTIME_DESCRIPTION;
    let input = attachments();
    let provider = AttachmentsRuntimeProvider::<Value>::new(Arc::clone(&input));
    let before = provider.get_data().unwrap();
    {
        let mut list = input.write().unwrap();
        list[0].write().unwrap().file_name = "changed.png".into();
        list[0].write().unwrap().extracted_text = Some("decoded text".into());
        let next = list[1].clone();
        list.push(next);
    }
    let after = provider.get_data().unwrap();
    assert_eq!(before["attachments"].as_array().unwrap().len(), 2);
    assert_eq!(before["attachments"][0]["fileName"], "image.png");
    assert!(before["attachments"][0].get("extractedText").is_none());
    assert_eq!(after["attachments"].as_array().unwrap().len(), 3);
    assert_eq!(after["attachments"][0]["fileName"], "changed.png");
    assert_eq!(after["attachments"][0]["extractedText"], "decoded text");
    assert_eq!(
        provider.get_description().unwrap(),
        ATTACHMENTS_RUNTIME_DESCRIPTION
    );
}

#[test]
fn unrelated_provider_messages_have_no_effect() {
    let console = ConsoleRuntimeProvider::new();
    let files = FileDownloadRuntimeProvider::new();
    let attachments = AttachmentsRuntimeProvider::new(attachments());
    let messages = vec![
        json!({}),
        json!({"type":null}),
        json!({"type":false}),
        json!({"type":2}),
        json!({"type":[]}),
        json!({"type":{}}),
        json!({"type":"unknown", "args":3}),
    ];
    for message in &messages {
        for provider in [
            &console as &dyn SandboxRuntimeProvider,
            &files,
            &attachments,
        ] {
            assert!(send(provider, message.clone()).is_empty());
        }
    }
    assert!(send(&console, json!({"type":"file-returned"})).is_empty());
    assert!(send(&files, json!({"type":"console"})).is_empty());
    assert!(send(&attachments, json!({"type":"console"})).is_empty());
    assert!(console.get_logs().read().unwrap().is_empty());
    assert!(!console.is_completed());
    assert!(files.get_files().read().unwrap().is_empty());
}

/// Inert presentation; only the provider's injected source is examined.
struct SourceOnly;
impl ArtifactPresentation for SourceOnly {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        unreachable!()
    }
    fn get_or_create_artifact_element(&self, _: &str, _: &str, _: ArtifactsPanelState) {
        unreachable!()
    }
    fn show_artifact(&self, _: &str) {
        unreachable!()
    }
    fn remove_artifact_element(&self, _: &str) {
        unreachable!()
    }
    fn reload_all_html_artifacts(&self, _: ArtifactsPanelState) {
        unreachable!()
    }
    fn get_logs(&self, _: &str) -> Result<ArtifactLogReader, ArtifactLogError> {
        unreachable!()
    }
    fn wait(&self, _: Duration) -> BoxFuture<()> {
        unreachable!()
    }
    fn request_update(&self) {
        unreachable!()
    }
}

#[test]
fn runtime_sources_keep_complete_functions() {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Source {
        name: String,
        source: String,
    }
    let fixture: Vec<Source> =
        serde_json::from_str(include_str!("fixtures/sandbox_runtime_sources.json")).unwrap();
    let console = ConsoleRuntimeProvider::<Value>::new();
    let attachments = AttachmentsRuntimeProvider::<Value>::new(Arc::default());
    let files = FileDownloadRuntimeProvider::<Value>::new();
    let panel = ArtifactsPanel::new(Arc::new(SourceOnly));
    let artifacts = ArtifactsRuntimeProvider::new(&panel, None, false);
    let providers: [(&str, &dyn SandboxRuntimeProvider); 4] = [
        ("ConsoleRuntimeProvider", &console),
        ("AttachmentsRuntimeProvider", &attachments),
        ("FileDownloadRuntimeProvider", &files),
        ("ArtifactsRuntimeProvider", &artifacts),
    ];
    assert_eq!(fixture.len(), providers.len());
    for (expected, (name, provider)) in fixture.iter().zip(providers) {
        assert_eq!(expected.name, name);
        assert_eq!(provider.get_runtime().unwrap(), expected.source);
    }
}

use maestro_app::BoxFuture;
use maestro_app::browser::components::sandbox::artifacts_runtime_provider::{
    ArtifactOperation, ArtifactResponse, ArtifactsRuntimeProvider,
};
use maestro_app::browser::components::sandbox::console_runtime_provider::ConsoleLog;
use maestro_app::browser::components::sandbox::file_download_runtime_provider::DownloadableFile;
use maestro_app::browser::components::sandbox::sandbox_runtime_provider::RuntimeProviderValue;
use maestro_app::browser::components::sandboxed_iframe::{SandboxError, SandboxFile, SandboxValue};
use maestro_app::browser::tools::artifacts::{
    ArtifactLogError, ArtifactLogReader, ArtifactPresentation, ArtifactsPanel, ArtifactsPanelState,
};
use maestro_models::DiagnosticErrorInfo;
use std::time::Duration;

/// A second host retaining a nonserializable shared payload.
#[derive(Clone)]
struct Retained {
    /// Message fields needed by the controlled host.
    fields: Value,
    /// Opaque payload whose identity must survive collection.
    payload: Arc<RwLock<Vec<u8>>>,
}
impl SandboxValue for Retained {
    fn format_repl_value(&self) -> Result<std::borrow::Cow<'_, str>, DiagnosticErrorInfo> {
        panic!("collection must not invoke display")
    }
}
impl Retained {
    /// Select a field while preserving the non-JSON payload handle.
    fn field(&self, name: &str) -> Self {
        Self {
            fields: self.fields.get(name).cloned().unwrap_or_default(),
            payload: Arc::clone(&self.payload),
        }
    }
}
thread_local! {
    /// One-shot adapter action, local to the test invoking the projection.
    static ATTACHMENT_CALLBACK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}
impl RuntimeProviderValue for Retained {
    fn message_type(&self) -> Result<Option<String>, DiagnosticErrorInfo> {
        self.fields.message_type()
    }
    fn project_console(&self) -> Result<ConsoleLog<Self>, DiagnosticErrorInfo> {
        if self.fields["failProjection"] == true {
            return Err(DiagnosticErrorInfo::error("property failed"));
        }
        Ok(ConsoleLog {
            r#type: ConsoleLogType::from_method(self.fields.get("method").and_then(Value::as_str)),
            text: self.field("text"),
            args: self.fields.get("args").map(|_| self.field("args")),
        })
    }
    fn project_download(&self) -> Result<DownloadableFile<Self>, DiagnosticErrorInfo> {
        if self.fields["failProjection"] == true {
            return Err(DiagnosticErrorInfo::error("property failed"));
        }
        Ok(SandboxFile {
            file_name: self.field("fileName"),
            content: self.field("content"),
            mime_type: self.field("mimeType"),
        })
    }
    fn project_completion_error(&self) -> Result<SandboxError, DiagnosticErrorInfo> {
        self.fields.project_completion_error()
    }
    fn success_response() -> Result<Self, DiagnosticErrorInfo> {
        Ok(Self {
            fields: Value::success_response()?,
            payload: Arc::default(),
        })
    }
    fn attachment_array(attachments: &[Attachment]) -> Result<Self, DiagnosticErrorInfo> {
        let callback = ATTACHMENT_CALLBACK.with(|slot| slot.borrow_mut().take());
        if let Some(callback) = callback {
            callback();
        }
        Ok(Self {
            fields: Value::attachment_array(attachments)?,
            payload: Arc::default(),
        })
    }
    fn project_artifact_operation(&self) -> Result<ArtifactOperation, DiagnosticErrorInfo> {
        self.fields.project_artifact_operation()
    }
    fn artifact_snapshot(
        artifacts: indexmap::IndexMap<String, String>,
    ) -> Result<Self, DiagnosticErrorInfo> {
        Ok(Self {
            fields: Value::artifact_snapshot(artifacts)?,
            payload: Arc::default(),
        })
    }
    fn artifact_response(response: ArtifactResponse) -> Result<Self, DiagnosticErrorInfo> {
        Ok(Self {
            fields: Value::artifact_response(response)?,
            payload: Arc::default(),
        })
    }
}

#[test]
fn retained_provider_values_do_not_require_json() {
    let console = ConsoleRuntimeProvider::<Retained>::new();
    let files = FileDownloadRuntimeProvider::<Retained>::new();
    let payload = Arc::new(RwLock::new(vec![0, 255, 128]));
    for (provider, tag) in [
        (&console as &dyn SandboxRuntimeProvider<Retained>, "console"),
        (&files, "file-returned"),
    ] {
        let message = Retained {
            fields: json!({"type":tag,"args":null}),
            payload: Arc::clone(&payload),
        };
        run(provider.handle_message(
            message,
            Arc::new(|reply| {
                assert_eq!(reply.fields, json!({"success":true}));
                Ok(())
            }),
        ))
        .unwrap();
        let bad = Retained {
            fields: json!({"type":tag,"failProjection":true}),
            payload: Arc::clone(&payload),
        };
        let error = run(provider.handle_message(
            bad,
            Arc::new(|_| panic!("failed projection must not reply")),
        ))
        .unwrap_err();
        assert_eq!(error.message, "property failed");
        let irrelevant = Retained {
            fields: json!({"type":"unknown","failProjection":true}),
            payload: Arc::clone(&payload),
        };
        run(provider.handle_message(
            irrelevant,
            Arc::new(|_| panic!("irrelevant message must not reply")),
        ))
        .unwrap();
    }
    let logs = console.get_logs();
    let files = files.get_files();
    assert_eq!(logs.read().unwrap().len(), 1);
    assert_eq!(files.read().unwrap().len(), 1);
    assert!(Arc::ptr_eq(&logs.read().unwrap()[0].text.payload, &payload));
    assert!(Arc::ptr_eq(
        &logs.read().unwrap()[0].args.as_ref().unwrap().payload,
        &payload
    ));
    assert!(Arc::ptr_eq(
        &files.read().unwrap()[0].content.payload,
        &payload
    ));
    payload.write().unwrap().push(1);
    assert_eq!(
        *files.read().unwrap()[0].content.payload.read().unwrap(),
        [0, 255, 128, 1]
    );
}

use maestro_app::browser::components::sandboxed_iframe::{
    SandboxEnvironment, SandboxExecution, SandboxHost, SandboxIframe, SandboxResult,
    SandboxUrlProvider,
};
use maestro_app::browser::tools::javascript_repl::{
    create_javascript_repl_tool, execute_javascript,
};

/// Controlled hidden sandbox observing the real provider request.
#[derive(Clone)]
struct ProviderHost {
    /// Per-run globals, sources and replies in execution order.
    observations: Arc<Mutex<Vec<Value>>>,
}
impl SandboxHost for ProviderHost {
    fn create_hidden(
        &self,
        _: Option<SandboxUrlProvider>,
    ) -> Result<Box<dyn SandboxIframe>, DiagnosticErrorInfo> {
        Ok(Box::new(self.clone()))
    }
}
impl SandboxIframe for ProviderHost {
    fn execute(
        &self,
        request: SandboxExecution,
    ) -> maestro_models::BoxFuture<Result<SandboxResult, DiagnosticErrorInfo>> {
        let observations = Arc::clone(&self.observations);
        Box::pin(async move {
            let mut events = Vec::new();
            for provider in request.runtime_providers {
                events
                    .push(json!({"data":provider.get_data()?, "runtime":provider.get_runtime()?}));
                events.push(json!({"replies":exchange(provider.as_ref(), json!({"type":"console", "text":"host log"})).await?}));
                events.push(json!({"replies":exchange(provider.as_ref(), json!({"type":"file-returned", "content":"host file"})).await?}));
            }
            observations.lock().unwrap().push(json!(events));
            Ok(SandboxResult {
                success: true,
                console: vec![],
                files: None,
                error: None,
                return_value: None,
            })
        })
    }
    fn remove(&self) {}
}

#[test]
fn providers_reach_controlled_sandbox_and_live_descriptions() {
    let host = Arc::new(ProviderHost {
        observations: Arc::default(),
    });
    let console = Arc::new(ConsoleRuntimeProvider::new());
    let files = Arc::new(FileDownloadRuntimeProvider::new());
    let input = attachments();
    let attachments = Arc::new(AttachmentsRuntimeProvider::new(Arc::clone(&input)));
    let providers: Vec<Arc<dyn SandboxRuntimeProvider>> =
        vec![console.clone(), attachments.clone(), files.clone()];
    let tool = create_javascript_repl_tool(host.clone());
    let factory_providers = providers.clone();
    *tool.runtime_providers_factory.write().unwrap() =
        Some(Arc::new(move || Ok(factory_providers.clone())));
    let description = tool.tool.read().unwrap().description.clone().unwrap();
    let expected = maestro_app::browser::tools::prompts::javascript_repl_tool_description(&[
        attachments.get_description().unwrap(),
        files.get_description().unwrap(),
    ]);
    assert_eq!(description().unwrap(), expected);
    for index in 0..2 {
        if index == 1 {
            input.read().unwrap()[0].write().unwrap().file_name = "live.png".into();
        }
        let output = run(execute_javascript(
            "code".into(),
            providers.clone(),
            None,
            SandboxEnvironment {
                host: host.clone(),
                sandbox_url_provider: None,
            },
        ))
        .unwrap();
        assert_eq!(output.output, "Code executed successfully (no output)");
        let observations = host.observations.lock().unwrap();
        assert_host_events(&observations[index], index, &providers);
    }
    assert_eq!(console.get_logs().read().unwrap().len(), 2);
    assert_eq!(files.get_files().read().unwrap().len(), 2);
    *tool.runtime_providers_factory.write().unwrap() = Some(Arc::new(|| Ok(vec![])));
    assert_eq!(
        description().unwrap(),
        maestro_app::browser::tools::prompts::javascript_repl_tool_description(&[])
    );
}

/// Check the failure state without mixing settlement with collection assertions.
fn assert_console_failure(provider: &ConsoleRuntimeProvider, message: &str, stack: &str) {
    assert!(provider.is_completed());
    let error = provider.get_completion_error().unwrap();
    assert_eq!(error.message, message);
    assert_eq!(error.stack, stack);
}
/// Assert the exact ordered observations from one controlled host invocation.
fn assert_host_events(
    observation: &Value,
    index: usize,
    providers: &[Arc<dyn SandboxRuntimeProvider>],
) {
    let events = observation.as_array().unwrap();
    assert_eq!(events.len(), 9);
    assert_eq!(
        events[0],
        json!({"data":{},"runtime":providers[0].get_runtime().unwrap()})
    );
    assert_eq!(events[1], json!({"replies":[{"success":true}]}));
    assert_eq!(events[2], json!({"replies":[]}));
    assert_eq!(
        events[3]["data"]["attachments"][0]["fileName"],
        if index == 0 { "image.png" } else { "live.png" }
    );
    assert_eq!(events[3]["runtime"], providers[1].get_runtime().unwrap());
    assert_eq!(events[4], json!({"replies":[]}));
    assert_eq!(events[5], json!({"replies":[]}));
    assert_eq!(
        events[6],
        json!({"data":{},"runtime":providers[2].get_runtime().unwrap()})
    );
    assert_eq!(events[7], json!({"replies":[]}));
    assert_eq!(events[8], json!({"replies":[{"success":true}]}));
}

/// Observe the reply before polling and preserve the original failure fields.
fn assert_immediate_failure(provider: &dyn SandboxRuntimeProvider, message: Value) {
    let failure = DiagnosticErrorInfo {
        name: Some("ReplyFailure".into()),
        message: "controlled failure".into(),
        stack: Some("original stack".into()),
        code: None,
    };
    let expected = failure.clone();
    let replied = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = Arc::clone(&replied);
    let pending = provider.handle_message(
        message,
        Arc::new(move |_| {
            observed.store(true, std::sync::atomic::Ordering::SeqCst);
            Err(failure.clone())
        }),
    );
    assert!(
        replied.load(std::sync::atomic::Ordering::SeqCst),
        "reply must occur before returning the future"
    );
    let error = run(pending).unwrap_err();
    assert_eq!(error, expected);
}

#[test]
fn attachment_owner_releases_all_guards_before_value_callbacks() {
    let input = attachments();
    let original = Arc::clone(&input.read().unwrap()[0]);
    original.write().unwrap().extracted_text = Some("before callback".into());
    let provider = AttachmentsRuntimeProvider::<Retained>::new(Arc::clone(&input));
    let (list, member) = (Arc::clone(&input), Arc::clone(&original));
    ATTACHMENT_CALLBACK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            let mut members = list.try_write().expect("collection guard released");
            let mut value = member.try_write().expect("member guard released");
            value.file_name = "edited.png".into();
            members.clear();
        }));
    });
    let before = provider
        .get_data()
        .unwrap()
        .shift_remove("attachments")
        .unwrap();
    assert_eq!(
        before.fields[0],
        json!({
            "id":"same", "fileName":"image.png", "mimeType":"image/png",
            "size":4, "content":"AAH/gA==", "extractedText":"before callback"
        })
    );
    assert_eq!(
        provider.get_data().unwrap()["attachments"].fields,
        json!([])
    );
    input.write().unwrap().push(original);
    let after = provider
        .get_data()
        .unwrap()
        .shift_remove("attachments")
        .unwrap();
    assert_eq!(after.fields[0]["fileName"], "edited.png");
    assert_eq!(before.fields[0]["fileName"], "image.png");
    assert!(after.fields[0].get("type").is_none());
    assert!(after.fields[0].get("preview").is_none());
}
