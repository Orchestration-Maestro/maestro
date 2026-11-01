//! JavaScript execution through supplied hidden sandbox effects.
use super::prompts::javascript_repl_tool_description;
use crate::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider;
use crate::browser::components::sandboxed_iframe::{
    SandboxEnvironment, SandboxExecution, SandboxFile, SandboxFileContent, SandboxHost,
    SandboxResult, SandboxUrlProvider, SandboxValue,
};
use crate::utils::lock::read;
use base64::Engine;
use maestro_agent::{AgentTool, AgentToolResult, SharedAgentTool};
use maestro_models::SharedHandle;
use maestro_models::{Cancellation, DiagnosticErrorInfo, Tool};
use maestro_text::is_whitespace_scalar;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Plain-text output and original downloads.
pub struct JavaScriptExecution {
    /// Formatted output.
    pub output: String,
    /// Original downloads, if supplied.
    pub files: Option<Vec<SandboxFile>>,
}
/// Execute authored code through the supplied platform.
///
/// # Errors
/// Returns preflight, execution or display failures.
pub async fn execute_javascript<V: SandboxValue>(
    code: String,
    runtime_providers: Vec<SharedHandle<dyn SandboxRuntimeProvider<V>>>,
    signal: Option<Cancellation>,
    environment: SandboxEnvironment<V>,
) -> Result<JavaScriptExecution, DiagnosticErrorInfo> {
    if code.is_empty() {
        return Err(DiagnosticErrorInfo::error("Code parameter is required"));
    }
    if signal.as_ref().is_some_and(Cancellation::is_aborted) {
        return Err(DiagnosticErrorInfo::error("Execution aborted"));
    }
    let sandbox = environment
        .host
        .create_hidden(environment.sandbox_url_provider)?;
    let file_hint = code.contains("returnFile");
    let result = sandbox
        .execute(SandboxExecution {
            sandbox_id: format!(
                "repl-{}-{}",
                maestro_models::timestamp_now(),
                rand::random::<u64>()
            ),
            code,
            runtime_providers,
            signal,
        })
        .await;
    sandbox.remove();
    let result = result.map_err(|mut error| {
        if error.message.is_empty() {
            error.message = "Execution failed".into();
        }
        error
    })?;
    let output = format_output(file_hint, &result)?;
    Ok(JavaScriptExecution {
        output,
        files: result.files,
    })
}

/// Assemble ordered console, return value and file notices after removal.
fn format_output<V: SandboxValue>(
    file_hint: bool,
    result: &SandboxResult<V>,
) -> Result<String, DiagnosticErrorInfo> {
    use std::fmt::Write;
    let mut output = String::new();
    for entry in &result.console {
        output.push_str(&entry.text);
        output.push('\n');
    }
    if !result.success {
        if !output.is_empty() {
            output.push('\n');
        }
        let error = result.error.as_ref();
        let message = error
            .filter(|error| !error.message.is_empty())
            .map_or("Unknown error", |error| error.message.as_str());
        let stack = error.map_or("", |error| error.stack.as_str());
        let _ = write!(output, "Error: {message}\n{stack}");
        return Err(DiagnosticErrorInfo::error(
            output.trim_matches(is_whitespace_scalar),
        ));
    }
    if let Some(value) = &result.return_value {
        if !output.is_empty() {
            output.push('\n');
        }
        let _ = write!(output, "=> {}", value.format_repl_value()?);
    }
    if let Some(files) = result.files.as_ref().filter(|files| !files.is_empty()) {
        let _ = write!(output, "\n[Files returned: {}]\n", files.len());
        for file in files {
            let _ = writeln!(output, "  - {} ({})", file.file_name, file.mime_type);
        }
    } else if file_hint {
        output.push_str("\n[No files returned - check async operations]");
    }
    let output = output.trim_matches(is_whitespace_scalar);
    if output.is_empty() {
        return Ok("Code executed successfully (no output)".into());
    }
    Ok(output.into())
}

/// Native retained factory of runtime providers.
#[cfg(not(target_arch = "wasm32"))]
pub type RuntimeProvidersFactory<V> = SharedHandle<
    dyn Fn() -> Result<Vec<SharedHandle<dyn SandboxRuntimeProvider<V>>>, DiagnosticErrorInfo>
        + Send
        + Sync,
>;
/// Browser-local retained factory of runtime providers.
#[cfg(target_arch = "wasm32")]
pub type RuntimeProvidersFactory<V> = SharedHandle<
    dyn Fn() -> Result<Vec<SharedHandle<dyn SandboxRuntimeProvider<V>>>, DiagnosticErrorInfo>,
