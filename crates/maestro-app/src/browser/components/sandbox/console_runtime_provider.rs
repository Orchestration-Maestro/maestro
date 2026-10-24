//! Captured console values and live execution settlement.
//!
//! ```
//! use maestro_app::browser::components::sandbox::{
//!     console_runtime_provider::ConsoleRuntimeProvider,
//!     sandbox_runtime_provider::SandboxRuntimeProvider,
//! };
//!
//! let console = ConsoleRuntimeProvider::<serde_json::Value>::new();
//! let earlier_logs = console.get_logs();
//! console.reset();
//! assert!(!std::sync::Arc::ptr_eq(&earlier_logs, &console.get_logs()));
//! assert!(console.get_data()?.is_empty());
//! # Ok::<(), maestro_models::DiagnosticErrorInfo>(())
//! ```
use super::providers::{RuntimeProviderValue, SandboxRespond, SandboxRuntimeProvider};
use crate::browser::components::sandboxed_iframe::{SandboxError, SandboxValue};
use crate::utils::lock::{read, write};
use indexmap::IndexMap;
use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use std::sync::{Arc, RwLock};

/// The four console categories exposed by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleLogType {
    /// Ordinary console output.
    Log,
    /// Warning output.
    Warn,
    /// Error output.
    Error,
    /// Informational output.
    Info,
}
impl ConsoleLogType {
    /// Select a supported method, falling back to ordinary output.
    #[must_use]
    pub fn from_method(method: Option<&str>) -> Self {
        match method {
            Some("warn") => Self::Warn,
            Some("error") => Self::Error,
            Some("info") => Self::Info,
            _ => Self::Log,
        }
    }
}
/// One entry retaining the original message fields.
pub struct ConsoleLog<V: SandboxValue = serde_json::Value> {
    /// Selected console category.
    pub r#type: ConsoleLogType,
    /// Unformatted message text.
    pub text: V,
    /// Supplied arguments, distinct from absence.
    pub args: Option<V>,
}
/// Current collection identity and execution settlement.
struct ConsoleState<V: SandboxValue> {
    /// Caller-visible live collection.
    logs: Arc<RwLock<Vec<ConsoleLog<V>>>>,
    /// Whether a settlement message has arrived.
    completed: bool,
    /// Most recently reported execution failure.
    error: Option<Arc<SandboxError>>,
}
/// Captures console messages without formatting their payloads.
pub struct ConsoleRuntimeProvider<V: RuntimeProviderValue = serde_json::Value> {
    /// One retained state slot shared by callers.
    state: RwLock<ConsoleState<V>>,
}
impl<V: RuntimeProviderValue> Default for ConsoleRuntimeProvider<V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<V: RuntimeProviderValue> ConsoleRuntimeProvider<V> {
    /// Create an empty, unsettled capture.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RwLock::new(ConsoleState {
                logs: Arc::default(),
                completed: false,
                error: None,
            }),
        }
    }
    /// Retain the current live collection, including caller edits.
    #[must_use]
    pub fn get_logs(&self) -> Arc<RwLock<Vec<ConsoleLog<V>>>> {
        Arc::clone(&read(&self.state).logs)
    }
    /// Whether execution has reported success or failure.
    #[must_use]
    pub fn is_completed(&self) -> bool {
        read(&self.state).completed
    }
    /// Retain the current failure without copying its contents.
    #[must_use]
    pub fn get_completion_error(&self) -> Option<Arc<SandboxError>> {
        read(&self.state).error.clone()
    }
    /// Replace the collection and clear settlement; earlier aliases remain valid.
    pub fn reset(&self) {
        let old = std::mem::replace(
            &mut *write(&self.state),
            ConsoleState {
                logs: Arc::default(),
                completed: false,
                error: None,
            },
        );
        drop(old);
    }
    /// Perform the synchronous message effects before returning their ready result.
    fn collect_message(
        &self,
        message: &V,
        respond: &SandboxRespond<V>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let tag = message.message_type()?;
        match tag.as_deref() {
            Some("console") => {
                let entry = message.project_console()?;
                let logs = Arc::clone(&read(&self.state).logs);
                write(&logs).push(entry);
                respond(V::success_response()?)?;
            }
            Some("execution-complete" | "execution-error") => {
                let error = (tag.as_deref() == Some("execution-error"))
                    .then(|| message.project_completion_error())
                    .transpose()?
                    .map(Arc::new);
                let mut state = write(&self.state);
                state.completed = true;
                state.error = error;
            }
            _ => {}
        }
        Ok(())
    }
}
impl<V: RuntimeProviderValue> SandboxRuntimeProvider<V> for ConsoleRuntimeProvider<V> {
    fn handle_message(
        &self,
        message: V,
        respond: SandboxRespond<V>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(self.collect_message(&message, &respond)))
    }

    fn get_data(&self) -> Result<IndexMap<String, V>, DiagnosticErrorInfo> {
        Ok(IndexMap::new())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(include_str!("console_runtime_provider/runtime.js").into())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
