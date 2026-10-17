//! Owned sandbox values and supplied execution effects.
use super::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider;
use maestro_agent::agent::Native;
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo, SharedHandle};
use serde_json::Value;
use std::{borrow::Cow, sync::Arc};

/// A retained host value that can supply its REPL display.
pub trait SandboxValue: Native + 'static {
    /// Format the value without requiring a JSON transport.
    ///
    /// # Errors
    /// Returns the host's display failure.
    fn format_repl_value(&self) -> Result<Cow<'_, str>, DiagnosticErrorInfo>;
}
impl SandboxValue for Value {
    fn format_repl_value(&self) -> Result<Cow<'_, str>, DiagnosticErrorInfo> {
        if let Self::String(text) = self {
            return Ok(Cow::Borrowed(text));
        }
        let text = match self {
            Self::Null | Self::Object(_) | Self::Array(_) => maestro_json::pretty_json(self),
            _ => maestro_json::compact_json(self),
        };
        text.map(Cow::Owned)
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
    }
}
/// File payload supplied by the sandbox.
#[derive(Clone, Debug, PartialEq)]
pub enum SandboxFileContent {
    /// UTF-8 text.
    Text(String),
    /// Uninterpreted bytes.
    Bytes(Vec<u8>),
}
/// Download produced by execution.
#[derive(Clone, Debug, PartialEq)]
pub struct SandboxFile<C = SandboxFileContent, M = String> {
    /// Original filename.
    pub file_name: M,
    /// Original payload.
    pub content: C,
    /// Original MIME type.
    pub mime_type: M,
}
/// One captured console entry.
pub struct SandboxConsoleEntry {
    /// Open console category.
    pub r#type: String,
    /// Captured text.
    pub text: String,
}
/// Sandbox-reported failure.
pub struct SandboxError {
    /// Error text, possibly empty.
    pub message: String,
    /// Stack text, possibly empty.
    pub stack: String,
}
/// Settled sandbox result with a host-owned return value.
pub struct SandboxResult<V: SandboxValue = Value, C = SandboxFileContent> {
    /// Whether execution succeeded.
    pub success: bool,
    /// Console entries in emission order.
    pub console: Vec<SandboxConsoleEntry>,
    /// Optional downloads.
    pub files: Option<Vec<SandboxFile<C>>>,
    /// Optional failure details.
    pub error: Option<SandboxError>,
    /// Absent return is distinct from a returned null.
    pub return_value: Option<V>,
}
/// Native callback retaining URL selection and its failure.
#[cfg(not(target_arch = "wasm32"))]
pub type SandboxUrlProvider =
    maestro_models::SharedHandle<dyn Fn() -> Result<String, DiagnosticErrorInfo> + Send + Sync>;
/// Browser-local callback retaining URL selection and its failure.
#[cfg(target_arch = "wasm32")]
pub type SandboxUrlProvider =
    maestro_models::SharedHandle<dyn Fn() -> Result<String, DiagnosticErrorInfo>>;
/// Owned arguments passed to a hidden sandbox.
pub struct SandboxExecution<V: SandboxValue = Value> {
    /// Distinguishing execution identifier.
    pub sandbox_id: String,
    /// Authored JavaScript.
    pub code: String,
    /// Retained runtime providers.
    pub runtime_providers: Vec<SharedHandle<dyn SandboxRuntimeProvider<V>>>,
    /// Caller cancellation handle.
    pub signal: Option<Cancellation>,
}
/// Supplied hidden sandbox lifecycle.
pub trait SandboxIframe<V: SandboxValue = Value>: Native {
    /// Run code to settlement.
    fn execute(
        &self,
        request: SandboxExecution<V>,
    ) -> BoxFuture<Result<SandboxResult<V>, DiagnosticErrorInfo>>;
    /// Remove the hidden sandbox after settlement.
    fn remove(&self);
}
/// Platform owner of creation and hidden insertion.
pub trait SandboxHost<V: SandboxValue = Value>: Native {
    /// Create and insert a hidden sandbox.
    ///
    /// # Errors
    /// Returns the platform creation failure.
    fn create_hidden(
        &self,
        url: Option<SandboxUrlProvider>,
    ) -> Result<Box<dyn SandboxIframe<V>>, DiagnosticErrorInfo>;
}
/// Supplied effects for one execution.
pub struct SandboxEnvironment<V: SandboxValue = Value> {
    /// Platform sandbox owner.
    pub host: Arc<dyn SandboxHost<V>>,
    /// Optional retained URL callback.
    pub sandbox_url_provider: Option<SandboxUrlProvider>,
}