>;
/// Required arguments for execution.
#[derive(Deserialize, Serialize)]
pub struct JavaScriptReplParams {
    /// Active-form display title.
    pub title: String,
    /// Authored code.
    pub code: String,
}
/// Encoded download details.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaScriptReplFile {
    /// Filename with an empty-name fallback.
    pub file_name: String,
    /// MIME type with an empty-type fallback.
    pub mime_type: String,
    /// UTF-8 or binary byte length.
    pub size: usize,
    /// Standard padded base64 payload.
    pub content_base64: String,
}
/// Download details returned by the tool.
#[derive(Serialize)]
pub struct JavaScriptReplToolResult {
    /// Always present for factory executions, including no downloads.
    pub files: Option<Vec<JavaScriptReplFile>>,
}
/// Registered execution with live, independently mutable configuration.
pub struct JavaScriptReplTool<V: SandboxValue = serde_json::Value> {
    /// Shared tool registration handle.
    pub tool: SharedAgentTool,
    /// Factory read afresh for descriptions and execution.
    pub runtime_providers_factory: SharedHandle<RwLock<Option<RuntimeProvidersFactory<V>>>>,
    /// Callback forwarded unchanged to the host.
    pub sandbox_url_provider: SharedHandle<RwLock<Option<SandboxUrlProvider>>>,
}
/// Create an executable tool retaining the supplied sandbox owner.
#[must_use]
pub fn create_javascript_repl_tool<V: SandboxValue>(
    host: Arc<dyn SandboxHost<V>>,
) -> JavaScriptReplTool<V> {
    let default: RuntimeProvidersFactory<V> = SharedHandle::new(|| Ok(vec![]));
    let runtime_providers_factory = SharedHandle::new(RwLock::new(Some(default)));
    let sandbox_url_provider = SharedHandle::new(RwLock::new(None));
    let factory = SharedHandle::clone(&runtime_providers_factory);
    let url = SharedHandle::clone(&sandbox_url_provider);
    let mut tool = AgentTool::typed(
        Tool {
            name: "javascript_repl".into(),
            description: String::new(),
            parameters: maestro_models::ParameterSchema::authored(
                serde_json::json!({"type":"object","required":["title","code"],"properties":{"title":{"type":"string","description":"Brief title describing what the code snippet tries to achieve in active form, e.g. 'Calculating sum'"},"code":{"type":"string","description":"JavaScript code to execute"}}}),
                [
                    (String::new(), maestro_models::SchemaKind::Object),
                    (
                        "/properties/title".to_owned(),
                        maestro_models::SchemaKind::String,
                    ),
                    (
                        "/properties/code".to_owned(),
                        maestro_models::SchemaKind::String,
                    ),
                ],
            ),
        },
        "JavaScript REPL".into(),
        SharedHandle::new(move |_, args: JavaScriptReplParams, signal, _| {
            let providers = runtime_providers(&factory);
            let url = read(&url).clone();
            Box::pin(execute_tool(
                args.code,
                providers,
                signal,
                SandboxEnvironment {
                    host: Arc::clone(&host),
                    sandbox_url_provider: url,
                },
            ))
        }),
    );
    let factory = SharedHandle::clone(&runtime_providers_factory);
    tool.description = Some(SharedHandle::new(move || {
        let descriptions = runtime_providers(&factory)?
            .iter()
            .map(|provider| provider.get_description())
            .collect::<Result<Vec<_>, _>>()?;
        let descriptions = descriptions
            .into_iter()
            .filter(|text| !text.trim_matches(is_whitespace_scalar).is_empty())
            .collect::<Vec<_>>();
        Ok(javascript_repl_tool_description(&descriptions))
    }));
    JavaScriptReplTool {
        tool: SharedHandle::new(RwLock::new(tool)),
        runtime_providers_factory,
        sandbox_url_provider,
    }
}
#[cfg(target_arch = "wasm32")]
impl<V: SandboxValue> JavaScriptReplTool<V> {
    /// The registered tool as an entry of a browser-local session.
    ///
    /// The entry shares this tool's callbacks, so it reads the current runtime providers and
    /// sandbox address on every description and call.
    #[must_use]
    pub fn local_tool(&self) -> SharedAgentTool<maestro_models::Local> {
        use maestro_agent::AgentExecution as _;
        let tool = read(&self.tool);
        std::rc::Rc::new(RwLock::new(maestro_agent::LocalAgentTool {
            definition: maestro_models::Threaded::local_definition(&tool.definition),
            description: tool.description.clone(),
            label: tool.label.clone(),
            prepare_arguments: tool.prepare_arguments.clone(),
            execute: tool.execute.clone(),
            execution_mode: tool.execution_mode,
        }))
    }
}
/// Execute the configured call and encode its downloads.
async fn execute_tool<V: SandboxValue>(
    code: String,
    providers: Result<Vec<SharedHandle<dyn SandboxRuntimeProvider<V>>>, DiagnosticErrorInfo>,
    signal: Option<Cancellation>,
    environment: SandboxEnvironment<V>,
) -> Result<AgentToolResult<JavaScriptReplToolResult>, DiagnosticErrorInfo> {
    let result = execute_javascript(code, providers?, signal, environment).await?;
    let files = result
        .files
        .unwrap_or_default()
        .into_iter()
        .map(encode_file)
        .collect();
    Ok(AgentToolResult::text_result(
        result.output,
        Some(JavaScriptReplToolResult { files: Some(files) }),
    ))
}
/// Invoke the current factory without holding the setting guard.
fn runtime_providers<V: SandboxValue>(
    setting: &RwLock<Option<RuntimeProvidersFactory<V>>>,
) -> Result<Vec<SharedHandle<dyn SandboxRuntimeProvider<V>>>, DiagnosticErrorInfo> {
    let factory = read(setting).clone();
    factory.map_or_else(|| Ok(vec![]), |factory| factory())
}
/// Encode borrowed payload bytes without intermediate binary strings.
fn encode_file(file: SandboxFile) -> JavaScriptReplFile {
    let bytes = match &file.content {
        SandboxFileContent::Text(text) => text.as_bytes(),
        SandboxFileContent::Bytes(bytes) => bytes,
    };
    JavaScriptReplFile {
        size: bytes.len(),
        content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        file_name: if file.file_name.is_empty() {
            "file".into()
        } else {
            file.file_name
        },
        mime_type: if file.mime_type.is_empty() {
            "application/octet-stream".into()
        } else {
            file.mime_type
        },
    }
}
